use std::sync::Arc;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

use super::{AgentStartOptions, SessionManager};
use crate::core::sessions::SessionInfo;
use crate::core::AppError;

type Manager<'a> = State<'a, Arc<SessionManager>>;

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct AgentStopOptions {
    /// Skip the interrupt and the 5-second grace period.
    pub force: bool,
}

/// Runs a manager call off the async runtime: starting runs the preflight, stopping waits,
/// writes can block.
async fn blocking<T: Send + 'static>(
    manager: &Arc<SessionManager>,
    call: impl FnOnce(&SessionManager) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let manager = Arc::clone(manager);
    tauri::async_runtime::spawn_blocking(move || call(&manager))
        .await
        .map_err(|err| AppError::internal(format!("agent task failed: {err}")))?
}

/// The project's agent sessions, ordered by agent id: live ones, and each other agent's last
/// finished one (`exited`, with its `exitCode`) until it starts again or the app exits.
#[tauri::command]
#[specta::specta]
pub async fn agent_list(
    manager: Manager<'_>,
    project_id: String,
) -> Result<Vec<SessionInfo>, AppError> {
    blocking(&manager, move |m| Ok(m.list(&project_id))).await
}

/// Starts the agent's Claude Code session, on a prompt file under `.foundry/prompts/` or,
/// without one, waiting for your instructions. The launch message is built by the backend
/// from `agents/roles.json` (and the prompt file). Clears the agent's finished session.
#[tauri::command]
#[specta::specta]
pub async fn agent_start(
    manager: Manager<'_>,
    project_id: String,
    agent_id: String,
    options: AgentStartOptions,
) -> Result<SessionInfo, AppError> {
    blocking(&manager, move |m| {
        m.start_agent(&project_id, &agent_id, options)
    })
    .await
}

/// Sends keystrokes or pasted text to a running session.
#[tauri::command]
#[specta::specta]
pub async fn agent_write(
    manager: Manager<'_>,
    project_id: String,
    agent_id: String,
    data: String,
) -> Result<(), AppError> {
    blocking(&manager, move |m| {
        m.write(&project_id, &agent_id, data.as_bytes())
    })
    .await
}

/// Resizes a running session's terminal.
#[tauri::command]
#[specta::specta]
pub async fn agent_resize(
    manager: Manager<'_>,
    project_id: String,
    agent_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), AppError> {
    blocking(&manager, move |m| {
        m.resize(&project_id, &agent_id, cols, rows)
    })
    .await
}

/// Stops a session: Ctrl+C, then after 5 s (at once with `force`) kills its whole process
/// tree. Resolves once the process is gone. `not_found` for a finished session.
#[tauri::command]
#[specta::specta]
pub async fn agent_stop(
    manager: Manager<'_>,
    project_id: String,
    agent_id: String,
    options: AgentStopOptions,
) -> Result<(), AppError> {
    blocking(&manager, move |m| {
        m.stop(&project_id, &agent_id, options.force)
    })
    .await
}

/// Base64 of the newest terminal output (≤ 256 KB), for a terminal view that remounts.
/// Empty if the agent hasn't run in this project.
#[tauri::command]
#[specta::specta]
pub async fn agent_snapshot(
    manager: Manager<'_>,
    project_id: String,
    agent_id: String,
) -> Result<String, AppError> {
    blocking(&manager, move |m| {
        Ok(base64::engine::general_purpose::STANDARD.encode(m.snapshot(&project_id, &agent_id)))
    })
    .await
}
