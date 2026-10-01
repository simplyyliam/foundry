pub mod core;
pub mod features;

#[cfg(test)]
mod testing;

use std::path::Path;

use specta_typescript::Typescript;
use tauri::Runtime;
use tauri_specta::{collect_commands, collect_events};

use crate::core::AppError;
use crate::features::{app, folders};

/// Where the generated TypeScript bindings live. Never hand-edit that file.
pub const BINDINGS_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/lib/bindings.ts");

const BINDINGS_HEADER: &str = "// Regenerate from src-tauri with a debug run or `cargo test`.";

/// Every command and event in the contract. Register new ones here.
pub fn specta_builder<R: Runtime>() -> tauri_specta::Builder<R> {
    tauri_specta::Builder::<R>::new()
        .commands(collect_commands![
            app::commands::app_info,
            folders::commands::list_folders,
            folders::commands::folder_locations,
        ])
        .events(collect_events![app::AppReady])
}

pub fn export_bindings<R: Runtime>(
    builder: &tauri_specta::Builder<R>,
    path: &Path,
) -> Result<(), AppError> {
    builder
        .export(Typescript::default().header(BINDINGS_HEADER), path)
        .map_err(|err| AppError::internal(format!("failed to export bindings: {err}")))
}

/// Wires commands, events and setup onto a Tauri builder (real or mock runtime).
pub fn app_builder<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    let specta = specta_builder::<R>();
    builder
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            specta.mount_events(app);
            app::init(app.handle())?;
            Ok(())
        })
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[cfg(debug_assertions)]
    if let Err(err) = export_bindings(&specta_builder::<tauri::Wry>(), Path::new(BINDINGS_PATH)) {
        tracing::error!(%err, "bindings export failed");
    }

    let mut builder = tauri::Builder::default();
    if cfg!(debug_assertions) {
        builder = builder.plugin(
            tauri_plugin_log::Builder::default()
                .level(log::LevelFilter::Info)
                .build(),
        );
    }

    if let Err(err) = app_builder(builder).run(tauri::generate_context!()) {
        tracing::error!(%err, "error while running tauri application");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regenerates the real bindings file, then checks the contract is in it.
    #[test]
    fn export_bindings_regenerates_file_with_app_info() {
        let path = Path::new(BINDINGS_PATH);
        export_bindings(&specta_builder::<tauri::Wry>(), path).unwrap();

        let ts = std::fs::read_to_string(path).unwrap();
        assert!(ts.starts_with(BINDINGS_HEADER));
        assert!(ts.contains("\"app_info\""), "app_info command missing");
        assert!(ts.contains("appInfo"), "appInfo wrapper missing");
        assert!(ts.contains("\"app:ready\""), "app:ready event missing");
        assert!(ts.contains("export type AppError"), "AppError type missing");
        assert!(ts.contains("export type AppInfo"), "AppInfo type missing");
        assert!(
            ts.contains("ts: number,"),
            "event ts must export as a plain number"
        );
    }
}
