use std::env;
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use terax_control_protocol::{
    validate_brief, validate_check_command, validate_model, validate_send_text, validate_title,
    CallerContext, ControlDescriptor, ControlRequest, ControlResponse, OpenParams, TaskInfo,
    DEFAULT_WAIT_SECS, MAX_BRIEF_BYTES, MAX_MESSAGE_BYTES, MAX_SCREEN_LINES, MAX_WAIT_SECS,
    METHOD_CAPABILITIES, METHOD_IDENTIFY, METHOD_OPEN, METHOD_ORCHESTRATE, METHOD_PING,
    METHOD_RUN_STOP, METHOD_TASK_CREATE, METHOD_TASK_LIST, METHOD_TASK_MERGE, METHOD_TASK_RESULT,
    METHOD_TASK_SCREEN, METHOD_TASK_SEND, METHOD_TASK_STOP, METHOD_TASK_WAIT, PROTOCOL_VERSION,
    SERVER_RESPONSE_ID, TASK_AGENTS,
};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const IO_TIMEOUT: Duration = Duration::from_secs(7);
// Starting a task creates a git worktree and may promote queued tasks.
const ORCHESTRA_TIMEOUT: Duration = Duration::from_secs(300);
const MERGE_TIMEOUT: Duration = Duration::from_secs(35 * 60);
const EXIT_USAGE: u8 = 2;
const EXIT_UNAVAILABLE: u8 = 3;
const EXIT_PROTOCOL: u8 = 4;
const EXIT_REQUEST: u8 = 5;
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, PartialEq)]
enum Action {
    Help,
    Version,
    Request { method: &'static str, params: Value },
}

#[derive(Debug, PartialEq)]
struct Config {
    json: bool,
    action: Action,
}

#[derive(Debug)]
struct CliError {
    code: String,
    message: String,
    exit: u8,
}

impl CliError {
    fn new(code: impl Into<String>, message: impl Into<String>, exit: u8) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            exit,
        }
    }
}

fn main() -> ExitCode {
    match run(env::args_os().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let mut display_args: Vec<OsString> = env::args_os().skip(1).collect();
            let json = extract_json_flag(&mut display_args);
            if json {
                eprintln!(
                    "{}",
                    json!({
                        "ok": false,
                        "error": { "code": error.code, "message": error.message }
                    })
                );
            } else {
                eprintln!("terax: {}", error.message);
            }
            ExitCode::from(error.exit)
        }
    }
}

fn run(args: Vec<OsString>) -> Result<(), CliError> {
    let config = parse_args(args)?;
    match config.action {
        Action::Help => {
            print_help();
            Ok(())
        }
        Action::Version => {
            println!("terax {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        Action::Request { method, params } => {
            let endpoint = load_endpoint()?;
            let caller = env::var("TERAX_PANE_ID")
                .ok()
                .and_then(|value| value.parse::<u32>().ok());
            let request = ControlRequest {
                protocol: PROTOCOL_VERSION,
                id: request_id(),
                token: endpoint.token,
                method: method.to_string(),
                params,
                caller: CallerContext { pane_id: caller },
            };
            let timeout = response_timeout(method, &request.params);
            let response = send_request(&endpoint.address, &request, timeout)?;
            if !response.ok {
                let error = response.error.unwrap_or_else(|| {
                    terax_control_protocol::ControlError::new(
                        "request_failed",
                        "Terax rejected the request",
                    )
                });
                return Err(CliError::new(error.code, error.message, EXIT_REQUEST));
            }
            let result = response.result.unwrap_or(Value::Null);
            print_result(method, result, config.json);
            Ok(())
        }
    }
}

fn parse_args(mut args: Vec<OsString>) -> Result<Config, CliError> {
    let json = extract_json_flag(&mut args);
    if args.is_empty() {
        return Ok(Config {
            json,
            action: Action::Help,
        });
    }

    let command = args.remove(0);
    let command_text = command.to_str();
    let action = match command_text {
        Some("help" | "--help" | "-h") => no_extra_args(args, Action::Help)?,
        Some("--version" | "-V" | "version") => no_extra_args(args, Action::Version)?,
        Some("ping") => request_without_params(args, METHOD_PING)?,
        Some("capabilities") => request_without_params(args, METHOD_CAPABILITIES)?,
        Some("identify") => request_without_params(args, METHOD_IDENTIFY)?,
        Some("open") => parse_open(args)?,
        Some("orchestrate") => parse_orchestrate(args)?,
        Some("task") => parse_task(args)?,
        Some("run") => parse_run(args)?,
        Some("--") => {
            args.insert(0, command);
            parse_open(args)?
        }
        Some(value) if value.starts_with('-') => {
            return Err(usage_error(format!("unknown option '{value}'")));
        }
        _ => {
            args.insert(0, command);
            parse_open(args)?
        }
    };
    Ok(Config { json, action })
}

fn extract_json_flag(args: &mut Vec<OsString>) -> bool {
    let mut json = false;
    let mut after_separator = false;
    args.retain(|arg| {
        if after_separator {
            return true;
        }
        if arg == "--" {
            after_separator = true;
            return true;
        }
        if arg == "--json" {
            json = true;
            return false;
        }
        true
    });
    json
}

fn no_extra_args(args: Vec<OsString>, action: Action) -> Result<Action, CliError> {
    if args.is_empty() {
        Ok(action)
    } else {
        Err(usage_error("unexpected arguments"))
    }
}

fn request_without_params(args: Vec<OsString>, method: &'static str) -> Result<Action, CliError> {
    no_extra_args(
        args,
        Action::Request {
            method,
            params: json!({}),
        },
    )
}

fn parse_open(args: Vec<OsString>) -> Result<Action, CliError> {
    let mut path: Option<OsString> = None;
    let mut line = None;
    let mut focus = true;
    let mut options = true;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        match arg.to_str() {
            Some("--") if options => options = false,
            Some("--line" | "-l") if options => {
                index += 1;
                let value = args
                    .get(index)
                    .and_then(|value| value.to_str())
                    .ok_or_else(|| usage_error("--line requires a positive integer"))?;
                let parsed = value
                    .parse::<u32>()
                    .ok()
                    .filter(|line| *line > 0)
                    .ok_or_else(|| usage_error("--line requires a positive integer"))?;
                line = Some(parsed);
            }
            Some("--no-focus") if options => focus = false,
            Some(value) if options && value.starts_with('-') => {
                return Err(usage_error(format!("unknown open option '{value}'")));
            }
            _ if path.is_none() => path = Some(arg.clone()),
            _ => return Err(usage_error("open accepts exactly one file path")),
        }
        index += 1;
    }

    let path = path.ok_or_else(|| usage_error("open requires a file path"))?;
    let canonical = std::fs::canonicalize(PathBuf::from(path)).map_err(|error| {
        CliError::new(
            "path_not_found",
            format!("cannot resolve file path: {error}"),
            EXIT_USAGE,
        )
    })?;
    if !canonical.is_file() {
        return Err(CliError::new(
            "not_a_file",
            format!("path is not a regular file: {}", canonical.display()),
            EXIT_USAGE,
        ));
    }
    let path = canonical.into_os_string().into_string().map_err(|_| {
        CliError::new(
            "non_utf8_path",
            "Terax cannot open a path that is not valid UTF-8",
            EXIT_USAGE,
        )
    })?;
    let params = serde_json::to_value(OpenParams {
        path,
        line,
        column: None,
        focus,
    })
    .map_err(|error| {
        CliError::new(
            "serialization_error",
            format!("could not encode open request: {error}"),
            EXIT_PROTOCOL,
        )
    })?;
    Ok(Action::Request {
        method: METHOD_OPEN,
        params,
    })
}

