use tauri::State;

use super::window_radius::{self, WindowRadius};
use super::AppInfo;
use crate::core::AppError;

/// App version and operating system.
#[tauri::command]
#[specta::specta]
pub fn app_info(info: State<'_, AppInfo>) -> Result<AppInfo, AppError> {
    Ok(info.inner().clone())
}

/// The OS window corner radius in pixels, or null when it can't be detected. Never fails.
#[tauri::command]
#[specta::specta]
pub async fn get_window_radius() -> Result<WindowRadius, AppError> {
    Ok(window_radius::detect())
}
