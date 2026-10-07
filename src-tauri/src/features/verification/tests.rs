use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::*;
use crate::core::process_tree::pid_alive;
use crate::core::sessions::SessionKey;
use crate::testing::{fake_agent_path, TestRepo};

const P: &str = "project-1";
const S: &str = "s-feed";

// ---------- fixtures ----------

#[derive(Default)]
struct Recorded(Mutex<Vec<VerifyProgress>>);

impl VerifyEvents for Recorded {
    fn progress(&self, event: VerifyProgress) {
        self.0.lock().unwrap().push(event);
    }
}

impl Recorded {
    fn stages(&self) -> Vec<VerifyStage> {
        self.0.lock().unwrap().iter().map(|e| e.stage).collect()
    }
}

struct Env {
    repo: TestRepo,
    data: tempfile::TempDir,
    /// Scratch space outside the repo (pid files).
    scratch: tempfile::TempDir,
    sessions: Arc<SessionRegistry>,
    events: Arc<Recorded>,
    store: Arc<VerificationStore>,
}

/// `"<fake-agent>" <args>`, ready for the platform shell.
fn fake(args: &str) -> String {
    format!("\"{}\" {args}", fake_agent_path().display())
}

fn report(requirements: Value) -> Value {
    json!({
        "sliceId": S,
        "agent": "ada",
        "requirements": requirements,
        "testsRun": [{ "command": "cargo test", "result": "3 passed" }],
        "blockers": [],
        "notes": ""
    })
}

fn met(id: &str, evidence: &str) -> Value {
    json!({ "id": id, "status": "met", "evidence": evidence })
}

fn codes(v: &Verification) -> Vec<&str> {
    v.review.findings.iter().map(|f| f.code.as_str()).collect()
}

fn finding<'a>(v: &'a Verification, code: &str) -> &'a Finding {
    v.review
        .findings
        .iter()
        .find(|f| f.code == code)
        .unwrap_or_else(|| panic!("no {code} in {:?}", v.review.findings))
}

impl Env {
    /// A git repo with a team (ada: `src/**`, protected `src/secret/**`), a committed start,
    /// and slice `s-feed` (requirements r-1 and r-2) in progress from that commit.
    fn new() -> Self {
        Self::with_git(Git::system())
    }

    fn with_git(git: Git) -> Self {
        let repo = TestRepo::with_git();
        repo.write(
            ".foundry/project.json",
            &json!({ "id": P, "name": "Orbit", "tagline": "", "createdAt": 1 }).to_string(),
        );
        repo.write(
            "agents/roles.json",
            &json!({ "agents": [
                { "id": "ada", "writeGlobs": ["src/**"], "protectedGlobs": ["src/secret/**"] },
                { "id": "bo", "writeGlobs": ["web/**"], "protectedGlobs": [] }
            ]})
            .to_string(),
        );
        repo.write("AGENTS.md", "# rules\n");
        repo.write("src/lib.rs", "// start\n");
        repo.write(".gitignore", "target/\n");
        repo.git(&["add", "-A"]);
        repo.git(&["commit", "--quiet", "-m", "start"]);
        let head = repo.git(&["rev-parse", "HEAD"]);
        let env = Self::in_repo(repo, git);
        env.plan(Some(&head));
        env
    }

    fn in_repo(repo: TestRepo, git: Git) -> Self {
        let data = tempfile::tempdir().unwrap();
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([(P, repo.path())]);
        let sessions = Arc::new(SessionRegistry::default());
        let events = Arc::new(Recorded::default());
        let store = VerificationStore::new(
            repos,
            Arc::clone(&sessions),
            git,
            data.path(),
            Arc::clone(&events) as Arc<dyn VerifyEvents>,
        );
        Self {
            repo,
            data,
            scratch: tempfile::tempdir().unwrap(),
            sessions,
            events,
            store: Arc::new(store),
        }
    }

    fn timeout(mut self, timeout: Duration) -> Self {
        let store = Arc::try_unwrap(self.store).ok().unwrap();
        self.store = Arc::new(store.with_timeout(timeout));
        self
    }

    fn plan(&self, start_commit: Option<&str>) {
        self.repo.write(
            ".foundry/plan.json",
            &json!({
                "version": 1,
                "revision": 3,
                "slices": [{
                    "id": S, "name": "Feed", "delivers": [], "dependsOn": [],
                    "status": "in_progress", "decisions": [], "openQuestions": [],
                    "startCommit": start_commit
                }],
                "requirements": [
                    { "id": "r-1", "sliceId": S },
                    { "id": "r-2", "sliceId": S },
                    { "id": "r-9", "sliceId": "s-other" }
                ]
            })
            .to_string(),
        );
    }

