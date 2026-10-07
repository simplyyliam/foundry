//! Black-box tests for the `foundry-hook` helper: the real binary, fed real captured hook
//! payloads (`src/features/agent_events/fixtures/`), against real temp repos, posting to the
//! real hook server.

#[path = "../src/testing.rs"]
mod testing;

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use app_lib::core::hook::HookEndpointCell;
use app_lib::core::repos::RepoIndex;
use app_lib::core::sessions::SessionRegistry;
use app_lib::features::agent_events::{
    server, AgentEvent, AgentEventSink, AgentEventsQuery, AgentEventsStore, EventKind, Severity,
};
use testing::{foundry_hook_path, link_dir, TestRepo};

const AGENT: &str = "builder";
const PROJECT: &str = "p1";

/// A captured payload, `{{ROOT}}` replaced by `root`.
fn fixture(name: &str, root: &Path) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/features/agent_events/fixtures")
        .join(format!("{name}.json"));
    let text = std::fs::read_to_string(path).unwrap();
    let root = serde_json::to_string(&root.to_string_lossy()).unwrap();
    serde_json::from_str(&text.replace("{{ROOT}}", &root[1..root.len() - 1])).unwrap()
}

fn write_payload(root: &Path, tool: &str, file: &str) -> Value {
    let mut payload = fixture("pre_tool_use_write", root);
    payload["tool_name"] = json!(tool);
    payload["tool_input"]["file_path"] = json!(file);
    payload
}

fn bash_payload(root: &Path, command: &str) -> Value {
    let mut payload = fixture("pre_tool_use_bash", root);
    payload["tool_input"]["command"] = json!(command);
    payload
}

fn roles(repo: &TestRepo, agents: Value) {
    repo.write(
        "agents/roles.json",
        &json!({ "agents": agents }).to_string(),
    );
}

fn standard_roles(repo: &TestRepo) {
    roles(
        repo,
        json!([
            { "id": "builder", "writeGlobs": ["src/**"], "protectedGlobs": ["src/secret/**"] },
            { "id": "designer", "writeGlobs": ["docs/**", "web/**"], "protectedGlobs": [] },
            { "id": "anything", "writeGlobs": ["**"], "protectedGlobs": [] }
        ]),
    );
}

/// The helper with a clean Foundry environment plus `env`.
fn helper(env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::new(foundry_hook_path());
    for key in [
        "FOUNDRY_AGENT_ID",
        "FOUNDRY_PROJECT_ID",
        "FOUNDRY_HOOK_PORT",
        "FOUNDRY_HOOK_TOKEN",
        "FOUNDRY_SLICE_ID",
        "CLAUDE_PROJECT_DIR",
    ] {
        cmd.env_remove(key);
    }
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    cmd
}

fn run_with(mut cmd: Command, stdin: &str) -> (Output, Duration) {
    let started = Instant::now();
    let mut child = cmd.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (out, started.elapsed())
}

/// Runs the helper as agent `agent` in `repo`.
fn run_as(repo: &Path, agent: &str, payload: &Value) -> Output {
    let root = repo.to_string_lossy().into_owned();
    let cmd = helper(&[("FOUNDRY_AGENT_ID", agent), ("CLAUDE_PROJECT_DIR", &root)]);
    run_with(cmd, &payload.to_string()).0
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn blocked(out: &Output, rule: &str) {
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(out));
    let message = stderr(out);
    assert!(
        message.contains(&format!("(rule: {rule})")),
        "{message:?} doesn't name {rule}"
    );
    assert!(message.contains("ask the human"), "{message:?}");
}

fn allowed(out: &Output) {
    assert_eq!(out.status.code(), Some(0), "stderr: {}", stderr(out));
    assert_eq!(stderr(out), "");
}

// ---------- R-8b-1: no agent id → nothing ----------

