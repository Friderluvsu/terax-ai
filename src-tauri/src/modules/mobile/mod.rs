//! Phone access: a loopback-only HTTP server that serves a small web client
//! and proxies a fixed set of terminal methods to the main window. Remote
//! reach comes from a cloudflared quick tunnel; every API call needs the
//! per-session bearer token, which travels in the link's URL fragment.

mod http;
mod tunnel;

use std::collections::HashMap;
use std::io::BufReader;
use std::net::{TcpListener, TcpStream};
use std::process::Child;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Emitter;

use crate::modules::control::{constant_time_eq, generate_token};

const REQUEST_EVENT: &str = "terax:mobile-request";
const STATUS_EVENT: &str = "terax:mobile-status";
const METHODS: &[&str] = &["list", "screen", "send", "key"];
const FRONTEND_TIMEOUT: Duration = Duration::from_secs(5);
const IO_TIMEOUT: Duration = Duration::from_secs(10);
const AUTH_FAILURE_DELAY: Duration = Duration::from_millis(300);
const MAX_CONNECTIONS: usize = 16;
const MAX_PENDING: usize = 32;

const INDEX_HTML: &str = include_str!("page/index.html");
const APP_JS: &str = include_str!("page/app.js");
const APP_CSS: &str = include_str!("page/app.css");