    fn write_report(&self, report: &Value) {
        self.repo
            .write(&format!(".foundry/reports/{S}.json"), &report.to_string());
    }

    /// Sets and confirms the test commands, commits `project.json`, and restarts the slice
    /// from that commit (so the change isn't in the slice's diff).
    fn commands(&self, commands: &[String]) {
        let shown = self.store.set_test_commands(P, commands.to_vec()).unwrap();
        assert!(
            self.store
                .confirm_test_commands(P, shown.commands)
                .unwrap()
                .confirmed
        );
        self.repo.git(&["add", ".foundry/project.json"]);
        self.repo
            .git(&["commit", "--quiet", "--allow-empty", "-m", "test commands"]);
        self.plan(Some(&self.repo.git(&["rev-parse", "HEAD"])));
    }

    fn verify(&self) -> Result<Verification, AppError> {
        self.store.verify(P, S)
    }

    fn pid_file(&self) -> PathBuf {
        self.scratch.path().join("pids.txt")
    }

    /// A command that starts a child, writes its pid to [`pid_file`], then sleeps 60 s.
    fn sleeper(&self) -> String {
        format!(
            "{} > \"{}\"",
            fake("--sleep-ms 300 --spawn-child 60000 --sleep-ms 60000"),
            self.pid_file().display()
        )
    }

