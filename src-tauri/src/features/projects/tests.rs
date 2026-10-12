use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::json;
use tempfile::TempDir;

use super::*;
use crate::testing::TestRepo;

// ---------- fixtures ----------

/// An app-data dir plus a git that ignores the user's own config.
struct Env {
    data: TempDir,
    git_home: TempDir,
}

impl Env {
    fn new() -> Self {
        let git_home = tempfile::tempdir().unwrap();
        fs::write(git_home.path().join("gitconfig"), "").unwrap();
        Self {
            data: tempfile::tempdir().unwrap(),
            git_home,
        }
    }

    fn isolated_git(&self) -> Git {
        Git::system()
            .env("GIT_CONFIG_GLOBAL", self.git_home.path().join("gitconfig"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
    }

    fn git_with_identity(&self) -> Git {
        self.isolated_git()
            .env("GIT_AUTHOR_NAME", "Foundry Test")
            .env("GIT_AUTHOR_EMAIL", "test@foundry.invalid")
            .env("GIT_COMMITTER_NAME", "Foundry Test")
            .env("GIT_COMMITTER_EMAIL", "test@foundry.invalid")
    }

    /// git that refuses to guess an identity, like a machine where none is configured.
    fn git_without_identity(&self) -> Git {
        let mut git = self.isolated_git();
        for var in [
            "GIT_AUTHOR_NAME",
            "GIT_AUTHOR_EMAIL",
            "GIT_COMMITTER_NAME",
            "GIT_COMMITTER_EMAIL",
            "EMAIL",
        ] {
            git = git.env_remove(var);
        }
        git.env("GIT_CONFIG_COUNT", "1")
            .env("GIT_CONFIG_KEY_0", "user.useConfigOnly")
            .env("GIT_CONFIG_VALUE_0", "true")
    }

    fn store(&self) -> ProjectsStore {
        self.store_with(self.git_with_identity())
    }

    fn store_with(&self, git: Git) -> ProjectsStore {
        ProjectsStore::load(
            self.data.path(),
            git,
            Default::default(),
            Default::default(),
        )
        .unwrap()
    }
}

fn create(store: &ProjectsStore, parent: &Path, name: &str) -> CreateRepositoryResult {
    store
        .create_repository(CreateRepositoryInput {
            name: name.into(),
            parent_dir: Some(parent.to_string_lossy().into_owned()),
        })
        .unwrap()
}

fn link(store: &ProjectsStore, folder: &Path) -> Result<LinkRepositoryResult, AppError> {
    store.link_repository(LinkRepositoryInput {
        folder: folder.to_string_lossy().into_owned(),
    })
}

/// Every file and folder under `dir` (relative, `/`-separated): full contents for files,
/// `None` for folders. Comparing two snapshots compares every byte, not just hashes.
fn snapshot(dir: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Option<Vec<u8>>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let rel = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            if path.symlink_metadata().unwrap().is_dir() {
                out.insert(rel, None);
                walk(root, &path, out);
            } else {
                out.insert(rel, Some(fs::read(&path).unwrap()));
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn without_git(snapshot: &BTreeMap<String, Option<Vec<u8>>>) -> Vec<&str> {
    snapshot
        .keys()
        .map(String::as_str)
        .filter(|rel| *rel != ".git" && !rel.starts_with(".git/"))
        .collect()
}

fn git_in(repo: &Path, args: &[&str]) -> std::process::Output {
    std::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .unwrap()
}

// ---------- R-1-1 ----------

#[test]
fn create_list_open_round_trip() {
    let env = Env::new();
    let parent = TestRepo::new();
    let store = env.store();

    let created = create(&store, parent.path(), "My App");
    assert_eq!(created.project.name, "My App");
    assert_eq!(
        Path::new(&created.project.repo_path),
        parent.path().join("my-app")
    );
    assert!(created.project.folder_exists);
    assert_eq!(created.project.last_opened, None);

    let listed = store.list().unwrap();
    assert_eq!(listed, vec![created.project.clone()]);
    assert_eq!(
        store.active_project().unwrap(),
        None,
        "creating doesn't open"
    );

    let opened = store.open_project(&created.project.id).unwrap();
    assert!(opened.last_opened.is_some());
    assert_eq!(store.active_project().unwrap(), Some(opened.clone()));
    assert_eq!(store.list().unwrap(), vec![opened]);
}

// ---------- R-1-2 ----------

#[test]
fn create_on_existing_folder_returns_folder_exists_and_changes_nothing() {
    let env = Env::new();
    let parent = TestRepo::new();
    parent.write("my-app/notes.txt", "mine");
    let store = env.store();
    let parent_before = snapshot(parent.path());
    let data_before = snapshot(env.data.path());

    let err = store
        .create_repository(CreateRepositoryInput {
            name: "My App".into(),
            parent_dir: Some(parent.path().to_string_lossy().into_owned()),
        })
        .unwrap_err();

    assert_eq!(err.code, ErrorCode::FolderExists);
    assert_eq!(snapshot(parent.path()), parent_before);
    assert_eq!(snapshot(env.data.path()), data_before);
    assert!(store.list().unwrap().is_empty());
}

// ---------- R-1-3 ----------

#[test]
fn link_leaves_every_existing_file_byte_identical_and_adds_only_missing_foundry_files() {
    let env = Env::new();
    let repo = TestRepo::with_git();
    repo.write("src/main.rs", "fn main() {}\r\n");
    repo.write("README.md", "# Mine\n");
    repo.write("AGENTS.md", "my own agents file");
    fs::write(repo.path().join("logo.bin"), [0u8, 159, 146, 150, 255]).unwrap();
    let before = snapshot(repo.path());

    let linked = link(&env.store(), repo.path()).unwrap();

    let after = snapshot(repo.path());
    for (rel, contents) in &before {
        if rel == ".git/info/exclude" {
            // Only gains Foundry's lines (see the git noise tests).
            let old = contents.as_deref().unwrap();
            assert!(after[rel].as_deref().unwrap().starts_with(old));
            continue;
        }
        assert_eq!(after.get(rel), Some(contents), "{rel} changed");
    }
    let added: Vec<&str> = after
        .keys()
        .filter(|rel| !before.contains_key(*rel))
        .map(String::as_str)
        .collect();
    assert_eq!(
        added,
        [
            ".foundry",
            ".foundry/project.json",
            "docs",
            "docs/brain-dump.md",
            "vsm_viewer.html"
        ]
    );
    assert_eq!(
        linked.created_files,
        [
            ".foundry/project.json",
            "docs/brain-dump.md",
            "vsm_viewer.html"
        ]
    );
    assert!(linked.is_git_repo);
    assert_eq!(
        linked.project.name,
        repo.path().file_name().unwrap().to_string_lossy()
    );
}

#[test]
fn link_keeps_an_existing_vsm_viewer_and_brain_dump() {
    let env = Env::new();
    let repo = TestRepo::new();
    repo.write("vsm_viewer.html", "<!-- customised -->");
    repo.write("docs/brain-dump.md", "my notes");

    let linked = link(&env.store(), repo.path()).unwrap();

    assert_eq!(linked.created_files, [".foundry/project.json"]);
    assert_eq!(repo.read("vsm_viewer.html"), "<!-- customised -->");
    assert_eq!(repo.read("docs/brain-dump.md"), "my notes");
    assert!(!linked.is_git_repo, "never runs git");
    assert!(!repo.path().join(".git").exists());
}

#[test]
fn link_rejects_missing_folders_files_and_duplicates() {
    let env = Env::new();
    let store = env.store();
    let repo = TestRepo::new();
    repo.write("file.txt", "x");

    let missing = link(&store, &repo.path().join("nope")).unwrap_err();
    assert_eq!(missing.code, ErrorCode::NotFound);
    let file = link(&store, &repo.path().join("file.txt")).unwrap_err();
    assert_eq!(file.code, ErrorCode::InvalidPath);

    link(&store, repo.path()).unwrap();
    let again = link(&store, repo.path()).unwrap_err();
    assert_eq!(again.code, ErrorCode::AlreadyLinked);
    assert_eq!(store.list().unwrap().len(), 1);
}

#[test]
fn relinking_a_removed_project_keeps_its_identity() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let created = create(&store, parent.path(), "Orbit");
    store
        .update_project(
            &created.project.id,
            ProjectPatch {
                tagline: Some("Space things".into()),
                ..Default::default()
            },
        )
        .unwrap();
    store.archive_project(&created.project.id).unwrap();
    store.remove_project(&created.project.id).unwrap();

    let linked = link(&store, Path::new(&created.project.repo_path)).unwrap();
    assert_eq!(linked.project.id, created.project.id);
    assert_eq!(linked.project.name, "Orbit");
    assert_eq!(linked.project.tagline, "Space things");
    assert!(linked.created_files.is_empty());
}

// ---------- R-1-4 ----------

#[test]
fn document_round_trips_2mb_exactly() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let id = create(&store, parent.path(), "Essay").project.id;

    let chunk = "# Heading ✓\r\nLine with émoji 🚀 and tabs\tand \"quotes\"\n\n- [ ] list\r\n";
    let mut doc = String::new();
    while doc.len() < 2 * 1024 * 1024 {
        doc.push_str(chunk);
    }

    store.save_document(&id, &doc).unwrap();
    let back = store.get_document(&id).unwrap();
    assert_eq!(back.len(), doc.len());
    assert!(back == doc, "document changed in the round trip");

    let on_disk =
        fs::read(Path::new(&store.list().unwrap()[0].repo_path).join("docs/brain-dump.md"))
            .unwrap();
    assert_eq!(on_disk, doc.as_bytes());
}

#[test]
fn new_documents_start_empty_and_missing_folders_are_reported() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Gone").project;
    assert_eq!(store.get_document(&project.id).unwrap(), "");

    fs::remove_dir_all(&project.repo_path).unwrap();
    assert_eq!(
        store.get_document(&project.id).unwrap_err().code,
        ErrorCode::FolderMissing
    );
    assert_eq!(
        store.open_project(&project.id).unwrap_err().code,
        ErrorCode::FolderMissing
    );
    assert!(!store.list().unwrap()[0].folder_exists);
}

// ---------- R-1-5 ----------

#[test]
fn remove_project_leaves_every_repo_file_untouched() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Keep Me").project;
    store.save_document(&project.id, "notes").unwrap();
    store.open_project(&project.id).unwrap();
    let before = snapshot(parent.path());

