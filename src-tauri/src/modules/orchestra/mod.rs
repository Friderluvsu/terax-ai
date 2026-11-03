//! Agent orchestration: one orchestrator pane plans, worker panes run tasks in
//! their own git worktrees. Roles are bound to pane credentials in Rust, so a
//! worker can never start other workers.

pub mod checks;
pub mod run;
pub mod orchestrator;
pub mod worktree;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Condvar, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use terax_control_protocol::{
    validate_brief, validate_check_command, validate_model, validate_send_text, validate_title,
    CheckStatus, TaskCreateParams, TaskInfo, TaskOpenParams, TaskStatus, TASK_AGENTS,
};

use checks::CheckJob;
use run::{Admission, Promotion, Run};

pub type OrchestraError = (&'static str, String);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Caller {
    /// Authenticated with the descriptor token or acting from the Terax UI:
    /// the human, with no pane.
    External,
    Pane(u32),
}

#[derive(Clone, Debug, PartialEq)]
enum Role {
    Plain,
    Orchestrator(String),
    Worker,
}

type Listener = Box<dyn Fn() + Send + Sync>;

#[derive(Default)]
pub struct Orchestra {
    inner: Mutex<Inner>,
    changed: Condvar,
    listener: OnceLock<Listener>,
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
    pub evicted: Option<u32>,
}

#[derive(Debug, Default, PartialEq)]
pub struct Stopped {
    pub tasks: Vec<String>,
    pub panes: Vec<u32>,
}

#[derive(Debug, PartialEq)]
pub struct TaskResult {
    pub task: TaskInfo,
    pub result: Option<String>,
    pub check_log: Option<String>,
}

#[derive(Default)]
pub struct SignalEffect {
    pub check: Option<CheckJob>,
    /// A slot may have freed up for the queue.
    pub promote: bool,
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

    /// Called after every state change, outside the state lock.
    pub fn set_listener(&self, listener: impl Fn() + Send + Sync + 'static) {
        let _ = self.listener.set(Box::new(listener));
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("orchestra state poisoned")
    }

