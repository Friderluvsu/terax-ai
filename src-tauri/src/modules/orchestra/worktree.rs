use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use super::run::valid_id;

const GIT_TIMEOUT_SECS: u64 = 120;

#[derive(Clone, Debug, PartialEq)]
pub struct TaskPaths {
    pub dir: PathBuf,
    pub worktree: PathBuf,
    pub target: PathBuf,
    pub brief: PathBuf,
}

/// Every task path is derived here, so a crafted id can never point a
/// worktree, brief or cargo target outside its run directory.
pub fn task_paths(root: &Path, run_id: &str, task_id: &str) -> Result<TaskPaths, String> {
    if !valid_id(run_id) || !valid_id(task_id) {
        return Err("invalid run or task id".to_string());
    }
    let run_dir = root.join(run_id);
    let dir = run_dir.join(task_id);
    let paths = TaskPaths {
        worktree: dir.join("worktree"),
        target: dir.join("target"),
        brief: dir.join("brief.md"),
        dir,
    };
    for path in [&paths.dir, &paths.worktree, &paths.target, &paths.brief] {
        if !is_strictly_under(path, &run_dir) {
            return Err(format!("task path escapes the run: {}", path.display()));
        }
    }
    Ok(paths)
}

fn is_strictly_under(path: &Path, base: &Path) -> bool {
    path != base
        && path.starts_with(base)
        && path
            .strip_prefix(base)
            .is_ok_and(|rest| rest.components().all(|c| matches!(c, Component::Normal(_))))
}

pub fn branch_name(task_id: &str) -> String {
    format!("task/{task_id}")
}

pub fn repo_toplevel(cwd: &Path) -> Result<PathBuf, String> {
    let out = git(cwd, &["rev-parse", "--show-toplevel"])?;
    let top = PathBuf::from(out.trim());
    std::fs::canonicalize(&top).map_err(|error| format!("resolve repository: {error}"))
}

pub fn current_base(repo: &Path) -> Result<String, String> {
    match git(repo, &["symbolic-ref", "--quiet", "--short", "HEAD"]) {
        Ok(branch) if !branch.trim().is_empty() => Ok(branch.trim().to_string()),
        _ => git(repo, &["rev-parse", "--verify", "HEAD"]).map(|sha| sha.trim().to_string()),
    }
}

pub fn create(repo: &Path, base: &str, branch: &str, paths: &TaskPaths) -> Result<(), String> {
    if base.starts_with('-') || branch.starts_with('-') {
        return Err("refusing an option-like git ref".to_string());
    }
    std::fs::create_dir_all(&paths.target)
        .map_err(|error| format!("create cargo target dir: {error}"))?;
    if paths.worktree.exists() {
        return Err(format!(
            "worktree path already exists: {}",
            paths.worktree.display()
        ));
    }
    let worktree = paths
        .worktree
        .to_str()
        .ok_or_else(|| "worktree path is not valid UTF-8".to_string())?;
    git(
        repo,
        &["worktree", "add", "--quiet", "-b", branch, worktree, base],
    )
    .map(|_| ())
}

fn git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let mut cmd = Command::new("git");
    cmd.args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::modules::proc::hide_console(&mut cmd);
    let child = cmd.spawn().map_err(|error| format!("run git: {error}"))?;
    let output = wait_with_timeout(child)?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git {}: {}", args[0], stderr.trim()));
    }
    String::from_utf8(output.stdout).map_err(|_| "git returned non-UTF-8 output".to_string())
}

fn wait_with_timeout(child: std::process::Child) -> Result<std::process::Output, String> {
    let pid = child.id();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(child.wait_with_output());
    });
    match rx.recv_timeout(std::time::Duration::from_secs(GIT_TIMEOUT_SECS)) {
        Ok(result) => result.map_err(|error| format!("wait for git: {error}")),
        Err(_) => {
            kill_pid(pid);
            Err(format!("git timed out after {GIT_TIMEOUT_SECS}s"))
        }
    }
}

#[cfg(unix)]
fn kill_pid(pid: u32) {
    if let Ok(pid) = libc::pid_t::try_from(pid) {
        unsafe {
            libc::kill(pid, libc::SIGKILL);
        }
    }
}

#[cfg(windows)]
fn kill_pid(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/F", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_paths_stay_strictly_under_the_run_dir() {
        let root = Path::new("/data/orchestra");
        let paths = task_paths(root, "a1b2", "a1b2-3").unwrap();
        let run_dir = root.join("a1b2");
        for path in [&paths.dir, &paths.worktree, &paths.target, &paths.brief] {
            assert!(path.starts_with(&run_dir), "{}", path.display());
            assert_ne!(path, &run_dir);
        }
        assert_eq!(paths.worktree, run_dir.join("a1b2-3").join("worktree"));
        for bad in ["..", "../x", "a/..", "/abs", "", "x\\..\\y"] {
            assert!(task_paths(root, "a1b2", bad).is_err(), "{bad:?}");
            assert!(task_paths(root, bad, "a1b2-1").is_err(), "{bad:?}");
        }
    }

    #[test]
    fn strictly_under_rejects_parent_components() {
        let base = Path::new("/r/run");
        assert!(is_strictly_under(Path::new("/r/run/t/worktree"), base));
        assert!(!is_strictly_under(Path::new("/r/run"), base));
        assert!(!is_strictly_under(Path::new("/r/run/../other"), base));
        assert!(!is_strictly_under(Path::new("/r/other"), base));
    }

    fn init_repo(dir: &Path) {
        for args in [
            vec!["init", "--quiet", "-b", "main"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "t"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "init"],
        ] {
            git(dir, &args).unwrap();
        }
    }

    #[test]
    fn creates_a_task_branch_from_the_current_branch_and_keeps_it() {
        let repo = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        init_repo(repo.path());
        git(repo.path(), &["checkout", "--quiet", "-b", "feature"]).unwrap();
        git(
            repo.path(),
            &["commit", "--quiet", "--allow-empty", "-m", "f"],
        )
        .unwrap();

        let top = repo_toplevel(repo.path()).unwrap();
        let base = current_base(&top).unwrap();
        assert_eq!(base, "feature");
        let paths = task_paths(data.path(), "abcd", "abcd-1").unwrap();
        create(&top, &base, &branch_name("abcd-1"), &paths).unwrap();

        assert!(paths.worktree.join(".git").exists());
        assert!(paths.target.is_dir());
        let head = git(&paths.worktree, &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap();
        assert_eq!(head.trim(), "task/abcd-1");
        let fork = git(&paths.worktree, &["rev-parse", "HEAD"]).unwrap();
        let tip = git(&top, &["rev-parse", "feature"]).unwrap();
        assert_eq!(fork, tip);

        let err = create(&top, &base, &branch_name("abcd-1"), &paths).unwrap_err();
        assert!(err.contains("already exists"), "{err}");
        assert!(paths.worktree.join(".git").exists());
    }

    #[test]
    fn refuses_option_like_refs() {
        let data = tempfile::tempdir().unwrap();
        let paths = task_paths(data.path(), "abcd", "abcd-1").unwrap();
        assert!(create(data.path(), "--upload-pack=x", "task/a", &paths).is_err());
    }
}
