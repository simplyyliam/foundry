//! Plan commands. Every mutation takes an optional `expectedRevision` (`stale_revision` on a
//! mismatch) and emits `plan:changed`.

use std::sync::Arc;

use tauri::State;

use super::{
    LoadRoadmapOptions, NewDeferred, NewRequirement, NewSlice, Plan, PlanAdded, PlanStore,
    PlanSummary, RequirementPatch, RoadmapLoaded, SlicePatch, SliceStatus, StatusOptions,
};
use crate::core::AppError;

type Store<'a> = State<'a, Arc<PlanStore>>;

/// Runs a store call off the async runtime: file I/O and the per-project lock can block.
async fn blocking<T: Send + 'static>(
    store: &Arc<PlanStore>,
    call: impl FnOnce(&PlanStore) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let store = Arc::clone(store);
    tauri::async_runtime::spawn_blocking(move || call(&store))
        .await
        .map_err(|err| AppError::internal(format!("plan task failed: {err}")))?
}

/// The plan with derived `layer`, `usedBy` and `ready` per slice.
#[tauri::command]
#[specta::specta]
pub async fn get_plan(store: Store<'_>, project_id: String) -> Result<Plan, AppError> {
    blocking(&store, move |s| s.get(&project_id)).await
}

/// Unassigned and unconfirmed requirements, ready and blocked slices, and whether the plan
/// is fully confirmed.
#[tauri::command]
#[specta::specta]
pub async fn plan_summary(store: Store<'_>, project_id: String) -> Result<PlanSummary, AppError> {
    blocking(&store, move |s| s.summary(&project_id)).await
}

#[tauri::command]
#[specta::specta]
pub async fn add_slice(
    store: Store<'_>,
    project_id: String,
    slice: NewSlice,
    expected_revision: Option<u32>,
) -> Result<PlanAdded, AppError> {
    blocking(&store, move |s| {
        s.add_slice(&project_id, slice, expected_revision)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn update_slice(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
    patch: SlicePatch,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.update_slice(&project_id, &slice_id, patch, expected_revision)
    })
    .await
}

/// `slice_in_use` while requirements are assigned to it, or slices or deferred items point
/// at it.
#[tauri::command]
#[specta::specta]
pub async fn remove_slice(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.remove_slice(&project_id, &slice_id, expected_revision)
    })
    .await
}

/// Replaces the slice's dependencies. Unknown ids → `not_found`; itself → `invalid_input`;
/// a cycle → `dependency_cycle`, naming it.
#[tauri::command]
#[specta::specta]
pub async fn set_dependencies(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
    depends_on: Vec<String>,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.set_dependencies(&project_id, &slice_id, depends_on, expected_revision)
    })
    .await
}

/// `queued → in_progress` needs every dependency done (`dependencies_not_done`), unless
/// `override` with a non-empty `reason`, which is recorded as a decision.
#[tauri::command]
#[specta::specta]
pub async fn set_slice_status(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
    status: SliceStatus,
    options: StatusOptions,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.set_slice_status(&project_id, &slice_id, status, options, expected_revision)
    })
    .await
}

/// Turns the slice's `workspaceAccess` on or off: an agent launched on it afterwards may
/// also write the root workspace files (`package.json`, `pnpm-workspace.yaml`,
/// `pnpm-lock.yaml`, `tsconfig.base.json`, `.gitignore`, `.npmrc`).
#[tauri::command]
#[specta::specta]
pub async fn set_slice_workspace_access(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
    value: bool,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.set_slice_workspace_access(&project_id, &slice_id, value, expected_revision)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn add_decision(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
    text: String,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.add_decision(&project_id, &slice_id, &text, expected_revision)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn add_open_question(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
    text: String,
    expected_revision: Option<u32>,
) -> Result<PlanAdded, AppError> {
    blocking(&store, move |s| {
        s.add_open_question(&project_id, &slice_id, &text, expected_revision)
    })
    .await
}

/// Removes the question and records "question → resolution" as a decision.
#[tauri::command]
#[specta::specta]
pub async fn resolve_open_question(
    store: Store<'_>,
    project_id: String,
    slice_id: String,
    question_id: String,
    resolution: String,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.resolve_open_question(
            &project_id,
            &slice_id,
            &question_id,
            &resolution,
            expected_revision,
        )
    })
    .await
}

