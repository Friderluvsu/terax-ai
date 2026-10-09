//! `terax-preview://` serves one authorized project root per token to a
//! sandboxed preview frame. Tauri treats custom-scheme origins as local, so
//! these frames must never get Tauri scripts and the main frame must never
//! navigate to the scheme.

pub mod handler;
pub mod pure;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use serde::Serialize;
use tauri::State;

use crate::modules::fs::to_canon;
use crate::modules::fs::watch::{unwatch_preview_dirs, FsWatchState};
use crate::modules::security::secret_paths::is_secret_path;
use crate::modules::workspace::{WorkspaceEnv, WorkspaceRegistry};

pub use pure::SCHEME;

const MAX_ROOTS: usize = 32;

pub struct PreviewRoot {
    pub root: PathBuf,
    pub entry: PathBuf,
    pub served_dirs: HashSet<PathBuf>,
    last_used: u64,
}

#[derive(Default)]
pub struct PreviewState {
    roots: Mutex<HashMap<String, PreviewRoot>>,
    clock: AtomicU64,
}

impl PreviewState {
    fn tick(&self) -> u64 {
        self.clock.fetch_add(1, Ordering::Relaxed) + 1
    }

    /// Returns the served dirs of an evicted least-recently-used entry.
    pub fn insert(&self, token: String, root: PathBuf, entry: PathBuf) -> HashSet<PathBuf> {
        let now = self.tick();
        let mut roots = self.roots.lock().expect("preview state poisoned");
        let mut evicted = HashSet::new();
        if roots.len() >= MAX_ROOTS && !roots.contains_key(&token) {
            let oldest = roots
                .iter()
                .min_by_key(|(_, r)| r.last_used)
                .map(|(t, _)| t.clone());
            if let Some(old) = oldest.and_then(|t| roots.remove(&t)) {
                evicted = old.served_dirs;
            }
        }
        roots.insert(
            token,
            PreviewRoot {
                root,
                entry,
                served_dirs: HashSet::new(),
                last_used: now,
            },
        );
        evicted
    }

    pub fn contains(&self, token: &str) -> bool {
        self.roots
            .lock()
            .expect("preview state poisoned")
            .contains_key(token)
    }

    /// Root for a live token, refreshing its LRU position.
    pub fn touch(&self, token: &str) -> Option<PathBuf> {
        let now = self.tick();
        let mut roots = self.roots.lock().expect("preview state poisoned");
        let entry = roots.get_mut(token)?;
        entry.last_used = now;
        Some(entry.root.clone())
    }

    pub fn entry(&self, token: &str) -> Option<(PathBuf, PathBuf)> {
        let roots = self.roots.lock().expect("preview state poisoned");
        roots.get(token).map(|r| (r.root.clone(), r.entry.clone()))
    }

    pub fn remove(&self, token: &str) -> HashSet<PathBuf> {
        self.roots
            .lock()
            .expect("preview state poisoned")
            .remove(token)
            .map(|r| r.served_dirs)
            .unwrap_or_default()
    }

    /// Holds the lock across `watch` so a concurrent release either sees the
    /// dir in `served_dirs` or the watch is never added.
    pub fn record_served_dir(&self, token: &str, dir: PathBuf, watch: impl FnOnce(&Path) -> bool) {
        let mut roots = self.roots.lock().expect("preview state poisoned");
        let Some(entry) = roots.get_mut(token) else {
            return;
        };
        if entry.served_dirs.contains(&dir) {
            return;
        }
        if watch(&dir) {
            entry.served_dirs.insert(dir);
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct ServePlan {
    pub root: PathBuf,
    pub entry: PathBuf,
    pub encoded_rel: String,
}

fn is_project_dir(dir: &Path) -> bool {
    dir.join(".git").exists() || dir.join("package.json").is_file()
}

pub fn plan_serve(
    path: &str,
    root_hint: Option<&str>,
    workspace: &WorkspaceEnv,
    registry: &WorkspaceRegistry,
    home: Option<&Path>,
) -> Result<ServePlan, String> {
    // A remote workspace path must never be served from the local disk.
    if !matches!(workspace, WorkspaceEnv::Local) {
        return Err("HTML preview is only available in local workspaces".into());
    }
    let file = std::fs::canonicalize(path)
        .map_err(|error| format!("preview file is not accessible: {error}"))?;
    if !file.is_file() {
        return Err("preview target is not a file".into());
    }
    let hint = match root_hint.map(str::trim).filter(|h| !h.is_empty()) {
        Some(hint) => Some(
            std::fs::canonicalize(hint)
                .map_err(|error| format!("preview root is not accessible: {error}"))?,
        ),
        None => None,
    };
    if hint.as_deref().is_some_and(|h| !file.starts_with(h)) {
        return Err("preview file is outside the preview root".into());
    }
    let root = pure::choose_root(&file, hint.as_deref(), home, is_project_dir)
        .ok_or("files directly in the home directory or a filesystem root cannot be previewed")?;
    if !registry.is_authorized(&root) {
        return Err(format!(
            "preview root is not authorized: {}",
            root.display()
        ));
    }
    if is_secret_path(&root) || is_secret_path(&file) {
        return Err("preview of a protected path is not allowed".into());
    }
    let rel = file
        .strip_prefix(&root)
        .map_err(|_| "preview file is outside the preview root".to_string())?;
    let encoded_rel = pure::encode_rel_path(rel)
        .filter(|enc| pure::sanitize_rel_path(enc).is_ok_and(|back| back == rel))
        .ok_or("preview file name cannot be served")?;
    Ok(ServePlan {
        root,
        entry: file,
        encoded_rel,
    })
}

fn generate_token() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| format!("generate preview token: {error}"))?;
    let mut token = String::with_capacity(pure::TOKEN_LEN);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(token, "{byte:02x}");
    }
    Ok(token)
}

