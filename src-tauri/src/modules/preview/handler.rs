use std::borrow::Cow;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use tauri::http::{header, Method, Request, Response, StatusCode};
use tauri::{AppHandle, Manager, UriSchemeContext, UriSchemeResponder, Wry};

use super::pure::{
    mime_for, page_csp, parse_range, resolve_within_root, sanitize_rel_path, split_preview_uri,
    RangeSpec, ServeError,
};
use super::PreviewState;
use crate::modules::fs::watch::{watch_preview_dirs, FsWatchState};
use crate::modules::workspace::WorkspaceRegistry;

// Media elements always send Range; this only bounds plain full-body loads.
const MAX_FULL_BODY: u64 = 64 * 1024 * 1024;
const MAX_RANGE_CHUNK: u64 = 4 * 1024 * 1024;

pub struct Served {
    pub response: Response<Vec<u8>>,
    pub served: Option<(String, PathBuf)>,
}

pub fn handle(
    ctx: UriSchemeContext<'_, Wry>,
    request: Request<Vec<u8>>,
    responder: UriSchemeResponder,
) {
    let app = ctx.app_handle().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let range = request
            .headers()
            .get(header::RANGE)
            .and_then(|v| v.to_str().ok());
        let state = app.state::<PreviewState>();
        let out = serve(&state, request.method(), &request.uri().to_string(), range);
        if let Some((token, dir)) = out.served {
            record_served_dir(&app, &token, dir);
        }
        responder.respond(out.response.map(Cow::<'static, [u8]>::Owned));
    });
}

fn record_served_dir(app: &AppHandle, token: &str, dir: PathBuf) {
    let previews = app.state::<PreviewState>();
    let watch = app.state::<FsWatchState>();
    let registry = app.state::<WorkspaceRegistry>();
    previews.record_served_dir(token, dir, |d| {
        !watch_preview_dirs(&watch, app, &registry, vec![d.to_path_buf()]).is_empty()
    });
}

