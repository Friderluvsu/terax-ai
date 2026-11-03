//! Agent orchestration: one orchestrator pane plans, worker panes run tasks in
//! their own git worktrees. Roles are bound to pane credentials in Rust, so a
//! worker can never start other workers.

pub mod run;
pub mod worktree;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use terax_control_protocol::{
    validate_brief, validate_title, TaskCreateParams, TaskInfo, TaskOpenParams, TaskStatus,
    TASK_AGENTS,
};

use run::Run;

pub type OrchestraError = (&'static str, String);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Caller {
    /// Authenticated with the descriptor token: a human-run CLI outside Terax.
    External,
    Pane(u32),
}

#[derive(Clone, Debug, PartialEq)]
enum Role {
    Plain,
    Orchestrator(String),
    Worker,
}

#[derive(Default)]
pub struct Orchestra {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    root: Option<PathBuf>,
    runs: BTreeMap<String, Run>,
    orchestrators: HashMap<u32, String>,
    workers: HashMap<u32, Option<String>>,
}

#[derive(Debug, PartialEq)]
pub struct Created {
    pub task: TaskInfo,
    pub start: bool,
}

#[derive(Debug, Default, PartialEq)]
pub struct Stopped {
    pub tasks: Vec<String>,
    pub panes: Vec<u32>,
}

impl Orchestra {
    pub fn init(&self, root: PathBuf) -> Result<(), String> {
        std::fs::create_dir_all(&root)
            .map_err(|error| format!("create orchestra dir {}: {error}", root.display()))?;
        let root = std::fs::canonicalize(&root)
            .map_err(|error| format!("resolve orchestra dir: {error}"))?;
        let mut runs = run::load_all(&root);
        for run in runs.values_mut() {
            if run::interrupt_live_tasks(run) {
                if let Err(error) = run::save(&root, run) {
                    log::warn!("could not persist run {}: {error}", run.id);
                }
            }
        }
        let mut inner = self.lock();
        inner.root = Some(root);
        inner.runs = runs;
        Ok(())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("orchestra state poisoned")
    }

    /// Called when a pane's shell spawns. A shell started inside the orchestra
    /// directory is a worker for its whole life, whatever it later cds into.
    pub fn register_pane(&self, pane_id: u32, cwd: Option<&Path>) -> Vec<(String, String)> {
        let mut inner = self.lock();
        let Some(root) = inner.root.clone() else {
            inner.workers.remove(&pane_id);
            return Vec::new();
        };
        let canonical = cwd.and_then(|cwd| std::fs::canonicalize(cwd).ok());
        let Some(cwd) = canonical.filter(|cwd| cwd.starts_with(&root)) else {
            inner.workers.remove(&pane_id);
            return Vec::new();
        };
        inner.orchestrators.remove(&pane_id);
        let task = inner.runs.values().flat_map(|run| &run.tasks).find(|task| {
            worktree::task_paths(&root, &task.run, &task.id)
                .is_ok_and(|paths| cwd.starts_with(&paths.worktree))
        });
        let Some(task) = task.cloned() else {
            inner.workers.insert(pane_id, None);
            return vec![("TERAX_ROLE".into(), "worker".into())];
        };
        inner.workers.insert(pane_id, Some(task.id.clone()));
        let mut env = vec![
            ("TERAX_ROLE".into(), "worker".into()),
            ("TERAX_TASK_ID".into(), task.id.clone()),
        ];
        if let Ok(paths) = worktree::task_paths(&root, &task.run, &task.id) {
            env.push((
                "CARGO_TARGET_DIR".into(),
                paths.target.to_string_lossy().into_owned(),
            ));
        }
        env
    }

    pub fn forget_pane(&self, pane_id: u32) {
        let mut inner = self.lock();
        inner.orchestrators.remove(&pane_id);
        inner.workers.remove(&pane_id);
    }

    fn role(inner: &Inner, pane_id: u32) -> Role {
        if inner.workers.contains_key(&pane_id) {
            return Role::Worker;
        }
        match inner.orchestrators.get(&pane_id) {
            Some(run) => Role::Orchestrator(run.clone()),
            None => Role::Plain,
        }
    }