#[derive(Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum TunnelState {
    #[default]
    Off,
    Starting,
    Ready,
    Missing,
    Failed,
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelInfo {
    state: TunnelState,
    public_url: Option<String>,
    link: Option<String>,
    qr_svg: Option<String>,
    error: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MobileStatus {
    running: bool,
    tunnel: TunnelInfo,
    last_client_ms: Option<u64>,
}

#[derive(Deserialize)]
pub struct FrontendReply {
    ok: bool,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    error: Option<String>,
}

struct Server {
    generation: u64,
    stop: Arc<AtomicBool>,
    port: u16,
    last_client_ms: Arc<AtomicU64>,
    tunnel: TunnelInfo,
    child: Option<Child>,
}

#[derive(Default)]
struct Inner {
    server: Mutex<Option<Server>>,
    pending: Mutex<HashMap<u64, SyncSender<FrontendReply>>>,
    next_request: AtomicU64,
    generation: AtomicU64,
}

#[derive(Clone, Default)]
pub struct MobileState(Arc<Inner>);

impl MobileState {
    pub fn shutdown(&self) {
        let server = self.0.server.lock().expect("mobile server poisoned").take();
        if let Some(server) = server {
            stop_server(server);
        }
    }

    fn status(&self) -> MobileStatus {
        status_of(
            self.0
                .server
                .lock()
                .expect("mobile server poisoned")
                .as_ref(),
        )
    }
}

fn status_of(server: Option<&Server>) -> MobileStatus {
    match server {
        None => MobileStatus {
            running: false,
            tunnel: TunnelInfo::default(),
            last_client_ms: None,
        },
        Some(server) => MobileStatus {
            running: true,
            tunnel: server.tunnel.clone(),
            last_client_ms: match server.last_client_ms.load(Ordering::Acquire) {
                0 => None,
                ms => Some(ms),
            },
        },
    }
}

fn stop_server(mut server: Server) {
    server.stop.store(true, Ordering::Release);
    let _ = TcpStream::connect_timeout(
        &([127, 0, 0, 1], server.port).into(),
        Duration::from_millis(200),
    );
    if let Some(mut child) = server.child.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn emit_status(app: &tauri::AppHandle, state: &MobileState) {
    let _ = app.emit_to("main", STATUS_EVENT, state.status());
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn qr_svg(text: &str) -> Option<String> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    Some(
        code.render::<qrcode::render::svg::Color<'_>>()
            .min_dimensions(232, 232)
            .quiet_zone(true)
            .build(),
    )
}

#[tauri::command]
pub fn mobile_status(state: tauri::State<'_, MobileState>) -> MobileStatus {
    state.status()
}

#[tauri::command]
pub async fn mobile_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, MobileState>,
) -> Result<MobileStatus, String> {
    let state = state.inner().clone();
    if state
        .0
        .server
        .lock()
        .expect("mobile server poisoned")
        .is_some()
    {
        return Ok(state.status());
    }

    let listener = TcpListener::bind(("127.0.0.1", 0))
        .map_err(|error| format!("bind phone access socket: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("read phone access address: {error}"))?
        .port();
    let token: Arc<str> = generate_token()?.into();
    let stop = Arc::new(AtomicBool::new(false));
    let last_client_ms = Arc::new(AtomicU64::new(0));
    let generation = state.0.generation.fetch_add(1, Ordering::AcqRel) + 1;

    {
        let ctx = ServeContext {
            app: app.clone(),
            state: state.clone(),
            token: token.clone(),
            stop: stop.clone(),
            last_client_ms: last_client_ms.clone(),
            active: Arc::new(AtomicUsize::new(0)),
        };
        thread::Builder::new()
            .name("terax-mobile-listener".into())
            .stack_size(256 * 1024)
            .spawn(move || accept_loop(listener, ctx))
            .map_err(|error| format!("spawn phone access listener: {error}"))?;
    }

    let server = Server {
        generation,
        stop,
        port,
        last_client_ms,
        tunnel: TunnelInfo {
            state: TunnelState::Starting,
            ..TunnelInfo::default()
        },
        child: None,
    };

    {
        let mut guard = state.0.server.lock().expect("mobile server poisoned");
        if guard.is_some() {
            drop(guard);
            stop_server(server);
            return Ok(state.status());
        }
        *guard = Some(server);
    }

    let tunnel_outcome: Result<Option<Child>, (TunnelState, Option<String>)> =
        match tunnel::find_cloudflared() {
            None => Err((TunnelState::Missing, None)),
            Some(binary) => {
                let last_error = Arc::new(Mutex::new(None::<String>));
                let on_line = {
                    let (app, state, token, last_error) = (
                        app.clone(),
                        state.clone(),
                        token.clone(),
                        last_error.clone(),
                    );
                    move |line: &str| {
                        if line.contains(" ERR ") {
                            *last_error.lock().expect("tunnel error poisoned") =
                                Some(line.chars().take(300).collect());
                        }
                        let Some(url) = tunnel::parse_quick_tunnel_url(line) else {
                            return;
                        };
                        let link = format!("{url}/#k={token}");
                        {
                            let mut guard = state.0.server.lock().expect("mobile server poisoned");
                            let Some(server) = guard
                                .as_mut()
                                .filter(|server| server.generation == generation)
                            else {
                                return;
                            };
                            if server.tunnel.public_url.is_some() {
                                return;
                            }
                            server.tunnel = TunnelInfo {
                                state: TunnelState::Ready,
                                public_url: Some(url),
                                qr_svg: qr_svg(&link),
                                link: Some(link),
                                error: None,
                            };
                        }
                        emit_status(&app, &state);
                    }
                };
                let on_exit = {
                    let (app, state) = (app.clone(), state.clone());
                    move || {
                        {
                            let mut guard = state.0.server.lock().expect("mobile server poisoned");
                            let Some(server) = guard
                                .as_mut()
                                .filter(|server| server.generation == generation)
                            else {
                                return;
                            };
                            let error = last_error.lock().expect("tunnel error poisoned").take();
                            server.tunnel = TunnelInfo {
                                state: TunnelState::Failed,
                                error: Some(error.unwrap_or_else(|| "cloudflared exited".into())),
                                ..TunnelInfo::default()
                            };
                        }
                        emit_status(&app, &state);
                    }
                };
                tunnel::spawn(&binary, port, on_line, on_exit)
                    .map(Some)
                    .map_err(|error| {
                        (
                            TunnelState::Failed,
                            Some(format!("start cloudflared: {error}")),
                        )
                    })
            }
        };

    {
        let mut guard = state.0.server.lock().expect("mobile server poisoned");
        match (
            guard.as_mut().filter(|s| s.generation == generation),
            tunnel_outcome,
        ) {
            (Some(server), Ok(child)) => server.child = child,
            (Some(server), Err((tunnel_state, error))) => {
                server.tunnel.state = tunnel_state;
                server.tunnel.error = error;
            }
            (None, Ok(Some(mut child))) => {
                let _ = child.kill();
                let _ = child.wait();
            }
            (None, _) => {}
        }
    }
    let status = state.status();
    emit_status(&app, &state);
    Ok(status)
}

#[tauri::command]
pub async fn mobile_stop(
    app: tauri::AppHandle,
    state: tauri::State<'_, MobileState>,
) -> Result<(), String> {
    let state = state.inner().clone();
    state.shutdown();
    emit_status(&app, &state);
    Ok(())
}

#[tauri::command]
pub fn mobile_respond(
    state: tauri::State<'_, MobileState>,
    request_id: u64,
    reply: FrontendReply,
) -> bool {
    let sender = state
        .0
        .pending
        .lock()
        .expect("mobile pending poisoned")
        .remove(&request_id);
    sender.is_some_and(|sender| sender.send(reply).is_ok())
}

#[derive(Clone)]
struct ServeContext {
    app: tauri::AppHandle,
    state: MobileState,
    token: Arc<str>,
    stop: Arc<AtomicBool>,
    last_client_ms: Arc<AtomicU64>,
    active: Arc<AtomicUsize>,
}

struct ConnectionSlot(Arc<AtomicUsize>);

impl Drop for ConnectionSlot {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn accept_loop(listener: TcpListener, ctx: ServeContext) {
    for incoming in listener.incoming() {
        if ctx.stop.load(Ordering::Acquire) {
            break;
        }
        let Ok(mut stream) = incoming else { continue };
        if ctx.active.fetch_add(1, Ordering::AcqRel) >= MAX_CONNECTIONS {
            ctx.active.fetch_sub(1, Ordering::AcqRel);
            let _ = http::write_response(&mut stream, 503, "text/plain", b"busy");
            continue;
        }
        let slot = ConnectionSlot(ctx.active.clone());
        let ctx = ctx.clone();
        let _ = thread::Builder::new()
            .name("terax-mobile-request".into())
            .stack_size(256 * 1024)
            .spawn(move || {
                let _slot = slot;
                handle_connection(stream, &ctx);
            });
    }
}

fn handle_connection(mut stream: TcpStream, ctx: &ServeContext) {
    let _ = stream.set_read_timeout(Some(IO_TIMEOUT));
    let _ = stream.set_write_timeout(Some(IO_TIMEOUT));
    let request = match http::read_request(&mut BufReader::new(&mut stream)) {
        Ok(request) => request,
        Err(http::HttpError::Closed) => return,
        Err(error) => {
            let _ = http::write_response(&mut stream, error.status(), "text/plain", b"bad request");
            return;
        }
    };
    let (status, content_type, body) = route(&request, ctx);
    let _ = http::write_response(&mut stream, status, content_type, body.as_bytes());
}

type Reply = (u16, &'static str, String);

fn route(request: &http::Request, ctx: &ServeContext) -> Reply {
    let html = "text/html; charset=utf-8";
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/") => (200, html, INDEX_HTML.to_string()),
        ("GET", "/app.js") => (200, "text/javascript; charset=utf-8", APP_JS.to_string()),
        ("GET", "/app.css") => (200, "text/css; charset=utf-8", APP_CSS.to_string()),
        ("POST", path) if path.starts_with("/api/") => api(request, &path[5..], ctx),
        (_, path) if path.starts_with("/api/") || matches!(path, "/" | "/app.js" | "/app.css") => {
            (405, "text/plain", "method not allowed".into())
        }
        _ => (404, "text/plain", "not found".into()),
    }
}

fn api(request: &http::Request, method: &str, ctx: &ServeContext) -> Reply {
    let json_type = "application/json";
    let authorized = request
        .bearer_token()
        .is_some_and(|token| constant_time_eq(token.as_bytes(), ctx.token.as_bytes()));
    if !authorized {
        thread::sleep(AUTH_FAILURE_DELAY);
        return (
            401,
            json_type,
            json!({ "error": "unauthorized" }).to_string(),
        );
    }
    let Some(method) = METHODS.iter().copied().find(|known| *known == method) else {
        return (
            404,
            json_type,
            json!({ "error": "unknown method" }).to_string(),
        );
    };
    let params = if request.body.is_empty() {
        Value::Null
    } else {
        match serde_json::from_slice::<Value>(&request.body) {
            Ok(value) => value,
            Err(_) => {
                return (
                    400,
                    json_type,
                    json!({ "error": "invalid json" }).to_string(),
                )
            }
        }
    };
    ctx.last_client_ms.store(now_ms(), Ordering::Release);
    match forward(ctx, method, params) {
        Ok(result) => (200, json_type, result.to_string()),
        Err((status, error)) => (status, json_type, json!({ "error": error }).to_string()),
    }
}

fn forward(ctx: &ServeContext, method: &str, params: Value) -> Result<Value, (u16, String)> {
    let inner = &ctx.state.0;
    let id = inner.next_request.fetch_add(1, Ordering::AcqRel);
    let (sender, receiver) = mpsc::sync_channel(1);
    {
        let mut pending = inner.pending.lock().expect("mobile pending poisoned");
        if pending.len() >= MAX_PENDING {
            return Err((429, "too many pending requests".into()));
        }
        pending.insert(id, sender);
    }
    let payload = json!({ "id": id, "method": method, "params": params });
    if ctx.app.emit_to("main", REQUEST_EVENT, payload).is_err() {
        inner
            .pending
            .lock()
            .expect("mobile pending poisoned")
            .remove(&id);
        return Err((502, "Terax window unavailable".into()));
    }
    match receiver.recv_timeout(FRONTEND_TIMEOUT) {
        Ok(reply) if reply.ok => Ok(reply.result),
        Ok(reply) => Err((400, reply.error.unwrap_or_else(|| "request failed".into()))),
        Err(_) => {
            inner
                .pending
                .lock()
                .expect("mobile pending poisoned")
                .remove(&id);
            Err((504, "Terax window did not respond".into()))
        }
    }
}
