use futures_util::stream::{self, StreamExt, TryStreamExt};
use std::future::Future;
use std::path::{Path, PathBuf};

use super::{
    join, nonce, sibling_name, split_parent, validate_path, EntryKind as RemoteKind, FsError,
    ReadOutcome, RemoteEntry, RemoteFs, RemoteMetadata,
};
use crate::modules::fs::file::{
    classify_bytes, FileStat, ReadResult, StatKind, FORCE_MAX_READ_BYTES, MAX_READ_BYTES,
};
use crate::modules::fs::mutate::{FsDeleteBatchResult, FsMoveResult};
use crate::modules::fs::tree::{natural_cmp, DirEntry, EntryKind};
use crate::modules::remote::registry::{is_under, normalize_posix};
use crate::modules::remote::RemoteRegistry;

const RESOLVE_CONCURRENCY: usize = 8;
/// Upper bound on entries one recursive delete or upload may touch.
pub const MAX_TREE_ENTRIES: usize = 10_000;
pub const MAX_UPLOAD_BYTES: u64 = 256 * 1024 * 1024;

#[cfg(unix)]
pub fn host_fs(app: &tauri::AppHandle, host: &str) -> Result<super::sftp::SftpHost, String> {
    use tauri::Manager;
    app.state::<crate::modules::remote::RemoteState>()
        .sftp(&app.config().identifier, host)
}

#[cfg(not(unix))]
pub struct UnsupportedFs;

#[cfg(not(unix))]
pub fn host_fs(_app: &tauri::AppHandle, _host: &str) -> Result<UnsupportedFs, String> {
    Err("SSH workspaces are not supported on this platform".into())
}

#[cfg(not(unix))]
macro_rules! unsupported {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {
        impl RemoteFs for UnsupportedFs {
            $(async fn $name(&self, $($arg: $ty),*) -> Result<$ret, FsError> {
                $(let _ = $arg;)*
                Err(FsError::Unsupported("SSH workspaces".into()))
            })*
        }
    };
}

#[cfg(not(unix))]
unsupported! {
    read_dir(path: &str) -> Vec<RemoteEntry>;
    stat(path: &str) -> super::RemoteMetadata;
    read(path: &str, limit: u64) -> ReadOutcome;
    write_atomic(path: &str, data: &[u8]) -> super::RemoteMetadata;
    write_new(path: &str, data: &[u8], mode: Option<u32>) -> ();
    write_in_place(path: &str, data: &[u8]) -> ();
    lstat(path: &str) -> super::RemoteMetadata;
    posix_rename(from: &str, to: &str) -> ();
    mkdir(path: &str) -> ();
    remove(path: &str) -> ();
    rename(from: &str, to: &str) -> ();
    realpath(path: &str) -> String;
    setstat(path: &str, attrs: super::SetAttrs) -> ();
    supports_posix_rename() -> bool;
}

struct Resolved {
    name: String,
    kind: EntryKind,
    size: u64,
    mtime: u64,
}

/// Mirrors the local listing: symlinks report their target's kind, size and
/// mtime, and stay `Symlink` only when the target cannot be resolved.
async fn resolve<F: RemoteFs>(fs: &F, dir: &str, e: RemoteEntry) -> Resolved {
    let plain = |kind| Resolved {
        kind,
        size: e.meta.size,
        mtime: e.meta.mtime,
        name: e.name.clone(),
    };
    if e.meta.kind != RemoteKind::Symlink {
        let kind = if e.meta.kind == RemoteKind::Dir {
            EntryKind::Dir
        } else {
            EntryKind::File
        };
        return plain(kind);
    }
    let link = join(dir, &e.name);
    let Ok(target) = fs.realpath(&link).await else {
        return plain(EntryKind::Symlink);
    };
    match fs.stat(&target).await {
        Ok(m) => Resolved {
            kind: if m.kind == RemoteKind::Dir {
                EntryKind::Dir
            } else {
                EntryKind::File
            },
            size: m.size,
            mtime: m.mtime,
            name: e.name,
        },
        Err(_) => plain(EntryKind::Symlink),
    }
}

async fn resolved_entries<F: RemoteFs>(
    fs: &F,
    path: &str,
    show_hidden: bool,
) -> Result<Vec<Resolved>, String> {
    let entries = fs.read_dir(path).await?;
    Ok(stream::iter(
        entries
            .into_iter()
            .filter(|e| show_hidden || !e.name.starts_with('.')),
    )
    .map(|e| resolve(fs, path, e))
    .buffered(RESOLVE_CONCURRENCY)
    .collect()
    .await)
}

pub async fn read_dir<F: RemoteFs>(
    fs: &F,
    path: &str,
    show_hidden: bool,
) -> Result<Vec<DirEntry>, String> {
    let mut entries: Vec<DirEntry> = resolved_entries(fs, path, show_hidden)
        .await?
        .into_iter()
        .map(|r| DirEntry {
            name: r.name,
            kind: r.kind,
            size: r.size,
            mtime: r.mtime,
            gitignored: false,
        })
        .collect();
    entries.sort_by(|a, b| {
        let rank = |k: &EntryKind| match k {
            EntryKind::Dir => 0,
            EntryKind::Symlink => 1,
            EntryKind::File => 2,
        };
        rank(&a.kind)
            .cmp(&rank(&b.kind))
            .then_with(|| natural_cmp(&a.name, &b.name))
    });
    Ok(entries)
}

pub async fn list_subdirs<F: RemoteFs>(
    fs: &F,
    path: &str,
    show_hidden: bool,
) -> Result<Vec<String>, String> {
    let mut dirs: Vec<String> = resolved_entries(fs, path, show_hidden)
        .await?
        .into_iter()
        .filter(|r| matches!(r.kind, EntryKind::Dir))
        .map(|r| r.name)
        .collect();
    dirs.sort_by(|a, b| natural_cmp(a, b));
    Ok(dirs)
}

