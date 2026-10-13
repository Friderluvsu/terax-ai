//! Backing store for Untitled editor buffers. Lives under `app_data_dir()/scratch`
//! and is addressed only by opaque ids, so the webview never supplies a path.

use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use tauri::Manager;
use tempfile::NamedTempFile;

pub(crate) const MAX_SCRATCH_BYTES: usize = 10 * 1024 * 1024;
const SCRATCH_DIR: &str = "scratch";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ScratchEntry {
    pub id: String,
    /// Milliseconds since the Unix epoch.
    pub mtime: u64,
}

fn is_valid_id(id: &str) -> bool {
    id.len() == 32 && id.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_id(id: &str) -> Result<(), String> {
    if is_valid_id(id) {
        Ok(())
    } else {
        Err("invalid scratch id".to_string())
    }
}

fn scratch_dir(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join(SCRATCH_DIR))
        .map_err(|e| e.to_string())
}

/// Temp file in the same directory, then rename, so a reader or a crash never
/// observes a partially written buffer.
pub(crate) fn write_in(dir: &Path, id: &str, content: &str) -> Result<(), String> {
    validate_id(id)?;
    if content.len() > MAX_SCRATCH_BYTES {
        return Err("Untitled buffer exceeds the 10 MB limit".to_string());
    }
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let mut tmp = NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
    tmp.as_file_mut()
        .write_all(content.as_bytes())
        .map_err(|e| e.to_string())?;
    tmp.as_file_mut().sync_all().map_err(|e| e.to_string())?;
    tmp.persist(dir.join(id)).map_err(|e| e.error.to_string())?;
    Ok(())
}

pub(crate) fn read_in(dir: &Path, id: &str) -> Result<Option<String>, String> {
    validate_id(id)?;
    let path = dir.join(id);
    let meta = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.to_string()),
    };
    if !meta.is_file() {
        return Err("scratch entry is not a regular file".to_string());
    }
    if meta.len() > MAX_SCRATCH_BYTES as u64 {
        return Err("Untitled buffer exceeds the 10 MB limit".to_string());
    }
    fs::read_to_string(&path)
        .map(Some)
        .map_err(|e| e.to_string())
}

pub(crate) fn delete_in(dir: &Path, id: &str) -> Result<(), String> {
    validate_id(id)?;
    match fs::remove_file(dir.join(id)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

pub(crate) fn list_in(dir: &Path) -> Result<Vec<ScratchEntry>, String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.to_string()),
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if !is_valid_id(&name) {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        out.push(ScratchEntry { id: name, mtime });
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(out)
}

#[tauri::command]
pub async fn scratch_write(
    id: String,
    content: String,
    app: tauri::AppHandle,
) -> Result<(), String> {
    write_in(&scratch_dir(&app)?, &id, &content)
}

#[tauri::command]
pub async fn scratch_read(id: String, app: tauri::AppHandle) -> Result<Option<String>, String> {
    read_in(&scratch_dir(&app)?, &id)
}

#[tauri::command]
pub async fn scratch_delete(id: String, app: tauri::AppHandle) -> Result<(), String> {
    delete_in(&scratch_dir(&app)?, &id)
}

#[tauri::command]
pub async fn scratch_list(app: tauri::AppHandle) -> Result<Vec<ScratchEntry>, String> {
    list_in(&scratch_dir(&app)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "0123456789abcdef0123456789abcdef";

    fn dir_names(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn rejects_ids_that_are_not_32_lowercase_hex() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("scratch");
        let bad = [
            "",
            "0123456789abcdef0123456789abcde",
            "0123456789abcdef0123456789abcdef0",
            "0123456789ABCDEF0123456789ABCDEF",
            "0123456789abcdef0123456789abcdeg",
            "../../../../../../../../etc/pass",
            "0123456789abcdef/123456789abcdef",
            "..",
            "/tmp/x",
        ];
        for id in bad {
            assert!(write_in(&dir, id, "x").is_err(), "write accepted {id:?}");
            assert!(read_in(&dir, id).is_err(), "read accepted {id:?}");
            assert!(delete_in(&dir, id).is_err(), "delete accepted {id:?}");
        }
        assert!(!dir.exists(), "a rejected id must not touch the disk");
    }

    #[test]
    fn round_trips_and_reports_missing_as_none() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("scratch");
        assert_eq!(read_in(&dir, ID).unwrap(), None);
        write_in(&dir, ID, "hello\nworld").unwrap();
        assert_eq!(read_in(&dir, ID).unwrap().as_deref(), Some("hello\nworld"));
        delete_in(&dir, ID).unwrap();
        delete_in(&dir, ID).unwrap();
        assert_eq!(read_in(&dir, ID).unwrap(), None);
    }

    #[test]
    fn overwrite_replaces_whole_file_and_leaves_no_temp_files() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("scratch");
        write_in(&dir, ID, &"long content ".repeat(100)).unwrap();
        write_in(&dir, ID, "short").unwrap();
        assert_eq!(read_in(&dir, ID).unwrap().as_deref(), Some("short"));
        assert_eq!(dir_names(&dir), vec![ID.to_string()]);
    }

    #[test]
    fn oversized_write_keeps_the_previous_content() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("scratch");
        write_in(&dir, ID, "kept").unwrap();
        let huge = "x".repeat(MAX_SCRATCH_BYTES + 1);
        assert!(write_in(&dir, ID, &huge).is_err());
        assert_eq!(read_in(&dir, ID).unwrap().as_deref(), Some("kept"));
        assert_eq!(dir_names(&dir), vec![ID.to_string()]);
    }

    // Windows refuses to replace a file while a reader holds it open.
    #[cfg(unix)]
    #[test]
    fn concurrent_writes_never_expose_a_torn_buffer() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("scratch");
        let len = 256 * 1024;
        write_in(&dir, ID, &"a".repeat(len)).unwrap();
        let writers: Vec<_> = ['a', 'b']
            .into_iter()
            .map(|c| {
                let dir = dir.clone();
                std::thread::spawn(move || {
                    let body = c.to_string().repeat(len);
                    for _ in 0..20 {
                        write_in(&dir, ID, &body).unwrap();
                    }
                })
            })
            .collect();
        for _ in 0..200 {
            let text = read_in(&dir, ID).unwrap().unwrap();
            assert_eq!(text.len(), len);
            let first = text.as_bytes()[0];
            assert!(text.bytes().all(|b| b == first), "torn read");
        }
        for w in writers {
            w.join().unwrap();
        }
        assert_eq!(dir_names(&dir), vec![ID.to_string()]);
    }

    #[test]
    fn list_skips_temp_and_foreign_entries() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("scratch");
        assert!(list_in(&dir).unwrap().is_empty());
        write_in(&dir, ID, "x").unwrap();
        fs::write(dir.join(".tmpXYZ"), "partial").unwrap();
        fs::write(dir.join("notes.txt"), "foreign").unwrap();
        fs::create_dir(dir.join("ffffffffffffffffffffffffffffffff")).unwrap();
        let listed = list_in(&dir).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, ID);
        assert!(listed[0].mtime > 0);
    }

    #[cfg(unix)]
    #[test]
    fn read_refuses_a_symlinked_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("scratch");
        fs::create_dir_all(&dir).unwrap();
        let secret = tmp.path().join("secret");
        fs::write(&secret, "token").unwrap();
        std::os::unix::fs::symlink(&secret, dir.join(ID)).unwrap();
        assert!(read_in(&dir, ID).is_err());
    }
}
