use super::{
    join, mtime_millis, sibling_name, split_parent, validate_path, EntryKind, FsError, ReadOutcome,
    RemoteEntry, RemoteFs, RemoteMetadata, SetAttrs,
};
use crate::modules::remote::control::{build_args, control_path, ssh_binary, Mode};
use crate::modules::remote::exec::kill_group;
use futures_util::stream::{self, FuturesOrdered, StreamExt, TryStreamExt};
use russh_sftp::client::error::Error as SftpError;
use russh_sftp::client::rawsession::Limits;
use russh_sftp::client::{Config, RawSftpSession};
use russh_sftp::extensions;
use russh_sftp::protocol::{File, FileAttributes, OpenFlags, Packet, StatusCode};
use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, ChildStderr, Command};
use tokio::task::JoinHandle;

const POSIX_RENAME: &str = "posix-rename@openssh.com";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(30);
const REQUEST_TIMEOUT_SECS: u64 = 30;
const DEFAULT_CHUNK: u64 = 32 * 1024;
const MAX_CHUNK: u64 = 128 * 1024;
const WINDOW: usize = 16;
const MAX_STDERR: usize = 8 * 1024;
const EXIT_GRACE: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Launch {
    pub program: PathBuf,
    pub args: Vec<String>,
}

pub fn sftp_args(socket_dir: &Path, host: &str) -> Result<Vec<String>, String> {
    build_args(
        Mode::Batch,
        &control_path(socket_dir),
        &["-s"],
        host,
        &["sftp"],
    )
}

pub fn ssh_launch(socket_dir: &Path, host: &str) -> Result<Launch, String> {
    Ok(Launch {
        args: sftp_args(socket_dir, host)?,
        program: ssh_binary()?,
    })
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

fn map_err(e: SftpError, path: &str) -> FsError {
    match e {
        SftpError::Status(s) => match s.status_code {
            StatusCode::NoSuchFile => FsError::NotFound(path.to_string()),
            StatusCode::PermissionDenied => FsError::PermissionDenied(path.to_string()),
            StatusCode::OpUnsupported => FsError::Unsupported(path.to_string()),
            StatusCode::NoConnection | StatusCode::ConnectionLost => {
                FsError::Disconnected(s.status_code.to_string())
            }
            code => {
                let msg = s.error_message.trim();
                if msg.is_empty() {
                    FsError::Failure(format!("{path}: {code}"))
                } else {
                    FsError::Failure(format!("{path}: {msg}"))
                }
            }
        },
        SftpError::Timeout => FsError::Timeout,
        SftpError::Limited(m) => FsError::Failure(format!("{path}: {m}")),
        SftpError::IO(m) | SftpError::UnexpectedBehavior(m) => FsError::Disconnected(m),
        SftpError::UnexpectedPacket => FsError::Disconnected("unexpected reply".into()),
    }
}

fn is_status(e: &SftpError, code: StatusCode) -> bool {
    matches!(e, SftpError::Status(s) if s.status_code == code)
}

fn metadata(attrs: &FileAttributes) -> RemoteMetadata {
    RemoteMetadata {
        kind: EntryKind::from_mode(attrs.permissions),
        size: attrs.size.unwrap_or(0),
        mtime: mtime_millis(attrs.mtime),
        mode: attrs.permissions,
    }
}

fn entry(file: File) -> Option<RemoteEntry> {
    let name = file.filename;
    // Lossily decoded names cannot be addressed again, so they are not listed.
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\u{FFFD}')
    {
        return None;
    }
    Some(RemoteEntry {
        meta: metadata(&file.attrs),
        name,
    })
}

fn encode_strings(items: &[&str]) -> Vec<u8> {
    let mut out = Vec::new();
    for s in items {
        out.extend_from_slice(&(s.len() as u32).to_be_bytes());
        out.extend_from_slice(s.as_bytes());
    }
    out
}

fn nonce() -> Result<String, FsError> {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).map_err(|e| FsError::Failure(format!("random name: {e}")))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn chunk_len(limit: Option<u64>) -> u64 {
    limit.map_or(DEFAULT_CHUNK, |l| l.min(MAX_CHUNK)).max(1)
}

async fn drain_stderr(mut pipe: ChildStderr, buf: Arc<Mutex<Vec<u8>>>) {
    let mut chunk = [0u8; 1024];
    loop {
        match pipe.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                let mut b = lock(&buf);
                let room = MAX_STDERR.saturating_sub(b.len());
                b.extend_from_slice(&chunk[..n.min(room)]);
            }
        }
    }
}

struct Caps {
    posix_rename: bool,
    fsync: bool,
    limits: Option<Limits>,
}

async fn handshake(raw: &RawSftpSession) -> Result<Caps, SftpError> {
    let version = raw.init().await?;
    let has = |name: &str| version.extensions.get(name).is_some_and(|v| v == "1");
    let limits = if has(extensions::LIMITS) {
        Some(Limits::from(raw.limits().await?))
    } else {
        None
    };
    Ok(Caps {
        posix_rename: has(POSIX_RENAME),
        fsync: has(extensions::FSYNC),
        limits,
    })
}

/// One SFTP session over the stdio of a child process (`ssh -s host sftp`).
/// The child leads its own process group, so killing it also takes down any
/// ProxyCommand/ProxyJump helper ssh started.
pub struct SftpConn {
    raw: RawSftpSession,
    child: Mutex<Child>,
    pgid: Option<u32>,
    stderr: Arc<Mutex<Vec<u8>>>,
    posix_rename: bool,
    fsync: bool,
    read_chunk: u64,
    write_chunk: usize,
}

impl SftpConn {
    pub async fn connect(launch: &Launch) -> Result<Self, FsError> {
        Self::connect_with(launch, CONNECT_TIMEOUT, REQUEST_TIMEOUT_SECS).await
    }

