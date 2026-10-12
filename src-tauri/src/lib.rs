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
use crate::core::hook::HookEndpointCell;
use crate::core::repos::RepoIndex;
use crate::core::sessions::SessionRegistry;
use crate::core::AppError;
#[cfg(test)]
use crate::core::ErrorCode;
use crate::features::{
    agent_events, agent_sessions, app, bakery, design, folders, hooks, plan, projects, prompts,
    roadmap, setup_commit, slice_commit, team, verification,
};

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
            plan::commands::get_plan,
            plan::commands::plan_summary,
            plan::commands::add_slice,
            plan::commands::update_slice,
            plan::commands::remove_slice,
            plan::commands::set_dependencies,
            plan::commands::set_slice_status,
            plan::commands::set_slice_workspace_access,
            plan::commands::add_decision,
            plan::commands::add_open_question,
            plan::commands::resolve_open_question,
            plan::commands::add_requirement,
            plan::commands::update_requirement,
            plan::commands::assign_requirement,
            plan::commands::set_requirement_project_wide,
            plan::commands::set_requirement_checks,
            plan::commands::accept_requirement_check,
            plan::commands::remove_requirement,
            plan::commands::confirm_requirement,
            plan::commands::confirm_all,
            plan::commands::unconfirm_requirement,
            plan::commands::add_deferred,
            plan::commands::remove_deferred,
            plan::commands::load_roadmap,
            plan::commands::list_roadmap_conflicts,
            plan::commands::resolve_roadmap_conflict,
            plan::commands::unresolve_roadmap_conflict,
            team::commands::role_handbook_list,
            team::commands::role_handbook_add,
            team::commands::role_handbook_add_from_paths,
            team::commands::role_handbook_update,
            team::commands::role_handbook_remove,
            team::commands::compose_team,
            team::commands::confirm_team,
            team::commands::add_agent_manually,
            team::commands::get_team,
            team::commands::builtin_avatar_list,
            team::commands::set_agent_avatar,
            team::commands::agent_avatar,
            design::commands::design_library_list,
            design::commands::design_library_add_from_paths,
            design::commands::design_library_update,
            design::commands::design_library_remove,
            design::commands::agents_placeholders,
            design::commands::confirm_placeholder,
            design::commands::generate_agents,
            prompts::commands::generate_handoff,
            prompts::commands::generate_task_prompt,
            prompts::commands::generate_fix_prompts,
            prompts::commands::launch_message,
            prompts::commands::list_prompts,
            prompts::commands::read_prompt,
            agent_events::commands::agent_events,
            agent_events::commands::agent_event_counts,
            agent_events::commands::agent_mark_read,
            hooks::commands::hooks_install,
            hooks::commands::hooks_uninstall,
            hooks::commands::hooks_status,
            verification::commands::get_test_commands,
            verification::commands::set_test_commands,
            verification::commands::confirm_test_commands,
            verification::commands::get_report,
            verification::commands::list_reports,
            verification::commands::verify_slice,
            verification::commands::check_verify_slice,
            verification::commands::get_verification,
            verification::commands::cancel_verification,
            verification::commands::manager_digest,
            verification::commands::verification_guidance,
            verification::commands::write_digest_file,
            setup_commit::commands::commit_setup,
            setup_commit::commands::setup_status,
            slice_commit::commands::commit_status,
            slice_commit::commands::commit_slice,
        ])
        .events(collect_events![
            app::AppReady,
            agent_sessions::AgentOutput,
            agent_sessions::AgentStatus,
            roadmap::RoadmapProgress,
            plan::PlanChanged,
            agent_events::AgentEvent,
            verification::VerifyProgress,
            verification::VerifySkipped,
            verification::VerifySkipCleared
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

/// The session manager, emitting to the webview. Read from settings on every launch: the
/// cap (`maxConcurrentAgents`), and for the preflight `claudePath` and `defaultModel`. The
/// hook server's port and token come from `endpoint`.
pub fn session_manager<R: Runtime>(
    app: &tauri::AppHandle<R>,
    sessions: Arc<SessionRegistry>,
    repos: Arc<RepoIndex>,
    projects: Arc<projects::ProjectsStore>,
    endpoint: Arc<HookEndpointCell>,
) -> agent_sessions::SessionManager {
    let for_launch = Arc::clone(&projects);
    agent_sessions::SessionManager::new(
        sessions,
        move || {
            projects
                .get_settings()
                .map_or(1, |settings| settings.max_concurrent_agents)
        },
        Arc::new(TauriSessionEvents { app: app.clone() }),
    )
    .with_launcher(repos, endpoint, move || {
        let settings = for_launch.get_settings().unwrap_or_default();
        Ok(agent_sessions::ClaudeLaunch {
            program: core::claude::preflight(settings.claude_path.as_deref())?,
            model: settings.default_model,
        })
    })
}

impl<R: Runtime> plan::PlanEvents for TauriSessionEvents<R> {
    fn changed(&self, event: plan::PlanChanged) {
        let _ = events::emit(&self.app, &event);
    }
}

impl<R: Runtime> agent_events::AgentEventSink for TauriSessionEvents<R> {
    fn event(&self, event: agent_events::AgentEvent) {
        let _ = events::emit(&self.app, &event);
    }
}

/// `osNotificationsEnabled` and `osNotificationSeverity` as the events store reads them.
pub fn notify_policy(settings: &projects::Settings) -> agent_events::NotifyPolicy {
    use agent_events::Severity;
    use projects::NotificationSeverity;
    agent_events::NotifyPolicy {
        enabled: settings.os_notifications_enabled,
        min_severity: match settings.os_notification_severity {
            NotificationSeverity::All => Severity::Info,
            NotificationSeverity::WarnAndDanger => Severity::Warn,
            // A value that isn't allowed never reaches settings; be strict if it does.
            NotificationSeverity::Danger | NotificationSeverity::Invalid(_) => Severity::Danger,
        },
    }
}

/// Desktop notifications through the notification plugin, titled with the agent and project.
/// Without the plugin (tests on the mock runtime) nothing is shown.
struct TauriNotifier<R: Runtime> {
    app: tauri::AppHandle<R>,
    projects: Arc<projects::ProjectsStore>,
}

impl<R: Runtime> agent_events::OsNotifier for TauriNotifier<R> {
    fn show(&self, event: &agent_events::AgentEvent) {
        let Some(plugin) = self
            .app
            .try_state::<tauri_plugin_notification::Notification<R>>()
        else {
            return;
        };
        let project = self
            .projects
            .list()
            .ok()
            .and_then(|all| all.into_iter().find(|p| p.id == event.project_id))
            .map(|p| p.name)
            .unwrap_or_else(|| event.project_id.clone());
        let shown = plugin
            .builder()
            .title(format!("{} \u{b7} {project}", event.agent_id))
            .body(&event.title)
            .show();
        if let Err(err) = shown {
            tracing::warn!(error = %err, "desktop notification failed");
        }
    }
}

/// The agent-events store emitting `agent:event` and desktop notifications (per settings,
/// read on every event), its quiet timer, and the hook server on 127.0.0.1, whose port and
/// token are published to `endpoint` for agent sessions to read. A server that can't bind is
/// logged, not fatal: the app runs without hook events, and `endpoint` stays empty.
pub fn agent_events_store<R: Runtime>(
    app: &tauri::AppHandle<R>,
    repos: Arc<RepoIndex>,
    sessions: Arc<SessionRegistry>,
    projects: Arc<projects::ProjectsStore>,
    endpoint: Arc<HookEndpointCell>,
) -> (
    Arc<agent_events::AgentEventsStore>,
    Option<agent_events::server::HookServer>,
) {
    let for_policy = Arc::clone(&projects);
    let store = Arc::new(
        agent_events::AgentEventsStore::new(
            repos,
            sessions,
            Arc::new(TauriSessionEvents { app: app.clone() }),
        )
        .with_notifications(
            move || notify_policy(&for_policy.get_settings().unwrap_or_default()),
            Arc::new(TauriNotifier {
                app: app.clone(),
                projects,
            }),
        ),
    );
    // Runs for the app's lifetime.
    let _timer = store.spawn_quiet_timer(agent_events::QUIET_CHECK_EVERY);
    let server = agent_events::server::start(Arc::clone(&store), endpoint)
        .map_err(|err| tracing::error!(error = %err, "hook server failed to start"))
        .ok();
    (store, server)
}

/// The hooks installer: the helper next to the app's executable, Claude Code found as agent
/// sessions find it (`claudePath`, else PATH).
pub fn hooks_store(
    repos: Arc<RepoIndex>,
    projects: Arc<projects::ProjectsStore>,
) -> hooks::HooksStore {
    hooks::HooksStore::new(repos, Git::system(), core::hook::helper_path, move || {
        let settings = projects.get_settings().unwrap_or_default();
        core::claude::find(
            settings.claude_path.as_deref(),
            std::env::var_os("PATH").as_deref(),
        )
    })
}

impl<R: Runtime> verification::VerifyEvents for TauriSessionEvents<R> {
    fn progress(&self, event: verification::VerifyProgress) {
        let _ = events::emit(&self.app, &event);
    }

    fn skipped(&self, event: verification::VerifySkipped) {
        let _ = events::emit(&self.app, &event);
    }

    fn skip_cleared(&self, event: verification::VerifySkipCleared) {
        let _ = events::emit(&self.app, &event);
    }
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

/// The `claude -p` runner, after the subscription preflight, with `claudePath` and
/// `defaultModel` read from settings on every run.
fn print_runner(
    projects: &Arc<projects::ProjectsStore>,
) -> impl Fn() -> Result<Arc<dyn core::llm::LlmRunner>, AppError> + Send + Sync + 'static {
    let projects = Arc::clone(projects);
    move || {
        let settings = projects.get_settings().unwrap_or_default();
        let program = core::claude::preflight(settings.claude_path.as_deref())?;
        let runner = core::llm::ClaudeRunner::new(program).with_model(settings.default_model);
        Ok(Arc::new(runner) as Arc<dyn core::llm::LlmRunner>)
    }
}

/// The team store: the Role Handbook in app data, `compose_team` on the real runner, its
/// time limit from `generationTimeoutMs`.
pub fn team_store(
    data_dir: &Path,
    repos: Arc<RepoIndex>,
    projects: Arc<projects::ProjectsStore>,
) -> team::TeamStore {
    let for_timeout = Arc::clone(&projects);
    team::TeamStore::new(data_dir, repos, print_runner(&projects), move || {
        Duration::from_millis(u64::from(
            for_timeout
                .get_settings()
                .unwrap_or_default()
                .generation_timeout_ms,
        ))
    })
}

/// The design store: the Design Library in app data, the template from `agentsTemplateId`.
pub fn design_store(
    data_dir: &Path,
    repos: Arc<RepoIndex>,
    projects: Arc<projects::ProjectsStore>,
) -> design::DesignStore {
    design::DesignStore::new(data_dir, repos, move || {
        projects
            .get_settings()
            .unwrap_or_default()
            .agents_template_id
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
            app.manage(Arc::new(verification::VerificationStore::new(
                Arc::clone(&repos),
                Arc::clone(&sessions),
                Git::system(),
                data_dir.clone(),
                Arc::new(TauriSessionEvents {
                    app: app.handle().clone(),
                }),
            )));
            let projects = Arc::new(projects_store(
                app.handle(),
                data_dir.clone(),
                Git::system(),
                Arc::clone(&repos),
                Arc::clone(&sessions),
            )?);
            let hook_endpoint = Arc::new(HookEndpointCell::default());
            let (agent_events, hook_server) = agent_events_store(
                app.handle(),
                Arc::clone(&repos),
                Arc::clone(&sessions),
                Arc::clone(&projects),
                Arc::clone(&hook_endpoint),
            );
            app.manage(agent_events);
            if let Some(server) = hook_server {
                app.manage(server);
            }
            app.manage(Arc::new(hooks_store(
                Arc::clone(&repos),
                Arc::clone(&projects),
            )));
            app.manage(Arc::new(setup_commit::SetupCommitter::new(
                Arc::clone(&repos),
                Arc::clone(&sessions),
                Git::system(),
            )));
            app.manage(Arc::new(slice_commit::SliceCommitter::new(
                Arc::clone(&repos),
                Arc::clone(&sessions),
                Git::system(),
            )));
            app.manage(Arc::new(session_manager(
                app.handle(),
                sessions,
                Arc::clone(&repos),
                Arc::clone(&projects),
                Arc::clone(&hook_endpoint),
            )));
            app.manage(hook_endpoint);
            app.manage(Arc::new(roadmap_store(
                app.handle(),
                Arc::clone(&repos),
                Arc::clone(&projects),
            )));
            app.manage(Arc::new(team_store(
                &data_dir,
                Arc::clone(&repos),
                Arc::clone(&projects),
            )));
            app.manage(Arc::new(design_store(
                &data_dir,
                Arc::clone(&repos),
                Arc::clone(&projects),
            )));
            app.manage(projects);
            app.manage(Arc::new(prompts::PromptsStore::new(Arc::clone(&repos))));
            app.manage(Arc::new(plan::PlanStore::new(
                Arc::clone(&repos),
                Git::system(),
                Arc::new(TauriSessionEvents {
                    app: app.handle().clone(),
                }),
            )));
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

    let mut builder = tauri::Builder::default().plugin(tauri_plugin_notification::init());
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
        let repos = Arc::new(RepoIndex::default());
        let projects = Arc::new(
            projects_store(
                app.handle(),
                data.path().into(),
                Git::system(),
                Arc::clone(&repos),
                Arc::clone(&sessions),
            )
            .unwrap(),
        );
        let manager = session_manager(
            app.handle(),
            sessions,
            repos,
            Arc::clone(&projects),
            Default::default(),
        );
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
                slice_id: None,
                workspace_access: false,
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

        manager.stop(&id, "builder", true).unwrap();
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
        // Well under the 5-minute default. (The clock also covers the preflight, a separate
        // `claude auth status` process, so no tighter bound holds on a loaded machine.)
        assert!(started.elapsed() < Duration::from_secs(15));
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

    // ---------- P7 follow-up: SessionManager::launch ----------

    /// A linked project, `claudePath` pointing at fake-claude, and the manager wired like the
    /// real app (real `core::claude::preflight`).
    struct Launch {
        manager: Arc<agent_sessions::SessionManager>,
        projects: Arc<projects::ProjectsStore>,
        sessions: Arc<SessionRegistry>,
        repos: Arc<RepoIndex>,
        endpoint: Arc<HookEndpointCell>,
        id: String,
        repo: crate::testing::TestRepo,
        bin: crate::testing::TestRepo,
        claude: std::path::PathBuf,
        _data: crate::testing::TestRepo,
        _app: tauri::App<tauri::test::MockRuntime>,
    }

    impl Drop for Launch {
        fn drop(&mut self) {
            self.manager.shutdown();
        }
    }

    fn launch_env(tweak: impl FnOnce(&mut projects::Settings)) -> Launch {
        use crate::features::projects::LinkRepositoryInput;
        use crate::testing::{install_fake_claude, TestRepo};
        use tauri::test::{mock_builder, mock_context, noop_assets};

        let app = app_builder(mock_builder())
            .build(mock_context(noop_assets()))
            .unwrap();
        specta_builder::<tauri::test::MockRuntime>().mount_events(&app);
        let (data, bin, repo) = (TestRepo::new(), TestRepo::new(), TestRepo::new());
        let claude = install_fake_claude(bin.path());
        let (repos, sessions) = (
            Arc::new(RepoIndex::default()),
            Arc::new(SessionRegistry::default()),
        );
        let projects = Arc::new(
            projects_store(
                app.handle(),
                data.path().into(),
                Git::system(),
                Arc::clone(&repos),
                Arc::clone(&sessions),
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
        let endpoint = Arc::new(HookEndpointCell::default());
        endpoint.publish(core::hook::HookEndpoint {
            port: 4567,
            token: "tok-launch".into(),
        });
        let manager = Arc::new(session_manager(
            app.handle(),
            Arc::clone(&sessions),
            Arc::clone(&repos),
            Arc::clone(&projects),
            Arc::clone(&endpoint),
        ));
        Launch {
            manager,
            projects,
            sessions,
            repos,
            endpoint,
            id,
            repo,
            bin,
            claude,
            _data: data,
            _app: app,
        }
    }

    fn request(project_id: &str, agent_id: &str, message: &str) -> agent_sessions::LaunchRequest {
        agent_sessions::LaunchRequest {
            project_id: project_id.into(),
            agent_id: agent_id.into(),
            launch_message: message.into(),
            hook: agent_sessions::HookEnv {
                port: 4567,
                token: "tok-launch".into(),
            },
            slice_id: None,
            workspace_access: false,
            cols: 120,
            rows: 30,
        }
    }

    /// A confirmed team with `builder`, whose title goes into the launch message (fake-claude
    /// acts on `FAKE:ENV` / `FAKE:ARGS` in it), and the hand-off prompt file.
    fn with_team(l: &Launch, title: &str) {
        l.repo.write(
            "agents/roles.json",
            &serde_json::json!({
                "version": 1,
                "agents": [{
                    "id": "builder", "displayName": "Bob", "title": title,
                    "writeGlobs": ["src/**"], "protectedGlobs": ["AGENTS.md"]
                }]
            })
            .to_string(),
        );
        l.repo.write(".foundry/prompts/handoff.md", "# Hand-off\n");
    }

    fn handoff(slice_id: Option<&str>) -> agent_sessions::AgentStartOptions {
        agent_sessions::AgentStartOptions {
            prompt_file: Some(".foundry/prompts/handoff.md".into()),
            slice_id: slice_id.map(Into::into),
        }
    }

    /// Waits for fake-claude to write `name` next to itself, and parses it.
    fn written(l: &Launch, name: &str) -> serde_json::Value {
        let path = l.bin.path().join(name);
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(value) = serde_json::from_str(&text) {
                    return value;
                }
            }
            assert!(std::time::Instant::now() < deadline, "{name} never written");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    fn nothing_started(l: &Launch) {
        assert!(l.manager.list(&l.id).is_empty(), "a session was started");
        assert!(!l.sessions.is_project_active(&l.id));
        assert!(
            l.manager.snapshot(&l.id, "builder").is_empty(),
            "nothing ever ran"
        );
    }

    /// R-7-9: each preflight failure is refused with its own code before anything spawns.
    #[test]
    fn launch_refuses_cleanly_when_the_preflight_fails() {
        for (auth, code) in [
            (
                r#"{"loggedIn":false,"authMethod":"none"}"#,
                ErrorCode::NotSignedIn,
            ),
            (
                r#"{"loggedIn":true,"authMethod":"console","apiProvider":"firstParty"}"#,
                ErrorCode::WrongAuthMode,
            ),
            (
                r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"vertex"}"#,
                ErrorCode::WrongAuthMode,
            ),
        ] {
            let l = launch_env(|_| {});
            std::fs::write(l.bin.path().join("auth.json"), auth).unwrap();
            let err = l
                .manager
                .launch(request(&l.id, "builder", "hello"))
                .unwrap_err();
            assert_eq!(err.code, code, "{auth}");
            nothing_started(&l);
        }

        let missing = launch_env(|s| s.claude_path = Some(r"C:\nowhere\claude.exe".into()));
        let err = missing
            .manager
            .launch(request(&missing.id, "builder", "hello"))
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::ClaudeNotFound);
        nothing_started(&missing);
    }

    #[test]
    fn launch_refuses_an_archived_project_and_starts_nothing() {
        let l = launch_env(|_| {});
        l.projects.archive_project(&l.id).unwrap();

        let err = l
            .manager
            .launch(request(&l.id, "builder", "hello"))
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::ProjectArchived);
        assert_eq!(
            err.message,
            "unarchive this project before starting an agent in it"
        );
        nothing_started(&l);

        l.projects.unarchive_project(&l.id).unwrap();
        let info = l
            .manager
            .launch(request(&l.id, "builder", "hello"))
            .unwrap();
        assert_eq!(info.project_id, l.id);
        assert!(l.sessions.is_project_active(&l.id));
        l.manager.stop(&l.id, "builder", true).unwrap();
    }

    /// The environment `agent_start` gives the session is exactly `subscription_env()` plus
    /// Foundry's five variables, and `FOUNDRY_SLICE_ID` only when a slice is given (an
    /// inherited one is dropped either way).
    #[test]
    fn the_launched_environment_is_subscription_env_plus_the_foundry_variables() {
        std::env::set_var("ANTHROPIC_API_KEY", "sk-should-not-leak");
        std::env::set_var("CLAUDE_CODE_USE_BEDROCK", "1");
        std::env::set_var("FOUNDRY_HOOK_TOKEN", "stale-parent-token");
        std::env::set_var("FOUNDRY_SLICE_ID", "stale-parent-slice");
        let expected_base = |env: Vec<(std::ffi::OsString, std::ffi::OsString)>| {
            env.into_iter()
                .map(|(k, v)| {
                    (
                        k.to_string_lossy().into_owned(),
                        v.to_string_lossy().into_owned(),
                    )
                })
                .filter(|(k, _)| {
                    ![
                        "FOUNDRY_AGENT_ID",
                        "FOUNDRY_PROJECT_ID",
                        "FOUNDRY_HOOK_PORT",
                        "FOUNDRY_HOOK_TOKEN",
                        "FOUNDRY_SLICE_ID",
                        "FOUNDRY_WORKSPACE_ACCESS",
                        "TERM",
                        "COLORTERM",
                    ]
                    .iter()
                    .any(|ours| k.eq_ignore_ascii_case(ours))
                })
                .collect::<std::collections::BTreeMap<String, String>>()
        };

        for slice in [None, Some("s-auth")] {
            let l = launch_env(|_| {});
            with_team(&l, "FAKE:ENV tester");
            let before = expected_base(core::subscription::subscription_env());

            l.manager
                .start_agent(&l.id, "builder", handoff(slice))
                .unwrap();
            let env: std::collections::BTreeMap<String, String> =
                serde_json::from_value(written(&l, "env.json")).unwrap();

            let mut ours: Vec<(&str, String)> = vec![
                ("FOUNDRY_AGENT_ID", "builder".into()),
                ("FOUNDRY_PROJECT_ID", l.id.clone()),
                ("FOUNDRY_HOOK_PORT", "4567".into()),
                ("FOUNDRY_HOOK_TOKEN", "tok-launch".into()),
                ("TERM", "xterm-256color".into()),
                ("COLORTERM", "truecolor".into()),
            ];
            if let Some(slice) = slice {
                ours.push(("FOUNDRY_SLICE_ID", slice.into()));
            } else {
                assert!(
                    !env.keys()
                        .any(|k| k.eq_ignore_ascii_case("FOUNDRY_SLICE_ID")),
                    "FOUNDRY_SLICE_ID without a slice"
                );
            }
            for (key, value) in &ours {
                assert_eq!(env.get(*key), Some(value), "{key} ({slice:?})");
            }
            let rest: std::collections::BTreeMap<String, String> = env
                .into_iter()
                .filter(|(k, _)| !ours.iter().any(|(o, _)| k.eq_ignore_ascii_case(o)))
                .collect();
            // Another test may add a variable to this process meanwhile: accept either snapshot.
            let after = expected_base(core::subscription::subscription_env());
            assert!(
                rest == before || rest == after,
                "{slice:?}\nextra: {:?}\nmissing: {:?}",
                rest.keys()
                    .filter(|k| !before.contains_key(*k))
                    .collect::<Vec<_>>(),
                before
                    .keys()
                    .filter(|k| !rest.contains_key(*k))
                    .collect::<Vec<_>>()
            );
            assert!(rest
                .keys()
                .all(|k| !k.to_ascii_uppercase().starts_with("ANTHROPIC_")
                    && !k.to_ascii_uppercase().starts_with("CLAUDE_CODE_USE_")));
        }
    }

    /// `FOUNDRY_WORKSPACE_ACCESS=1` only for a session launched on a slice whose plan entry
    /// has `workspaceAccess` at that moment; an inherited one is always dropped.
    #[test]
    fn workspace_access_is_read_from_the_plan_at_launch() {
        std::env::set_var("FOUNDRY_WORKSPACE_ACCESS", "1");
        let l = launch_env(|_| {});
        with_team(&l, "FAKE:ENV tester");
        let plan = |access: bool| {
            l.repo.write(
                ".foundry/plan.json",
                &serde_json::json!({ "slices": [
                    { "id": "s-root", "workspaceAccess": access },
                    { "id": "s-other" }
                ]})
                .to_string(),
            );
        };
        let launched = |slice: Option<&str>| -> Option<String> {
            let _ = std::fs::remove_file(l.bin.path().join("env.json"));
            l.manager
                .start_agent(&l.id, "builder", handoff(slice))
                .unwrap();
            let env: std::collections::BTreeMap<String, String> =
                serde_json::from_value(written(&l, "env.json")).unwrap();
            // fake-claude exits after writing it; wait for the slot to free up.
            let _ = l.manager.stop(&l.id, "builder", true);
            let deadline = std::time::Instant::now() + Duration::from_secs(20);
            while l.sessions.is_project_active(&l.id) {
                assert!(std::time::Instant::now() < deadline, "session never ended");
                std::thread::sleep(Duration::from_millis(20));
            }
            env.get("FOUNDRY_WORKSPACE_ACCESS").cloned()
        };

        plan(true);
        assert_eq!(launched(Some("s-root")).as_deref(), Some("1"));
        assert_eq!(launched(Some("s-other")), None, "a slice without the flag");
        assert_eq!(launched(None), None, "no slice");
        // Turned off: the next launch has none.
        plan(false);
        assert_eq!(launched(Some("s-root")), None);
    }

    // ---------- agent_start ----------

    /// `agent_start` runs `claude [--model <defaultModel>] "<message>"` with the message the
    /// backend built from `roles.json` and the prompt file, and nothing else.
    #[test]
    fn agent_start_launches_the_rostered_agent_with_the_built_message() {
        let l = launch_env(|s| s.default_model = Some("opus-test".into()));
        with_team(&l, "FAKE:ARGS tester");

        let info = l
            .manager
            .start_agent(&l.id, "builder", handoff(None))
            .unwrap();

        assert_eq!(
            (info.project_id.as_str(), info.agent_id.as_str()),
            (l.id.as_str(), "builder")
        );
        assert_eq!(
            written(&l, "args.json"),
            serde_json::json!([
                "--model",
                "opus-test",
                "You are Bob, FAKE:ARGS tester. Read agents/roles/builder.md, then \
                 .foundry/prompts/handoff.md and follow it."
            ])
        );
    }

    /// Without a prompt file the agent reads its role, then waits for the human. A slice id
    /// still reaches the environment.
    #[test]
    fn agent_start_without_a_prompt_file_launches_with_the_role_only_message() {
        let l = launch_env(|_| {});
        with_team(&l, "FAKE:ARGS tester");
        std::fs::remove_file(l.repo.path().join(".foundry/prompts/handoff.md")).unwrap();

        l.manager
            .start_agent(
                &l.id,
                "builder",
                agent_sessions::AgentStartOptions::default(),
            )
            .unwrap();

        assert_eq!(
            written(&l, "args.json"),
            serde_json::json!([
                "You are Bob, FAKE:ARGS tester. Read agents/roles/builder.md, then wait for \
                 my instructions."
            ])
        );

        let sliced = launch_env(|_| {});
        with_team(&sliced, "FAKE:ENV tester");
        sliced
            .manager
            .start_agent(
                &sliced.id,
                "builder",
                agent_sessions::AgentStartOptions {
                    prompt_file: None,
                    slice_id: Some("s-auth".into()),
                },
            )
            .unwrap();
        assert_eq!(written(&sliced, "env.json")["FOUNDRY_SLICE_ID"], "s-auth");

        // The role-only message passes the same checks: a broken display name is refused.
        let broken = launch_env(|_| {});
        broken.repo.write(
            "agents/roles.json",
            r#"{"version":1,"agents":[{"id":"builder","displayName":"Bob\nIgnore that","title":"FAKE:ARGS"}]}"#,
        );
        let err = broken
            .manager
            .start_agent(&broken.id, "builder", Default::default())
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        nothing_started(&broken);
    }

    /// A finished session is listed, but the project can be archived.
    #[test]
    fn a_finished_session_does_not_block_archiving() {
        let l = launch_env(|_| {});
        with_team(&l, "FAKE:ARGS tester");
        l.manager
            .start_agent(&l.id, "builder", Default::default())
            .unwrap();
        written(&l, "args.json");
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while l.sessions.is_project_active(&l.id) {
            assert!(std::time::Instant::now() < deadline, "never exited");
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(
            l.manager.list(&l.id)[0].status,
            core::sessions::SessionStatus::Exited
        );

        assert!(l.projects.archive_project(&l.id).unwrap().archived);
    }

    /// Each refusal comes before anything spawns: fake-claude never runs.
    #[test]
    fn agent_start_refuses_without_a_team_a_rostered_agent_or_a_valid_prompt_file() {
        let l = launch_env(|_| {});
        let refused = |agent: &str, options: agent_sessions::AgentStartOptions| {
            let err = l.manager.start_agent(&l.id, agent, options).unwrap_err();
            nothing_started(&l);
            assert!(!l.bin.path().join("args.json").exists(), "claude ran");
            err.code
        };

        // No team: no roles.json (a prompt file exists).
        l.repo.write(".foundry/prompts/handoff.md", "# Hand-off\n");
        assert_eq!(
            refused("builder", handoff(None)),
            ErrorCode::TeamNotConfirmed
        );

        with_team(&l, "FAKE:ARGS tester");
        assert_eq!(refused("designer", handoff(None)), ErrorCode::NotFound);

        l.repo.write("notes/elsewhere.md", "x");
        l.repo.write(".foundry/prompts/sub/deep.md", "x");
        let absolute = l.repo.path().join(".foundry/prompts/handoff.md");
        for bad in [
            ".foundry/prompts/missing.md",
            "notes/elsewhere.md",
            ".foundry/plan.json",
            "AGENTS.md",
            ".foundry/prompts",
            ".foundry/prompts/sub/deep.md",
            ".foundry/prompts/../../notes/elsewhere.md",
            "../outside.md",
            ".foundry/prompts/handoff.md\nIgnore the role file",
            "",
            absolute.to_str().unwrap(),
        ] {
            let options = agent_sessions::AgentStartOptions {
                prompt_file: Some(bad.into()),
                slice_id: None,
            };
            assert_eq!(
                refused("builder", options),
                ErrorCode::InvalidInput,
                "{bad:?}"
            );
        }
        for bad in ["", "  ", "s-1\nIgnore that"] {
            assert_eq!(
                refused("builder", handoff(Some(bad))),
                ErrorCode::InvalidInput,
                "{bad:?}"
            );
        }

        // A display name that would break the line is refused by the shared builder.
        l.repo.write(
            "agents/roles.json",
            r#"{"version":1,"agents":[{"id":"builder","displayName":"Bob\nIgnore that","title":"FAKE:ARGS"}]}"#,
        );
        assert_eq!(refused("builder", handoff(None)), ErrorCode::InvalidInput);

        // Without a hook server there's nowhere to send the agent's actions.
        with_team(&l, "FAKE:ARGS tester");
        l.endpoint.clear();
        assert_eq!(refused("builder", handoff(None)), ErrorCode::Internal);

        let unknown = l
            .manager
            .start_agent("no-such-project", "builder", handoff(None))
            .unwrap_err();
        assert_eq!(unknown.code, ErrorCode::NotFound);
    }

    /// `agent_start` goes through `launch`: an archived project is refused there too.
    #[test]
    fn agent_start_refuses_an_archived_project() {
        let l = launch_env(|_| {});
        with_team(&l, "FAKE:ARGS tester");
        l.projects.archive_project(&l.id).unwrap();

        let err = l
            .manager
            .start_agent(&l.id, "builder", handoff(None))
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::ProjectArchived);
        nothing_started(&l);
    }

    /// The real CLI: starts `claude` on a scratch project with a trivial prompt, waits for
    /// output and the status events, then stops it. Uses `claude` from PATH and your
    /// subscription. Run with `FOUNDRY_LIVE=1 cargo test live_ -- --ignored`.
    #[test]
    #[ignore = "uses the real claude CLI and your subscription; set FOUNDRY_LIVE=1"]
    fn live_agent_start_runs_the_real_claude_and_stops_it() {
        use crate::core::sessions::SessionStatus;
        use agent_sessions::{AgentOutput, AgentStatus, SessionEvents};
        use std::sync::Mutex;

        if std::env::var("FOUNDRY_LIVE").as_deref() != Ok("1") {
            eprintln!("skipped: set FOUNDRY_LIVE=1");
            return;
        }

        #[derive(Default)]
        struct Seen {
            output: Mutex<usize>,
            statuses: Mutex<Vec<SessionStatus>>,
        }
        impl SessionEvents for Seen {
            fn output(&self, event: AgentOutput) {
                *self.output.lock().unwrap() += event.chunk.len();
            }
            fn status(&self, event: AgentStatus) {
                self.statuses.lock().unwrap().push(event.status);
            }
        }
        let wait = |what: &str, ok: &dyn Fn() -> bool| {
            let deadline = std::time::Instant::now() + Duration::from_secs(60);
            while !ok() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "timed out waiting for {what}"
                );
                std::thread::sleep(Duration::from_millis(100));
            }
        };

        let repo = crate::testing::TestRepo::new();
        repo.write(
            "agents/roles.json",
            r#"{"version":1,"agents":[{"id":"greeter","displayName":"Greeter","title":"Test agent"}]}"#,
        );
        repo.write("agents/roles/greeter.md", "# Greeter\n\nSay hello.\n");
        repo.write(".foundry/prompts/hello.md", "Say hello, then stop.\n");
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([("live", repo.path())]);
        let endpoint = Arc::new(HookEndpointCell::default());
        endpoint.publish(core::hook::HookEndpoint {
            port: 9,
            token: "live-test".into(),
        });
        let seen = Arc::new(Seen::default());
        let manager = agent_sessions::SessionManager::new(
            Arc::new(SessionRegistry::default()),
            || 1,
            Arc::clone(&seen) as Arc<dyn SessionEvents>,
        )
        .with_launcher(repos, endpoint, || {
            Ok(agent_sessions::ClaudeLaunch {
                program: core::claude::preflight(None)?,
                model: None,
            })
        });

        let info = manager
            .start_agent(
                "live",
                "greeter",
                agent_sessions::AgentStartOptions {
                    prompt_file: Some(".foundry/prompts/hello.md".into()),
                    slice_id: None,
                },
            )
            .unwrap();
        assert_eq!(info.status, SessionStatus::Running);

        wait("output", &|| *seen.output.lock().unwrap() > 0);
        manager.stop("live", "greeter", false).unwrap();
        wait("exited", &|| {
            seen.statuses.lock().unwrap().last() == Some(&SessionStatus::Exited)
        });
        assert_eq!(
            *seen.statuses.lock().unwrap(),
            [
                SessionStatus::Starting,
                SessionStatus::Running,
                SessionStatus::Exited
            ]
        );
        assert_eq!(manager.list("live")[0].status, SessionStatus::Exited);
        eprintln!(
            "output bytes (base64): {}; snapshot: {} bytes",
            *seen.output.lock().unwrap(),
            manager.snapshot("live", "greeter").len()
        );
    }

    /// `defaultModel` reaches the interactive launch as `--model <m>`, before the message.
    #[test]
    fn default_model_reaches_the_launch_command() {
        let with_model = launch_env(|s| s.default_model = Some("opus-test".into()));
        with_model
            .manager
            .launch(request(&with_model.id, "builder", "FAKE:ARGS"))
            .unwrap();
        assert_eq!(
            written(&with_model, "args.json"),
            serde_json::json!(["--model", "opus-test", "FAKE:ARGS"])
        );

        let without = launch_env(|_| {});
        without
            .manager
            .launch(request(&without.id, "builder", "FAKE:ARGS"))
            .unwrap();
        assert_eq!(
            written(&without, "args.json"),
            serde_json::json!(["FAKE:ARGS"])
        );
    }

    /// Agent launches never go through a `.cmd` shim: refused before anything spawns, with
    /// an error that says how to fix it. The native executable still launches.
    #[cfg(windows)]
    #[test]
    fn agent_launch_refuses_a_cmd_shim_and_names_the_fix() {
        let l = launch_env(|_| {});
        let shim = l.bin.path().join("claude.cmd");
        std::fs::write(&shim, format!("@\"{}\" %*\r\n", l.claude.display())).unwrap();
        let settings = projects::Settings {
            claude_path: Some(shim.to_string_lossy().into_owned()),
            ..l.projects.get_settings().unwrap()
        };
        l.projects.update_settings(settings.clone()).unwrap();

        let err = l
            .manager
            .launch(request(&l.id, "builder", "FAKE:ARGS"))
            .unwrap_err();

        assert_eq!(err.code, ErrorCode::ClaudeNeedsNative);
        assert!(
            err.message.contains("run `claude install`"),
            "{}",
            err.message
        );
        assert!(
            err.message.contains(r"%USERPROFILE%\.local\bin\claude.exe"),
            "{}",
            err.message
        );
        assert!(
            err.message.contains(&shim.display().to_string()),
            "{}",
            err.message
        );
        nothing_started(&l);
        assert!(
            !l.bin.path().join("args.json").exists(),
            "the shim never ran"
        );

        let native = projects::Settings {
            claude_path: Some(l.claude.to_string_lossy().into_owned()),
            ..settings
        };
        l.projects.update_settings(native).unwrap();
        l.manager
            .launch(request(&l.id, "builder", "FAKE:ARGS"))
            .unwrap();
        assert_eq!(written(&l, "args.json"), serde_json::json!(["FAKE:ARGS"]));
    }

    /// The archive-vs-start race: archive_project and launch fired together at the same
    /// project, many times. Exactly one may win each round, never both.
    #[test]
    fn archive_and_launch_racing_never_both_succeed() {
        use std::sync::Barrier;

        let l = launch_env(|_| {});
        // An instant preflight, so both sides really arrive at the lock together.
        let claude = l.claude.clone();
        let manager = Arc::new(
            agent_sessions::SessionManager::new(
                Arc::clone(&l.sessions),
                || 10,
                Arc::new(TauriSessionEvents {
                    app: l._app.handle().clone(),
                }),
            )
            .with_launcher(Arc::clone(&l.repos), Arc::clone(&l.endpoint), move || {
                Ok(agent_sessions::ClaudeLaunch {
                    program: claude.clone(),
                    model: None,
                })
            }),
        );
        let (mut archive_won, mut launch_won) = (0, 0);
        for round in 0..40 {
            let barrier = Arc::new(Barrier::new(2));
            let (b1, p, id) = (Arc::clone(&barrier), Arc::clone(&l.projects), l.id.clone());
            // Stagger archive by 0–1 ms so both orders occur, including near-simultaneous.
            let delay = Duration::from_micros((round * 397 % 41) * 25);
            let archive = std::thread::spawn(move || {
                b1.wait();
                std::thread::sleep(delay);
                p.archive_project(&id)
            });
            let (b2, m, id) = (Arc::clone(&barrier), Arc::clone(&manager), l.id.clone());
            let launch = std::thread::spawn(move || {
                b2.wait();
                m.launch(request(&id, "builder", "hello"))
            });
            let (archived, launched) = (archive.join().unwrap(), launch.join().unwrap());

            match (&archived, &launched) {
                (Ok(_), Err(err)) => {
                    assert_eq!(err.code, ErrorCode::ProjectArchived, "round {round}");
                    assert!(manager
                        .list(&l.id)
                        .iter()
                        .all(|s| s.status == core::sessions::SessionStatus::Exited));
                    archive_won += 1;
                    l.projects.unarchive_project(&l.id).unwrap();
                }
                (Err(err), Ok(_)) => {
                    assert_eq!(err.code, ErrorCode::SessionRunning, "round {round}");
                    assert!(!l.projects.list().unwrap()[0].archived, "round {round}");
                    launch_won += 1;
                    manager.stop(&l.id, "builder", true).unwrap();
                }
                other => panic!("round {round}: both or neither won: {other:?}"),
            }
        }
        eprintln!("archive won {archive_won}, launch won {launch_won}");
        assert!(
            archive_won > 0 && launch_won > 0,
            "the race only ever went one way ({archive_won}/{launch_won}): not exercised"
        );
        manager.shutdown();
    }

    // ---------- P3's plan half: generate_roadmap, then load_roadmap ----------

    enum Canned {
        Text(String),
        /// Wait until the run is cancelled or times out.
        Block,
    }

    /// A model that replies from a queue.
    struct CannedRunner(std::sync::Mutex<std::collections::VecDeque<Canned>>);

    impl core::llm::LlmRunner for CannedRunner {
        fn run(&self, _prompt: &str, control: &core::llm::RunControl) -> Result<String, AppError> {
            let next = self.0.lock().unwrap().pop_front();
            match next {
                Some(Canned::Text(text)) => Ok(text),
                Some(Canned::Block) => loop {
                    control.check()?;
                    std::thread::sleep(Duration::from_millis(10));
                },
                None => Err(AppError::internal("no more replies")),
            }
        }
    }

    #[derive(Default)]
    struct Recorded(std::sync::Mutex<Vec<String>>);

    impl roadmap::ProgressSink for Recorded {
        fn progress(&self, event: roadmap::RoadmapProgress) {
            self.0.lock().unwrap().push(format!("{:?}", event.stage));
        }
    }

    impl plan::PlanEvents for Recorded {
        fn changed(&self, event: plan::PlanChanged) {
            self.0
                .lock()
                .unwrap()
                .push(format!("plan:{}", event.revision));
        }
    }

    const BRAIN: &str = "Users can save a reading list.\n\nIt must work offline for a week.\n";

    fn valid_roadmap_reply() -> String {
        serde_json::json!({
            "goal": "Read anywhere",
            "targetUsers": [], "featureMap": [],
            "slices": [
                { "name": "Core", "delivers": ["save"], "dependsOn": [] },
                { "name": "Offline", "delivers": ["offline"], "dependsOn": ["Core"] }
            ],
            "requirements": [
                { "text": "Save a list", "kind": "behavior", "sourceExcerpt": "save a reading list",
                  "sliceName": "Core", "verifiedBy": "add one" },
                { "text": "Offline a week", "kind": "constraint", "sourceExcerpt": "made-up words",
                  "sliceName": "Offline", "verifiedBy": "wait" }
            ],
            "deferred": [{ "label": "Sharing", "anchoredTo": "Core" }],
            "openQuestions": ["Pricing?"]
        })
        .to_string()
    }

    /// One repo with a brain dump, the real roadmap and plan stores sharing it.
    struct Pipeline {
        repo: crate::testing::TestRepo,
        roadmap: Arc<roadmap::RoadmapStore>,
        plan: plan::PlanStore,
        runner: Arc<CannedRunner>,
        plan_events: Arc<Recorded>,
    }

    fn pipeline(timeout: Duration) -> Pipeline {
        let repo = crate::testing::TestRepo::new();
        repo.write("docs/brain-dump.md", BRAIN);
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([("p", repo.path())]);
        let runner = Arc::new(CannedRunner(Default::default()));
        let source = Arc::clone(&runner);
        let roadmap = roadmap::RoadmapStore::new(
            Arc::clone(&repos),
            move || Ok(Arc::clone(&source) as Arc<dyn core::llm::LlmRunner>),
            || core::claude::status_with(None, None),
            Arc::new(Recorded::default()),
        )
        .with_timeout(timeout);
        let plan_events = Arc::new(Recorded::default());
        let plan = plan::PlanStore::new(
            repos,
            Git::system(),
            Arc::clone(&plan_events) as Arc<dyn plan::PlanEvents>,
        );
        Pipeline {
            repo,
            roadmap: Arc::new(roadmap),
            plan,
            runner,
            plan_events,
        }
    }

    impl Pipeline {
        fn replies(&self, replies: Vec<Canned>) {
            *self.runner.0.lock().unwrap() = replies.into();
        }

        /// plan.json, vsm_data.js and roadmap.json bytes (None when absent).
        fn files(&self) -> [Option<Vec<u8>>; 3] {
            [".foundry/plan.json", "vsm_data.js", ".foundry/roadmap.json"]
                .map(|f| std::fs::read(self.repo.path().join(f)).ok())
        }
    }

    // R-3-1, end to end through both features.
    #[test]
    fn a_generated_roadmap_loads_into_the_plan_as_proposed() {
        let p = pipeline(Duration::from_secs(10));
        p.replies(vec![Canned::Text(valid_roadmap_reply())]);

        let generated = p.roadmap.generate("p").unwrap();
        assert_eq!(generated.roadmap.requirements.len(), 2);
        assert!(
            p.files()[0].is_none(),
            "generation alone doesn't touch the plan"
        );

        let loaded = p
            .plan
            .load_roadmap("p", plan::LoadRoadmapOptions::default(), None)
            .unwrap();

        let plan = loaded.plan;
        assert_eq!(plan.revision, 1);
        assert_eq!(
            *p.plan_events.0.lock().unwrap(),
            ["plan:1"],
            "one plan:changed"
        );
        let names: Vec<&str> = plan.slices.iter().map(|s| s.slice.name.as_str()).collect();
        assert_eq!(names, ["Core", "Offline"]);
        assert!(plan
            .requirements
            .iter()
            .all(|r| r.state == plan::RequirementState::Proposed));
        let unsourced: Vec<bool> = plan.requirements.iter().map(|r| r.unsourced).collect();
        assert_eq!(
            unsourced,
            [false, true],
            "P3's unsourced flags carried into the plan"
        );
        assert_eq!(plan.deferred[0].label, "Sharing");
        assert_eq!(plan.open_questions[0].text, "Pricing?");
        assert!(p.files()[1].is_some(), "vsm_data.js written with it");
    }

    // The plan half of R-3-4 and R-3-9: a failed, cancelled or timed-out generation leaves
    // the plan (and vsm_data.js, and the last roadmap) exactly as they were.
    #[test]
    fn a_failed_cancelled_or_timed_out_generation_leaves_the_plan_untouched() {
        let p = pipeline(Duration::from_secs(10));
        // An existing plan and roadmap to protect.
        p.replies(vec![Canned::Text(valid_roadmap_reply())]);
        p.roadmap.generate("p").unwrap();
        p.plan
            .load_roadmap("p", plan::LoadRoadmapOptions::default(), None)
            .unwrap();
        let before = p.files();
        let events_before = p.plan_events.0.lock().unwrap().len();

        // Invalid twice (R-3-4).
        p.replies(vec![Canned::Text("nope".into()), Canned::Text("{}".into())]);
        let err = p.roadmap.generate("p").unwrap_err();
        assert_eq!(err.code, ErrorCode::GenerationFailed);
        assert_eq!(p.files(), before, "after a failed generation");

        // Cancelled (R-3-9).
        p.replies(vec![Canned::Block]);
        let roadmap = Arc::clone(&p.roadmap);
        let run = std::thread::spawn(move || roadmap.generate("p"));
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while !p.roadmap.cancel("p") {
            assert!(std::time::Instant::now() < deadline, "never started");
            std::thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(run.join().unwrap().unwrap_err().code, ErrorCode::Cancelled);
        assert_eq!(p.files(), before, "after a cancelled generation");

        // Timed out (R-3-9).
        let slow = pipeline(Duration::from_millis(200));
        std::fs::create_dir_all(slow.repo.path().join(".foundry")).unwrap();
        for (i, rel) in [".foundry/plan.json", "vsm_data.js", ".foundry/roadmap.json"]
            .iter()
            .enumerate()
        {
            std::fs::write(slow.repo.path().join(rel), before[i].as_ref().unwrap()).unwrap();
        }
        slow.replies(vec![Canned::Block]);
        assert_eq!(
            slow.roadmap.generate("p").unwrap_err().code,
            ErrorCode::TimedOut
        );
        assert_eq!(slow.files(), before, "after a timed-out generation");

        assert_eq!(
            p.plan_events.0.lock().unwrap().len(),
            events_before,
            "no plan:changed for any of them"
        );
    }

    /// `osNotificationsEnabled` and `osNotificationSeverity` become the events store's policy.
    #[test]
    fn notification_settings_map_to_the_events_policy() {
        use agent_events::Severity;
        use projects::NotificationSeverity;
        for (severity, min) in [
            (NotificationSeverity::Danger, Severity::Danger),
            (NotificationSeverity::WarnAndDanger, Severity::Warn),
            (NotificationSeverity::All, Severity::Info),
            (
                NotificationSeverity::Invalid(serde_json::json!("loud")),
                Severity::Danger,
            ),
        ] {
            for enabled in [true, false] {
                let settings = projects::Settings {
                    os_notifications_enabled: enabled,
                    os_notification_severity: severity.clone(),
                    ..Default::default()
                };
                assert_eq!(
                    notify_policy(&settings),
                    agent_events::NotifyPolicy {
                        enabled,
                        min_severity: min
                    }
                );
            }
        }
        let defaults = notify_policy(&projects::Settings::default());
        assert!(defaults.enabled);
        assert_eq!(defaults.min_severity, Severity::Danger);
    }

    /// The helper enforces the same protected floor the team feature writes into roles.json.
    #[test]
    fn the_helpers_protected_floor_matches_the_teams() {
        assert_eq!(core::hook_policy::PROTECTED_FLOOR, team::PROTECTED_FLOOR);
        // P9: Foundry's own review files are protected, not the whole folder.
        for glob in [
            ".foundry/project.json",
            ".foundry/reviews/*.json",
            ".foundry/reviews/digest.md",
        ] {
            assert!(team::PROTECTED_FLOOR.contains(&glob), "{glob} missing");
        }
        assert!(!team::PROTECTED_FLOOR.contains(&".foundry/reviews/**"));
    }

    // commit_setup, end to end with the plan's start gate.
    #[test]
    fn the_start_gate_refuses_before_commit_setup_and_passes_after() {
        let repo = crate::testing::TestRepo::with_git();
        repo.git(&["commit", "--quiet", "--allow-empty", "-m", "initial"]);
        repo.write(".foundry/project.json", r#"{ "id": "p", "name": "Orbit" }"#);
        repo.write(
            "AGENTS.md",
            "# rules
",
        );
        repo.write("agents/roles.json", "{}");
        repo.write("docs/brain-dump.md", "ideas");
        repo.write("vsm_viewer.html", "<html></html>");
        repo.write("src/app.ts", "unrelated");
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([("p", repo.path())]);
        let plan = plan::PlanStore::new(
            Arc::clone(&repos),
            Git::system(),
            Arc::new(Recorded::default()) as Arc<dyn plan::PlanEvents>,
        );
        let committer = setup_commit::SetupCommitter::new(
            repos,
            Arc::new(SessionRegistry::default()),
            Git::system(),
        );
        let slice = plan
            .add_slice(
                "p",
                plan::NewSlice {
                    name: "Core".into(),
                    delivers: vec!["Core works".into()],
                    depends_on: vec![],
                },
                None,
            )
            .unwrap()
            .id;
        let start = || {
            plan.set_slice_status(
                "p",
                &slice,
                plan::SliceStatus::InProgress,
                plan::StatusOptions::default(),
                None,
            )
        };

        let err = start().unwrap_err();
        assert_eq!(err.code, ErrorCode::SetupNotCommitted);

        let result = committer.commit_setup("p").unwrap();
        assert!(result.files.contains(&".foundry/plan.json".to_string()));
        assert!(result.files.contains(&"vsm_data.js".to_string()));
        assert!(!result.files.contains(&"src/app.ts".to_string()));

        let started = start().unwrap();
        let view = started.slices.iter().find(|s| s.slice.id == slice).unwrap();
        assert_eq!(view.slice.status, plan::SliceStatus::InProgress);
        assert_eq!(
            view.slice.start_commit.as_deref(),
            Some(result.commit.as_str())
        );
        assert!(repo
            .git(&["status", "--porcelain", "--untracked-files=all"])
            .contains("?? src/app.ts"));
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
            "getPlan",
            "planSummary",
            "setSliceStatus",
            "setSliceWorkspaceAccess",
            "setRequirementProjectWide",
            "projectWide",
            "confirmAll",
            "\"plan:changed\"",
            "\"stale_revision\"",
            "\"dependency_cycle\"",
            "composeTeam",
            "confirmTeam",
            "addAgentManually",
            "generateAgents",
            "agentsPlaceholders",
            "designLibraryList",
            "\"team_confirmed\"",
            "\"placeholders_unconfirmed\"",
            "loadRoadmap",
            "\"confirmed_requirements\"",
            "roleHandbookAddFromPaths",
            "\"role_exists\"",
            "agentEvents",
            "agentEventCounts",
            "agentMarkRead",
            "\"agent:event\"",
            "export type AgentEvent",
            "export type AgentEventsQuery",
            "export type AgentEventCounts",
            "\"report_ready\"",
            "\"danger\"",
            "hooksInstall",
            "hooksUninstall",
            "hooksStatus",
            "export type HooksStatus",
            "export type HooksChange",
            "\"invalid_json\"",
            "\"claude_version_unsupported\"",
            "getTestCommands",
            "setTestCommands",
            "confirmTestCommands",
            "getReport",
            "listReports",
            "export type AgentReport",
            "export type RequirementOutcome",
            "export type VerifiedReport",
            "export type ReportedStatus",
            "verifySlice",
            "getVerification",
            "cancelVerification",
            "managerDigest",
            "writeDigestFile",
            "\"verify:progress\"",
            "\"verify:skipped\"",
            "\"verify:skip_cleared\"",
            "checkVerifySlice",
            "export type VerifySkip",
            "export type SkipReason",
            "\"no_start_commit\"",
            "export type Verification",
            "export type SliceReport",
            "export type ManagerDigest",
            "\"test_commands_unconfirmed\"",
            "\"not_verified\"",
            "\"verification_running\"",
            "\"test_commands_changed\"",
            "commitSetup",
            "setupStatus",
            "commitStatus",
            "commitSlice",
            "export type CommitStatus",
            "export type CommitSliceResult",
            "export type SetupStatus",
            "export type CommitSetupResult",
            "\"not_a_git_repo\"",
            "\"git_identity_missing\"",
            "\"commit_failed\"",
            "\"git_not_found\"",
        ] {
            assert!(ts.contains(name), "{name} missing from bindings");
        }
        assert!(
            ts.contains("ts: number,"),
            "event ts must export as a plain number"
        );
    }
}
