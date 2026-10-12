//! App-level info and lifecycle events.

pub mod commands;
pub mod window_radius;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{Emitter, Manager, Runtime};
use tauri_specta::Event;

use crate::core::events::{self, EventMeta};
use crate::core::AppError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AppInfo {
    /// App version from the Tauri config (falls back to Cargo.toml).
    pub version: String,
    /// `std::env::consts::OS`: "windows", "macos", "linux", ...
    pub os: String,
}

pub fn info(version: impl Into<String>) -> AppInfo {
    AppInfo {
        version: version.into(),
        os: std::env::consts::OS.to_string(),
    }
}

/// Emitted once when the backend has finished setup. Best-effort: it can fire before the
/// webview is listening, so call `appInfo()` on mount rather than waiting for this event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[tauri_specta(event_name = "app:ready")]
pub struct AppReady {
    #[serde(flatten)]
    pub meta: EventMeta,
    pub version: String,
}

/// Called once from setup, after events are mounted: stores [`AppInfo`] for the
/// `app_info` command, then announces `app:ready`.
pub fn init<R, H>(handle: &H) -> Result<(), AppError>
where
    R: Runtime,
    H: Emitter<R> + Manager<R>,
{
    handle.manage(info(handle.package_info().version.to_string()));
    announce_ready(handle)
}

pub fn announce_ready<R, H>(handle: &H) -> Result<(), AppError>
where
    R: Runtime,
    H: Emitter<R> + Manager<R>,
{
    let version = handle.package_info().version.to_string();
    tracing::info!(%version, "app ready");
    events::emit(
        handle,
        &AppReady {
            meta: EventMeta::new(),
            version,
        },
    )
}

#[cfg(test)]
mod tests;