fn response_timeout(method: &str, params: &Value) -> Duration {
    match method {
        METHOD_ORCHESTRATE | METHOD_TASK_CREATE | METHOD_TASK_STOP | METHOD_RUN_STOP => {
            ORCHESTRA_TIMEOUT
        }
        // A merge may also run the run's check command, which can take long.
        METHOD_TASK_MERGE => MERGE_TIMEOUT,
        METHOD_TASK_WAIT => {
            let secs = params
                .get("timeout_secs")
                .and_then(Value::as_u64)
                .unwrap_or(DEFAULT_WAIT_SECS)
                .min(MAX_WAIT_SECS);
            Duration::from_secs(secs) + IO_TIMEOUT
        }
        _ => IO_TIMEOUT,
    }
}

fn utf8_arg(value: &OsString, what: &str) -> Result<String, CliError> {
    value
        .to_str()
        .map(str::to_string)
        .ok_or_else(|| usage_error(format!("{what} must be valid UTF-8")))
}

fn parse_orchestrate(args: Vec<OsString>) -> Result<Action, CliError> {
    let check = match args.as_slice() {
        [] => None,
        [flag, command] if flag == "--check" => {
            let command = utf8_arg(command, "check command")?;
            validate_check_command(&command).map_err(usage_error)?;
            Some(command)
        }
        _ => return Err(usage_error("orchestrate accepts only --check <command>")),
    };
    let cwd = env::current_dir()
        .map_err(|error| usage_error(format!("cannot read the current directory: {error}")))?;
    let cwd = utf8_arg(&cwd.into_os_string(), "current directory")?;
    let mut params = json!({ "cwd": cwd });
    if let Some(check) = check {
        params["check"] = json!(check);
    }
    Ok(Action::Request {
        method: METHOD_ORCHESTRATE,
        params,
    })
}

fn parse_task_wait(args: Vec<OsString>) -> Result<Action, CliError> {
    let mut params = json!({});
    let mut index = 0;
    while index < args.len() {
        match args[index].to_str() {
            Some("--timeout") => {
                let secs = args
                    .get(index + 1)
                    .and_then(|value| value.to_str())
                    .and_then(|value| value.parse::<u64>().ok())
                    .filter(|secs| (1..=MAX_WAIT_SECS).contains(secs))
                    .ok_or_else(|| {
                        usage_error(format!("--timeout requires 1..={MAX_WAIT_SECS} seconds"))
                    })?;
                params["timeout_secs"] = json!(secs);
                index += 2;
            }
            Some(task) if !task.starts_with('-') && params.get("task").is_none() => {
                params["task"] = json!(task);
                index += 1;
            }
            _ => return Err(usage_error("task wait accepts [<task>] [--timeout <secs>]")),
        }
    }
    Ok(Action::Request {
        method: METHOD_TASK_WAIT,
        params,
    })
}