    fn child_pid(&self, within: Duration) -> u32 {
        let deadline = Instant::now() + within;
        loop {
            if let Some(pid) = fs::read_to_string(self.pid_file())
                .ok()
                .and_then(|t| t.trim().strip_prefix("child-pid=")?.parse().ok())
            {
                return pid;
            }
            assert!(
                Instant::now() < deadline,
                "the test command never started its child"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

fn gone_within(timeout: Duration, pid: u32) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if !pid_alive(pid) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// Every file under `root` (including `.git`), relative with `/`, → its bytes.
fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, out);
            } else {
                let rel = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                out.insert(rel, fs::read(&path).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

// ---------- 1. requirement coverage ----------

#[test]
fn a_report_missing_a_requirement_id_gives_unreported_requirement() {
    let env = Env::new();
    env.commands(&[]);
    env.write_report(&report(json!([
        met("r-1", "test feed_loads"),
        met("r-7", "test extra")
    ])));

    let v = env.verify().unwrap();

    let missing = finding(&v, code::UNREPORTED_REQUIREMENT);
    assert_eq!(missing.requirement_id.as_deref(), Some("r-2"));
    assert_eq!(missing.severity, FindingSeverity::Danger);
    let extra = finding(&v, code::UNKNOWN_REQUIREMENT);
    assert_eq!(
        (extra.requirement_id.as_deref(), extra.severity),
        (Some("r-7"), FindingSeverity::Warn),
        "an unknown extra only warns"
    );
    assert!(!v.review.passed);
    assert!(
        v.review
            .findings
            .iter()
            .all(|f| f.requirement_id.as_deref() != Some("r-9")),
        "another slice's requirement isn't expected"
    );
}

#[test]
fn no_report_file_is_refused_with_not_found() {
    let env = Env::new();
    env.commands(&[]);
    assert_eq!(env.verify().unwrap_err().code, ErrorCode::NotFound);
    assert!(!env.repo.path().join(".foundry/reviews").exists());
    assert!(env.events.stages().is_empty(), "refused before starting");
    assert_eq!(env.store.get_report(P, S).unwrap(), None);
    assert_eq!(env.store.get_verification(P, S).unwrap(), None);
}

// ---------- 2. evidence ----------

#[test]
fn met_with_empty_or_unnamed_evidence_is_a_finding() {
    let env = Env::new();
    env.commands(&[]);
    env.write_report(&report(json!([
        met("r-1", "  "),
        met("r-2", "it works on my machine"),
    ])));
    let v = env.verify().unwrap();
    let flagged: Vec<&str> = v
        .review
        .findings
        .iter()
        .filter(|f| f.code == code::MISSING_EVIDENCE)
        .filter_map(|f| f.requirement_id.as_deref())
        .collect();
    assert_eq!(flagged, ["r-1", "r-2"]);

    // Naming a test, a backticked name, or a command the agent ran is enough.
    env.write_report(&report(json!([
        met("r-1", "feed_loads_page passes in the test suite"),
        met("r-2", "ran `cargo test` → 3 passed"),
    ])));
    let v = env.verify().unwrap();
    assert!(
        !codes(&v).contains(&code::MISSING_EVIDENCE),
        "{:?}",
        v.review.findings
    );
    // A partly-met requirement needs no evidence check.
    env.write_report(&report(json!([
        met("r-1", "test a"),
        { "id": "r-2", "status": "partly", "evidence": "" }
    ])));
    assert!(!codes(&env.verify().unwrap()).contains(&code::MISSING_EVIDENCE));
}

// ---------- 3. test commands ----------

#[test]
fn a_failing_test_command_fails_the_verification_and_a_passing_one_is_recorded() {
    let env = Env::new();
    std::env::set_var("ANTHROPIC_FOUNDRY_VERIFY_TEST", "provider-secret");
    std::env::set_var("FOUNDRY_VERIFY_TEST_KEEP", "kept-value");
    let passing = fake("--print-cwd");
    // Through the platform shell, so its own syntax works.
    let echo_env = if cfg!(windows) {
        "echo [%ANTHROPIC_FOUNDRY_VERIFY_TEST%] [%FOUNDRY_VERIFY_TEST_KEEP%]"
    } else {
        "echo \"[$ANTHROPIC_FOUNDRY_VERIFY_TEST] [$FOUNDRY_VERIFY_TEST_KEEP]\""
    };
    env.commands(&[passing.clone(), echo_env.to_string()]);
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));

    let v = env.verify().unwrap();
    assert!(v.review.passed, "{:?}", v.review.findings);
    assert!(!v.stale);
    let run = &v.review.test_runs[0];
    assert_eq!(
        (run.command.as_str(), run.exit_code, run.timed_out),
        (passing.as_str(), Some(0), false)
    );
    let cwd = env.repo.path().canonicalize().unwrap();
    let printed = run
        .tail
        .lines()
        .find_map(|l| l.strip_prefix("cwd="))
        .unwrap();
    assert_eq!(
        Path::new(printed.trim()).canonicalize().unwrap(),
        cwd,
        "runs in the repo root"
    );
    let echoed = &v.review.test_runs[1].tail;
    assert!(
        echoed.contains("kept-value"),
        "the environment is passed on: {echoed}"
    );
    assert!(
        !echoed.contains("provider-secret"),
        "provider variables are removed: {echoed}"
    );
    assert_eq!(
        env.events.stages(),
        [
            VerifyStage::Starting,
            VerifyStage::Checking,
            VerifyStage::RunningTests,
            VerifyStage::RunningTests,
            VerifyStage::RunningTests,
            VerifyStage::Checking,
            VerifyStage::Done
        ]
    );
    let with_command: Vec<Option<String>> = env
        .events
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|e| e.command.clone())
        .collect();
    assert_eq!(with_command[3].as_deref(), Some(passing.as_str()));
    assert_eq!(with_command[4].as_deref(), Some(echo_env));
    // Saved, and read back the same.
    let saved: Value =
        serde_json::from_str(&env.repo.read(".foundry/reviews/s-feed.json")).unwrap();
    assert_eq!(saved["passed"], json!(true));
    assert_eq!(
        saved["report"],
        report(json!([met("r-1", "test a"), met("r-2", "test b")]))
    );
    assert!(
        saved.get("stale").is_none(),
        "worked out on read, never stored"
    );
    assert_eq!(env.store.get_verification(P, S).unwrap().unwrap(), v);

    // Now one that fails.
    let failing = fake("--exit 3");
    env.commands(&[passing, failing.clone()]);
    env.write_report(&report(json!([
        met("r-1", "test a"),
        { "id": "r-2", "status": "partly", "evidence": "" }
    ])));
    let v = env.verify().unwrap();
    assert!(!v.review.passed);
    assert_eq!(v.review.test_runs[1].exit_code, Some(3));
    let failed = finding(&v, code::TEST_FAILED);
    assert_eq!(failed.severity, FindingSeverity::Danger);
    assert!(failed.message.contains(&failing), "{}", failed.message);
    assert!(
        !codes(&v).contains(&code::CONTRADICTION),
        "not everything was claimed met"
    );
}

#[test]
fn a_timeout_kills_the_whole_process_tree() {
    let env = Env::new().timeout(Duration::from_secs(2));
    env.commands(&[env.sleeper()]);
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));

    let started = Instant::now();
    let v = env.verify().unwrap();
    assert!(
        started.elapsed() < Duration::from_secs(20),
        "the 60 s sleep was cut short"
    );

    let run = &v.review.test_runs[0];
    assert!(run.timed_out);
    assert_eq!(run.exit_code, None);
    assert_eq!(
        finding(&v, code::TEST_TIMED_OUT).severity,
        FindingSeverity::Danger
    );
    assert!(!v.review.passed);
    let child = env.child_pid(Duration::from_secs(1));
    assert!(
        gone_within(Duration::from_secs(5), child),
        "the command's child survived"
    );
}

