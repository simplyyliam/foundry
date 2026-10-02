pub mod core;
pub mod features;

#[cfg(test)]
mod testing;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use specta_typescript::Typescript;
use tauri::{Manager, Runtime};
use tauri_specta::{collect_commands, collect_events};

use crate::core::events;
use crate::core::git::Git;
use crate::core::repos::RepoIndex;
use crate::core::sessions::SessionRegistry;
use crate::core::AppError;
#[cfg(test)]
use crate::core::ErrorCode;
use crate::features::{agent_sessions, app, bakery, folders, projects, roadmap};

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
            agent_sessions::commands::agent_list,
            agent_sessions::commands::agent_start,
            agent_sessions::commands::agent_write,
            agent_sessions::commands::agent_resize,
            agent_sessions::commands::agent_stop,
            agent_sessions::commands::agent_snapshot,
            roadmap::commands::generate_roadmap,
            roadmap::commands::cancel_generation,
            roadmap::commands::get_roadmap,
            roadmap::commands::claude_status,
        ])
        .events(collect_events![
            app::AppReady,
            agent_sessions::AgentOutput,
            agent_sessions::AgentStatus,
            roadmap::RoadmapProgress
        ])
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
    sessions: Arc<SessionRegistry>,
) -> Result<projects::ProjectsStore, AppError> {
    let scope = app.asset_protocol_scope();
    Ok(
        projects::ProjectsStore::load(data_dir, git, repos, sessions)?.with_released_hook(move |repo| {
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

/// Sends session events to the webview.
struct TauriSessionEvents<R: Runtime> {
    app: tauri::AppHandle<R>,
}

impl<R: Runtime> agent_sessions::SessionEvents for TauriSessionEvents<R> {
    // emit() already logs failures; a missed frame isn't worth more.
    fn output(&self, event: agent_sessions::AgentOutput) {
        let _ = events::emit(&self.app, &event);
    }

    fn status(&self, event: agent_sessions::AgentStatus) {
        let _ = events::emit(&self.app, &event);
    }
}

/// The session manager, emitting to the webview, with the cap read from the
/// `maxConcurrentAgents` setting on every start.
pub fn session_manager<R: Runtime>(
    app: &tauri::AppHandle<R>,
    sessions: Arc<SessionRegistry>,
    projects: Arc<projects::ProjectsStore>,
) -> agent_sessions::SessionManager {
    agent_sessions::SessionManager::new(
        sessions,
        move || {
            projects
                .get_settings()
                .map_or(1, |settings| settings.max_concurrent_agents)
        },
        Arc::new(TauriSessionEvents { app: app.clone() }),
    )
}

impl<R: Runtime> roadmap::ProgressSink for TauriSessionEvents<R> {
    fn progress(&self, event: roadmap::RoadmapProgress) {
        let _ = events::emit(&self.app, &event);
    }
}

/// The roadmap store, running the real `claude -p` after the subscription preflight, with
/// `claudePath`, `defaultModel` and `generationTimeoutMs` read from settings on every run.
pub fn roadmap_store<R: Runtime>(
    app: &tauri::AppHandle<R>,
    repos: Arc<RepoIndex>,
    projects: Arc<projects::ProjectsStore>,
) -> roadmap::RoadmapStore {
    let settings = move || projects.get_settings().unwrap_or_default();
    let (for_status, for_timeout) = (settings.clone(), settings.clone());
    roadmap::RoadmapStore::new(
        repos,
        move || {
            let settings = settings();
            let program = core::claude::preflight(settings.claude_path.as_deref())?;
            let runner = roadmap::ClaudeRunner::new(program).with_model(settings.default_model);
            Ok(Arc::new(runner) as Arc<dyn roadmap::LlmRunner>)
        },
        move || core::claude::status(for_status().claude_path.as_deref()),
        Arc::new(TauriSessionEvents { app: app.clone() }),
    )
    .with_timeout_source(move || {
        Duration::from_millis(u64::from(for_timeout().generation_timeout_ms))
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
            let sessions = Arc::new(SessionRegistry::default());
            let projects = Arc::new(projects_store(
                app.handle(),
                data_dir,
                Git::system(),
                Arc::clone(&repos),
                Arc::clone(&sessions),
            )?);
            app.manage(Arc::new(session_manager(
                app.handle(),
                sessions,
                Arc::clone(&projects),
            )));
            app.manage(Arc::new(roadmap_store(
                app.handle(),
                Arc::clone(&repos),
                Arc::clone(&projects),
            )));
            app.manage(projects);
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

    let app = match app_builder(builder).build(tauri::generate_context!()) {
        Ok(app) => app,
        Err(err) => {
            tracing::error!(%err, "error while building tauri application");
            std::process::exit(1);
        }
    };
    app.run(|handle, event| {
        // Kill every agent's process tree on the way out. (Each session's Job Object would
        // also kill them when Foundry's handles close, even on a crash.)
        if let tauri::RunEvent::Exit = event {
            if let Some(manager) = handle.try_state::<Arc<agent_sessions::SessionManager>>() {
                manager.shutdown();
            }
        }
    });
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
            Default::default(),
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

    /// P1's archive_project refuses while the project has a live agent session (the real
    /// fake-agent process, wired exactly as the app wires it), and changes nothing.
    #[test]
    fn archive_project_refuses_while_an_agent_session_runs() {
        use crate::features::agent_sessions::{HookEnv, LaunchSpec};
        use crate::features::projects::LinkRepositoryInput;
        use crate::testing::{fake_agent_path, TestRepo};
        use tauri::test::{mock_builder, mock_context, noop_assets};

        let app = app_builder(mock_builder())
            .build(mock_context(noop_assets()))
            .unwrap();
        // A mock app doesn't run setup, where the real app registers its typed events.
        specta_builder::<tauri::test::MockRuntime>().mount_events(&app);
        let (data, repo) = (TestRepo::new(), TestRepo::new());
        let sessions = Arc::new(SessionRegistry::default());
        let projects = Arc::new(
            projects_store(
                app.handle(),
                data.path().into(),
                Git::system(),
                Default::default(),
                Arc::clone(&sessions),
            )
            .unwrap(),
        );
        let manager = session_manager(app.handle(), sessions, Arc::clone(&projects));
        let id = projects
            .link_repository(LinkRepositoryInput {
                folder: repo.path().to_string_lossy().into_owned(),
            })
            .unwrap()
            .project
            .id;
        projects.open_project(&id).unwrap();
        manager
            .start(LaunchSpec {
                agent_id: "builder".into(),
                project_id: id.clone(),
                program: fake_agent_path(),
                args: vec!["--sleep-ms".into(), "60000".into()],
                cwd: repo.path().into(),
                hook: HookEnv {
                    port: 1,
                    token: "t".into(),
                },
                cols: 80,
                rows: 24,
            })
            .unwrap();

        let refused = projects.archive_project(&id).unwrap_err();

        assert_eq!(refused.code, ErrorCode::SessionRunning);
        let listed = projects.list().unwrap();
        assert!(!listed[0].archived, "nothing changed");
        assert_eq!(
            projects.active_project().unwrap().unwrap().id,
            id,
            "still open"
        );

        manager.stop("builder", true).unwrap();
        assert!(
            projects.archive_project(&id).unwrap().archived,
            "allowed once stopped"
        );
    }

    /// A project whose settings point `claudePath` at fake-claude, wired like the real app.
    struct Generation {
        roadmap: roadmap::RoadmapStore,
        projects: Arc<projects::ProjectsStore>,
        id: String,
        repo: crate::testing::TestRepo,
        _dirs: (crate::testing::TestRepo, crate::testing::TestRepo),
    }

    fn generation(brain_dump: &str, tweak: impl FnOnce(&mut projects::Settings)) -> Generation {
        use crate::features::projects::LinkRepositoryInput;
        use crate::testing::{install_fake_claude, TestRepo};
        use tauri::test::{mock_builder, mock_context, noop_assets};

        let app = app_builder(mock_builder())
            .build(mock_context(noop_assets()))
            .unwrap();
        specta_builder::<tauri::test::MockRuntime>().mount_events(&app);
        let (data, bin, repo) = (TestRepo::new(), TestRepo::new(), TestRepo::new());
        let claude = install_fake_claude(bin.path());
        let repos = Arc::new(RepoIndex::default());
        let projects = Arc::new(
            projects_store(
                app.handle(),
                data.path().into(),
                Git::system(),
                Arc::clone(&repos),
                Default::default(),
            )
            .unwrap(),
        );
        let mut settings = projects::Settings {
            claude_path: Some(claude.to_string_lossy().into_owned()),
            ..Default::default()
        };
        tweak(&mut settings);
        projects.update_settings(settings).unwrap();
        let id = projects
            .link_repository(LinkRepositoryInput {
                folder: repo.path().to_string_lossy().into_owned(),
            })
            .unwrap()
            .project
            .id;
        repo.write("docs/brain-dump.md", brain_dump);
        let roadmap = roadmap_store(app.handle(), repos, Arc::clone(&projects));
        Generation {
            roadmap,
            projects,
            id,
            repo,
            _dirs: (data, bin),
        }
    }

    /// `generationTimeoutMs` is honored: a 1.5 s limit stops the run long before 5 minutes.
    #[test]
    fn a_configured_generation_timeout_is_honored() {
        let g = generation("FAKE:SLEEP", |s| s.generation_timeout_ms = 1500);
        let started = std::time::Instant::now();

        let err = g.roadmap.generate(&g.id).unwrap_err();

        let took = started.elapsed();
        assert_eq!(err.code, ErrorCode::TimedOut);
        assert!(
            took >= Duration::from_millis(1500),
            "stopped early: {took:?}"
        );
        assert!(
            took < Duration::from_secs(15),
            "the setting was ignored: {took:?}"
        );

        // Read on every run: a new value applies to the next generation.
        let settings = projects::Settings {
            generation_timeout_ms: 400,
            ..g.projects.get_settings().unwrap()
        };
        g.projects.update_settings(settings).unwrap();
        let started = std::time::Instant::now();
        assert_eq!(
            g.roadmap.generate(&g.id).unwrap_err().code,
            ErrorCode::TimedOut
        );
        assert!(started.elapsed() < Duration::from_millis(1500));
    }

    /// `defaultModel` reaches `claude -p` as `--model <name>`; unset, no `--model` is passed.
    #[test]
    fn default_model_is_passed_to_claude_as_model_flag() {
        // FAKE:ARGS replies with the arguments, which fails validation twice, so the raw
        // reply (the argument list) lands in last-generation-error.txt.
        let g = generation("FAKE:ARGS", |s| s.default_model = Some("opus-test".into()));
        assert_eq!(
            g.roadmap.generate(&g.id).unwrap_err().code,
            ErrorCode::GenerationFailed
        );
        let args =
            std::fs::read_to_string(g.repo.path().join(".foundry/last-generation-error.txt"))
                .unwrap();
        assert!(
            args.ends_with(
                "--model
opus-test"
            ),
            "{args}"
        );

        let unset = generation("FAKE:ARGS", |_| {});
        unset.roadmap.generate(&unset.id).unwrap_err();
        let args =
            std::fs::read_to_string(unset.repo.path().join(".foundry/last-generation-error.txt"))
                .unwrap();
        assert!(!args.contains("--model"), "{args}");
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
            "agentList",
            "agentStart",
            "agentWrite",
            "agentResize",
            "agentStop",
            "agentSnapshot",
            "\"agent:output\"",
            "\"agent:status\"",
            "\"session_running\"",
            "\"too_many_sessions\"",
            "generateRoadmap",
            "cancelGeneration",
            "getRoadmap",
            "claudeStatus",
            "\"roadmap:progress\"",
            "\"not_signed_in\"",
            "\"wrong_auth_mode\"",
            "\"claude_not_found\"",
        ] {
            assert!(ts.contains(name), "{name} missing from bindings");
        }
        assert!(
            ts.contains("ts: number,"),
            "event ts must export as a plain number"
        );
    }
}
