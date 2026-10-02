use std::sync::Arc;

use tauri::State;

use super::{BakeryAddResult, BakeryPatch, BakeryState, BakeryStore, Coord, DropPoint};
use crate::core::AppError;

type Store<'a> = State<'a, Arc<BakeryStore>>;

/// Runs a store call off the async runtime: decoding and file I/O block.
async fn blocking<T: Send + 'static>(
    store: &Arc<BakeryStore>,
    call: impl FnOnce(&BakeryStore) -> Result<T, AppError> + Send + 'static,
) -> Result<T, AppError> {
    let store = Arc::clone(store);
    tauri::async_runtime::spawn_blocking(move || call(&store))
        .await
        .map_err(|err| AppError::internal(format!("bakery task failed: {err}")))?
}

/// Every image on the project's canvas, ordered by `NN`.
#[tauri::command]
#[specta::specta]
pub async fn bakery_list(store: Store<'_>, project_id: String) -> Result<BakeryState, AppError> {
    blocking(&store, move |s| s.list(&project_id)).await
}

/// Adds dropped files (absolute paths). All or nothing. PNG, JPEG or WebP, 25 MB max each;
/// `bakery_full` past the cap. The first card lands at `dropPoint`, the rest cascade from it.
#[tauri::command]
#[specta::specta]
pub async fn bakery_add_from_paths(
    store: Store<'_>,
    project_id: String,
    paths: Vec<String>,
    drop_point: Option<DropPoint>,
) -> Result<BakeryAddResult, AppError> {
    blocking(&store, move |s| {
        s.add_from_paths(&project_id, &paths, drop_point)
    })
    .await
}

/// Adds a pasted image. `name` must be a plain file name; the format is read from the bytes.
#[tauri::command]
#[specta::specta]
pub async fn bakery_add_from_bytes(
    store: Store<'_>,
    project_id: String,
    name: String,
    bytes: Vec<u8>,
) -> Result<BakeryAddResult, AppError> {
    blocking(&store, move |s| {
        s.add_from_bytes(&project_id, &name, &bytes)
    })
    .await
}

/// Changes notes. A label change renames the file and updates layout and manifest.
#[tauri::command]
#[specta::specta]
pub async fn bakery_update(
    store: Store<'_>,
    project_id: String,
    id: String,
    patch: BakeryPatch,
) -> Result<BakeryState, AppError> {
    blocking(&store, move |s| s.update(&project_id, &id, patch)).await
}

/// Moves, resizes or restacks a card. Layout only: `manifest.md` is never rewritten.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)]
pub async fn bakery_move(
    store: Store<'_>,
    project_id: String,
    id: String,
    x: Coord,
    y: Coord,
    w: Option<Coord>,
    h: Option<Coord>,
    z: Option<i32>,
) -> Result<BakeryState, AppError> {
    blocking(&store, move |s| {
        s.move_item(&project_id, &id, x, y, w, h, z)
    })
    .await
}

/// Removes the card and deletes its image file (only ever inside `docs/bakery/`).
#[tauri::command]
#[specta::specta]
pub async fn bakery_remove(
    store: Store<'_>,
    project_id: String,
    id: String,
) -> Result<BakeryState, AppError> {
    blocking(&store, move |s| s.remove(&project_id, &id)).await
}