#[test]
fn cancel_kills_the_process_tree_and_saves_nothing() {
    let env = Env::new();
    env.commands(&[env.sleeper()]);
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    assert!(!env.store.cancel(P), "nothing running yet");

    let store = Arc::clone(&env.store);
    let worker = std::thread::spawn(move || store.verify(P, S));
    let child = env.child_pid(Duration::from_secs(20));
    assert!(pid_alive(child));
    // One verification per project at a time.
    assert_eq!(
        env.verify().unwrap_err().code,
        ErrorCode::VerificationRunning
    );

    assert!(env.store.cancel(P));
    let err = worker.join().unwrap().unwrap_err();
    assert_eq!(err.code, ErrorCode::Cancelled);
    assert!(
        gone_within(Duration::from_secs(5), child),
        "the command's child survived"
    );
    assert!(
        !env.repo.path().join(".foundry/reviews").exists(),
        "nothing saved"
    );
    assert_eq!(env.events.stages().last(), Some(&VerifyStage::Cancelled));
    assert!(!env.store.cancel(P), "no longer running");
}

#[test]
fn no_test_command_is_a_warning_not_a_failure() {
    let env = Env::new();
    env.commands(&[]);
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    let v = env.verify().unwrap();
    assert_eq!(
        finding(&v, code::NO_TEST_COMMAND).severity,
        FindingSeverity::Warn
    );
    assert!(v.review.test_runs.is_empty());
    assert!(v.review.passed, "{:?}", v.review.findings);
}

// ---------- confirmation ----------

#[test]
fn verify_is_refused_until_the_commands_are_confirmed_and_again_after_they_change() {
    let env = Env::new();
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    let unconfirmed = |env: &Env| {
        assert_eq!(
            env.verify().unwrap_err().code,
            ErrorCode::TestCommandsUnconfirmed
        );
    };

    let ok = fake("--exit 0");
    let set = env
        .store
        .set_test_commands(P, vec![format!("  {ok}  ")])
        .unwrap();
    assert_eq!(
        set,
        TestCommands {
            commands: vec![ok.clone()],
            confirmed: false
        }
    );
    let file: Value = serde_json::from_str(&env.repo.read(".foundry/project.json")).unwrap();
    assert_eq!(file["testCommands"], json!([ok]));
    assert_eq!(file["name"], json!("Orbit"), "other fields kept");
    unconfirmed(&env);

    assert!(
        env.store
            .confirm_test_commands(P, vec![ok.clone()])
            .unwrap()
            .confirmed
    );
    env.verify().unwrap();

    // Setting them again, even to the same list, needs a new confirmation.
    env.store.set_test_commands(P, vec![ok.clone()]).unwrap();
    unconfirmed(&env);
    env.store
        .confirm_test_commands(P, vec![ok.clone()])
        .unwrap();
    env.verify().unwrap();

    // Someone edits project.json behind Foundry's back: refused, and nothing ran.
    let mut file: Value = serde_json::from_str(&env.repo.read(".foundry/project.json")).unwrap();
    file["testCommands"] = json!([ok, "echo pwned"]);
    env.repo.write(".foundry/project.json", &file.to_string());
    assert!(!env.store.get_test_commands(P).unwrap().confirmed);
    let before = env.store.get_verification(P, S).unwrap();
    unconfirmed(&env);
    assert_eq!(env.store.get_verification(P, S).unwrap(), before);
    // Even reformatted whitespace inside a command counts as a change (verbatim).
    file["testCommands"] = json!([format!("{ok} ")]);
    env.repo.write(".foundry/project.json", &file.to_string());
    unconfirmed(&env);

    // The confirmation lives in app data, not in the repo.
    let stored = fs::read_to_string(
        env.data
            .path()
            .join("verification/confirmed-test-commands.json"),
    )
    .unwrap();
    assert!(stored.contains(P));
    let repo_files = snapshot(env.repo.path());
    assert!(
        !repo_files.keys().any(|k| k.contains("confirmed")),
        "{:?}",
        repo_files.keys()
    );

    // Bad input.
    for bad in [vec!["".to_string()], vec!["a\nb".to_string()]] {
        assert_eq!(
            env.store.set_test_commands(P, bad).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
}

// ---------- 4. boundary ----------

#[test]
fn a_diff_touching_a_protected_file_gives_danger() {
    let env = Env::new();
    env.commands(&[]);
    env.repo.write("src/feed.rs", "// the work\n"); // inside ada's globs: fine
    env.repo.write("src/secret/key.rs", "// role-protected\n");
    env.repo.write("AGENTS.md", "# rewritten\n"); // the floor, committed below
    env.repo.write("web/page.html", "<p>someone else's?</p>");
    env.repo.write("target/ignored.bin", "x"); // .gitignore'd
    env.repo.write(".foundry/events/ada.jsonl", "{}\n"); // Foundry's own folders
    env.repo.git(&["add", "AGENTS.md"]);
    env.repo.git(&["commit", "--quiet", "-m", "agent commit"]);
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));

    let v = env.verify().unwrap();
    let by_path: BTreeMap<&str, (&str, FindingSeverity)> = v
        .review
        .findings
        .iter()
        .filter_map(|f| Some((f.path.as_deref()?, (f.code.as_str(), f.severity))))
        .collect();
    assert_eq!(
        by_path,
        BTreeMap::from([
            (
                "AGENTS.md",
                (code::PROTECTED_FILE_CHANGED, FindingSeverity::Danger)
            ),
            (
                "src/secret/key.rs",
                (code::PROTECTED_FILE_CHANGED, FindingSeverity::Danger)
            ),
            (
                "web/page.html",
                (code::OUTSIDE_WRITE_GLOBS, FindingSeverity::Warn)
            ),
            (
                ".foundry/plan.json",
                (code::FOUNDRY_FILE_CHANGED, FindingSeverity::Info)
            ),
        ])
    );
    assert!(!v.review.passed);
}