    /// Persists the given runs while still holding the lock, so snapshots reach
    /// disk in order, then wakes waiters and the UI.
    fn commit(&self, inner: MutexGuard<'_, Inner>, run_ids: &[&str]) -> Result<(), OrchestraError> {
        let mut result = Ok(());
        if let Some(root) = &inner.root {
            for id in run_ids {
                if let Some(run) = inner.runs.get(*id) {
                    if let Err(error) = persist(root, run) {
                        log::warn!("could not persist run {id}: {}", error.1);
                        result = Err(error);
                    }
                }
            }
        }
        drop(inner);
        self.changed.notify_all();
        if let Some(listener) = self.listener.get() {
            listener();
        }
        result
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
    /// repository containing `cwd`. Calling it again returns the same run and
    /// updates its check command when one is given.
    pub fn orchestrate(
        &self,
        caller: Caller,
        cwd: &Path,
        check: Option<String>,
    ) -> Result<Run, OrchestraError> {
        let Caller::Pane(pane_id) = caller else {
            return Err(forbidden("orchestrate must be run from a Terax pane"));
        };
        if let Some(check) = &check {
            validate_check_command(check).map_err(|error| ("invalid_params", error))?;
        }
        let root = {
            let mut inner = self.lock();
            match Self::role(&inner, pane_id) {
                Role::Worker => return Err(forbidden("worker panes cannot orchestrate")),
                Role::Orchestrator(id) => {
                    if inner.runs.get(&id).is_some_and(|run| !run.stopped) {
                        let run = inner.runs.get_mut(&id).expect("run checked above");
                        if check.is_some() {
                            run.check = check;
                        }
                        let run = run.clone();
                        self.commit(inner, &[&id])?;
                        return Ok(run);
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
            check,
            tasks: Vec::new(),
        };
        inner.runs.insert(id.clone(), run.clone());
        inner.orchestrators.insert(pane_id, id.clone());
        self.commit(inner, &[&id])?;
        Ok(run)
    }

    fn require_orchestrator(inner: &Inner, caller: Caller) -> Result<String, OrchestraError> {
        match caller {
            Caller::External => Err(forbidden(
                "tasks can only be created from an orchestrator pane",
            )),
            Caller::Pane(pane_id) => match Self::role(inner, pane_id) {
                Role::Worker => Err(forbidden("worker panes cannot create tasks")),
                Role::Plain => Err((
                    "not_orchestrator",
                    "run `terax orchestrate` in this pane first".to_string(),
                )),
                Role::Orchestrator(run) => Ok(run),
            },
        }
    }

    pub fn create_task(
        &self,
        caller: Caller,
        params: TaskCreateParams,
    ) -> Result<Created, OrchestraError> {
        let mut inner = self.lock();
        let run_id = Self::require_orchestrator(&inner, caller)?;
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
        if let Some(model) = &params.model {
            validate_model(model).map_err(|error| ("invalid_params", error))?;
        }
        let root = inner.root.clone().ok_or_else(unavailable)?;
        let admission = run::admit(inner.runs.values());
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
        if params.agent == "claude" {
            let settings = checks::claude_settings(&paths.dir);
            std::fs::write(&paths.claude_settings, settings.to_string())
                .map_err(|error| ("io_error", format!("write worker settings: {error}")))?;
        }
        let (status, evict) = match admission {
            Admission::Start { evict } => (TaskStatus::Starting, evict),
            Admission::Queue => (TaskStatus::Queued, None),
        };
        let task = TaskInfo {
            id: id.clone(),
            run: run_id.clone(),
            agent: params.agent,
            title: params.title.map(|title| title.trim().to_string()),
            status,
            branch: worktree::branch_name(&id),
            worktree: crate::modules::fs::to_canon(&paths.worktree),
            brief_path: crate::modules::fs::to_canon(&paths.brief),
            pane_id: None,
            error: None,
            model: params.model,
            checks: None,
            unreported: false,
        };
        run.tasks.push(task.clone());
        let mut evicted = None;
        let mut evicted_run = None;
        if let Some(evict_id) = evict {
            if let Ok(idle) = find_task_mut(&mut inner.runs, &evict_id) {
                evicted = idle.pane_id.take();
                evicted_run = Some(idle.run.clone());
            }
        }
        let mut changed = vec![run_id.as_str()];
        changed.extend(evicted_run.as_deref());
        self.commit(inner, &changed)?;
        Ok(Created {
            start: status == TaskStatus::Starting,
            task,
            evicted,
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
                        current.unreported = true;
                    }
                }
            }
            current.status == TaskStatus::Running
        };
        self.commit(inner, &[&task.run])?;
        result.map_err(|error| ("git_failed", error))?;
        if !still_running {
            return Err(("invalid_state", format!("task {task_id} was stopped")));
        }
        let settings_path =
            (task.agent == "claude").then(|| crate::modules::fs::to_canon(&paths.claude_settings));
        Ok(TaskOpenParams {
            task: task.id,
            agent: task.agent,
            title: task.title,
            cwd: task.worktree,
            brief_path: task.brief_path,
            model: task.model,
            settings_path,
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
            if task.status.is_working() || task.status == TaskStatus::Queued {
                task.status = TaskStatus::Failed;
                task.error = Some(error);
                task.unreported = true;
            }
        });
    }

    fn update_task(&self, task_id: &str, apply: impl FnOnce(&mut TaskInfo)) {
        let mut inner = self.lock();
        let Ok(task) = find_task_mut(&mut inner.runs, task_id) else {
            return;
        };
        apply(task);
        let run_id = task.run.clone();
        let _ = self.commit(inner, &[&run_id]);
    }

    /// Routes an agent hook signal from a pane's PTY to the task it runs.
    pub fn agent_signal(&self, pane_id: u32, kind: &str) -> SignalEffect {
        let Some(signal) = run::parse_signal(kind) else {
            return SignalEffect::default();
        };
        let mut inner = self.lock();
        let Some(root) = inner.root.clone() else {
            return SignalEffect::default();
        };
        let Some(run) = inner
            .runs
            .values_mut()
            .find(|run| run.tasks.iter().any(|task| task.pane_id == Some(pane_id)))
        else {
            return SignalEffect::default();
        };
        let check = run.check.clone();
        let run_id = run.id.clone();
        let task = run
            .tasks
            .iter_mut()
            .find(|task| task.pane_id == Some(pane_id))
            .expect("task found above");
        let before = (task.status, task.checks, task.unreported);
        let start_check = run::apply_signal(task, signal, check.is_some());
        if before == (task.status, task.checks, task.unreported) {
            return SignalEffect::default();
        }
        let promote = signal == run::Signal::Finished;
        let job = match (start_check, check) {
            (true, Some(command)) => {
                worktree::task_paths(&root, &task.run, &task.id)
                    .ok()
                    .map(|paths| CheckJob {
                        task: task.id.clone(),
                        command,
                        cwd: paths.worktree,
                        target: paths.target,
                        log: paths.check_log,
                    })
            }
            _ => None,
        };
        let _ = self.commit(inner, &[&run_id]);
        SignalEffect {
            check: job,
            promote,
        }
    }

    pub fn finish_check(&self, task_id: &str, outcome: Result<bool, String>) {
        self.update_task(task_id, |task| {
            if task.checks != Some(CheckStatus::Running) {
                return;
            }
            task.checks = Some(match outcome {
                Ok(true) => CheckStatus::Passed,
                Ok(false) => CheckStatus::Failed,
                Err(error) => {
                    task.error = Some(error);
                    CheckStatus::Failed
                }
            });
            task.unreported = true;
        });
    }

    pub fn list(&self, caller: Caller) -> Result<Vec<TaskInfo>, OrchestraError> {
        let inner = self.lock();
        let scope = Self::scope(&inner, caller)?;
        Ok(in_scope(&inner.runs, &scope).cloned().collect())
    }

    /// `Some(run)` limits an orchestrator to its own run; `None` means the
    /// human sees everything. Workers are refused.
    fn scope(inner: &Inner, caller: Caller) -> Result<Option<String>, OrchestraError> {
        match caller {
            Caller::External => Ok(None),
            Caller::Pane(pane_id) => match Self::role(inner, pane_id) {
                Role::Worker => Err(forbidden("worker panes cannot manage tasks")),
                Role::Orchestrator(run) => Ok(Some(run)),
                Role::Plain => Ok(None),
            },
        }
    }

    /// Blocks until a task needs the orchestrator: it finished (after its
    /// checks), asked a question, failed or stopped. Each event is reported
    /// once. Returns immediately with nothing when no task is in flight.
    pub fn wait(
        &self,
        caller: Caller,
        task_id: Option<&str>,
        timeout: Duration,
    ) -> Result<Vec<TaskInfo>, OrchestraError> {
        let deadline = Instant::now() + timeout;
        let mut inner = self.lock();
        loop {
            let scope = Self::scope(&inner, caller)?;
            if let Some(task_id) = task_id {
                let task = find_task_mut(&mut inner.runs, task_id)?;
                if scope.as_ref().is_some_and(|run| run != &task.run) {
                    return Err(forbidden("task belongs to another run"));
                }
                if run::is_settled(task) {
                    task.unreported = false;
                    let task = task.clone();
                    let _ = self.commit(inner, &[&task.run]);
                    return Ok(vec![task]);
                }
            } else {
                let mut ready = Vec::new();
                let mut in_flight = false;
                for run in inner.runs.values_mut() {
                    if scope.as_ref().is_some_and(|id| id != &run.id) {
                        continue;
                    }
                    for task in &mut run.tasks {
                        if task.unreported && run::is_settled(task) {
                            task.unreported = false;
                            ready.push(task.clone());
                        } else if run::is_busy(task) || task.status == TaskStatus::Queued {
                            in_flight = true;
                        }
                    }
                }
                if !ready.is_empty() {
                    let runs: Vec<String> = ready.iter().map(|task| task.run.clone()).collect();
                    let ids: Vec<&str> = runs.iter().map(String::as_str).collect();
                    let _ = self.commit(inner, &ids);
                    return Ok(ready);
                }
                if !in_flight {
                    return Ok(Vec::new());
                }
            }
            let now = Instant::now();
            if now >= deadline {
                return Ok(Vec::new());
            }
            inner = self
                .changed
                .wait_timeout(inner, deadline - now)
                .expect("orchestra state poisoned")
                .0;
        }
    }

    pub fn result(&self, caller: Caller, task_id: &str) -> Result<TaskResult, OrchestraError> {
        let (root, task) = {
            let inner = self.lock();
            let scope = Self::scope(&inner, caller)?;
            let (_, task) = find_task(&inner.runs, task_id)?;
            if scope.as_ref().is_some_and(|run| run != &task.run) {
                return Err(forbidden("task belongs to another run"));
            }
            (inner.root.clone().ok_or_else(unavailable)?, task.clone())
        };
        let paths = worktree::task_paths(&root, &task.run, &task.id).map_err(internal)?;
        Ok(TaskResult {
            result: checks::read_tail(&paths.result, checks::RESULT_MAX_BYTES),
            check_log: checks::read_tail(&paths.check_log, checks::LOG_TAIL_BYTES),
            task,
        })
    }

    /// Validates a follow-up for a worker and returns its pane.
    pub fn send_target(
        &self,
        caller: Caller,
        task_id: &str,
        text: &str,
    ) -> Result<u32, OrchestraError> {
        validate_send_text(text).map_err(|error| ("invalid_params", error))?;
        let mut inner = self.lock();
        let run_id = match Self::require_orchestrator(&inner, caller) {
            Ok(run) => Some(run),
            Err(_) if caller == Caller::External => None,
            Err(error) => return Err(error),
        };
        let task = find_task_mut(&mut inner.runs, task_id)?;
        if run_id.as_ref().is_some_and(|run| run != &task.run) {
            return Err(forbidden("task belongs to another run"));
        }
        let pane = match (task.status, task.pane_id) {
            (TaskStatus::Running | TaskStatus::NeedsInput | TaskStatus::Done, Some(pane)) => pane,
            _ => {
                return Err((
                    "invalid_state",
                    format!("task {task_id} has no open worker; create a follow-up task"),
                ))
            }
        };
        task.status = TaskStatus::Running;
        task.checks = None;
        task.unreported = false;
        let run = task.run.clone();
        self.commit(inner, &[&run])?;
        Ok(pane)
    }

    pub fn stop_task(&self, caller: Caller, task_id: &str) -> Result<Stopped, OrchestraError> {
        let mut inner = self.lock();
        let scope = Self::scope(&inner, caller)?;
        let task = find_task_mut(&mut inner.runs, task_id)?;
        if scope.as_ref().is_some_and(|run| run != &task.run) {
            return Err(forbidden("task belongs to another run"));
        }
        let mut stopped = Stopped::default();
        stop_one(task, &mut stopped);
        let run = task.run.clone();
        self.commit(inner, &[&run])?;
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
        let scope = Self::scope(&inner, caller)?;
        let targets: Vec<String> = match (run_id, scope) {
            (Some(id), Some(own)) if id != own => {
                return Err(forbidden("run belongs to another orchestrator"))
            }
            (Some(id), _) => vec![id.to_string()],
            (None, Some(own)) => vec![own],
            (None, None) => inner.runs.keys().cloned().collect(),
        };
        let mut stopped = Stopped::default();
        for id in &targets {
            let run = inner
                .runs
                .get_mut(id)
                .ok_or_else(|| ("not_found", format!("unknown run {id}")))?;
            run.stopped = true;
            for task in &mut run.tasks {
                stop_one(task, &mut stopped);
            }
        }
        inner.orchestrators.retain(|_, run| !targets.contains(run));
        let ids: Vec<&str> = targets.iter().map(String::as_str).collect();
        self.commit(inner, &ids)?;
        Ok(stopped)
    }

    pub fn promote(&self) -> Promotion {
        let mut inner = self.lock();
        let promotion = run::promote(&mut inner.runs);
        if promotion.started.is_empty() {
            return promotion;
        }
        let runs: Vec<String> = inner.runs.keys().cloned().collect();
        let ids: Vec<&str> = runs.iter().map(String::as_str).collect();
        let _ = self.commit(inner, &ids);
        promotion
    }
}

fn in_scope<'a>(
    runs: &'a BTreeMap<String, Run>,
    scope: &'a Option<String>,
) -> impl Iterator<Item = &'a TaskInfo> {
    runs.values()
        .filter(move |run| scope.as_ref().is_none_or(|id| &run.id == id))
        .flat_map(|run| run.tasks.iter())
}