fn parse_task_screen(args: Vec<OsString>) -> Result<Action, CliError> {
    let usage = || usage_error("task screen accepts <task> [--lines <n>]");
    let mut params = json!({});
    let mut index = 0;
    while index < args.len() {
        match args[index].to_str() {
            Some("--lines") => {
                let lines = args
                    .get(index + 1)
                    .and_then(|value| value.to_str())
                    .and_then(|value| value.parse::<u32>().ok())
                    .filter(|lines| (1..=MAX_SCREEN_LINES).contains(lines))
                    .ok_or_else(|| {
                        usage_error(format!("--lines requires 1..={MAX_SCREEN_LINES}"))
                    })?;
                params["lines"] = json!(lines);
                index += 2;
            }
            Some(task) if !task.starts_with('-') && params.get("task").is_none() => {
                params["task"] = json!(task);
                index += 1;
            }
            _ => return Err(usage()),
        }
    }
    if params.get("task").is_none() {
        return Err(usage());
    }
    Ok(Action::Request {
        method: METHOD_TASK_SCREEN,
        params,
    })
}

fn parse_task_merge(args: Vec<OsString>) -> Result<Action, CliError> {
    let usage = || usage_error("task merge accepts [<task>] [--continue | --abort] [--check]");
    let mut params = json!({});
    for arg in &args {
        match arg.to_str() {
            Some("--continue") => params["continue"] = json!(true),
            Some("--abort") => params["abort"] = json!(true),
            Some("--check") => params["check"] = json!(true),
            Some(task) if !task.starts_with('-') && params.get("task").is_none() => {
                params["task"] = json!(task);
            }
            _ => return Err(usage()),
        }
    }
    if params.get("continue") == Some(&json!(true)) && params.get("abort") == Some(&json!(true)) {
        return Err(usage());
    }
    Ok(Action::Request {
        method: METHOD_TASK_MERGE,
        params,
    })
}

fn parse_task_send(args: Vec<OsString>) -> Result<Action, CliError> {
    let [task, words @ ..] = args.as_slice() else {
        return Err(usage_error("task send requires a task id and text"));
    };
    let words = words
        .iter()
        .map(|word| utf8_arg(word, "text"))
        .collect::<Result<Vec<_>, _>>()?;
    let text = words.join(" ");
    validate_send_text(&text).map_err(usage_error)?;
    Ok(Action::Request {
        method: METHOD_TASK_SEND,
        params: json!({ "task": utf8_arg(task, "task id")?, "text": text }),
    })
}

fn parse_task(mut args: Vec<OsString>) -> Result<Action, CliError> {
    if args.is_empty() {
        return Err(usage_error(
            "task requires a subcommand: create, list, wait, result, send, screen, merge or stop",
        ));
    }
    let sub = args.remove(0);
    match sub.to_str() {
        Some("create") => parse_task_create(args, read_brief),
        Some("list") => request_without_params(args, METHOD_TASK_LIST),
        Some("wait") => parse_task_wait(args),
        Some("send") => parse_task_send(args),
        Some("screen") => parse_task_screen(args),
        Some("merge") => parse_task_merge(args),
        Some(sub @ ("stop" | "result")) => match args.as_slice() {
            [task] => Ok(Action::Request {
                method: if sub == "stop" {
                    METHOD_TASK_STOP
                } else {
                    METHOD_TASK_RESULT
                },
                params: json!({ "task": utf8_arg(task, "task id")? }),
            }),
            _ => Err(usage_error(format!(
                "task {sub} requires exactly one task id"
            ))),
        },
        _ => Err(usage_error(
            "unknown task subcommand; use create, list, wait, result, send, screen, merge or stop",
        )),
    }
}

fn parse_task_create(
    args: Vec<OsString>,
    read: impl Fn(&OsString) -> Result<String, CliError>,
) -> Result<Action, CliError> {
    let mut agent = None;
    let mut brief = None;
    let mut title = None;
    let mut model = None;
    let mut index = 0;
    while index < args.len() {
        let flag = args[index].to_str().unwrap_or("");
        let value = args.get(index + 1);
        let take = |name: &str| {
            value
                .ok_or_else(|| usage_error(format!("{name} requires a value")))
                .cloned()
        };
        match flag {
            "--agent" | "-a" => agent = Some(utf8_arg(&take("--agent")?, "agent")?),
            "--brief" | "-b" => brief = Some(take("--brief")?),
            "--title" | "-t" => title = Some(utf8_arg(&take("--title")?, "title")?),
            "--model" | "-m" => model = Some(utf8_arg(&take("--model")?, "model")?),
            other => return Err(usage_error(format!("unknown task create option '{other}'"))),
        }
        index += 2;
    }
    let agent = agent.unwrap_or_else(|| "claude".to_string());
    if !TASK_AGENTS.contains(&agent.as_str()) {
        return Err(usage_error(format!(
            "--agent must be one of: {}",
            TASK_AGENTS.join(", ")
        )));
    }
    let brief = read(&brief.ok_or_else(|| usage_error("task create requires --brief <file|->"))?)?;
    validate_brief(&brief).map_err(usage_error)?;
    if let Some(title) = &title {
        validate_title(title).map_err(usage_error)?;
    }
    if let Some(model) = &model {
        validate_model(model).map_err(usage_error)?;
    }
    let mut params = json!({ "agent": agent, "brief": brief });
    if let Some(title) = title {
        params["title"] = json!(title);
    }
    if let Some(model) = model {
        params["model"] = json!(model);
    }
    Ok(Action::Request {
        method: METHOD_TASK_CREATE,
        params,
    })
}