#[test]
fn setting_test_commands_after_the_start_commit_gives_no_boundary_finding() {
    let env = Env::new();
    // Rewrites project.json after the slice's start commit, as the UI would mid-slice.
    let shown = env
        .store
        .set_test_commands(P, vec![fake("--exit 0")])
        .unwrap();
    env.store.confirm_test_commands(P, shown.commands).unwrap();
    assert_ne!(
        env.repo
            .git(&["status", "--porcelain", "--", ".foundry/project.json"]),
        "",
        "project.json differs from the start commit"
    );
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    let v = env.verify().unwrap();
    assert!(
        v.review
            .findings
            .iter()
            .all(|f| f.path.as_deref() != Some(".foundry/project.json")),
        "{:?}",
        v.review.findings
    );
    assert!(v.review.passed, "{:?}", v.review.findings);
}

#[test]
fn confirming_a_list_that_changed_while_shown_is_refused_and_confirms_nothing() {
    let env = Env::new();
    let ok = fake("--exit 0");
    let shown = env
        .store
        .set_test_commands(P, vec![ok.clone()])
        .unwrap()
        .commands;
    // Something rewrites the list while the human reviews it.
    let mut file: Value = serde_json::from_str(&env.repo.read(".foundry/project.json")).unwrap();
    file["testCommands"] = json!([ok, "echo pwned"]);
    env.repo.write(".foundry/project.json", &file.to_string());
    let confirmed_file = env
        .data
        .path()
        .join("verification/confirmed-test-commands.json");
    let before = fs::read(&confirmed_file).ok();

    let err = env.store.confirm_test_commands(P, shown).unwrap_err();
    assert_eq!(err.code, ErrorCode::TestCommandsChanged);
    assert_eq!(
        err.message,
        "The test commands changed while you were reviewing them. Review them again."
    );
    assert_eq!(fs::read(&confirmed_file).ok(), before, "nothing confirmed");
    assert!(!env.store.get_test_commands(P).unwrap().confirmed);
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    assert_eq!(
        env.verify().unwrap_err().code,
        ErrorCode::TestCommandsUnconfirmed
    );
    // A reordering, a subset or an empty list is a change too.
    let on_disk = env.store.get_test_commands(P).unwrap().commands;
    for wrong in [
        vec![on_disk[1].clone(), on_disk[0].clone()],
        vec![on_disk[0].clone()],
        vec![],
    ] {
        assert_eq!(
            env.store.confirm_test_commands(P, wrong).unwrap_err().code,
            ErrorCode::TestCommandsChanged
        );
    }
    // Confirming what's really there works.
    assert!(
        env.store
            .confirm_test_commands(P, on_disk)
            .unwrap()
            .confirmed
    );
}