    pub async fn connect_with(
        launch: &Launch,
        connect_timeout: Duration,
        request_timeout_secs: u64,
    ) -> Result<Self, FsError> {
        let mut child = Command::new(&launch.program)
            .args(&launch.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                FsError::Disconnected(format!("spawn {}: {e}", launch.program.display()))
            })?;
        let pgid = child.id();
        let stderr = Arc::new(Mutex::new(Vec::new()));
        let drain = child
            .stderr
            .take()
            .map(|pipe| tokio::spawn(drain_stderr(pipe, stderr.clone())));
        let (Some(stdin), Some(stdout)) = (child.stdin.take(), child.stdout.take()) else {
            let err = FsError::Disconnected("sftp child has no stdio".into());
            return Err(connect_failed(child, pgid, drain, &stderr, err).await);
        };
        let cfg = Config {
            request_timeout_secs: request_timeout_secs.max(1),
            ..Config::default()
        };
        let mut raw = RawSftpSession::new_with_config(tokio::io::join(stdout, stdin), cfg);
        let caps = match tokio::time::timeout(connect_timeout, handshake(&raw)).await {
            Ok(Ok(caps)) => caps,
            Ok(Err(e)) => {
                drop(raw);
                let err = map_err(e, "handshake");
                return Err(connect_failed(child, pgid, drain, &stderr, err).await);
            }
            Err(_) => {
                drop(raw);
                return Err(connect_failed(child, pgid, drain, &stderr, FsError::Timeout).await);
            }
        };
        if let Some(limits) = caps.limits {
            raw.set_limits(limits);
        }
        let limits = caps.limits.unwrap_or_default();
        Ok(Self {
            raw,
            child: Mutex::new(child),
            pgid,
            stderr,
            posix_rename: caps.posix_rename,
            fsync: caps.fsync,
            read_chunk: chunk_len(limits.read_len),
            write_chunk: chunk_len(limits.write_len) as usize,
        })
    }

    pub fn is_alive(&self) -> bool {
        matches!(lock(&self.child).try_wait(), Ok(None))
    }

    pub fn kill(&self) {
        let mut child = lock(&self.child);
        if let Some(pgid) = self.pgid {
            kill_group(pgid);
        }
        let _ = child.start_kill();
    }

    fn stderr_text(&self) -> String {
        String::from_utf8_lossy(&lock(&self.stderr))
            .trim()
            .to_string()
    }

    async fn lstat_opt(&self, path: &str) -> Result<Option<RemoteMetadata>, FsError> {
        match self.raw.lstat(path).await {
            Ok(a) => Ok(Some(metadata(&a.attrs))),
            Err(e) if is_status(&e, StatusCode::NoSuchFile) => Ok(None),
            Err(e) => Err(map_err(e, path)),
        }
    }

    async fn read_at(
        &self,
        handle: &str,
        path: &str,
        offset: u64,
        len: u32,
    ) -> Result<(u64, u32, Option<Vec<u8>>), FsError> {
        match self.raw.read(handle, offset, len).await {
            Ok(d) => Ok((offset, len, Some(d.data))),
            Err(e) if is_status(&e, StatusCode::Eof) => Ok((offset, len, None)),
            Err(e) => Err(map_err(e, path)),
        }
    }

    async fn read_handle(
        &self,
        handle: &str,
        path: &str,
        limit: u64,
    ) -> Result<ReadOutcome, FsError> {
        let meta = metadata(
            &self
                .raw
                .fstat(handle)
                .await
                .map_err(|e| map_err(e, path))?
                .attrs,
        );
        if meta.kind == EntryKind::Dir {
            return Err(FsError::Failure(format!("{path}: is a directory")));
        }
        if meta.size > limit {
            return Ok(ReadOutcome::TooLarge {
                size: meta.size,
                limit,
            });
        }
        let chunk = self.read_chunk;
        let mut buf: Vec<u8> = Vec::with_capacity(meta.size as usize);
        let mut complete = meta.size > 0;
        {
            let mut pending = FuturesOrdered::new();
            let mut next = 0u64;
            loop {
                while pending.len() < WINDOW && next < meta.size {
                    let len = (meta.size - next).min(chunk) as u32;
                    pending.push_back(self.read_at(handle, path, next, len));
                    next += u64::from(len);
                }
                let Some(res) = pending.next().await else {
                    break;
                };
                let (offset, len, data) = res?;
                match data {
                    Some(d) if offset == buf.len() as u64 => {
                        buf.extend_from_slice(&d);
                        if d.len() < len as usize {
                            complete = false;
                            break;
                        }
                    }
                    _ => {
                        complete = false;
                        break;
                    }
                }
            }
        }
        // Short reads, files that grew and size-less files (procfs) end here.
        while !complete {
            let (_, _, data) = self
                .read_at(handle, path, buf.len() as u64, chunk as u32)
                .await?;
            match data {
                Some(d) if !d.is_empty() => buf.extend_from_slice(&d),
                _ => complete = true,
            }
            if buf.len() as u64 > limit {
                return Ok(ReadOutcome::TooLarge {
                    size: buf.len() as u64,
                    limit,
                });
            }
        }
        Ok(ReadOutcome::Data {
            size: buf.len() as u64,
            mtime: meta.mtime,
            bytes: buf,
        })
    }

    async fn fill(
        &self,
        handle: &str,
        path: &str,
        data: &[u8],
        mode: Option<u32>,
    ) -> Result<(), FsError> {
        let chunk = self.write_chunk;
        stream::iter(data.chunks(chunk).enumerate().map(Ok::<_, FsError>))
            .try_for_each_concurrent(WINDOW, |(i, part)| async move {
                self.raw
                    .write(handle, (i * chunk) as u64, part.to_vec())
                    .await
                    .map(drop)
                    .map_err(|e| map_err(e, path))
            })
            .await?;
        if let Some(mode) = mode {
            let attrs = FileAttributes {
                permissions: Some(mode & 0o7777),
                ..FileAttributes::empty()
            };
            self.raw
                .fsetstat(handle, attrs)
                .await
                .map_err(|e| map_err(e, path))?;
        }
        if self.fsync {
            self.raw.fsync(handle).await.map_err(|e| map_err(e, path))?;
        }
        Ok(())
    }

    async fn write_temp(
        &self,
        tmp: &str,
        path: &str,
        data: &[u8],
        mode: Option<u32>,
    ) -> Result<(), FsError> {
        let create = FileAttributes {
            permissions: mode.map(|m| m & 0o777),
            ..FileAttributes::empty()
        };
        let flags = OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::EXCLUDE;
        let handle = self
            .raw
            .open(tmp, flags, create)
            .await
            .map_err(|e| map_err(e, path))?
            .handle;
        let filled = self.fill(&handle, path, data, mode).await;
        let closed = self
            .raw
            .close(handle)
            .await
            .map(drop)
            .map_err(|e| map_err(e, path));
        if let Err(e) = filled.and(closed) {
            let _ = self.raw.remove(tmp).await;
            return Err(e);
        }
        Ok(())
    }

    async fn posix_rename_to(&self, from: &str, to: &str) -> Result<(), FsError> {
        match self
            .raw
            .extended(POSIX_RENAME, encode_strings(&[from, to]))
            .await
        {
            Ok(Packet::Status(s)) if s.status_code == StatusCode::Ok => Ok(()),
            Ok(Packet::Status(s)) => Err(map_err(SftpError::Status(s), to)),
            Ok(_) => Err(FsError::Disconnected(
                "unexpected posix-rename reply".into(),
            )),
            Err(e) => Err(map_err(e, to)),
        }
    }

    /// Without posix-rename the old file moves to a backup first, so a failed
    /// swap can be undone; the window between the two renames is not atomic.
    async fn replace(&self, tmp: &str, path: &str, backup: &str) -> Result<(), FsError> {
        if self.posix_rename {
            return self.posix_rename_to(tmp, path).await;
        }
        let rename = |from: &str, to: &str| {
            let (from, to) = (from.to_string(), to.to_string());
            async move {
                self.raw
                    .rename(from.as_str(), to.as_str())
                    .await
                    .map(drop)
                    .map_err(|e| map_err(e, &to))
            }
        };
        if self.lstat_opt(path).await?.is_none() {
            return rename(tmp, path).await;
        }
        rename(path, backup).await?;
        if let Err(e) = rename(tmp, path).await {
            let _ = rename(backup, path).await;
            return Err(e);
        }
        let _ = self.raw.remove(backup).await;
        Ok(())
    }

    #[cfg(test)]
    fn without_posix_rename(mut self) -> Self {
        self.posix_rename = false;
        self
    }
}