    store.archive_project(&project.id).unwrap();

    store.remove_project(&project.id).unwrap();

    assert_eq!(snapshot(parent.path()), before);
    assert!(store.list().unwrap().is_empty());
    assert_eq!(store.active_project().unwrap(), None);
    assert_eq!(
        store.remove_project(&project.id).unwrap_err().code,
        ErrorCode::NotFound
    );
}

// ---------- archiving ----------

#[test]
fn archive_flips_only_the_registry_flag_and_never_touches_the_folder() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Shelf").project;
    assert!(!project.archived, "new projects start active");
    store.save_document(&project.id, "notes").unwrap();
    let before = snapshot(parent.path());

    let archived = store.archive_project(&project.id).unwrap();
    assert!(archived.archived);
    assert_eq!(snapshot(parent.path()), before);
    assert!(archived.folder_exists);
    // Idempotent.
    assert!(store.archive_project(&project.id).unwrap().archived);

    let restored = store.unarchive_project(&project.id).unwrap();
    assert!(!restored.archived);
    assert!(!store.unarchive_project(&project.id).unwrap().archived);
    assert_eq!(snapshot(parent.path()), before);
    assert_eq!(
        restored,
        Project {
            archived: false,
            ..archived
        }
    );
}

#[test]
fn archiving_the_active_project_closes_it_and_leaves_others_open() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let open = create(&store, parent.path(), "Open").project;
    let other = create(&store, parent.path(), "Other").project;
    store.open_project(&open.id).unwrap();

    // Archiving a different project leaves the active one alone.
    store.archive_project(&other.id).unwrap();
    assert_eq!(store.active_project().unwrap().unwrap().id, open.id);

    let before = snapshot(parent.path());
    let archived = store.archive_project(&open.id).unwrap();
    assert!(archived.archived);
    assert_eq!(store.active_project().unwrap(), None, "back to Home");
    assert_eq!(snapshot(parent.path()), before);

    // Restoring doesn't reopen it.
    store.unarchive_project(&open.id).unwrap();
    assert_eq!(store.active_project().unwrap(), None);
}