#[test]
fn an_empty_command_list_needs_no_confirmation() {
    let env = Env::new();
    // Never set, never confirmed.
    assert_eq!(
        env.store.get_test_commands(P).unwrap(),
        TestCommands {
            commands: vec![],
            confirmed: true
        }
    );
    // The other checks still run.
    env.write_report(&report(json!([met("r-1", "test a")])));
    let v = env.verify().unwrap();
    assert_eq!(
        finding(&v, code::NO_TEST_COMMAND).severity,
        FindingSeverity::Warn
    );
    assert!(codes(&v).contains(&code::UNREPORTED_REQUIREMENT));
    assert!(v.review.test_runs.is_empty());

    // Cleared through Foundry after a non-empty list: still nothing to confirm.
    env.store
        .set_test_commands(P, vec![fake("--exit 0")])
        .unwrap();
    assert_eq!(
        env.verify().unwrap_err().code,
        ErrorCode::TestCommandsUnconfirmed
    );
    assert!(env.store.set_test_commands(P, vec![]).unwrap().confirmed);
    let v = env.verify().unwrap();
    assert!(codes(&v).contains(&code::NO_TEST_COMMAND));
}

#[test]
fn missing_git_or_start_commit_is_a_warning_not_a_crash() {
    let env = Env::with_git(Git::new("foundry-definitely-not-git"));
    env.commands(&[]);
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    let v = env.verify().unwrap();
    let skipped = finding(&v, code::BOUNDARY_SKIPPED);
    assert_eq!(skipped.severity, FindingSeverity::Warn);
    assert!(
        skipped.message.contains("git isn't installed"),
        "{}",
        skipped.message
    );
    assert!(v.review.passed);

    let env = Env::new();
    env.commands(&[]);
    env.plan(None);
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    let v = env.verify().unwrap();
    assert!(finding(&v, code::BOUNDARY_SKIPPED)
        .message
        .contains("no start commit"));

    // A start commit that isn't one never reaches git as an option.
    env.plan(Some("--output=x"));
    let v = env.verify().unwrap();
    assert!(finding(&v, code::BOUNDARY_SKIPPED)
        .message
        .contains("isn't a commit id"));
}

// ---------- 5. contradiction ----------

#[test]
fn a_report_claiming_all_met_while_a_test_fails_gives_danger() {
    let env = Env::new();
    env.commands(&[fake("--exit 1")]);
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    let v = env.verify().unwrap();
    let c = finding(&v, code::CONTRADICTION);
    assert_eq!(c.severity, FindingSeverity::Danger);
    assert!(!v.review.passed);
}

// ---------- stale ----------

#[test]
fn editing_the_report_after_verifying_makes_the_verification_stale() {
    let env = Env::new();
    env.commands(&[]);
    let original = report(json!([met("r-1", "test a"), met("r-2", "test b")]));
    env.write_report(&original);
    assert!(!env.verify().unwrap().stale);

    // Same JSON, different formatting: still current (compared as values).
    env.repo.write(
        &format!(".foundry/reports/{S}.json"),
        &serde_json::to_string_pretty(&original).unwrap(),
    );
    assert!(!env.store.get_verification(P, S).unwrap().unwrap().stale);

    let mut edited = original.clone();
    edited["notes"] = json!("one more thing");
    env.write_report(&edited);
    let v = env.store.get_verification(P, S).unwrap().unwrap();
    assert!(v.stale);
    assert_eq!(
        v.review.report, original,
        "the stored copy is the report that was verified"
    );
    assert_eq!(
        env.store.get_report(P, S).unwrap().unwrap().notes,
        "one more thing"
    );

    fs::remove_file(env.repo.path().join(format!(".foundry/reports/{S}.json"))).unwrap();
    assert!(
        env.store.get_verification(P, S).unwrap().unwrap().stale,
        "a deleted report too"
    );
}

// ---------- writes ----------