impl Drop for SftpConn {
    fn drop(&mut self) {
        self.kill();
    }
}

impl RemoteFs for SftpConn {
    async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, FsError> {
        validate_path(path)?;
        let handle = self
            .raw
            .opendir(path)
            .await
            .map_err(|e| map_err(e, path))?
            .handle;
        let mut entries = Vec::new();
        let listed = loop {
            match self.raw.readdir(handle.as_str()).await {
                Ok(name) => entries.extend(name.files.into_iter().filter_map(entry)),
                Err(e) if is_status(&e, StatusCode::Eof) => break Ok(()),
                Err(e) => break Err(map_err(e, path)),
            }
        };
        let _ = self.raw.close(handle).await;
        listed.map(|_| entries)
    }

    async fn stat(&self, path: &str) -> Result<RemoteMetadata, FsError> {
        validate_path(path)?;
        let (link, target) =
            futures_util::future::join(self.raw.lstat(path), self.raw.stat(path)).await;
        let mut meta = metadata(&target.map_err(|e| map_err(e, path))?.attrs);
        if link.is_ok_and(|l| EntryKind::from_mode(l.attrs.permissions) == EntryKind::Symlink) {
            meta.kind = EntryKind::Symlink;
        }
        Ok(meta)
    }

    async fn read(&self, path: &str, limit: u64) -> Result<ReadOutcome, FsError> {
        validate_path(path)?;
        // Opening a FIFO or device blocks the single-threaded sftp-server.
        let attrs = self.raw.stat(path).await.map_err(|e| map_err(e, path))?;
        match metadata(&attrs.attrs).kind {
            EntryKind::File => {}
            EntryKind::Dir => return Err(FsError::Failure(format!("{path}: is a directory"))),
            _ => return Err(FsError::Failure(format!("{path}: not a regular file"))),
        }
        let handle = self
            .raw
            .open(path, OpenFlags::READ, FileAttributes::empty())
            .await
            .map_err(|e| map_err(e, path))?
            .handle;
        let result = self.read_handle(&handle, path, limit).await;
        let _ = self.raw.close(handle).await;
        result
    }

    async fn write_atomic(&self, path: &str, data: &[u8]) -> Result<RemoteMetadata, FsError> {
        validate_path(path)?;
        let (parent, name) = split_parent(path)
            .ok_or_else(|| FsError::InvalidPath(format!("{path} has no file name")))?;
        let existing = match self.raw.stat(path).await {
            Ok(a) => Some(metadata(&a.attrs)),
            Err(e) if is_status(&e, StatusCode::NoSuchFile) => None,
            Err(e) => return Err(map_err(e, path)),
        };
        if existing.is_some_and(|m| m.kind == EntryKind::Dir) {
            return Err(FsError::Failure(format!("{path}: is a directory")));
        }
        let nonce = nonce()?;
        let tmp = join(parent, &sibling_name(name, &nonce, "tmp"));
        let backup = join(parent, &sibling_name(name, &nonce, "bak"));
        self.write_temp(&tmp, path, data, existing.and_then(|m| m.mode))
            .await?;
        if let Err(e) = self.replace(&tmp, path, &backup).await {
            let _ = self.raw.remove(tmp.as_str()).await;
            return Err(e);
        }
        let attrs = self.raw.stat(path).await.map_err(|e| map_err(e, path))?;
        Ok(metadata(&attrs.attrs))
    }

