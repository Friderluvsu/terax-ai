use futures_util::stream::{self, StreamExt};

#[cfg(not(unix))]
use super::FsError;
use super::{join, EntryKind as RemoteKind, ReadOutcome, RemoteEntry, RemoteFs};
use crate::modules::fs::file::{
    classify_bytes, FileStat, ReadResult, StatKind, FORCE_MAX_READ_BYTES, MAX_READ_BYTES,
};
use crate::modules::fs::tree::{natural_cmp, DirEntry, EntryKind};

const RESOLVE_CONCURRENCY: usize = 8;

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
}
