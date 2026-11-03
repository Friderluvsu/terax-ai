use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use terax_control_protocol::{CheckStatus, TaskInfo, TaskStatus};

/// Each worker costs 300-500 MB, so the cap is global across runs.
pub const MAX_WORKERS: usize = 4;
const MAX_TASKS_PER_RUN: usize = 256;
const MAX_STORED_RUNS: usize = 64;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Run {
    pub id: String,
    pub repo: String,
    /// Branch name, or a commit when the orchestrator was on a detached HEAD.
    pub base: String,
    pub created_at_ms: u64,
    #[serde(default)]
    pub stopped: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<String>,
    /// Owned by the Terax board rather than an orchestrator pane.
    #[serde(default)]
    pub board: bool,
    #[serde(default)]
    pub tasks: Vec<TaskInfo>,
}

/// A task consumes CPU and memory while its agent works or its checks run.
pub fn is_busy(task: &TaskInfo) -> bool {
    task.status.is_working()
        || (task.status == TaskStatus::Done && task.checks == Some(CheckStatus::Running))
}

/// An idle finished agent keeps its tab for follow-ups until a slot is needed.
pub fn is_idle_tab(task: &TaskInfo) -> bool {
    task.status == TaskStatus::Done && task.pane_id.is_some() && !is_busy(task)
}

pub fn has_live_tab(task: &TaskInfo) -> bool {
    is_busy(task) || is_idle_tab(task)
}

#[derive(Debug, PartialEq)]
pub enum Admission {
    Start { evict: Option<String> },
    Queue,
}

fn tasks_in_order<'a>(runs: impl IntoIterator<Item = &'a Run>) -> Vec<&'a TaskInfo> {
    let mut runs: Vec<&Run> = runs.into_iter().collect();
    runs.sort_by_key(|run| (run.created_at_ms, run.id.clone()));
    runs.into_iter().flat_map(|run| &run.tasks).collect()
}

/// At most `MAX_WORKERS` agents are resident: busy ones never yield, idle
/// finished ones are closed oldest first to make room.
pub fn admit<'a>(runs: impl IntoIterator<Item = &'a Run>) -> Admission {
    let tasks = tasks_in_order(runs);
    let busy = tasks.iter().filter(|task| is_busy(task)).count();
    if busy >= MAX_WORKERS {
        return Admission::Queue;
    }
    let live = tasks.iter().filter(|task| has_live_tab(task)).count();
    if live < MAX_WORKERS {
        return Admission::Start { evict: None };
    }
    match tasks.iter().find(|task| is_idle_tab(task)) {
        Some(task) => Admission::Start {
            evict: Some(task.id.clone()),
        },
        None => Admission::Queue,
    }
}

pub fn can_add_task(run: &Run) -> Result<(), String> {
    if run.stopped {
        return Err(format!("run {} was stopped; start a new one", run.id));
    }
    if run.tasks.len() >= MAX_TASKS_PER_RUN {
        return Err(format!("run {} reached {MAX_TASKS_PER_RUN} tasks", run.id));
    }
    Ok(())
}

pub fn next_task_id(run: &Run) -> String {
    format!("{}-{}", run.id, run.tasks.len() + 1)
}

fn task_mut<'a>(runs: &'a mut BTreeMap<String, Run>, id: &str) -> Option<&'a mut TaskInfo> {
    runs.values_mut()
        .flat_map(|run| run.tasks.iter_mut())
        .find(|task| task.id == id)
}

#[derive(Debug, Default, PartialEq)]
pub struct Promotion {
    pub started: Vec<String>,
    /// Panes of idle finished workers closed to make room.
    pub evicted: Vec<u32>,
}

