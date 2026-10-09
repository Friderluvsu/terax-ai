use std::path::{Path, PathBuf};

use serde::Serialize;

use super::workspace::WorkspaceRegistry;

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScriptLaunchInfo {
    pub executable: bool,
    pub quarantined: bool,
}

fn regular_file(path: &str) -> Result<PathBuf, String> {
    let canonical =
        std::fs::canonicalize(path).map_err(|e| format!("script not accessible: {e}"))?;
    if !canonical.is_file() {
        return Err(format!("not a regular file: {}", canonical.display()));
    }
    Ok(canonical)
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable(_path: &Path) -> bool {
    true
}

#[cfg(target_os = "macos")]
fn is_quarantined(path: &Path) -> bool {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    let name = c"com.apple.quarantine";
    let size = unsafe {
        libc::getxattr(
            c_path.as_ptr(),
            name.as_ptr(),
            std::ptr::null_mut(),
            0,
            0,
            0,
        )
    };
    size >= 0
}

#[cfg(not(target_os = "macos"))]
fn is_quarantined(_path: &Path) -> bool {
    false
}

fn launch_info(path: &Path) -> ScriptLaunchInfo {
    ScriptLaunchInfo {
        executable: is_executable(path),
        quarantined: is_quarantined(path),
    }
}

#[cfg(unix)]
fn add_exec_where_readable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)
        .map_err(|e| format!("stat failed: {e}"))?
        .permissions();
    let mode = perms.mode();
    perms.set_mode(mode | ((mode & 0o444) >> 2));
    std::fs::set_permissions(path, perms).map_err(|e| format!("chmod failed: {e}"))
}

#[cfg(not(unix))]
fn add_exec_where_readable(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn make_executable(path: &Path, registry: &WorkspaceRegistry) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "script has no parent directory".to_string())?;
    if !registry.is_authorized(parent) {
        return Err(format!(
            "directory is outside the authorized workspace: {}",
            parent.display()
        ));
    }
    add_exec_where_readable(path)
}

#[tauri::command]
pub fn script_launch_info(path: String) -> Result<ScriptLaunchInfo, String> {
    Ok(launch_info(&regular_file(&path)?))
}

#[tauri::command]
pub fn script_make_executable(
    path: String,
    registry: tauri::State<'_, WorkspaceRegistry>,
) -> Result<(), String> {
    make_executable(&regular_file(&path)?, &registry)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn script(dir: &Path, mode: u32) -> PathBuf {
        let p = dir.join("run.command");
        std::fs::write(&p, "#!/bin/sh\necho hi\n").unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
        std::fs::canonicalize(p).unwrap()
    }

    fn registry_for(dir: &Path) -> WorkspaceRegistry {
        let registry = WorkspaceRegistry::default();
        registry.authorize(dir).unwrap();
        registry
    }

    #[test]
    fn executable_follows_mode_bits() {
        let dir = tempfile::tempdir().unwrap();
        let p = script(dir.path(), 0o644);
        assert!(!launch_info(&p).executable);
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o744)).unwrap();
        assert!(launch_info(&p).executable);
    }

    #[test]
    fn regular_file_rejects_directory_and_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(regular_file(dir.path().to_str().unwrap()).is_err());
        assert!(regular_file(dir.path().join("nope").to_str().unwrap()).is_err());
    }

    #[test]
    fn make_executable_adds_x_where_r() {
        let dir = tempfile::tempdir().unwrap();
        let p = script(dir.path(), 0o640);
        make_executable(&p, &registry_for(dir.path())).unwrap();
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o750);
    }

    #[test]
    fn make_executable_refuses_unauthorized_dir() {
        let dir = tempfile::tempdir().unwrap();
        let p = script(dir.path(), 0o644);
        let registry = WorkspaceRegistry::default();
        assert!(make_executable(&p, &registry).is_err());
        let mode = std::fs::metadata(&p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o644);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn quarantine_detected_via_xattr() {
        let dir = tempfile::tempdir().unwrap();
        let p = script(dir.path(), 0o755);
        assert!(!launch_info(&p).quarantined);
        let status = std::process::Command::new("xattr")
            .args(["-w", "com.apple.quarantine", "0081;00000000;Test;"])
            .arg(&p)
            .status()
            .unwrap();
        assert!(status.success());
        assert!(launch_info(&p).quarantined);
    }
}
