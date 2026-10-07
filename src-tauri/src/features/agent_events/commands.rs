use std::sync::Arc;

use tauri::State;

use super::{AgentEvent, AgentEventCounts, AgentEventsQuery, AgentEventsStore};
use crate::core::AppError;

type Store<'a> = State<'a, Arc<AgentEventsStore>>;

async fn blocking<T: Send + 'static>(
    store: &Arc<AgentEventsStore>,
    call: impl FnOnce(&AgentEventsStore) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let store = Arc::clone(store);
    tauri::async_runtime::spawn_blocking(move || call(&store))
        .await
        .map_err(|err| AppError::internal(format!("agent events task failed: {err}")))?
}

/// The agent's events in the project, newest first: those before `beforeId`, at least
/// `minSeverity`, at most `limit` (1–500, default 100).
#[tauri::command]
#[specta::specta]
pub async fn agent_events(
    store: Store<'_>,
    project_id: String,
    agent_id: String,
    query: AgentEventsQuery,
) -> Result<Vec<AgentEvent>, AppError> {
    blocking(&store, move |s| s.events(&project_id, &agent_id, &query)).await
}

/// Unread and unread-danger counts per agent with events in the project, by agent id.
#[tauri::command]
#[specta::specta]
pub async fn agent_event_counts(
    store: Store<'_>,
    project_id: String,
) -> Result<Vec<AgentEventCounts>, AppError> {
    blocking(&store, move |s| s.counts(&project_id)).await
}

/// Marks the agent's events up to `upToId` read (never backwards, never past the newest).
#[tauri::command]
#[specta::specta]
pub async fn agent_mark_read(
    store: Store<'_>,
    project_id: String,
    agent_id: String,
    up_to_id: u32,
) -> Result<(), AppError> {
    blocking(&store, move |s| {
        s.mark_read(&project_id, &agent_id, u64::from(up_to_id))
    })
    .await
}