    async fn mkdir(&self, path: &str) -> Result<(), FsError> {
        validate_path(path)?;
        self.raw
            .mkdir(path, FileAttributes::empty())
            .await
            .map(drop)
            .map_err(|e| map_err(e, path))
    }

    async fn remove(&self, path: &str) -> Result<(), FsError> {
        validate_path(path)?;
        let meta = self
            .lstat_opt(path)
            .await?
            .ok_or_else(|| FsError::NotFound(path.to_string()))?;
        let res = if meta.kind == EntryKind::Dir {
            self.raw.rmdir(path).await
        } else {
            self.raw.remove(path).await
        };
        res.map(drop).map_err(|e| map_err(e, path))
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FsError> {
        validate_path(from)?;
        validate_path(to)?;
        self.raw
            .rename(from, to)
            .await
            .map(drop)
            .map_err(|e| map_err(e, &format!("{from} -> {to}")))
    }

    async fn realpath(&self, path: &str) -> Result<String, FsError> {
        if path.contains('\0') {
            return Err(FsError::InvalidPath("contains a NUL byte".into()));
        }
        let name = self
            .raw
            .realpath(path)
            .await
            .map_err(|e| map_err(e, path))?;
        let resolved = name
            .files
            .into_iter()
            .next()
            .map(|f| f.filename)
            .ok_or_else(|| FsError::Failure(format!("{path}: empty realpath reply")))?;
        if !resolved.starts_with('/') || resolved.contains('\u{FFFD}') {
            return Err(FsError::Failure(format!(
                "{path}: unusable realpath reply {resolved}"
            )));
        }
        Ok(resolved)
    }

    async fn setstat(&self, path: &str, attrs: SetAttrs) -> Result<(), FsError> {
        validate_path(path)?;
        if attrs == SetAttrs::default() {
            return Ok(());
        }
        let wire = FileAttributes {
            permissions: attrs.mode.map(|m| m & 0o7777),
            atime: attrs.times.map(|t| t.0),
            mtime: attrs.times.map(|t| t.1),
            ..FileAttributes::empty()
        };
        self.raw
            .setstat(path, wire)
            .await
            .map(drop)
            .map_err(|e| map_err(e, path))
    }

    async fn supports_posix_rename(&self) -> Result<bool, FsError> {
        Ok(self.posix_rename)
    }
}

struct HostSlot {
    launch: Launch,
    current: Mutex<Option<Arc<SftpConn>>>,
    connecting: tokio::sync::Mutex<()>,
    closed: Arc<AtomicBool>,
}

impl HostSlot {
    fn live(&self) -> Option<Arc<SftpConn>> {
        let mut cur = lock(&self.current);
        if cur.as_ref().is_some_and(|c| c.is_alive()) {
            return cur.clone();
        }
        if let Some(dead) = cur.take() {
            dead.kill();
        }
        None
    }

    fn reset(&self) {
        if let Some(conn) = lock(&self.current).take() {
            conn.kill();
        }
    }

    fn invalidate(&self, conn: &Arc<SftpConn>) {
        let mut cur = lock(&self.current);
        if cur.as_ref().is_some_and(|c| Arc::ptr_eq(c, conn)) {
            *cur = None;
        }
        conn.kill();
    }
}

/// Lazily connected, self-healing SFTP session for one host.
#[derive(Clone)]
pub struct SftpHost {
    slot: Arc<HostSlot>,
}

impl SftpHost {
    async fn conn(&self) -> Result<Arc<SftpConn>, FsError> {
        if let Some(conn) = self.slot.live() {
            return Ok(conn);
        }
        let _connecting = self.slot.connecting.lock().await;
        if let Some(conn) = self.slot.live() {
            return Ok(conn);
        }
        if self.slot.closed.load(Ordering::Acquire) {
            return Err(FsError::Disconnected(
                "remote sessions are shut down".into(),
            ));
        }
        let conn = Arc::new(SftpConn::connect(&self.slot.launch).await?);
        if self.slot.closed.load(Ordering::Acquire) {
            conn.kill();
            return Err(FsError::Disconnected(
                "remote sessions are shut down".into(),
            ));
        }
        *lock(&self.slot.current) = Some(conn.clone());
        Ok(conn)
    }

    /// Read-only operations retry once on a fresh session after EOF.
    async fn run<T, F, Fut>(&self, retry: bool, op: F) -> Result<T, FsError>
    where
        F: Fn(Arc<SftpConn>) -> Fut + Send + Sync,
        Fut: Future<Output = Result<T, FsError>> + Send,
        T: Send,
    {
        let mut retried = false;
        loop {
            let conn = self.conn().await?;
            match op(conn.clone()).await {
                Err(e) if e.is_connection() => {
                    self.slot.invalidate(&conn);
                    if retry && !retried && e != FsError::Timeout {
                        retried = true;
                        continue;
                    }
                    let detail = conn.stderr_text();
                    return Err(match e {
                        FsError::Disconnected(_) if !detail.is_empty() => {
                            FsError::Disconnected(detail)
                        }
                        e => e,
                    });
                }
                res => return res,
            }
        }
    }

