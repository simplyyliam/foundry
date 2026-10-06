use std::sync::Arc;

use tauri::State;

use super::{
    AvatarChoice, HandbookRole, HandbookRolePatch, NewHandbookRole, RolesImported, RosterAgent,
    Team, TeamAgent, TeamProposal, TeamStore, TeamWritten,
};
use crate::core::AppError;

type Store<'a> = State<'a, Arc<TeamStore>>;

async fn blocking<T: Send + 'static>(
    store: &Arc<TeamStore>,
    call: impl FnOnce(&TeamStore) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let store = Arc::clone(store);
    tauri::async_runtime::spawn_blocking(move || call(&store))
        .await
        .map_err(|err| AppError::internal(format!("team task failed: {err}")))?
}

/// The Role Handbook (seeded with the starter roles the first time).
#[tauri::command]
#[specta::specta]
pub async fn role_handbook_list(store: Store<'_>) -> Result<Vec<HandbookRole>, AppError> {
    blocking(&store, |s| s.handbook()).await
}

#[tauri::command]
#[specta::specta]
pub async fn role_handbook_add(
    store: Store<'_>,
    role: NewHandbookRole,
) -> Result<Vec<HandbookRole>, AppError> {
    blocking(&store, move |s| s.add_role(role)).await
}

/// One role per dropped `.md` file (frontmatter optional). All or nothing; an id the
/// handbook already has is `role_exists` unless `replace`. The files are only read.
#[tauri::command]
#[specta::specta]
pub async fn role_handbook_add_from_paths(
    store: Store<'_>,
    paths: Vec<String>,
    replace: bool,
) -> Result<RolesImported, AppError> {
    blocking(&store, move |s| s.add_roles_from_paths(&paths, replace)).await
}

#[tauri::command]
#[specta::specta]
pub async fn role_handbook_update(
    store: Store<'_>,
    id: String,
    patch: HandbookRolePatch,
) -> Result<Vec<HandbookRole>, AppError> {
    blocking(&store, move |s| s.update_role(&id, patch)).await
}

#[tauri::command]
#[specta::specta]
pub async fn role_handbook_remove(
    store: Store<'_>,
    id: String,
) -> Result<Vec<HandbookRole>, AppError> {
    blocking(&store, move |s| s.remove_role(&id)).await
}

/// The Product Lead's proposed roster. Writes nothing; edit it, then `confirm_team`.
#[tauri::command]
#[specta::specta]
pub async fn compose_team(store: Store<'_>, project_id: String) -> Result<TeamProposal, AppError> {
    blocking(&store, move |s| s.compose(&project_id)).await
}

/// Writes the confirmed roster: `agents/roles.json` and one role file per agent.
#[tauri::command]
#[specta::specta]
pub async fn confirm_team(
    store: Store<'_>,
    project_id: String,
    roster: Vec<RosterAgent>,
) -> Result<TeamWritten, AppError> {
    blocking(&store, move |s| s.confirm(&project_id, roster)).await
}

/// Adds one agent to a confirmed team, leaving every existing agent as it was.
#[tauri::command]
#[specta::specta]
pub async fn add_agent_manually(
    store: Store<'_>,
    project_id: String,
    agent: RosterAgent,
) -> Result<TeamWritten, AppError> {
    blocking(&store, move |s| s.add_manually(&project_id, agent)).await
}

/// The confirmed team, or null.
#[tauri::command]
#[specta::specta]
pub async fn get_team(store: Store<'_>, project_id: String) -> Result<Option<Team>, AppError> {
    blocking(&store, move |s| s.team(&project_id)).await
}

/// Ids of the built-in avatars (empty until images are added).
#[tauri::command]
#[specta::specta]
pub async fn builtin_avatar_list() -> Result<Vec<String>, AppError> {
    Ok(TeamStore::builtin_avatars())
}

/// A built-in avatar, or an uploaded image (processed like a Bakery image).
#[tauri::command]
#[specta::specta]
pub async fn set_agent_avatar(
    store: Store<'_>,
    project_id: String,
    agent_id: String,
    choice: AvatarChoice,
) -> Result<TeamAgent, AppError> {
    blocking(&store, move |s| {
        s.set_avatar(&project_id, &agent_id, choice)
    })
    .await
}

/// The agent's avatar as a data URL (at most 256 px on its long edge), or null when it has
/// none or its file is missing. Read-only.
#[tauri::command]
#[specta::specta]
pub async fn agent_avatar(
    store: Store<'_>,
    project_id: String,
    agent_id: String,
) -> Result<Option<String>, AppError> {
    blocking(&store, move |s| s.avatar(&project_id, &agent_id)).await
}