    /// Marks the calling pane as the orchestrator of a run rooted at the git
    /// repository containing `cwd`. Calling it again returns the same run.
    pub fn orchestrate(&self, caller: Caller, cwd: &Path) -> Result<Run, OrchestraError> {
        let Caller::Pane(pane_id) = caller else {
            return Err(forbidden("orchestrate must be run from a Terax pane"));
        };
        let root = {
            let inner = self.lock();
            match Self::role(&inner, pane_id) {
                Role::Worker => return Err(forbidden("worker panes cannot orchestrate")),
                Role::Orchestrator(run) => {
                    if let Some(run) = inner.runs.get(&run).filter(|run| !run.stopped) {
                        return Ok(run.clone());
                    }
                }
                Role::Plain => {}
            }
            inner.root.clone().ok_or_else(unavailable)?
        };
        let repo = worktree::repo_toplevel(cwd).map_err(|error| ("not_a_repository", error))?;
        if repo.starts_with(&root) || root.starts_with(&repo) {
            return Err(forbidden("a task worktree cannot orchestrate"));
        }
        let base = worktree::current_base(&repo).map_err(|error| ("git_failed", error))?;
        let mut inner = self.lock();
        if Self::role(&inner, pane_id) == Role::Worker {
            return Err(forbidden("worker panes cannot orchestrate"));
        }
        let id = loop {
            let id = random_id()?;
            if !inner.runs.contains_key(&id) {
                break id;
            }
        };
        let run = Run {
            id: id.clone(),
            repo: crate::modules::fs::to_canon(&repo),
            base,
            created_at_ms: now_ms(),
            stopped: false,
            tasks: Vec::new(),
        };
        persist(&root, &run)?;
        inner.runs.insert(id.clone(), run.clone());
        inner.orchestrators.insert(pane_id, id);
        Ok(run)
    }

    pub fn create_task(
        &self,
        caller: Caller,
        params: TaskCreateParams,
    ) -> Result<Created, OrchestraError> {
        let mut inner = self.lock();
        let run_id = match caller {
            Caller::External => {
                return Err(forbidden(
                    "tasks can only be created from an orchestrator pane",
                ))
            }
            Caller::Pane(pane_id) => match Self::role(&inner, pane_id) {
                Role::Worker => return Err(forbidden("worker panes cannot create tasks")),
                Role::Plain => {
                    return Err((
                        "not_orchestrator",
                        "run `terax orchestrate` in this pane first".to_string(),
                    ))
                }
                Role::Orchestrator(run) => run,
            },
        };
        if !TASK_AGENTS.contains(&params.agent.as_str()) {
            return Err((
                "invalid_params",
                format!("agent must be one of: {}", TASK_AGENTS.join(", ")),
            ));
        }
        validate_brief(&params.brief).map_err(|error| ("invalid_params", error))?;
        if let Some(title) = &params.title {
            validate_title(title).map_err(|error| ("invalid_params", error))?;
        }
        let root = inner.root.clone().ok_or_else(unavailable)?;
        let status = run::admit(inner.runs.values());
        let run = inner
            .runs
            .get_mut(&run_id)
            .ok_or_else(|| ("not_orchestrator", "orchestrator run is gone".to_string()))?;
        run::can_add_task(run).map_err(|error| ("run_closed", error))?;
        let id = run::next_task_id(run);
        let paths = worktree::task_paths(&root, &run_id, &id).map_err(internal)?;
        std::fs::create_dir_all(&paths.dir)
            .and_then(|_| std::fs::write(&paths.brief, params.brief.as_bytes()))
            .map_err(|error| ("io_error", format!("write brief: {error}")))?;
        let task = TaskInfo {
            id: id.clone(),
            run: run_id,
            agent: params.agent,
            title: params.title.map(|title| title.trim().to_string()),
            status,
            branch: worktree::branch_name(&id),
            worktree: crate::modules::fs::to_canon(&paths.worktree),
            brief_path: crate::modules::fs::to_canon(&paths.brief),
            pane_id: None,
            error: None,
        };
        run.tasks.push(task.clone());
        let snapshot = run.clone();
        drop(inner);
        persist(&root, &snapshot)?;
        Ok(Created {
            start: status == TaskStatus::Starting,
            task,
        })
    }