    #[cfg(test)]
    fn current(&self) -> Option<Arc<SftpConn>> {
        lock(&self.slot.current).clone()
    }
}

impl RemoteFs for SftpHost {
    async fn read_dir(&self, path: &str) -> Result<Vec<RemoteEntry>, FsError> {
        self.run(true, move |c| async move { c.read_dir(path).await })
            .await
    }

    async fn stat(&self, path: &str) -> Result<RemoteMetadata, FsError> {
        self.run(true, move |c| async move { c.stat(path).await })
            .await
    }

    async fn read(&self, path: &str, limit: u64) -> Result<ReadOutcome, FsError> {
        self.run(true, move |c| async move { c.read(path, limit).await })
            .await
    }

    async fn write_atomic(&self, path: &str, data: &[u8]) -> Result<RemoteMetadata, FsError> {
        self.run(
            false,
            move |c| async move { c.write_atomic(path, data).await },
        )
        .await
    }

    async fn mkdir(&self, path: &str) -> Result<(), FsError> {
        self.run(false, move |c| async move { c.mkdir(path).await })
            .await
    }

    async fn remove(&self, path: &str) -> Result<(), FsError> {
        self.run(false, move |c| async move { c.remove(path).await })
            .await
    }

    async fn rename(&self, from: &str, to: &str) -> Result<(), FsError> {
        self.run(false, move |c| async move { c.rename(from, to).await })
            .await
    }

    async fn realpath(&self, path: &str) -> Result<String, FsError> {
        self.run(true, move |c| async move { c.realpath(path).await })
            .await
    }

    async fn setstat(&self, path: &str, attrs: SetAttrs) -> Result<(), FsError> {
        self.run(false, move |c| async move { c.setstat(path, attrs).await })
            .await
    }

    async fn supports_posix_rename(&self) -> Result<bool, FsError> {
        self.run(true, |c| async move { Ok(c.posix_rename) }).await
    }
}

/// One SFTP session per host alias, shared by every caller.
#[derive(Default)]
pub struct SftpPool {
    hosts: Mutex<HashMap<String, Arc<HostSlot>>>,
    closed: Arc<AtomicBool>,
}

impl SftpPool {
    pub fn host<F>(&self, name: &str, launch: F) -> Result<SftpHost, String>
    where
        F: FnOnce() -> Result<Launch, String>,
    {
        if self.closed.load(Ordering::Acquire) {
            return Err("remote sessions are shut down".into());
        }
        let mut hosts = lock(&self.hosts);
        if let Some(slot) = hosts.get(name) {
            return Ok(SftpHost { slot: slot.clone() });
        }
        let slot = Arc::new(HostSlot {
            launch: launch()?,
            current: Mutex::new(None),
            connecting: tokio::sync::Mutex::new(()),
            closed: self.closed.clone(),
        });
        hosts.insert(name.to_string(), slot.clone());
        Ok(SftpHost { slot })
    }

    pub fn disconnect(&self, name: &str) {
        if let Some(slot) = lock(&self.hosts).get(name) {
            slot.reset();
        }
    }

