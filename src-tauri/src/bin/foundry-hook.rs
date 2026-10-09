//! `foundry-hook`: the Claude Code hook helper, run for every hook event Foundry installs
//! (`hooks_install`). Shipped as a Tauri sidecar next to the app's executable.
//!
//! 1. No `FOUNDRY_AGENT_ID` in the environment (a human's own Claude Code session in the
//!    repo): does nothing, exits 0.
//! 2. Reads the hook JSON from stdin. For `PreToolUse` it decides locally, never over the
//!    network (see `core/hook_policy.rs`): a block exits 2 with the reason on stderr, which
//!    Claude Code shows the agent; a warning exits 0 with a `systemMessage`.
//! 3. POSTs the event to Foundry at `127.0.0.1:FOUNDRY_HOOK_PORT` with `FOUNDRY_HOOK_TOKEN`,
//!    within 500 ms, ignoring the answer. Foundry not running changes nothing.
//!
//! Only std, serde and serde_json: it starts on every tool call, so it stays small and fast.

#[path = "../core/glob.rs"]
mod glob;
#[allow(dead_code)]
#[path = "../core/hook.rs"]
mod hook;
#[path = "../core/hook_policy.rs"]
mod hook_policy;

use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

use hook_policy::{Verdict, COMMAND_TOOLS, EDIT_TOOLS, ROLES_FILE};

/// The whole POST (connect, send, first bytes of the answer) gets this long.
const POST_BUDGET: Duration = Duration::from_millis(500);
/// Must match the hook server's route (an integration test posts to the real server).
const ROUTE: &str = "/events";
/// The server refuses bodies over 256 KB; stay under it.
const BODY_LIMIT: usize = 250 * 1024;
/// Bulky fields Foundry never needs (file contents, diffs, output), dropped before posting.
const BULKY_INPUT: &[&str] = &[
    "content",
    "old_string",
    "new_string",
    "edits",
    "new_source",
    "file_text",
    "old_str",
    "new_str",
];
const BULKY_RESPONSE: &[&str] = &[
    "stdout",
    "stderr",
    "content",
    "originalFile",
    "structuredPatch",
    "oldString",
    "newString",
    "file",
    "edits",
];
const LAST_MESSAGE_CAP: usize = 1000;

fn env(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
}

fn main() -> ExitCode {
    let Some(agent_id) = env("FOUNDRY_AGENT_ID") else {
        return ExitCode::SUCCESS;
    };
    let mut raw = String::new();
    let payload: Value = match std::io::stdin()
        .read_to_string(&mut raw)
        .ok()
        .and_then(|_| serde_json::from_str(&raw).ok())
    {
        Some(payload) => payload,
        None => {
            eprintln!("Foundry blocked this: foundry-hook couldn't read the hook input. Stop and ask the human.");
            return ExitCode::from(2);
        }
    };

    let event = text(&payload, "hook_event_name")
        .unwrap_or_default()
        .to_string();
    let tool = text(&payload, "tool_name").unwrap_or_default().to_string();
    let decided = event == "PreToolUse"
        && (EDIT_TOOLS.contains(&tool.as_str()) || COMMAND_TOOLS.contains(&tool.as_str()));
    let verdict = if decided {
        decide(&payload, &agent_id, &tool)
    } else {
        Verdict::Allow
    };

    let code = match &verdict {
        Verdict::Allow => ExitCode::SUCCESS,
        Verdict::Warn { message, .. } => {
            let out = json!({
                "systemMessage": message,
                "hookSpecificOutput": { "hookEventName": "PreToolUse", "additionalContext": message },
            });
            println!("{out}");
            ExitCode::SUCCESS
        }
        Verdict::Block { message, .. } => {
            eprintln!("{message}");
            ExitCode::from(2)
        }
    };
    let _ = std::io::stdout().flush();
    let _ = std::io::stderr().flush();

    if let (Some(port), Some(token), Some(project_id)) = (
        env("FOUNDRY_HOOK_PORT").and_then(|p| p.parse::<u16>().ok()),
        env("FOUNDRY_HOOK_TOKEN"),
        env("FOUNDRY_PROJECT_ID"),
    ) {
        let decision = decided.then(|| json!({ "blocked": matches!(verdict, Verdict::Block { .. }), "rule": verdict.rule() }));
        let body = json!({
            "projectId": project_id,
            "agentId": agent_id,
            "sliceId": env("FOUNDRY_SLICE_ID"),
            "hookEvent": event,
            "payload": slim(payload),
            "decision": decision,
        });
        post(port, &token, body.to_string().as_bytes());
    }
    code
}

