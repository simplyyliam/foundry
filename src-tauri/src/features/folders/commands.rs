use super::{FolderListing, FolderLocations};
use crate::core::AppError;

/// Visible subfolders of `path` (absolute), or of the home folder when `path` is null.
#[tauri::command]
#[specta::specta]
pub async fn list_folders(path: Option<String>) -> Result<FolderListing, AppError> {
    // Off the async runtime: listing a slow or network drive can block.
    tauri::async_runtime::spawn_blocking(move || super::list(path.as_deref()))
        .await
        .map_err(|err| AppError::internal(format!("folder listing task failed: {err}")))?
}

/// The home folder and filesystem roots, for the picker's quick locations.
#[tauri::command]
#[specta::specta]
pub fn folder_locations() -> Result<FolderLocations, AppError> {
    super::locations()
}