#[test]
fn nothing_is_written_outside_reviews_and_app_data() {
    let env = Env::new();
    env.commands(&[fake("--exit 1"), fake("--exit 0")]);
    env.repo.write("src/feed.rs", "// work\n");
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    let data_before = snapshot(env.data.path());
    let before = snapshot(env.repo.path());

    env.verify().unwrap();
    env.store.get_verification(P, S).unwrap();
    env.store.get_report(P, S).unwrap();
    env.store.get_test_commands(P).unwrap();
    env.store.manager_digest(P).unwrap();
    env.store.write_digest_file(P).unwrap();

    let after = snapshot(env.repo.path());
    let changed: Vec<&String> = after
        .iter()
        .filter(|(k, v)| before.get(*k) != Some(*v))
        .map(|(k, _)| k)
        .chain(before.keys().filter(|k| !after.contains_key(*k)))
        .collect();
    assert_eq!(
        changed,
        [".foundry/reviews/digest.md", ".foundry/reviews/s-feed.json"]
    );
    assert_eq!(
        snapshot(env.data.path()),
        data_before,
        "verifying doesn't touch app data"
    );
}

// ---------- digest ----------

fn event(id: u64, ts: u64, agent: &str, severity: &str, title: &str) -> String {
    json!({
        "id": id, "ts": ts, "projectId": P, "agentId": agent, "sliceId": "s-1",
        "kind": "edit", "severity": severity, "title": title,
        "detail": null, "path": null, "command": null
    })
    .to_string()
}

