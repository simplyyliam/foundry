use tauri::State;

use super::AppInfo;
use crate::core::AppError;

/// App version and operating system.
#[tauri::command]
#[specta::specta]
pub fn app_info(info: State<'_, AppInfo>) -> Result<AppInfo, AppError> {
    Ok(info.inner().clone())
}