    /// Creates the worktree for a task in `Starting`. Git runs without the lock;
    /// the slot stays held by the `Starting` status meanwhile.
    pub fn prepare_worktree(&self, task_id: &str) -> Result<TaskOpenParams, OrchestraError> {
        let (root, repo, base, task) = {
            let inner = self.lock();
            let root = inner.root.clone().ok_or_else(unavailable)?;
            let (run, task) = find_task(&inner.runs, task_id)?;
            if task.status != TaskStatus::Starting {
                return Err(("invalid_state", format!("task {task_id} is not starting")));
            }
            (
                root,
                PathBuf::from(&run.repo),
                run.base.clone(),
                task.clone(),
            )
        };
        let paths = worktree::task_paths(&root, &task.run, &task.id).map_err(internal)?;
        let result = worktree::create(&repo, &base, &task.branch, &paths);
        let mut inner = self.lock();
        let still_running = {
            let current = find_task_mut(&mut inner.runs, task_id)?;
            if current.status == TaskStatus::Starting {
                match &result {
                    Ok(()) => current.status = TaskStatus::Running,
                    Err(error) => {
                        current.status = TaskStatus::Failed;
                        current.error = Some(error.clone());
                    }
                }
            }
            current.status == TaskStatus::Running
        };
        let snapshot = inner.runs[&task.run].clone();
        drop(inner);
        persist(&root, &snapshot)?;
        result.map_err(|error| ("git_failed", error))?;
        if !still_running {
            return Err(("invalid_state", format!("task {task_id} was stopped")));
        }
        Ok(TaskOpenParams {
            task: task.id,
            agent: task.agent,
            title: task.title,
            cwd: task.worktree,
            brief_path: task.brief_path,
        })
    }

    pub fn attach_pane(&self, task_id: &str, pane_id: u32) {
        self.update_task(task_id, |task| {
            if task.status == TaskStatus::Running {
                task.pane_id = Some(pane_id);
            }
        });
    }

    pub fn fail_task(&self, task_id: &str, error: String) {
        self.update_task(task_id, |task| {
            if task.status.is_active() {
                task.status = TaskStatus::Failed;
                task.error = Some(error);
            }
        });
    }

    fn update_task(&self, task_id: &str, apply: impl FnOnce(&mut TaskInfo)) {
        let mut inner = self.lock();
        let Some(root) = inner.root.clone() else {
            return;
        };
        let Ok(task) = find_task_mut(&mut inner.runs, task_id) else {
            return;
        };
        apply(task);
        let run_id = task.run.clone();
        let snapshot = inner.runs[&run_id].clone();
        drop(inner);
        if let Err((_, error)) = persist(&root, &snapshot) {
            log::warn!("could not persist run {run_id}: {error}");
        }
    }

    pub fn list(&self, caller: Caller) -> Result<Vec<TaskInfo>, OrchestraError> {
        let inner = self.lock();
        let scope = self.scope(&inner, caller)?;
        Ok(inner
            .runs
            .values()
            .filter(|run| scope.as_ref().is_none_or(|id| &run.id == id))
            .flat_map(|run| run.tasks.iter().cloned())
            .collect())
    }

    /// `Some(run)` limits an orchestrator to its own run; `None` means the
    /// human sees everything. Workers are refused.
    fn scope(&self, inner: &Inner, caller: Caller) -> Result<Option<String>, OrchestraError> {
        match caller {
            Caller::External => Ok(None),
            Caller::Pane(pane_id) => match Self::role(inner, pane_id) {
                Role::Worker => Err(forbidden("worker panes cannot manage tasks")),
                Role::Orchestrator(run) => Ok(Some(run)),
                Role::Plain => Ok(None),
            },
        }
    }

    pub fn stop_task(&self, caller: Caller, task_id: &str) -> Result<Stopped, OrchestraError> {
        let mut inner = self.lock();
        let scope = self.scope(&inner, caller)?;
        let root = inner.root.clone().ok_or_else(unavailable)?;
        let task = find_task_mut(&mut inner.runs, task_id)?;
        if scope.as_ref().is_some_and(|run| run != &task.run) {
            return Err(forbidden("task belongs to another run"));
        }
        let mut stopped = Stopped::default();
        stop_one(task, &mut stopped);
        let snapshot = inner.runs[&task_run(task_id)].clone();
        drop(inner);
        persist(&root, &snapshot)?;
        Ok(stopped)
    }

