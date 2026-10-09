use std::path::PathBuf;
use std::sync::Mutex;

#[derive(Default)]
pub struct RemoteState {
    socket_dir: Mutex<Option<PathBuf>>,
    #[cfg(unix)]
    sftp: super::fs::sftp::SftpPool,
}

impl RemoteState {
    #[cfg(unix)]
    fn dir_path(&self, identifier: &str) -> Result<PathBuf, String> {
        let mut guard = self.socket_dir.lock().map_err(|e| e.to_string())?;
        if let Some(dir) = guard.as_ref() {
            return Ok(dir.clone());
        }
        let home =
            dirs::home_dir().ok_or_else(|| "could not resolve home directory".to_string())?;
        let dir = super::control::socket_dir(&home, identifier)?;
        *guard = Some(dir.clone());
        Ok(dir)
    }

    #[cfg(unix)]
    pub fn socket_dir(&self, identifier: &str) -> Result<PathBuf, String> {
        let dir = self.dir_path(identifier)?;
        super::control::ensure_socket_dir(&dir)?;
        Ok(dir)
    }

    #[cfg(unix)]
    pub fn sftp(&self, identifier: &str, host: &str) -> Result<super::fs::sftp::SftpHost, String> {
        super::target::validate_alias(host)?;
        let dir = self.socket_dir(identifier)?;
        self.sftp
            .host(host, || super::fs::sftp::ssh_launch(&dir, host))
    }

    #[cfg(unix)]
    pub fn sftp_disconnect(&self, host: &str) {
        self.sftp.disconnect(host);
    }

    pub fn shutdown(&self, identifier: &str) {
        #[cfg(unix)]
        self.sftp.shutdown();
        #[cfg(unix)]
        if let Ok(dir) = self.dir_path(identifier) {
            super::control::exit_all(&dir);
        }
        #[cfg(not(unix))]
        let _ = (identifier, &self.socket_dir);
    }
}
