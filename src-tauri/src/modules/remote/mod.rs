#[cfg(unix)]
pub mod control;
#[cfg(unix)]
pub mod exec;
#[cfg(test)]
mod fail_closed_tests;
pub mod fs;
pub mod quote;
pub mod registry;
pub mod shell_init;
pub mod ssh_config;
pub mod state;
pub mod target;

pub use registry::RemoteRegistry;
use ssh_config::SshHost;
pub use state::RemoteState;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn ssh_list_hosts() -> Result<Vec<SshHost>, String> {
    tauri::async_runtime::spawn_blocking(ssh_config::list_user_hosts)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(unix)]
mod ops {
    use super::control;
    use super::exec::exec;
    use std::path::Path;
    use std::time::Duration;

    const HOME_TIMEOUT: Duration = Duration::from_secs(20);

    pub fn home(dir: &Path, host: &str) -> Result<String, String> {
        let out = exec(dir, host, "printf %s \"$HOME\"", HOME_TIMEOUT, 64 * 1024)?;
        control::ensure_success(&out)?;
        let home = String::from_utf8(out.stdout)
            .map_err(|_| "remote home is not valid UTF-8".to_string())?;
        if !home.starts_with('/') || out.truncated {
            return Err("remote home is not an absolute path".into());
        }
        Ok(home)
    }
}

#[cfg(unix)]
async fn blocking<T, F>(f: F) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, String> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(not(unix))]
const UNSUPPORTED: &str = "SSH workspaces are not supported on this platform";

#[tauri::command]
pub async fn ssh_check(
    app: AppHandle,
    state: State<'_, RemoteState>,
    host: String,
) -> Result<bool, String> {
    #[cfg(unix)]
    {
        target::validate_alias(&host)?;
        let dir = state.socket_dir(&app.config().identifier)?;
        blocking(move || control::check(&dir, &host)).await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, state, host);
        Err(UNSUPPORTED.into())
    }
}

#[tauri::command]
pub async fn ssh_disconnect(
    app: AppHandle,
    state: State<'_, RemoteState>,
    host: String,
) -> Result<bool, String> {
    #[cfg(unix)]
    {
        target::validate_alias(&host)?;
        let dir = state.socket_dir(&app.config().identifier)?;
        state.sftp_disconnect(&host);
        blocking(move || control::exit(&dir, &host)).await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, state, host);
        Err(UNSUPPORTED.into())
    }
}

#[tauri::command]
pub async fn ssh_home(
    app: AppHandle,
    state: State<'_, RemoteState>,
    host: String,
) -> Result<String, String> {
    #[cfg(unix)]
    {
        target::validate_alias(&host)?;
        let dir = state.socket_dir(&app.config().identifier)?;
        blocking(move || ops::home(&dir, &host)).await
    }
    #[cfg(not(unix))]
    {
        let _ = (app, state, host);
        Err(UNSUPPORTED.into())
    }
}

#[cfg(unix)]
pub type SshResolved = control::SshResolved;
#[cfg(not(unix))]
pub type SshResolved = ();

#[tauri::command]
pub async fn ssh_resolve(host: String) -> Result<SshResolved, String> {
    #[cfg(unix)]
    {
        target::validate_alias(&host)?;
        blocking(move || control::resolve(&host)).await
    }
    #[cfg(not(unix))]
    {
        let _ = host;
        Err(UNSUPPORTED.into())
    }
}

#[cfg(all(test, unix))]
mod integration_tests {
    use super::control::{self, build_args, control_path, Mode};
    use super::exec::exec;
    use super::ops;
    use super::quote::posix_quote;
    use std::path::{Path, PathBuf};
    use std::process::{Command, Stdio};
    use std::time::Duration;

    struct Master<'a> {
        dir: &'a Path,
        host: &'a str,
    }

    impl Drop for Master<'_> {
        fn drop(&mut self) {
            let _ = control::exit(self.dir, self.host);
        }
    }

    fn start_master(dir: &Path, host: &str) {
        let args = build_args(
            Mode::Interactive,
            &control_path(dir),
            &["-o", "BatchMode=yes", "-f", "-N"],
            host,
            &[],
        )
        .unwrap();
        let status = Command::new(control::ssh_binary().unwrap())
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "could not start a master for {host}");
    }

    #[test]
    #[ignore = "needs TERAX_SSH_TEST_HOST"]
    fn remote_check_exec_home_disconnect() {
        let Ok(host) = std::env::var("TERAX_SSH_TEST_HOST") else {
            eprintln!("TERAX_SSH_TEST_HOST not set, skipping");
            return;
        };
        let tmp = tempfile::Builder::new()
            .prefix("tx")
            .tempdir_in("/tmp")
            .unwrap();
        let dir: PathBuf = tmp.path().join("cm");
        control::ensure_socket_dir(&dir).unwrap();

        assert!(!control::check(&dir, &host).unwrap());
        start_master(&dir, &host);
        let _guard = Master {
            dir: &dir,
            host: &host,
        };
        assert!(control::check(&dir, &host).unwrap());
        assert_eq!(control::master_sockets(&dir).len(), 1);

        let args = ["a\\", "'", "\\\\", "$(x)", "l1\nl2"];
        let script = format!(
            "for a in {}; do printf '[%s]' \"$a\"; done",
            args.iter()
                .map(|a| posix_quote(a))
                .collect::<Vec<_>>()
                .join(" ")
        );
        let out = exec(&dir, &host, &script, Duration::from_secs(20), 1 << 16).unwrap();
        control::ensure_success(&out).unwrap();
        let want: String = args.iter().map(|a| format!("[{a}]")).collect();
        assert_eq!(String::from_utf8_lossy(&out.stdout), want);

        let home = ops::home(&dir, &host).unwrap();
        assert!(home.starts_with('/'));

        assert!(control::exit(&dir, &host).unwrap());
        std::thread::sleep(Duration::from_millis(300));
        assert!(!control::check(&dir, &host).unwrap());
    }
}
