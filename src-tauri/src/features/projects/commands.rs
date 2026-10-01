use std::sync::Arc;

use tauri::State;

use super::{
    CreateRepositoryInput, CreateRepositoryResult, LinkRepositoryInput, LinkRepositoryResult,
    Project, ProjectPatch, ProjectsStore, Settings,
};
use crate::core::AppError;

type Store<'a> = State<'a, Arc<ProjectsStore>>;

/// Runs a store call off the async runtime: git and large-document I/O can block.
async fn blocking<T: Send + 'static>(
    store: &Arc<ProjectsStore>,
    call: impl FnOnce(&ProjectsStore) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let store = Arc::clone(store);
    tauri::async_runtime::spawn_blocking(move || call(&store))
        .await
        .map_err(|err| AppError::internal(format!("projects task failed: {err}")))?
}

/// Every registered project, in the order they were added.
#[tauri::command]
#[specta::specta]
pub async fn list_projects(store: Store<'_>) -> Result<Vec<Project>, AppError> {
    blocking(&store, |s| s.list()).await
}

/// Creates `<parentDir>/<slug(name)>` with git and the Foundry files, and registers it.
/// Errors with `folder_exists` (touching nothing) if that folder already exists.
#[tauri::command]
#[specta::specta]
pub async fn create_repository(
    store: Store<'_>,
    input: CreateRepositoryInput,
) -> Result<CreateRepositoryResult, AppError> {
    blocking(&store, move |s| s.create_repository(input)).await
}

/// Registers an existing folder, adding only missing Foundry files. Never runs git.
#[tauri::command]
#[specta::specta]
pub async fn link_repository(
    store: Store<'_>,
    input: LinkRepositoryInput,
) -> Result<LinkRepositoryResult, AppError> {
    blocking(&store, move |s| s.link_repository(input)).await
}

/// Changes name and/or tagline. Never renames or moves the folder.
#[tauri::command]
#[specta::specta]
pub async fn update_project(
    store: Store<'_>,
    id: String,
    patch: ProjectPatch,
) -> Result<Project, AppError> {
    blocking(&store, move |s| s.update_project(&id, patch)).await
}

/// Makes the project active and records `lastOpened`. `folder_missing` if its folder is gone.
#[tauri::command]
#[specta::specta]
pub async fn open_project(store: Store<'_>, id: String) -> Result<Project, AppError> {
    blocking(&store, move |s| s.open_project(&id)).await
}

/// The project opened most recently in this session, or null.
#[tauri::command]
#[specta::specta]
pub async fn active_project(store: Store<'_>) -> Result<Option<Project>, AppError> {
    blocking(&store, |s| s.active_project()).await
}

/// Points a project at a new folder, e.g. after it was moved.
#[tauri::command]
#[specta::specta]
pub async fn relink_project(
    store: Store<'_>,
    id: String,
    folder: String,
) -> Result<Project, AppError> {
    blocking(&store, move |s| s.relink_project(&id, &folder)).await
}

/// Archives the project (registry flag only; the folder is untouched). Idempotent.
/// If it's the active project, it's closed: `active_project` returns null afterwards.
#[tauri::command]
#[specta::specta]
pub async fn archive_project(store: Store<'_>, id: String) -> Result<Project, AppError> {
    blocking(&store, move |s| s.archive_project(&id)).await
}

/// Unarchives (restores) the project. Idempotent.
#[tauri::command]
#[specta::specta]
pub async fn unarchive_project(store: Store<'_>, id: String) -> Result<Project, AppError> {
    blocking(&store, move |s| s.unarchive_project(&id)).await
}

/// Unregisters an archived project. Never deletes repo files.
/// `not_archived` if the project isn't archived.
#[tauri::command]
#[specta::specta]
pub async fn remove_project(store: Store<'_>, id: String) -> Result<(), AppError> {
    blocking(&store, move |s| s.remove_project(&id)).await
}

/// The project's `docs/brain-dump.md` (empty if missing).
#[tauri::command]
#[specta::specta]
pub async fn get_document(store: Store<'_>, id: String) -> Result<String, AppError> {
    blocking(&store, move |s| s.get_document(&id)).await
}

/// Replaces `docs/brain-dump.md` atomically.
#[tauri::command]
#[specta::specta]
pub async fn save_document(store: Store<'_>, id: String, markdown: String) -> Result<(), AppError> {
    blocking(&store, move |s| s.save_document(&id, &markdown)).await
}

#[tauri::command]
#[specta::specta]
pub async fn get_settings(store: Store<'_>) -> Result<Settings, AppError> {
    blocking(&store, |s| s.get_settings()).await
}

/// Replaces the settings; returns them as saved (paths normalized).
#[tauri::command]
#[specta::specta]
pub async fn update_settings(store: Store<'_>, settings: Settings) -> Result<Settings, AppError> {
    blocking(&store, move |s| s.update_settings(settings)).await
}