#[test]
fn list_projects_returns_archived_and_active_projects_with_the_flag() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let a = create(&store, parent.path(), "A").project;
    let b = create(&store, parent.path(), "B").project;
    let c = create(&store, parent.path(), "C").project;
    store.archive_project(&b.id).unwrap();

    let listed: Vec<(String, bool)> = store
        .list()
        .unwrap()
        .into_iter()
        .map(|p| (p.id, p.archived))
        .collect();
    assert_eq!(listed, vec![(a.id, false), (b.id, true), (c.id, false)]);
}

#[test]
fn archived_flag_survives_a_restart_and_old_registries_default_to_active() {
    let env = Env::new();
    let parent = TestRepo::new();
    let id = {
        let store = env.store();
        let id = create(&store, parent.path(), "Persist").project.id;
        store.archive_project(&id).unwrap();
        id
    };
    let store = env.store();
    assert!(store.list().unwrap()[0].archived);
    assert_eq!(store.list().unwrap()[0].id, id);

    // A projects.json written before the flag existed.
    let old = Env::new();
    let repo = TestRepo::new();
    fs::write(
        old.data.path().join("projects.json"),
        serde_json::json!({
            "version": 1,
            "projects": [{ "id": "p1", "name": "Old", "tagline": "", "repoPath": repo.path() }]
        })
        .to_string(),
    )
    .unwrap();
    assert!(!old.store().list().unwrap()[0].archived);
}

#[test]
fn remove_project_requires_archiving_first() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Not Yet").project;

    let refused = store.remove_project(&project.id).unwrap_err();
    assert_eq!(refused.code, ErrorCode::NotArchived);
    assert_eq!(store.list().unwrap().len(), 1, "nothing was removed");

    // Unarchiving makes it unremovable again.
    store.archive_project(&project.id).unwrap();
    store.unarchive_project(&project.id).unwrap();
    assert_eq!(
        store.remove_project(&project.id).unwrap_err().code,
        ErrorCode::NotArchived
    );

    store.archive_project(&project.id).unwrap();
    store.remove_project(&project.id).unwrap();
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn archive_and_unarchive_report_unknown_ids() {
    let env = Env::new();
    let store = env.store();
    assert_eq!(
        store.archive_project("nope").unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(
        store.unarchive_project("nope").unwrap_err().code,
        ErrorCode::NotFound
    );
}

// ---------- delete_repository (replaces R-1-5 for this command) ----------

#[cfg(windows)]
#[test]
fn delete_repository_moves_the_folder_to_the_recycle_bin_and_unregisters() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Bin Me").project;
    store.save_document(&project.id, "notes").unwrap();
    store.open_project(&project.id).unwrap();
    store.archive_project(&project.id).unwrap();
    // Archiving closed it; reopen so the delete has an active project to clear.
    store.open_project(&project.id).unwrap();

    store.delete_repository(&project.id).unwrap();

    assert!(!Path::new(&project.repo_path).exists(), "folder is gone");
    assert!(parent.path().exists(), "only the project folder was moved");
    assert!(store.list().unwrap().is_empty());
    assert_eq!(store.active_project().unwrap(), None);
    assert_eq!(
        store.delete_repository(&project.id).unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn delete_repository_requires_archiving_first_and_changes_nothing() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Not Archived").project;
    let before = snapshot(parent.path());

    let refused = store.delete_repository(&project.id).unwrap_err();

    assert_eq!(refused.code, ErrorCode::NotArchived);
    assert_eq!(snapshot(parent.path()), before);
    assert_eq!(store.list().unwrap().len(), 1);
}

#[test]
fn delete_repository_with_a_missing_folder_just_unregisters() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Already Gone").project;
    store.archive_project(&project.id).unwrap();
    fs::remove_dir_all(&project.repo_path).unwrap();

    store.delete_repository(&project.id).unwrap();

    assert!(store.list().unwrap().is_empty());
}

#[test]
fn delete_repository_reports_unknown_ids() {
    let env = Env::new();
    let store = env.store();
    assert_eq!(
        store.delete_repository("nope").unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[cfg(windows)]
#[test]
fn delete_repository_with_a_locked_file_fails_with_io_and_changes_nothing() {
    use std::os::windows::fs::OpenOptionsExt;

    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Locked").project;
    store.archive_project(&project.id).unwrap();
    let before = snapshot(parent.path());
    // No sharing at all, like an editor or process holding the file exclusively.
    let lock = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(Path::new(&project.repo_path).join("docs/brain-dump.md"))
        .unwrap();

    let err = store.delete_repository(&project.id).unwrap_err();
    drop(lock);

    assert_eq!(err.code, ErrorCode::Io);
    assert!(err.message.contains("Recycle Bin"), "{}", err.message);
    assert!(
        err.message.contains("open in another program"),
        "{}",
        err.message
    );
    assert_eq!(snapshot(parent.path()), before);
    let listed = store.list().unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].archived && listed[0].folder_exists);
}

#[test]
fn remove_project_still_only_unregisters_alongside_delete_repository() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let keep = create(&store, parent.path(), "Keep Files").project;
    let gone = create(&store, parent.path(), "Gone Files").project;
    store.save_document(&keep.id, "keep me").unwrap();
    store.archive_project(&keep.id).unwrap();
    store.archive_project(&gone.id).unwrap();
    // Folder already removed, so this runs the same on every OS.
    fs::remove_dir_all(&gone.repo_path).unwrap();
    store.delete_repository(&gone.id).unwrap();
    let before = snapshot(Path::new(&keep.repo_path));

    store.remove_project(&keep.id).unwrap();

    assert_eq!(snapshot(Path::new(&keep.repo_path)), before);
    assert_eq!(
        fs::read_to_string(Path::new(&keep.repo_path).join("docs/brain-dump.md")).unwrap(),
        "keep me"
    );
    assert!(store.list().unwrap().is_empty());
}

// ---------- R-1-6 ----------

