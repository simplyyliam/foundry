use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use super::*;
use crate::core::git::Git;
use crate::core::ErrorCode;
use crate::testing::{install_fake_claude, TestRepo};

const P: &str = "p1";

struct Env {
    store: HooksStore,
    repo: TestRepo,
    helper: PathBuf,
    claude_dir: TestRepo,
    _helper_dir: TestRepo,
    _git_home: TestRepo,
}

impl Env {
    /// A repo, a stand-in helper in a folder with spaces, and a fake `claude` printing
    /// `version` (`None`: no `claude` at all).
    fn new(version: Option<&str>) -> Self {
        Self::with_repo(TestRepo::new(), version)
    }

    fn with_repo(repo: TestRepo, version: Option<&str>) -> Self {
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([(P, repo.path())]);
        let helper_dir = TestRepo::new();
        let helper = helper_dir.write("Foundry App/bin dir/foundry-hook.exe", "stand-in");
        let claude_dir = TestRepo::new();
        let claude = match version {
            Some(version) => {
                let claude = install_fake_claude(claude_dir.path());
                std::fs::write(claude_dir.path().join("version.txt"), version).unwrap();
                claude
            }
            None => claude_dir.path().join("claude-missing.exe"),
        };
        let for_helper = helper.clone();
        // Git without the user's global config or global excludes file.
        let git_home = TestRepo::new();
        let empty_config = git_home.write("gitconfig", "");
        let git = Git::system()
            .env("GIT_CONFIG_GLOBAL", &empty_config)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("XDG_CONFIG_HOME", git_home.path())
            .env("HOME", git_home.path());
        let store = HooksStore::new(
            repos,
            git,
            move || Some(for_helper.clone()),
            move || crate::core::claude::find(Some(&claude.to_string_lossy()), None),
        );
        Self {
            store,
            repo,
            helper,
            claude_dir,
            _helper_dir: helper_dir,
            _git_home: git_home,
        }
    }

    fn settings(&self) -> Value {
        serde_json::from_str(&self.repo.read(SETTINGS_FILE)).unwrap()
    }

    fn helper(&self) -> String {
        self.helper.to_string_lossy().into_owned()
    }
}

const SUPPORTED: Option<&str> = Some("2.1.292 (Claude Code)\n");

fn user_settings() -> Value {
    json!({
        "permissions": { "allow": ["Bash(npm test)"] },
        "hooks": {
            "PreToolUse": [
                { "matcher": "Bash", "hooks": [{ "type": "command", "command": "my-lint.sh" }] }
            ],
            "UserPromptSubmit": [
                { "hooks": [{ "type": "command", "command": "log-prompt.sh", "timeout": 5 }] }
            ],
            "Stop": []
        },
        "model": "opus"
    })
}

fn user_hooks_kept(settings: &Value) {
    assert_eq!(
        settings["permissions"],
        json!({ "allow": ["Bash(npm test)"] })
    );
    assert_eq!(settings["model"], "opus");
    assert_eq!(
        settings["hooks"]["PreToolUse"][0],
        json!({ "matcher": "Bash", "hooks": [{ "type": "command", "command": "my-lint.sh" }] })
    );
    assert_eq!(
        settings["hooks"]["UserPromptSubmit"],
        json!([{ "hooks": [{ "type": "command", "command": "log-prompt.sh", "timeout": 5 }] }])
    );
}

// ---------- install ----------