#[test]
fn without_foundry_agent_id_it_does_nothing_and_exits_0() {
    let repo = TestRepo::new(); // no roles.json: an agent's edit would be refused
    let root = repo.path().to_string_lossy().into_owned();
    for payload in [
        write_payload(repo.path(), "Write", "AGENTS.md").to_string(),
        bash_payload(repo.path(), "git push --force").to_string(),
        "not json at all".to_string(),
        String::new(),
    ] {
        let cmd = helper(&[("CLAUDE_PROJECT_DIR", &root), ("FOUNDRY_HOOK_PORT", "1")]);
        let (out, _) = run_with(cmd, &payload);
        assert_eq!(out.status.code(), Some(0));
        assert_eq!((stdout(&out), stderr(&out)), (String::new(), String::new()));
    }
    // An empty FOUNDRY_AGENT_ID counts as unset.
    let cmd = helper(&[("FOUNDRY_AGENT_ID", " "), ("CLAUDE_PROJECT_DIR", &root)]);
    let (out, _) = run_with(
        cmd,
        &write_payload(repo.path(), "Write", "AGENTS.md").to_string(),
    );
    assert_eq!(out.status.code(), Some(0));
}

// ---------- R-8b-2: an edit outside the role's globs exits 2 naming the rule ----------

#[test]
fn an_edit_outside_the_roles_write_globs_is_blocked_naming_the_rule() {
    let repo = TestRepo::new();
    standard_roles(&repo);
    let abs = |rel: &str| repo.path().join(rel).to_string_lossy().into_owned();
    for tool in ["Write", "Edit", "MultiEdit", "NotebookEdit"] {
        let out = run_as(
            repo.path(),
            AGENT,
            &write_payload(repo.path(), tool, &abs("web/app.ts")),
        );
        blocked(&out, "outside_write_globs");
        assert!(stderr(&out).contains("web/app.ts") && stderr(&out).contains("src/**"));
    }
    allowed(&run_as(
        repo.path(),
        AGENT,
        &write_payload(repo.path(), "Write", &abs("src/main.rs")),
    ));
    allowed(&run_as(
        repo.path(),
        "designer",
        &write_payload(repo.path(), "Edit", &abs("web/app.ts")),
    ));
    // NotebookEdit names its file `notebook_path`.
    let mut notebook = fixture("pre_tool_use_edit", repo.path());
    notebook["tool_name"] = json!("NotebookEdit");
    notebook["tool_input"] = json!({ "notebook_path": abs("web/n.ipynb") });
    blocked(
        &run_as(repo.path(), AGENT, &notebook),
        "outside_write_globs",
    );
    // An agent missing from roles.json may not edit at all.
    blocked(
        &run_as(
            repo.path(),
            "stranger",
            &write_payload(repo.path(), "Write", &abs("src/a.rs")),
        ),
        "unknown_agent",
    );
    // Other tools are never judged.
    let mut read = write_payload(repo.path(), "Read", &abs("web/app.ts"));
    read["tool_name"] = json!("Read");
    allowed(&run_as(repo.path(), AGENT, &read));
}

// ---------- R-8b-3: protected files are blocked for every role ----------

#[test]
fn a_protected_file_edit_is_blocked_for_every_role() {
    let repo = TestRepo::new();
    standard_roles(&repo);
    repo.write("AGENTS.md", "# rules");
    for agent in ["builder", "designer", "anything"] {
        for rel in [
            "AGENTS.md",
            "agents/roles.json",
            "agents/roles/builder.md",
            "docs/design/tokens.json",
            ".foundry/plan.json",
            ".claude/settings.local.json",
        ] {
            let file = repo.path().join(rel).to_string_lossy().into_owned();
            let out = run_as(
                repo.path(),
                agent,
                &write_payload(repo.path(), "Edit", &file),
            );
            blocked(&out, "protected_path");
            assert!(
                stderr(&out).contains(rel),
                "{agent} {rel}: {}",
                stderr(&out)
            );
        }
    }
    // The role's own protectedGlobs too, even inside its writeGlobs.
    let secret = repo
        .path()
        .join("src/secret/key.rs")
        .to_string_lossy()
        .into_owned();
    blocked(
        &run_as(
            repo.path(),
            AGENT,
            &write_payload(repo.path(), "Write", &secret),
        ),
        "protected_path",
    );
}

