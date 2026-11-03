use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use terax_control_protocol::{validate_brief, validate_title};

use super::catalog::{self, AgentCatalog};

pub const PLANNER_AGENTS: &[&str] = &["claude", "codex", "gemini"];
pub const MAX_JOB_BYTES: usize = 8 * 1024;
const MAX_PLANNED_TASKS: usize = 8;
const MAX_OUTPUT_BYTES: usize = 512 * 1024;
const PLAN_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const POLL: Duration = Duration::from_millis(200);

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PlannedTask {
    pub title: String,
    pub brief: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
}

pub fn prompt(job: &str, catalog: &[AgentCatalog]) -> String {
    format!(
        "You plan work for parallel coding agents. Each task runs in its own git worktree, \
so tasks must be independent and should touch different files. Read the repository \
only as much as you need; do not modify anything. Split the job into 1 to 6 tasks \
(one task when the job is small). For each task give a short title and a brief that \
states the goal, the files or area to change and what not to touch, and how to \
verify it. Also pick the agent and model that should run each task from the list \
below: use a cheap, fast model for mechanical edits, a balanced one for ordinary \
feature work, and the strongest for hard design or tricky bugs. Use an agent and \
model exactly as written, or omit \"model\" to use the agent's default.\n\n\
Available:\n{available}\n\
Reply with only a JSON array, no prose: \
[{{\"title\": \"...\", \"brief\": \"...\", \"agent\": \"...\", \"model\": \"...\"}}]\n\nJob:\n{job}",
        available = catalog::describe(catalog),
    )
}

/// The read-only headless invocation of each planner CLI. The prompt is the
/// last argument, never interpolated into a shell string.
pub fn command(agent: &str, prompt: &str, last_message: &Path) -> Option<(String, Vec<String>)> {
    let prompt = prompt.to_string();
    let args = match agent {
        "claude" => vec![
            "-p".into(),
            "--permission-mode".into(),
            "plan".into(),
            "--output-format".into(),
            "text".into(),
            prompt,
        ],
        "codex" => vec![
            "exec".into(),
            "--sandbox".into(),
            "read-only".into(),
            "--skip-git-repo-check".into(),
            "--output-last-message".into(),
            last_message.to_string_lossy().into_owned(),
            prompt,
        ],
        // Non-interactive Gemini denies tools that need approval, so edits never run.
        "gemini" => vec!["-p".into(), prompt],
        _ => return None,
    };
    Some((agent.to_string(), args))
}

/// Pulls the task list out of a planner reply: the last JSON array in the
/// text, tolerating code fences and surrounding prose.
pub fn parse(reply: &str, catalog: &[AgentCatalog]) -> Result<Vec<PlannedTask>, String> {
    let mut end = reply.len();
    while let Some(close) = reply[..end].rfind(']') {
        let mut start = close;
        while let Some(open) = reply[..start].rfind('[') {
            if let Ok(tasks) = serde_json::from_str::<Vec<PlannedTask>>(&reply[open..=close]) {
                return finish(tasks, catalog);
            }
            start = open;
        }
        end = close;
    }
    Err("the planner did not return a task list".to_string())
}

fn finish(tasks: Vec<PlannedTask>, catalog: &[AgentCatalog]) -> Result<Vec<PlannedTask>, String> {
    let tasks: Vec<PlannedTask> = tasks
        .into_iter()
        .map(|task| {
            let (agent, model) =
                catalog::check(catalog, task.agent.as_deref(), task.model.as_deref());
            PlannedTask {
                title: task.title.trim().chars().take(120).collect(),
                brief: task.brief.trim().to_string(),
                agent,
                model,
            }
        })
        .filter(|task| validate_title(&task.title).is_ok() && validate_brief(&task.brief).is_ok())
        .take(MAX_PLANNED_TASKS)
        .collect();
    if tasks.is_empty() {
        return Err("the planner returned no usable tasks".to_string());
    }
    Ok(tasks)
}