#[test]
fn registry_and_settings_survive_a_restart() {
    let env = Env::new();
    let parent = TestRepo::new();
    let linked_repo = TestRepo::new();

    let (projects, settings) = {
        let store = env.store();
        let created = create(&store, parent.path(), "Alpha").project;
        link(&store, linked_repo.path()).unwrap();
        store.open_project(&created.id).unwrap();
        let settings = store
            .update_settings(Settings {
                max_concurrent_agents: 5,
                default_projects_dir: Some(parent.path().to_string_lossy().into_owned()),
                claude_path: None,
                default_model: Some("sonnet".into()),
                generation_timeout_ms: 90_000,
                agents_template_id: "default".into(),
                ..Settings::default()
            })
            .unwrap();
        (store.list().unwrap(), settings)
    };

    let restarted = env.store();
    assert_eq!(restarted.list().unwrap(), projects);
    assert_eq!(restarted.get_settings().unwrap(), settings);
    assert_eq!(
        restarted.active_project().unwrap(),
        None,
        "active project is in-memory only"
    );
}

#[test]
fn settings_default_and_validate() {
    let env = Env::new();
    let store = env.store();
    assert_eq!(
        store.get_settings().unwrap(),
        Settings {
            max_concurrent_agents: 3,
            default_projects_dir: None,
            claude_path: None,
            default_model: None,
            generation_timeout_ms: 300_000,
            agents_template_id: "default".into(),
            theme: Theme::System,
            reduced_motion_override: false,
            auto_install_hooks: true,
            default_verify_mode: VerifyMode::Manual,
            os_notifications_enabled: true,
            os_notification_severity: NotificationSeverity::Danger,
        }
    );
    let zero = store
        .update_settings(Settings {
            max_concurrent_agents: 0,
            ..Settings::default()
        })
        .unwrap_err();
    assert_eq!(zero.code, ErrorCode::InvalidInput);
    let relative = store
        .update_settings(Settings {
            default_projects_dir: Some("projects".into()),
            ..Settings::default()
        })
        .unwrap_err();
    assert_eq!(relative.code, ErrorCode::InvalidPath);
    let blank = store
        .update_settings(Settings {
            claude_path: Some("  ".into()),
            ..Settings::default()
        })
        .unwrap();
    assert_eq!(blank.claude_path, None);
    let no_time = store
        .update_settings(Settings {
            generation_timeout_ms: 0,
            ..Settings::default()
        })
        .unwrap_err();
    assert_eq!(no_time.code, ErrorCode::InvalidInput);
    let model = store
        .update_settings(Settings {
            default_model: Some("  opus  ".into()),
            ..Settings::default()
        })
        .unwrap();
    assert_eq!(model.default_model.as_deref(), Some("opus"));
    let blank_model = store
        .update_settings(Settings {
            default_model: Some(" ".into()),
            ..Settings::default()
        })
        .unwrap();
    assert_eq!(blank_model.default_model, None);
    for ok in [
        "sonnet",
        "claude-opus-4-1",
        "opus[1m]",
        "us.anthropic.model-v1:0",
    ] {
        let saved = store
            .update_settings(Settings {
                default_model: Some(ok.into()),
                ..Settings::default()
            })
            .unwrap();
        assert_eq!(saved.default_model.as_deref(), Some(ok));
    }
    for bad in ["a&b", "x y", "50%", "a|b", "\"q\"", "m\nn", "émoji"] {
        let err = store
            .update_settings(Settings {
                default_model: Some(bad.into()),
                ..Settings::default()
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{bad:?}");
    }
}

#[test]
fn settings_files_missing_fields_take_defaults() {
    let env = Env::new();
    fs::write(
        env.data.path().join("settings.json"),
        r#"{ "claudePath": "/opt/tools/claude" }"#,
    )
    .unwrap();
    let settings = env.store().get_settings().unwrap();
    assert_eq!(settings.max_concurrent_agents, 3);
    assert_eq!(settings.default_projects_dir, None);
    assert_eq!(settings.claude_path.as_deref(), Some("/opt/tools/claude"));
    assert_eq!(settings.default_model, None);
    assert_eq!(settings.generation_timeout_ms, 300_000, "unset → 5 minutes");
    assert_eq!(
        settings.agents_template_id, "default",
        "unset → the one template"
    );

    fs::write(
        env.data.path().join("settings.json"),
        r#"{ "generationTimeoutMs": 0, "defaultModel": "haiku" }"#,
    )
    .unwrap();
    let settings = env.store().get_settings().unwrap();
    assert_eq!(settings.generation_timeout_ms, 300_000, "0 → 5 minutes");
    assert_eq!(settings.default_model.as_deref(), Some("haiku"));

    // A hand-edited model outside the allowed characters is ignored on load.
    fs::write(
        env.data.path().join("settings.json"),
        r#"{ "defaultModel": "x&y" }"#,
    )
    .unwrap();
    assert_eq!(env.store().get_settings().unwrap().default_model, None);
}

#[test]
fn an_old_settings_file_loads_every_app_wide_default() {
    let env = Env::new();
    // Written before any of the app-wide fields existed.
    fs::write(
        env.data.path().join("settings.json"),
        r#"{ "maxConcurrentAgents": 4, "defaultProjectsDir": null, "claudePath": null }"#,
    )
    .unwrap();
    assert_eq!(
        env.store().get_settings().unwrap(),
        Settings {
            max_concurrent_agents: 4,
            ..Settings::default()
        }
    );
    let defaults = Settings::default();
    assert_eq!(defaults.default_model, None);
    assert_eq!(defaults.generation_timeout_ms, 300_000);
    assert_eq!(defaults.agents_template_id, DEFAULT_AGENTS_TEMPLATE_ID);
    assert_eq!(defaults.theme, Theme::System);
    assert!(!defaults.reduced_motion_override);
    assert!(defaults.auto_install_hooks);
    assert_eq!(defaults.default_verify_mode, VerifyMode::Manual);
    assert!(defaults.os_notifications_enabled);
    assert_eq!(
        defaults.os_notification_severity,
        NotificationSeverity::Danger
    );
}

#[test]
fn update_settings_round_trips_every_app_wide_field() {
    let env = Env::new();
    let wanted = Settings {
        theme: Theme::Dark,
        reduced_motion_override: true,
        auto_install_hooks: false,
        default_verify_mode: VerifyMode::Auto,
        os_notifications_enabled: false,
        os_notification_severity: NotificationSeverity::WarnAndDanger,
        ..Settings::default()
    };
    let saved = env.store().update_settings(wanted.clone()).unwrap();
    assert_eq!(saved, wanted);
    assert_eq!(
        env.store().get_settings().unwrap(),
        wanted,
        "after a restart"
    );

    // On disk and over the wire: the camelCase names and snake_case values the TS type lists.
    let file: serde_json::Value =
        serde_json::from_slice(&fs::read(env.data.path().join("settings.json")).unwrap()).unwrap();
    for (key, value) in [
        ("theme", json!("dark")),
        ("reducedMotionOverride", json!(true)),
        ("autoInstallHooks", json!(false)),
        ("defaultVerifyMode", json!("auto")),
        ("osNotificationsEnabled", json!(false)),
        ("osNotificationSeverity", json!("warn_and_danger")),
    ] {
        assert_eq!(file[key], value, "{key}");
    }

    for (theme, severity) in [
        (Theme::Light, NotificationSeverity::All),
        (Theme::System, NotificationSeverity::Danger),
    ] {
        let next = Settings {
            theme: theme.clone(),
            os_notification_severity: severity.clone(),
            ..Settings::default()
        };
        let saved = env.store().update_settings(next).unwrap();
        assert_eq!(
            (saved.theme, saved.os_notification_severity),
            (theme, severity)
        );
    }
}

/// Decodes `Settings` the way Tauri decodes the `update_settings` argument.
fn from_frontend(patch: serde_json::Value) -> Settings {
    let mut value = serde_json::to_value(Settings::default()).unwrap();
    for (key, v) in patch.as_object().unwrap() {
        value[key] = v.clone();
    }
    serde_json::from_value(value).expect("a bad enum value still decodes")
}

#[test]
fn update_settings_rejects_a_bad_value_for_each_enum() {
    let env = Env::new();
    let store = env.store();
    let before = fs::read(env.data.path().join("settings.json")).ok();
    for (patch, field, allowed) in [
        (
            json!({ "theme": "neon" }),
            "theme",
            r#""light", "dark", "system""#,
        ),
        (
            json!({ "theme": 5 }),
            "theme",
            r#""light", "dark", "system""#,
        ),
        (
            json!({ "defaultVerifyMode": "sometimes" }),
            "defaultVerifyMode",
            r#""auto", "manual""#,
        ),
        (
            json!({ "osNotificationSeverity": "warn" }),
            "osNotificationSeverity",
            r#""danger", "warn_and_danger", "all""#,
        ),
    ] {
        let err = store
            .update_settings(from_frontend(patch.clone()))
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{patch}");
        let bad = &patch[field];
        assert_eq!(
            err.message,
            format!("{field} must be one of {allowed}, not {bad}")
        );
    }
    assert_eq!(store.get_settings().unwrap(), Settings::default());
    assert_eq!(fs::read(env.data.path().join("settings.json")).ok(), before);
}

#[test]
fn a_hand_edited_bad_enum_in_the_file_loads_as_its_default() {
    let env = Env::new();
    fs::write(
        env.data.path().join("settings.json"),
        r#"{ "theme": "neon", "defaultVerifyMode": 3, "osNotificationSeverity": "loud",
             "autoInstallHooks": false, "maxConcurrentAgents": 2 }"#,
    )
    .unwrap();
    assert_eq!(
        env.store().get_settings().unwrap(),
        Settings {
            auto_install_hooks: false,
            max_concurrent_agents: 2,
            ..Settings::default()
        },
        "only the bad enums fall back; the rest of the file is kept"
    );
    let aside = fs::read_dir(env.data.path())
        .unwrap()
        .filter_map(Result::ok)
        .any(|e| e.file_name().to_string_lossy().contains(".corrupt-"));
    assert!(!aside, "a bad enum isn't treated as a corrupt file");
}

#[test]
fn corrupt_registry_is_set_aside_not_lost() {
    let env = Env::new();
    fs::write(env.data.path().join("projects.json"), "{ not json").unwrap();

    let store = env.store();

    assert!(store.list().unwrap().is_empty());
    let aside: Vec<String> = fs::read_dir(env.data.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("projects.json.corrupt-"))
        .collect();
    assert_eq!(aside.len(), 1);
    assert_eq!(
        fs::read_to_string(env.data.path().join(&aside[0])).unwrap(),
        "{ not json"
    );
}

// ---------- R-1-7 ----------
// The allowlist itself is proven in core::repo_write::tests
// (rejects_writes_outside_the_allowlist_and_touches_nothing). Here: projects can't be used
// to write through a link that escapes the repo.

#[test]
fn link_refuses_to_write_through_an_escaping_foundry_link() {
    let env = Env::new();
    let repo = TestRepo::new();
    let outside = tempfile::tempdir().unwrap();
    crate::testing::link_dir(outside.path(), &repo.path().join(".foundry"));

    let err = link(&env.store(), repo.path()).unwrap_err();

    assert_eq!(err.code, ErrorCode::PathEscape);
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
}

// ---------- R-1-8 ----------

#[test]
fn create_makes_foundry_files_an_empty_first_commit_and_a_gitignore_commit() {
    let env = Env::new();
    let parent = TestRepo::new();
    let created = create(&env.store(), parent.path(), "Fresh");
    let repo = Path::new(&created.project.repo_path);

    assert!(created.git_initialized);
    assert!(created.initial_commit);
    assert_eq!(
        without_git(&snapshot(repo)),
        [
            ".foundry",
            ".foundry/project.json",
            ".gitignore",
            "docs",
            "docs/brain-dump.md",
            "vsm_viewer.html"
        ]
    );
    assert_eq!(
        fs::read_to_string(repo.join(".gitignore")).unwrap(),
        "node_modules/\ndist/\n.env\n*.db\ntest-results/\nplaywright-report/\n*.log\n"
    );
    assert!(repo.join(".git").is_dir());
    // One root commit, and it's empty; the one follow-up commit holds only .gitignore.
    let commits = git_in(repo, &["rev-list", "--count", "HEAD"]);
    assert_eq!(String::from_utf8_lossy(&commits.stdout).trim(), "2");
    let root = git_in(repo, &["rev-list", "--max-parents=0", "HEAD"]);
    let root = String::from_utf8_lossy(&root.stdout).trim().to_string();
    assert_eq!(root.lines().count(), 1);
    let tree = git_in(repo, &["ls-tree", "-r", "--name-only", &root]);
    assert!(tree.stdout.is_empty(), "initial commit must be empty");
    let tree = git_in(repo, &["ls-tree", "-r", "--name-only", "HEAD"]);
    assert_eq!(String::from_utf8_lossy(&tree.stdout), ".gitignore\n");

    // The Foundry files stay untracked, and the runtime files are excluded locally.
    assert_eq!(
        fs::read_to_string(repo.join(".git/info/exclude"))
            .unwrap()
            .lines()
            .filter(|line| GIT_EXCLUDES.contains(line))
            .count(),
        GIT_EXCLUDES.len()
    );
    let status = git_in(repo, &["status", "--porcelain", "--untracked-files=all"]);
    let status = String::from_utf8_lossy(&status.stdout);
    assert!(status.contains(".foundry/project.json"), "{status}");
    assert!(!status.contains(".gitignore"), "{status}");
}

#[test]
fn create_without_git_succeeds_and_reports_it() {
    let env = Env::new();
    let parent = TestRepo::new();
    let store = env.store_with(Git::new("foundry-definitely-not-git"));

    let created = create(&store, parent.path(), "No Git");

    assert!(!created.git_initialized);
    assert!(!created.initial_commit);
    let repo = Path::new(&created.project.repo_path);
    assert_eq!(
        without_git(&snapshot(repo)),
        [
            ".foundry",
            ".foundry/project.json",
            ".gitignore",
            "docs",
            "docs/brain-dump.md",
            "vsm_viewer.html"
        ]
    );
    assert!(!repo.join(".git").exists());
}

#[test]
fn create_without_git_identity_succeeds_without_a_commit() {
    let env = Env::new();
    let parent = TestRepo::new();
    let store = env.store_with(env.git_without_identity());

    let created = create(&store, parent.path(), "No Identity");

    assert!(created.git_initialized);
    assert!(!created.initial_commit);
    let repo = Path::new(&created.project.repo_path);
    assert!(repo.join(".git").is_dir());
    assert!(!git_in(repo, &["rev-parse", "--verify", "HEAD"])
        .status
        .success());
}

// ---------- git noise ----------

#[test]
fn link_adds_git_excludes_once_and_never_touches_the_gitignore() {
    let env = Env::new();
    let repo = TestRepo::with_git();
    repo.write(".gitignore", "target/\n");
    repo.write(".git/info/exclude", "# mine\n*.swp\n");
    let gitignore = fs::read(repo.path().join(".gitignore")).unwrap();
    let expected = "# mine\n*.swp\n.foundry/events/\n.foundry/reviews/digest.md\n";

    let store = env.store();
    let linked = link(&store, repo.path()).unwrap();
    assert!(linked.already_tracked.is_empty());
    assert_eq!(repo.read(".git/info/exclude"), expected);
    assert_eq!(fs::read(repo.path().join(".gitignore")).unwrap(), gitignore);

    // Linking the same repo again adds nothing twice.
    store.archive_project(&linked.project.id).unwrap();
    store.remove_project(&linked.project.id).unwrap();
    link(&store, repo.path()).unwrap();
    assert_eq!(repo.read(".git/info/exclude"), expected);
    assert_eq!(fs::read(repo.path().join(".gitignore")).unwrap(), gitignore);

    // The runtime files go quiet; the verdict files stay visible to git.
    repo.write(".foundry/events/ada.jsonl", "{}\n");
    repo.write(".foundry/reviews/digest.md", "# digest\n");
    repo.write(".foundry/reviews/s-1.json", "{}\n");
    repo.write(".foundry/reports/s-1.json", "{}\n");
    let others = repo.git(&["ls-files", "--others", "--exclude-standard"]);
    assert!(!others.contains(".foundry/events/"), "{others}");
    assert!(!others.contains("digest.md"), "{others}");
    assert!(others.contains(".foundry/reviews/s-1.json"), "{others}");
    assert!(others.contains(".foundry/reports/s-1.json"), "{others}");
}

#[test]
fn link_creates_info_exclude_when_missing() {
    let env = Env::new();
    let repo = TestRepo::with_git();
    fs::remove_dir_all(repo.path().join(".git/info")).ok();

    link(&env.store(), repo.path()).unwrap();
    assert_eq!(
        repo.read(".git/info/exclude"),
        ".foundry/events/\n.foundry/reviews/digest.md\n"
    );
}

#[test]
fn link_leaves_already_tracked_event_files_tracked_and_reports_them() {
    let env = Env::new();
    let repo = TestRepo::with_git();
    let runtime = [
        ".foundry/events/ada.jsonl",
        ".foundry/events/read.json",
        ".foundry/reviews/digest.md",
    ];
    for rel in runtime {
        repo.write(rel, "{}\n");
    }
    repo.write(".foundry/reviews/s-1.json", "{}\n");
    repo.git(&["add", "."]);
    repo.git(&["commit", "--quiet", "-m", "runtime files"]);

    let linked = link(&env.store(), repo.path()).unwrap();
    assert_eq!(linked.already_tracked, runtime);
    let tracked = repo.git(&["ls-files"]);
    for rel in runtime {
        assert!(tracked.contains(rel), "{rel} untracked: {tracked}");
    }
    // Excluding doesn't hide a change to a tracked file.
    repo.write(".foundry/events/ada.jsonl", "{}\n{}\n");
    let status = repo.git(&["status", "--porcelain"]);
    assert!(status.contains(".foundry/events/ada.jsonl"), "{status}");
}

#[test]
fn link_of_a_folder_without_git_adds_no_git_files() {
    let env = Env::new();
    let folder = TestRepo::new();
    let linked = link(&env.store(), folder.path()).unwrap();
    assert!(!linked.is_git_repo);
    assert!(linked.already_tracked.is_empty());
    assert!(!folder.path().join(".git").exists());
    assert!(!folder.path().join(".gitignore").exists());
}

#[test]
fn create_uses_default_projects_dir_or_explains_what_is_missing() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let input = || CreateRepositoryInput {
        name: "Defaulted".into(),
        parent_dir: None,
    };

    assert_eq!(
        store.create_repository(input()).unwrap_err().code,
        ErrorCode::InvalidPath
    );
    store
        .update_settings(Settings {
            default_projects_dir: Some(parent.path().to_string_lossy().into_owned()),
            ..Settings::default()
        })
        .unwrap();
    let created = store.create_repository(input()).unwrap();
    assert_eq!(
        Path::new(&created.project.repo_path),
        parent.path().join("defaulted")
    );
}

