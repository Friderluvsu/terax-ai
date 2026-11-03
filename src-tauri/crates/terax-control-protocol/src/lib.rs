use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const PROTOCOL_VERSION: u16 = 1;
pub const MAX_MESSAGE_BYTES: usize = 64 * 1024;
pub const METHOD_PING: &str = "ping";
pub const METHOD_CAPABILITIES: &str = "capabilities";
pub const METHOD_IDENTIFY: &str = "identify";
pub const METHOD_OPEN: &str = "open";
pub const METHOD_ORCHESTRATE: &str = "orchestrate";
pub const METHOD_TASK_CREATE: &str = "task.create";
pub const METHOD_TASK_LIST: &str = "task.list";
pub const METHOD_TASK_STOP: &str = "task.stop";
pub const METHOD_TASK_WAIT: &str = "task.wait";
pub const METHOD_TASK_RESULT: &str = "task.result";
pub const METHOD_TASK_SEND: &str = "task.send";
pub const METHOD_TASK_SCREEN: &str = "task.screen";
pub const METHOD_TASK_MERGE: &str = "task.merge";
pub const METHOD_RUN_STOP: &str = "run.stop";
pub const SERVER_RESPONSE_ID: &str = "server";
pub const METHODS: &[&str] = &[
    METHOD_PING,
    METHOD_CAPABILITIES,
    METHOD_IDENTIFY,
    METHOD_OPEN,
    METHOD_ORCHESTRATE,
    METHOD_TASK_CREATE,
    METHOD_TASK_LIST,
    METHOD_TASK_STOP,
    METHOD_TASK_WAIT,
    METHOD_TASK_RESULT,
    METHOD_TASK_SEND,
    METHOD_TASK_SCREEN,
    METHOD_TASK_MERGE,
    METHOD_RUN_STOP,
];
pub const MAX_BRIEF_BYTES: usize = 16 * 1024;
const _: () = assert!(MAX_BRIEF_BYTES * 2 < MAX_MESSAGE_BYTES);
pub const MAX_TASK_TITLE_CHARS: usize = 120;
pub const TASK_AGENTS: &[&str] = &["claude", "codex", "antigravity", "pi", "opencode", "grok"];
pub const MAX_CHECK_COMMAND_CHARS: usize = 1024;
pub const MAX_SEND_TEXT_BYTES: usize = 4 * 1024;
pub const DEFAULT_WAIT_SECS: u64 = 600;
pub const MAX_WAIT_SECS: u64 = 3600;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct CallerContext {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<u32>,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub struct ControlRequest {
    pub protocol: u16,
    pub id: String,
    pub token: String,
    pub method: String,
    #[serde(default)]
    pub params: Value,
    #[serde(default)]
    pub caller: CallerContext,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ControlError {
    pub code: String,
    pub message: String,
}

impl ControlError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ControlResponse {
    pub protocol: u16,
    pub id: String,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ControlError>,
}

impl ControlResponse {
    pub fn success(id: impl Into<String>, result: Value) -> Self {
        Self {
            protocol: PROTOCOL_VERSION,
            id: id.into(),
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(
        id: impl Into<String>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            protocol: PROTOCOL_VERSION,
            id: id.into(),
            ok: false,
            result: None,
            error: Some(ControlError::new(code, message)),
        }
    }
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
pub struct ControlDescriptor {
    pub protocol: u16,
    pub address: String,
    pub token: String,
    pub pid: u32,
    pub app_version: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FrontendRequest {
    pub id: String,
    pub method: String,
    pub params: Value,
    pub caller: CallerContext,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FrontendResponse {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<ControlError>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OpenParams {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    #[serde(default = "default_focus")]
    pub focus: bool,
}

fn default_focus() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OrchestrateParams {
    pub cwd: String,
    /// Shell command run in a task's worktree after its agent finishes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskCreateParams {
    pub agent: String,
    pub brief: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskWaitParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskScreenParams {
    pub task: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lines: Option<u32>,
}

/// `task.merge` merges one finished task, or every finished task when `task`
/// is absent, into the run's integration branch. `continue` commits a
/// resolved conflict and `abort` drops it.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct TaskMergeParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task: Option<String>,
    #[serde(default, rename = "continue", skip_serializing_if = "std::ops::Not::not")]
    pub continue_merge: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub abort: bool,
    /// Run the run's check command in the integration worktree afterwards.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub check: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PaneScreenParams {
    pub pane_id: u32,
    pub lines: u32,
}

pub const MAX_SCREEN_LINES: u32 = 200;
pub const DEFAULT_SCREEN_LINES: u32 = 60;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskSendParams {
    pub task: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskStopParams {
    pub task: String,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct RunStopParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Starting,
    Running,
    /// The agent asked a question or a permission.
    NeedsInput,
    /// The agent finished its turn; its tab stays open for follow-ups.
    Done,
    Stopped,
    Failed,
    Interrupted,
}

impl TaskStatus {
    pub fn is_working(self) -> bool {
        matches!(self, Self::Starting | Self::Running | Self::NeedsInput)
    }

    pub fn is_active(self) -> bool {
        self.is_working() || matches!(self, Self::Queued | Self::Done)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Running,
    Passed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskInfo {
    pub id: String,
    pub run: String,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub status: TaskStatus,
    pub branch: String,
    pub worktree: String,
    pub brief_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checks: Option<CheckStatus>,
    /// Settled but not yet returned by `task.wait`.
    #[serde(default)]
    pub unreported: bool,
    /// Merged into the run's integration branch.
    #[serde(default)]
    pub merged: bool,
}

/// Sent to the UI when a task leaves the queue: it opens one worker tab.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskOpenParams {
    pub task: String,
    pub agent: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub cwd: String,
    pub brief_path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Claude Code settings that sandbox the worker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_path: Option<String>,
    /// The board run that owns the task; its terminal goes on that board.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub board: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct TaskCloseParams {
    pub pane_id: u32,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct PaneSendParams {
    pub pane_id: u32,
    pub text: String,
}

pub fn validate_model(model: &str) -> Result<(), String> {
    let ok = !model.is_empty()
        && model.len() <= 64
        && model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._:/-[]".contains(&b))
        && !model.starts_with('-');
    ok.then_some(())
        .ok_or_else(|| "model must be 1-64 characters of [A-Za-z0-9._:/-[]]".to_string())
}

pub fn validate_check_command(command: &str) -> Result<(), String> {
    if command.trim().is_empty() || command.chars().count() > MAX_CHECK_COMMAND_CHARS {
        return Err(format!(
            "check command must contain 1-{MAX_CHECK_COMMAND_CHARS} characters"
        ));
    }
    if command.chars().any(char::is_control) {
        return Err("check command must be a single line".to_string());
    }
    Ok(())
}

pub fn validate_send_text(text: &str) -> Result<(), String> {
    if text.trim().is_empty() || text.len() > MAX_SEND_TEXT_BYTES {
        return Err(format!("text must contain 1-{MAX_SEND_TEXT_BYTES} bytes"));
    }
    if text.chars().any(char::is_control) {
        return Err("text must be a single line".to_string());
    }
    Ok(())
}

// Only tab and line breaks are allowed, which bounds JSON escaping at 2x and
// keeps a brief inside one control message.
pub fn validate_brief(brief: &str) -> Result<(), String> {
    if brief.trim().is_empty() {
        return Err("brief is empty".to_string());
    }
    if brief.len() > MAX_BRIEF_BYTES {
        return Err(format!("brief exceeds {MAX_BRIEF_BYTES} bytes"));
    }
    if brief
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err("brief contains control characters".to_string());
    }
    Ok(())
}

pub fn validate_title(title: &str) -> Result<(), String> {
    if title.trim().is_empty() || title.chars().count() > MAX_TASK_TITLE_CHARS {
        return Err(format!(
            "title must contain 1-{MAX_TASK_TITLE_CHARS} characters"
        ));
    }
    if title.chars().any(char::is_control) {
        return Err("title must be a single line".to_string());
    }
    Ok(())
}

pub const FRONTEND_TASK_OPEN: &str = "task.open";
pub const FRONTEND_TASK_CLOSE: &str = "task.close";
pub const FRONTEND_TASK_SEND: &str = "task.send";
pub const FRONTEND_TASK_SCREEN: &str = "task.screen";

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn request_round_trips_without_caller_context() {
        let raw = json!({
            "protocol": PROTOCOL_VERSION,
            "id": "42",
            "token": "secret",
            "method": METHOD_PING,
            "params": {}
        });
        let request: ControlRequest = serde_json::from_value(raw).expect("deserialize request");
        assert_eq!(request.caller, CallerContext::default());
        assert_eq!(request.method, METHOD_PING);
    }

    #[test]
    fn response_shapes_are_unambiguous() {
        let success = ControlResponse::success("1", json!({ "pong": true }));
        assert!(success.ok);
        assert!(success.result.is_some());
        assert!(success.error.is_none());

        let failure = ControlResponse::failure("2", "invalid_request", "bad request");
        assert!(!failure.ok);
        assert!(failure.result.is_none());
        assert_eq!(failure.error.expect("error").code, "invalid_request");
    }

    #[test]
    fn open_defaults_to_focusing_the_target() {
        let params: OpenParams =
            serde_json::from_value(json!({ "path": "/tmp/a" })).expect("deserialize open params");
        assert!(params.focus);
    }

    #[test]
    fn merge_params_use_the_continue_keyword_on_the_wire() {
        let params: TaskMergeParams =
            serde_json::from_value(json!({ "continue": true, "check": true })).unwrap();
        assert!(params.continue_merge && params.check && !params.abort);
        assert_eq!(
            serde_json::to_value(TaskMergeParams::default()).unwrap(),
            json!({})
        );
    }

    #[test]
    fn protocol_version_is_locked_at_one() {
        assert_eq!(PROTOCOL_VERSION, 1);
        assert_eq!(MAX_MESSAGE_BYTES, 64 * 1024);
    }

    #[test]
    fn methods_catalog_matches_the_wire_literals() {
        assert_eq!(
            METHODS,
            &[
                "ping",
                "capabilities",
                "identify",
                "open",
                "orchestrate",
                "task.create",
                "task.list",
                "task.stop",
                "task.wait",
                "task.result",
                "task.send",
                "task.screen",
                "task.merge",
                "run.stop",
            ]
        );
    }

    #[test]
    fn task_status_uses_snake_case_and_counts_only_live_workers() {
        assert_eq!(
            serde_json::to_value(TaskStatus::Interrupted).unwrap(),
            json!("interrupted")
        );
        assert_eq!(
            serde_json::to_value(TaskStatus::NeedsInput).unwrap(),
            json!("needs_input")
        );
        assert!(TaskStatus::NeedsInput.is_working());
        assert!(!TaskStatus::Done.is_working());
        assert!(!TaskStatus::Queued.is_working());
        assert!(TaskStatus::Done.is_active());
        assert!(!TaskStatus::Stopped.is_active());
    }

    #[test]
    fn model_check_and_send_validation_rejects_shell_tricks() {
        assert!(validate_model("sonnet").is_ok());
        assert!(validate_model("claude-sonnet-4-5[1m]").is_ok());
        for bad in ["", "--dangerous", "a b", "x;rm", "$(id)", &"m".repeat(65)] {
            assert!(validate_model(bad).is_err(), "{bad:?}");
        }
        assert!(validate_check_command("pnpm lint && pnpm test").is_ok());
        assert!(validate_check_command("a\nb").is_err());
        assert!(validate_send_text("also add tests").is_ok());
        assert!(validate_send_text("x\r").is_err());
        assert!(validate_send_text(&"x".repeat(MAX_SEND_TEXT_BYTES + 1)).is_err());
    }

    #[test]
    fn brief_and_title_validation_bounds_the_wire_size() {
        assert!(validate_brief("do the thing\n\twith tabs").is_ok());
        assert!(validate_brief("  \n").is_err());
        assert!(validate_brief("bell\u{7}").is_err());
        assert!(validate_brief(&"a".repeat(MAX_BRIEF_BYTES + 1)).is_err());
        let worst = "\"".repeat(MAX_BRIEF_BYTES);
        assert!(validate_brief(&worst).is_ok());
        assert!(serde_json::to_string(&worst).unwrap().len() < MAX_MESSAGE_BYTES / 2 + 8);
        assert!(validate_title("Export sessions").is_ok());
        assert!(validate_title("two\nlines").is_err());
        assert!(validate_title(&"x".repeat(MAX_TASK_TITLE_CHARS + 1)).is_err());
    }

    #[test]
    fn open_params_omit_absent_optionals_from_the_wire() {
        let minimal = OpenParams {
            path: "/tmp/a".into(),
            line: None,
            column: None,
            focus: true,
        };
        assert_eq!(
            serde_json::to_value(&minimal).unwrap(),
            json!({ "path": "/tmp/a", "focus": true })
        );

        let located = OpenParams {
            path: "/tmp/a".into(),
            line: Some(12),
            column: Some(34),
            focus: false,
        };
        assert_eq!(
            serde_json::to_value(&located).unwrap(),
            json!({
                "path": "/tmp/a",
                "line": 12,
                "column": 34,
                "focus": false,
            })
        );
    }

    #[test]
    fn success_responses_omit_null_fields_and_failures_omit_results() {
        let success = ControlResponse::success("7", json!({ "version": "0.8.6" }));
        let value = serde_json::to_value(&success).unwrap();
        assert!(value.get("result").is_some());
        assert!(value.get("error").is_none());

        let failure = ControlResponse::failure("8", "unauthorized", "bad token");
        let value = serde_json::to_value(&failure).unwrap();
        assert!(value.get("result").is_none());
        assert!(value.get("error").is_some());
    }

    #[test]
    fn caller_context_round_trips_with_and_without_a_pane() {
        let with_pane = CallerContext { pane_id: Some(3) };
        let value = serde_json::to_value(&with_pane).unwrap();
        assert_eq!(value, json!({ "pane_id": 3 }));
        let back: CallerContext = serde_json::from_value(value).unwrap();
        assert_eq!(back, with_pane);

        let value = serde_json::to_value(CallerContext::default()).unwrap();
        assert_eq!(value, json!({}));
    }

    #[test]
    fn frontend_exchange_shapes_survive_a_round_trip() {
        let request = FrontendRequest {
            id: "9".into(),
            method: METHOD_OPEN.into(),
            params: json!({ "path": "/tmp/a", "line": 1 }),
            caller: CallerContext { pane_id: Some(5) },
        };
        let value = serde_json::to_value(&request).unwrap();
        let back: FrontendRequest = serde_json::from_value(value).unwrap();
        assert_eq!(back, request);

        let response = FrontendResponse {
            ok: true,
            result: Some(json!({ "opened": true })),
            error: None,
        };
        let value = serde_json::to_value(&response).unwrap();
        assert!(value.get("error").is_none());
        let back: FrontendResponse = serde_json::from_value(value).unwrap();
        assert_eq!(back, response);
    }

    #[test]
    fn descriptor_carries_the_discovery_fields() {
        let descriptor = ControlDescriptor {
            protocol: PROTOCOL_VERSION,
            address: "127.0.0.1:54321".into(),
            token: "tok".into(),
            pid: 4242,
            app_version: "0.8.6".into(),
        };
        let back: ControlDescriptor =
            serde_json::from_str(&serde_json::to_string(&descriptor).unwrap()).unwrap();
        assert_eq!(back.protocol, 1);
        assert_eq!(back.address, "127.0.0.1:54321");
        assert_eq!(back.token, "tok");
        assert_eq!(back.pid, 4242);
        assert_eq!(back.app_version, "0.8.6");
    }
}
