use std::path::{Component, Path, PathBuf};

use crate::modules::security::secret_paths::is_secret_path;

pub const SCHEME: &str = "terax-preview";
pub const WINDOWS_HOST: &str = "terax-preview.localhost";
pub const TOKEN_LEN: usize = 32;
const INDEX: &str = "index.html";

#[derive(Debug, PartialEq, Eq)]
pub enum ServeError {
    BadRequest,
    NotFound,
}

#[derive(Debug, PartialEq, Eq)]
pub enum RangeSpec {
    Full,
    Partial { start: u64, end: u64 },
    Unsatisfiable,
}

pub fn is_token(s: &str) -> bool {
    s.len() == TOKEN_LEN && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &s[prefix.len()..])
}

/// `terax-preview://<token>/<path>` or `http(s)://terax-preview.localhost/<token>/<path>`.
/// Returns the token and the still percent-encoded path after it.
pub fn split_preview_uri(uri: &str) -> Option<(&str, &str)> {
    let uri = &uri[..uri.find(['?', '#']).unwrap_or(uri.len())];
    let rest = if let Some(rest) = strip_prefix_ci(uri, &format!("{SCHEME}://")) {
        rest
    } else {
        let rest = strip_prefix_ci(uri, "http://").or_else(|| strip_prefix_ci(uri, "https://"))?;
        let rest = strip_prefix_ci(rest, WINDOWS_HOST)?;
        if rest.is_empty() {
            return None;
        }
        rest.strip_prefix('/')?
    };
    let (token, path) = rest.split_once('/').unwrap_or((rest, ""));
    is_token(token).then_some((token, path))
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(s: &str) -> Option<Vec<u8>> {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hi = hex_val(*bytes.get(i + 1)?)?;
            let lo = hex_val(*bytes.get(i + 2)?)?;
            out.push((hi << 4) | lo);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    Some(out)
}

/// Decodes the URL path into a relative path made only of normal segments.
/// Empty paths and directory paths map to `index.html`.
pub fn sanitize_rel_path(raw: &str) -> Result<PathBuf, ServeError> {
    let decoded = percent_decode(raw).ok_or(ServeError::BadRequest)?;
    let decoded = String::from_utf8(decoded).map_err(|_| ServeError::BadRequest)?;
    if decoded.starts_with('/') || decoded.contains('\\') || decoded.chars().any(char::is_control) {
        return Err(ServeError::BadRequest);
    }
    // A second encoding layer is never legitimate here and hides `..` or separators.
    let lower = decoded.to_ascii_lowercase();
    if ["%2e", "%2f", "%5c", "%00"]
        .iter()
        .any(|p| lower.contains(p))
    {
        return Err(ServeError::BadRequest);
    }
    let mut out = PathBuf::new();
    for seg in decoded.split('/') {
        match seg {
            "" | "." => {}
            _ if seg.contains(':') => return Err(ServeError::BadRequest),
            // Windows trims trailing dots and spaces, so `...` or `.. ` can act as `..`.
            _ if seg.trim_end_matches(['.', ' ']).is_empty() => return Err(ServeError::BadRequest),
            _ => out.push(seg),
        }
    }
    if out.as_os_str().is_empty() || decoded.ends_with('/') {
        out.push(INDEX);
    }
    Ok(out)
}

/// `root` must be canonical. Both the lexical and the canonical path pass the
/// deny-list; the returned canonical path is the one to open.
pub fn resolve_within_root(root: &Path, rel: &Path) -> Result<PathBuf, ServeError> {
    if rel.components().any(|c| !matches!(c, Component::Normal(_))) {
        return Err(ServeError::BadRequest);
    }
    let lexical = root.join(rel);
    if is_secret_path(&lexical) {
        return Err(ServeError::NotFound);
    }
    let canonical = std::fs::canonicalize(&lexical).map_err(|_| ServeError::NotFound)?;
    if !canonical.starts_with(root) || is_secret_path(&canonical) || !canonical.is_file() {
        return Err(ServeError::NotFound);
    }
    Ok(canonical)
}

/// Nearest ancestor of `file` that holds a project marker, bounded by `hint`.
/// Never `$HOME`, a filesystem root, or an ancestor of `$HOME`: those are
/// authorized by prefix, so such a root would expose the whole home directory.
pub fn choose_root(
    file: &Path,
    hint: Option<&Path>,
    home: Option<&Path>,
    is_project: impl Fn(&Path) -> bool,
) -> Option<PathBuf> {
    let start = file.parent()?;
    let forbidden = |dir: &Path| dir.parent().is_none() || home.is_some_and(|h| h.starts_with(dir));
    for dir in start.ancestors() {
        if forbidden(dir) || hint.is_some_and(|h| !dir.starts_with(h)) {
            break;
        }
        if is_project(dir) {
            return Some(dir.to_path_buf());
        }
    }
    (!forbidden(start)).then(|| start.to_path_buf())
}

fn encode_segment(seg: &str, out: &mut String) {
    for b in seg.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
}

/// Percent-encodes each segment of a relative path, joined with `/`.
pub fn encode_rel_path(rel: &Path) -> Option<String> {
    let mut out = String::new();
    for (i, comp) in rel.components().enumerate() {
        let Component::Normal(seg) = comp else {
            return None;
        };
        if i > 0 {
            out.push('/');
        }
        encode_segment(seg.to_str()?, &mut out);
    }
    Some(out)
}

pub fn preview_url(token: &str, encoded_rel: &str) -> String {
    if cfg!(windows) {
        format!("http://{WINDOWS_HOST}/{token}/{encoded_rel}")
    } else {
        format!("{SCHEME}://{token}/{encoded_rel}")
    }
}

/// The token is the host on macOS and Linux, so `'self'` alone is not relied
/// on: WebKit's matching of `'self'` for an opaque-origin sandboxed document on
/// a custom scheme is unverified.
pub fn page_csp(token: &str) -> String {
    let own = format!("{SCHEME}://{token} http://{WINDOWS_HOST}");
    format!(
        "default-src {own} 'self' 'unsafe-inline' 'unsafe-eval' data: blob: https:; \
         connect-src {own} 'self' data: blob: https:"
    )
}

/// One `bytes=` range. Malformed or multi-range headers are ignored (full body).
pub fn parse_range(header: Option<&str>, len: u64) -> RangeSpec {
    let Some(spec) = header.and_then(|h| strip_prefix_ci(h.trim(), "bytes=")) else {
        return RangeSpec::Full;
    };
    if spec.contains(',') {
        return RangeSpec::Full;
    }
    let Some((a, b)) = spec.split_once('-') else {
        return RangeSpec::Full;
    };
    let (a, b) = (a.trim(), b.trim());
    if a.is_empty() {
        let Ok(n) = b.parse::<u64>() else {
            return RangeSpec::Full;
        };
        if n == 0 || len == 0 {
            return RangeSpec::Unsatisfiable;
        }
        return RangeSpec::Partial {
            start: len - n.min(len),
            end: len - 1,
        };
    }
    let Ok(start) = a.parse::<u64>() else {
        return RangeSpec::Full;
    };
    let end = if b.is_empty() {
        u64::MAX
    } else {
        match b.parse::<u64>() {
            Ok(end) if end >= start => end,
            _ => return RangeSpec::Full,
        }
    };
    if start >= len {
        return RangeSpec::Unsatisfiable;
    }
    RangeSpec::Partial {
        start,
        end: end.min(len - 1),
    }
}

pub fn mime_for(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "xhtml" => "application/xhtml+xml; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" | "cjs" => "text/javascript; charset=utf-8",
        "json" | "map" => "application/json; charset=utf-8",
        "webmanifest" => "application/manifest+json; charset=utf-8",
        "txt" | "md" => "text/plain; charset=utf-8",
        "csv" => "text/csv; charset=utf-8",
        "xml" => "application/xml; charset=utf-8",
        "svg" => "image/svg+xml; charset=utf-8",
        "wasm" => "application/wasm",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "ogv" => "video/ogg",
        "mov" => "video/quicktime",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" | "oga" | "opus" => "audio/ogg",
        "flac" => "audio/flac",
        "aac" => "audio/aac",
        "pdf" => "application/pdf",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOK: &str = "0123456789abcdef0123456789abcdef";

    #[test]
    fn splits_both_uri_forms() {
        let mac = format!("terax-preview://{TOK}/a/b.css?x=1#y");
        assert_eq!(split_preview_uri(&mac), Some((TOK, "a/b.css")));
        let win = format!("http://terax-preview.localhost/{TOK}/a/b.css");
        assert_eq!(split_preview_uri(&win), Some((TOK, "a/b.css")));
        let win_https = format!("https://terax-preview.localhost/{TOK}");
        assert_eq!(split_preview_uri(&win_https), Some((TOK, "")));
        assert_eq!(
            split_preview_uri(&format!("terax-preview://{TOK}")),
            Some((TOK, ""))
        );
        assert_eq!(
            split_preview_uri(&format!("terax-preview://{TOK}/")),
            Some((TOK, ""))
        );
    }

    #[test]
    fn split_rejects_bad_tokens_and_hosts() {
        assert_eq!(split_preview_uri("terax-preview://short/a"), None);
        assert_eq!(
            split_preview_uri(&format!("terax-preview://{}/a", TOK.to_uppercase())),
            None
        );
        assert_eq!(
            split_preview_uri(&format!("http://evil.localhost/{TOK}/a")),
            None
        );
        assert_eq!(
            split_preview_uri(&format!("http://terax-preview.localhost.evil/{TOK}/a")),
            None
        );
        assert_eq!(split_preview_uri(&format!("asset://localhost/{TOK}")), None);
    }

    #[test]
    fn sanitize_rejects_traversal_in_every_encoding() {
        for raw in [
            "..",
            "../x",
            "a/../../x",
            "%2e%2e/x",
            "%2E%2E/x",
            ".%2e/x",
            "..%2fx",
            "..%2Fx",
            "a/%2e%2e%2f%2e%2e/x",
            "%252e%252e/x",
            "...",
            ".. /x",
        ] {
            assert_eq!(sanitize_rel_path(raw), Err(ServeError::BadRequest), "{raw}");
        }
    }

    #[test]
    fn sanitize_rejects_absolute_nul_and_backslash() {
        for raw in [
            "/etc/passwd",
            "%2fetc/passwd",
            "a%00b",
            "a\0b",
            "a\\b",
            "a%5cb",
            "C:/x",
            "C%3A/x",
            "%zz",
            "%e2%28",
        ] {
            assert_eq!(sanitize_rel_path(raw), Err(ServeError::BadRequest), "{raw}");
        }
    }

    #[test]
    fn sanitize_maps_directories_to_index() {
        assert_eq!(sanitize_rel_path(""), Ok(PathBuf::from("index.html")));
        assert_eq!(sanitize_rel_path("."), Ok(PathBuf::from("index.html")));
        assert_eq!(
            sanitize_rel_path("docs/"),
            Ok(PathBuf::from("docs/index.html"))
        );
        assert_eq!(
            sanitize_rel_path("a//./b%20c.css"),
            Ok(PathBuf::from("a/b c.css"))
        );
        assert_eq!(
            sanitize_rel_path("%D0%BF%D1%80%D0%B8%D0%B2%D0%B5%D1%82.html"),
            Ok(PathBuf::from("привет.html"))
        );
    }

    #[test]
    fn encode_round_trips_through_sanitize() {
        let rel = Path::new("a b/#x?/привет.html");
        let encoded = encode_rel_path(rel).expect("utf-8 path");
        assert!(!encoded.contains(['#', '?', ' ']));
        assert_eq!(sanitize_rel_path(&encoded), Ok(rel.to_path_buf()));
    }

    #[cfg(unix)]
    #[test]
    fn preview_url_uses_token_host() {
        assert_eq!(
            preview_url(TOK, "a/index.html"),
            format!("terax-preview://{TOK}/a/index.html")
        );
    }

    #[test]
    fn csp_names_the_token_origin_explicitly() {
        let csp = page_csp(TOK);
        assert!(csp.contains(&format!("default-src terax-preview://{TOK} ")));
        assert!(csp.contains(&format!("connect-src terax-preview://{TOK} ")));
        assert!(!csp.contains("http:;") && !csp.contains("http: "));
    }

    #[test]
    fn mime_table_has_octet_stream_fallback() {
        let cases = [
            ("a.html", "text/html; charset=utf-8"),
            ("a.HTM", "text/html; charset=utf-8"),
            ("a.js", "text/javascript; charset=utf-8"),
            ("a.mjs", "text/javascript; charset=utf-8"),
            ("a.css", "text/css; charset=utf-8"),
            ("a.svg", "image/svg+xml; charset=utf-8"),
            ("a.wasm", "application/wasm"),
            ("a.json", "application/json; charset=utf-8"),
            ("a.mp4", "video/mp4"),
            ("a.woff2", "font/woff2"),
            ("a.unknownext", "application/octet-stream"),
            ("Makefile", "application/octet-stream"),
        ];
        for (name, mime) in cases {
            assert_eq!(mime_for(Path::new(name)), mime, "{name}");
        }
    }

    #[test]
    fn parses_single_ranges() {
        assert_eq!(parse_range(None, 10), RangeSpec::Full);
        assert_eq!(
            parse_range(Some("bytes=0-3"), 10),
            RangeSpec::Partial { start: 0, end: 3 }
        );
        assert_eq!(
            parse_range(Some("bytes=4-"), 10),
            RangeSpec::Partial { start: 4, end: 9 }
        );
        assert_eq!(
            parse_range(Some("bytes=-3"), 10),
            RangeSpec::Partial { start: 7, end: 9 }
        );
        assert_eq!(
            parse_range(Some("bytes=-30"), 10),
            RangeSpec::Partial { start: 0, end: 9 }
        );
        assert_eq!(
            parse_range(Some("bytes=5-100"), 10),
            RangeSpec::Partial { start: 5, end: 9 }
        );
        assert_eq!(parse_range(Some("bytes=0-1,4-5"), 10), RangeSpec::Full);
        assert_eq!(parse_range(Some("bytes=5-2"), 10), RangeSpec::Full);
        assert_eq!(parse_range(Some("items=0-1"), 10), RangeSpec::Full);
        assert_eq!(parse_range(Some("bytes=x-1"), 10), RangeSpec::Full);
    }

    #[test]
    fn unsatisfiable_ranges() {
        assert_eq!(parse_range(Some("bytes=10-"), 10), RangeSpec::Unsatisfiable);
        assert_eq!(
            parse_range(Some("bytes=20-30"), 10),
            RangeSpec::Unsatisfiable
        );
        assert_eq!(parse_range(Some("bytes=-0"), 10), RangeSpec::Unsatisfiable);
        assert_eq!(parse_range(Some("bytes=0-"), 0), RangeSpec::Unsatisfiable);
    }

    fn mk<'a>(paths: &'a [&'a Path]) -> impl Fn(&Path) -> bool + 'a {
        move |d: &Path| paths.contains(&d)
    }

    #[test]
    fn root_is_nearest_project_ancestor() {
        let home = Path::new("/home/u");
        let file = Path::new("/home/u/proj/site/pages/a.html");
        let project = Path::new("/home/u/proj");
        assert_eq!(
            choose_root(file, None, Some(home), mk(&[project])),
            Some(project.to_path_buf())
        );
        let nested = Path::new("/home/u/proj/site");
        assert_eq!(
            choose_root(file, None, Some(home), mk(&[project, nested])),
            Some(nested.to_path_buf())
        );
    }

    #[test]
    fn root_falls_back_to_file_dir() {
        let home = Path::new("/home/u");
        let file = Path::new("/home/u/notes/a.html");
        assert_eq!(
            choose_root(file, None, Some(home), mk(&[])),
            Some(PathBuf::from("/home/u/notes"))
        );
    }

    #[test]
    fn root_never_home_or_above() {
        let home = Path::new("/home/u");
        assert_eq!(
            choose_root(Path::new("/home/u/a.html"), None, Some(home), mk(&[home])),
            None
        );
        assert_eq!(
            choose_root(Path::new("/home/a.html"), None, Some(home), mk(&[])),
            None
        );
        assert_eq!(choose_root(Path::new("/a.html"), None, None, mk(&[])), None);
        let file = Path::new("/home/u/notes/a.html");
        assert_eq!(
            choose_root(file, None, Some(home), mk(&[home, Path::new("/")])),
            Some(PathBuf::from("/home/u/notes"))
        );
    }

    #[test]
    fn root_does_not_climb_above_hint() {
        let home = Path::new("/home/u");
        let file = Path::new("/home/u/proj/site/a.html");
        let hint = Path::new("/home/u/proj/site");
        assert_eq!(
            choose_root(
                file,
                Some(hint),
                Some(home),
                mk(&[Path::new("/home/u/proj")])
            ),
            Some(hint.to_path_buf())
        );
    }

    #[test]
    fn resolve_denies_secrets_case_insensitively() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = std::fs::canonicalize(dir.path()).expect("canon");
        std::fs::create_dir(root.join(".ssh")).expect("mkdir");
        for name in [".env", ".ENV", "id_rsa", "ID_RSA", ".ssh/x", "ok.html"] {
            std::fs::write(root.join(name), b"x").ok();
        }
        assert!(resolve_within_root(&root, Path::new("ok.html")).is_ok());
        for name in [".env", ".ENV", "id_rsa", "ID_RSA", ".ssh/x", ".SSH/x"] {
            assert_eq!(
                resolve_within_root(&root, Path::new(name)),
                Err(ServeError::NotFound),
                "{name}"
            );
        }
    }

    #[test]
    fn resolve_rejects_missing_and_directories() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = std::fs::canonicalize(dir.path()).expect("canon");
        std::fs::create_dir(root.join("sub")).expect("mkdir");
        assert_eq!(
            resolve_within_root(&root, Path::new("missing.html")),
            Err(ServeError::NotFound)
        );
        assert_eq!(
            resolve_within_root(&root, Path::new("sub")),
            Err(ServeError::NotFound)
        );
        assert_eq!(
            resolve_within_root(&root, Path::new("../x")),
            Err(ServeError::BadRequest)
        );
    }

    #[cfg(unix)]
    #[test]
    fn resolve_refuses_symlink_escaping_root() {
        let outside = tempfile::tempdir().expect("outside");
        let secret = outside.path().join("data.txt");
        std::fs::write(&secret, b"secret").expect("write");
        let dir = tempfile::tempdir().expect("tempdir");
        let root = std::fs::canonicalize(dir.path()).expect("canon");
        std::os::unix::fs::symlink(&secret, root.join("link.txt")).expect("symlink");
        std::os::unix::fs::symlink(outside.path(), root.join("linkdir")).expect("symlink");
        assert_eq!(
            resolve_within_root(&root, Path::new("link.txt")),
            Err(ServeError::NotFound)
        );
        assert_eq!(
            resolve_within_root(&root, Path::new("linkdir/data.txt")),
            Err(ServeError::NotFound)
        );
    }

    #[cfg(unix)]
    #[test]
    fn resolve_refuses_symlink_to_secret_inside_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = std::fs::canonicalize(dir.path()).expect("canon");
        std::fs::write(root.join(".env"), b"K=V").expect("write");
        std::os::unix::fs::symlink(root.join(".env"), root.join("innocent.txt")).expect("symlink");
        assert_eq!(
            resolve_within_root(&root, Path::new("innocent.txt")),
            Err(ServeError::NotFound)
        );
    }
}