/// P9: `.foundry/project.json` (its `testCommands` run on Verify) and `.foundry/reviews/` (the
/// plan trusts them) are on the helper's built-in floor, so a team whose `roles.json` predates
/// them, with neither in its `protectedGlobs` and `**` as a write glob, is still blocked.
#[test]
fn project_json_and_reviews_are_blocked_for_every_role_without_editing_roles_json() {
    let repo = TestRepo::new();
    standard_roles(&repo);
    repo.write(".foundry/project.json", "{}");
    let roles = repo.read("agents/roles.json");
    assert!(!roles.contains("project.json") && !roles.contains("reviews"));
    for agent in ["builder", "designer", "anything"] {
        for rel in [
            ".foundry/project.json",
            ".foundry/reviews/s-1.json",
            ".foundry/reviews/digest.md",
        ] {
            for tool in ["Write", "Edit", "MultiEdit"] {
                let file = repo.path().join(rel).to_string_lossy().into_owned();
                let out = run_as(repo.path(), agent, &write_payload(repo.path(), tool, &file));
                blocked(&out, "protected_path");
                assert!(
                    stderr(&out).contains(rel),
                    "{agent} {tool} {rel}: {}",
                    stderr(&out)
                );
            }
        }
    }
    // The report carve-out next to them still works.
    let report = repo
        .path()
        .join(".foundry/reports/s-1.json")
        .to_string_lossy()
        .into_owned();
    allowed(&run_as(
        repo.path(),
        "builder",
        &write_payload(repo.path(), "Write", &report),
    ));
}

// ---------- R-8b-4: destructive Bash is blocked, git push only warns ----------

#[test]
fn destructive_bash_is_blocked_and_git_push_only_warns() {
    let repo = TestRepo::new(); // Bash is judged without roles.json
    for (command, rule) in [
        ("git push --force origin main", "git_push_force"),
        ("git reset --hard HEAD~3", "git_reset_hard"),
        ("git clean -fd", "git_clean_force"),
        ("rm -rf /", "rm_rf_outside"),
        ("rm -rf ../sibling", "rm_rf_outside"),
    ] {
        blocked(
            &run_as(repo.path(), AGENT, &bash_payload(repo.path(), command)),
            rule,
        );
    }
    for (command, rule) in [
        ("git push origin main", "git_push"),
        ("npm install left-pad", "dependency_install"),
    ] {
        let out = run_as(repo.path(), AGENT, &bash_payload(repo.path(), command));
        allowed(&out);
        let reply: Value = serde_json::from_str(stdout(&out).trim()).unwrap();
        let message = reply["systemMessage"].as_str().unwrap();
        assert!(message.contains(rule), "{message}");
        assert_eq!(reply["hookSpecificOutput"]["hookEventName"], "PreToolUse");
        assert!(
            reply["hookSpecificOutput"]
                .get("permissionDecision")
                .is_none(),
            "never auto-allows"
        );
    }
    let out = run_as(repo.path(), AGENT, &bash_payload(repo.path(), "cargo test"));
    allowed(&out);
    assert_eq!(stdout(&out), "");
}

// ---------- R-8b-6: roles.json missing or invalid → edit tools fail closed ----------

#[test]
fn edit_tools_fail_closed_without_a_readable_roles_json() {
    let repo = TestRepo::new();
    let file = repo
        .path()
        .join("src/main.rs")
        .to_string_lossy()
        .into_owned();
    for tool in ["Write", "Edit", "MultiEdit", "NotebookEdit"] {
        blocked(
            &run_as(repo.path(), AGENT, &write_payload(repo.path(), tool, &file)),
            "roles_missing",
        );
    }
    for bad in ["{ not json", "{\"agents\": 3}", ""] {
        repo.write("agents/roles.json", bad);
        blocked(
            &run_as(
                repo.path(),
                AGENT,
                &write_payload(repo.path(), "Write", &file),
            ),
            "roles_invalid",
        );
    }
    // Bash isn't an edit tool: still judged on its own.
    allowed(&run_as(
        repo.path(),
        AGENT,
        &bash_payload(repo.path(), "ls"),
    ));
}

// ---------- R-8b-7: the report carve-out ----------

