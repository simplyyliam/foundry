//! Setup-commit command.

use std::sync::Arc;

use tauri::State;

use super::{CommitSetupResult, SetupCommitter};
use crate::core::AppError;

/// Stages and commits Foundry's new or modified setup files ("foundry setup"), and nothing
/// else. Nothing to commit: `files` is empty and `commit` is the current HEAD.
#[tauri::command]
#[specta::specta]
pub async fn commit_setup(
    store: State<'_, Arc<SetupCommitter>>,
    project_id: String,
) -> Result<CommitSetupResult, AppError> {
    let store = Arc::clone(&store);
    tauri::async_runtime::spawn_blocking(move || store.commit_setup(&project_id))
        .await
        .map_err(|err| AppError::internal(format!("setup commit task failed: {err}")))?
}