fn read_brief(source: &OsString) -> Result<String, CliError> {
    let mut bytes = Vec::new();
    let limit = (MAX_BRIEF_BYTES + 1) as u64;
    let result = if source == "-" {
        std::io::stdin().take(limit).read_to_end(&mut bytes)
    } else {
        std::fs::File::open(source).and_then(|file| file.take(limit).read_to_end(&mut bytes))
    };
    result.map_err(|error| {
        CliError::new(
            "brief_unreadable",
            format!("cannot read brief: {error}"),
            EXIT_USAGE,
        )
    })?;
    if bytes.len() > MAX_BRIEF_BYTES {
        return Err(usage_error(format!(
            "brief exceeds {MAX_BRIEF_BYTES} bytes"
        )));
    }
    String::from_utf8(bytes).map_err(|_| usage_error("brief must be UTF-8 text"))
}

fn parse_run(mut args: Vec<OsString>) -> Result<Action, CliError> {
    if args.first().and_then(|arg| arg.to_str()) != Some("stop") {
        return Err(usage_error("run requires the subcommand: stop"));
    }
    args.remove(0);
    let params = match args.as_slice() {
        [] => json!({}),
        [run] => json!({ "run": utf8_arg(run, "run id")? }),
        _ => return Err(usage_error("run stop accepts at most one run id")),
    };
    Ok(Action::Request {
        method: METHOD_RUN_STOP,
        params,
    })
}

fn usage_error(message: impl Into<String>) -> CliError {
    CliError::new("usage", message, EXIT_USAGE)
}

fn load_endpoint() -> Result<ControlDescriptor, CliError> {
    let env_address = env::var("TERAX_CONTROL_ADDR").ok();
    let env_token = env::var("TERAX_CONTROL_TOKEN").ok();
    let (descriptor, require_live_process) = match (env_address, env_token) {
        (Some(address), Some(token)) => (
            ControlDescriptor {
                protocol: PROTOCOL_VERSION,
                address,
                token,
                pid: 0,
                app_version: String::new(),
            },
            false,
        ),
        (None, None) => {
            let path = dirs::cache_dir()
                .map(|dir| dir.join("terax").join("control.json"))
                .ok_or_else(|| {
                    CliError::new(
                        "app_unavailable",
                        "could not locate the user cache directory",
                        EXIT_UNAVAILABLE,
                    )
                })?;
            let bytes = std::fs::read(&path).map_err(|_| {
                CliError::new(
                    "app_unavailable",
                    "Terax is not running; start the app and try again",
                    EXIT_UNAVAILABLE,
                )
            })?;
            let descriptor = serde_json::from_slice(&bytes).map_err(|error| {
                CliError::new(
                    "invalid_descriptor",
                    format!("invalid Terax control descriptor: {error}"),
                    EXIT_PROTOCOL,
                )
            })?;
            (descriptor, true)
        }
        _ => {
            return Err(CliError::new(
                "invalid_environment",
                "TERAX_CONTROL_ADDR and TERAX_CONTROL_TOKEN must be set together",
                EXIT_PROTOCOL,
            ));
        }
    };
    validate_endpoint(descriptor, require_live_process)
}

fn validate_endpoint(
    descriptor: ControlDescriptor,
    require_live_process: bool,
) -> Result<ControlDescriptor, CliError> {
    if descriptor.protocol != PROTOCOL_VERSION {
        return Err(CliError::new(
            "unsupported_protocol",
            format!(
                "Terax uses control protocol {}, but this CLI supports {PROTOCOL_VERSION}",
                descriptor.protocol
            ),
            EXIT_PROTOCOL,
        ));
    }
    if descriptor.token.len() != 64
        || !descriptor
            .token
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(CliError::new(
            "invalid_endpoint",
            "Terax control token is invalid",
            EXIT_PROTOCOL,
        ));
    }
    parse_loopback_address(&descriptor.address)?;
    if require_live_process && !process_is_alive(descriptor.pid) {
        return Err(CliError::new(
            "invalid_endpoint",
            "Terax control process is not running",
            EXIT_PROTOCOL,
        ));
    }
    Ok(descriptor)
}

#[cfg(unix)]
fn process_is_alive(pid: u32) -> bool {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return false;
    };
    if pid <= 0 {
        return false;
    }
    let result = unsafe { libc::kill(pid, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
fn process_is_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ACCESS_DENIED};
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    if pid == 0 {
        return false;
    }
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if !handle.is_null() {
            CloseHandle(handle);
            true
        } else {
            GetLastError() == ERROR_ACCESS_DENIED
        }
    }
}

fn send_request(
    address: &str,
    request: &ControlRequest,
    read_timeout: Duration,
) -> Result<ControlResponse, CliError> {
    let address = parse_loopback_address(address)?;
    let mut stream = TcpStream::connect_timeout(&address, CONNECT_TIMEOUT).map_err(|error| {
        CliError::new(
            "app_unavailable",
            format!("could not connect to Terax: {error}"),
            EXIT_UNAVAILABLE,
        )
    })?;
    stream.set_read_timeout(Some(read_timeout)).ok();
    stream.set_write_timeout(Some(IO_TIMEOUT)).ok();
    write_request(&mut stream, request)?;

    let mut reader = BufReader::new(stream);
    read_response(&mut reader, request)
}

fn parse_loopback_address(address: &str) -> Result<SocketAddr, CliError> {
    let address: SocketAddr = address.parse().map_err(|error| {
        CliError::new(
            "invalid_endpoint",
            format!("invalid Terax control address: {error}"),
            EXIT_PROTOCOL,
        )
    })?;
    if !address.ip().is_loopback() {
        return Err(CliError::new(
            "invalid_endpoint",
            "Terax control address must be loopback-only",
            EXIT_PROTOCOL,
        ));
    }
    Ok(address)
}