fn decide(payload: &Value, agent_id: &str, tool: &str) -> Verdict {
    let input = payload.get("tool_input").unwrap_or(&Value::Null);
    if COMMAND_TOOLS.contains(&tool) {
        return hook_policy::check_command(text(input, "command").unwrap_or_default());
    }
    let cwd = text(payload, "cwd")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_default();
    let root = env("CLAUDE_PROJECT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| cwd.clone());
    let roles = read_roles(&root);
    let target = text(input, "file_path").or_else(|| text(input, "notebook_path"));
    let slice_id = env("FOUNDRY_SLICE_ID");
    hook_policy::check_edit(
        &root,
        roles.as_deref(),
        agent_id,
        slice_id.as_deref(),
        target,
        &cwd,
    )
}

/// `None` when `roles.json` is missing. Unreadable reads as empty, which doesn't parse.
fn read_roles(root: &Path) -> Option<String> {
    match std::fs::read_to_string(root.join(ROLES_FILE)) {
        Ok(text) => Some(text),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => Some(String::new()),
    }
}

/// The payload without file contents or command output, and under the server's size cap.
fn slim(mut payload: Value) -> Value {
    let drop_keys = |value: Option<&mut Value>, keys: &[&str]| {
        if let Some(Value::Object(map)) = value {
            for key in keys {
                map.remove(*key);
            }
        }
    };
    drop_keys(payload.get_mut("tool_input"), BULKY_INPUT);
    drop_keys(payload.get_mut("tool_response"), BULKY_RESPONSE);
    if let Some(Value::String(message)) = payload.get_mut("last_assistant_message") {
        if message.chars().count() > LAST_MESSAGE_CAP {
            *message = message.chars().take(LAST_MESSAGE_CAP).collect();
        }
    }
    if payload.to_string().len() <= BODY_LIMIT {
        return payload;
    }
    // Still too big: keep only what the classifier reads.
    let mut small = Map::new();
    for key in [
        "hook_event_name",
        "tool_name",
        "cwd",
        "notification_type",
        "source",
        "reason",
        "error",
        "is_interrupt",
    ] {
        if let Some(value) = payload.get(key) {
            small.insert(key.into(), value.clone());
        }
    }
    let input = payload.get("tool_input").unwrap_or(&Value::Null);
    let mut kept = Map::new();
    for key in ["command", "file_path", "notebook_path"] {
        if let Some(value) = input
            .get(key)
            .filter(|v| v.as_str().is_some_and(|s| s.len() < 8 * 1024))
        {
            kept.insert(key.into(), value.clone());
        }
    }
    small.insert("tool_input".into(), Value::Object(kept));
    Value::Object(small)
}

/// Fire and forget: one HTTP/1.1 POST over a plain socket, all within [`POST_BUDGET`].
fn post(port: u16, token: &str, body: &[u8]) {
    if token.contains(['\r', '\n']) {
        return;
    }
    let deadline = Instant::now() + POST_BUDGET;
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut stream) = TcpStream::connect_timeout(&addr, POST_BUDGET) else {
        return;
    };
    let left = || {
        deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
    };
    let Some(time) = left() else { return };
    let _ = stream.set_write_timeout(Some(time));
    let head = format!(
        "POST {ROUTE} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n{}: {token}\r\nConnection: close\r\n\r\n",
        body.len(),
        hook::TOKEN_HEADER,
    );
    if stream
        .write_all(head.as_bytes())
        .and_then(|()| stream.write_all(body))
        .is_err()
    {
        return;
    }
    // Wait briefly for the status line so the request is read before the socket closes.
    let Some(time) = left() else { return };
    let _ = stream.set_read_timeout(Some(time));
    let mut answer = [0u8; 64];
    let _ = stream.read(&mut answer);
}