#[test]
fn install_writes_exec_form_entries_for_every_event_with_the_unquoted_path() {
    let env = Env::new(SUPPORTED);
    let change = env.store.install(P).unwrap();
    assert!(change.changed);
    assert_eq!(change.backup, None, "nothing to back up");
    assert!(env.helper().contains(' '), "the helper path has spaces");

    let settings = env.settings();
    for (event, matcher) in EVENTS {
        let groups = settings["hooks"][event].as_array().unwrap();
        assert_eq!(groups.len(), 1, "{event}");
        let hook = &groups[0]["hooks"][0];
        assert_eq!(hook["type"], "command");
        assert_eq!(
            hook["command"],
            env.helper(),
            "verbatim: exec form needs no quotes"
        );
        assert_eq!(hook["args"], json!(["hook"]));
        assert_eq!(
            groups[0].get("matcher").and_then(Value::as_str),
            *matcher,
            "{event}"
        );
    }
    let status = change.status;
    assert!(status.installed && !status.outdated);
    assert_eq!(status.events.len(), EVENTS.len());
    assert_eq!(
        status.claude_version.as_deref(),
        Some("2.1.292 (Claude Code)")
    );
    assert!(status.claude_version_supported);
    assert_eq!(status.min_claude_version, "2.1.139");
    assert!(status.helper_exists && status.settings_valid);
}

#[test]
fn install_preserves_user_hooks_and_is_idempotent() {
    let env = Env::new(SUPPORTED);
    env.repo.write(
        SETTINGS_FILE,
        &serde_json::to_string_pretty(&user_settings()).unwrap(),
    );

    let first = env.store.install(P).unwrap();
    assert!(first.changed);
    let backup = first.backup.unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&env.repo.read(&backup)).unwrap(),
        user_settings(),
        "the backup is the file as it was"
    );
    let settings = env.settings();
    user_hooks_kept(&settings);
    let pre = settings["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre.len(), 2, "the user's group, then Foundry's");
    assert_eq!(pre[1]["hooks"][0]["command"], env.helper());
    assert_eq!(settings["hooks"]["Stop"].as_array().unwrap().len(), 1);

    let bytes = env.repo.read(SETTINGS_FILE);
    let again = env.store.install(P).unwrap();
    assert_eq!((again.changed, again.backup), (false, None));
    assert_eq!(env.repo.read(SETTINGS_FILE), bytes, "byte for byte");
    assert!(again.status.installed);

    // A user group added after Foundry's stays where it is on reinstall.
    let mut edited = env.settings();
    edited["hooks"]["PreToolUse"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "matcher": "Write", "hooks": [{ "type": "command", "command": "fmt.sh" }] }));
    env.repo.write(SETTINGS_FILE, &edited.to_string());
    assert!(!env.store.install(P).unwrap().changed);
    assert_eq!(
        env.settings()["hooks"]["PreToolUse"][2]["hooks"][0]["command"],
        "fmt.sh"
    );
}

#[test]
fn install_replaces_an_outdated_foundry_entry_and_nothing_else() {
    let env = Env::new(SUPPORTED);
    let mut settings = user_settings();
    // An old install: another helper path, shell form, sharing a group with a user hook.
    settings["hooks"]["PostToolUse"] = json!([{
        "matcher": "*",
        "hooks": [
            { "type": "command", "command": "\"C:/Old Place/foundry-hook.exe\"" },
            { "type": "command", "command": "user-after.sh" }
        ]
    }]);
    settings["hooks"]["SubagentStop"] =
        json!([{ "hooks": [{ "type": "command", "command": "/opt/foundry-hook" }] }]);
    env.repo.write(SETTINGS_FILE, &settings.to_string());

    let before = env.store.status(P).unwrap();
    assert!(before.outdated && !before.installed);
    assert!(before.events.contains(&"PostToolUse".to_string()));

    env.store.install(P).unwrap();
    let after = env.settings();
    user_hooks_kept(&after);
    let post = after["hooks"]["PostToolUse"].as_array().unwrap();
    assert_eq!(
        post[0],
        json!({ "matcher": "*", "hooks": [{ "type": "command", "command": "user-after.sh" }] }),
        "the old Foundry hook left the shared group; the user's stayed"
    );
    assert_eq!(post[1]["hooks"][0]["command"], env.helper());
    assert!(
        after["hooks"].get("SubagentStop").is_none(),
        "a stray Foundry-only event is gone"
    );
    assert!(env.store.status(P).unwrap().installed);
}