fn write_request(writer: &mut impl Write, request: &ControlRequest) -> Result<(), CliError> {
    serde_json::to_writer(&mut *writer, request).map_err(|error| {
        CliError::new(
            "serialization_error",
            format!("could not encode request: {error}"),
            EXIT_PROTOCOL,
        )
    })?;
    writer.write_all(b"\n").map_err(io_error)?;
    writer.flush().map_err(io_error)
}

fn read_response(
    reader: &mut impl BufRead,
    request: &ControlRequest,
) -> Result<ControlResponse, CliError> {
    let mut bytes = Vec::new();
    reader
        .by_ref()
        .take((MAX_MESSAGE_BYTES + 1) as u64)
        .read_until(b'\n', &mut bytes)
        .map_err(io_error)?;
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(CliError::new(
            "message_too_large",
            "Terax response exceeded the protocol limit",
            EXIT_PROTOCOL,
        ));
    }
    if bytes.last() != Some(&b'\n') {
        return Err(CliError::new(
            "invalid_response",
            "Terax returned an incomplete response",
            EXIT_PROTOCOL,
        ));
    }
    let response: ControlResponse = serde_json::from_slice(&bytes).map_err(|error| {
        CliError::new(
            "invalid_response",
            format!("Terax returned invalid JSON: {error}"),
            EXIT_PROTOCOL,
        )
    })?;
    let matched_id =
        response.id == request.id || (!response.ok && response.id == SERVER_RESPONSE_ID);
    if response.protocol != PROTOCOL_VERSION || !matched_id {
        return Err(CliError::new(
            "invalid_response",
            "Terax returned a mismatched protocol version or request id",
            EXIT_PROTOCOL,
        ));
    }
    Ok(response)
}

fn io_error(error: std::io::Error) -> CliError {
    CliError::new(
        "io_error",
        format!("control connection failed: {error}"),
        EXIT_UNAVAILABLE,
    )
}

fn request_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let sequence = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}-{nanos}-{sequence}", std::process::id())
}

