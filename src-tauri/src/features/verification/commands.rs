use std::sync::Arc;

use tauri::State;

use super::{AgentReport, ManagerDigest, TestCommands, Verification, VerificationStore};
use crate::core::report::SliceReport;
use crate::core::AppError;

type Store<'a> = State<'a, Arc<VerificationStore>>;

/// Runs a store call off the async runtime: test commands can run for minutes.
async fn blocking<T: Send + 'static>(
    store: &Arc<VerificationStore>,
    call: impl FnOnce(&VerificationStore) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let store = Arc::clone(store);
    tauri::async_runtime::spawn_blocking(move || call(&store))
        .await
        .map_err(|err| AppError::internal(format!("verification task failed: {err}")))?
}

/// `testCommands` from `.foundry/project.json`, and whether they're the confirmed list.
#[tauri::command]
#[specta::specta]
pub async fn get_test_commands(
    store: Store<'_>,
    project_id: String,
) -> Result<TestCommands, AppError> {
    blocking(&store, move |s| s.get_test_commands(&project_id)).await
}

/// Replaces `testCommands` (each trimmed; blank or multi-line → `invalid_input`). They're
/// then unconfirmed.
#[tauri::command]
#[specta::specta]
pub async fn set_test_commands(
    store: Store<'_>,
    project_id: String,
    commands: Vec<String>,
) -> Result<TestCommands, AppError> {
    blocking(&store, move |s| s.set_test_commands(&project_id, commands)).await
}

/// Confirms `commands`, the list the human was shown (kept in app data, never in the repo).
/// `test_commands_changed`, confirming nothing, if `testCommands` no longer equals it.
#[tauri::command]
#[specta::specta]
pub async fn confirm_test_commands(
    store: Store<'_>,
    project_id: String,
    commands: Vec<String>,
) -> Result<TestCommands, AppError> {
    blocking(&store, move |s| {
        s.confirm_test_commands(&project_id, commands)
    })
    .await
}

/// The slice's first report (the legacy `.foundry/reports/<sliceId>.json`, else the first
/// agent's `<sliceId>.<agentId>.json` by agent id), or null if there's none. Kept for its
/// shape; `list_reports` returns every agent's.
#[tauri::command]
#[specta::specta]
pub async fn get_report(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
) -> Result<Option<SliceReport>, AppError> {
    blocking(&store, move |s| s.get_report(&project_id, &slice_id)).await
}

/// Every report of the slice, each with its file and agent: the legacy `<sliceId>.json`
/// first, then one per agent by agent id. Empty if there's none.
#[tauri::command]
#[specta::specta]
pub async fn list_reports(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
) -> Result<Vec<AgentReport>, AppError> {
    blocking(&store, move |s| s.list_reports(&project_id, &slice_id)).await
}

/// Verifies every report of the slice and saves `.foundry/reviews/<sliceId>.json`. Progress arrives
/// as `verify:progress`.
#[tauri::command]
#[specta::specta]
pub async fn verify_slice(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
) -> Result<Verification, AppError> {
    blocking(&store, move |s| s.verify(&project_id, &slice_id)).await
}

/// The slice's last verification with `stale` worked out now, or null.
#[tauri::command]
#[specta::specta]
pub async fn get_verification(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
) -> Result<Option<Verification>, AppError> {
    blocking(&store, move |s| s.get_verification(&project_id, &slice_id)).await
}

/// Stops the project's running verification (it fails with `cancelled`, nothing saved).
/// Returns whether one was running.
#[tauri::command]
#[specta::specta]
pub async fn cancel_verification(store: Store<'_>, project_id: String) -> Result<bool, AppError> {
    blocking(&store, move |s| Ok(s.cancel(&project_id))).await
}

#[tauri::command]
#[specta::specta]
pub async fn manager_digest(
    store: Store<'_>,
    project_id: String,
) -> Result<ManagerDigest, AppError> {
    blocking(&store, move |s| s.manager_digest(&project_id)).await
}

/// Writes the digest to `.foundry/reviews/digest.md` and returns it.
#[tauri::command]
#[specta::specta]
pub async fn write_digest_file(
    store: Store<'_>,
    project_id: String,
) -> Result<ManagerDigest, AppError> {
    blocking(&store, move |s| s.write_digest_file(&project_id)).await
}
