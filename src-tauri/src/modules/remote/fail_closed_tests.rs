use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::modules::fs::watch;
use crate::modules::git::{operations, utils};
use crate::modules::shell::{background, build_oneshot_command, run_blocking_inner, session};
use crate::modules::workspace::{
    authorize_spawn_cwd, authorize_user_spawn_cwd, local_path, user_spawn_cwd_or_home,
    WorkspaceEnv, WorkspaceRegistry, SSH_UNSUPPORTED,
};

const ORIGINAL: &str = "local secret\n";

fn ssh() -> WorkspaceEnv {
    WorkspaceEnv::Ssh {
        host: "example-host".into(),
    }
}

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    file: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = fs::canonicalize(dir.path()).expect("canonical tempdir");
        let file = root.join("a.txt");
        fs::write(&file, ORIGINAL).expect("seed file");
        fs::create_dir(root.join("sub")).expect("seed dir");
        Self {
            _dir: dir,
            root,
            file,
        }
    }

    fn root_str(&self) -> String {
        s(&self.root)
    }

    fn registry(&self) -> WorkspaceRegistry {
        let reg = WorkspaceRegistry::default();
        reg.authorize(&self.root).expect("authorize root");
        reg
    }

    fn assert_untouched(&self) {
        assert_eq!(fs::read_to_string(&self.file).expect("seed file"), ORIGINAL);
        let names: BTreeSet<String> = fs::read_dir(&self.root)
            .expect("read root")
            .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
            .collect();
        let expected: BTreeSet<String> = ["a.txt", "sub"].iter().map(|n| n.to_string()).collect();
        assert_eq!(names, expected);
        assert_eq!(
            fs::read_dir(self.root.join("sub"))
                .expect("read sub")
                .count(),
            0
        );
    }
}

fn s(p: &Path) -> String {
    p.to_string_lossy().into_owned()
}

#[track_caller]
fn assert_ssh_err<T>(result: Result<T, String>) {
    match result {
        Ok(_) => panic!("ssh workspace must fail closed"),
        Err(e) => assert!(e.contains(SSH_UNSUPPORTED), "unexpected error: {e}"),
    }
}

#[test]
fn deserializes_ssh_variant() {
    let ws: WorkspaceEnv = serde_json::from_str(r#"{"kind":"ssh","host":"oracle"}"#).expect("ssh");
    assert!(matches!(ws, WorkspaceEnv::Ssh { ref host } if host == "oracle"));
    let ws: WorkspaceEnv = serde_json::from_str(r#"{"kind":"local"}"#).expect("local");
    assert!(matches!(ws, WorkspaceEnv::Local));
    let ws: WorkspaceEnv =
        serde_json::from_str(r#"{"kind":"wsl","distro":"Ubuntu"}"#).expect("wsl");
    assert!(matches!(ws, WorkspaceEnv::Wsl { ref distro } if distro == "Ubuntu"));
}

#[test]
fn local_path_rejects_ssh_and_keeps_local() {
    assert_ssh_err(local_path("/etc/hosts", &ssh()));
    assert_eq!(
        local_path("/etc/hosts", &WorkspaceEnv::Local),
        Ok(PathBuf::from("/etc/hosts"))
    );
}

#[test]
fn workspace_auth_rejects_ssh() {
    let fx = Fixture::new();
    let reg = fx.registry();
    let root = fx.root_str();
    assert_ssh_err(authorize_spawn_cwd(&reg, Some(&root), &ssh()));
    assert_ssh_err(authorize_spawn_cwd(&reg, None, &ssh()));

    let fresh = WorkspaceRegistry::default();
    assert_ssh_err(authorize_user_spawn_cwd(&fresh, Some(&root), &ssh()));
    assert!(!fresh.is_authorized(&fx.root));
    assert_eq!(user_spawn_cwd_or_home(&fresh, Some(&root), &ssh()), None);
    assert!(!fresh.is_authorized(&fx.root));
}

// File writes and mutations are served over SFTP (remote::fs::dispatch);
// the watcher is not ported yet and must still fail closed.
#[test]
fn fs_watch_rejects_ssh() {
    let fx = Fixture::new();
    let reg = fx.registry();
    let root = fx.root_str();
    assert_ssh_err(watch::prepare_add(&reg, &ssh(), vec![root.clone()]));
    assert_ssh_err(watch::prepare_remove(&ssh(), vec![root]));
    fx.assert_untouched();
}

#[test]
fn shell_rejects_ssh_without_spawning() {
    let fx = Fixture::new();
    let root = fx.root_str();
    let marker = fx.root.join("marker");
    let touch = format!("touch '{}'", s(&marker));
    let dur = Duration::from_secs(5);

    assert_ssh_err(build_oneshot_command(&touch, &ssh(), Some(&root)));
    assert_ssh_err(run_blocking_inner(
        touch.clone(),
        Some(root.clone()),
        ssh(),
        dur,
    ));
    assert_ssh_err(run_blocking_inner(touch.clone(), None, ssh(), dur));
    assert_ssh_err(background::spawn(touch.clone(), Some(root.clone()), ssh()).map(|_| ()));
    assert_ssh_err(background::spawn(touch.clone(), None, ssh()).map(|_| ()));

    let remote = session::ShellSession::new(root.clone(), ssh());
    assert_ssh_err(remote.run(touch.clone(), Some(root.clone()), None, dur));
    let local = session::ShellSession::new(root.clone(), WorkspaceEnv::Local);
    assert_ssh_err(local.run(touch, Some(root), Some(ssh()), dur));

    assert!(!marker.exists());
    fx.assert_untouched();
}

#[test]
fn pty_rejects_ssh_and_withholds_control_env() {
    let fx = Fixture::new();
    assert_ssh_err(
        crate::modules::pty::shell_init::build_command(
            Some(fx.root_str()),
            ssh(),
            true,
            None,
            None,
            None,
        )
        .map(|_| ()),
    );
    assert!(!crate::modules::pty::control_env_allowed(&ssh()));
    assert!(!crate::modules::pty::control_env_allowed(
        &WorkspaceEnv::Wsl {
            distro: "Ubuntu".into(),
        }
    ));
    assert!(crate::modules::pty::control_env_allowed(
        &WorkspaceEnv::Local
    ));
    fx.assert_untouched();
}

#[test]
fn lsp_accepts_only_local() {
    assert_ssh_err(crate::modules::lsp::ensure_lsp_workspace(&ssh()));
    assert!(
        crate::modules::lsp::ensure_lsp_workspace(&WorkspaceEnv::Wsl {
            distro: "Ubuntu".into(),
        })
        .is_err()
    );
    assert!(crate::modules::lsp::ensure_lsp_workspace(&WorkspaceEnv::Local).is_ok());
}

#[test]
fn git_rejects_ssh() {
    let fx = Fixture::new();
    let reg = fx.registry();
    let root = fx.root_str();
    assert_ssh_err(utils::canonical_dir(&reg, &root, &ssh()).map_err(String::from));
    assert_ssh_err(utils::authorized_repo_root(&reg, &root, &ssh()).map_err(String::from));
    assert_ssh_err(operations::resolve_repo(&reg, &root, &ssh()).map_err(String::from));
    fx.assert_untouched();
}
