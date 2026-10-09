//! Deny-list of secret file locations. Port of `src/modules/ai/lib/security.ts`
//! (`checkReadable`); both sides are tested against
//! `src-tauri/tests/fixtures/secret_paths.json` so the lists cannot drift.
//!
//! Protected directories are matched as a path-segment substring anywhere in
//! the path, not only as a prefix. Segments such as `etc/`, `sys/`, `proc/` or
//! `.git/` inside a project are therefore denied too. This is intended for the
//! AI tools; for the preview protocol it means a 404 on such paths.
//!
//! Matching is always case-insensitive, as in the TS version.

use std::path::Path;

const PROTECTED_DIRS: &[&str] = &[
    "/.ssh",
    "/.gnupg",
    "/.aws",
    "/.azure",
    "/.kube",
    "/.docker",
    "/.config/gh",
    "/.config/git",
    "/.config/gcloud",
    "/.config/op",
    "/.git",
    "/.terraform.d",
    "/library/keychains",
    "/library/cookies",
    "/etc",
    "/private/etc",
    "/proc",
    "/sys",
    "/var/db",
    "/var/root",
    "/private/var/db",
    "/private/var/root",
    "/appdata/roaming/microsoft/credentials",
    "/appdata/local/microsoft/credentials",
    "/appdata/roaming/gcloud",
];

/// Local filesystem path (Windows-aware normalization, case-insensitive).
pub fn is_secret_path(path: &Path) -> bool {
    check(&path.to_string_lossy(), true)
}

/// Remote POSIX path: `\` is an ordinary character, no drive or stream rules.
pub fn is_secret_posix_path(path: &str) -> bool {
    check(path, false)
}

fn check(path: &str, windows: bool) -> bool {
    if path.is_empty() || path.chars().any(|c| (c as u32) < 0x20) {
        return true;
    }
    let base = if windows {
        path.rsplit(['/', '\\']).next().unwrap_or(path)
    } else {
        path.rsplit('/').next().unwrap_or(path)
    };
    if secret_basename(&base.to_lowercase()) {
        return true;
    }
    let with_slash = format!("{}/", comparison_form(path, windows));
    PROTECTED_DIRS
        .iter()
        .any(|dir| with_slash.contains(&format!("{dir}/")))
}

fn comparison_form(path: &str, windows: bool) -> String {
    let mut s = path.to_string();
    if windows {
        s = s.replace('\\', "/");
        if let Some(rest) = s.strip_prefix("//?/") {
            s = format!("/{rest}");
        }
        let b = s.as_bytes();
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            s = s[2..].to_string();
        }
        s = s
            .split('/')
            .map(|seg| {
                let seg = seg.split(':').next().unwrap_or("");
                seg.trim_end_matches(|c: char| c == '.' || c.is_whitespace())
            })
            .collect::<Vec<_>>()
            .join("/");
    }
    let mut out = String::with_capacity(s.len());
    let mut prev_slash = false;
    for c in s.to_lowercase().chars() {
        if c == '/' {
            if prev_slash {
                continue;
            }
            prev_slash = true;
        } else {
            prev_slash = false;
        }
        out.push(c);
    }
    if out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    out
}

/// Mirrors `(?:[.\s:]|$)` from the TS patterns.
fn tail_ok(rest: &str) -> bool {
    match rest.chars().next() {
        None => true,
        Some(c) => c == '.' || c == ':' || c.is_whitespace(),
    }
}

fn prefix_then_tail(base: &str, prefix: &str) -> bool {
    base.strip_prefix(prefix).is_some_and(tail_ok)
}

fn contains_ext(base: &str, ext: &str) -> bool {
    base.match_indices(ext)
        .any(|(i, _)| tail_ok(&base[i + ext.len()..]))
}

fn secret_basename(b: &str) -> bool {
    if prefix_then_tail(b, ".env") {
        return true;
    }
    for ext in [
        ".pem",
        ".key",
        ".p12",
        ".pfx",
        ".asc",
        ".gpg",
        ".keystore",
        ".jks",
    ] {
        if contains_ext(b, ext) {
            return true;
        }
    }
    for alg in ["rsa", "dsa", "ecdsa", "ed25519"] {
        if let Some(rest) = b.strip_prefix("id_").and_then(|r| r.strip_prefix(alg)) {
            let ok = match rest.chars().next() {
                None => true,
                Some(c) => matches!(c, '.' | '_' | '-' | ':') || c.is_whitespace(),
            };
            if ok {
                return true;
            }
        }
    }
    for name in [
        "known_hosts",
        "authorized_keys",
        "htpasswd",
        ".netrc",
        "_netrc",
        "credentials",
        ".pgpass",
        ".npmrc",
        ".pypirc",
    ] {
        if prefix_then_tail(b, name) {
            return true;
        }
    }
    for stem in ["secret", "secrets"] {
        if let Some(rest) = b.strip_prefix(stem).and_then(|r| r.strip_prefix('.')) {
            for ext in ["json", "yaml", "yml", "toml", "env"] {
                if rest.strip_prefix(ext).is_some_and(tail_ok) {
                    return true;
                }
            }
        }
    }
    if let Some(rest) = b.strip_prefix("service") {
        let rest = rest.strip_prefix(['-', '_']).unwrap_or(rest);
        if let Some(rest) = rest.strip_prefix("account") {
            if contains_ext(rest, ".json") {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Case {
        path: String,
        secret: bool,
    }

    #[derive(Deserialize)]
    struct Fixture {
        cases: Vec<Case>,
        local_only: Vec<Case>,
        protected_dirs: Vec<String>,
    }

    fn fixture() -> Fixture {
        let raw = include_str!("../../../tests/fixtures/secret_paths.json");
        serde_json::from_str(raw).unwrap()
    }

    #[test]
    fn shared_cases_match_for_local_and_posix() {
        for c in fixture().cases {
            assert_eq!(
                is_secret_path(Path::new(&c.path)),
                c.secret,
                "local {}",
                c.path
            );
            assert_eq!(is_secret_posix_path(&c.path), c.secret, "posix {}", c.path);
        }
    }

    #[test]
    fn protected_dirs_equal_fixture() {
        assert_eq!(PROTECTED_DIRS.to_vec(), fixture().protected_dirs);
    }

    #[test]
    fn local_only_cases() {
        for c in fixture().local_only {
            assert_eq!(
                is_secret_path(Path::new(&c.path)),
                c.secret,
                "local {}",
                c.path
            );
        }
    }
}