#[test]
fn any_role_can_write_its_report_but_nothing_near_it() {
    let repo = TestRepo::new();
    standard_roles(&repo);
    let path = |rel: &str| repo.path().join(rel).to_string_lossy().into_owned();
    for agent in ["builder", "designer"] {
        // Neither role's writeGlobs mention .foundry/reports/.
        allowed(&run_as(
            repo.path(),
            agent,
            &write_payload(repo.path(), "Write", &path(".foundry/reports/s-feed.json")),
        ));
    }
    let report = |rel: &str| {
        run_as(
            repo.path(),
            AGENT,
            &write_payload(repo.path(), "Write", rel),
        )
    };
    blocked(
        &report(&path(".foundry/reports/sub/x.json")),
        "outside_write_globs",
    );
    blocked(
        &report(&path(".foundry/reports/s-feed.md")),
        "outside_write_globs",
    );
    blocked(&report(&path(".foundry/plan.json")), "protected_path");
    blocked(
        &report(&path(".foundry/reports/../plan.json")),
        "path_escape",
    );
    blocked(&report(".foundry/reports/../reports/x.json"), "path_escape");
    blocked(
        &report(&format!("{}/../elsewhere/x.json", repo.path().display())),
        "path_escape",
    );
    // A file outside the repo, and a link that leads out of it.
    let outside = TestRepo::new();
    blocked(
        &report(&outside.path().join("x.json").to_string_lossy()),
        "outside_repo",
    );
    std::fs::create_dir_all(repo.path().join(".foundry")).unwrap();
    link_dir(
        outside.path(),
        &repo.path().join(".foundry").join("reports"),
    );
    blocked(
        &report(&path(".foundry/reports/s-feed.json")),
        "outside_repo",
    );
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);

    // protectedGlobs still win over the carve-out.
    let strict = TestRepo::new();
    roles(
        &strict,
        json!([{ "id": "builder", "writeGlobs": ["**"], "protectedGlobs": [".foundry/reports/**"] }]),
    );
    let file = strict
        .path()
        .join(".foundry/reports/s.json")
        .to_string_lossy()
        .into_owned();
    blocked(
        &run_as(
            strict.path(),
            AGENT,
            &write_payload(strict.path(), "Write", &file),
        ),
        "protected_path",
    );
}

#[test]
fn relative_targets_resolve_against_the_payload_cwd_inside_the_project() {
    let repo = TestRepo::new();
    standard_roles(&repo);
    std::fs::create_dir_all(repo.path().join("src")).unwrap();
    let mut payload = write_payload(repo.path(), "Write", "lib.rs");
    payload["cwd"] = json!(repo.path().join("src").to_string_lossy());
    allowed(&run_as(repo.path(), AGENT, &payload));
    payload["cwd"] = json!(repo.path().to_string_lossy());
    blocked(&run_as(repo.path(), AGENT, &payload), "outside_write_globs");
    // Without CLAUDE_PROJECT_DIR, the payload's cwd is the project root.
    let cmd = helper(&[("FOUNDRY_AGENT_ID", AGENT)]);
    let (out, _) = run_with(
        cmd,
        &write_payload(repo.path(), "Write", "src/x.rs").to_string(),
    );
    allowed(&out);
}

// ---------- R-8b-5: under a second when Foundry isn't running; p95 of the decision ----------

fn p95(mut samples: Vec<Duration>) -> Duration {
    samples.sort();
    samples[(samples.len() * 95).div_ceil(100) - 1]
}

#[test]
fn the_helper_returns_in_under_a_second_when_the_app_is_not_running() {
    let repo = TestRepo::new();
    standard_roles(&repo);
    let root = repo.path().to_string_lossy().into_owned();
    // A port nobody listens on: bound, then released.
    let closed = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let closed = closed.to_string();
    let file = repo.path().join("src/a.rs").to_string_lossy().into_owned();
    let payload = write_payload(repo.path(), "Write", &file).to_string();
    let measure = |env: &[(&str, &str)]| {
        (0..40)
            .map(|_| {
                let (out, took) = run_with(helper(env), &payload);
                assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
                took
            })
            .collect::<Vec<_>>()
    };
    let base = [
        ("FOUNDRY_AGENT_ID", AGENT),
        ("CLAUDE_PROJECT_DIR", root.as_str()),
    ];
    let decision = measure(&base);
    let mut with_port = base.to_vec();
    with_port.extend([
        ("FOUNDRY_PROJECT_ID", PROJECT),
        ("FOUNDRY_HOOK_PORT", closed.as_str()),
        ("FOUNDRY_HOOK_TOKEN", "tok"),
    ]);
    let app_down = measure(&with_port);
    let (p_decision, p_down) = (p95(decision.clone()), p95(app_down.clone()));
    eprintln!(
        "foundry-hook p95 over 40 runs (process spawn to exit): decision only {p_decision:?}, \
         decision + POST with the app not running {p_down:?}; max {:?} / {:?}",
        decision.iter().max().unwrap(),
        app_down.iter().max().unwrap()
    );
    assert!(
        app_down.iter().all(|d| *d < Duration::from_secs(1)),
        "{app_down:?}"
    );
    assert!(p_decision < Duration::from_secs(1));
}