/// Added as `proposed`; `unsourced` if the excerpt isn't in `docs/brain-dump.md`.
#[tauri::command]
#[specta::specta]
pub async fn add_requirement(
    store: Store<'_>,
    project_id: String,
    requirement: NewRequirement,
    expected_revision: Option<u32>,
) -> Result<PlanAdded, AppError> {
    blocking(&store, move |s| {
        s.add_requirement(&project_id, requirement, expected_revision)
    })
    .await
}

/// Changing a confirmed requirement's text, kind or excerpt makes it `proposed` again.
#[tauri::command]
#[specta::specta]
pub async fn update_requirement(
    store: Store<'_>,
    project_id: String,
    requirement_id: String,
    patch: RequirementPatch,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.update_requirement(&project_id, &requirement_id, patch, expected_revision)
    })
    .await
}

/// `sliceId: null` unassigns it, which also makes it `proposed`.
#[tauri::command]
#[specta::specta]
pub async fn assign_requirement(
    store: Store<'_>,
    project_id: String,
    requirement_id: String,
    slice_id: Option<String>,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.assign_requirement(&project_id, &requirement_id, slice_id, expected_revision)
    })
    .await
}

/// Only a `constraint` can be project-wide (`invalid_input`). On clears its slice; off leaves
/// it unassigned. Its state (proposed or confirmed) is unchanged either way.
#[tauri::command]
#[specta::specta]
pub async fn set_requirement_project_wide(
    store: Store<'_>,
    project_id: String,
    requirement_id: String,
    value: bool,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.set_requirement_project_wide(&project_id, &requirement_id, value, expected_revision)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_requirement(
    store: Store<'_>,
    project_id: String,
    requirement_id: String,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.remove_requirement(&project_id, &requirement_id, expected_revision)
    })
    .await
}

/// `requirement_unassigned` while it has no slice and isn't project-wide.
#[tauri::command]
#[specta::specta]
pub async fn confirm_requirement(
    store: Store<'_>,
    project_id: String,
    requirement_id: String,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.confirm_requirement(&project_id, &requirement_id, expected_revision)
    })
    .await
}

/// Confirms every assigned or project-wide requirement; unassigned ones stay `proposed`.
#[tauri::command]
#[specta::specta]
pub async fn confirm_all(
    store: Store<'_>,
    project_id: String,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.confirm_all(&project_id, expected_revision)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn unconfirm_requirement(
    store: Store<'_>,
    project_id: String,
    requirement_id: String,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.unconfirm_requirement(&project_id, &requirement_id, expected_revision)
    })
    .await
}

/// `anchoredTo` must be a slice id.
#[tauri::command]
#[specta::specta]
pub async fn add_deferred(
    store: Store<'_>,
    project_id: String,
    deferred: NewDeferred,
    expected_revision: Option<u32>,
) -> Result<PlanAdded, AppError> {
    blocking(&store, move |s| {
        s.add_deferred(&project_id, deferred, expected_revision)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_deferred(
    store: Store<'_>,
    project_id: String,
    deferred_id: String,
    expected_revision: Option<u32>,
) -> Result<Plan, AppError> {
    blocking(&store, move |s| {
        s.remove_deferred(&project_id, &deferred_id, expected_revision)
    })
    .await
}

/// Replaces the plan with the last generated roadmap (`.foundry/roadmap.json`), everything
/// `proposed`, in one mutation. `confirmed_requirements` while any requirement is confirmed,
/// unless `discardConfirmed`. Call after `generate_roadmap` succeeds.
#[tauri::command]
#[specta::specta]
pub async fn load_roadmap(
    store: Store<'_>,
    project_id: String,
    options: LoadRoadmapOptions,
    expected_revision: Option<u32>,
) -> Result<RoadmapLoaded, AppError> {
    blocking(&store, move |s| {
        s.load_roadmap(&project_id, options, expected_revision)
    })
    .await
}
