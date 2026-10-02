pub mod core;
pub mod features;

#[cfg(test)]
mod testing;

use std::path::Path;
use std::sync::Arc;

use specta_typescript::Typescript;
use tauri::{Manager, Runtime};
use tauri_specta::{collect_commands, collect_events};

use crate::core::git::Git;
use crate::core::repos::RepoIndex;
use crate::core::AppError;
use crate::features::{app, bakery, folders, projects};

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
            projects::commands::list_projects,
            projects::commands::create_repository,
            projects::commands::link_repository,
            projects::commands::update_project,
            projects::commands::open_project,
            projects::commands::active_project,
            projects::commands::relink_project,
            projects::commands::archive_project,
            projects::commands::unarchive_project,
            projects::commands::remove_project,
            projects::commands::delete_repository,
            projects::commands::get_document,
            projects::commands::save_document,
            projects::commands::get_settings,
            projects::commands::update_settings,
            bakery::commands::bakery_list,
            bakery::commands::bakery_add_from_paths,
            bakery::commands::bakery_add_from_bytes,
            bakery::commands::bakery_update,
            bakery::commands::bakery_move,
            bakery::commands::bakery_remove,
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

/// The projects store, wired so removing or deleting a repo revokes the asset-protocol access
/// the Bakery granted in it. Tauri can't remove a granted folder, so it's forbidden instead (forbids
/// win over grants). A forbid can't be undone either: it lasts until the app restarts.
pub fn projects_store<R: Runtime>(
    app: &tauri::AppHandle<R>,
    data_dir: std::path::PathBuf,
    git: Git,
    repos: Arc<RepoIndex>,
) -> Result<projects::ProjectsStore, AppError> {
    let scope = app.asset_protocol_scope();
    Ok(
        projects::ProjectsStore::load(data_dir, git, repos)?.with_released_hook(move |repo| {
            if let Err(err) = scope.forbid_directory(bakery::image_dir(repo), true) {
                tracing::warn!(error = %err, "could not revoke asset access to a released repo");
            }
        }),
    )
}

/// The Bakery store, wired so the UI can display a project's images through the asset
/// protocol: each `docs/bakery/` folder is added to the scope (not recursively) once a Bakery
/// command touches it. The static scope in `tauri.conf.json` is empty.
pub fn bakery_store<R: Runtime>(
    app: &tauri::AppHandle<R>,
    repos: Arc<RepoIndex>,
) -> bakery::BakeryStore {
    let scope = app.asset_protocol_scope();
    bakery::BakeryStore::new(repos).with_dir_hook(move |dir| {
        if let Err(err) = scope.allow_directory(dir, false) {
            tracing::warn!(error = %err, "could not allow bakery folder for the asset protocol");
        }
    })
}