// ---------- the helper reaches the real hook server ----------

#[derive(Default)]
struct Recorded(Mutex<Vec<AgentEvent>>);

impl AgentEventSink for Recorded {
    fn event(&self, event: AgentEvent) {
        self.0.lock().unwrap().push(event);
    }
}

#[test]
fn the_helper_posts_decisions_and_events_to_the_hook_server() {
    let repo = TestRepo::new();
    standard_roles(&repo);
    let repos = Arc::new(RepoIndex::default());
    repos.replace_all([(PROJECT, repo.path())]);
    let sink = Arc::new(Recorded::default());
    let store = Arc::new(AgentEventsStore::new(
        repos,
        Arc::new(SessionRegistry::default()),
        sink.clone(),
    ));
    let cell = Arc::new(HookEndpointCell::default());
    let _server = server::start(Arc::clone(&store), Arc::clone(&cell)).unwrap();
    let endpoint = cell.get().unwrap();
    let (root, port) = (
        repo.path().to_string_lossy().into_owned(),
        endpoint.port.to_string(),
    );
    let env = [
        ("FOUNDRY_AGENT_ID", AGENT),
        ("FOUNDRY_PROJECT_ID", PROJECT),
        ("FOUNDRY_HOOK_PORT", port.as_str()),
        ("FOUNDRY_HOOK_TOKEN", endpoint.token.as_str()),
        ("FOUNDRY_SLICE_ID", "s-feed"),
        ("CLAUDE_PROJECT_DIR", root.as_str()),
    ];
    let send = |payload: Value| run_with(helper(&env), &payload.to_string()).0;

    let agents_md = repo.path().join("AGENTS.md").to_string_lossy().into_owned();
    blocked(
        &send(write_payload(repo.path(), "Write", &agents_md)),
        "protected_path",
    );
    allowed(&send(bash_payload(repo.path(), "git push")));
    let mut big = fixture("post_tool_use_write", repo.path());
    big["tool_input"]["content"] = json!("x".repeat(400 * 1024)); // over the server's cap
    big["tool_response"]["content"] = json!("x".repeat(400 * 1024));
    allowed(&send(big));
    allowed(&send(fixture("stop", repo.path())));

    let deadline = Instant::now() + Duration::from_secs(10);
    while sink.0.lock().unwrap().len() < 4 {
        assert!(
            Instant::now() < deadline,
            "events never arrived: {:?}",
            sink.0.lock().unwrap()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let events = store
        .events(PROJECT, AGENT, &AgentEventsQuery::default())
        .unwrap();
    let mut events: Vec<AgentEvent> = events.into_iter().rev().collect();
    assert_eq!(
        events
            .iter()
            .map(|e| (e.kind, e.severity))
            .collect::<Vec<_>>(),
        [
            (EventKind::Blocked, Severity::Danger),
            (EventKind::Command, Severity::Warn),
            (EventKind::Edit, Severity::Info),
            (EventKind::TurnComplete, Severity::Info),
        ]
    );
    let first = events.remove(0);
    assert_eq!(first.path.as_deref(), Some("AGENTS.md"));
    assert_eq!(first.detail.as_deref(), Some("protected_path"));
    assert!(events
        .iter()
        .all(|e| e.slice_id.as_deref() == Some("s-feed")));
    let log = repo.read(".foundry/events/builder.jsonl");
    assert!(!log.contains("xxxxxxxx"), "file contents are never sent");
}