#[test]
fn create_rejects_unusable_names_without_touching_disk() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    for name in ["", "   ", "!!!", "con"] {
        let err = store
            .create_repository(CreateRepositoryInput {
                name: name.into(),
                parent_dir: Some(parent.path().to_string_lossy().into_owned()),
            })
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput, "{name:?}");
    }
    assert!(snapshot(parent.path()).is_empty());
}

// ---------- R-1-9 ----------

#[test]
fn update_project_never_touches_the_folder() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Before").project;
    let repo = Path::new(&project.repo_path);
    // A field from a later phase must survive the edit.
    let mut json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(repo.join(".foundry/project.json")).unwrap())
            .unwrap();
    json["testCommands"] = serde_json::json!(["cargo test"]);
    json["laterField"] = serde_json::json!({ "keep": true });
    fs::write(repo.join(".foundry/project.json"), json.to_string()).unwrap();
    fs::write(repo.join("notes.txt"), "untouched").unwrap();
    let before = snapshot(parent.path());

    let updated = store
        .update_project(
            &project.id,
            ProjectPatch {
                name: Some("After".into()),
                tagline: Some("New tagline".into()),
            },
        )
        .unwrap();

    assert_eq!(
        updated.repo_path, project.repo_path,
        "folder path unchanged"
    );
    assert_eq!(updated.name, "After");
    assert_eq!(updated.tagline, "New tagline");
    let after = snapshot(parent.path());
    assert_eq!(
        after.keys().collect::<Vec<_>>(),
        before.keys().collect::<Vec<_>>(),
        "no folder renamed, moved, added or removed"
    );
    for (rel, contents) in &before {
        if !rel.ends_with(".foundry/project.json") {
            assert_eq!(after.get(rel), Some(contents), "{rel} changed");
        }
    }
    let file: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(repo.join(".foundry/project.json")).unwrap())
            .unwrap();
    assert_eq!(file["name"], "After");
    assert_eq!(file["tagline"], "New tagline");
    assert_eq!(file["id"], json["id"]);
    assert_eq!(file["createdAt"], json["createdAt"]);
    assert_eq!(file["testCommands"], serde_json::json!(["cargo test"]));
    assert_eq!(file["laterField"], serde_json::json!({ "keep": true }));
}