/// Working tasks stop; a finished task keeps its result and only loses its tab.
fn stop_one(task: &mut TaskInfo, stopped: &mut Stopped) {
    if !task.status.is_active() {
        return;
    }
    if task.status != TaskStatus::Done {
        task.status = TaskStatus::Stopped;
        task.unreported = false;
    }
    if task.checks == Some(CheckStatus::Running) {
        task.checks = None;
    }
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
    use std::sync::Arc;

    struct Fixture {
        orchestra: Arc<Orchestra>,
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
        let orchestra = Arc::new(Orchestra::default());
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
            model: None,
        }
    }

    impl Fixture {
        fn orchestrate(&self, pane: u32) -> Run {
            self.orchestra
                .orchestrate(Caller::Pane(pane), self.repo.path(), None)
                .unwrap()
        }

        /// Creates a task and opens its worker the way the control server does.
        fn start(&self, pane: u32, worker_pane: u32) -> TaskInfo {
            let created = self
                .orchestra
                .create_task(Caller::Pane(pane), params("do it"))
                .unwrap();
            assert!(created.start);
            self.orchestra.prepare_worktree(&created.task.id).unwrap();
            self.orchestra.attach_pane(&created.task.id, worker_pane);
            self.task(&created.task.id)
        }

        fn task(&self, id: &str) -> TaskInfo {
            self.orchestra
                .list(Caller::External)
                .unwrap()
                .into_iter()
                .find(|t| t.id == id)
                .unwrap()
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
                .orchestrate(Caller::External, f.repo.path(), None)
                .unwrap_err()
                .0,
            "forbidden"
        );
    }

    #[test]
    fn a_worker_pane_cannot_create_tasks_or_become_orchestrator() {
        let f = fixture();
        let run = f.orchestrate(1);
        let task = f.start(1, 2);
        let env = f
            .orchestra
            .register_pane(2, Some(Path::new(&task.worktree)));
        assert!(env.contains(&("TERAX_TASK_ID".into(), task.id.clone())));
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
        for err in [
            f.orchestra
                .orchestrate(Caller::Pane(2), f.repo.path(), None)
                .unwrap_err(),
            f.orchestra.list(Caller::Pane(2)).unwrap_err(),
            f.orchestra
                .stop_task(Caller::Pane(2), &task.id)
                .unwrap_err(),
            f.orchestra
                .wait(Caller::Pane(2), None, Duration::ZERO)
                .unwrap_err(),
            f.orchestra
                .send_target(Caller::Pane(2), &task.id, "hi")
                .unwrap_err(),
        ] {
            assert_eq!(err.0, "forbidden");
        }

        let nested = Path::new(&task.worktree).join("sub");
        std::fs::create_dir(&nested).unwrap();
        f.orchestra.register_pane(3, Some(&nested));
        assert_eq!(
            f.orchestra
                .orchestrate(Caller::Pane(3), &nested, None)
                .unwrap_err()
                .0,
            "forbidden"
        );
    }

    #[test]
    fn claude_workers_get_sandbox_settings_inside_their_task_dir() {
        let f = fixture();
        let run = f.orchestrate(1);
        let created = f
            .orchestra
            .create_task(Caller::Pane(1), params("x"))
            .unwrap();
        let open = f.orchestra.prepare_worktree(&created.task.id).unwrap();
        let settings = PathBuf::from(open.settings_path.unwrap());
        assert!(settings.starts_with(f.root.join(&run.id)));
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(settings).unwrap()).unwrap();
        assert_eq!(value["sandbox"]["enabled"], true);
    }

    #[test]
    fn an_orchestrator_respawned_inside_a_worktree_loses_its_role() {
        let f = fixture();
        f.orchestrate(1);
        let task = f.start(1, 5);
        f.orchestra
            .register_pane(1, Some(Path::new(&task.worktree)));
        let err = f
            .orchestra
            .create_task(Caller::Pane(1), params("x"))
            .unwrap_err();
        assert_eq!(err.0, "forbidden");
    }

    #[test]
    fn finishing_frees_the_slot_and_evicts_the_oldest_idle_tab() {
        let f = fixture();
        f.orchestrate(1);
        let workers: Vec<TaskInfo> = (0..4).map(|n| f.start(1, 10 + n)).collect();
        let queued = f
            .orchestra
            .create_task(Caller::Pane(1), params("fifth"))
            .unwrap();
        assert!(!queued.start);

        let effect = f.orchestra.agent_signal(10, "finished");
        assert!(effect.promote && effect.check.is_none());
        assert_eq!(f.task(&workers[0].id).status, TaskStatus::Done);
        let promotion = f.orchestra.promote();
        assert_eq!(promotion.started, vec![queued.task.id.clone()]);
        assert_eq!(promotion.evicted, vec![10]);
        assert_eq!(f.task(&workers[0].id).pane_id, None);
        assert!(
            Path::new(&workers[0].worktree).join(".git").exists(),
            "eviction keeps the worktree"
        );
    }

    #[test]
    fn wait_reports_each_settled_task_once_and_returns_when_idle() {
        let f = fixture();
        f.orchestrate(1);
        let task = f.start(1, 7);
        let orchestra = f.orchestra.clone();
        let waiter = std::thread::spawn(move || {
            orchestra.wait(Caller::Pane(1), None, Duration::from_secs(10))
        });
        std::thread::sleep(Duration::from_millis(50));
        f.orchestra.agent_signal(7, "attention");
        let reported = waiter.join().unwrap().unwrap();
        assert_eq!(reported.len(), 1);
        assert_eq!(reported[0].status, TaskStatus::NeedsInput);

        let again = f
            .orchestra
            .wait(Caller::Pane(1), None, Duration::from_millis(20))
            .unwrap();
        assert!(again.is_empty(), "an event is reported once");

        f.orchestra.agent_signal(7, "finished");
        let done = f
            .orchestra
            .wait(Caller::Pane(1), Some(&task.id), Duration::from_secs(1))
            .unwrap();
        assert_eq!(done[0].status, TaskStatus::Done);
        let start = Instant::now();
        let idle = f
            .orchestra
            .wait(Caller::Pane(1), None, Duration::from_secs(10))
            .unwrap();
        assert!(idle.is_empty());
        assert!(
            start.elapsed() < Duration::from_secs(1),
            "nothing in flight"
        );
    }

    #[cfg(unix)]
    #[test]
    fn checks_run_after_finishing_and_gate_the_report() {
        let f = fixture();
        f.orchestra
            .orchestrate(Caller::Pane(1), f.repo.path(), Some("exit 1".into()))
            .unwrap();
        let task = f.start(1, 8);
        let job = f.orchestra.agent_signal(8, "finished").check.unwrap();
        assert_eq!(f.task(&task.id).checks, Some(CheckStatus::Running));
        assert!(f
            .orchestra
            .wait(Caller::Pane(1), None, Duration::from_millis(20))
            .unwrap()
            .is_empty());
        f.orchestra.finish_check(&job.task, checks::run(&job));
        let reported = f
            .orchestra
            .wait(Caller::Pane(1), None, Duration::from_secs(1))
            .unwrap();
        assert_eq!(reported[0].checks, Some(CheckStatus::Failed));

        std::fs::write(
            f.root.join(&task.run).join(&task.id).join("result.md"),
            "did the thing",
        )
        .unwrap();
        let result = f.orchestra.result(Caller::Pane(1), &task.id).unwrap();
        assert_eq!(result.result.as_deref(), Some("did the thing"));
        assert!(result.check_log.is_some());
    }

    #[test]
    fn follow_ups_reopen_a_finished_task_and_need_an_open_tab() {
        let f = fixture();
        f.orchestrate(1);
        let task = f.start(1, 9);
        f.orchestra.agent_signal(9, "finished");
        assert_eq!(
            f.orchestra
                .send_target(Caller::Pane(1), &task.id, "also add docs")
                .unwrap(),
            9
        );
        assert_eq!(f.task(&task.id).status, TaskStatus::Running);
        assert_eq!(
            f.orchestra
                .send_target(Caller::Pane(1), &task.id, "two\nlines")
                .unwrap_err()
                .0,
            "invalid_params"
        );
        f.orchestra.stop_task(Caller::Pane(1), &task.id).unwrap();
        assert_eq!(
            f.orchestra
                .send_target(Caller::Pane(1), &task.id, "more")
                .unwrap_err()
                .0,
            "invalid_state"
        );
    }

    #[test]
    fn orchestrators_see_only_their_run_and_humans_stop_everything() {
        let f = fixture();
        let other_repo = fixture();
        let a = f.orchestrate(1);
        let b = f
            .orchestra
            .orchestrate(Caller::Pane(2), other_repo.repo.path(), None)
            .unwrap();
        assert_ne!(a.id, b.id);
        assert_eq!(f.orchestrate(1).id, a.id);
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
    fn invalid_agents_models_and_briefs_are_rejected_before_anything_is_written() {
        let f = fixture();
        f.orchestrate(1);
        let mut bad = params("x");
        bad.agent = "sh -c evil".into();
        assert_eq!(
            f.orchestra.create_task(Caller::Pane(1), bad).unwrap_err().0,
            "invalid_params"
        );
        let mut bad = params("x");
        bad.model = Some("--yolo".into());
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
        f.orchestrate(1);
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