/// Runs a planner CLI in the repository and parses its task list.
pub fn run(agent: &str, job: &str, repo: &Path) -> Result<Vec<PlannedTask>, String> {
    if job.trim().is_empty() || job.len() > MAX_JOB_BYTES {
        return Err(format!("describe the job in 1-{MAX_JOB_BYTES} bytes"));
    }
    let scratch = tempfile::tempdir().map_err(|e| format!("planner temp dir: {e}"))?;
    let last_message = scratch.path().join("last-message.txt");
    let catalog = catalog::load();
    let (bin, args) = command(agent, &prompt(job, &catalog), &last_message)
        .ok_or_else(|| format!("planner must be one of: {}", PLANNER_AGENTS.join(", ")))?;
    let stdout_path = scratch.path().join("stdout.txt");
    let stdout = std::fs::File::create(&stdout_path).map_err(|e| e.to_string())?;
    let env = crate::modules::lsp::env::server_env_overlay();
    let program = crate::modules::lsp::env::resolve_binary(&bin)
        .ok_or_else(|| format!("{bin} is not installed or not on your login shell PATH"))?;
    let mut cmd = std::process::Command::new(program);
    cmd.args(&args)
        .current_dir(repo)
        .envs(env)
        .env("CI", "1")
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(Stdio::null());
    crate::modules::proc::hide_console(&mut cmd);
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("could not start {bin}: {e}. Is it installed and signed in?"))?;
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            break status;
        }
        if started.elapsed() >= PLAN_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{bin} did not answer within 5 minutes"));
        }
        std::thread::sleep(POLL);
    };
    let read = |path: &Path| {
        std::fs::read(path)
            .map(|bytes| {
                String::from_utf8_lossy(&bytes[..bytes.len().min(MAX_OUTPUT_BYTES)]).into_owned()
            })
            .unwrap_or_default()
    };
    let reply = match read(&last_message) {
        text if !text.trim().is_empty() => text,
        _ => read(&stdout_path),
    };
    if !status.success() && reply.trim().is_empty() {
        return Err(format!("{bin} exited with {status}"));
    }
    parse(&reply, &catalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Vec<AgentCatalog> {
        catalog::build(|agent| agent == "claude", None)
    }

    #[test]
    fn parses_the_last_task_array_through_prose_and_fences() {
        let reply = "I looked at [the repo].\n```json\n[{\"title\": \"Add sub\", \"brief\": \"Add subtract to calc.py\"}, {\"title\": \"Add mul\", \"brief\": \"Add multiply\"}]\n```\nDone.";
        let tasks = parse(reply, &catalog()).unwrap();
        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks[0].title, "Add sub");
        assert!(parse("no list here", &catalog()).is_err());
        assert!(parse("[1, 2]", &catalog()).is_err());
    }

    #[test]
    fn drops_invalid_tasks_and_caps_the_count() {
        let many: Vec<_> = (0..12)
            .map(|i| serde_json::json!({ "title": format!("t{i}"), "brief": "b" }))
            .collect();
        assert_eq!(
            parse(&serde_json::to_string(&many).unwrap(), &catalog())
                .unwrap()
                .len(),
            8
        );
        let bad = r#"[{"title": "a\nb", "brief": "x"}, {"title": "ok", "brief": " "}]"#;
        assert!(parse(bad, &catalog()).is_err());
    }

    #[test]
    fn planners_run_read_only_with_the_prompt_as_one_argument() {
        let out = Path::new("/tmp/last");
        let (bin, args) = command("claude", "p; rm -rf /", out).unwrap();
        assert_eq!(bin, "claude");
        assert_eq!(args.last().unwrap(), "p; rm -rf /");
        assert!(args.windows(2).any(|w| w == ["--permission-mode", "plan"]));
        let (_, args) = command("codex", "p", out).unwrap();
        assert!(args.windows(2).any(|w| w == ["--sandbox", "read-only"]));
        assert!(command("bash", "p", out).is_none());
        let text = prompt("do x", &catalog());
        assert!(text.ends_with("Job:\ndo x"));
        assert!(text.contains("\"haiku\""));
    }

    #[test]
    fn planner_picks_survive_only_when_they_exist() {
        let reply = r#"[
            {"title": "a", "brief": "x", "agent": "claude", "model": "haiku"},
            {"title": "b", "brief": "x", "agent": "claude", "model": "gpt-9"},
            {"title": "c", "brief": "x", "agent": "gemini", "model": "gemini-2.5-pro"},
            {"title": "d", "brief": "x"}
        ]"#;
        let tasks = parse(reply, &catalog()).unwrap();
        let picks: Vec<_> = tasks
            .iter()
            .map(|t| (t.agent.as_deref(), t.model.as_deref()))
            .collect();
        assert_eq!(
            picks,
            [
                (Some("claude"), Some("haiku")),
                (Some("claude"), None),
                (None, None),
                (None, None)
            ]
        );
    }
}
