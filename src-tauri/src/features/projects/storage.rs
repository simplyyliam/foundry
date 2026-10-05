//! App-level files in the OS app-data directory: `projects.json` and `settings.json`.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::{NotificationSeverity, Settings, Theme, VerifyMode};
use crate::core::{fsx, AppError};

pub const REGISTRY_FILE: &str = "projects.json";
pub const SETTINGS_FILE: &str = "settings.json";

/// One registered project, as stored in `projects.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryEntry {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub tagline: String,
    pub repo_path: String,
    /// Epoch ms of the last `open_project`.
    #[serde(default)]
    pub last_opened: Option<u64>,
    /// Foundry-side flag only; archiving never touches the folder.
    #[serde(default)]
    pub archived: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct RegistryFile {
    /// Bumped on incompatible changes so later versions can migrate.
    pub version: u32,
    pub projects: Vec<RegistryEntry>,
}

pub const REGISTRY_VERSION: u32 = 1;

/// `settings.json` as read from disk: any missing field takes its default, so files written
/// by older versions still load. (The exported `Settings` keeps every field required.)
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SettingsFile {
    max_concurrent_agents: u32,
    default_projects_dir: Option<String>,
    claude_path: Option<String>,
    default_model: Option<String>,
    generation_timeout_ms: u32,
    agents_template_id: String,
    theme: Theme,
    reduced_motion_override: bool,
    auto_install_hooks: bool,
    default_verify_mode: VerifyMode,
    os_notifications_enabled: bool,
    os_notification_severity: NotificationSeverity,
}

impl Default for SettingsFile {
    fn default() -> Self {
        let defaults = Settings::default();
        Self {
            max_concurrent_agents: defaults.max_concurrent_agents,
            default_projects_dir: defaults.default_projects_dir,
            claude_path: defaults.claude_path,
            default_model: defaults.default_model,
            generation_timeout_ms: defaults.generation_timeout_ms,
            agents_template_id: defaults.agents_template_id,
            theme: defaults.theme,
            reduced_motion_override: defaults.reduced_motion_override,
            auto_install_hooks: defaults.auto_install_hooks,
            default_verify_mode: defaults.default_verify_mode,
            os_notifications_enabled: defaults.os_notifications_enabled,
            os_notification_severity: defaults.os_notification_severity,
        }
    }
}

impl From<SettingsFile> for Settings {
    fn from(file: SettingsFile) -> Self {
        Self {
            max_concurrent_agents: file.max_concurrent_agents,
            default_projects_dir: file.default_projects_dir,
            claude_path: file.claude_path,
            // A hand-edited model name outside the allowed characters is ignored.
            default_model: file
                .default_model
                .map(|model| model.trim().to_string())
                .filter(|model| super::is_model_name(model)),
            // An unusable 0 in the file falls back to the default.
            generation_timeout_ms: match file.generation_timeout_ms {
                0 => super::DEFAULT_GENERATION_TIMEOUT_MS,
                ms => ms,
            },
            agents_template_id: Some(file.agents_template_id.trim().to_string())
                .filter(|id| !id.is_empty())
                .unwrap_or_else(|| super::DEFAULT_AGENTS_TEMPLATE_ID.into()),
            // A hand-edited value outside each enum loads as that field's default.
            theme: file.theme.or_default(),
            reduced_motion_override: file.reduced_motion_override,
            auto_install_hooks: file.auto_install_hooks,
            default_verify_mode: file.default_verify_mode.or_default(),
            os_notifications_enabled: file.os_notifications_enabled,
            os_notification_severity: file.os_notification_severity.or_default(),
        }
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Loads a JSON file, or the default when it doesn't exist. An unreadable file is moved
/// aside to `<name>.corrupt-<ms>` (kept for recovery, never deleted) and the default is used,
/// so one bad write can't stop the app from starting.
pub fn load<T: DeserializeOwned + Default>(dir: &Path, name: &str) -> Result<T, AppError> {
    let path = dir.join(name);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(err) => return Err(err.into()),
    };
    match serde_json::from_slice(&bytes) {
        Ok(value) => Ok(value),
        Err(err) => {
            let aside = dir.join(format!("{name}.corrupt-{}", now_ms()));
            tracing::warn!(file = name, error = %err, moved_to = %aside.display(), "unreadable app file; starting fresh");
            std::fs::rename(&path, &aside)?;
            Ok(T::default())
        }
    }
}

pub fn save<T: Serialize>(dir: &Path, name: &str, value: &T) -> Result<(), AppError> {
    let json = serde_json::to_vec_pretty(value)
        .map_err(|err| AppError::internal(format!("serialize {name}: {err}")))?;
    fsx::write_atomic(&dir.join(name), json)
}

pub fn save_registry(dir: &Path, projects: &[RegistryEntry]) -> Result<(), AppError> {
    #[derive(Serialize)]
    struct Out<'a> {
        version: u32,
        projects: &'a [RegistryEntry],
    }
    save(
        dir,
        REGISTRY_FILE,
        &Out {
            version: REGISTRY_VERSION,
            projects,
        },
    )
}

pub fn save_settings(dir: &Path, settings: &Settings) -> Result<(), AppError> {
    save(dir, SETTINGS_FILE, settings)
}