#[test]
fn update_project_validates_and_reports_unknown_ids() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Named").project;
    let blank = store
        .update_project(
            &project.id,
            ProjectPatch {
                name: Some("  ".into()),
                tagline: None,
            },
        )
        .unwrap_err();
    assert_eq!(blank.code, ErrorCode::InvalidInput);
    assert_eq!(store.list().unwrap()[0].name, "Named");
    assert_eq!(
        store
            .update_project("missing", ProjectPatch::default())
            .unwrap_err()
            .code,
        ErrorCode::NotFound
    );
}

// ---------- R-1-10 ----------

#[test]
fn vsm_viewer_is_byte_identical_and_never_overwritten() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Viewer").project;
    let viewer = Path::new(&project.repo_path).join("vsm_viewer.html");
    assert_eq!(fs::read(&viewer).unwrap(), VSM_VIEWER);
    assert!(!VSM_VIEWER.is_empty());

    // The project's copy changes (say, a newer viewer mid-build): nothing may replace it.
    fs::write(&viewer, "<!-- project's own copy -->").unwrap();

    let again = store.create_repository(CreateRepositoryInput {
        name: "Viewer".into(),
        parent_dir: Some(parent.path().to_string_lossy().into_owned()),
    });
    assert_eq!(again.unwrap_err().code, ErrorCode::FolderExists);
    store.archive_project(&project.id).unwrap();
    store.remove_project(&project.id).unwrap();
    link(&store, Path::new(&project.repo_path)).unwrap();
    let other = env.store();
    other
        .relink_project(&store.list().unwrap()[0].id, &project.repo_path)
        .unwrap();

    assert_eq!(
        fs::read_to_string(&viewer).unwrap(),
        "<!-- project's own copy -->"
    );
}

