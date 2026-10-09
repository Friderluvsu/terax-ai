#[cfg(unix)]
pub mod sftp;

use serde::Serialize;
use std::fmt;
use std::future::Future;

const S_IFMT: u32 = 0o170_000;
const S_IFREG: u32 = 0o100_000;
const S_IFDIR: u32 = 0o040_000;
const S_IFLNK: u32 = 0o120_000;
const MAX_SIBLING_STEM: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
    Other,
}

impl EntryKind {
    pub fn from_mode(mode: Option<u32>) -> Self {
        match mode.map(|m| m & S_IFMT) {
            Some(S_IFREG) => Self::File,
            Some(S_IFDIR) => Self::Dir,
            Some(S_IFLNK) => Self::Symlink,
            _ => Self::Other,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteMetadata {
    pub kind: EntryKind,
    pub size: u64,
    /// Milliseconds since the epoch; SFTP v3 only carries whole seconds.
    pub mtime: u64,
    pub mode: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteEntry {
    pub name: String,
    #[serde(flatten)]
    pub meta: RemoteMetadata,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadOutcome {
    Data {
        bytes: Vec<u8>,
        size: u64,
        mtime: u64,
    },
    TooLarge {
        size: u64,
        limit: u64,
    },
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SetAttrs {
    pub mode: Option<u32>,
    /// (atime, mtime) in seconds; SFTP v3 sets both or neither.
    pub times: Option<(u32, u32)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FsError {
    InvalidPath(String),
    NotFound(String),
    PermissionDenied(String),
    Unsupported(String),
    Failure(String),
    Timeout,
    Disconnected(String),
}

impl FsError {
    pub fn is_connection(&self) -> bool {
        matches!(self, Self::Timeout | Self::Disconnected(_))
    }
}

impl fmt::Display for FsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidPath(m) => write!(f, "invalid remote path: {m}"),
            Self::NotFound(m) => write!(f, "no such file or directory: {m}"),
            Self::PermissionDenied(m) => write!(f, "permission denied: {m}"),
            Self::Unsupported(m) => write!(f, "not supported by the remote server: {m}"),
            Self::Failure(m) => f.write_str(m),
            Self::Timeout => f.write_str("remote file request timed out"),
            Self::Disconnected(m) => write!(f, "remote file session lost: {m}"),
        }
    }
}

impl std::error::Error for FsError {}

impl From<FsError> for String {
    fn from(e: FsError) -> Self {
        e.to_string()
    }
}

pub fn validate_path(path: &str) -> Result<(), FsError> {
    if path.contains('\0') {
        return Err(FsError::InvalidPath("contains a NUL byte".into()));
    }
    if !path.starts_with('/') {
        return Err(FsError::InvalidPath(format!("{path} is not absolute")));
    }
    Ok(())
}

/// Splits an absolute path into its parent and a final component that can be
/// created or replaced. Rejects the root, trailing slashes, "." and "..".
pub fn split_parent(path: &str) -> Option<(&str, &str)> {
    let idx = path.rfind('/')?;
    let name = &path[idx + 1..];
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }
    let parent = if idx == 0 { "/" } else { &path[..idx] };
    Some((parent, name))
}

pub fn join(parent: &str, name: &str) -> String {
    if parent.ends_with('/') {
        format!("{parent}{name}")
    } else {
        format!("{parent}/{name}")
    }
}

/// Hidden sibling name used for temporaries and backups next to `name`.
pub fn sibling_name(name: &str, nonce: &str, ext: &str) -> String {
    let mut end = name.len().min(MAX_SIBLING_STEM);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    format!(".{}.terax-{nonce}.{ext}", &name[..end])
}

pub fn mtime_millis(secs: Option<u32>) -> u64 {
    secs.map(|s| u64::from(s) * 1000).unwrap_or(0)
}

/// File access on a remote host. Paths are absolute POSIX paths on the
/// server, except `realpath`, which also resolves relative ones.
pub trait RemoteFs: Send + Sync {
    /// Directory entries without "." and "..", with lstat-style kinds.
    fn read_dir(
        &self,
        path: &str,
    ) -> impl Future<Output = Result<Vec<RemoteEntry>, FsError>> + Send;
    /// Follows symlinks for size and mtime; `kind` reports the link itself.
    fn stat(&self, path: &str) -> impl Future<Output = Result<RemoteMetadata, FsError>> + Send;
    fn read(
        &self,
        path: &str,
        limit: u64,
    ) -> impl Future<Output = Result<ReadOutcome, FsError>> + Send;
    /// Writes a sibling temporary and renames it over `path`, keeping the
    /// previous permissions. Atomic only when `supports_posix_rename` is true.
    fn write_atomic(
        &self,
        path: &str,
        data: &[u8],
    ) -> impl Future<Output = Result<RemoteMetadata, FsError>> + Send;
    fn mkdir(&self, path: &str) -> impl Future<Output = Result<(), FsError>> + Send;
    /// Removes a file, a symlink or an empty directory.
    fn remove(&self, path: &str) -> impl Future<Output = Result<(), FsError>> + Send;
    /// Plain SFTP rename; fails when `to` exists.
    fn rename(&self, from: &str, to: &str) -> impl Future<Output = Result<(), FsError>> + Send;
    fn realpath(&self, path: &str) -> impl Future<Output = Result<String, FsError>> + Send;
    fn setstat(
        &self,
        path: &str,
        attrs: SetAttrs,
    ) -> impl Future<Output = Result<(), FsError>> + Send;
    fn supports_posix_rename(&self) -> impl Future<Output = Result<bool, FsError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_from_mode() {
        assert_eq!(EntryKind::from_mode(Some(0o100_644)), EntryKind::File);
        assert_eq!(EntryKind::from_mode(Some(0o040_755)), EntryKind::Dir);
        assert_eq!(EntryKind::from_mode(Some(0o120_777)), EntryKind::Symlink);
        assert_eq!(EntryKind::from_mode(Some(0o060_660)), EntryKind::Other);
        assert_eq!(EntryKind::from_mode(None), EntryKind::Other);
    }

    #[test]
    fn validates_paths() {
        assert!(validate_path("/home/u/a b").is_ok());
        assert!(validate_path("relative").is_err());
        assert!(validate_path("").is_err());
        assert!(validate_path("/etc/passwd\0.txt").is_err());
    }

    #[test]
    fn splits_parent() {
        assert_eq!(split_parent("/a/b.txt"), Some(("/a", "b.txt")));
        assert_eq!(split_parent("/b"), Some(("/", "b")));
        for bad in ["/", "/a/", "/a/.", "/a/..", "relative"] {
            assert_eq!(split_parent(bad), None, "{bad}");
        }
        assert_eq!(join("/", "x"), "/x");
        assert_eq!(join("/a", "x"), "/a/x");
    }

    #[test]
    fn sibling_names_are_hidden_and_bounded() {
        assert_eq!(sibling_name("a.rs", "ff00", "tmp"), ".a.rs.terax-ff00.tmp");
        let long = "é".repeat(200);
        let s = sibling_name(&long, "00", "bak");
        assert!(s.len() < 160);
        assert!(s.starts_with(".é"));
    }

    #[test]
    fn mtime_is_second_granular_millis() {
        assert_eq!(mtime_millis(Some(2)), 2000);
        assert_eq!(mtime_millis(None), 0);
    }

    #[test]
    fn errors_render_for_ipc() {
        let s: String = FsError::NotFound("/x".into()).into();
        assert_eq!(s, "no such file or directory: /x");
        assert!(FsError::Timeout.is_connection());
        assert!(!FsError::Failure("x".into()).is_connection());
    }
}
