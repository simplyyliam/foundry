//! The only place that reads Claude Code's raw hook JSON.
//!
//! Shapes follow the Claude Code hooks reference (common fields `session_id`,
//! `transcript_path`, `cwd`, `hook_event_name`; `tool_name`, `tool_input`, `tool_response`
//! for tool events; `message`, `notification_type` for Notification; `last_assistant_message`
//! for Stop; `source` for SessionStart; `reason` for SessionEnd). Every field is optional
//! here: a missing or oddly typed field degrades to `None`, never to an error.
//!
//! Checked against real payloads captured from Claude Code 2.1.292 (P8b, in `fixtures/`):
//! Bash's `tool_response` has no exit code (`stdout`, `stderr`, `interrupted`, `isImage`,
//! `noOutputExpected`); a non-zero exit fires `PostToolUseFailure` instead, with `error`
//! (`"Exit code 3"`) and `is_interrupt`, not the documented `tool_error`. Fix differences here
//! and nowhere else.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The helper's POST body.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookPost {
    pub project_id: String,
    pub agent_id: String,
    #[serde(default)]
    pub slice_id: Option<String>,
    /// The hook event name. Falls back to the payload's `hook_event_name` when missing.
    #[serde(default)]
    pub hook_event: Option<String>,
    /// The raw hook JSON, as Claude Code passed it to the helper.
    #[serde(default)]
    pub payload: Value,
    /// The helper's own allow-or-block decision (it decides; Foundry only classifies).
    #[serde(default)]
    pub decision: Option<HookDecision>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookDecision {
    #[serde(default)]
    pub blocked: bool,
    #[serde(default)]
    pub rule: Option<String>,
}

/// What a tool call touched, from `tool_name` and `tool_input` (and `tool_response`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolCall {
    pub tool: Option<String>,
    pub command: Option<String>,
    pub file_path: Option<String>,
    /// Why it failed (`PostToolUseFailure`), e.g. `Exit code 3`.
    pub error: Option<String>,
}

/// How a finished tool call went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Passed,
    Failed,
    /// The user (or Claude Code) interrupted it: not a failure of the command.
    Interrupted,
}

/// One hook call, read tolerantly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hook {
    PreToolUse(ToolCall),
    /// `PostToolUse` (it only fires after success), or `PostToolUseFailure`.
    PostToolUse(ToolCall, Outcome),
    Notification {
        kind: Option<String>,
        message: Option<String>,
    },
    Stop {
        last_message: Option<String>,
    },
    SessionStart {
        source: Option<String>,
    },
    SessionEnd {
        reason: Option<String>,
    },
    /// Any other hook event (or none named): classified as nothing.
    Other(Option<String>),
}

impl Hook {
    /// The tool call, for events that have one.
    pub fn tool_call(&self) -> Option<&ToolCall> {
        match self {
            Self::PreToolUse(call) | Self::PostToolUse(call, _) => Some(call),
            _ => None,
        }
    }
}

/// A non-empty string field.
fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn first_text(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| text(value, key))
}

/// The hook event name: the body's `hookEvent`, else the payload's `hook_event_name`.
pub fn event_name(post: &HookPost) -> Option<String> {
    post.hook_event
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| text(&post.payload, "hook_event_name"))
}

fn tool_call(payload: &Value) -> ToolCall {
    let input = payload.get("tool_input").unwrap_or(&Value::Null);
    let response = payload.get("tool_response").unwrap_or(&Value::Null);
    ToolCall {
        tool: text(payload, "tool_name"),
        command: text(input, "command"),
        file_path: first_text(input, &["file_path", "notebook_path", "path"])
            .or_else(|| first_text(response, &["filePath", "file_path", "path"])),
        error: first_text(payload, &["error", "tool_error"]),
    }
}

fn exit_code(response: &Value) -> Option<i64> {
    ["exit_code", "exitCode", "returnCode", "code"]
        .iter()
        .find_map(|key| response.get(key).and_then(Value::as_i64))
}

fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

fn outcome(payload: &Value, failure_event: bool) -> Outcome {
    let response = payload.get("tool_response").unwrap_or(&Value::Null);
    if failure_event {
        return if flag(payload, "is_interrupt") {
            Outcome::Interrupted
        } else {
            Outcome::Failed
        };
    }
    match exit_code(response) {
        Some(0) => Outcome::Passed,
        Some(_) => Outcome::Failed,
        None if flag(response, "interrupted") => Outcome::Interrupted,
        None if payload.get("tool_error").is_some_and(|e| !e.is_null()) => Outcome::Failed,
        // PostToolUse only fires after the tool succeeded (a non-zero exit is a failure event).
        None => Outcome::Passed,
    }
}

pub fn parse(post: &HookPost) -> Hook {
    let payload = &post.payload;
    let name = event_name(post);
    match name.as_deref() {
        Some("PreToolUse") => Hook::PreToolUse(tool_call(payload)),
        Some("PostToolUse") => Hook::PostToolUse(tool_call(payload), outcome(payload, false)),
        Some("PostToolUseFailure") => Hook::PostToolUse(tool_call(payload), outcome(payload, true)),
        Some("Notification") => Hook::Notification {
            kind: text(payload, "notification_type"),
            message: text(payload, "message"),
        },
        Some("Stop") => Hook::Stop {
            last_message: first_text(payload, &["last_assistant_message", "lastAssistantMessage"]),
        },
        Some("SessionStart") => Hook::SessionStart {
            source: text(payload, "source"),
        },
        Some("SessionEnd") => Hook::SessionEnd {
            reason: text(payload, "reason"),
        },
        _ => Hook::Other(name),
    }
}
