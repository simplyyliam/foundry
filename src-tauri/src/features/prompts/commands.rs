use std::sync::Arc;

use tauri::State;

use super::{HandoffResult, PromptFile, PromptsStore, TaskPromptOptions, TaskPromptResult};
use crate::core::AppError;

type Store<'a> = State<'a, Arc<PromptsStore>>;

async fn blocking<T: Send + 'static>(
    store: &Arc<PromptsStore>,
    call: impl FnOnce(&PromptsStore) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let store = Arc::clone(store);
    tauri::async_runtime::spawn_blocking(move || call(&store))
        .await
        .map_err(|err| AppError::internal(format!("prompts task failed: {err}")))?
}

/// Fills the hand-off template into `.foundry/prompts/handoff.md`. Refused unless the plan
/// is confirmed (`plan_not_confirmed`) and AGENTS.md is ready (`agents_md_not_ready`).
#[tauri::command]
#[specta::specta]
pub async fn generate_handoff(
    store: Store<'_>,
    project_id: String,
) -> Result<HandoffResult, AppError> {
    blocking(&store, move |s| s.generate_handoff(&project_id)).await
}

/// Writes `.foundry/prompts/<sliceId>-<agentId>.md`. Unfinished dependencies →
/// `dependencies_not_done`, unless `force`.
#[tauri::command]
#[specta::specta]
pub async fn generate_task_prompt(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
    agent_id: String,
    options: TaskPromptOptions,
) -> Result<TaskPromptResult, AppError> {
    blocking(&store, move |s| {
        s.generate_task_prompt(&project_id, &slice_id, &agent_id, options)
    })
    .await
}

/// One line that starts an agent's session on a prompt file under `.foundry/prompts/`.
#[tauri::command]
#[specta::specta]
pub async fn launch_message(
    store: Store<'_>,
    project_id: String,
    agent_id: String,
    prompt_file: String,
) -> Result<String, AppError> {
    blocking(&store, move |s| {
        s.launch_message(&project_id, &agent_id, &prompt_file)
    })
    .await
}

/// The text of a prompt file under `.foundry/prompts/` (`.md`, at most 200 KB).
#[tauri::command]
#[specta::specta]
pub async fn read_prompt(
    store: Store<'_>,
    project_id: String,
    file: String,
) -> Result<String, AppError> {
    blocking(&store, move |s| s.read_prompt(&project_id, &file)).await
}

#[tauri::command]
#[specta::specta]
pub async fn list_prompts(
    store: Store<'_>,
    project_id: String,
) -> Result<Vec<PromptFile>, AppError> {
    blocking(&store, move |s| s.list_prompts(&project_id)).await
}