// ---------- relink ----------

#[test]
fn relink_points_a_moved_project_at_its_new_folder() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Mover").project;
    let moved = parent.path().join("moved-here");
    fs::rename(&project.repo_path, &moved).unwrap();
    assert!(!store.list().unwrap()[0].folder_exists);

    let relinked = store
        .relink_project(&project.id, &moved.to_string_lossy())
        .unwrap();

    assert_eq!(Path::new(&relinked.repo_path), moved);
    assert!(relinked.folder_exists);
    store.open_project(&project.id).unwrap();
}

#[test]
fn relink_adds_git_excludes_once_and_keeps_existing_lines() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let project = create(&store, parent.path(), "Mover").project;
    let moved = parent.path().join("moved-here");
    fs::rename(&project.repo_path, &moved).unwrap();
    assert!(moved.join(".git").is_dir(), "create made a git repo");
    // The user's own lines, CRLF, with one of ours already present but out of order.
    let exclude = moved.join(".git/info/exclude");
    fs::write(
        &exclude,
        "# mine
.foundry/reviews/digest.md
*.swp
",
    )
    .unwrap();
    let gitignore = fs::read(moved.join(".gitignore")).unwrap();

    store
        .relink_project(&project.id, &moved.to_string_lossy())
        .unwrap();
    let expected = "# mine
