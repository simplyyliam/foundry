use std::sync::Arc;

use tauri::State;

use super::{
    DesignFilePatch, DesignLibrary, DesignStore, GenerateAgentsResult, GenerateOptions, Placeholder,
};
use crate::core::AppError;

type Store<'a> = State<'a, Arc<DesignStore>>;

async fn blocking<T: Send + 'static>(
    store: &Arc<DesignStore>,
    call: impl FnOnce(&DesignStore) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let store = Arc::clone(store);
    tauri::async_runtime::spawn_blocking(move || call(&store))
        .await
        .map_err(|err| AppError::internal(format!("design task failed: {err}")))?
}

/// The Design Library, with `untaggedCount` (untagged files are never copied).
#[tauri::command]
#[specta::specta]
pub async fn design_library_list(store: Store<'_>) -> Result<DesignLibrary, AppError> {
    blocking(&store, |s| s.library()).await
}

/// Adds `.md` files (absolute paths), untagged. All or nothing; other files →
/// `unsupported_file`.
#[tauri::command]
#[specta::specta]
pub async fn design_library_add_from_paths(
    store: Store<'_>,
    paths: Vec<String>,
) -> Result<DesignLibrary, AppError> {
    blocking(&store, move |s| s.add_from_paths(&paths)).await
}

#[tauri::command]
#[specta::specta]
pub async fn design_library_update(
    store: Store<'_>,
    id: String,
    patch: DesignFilePatch,
) -> Result<DesignLibrary, AppError> {
    blocking(&store, move |s| s.update(&id, patch)).await
}

#[tauri::command]
#[specta::specta]
pub async fn design_library_remove(
    store: Store<'_>,
    id: String,
) -> Result<DesignLibrary, AppError> {
    blocking(&store, move |s| s.remove(&id)).await
}

/// The template's markers (`inTemplate: true`) plus `designFamily`, with confirmed values.
#[tauri::command]
#[specta::specta]
pub async fn agents_placeholders(
    store: Store<'_>,
    project_id: String,
) -> Result<Vec<Placeholder>, AppError> {
    blocking(&store, move |s| s.placeholders(&project_id)).await
}

/// Confirms one placeholder. `designFamily` takes comma-separated tags.
#[tauri::command]
#[specta::specta]
pub async fn confirm_placeholder(
    store: Store<'_>,
    project_id: String,
    key: String,
    value: String,
) -> Result<Vec<Placeholder>, AppError> {
    blocking(&store, move |s| {
        s.confirm_placeholder(&project_id, &key, &value)
    })
    .await
}

/// Writes AGENTS.md, docs/design/ copies and docs/design/manifest.md.
#[tauri::command]
#[specta::specta]
pub async fn generate_agents(
    store: Store<'_>,
    project_id: String,
    options: GenerateOptions,
) -> Result<GenerateAgentsResult, AppError> {
    blocking(&store, move |s| s.generate(&project_id, options)).await
}