// ---------- uninstall ----------

#[test]
fn uninstall_removes_only_foundrys_entries() {
    let env = Env::new(SUPPORTED);
    env.repo.write(SETTINGS_FILE, &user_settings().to_string());
    env.store.install(P).unwrap();

    let removed = env.store.uninstall(P).unwrap();
    assert!(removed.changed && removed.backup.is_some());
    let settings = env.settings();
    user_hooks_kept(&settings);
    // A list that Foundry's removal empties goes, even one the user had left empty before.
    for event in [
        "SessionStart",
        "PostToolUse",
        "PostToolUseFailure",
        "Notification",
        "Stop",
        "SessionEnd",
    ] {
        assert!(
            settings["hooks"].get(event).is_none(),
            "{event} was only Foundry's"
        );
    }
    assert_eq!(settings["hooks"]["PreToolUse"].as_array().unwrap().len(), 1);
    assert!(!env.repo.read(SETTINGS_FILE).contains(MARKER));
    let status = removed.status;
    assert!(!status.installed && !status.outdated && status.events.is_empty());

    let bytes = env.repo.read(SETTINGS_FILE);
    let again = env.store.uninstall(P).unwrap();
    assert_eq!((again.changed, again.backup), (false, None));
    assert_eq!(env.repo.read(SETTINGS_FILE), bytes);
}

#[test]
fn install_then_uninstall_of_a_new_file_leaves_an_empty_object() {
    let env = Env::new(SUPPORTED);
    env.store.install(P).unwrap();
    env.store.uninstall(P).unwrap();
    assert_eq!(env.settings(), json!({}));
    // Uninstalling where nothing was ever installed creates nothing.
    let fresh = Env::new(None);
    assert!(!fresh.store.uninstall(P).unwrap().changed);
    assert!(!fresh.repo.path().join(".claude").exists());
}

// ---------- refusals ----------

#[test]
fn invalid_json_is_refused_untouched_by_install_and_uninstall() {
    let env = Env::new(SUPPORTED);
    for bad in [
        "{ \"hooks\": ",
        "[]",
        "{\"hooks\": []}",
        "{\"hooks\": {\"Stop\": {}}}",
    ] {
        env.repo.write(SETTINGS_FILE, bad);
        assert_eq!(
            env.store.install(P).unwrap_err().code,
            ErrorCode::InvalidJson,
            "{bad}"
        );
        assert_eq!(
            env.store.uninstall(P).unwrap_err().code,
            ErrorCode::InvalidJson,
            "{bad}"
        );
        assert_eq!(env.repo.read(SETTINGS_FILE), bad, "untouched");
    }
    env.repo.write(SETTINGS_FILE, "{ nope");
    let status = env.store.status(P).unwrap();
    assert!(!status.settings_valid && !status.installed);
    assert!(
        !env.repo.path().join(".foundry").exists(),
        "no backup either"
    );
}

fn refused(env: &Env, code: ErrorCode, mentions: &[&str]) {
    let err = env.store.install(P).unwrap_err();
    assert_eq!(err.code, code, "{}", err.message);
    for word in mentions {
        assert!(err.message.contains(word), "{:?} lacks {word}", err.message);
    }
    assert!(
        !env.repo.path().join(".claude").exists(),
        "nothing installed"
    );
    assert!(!env.repo.path().join(".foundry").exists());
}

