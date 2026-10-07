use std::sync::Arc;

use tauri::State;

use super::{HooksChange, HooksStatus, HooksStore};
use crate::core::AppError;

type Store<'a> = State<'a, Arc<HooksStore>>;

async fn blocking<T: Send + 'static>(
    store: &Arc<HooksStore>,
    call: impl FnOnce(&HooksStore) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let store = Arc::clone(store);
    tauri::async_runtime::spawn_blocking(move || call(&store))
        .await
        .map_err(|err| AppError::internal(format!("hooks task failed: {err}")))?
}

/// Merges Foundry's hooks into `.claude/settings.local.json`. Refused with nothing written:
/// `claude_not_found`, `claude_version_unsupported` (older than 2.1.139, or unreadable),
/// `not_found` (no helper), `invalid_json` (the file doesn't parse).
#[tauri::command]
#[specta::specta]
pub async fn hooks_install(store: Store<'_>, project_id: String) -> Result<HooksChange, AppError> {
    blocking(&store, move |s| s.install(&project_id)).await
}

/// Removes Foundry's hooks only. `invalid_json` when the file doesn't parse.
#[tauri::command]
#[specta::specta]
pub async fn hooks_uninstall(
    store: Store<'_>,
    project_id: String,
) -> Result<HooksChange, AppError> {
    blocking(&store, move |s| s.uninstall(&project_id)).await
}

/// Whether Foundry's hooks are installed, and whether they can be.
#[tauri::command]
#[specta::specta]
pub async fn hooks_status(store: Store<'_>, project_id: String) -> Result<HooksStatus, AppError> {
    blocking(&store, move |s| s.status(&project_id)).await
}