    /// Stops the given run, the caller's own run, or every run for the human.
    /// Worktrees are kept; deleting them is always a separate, explicit step.
    pub fn stop_run(
        &self,
        caller: Caller,
        run_id: Option<&str>,
    ) -> Result<Stopped, OrchestraError> {
        let mut inner = self.lock();
        let scope = self.scope(&inner, caller)?;
        let root = inner.root.clone().ok_or_else(unavailable)?;
        let targets: Vec<String> = match (run_id, scope) {
            (Some(id), Some(own)) if id != own => {
                return Err(forbidden("run belongs to another orchestrator"))
            }
            (Some(id), _) => vec![id.to_string()],
            (None, Some(own)) => vec![own],
            (None, None) => inner.runs.keys().cloned().collect(),
        };
        let mut stopped = Stopped::default();
        let mut snapshots = Vec::new();
        for id in targets {
            let run = inner
                .runs
                .get_mut(&id)
                .ok_or_else(|| ("not_found", format!("unknown run {id}")))?;
            run.stopped = true;
            for task in &mut run.tasks {
                stop_one(task, &mut stopped);
            }
            snapshots.push(run.clone());
        }
        inner
            .orchestrators
            .retain(|_, run| !snapshots.iter().any(|stopped_run| &stopped_run.id == run));
        drop(inner);
        for snapshot in &snapshots {
            persist(&root, snapshot)?;
        }
        Ok(stopped)
    }

    pub fn promote(&self) -> Vec<String> {
        let mut inner = self.lock();
        let Some(root) = inner.root.clone() else {
            return Vec::new();
        };
        let promoted = run::promote(&mut inner.runs);
        let snapshots: Vec<Run> = promoted
            .iter()
            .filter_map(|id| inner.runs.get(&task_run(id)).cloned())
            .collect();
        drop(inner);
        for snapshot in &snapshots {
            if let Err((_, error)) = persist(&root, snapshot) {
                log::warn!("could not persist run {}: {error}", snapshot.id);
            }
        }
        promoted
    }
}

fn stop_one(task: &mut TaskInfo, stopped: &mut Stopped) {
    if !task.status.is_active() {
        return;
    }
    task.status = TaskStatus::Stopped;
    stopped.tasks.push(task.id.clone());
    if let Some(pane) = task.pane_id.take() {
        stopped.panes.push(pane);
    }
}

fn task_run(task_id: &str) -> String {
    task_id
        .rsplit_once('-')
        .map_or_else(String::new, |(run, _)| run.to_string())
}

fn find_task<'a>(
    runs: &'a BTreeMap<String, Run>,
    task_id: &str,
) -> Result<(&'a Run, &'a TaskInfo), OrchestraError> {
    runs.get(&task_run(task_id))
        .and_then(|run| run.tasks.iter().find(|t| t.id == task_id).map(|t| (run, t)))
        .ok_or_else(|| ("not_found", format!("unknown task {task_id}")))
}

fn find_task_mut<'a>(
    runs: &'a mut BTreeMap<String, Run>,
    task_id: &str,
) -> Result<&'a mut TaskInfo, OrchestraError> {
    runs.get_mut(&task_run(task_id))
        .and_then(|run| run.tasks.iter_mut().find(|t| t.id == task_id))
        .ok_or_else(|| ("not_found", format!("unknown task {task_id}")))
}

fn persist(root: &Path, run: &Run) -> Result<(), OrchestraError> {
    run::save(root, run).map_err(|error| ("io_error", error))
}

fn forbidden(message: &str) -> OrchestraError {
    ("forbidden", message.to_string())
}

fn unavailable() -> OrchestraError {
    (
        "orchestra_unavailable",
        "orchestration storage is not initialized".to_string(),
    )
}

fn internal(error: String) -> OrchestraError {
    ("internal_error", error)
}