pub fn serve(state: &PreviewState, method: &Method, uri: &str, range: Option<&str>) -> Served {
    let plain = |status| Served {
        response: text_response(status),
        served: None,
    };
    let Some((token, raw)) = split_preview_uri(uri) else {
        return plain(StatusCode::NOT_FOUND);
    };
    let Some(root) = state.touch(token) else {
        return plain(StatusCode::NOT_FOUND);
    };
    if method == Method::OPTIONS {
        return Served {
            response: with_common(Response::builder().status(StatusCode::NO_CONTENT), token)
                .header(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, HEAD, OPTIONS")
                .header(header::ACCESS_CONTROL_ALLOW_HEADERS, "*")
                .body(Vec::new())
                .unwrap_or_else(|_| fallback()),
            served: None,
        };
    }
    if method != Method::GET && method != Method::HEAD {
        return plain(StatusCode::METHOD_NOT_ALLOWED);
    }
    let path = match sanitize_rel_path(raw).and_then(|rel| resolve_within_root(&root, &rel)) {
        Ok(path) => path,
        Err(ServeError::BadRequest) => return plain(StatusCode::BAD_REQUEST),
        Err(ServeError::NotFound) => return plain(StatusCode::NOT_FOUND),
    };
    let response = match read_file(&path, range) {
        Ok(body) => body,
        Err(status) => return plain(status),
    };
    let mut builder = with_common(Response::builder().status(response.status), token)
        .header(header::CONTENT_TYPE, mime_for(&path))
        .header(header::ACCEPT_RANGES, "bytes");
    if let Some(content_range) = response.content_range {
        builder = builder.header(header::CONTENT_RANGE, content_range);
    }
    let length = response.body.len();
    let body = if method == Method::HEAD {
        Vec::new()
    } else {
        response.body
    };
    let response = builder
        .header(header::CONTENT_LENGTH, length)
        .body(body)
        .unwrap_or_else(|_| fallback());
    Served {
        response,
        served: path
            .parent()
            .map(|dir| (token.to_string(), dir.to_path_buf())),
    }
}

struct FileBody {
    status: StatusCode,
    content_range: Option<String>,
    body: Vec<u8>,
}

fn read_file(path: &Path, range: Option<&str>) -> Result<FileBody, StatusCode> {
    let mut file = File::open(path).map_err(|_| StatusCode::NOT_FOUND)?;
    let len = file.metadata().map_err(|_| StatusCode::NOT_FOUND)?.len();
    match parse_range(range, len) {
        RangeSpec::Full => {
            if len > MAX_FULL_BODY {
                return Err(StatusCode::PAYLOAD_TOO_LARGE);
            }
            let mut body = Vec::with_capacity(len as usize);
            file.take(len)
                .read_to_end(&mut body)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            Ok(FileBody {
                status: StatusCode::OK,
                content_range: None,
                body,
            })
        }
        RangeSpec::Partial { start, end } => {
            let end = end.min(start + MAX_RANGE_CHUNK - 1);
            let want = end - start + 1;
            file.seek(SeekFrom::Start(start))
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            let mut body = Vec::with_capacity(want as usize);
            file.take(want)
                .read_to_end(&mut body)
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            if body.is_empty() {
                return Err(StatusCode::RANGE_NOT_SATISFIABLE);
            }
            let end = start + body.len() as u64 - 1;
            Ok(FileBody {
                status: StatusCode::PARTIAL_CONTENT,
                content_range: Some(format!("bytes {start}-{end}/{len}")),
                body,
            })
        }
        RangeSpec::Unsatisfiable => Err(StatusCode::RANGE_NOT_SATISFIABLE),
    }
}

fn base_headers(builder: tauri::http::response::Builder) -> tauri::http::response::Builder {
    builder
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
}

fn with_common(
    builder: tauri::http::response::Builder,
    token: &str,
) -> tauri::http::response::Builder {
    base_headers(builder).header(header::CONTENT_SECURITY_POLICY, page_csp(token))
}

fn text_response(status: StatusCode) -> Response<Vec<u8>> {
    let body = status
        .canonical_reason()
        .unwrap_or("Error")
        .as_bytes()
        .to_vec();
    base_headers(Response::builder().status(status))
        .header(header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(header::CONTENT_LENGTH, body.len())
        .body(body)
        .unwrap_or_else(|_| fallback())
}

fn fallback() -> Response<Vec<u8>> {
    let mut response = Response::new(Vec::new());
    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Fixture {
        _dir: tempfile::TempDir,
        root: PathBuf,
        state: PreviewState,
        token: String,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = std::fs::canonicalize(dir.path()).expect("canon");
        std::fs::create_dir_all(root.join("css")).expect("mkdir");
        std::fs::write(root.join("index.html"), b"<h1>hi</h1>").expect("write");
        std::fs::write(root.join("css/site.css"), b"body{}").expect("write");
        std::fs::write(root.join("clip.mp4"), b"0123456789").expect("write");
        std::fs::write(root.join(".env"), b"K=V").expect("write");
        let state = PreviewState::default();
        let token = "0123456789abcdef0123456789abcdef".to_string();
        state.insert(token.clone(), root.clone(), root.join("index.html"));
        Fixture {
            _dir: dir,
            root,
            state,
            token,
        }
    }

    fn get(f: &Fixture, path: &str, range: Option<&str>) -> Served {
        let uri = format!("terax-preview://{}/{path}", f.token);
        serve(&f.state, &Method::GET, &uri, range)
    }

    fn header_of(r: &Response<Vec<u8>>, name: header::HeaderName) -> &str {
        r.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
    }

    #[test]
    fn serves_index_with_headers_and_records_dir() {
        let f = fixture();
        let out = get(&f, "", None);
        assert_eq!(out.response.status(), StatusCode::OK);
        assert_eq!(out.response.body(), b"<h1>hi</h1>");
        let r = &out.response;
        assert_eq!(
            header_of(r, header::CONTENT_TYPE),
            "text/html; charset=utf-8"
        );
        assert_eq!(header_of(r, header::X_CONTENT_TYPE_OPTIONS), "nosniff");
        assert_eq!(header_of(r, header::CACHE_CONTROL), "no-store");
        assert_eq!(header_of(r, header::ACCESS_CONTROL_ALLOW_ORIGIN), "*");
        assert!(header_of(r, header::CONTENT_SECURITY_POLICY)
            .contains(&format!("terax-preview://{}", f.token)));
        assert_eq!(out.served, Some((f.token.clone(), f.root.clone())));

        let css = get(&f, "css/site.css", None);
        assert_eq!(css.response.status(), StatusCode::OK);
        assert_eq!(
            header_of(&css.response, header::CONTENT_TYPE),
            "text/css; charset=utf-8"
        );
        assert_eq!(css.served, Some((f.token.clone(), f.root.join("css"))));
    }

    #[test]
    fn windows_uri_form_is_served() {
        let f = fixture();
        let uri = format!("http://terax-preview.localhost/{}/css/site.css", f.token);
        let out = serve(&f.state, &Method::GET, &uri, None);
        assert_eq!(out.response.status(), StatusCode::OK);
        assert_eq!(out.response.body(), b"body{}");
    }

    #[test]
    fn traversal_and_secrets_never_reach_disk() {
        let f = fixture();
        for path in ["../x", "%2e%2e/x", "..%2fx", "%2fetc/passwd", "a%00b"] {
            let out = get(&f, path, None);
            assert_eq!(out.response.status(), StatusCode::BAD_REQUEST, "{path}");
            assert!(out.served.is_none());
        }
        for path in [".env", ".ENV", "missing.html", "css"] {
            let out = get(&f, path, None);
            assert_eq!(out.response.status(), StatusCode::NOT_FOUND, "{path}");
            assert!(out.served.is_none());
        }
    }

    #[test]
    fn unknown_and_released_tokens_give_404() {
        let f = fixture();
        let uri = "terax-preview://ffffffffffffffffffffffffffffffff/index.html";
        assert_eq!(
            serve(&f.state, &Method::GET, uri, None).response.status(),
            StatusCode::NOT_FOUND
        );
        f.state.remove(&f.token);
        assert_eq!(
            get(&f, "index.html", None).response.status(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn single_range_and_416() {
        let f = fixture();
        let out = get(&f, "clip.mp4", Some("bytes=2-5"));
        assert_eq!(out.response.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(out.response.body(), b"2345");
        assert_eq!(
            header_of(&out.response, header::CONTENT_RANGE),
            "bytes 2-5/10"
        );
        assert_eq!(header_of(&out.response, header::CONTENT_LENGTH), "4");
        assert_eq!(header_of(&out.response, header::CONTENT_TYPE), "video/mp4");

        let tail = get(&f, "clip.mp4", Some("bytes=-3"));
        assert_eq!(tail.response.body(), b"789");

        let bad = get(&f, "clip.mp4", Some("bytes=10-"));
        assert_eq!(bad.response.status(), StatusCode::RANGE_NOT_SATISFIABLE);
    }

    #[test]
    fn head_and_method_handling() {
        let f = fixture();
        let uri = format!("terax-preview://{}/index.html", f.token);
        let head = serve(&f.state, &Method::HEAD, &uri, None);
        assert_eq!(head.response.status(), StatusCode::OK);
        assert!(head.response.body().is_empty());
        assert_eq!(header_of(&head.response, header::CONTENT_LENGTH), "11");
        assert_eq!(
            serve(&f.state, &Method::POST, &uri, None).response.status(),
            StatusCode::METHOD_NOT_ALLOWED
        );
        assert_eq!(
            serve(&f.state, &Method::OPTIONS, &uri, None)
                .response
                .status(),
            StatusCode::NO_CONTENT
        );
    }
}
