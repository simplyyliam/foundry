//! Slice commit commands.

use std::sync::Arc;

use tauri::State;

use super::{CommitSliceResult, CommitStatus, SliceCommitter};
use crate::core::AppError;

/// Every uncommitted file in the repo (ignored ones never), at most 500, with whether each is
/// Foundry's own. With a `sliceId`, also the type `commit_slice` would pick and why.
/// Read-only.
#[tauri::command]
#[specta::specta]
pub async fn commit_status(
    store: State<'_, Arc<SliceCommitter>>,
    project_id: String,
    slice_id: Option<String>,
) -> Result<CommitStatus, AppError> {
    let store = Arc::clone(&store);
    tauri::async_runtime::spawn_blocking(move || {
        store.commit_status(&project_id, slice_id.as_deref())
    })
    .await
    .map_err(|err| AppError::internal(format!("commit status task failed: {err}")))?
}

/// Stages everything (`git add -A`) and commits it, with `message` as given or one generated
/// from the slice. Never pushes, branches or amends. Nothing to commit: `files` is empty and
/// `commit` is the current HEAD.
#[tauri::command]
#[specta::specta]
pub async fn commit_slice(
    store: State<'_, Arc<SliceCommitter>>,
    project_id: String,
    slice_id: String,
    message: Option<String>,
) -> Result<CommitSliceResult, AppError> {
    let store = Arc::clone(&store);
    tauri::async_runtime::spawn_blocking(move || {
        store.commit_slice(&project_id, &slice_id, message.as_deref())
    })
    .await
    .map_err(|err| AppError::internal(format!("slice commit task failed: {err}")))?
}