fn print_result(method: &str, result: Value, as_json: bool) {
    if as_json {
        println!("{}", json!({ "ok": true, "result": result }));
        return;
    }
    match method {
        METHOD_PING => {
            let version = result
                .get("app_version")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            println!("Terax {version} is running");
        }
        METHOD_CAPABILITIES => {
            if let Some(methods) = result.get("methods").and_then(Value::as_array) {
                for method in methods.iter().filter_map(Value::as_str) {
                    println!("{method}");
                }
            }
        }
        METHOD_IDENTIFY => {
            let space = result
                .get("space_id")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            let tab = result
                .get("tab_id")
                .and_then(Value::as_u64)
                .map_or_else(|| "none".to_string(), |id| id.to_string());
            let pane = result
                .get("pane_id")
                .and_then(Value::as_u64)
                .map_or_else(|| "none".to_string(), |id| id.to_string());
            println!("space={space} tab={tab} pane={pane}");
        }
        METHOD_OPEN => {
            let path = result.get("path").and_then(Value::as_str).unwrap_or("");
            let line = result.get("line").and_then(Value::as_u64);
            if let Some(line) = line {
                println!("Opened {path}:{line} in Terax");
            } else {
                println!("Opened {path} in Terax");
            }
        }
        METHOD_ORCHESTRATE => {
            let run = result.get("run").and_then(Value::as_str).unwrap_or("?");
            let base = result.get("base").and_then(Value::as_str).unwrap_or("?");
            println!("This pane orchestrates run {run} (tasks branch from {base})");
            if let Some(check) = result.get("check").and_then(Value::as_str) {
                println!("Finished tasks run checks: {check}");
            }
        }
        METHOD_TASK_CREATE => match result.get("task").cloned().map(serde_json::from_value) {
            Some(Ok(task)) => print_task(&task),
            _ => println!("{result}"),
        },
        METHOD_TASK_WAIT => {
            let tasks: Vec<TaskInfo> = result
                .get("tasks")
                .cloned()
                .and_then(|tasks| serde_json::from_value(tasks).ok())
                .unwrap_or_default();
            if tasks.is_empty() {
                println!("Nothing to report: no task is in flight or the wait timed out");
            }
            for task in &tasks {
                print_task(task);
            }
        }
        METHOD_TASK_RESULT => {
            if let Some(Ok(task)) = result.get("task").cloned().map(serde_json::from_value) {
                print_task(&task);
            }
            match result.get("result").and_then(Value::as_str) {
                Some(text) => println!("\n{}", text.trim_end()),
                None => println!("\nThe worker wrote no result.md"),
            }
            if let Some(log) = result.get("check_log").and_then(Value::as_str) {
                println!("\nCheck log (tail):\n{}", log.trim_end());
            }
        }
        METHOD_TASK_SEND => println!("Sent"),
        METHOD_TASK_SCREEN => match result.get("screen").and_then(Value::as_str) {
            Some(text) => println!("{}", text.trim_end()),
            None => println!("The worker terminal shows nothing"),
        },
        METHOD_TASK_MERGE => print_merge(&result),
        METHOD_TASK_LIST => {
            let tasks: Vec<TaskInfo> = result
                .get("tasks")
                .cloned()
                .and_then(|tasks| serde_json::from_value(tasks).ok())
                .unwrap_or_default();
            if tasks.is_empty() {
                println!("No tasks");
            }
            for task in &tasks {
                print_task(task);
            }
        }
        METHOD_TASK_STOP | METHOD_RUN_STOP => {
            let list = |key: &str| {
                result
                    .get(key)
                    .and_then(Value::as_array)
                    .map(|ids| {
                        ids.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .unwrap_or_default()
            };
            println!("Stopped: {}", list("stopped"));
            let started = list("started");
            if !started.is_empty() {
                println!("Started from queue: {started}");
            }
        }
        _ => println!("{result}"),
    }
}

fn print_merge(result: &Value) {
    let text = |key: &str| result.get(key).and_then(Value::as_str).unwrap_or_default();
    let ids = |key: &str| {
        result
            .get(key)
            .and_then(Value::as_array)
            .map(|ids| {
                ids.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default()
    };
    println!("Integration branch: {}", text("branch"));
    println!("Integration worktree: {}", text("worktree"));
    if result.get("aborted") == Some(&json!(true)) {
        println!("Aborted the open merge");
    }
    if result.get("continued") == Some(&json!(true)) {
        println!("Committed the resolved merge");
    }
    for (label, key) in [
        ("Merged", "merged"),
        ("Already merged", "already"),
        ("Waiting (not finished)", "waiting"),
    ] {
        let list = ids(key);
        if !list.is_empty() {
            println!("{label}: {list}");
        }
    }
    for skip in result
        .get("skipped")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let task = skip.get("task").and_then(Value::as_str).unwrap_or("?");
        let reason = skip.get("reason").and_then(Value::as_str).unwrap_or("");
        println!("Skipped {task}: {reason}");
    }
    if let Some(conflict) = result.get("conflict").filter(|c| !c.is_null()) {
        let task = conflict
            .get("task")
            .and_then(Value::as_str)
            .unwrap_or("an earlier task");
        println!("CONFLICT while merging {task} in:");
        for file in conflict
            .get("files")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            println!("  {}", file.as_str().unwrap_or(""));
        }
        println!("Resolve the files in the integration worktree, then run: task merge --continue");
    }
    if let Some(check) = result.get("check").filter(|c| !c.is_null()) {
        let passed = check.get("passed") == Some(&json!(true));
        println!("Check: {}", if passed { "passed" } else { "FAILED" });
        if let Some(log) = check.get("log").and_then(Value::as_str) {
            println!("{}", log.trim_end());
        }
    }
}

fn print_task(task: &TaskInfo) {
    let status = serde_json::to_value(task.status)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default();
    let mut fields = vec![
        task.id.as_str(),
        status.as_str(),
        task.agent.as_str(),
        task.branch.as_str(),
        task.worktree.as_str(),
    ];
    let checks = task
        .checks
        .and_then(|checks| serde_json::to_value(checks).ok())
        .and_then(|value| value.as_str().map(|status| format!("checks:{status}")));
    fields.extend(checks.as_deref());
    let model = task.model.as_deref().map(|model| format!("model:{model}"));
    fields.extend(model.as_deref());
    fields.extend(task.title.as_deref());
    println!("{}", fields.join("\t"));
    if let Some(error) = &task.error {
        println!("  error: {error}");
    }
}

fn print_help() {
    println!(
        "Terax command line interface\n\n\
Usage:\n  terax <file> [--line <n>] [--no-focus] [--json]\n  terax open <file> [--line <n>] [--no-focus] [--json]\n  terax ping [--json]\n  terax capabilities [--json]\n  terax identify [--json]\n  terax --version\n\n\
Orchestration:\n  terax orchestrate [--check <cmd>]  make this pane the orchestrator; <cmd> runs in each finished worktree\n  terax task create --brief <file|-> [--agent claude] [--title <text>] [--model <name>]\n  terax task list\n  terax task wait [<task>] [--timeout <secs>]   block until a task finishes or needs input\n  terax task result <task>          print the worker's result.md and check log\n  terax task send <task> <text>     type a follow-up into the worker\n  terax task screen <task> [--lines <n>]   read what the worker terminal shows\n  terax task merge [<task>] [--continue|--abort] [--check]   merge finished tasks into one branch\n  terax task stop <task>\n  terax run stop [<run>]\n\n\
The app must be running. Commands launched in a Terax pane target that pane's space.\n\
Each task runs in its own worktree on branch task/<id>; at most 4 run at once and the rest queue.\n\
Stopping keeps the worktree."
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_transport_commands() {
        let config = parse_args(args(&["ping", "--json"])).expect("parse ping");
        assert!(config.json);
        assert_eq!(
            config.action,
            Action::Request {
                method: METHOD_PING,
                params: json!({}),
            }
        );
    }

    #[test]
    fn rejects_unknown_options() {
        let error = parse_args(args(&["ping", "--wat"])).expect_err("reject option");
        assert_eq!(error.exit, EXIT_USAGE);
    }

    #[test]
    fn json_flag_parsing_respects_the_option_separator() {
        let mut values = args(&["open", "--json", "--", "--json"]);
        assert!(extract_json_flag(&mut values));
        assert_eq!(values, args(&["open", "--", "--json"]));
    }

    #[test]
    fn rejects_zero_line_before_reading_path() {
        let error =
            parse_args(args(&["open", "--line", "0", "missing.rs"])).expect_err("reject zero line");
        assert_eq!(error.code, "usage");
    }

    #[test]
    fn parses_file_shorthand_with_line_and_focus_policy() {
        let file = tempfile::NamedTempFile::new().expect("temp file");
        let config = parse_args(vec![
            file.path().as_os_str().to_owned(),
            "--line".into(),
            "7".into(),
            "--no-focus".into(),
        ])
        .expect("parse open shorthand");
        let Action::Request { method, params } = config.action else {
            panic!("expected request action");
        };
        assert_eq!(method, METHOD_OPEN);
        assert_eq!(params["line"], 7);
        assert_eq!(params["focus"], false);
        assert_eq!(
            params["path"],
            std::fs::canonicalize(file.path())
                .expect("canonical temp path")
                .to_string_lossy()
                .as_ref()
        );
    }

    fn brief_stub(source: &OsString) -> Result<String, CliError> {
        Ok(format!("brief from {}", source.to_string_lossy()))
    }

    #[test]
    fn parses_orchestration_commands() {
        let action = parse_task_create(
            args(&["--brief", "b.md", "--agent", "codex", "--title", "Export"]),
            brief_stub,
        )
        .expect("parse create");
        assert_eq!(
            action,
            Action::Request {
                method: METHOD_TASK_CREATE,
                params: json!({ "agent": "codex", "brief": "brief from b.md", "title": "Export" }),
            }
        );
        let action = parse_task_create(args(&["--brief", "-"]), brief_stub).expect("default");
        let Action::Request { params, .. } = action else {
            panic!("expected request");
        };
        assert_eq!(params["agent"], "claude");

        assert!(parse_task_create(args(&[]), brief_stub).is_err());
        assert!(parse_task_create(args(&["--brief"]), brief_stub).is_err());
        assert!(parse_task_create(args(&["--brief", "b", "--agent", "sh"]), brief_stub).is_err());
        assert!(parse_task_create(args(&["--brief", "b", "--title", "a\nb"]), brief_stub).is_err());

        let config = parse_args(args(&["task", "stop", "ab12-1"])).expect("stop");
        assert_eq!(
            config.action,
            Action::Request {
                method: METHOD_TASK_STOP,
                params: json!({ "task": "ab12-1" }),
            }
        );
        assert!(parse_args(args(&["task", "stop"])).is_err());
        assert!(parse_args(args(&["task", "explode"])).is_err());
        let config = parse_args(args(&["run", "stop"])).expect("run stop");
        assert_eq!(
            config.action,
            Action::Request {
                method: METHOD_RUN_STOP,
                params: json!({}),
            }
        );
        assert!(parse_args(args(&["run", "stop", "a", "b"])).is_err());
        let config = parse_args(args(&["orchestrate", "--json"])).expect("orchestrate");
        assert!(config.json);
        assert!(matches!(
            config.action,
            Action::Request {
                method: METHOD_ORCHESTRATE,
                ..
            }
        ));
    }

    #[test]
    fn orchestration_requests_wait_longer_than_plain_requests() {
        let none = json!({});
        assert_eq!(response_timeout(METHOD_PING, &none), IO_TIMEOUT);
        assert_eq!(response_timeout(METHOD_TASK_LIST, &none), IO_TIMEOUT);
        assert_eq!(
            response_timeout(METHOD_TASK_CREATE, &none),
            ORCHESTRA_TIMEOUT
        );
        assert_eq!(
            response_timeout(METHOD_TASK_WAIT, &json!({ "timeout_secs": 30 })),
            Duration::from_secs(30) + IO_TIMEOUT
        );
        assert_eq!(
            response_timeout(METHOD_TASK_WAIT, &json!({ "timeout_secs": 999_999 })),
            Duration::from_secs(MAX_WAIT_SECS) + IO_TIMEOUT
        );
    }

    #[test]
    fn parses_screen_and_merge_for_the_supervisor() {
        let request = |values: &[&str]| match parse_args(args(values)).expect("parse").action {
            Action::Request { method, params } => (method, params),
            other => panic!("unexpected {other:?}"),
        };
        assert_eq!(
            request(&["task", "screen", "ab-1", "--lines", "40"]),
            (METHOD_TASK_SCREEN, json!({ "task": "ab-1", "lines": 40 }))
        );
        assert!(parse_args(args(&["task", "screen"])).is_err());
        assert!(parse_args(args(&["task", "screen", "ab-1", "--lines", "0"])).is_err());
        assert_eq!(request(&["task", "merge"]), (METHOD_TASK_MERGE, json!({})));
        assert_eq!(
            request(&["task", "merge", "ab-2", "--check"]),
            (METHOD_TASK_MERGE, json!({ "task": "ab-2", "check": true }))
        );
        assert_eq!(
            request(&["task", "merge", "--continue"]),
            (METHOD_TASK_MERGE, json!({ "continue": true }))
        );
        assert!(parse_args(args(&["task", "merge", "--continue", "--abort"])).is_err());
        assert!(parse_args(args(&["task", "merge", "--force"])).is_err());
        assert_eq!(
            response_timeout(METHOD_TASK_MERGE, &json!({})),
            MERGE_TIMEOUT
        );
    }

    #[test]
    fn parses_wait_result_send_and_model() {
        let request = |values: &[&str]| match parse_args(args(values)).expect("parse").action {
            Action::Request { method, params } => (method, params),
            other => panic!("unexpected {other:?}"),
        };
        assert_eq!(request(&["task", "wait"]), (METHOD_TASK_WAIT, json!({})));
        assert_eq!(
            request(&["task", "wait", "ab-1", "--timeout", "60"]),
            (
                METHOD_TASK_WAIT,
                json!({ "task": "ab-1", "timeout_secs": 60 })
            )
        );
        assert!(parse_args(args(&["task", "wait", "--timeout", "0"])).is_err());
        assert!(parse_args(args(&["task", "wait", "a", "b"])).is_err());
        assert_eq!(
            request(&["task", "result", "ab-1"]),
            (METHOD_TASK_RESULT, json!({ "task": "ab-1" }))
        );
        assert_eq!(
            request(&["task", "send", "ab-1", "fix", "the", "test"]),
            (
                METHOD_TASK_SEND,
                json!({ "task": "ab-1", "text": "fix the test" })
            )
        );
        assert!(parse_args(args(&["task", "send", "ab-1"])).is_err());
        assert!(parse_args(args(&["task", "send", "ab-1", "a\nb"])).is_err());

        let action = parse_task_create(args(&["--brief", "b", "--model", "sonnet"]), brief_stub)
            .expect("model");
        let Action::Request { params, .. } = action else {
            panic!("expected request");
        };
        assert_eq!(params["model"], "sonnet");
        assert!(parse_task_create(args(&["--brief", "b", "--model", "--x"]), brief_stub).is_err());

        let (_, params) = request(&["orchestrate", "--check", "pnpm test"]);
        assert_eq!(params["check"], "pnpm test");
        assert!(parse_args(args(&["orchestrate", "--check"])).is_err());
    }

    #[test]
    fn help_is_the_default() {
        assert_eq!(
            parse_args(Vec::new()).expect("parse default").action,
            Action::Help
        );
    }

    #[test]
    fn request_ids_are_safe_ascii() {
        let id = request_id();
        assert!(id.bytes().all(|byte| byte.is_ascii_digit() || byte == b'-'));
    }

    #[test]
    fn endpoint_validation_rejects_non_loopback_addresses() {
        assert!(parse_loopback_address("127.0.0.1:4312").is_ok());
        assert!(parse_loopback_address("[::1]:4312").is_ok());
        let error = parse_loopback_address("192.0.2.1:4312").expect_err("reject remote endpoint");
        assert_eq!(error.code, "invalid_endpoint");
    }

    #[test]
    fn endpoint_validation_requires_a_full_random_token() {
        let descriptor = ControlDescriptor {
            protocol: PROTOCOL_VERSION,
            address: "127.0.0.1:4312".into(),
            token: "short".into(),
            pid: 1,
            app_version: "test".into(),
        };
        let Err(error) = validate_endpoint(descriptor, false) else {
            panic!("accepted invalid token");
        };
        assert_eq!(error.code, "invalid_endpoint");
    }

    #[test]
    fn endpoint_validation_rejects_a_stale_descriptor_process() {
        let descriptor = ControlDescriptor {
            protocol: PROTOCOL_VERSION,
            address: "127.0.0.1:4312".into(),
            token: "a".repeat(64),
            pid: u32::MAX,
            app_version: "test".into(),
        };
        let Err(error) = validate_endpoint(descriptor, true) else {
            panic!("accepted stale process");
        };
        assert_eq!(error.code, "invalid_endpoint");
        assert!(process_is_alive(std::process::id()));
    }

    #[test]
    fn endpoint_validation_rejects_a_protocol_mismatch() {
        let descriptor = ControlDescriptor {
            protocol: PROTOCOL_VERSION + 1,
            address: "127.0.0.1:4312".into(),
            token: "a".repeat(64),
            pid: 1,
            app_version: "test".into(),
        };
        let Err(error) = validate_endpoint(descriptor, false) else {
            panic!("accepted foreign protocol");
        };
        assert_eq!(error.code, "unsupported_protocol");
    }

    #[test]
    fn loopback_parsing_rejects_hostnames_and_remote_hosts() {
        let error = parse_loopback_address("localhost:4312").expect_err("reject hostname");
        assert_eq!(error.code, "invalid_endpoint");
        let error = parse_loopback_address("127.0.0.1").expect_err("reject missing port");
        assert_eq!(error.code, "invalid_endpoint");
    }

    #[test]
    fn protocol_framing_round_trips_one_bounded_json_message() {
        let request = ControlRequest {
            protocol: PROTOCOL_VERSION,
            id: "transport-test".into(),
            token: "test-token".into(),
            method: METHOD_PING.into(),
            params: json!({}),
            caller: CallerContext::default(),
        };
        let response = ControlResponse::success(request.id.clone(), json!({ "pong": true }));
        let mut bytes = serde_json::to_vec(&response).expect("encode response");
        bytes.push(b'\n');
        let response = read_response(&mut Cursor::new(bytes), &request).expect("read response");
        assert_eq!(response.result, Some(json!({ "pong": true })));
    }

    #[test]
    fn protocol_framing_rejects_oversized_responses() {
        let request = ControlRequest {
            protocol: PROTOCOL_VERSION,
            id: "size-test".into(),
            token: "test-token".into(),
            method: METHOD_PING.into(),
            params: json!({}),
            caller: CallerContext::default(),
        };
        let bytes = vec![b'x'; MAX_MESSAGE_BYTES + 1];
        let error = read_response(&mut Cursor::new(bytes), &request).expect_err("reject response");
        assert_eq!(error.code, "message_too_large");
    }

    #[test]
    fn protocol_framing_accepts_server_errors_without_a_parsed_request_id() {
        let request = ControlRequest {
            protocol: PROTOCOL_VERSION,
            id: "busy-test".into(),
            token: "test-token".into(),
            method: METHOD_PING.into(),
            params: json!({}),
            caller: CallerContext::default(),
        };
        let response = ControlResponse::failure(SERVER_RESPONSE_ID, "server_busy", "busy");
        let mut bytes = serde_json::to_vec(&response).expect("encode response");
        bytes.push(b'\n');
        let response = read_response(&mut Cursor::new(bytes), &request).expect("read response");
        assert!(!response.ok);
    }
}