#[test]
fn manager_digest_and_the_digest_file_are_correct_for_a_known_state() {
    let env = Env::in_repo(TestRepo::new(), Git::system());
    let repo = &env.repo;
    repo.write(
        "agents/roles.json",
        &json!({ "agents": [{ "id": "ada" }, { "id": "bo" }] }).to_string(),
    );
    repo.write(
        ".foundry/events/ada.jsonl",
        &[
            event(1, 1_000, "ada", "info", "Edited src/a.rs"),
            event(2, 2_000, "ada", "warn", "Pushed to origin"),
            event(
                3,
                1_791_381_780_000,
                "ada",
                "danger",
                "Blocked | a write\nto AGENTS.md",
            ),
            "{ torn".into(),
        ]
        .join("\n"),
    );
    repo.write(
        ".foundry/events/cy.1.jsonl",
        &event(1, 500, "cy", "danger", "Old"),
    );
    repo.write(".foundry/events/read.json", r#"{ "ada": 1, "cy": 1 }"#);
    env.sessions.reserve(&SessionKey::new(P, "bo"), 3).unwrap();
    env.sessions.set_running(&SessionKey::new(P, "bo"), Some(9));
    env.sessions
        .reserve(&SessionKey::new("other", "ada"), 3)
        .unwrap();

    let slice = |id: &str, name: &str, status: &str| json!({ "id": id, "name": name, "status": status, "startCommit": null });
    repo.write(
        ".foundry/plan.json",
        &json!({ "slices": [
            slice("s-1", "Feed", "done"),
            slice("s-2", "Share", "in_progress"),
            slice("s-3", "Search", "in_progress"),
            slice("s-4", "Profile", "in_progress"),
            slice("s-5", "Later", "queued"),
        ], "requirements": [] })
        .to_string(),
    );
    let rep = |id: &str, notes: &str| json!({ "sliceId": id, "agent": "ada", "requirements": [], "testsRun": [], "blockers": [], "notes": notes });
    for id in ["s-1", "s-2", "s-3", "s-4"] {
        repo.write(
            &format!(".foundry/reports/{id}.json"),
            &rep(id, "").to_string(),
        );
    }
    for (id, passed) in [("s-1", true), ("s-2", false), ("s-4", true)] {
        repo.write(
            &format!(".foundry/reviews/{id}.json"),
            &json!({ "sliceId": id, "passed": passed, "findings": [], "testRuns": [], "report": rep(id, ""), "verifiedAt": 1 }).to_string(),
        );
    }
    repo.write(
        ".foundry/reports/s-4.json",
        &rep("s-4", "edited").to_string(),
    );

    let digest = env.store.manager_digest(P).unwrap();
    let agent = |id: &str, running, last, unread, danger| AgentDigest {
        agent_id: id.into(),
        running,
        last_event_at: last,
        unread,
        unread_danger: danger,
    };
    assert_eq!(
        digest.agents,
        [
            agent("ada", false, Some(1_791_381_780_000), 2, 1),
            agent("bo", true, None, 0, 0),
            agent("cy", false, Some(500), 0, 0),
        ]
    );
    let states: Vec<(&str, &str, bool, VerificationState)> = digest
        .slices
        .iter()
        .map(|s| {
            (
                s.slice_id.as_str(),
                s.status.as_str(),
                s.report_present,
                s.verification,
            )
        })
        .collect();
    assert_eq!(
        states,
        [
            ("s-1", "done", true, VerificationState::Passed),
            ("s-2", "in_progress", true, VerificationState::Failed),
            ("s-3", "in_progress", true, VerificationState::None),
            ("s-4", "in_progress", true, VerificationState::Stale),
            ("s-5", "queued", false, VerificationState::None),
        ]
    );
    let attention: Vec<(FindingSeverity, AttentionKind, Option<&str>, Option<u64>)> = digest
        .attention
        .iter()
        .map(|a| (a.severity, a.kind, a.slice_id.as_deref(), a.event_id))
        .collect();
    use AttentionKind::*;
    use FindingSeverity::{Danger, Warn};
    assert_eq!(
        attention,
        [
            (Danger, FailedVerification, Some("s-2"), None),
            (Danger, UnreadEvent, Some("s-1"), Some(3)),
            (Warn, UnverifiedReport, Some("s-3"), None),
            (Warn, StaleVerification, Some("s-4"), None),
            (Warn, UnreadEvent, Some("s-1"), Some(2)),
        ]
    );

    let written = env.store.write_digest_file(P).unwrap();
    assert_eq!(
        (&written.agents, &written.slices, &written.attention),
        (&digest.agents, &digest.slices, &digest.attention)
    );
    let md = repo.read(".foundry/reviews/digest.md");
    assert_eq!(md, digest::markdown(&written));
    for line in [
        "# Manager digest",
        "## Needs attention",
        "- **danger** (slice s-2): \"Share\" failed verification",
        "- **danger** (agent ada, slice s-1): Blocked \\| a write to AGENTS.md",
        "- **warn** (slice s-3): \"Search\" has a report that hasn't been verified",
        "- **warn** (slice s-4): \"Profile\"'s report changed since it was verified",
        "- **warn** (agent ada, slice s-1): Pushed to origin",
        "| Agent | Running | Last event | Unread | Unread danger |",
        "| ada | no | 2026-10-07 14:03 UTC | 2 | 1 |",
        "| bo | yes | none | 0 | 0 |",
        "| cy | no | 1970-01-01 00:00 UTC | 0 | 0 |",
        "| Feed (s-1) | done | yes | passed |",
        "| Share (s-2) | in progress | yes | failed |",
        "| Later (s-5) | queued | no | none |",
    ] {
        assert!(
            md.lines().any(|l| l == line),
            "{line:?} missing from:\n{md}"
        );
    }
}

#[test]
fn an_empty_project_digests_to_nothing_needing_attention() {
    let env = Env::in_repo(TestRepo::new(), Git::system());
    let digest = env.store.write_digest_file(P).unwrap();
    assert!(digest.agents.is_empty() && digest.slices.is_empty() && digest.attention.is_empty());
    let md = env.repo.read(".foundry/reviews/digest.md");
    assert!(md.contains("Nothing needs attention.") && md.contains("No slices yet."));
}

#[test]
fn utc_formats_epoch_millis() {
    assert_eq!(digest::utc(0), "1970-01-01 00:00 UTC");
    assert_eq!(digest::utc(1_791_381_780_000), "2026-10-07 14:03 UTC");
    assert_eq!(digest::utc(951_868_740_000), "2000-02-29 23:59 UTC");
}

#[test]
fn bad_ids_are_refused_before_touching_disk() {
    let env = Env::new();
    for bad in ["", "../x", "a/b", "a.json", &"x".repeat(129)] {
        assert_eq!(
            env.store.verify(P, bad).unwrap_err().code,
            ErrorCode::InvalidInput,
            "{bad}"
        );
        assert_eq!(
            env.store.get_report(P, bad).unwrap_err().code,
            ErrorCode::InvalidInput
        );
        assert_eq!(
            env.store.get_verification(P, bad).unwrap_err().code,
            ErrorCode::InvalidInput
        );
    }
    assert_eq!(
        env.store.manager_digest("nope").unwrap_err().code,
        ErrorCode::NotFound
    );
}

#[test]
fn a_report_that_isnt_one_is_invalid_json() {
    let env = Env::new();
    env.commands(&[]);
    env.repo.write(
        &format!(".foundry/reports/{S}.json"),
        r#"{ "sliceId": "s-feed" }"#,
    );
    assert_eq!(env.verify().unwrap_err().code, ErrorCode::InvalidJson);
    assert_eq!(
        env.store.get_report(P, S).unwrap_err().code,
        ErrorCode::InvalidJson
    );
}
