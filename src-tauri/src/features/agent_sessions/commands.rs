use std::sync::Arc;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use super::SessionManager;
use crate::core::sessions::SessionInfo;
use crate::core::{AppError, ErrorCode};

type Manager<'a> = State<'a, Arc<SessionManager>>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentStartOptions {
    pub prompt_file: Option<String>,
    pub slice_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct AgentStopOptions {
    /// Skip the interrupt and the 5-second grace period.
    pub force: bool,
}

/// Runs a manager call off the async runtime: stopping waits, writes can block.
async fn blocking<T: Send + 'static>(
    manager: &Arc<SessionManager>,
    call: impl FnOnce(&SessionManager) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let manager = Arc::clone(manager);
    tauri::async_runtime::spawn_blocking(move || call(&manager))
        .await
        .map_err(|err| AppError::internal(format!("agent task failed: {err}")))?
}

/// Live agent sessions, ordered by agent id. (Idle agents join this once the roster exists.)
#[tauri::command]
#[specta::specta]
pub async fn agent_list(manager: Manager<'_>) -> Result<Vec<SessionInfo>, AppError> {
    blocking(&manager, |m| Ok(m.list())).await
}

/// Starts the agent's Claude Code session. Not available yet: it needs the roster and launch
/// message (P6), the subscription preflight (P3) and the hook server (P8). Always returns
/// `not_implemented` and starts nothing.
#[tauri::command]
#[specta::specta]
pub async fn agent_start(
    agent_id: String,
    options: AgentStartOptions,
) -> Result<SessionInfo, AppError> {
    let _ = options;
    Err(AppError::new(
        ErrorCode::NotImplemented,
        format!(
            "agent {agent_id} can't be started yet: Foundry still needs the agent roster (P6), \
             the subscription check (P3) and the hook server (P8)"
        ),
    ))
}

/// Sends keystrokes or pasted text to a running session.
#[tauri::command]
#[specta::specta]
pub async fn agent_write(
    manager: Manager<'_>,
    agent_id: String,
    data: String,
) -> Result<(), AppError> {
    blocking(&manager, move |m| m.write(&agent_id, data.as_bytes())).await
}

/// Resizes a running session's terminal.
#[tauri::command]
#[specta::specta]
pub async fn agent_resize(
    manager: Manager<'_>,
    agent_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), AppError> {
    blocking(&manager, move |m| m.resize(&agent_id, cols, rows)).await
}

/// Stops a session: Ctrl+C, then after 5 s (at once with `force`) kills its whole process
/// tree. Resolves once the process is gone.
#[tauri::command]
#[specta::specta]
pub async fn agent_stop(
    manager: Manager<'_>,
    agent_id: String,
    options: AgentStopOptions,
) -> Result<(), AppError> {
    blocking(&manager, move |m| m.stop(&agent_id, options.force)).await
}

/// Base64 of the newest terminal output (≤ 256 KB), for a terminal view that remounts.
/// Empty if the agent hasn't run.
#[tauri::command]
#[specta::specta]
pub async fn agent_snapshot(manager: Manager<'_>, agent_id: String) -> Result<String, AppError> {
    blocking(&manager, move |m| {
        Ok(base64::engine::general_purpose::STANDARD.encode(m.snapshot(&agent_id)))
    })
    .await
}