/// Oldest queued tasks move to Starting while the admission policy allows.
pub fn promote(runs: &mut BTreeMap<String, Run>) -> Promotion {
    let mut promotion = Promotion::default();
    loop {
        let Some(next) = tasks_in_order(runs.values())
            .into_iter()
            .find(|task| task.status == TaskStatus::Queued)
            .map(|task| task.id.clone())
        else {
            return promotion;
        };
        match admit(runs.values()) {
            Admission::Queue => return promotion,
            Admission::Start { evict } => {
                if let Some(pane) = evict
                    .and_then(|id| task_mut(runs, &id))
                    .and_then(|task| task.pane_id.take())
                {
                    promotion.evicted.push(pane);
                }
                if let Some(task) = task_mut(runs, &next) {
                    task.status = TaskStatus::Starting;
                }
                promotion.started.push(next);
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Signal {
    Working,
    Attention,
    Finished,
}

pub fn parse_signal(kind: &str) -> Option<Signal> {
    match kind {
        "started" | "working" => Some(Signal::Working),
        "attention" => Some(Signal::Attention),
        "finished" | "exited" => Some(Signal::Finished),
        _ => None,
    }
}

/// Applies an agent hook signal. Returns true when checks should start.
pub fn apply_signal(task: &mut TaskInfo, signal: Signal, has_check: bool) -> bool {
    if !matches!(
        task.status,
        TaskStatus::Running | TaskStatus::NeedsInput | TaskStatus::Done
    ) {
        return false;
    }
    match signal {
        Signal::Working => {
            if task.status != TaskStatus::Running {
                task.status = TaskStatus::Running;
                task.checks = None;
                task.unreported = false;
            }
            false
        }
        Signal::Attention => {
            task.status = TaskStatus::NeedsInput;
            task.unreported = true;
            false
        }
        Signal::Finished => {
            if task.status == TaskStatus::Done {
                return false;
            }
            task.status = TaskStatus::Done;
            if has_check {
                task.checks = Some(CheckStatus::Running);
                task.unreported = false;
                true
            } else {
                task.unreported = true;
                false
            }
        }
    }
}

/// What `task.wait` reports: settled states the orchestrator has to act on.
pub fn is_settled(task: &TaskInfo) -> bool {
    match task.status {
        TaskStatus::NeedsInput | TaskStatus::Failed | TaskStatus::Stopped => true,
        TaskStatus::Done => task.checks != Some(CheckStatus::Running),
        TaskStatus::Interrupted => true,
        _ => false,
    }
}

/// Panes do not survive a restart: working and queued tasks are interrupted,
/// finished ones keep their result but lose their tab.
pub fn interrupt_live_tasks(run: &mut Run) -> bool {
    let mut changed = false;
    for task in &mut run.tasks {
        if task.status == TaskStatus::Done {
            if task.pane_id.take().is_some() || task.checks == Some(CheckStatus::Running) {
                if task.checks == Some(CheckStatus::Running) {
                    task.checks = None;
                }
                changed = true;
            }
        } else if task.status.is_active() {
            task.status = TaskStatus::Interrupted;
            task.pane_id = None;
            changed = true;
        }
    }
    changed
}

pub fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && !id.starts_with('-')
}

pub fn state_path(root: &Path, run_id: &str) -> Option<PathBuf> {
    valid_id(run_id).then(|| root.join(format!("{run_id}.json")))
}

pub fn save(root: &Path, run: &Run) -> Result<(), String> {
    let path = state_path(root, &run.id).ok_or_else(|| "invalid run id".to_string())?;
    let mut temp = tempfile::NamedTempFile::new_in(root)
        .map_err(|error| format!("create run state: {error}"))?;
    serde_json::to_writer_pretty(&mut temp, run)
        .map_err(|error| format!("serialize run state: {error}"))?;
    temp.write_all(b"\n")
        .map_err(|error| format!("write run state: {error}"))?;
    temp.persist(&path)
        .map_err(|error| format!("publish run state: {}", error.error))?;
    Ok(())
}

pub fn load_all(root: &Path) -> BTreeMap<String, Run> {
    let mut runs = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return runs;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(stem) = path
            .file_name()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix(".json"))
        else {
            continue;
        };
        if !valid_id(stem) {
            continue;
        }
        let run = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Run>(&bytes).ok())
            .filter(|run| run.id == stem);
        match run {
            Some(run) => {
                runs.insert(run.id.clone(), run);
            }
            None => log::warn!("ignoring unreadable run state {}", path.display()),
        }
    }
    while runs.len() > MAX_STORED_RUNS {
        let Some(oldest) = runs
            .values()
            .min_by_key(|run| run.created_at_ms)
            .map(|run| run.id.clone())
        else {
            break;
        };
        runs.remove(&oldest);
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(run: &str, n: usize, status: TaskStatus) -> TaskInfo {
        TaskInfo {
            id: format!("{run}-{n}"),
            run: run.into(),
            agent: "claude".into(),
            title: None,
            status,
            branch: format!("task/{run}-{n}"),
            worktree: String::new(),
            brief_path: String::new(),
            pane_id: Some(n as u32 + 100),
            error: None,
            model: None,
            checks: None,
            unreported: false,
            merged: false,
        }
    }

    fn run(id: &str, created_at_ms: u64, tasks: Vec<TaskInfo>) -> Run {
        Run {
            id: id.into(),
            repo: "/repo".into(),
            base: "main".into(),
            created_at_ms,
            stopped: false,
            check: None,
            board: false,
            tasks,
        }
    }

    fn admit_into(r: &mut Run) -> Admission {
        let admission = admit(std::iter::once(&*r));
        let status = match admission {
            Admission::Start { .. } => TaskStatus::Starting,
            Admission::Queue => TaskStatus::Queued,
        };
        let n = r.tasks.len() + 1;
        r.tasks.push(task(&r.id.clone(), n, status));
        admission
    }

    #[test]
    fn at_most_four_workers_are_busy_and_the_rest_queue() {
        let mut a = run("aaaa", 1, Vec::new());
        for _ in 0..4 {
            assert_eq!(admit_into(&mut a), Admission::Start { evict: None });
        }
        assert_eq!(admit_into(&mut a), Admission::Queue);
        assert_eq!(a.tasks.iter().filter(|t| is_busy(t)).count(), MAX_WORKERS);
        let b = run("bbbb", 2, Vec::new());
        assert_eq!(admit([&a, &b]), Admission::Queue);
    }

    #[test]
    fn idle_finished_tabs_are_evicted_oldest_first_but_busy_ones_never() {
        let mut runs = BTreeMap::new();
        runs.insert(
            "aaaa".to_string(),
            run(
                "aaaa",
                1,
                vec![
                    task("aaaa", 1, TaskStatus::Running),
                    task("aaaa", 2, TaskStatus::Done),
                    task("aaaa", 3, TaskStatus::Done),
                    task("aaaa", 4, TaskStatus::Running),
                    task("aaaa", 5, TaskStatus::Queued),
                    task("aaaa", 6, TaskStatus::Queued),
                ],
            ),
        );
        let promotion = promote(&mut runs);
        assert_eq!(promotion.started, vec!["aaaa-5", "aaaa-6"]);
        assert_eq!(promotion.evicted, vec![102, 103]);
        let tasks = &runs["aaaa"].tasks;
        assert_eq!(
            tasks.iter().filter(|t| has_live_tab(t)).count(),
            MAX_WORKERS
        );
        assert_eq!(
            tasks[1].status,
            TaskStatus::Done,
            "eviction keeps the result"
        );

        runs.get_mut("aaaa")
            .unwrap()
            .tasks
            .push(task("aaaa", 7, TaskStatus::Queued));
        assert_eq!(promote(&mut runs), Promotion::default());
    }

    #[test]
    fn a_running_check_keeps_the_slot_busy() {
        let mut t = task("aaaa", 1, TaskStatus::Done);
        t.checks = Some(CheckStatus::Running);
        assert!(is_busy(&t));
        assert!(!is_idle_tab(&t));
        assert!(!is_settled(&t));
        t.checks = Some(CheckStatus::Failed);
        assert!(!is_busy(&t));
        assert!(is_settled(&t));
    }

    #[test]
    fn hook_signals_drive_status_and_reporting() {
        let mut t = task("aaaa", 1, TaskStatus::Running);
        assert!(!apply_signal(&mut t, Signal::Attention, false));
        assert_eq!(t.status, TaskStatus::NeedsInput);
        assert!(t.unreported);
        apply_signal(&mut t, Signal::Working, false);
        assert_eq!(t.status, TaskStatus::Running);
        assert!(!t.unreported);
        assert!(apply_signal(&mut t, Signal::Finished, true));
        assert_eq!(t.checks, Some(CheckStatus::Running));
        assert!(!t.unreported, "reported only after checks finish");
        assert!(
            !apply_signal(&mut t, Signal::Finished, true),
            "no double checks"
        );

        let mut stopped = task("aaaa", 2, TaskStatus::Stopped);
        apply_signal(&mut stopped, Signal::Working, false);
        assert_eq!(stopped.status, TaskStatus::Stopped);
        assert_eq!(parse_signal("exited"), Some(Signal::Finished));
        assert_eq!(parse_signal("bogus"), None);
    }

    #[test]
    fn reload_interrupts_working_tasks_and_keeps_finished_results() {
        let mut r = run(
            "aaaa",
            1,
            vec![
                task("aaaa", 1, TaskStatus::Running),
                task("aaaa", 2, TaskStatus::Queued),
                task("aaaa", 3, TaskStatus::Stopped),
                task("aaaa", 4, TaskStatus::Done),
            ],
        );
        assert!(interrupt_live_tasks(&mut r));
        assert_eq!(r.tasks[0].status, TaskStatus::Interrupted);
        assert_eq!(r.tasks[1].status, TaskStatus::Interrupted);
        assert_eq!(r.tasks[2].status, TaskStatus::Stopped);
        assert_eq!(r.tasks[3].status, TaskStatus::Done);
        assert!(r.tasks.iter().all(|t| !has_live_tab(t)));
        assert!(!interrupt_live_tasks(&mut r));
    }

    #[test]
    fn ids_reject_traversal_and_separators() {
        assert!(valid_id("a1b2c3d4-12"));
        for bad in ["", "..", "a/b", "a\\b", "A1", "-a", "a.b", &"a".repeat(65)] {
            assert!(!valid_id(bad), "{bad:?} must be rejected");
            assert!(state_path(Path::new("/root"), bad).is_none());
        }
    }

    #[test]
    fn stopped_runs_refuse_new_tasks() {
        let mut r = run("aaaa", 1, Vec::new());
        assert!(can_add_task(&r).is_ok());
        assert_eq!(next_task_id(&r), "aaaa-1");
        r.stopped = true;
        assert!(can_add_task(&r).is_err());
    }

    #[test]
    fn state_round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let r = run("aaaa", 1, vec![task("aaaa", 1, TaskStatus::Running)]);
        save(dir.path(), &r).unwrap();
        std::fs::write(dir.path().join("evil..json"), b"{}").unwrap();
        std::fs::write(dir.path().join("bbbb.json"), b"not json").unwrap();
        let loaded = load_all(dir.path());
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded["aaaa"], r);
    }
}