pub async fn read_file<F: RemoteFs>(fs: &F, path: &str, force: bool) -> Result<ReadResult, String> {
    let limit = if force {
        FORCE_MAX_READ_BYTES
    } else {
        MAX_READ_BYTES
    };
    Ok(match fs.read(path, limit).await? {
        ReadOutcome::TooLarge { size, limit } => ReadResult::TooLarge { size, limit },
        ReadOutcome::Data { bytes, size, mtime } => classify_bytes(bytes, size, mtime),
    })
}

pub async fn canonicalize<F: RemoteFs>(fs: &F, path: &str) -> Result<String, String> {
    Ok(fs.realpath(path).await?)
}

pub async fn stat<F: RemoteFs>(fs: &F, path: &str) -> Result<FileStat, String> {
    let m = fs.stat(path).await.map_err(String::from)?;
    Ok(FileStat {
        size: m.size,
        mtime: m.mtime,
        kind: match m.kind {
            RemoteKind::Symlink => StatKind::Symlink,
            RemoteKind::Dir => StatKind::Dir,
            _ => StatKind::File,
        },
    })
}

#[cfg(unix)]
pub async fn mv_force(
    app: &tauri::AppHandle,
    host: &str,
    from: String,
    to: String,
) -> Result<(), String> {
    use crate::modules::remote::{control, exec, RemoteState};
    use tauri::Manager;
    const MV_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
    let dir = app
        .state::<RemoteState>()
        .socket_dir(&app.config().identifier)?;
    let host = host.to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let out = exec::exec(&dir, &host, &mv_script(&from, &to), MV_TIMEOUT, 4096)?;
        control::ensure_success(&out)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(not(unix))]
pub async fn mv_force(
    _app: &tauri::AppHandle,
    _host: &str,
    _from: String,
    _to: String,
) -> Result<(), String> {
    Err("SSH workspaces are not supported on this platform".into())
}

/// `mv` moves into a directory instead of replacing it, so one is refused.
#[cfg_attr(not(unix), allow(dead_code))]
pub fn mv_script(from: &str, to: &str) -> String {
    use crate::modules::remote::quote::posix_quote;
    let (from, to) = (posix_quote(from), posix_quote(to));
    format!(
        "if [ -d {to} ]; then echo 'destination is a directory' >&2; exit 1; fi; exec mv -f -- {from} {to}"
    )
}

/// What a mutation may touch: any root registered for the host, or a single
/// root a caller already authorized (move, batch delete).
pub enum Scope<'a> {
    Host {
        registry: &'a RemoteRegistry,
        host: &'a str,
    },
    Root(String),
}

impl<'a> Scope<'a> {
    pub fn host(registry: &'a RemoteRegistry, host: &'a str) -> Self {
        Self::Host { registry, host }
    }

    fn allows(&self, resolved: &str) -> bool {
        match self {
            Self::Host { registry, host } => registry.is_authorized(host, resolved),
            Self::Root(root) => normalize_posix(resolved).is_ok_and(|p| is_under(root, &p)),
        }
    }
}

pub async fn authorized_root<F: RemoteFs>(
    fs: &F,
    registry: &RemoteRegistry,
    host: &str,
    root: &str,
) -> Result<Scope<'static>, String> {
    validate_path(root)?;
    let real = fs
        .realpath(root)
        .await
        .map_err(|e| format!("workspace root is not accessible: {e}"))?;
    if !registry.is_authorized(host, &real) {
        return Err(format!("workspace root is not authorized: {real}"));
    }
    Ok(Scope::Root(normalize_posix(&real)?))
}

fn outside(path: &str) -> String {
    format!("path is outside the authorized workspace: {path}")
}

/// Resolves the parent on the server and keeps the final component, so a
/// symlinked parent cannot lead outside the scope and the entry itself is
/// never followed.
async fn entry<F: RemoteFs>(fs: &F, scope: &Scope<'_>, path: &str) -> Result<String, String> {
    validate_path(path)?;
    let (parent, name) =
        split_parent(path).ok_or_else(|| format!("invalid remote path: {path}"))?;
    let real = fs
        .realpath(parent)
        .await
        .map_err(|e| format!("path parent is not accessible: {e}"))?;
    if !scope.allows(&real) {
        return Err(outside(path));
    }
    Ok(join(&real, name))
}

/// The file a save changes: a symlink is followed so the link survives and
/// its target, which must be in scope too, gets the new content.
async fn write_target<F: RemoteFs>(
    fs: &F,
    scope: &Scope<'_>,
    path: &str,
) -> Result<String, String> {
    let entry = entry(fs, scope, path).await?;
    match fs.realpath(&entry).await {
        Ok(real) if real == entry => Ok(entry),
        Ok(real) if scope.allows(&real) => Ok(real),
        Ok(_) => Err(outside(path)),
        Err(FsError::NotFound(_)) => Ok(entry),
        Err(e) => Err(e.into()),
    }
}