#[test]
fn install_refuses_an_old_unreadable_or_missing_claude_code() {
    let old = Env::new(Some("2.1.138 (Claude Code)\n"));
    refused(
        &old,
        ErrorCode::ClaudeVersionUnsupported,
        &["2.1.138", "2.1.139"],
    );
    let status = old.store.status(P).unwrap();
    assert_eq!(
        status.claude_version.as_deref(),
        Some("2.1.138 (Claude Code)")
    );
    assert!(!status.claude_version_supported);

    let garbage = Env::new(Some("command not found: claude\n"));
    refused(
        &garbage,
        ErrorCode::ClaudeVersionUnsupported,
        &["command not found", "2.1.139"],
    );
    assert!(!garbage.store.status(P).unwrap().claude_version_supported);

    let silent = Env::new(Some(""));
    refused(&silent, ErrorCode::ClaudeVersionUnsupported, &["2.1.139"]);

    let missing = Env::new(None);
    refused(&missing, ErrorCode::ClaudeNotFound, &["2.1.139"]);
    let status = missing.store.status(P).unwrap();
    assert_eq!(
        (status.claude_version, status.claude_version_supported),
        (None, false)
    );

    // The minimum itself is enough.
    let exact = Env::new(Some("2.1.139 (Claude Code)"));
    assert!(exact.store.install(P).unwrap().status.installed);
    assert!(exact.claude_dir.path().join("version.txt").exists());
}

#[test]
fn install_refuses_when_the_helper_is_missing() {
    let env = Env::new(SUPPORTED);
    std::fs::remove_file(&env.helper).unwrap();
    refused(
        &env,
        ErrorCode::NotFound,
        &["foundry-hook", "cargo build --bin foundry-hook"],
    );
    assert!(!env.store.status(P).unwrap().helper_exists);
}

#[test]
fn an_unknown_project_is_not_found() {
    let env = Env::new(SUPPORTED);
    for result in [
        env.store.install("nope").map(|_| ()),
        env.store.uninstall("nope").map(|_| ()),
        env.store.status("nope").map(|_| ()),
    ] {
        assert_eq!(result.unwrap_err().code, ErrorCode::NotFound);
    }
}

// ---------- git-ignore report ----------

#[test]
fn status_reports_whether_the_settings_file_is_git_ignored() {
    let not_git = Env::new(SUPPORTED);
    assert_eq!(not_git.store.status(P).unwrap().git_ignored, None);

    let tracked = Env::with_repo(TestRepo::with_git(), SUPPORTED);
    assert_eq!(
        tracked.store.install(P).unwrap().status.git_ignored,
        Some(false)
    );

    let ignored = Env::with_repo(TestRepo::with_git(), SUPPORTED);
    ignored
        .repo
        .write(".gitignore", ".claude/settings.local.json\n");
    assert_eq!(
        ignored.store.install(P).unwrap().status.git_ignored,
        Some(true)
    );
}

// ---------- the contract ----------

#[test]
fn commands_round_trip_over_ipc() {
    use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::{Manager, WebviewWindowBuilder};

    let env = Env::new(SUPPORTED);
    let helper = env.helper();
    let app = mock_builder()
        .invoke_handler(crate::specta_builder::<tauri::test::MockRuntime>().invoke_handler())
        .build(mock_context(noop_assets()))
        .unwrap();
    app.manage(Arc::new(env.store));
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let invoke = |cmd: &str| {
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
                body: tauri::ipc::InvokeBody::Json(json!({ "projectId": P })),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_string(),
            },
        )
        .map(|body| body.deserialize::<Value>().unwrap())
    };

    let status = invoke("hooks_status").unwrap();
    assert_eq!(status["installed"], false);
    assert_eq!(status["helperPath"], helper);
    assert_eq!(status["minClaudeVersion"], "2.1.139");
    assert_eq!(status["claudeVersionSupported"], true);
    assert_eq!(status["gitIgnored"], Value::Null);

    let installed = invoke("hooks_install").unwrap();
    assert_eq!(installed["changed"], true);
    assert_eq!(installed["backup"], Value::Null);
    assert_eq!(installed["status"]["installed"], true);

    let removed = invoke("hooks_uninstall").unwrap();
    assert_eq!(removed["changed"], true);
    assert_eq!(removed["status"]["installed"], false);
}