/// Wires commands, events and setup onto a Tauri builder (real or mock runtime).
pub fn app_builder<R: Runtime>(builder: tauri::Builder<R>) -> tauri::Builder<R> {
    let specta = specta_builder::<R>();
    builder
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            specta.mount_events(app);
            let data_dir = app.path().app_data_dir()?;
            let repos = Arc::new(RepoIndex::default());
            app.manage(Arc::new(projects_store(
                app.handle(),
                data_dir,
                Git::system(),
                Arc::clone(&repos),
            )?));
            app.manage(Arc::new(bakery_store(app.handle(), repos)));
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

    /// A linked project with one Bakery image, wired like the real app.
    struct Linked {
        app: tauri::App<tauri::test::MockRuntime>,
        projects: projects::ProjectsStore,
        id: String,
        folder: std::path::PathBuf,
        image: std::path::PathBuf,
        png: Vec<u8>,
        _dirs: (crate::testing::TestRepo, crate::testing::TestRepo),
    }

    fn linked_with_image() -> Linked {
        use std::io::Cursor;

        use crate::features::projects::LinkRepositoryInput;
        use crate::testing::TestRepo;
        use tauri::test::{mock_builder, mock_context, noop_assets};

        let app = app_builder(mock_builder())
            .build(mock_context(noop_assets()))
            .unwrap();
        let data = TestRepo::new();
        let parent = TestRepo::new();
        let folder = parent.path().join("project");
        std::fs::create_dir(&folder).unwrap();
        let repos = Arc::new(RepoIndex::default());
        let projects = projects_store(
            app.handle(),
            data.path().into(),
            Git::system(),
            Arc::clone(&repos),
        )
        .unwrap();
        let bakery = bakery_store(app.handle(), repos);

        let id = projects
            .link_repository(LinkRepositoryInput {
                folder: folder.to_string_lossy().into_owned(),
            })
            .unwrap()
            .project
            .id;
        let mut png = Vec::new();
        image::DynamicImage::new_rgb8(4, 4)
            .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        let added = bakery.add_from_bytes(&id, "a.png", &png).unwrap();
        let image = Path::new(&added.bakery.dir).join(&added.bakery.items[0].file);
        assert!(
            app.asset_protocol_scope().is_allowed(&image),
            "granted while the project is registered"
        );
        Linked {
            app,
            projects,
            id,
            folder,
            image,
            png,
            _dirs: (data, parent),
        }
    }

    /// Deleting a repo revokes the asset-protocol access its Bakery was granted.
    #[cfg(windows)]
    #[test]
    fn delete_repository_revokes_asset_access_to_its_bakery() {
        let t = linked_with_image();
        let scope = t.app.asset_protocol_scope();

        t.projects.archive_project(&t.id).unwrap();
        t.projects.delete_repository(&t.id).unwrap();
        assert!(!scope.is_allowed(&t.image), "revoked after delete");

        // Restored from the Recycle Bin to the same path: still refused until a restart,
        // because a forbid can't be lifted at runtime.
        std::fs::create_dir_all(t.image.parent().unwrap()).unwrap();
        std::fs::write(&t.image, &t.png).unwrap();
        assert!(scope.is_forbidden(&t.image));
        assert!(!scope.is_allowed(&t.image));
    }

    /// Removing (unregistering) a repo revokes its Bakery's asset access too, though the
    /// files stay on disk.
    #[test]
    fn remove_project_revokes_asset_access_to_its_bakery() {
        use crate::features::projects::LinkRepositoryInput;

        let t = linked_with_image();
        let scope = t.app.asset_protocol_scope();

        t.projects.archive_project(&t.id).unwrap();
        t.projects.remove_project(&t.id).unwrap();

        assert_eq!(
            std::fs::read(&t.image).unwrap(),
            t.png,
            "file still on disk"
        );
        assert!(scope.is_forbidden(&t.image));
        assert!(!scope.is_allowed(&t.image), "revoked after remove");

        // Linked again in the same session: still refused until a restart.
        t.projects
            .link_repository(LinkRepositoryInput {
                folder: t.folder.to_string_lossy().into_owned(),
            })
            .unwrap();
        assert!(!scope.is_allowed(&t.image));
    }

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
            ts.contains("archiveProject"),
            "archiveProject wrapper missing"
        );
        assert!(
            ts.contains("unarchiveProject"),
            "unarchiveProject wrapper missing"
        );
        assert!(
            ts.contains("deleteRepository"),
            "deleteRepository wrapper missing"
        );
        assert!(ts.contains("archived: boolean"), "Project.archived missing");
        assert!(ts.contains("\"not_archived\""), "not_archived code missing");
        for name in [
            "bakeryList",
            "bakeryAddFromPaths",
            "bakeryAddFromBytes",
            "bakeryUpdate",
            "bakeryMove",
            "bakeryRemove",
            "export type BakeryState",
            "unlabeledCount: number",
            "\"bakery_full\"",
        ] {
            assert!(ts.contains(name), "{name} missing from bindings");
        }
        assert!(
            ts.contains("ts: number,"),
            "event ts must export as a plain number"
        );
    }
}