    pub fn shutdown(&self) {
        self.closed.store(true, Ordering::Release);
        for slot in lock(&self.hosts).values() {
            slot.reset();
        }
    }
}

async fn connect_failed(
    mut child: Child,
    pgid: Option<u32>,
    drain: Option<JoinHandle<()>>,
    stderr: &Mutex<Vec<u8>>,
    err: FsError,
) -> FsError {
    let exited = tokio::time::timeout(EXIT_GRACE, child.wait()).await.is_ok();
    // Helpers (ProxyCommand, ProxyJump) share the group and can outlive ssh.
    if let Some(pgid) = pgid {
        kill_group(pgid);
    }
    if !exited {
        let _ = child.start_kill();
        let _ = tokio::time::timeout(EXIT_GRACE, child.wait()).await;
    }
    if let Some(drain) = drain {
        let _ = tokio::time::timeout(EXIT_GRACE, drain).await;
    }
    let detail = String::from_utf8_lossy(&lock(stderr)).trim().to_string();
    if detail.is_empty() {
        err
    } else {
        FsError::Disconnected(detail)
    }
}

#[cfg(test)]
pub(crate) fn test_launch() -> Option<Launch> {
    const SERVERS: [&str; 4] = [
        "/usr/libexec/sftp-server",
        "/usr/lib/openssh/sftp-server",
        "/usr/libexec/openssh/sftp-server",
        "/usr/lib/ssh/sftp-server",
    ];
    let program = SERVERS.iter().map(Path::new).find(|p| p.is_file())?;
    Some(Launch {
        program: program.to_path_buf(),
        args: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::remote::control::base_options;

    #[test]
    fn sftp_child_uses_shared_batch_builder() {
        let dir = Path::new("/Users/u/.terax/cm");
        let args = sftp_args(dir, "oracle").unwrap();
        assert_eq!(
            args,
            build_args(
                Mode::Batch,
                &control_path(dir),
                &["-s"],
                "oracle",
                &["sftp"]
            )
            .unwrap()
        );
        let base = base_options(Mode::Batch, &control_path(dir));
        assert_eq!(&args[..base.len()], base.as_slice());
        assert_eq!(&args[base.len()..], &["-s", "--", "oracle", "sftp"]);
        assert!(args.iter().any(|a| a == "BatchMode=yes"));
        assert!(args.iter().any(|a| a == "ControlMaster=no"));
        for bad in ["-oProxyCommand=x", "a b", ""] {
            assert!(sftp_args(dir, bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn encodes_ssh_strings() {
        assert_eq!(
            encode_strings(&["ab", ""]),
            vec![0, 0, 0, 2, b'a', b'b', 0, 0, 0, 0]
        );
    }

    #[test]
    fn maps_status_codes() {
        let status = |code: StatusCode, msg: &str| {
            SftpError::Status(russh_sftp::protocol::Status {
                id: 1,
                status_code: code,
                error_message: msg.into(),
                language_tag: String::new(),
            })
        };
        assert_eq!(
            map_err(status(StatusCode::NoSuchFile, "x"), "/a"),
            FsError::NotFound("/a".into())
        );
        assert_eq!(
            map_err(status(StatusCode::PermissionDenied, ""), "/a"),
            FsError::PermissionDenied("/a".into())
        );
        assert_eq!(
            map_err(status(StatusCode::Failure, "exists"), "/a"),
            FsError::Failure("/a: exists".into())
        );
        assert!(map_err(status(StatusCode::ConnectionLost, ""), "/a").is_connection());
        assert_eq!(map_err(SftpError::Timeout, "/a"), FsError::Timeout);
        assert!(
            map_err(SftpError::UnexpectedBehavior("session closed".into()), "/a").is_connection()
        );
    }

    #[test]
    fn chunk_sizes_respect_server_limits() {
        assert_eq!(chunk_len(None), DEFAULT_CHUNK);
        assert_eq!(chunk_len(Some(1 << 20)), MAX_CHUNK);
        assert_eq!(chunk_len(Some(1000)), 1000);
    }

    #[test]
    fn entries_skip_dots_and_lossy_names() {
        let file = |name: &str| File::new(name, FileAttributes::empty());
        assert!(entry(file(".")).is_none());
        assert!(entry(file("..")).is_none());
        assert!(entry(file("a\u{FFFD}b")).is_none());
        assert!(entry(file("a/b")).is_none());
        assert_eq!(entry(file(".hidden")).unwrap().name, ".hidden");
    }
}

#[cfg(test)]
mod server_tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::time::Instant;

    fn local_server() -> Option<Launch> {
        super::test_launch()
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    fn root() -> (tempfile::TempDir, String) {
        let tmp = tempfile::tempdir().unwrap();
        let canon = std::fs::canonicalize(tmp.path()).unwrap();
        let s = canon.to_str().unwrap().to_string();
        (tmp, s)
    }

    fn mode(path: &str) -> u32 {
        std::fs::metadata(path).unwrap().permissions().mode() & 0o7777
    }

    fn leftovers(dir: &str) -> Vec<String> {
        std::fs::read_dir(dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n.contains(".terax-"))
            .collect()
    }

    fn pattern(len: usize) -> Vec<u8> {
        let mut x: u32 = 0x9e37_79b9;
        (0..len)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 17;
                x ^= x << 5;
                x as u8
            })
            .collect()
    }

    async fn exited(conn: &SftpConn) -> bool {
        for _ in 0..200 {
            if !conn.is_alive() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }

    macro_rules! server_or_skip {
        () => {
            match local_server() {
                Some(l) => l,
                None => {
                    eprintln!("sftp-server not found, skipping");
                    return;
                }
            }
        };
    }

    #[test]
    fn file_operations_against_local_server() {
        let launch = server_or_skip!();
        let (_tmp, root) = root();
        rt().block_on(async {
            let fs = SftpConn::connect(&launch).await.unwrap();
            assert!(fs.supports_posix_rename().await.unwrap());
            assert_eq!(fs.realpath(&root).await.unwrap(), root);
            assert_eq!(fs.realpath(&format!("{root}/./.")).await.unwrap(), root);

            let a = format!("{root}/a.txt");
            let meta = fs.write_atomic(&a, b"hello").await.unwrap();
            assert_eq!((meta.kind, meta.size), (EntryKind::File, 5));
            assert_eq!(std::fs::read(&a).unwrap(), b"hello");
            std::fs::set_permissions(&a, std::fs::Permissions::from_mode(0o640)).unwrap();
            fs.write_atomic(&a, b"world!").await.unwrap();
            assert_eq!(std::fs::read(&a).unwrap(), b"world!");
            assert_eq!(mode(&a), 0o640);
            assert!(leftovers(&root).is_empty());

            match fs.read(&a, 1024).await.unwrap() {
                ReadOutcome::Data { bytes, size, mtime } => {
                    assert_eq!(bytes, b"world!");
                    assert_eq!(size, 6);
                    assert!(mtime > 0 && mtime % 1000 == 0);
                }
                other => panic!("{other:?}"),
            }
            assert_eq!(
                fs.read(&a, 3).await.unwrap(),
                ReadOutcome::TooLarge { size: 6, limit: 3 }
            );
            let empty = format!("{root}/empty");
            fs.write_atomic(&empty, b"").await.unwrap();
            assert_eq!(
                fs.read(&empty, 0).await.unwrap(),
                ReadOutcome::Data {
                    bytes: Vec::new(),
                    size: 0,
                    mtime: fs.stat(&empty).await.unwrap().mtime
                }
            );

            let sub = format!("{root}/sub");
            fs.mkdir(&sub).await.unwrap();
            std::os::unix::fs::symlink(&a, format!("{root}/link")).unwrap();
            let mut listed = fs.read_dir(&root).await.unwrap();
            listed.sort_by(|x, y| x.name.cmp(&y.name));
            let names: Vec<_> = listed
                .iter()
                .map(|e| (e.name.as_str(), e.meta.kind))
                .collect();
            assert_eq!(
                names,
                vec![
                    ("a.txt", EntryKind::File),
                    ("empty", EntryKind::File),
                    ("link", EntryKind::Symlink),
                    ("sub", EntryKind::Dir),
                ]
            );
            assert_eq!(listed[0].meta.size, 6);

            let link = fs.stat(&format!("{root}/link")).await.unwrap();
            assert_eq!((link.kind, link.size), (EntryKind::Symlink, 6));
            assert_eq!(fs.stat(&sub).await.unwrap().kind, EntryKind::Dir);

            let b = format!("{root}/b.txt");
            fs.rename(&a, &b).await.unwrap();
            assert!(!Path::new(&a).exists());
            assert_eq!(std::fs::read(&b).unwrap(), b"world!");
            std::fs::write(format!("{root}/c.txt"), "keep").unwrap();
            assert!(fs.rename(&b, &format!("{root}/c.txt")).await.is_err());
            assert_eq!(std::fs::read(&b).unwrap(), b"world!");
            assert_eq!(std::fs::read(format!("{root}/c.txt")).unwrap(), b"keep");

            fs.setstat(
                &b,
                SetAttrs {
                    mode: Some(0o600),
                    times: Some((1_000_000_000, 1_000_000_000)),
                },
            )
            .await
            .unwrap();
            assert_eq!(mode(&b), 0o600);
            assert_eq!(fs.stat(&b).await.unwrap().mtime, 1_000_000_000_000);

            std::fs::write(format!("{sub}/inner"), "x").unwrap();
            assert!(fs.remove(&sub).await.is_err());
            fs.remove(&format!("{sub}/inner")).await.unwrap();
            fs.remove(&sub).await.unwrap();
            fs.remove(&format!("{root}/link")).await.unwrap();
            assert!(Path::new(&b).exists());
            fs.remove(&b).await.unwrap();
            assert!(!Path::new(&sub).exists() && !Path::new(&b).exists());
        });
    }

    #[test]
    fn errors_against_local_server() {
        let launch = server_or_skip!();
        let (_tmp, root) = root();
        rt().block_on(async {
            let fs = SftpConn::connect(&launch).await.unwrap();
            let missing = format!("{root}/missing");
            assert_eq!(
                fs.stat(&missing).await,
                Err(FsError::NotFound(missing.clone()))
            );
            assert_eq!(
                fs.read(&missing, 10).await,
                Err(FsError::NotFound(missing.clone()))
            );
            assert!(matches!(
                fs.read_dir(&missing).await,
                Err(FsError::NotFound(_))
            ));
            assert!(matches!(
                fs.remove(&missing).await,
                Err(FsError::NotFound(_))
            ));
            assert!(matches!(
                fs.write_atomic(&format!("{missing}/f"), b"x").await,
                Err(FsError::NotFound(_))
            ));
            assert!(matches!(
                fs.write_atomic("relative/f", b"x").await,
                Err(FsError::InvalidPath(_))
            ));
            assert!(matches!(
                fs.write_atomic(&format!("{root}/"), b"x").await,
                Err(FsError::InvalidPath(_))
            ));
            assert!(matches!(
                fs.stat(&format!("{root}/a\0b")).await,
                Err(FsError::InvalidPath(_))
            ));
            assert!(fs.write_atomic(&root, b"x").await.is_err());
            assert!(fs.read(&root, 10).await.is_err());
            let file = format!("{root}/file");
            std::fs::write(&file, "x").unwrap();
            assert!(fs.read_dir(&file).await.is_err());
            assert!(fs.mkdir(&file).await.is_err());

            let fifo = format!("{root}/fifo");
            let c = std::ffi::CString::new(fifo.as_str()).unwrap();
            // SAFETY: c is a valid NUL-terminated path.
            assert_eq!(unsafe { libc::mkfifo(c.as_ptr(), 0o600) }, 0);
            let refused = tokio::time::timeout(Duration::from_secs(5), fs.read(&fifo, 10))
                .await
                .expect("read of a fifo must not block");
            assert!(matches!(refused, Err(FsError::Failure(_))));
            assert_eq!(fs.read(&file, 10).await.map(|_| ()), Ok(()));

            // SAFETY: geteuid has no preconditions.
            if unsafe { libc::geteuid() } != 0 {
                let locked = format!("{root}/locked");
                std::fs::create_dir(&locked).unwrap();
                std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o500)).unwrap();
                assert_eq!(
                    fs.write_atomic(&format!("{locked}/f"), b"x").await,
                    Err(FsError::PermissionDenied(format!("{locked}/f")))
                );
                std::fs::set_permissions(&locked, std::fs::Permissions::from_mode(0o700)).unwrap();
            }
            assert!(leftovers(&root).is_empty());
        });
    }

    #[test]
    fn large_files_round_trip() {
        let launch = server_or_skip!();
        let (_tmp, root) = root();
        rt().block_on(async {
            let fs = SftpConn::connect(&launch).await.unwrap();
            let data = pattern(5 * 1024 * 1024 + 123);
            let big = format!("{root}/big.bin");
            let start = Instant::now();
            let meta = fs.write_atomic(&big, &data).await.unwrap();
            assert_eq!(meta.size, data.len() as u64);
            assert!(std::fs::read(&big).unwrap() == data);
            match fs.read(&big, data.len() as u64).await.unwrap() {
                ReadOutcome::Data { bytes, .. } => assert!(bytes == data),
                other => panic!("{:?}", std::mem::discriminant(&other)),
            }
            assert!(start.elapsed() < Duration::from_secs(30));

            let local = pattern(300_001);
            std::fs::write(format!("{root}/local.bin"), &local).unwrap();
            match fs
                .read(&format!("{root}/local.bin"), u64::MAX)
                .await
                .unwrap()
            {
                ReadOutcome::Data { bytes, size, .. } => {
                    assert_eq!(size, 300_001);
                    assert!(bytes == local);
                }
                other => panic!("{:?}", std::mem::discriminant(&other)),
            }
        });
    }

    #[test]
    fn write_atomic_without_posix_rename() {
        let launch = server_or_skip!();
        let (_tmp, root) = root();
        rt().block_on(async {
            let fs = SftpConn::connect(&launch)
                .await
                .unwrap()
                .without_posix_rename();
            assert!(!fs.supports_posix_rename().await.unwrap());
            let f = format!("{root}/f.txt");
            fs.write_atomic(&f, b"one").await.unwrap();
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o604)).unwrap();
            fs.write_atomic(&f, b"two").await.unwrap();
            assert_eq!(std::fs::read(&f).unwrap(), b"two");
            assert_eq!(mode(&f), 0o604);
            assert!(leftovers(&root).is_empty());
        });
    }

    #[test]
    fn pool_connects_lazily_and_reconnects_after_eof() {
        let launch = server_or_skip!();
        let (_tmp, root) = root();
        rt().block_on(async {
            let pool = SftpPool::default();
            let host = pool.host("local", || Ok(launch.clone())).unwrap();
            assert!(host.current().is_none());
            let again = pool
                .host("local", || Err("launch must not be rebuilt".into()))
                .unwrap();
            assert!(Arc::ptr_eq(&host.slot, &again.slot));

            assert_eq!(host.realpath(&root).await.unwrap(), root);
            let first = host.current().unwrap();
            assert!(first.is_alive());
            host.stat(&root).await.unwrap();
            assert!(Arc::ptr_eq(&first, &host.current().unwrap()));

            first.kill();
            assert!(exited(&first).await);
            assert_eq!(host.stat(&root).await.unwrap().kind, EntryKind::Dir);
            let second = host.current().unwrap();
            assert!(!Arc::ptr_eq(&first, &second));

            let f = format!("{root}/x");
            host.write_atomic(&f, b"x").await.unwrap();
            pool.disconnect("local");
            assert!(host.current().is_none());
            assert!(exited(&second).await);
            assert!(matches!(
                host.read(&f, 10).await.unwrap(),
                ReadOutcome::Data { .. }
            ));

            let third = host.current().unwrap();
            pool.shutdown();
            assert!(exited(&third).await);
            assert!(host.stat(&root).await.unwrap_err().is_connection());
            assert!(pool.host("other", || Ok(launch.clone())).is_err());
        });
    }

    #[test]
    fn connect_failures_and_timeouts() {
        rt().block_on(async {
            let failing = Launch {
                program: PathBuf::from("/bin/sh"),
                args: vec![
                    "-c".into(),
                    "echo 'Permission denied (publickey)' >&2; exit 255".into(),
                ],
            };
            let err = SftpConn::connect(&failing).await.err().unwrap();
            assert_eq!(
                err,
                FsError::Disconnected("Permission denied (publickey)".into())
            );

            let silent = Launch {
                program: PathBuf::from("/bin/sleep"),
                args: vec!["30".into()],
            };
            let start = Instant::now();
            let err = SftpConn::connect_with(&silent, Duration::from_millis(300), 1)
                .await
                .err()
                .unwrap();
            assert_eq!(err, FsError::Timeout);
            assert!(start.elapsed() < Duration::from_secs(5));

            // Answers the version handshake, then never replies again.
            let stuck = Launch {
                program: PathBuf::from("/bin/sh"),
                args: vec![
                    "-c".into(),
                    "printf '\\000\\000\\000\\005\\002\\000\\000\\000\\003'; exec sleep 30".into(),
                ],
            };
            let fs = SftpConn::connect_with(&stuck, Duration::from_secs(5), 1)
                .await
                .unwrap();
            assert!(!fs.supports_posix_rename().await.unwrap());
            let start = Instant::now();
            assert_eq!(fs.stat("/").await, Err(FsError::Timeout));
            assert!(start.elapsed() < Duration::from_secs(5));
            fs.kill();
        });
    }

    /// Fake server whose shell backgrounds a `sleep` in the same process group
    /// (like ssh's ProxyCommand helper) and records its pid in `pidfile`.
    fn group_launch(pidfile: &Path, rest: &str) -> Launch {
        Launch {
            program: PathBuf::from("/bin/sh"),
            args: vec![
                "-c".into(),
                format!("sleep 30 & echo $! > '{}'; {rest}", pidfile.display()),
            ],
        }
    }

    fn helper_pid(pidfile: &Path) -> i32 {
        std::fs::read_to_string(pidfile)
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    }

    async fn gone(pid: i32) -> bool {
        for _ in 0..300 {
            // SAFETY: signal 0 only probes whether the pid exists.
            let rc = unsafe { libc::kill(pid, 0) };
            if rc != 0 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        false
    }

    #[test]
    fn kill_takes_down_the_whole_process_group() {
        let tmp = tempfile::tempdir().unwrap();
        let handshake = "printf '\\000\\000\\000\\005\\002\\000\\000\\000\\003'; exec sleep 30";
        rt().block_on(async {
            let pidfile = tmp.path().join("kill");
            let launch = group_launch(&pidfile, handshake);
            let fs = SftpConn::connect_with(&launch, Duration::from_secs(5), 1)
                .await
                .unwrap();
            let helper = helper_pid(&pidfile);
            fs.kill();
            assert!(exited(&fs).await);
            assert!(gone(helper).await, "helper survived kill()");

            let pidfile = tmp.path().join("drop");
            let launch = group_launch(&pidfile, handshake);
            let fs = SftpConn::connect_with(&launch, Duration::from_secs(5), 1)
                .await
                .unwrap();
            let helper = helper_pid(&pidfile);
            drop(fs);
            assert!(gone(helper).await, "helper survived drop");

            let pidfile = tmp.path().join("timeout");
            let launch = group_launch(&pidfile, "exec sleep 30");
            let err = SftpConn::connect_with(&launch, Duration::from_millis(300), 1)
                .await
                .err()
                .unwrap();
            assert_eq!(err, FsError::Timeout);
            let helper = helper_pid(&pidfile);
            assert!(gone(helper).await, "helper survived connect timeout");
        });
    }
}
