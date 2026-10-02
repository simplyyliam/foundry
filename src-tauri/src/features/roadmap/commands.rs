use std::sync::Arc;

use tauri::State;

use super::{RoadmapResult, RoadmapStore};
use crate::core::claude::ClaudeStatus;
use crate::core::AppError;

type Store<'a> = State<'a, Arc<RoadmapStore>>;

/// Runs a store call off the async runtime: generation blocks for minutes.
async fn blocking<T: Send + 'static>(
    store: &Arc<RoadmapStore>,
    call: impl FnOnce(&RoadmapStore) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let store = Arc::clone(store);
    tauri::async_runtime::spawn_blocking(move || call(&store))
        .await
        .map_err(|err| AppError::internal(format!("roadmap task failed: {err}")))?
}

/// Generates the roadmap from `docs/brain-dump.md`, validates it (one retry), and saves it
/// to `.foundry/roadmap.json`. Progress arrives as `roadmap:progress`.
#[tauri::command]
#[specta::specta]
pub async fn generate_roadmap(
    store: Store<'_>,
    project_id: String,
) -> Result<RoadmapResult, AppError> {
    blocking(&store, move |s| s.generate(&project_id)).await
}

/// Stops the project's running generation; it then fails with `cancelled`. Returns whether
/// one was running.
#[tauri::command]
#[specta::specta]
pub async fn cancel_generation(store: Store<'_>, project_id: String) -> Result<bool, AppError> {
    blocking(&store, move |s| Ok(s.cancel(&project_id))).await
}

/// The last saved roadmap, or null.
#[tauri::command]
#[specta::specta]
pub async fn get_roadmap(
    store: Store<'_>,
    project_id: String,
) -> Result<Option<RoadmapResult>, AppError> {
    blocking(&store, move |s| s.get(&project_id)).await
}

/// Whether Claude Code is installed and signed in with a subscription. Never fails: a
/// problem is reported in `problem`.
#[tauri::command]
#[specta::specta]
pub async fn claude_status(store: Store<'_>) -> Result<ClaudeStatus, AppError> {
    blocking(&store, |s| Ok(s.claude_status())).await
}