fn canonical_home() -> Option<PathBuf> {
    dirs::home_dir().map(|home| std::fs::canonicalize(&home).unwrap_or(home))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewServeResult {
    pub token: String,
    pub url: String,
    pub root: String,
}

#[tauri::command]
pub fn preview_serve(
    path: String,
    root_hint: Option<String>,
    workspace: Option<WorkspaceEnv>,
    state: State<'_, PreviewState>,
    registry: State<'_, WorkspaceRegistry>,
    watch: State<'_, FsWatchState>,
) -> Result<PreviewServeResult, String> {
    let workspace = WorkspaceEnv::from_option(workspace);
    let home = canonical_home();
    let plan = plan_serve(
        &path,
        root_hint.as_deref(),
        &workspace,
        &registry,
        home.as_deref(),
    )?;
    let mut token = generate_token()?;
    while state.contains(&token) {
        token = generate_token()?;
    }
    let url = pure::preview_url(&token, &plan.encoded_rel);
    let root = to_canon(&plan.root);
    let evicted = state.insert(token.clone(), plan.root, plan.entry);
    unwatch_preview_dirs(&watch, evicted);
    Ok(PreviewServeResult { token, url, root })
}

#[tauri::command]
pub fn preview_release(
    token: String,
    state: State<'_, PreviewState>,
    watch: State<'_, FsWatchState>,
) {
    unwatch_preview_dirs(&watch, state.remove(&token));
}

#[tauri::command]
pub async fn preview_open_external(
    token: String,
    state: State<'_, PreviewState>,
) -> Result<(), String> {
    let (root, entry) = state.entry(&token).ok_or("unknown preview")?;
    tauri::async_runtime::spawn_blocking(move || {
        let canonical = std::fs::canonicalize(&entry)
            .map_err(|error| format!("preview file is not accessible: {error}"))?;
        if !canonical.starts_with(&root) || is_secret_path(&canonical) {
            return Err("preview file is no longer servable".to_string());
        }
        open_in_browser(&canonical)
    })
    .await
    .map_err(|error| error.to_string())?
}

// Terax is registered for .html, so a plain open could route back into Terax;
// use the app macOS picks for https: links instead.
#[cfg(target_os = "macos")]
fn open_in_browser(path: &Path) -> Result<(), String> {
    use objc2_app_kit::NSWorkspace;
    use objc2_foundation::{NSString, NSURL};

    let probe = NSURL::URLWithString(&NSString::from_str("https://example.com"))
        .ok_or("could not build browser probe URL")?;
    let browser = NSWorkspace::sharedWorkspace()
        .URLForApplicationToOpenURL(&probe)
        .and_then(|url| url.path())
        .ok_or("no default browser is configured")?
        .to_string();
    let status = std::process::Command::new("/usr/bin/open")
        .arg("-a")
        .arg(&browser)
        .arg(path)
        .status()
        .map_err(|error| format!("could not open browser: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("could not open browser: {status}"))
    }
}

#[cfg(not(target_os = "macos"))]
fn open_in_browser(path: &Path) -> Result<(), String> {
    tauri_plugin_opener::open_path(path, None::<&str>).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry_for(root: &Path) -> WorkspaceRegistry {
        let registry = WorkspaceRegistry::default();
        registry.authorize(root).expect("authorize");
        registry
    }

    fn canon(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).expect("canon")
    }

    #[test]
    fn rejects_non_local_workspaces() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.html");
        std::fs::write(&file, b"x").expect("write");
        let registry = registry_for(dir.path());
        let wsl = WorkspaceEnv::Wsl {
            distro: "Ubuntu".into(),
        };
        let err = plan_serve(file.to_str().expect("utf8"), None, &wsl, &registry, None)
            .expect_err("non-local must be rejected");
        assert!(err.contains("local"), "{err}");
    }

    #[test]
    fn rejects_unauthorized_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let file = dir.path().join("a.html");
        std::fs::write(&file, b"x").expect("write");
        let registry = WorkspaceRegistry::default();
        let err = plan_serve(
            file.to_str().expect("utf8"),
            None,
            &WorkspaceEnv::Local,
            &registry,
            None,
        )
        .expect_err("unauthorized");
        assert!(err.contains("not authorized"), "{err}");
    }

    #[test]
    fn rejects_file_outside_root_hint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let other = tempfile::tempdir().expect("other");
        let file = dir.path().join("a.html");
        std::fs::write(&file, b"x").expect("write");
        let registry = registry_for(dir.path());
        let err = plan_serve(
            file.to_str().expect("utf8"),
            other.path().to_str(),
            &WorkspaceEnv::Local,
            &registry,
            None,
        )
        .expect_err("outside");
        assert!(err.contains("outside"), "{err}");
    }

    #[test]
    fn picks_git_project_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let project = dir.path().join("proj");
        std::fs::create_dir_all(project.join(".git")).expect("mkdir");
        std::fs::create_dir_all(project.join("site/pages")).expect("mkdir");
        let file = project.join("site/pages/a b.html");
        std::fs::write(&file, b"x").expect("write");
        let registry = registry_for(dir.path());
        let plan = plan_serve(
            file.to_str().expect("utf8"),
            None,
            &WorkspaceEnv::Local,
            &registry,
            None,
        )
        .expect("plan");
        assert_eq!(plan.root, canon(&project));
        assert_eq!(plan.entry, canon(&file));
        assert_eq!(plan.encoded_rel, "site/pages/a%20b.html");
    }

    #[test]
    fn file_without_project_uses_its_dir() {
        let dir = tempfile::tempdir().expect("tempdir");
        let notes = dir.path().join("notes");
        std::fs::create_dir_all(&notes).expect("mkdir");
        let file = notes.join("a.html");
        std::fs::write(&file, b"x").expect("write");
        let registry = registry_for(dir.path());
        let plan = plan_serve(
            file.to_str().expect("utf8"),
            None,
            &WorkspaceEnv::Local,
            &registry,
            Some(&canon(dir.path())),
        )
        .expect("plan");
        assert_eq!(plan.root, canon(&notes));
    }

    #[test]
    fn file_directly_in_home_is_never_rooted_at_home() {
        let home = tempfile::tempdir().expect("home");
        std::fs::create_dir(home.path().join(".git")).expect("mkdir");
        std::fs::write(home.path().join("package.json"), b"{}").expect("write");
        let file = home.path().join("a.html");
        std::fs::write(&file, b"x").expect("write");
        let registry = registry_for(home.path());
        let result = plan_serve(
            file.to_str().expect("utf8"),
            None,
            &WorkspaceEnv::Local,
            &registry,
            Some(&canon(home.path())),
        );
        assert!(result.is_err(), "{result:?}");

        let sub = home.path().join("site");
        std::fs::create_dir(&sub).expect("mkdir");
        let nested = sub.join("a.html");
        std::fs::write(&nested, b"x").expect("write");
        let plan = plan_serve(
            nested.to_str().expect("utf8"),
            None,
            &WorkspaceEnv::Local,
            &registry,
            Some(&canon(home.path())),
        )
        .expect("plan");
        assert_eq!(plan.root, canon(&sub));
    }

    #[test]
    fn rejects_protected_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ssh = dir.path().join(".ssh");
        std::fs::create_dir(&ssh).expect("mkdir");
        let file = ssh.join("a.html");
        std::fs::write(&file, b"x").expect("write");
        let registry = registry_for(dir.path());
        let err = plan_serve(
            file.to_str().expect("utf8"),
            None,
            &WorkspaceEnv::Local,
            &registry,
            None,
        )
        .expect_err("protected");
        assert!(err.contains("protected"), "{err}");
    }

    #[test]
    fn lru_evicts_oldest_and_returns_its_dirs() {
        let state = PreviewState::default();
        for i in 0..MAX_ROOTS {
            state.insert(format!("t{i}"), PathBuf::from("/r"), PathBuf::from("/r/a"));
        }
        state.record_served_dir("t1", PathBuf::from("/r/x"), |_| true);
        assert!(state.touch("t0").is_some());
        let evicted = state.insert("new".into(), PathBuf::from("/r"), PathBuf::from("/r/a"));
        assert_eq!(evicted, HashSet::from([PathBuf::from("/r/x")]));
        assert!(state.contains("t0"));
        assert!(!state.contains("t1"));
        assert!(state.contains("new"));
    }

    #[test]
    fn release_returns_only_watched_dirs() {
        let state = PreviewState::default();
        state.insert("t".into(), PathBuf::from("/r"), PathBuf::from("/r/a"));
        state.record_served_dir("t", PathBuf::from("/r/ok"), |_| true);
        state.record_served_dir("t", PathBuf::from("/r/fail"), |_| false);
        state.record_served_dir("gone", PathBuf::from("/r/x"), |_| panic!("no watch"));
        let dirs = state.remove("t");
        assert_eq!(dirs, HashSet::from([PathBuf::from("/r/ok")]));
        assert!(state.remove("t").is_empty());
    }

    #[test]
    fn token_is_a_valid_host_label() {
        let token = generate_token().expect("token");
        assert!(pure::is_token(&token));
    }
}
