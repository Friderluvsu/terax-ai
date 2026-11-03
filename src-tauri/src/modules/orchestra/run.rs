use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use terax_control_protocol::{TaskInfo, TaskStatus};

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
    #[serde(default)]
    pub tasks: Vec<TaskInfo>,
}

pub fn worker_slots_in_use<'a>(runs: impl IntoIterator<Item = &'a Run>) -> usize {
    runs.into_iter()
        .flat_map(|run| &run.tasks)
        .filter(|task| task.status.holds_worker_slot())
        .count()
}

pub fn admit<'a>(runs: impl IntoIterator<Item = &'a Run>) -> TaskStatus {
    if worker_slots_in_use(runs) < MAX_WORKERS {
        TaskStatus::Starting
    } else {
        TaskStatus::Queued
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

/// Oldest queued tasks move to Starting while worker slots are free.
pub fn promote(runs: &mut BTreeMap<String, Run>) -> Vec<String> {
    let mut free = MAX_WORKERS.saturating_sub(worker_slots_in_use(runs.values()));
    let mut queued: Vec<(&String, usize)> = runs
        .iter()
        .flat_map(|(run_id, run)| {
            run.tasks
                .iter()
                .enumerate()
                .filter(|(_, task)| task.status == TaskStatus::Queued)
                .map(move |(index, _)| (run_id, index))
        })
        .collect();
    queued.sort_by_key(|(run_id, index)| (runs[*run_id].created_at_ms, *index));
    let picked: Vec<(String, usize)> = queued
        .into_iter()
        .take_while(|_| {
            let take = free > 0;
            free = free.saturating_sub(1);
            take
        })
        .map(|(run_id, index)| (run_id.clone(), index))
        .collect();
    picked
        .into_iter()
        .filter_map(|(run_id, index)| {
            let task = runs.get_mut(&run_id)?.tasks.get_mut(index)?;
            task.status = TaskStatus::Starting;
            Some(task.id.clone())
        })
        .collect()
}

/// Panes do not survive a restart, so live tasks are marked interrupted on load.
pub fn interrupt_live_tasks(run: &mut Run) -> bool {
    let mut changed = false;
    for task in &mut run.tasks {
        if task.status.is_active() {
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
            pane_id: Some(n as u32),
            error: None,
        }
    }

    fn run(id: &str, created_at_ms: u64, tasks: Vec<TaskInfo>) -> Run {
        Run {
            id: id.into(),
            repo: "/repo".into(),
            base: "main".into(),
            created_at_ms,
            stopped: false,
            tasks,
        }
    }

    #[test]
    fn concurrency_cap_is_global_and_queues_the_rest() {
        let mut runs = BTreeMap::new();
        let mut a = run("aaaa", 1, Vec::new());
        for n in 1..=6 {
            let status = admit(std::iter::once(&a));
            a.tasks.push(task("aaaa", n, status));
        }
        let statuses: Vec<_> = a.tasks.iter().map(|t| t.status).collect();
        assert_eq!(
            statuses,
            [
                TaskStatus::Starting,
                TaskStatus::Starting,
                TaskStatus::Starting,
                TaskStatus::Starting,
                TaskStatus::Queued,
                TaskStatus::Queued,
            ]
        );
        runs.insert(a.id.clone(), a);
        let b = run("bbbb", 2, Vec::new());
        assert_eq!(admit(runs.values().chain([&b])), TaskStatus::Queued);
    }

    #[test]
    fn promotion_fills_only_freed_slots_in_creation_order() {
        let mut runs = BTreeMap::new();
        runs.insert(
            "bbbb".to_string(),
            run("bbbb", 2, vec![task("bbbb", 1, TaskStatus::Queued)]),
        );
        runs.insert(
            "aaaa".to_string(),
            run(
                "aaaa",
                1,
                vec![
                    task("aaaa", 1, TaskStatus::Running),
                    task("aaaa", 2, TaskStatus::Running),
                    task("aaaa", 3, TaskStatus::Running),
                    task("aaaa", 4, TaskStatus::Stopped),
                    task("aaaa", 5, TaskStatus::Queued),
                ],
            ),
        );
        assert_eq!(promote(&mut runs), vec!["aaaa-5".to_string()]);
        assert_eq!(worker_slots_in_use(runs.values()), MAX_WORKERS);
        assert!(promote(&mut runs).is_empty());
        assert_eq!(runs["bbbb"].tasks[0].status, TaskStatus::Queued);

        runs.get_mut("aaaa").unwrap().tasks[0].status = TaskStatus::Stopped;
        assert_eq!(promote(&mut runs), vec!["bbbb-1".to_string()]);
    }

    #[test]
    fn reload_interrupts_live_tasks_and_frees_their_slots() {
        let mut r = run(
            "aaaa",
            1,
            vec![
                task("aaaa", 1, TaskStatus::Running),
                task("aaaa", 2, TaskStatus::Queued),
                task("aaaa", 3, TaskStatus::Stopped),
            ],
        );
        assert!(interrupt_live_tasks(&mut r));
        assert_eq!(worker_slots_in_use([&r]), 0);
        assert_eq!(r.tasks[0].status, TaskStatus::Interrupted);
        assert_eq!(r.tasks[0].pane_id, None);
        assert_eq!(r.tasks[2].status, TaskStatus::Stopped);
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
    fn stopped_runs_and_full_runs_refuse_new_tasks() {
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
