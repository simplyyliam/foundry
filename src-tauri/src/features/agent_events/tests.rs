use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::classify::{self, is_test_command, Context, Draft, Streak};
use super::*;
use crate::core::hook::{HookEndpointCell, TOKEN_HEADER};
use crate::core::sessions::SessionKey;
use crate::core::ErrorCode;
use crate::testing::TestRepo;

// ---------- helpers ----------

const P1: &str = "p1";
const P2: &str = "p2";
const AGENT: &str = "builder";

fn post(event: &str, payload: Value) -> HookPost {
    HookPost {
        project_id: P1.into(),
        agent_id: AGENT.into(),
        slice_id: None,
        hook_event: Some(event.into()),
        payload,
        decision: None,
    }
}

fn bash(event: &str, command: &str, response: Value) -> HookPost {
    post(
        event,
        json!({
            "session_id": "abc123",
            "transcript_path": "/home/u/.claude/projects/x/t.jsonl",
            "cwd": "/home/u/repo",
            "hook_event_name": event,
            "tool_name": "Bash",
            "tool_input": { "command": command, "description": "run it" },
            "tool_response": response,
            "tool_use_id": "toolu_01"
        }),
    )
}

fn failed(command: &str) -> HookPost {
    post(
        "PostToolUseFailure",
        json!({
            "hook_event_name": "PostToolUseFailure",
            "tool_name": "Bash",
            "tool_input": { "command": command },
            "tool_error": "Command exited with code 1"
        }),
    )
}

fn write_tool(tool: &str, file: &Path) -> HookPost {
    let file = file.to_string_lossy();
    post(
        "PostToolUse",
        json!({
            "hook_event_name": "PostToolUse",
            "tool_name": tool,
            "tool_input": { "file_path": file, "content": "secret file contents" },
            "tool_response": { "filePath": file, "success": true }
        }),
    )
}

fn drafts(post: &HookPost, root: &Path, globs: &[String]) -> Vec<Draft> {
    classify::classify(
        post,
        &Context {
            repo_root: Some(root),
            protected_globs: globs,
            streak: None,
        },
    )
    .drafts
}

fn one(post: &HookPost, root: &Path, globs: &[String]) -> Draft {
    let mut all = drafts(post, root, globs);
    assert_eq!(all.len(), 1, "{all:?}");
    all.remove(0)
}

fn floor() -> Vec<String> {
    [
        "AGENTS.md",
        "agents/**",
        "docs/design/**",
        ".foundry/plan.json",
        ".claude/**",
    ]
    .map(String::from)
    .to_vec()
}

#[derive(Default)]
struct Recorded(Mutex<Vec<AgentEvent>>);

impl AgentEventSink for Recorded {
    fn event(&self, event: AgentEvent) {
        self.0.lock().unwrap().push(event);
    }
}

impl Recorded {
    fn all(&self) -> Vec<AgentEvent> {
        self.0.lock().unwrap().clone()
    }
}

struct FakeClock(AtomicU64);

impl Clock for FakeClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

impl FakeClock {
    fn set(&self, ms: u64) {
        self.0.store(ms, Ordering::SeqCst);
    }
}

struct Env {
    store: Arc<AgentEventsStore>,
    sink: Arc<Recorded>,
    clock: Arc<FakeClock>,
    sessions: Arc<SessionRegistry>,
    repos: Arc<RepoIndex>,
    p1: TestRepo,
    p2: TestRepo,
}

impl Env {
    fn new() -> Self {
        Self::with(|store| store)
    }

    fn with(tweak: impl FnOnce(AgentEventsStore) -> AgentEventsStore) -> Self {
        let (p1, p2) = (TestRepo::new(), TestRepo::new());
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([(P1, p1.path()), (P2, p2.path())]);
        let sessions = Arc::new(SessionRegistry::default());
        let sink = Arc::new(Recorded::default());
        let clock = Arc::new(FakeClock(AtomicU64::new(1_700_000_000_000)));
        let store = Arc::new(tweak(
            AgentEventsStore::new(Arc::clone(&repos), Arc::clone(&sessions), sink.clone())
                .with_clock(clock.clone()),
        ));
        Self {
            store,
            sink,
            clock,
            sessions,
            repos,
            p1,
            p2,
        }
    }

    /// A second store over the same repos, as after an app restart.
    fn restarted(&self) -> AgentEventsStore {
        AgentEventsStore::new(
            Arc::clone(&self.repos),
            Arc::clone(&self.sessions),
            Arc::new(Recorded::default()),
        )
    }

    fn all(&self, project: &str) -> Vec<AgentEvent> {
        self.store
            .events(
                project,
                AGENT,
                &AgentEventsQuery {
                    limit: Some(MAX_LIMIT),
                    ..Default::default()
                },
            )
            .unwrap()
    }
}

fn in_project(mut post: HookPost, project: &str) -> HookPost {
    post.project_id = project.into();
    post
}

fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !done() {
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

// ---------- R-8a-1: one fixture per classifier rule ----------

#[test]
fn rule_blocked_write_or_destructive_command_is_blocked_danger() {
    let repo = TestRepo::new();
    let mut rm = bash("PreToolUse", "rm -rf ~/", Value::Null);
    rm.decision = Some(HookDecision {
        blocked: true,
        rule: Some("destructive_command".into()),
    });
    let d = one(&rm, repo.path(), &[]);
    assert_eq!((d.kind, d.severity), (EventKind::Blocked, Severity::Danger));
    assert_eq!(d.command.as_deref(), Some("rm -rf ~/"));
    assert_eq!(d.detail.as_deref(), Some("destructive_command"));
    assert_eq!(d.title, "Blocked a command: rm -rf ~/");

    let mut write = write_tool("Write", &repo.path().join("src").join("main.rs"));
    write.hook_event = Some("PreToolUse".into());
    write.decision = Some(HookDecision {
        blocked: true,
        rule: None,
    });
    let d = one(&write, repo.path(), &[]);
    assert_eq!((d.kind, d.severity), (EventKind::Blocked, Severity::Danger));
    assert_eq!(d.path.as_deref(), Some("src/main.rs"));
    assert_eq!(d.title, "Blocked a write to src/main.rs");
    assert_eq!(d.detail, None);
}

#[test]
fn rule_protected_file_edit_is_edit_warn() {
    let repo = TestRepo::new();
    for rel in ["AGENTS.md", "agents/roles/builder.md", ".foundry/plan.json"] {
        let d = one(
            &write_tool("Edit", &repo.path().join(rel)),
            repo.path(),
            &floor(),
        );
        assert_eq!(
            (d.kind, d.severity),
            (EventKind::Edit, Severity::Warn),
            "{rel}"
        );
        assert_eq!(d.path.as_deref(), Some(rel));
        assert!(
            d.title.starts_with("Edited a protected file"),
            "{}",
            d.title
        );
    }
    // The same file is a plain edit for an agent whose role doesn't protect it.
    let d = one(
        &write_tool("Edit", &repo.path().join("AGENTS.md")),
        repo.path(),
        &[],
    );
    assert_eq!(d.severity, Severity::Info);
}

#[test]
fn rule_manifest_or_lockfile_edit_is_edit_warn() {
    let repo = TestRepo::new();
    for rel in [
        "Cargo.toml",
        "src-tauri/Cargo.lock",
        "web/package.json",
        "pnpm-lock.yaml",
    ] {
        let d = one(
            &write_tool("Write", &repo.path().join(rel)),
            repo.path(),
            &[],
        );
        assert_eq!(
            (d.kind, d.severity),
            (EventKind::Edit, Severity::Warn),
            "{rel}"
        );
        assert!(
            d.title.starts_with("Edited a dependency file"),
            "{}",
            d.title
        );
    }
}

#[test]
fn rule_notification_is_needs_attention_warn() {
    let repo = TestRepo::new();
    for (kind, title) in [
        ("permission_prompt", "Needs permission"),
        ("idle_prompt", "Waiting for input"),
    ] {
        let n = post(
            "Notification",
            json!({
                "session_id": "abc123",
                "hook_event_name": "Notification",
                "message": "Permission needed to run Bash command",
                "notification_type": kind
            }),
        );
        let d = one(&n, repo.path(), &[]);
        assert_eq!(
            (d.kind, d.severity, d.title.as_str()),
            (EventKind::NeedsAttention, Severity::Warn, title)
        );
        assert_eq!(
            d.detail.as_deref(),
            Some("Permission needed to run Bash command")
        );
    }
    // No type at all (older Claude Code) still asks for attention; auth_success doesn't.
    assert_eq!(
        one(&post("Notification", json!({})), repo.path(), &[]).kind,
        EventKind::NeedsAttention
    );
    let ok = post(
        "Notification",
        json!({ "notification_type": "auth_success" }),
    );
    assert!(drafts(&ok, repo.path(), &[]).is_empty());
}

#[test]
fn rule_same_command_failing_three_times_in_a_row_is_stuck_warn() {
    let repo = TestRepo::new();
    let mut streak = Streak::default();
    let mut run = |p: HookPost| {
        let out = classify::classify(
            &p,
            &Context {
                repo_root: Some(repo.path()),
                protected_globs: &[],
                streak: Some(&streak),
            },
        );
        streak = out.streak;
        out.drafts.into_iter().map(|d| d.kind).collect::<Vec<_>>()
    };
    assert_eq!(run(failed("cargo build")), [EventKind::Command]);
    assert_eq!(run(failed("cargo build")), [EventKind::Command]);
    // A different command in between breaks the run.
    assert_eq!(run(failed("cargo check")), [EventKind::Command]);
    assert_eq!(run(failed("cargo build")), [EventKind::Command]);
    assert_eq!(run(failed("cargo build")), [EventKind::Command]);
    assert_eq!(
        run(failed("cargo build")),
        [EventKind::Command, EventKind::Stuck]
    );
    assert_eq!(
        run(failed("cargo build")),
        [EventKind::Command],
        "once per run"
    );
    // A success resets it; a non-zero exit code counts as a failure.
    assert_eq!(
        run(bash(
            "PostToolUse",
            "cargo build",
            json!({ "exit_code": 0 })
        )),
        [EventKind::Command]
    );
    let exit1 = || bash("PostToolUse", "make", json!({ "exit_code": 2 }));
    run(exit1());
    run(exit1());
    assert_eq!(run(exit1()), [EventKind::Command, EventKind::Stuck]);

    let stuck = classify::classify(
        &failed("make"),
        &Context {
            streak: Some(&Streak {
                command: Some("make".into()),
                failures: 2,
            }),
            ..Default::default()
        },
    )
    .drafts;
    assert_eq!(
        (stuck[1].severity, stuck[1].command.as_deref()),
        (Severity::Warn, Some("make"))
    );
}

#[test]
fn rule_stop_is_turn_complete_with_the_last_message_cut_to_300() {
    let repo = TestRepo::new();
    let long = "é".repeat(400);
    let stop = post(
        "Stop",
        json!({
            "hook_event_name": "Stop",
            "last_assistant_message": long,
            "stop_hook_active": false
        }),
    );
    let d = one(&stop, repo.path(), &[]);
    assert_eq!(
        (d.kind, d.severity),
        (EventKind::TurnComplete, Severity::Info)
    );
    let detail = d.detail.unwrap();
    assert_eq!(detail.chars().count(), 300);
    assert!(detail.ends_with('…'));

    let short = post(
        "Stop",
        json!({ "last_assistant_message": "The tests pass." }),
    );
    assert_eq!(
        one(&short, repo.path(), &[]).detail.as_deref(),
        Some("The tests pass.")
    );
    assert_eq!(one(&post("Stop", json!({})), repo.path(), &[]).detail, None);
}

#[test]
fn rule_report_write_is_exactly_one_report_ready_with_the_file_slice() {
    let repo = TestRepo::new();
    // A per-agent report names its slice like the legacy one does.
    let own = repo.path().join(".foundry/reports/s-feed.qa.json");
    let d = one(&write_tool("Write", &own), repo.path(), &floor());
    assert_eq!(
        (d.kind, d.slice_id.as_deref()),
        (EventKind::ReportReady, Some("s-feed"))
    );
    let file = repo
        .path()
        .join(".foundry")
        .join("reports")
        .join("s-feed.json");
    for tool in ["Write", "Edit"] {
        let mut w = write_tool(tool, &file);
        w.slice_id = Some("s-other".into());
        let all = drafts(&w, repo.path(), &floor());
        assert_eq!(all.len(), 1, "{all:?}");
        let d = &all[0];
        assert_eq!(
            (d.kind, d.severity),
            (EventKind::ReportReady, Severity::Info)
        );
        assert_eq!(d.slice_id.as_deref(), Some("s-feed"));
        assert_eq!(d.path.as_deref(), Some(".foundry/reports/s-feed.json"));
    }
    // Not a report: a nested file, or not `.json`.
    for rel in [".foundry/reports/a/b.json", ".foundry/reports/s.md"] {
        assert_eq!(
            one(
                &write_tool("Write", &repo.path().join(rel)),
                repo.path(),
                &[]
            )
            .kind,
            EventKind::Edit
        );
    }
}

#[test]
fn rule_session_start_and_end_are_info() {
    let repo = TestRepo::new();
    let start = post(
        "SessionStart",
        json!({ "hook_event_name": "SessionStart", "source": "startup", "model": "m" }),
    );
    let d = one(&start, repo.path(), &[]);
    assert_eq!(
        (d.kind, d.severity),
        (EventKind::SessionStarted, Severity::Info)
    );
    assert_eq!(d.detail.as_deref(), Some("startup"));
    let end = post(
        "SessionEnd",
        json!({ "hook_event_name": "SessionEnd", "reason": "prompt_input_exit" }),
    );
    let d = one(&end, repo.path(), &[]);
    assert_eq!(
        (d.kind, d.severity),
        (EventKind::SessionEnded, Severity::Info)
    );
    assert_eq!(d.detail.as_deref(), Some("prompt_input_exit"));
}

#[test]
fn rule_test_commands_are_tests_ran_with_pass_or_fail() {
    let repo = TestRepo::new();
    let cases = [
        (
            bash(
                "PostToolUse",
                "cd src-tauri && cargo test -p app",
                json!({ "stdout": "ok", "stderr": "", "exit_code": 0, "interrupted": false }),
            ),
            "Tests passed",
        ),
        (failed("pnpm test"), "Tests failed"),
        (
            // The real shape: PostToolUse has no exit code (it only fires on success).
            bash(
                "PostToolUse",
                "npx vitest run",
                json!({ "stdout": "", "interrupted": false }),
            ),
            "Tests passed",
        ),
        (
            bash(
                "PostToolUse",
                "cargo test",
                json!({ "stdout": "", "interrupted": true }),
            ),
            "Tests interrupted",
        ),
        (
            bash(
                "PostToolUse",
                "python -m pytest -q",
                json!({ "exitCode": 1 }),
            ),
            "Tests failed",
        ),
    ];
    for (p, title) in cases {
        let d = one(&p, repo.path(), &[]);
        assert_eq!(
            (d.kind, d.severity, d.title.as_str()),
            (EventKind::TestsRan, Severity::Info, title)
        );
        assert!(d.command.is_some());
    }
    for yes in [
        "cargo test",
        "cargo +nightly test --workspace",
        "cargo nextest run",
        "npm test",
        "npm run test",
        "pnpm -C web test",
        "yarn run test:unit",
        "bun test",
        "vitest",
        "pnpm exec playwright test",
        "npx playwright test e2e",
        "pytest tests/",
        "RUST_LOG=debug cargo test",
        "C:\\tools\\pytest.exe -x",
    ] {
        assert!(is_test_command(yes), "{yes}");
    }
    for no in [
        "cargo build",
        "cargo fmt --check",
        "pnpm add -D vitest",
        "npm install",
        "echo test",
        "ls tests",
        "git commit -m test",
        "npx playwright install",
    ] {
        assert!(!is_test_command(no), "{no}");
    }
}

#[test]
fn rule_any_other_edit_or_command_is_info() {
    let repo = TestRepo::new();
    let d = one(
        &write_tool("Edit", &repo.path().join("src").join("app.ts")),
        repo.path(),
        &floor(),
    );
    assert_eq!((d.kind, d.severity), (EventKind::Edit, Severity::Info));
    assert_eq!(
        (d.title.as_str(), d.path.as_deref()),
        ("Edited src/app.ts", Some("src/app.ts"))
    );

    let notebook = post(
        "PostToolUse",
        json!({ "tool_name": "NotebookEdit", "tool_input": { "notebook_path": "nb/a.ipynb" } }),
    );
    assert_eq!(
        one(&notebook, repo.path(), &[]).path.as_deref(),
        Some("nb/a.ipynb")
    );

    let d = one(
        &bash(
            "PostToolUse",
            "ls -la",
            json!({ "stdout": "x", "exit_code": 0 }),
        ),
        repo.path(),
        &[],
    );
    assert_eq!(
        (d.kind, d.severity, d.title.as_str(), d.command.as_deref()),
        (
            EventKind::Command,
            Severity::Info,
            "Ran a command",
            Some("ls -la")
        )
    );
    let d = one(&failed("ls nope"), repo.path(), &[]);
    assert_eq!(d.title, "Command failed");
}

#[test]
fn the_classifier_tolerates_missing_and_unknown_fields() {
    let repo = TestRepo::new();
    let empty = |event: &str, payload: Value| drafts(&post(event, payload), repo.path(), &[]);
    // A tool about to run, tools that are neither edits nor commands, unknown hook events
    // and malformed payloads: no events, no panic.
    assert!(empty("PreToolUse", json!({ "tool_name": "Bash" })).is_empty());
    assert!(empty(
        "PostToolUse",
        json!({ "tool_name": "Read", "tool_input": { "file_path": "a" } })
    )
    .is_empty());
    assert!(empty("PostToolUse", json!({ "tool_name": "Bash" })).is_empty());
    assert!(empty("UserPromptSubmit", json!({ "prompt": "do it" })).is_empty());
    assert!(empty("PostToolUse", json!("not an object")).is_empty());
    assert!(empty("PostToolUse", json!({ "tool_name": 7, "tool_input": [1] })).is_empty());
    assert_eq!(
        empty("PostToolUse", json!({ "tool_name": "Write" }))[0].title,
        "Edited a file"
    );
    // The event name may come from the payload instead of the body.
    let mut from_payload = post("", json!({ "hook_event_name": "SessionStart" }));
    from_payload.hook_event = None;
    assert_eq!(
        one(&from_payload, repo.path(), &[]).kind,
        EventKind::SessionStarted
    );
    // The whole POST body with only the required fields parses.
    let minimal: HookPost =
        serde_json::from_value(json!({ "projectId": "p", "agentId": "a" })).unwrap();
    assert!(drafts(&minimal, repo.path(), &[]).is_empty());
}

#[test]
fn paths_outside_the_repo_stay_absolute_and_never_match_role_globs() {
    let repo = TestRepo::new();
    let outside = TestRepo::new();
    let d = one(
        &write_tool("Write", &outside.path().join("AGENTS.md")),
        repo.path(),
        &floor(),
    );
    assert_eq!(d.severity, Severity::Info);
    assert!(d.path.unwrap().ends_with("/AGENTS.md"));
}

// ---------- R-8a-5: sliceId from the body appears on events ----------

#[test]
fn slice_id_from_the_body_appears_on_stored_events() {
    let env = Env::new();
    let mut p = bash("PostToolUse", "ls", json!({ "exit_code": 0 }));
    p.slice_id = Some("s-feed".into());
    let stored = env.store.ingest(&p).unwrap();
    assert_eq!(stored[0].slice_id.as_deref(), Some("s-feed"));
    assert_eq!(env.all(P1)[0].slice_id.as_deref(), Some("s-feed"));
    assert_eq!(env.sink.all()[0].slice_id.as_deref(), Some("s-feed"));

    let none = env
        .store
        .ingest(&bash("PostToolUse", "ls", json!({})))
        .unwrap();
    assert_eq!(none[0].slice_id, None);
}

// ---------- R-8a-6: a report write gives exactly one report_ready ----------

#[test]
fn a_report_write_stores_and_emits_exactly_one_report_ready() {
    let env = Env::new();
    let file = env.p1.path().join(".foundry/reports/s-feed.json");
    let mut pre = write_tool("Write", &file);
    pre.hook_event = Some("PreToolUse".into());
    env.store.ingest(&pre).unwrap();
    env.store.ingest(&write_tool("Write", &file)).unwrap();

    let ready: Vec<AgentEvent> = env
        .all(P1)
        .into_iter()
        .filter(|e| e.kind == EventKind::ReportReady)
        .collect();
    assert_eq!(ready.len(), 1);
    assert_eq!(env.all(P1).len(), 1, "no edit event beside it");
    assert_eq!(ready[0].slice_id.as_deref(), Some("s-feed"));
    assert_eq!(env.sink.all(), ready);
}

// ---------- R-8a-3: the store appends, rotates and counts unread ----------

#[test]
fn the_store_appends_jsonl_with_monotonic_ids_that_survive_a_restart() {
    let env = Env::new();
    for n in 0..3 {
        env.store
            .ingest(&bash("PostToolUse", &format!("echo {n}"), json!({})))
            .unwrap();
    }
    let text = env.p1.read(".foundry/events/builder.jsonl");
    let lines: Vec<AgentEvent> = text
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(lines.iter().map(|e| e.id).collect::<Vec<_>>(), [1, 2, 3]);
    assert!(lines
        .iter()
        .all(|e| e.project_id == P1 && e.agent_id == AGENT));
    assert_eq!(lines[0].ts, 1_700_000_000_000, "ts from the injected clock");
    assert!(!text.contains("secret"), "no file contents stored");

    // Newest first, paging and filters.
    let ids = |q: AgentEventsQuery| {
        env.store
            .events(P1, AGENT, &q)
            .unwrap()
            .into_iter()
            .map(|e| e.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(Default::default()), [3, 2, 1]);
    assert_eq!(
        ids(AgentEventsQuery {
            before_id: Some(3),
            limit: Some(1),
            ..Default::default()
        }),
        [2]
    );
    assert_eq!(
        ids(AgentEventsQuery {
            min_severity: Some(Severity::Warn),
            ..Default::default()
        }),
        Vec::<u64>::new()
    );
    for limit in [0, MAX_LIMIT + 1] {
        let err = env
            .store
            .events(
                P1,
                AGENT,
                &AgentEventsQuery {
                    limit: Some(limit),
                    ..Default::default()
                },
            )
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
    }

    // A torn last line (a crash mid-append) is skipped, and the next event starts a new line.
    let path = env.p1.path().join(".foundry/events/builder.jsonl");
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{\"id\":99,\"tor")
        .unwrap();
    let again = env.restarted();
    let next = again.ingest(&bash("PostToolUse", "ls", json!({}))).unwrap();
    assert_eq!(next[0].id, 4, "ids continue after a restart");
    assert_eq!(
        env.all(P1).iter().map(|e| e.id).collect::<Vec<_>>(),
        [4, 3, 2, 1]
    );
}

#[test]
fn the_store_rotates_keeping_the_last_two_files() {
    let env = Env::with(|s| s.with_rotate_at(1500));
    let dir = env.p1.path().join(".foundry/events");
    let mut rotations = 0;
    let mut had_previous = false;
    for n in 0..40 {
        env.store
            .ingest(&bash("PostToolUse", &format!("echo {n:03}"), json!({})))
            .unwrap();
        let has = dir.join("builder.1.jsonl").exists();
        if has && !had_previous {
            rotations += 1;
        }
        had_previous = has;
        assert!(std::fs::metadata(dir.join("builder.jsonl")).unwrap().len() <= 1500);
    }
    assert!(rotations >= 1);
    let mut files: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    files.sort();
    assert_eq!(files, ["builder.1.jsonl", "builder.jsonl"]);

    // What's kept is a contiguous run ending at the newest id; the oldest are gone.
    let kept: Vec<u64> = env.all(P1).iter().map(|e| e.id).collect();
    assert_eq!(kept[0], 40);
    assert!(kept.windows(2).all(|w| w[0] == w[1] + 1), "{kept:?}");
    assert!(*kept.last().unwrap() > 1, "old files were dropped");
    // A restart continues from the newest id, even right after a rotation.
    assert_eq!(
        env.restarted()
            .ingest(&bash("PostToolUse", "ls", json!({})))
            .unwrap()[0]
            .id,
        41
    );
    assert_eq!(ROTATE_AT, 5 * 1024 * 1024);
}

#[test]
fn unread_and_danger_counts_follow_the_read_marker() {
    let env = Env::new();
    let mut blocked = bash("PreToolUse", "rm -rf /", Value::Null);
    blocked.decision = Some(HookDecision {
        blocked: true,
        rule: Some("destructive".into()),
    });
    env.store
        .ingest(&bash("PostToolUse", "ls", json!({})))
        .unwrap();
    env.store.ingest(&blocked).unwrap();
    env.store.ingest(&post("Stop", json!({}))).unwrap();
    let mut other = post("SessionStart", json!({}));
    other.agent_id = "designer".into();
    env.store.ingest(&other).unwrap();

    let counts = |env: &Env| env.store.counts(P1).unwrap();
    let c = |agent: &str, unread, danger| AgentEventCounts {
        agent_id: agent.into(),
        unread,
        danger,
    };
    assert_eq!(counts(&env), [c("builder", 3, 1), c("designer", 1, 0)]);

    env.store.mark_read(P1, AGENT, 1).unwrap();
    assert_eq!(counts(&env)[0], c("builder", 2, 1));
    env.store.mark_read(P1, AGENT, 2).unwrap();
    assert_eq!(counts(&env)[0], c("builder", 1, 0));
    // Never backwards; never past the newest event.
    env.store.mark_read(P1, AGENT, 1).unwrap();
    assert_eq!(counts(&env)[0], c("builder", 1, 0));
    env.store.mark_read(P1, AGENT, 1_000).unwrap();
    assert_eq!(counts(&env)[0], c("builder", 0, 0));
    env.store.ingest(&post("Stop", json!({}))).unwrap();
    assert_eq!(
        counts(&env)[0],
        c("builder", 1, 0),
        "a later event arrives unread"
    );

    let markers: Value = serde_json::from_str(&env.p1.read(READ_FILE)).unwrap();
    assert_eq!(markers, json!({ "builder": 3 }));
    assert_eq!(
        counts(&env)[1],
        c("designer", 1, 0),
        "markers are per agent"
    );
}

#[test]
fn the_store_only_writes_inside_foundry_events() {
    let env = Env::new();
    env.store
        .ingest(&bash("PostToolUse", "ls", json!({})))
        .unwrap();
    env.store.mark_read(P1, AGENT, 1).unwrap();
    let top: Vec<String> = std::fs::read_dir(env.p1.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(top, [".foundry"]);
    let inner: Vec<String> = std::fs::read_dir(env.p1.path().join(".foundry"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(inner, ["events"]);
}

#[test]
fn bad_agent_ids_and_unknown_projects_are_refused() {
    let env = Env::new();
    for bad in ["", "../x", "a/b", "a.1", &"x".repeat(65)] {
        let mut p = post("Stop", json!({}));
        p.agent_id = bad.to_string();
        assert_eq!(
            env.store.ingest(&p).unwrap_err().code,
            ErrorCode::InvalidInput,
            "{bad}"
        );
    }
    let unknown = in_project(post("Stop", json!({})), "nope");
    assert_eq!(
        env.store.ingest(&unknown).unwrap_err().code,
        ErrorCode::NotFound
    );
    assert_eq!(
        env.store.counts("nope").unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn protected_globs_come_from_the_agents_role_in_roles_json() {
    let env = Env::new();
    env.p1.write(
        "agents/roles.json",
        &json!({ "agents": [
            { "id": "builder", "protectedGlobs": ["src/core/**"], "displayName": "B" },
            { "id": "designer", "protectedGlobs": [] }
        ] })
        .to_string(),
    );
    let file = env.p1.path().join("src/core/mod.rs");
    assert_eq!(
        env.store.ingest(&write_tool("Edit", &file)).unwrap()[0].severity,
        Severity::Warn
    );
    let mut designer = write_tool("Edit", &file);
    designer.agent_id = "designer".into();
    assert_eq!(
        env.store.ingest(&designer).unwrap()[0].severity,
        Severity::Info
    );
}

// ---------- R-8a-4: two projects with the same agent id never mix ----------

#[test]
fn two_projects_with_the_same_agent_id_never_mix() {
    let env = Env::new();
    env.store
        .ingest(&bash("PostToolUse", "echo one", json!({})))
        .unwrap();
    env.store
        .ingest(&bash("PostToolUse", "echo two", json!({})))
        .unwrap();
    env.store
        .ingest(&in_project(
            bash("PostToolUse", "echo other", json!({})),
            P2,
        ))
        .unwrap();

    let p1 = env.all(P1);
    let p2 = env.all(P2);
    assert_eq!(p1.iter().map(|e| e.id).collect::<Vec<_>>(), [2, 1]);
    assert_eq!(
        p2.iter().map(|e| e.id).collect::<Vec<_>>(),
        [1],
        "ids are per project"
    );
    assert!(p1.iter().all(|e| e.project_id == P1));
    assert_eq!(
        (p2[0].project_id.as_str(), p2[0].command.as_deref()),
        (P2, Some("echo other"))
    );
    assert!(!env
        .p2
        .read(".foundry/events/builder.jsonl")
        .contains("echo one"));

    env.store.mark_read(P1, AGENT, 2).unwrap();
    assert_eq!(env.store.counts(P1).unwrap()[0].unread, 0);
    assert_eq!(
        env.store.counts(P2).unwrap()[0].unread,
        1,
        "markers are per project"
    );

    // Failure streaks are per project too: 2 in p1 + 1 in p2 is no `stuck`.
    env.store.ingest(&failed("make")).unwrap();
    env.store.ingest(&failed("make")).unwrap();
    let p2_fail = env.store.ingest(&in_project(failed("make"), P2)).unwrap();
    assert_eq!(p2_fail.len(), 1);
    assert!(env.sink.all().iter().all(|e| e.kind != EventKind::Stuck));
}

// ---------- R-8a-7: quiet fires once per quiet stretch ----------

fn running(env: &Env, project: &str) -> u64 {
    let key = SessionKey::new(project, AGENT);
    env.sessions.reserve(&key, 5).unwrap();
    env.sessions.set_running(&key, Some(1)).unwrap().since
}

#[test]
fn quiet_fires_once_per_quiet_stretch() {
    let env = Env::new();
    let quiet_ms = QUIET_AFTER.as_millis() as u64;
    let since = running(&env, P1);
    let quiet = |env: &Env| env.store.check_quiet();

    env.clock.set(since + quiet_ms - 1);
    assert!(quiet(&env).is_empty(), "not yet 10 minutes");
    env.clock.set(since + quiet_ms);
    let first = quiet(&env);
    assert_eq!(first.len(), 1);
    assert_eq!(
        (
            first[0].kind,
            first[0].severity,
            first[0].project_id.as_str()
        ),
        (EventKind::Quiet, Severity::Info, P1)
    );
    env.clock.set(since + 5 * quiet_ms);
    assert!(quiet(&env).is_empty(), "still the same stretch");

    // Activity ends the stretch; the next one gets its own quiet, with the agent's slice.
    let mut active = bash("PostToolUse", "ls", json!({}));
    active.slice_id = Some("s-feed".into());
    env.store.ingest(&active).unwrap();
    let resumed = env.clock.now_ms();
    env.clock.set(resumed + quiet_ms - 1);
    assert!(quiet(&env).is_empty());
    env.clock.set(resumed + quiet_ms);
    let second = quiet(&env);
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].slice_id.as_deref(), Some("s-feed"));

    let stored: Vec<EventKind> = env.all(P1).into_iter().map(|e| e.kind).collect();
    assert_eq!(
        stored,
        [EventKind::Quiet, EventKind::Command, EventKind::Quiet]
    );
    assert_eq!(env.sink.all().len(), 3);
    assert!(env.all(P2).is_empty(), "the other project saw nothing");
}

#[test]
fn quiet_ignores_sessions_that_are_not_running() {
    let env = Env::new();
    let key = SessionKey::new(P1, AGENT);
    env.sessions.reserve(&key, 5).unwrap(); // starting, not running
    env.clock.set(u64::MAX / 2);
    assert!(env.store.check_quiet().is_empty());
    env.sessions.release(&key);
    assert!(env.store.check_quiet().is_empty());
}

#[test]
fn the_quiet_timer_task_checks_sessions_on_its_own() {
    let env = Env::new();
    let since = running(&env, P1);
    env.clock.set(since + QUIET_AFTER.as_millis() as u64);
    let timer = env.store.spawn_quiet_timer(Duration::from_millis(10));
    wait_until("a quiet event", || !env.sink.all().is_empty());
    std::thread::sleep(Duration::from_millis(100));
    timer.abort();
    let quiet: Vec<AgentEvent> = env.sink.all();
    assert_eq!(quiet.len(), 1, "one per stretch, however many ticks");
    assert_eq!(quiet[0].kind, EventKind::Quiet);
}

// ---------- R-8a-2: the server rejects wrong tokens and unknown projects ----------

/// Sends one raw HTTP/1.1 request and returns the status and body.
fn http(
    port: u16,
    token: Option<&str>,
    path: &str,
    body: &str,
    declared: Option<usize>,
) -> (u16, Value) {
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut req = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n",
        declared.unwrap_or(body.len())
    );
    if let Some(token) = token {
        req.push_str(&format!("{TOKEN_HEADER}: {token}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    let _ = stream.write_all(req.as_bytes());
    let mut raw = String::new();
    let _ = stream.read_to_string(&mut raw);
    let status = raw
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    let json = raw
        .split_once("\r\n\r\n")
        .and_then(|(_, b)| serde_json::from_str(b).ok())
        .unwrap_or(Value::Null);
    (status, json)
}

fn body(project: &str, slice: Option<&str>) -> String {
    json!({
        "projectId": project,
        "agentId": AGENT,
        "sliceId": slice,
        "hookEvent": "SessionStart",
        "payload": { "hook_event_name": "SessionStart", "source": "startup" }
    })
    .to_string()
}

#[test]
fn the_server_rejects_wrong_or_missing_tokens_and_unknown_projects() {
    let env = Env::new();
    let cell = Arc::new(HookEndpointCell::default());
    let server = server::start(Arc::clone(&env.store), Arc::clone(&cell)).unwrap();
    let (port, token) = (server.endpoint().port, server.endpoint().token.clone());
    assert_eq!(token.len(), 64);

    let (status, err) = http(port, Some("wrong"), "/events", &body(P1, None), None);
    assert_eq!((status, err["code"].as_str()), (401, Some("unauthorized")));
    let almost = format!("{}x", &token[..63]);
    assert_eq!(
        http(port, Some(&almost), "/events", &body(P1, None), None).0,
        401
    );
    assert_eq!(http(port, None, "/events", &body(P1, None), None).0, 401);
    let (status, err) = http(port, Some(&token), "/events", &body("nope", None), None);
    assert_eq!(
        (status, err["code"].as_str()),
        (404, Some("unknown_project"))
    );
    assert_eq!(
        http(port, Some(&token), "/events", "{not json", None).0,
        400
    );
    assert_eq!(
        http(port, Some(&token), "/other", &body(P1, None), None).0,
        404
    );
    let (status, err) = http(
        port,
        Some(&token),
        "/events",
        &body(P1, None),
        Some(server::BODY_CAP + 1),
    );
    assert_eq!(
        (status, err["code"].as_str()),
        (413, Some("payload_too_large"))
    );
    let huge =
        json!({ "projectId": P1, "agentId": AGENT, "payload": "x".repeat(server::BODY_CAP) })
            .to_string();
    assert_eq!(http(port, Some(&token), "/events", &huge, None).0, 413);
    std::thread::sleep(Duration::from_millis(100));
    assert!(env.all(P1).is_empty(), "nothing refused was stored");
    assert!(env.sink.all().is_empty());

    let (status, ok) = http(port, Some(&token), "/events", &body(P1, Some("s-1")), None);
    assert_eq!((status, ok["accepted"].as_bool()), (202, Some(true)));
    wait_until("the event", || !env.sink.all().is_empty());
    let stored = env.all(P1);
    assert_eq!(stored.len(), 1);
    assert_eq!(
        (stored[0].kind, stored[0].slice_id.as_deref()),
        (EventKind::SessionStarted, Some("s-1"))
    );

    // Requests from another project land in that project, in order.
    for _ in 0..3 {
        assert_eq!(
            http(port, Some(&token), "/events", &body(P2, None), None).0,
            202
        );
    }
    wait_until("p2's events", || env.all(P2).len() == 3);
    assert_eq!(
        env.all(P2).iter().map(|e| e.id).collect::<Vec<_>>(),
        [3, 2, 1]
    );
    assert_eq!(env.all(P1).len(), 1);

    drop(server);
    assert_eq!(cell.get(), None, "a stopped server withdraws its endpoint");
}

// ---------- R-8a-9: another feature reads the endpoint through core only ----------

/// Stands in for agent sessions: given only the core cell, it finds the server.
fn foundry_hook_env(cell: &HookEndpointCell) -> Option<(String, String)> {
    cell.get().map(|e| (e.port.to_string(), e.token))
}

#[test]
fn the_published_port_and_token_are_readable_through_core_and_work() {
    let env = Env::new();
    let cell = Arc::new(HookEndpointCell::default());
    assert_eq!(
        foundry_hook_env(&cell),
        None,
        "nothing before the server starts"
    );
    let server = server::start(Arc::clone(&env.store), Arc::clone(&cell)).unwrap();

    let (port, token) = foundry_hook_env(&cell).unwrap();
    assert_eq!(port, server.endpoint().port.to_string());
    let (status, _) = http(
        port.parse().unwrap(),
        Some(&token),
        "/events",
        &body(P1, None),
        None,
    );
    assert_eq!(status, 202);
    wait_until("the event", || env.all(P1).len() == 1);

    // Each run gets its own token.
    let other_cell = Arc::new(HookEndpointCell::default());
    let other = server::start(Arc::clone(&env.store), other_cell).unwrap();
    assert_ne!(other.endpoint().token, token);
    assert_ne!(other.endpoint().port, server.endpoint().port);
}

#[test]
fn the_server_listens_on_loopback_only() {
    let env = Env::new();
    let server = server::start(
        Arc::clone(&env.store),
        Arc::new(HookEndpointCell::default()),
    )
    .unwrap();
    assert_eq!(server.address().ip(), std::net::Ipv4Addr::LOCALHOST);
    assert_eq!(server.address().port(), server.endpoint().port);
    assert!(std::net::TcpStream::connect(server.address()).is_ok());
}

// ---------- R-8a-8: the commands round trip ----------

#[test]
fn commands_round_trip_over_ipc() {
    use tauri::test::{mock_builder, mock_context, noop_assets, INVOKE_KEY};
    use tauri::{Manager, WebviewWindowBuilder};

    let env = Env::new();
    env.store
        .ingest(&bash("PostToolUse", "ls", json!({})))
        .unwrap();
    let mut blocked = bash("PreToolUse", "rm -rf /", Value::Null);
    blocked.decision = Some(HookDecision {
        blocked: true,
        rule: None,
    });
    env.store.ingest(&blocked).unwrap();

    let app = mock_builder()
        .invoke_handler(crate::specta_builder::<tauri::test::MockRuntime>().invoke_handler())
        .build(mock_context(noop_assets()))
        .unwrap();
    app.manage(Arc::clone(&env.store));
    let webview = WebviewWindowBuilder::new(&app, "main", Default::default())
        .build()
        .unwrap();
    let invoke = |cmd: &str, body: Value| {
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
        .map(|body| body.deserialize::<Value>().unwrap())
    };

    let events = invoke(
        "agent_events",
        json!({ "projectId": P1, "agentId": AGENT, "query": {} }),
    )
    .unwrap();
    assert_eq!(events[0]["id"], 2);
    assert_eq!(events[0]["kind"], "blocked");
    assert_eq!(events[0]["severity"], "danger");
    assert_eq!(events[0]["projectId"], P1);
    assert_eq!(events[0]["sliceId"], Value::Null);
    assert_eq!(events[1]["command"], "ls");

    let filtered = invoke(
        "agent_events",
        json!({ "projectId": P1, "agentId": AGENT,
                "query": { "beforeId": null, "limit": 5, "minSeverity": "warn" } }),
    )
    .unwrap();
    assert_eq!(filtered.as_array().unwrap().len(), 1);

    let counts = invoke("agent_event_counts", json!({ "projectId": P1 })).unwrap();
    assert_eq!(
        counts,
        json!([{ "agentId": "builder", "unread": 2, "danger": 1 }])
    );
    assert_eq!(
        invoke(
            "agent_mark_read",
            json!({ "projectId": P1, "agentId": AGENT, "upToId": 2 })
        )
        .unwrap(),
        Value::Null
    );
    let counts = invoke("agent_event_counts", json!({ "projectId": P1 })).unwrap();
    assert_eq!(counts[0]["unread"], 0);

    let err = invoke("agent_event_counts", json!({ "projectId": "nope" })).unwrap_err();
    assert_eq!(err["code"], "not_found");
}

#[test]
fn agent_event_serializes_with_every_field() {
    let event = AgentEvent {
        id: 7,
        ts: 5,
        project_id: "p".into(),
        agent_id: "a".into(),
        slice_id: Some("s".into()),
        kind: EventKind::ReportReady,
        severity: Severity::Info,
        title: "t".into(),
        detail: None,
        path: Some(".foundry/reports/s.json".into()),
        command: None,
    };
    assert_eq!(
        serde_json::to_value(&event).unwrap(),
        json!({
            "id": 7, "ts": 5, "projectId": "p", "agentId": "a", "sliceId": "s",
            "kind": "report_ready", "severity": "info", "title": "t", "detail": null,
            "path": ".foundry/reports/s.json", "command": null
        })
    );
    assert!(Severity::Info < Severity::Warn && Severity::Warn < Severity::Danger);
}

// ---------- P8b: real payloads captured from Claude Code 2.1.292 ----------

/// A captured payload from `fixtures/`, with `{{ROOT}}` pointing at `root`.
fn fixture(name: &str, root: &Path) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/features/agent_events/fixtures")
        .join(format!("{name}.json"));
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{name}: {e}"));
    // Substituted inside JSON strings, so the root's backslashes must be escaped.
    let root = serde_json::to_string(&root.to_string_lossy()).unwrap();
    let root = &root[1..root.len() - 1];
    serde_json::from_str(&text.replace("{{ROOT}}", root)).unwrap()
}

fn from_fixture(name: &str, root: &Path) -> HookPost {
    let payload = fixture(name, root);
    let event = payload["hook_event_name"].as_str().unwrap().to_string();
    post(&event, payload)
}

#[test]
fn every_captured_payload_classifies_as_expected() {
    let repo = TestRepo::new();
    let root = repo.path();
    let cases: &[(&str, Option<(EventKind, &str)>)] = &[
        (
            "session_start",
            Some((EventKind::SessionStarted, "Session started")),
        ),
        ("pre_tool_use_bash", None),
        ("pre_tool_use_write", None),
        ("pre_tool_use_edit", None),
        (
            "post_tool_use_bash",
            Some((EventKind::Command, "Ran a command")),
        ),
        (
            "post_tool_use_failure_bash",
            Some((EventKind::Command, "Command failed")),
        ),
        (
            "post_tool_use_write",
            Some((EventKind::Edit, "Edited a.txt")),
        ),
        (
            "post_tool_use_edit",
            Some((EventKind::Edit, "Edited a.txt")),
        ),
        ("stop", Some((EventKind::TurnComplete, "Turn complete"))),
        (
            "session_end",
            Some((EventKind::SessionEnded, "Session ended")),
        ),
    ];
    for (name, expected) in cases {
        let all = drafts(&from_fixture(name, root), root, &floor());
        match expected {
            None => assert!(all.is_empty(), "{name}: {all:?}"),
            Some((kind, title)) => {
                assert_eq!(all.len(), 1, "{name}: {all:?}");
                assert_eq!(
                    (all[0].kind, all[0].title.as_str()),
                    (*kind, *title),
                    "{name}"
                );
            }
        }
    }
    let detail = |name: &str| one(&from_fixture(name, root), root, &[]).detail;
    assert_eq!(detail("session_start").as_deref(), Some("startup"));
    assert_eq!(detail("session_end").as_deref(), Some("other"));
    assert_eq!(detail("stop").as_deref(), Some("done"));
    assert_eq!(
        detail("post_tool_use_failure_bash").as_deref(),
        Some("Exit code 3")
    );
    let edit = one(&from_fixture("post_tool_use_edit", root), root, &[]);
    assert_eq!(edit.path.as_deref(), Some("a.txt"), "made repo-relative");
    assert_eq!(
        one(&from_fixture("post_tool_use_bash", root), root, &[])
            .command
            .as_deref(),
        Some("echo hi")
    );
}

#[test]
fn captured_shapes_give_test_results_and_stuck() {
    let repo = TestRepo::new();
    let root = repo.path();
    let with_command = |name: &str, command: &str| {
        let mut p = from_fixture(name, root);
        p.payload["tool_input"]["command"] = json!(command);
        p
    };
    let passed = one(&with_command("post_tool_use_bash", "cargo test"), root, &[]);
    assert_eq!(
        (passed.kind, passed.title.as_str()),
        (EventKind::TestsRan, "Tests passed")
    );
    let failed_tests = one(
        &with_command("post_tool_use_failure_bash", "pnpm test"),
        root,
        &[],
    );
    assert_eq!(failed_tests.title, "Tests failed");

    // Three real failures of the same command: stuck on the third.
    let mut streak = Streak::default();
    let mut kinds = Vec::new();
    for _ in 0..3 {
        let out = classify::classify(
            &from_fixture("post_tool_use_failure_bash", root),
            &Context {
                repo_root: Some(root),
                protected_globs: &[],
                streak: Some(&streak),
            },
        );
        streak = out.streak;
        kinds = out.drafts.into_iter().map(|d| d.kind).collect();
    }
    assert_eq!(kinds, [EventKind::Command, EventKind::Stuck]);

    // An interrupted call (is_interrupt) isn't a failure and breaks the run.
    let mut interrupted = from_fixture("post_tool_use_failure_bash", root);
    interrupted.payload["is_interrupt"] = json!(true);
    let out = classify::classify(
        &interrupted,
        &Context {
            repo_root: Some(root),
            protected_globs: &[],
            streak: Some(&streak),
        },
    );
    assert_eq!(out.drafts[0].title, "Command interrupted");
    assert_eq!(out.drafts[0].detail, None);
    assert_eq!(out.streak, Streak::default());
}

#[test]
fn a_warning_from_the_helper_is_a_warn_event() {
    let repo = TestRepo::new();
    let mut push = from_fixture("pre_tool_use_bash", repo.path());
    push.payload["tool_input"]["command"] = json!("git push origin main");
    push.decision = Some(HookDecision {
        blocked: false,
        rule: Some("git_push".into()),
    });
    let d = one(&push, repo.path(), &[]);
    assert_eq!((d.kind, d.severity), (EventKind::Command, Severity::Warn));
    assert_eq!(d.detail.as_deref(), Some("git_push"));
    assert_eq!(d.command.as_deref(), Some("git push origin main"));
    // An allowed call with no rule is still nothing until it finishes.
    let mut plain = from_fixture("pre_tool_use_bash", repo.path());
    plain.decision = Some(HookDecision::default());
    assert!(drafts(&plain, repo.path(), &[]).is_empty());
}

// ---------- P8b: desktop notifications ----------

#[derive(Default)]
struct Shown(Mutex<Vec<AgentEvent>>);

impl OsNotifier for Shown {
    fn show(&self, event: &AgentEvent) {
        self.0.lock().unwrap().push(event.clone());
    }
}

impl Shown {
    fn kinds(&self) -> Vec<EventKind> {
        self.0.lock().unwrap().iter().map(|e| e.kind).collect()
    }
    fn count(&self) -> usize {
        self.0.lock().unwrap().len()
    }
}

struct Notifying {
    env: Env,
    shown: Arc<Shown>,
    policy: Arc<Mutex<NotifyPolicy>>,
}

fn notifying(enabled: bool, min_severity: Severity) -> Notifying {
    let shown = Arc::new(Shown::default());
    let policy = Arc::new(Mutex::new(NotifyPolicy {
        enabled,
        min_severity,
    }));
    let (for_store, for_policy) = (Arc::clone(&shown), Arc::clone(&policy));
    let env = Env::with(move |store| {
        store.with_notifications(move || *for_policy.lock().unwrap(), for_store)
    });
    Notifying { env, shown, policy }
}

fn blocked_rm() -> HookPost {
    let mut p = bash("PreToolUse", "rm -rf /", Value::Null);
    p.decision = Some(HookDecision {
        blocked: true,
        rule: Some("rm_rf_outside".into()),
    });
    p
}

fn needs_attention() -> HookPost {
    post(
        "Notification",
        json!({ "notification_type": "permission_prompt" }),
    )
}

#[test]
fn notifications_fire_only_at_or_above_the_threshold() {
    let n = notifying(true, Severity::Danger);
    let step = DEBOUNCE_MS + 1;
    let at = |n: &Notifying, k: u64| n.env.clock.set(1_700_000_000_000 + k * step);
    n.env
        .store
        .ingest(&bash("PostToolUse", "ls", json!({})))
        .unwrap();
    at(&n, 1);
    n.env.store.ingest(&needs_attention()).unwrap();
    assert_eq!(n.shown.count(), 0, "info and warn are below danger");
    at(&n, 2);
    n.env.store.ingest(&blocked_rm()).unwrap();
    assert_eq!(n.shown.kinds(), [EventKind::Blocked]);

    let warn = notifying(true, Severity::Warn);
    warn.env
        .store
        .ingest(&bash("PostToolUse", "ls", json!({})))
        .unwrap();
    assert_eq!(warn.shown.count(), 0);
    warn.env.store.ingest(&needs_attention()).unwrap();
    assert_eq!(
        warn.shown.kinds(),
        [EventKind::NeedsAttention],
        "warn meets warn"
    );

    let all = notifying(true, Severity::Info);
    all.env.store.ingest(&post("Stop", json!({}))).unwrap();
    assert_eq!(all.shown.kinds(), [EventKind::TurnComplete]);
}

#[test]
fn notifications_never_fire_when_disabled() {
    let n = notifying(false, Severity::Info);
    for (k, p) in [blocked_rm(), needs_attention(), post("Stop", json!({}))]
        .iter()
        .enumerate()
    {
        n.env.clock.set(1_700_000_000_000 + k as u64 * 60_000);
        n.env.store.ingest(p).unwrap();
    }
    let since = running(&n.env, P1);
    n.env.clock.set(since + QUIET_AFTER.as_millis() as u64);
    assert_eq!(n.env.store.check_quiet().len(), 1);
    assert_eq!(n.shown.count(), 0);
    assert_eq!(
        n.env.sink.all().len(),
        4,
        "events are still stored and emitted"
    );

    // The policy is read on every event: enabling it applies at once.
    n.policy.lock().unwrap().enabled = true;
    n.env.clock.set(n.env.clock.now_ms() + 60_000);
    n.env.store.ingest(&blocked_rm()).unwrap();
    assert_eq!(n.shown.count(), 1);
}

const DEBOUNCE_MS: u64 = notify::DEBOUNCE.as_millis() as u64;

#[test]
fn a_burst_from_one_agent_gives_one_notification() {
    let n = notifying(true, Severity::Warn);
    let start = n.env.clock.now_ms();
    for k in 0..6 {
        n.env.clock.set(start + k * 500);
        n.env.store.ingest(&blocked_rm()).unwrap();
    }
    assert_eq!(n.env.sink.all().len(), 6, "every event is still emitted");
    assert_eq!(n.shown.count(), 1, "one per burst");

    // One hook call that makes two qualifying events (a failure and stuck) shows the worst.
    let other = notifying(true, Severity::Info);
    other.env.store.ingest(&failed("make")).unwrap();
    other.env.clock.set(other.env.clock.now_ms() + DEBOUNCE_MS);
    other.env.store.ingest(&failed("make")).unwrap();
    other.env.clock.set(other.env.clock.now_ms() + DEBOUNCE_MS);
    other.env.store.ingest(&failed("make")).unwrap();
    assert_eq!(other.shown.kinds().last(), Some(&EventKind::Stuck));

    // Another agent, or the same agent in another project, isn't held back.
    let mut designer = blocked_rm();
    designer.agent_id = "designer".into();
    n.env.store.ingest(&designer).unwrap();
    n.env.store.ingest(&in_project(blocked_rm(), P2)).unwrap();
    assert_eq!(n.shown.count(), 3);

    // After the window, the same agent notifies again.
    n.env.clock.set(start + 5 * 500 + DEBOUNCE_MS);
    n.env.store.ingest(&blocked_rm()).unwrap();
    assert_eq!(n.shown.count(), 4);
}

// ---------- the writes ledger ----------

/// `post` from session `session` on slice `feed`.
fn in_session(mut post: HookPost, session: &str) -> HookPost {
    post.payload["session_id"] = json!(session);
    post.slice_id = Some("feed".into());
    post
}

#[test]
fn ingest_records_edit_tool_writes_and_snapshots_each_session_in_the_writes_ledger() {
    use crate::core::writes::{self, EntryKind, Moment};
    let repo = TestRepo::with_git();
    repo.write("src/lib.rs", "// start\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "--quiet", "-m", "start"]);
    let repos = Arc::new(RepoIndex::default());
    repos.replace_all([(P1, repo.path())]);
    let clock = Arc::new(FakeClock(AtomicU64::new(1_000)));
    let store = AgentEventsStore::new(
        repos,
        Arc::new(SessionRegistry::default()),
        Arc::new(Recorded::default()),
    )
    .with_clock(clock.clone());
    let start = |event: &str| {
        in_session(
            post(
                event,
                json!({ "hook_event_name": event, "source": "startup" }),
            ),
            "sess-1",
        )
    };

    store.ingest(&start("SessionStart")).unwrap();
    clock.set(2_000);
    repo.write("src/feed.rs", "// edited\n");
    store
        .ingest(&in_session(
            write_tool("Edit", &repo.path().join("src").join("feed.rs")),
            "sess-1",
        ))
        .unwrap();
    // A shell write: a command event, no ledger write, but the snapshots see it.
    repo.write("gen/out.txt", "generated\n");
    store
        .ingest(&in_session(
            bash("PostToolUse", "make gen", json!({})),
            "sess-1",
        ))
        .unwrap();
    // Blocked, failed, and session-less calls add nothing.
    let mut blocked = in_session(
        write_tool("Write", &repo.path().join("AGENTS.md")),
        "sess-1",
    );
    blocked.decision = Some(HookDecision {
        blocked: true,
        rule: None,
    });
    store.ingest(&blocked).unwrap();
    store
        .ingest(&write_tool("Write", &repo.path().join("src/anon.rs")))
        .unwrap();
    clock.set(3_000);
    // The agent's report: a write, then an end snapshot.
    let report = repo.write(".foundry/reports/feed.builder.json", "{}");
    store
        .ingest(&in_session(write_tool("Write", &report), "sess-1"))
        .unwrap();
    clock.set(4_000);
    store.ingest(&start("SessionEnd")).unwrap();

    let ledger: Vec<(EntryKind, Option<String>, u64)> = writes::read_ledger(repo.path())
        .unwrap()
        .into_iter()
        .inspect(|e| {
            assert_eq!(
                (
                    e.session_id.as_str(),
                    e.slice_id.as_deref(),
                    e.agent_id.as_str()
                ),
                ("sess-1", Some("feed"), AGENT)
            )
        })
        .map(|e| (e.kind, e.path, e.ts))
        .collect();
    assert_eq!(
        ledger,
        [
            (EntryKind::Start, None, 1_000),
            (EntryKind::Write, Some("src/feed.rs".into()), 2_000),
            (
                EntryKind::Write,
                Some(".foundry/reports/feed.builder.json".into()),
                3_000
            ),
            (EntryKind::Snapshot, None, 3_000),
            (EntryKind::Snapshot, None, 4_000),
            (EntryKind::End, None, 4_000),
        ]
    );
    let before = writes::read_snapshot(repo.path(), "sess-1", Moment::Start)
        .unwrap()
        .unwrap();
    let after = writes::read_snapshot(repo.path(), "sess-1", Moment::End)
        .unwrap()
        .unwrap();
    let changed = before.changed(&after);
    assert!(changed.contains("gen/out.txt"), "{changed:?}");
    assert!(changed.contains("src/feed.rs"), "{changed:?}");
    assert!(!changed.contains("src/lib.rs"), "{changed:?}");

    // A second SessionStart (a resume) keeps the first start snapshot.
    clock.set(5_000);
    store.ingest(&start("SessionStart")).unwrap();
    assert_eq!(
        writes::read_snapshot(repo.path(), "sess-1", Moment::Start)
            .unwrap()
            .unwrap(),
        before
    );
}
