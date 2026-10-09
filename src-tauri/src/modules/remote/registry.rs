use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

use super::target::validate_alias;

/// Lexically normalizes an absolute POSIX path: collapses `.`, `..` and `//`.
/// Rejects relative paths and NUL. `..` at the root stays at the root.
/// This does not resolve symlinks; see `RemoteRegistry`.
pub fn normalize_posix(path: &str) -> Result<String, String> {
    if path.contains('\0') {
        return Err("remote path contains NUL".into());
    }
    if !path.starts_with('/') {
        return Err("remote path must be absolute".into());
    }
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    Ok(format!("/{}", parts.join("/")))
}

fn is_under(root: &str, path: &str) -> bool {
    root == "/"
        || path == root
        || path
            .strip_prefix(root)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Per-host set of authorized remote roots.
///
/// Both `add_root` and `is_authorized` take a path that the caller has
/// ALREADY resolved on the server (SFTP realpath of the object, or of its
/// parent when the object does not exist yet). Lexical normalization alone
/// cannot defend against a symlink such as `root/link -> /etc`, so passing an
/// unresolved path to `is_authorized` for a mutation is a bug in the caller.
///
/// Roots must be added only by explicit user actions (connect, open folder,
/// cwd of an SSH tab), never by AI tools.
#[derive(Default)]
pub struct RemoteRegistry {
    roots: Mutex<HashMap<String, HashSet<String>>>,
}

impl RemoteRegistry {
    pub fn add_root(&self, host: &str, resolved_path: &str) -> Result<String, String> {
        validate_alias(host)?;
        let root = normalize_posix(resolved_path)?;
        let mut map = self.roots.lock().map_err(|e| e.to_string())?;
        map.entry(host.to_string())
            .or_default()
            .insert(root.clone());
        Ok(root)
    }

    pub fn is_authorized(&self, host: &str, resolved_path: &str) -> bool {
        let Ok(path) = normalize_posix(resolved_path) else {
            return false;
        };
        let Ok(map) = self.roots.lock() else {
            return false;
        };
        map.get(host)
            .is_some_and(|roots| roots.iter().any(|r| is_under(r, &path)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes() {
        assert_eq!(normalize_posix("/").unwrap(), "/");
        assert_eq!(normalize_posix("//a/./b//c/").unwrap(), "/a/b/c");
        assert_eq!(normalize_posix("/a/b/../c").unwrap(), "/a/c");
        assert_eq!(normalize_posix("/../..").unwrap(), "/");
    }

    #[test]
    fn rejects_relative_and_nul() {
        assert!(normalize_posix("a/b").is_err());
        assert!(normalize_posix("").is_err());
        assert!(normalize_posix("/a\0b").is_err());
    }

    #[test]
    fn authorizes_within_root() {
        let r = RemoteRegistry::default();
        r.add_root("h", "/srv/app/").unwrap();
        assert!(r.is_authorized("h", "/srv/app"));
        assert!(r.is_authorized("h", "/srv/app/src/x"));
        assert!(!r.is_authorized("h", "/srv/application"));
        assert!(!r.is_authorized("h", "/srv"));
        assert!(!r.is_authorized("other", "/srv/app"));
    }

    #[test]
    fn dotdot_cannot_escape() {
        let r = RemoteRegistry::default();
        r.add_root("h", "/srv/app").unwrap();
        assert!(!r.is_authorized("h", "/srv/app/../secret"));
        assert!(!r.is_authorized("h", "/srv/app/../../etc/passwd"));
        assert!(r.is_authorized("h", "/srv/app/a/../b"));
    }

    #[test]
    fn invalid_input_is_denied() {
        let r = RemoteRegistry::default();
        r.add_root("h", "/").unwrap();
        assert!(r.is_authorized("h", "/etc"));
        assert!(!r.is_authorized("h", "etc"));
        assert!(!r.is_authorized("h", "/a\0"));
        assert!(r.add_root("-bad", "/x").is_err());
        assert!(r.add_root("h", "rel").is_err());
    }
}