.foundry/reviews/digest.md
*.swp
.foundry/events/
";
    assert_eq!(fs::read_to_string(&exclude).unwrap(), expected);

    // Relinking again (both lines now present) changes nothing.
    store
        .relink_project(&project.id, &moved.to_string_lossy())
        .unwrap();
    assert_eq!(fs::read_to_string(&exclude).unwrap(), expected);
    assert_eq!(fs::read(moved.join(".gitignore")).unwrap(), gitignore);
}

#[test]
fn relink_refuses_another_projects_folder() {
    let env = Env::new();
    let store = env.store();
    let parent = TestRepo::new();
    let a = create(&store, parent.path(), "A").project;
    let b = create(&store, parent.path(), "B").project;

    let taken = store.relink_project(&a.id, &b.repo_path).unwrap_err();
    assert_eq!(taken.code, ErrorCode::AlreadyLinked);

    store.archive_project(&b.id).unwrap();

    store.remove_project(&b.id).unwrap();
    let foreign = store.relink_project(&a.id, &b.repo_path).unwrap_err();
    assert_eq!(
        foreign.code,
        ErrorCode::InvalidInput,
        "B's folder still has B's project.json"
    );
    assert_eq!(store.list().unwrap()[0].repo_path, a.repo_path);
}

// ---------- repo index (for other features) ----------

#[test]
fn repo_index_follows_create_relink_remove_and_restart() {
    use std::sync::Arc;

    use crate::core::repos::RepoIndex;

    let env = Env::new();
    let parent = TestRepo::new();
    let repos = Arc::new(RepoIndex::default());
    let store = ProjectsStore::load(
        env.data.path(),
        env.git_with_identity(),
        Arc::clone(&repos),
        Default::default(),
    )
    .unwrap();
    let project = create(&store, parent.path(), "Indexed").project;
    assert_eq!(
        repos.repo(&project.id).unwrap(),
        Path::new(&project.repo_path)
    );

    let moved = parent.path().join("moved");
    fs::rename(&project.repo_path, &moved).unwrap();
    assert_eq!(
        repos.repo(&project.id).unwrap_err().code,
        ErrorCode::FolderMissing
    );
    store
        .relink_project(&project.id, &moved.to_string_lossy())
        .unwrap();
    assert_eq!(repos.repo(&project.id).unwrap(), moved);

    // A fresh load publishes what's on disk.
    let fresh = Arc::new(RepoIndex::default());
    let reloaded = ProjectsStore::load(
        env.data.path(),
        env.git_with_identity(),
        Arc::clone(&fresh),
        Default::default(),
    )
    .unwrap();
    assert_eq!(fresh.repo(&project.id).unwrap(), moved);

    reloaded.archive_project(&project.id).unwrap();
    reloaded.remove_project(&project.id).unwrap();
    assert_eq!(
        fresh.repo(&project.id).unwrap_err().code,
        ErrorCode::NotFound
    );
}

// ---------- commands over IPC ----------

#[test]
fn commands_round_trip_over_ipc() {
    use std::sync::Arc;
    use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::{Manager, WebviewWindowBuilder};

    let env = Env::new();
    let parent = TestRepo::new();
    let app = crate::app_builder(mock_builder())
        .build(mock_context(noop_assets()))
        .unwrap();
    app.manage(Arc::new(env.store()));
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let invoke = |cmd: &str, body: serde_json::Value| {
        tauri::test::get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: cmd.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: tauri::ipc::InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<serde_json::Value>().unwrap())
    };

    let created = invoke(
        "create_repository",
        serde_json::json!({ "input": { "name": "Over Ipc", "parentDir": parent.path() } }),
    )
    .unwrap();
    assert_eq!(created["gitInitialized"], true);
    let id = created["project"]["id"].as_str().unwrap().to_string();

    let listed = invoke("list_projects", serde_json::json!({})).unwrap();
    assert_eq!(listed[0]["name"], "Over Ipc");

    invoke(
        "save_document",
        serde_json::json!({ "id": id, "markdown": "# Hello" }),
    )
    .unwrap();
    let doc = invoke("get_document", serde_json::json!({ "id": id })).unwrap();
    assert_eq!(doc, "# Hello");

    let exists = invoke(
        "create_repository",
        serde_json::json!({ "input": { "name": "Over Ipc", "parentDir": parent.path() } }),
    )
    .unwrap_err();
    assert_eq!(exists["code"], "folder_exists");

    let not_archived = invoke("remove_project", serde_json::json!({ "id": id })).unwrap_err();
    assert_eq!(not_archived["code"], "not_archived");
    invoke("open_project", serde_json::json!({ "id": id })).unwrap();
    let archived = invoke("archive_project", serde_json::json!({ "id": id })).unwrap();
    assert_eq!(archived["archived"], true);
    assert_eq!(
        invoke("active_project", serde_json::json!({})).unwrap(),
        serde_json::Value::Null
    );
    let listed = invoke("list_projects", serde_json::json!({})).unwrap();
    assert_eq!(listed[0]["archived"], true);
    let restored = invoke("unarchive_project", serde_json::json!({ "id": id })).unwrap();
    assert_eq!(restored["archived"], false);
    invoke("archive_project", serde_json::json!({ "id": id })).unwrap();
    invoke("remove_project", serde_json::json!({ "id": id })).unwrap();

    let doomed = invoke(
        "create_repository",
        serde_json::json!({ "input": { "name": "Ipc Delete", "parentDir": parent.path() } }),
    )
    .unwrap();
    let doomed_id = doomed["project"]["id"].as_str().unwrap().to_string();
    let doomed_path = doomed["project"]["repoPath"].as_str().unwrap().to_string();
    let refused = invoke("delete_repository", serde_json::json!({ "id": doomed_id })).unwrap_err();
    assert_eq!(refused["code"], "not_archived");
    invoke("archive_project", serde_json::json!({ "id": doomed_id })).unwrap();
    fs::remove_dir_all(&doomed_path).unwrap();
    let deleted = invoke("delete_repository", serde_json::json!({ "id": doomed_id })).unwrap();
    assert_eq!(deleted, serde_json::Value::Null);
    assert_eq!(
        invoke("list_projects", serde_json::json!({})).unwrap(),
        serde_json::json!([])
    );
}