async fn lstat_opt<F: RemoteFs>(fs: &F, path: &str) -> Result<Option<RemoteMetadata>, String> {
    match fs.lstat(path).await {
        Ok(m) => Ok(Some(m)),
        Err(FsError::NotFound(_)) => Ok(None),
        Err(e) => Err(e.into()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Saved {
    pub mtime: u64,
    /// The directory was not writable, so the file was rewritten in place.
    pub in_place: bool,
}

/// Writes a random sibling of the real file with the old mode already set,
/// then swaps it in with posix-rename, or `mv` (`mv_force`) when the server
/// lacks it. The sibling is removed when the swap fails.
pub async fn save<F, M, Fut>(
    fs: &F,
    scope: &Scope<'_>,
    path: &str,
    data: &[u8],
    mv_force: M,
) -> Result<Saved, String>
where
    F: RemoteFs,
    M: FnOnce(String, String) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let target = write_target(fs, scope, path).await?;
    let existing = match fs.stat(&target).await {
        Ok(m) => Some(m),
        Err(FsError::NotFound(_)) => None,
        Err(e) => return Err(e.into()),
    };
    if existing.is_some_and(|m| m.kind == RemoteKind::Dir) {
        return Err(format!("{path}: is a directory"));
    }
    let (parent, name) =
        split_parent(&target).ok_or_else(|| format!("invalid remote path: {target}"))?;
    let tmp = join(parent, &sibling_name(name, &nonce()?, "tmp"));
    let posix = fs.supports_posix_rename().await?;
    match fs
        .write_new(&tmp, data, existing.and_then(|m| m.mode))
        .await
    {
        Ok(()) => {}
        Err(FsError::PermissionDenied(_))
            if existing.is_some_and(|m| m.kind == RemoteKind::File) =>
        {
            log::warn!("{target}: directory not writable, saving in place");
            fs.write_in_place(&target, data).await?;
            let mtime = fs.stat(&target).await?.mtime;
            return Ok(Saved {
                mtime,
                in_place: true,
            });
        }
        Err(FsError::NotFound(_)) => return Err(FsError::NotFound(target).into()),
        Err(FsError::PermissionDenied(_)) => return Err(FsError::PermissionDenied(target).into()),
        Err(e) => return Err(e.into()),
    }
    let swapped = if posix {
        fs.posix_rename(&tmp, &target).await.map_err(String::from)
    } else {
        mv_force(tmp.clone(), target.clone()).await
    };
    if let Err(e) = swapped {
        let _ = fs.remove(&tmp).await;
        return Err(e);
    }
    Ok(Saved {
        mtime: fs.stat(&target).await?.mtime,
        in_place: false,
    })
}

pub async fn create_file<F: RemoteFs>(fs: &F, scope: &Scope<'_>, path: &str) -> Result<(), String> {
    let target = entry(fs, scope, path).await?;
    if lstat_opt(fs, &target).await?.is_some() {
        return Err(format!("already exists: {path}"));
    }
    Ok(fs.write_new(&target, b"", None).await?)
}

/// Creates missing parents like `create_dir_all`, starting below the deepest
/// existing ancestor, which must resolve inside the scope.
pub async fn create_dir<F: RemoteFs>(fs: &F, scope: &Scope<'_>, path: &str) -> Result<(), String> {
    validate_path(path)?;
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    if parts.iter().any(|p| *p == "." || *p == "..") {
        return Err(format!("invalid remote path: {path}"));
    }
    if parts.is_empty() || lstat_opt(fs, path).await?.is_some() {
        return Err(format!("already exists: {path}"));
    }
    let mut have = parts.len() - 1;
    let base = loop {
        let prefix = format!("/{}", parts[..have].join("/"));
        let real = match fs.realpath(&prefix).await {
            Ok(real) => real,
            Err(FsError::NotFound(_)) if have > 0 => {
                have -= 1;
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        match fs.stat(&real).await {
            Ok(m) if m.kind == RemoteKind::Dir => break real,
            Ok(_) => return Err(format!("not a directory: {prefix}")),
            Err(FsError::NotFound(_)) if have > 0 => have -= 1,
            Err(e) => return Err(e.into()),
        }
    };
    if !scope.allows(&base) {
        return Err(outside(path));
    }
    let mut cur = base;
    for part in &parts[have..] {
        cur = join(&cur, part);
        fs.mkdir(&cur).await?;
    }
    Ok(())
}

pub async fn rename<F: RemoteFs>(
    fs: &F,
    scope: &Scope<'_>,
    from: &str,
    to: &str,
) -> Result<(), String> {
    let src = entry(fs, scope, from).await?;
    let dst = entry(fs, scope, to).await?;
    if lstat_opt(fs, &src).await?.is_none() {
        return Err(format!("not found: {from}"));
    }
    if lstat_opt(fs, &dst).await?.is_some() {
        return Err(format!("already exists: {to}"));
    }
    Ok(fs.rename(&src, &dst).await?)
}

/// SFTP v3 has no inode, so the token pins type, mode, size and mtime.
fn conflict_token(meta: &RemoteMetadata) -> String {
    format!(
        "sftp:{}:{}:{}",
        meta.mode.unwrap_or(0),
        meta.size,
        meta.mtime
    )
}

fn conflict(src: &RemoteMetadata, dst: &RemoteMetadata) -> FsMoveResult {
    FsMoveResult::Conflict {
        replaceable: src.kind != RemoteKind::Dir && dst.kind != RemoteKind::Dir,
        token: conflict_token(dst),
    }
}

/// Mirrors the local move: never clobbers unless the caller approved the
/// exact conflict token it was shown.
pub async fn move_entry<F, M, Fut>(
    fs: &F,
    root: &Scope<'_>,
    from: &str,
    to: &str,
    expected_conflict: Option<&str>,
    mv_force: M,
) -> Result<FsMoveResult, String>
where
    F: RemoteFs,
    M: FnOnce(String, String) -> Fut,
    Fut: Future<Output = Result<(), String>>,
{
    let src = entry(fs, root, from).await?;
    let dst = entry(fs, root, to).await?;
    if src == dst {
        return Ok(FsMoveResult::Moved);
    }
    let src_meta = lstat_opt(fs, &src)
        .await?
        .ok_or_else(|| format!("not found: {from}"))?;
    if let Some(dst_meta) = lstat_opt(fs, &dst).await? {
        let found = conflict(&src_meta, &dst_meta);
        let approved = matches!(
            &found,
            FsMoveResult::Conflict { replaceable: true, token }
                if expected_conflict == Some(token.as_str())
        );
        if !approved {
            return Ok(found);
        }
        if fs.supports_posix_rename().await? {
            fs.posix_rename(&src, &dst).await?;
        } else {
            mv_force(src, dst).await?;
        }
        return Ok(FsMoveResult::Moved);
    }
    match fs.rename(&src, &dst).await {
        Ok(()) => Ok(FsMoveResult::Moved),
        Err(e) => match lstat_opt(fs, &dst).await? {
            Some(dst_meta) => Ok(conflict(&src_meta, &dst_meta)),
            None => Err(e.into()),
        },
    }
}

/// Lists everything below `path` first and refuses trees larger than
/// `limit`, so nothing is deleted when the walk would be cut short.
async fn remove_tree<F: RemoteFs>(fs: &F, path: &str, limit: usize) -> Result<(), String> {
    if fs.lstat(path).await?.kind != RemoteKind::Dir {
        return Ok(fs.remove(path).await?);
    }
    let mut pending = vec![path.to_string()];
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    while let Some(dir) = pending.pop() {
        for e in fs.read_dir(&dir).await? {
            let child = join(&dir, &e.name);
            if e.meta.kind == RemoteKind::Dir {
                pending.push(child.clone());
                dirs.push(child);
            } else {
                files.push(child);
            }
            if dirs.len() + files.len() > limit {
                return Err(format!(
                    "{path}: more than {limit} entries; delete it from a terminal"
                ));
            }
        }
    }
    stream::iter(files)
        .map(|f| async move { fs.remove(&f).await })
        .buffer_unordered(RESOLVE_CONCURRENCY)
        .try_collect::<Vec<()>>()
        .await?;
    for dir in dirs.iter().rev() {
        fs.remove(dir).await?;
    }
    Ok(fs.remove(path).await?)
}

pub async fn delete<F: RemoteFs>(fs: &F, scope: &Scope<'_>, path: &str) -> Result<(), String> {
    let target = entry(fs, scope, path).await?;
    remove_tree(fs, &target, MAX_TREE_ENTRIES).await
}

pub async fn delete_batch<F: RemoteFs>(
    fs: &F,
    root: &Scope<'_>,
    paths: Vec<String>,
) -> FsDeleteBatchResult {
    let mut result = FsDeleteBatchResult {
        deleted: Vec::with_capacity(paths.len()),
        failed: 0,
    };
    for path in paths {
        match delete(fs, root, &path).await {
            Ok(()) => result.deleted.push(path),
            Err(error) => {
                result.failed += 1;
                log::warn!("remote delete_batch({path}) failed: {error}");
            }
        }
    }
    result
}

#[cfg(unix)]
fn local_mode(meta: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(meta.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn local_mode(_meta: &std::fs::Metadata) -> Option<u32> {
    None
}

fn utf8_name(path: &Path) -> Result<&str, String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("invalid source: {}", path.display()))
}

/// Uploads local files and folders into `dest_dir`; never overwrites.
pub async fn upload<F: RemoteFs>(
    fs: &F,
    scope: &Scope<'_>,
    sources: &[String],
    dest_dir: &str,
) -> Result<(), String> {
    validate_path(dest_dir)?;
    let dest = fs.realpath(dest_dir).await?;
    if !scope.allows(&dest) {
        return Err(outside(dest_dir));
    }
    let mut budget = MAX_TREE_ENTRIES;
    for source in sources {
        let src = PathBuf::from(source);
        let target = join(&dest, utf8_name(&src)?);
        if lstat_opt(fs, &target).await?.is_some() {
            return Err(format!("already exists: {target}"));
        }
        let mut pending = vec![(src, target)];
        while let Some((src, target)) = pending.pop() {
            budget = budget
                .checked_sub(1)
                .ok_or_else(|| format!("more than {MAX_TREE_ENTRIES} entries to upload"))?;
            let meta = std::fs::metadata(&src).map_err(|e| e.to_string())?;
            if meta.is_dir() {
                fs.mkdir(&target).await?;
                for child in std::fs::read_dir(&src).map_err(|e| e.to_string())? {
                    let child = child.map_err(|e| e.to_string())?.path();
                    let name = join(&target, utf8_name(&child)?);
                    pending.push((child, name));
                }
            } else {
                if meta.len() > MAX_UPLOAD_BYTES {
                    return Err(format!(
                        "{}: larger than {} MB",
                        src.display(),
                        MAX_UPLOAD_BYTES >> 20
                    ));
                }
                let data = std::fs::read(&src).map_err(|e| e.to_string())?;
                fs.write_new(&target, &data, local_mode(&meta)).await?;
            }
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::modules::fs::file::{read_file_sync, stat_sync};
    use crate::modules::fs::tree::{list_subdirs_local, read_dir_local};
    use crate::modules::remote::fs::sftp::{test_launch, SftpConn};
    use serde_json::{json, Value};
    use std::path::Path;

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
    }

    fn seed() -> (tempfile::TempDir, String) {
        let tmp = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(tmp.path()).unwrap();
        let r = root.to_str().unwrap().to_string();
        std::fs::write(root.join("b10.txt"), "ten").unwrap();
        std::fs::write(root.join("b2.txt"), "two!").unwrap();
        std::fs::write(root.join(".hidden"), "h").unwrap();
        std::fs::write(root.join("bin.dat"), b"PNG\0\x89").unwrap();
        std::fs::write(root.join("bad.dat"), [0xff, 0xfe, 0xfd]).unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::fs::create_dir(root.join(".dot")).unwrap();
        let link = |to: &str, name: &str| std::os::unix::fs::symlink(to, root.join(name)).unwrap();
        link("sub", "dirlink");
        link("b2.txt", "filelink");
        link("nowhere", "broken");
        (tmp, r)
    }

    fn norm(mut v: Value) -> Value {
        match &mut v {
            Value::Object(m) => {
                if let Some(Value::Number(n)) = m.get("mtime") {
                    let secs = n.as_u64().unwrap() / 1000;
                    m.insert("mtime".into(), json!(secs));
                }
                m.values_mut().for_each(|x| *x = norm(x.take()));
            }
            Value::Array(a) => a.iter_mut().for_each(|x| *x = norm(x.take())),
            _ => {}
        }
        v
    }

    fn j<T: serde::Serialize>(v: T) -> Value {
        norm(serde_json::to_value(v).unwrap())
    }

    #[test]
    fn remote_responses_match_local_shapes() {
        let Some(launch) = test_launch() else {
            eprintln!("sftp-server not found, skipping");
            return;
        };
        let (_tmp, root) = seed();
        let p = Path::new(&root);
        rt().block_on(async {
            let fs = SftpConn::connect(&launch).await.unwrap();
            for hidden in [false, true] {
                assert_eq!(
                    j(read_dir(&fs, &root, hidden).await.unwrap()),
                    j(read_dir_local(p, hidden, None).unwrap()),
                    "read_dir hidden={hidden}"
                );
                assert_eq!(
                    list_subdirs(&fs, &root, hidden).await.unwrap(),
                    list_subdirs_local(p, hidden).unwrap(),
                    "list_subdirs hidden={hidden}"
                );
            }
            for name in ["b2.txt", "bin.dat", "bad.dat", "filelink", "sub", "dirlink"] {
                let f = format!("{root}/{name}");
                assert_eq!(
                    j(stat(&fs, &f).await),
                    j(stat_sync(Path::new(&f))),
                    "stat {name}"
                );
            }
            let broken = format!("{root}/broken");
            assert!(stat(&fs, &broken).await.is_err());
            assert!(stat_sync(Path::new(&broken)).is_err());
            for name in ["b2.txt", "bin.dat", "bad.dat", "filelink"] {
                let f = format!("{root}/{name}");
                assert_eq!(
                    j(read_file(&fs, &f, false).await),
                    j(read_file_sync(Path::new(&f), false)),
                    "read {name}"
                );
            }
            assert!(read_file(&fs, &format!("{root}/sub"), false).await.is_err());
            assert_eq!(
                canonicalize(&fs, &format!("{root}/dirlink")).await.unwrap(),
                format!("{root}/sub")
            );
        });
    }

    #[test]
    fn remote_read_applies_size_limits_and_force() {
        let Some(launch) = test_launch() else {
            return;
        };
        let (_tmp, root) = seed();
        let big = format!("{root}/big.txt");
        std::fs::write(&big, vec![b'a'; (MAX_READ_BYTES + 1) as usize]).unwrap();
        rt().block_on(async {
            let fs = SftpConn::connect(&launch).await.unwrap();
            assert!(matches!(
                read_file(&fs, &big, false).await.unwrap(),
                ReadResult::TooLarge { limit, .. } if limit == MAX_READ_BYTES
            ));
            assert!(matches!(
                read_file(&fs, &big, true).await.unwrap(),
                ReadResult::Text { .. }
            ));
        });
    }

    mod mutations {
        use super::super::*;
        use super::rt;
        use crate::modules::remote::exec::run;
        use crate::modules::remote::fs::sftp::{test_launch, SftpConn};
        use std::os::unix::fs::{symlink, PermissionsExt};
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::{Arc, Mutex};

        const HOST: &str = "box";

        struct Tree {
            _tmp: tempfile::TempDir,
            root: String,
            outside: String,
            registry: RemoteRegistry,
        }

        impl Tree {
            fn new() -> Self {
                let tmp = tempfile::tempdir().unwrap();
                let base = std::fs::canonicalize(tmp.path()).unwrap();
                let root = base.join("root");
                let outside = base.join("outside");
                std::fs::create_dir(&root).unwrap();
                std::fs::create_dir(&outside).unwrap();
                std::fs::write(outside.join("secret.txt"), "secret").unwrap();
                let registry = RemoteRegistry::default();
                registry.add_root(HOST, root.to_str().unwrap()).unwrap();
                Self {
                    _tmp: tmp,
                    root: root.to_str().unwrap().into(),
                    outside: outside.to_str().unwrap().into(),
                    registry,
                }
            }

            fn scope(&self) -> Scope<'_> {
                Scope::host(&self.registry, HOST)
            }

            fn p(&self, rel: &str) -> String {
                format!("{}/{rel}", self.root)
            }

            fn secret_intact(&self) {
                assert_eq!(
                    std::fs::read_to_string(format!("{}/secret.txt", self.outside)).unwrap(),
                    "secret"
                );
                let names: Vec<_> = std::fs::read_dir(&self.outside)
                    .unwrap()
                    .map(|e| e.unwrap().file_name())
                    .collect();
                assert_eq!(names.len(), 1, "{names:?}");
            }
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

        async fn local_mv(from: String, to: String) -> Result<(), String> {
            std::fs::rename(from, to).map_err(|e| e.to_string())
        }

        async fn no_mv(_: String, _: String) -> Result<(), String> {
            panic!("posix-rename is available, mv must not run")
        }

        macro_rules! conn_or_skip {
            () => {
                match test_launch() {
                    Some(l) => SftpConn::connect(&l).await.unwrap(),
                    None => {
                        eprintln!("sftp-server not found, skipping");
                        return;
                    }
                }
            };
        }

        #[test]
        fn readers_never_see_a_partial_save() {
            let t = Tree::new();
            let f = t.p("big.txt");
            let (a, b) = (vec![b'a'; 3 << 20], vec![b'b'; (2 << 20) + 7]);
            std::fs::write(&f, &a).unwrap();
            let stop = Arc::new(AtomicBool::new(false));
            let reader = {
                let (f, stop) = (f.clone(), stop.clone());
                let (la, lb) = (a.len(), b.len());
                std::thread::spawn(move || {
                    let mut reads = 0;
                    while !stop.load(Ordering::Relaxed) {
                        let got = std::fs::read(&f).unwrap();
                        let whole = (got.len() == la && got.iter().all(|&c| c == b'a'))
                            || (got.len() == lb && got.iter().all(|&c| c == b'b'));
                        assert!(whole, "torn read of {} bytes", got.len());
                        reads += 1;
                    }
                    reads
                })
            };
            rt().block_on(async {
                let fs = conn_or_skip!();
                for i in 0..8 {
                    let data = if i % 2 == 0 { &b } else { &a };
                    save(&fs, &t.scope(), &f, data, no_mv).await.unwrap();
                }
                let fs = fs.without_posix_rename();
                for i in 0..4 {
                    let data = if i % 2 == 0 { &b } else { &a };
                    save(&fs, &t.scope(), &f, data, local_mv).await.unwrap();
                }
            });
            stop.store(true, Ordering::Relaxed);
            assert!(reader.join().unwrap() > 0);
            assert!(leftovers(&t.root).is_empty());
        }

        #[test]
        fn mode_is_set_before_the_swap_and_mtime_returned() {
            let t = Tree::new();
            let f = t.p("script.sh");
            std::fs::write(&f, "old").unwrap();
            std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o750)).unwrap();
            let seen = Mutex::new(None);
            rt().block_on(async {
                let fs = conn_or_skip!().without_posix_rename();
                let mv = |from: String, to: String| {
                    *seen.lock().unwrap() = Some(mode(&from));
                    local_mv(from, to)
                };
                let saved = save(&fs, &t.scope(), &f, b"new", mv).await.unwrap();
                assert!(!saved.in_place);
                assert_eq!(saved.mtime, stat(&fs, &f).await.unwrap().mtime);
                assert!(saved.mtime > 0);
            });
            assert_eq!(*seen.lock().unwrap(), Some(0o750));
            assert_eq!(mode(&f), 0o750);
            assert_eq!(std::fs::read(&f).unwrap(), b"new");
        }

        #[test]
        fn saving_through_a_symlink_keeps_the_link() {
            let t = Tree::new();
            std::fs::create_dir(t.p("sub")).unwrap();
            std::fs::write(t.p("real.txt"), "old").unwrap();
            symlink("real.txt", t.p("link")).unwrap();
            symlink("../real.txt", t.p("sub/uplink")).unwrap();
            rt().block_on(async {
                let fs = conn_or_skip!();
                save(&fs, &t.scope(), &t.p("link"), b"one", no_mv)
                    .await
                    .unwrap();
                save(&fs, &t.scope(), &t.p("sub/uplink"), b"two", no_mv)
                    .await
                    .unwrap();
            });
            for link in ["link", "sub/uplink"] {
                let meta = std::fs::symlink_metadata(t.p(link)).unwrap();
                assert!(meta.file_type().is_symlink(), "{link}");
            }
            assert_eq!(std::fs::read(t.p("real.txt")).unwrap(), b"two");
            assert!(leftovers(&t.root).is_empty());
            assert!(leftovers(&t.p("sub")).is_empty());
        }

        #[test]
        fn mutations_outside_the_registered_root_are_refused() {
            let t = Tree::new();
            let secret = format!("{}/secret.txt", t.outside);
            std::fs::write(t.p("a.txt"), "a").unwrap();
            symlink(&secret, t.p("escape")).unwrap();
            symlink(&t.outside, t.p("dirlink")).unwrap();
            let local = tempfile::tempdir().unwrap();
            let upload_src = local.path().join("up.txt");
            std::fs::write(&upload_src, "up").unwrap();
            let up = vec![upload_src.to_str().unwrap().to_string()];
            rt().block_on(async {
                let fs = conn_or_skip!();
                let sc = t.scope();
                let refused = |r: Result<(), String>| {
                    let e = r.unwrap_err();
                    assert!(e.contains("outside the authorized workspace"), "{e}");
                };
                let saved = |r: Result<Saved, String>| refused(r.map(drop));
                saved(save(&fs, &sc, &secret, b"x", no_mv).await);
                saved(save(&fs, &sc, &t.p("escape"), b"x", no_mv).await);
                saved(save(&fs, &sc, &t.p("dirlink/secret.txt"), b"x", no_mv).await);
                saved(save(&fs, &sc, &t.p("../outside/secret.txt"), b"x", no_mv).await);
                saved(save(&fs, &sc, &t.p("dirlink/new.txt"), b"x", no_mv).await);
                refused(create_file(&fs, &sc, &t.p("dirlink/new.txt")).await);
                refused(create_dir(&fs, &sc, &t.p("dirlink/new/deeper")).await);
                refused(rename(&fs, &sc, &t.p("a.txt"), &t.p("dirlink/a.txt")).await);
                refused(rename(&fs, &sc, &t.p("dirlink/secret.txt"), &t.p("s.txt")).await);
                refused(delete(&fs, &sc, &t.p("dirlink/secret.txt")).await);
                refused(upload(&fs, &sc, &up, &t.p("dirlink")).await);
                let moved = move_entry(
                    &fs,
                    &sc,
                    &t.p("dirlink/secret.txt"),
                    &t.p("s.txt"),
                    None,
                    no_mv,
                )
                .await;
                refused(moved.map(drop));

                let other = RemoteRegistry::default();
                let e = save(&fs, &Scope::host(&other, HOST), &t.p("a.txt"), b"x", no_mv)
                    .await
                    .unwrap_err();
                assert!(e.contains("outside"), "{e}");
                assert!(authorized_root(&fs, &t.registry, HOST, &t.outside)
                    .await
                    .is_err());
                assert!(authorized_root(&fs, &t.registry, HOST, &t.p("dirlink"))
                    .await
                    .is_err());
                assert!(authorized_root(&fs, &t.registry, "other", &t.root)
                    .await
                    .is_err());
                let root = authorized_root(&fs, &t.registry, HOST, &t.root)
                    .await
                    .unwrap();
                let batch = delete_batch(&fs, &root, vec![t.p("dirlink/secret.txt")]).await;
                assert_eq!((batch.deleted.len(), batch.failed), (0, 1));

                // Deleting the link itself is in scope and never follows it.
                delete(&fs, &sc, &t.p("dirlink")).await.unwrap();
                delete(&fs, &sc, &t.p("escape")).await.unwrap();
            });
            t.secret_intact();
            assert_eq!(std::fs::read(t.p("a.txt")).unwrap(), b"a");
        }

        #[test]
        fn move_needs_the_current_conflict_token() {
            let t = Tree::new();
            std::fs::write(t.p("from.txt"), "new").unwrap();
            std::fs::write(t.p("to.txt"), "old").unwrap();
            std::fs::create_dir(t.p("dir")).unwrap();
            rt().block_on(async {
                let fs = conn_or_skip!();
                let root = authorized_root(&fs, &t.registry, HOST, &t.root)
                    .await
                    .unwrap();
                let (from, to) = (t.p("from.txt"), t.p("to.txt"));
                let token = match move_entry(&fs, &root, &from, &to, None, no_mv).await {
                    Ok(FsMoveResult::Conflict {
                        replaceable: true,
                        token,
                    }) => token,
                    other => panic!("{other:?}"),
                };
                assert_eq!(std::fs::read(&from).unwrap(), b"new");
                assert_eq!(std::fs::read(&to).unwrap(), b"old");

                std::fs::write(&to, "changed after prompt").unwrap();
                assert!(matches!(
                    move_entry(&fs, &root, &from, &to, Some(&token), no_mv).await,
                    Ok(FsMoveResult::Conflict { .. })
                ));
                assert_eq!(std::fs::read(&to).unwrap(), b"changed after prompt");

                let token = match move_entry(&fs, &root, &from, &to, None, no_mv).await {
                    Ok(FsMoveResult::Conflict { token, .. }) => token,
                    other => panic!("{other:?}"),
                };
                assert_eq!(
                    move_entry(&fs, &root, &from, &to, Some(&token), no_mv).await,
                    Ok(FsMoveResult::Moved)
                );
                assert!(!Path::new(&from).exists());
                assert_eq!(std::fs::read(&to).unwrap(), b"new");

                assert!(matches!(
                    move_entry(&fs, &root, &to, &t.p("dir"), None, no_mv).await,
                    Ok(FsMoveResult::Conflict {
                        replaceable: false,
                        ..
                    })
                ));
                assert_eq!(
                    move_entry(&fs, &root, &to, &t.p("dir/moved.txt"), None, no_mv).await,
                    Ok(FsMoveResult::Moved)
                );

                std::fs::write(t.p("x.txt"), "x").unwrap();
                let posixless = fs.without_posix_rename();
                let dst = t.p("dir/moved.txt");
                let token =
                    match move_entry(&posixless, &root, &t.p("x.txt"), &dst, None, no_mv).await {
                        Ok(FsMoveResult::Conflict { token, .. }) => token,
                        other => panic!("{other:?}"),
                    };
                assert_eq!(
                    move_entry(
                        &posixless,
                        &root,
                        &t.p("x.txt"),
                        &dst,
                        Some(&token),
                        local_mv
                    )
                    .await,
                    Ok(FsMoveResult::Moved)
                );
                assert_eq!(std::fs::read(&dst).unwrap(), b"x");
            });
        }

        #[test]
        fn failed_swap_removes_the_temporary() {
            let t = Tree::new();
            let f = t.p("keep.txt");
            std::fs::write(&f, "keep").unwrap();
            rt().block_on(async {
                let fs = conn_or_skip!().without_posix_rename();
                let failing = |_: String, _: String| async { Err("mv failed".to_string()) };
                let e = save(&fs, &t.scope(), &f, b"lost", failing)
                    .await
                    .unwrap_err();
                assert_eq!(e, "mv failed");
                let e = save(&fs, &t.scope(), &t.p("missing/f"), b"x", failing)
                    .await
                    .unwrap_err();
                assert!(e.contains("missing") && !e.contains(".terax-"), "{e}");
                std::fs::create_dir(t.p("d")).unwrap();
                assert!(save(&fs, &t.scope(), &t.p("d"), b"x", failing)
                    .await
                    .is_err());
            });
            assert_eq!(std::fs::read(&f).unwrap(), b"keep");
            assert!(leftovers(&t.root).is_empty());
        }

        #[test]
        fn unwritable_folder_falls_back_to_writing_in_place() {
            // SAFETY: geteuid has no preconditions.
            if unsafe { libc::geteuid() } == 0 {
                return;
            }
            let t = Tree::new();
            let dir = t.p("locked");
            std::fs::create_dir(&dir).unwrap();
            let f = format!("{dir}/f.txt");
            std::fs::write(&f, "old").unwrap();
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
            let result = rt().block_on(async {
                let fs = match test_launch() {
                    Some(l) => SftpConn::connect(&l).await.unwrap(),
                    None => return None,
                };
                let saved = save(&fs, &t.scope(), &f, b"new", no_mv).await;
                let created = save(&fs, &t.scope(), &format!("{dir}/n.txt"), b"x", no_mv).await;
                Some((saved, created))
            });
            std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
            let Some((saved, created)) = result else {
                return;
            };
            assert!(saved.unwrap().in_place);
            assert_eq!(std::fs::read(&f).unwrap(), b"new");
            assert_eq!(
                created.unwrap_err(),
                format!("permission denied: {dir}/n.txt")
            );
            assert!(leftovers(&dir).is_empty());
        }

        #[test]
        fn create_rename_delete_and_upload() {
            let t = Tree::new();
            let local = tempfile::tempdir().unwrap();
            std::fs::write(local.path().join("a.txt"), "payload").unwrap();
            std::fs::set_permissions(
                local.path().join("a.txt"),
                std::fs::Permissions::from_mode(0o751),
            )
            .unwrap();
            std::fs::create_dir_all(local.path().join("d/inner")).unwrap();
            std::fs::write(local.path().join("d/inner/y.txt"), "y").unwrap();
            let src = |n: &str| local.path().join(n).to_str().unwrap().to_string();
            rt().block_on(async {
                let fs = conn_or_skip!();
                let sc = t.scope();
                create_file(&fs, &sc, &t.p("new.txt")).await.unwrap();
                assert_eq!(std::fs::read(t.p("new.txt")).unwrap(), b"");
                std::fs::write(t.p("new.txt"), "data").unwrap();
                let e = create_file(&fs, &sc, &t.p("new.txt")).await.unwrap_err();
                assert!(e.contains("already exists"), "{e}");
                assert_eq!(std::fs::read(t.p("new.txt")).unwrap(), b"data");

                create_dir(&fs, &sc, &t.p("a/b/c")).await.unwrap();
                assert!(Path::new(&t.p("a/b/c")).is_dir());
                let e = create_dir(&fs, &sc, &t.p("a/b")).await.unwrap_err();
                assert!(e.contains("already exists"), "{e}");
                assert!(create_dir(&fs, &sc, &t.p("new.txt/x")).await.is_err());
                assert!(create_dir(&fs, &sc, &t.p("a/../z")).await.is_err());

                rename(&fs, &sc, &t.p("new.txt"), &t.p("a/moved.txt"))
                    .await
                    .unwrap();
                assert_eq!(std::fs::read(t.p("a/moved.txt")).unwrap(), b"data");
                std::fs::write(t.p("keep.txt"), "keep").unwrap();
                let e = rename(&fs, &sc, &t.p("a/moved.txt"), &t.p("keep.txt"))
                    .await
                    .unwrap_err();
                assert!(e.contains("already exists"), "{e}");
                let e = rename(&fs, &sc, &t.p("gone"), &t.p("x")).await.unwrap_err();
                assert!(e.contains("not found"), "{e}");

                upload(&fs, &sc, &[src("a.txt"), src("d")], &t.root)
                    .await
                    .unwrap();
                assert_eq!(std::fs::read(t.p("a.txt")).unwrap(), b"payload");
                assert_eq!(mode(&t.p("a.txt")), 0o751);
                assert_eq!(std::fs::read(t.p("d/inner/y.txt")).unwrap(), b"y");
                assert!(local.path().join("a.txt").exists());
                let e = upload(&fs, &sc, &[src("a.txt")], &t.root)
                    .await
                    .unwrap_err();
                assert!(e.contains("already exists"), "{e}");

                for i in 0..5 {
                    std::fs::write(t.p(&format!("a/b/f{i}")), "x").unwrap();
                }
                symlink(&t.outside, t.p("a/b/out")).unwrap();
                let a = t.p("a");
                assert!(remove_tree(&fs, &a, 4)
                    .await
                    .unwrap_err()
                    .contains("more than 4"));
                assert!(Path::new(&t.p("a/b/f0")).exists());
                delete(&fs, &sc, &a).await.unwrap();
                assert!(!Path::new(&a).exists());

                let root = authorized_root(&fs, &t.registry, HOST, &t.root)
                    .await
                    .unwrap();
                let paths = vec![t.p("keep.txt"), t.p("missing"), t.p("d")];
                let batch = delete_batch(&fs, &root, paths.clone()).await;
                assert_eq!(batch.deleted, vec![paths[0].clone(), paths[2].clone()]);
                assert_eq!(batch.failed, 1);
            });
            t.secret_intact();
            let rest: Vec<_> = std::fs::read_dir(&t.root)
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            assert_eq!(rest, vec!["a.txt".to_string()]);
        }

        #[test]
        fn mv_script_replaces_files_and_refuses_directories() {
            let tmp = tempfile::tempdir().unwrap();
            let p = |n: &str| tmp.path().join(n).to_str().unwrap().to_string();
            std::fs::write(p("it's $x.txt"), "new").unwrap();
            std::fs::write(p("to.txt"), "old").unwrap();
            std::fs::create_dir(p("dir")).unwrap();
            let sh = |script: String| {
                run(
                    Path::new("/bin/sh"),
                    &["-c".into(), script],
                    std::time::Duration::from_secs(10),
                    1024,
                )
                .unwrap()
            };
            let out = sh(mv_script(&p("it's $x.txt"), &p("dir")));
            assert_ne!(out.status, Some(0));
            assert!(Path::new(&p("it's $x.txt")).exists());
            assert_eq!(std::fs::read_dir(p("dir")).unwrap().count(), 0);
            let out = sh(mv_script(&p("it's $x.txt"), &p("to.txt")));
            assert_eq!(out.status, Some(0));
            assert_eq!(std::fs::read(p("to.txt")).unwrap(), b"new");
            assert!(!Path::new(&p("it's $x.txt")).exists());
        }
    }
}