fn random_id() -> Result<String, OrchestraError> {
    let mut bytes = [0_u8; 4];
    getrandom::fill(&mut bytes).map_err(|error| internal(format!("random id: {error}")))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    struct Fixture {
        orchestra: Orchestra,
        repo: tempfile::TempDir,
        _data: tempfile::TempDir,
        root: PathBuf,
    }

    fn fixture() -> Fixture {
        let repo = tempfile::tempdir().unwrap();
        for args in [
            vec!["init", "--quiet", "-b", "main"],
            vec!["config", "user.email", "t@example.com"],
            vec!["config", "user.name", "t"],
            vec!["commit", "--quiet", "--allow-empty", "-m", "init"],
        ] {
            assert!(Command::new("git")
                .args(&args)
                .current_dir(repo.path())
                .status()
                .unwrap()
                .success());
        }
        let data = tempfile::tempdir().unwrap();
        let orchestra = Orchestra::default();
        orchestra.init(data.path().join("orchestra")).unwrap();
        let root = std::fs::canonicalize(data.path().join("orchestra")).unwrap();
        Fixture {
            orchestra,
            repo,
            _data: data,
            root,
        }
    }

    fn params(brief: &str) -> TaskCreateParams {
        TaskCreateParams {
            agent: "claude".into(),
            brief: brief.into(),
            title: None,
        }
    }

    #[test]
    fn plain_and_external_callers_cannot_create_tasks() {
        let f = fixture();
        let err = f
            .orchestra
            .create_task(Caller::Pane(1), params("x"))
            .unwrap_err();
        assert_eq!(err.0, "not_orchestrator");
        let err = f
            .orchestra
            .create_task(Caller::External, params("x"))
            .unwrap_err();
        assert_eq!(err.0, "forbidden");
        assert_eq!(
            f.orchestra
                .orchestrate(Caller::External, f.repo.path())
                .unwrap_err()
                .0,
            "forbidden"
        );
    }

    #[test]
    fn a_worker_pane_cannot_create_tasks_or_become_orchestrator() {
        let f = fixture();
        let run = f
            .orchestra
            .orchestrate(Caller::Pane(1), f.repo.path())
            .unwrap();
        let created = f
            .orchestra
            .create_task(Caller::Pane(1), params("do it"))
            .unwrap();
        assert!(created.start);
        let open = f.orchestra.prepare_worktree(&created.task.id).unwrap();
        assert!(Path::new(&open.cwd).join(".git").exists());

        let env = f.orchestra.register_pane(2, Some(Path::new(&open.cwd)));
        assert!(env.contains(&("TERAX_TASK_ID".into(), created.task.id.clone())));
        let target = env
            .iter()
            .find(|(key, _)| key == "CARGO_TARGET_DIR")
            .map(|(_, value)| PathBuf::from(value))
            .unwrap();
        assert!(target.starts_with(f.root.join(&run.id)));

        let err = f
            .orchestra
            .create_task(Caller::Pane(2), params("x"))
            .unwrap_err();
        assert_eq!(err, forbidden("worker panes cannot create tasks"));
        let err = f
            .orchestra
            .orchestrate(Caller::Pane(2), f.repo.path())
            .unwrap_err();
        assert_eq!(err.0, "forbidden");
        assert_eq!(
            f.orchestra.list(Caller::Pane(2)).unwrap_err().0,
            "forbidden"
        );
        assert_eq!(
            f.orchestra
                .stop_task(Caller::Pane(2), &created.task.id)
                .unwrap_err()
                .0,
            "forbidden"
        );

        let nested = Path::new(&open.cwd).join("sub");
        std::fs::create_dir(&nested).unwrap();
        f.orchestra.register_pane(3, Some(&nested));
        assert_eq!(
            f.orchestra
                .orchestrate(Caller::Pane(3), &nested)
                .unwrap_err()
                .0,
            "forbidden"
        );
    }

    #[test]
    fn an_orchestrator_respawned_inside_a_worktree_loses_its_role() {
        let f = fixture();
        f.orchestra
            .orchestrate(Caller::Pane(1), f.repo.path())
            .unwrap();
        let created = f
            .orchestra
            .create_task(Caller::Pane(1), params("x"))
            .unwrap();
        let open = f.orchestra.prepare_worktree(&created.task.id).unwrap();
        f.orchestra.register_pane(1, Some(Path::new(&open.cwd)));
        let err = f
            .orchestra
            .create_task(Caller::Pane(1), params("x"))
            .unwrap_err();
        assert_eq!(err.0, "forbidden");
    }

    #[test]
    fn the_worker_cap_holds_and_stopping_promotes_the_queue() {
        let f = fixture();
        f.orchestra
            .orchestrate(Caller::Pane(1), f.repo.path())
            .unwrap();
        let created: Vec<Created> = (0..6)
            .map(|n| {
                f.orchestra
                    .create_task(Caller::Pane(1), params(&format!("task {n}")))
                    .unwrap()
            })
            .collect();
        let starts: Vec<bool> = created.iter().map(|c| c.start).collect();
        assert_eq!(starts, [true, true, true, true, false, false]);
        assert!(f.orchestra.promote().is_empty());

        let first = &created[0].task.id;
        f.orchestra.prepare_worktree(first).unwrap();
        f.orchestra.attach_pane(first, 9);
        let stopped = f.orchestra.stop_task(Caller::Pane(1), first).unwrap();
        assert_eq!(stopped.panes, vec![9]);
        assert_eq!(f.orchestra.promote(), vec![created[4].task.id.clone()]);
        assert!(f.orchestra.promote().is_empty());

        let worktree = PathBuf::from(&created[0].task.worktree);
        assert!(
            worktree.join(".git").exists(),
            "stop must keep the worktree"
        );
    }

    #[test]
    fn orchestrators_see_only_their_run_and_humans_stop_everything() {
        let f = fixture();
        let other_repo = fixture();
        let a = f
            .orchestra
            .orchestrate(Caller::Pane(1), f.repo.path())
            .unwrap();
        let b = f
            .orchestra
            .orchestrate(Caller::Pane(2), other_repo.repo.path())
            .unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(
            f.orchestra
                .orchestrate(Caller::Pane(1), f.repo.path())
                .unwrap()
                .id,
            a.id
        );
        let ta = f
            .orchestra
            .create_task(Caller::Pane(1), params("a"))
            .unwrap();
        let tb = f
            .orchestra
            .create_task(Caller::Pane(2), params("b"))
            .unwrap();

        let own: Vec<String> = f
            .orchestra
            .list(Caller::Pane(1))
            .unwrap()
            .into_iter()
            .map(|t| t.id)
            .collect();
        assert_eq!(own, vec![ta.task.id.clone()]);
        assert_eq!(f.orchestra.list(Caller::External).unwrap().len(), 2);
        assert_eq!(
            f.orchestra
                .stop_task(Caller::Pane(1), &tb.task.id)
                .unwrap_err()
                .0,
            "forbidden"
        );
        assert_eq!(
            f.orchestra
                .stop_run(Caller::Pane(1), Some(&b.id))
                .unwrap_err()
                .0,
            "forbidden"
        );

        let stopped = f.orchestra.stop_run(Caller::External, None).unwrap();
        assert_eq!(stopped.tasks.len(), 2);
        assert_eq!(
            f.orchestra
                .create_task(Caller::Pane(1), params("x"))
                .unwrap_err()
                .0,
            "not_orchestrator"
        );
    }

    #[test]
    fn invalid_agents_and_briefs_are_rejected_before_anything_is_written() {
        let f = fixture();
        f.orchestra
            .orchestrate(Caller::Pane(1), f.repo.path())
            .unwrap();
        let mut bad = params("x");
        bad.agent = "sh -c evil".into();
        assert_eq!(
            f.orchestra.create_task(Caller::Pane(1), bad).unwrap_err().0,
            "invalid_params"
        );
        assert_eq!(
            f.orchestra
                .create_task(Caller::Pane(1), params(" "))
                .unwrap_err()
                .0,
            "invalid_params"
        );
        assert!(f.orchestra.list(Caller::Pane(1)).unwrap().is_empty());
    }

    #[test]
    fn restart_interrupts_running_tasks_and_keeps_history() {
        let f = fixture();
        f.orchestra
            .orchestrate(Caller::Pane(1), f.repo.path())
            .unwrap();
        let created = f
            .orchestra
            .create_task(Caller::Pane(1), params("x"))
            .unwrap();
        let reloaded = Orchestra::default();
        reloaded.init(f.root.clone()).unwrap();
        let tasks = reloaded.list(Caller::External).unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].id, created.task.id);
        assert_eq!(tasks[0].status, TaskStatus::Interrupted);
        assert_eq!(
            reloaded
                .create_task(Caller::Pane(1), params("x"))
                .unwrap_err()
                .0,
            "not_orchestrator"
        );
    }
}
