use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const CHECK_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const POLL: Duration = Duration::from_millis(250);
pub const RESULT_MAX_BYTES: u64 = 16 * 1024;
pub const LOG_TAIL_BYTES: u64 = 6 * 1024;

/// Package registries a worker may reach from sandboxed shell commands. Git
/// hosts are left out so a worker cannot push even through `git -C . push`.
const REGISTRY_DOMAINS: &[&str] = &[
    "registry.npmjs.org",
    "registry.yarnpkg.com",
    "crates.io",
    "index.crates.io",
    "static.crates.io",
    "pypi.org",
    "files.pythonhosted.org",
    "proxy.golang.org",
    "sum.golang.org",
];

const DENIED_GIT: &[&str] = &[
    "Bash(git push *)",
    "Bash(git merge *)",
    "Bash(git rebase *)",
    "Bash(git reset --hard *)",
    "Bash(git branch -D *)",
    "Bash(git worktree *)",
    "Bash(git checkout main*)",
    "Bash(git switch *)",
];

/// Claude Code settings for a worker: edits and sandboxed shell commands run
/// without prompts, writes stay in the worktree and task dir, git history
/// outside the task branch is off limits.
pub fn claude_settings(task_dir: &Path) -> Value {
    let dir = task_dir.to_string_lossy();
    json!({
        "permissions": {
            "additionalDirectories": [dir],
            "deny": DENIED_GIT,
        },
        "sandbox": {
            "enabled": true,
            "autoAllowBashIfSandboxed": true,
            "filesystem": { "allowWrite": [dir] },
            "network": { "allowedDomains": REGISTRY_DOMAINS },
        },
    })
}

pub struct CheckJob {
    pub task: String,
    pub command: String,
    pub cwd: PathBuf,
    pub target: PathBuf,
    pub log: PathBuf,
}

/// Runs the run's check command in the task worktree; output goes to a log.
pub fn run(job: &CheckJob) -> Result<bool, String> {
    let log = std::fs::File::create(&job.log).map_err(|e| format!("create check log: {e}"))?;
    let err = log
        .try_clone()
        .map_err(|e| format!("clone check log: {e}"))?;
    let cwd = job.cwd.to_string_lossy().into_owned();
    let mut cmd = crate::modules::shell::build_oneshot_command(
        &job.command,
        &crate::modules::workspace::WorkspaceEnv::Local,
        Some(&cwd),
    )?;
    cmd.current_dir(&job.cwd)
        .envs(crate::modules::lsp::env::server_env_overlay())
        .env("CARGO_TARGET_DIR", &job.target)
        .env("TERAX_TASK_ID", &job.task)
        .env("CI", "1")
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(err);
    crate::modules::proc::hide_console(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("start checks: {e}"))?;
    let started = Instant::now();
    loop {
        if let Some(status) = child.try_wait().map_err(|e| format!("wait checks: {e}"))? {
            return Ok(status.success());
        }
        if started.elapsed() >= CHECK_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "checks timed out after {}s",
                CHECK_TIMEOUT.as_secs()
            ));
        }
        std::thread::sleep(POLL);
    }
}

/// Reads at most `max` bytes from the end of a file, lossily as UTF-8.
pub fn read_tail(path: &Path, max: u64) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let len = file.metadata().ok()?.len();
    let start = len.saturating_sub(max);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.take(max).read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let text = if start > 0 {
        text.split_once('\n').map_or(&*text, |(_, rest)| rest)
    } else {
        &text
    };
    Some(text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_settings_sandbox_writes_and_block_history_rewrites() {
        let settings = claude_settings(Path::new("/data/run/task"));
        assert_eq!(settings["sandbox"]["enabled"], true);
        assert_eq!(settings["sandbox"]["autoAllowBashIfSandboxed"], true);
        assert_eq!(
            settings["sandbox"]["filesystem"]["allowWrite"],
            json!(["/data/run/task"])
        );
        let domains = settings["sandbox"]["network"]["allowedDomains"]
            .as_array()
            .unwrap();
        assert!(domains
            .iter()
            .all(|d| !d.as_str().unwrap().contains("github")));
        let deny = settings["permissions"]["deny"].as_array().unwrap();
        assert!(deny.contains(&json!("Bash(git push *)")));
        assert!(deny.contains(&json!("Bash(git merge *)")));
    }

    #[cfg(unix)]
    #[test]
    fn checks_report_exit_status_and_capture_output() {
        let dir = tempfile::tempdir().unwrap();
        let job = |command: &str| CheckJob {
            task: "abcd-1".into(),
            command: command.into(),
            cwd: dir.path().to_path_buf(),
            target: dir.path().join("target"),
            log: dir.path().join("check.log"),
        };
        assert_eq!(run(&job("echo ok-$TERAX_TASK_ID")), Ok(true));
        assert_eq!(
            read_tail(&dir.path().join("check.log"), 100).as_deref(),
            Some("ok-abcd-1\n")
        );
        assert_eq!(run(&job("echo broken >&2; exit 3")), Ok(false));
        assert!(read_tail(&dir.path().join("check.log"), 100)
            .unwrap()
            .contains("broken"));
    }

    #[test]
    fn tail_reads_only_the_end_and_starts_on_a_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        std::fs::write(&path, "first line\nsecond\nthird\n").unwrap();
        assert_eq!(read_tail(&path, 12).as_deref(), Some("third\n"));
        assert_eq!(
            read_tail(&path, 1000).as_deref(),
            Some("first line\nsecond\nthird\n")
        );
        assert!(read_tail(&dir.path().join("missing"), 10).is_none());
    }
}
