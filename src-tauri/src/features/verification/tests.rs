use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::*;
use crate::core::process_tree::pid_alive;
use crate::core::sessions::SessionKey;
use crate::core::writes;
use crate::testing::{fake_agent_path, TestRepo};

const P: &str = "project-1";
const S: &str = "s-feed";

// ---------- fixtures ----------

#[derive(Default)]
struct Recorded {
    progress: Mutex<Vec<VerifyProgress>>,
    /// `verify:skipped` as `(sliceId, reason)` and `verify:skip_cleared` as `(sliceId, None)`,
    /// in order.
    skips: Mutex<Vec<(String, Option<SkipReason>)>>,
}

impl VerifyEvents for Recorded {
    fn progress(&self, event: VerifyProgress) {
        self.progress.lock().unwrap().push(event);
    }

    fn skipped(&self, event: VerifySkipped) {
        assert_eq!(event.project_id, P);
        self.skips
            .lock()
            .unwrap()
            .push((event.skip.slice_id, Some(event.skip.reason)));
    }

    fn skip_cleared(&self, event: VerifySkipCleared) {
        assert_eq!(event.project_id, P);
        self.skips.lock().unwrap().push((event.slice_id, None));
    }
}

impl Recorded {
    fn stages(&self) -> Vec<VerifyStage> {
        self.progress
            .lock()
            .unwrap()
            .iter()
            .map(|e| e.stage)
            .collect()
    }

    /// The skip events since the last call.
    fn take_skips(&self) -> Vec<(String, Option<SkipReason>)> {
        std::mem::take(&mut *self.skips.lock().unwrap())
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

fn status(id: &str, status: &str) -> Value {
    json!({ "id": id, "status": status, "evidence": "" })
}

/// A report by `agent` (`ada` or `bo`, both on the team).
fn report_by(agent: &str, requirements: Value) -> Value {
    let mut value = report(requirements);
    value["agent"] = json!(agent);
    value
}

fn outcome<'a>(v: &'a Verification, id: &str) -> &'a RequirementOutcome {
    v.review
        .requirements
        .iter()
        .find(|o| o.id == id)
        .unwrap_or_else(|| panic!("no {id} in {:?}", v.review.requirements))
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
                { "id": "bo", "writeGlobs": ["web/**"], "protectedGlobs": [] },
                { "id": "cy", "writeGlobs": ["src/secret/**"], "protectedGlobs": [] }
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

    /// The legacy `<sliceId>.json`, as projects made before per-agent reports have it.
    fn write_report(&self, report: &Value) {
        self.repo
            .write(&format!(".foundry/reports/{S}.json"), &report.to_string());
    }

    /// `<sliceId>.<agent>.json`, the agent's own report.
    fn write_agent_report(&self, agent: &str, report: &Value) {
        self.repo.write(
            &format!(".foundry/reports/{S}.{agent}.json"),
            &report.to_string(),
        );
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
        .progress
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
fn workspace_access_lets_the_boundary_check_accept_exactly_the_root_files() {
    let env = Env::new();
    env.commands(&[]);
    for file in hook_policy::WORKSPACE_FILES {
        env.repo.write(file, "{}\n");
    }
    env.repo.write("turbo.json", "{}\n"); // a root file not on the list
    env.repo.write("AGENTS.md", "# rewritten\n"); // the floor
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    let flagged = |v: &Verification| -> BTreeMap<String, String> {
        v.review
            .findings
            .iter()
            .filter_map(|f| Some((f.path.clone()?, f.code.clone())))
            .filter(|(p, _)| !p.starts_with(".foundry/"))
            .collect()
    };

    // Off: every listed file is outside ada's write globs.
    let off = flagged(&env.verify().unwrap());
    for file in hook_policy::WORKSPACE_FILES {
        assert_eq!(
            off.get(*file).map(String::as_str),
            Some(code::OUTSIDE_WRITE_GLOBS),
            "{file}"
        );
    }

    // On: none of them is flagged; the rest is as before.
    let mut plan: Value = serde_json::from_str(&env.repo.read(".foundry/plan.json")).unwrap();
    plan["slices"][0]["workspaceAccess"] = json!(true);
    env.repo.write(".foundry/plan.json", &plan.to_string());
    let on = flagged(&env.verify().unwrap());
    assert_eq!(
        on,
        BTreeMap::from([
            (
                "AGENTS.md".to_string(),
                code::PROTECTED_FILE_CHANGED.to_string()
            ),
            (
                "turbo.json".to_string(),
                code::OUTSIDE_WRITE_GLOBS.to_string()
            ),
        ])
    );
}

#[test]
fn a_second_verify_right_after_the_first_still_passes() {
    let env = Env::new();
    env.commands(&[]);
    env.repo.write("src/feed.rs", "// the work\n");
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));

    let first = env.verify().unwrap();
    assert!(first.review.passed, "{:?}", first.review.findings);
    env.store.write_digest_file(P).unwrap();
    let second = env.verify().unwrap();
    assert!(second.review.passed, "{:?}", second.review.findings);

    // Committing Foundry's own output (reviews, digest, events) doesn't change that.
    env.repo.write(".foundry/events/ada.jsonl", "{}\n");
    env.repo.git(&["add", "-A"]);
    env.repo.git(&["commit", "--quiet", "-m", "slice work"]);
    let third = env.verify().unwrap();
    assert!(third.review.passed, "{:?}", third.review.findings);
    let own = |p: &str| p.starts_with(".foundry/reviews/") || p.starts_with(".foundry/events/");
    assert!(
        third
            .review
            .findings
            .iter()
            .all(|f| !f.path.as_deref().is_some_and(own)),
        "{:?}",
        third.review.findings
    );
}

#[test]
fn a_hand_edit_to_agents_md_still_fails_on_every_verify() {
    let env = Env::new();
    env.commands(&[]);
    env.repo.write("AGENTS.md", "# loosened rules\n");
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));

    for _ in 0..2 {
        let v = env.verify().unwrap();
        let hit = finding(&v, code::PROTECTED_FILE_CHANGED);
        assert_eq!(hit.path.as_deref(), Some("AGENTS.md"));
        assert!(!v.review.passed);
    }
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

    // No start commit, or one that isn't one (never passed to git as an option): refused.
    let env = Env::new();
    env.commands(&[]);
    env.repo.git(&["add", "-A"]);
    env.repo.git(&["commit", "--quiet", "-m", "setup"]);
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    for start in [None, Some("--output=x")] {
        env.plan(start);
        let err = env.verify().unwrap_err();
        assert_eq!(
            err.code,
            ErrorCode::NoStartCommit,
            "{start:?}: {}",
            err.message
        );
        assert!(err.message.contains("\"Feed\""), "{}", err.message);
    }
    assert!(!env.repo.path().join(".foundry/output=x").exists());
}

// ---------- skipped verifications ----------

/// The slice's recorded skip reason, and what `check_verify_slice` says now.
fn skip_now(env: &Env) -> (Option<SkipReason>, Option<SkipReason>) {
    (
        env.store.verify_skip(P, S).map(|s| s.reason),
        env.store.check_verify(P, S).unwrap().map(|s| s.reason),
    )
}

#[test]
fn every_skip_reason_is_produced_returned_as_its_error_and_recorded() {
    let env = Env::new();
    let refused = |code: ErrorCode, reason: SkipReason| {
        let err = env.verify().unwrap_err();
        assert_eq!(
            (err.code, err.message.is_empty()),
            (code, false),
            "{reason:?}"
        );
        let skip = env.store.verify_skip(P, S).expect("recorded");
        assert_eq!((skip.reason, skip.code), (reason, code));
        assert_eq!(skip.message, err.message, "the error's message as-is");
        assert_eq!(
            env.store.check_verify(P, S).unwrap().map(|s| s.reason),
            Some(reason)
        );
        assert_eq!(env.events.take_skips(), [(S.to_string(), Some(reason))]);
        let digest = env.store.manager_digest(P).unwrap();
        if let Some(slice) = digest.slices.first() {
            assert_eq!(slice.skip.as_ref().map(|s| s.reason), Some(reason));
        }
        assert!(env.events.stages().is_empty(), "nothing ran");
    };

    // Commands never confirmed.
    env.store
        .set_test_commands(P, vec![fake("--exit 0")])
        .unwrap();
    env.repo.git(&["add", "-A"]);
    env.repo.git(&["commit", "--quiet", "-m", "commands"]);
    env.plan(Some(&env.repo.git(&["rev-parse", "HEAD"])));
    env.events.take_skips();
    refused(
        ErrorCode::TestCommandsUnconfirmed,
        SkipReason::TestCommandsUnconfirmed,
    );
    env.commands(&[]);
    // Confirming them is the cause going away, but the next cause (no report) replaces it.
    assert_eq!(
        skip_now(&env),
        (Some(SkipReason::NoReport), Some(SkipReason::NoReport))
    );
    env.events.take_skips();

    refused(ErrorCode::NotFound, SkipReason::NoReport);
    env.repo.write(
        &format!(".foundry/reports/{S}.ada.json"),
        r#"{ "sliceId": "s-feed" }"#,
    );
    refused(ErrorCode::InvalidJson, SkipReason::ReportIncomplete);
    env.write_agent_report(
        "ada",
        &report(json!([met("r-1", "test a"), met("r-2", "b test")])),
    );

    // No start commit while the setup is committed; then while it isn't.
    let head = env.repo.git(&["rev-parse", "HEAD"]);
    env.plan(None);
    refused(ErrorCode::NoStartCommit, SkipReason::NoStartCommit);
    env.repo.write("agents/roles/ada.md", "# Ada\n");
    refused(ErrorCode::SetupNotCommitted, SkipReason::SetupNotCommitted);
    let skip = env.store.verify_skip(P, S).unwrap();
    assert!(
        skip.message.contains("agents/roles/ada.md"),
        "{}",
        skip.message
    );
    fs::remove_file(env.repo.path().join("agents/roles/ada.md")).unwrap();

    // Another verification running for the project.
    env.plan(Some(&head));
    env.events.take_skips();
    lock(&env.store.running).insert(P.to_string(), Arc::new(AtomicBool::new(false)));
    refused(
        ErrorCode::VerificationRunning,
        SkipReason::AnotherVerificationRunning,
    );
    lock(&env.store.running).remove(P);

    // Anything else: no such slice in the plan.
    env.repo.write(
        ".foundry/plan.json",
        &json!({ "version": 1, "slices": [], "requirements": [] }).to_string(),
    );
    refused(ErrorCode::NotFound, SkipReason::Other);
    assert!(env
        .store
        .verify_skip(P, S)
        .unwrap()
        .message
        .contains("no slice"));

    // A bad slice id has nothing to record against.
    assert_eq!(
        env.store.verify(P, "a/b").unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert!(env.events.take_skips().is_empty());
}

#[test]
fn a_skip_clears_when_the_slice_is_verified() {
    let env = Env::new();
    env.commands(&[]);
    env.verify().unwrap_err();
    assert_eq!(
        env.store.verify_skip(P, S).unwrap().reason,
        SkipReason::NoReport
    );
    env.events.take_skips();

    // The report arrives and auto mode (or a click) verifies: the skip is gone.
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    let v = env.verify().unwrap();
    assert!(v.review.passed, "{:?}", v.review.findings);
    assert_eq!(skip_now(&env), (None, None));
    assert_eq!(env.events.take_skips(), [(S.to_string(), None)]);
    assert_eq!(env.store.manager_digest(P).unwrap().slices[0].skip, None);

    // A verification that runs and fails still ran, so it clears one too.
    env.commands(&[fake("--exit 1")]);
    lock(&env.store.running).insert(P.to_string(), Arc::new(AtomicBool::new(false)));
    env.verify().unwrap_err();
    lock(&env.store.running).remove(P);
    env.events.take_skips();
    assert!(!env.verify().unwrap().review.passed);
    assert_eq!(env.events.take_skips(), [(S.to_string(), None)]);
    assert_eq!(env.store.verify_skip(P, S), None);
}

#[test]
fn a_skip_clears_when_its_cause_goes_away_without_a_verification() {
    let env = Env::new();
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    // The setup (plan.json too) is committed, so a missing start commit is just that.
    env.repo.git(&["add", "-A"]);
    env.repo.git(&["commit", "--quiet", "-m", "setup"]);
    env.store
        .set_test_commands(P, vec![fake("--exit 0")])
        .unwrap();
    env.events.take_skips();
    env.verify().unwrap_err();
    assert_eq!(
        env.events.take_skips(),
        [(S.to_string(), Some(SkipReason::TestCommandsUnconfirmed))]
    );

    // Confirming the commands is the cause going away: cleared at once, with the event.
    let shown = env.store.get_test_commands(P).unwrap().commands;
    env.store.confirm_test_commands(P, shown).unwrap();
    assert_eq!(env.store.verify_skip(P, S), None);
    assert_eq!(env.events.take_skips(), [(S.to_string(), None)]);
    assert!(env.events.stages().is_empty(), "clearing runs nothing");

    // Another run ending clears one that waited on it.
    lock(&env.store.running).insert(P.to_string(), Arc::new(AtomicBool::new(false)));
    env.verify().unwrap_err();
    assert_eq!(
        env.store.verify_skip(P, S).unwrap().reason,
        SkipReason::AnotherVerificationRunning
    );
    lock(&env.store.running).remove(P);
    // Read lazily: the digest looks again.
    assert_eq!(env.store.manager_digest(P).unwrap().slices[0].skip, None);
    env.events.take_skips();

    // A cause that changes is recorded again with the new reason.
    env.plan(None);
    env.verify().unwrap_err();
    fs::remove_file(env.repo.path().join(format!(".foundry/reports/{S}.json"))).unwrap();
    env.store.manager_digest(P).unwrap();
    assert_eq!(
        env.store.verify_skip(P, S).unwrap().reason,
        SkipReason::NoReport,
        "no report comes before no start commit"
    );
    assert_eq!(
        env.events.take_skips(),
        [
            (S.to_string(), Some(SkipReason::NoStartCommit)),
            (S.to_string(), Some(SkipReason::NoReport))
        ]
    );
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

// ---------- several agents on one slice ----------

#[test]
fn two_agents_reports_on_one_slice_are_both_kept_and_both_verified() {
    let env = Env::new();
    env.commands(&[]);
    let ada = report_by("ada", json!([met("r-1", "test feed_loads")]));
    let bo = report_by("bo", json!([met("r-2", "test feed_renders")]));
    env.write_agent_report("ada", &ada);
    env.write_agent_report("bo", &bo);

    // Neither overwrote the other.
    let listed = env.store.list_reports(P, S).unwrap();
    let summary: Vec<(&str, &str, &str)> = listed
        .iter()
        .map(|r| (r.path.as_str(), r.agent.as_str(), r.report.agent.as_str()))
        .collect();
    assert_eq!(
        summary,
        [
            (".foundry/reports/s-feed.ada.json", "ada", "ada"),
            (".foundry/reports/s-feed.bo.json", "bo", "bo")
        ]
    );
    assert_eq!(listed[1].report.requirements[0].id, "r-2");
    // The single-report command keeps its shape: the first of them.
    assert_eq!(env.store.get_report(P, S).unwrap().unwrap().agent, "ada");

    // Each covers half; together they cover the slice.
    let v = env.verify().unwrap();
    assert!(v.review.passed, "{:?}", v.review.findings);
    assert!(!codes(&v).contains(&code::UNREPORTED_REQUIREMENT));
    let read: Vec<(&str, &str)> = v
        .review
        .reports
        .iter()
        .map(|r| (r.path.as_str(), r.agent.as_str()))
        .collect();
    assert_eq!(
        read,
        [
            (".foundry/reports/s-feed.ada.json", "ada"),
            (".foundry/reports/s-feed.bo.json", "bo")
        ],
        "the result records which report files it read"
    );
    assert_eq!(v.review.reports[0].report, ada);
    assert_eq!(v.review.reports[1].report, bo);
    assert_eq!(v.review.report, ada, "`report` is the first of `reports`");
    assert_eq!(outcome(&v, "r-1").status, Some(RequirementStatus::Met));
    assert_eq!(outcome(&v, "r-2").reported_by[0].agent, "bo");
    // Saved as returned.
    let saved = env.store.get_verification(P, S).unwrap().unwrap();
    assert_eq!(saved.review, v.review);
    assert!(!saved.stale);
}

#[test]
fn met_in_one_report_and_partly_in_another_is_met_with_an_info_finding() {
    let env = Env::new();
    env.commands(&[]);
    env.write_agent_report(
        "ada",
        &report_by("ada", json!([met("r-1", "test a"), met("r-2", "test b")])),
    );
    env.write_agent_report("bo", &report_by("bo", json!([status("r-2", "partly")])));

    let v = env.verify().unwrap();
    let r2 = outcome(&v, "r-2");
    assert_eq!(r2.status, Some(RequirementStatus::Met));
    assert_eq!(
        r2.reported_by,
        [
            ReportedStatus {
                agent: "ada".into(),
                status: RequirementStatus::Met
            },
            ReportedStatus {
                agent: "bo".into(),
                status: RequirementStatus::Partly
            }
        ]
    );
    let note = finding(&v, code::PARTLY_REPORTED);
    assert_eq!(note.severity, FindingSeverity::Info);
    assert_eq!(note.requirement_id.as_deref(), Some("r-2"));
    assert_eq!(note.agent.as_deref(), Some("bo"));
    assert!(note.message.contains("bo"), "{}", note.message);
    assert!(v.review.passed, "{:?}", v.review.findings);
    // Partly everywhere is partly, with no note.
    env.write_agent_report(
        "ada",
        &report_by(
            "ada",
            json!([met("r-1", "test a"), status("r-2", "partly")]),
        ),
    );
    let v = env.verify().unwrap();
    assert_eq!(outcome(&v, "r-2").status, Some(RequirementStatus::Partly));
    assert!(!codes(&v).contains(&code::PARTLY_REPORTED));
}

#[test]
fn not_met_in_either_report_is_not_met() {
    for (ada_says, bo_says) in [
        ("met", "not_met"),
        ("not_met", "met"),
        ("partly", "not_met"),
    ] {
        let env = Env::new();
        env.commands(&[]);
        env.write_agent_report(
            "ada",
            &report_by(
                "ada",
                json!([met("r-1", "test a"), status("r-2", ada_says)]),
            ),
        );
        env.write_agent_report("bo", &report_by("bo", json!([status("r-2", bo_says)])));
        let v = env.verify().unwrap();
        assert_eq!(
            outcome(&v, "r-2").status,
            Some(RequirementStatus::NotMet),
            "ada {ada_says}, bo {bo_says}"
        );
        assert!(!codes(&v).contains(&code::PARTLY_REPORTED));
    }
}

#[test]
fn a_requirement_in_neither_report_is_unreported_and_an_unknown_one_names_its_agent() {
    let env = Env::new();
    env.commands(&[]);
    env.write_agent_report("ada", &report_by("ada", json!([met("r-1", "test a")])));
    env.write_agent_report(
        "bo",
        &report_by("bo", json!([met("r-1", "test a2"), met("r-7", "test x")])),
    );

    let v = env.verify().unwrap();
    let missing: Vec<&Finding> = v
        .review
        .findings
        .iter()
        .filter(|f| f.code == code::UNREPORTED_REQUIREMENT)
        .collect();
    assert_eq!(missing.len(), 1, "{missing:?}");
    assert_eq!(missing[0].requirement_id.as_deref(), Some("r-2"));
    assert_eq!(missing[0].severity, FindingSeverity::Danger);
    assert_eq!(outcome(&v, "r-2").status, None);
    let extra = finding(&v, code::UNKNOWN_REQUIREMENT);
    assert_eq!(
        (extra.requirement_id.as_deref(), extra.agent.as_deref()),
        (Some("r-7"), Some("bo"))
    );
    assert!(!v.review.passed);
}

#[test]
fn a_bad_bugs_list_is_a_warn_finding_and_the_report_still_loads() {
    let env = Env::new();
    env.commands(&[]);
    let mut qa = report_by("bo", json!([met("r-1", "test a"), met("r-2", "test b")]));
    qa["bugs"] = json!([
        { "id": "b-1", "severity": "critical", "title": "x", "requirementIds": [],
          "steps": [], "failingTests": [], "suspectedLocations": [] },
        { "id": "b-2", "severity": "low", "title": "J skips", "requirementIds": ["r-2"],
          "steps": ["press J"], "failingTests": [], "suspectedLocations": ["src/a.ts"] }
    ]);
    env.write_agent_report("bo", &qa);

    let v = env.verify().unwrap();
    let bad: Vec<&Finding> = v
        .review
        .findings
        .iter()
        .filter(|f| f.code == code::INVALID_BUG)
        .collect();
    assert_eq!(bad.len(), 2, "{bad:?}");
    assert!(bad.iter().all(|f| f.severity == FindingSeverity::Warn
        && f.agent.as_deref() == Some("bo")
        && f.path.as_deref() == Some(".foundry/reports/s-feed.bo.json")));
    assert!(bad[0].message.contains("bug 1 isn't in the bug shape"));
    assert!(bad[1].message.contains("bug b-2 names no failing test"));
    let listed = env.store.list_reports(P, S).unwrap();
    assert_eq!(
        listed[0].report.bugs.len(),
        1,
        "the well-formed bug is kept"
    );
}

#[test]
fn every_reports_blockers_and_evidence_are_checked_and_labeled_with_the_agent() {
    let env = Env::new();
    env.commands(&[]);
    let mut ada = report_by("ada", json!([met("r-1", "test a"), met("r-2", "test b")]));
    ada["blockers"] = json!(["needs an API key", "  "]);
    let mut bo = report_by("bo", json!([met("r-2", "looks fine")]));
    bo["blockers"] = json!(["the design isn't final"]);
    env.write_agent_report("ada", &ada);
    env.write_agent_report("bo", &bo);

    let v = env.verify().unwrap();
    let blockers: Vec<(Option<&str>, FindingSeverity)> = v
        .review
        .findings
        .iter()
        .filter(|f| f.code == code::REPORTED_BLOCKER)
        .map(|f| (f.agent.as_deref(), f.severity))
        .collect();
    assert_eq!(
        blockers,
        [
            (Some("ada"), FindingSeverity::Warn),
            (Some("bo"), FindingSeverity::Warn)
        ],
        "a blank blocker is none"
    );
    assert!(v
        .review
        .findings
        .iter()
        .any(|f| f.code == code::REPORTED_BLOCKER && f.message.contains("needs an API key")));
    // r-2 is met in both; only bo's evidence names nothing, so only bo's is flagged.
    let evidence: Vec<(Option<&str>, Option<&str>)> = v
        .review
        .findings
        .iter()
        .filter(|f| f.code == code::MISSING_EVIDENCE)
        .map(|f| (f.requirement_id.as_deref(), f.agent.as_deref()))
        .collect();
    assert_eq!(evidence, [(Some("r-2"), Some("bo"))]);
}

#[test]
fn a_legacy_report_is_still_read_with_its_agent_from_its_field() {
    let env = Env::new();
    env.commands(&[]);
    env.write_report(&report_by("bo", json!([met("r-1", "test a")])));
    env.write_agent_report("ada", &report_by("ada", json!([met("r-2", "test b")])));

    let listed = env.store.list_reports(P, S).unwrap();
    let agents: Vec<(&str, &str)> = listed
        .iter()
        .map(|r| (r.path.as_str(), r.agent.as_str()))
        .collect();
    assert_eq!(
        agents,
        [
            (".foundry/reports/s-feed.json", "bo"),
            (".foundry/reports/s-feed.ada.json", "ada")
        ]
    );
    let v = env.verify().unwrap();
    assert!(v.review.passed, "{:?}", v.review.findings);
    assert_eq!(outcome(&v, "r-1").reported_by[0].agent, "bo");
    assert_eq!(v.review.reports.len(), 2);
}

#[test]
fn a_change_to_any_of_the_slices_reports_makes_the_verification_stale() {
    let env = Env::new();
    env.commands(&[]);
    let ada = report_by("ada", json!([met("r-1", "test a"), met("r-2", "test b")]));
    let bo = report_by("bo", json!([met("r-2", "test c")]));
    let stale = || env.store.get_verification(P, S).unwrap().unwrap().stale;
    let reset = || {
        env.write_agent_report("ada", &ada);
        env.write_agent_report("bo", &bo);
        for extra in ["", ".zed"] {
            let _ = fs::remove_file(
                env.repo
                    .path()
                    .join(format!(".foundry/reports/{S}{extra}.json")),
            );
        }
        assert!(!env.verify().unwrap().stale);
        assert!(!stale());
    };

    for agent in ["ada", "bo"] {
        reset();
        let mut edited = if agent == "ada" {
            ada.clone()
        } else {
            bo.clone()
        };
        edited["notes"] = json!("one more thing");
        env.write_agent_report(agent, &edited);
        assert!(stale(), "{agent}'s report changed");
    }
    reset();
    fs::remove_file(
        env.repo
            .path()
            .join(format!(".foundry/reports/{S}.bo.json")),
    )
    .unwrap();
    assert!(stale(), "a report is gone");
    reset();
    env.write_report(&report(json!([met("r-1", "test a")])));
    assert!(stale(), "a new (legacy) report appeared");
    reset();
    env.repo
        .write(&format!(".foundry/reports/{S}.zed.json"), "{ not json");
    assert!(
        stale(),
        "a new report appeared, even one that doesn't parse"
    );
    // Another slice's report, or a non-report file, is no change.
    reset();
    env.repo
        .write(".foundry/reports/s-other.ada.json", &ada.to_string());
    env.repo
        .write(&format!(".foundry/reports/{S}.notes.md"), "x");
    assert!(!stale());
}

#[test]
fn a_review_saved_before_per_agent_reports_still_reads_and_compares_to_the_legacy_file() {
    let env = Env::new();
    let legacy = report(json!([met("r-1", "test a"), met("r-2", "test b")]));
    env.write_report(&legacy);
    env.repo.write(
        &format!(".foundry/reviews/{S}.json"),
        &json!({
            "sliceId": S, "passed": true, "testRuns": [], "report": legacy, "verifiedAt": 1,
            "findings": [{ "severity": "warn", "code": "no_test_command", "message": "m",
                           "requirementId": null, "path": null }]
        })
        .to_string(),
    );
    let v = env.store.get_verification(P, S).unwrap().unwrap();
    assert!(!v.stale);
    assert!(v.review.reports.is_empty() && v.review.requirements.is_empty());
    assert_eq!(v.review.findings[0].agent, None);
    assert_eq!(
        env.store.manager_digest(P).unwrap().slices[0].verification,
        VerificationState::Passed
    );
    // A per-agent report added since is a change.
    env.write_agent_report("ada", &legacy);
    assert!(env.store.get_verification(P, S).unwrap().unwrap().stale);
    assert_eq!(
        env.store.manager_digest(P).unwrap().slices[0].verification,
        VerificationState::Stale
    );
}

#[test]
fn the_boundary_check_allows_a_file_any_reporting_agent_may_write() {
    let env = Env::new();
    env.commands(&[]);
    env.repo.write(
        "src/feed.rs",
        "// ada's
",
    );
    env.repo.write("web/page.html", "<p>bo's</p>");
    env.repo.write("docs/notes.md", "nobody's");
    env.write_agent_report("ada", &report_by("ada", json!([met("r-1", "test a")])));
    env.write_agent_report("bo", &report_by("bo", json!([met("r-2", "test b")])));

    let v = env.verify().unwrap();
    let by_path: BTreeMap<&str, &str> = v
        .review
        .findings
        .iter()
        .filter_map(|f| Some((f.path.as_deref()?, f.code.as_str())))
        .filter(|(p, _)| !p.starts_with(".foundry/"))
        .collect();
    assert_eq!(
        by_path,
        BTreeMap::from([("docs/notes.md", code::OUTSIDE_WRITE_GLOBS)])
    );
    let outside = finding(&v, code::OUTSIDE_WRITE_GLOBS);
    assert!(
        outside.message.contains("ada and bo"),
        "{}",
        outside.message
    );
}

#[test]
fn a_digest_counts_any_agents_report_as_present() {
    let env = Env::new();
    env.write_agent_report("bo", &report_by("bo", json!([])));
    let slice = &env.store.manager_digest(P).unwrap().slices[0];
    assert!(slice.report_present);
    assert_eq!(slice.verification, VerificationState::None);
}

#[test]
fn a_digest_counts_each_slices_report_files() {
    let env = Env::new();
    let report_count = |env: &Env| env.store.manager_digest(P).unwrap().slices[0].report_count;
    assert_eq!(report_count(&env), 0);
    assert!(!env.store.manager_digest(P).unwrap().slices[0].report_present);

    env.write_agent_report("ada", &report_by("ada", json!([])));
    assert_eq!(report_count(&env), 1);

    env.write_agent_report("bo", &report_by("bo", json!([])));
    assert_eq!(report_count(&env), 2);

    // The legacy `<sliceId>.json` counts as one more.
    env.write_report(&report_by("cy", json!([])));
    let slice = &env.store.manager_digest(P).unwrap().slices[0];
    assert_eq!(slice.report_count, 3);
    assert!(slice.report_present);
}

#[test]
fn listed_reports_carry_their_files_write_time_legacy_ones_too() {
    use crate::core::time;
    let env = Env::new();
    env.write_report(&report_by("bo", json!([met("r-1", "test a")])));
    env.write_agent_report("ada", &report_by("ada", json!([met("r-2", "test b")])));
    // A legacy report from long ago: its time is the file's modified time.
    let legacy = env.repo.path().join(".foundry/reports/s-feed.json");
    let old = std::time::UNIX_EPOCH + Duration::from_millis(1_791_381_785_123);
    fs::File::options()
        .write(true)
        .open(&legacy)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let fresh = env.repo.path().join(".foundry/reports/s-feed.ada.json");
    let fresh_ms = time::epoch_ms(fs::metadata(&fresh).unwrap().modified().unwrap());

    let listed = env.store.list_reports(P, S).unwrap();
    let times: Vec<(&str, &str)> = listed
        .iter()
        .map(|r| (r.path.as_str(), r.written_at.as_str()))
        .collect();
    assert_eq!(
        times,
        [
            (".foundry/reports/s-feed.json", "2026-10-07T14:03:05.123Z"),
            (
                ".foundry/reports/s-feed.ada.json",
                time::iso8601(fresh_ms).as_str()
            )
        ]
    );
    let now_ms = time::epoch_ms(std::time::SystemTime::now());
    assert!(
        now_ms.abs_diff(fresh_ms) < 600_000,
        "a new report is recent"
    );
    // It's on the wire as `writtenAt`.
    let wire = serde_json::to_value(&listed[0]).unwrap();
    assert_eq!(wire["writtenAt"], json!("2026-10-07T14:03:05.123Z"));
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

// ---------- project rules ----------

/// Adds a project-wide constraint `r-pw` (no slice) to the plan.
fn add_project_rule(env: &Env) {
    let mut plan: Value = serde_json::from_str(&env.repo.read(".foundry/plan.json")).unwrap();
    plan["requirements"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "r-pw", "sliceId": null, "projectWide": true }));
    env.repo.write(".foundry/plan.json", &plan.to_string());
}

#[test]
fn verification_neither_requires_nor_checks_project_rules() {
    // Not reported: not required.
    let env = Env::new();
    env.commands(&[]);
    add_project_rule(&env);
    env.write_agent_report(
        "ada",
        &report_by(
            "ada",
            json!([met("r-1", "cargo test"), met("r-2", "cargo test")]),
        ),
    );
    let v = env.verify().unwrap();
    assert!(v.review.passed, "{:?}", v.review.findings);
    assert!(!codes(&v).contains(&code::UNREPORTED_REQUIREMENT));
    assert!(v.review.requirements.iter().all(|o| o.id != "r-pw"));

    // Reported anyway (partly, no evidence): ignored, no finding, not in the outcomes.
    let env = Env::new();
    env.commands(&[]);
    add_project_rule(&env);
    let report = report_by(
        "ada",
        json!([
            met("r-1", "cargo test"),
            met("r-2", "cargo test"),
            status("r-pw", "partly"),
        ]),
    );
    env.write_agent_report("ada", &report);
    let v = env.verify().unwrap();
    assert!(v.review.passed, "{:?}", v.review.findings);
    assert!(v
        .review
        .findings
        .iter()
        .all(|f| f.requirement_id.as_deref() != Some("r-pw")));
    let ids: Vec<&str> = v
        .review
        .requirements
        .iter()
        .map(|o| o.id.as_str())
        .collect();
    assert_eq!(ids, ["r-1", "r-2"]);
    // The stored report is kept exactly as written.
    assert_eq!(v.review.reports[0].report, report);
}

// ---------- attribution: the writes ledger ----------

const OTHER: &str = "s-other";

impl Env {
    /// One ledger entry.
    fn ledger(&self, kind: writes::EntryKind, session: &str, slice: &str, agent: &str, ts: u64) {
        self.ledger_path(kind, session, slice, agent, None, ts);
    }

    fn ledger_path(
        &self,
        kind: writes::EntryKind,
        session: &str,
        slice: &str,
        agent: &str,
        path: Option<&str>,
        ts: u64,
    ) {
        writes::record(
            self.repo.path(),
            &writes::LedgerEntry {
                kind,
                session_id: session.into(),
                slice_id: Some(slice.into()),
                agent_id: agent.into(),
                path: path.map(str::to_string),
                ts,
            },
        )
        .unwrap();
    }

    /// A session from `from` to `to` (start and end entries) whose edit tools wrote `paths`
    /// (to disk too) in between.
    fn session(&self, id: &str, slice: &str, agent: &str, from: u64, to: u64, paths: &[&str]) {
        use writes::EntryKind::{End, Start, Write};
        self.ledger(Start, id, slice, agent, from);
        for path in paths {
            self.repo.write(path, &format!("// {agent} on {slice}\n"));
            self.ledger_path(Write, id, slice, agent, Some(path), from + 1);
        }
        self.ledger(End, id, slice, agent, to);
    }

    /// Takes and saves the session's snapshot now.
    fn snap(&self, session: &str, moment: writes::Moment, ts: u64) {
        let snapshot = writes::take_snapshot(&Git::system(), self.repo.path(), ts).unwrap();
        writes::save_snapshot(self.repo.path(), session, moment, &snapshot).unwrap();
    }

    /// The plan with both `s-feed` and `s-other` in progress from `start`.
    fn plan_two(&self, start: &str) {
        let slice = |id: &str, name: &str| {
            json!({
                "id": id, "name": name, "delivers": [], "dependsOn": [],
                "status": "in_progress", "decisions": [], "openQuestions": [],
                "startCommit": start
            })
        };
        self.repo.write(
            ".foundry/plan.json",
            &json!({
                "version": 1, "revision": 3,
                "slices": [slice(S, "Feed"), slice(OTHER, "Other")],
                "requirements": [
                    { "id": "r-1", "sliceId": S },
                    { "id": "r-2", "sliceId": S },
                    { "id": "r-9", "sliceId": OTHER }
                ]
            })
            .to_string(),
        );
    }

    /// Ada's report for `s-feed`, everything met.
    fn ada_reports(&self) {
        self.write_agent_report(
            "ada",
            &report_by("ada", json!([met("r-1", "test a"), met("r-2", "test b")])),
        );
    }
}

/// Boundary findings (those with a path, Foundry's own files left out) by path.
fn boundary_by_path(v: &Verification) -> BTreeMap<&str, (&str, FindingSeverity)> {
    v.review
        .findings
        .iter()
        .filter_map(|f| Some((f.path.as_deref()?, (f.code.as_str(), f.severity))))
        .filter(|(p, _)| !p.starts_with(".foundry/"))
        .collect()
}

#[test]
fn two_concurrent_slices_writing_different_folders_give_no_false_protected_finding() {
    let env = Env::new();
    env.commands(&[]);
    // Both slices start from the same commit; cy's folder is protected for ada.
    env.session("sess-ada", S, "ada", 100, 500, &["src/feed.rs"]);
    env.session("sess-cy", OTHER, "cy", 200, 600, &["src/secret/key.rs"]);
    env.ada_reports();

    let v = env.verify().unwrap();
    assert_eq!(
        boundary_by_path(&v),
        BTreeMap::from([(
            "src/secret/key.rs",
            (code::OWNED_BY_OTHER_SLICE, FindingSeverity::Info)
        )])
    );
    let owned = finding(&v, code::OWNED_BY_OTHER_SLICE);
    assert!(
        owned.message.contains("cy on slice s-other"),
        "{}",
        owned.message
    );
    assert!(v.review.passed, "{:?}", v.review.findings);
}

#[test]
fn the_same_file_written_by_two_overlapping_slices_is_shared_file_touched_on_both() {
    let env = Env::new();
    env.commands(&[]);
    env.plan_two(&env.repo.git(&["rev-parse", "HEAD"]));
    env.session("sess-ada", S, "ada", 100, 500, &["src/feed.rs"]);
    env.session("sess-cy", OTHER, "cy", 200, 600, &["src/feed.rs"]);
    env.ada_reports();
    env.repo.write(
        &format!(".foundry/reports/{OTHER}.cy.json"),
        &json!({
            "sliceId": OTHER, "agent": "cy",
            "requirements": [met("r-9", "test c")],
            "testsRun": [], "blockers": [], "notes": ""
        })
        .to_string(),
    );

    let feed = env.verify().unwrap();
    let shared = finding(&feed, code::SHARED_FILE_TOUCHED);
    assert_eq!(
        (shared.severity, shared.path.as_deref()),
        (FindingSeverity::Warn, Some("src/feed.rs"))
    );
    assert!(
        shared.message.contains("cy on slice s-other"),
        "{}",
        shared.message
    );

    let other = env.store.verify(P, OTHER).unwrap();
    let shared = finding(&other, code::SHARED_FILE_TOUCHED);
    assert_eq!(
        (shared.severity, shared.path.as_deref()),
        (FindingSeverity::Warn, Some("src/feed.rs"))
    );
    assert!(
        shared.message.contains("ada on slice s-feed"),
        "{}",
        shared.message
    );

    // One after the other, not overlapping: no warning.
    let env = Env::new();
    env.commands(&[]);
    env.session("sess-cy", OTHER, "cy", 100, 200, &["src/feed.rs"]);
    env.session("sess-ada", S, "ada", 300, 400, &["src/feed.rs"]);
    env.ada_reports();
    assert!(!codes(&env.verify().unwrap()).contains(&code::SHARED_FILE_TOUCHED));
}

#[test]
fn a_shell_write_is_attributed_to_the_session_by_its_snapshots() {
    use writes::EntryKind::{Snapshot, Start};
    let env = Env::new();
    env.commands(&[]);
    env.ledger(Start, "sess-ada", S, "ada", 100);
    env.snap("sess-ada", writes::Moment::Start, 100);
    // No edit tool: a shell command wrote these.
    env.repo.write("src/gen.rs", "// generated\n");
    env.repo.write("web/gen.html", "<p>generated</p>");
    env.snap("sess-ada", writes::Moment::End, 300);
    env.ledger(Snapshot, "sess-ada", S, "ada", 300);
    // After the session's end snapshot: not its change, and nobody's.
    env.repo.write("docs/human.md", "the human's\n");
    env.ada_reports();

    let v = env.verify().unwrap();
    assert_eq!(
        boundary_by_path(&v),
        BTreeMap::from([(
            "web/gen.html",
            (code::OUTSIDE_WRITE_GLOBS, FindingSeverity::Warn)
        )])
    );
    let outside = finding(&v, code::OUTSIDE_WRITE_GLOBS);
    assert!(
        outside.message.starts_with("ada wrote web/gen.html"),
        "{}",
        outside.message
    );
}

#[test]
fn two_sequential_sessions_in_one_slice_are_both_counted() {
    let env = Env::new();
    env.commands(&[]);
    env.session("sess-1", S, "ada", 100, 200, &["src/secret/key.rs"]);
    env.session("sess-2", S, "bo", 300, 400, &["docs/notes.md"]);
    env.ada_reports();

    let v = env.verify().unwrap();
    assert_eq!(
        boundary_by_path(&v),
        BTreeMap::from([
            (
                "src/secret/key.rs",
                (code::PROTECTED_FILE_CHANGED, FindingSeverity::Danger)
            ),
            (
                "docs/notes.md",
                (code::OUTSIDE_WRITE_GLOBS, FindingSeverity::Warn)
            ),
        ])
    );
    let outside = finding(&v, code::OUTSIDE_WRITE_GLOBS);
    assert!(
        outside.message.starts_with("bo wrote"),
        "{}",
        outside.message
    );
}

#[test]
fn a_shell_change_while_another_slice_ran_is_ambiguous_info() {
    use writes::EntryKind::{End, Snapshot, Start};
    let env = Env::new();
    env.commands(&[]);
    env.ledger(Start, "sess-ada", S, "ada", 100);
    env.snap("sess-ada", writes::Moment::Start, 100);
    env.ledger(Start, "sess-bo", OTHER, "bo", 200);
    env.repo.write("docs/gen.md", "whose?\n");
    env.ledger(End, "sess-bo", OTHER, "bo", 300);
    env.snap("sess-ada", writes::Moment::End, 400);
    env.ledger(Snapshot, "sess-ada", S, "ada", 400);
    env.ada_reports();

    let v = env.verify().unwrap();
    assert_eq!(
        boundary_by_path(&v),
        BTreeMap::from([(
            "docs/gen.md",
            (code::AMBIGUOUS_SHELL_CHANGE, FindingSeverity::Info)
        )])
    );
    let ambiguous = finding(&v, code::AMBIGUOUS_SHELL_CHANGE);
    assert!(
        ambiguous.message.contains("bo on slice s-other"),
        "{}",
        ambiguous.message
    );
    assert!(!ambiguous.message.contains('\n'));
    assert!(v.review.passed);
}

#[test]
fn a_real_protected_edit_is_still_danger_for_the_session_that_made_it() {
    let env = Env::new();
    env.commands(&[]);
    env.session(
        "sess-ada",
        S,
        "ada",
        100,
        500,
        &["src/feed.rs", "src/secret/key.rs", "AGENTS.md"],
    );
    env.session("sess-bo", OTHER, "bo", 200, 600, &["web/page.html"]);
    env.ada_reports();

    let v = env.verify().unwrap();
    assert_eq!(
        boundary_by_path(&v),
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
                (code::OWNED_BY_OTHER_SLICE, FindingSeverity::Info)
            ),
        ])
    );
    assert!(finding(&v, code::PROTECTED_FILE_CHANGED)
        .message
        .contains("protected for ada"));
    assert!(!v.review.passed);
}

#[test]
fn without_ledger_sessions_the_slice_falls_back_to_the_diff() {
    let env = Env::new();
    env.commands(&[]);
    // Only another slice is in the ledger; this slice's run predates it.
    env.session("sess-cy", OTHER, "cy", 100, 200, &["src/secret/key.rs"]);
    env.repo.write("src/feed.rs", "// ada's, unrecorded\n");
    env.repo.write("AGENTS.md", "# rewritten, unrecorded\n");
    env.repo.write("docs/notes.md", "nobody's\n");
    env.ada_reports();

    let v = env.verify().unwrap();
    assert_eq!(
        boundary_by_path(&v),
        BTreeMap::from([
            (
                "AGENTS.md",
                (code::PROTECTED_FILE_CHANGED, FindingSeverity::Danger)
            ),
            (
                "docs/notes.md",
                (code::OUTSIDE_WRITE_GLOBS, FindingSeverity::Warn)
            ),
            (
                "src/secret/key.rs",
                (code::OWNED_BY_OTHER_SLICE, FindingSeverity::Info)
            ),
        ])
    );
}

#[test]
fn another_slices_change_after_verifying_doesnt_make_the_verification_stale() {
    let env = Env::new();
    env.commands(&[]);
    env.session("sess-ada", S, "ada", 100, 200, &["src/feed.rs"]);
    env.ada_reports();
    let v = env.verify().unwrap();
    assert!(!v.stale);

    env.session(
        "sess-cy",
        OTHER,
        "cy",
        300,
        400,
        &["src/secret/key.rs", "src/other.rs"],
    );
    let now = env.store.get_verification(P, S).unwrap().unwrap();
    assert!(!now.stale);
}

// ---------- guidance ----------

mod guidance_rules {
    use super::*;
    use crate::features::verification::guidance::{self, Inputs, PathFacts};

    fn mk(code: &str, severity: FindingSeverity) -> Finding {
        Finding {
            severity,
            code: code.into(),
            message: format!("{code} happened"),
            requirement_id: None,
            path: None,
            agent: None,
        }
    }

    fn at(mut f: Finding, path: &str) -> Finding {
        f.path = Some(path.into());
        f
    }

    fn review(passed: bool, findings: Vec<Finding>) -> Review {
        Review {
            slice_id: S.into(),
            passed,
            findings,
            test_runs: Vec::new(),
            reports: Vec::new(),
            requirements: Vec::new(),
            check_results: Vec::new(),
            report: Value::Null,
            verified_at: 1,
        }
    }

    fn inputs(review: Review) -> Inputs {
        Inputs {
            slice_id: S.into(),
            review: Some(review),
            on_slice: vec!["ada".into()],
            reported: ["ada".to_string()].into(),
            names: [("ada".to_string(), "Ada".to_string())].into(),
            ..Inputs::default()
        }
    }

    fn failed_run(command: &str, tail: &str) -> CommandRun {
        CommandRun {
            command: command.into(),
            exit_code: Some(1),
            duration_ms: 10,
            tail: tail.into(),
            timed_out: false,
        }
    }

    fn test_failure(tail: &str) -> Inputs {
        let mut r = review(
            false,
            vec![Finding {
                message: "`pnpm test` failed (exit code 1)".into(),
                ..mk(code::TEST_FAILED, FindingSeverity::Danger)
            }],
        );
        r.test_runs.push(failed_run("pnpm test", tail));
        inputs(r)
    }

    fn kinds(g: &VerificationGuidance) -> Vec<GuidanceActionKind> {
        g.actions.iter().map(|a| a.kind).collect()
    }

    #[test]
    fn every_finding_code_has_a_rule() {
        let source = include_str!("mod.rs");
        let block = source
            .split("pub mod code {")
            .nth(1)
            .and_then(|rest| rest.split("\n}").next())
            .expect("the code module");
        let codes: Vec<&str> = block
            .lines()
            .filter(|l| l.trim_start().starts_with("pub const"))
            .filter_map(|l| l.split('"').nth(1))
            .collect();
        assert_eq!(codes.len(), 19, "{codes:?}");
        for c in codes {
            assert!(
                guidance::has_rule(c),
                "finding code {c} has no guidance rule"
            );
        }
    }

    #[test]
    fn test_failed_sends_the_command_and_its_last_40_lines_to_the_slice_agent() {
        let tail: String = (1..=60).map(|n| format!("line {n}\n")).collect();
        let g = guidance::build(&test_failure(&tail));
        assert_eq!(g.headline.kind, HeadlineKind::NeedsFix);
        let f = &g.findings[0];
        assert_eq!(f.group, FindingGroup::Fix);
        assert_eq!(f.plain, "`pnpm test` failed (exit code 1).");
        let output = f.output.as_deref().unwrap();
        assert_eq!(output.lines().count(), 40);
        assert!(output.starts_with("line 21\n") && output.ends_with("line 60"));
        let send = &g.actions[0];
        assert_eq!(
            (send.kind, send.agent_id.as_deref()),
            (GuidanceActionKind::SendToAgent, Some("ada"))
        );
        let message = send.message.as_deref().unwrap();
        assert!(message.contains("`pnpm test` failed"), "{message}");
        assert!(message.contains("line 60") && !message.contains("line 20\n"));
        assert!(message
            .ends_with("Fix the code, don't weaken tests, run the command, report the result."));
        assert_eq!(f.action_ids, [send.id.clone()]);
        assert_eq!(kinds(&g).last(), Some(&GuidanceActionKind::Reverify));
    }

    #[test]
    fn eperm_on_the_prisma_engine_says_stop_the_dev_server_then_reverify() {
        let tail = "Error: EPERM: operation not permitted, rename \
                    'node_modules/.prisma/client/query_engine-windows.dll.node.tmp123'\n";
        let g = guidance::build(&test_failure(tail));
        assert_eq!(
            kinds(&g),
            [
                GuidanceActionKind::StopDevServer,
                GuidanceActionKind::Reverify
            ]
        );
        assert!(g.findings[0]
            .what_to_do
            .contains("Stop the dev server and Prisma Studio"));
        // EBUSY on a .node file too; EPERM on something else is the code's problem.
        let busy = guidance::build(&test_failure(
            "EBUSY: resource busy or locked, open 'x/sharp.node'",
        ));
        assert_eq!(busy.actions[0].kind, GuidanceActionKind::StopDevServer);
        let other = guidance::build(&test_failure(
            "EPERM: operation not permitted, open 'a.txt'",
        ));
        assert_eq!(other.actions[0].kind, GuidanceActionKind::SendToAgent);
    }

    #[test]
    fn eaddrinuse_says_a_port_is_in_use() {
        let g = guidance::build(&test_failure(
            "Error: listen EADDRINUSE: address already in use :::3000",
        ));
        assert_eq!(
            kinds(&g),
            [GuidanceActionKind::Manual, GuidanceActionKind::Reverify]
        );
        assert!(g.actions[0]
            .message
            .as_deref()
            .unwrap()
            .starts_with("A port is in use: stop whatever is using it, or change the port"));
    }

    #[test]
    fn a_timed_out_test_follows_the_test_failed_flow() {
        let mut i = test_failure("still running...");
        let r = i.review.as_mut().unwrap();
        r.findings[0].code = code::TEST_TIMED_OUT.into();
        r.test_runs[0].timed_out = true;
        r.test_runs[0].exit_code = None;
        let g = guidance::build(&i);
        assert_eq!(g.findings[0].plain, "`pnpm test` didn't finish in time.");
        assert_eq!(g.actions[0].kind, GuidanceActionKind::SendToAgent);
    }

    #[test]
    fn a_change_to_rules_design_claude_or_the_plan_is_revert_and_tell_the_agent() {
        for path in [
            "AGENTS.md",
            "agents/roles.json",
            "docs/design/a.png",
            ".claude/x.json",
            ".foundry/plan.json",
        ] {
            let g = guidance::build(&inputs(review(
                false,
                vec![at(
                    mk(code::PROTECTED_FILE_CHANGED, FindingSeverity::Danger),
                    path,
                )],
            )));
            assert_eq!(g.headline.kind, HeadlineKind::NeedsFix, "{path}");
            assert!(g.findings[0].what_to_do.starts_with("Revert it"), "{path}");
            let send = &g.actions[0];
            assert_eq!(send.kind, GuidanceActionKind::SendToAgent);
            assert!(send.message.as_deref().unwrap().contains(path));
        }
    }

    #[test]
    fn a_file_of_another_slice_is_safe_to_override_with_a_reason_template() {
        let mut i = inputs(review(
            false,
            vec![at(
                mk(code::PROTECTED_FILE_CHANGED, FindingSeverity::Danger),
                "packages/server/a.ts",
            )],
        ));
        i.paths.insert(
            "packages/server/a.ts".into(),
            PathFacts {
                elsewhere: Some("lisa on slice s-api".into()),
                ..PathFacts::default()
            },
        );
        let g = guidance::build(&i);
        assert_eq!(g.headline.kind, HeadlineKind::SafeToOverride);
        assert_eq!(g.findings[0].group, FindingGroup::Override);
        assert_eq!(
            kinds(&g),
            [GuidanceActionKind::Override, GuidanceActionKind::Commit]
        );
        assert_eq!(
            g.actions[0].reason_template.as_deref(),
            Some(
                "packages/server/a.ts was changed by lisa on slice s-api, not this slice; <reason>"
            )
        );
        assert_eq!(g.findings[0].action_ids, ["override"]);
    }

    #[test]
    fn one_small_out_of_scope_edit_gets_a_reason_template_naming_the_file() {
        let outside = |n| {
            let mut i = inputs(review(
                true,
                vec![at(
                    mk(code::OUTSIDE_WRITE_GLOBS, FindingSeverity::Warn),
                    "turbo.json",
                )],
            ));
            i.paths.insert(
                "turbo.json".into(),
                PathFacts {
                    changed_lines: Some(n),
                    ..PathFacts::default()
                },
            );
            guidance::build(&i)
        };
        let small = outside(3);
        assert_eq!(small.headline.kind, HeadlineKind::SafeToOverride);
        assert_eq!(
            small.actions[0].reason_template.as_deref(),
            Some("Edit to turbo.json needed for this slice; <reason>")
        );
        let big = outside(200);
        assert_eq!(big.headline.kind, HeadlineKind::NeedsDecision);
        assert_eq!(big.findings[0].group, FindingGroup::Decide);
    }

    #[test]
    fn shared_file_touched_needs_a_decision_to_check_the_merged_result() {
        let g = guidance::build(&inputs(review(
            true,
            vec![at(
                mk(code::SHARED_FILE_TOUCHED, FindingSeverity::Warn),
                "src/app.ts",
            )],
        )));
        assert_eq!(g.headline.kind, HeadlineKind::NeedsDecision);
        assert_eq!(
            g.findings[0].plain,
            "Both slices changed src/app.ts: open it and check the merged result."
        );
        assert_eq!(
            kinds(&g),
            [GuidanceActionKind::Manual, GuidanceActionKind::Reverify]
        );
    }

    #[test]
    fn missing_evidence_doesnt_block_and_asks_the_agent_to_update_its_report() {
        let mut f = mk(code::MISSING_EVIDENCE, FindingSeverity::Warn);
        f.requirement_id = Some("r-1".into());
        f.agent = Some("ada".into());
        let g = guidance::build(&inputs(review(true, vec![f])));
        assert_eq!(g.headline.kind, HeadlineKind::Passed);
        assert_eq!(g.findings[0].group, FindingGroup::FollowUp);
        assert_eq!(
            kinds(&g),
            [
                GuidanceActionKind::MarkDone,
                GuidanceActionKind::Commit,
                GuidanceActionKind::SendToAgent
            ]
        );
        assert_eq!(
            g.actions[2].message.as_deref(),
            Some(
                "Update your report for requirement r-1 to name the test and command that \
                 cover it. Don't change any code."
            )
        );
    }

    #[test]
    fn a_not_met_or_partly_requirement_goes_back_with_its_text_and_what_the_report_says() {
        let mut r = review(true, Vec::new());
        r.reports.push(VerifiedReport {
            path: format!(".foundry/reports/{S}.ada.json"),
            agent: "ada".into(),
            report: json!({ "requirements": [
                { "id": "r-1", "status": "not_met", "evidence": "the API isn't there yet" }
            ]}),
        });
        for (id, status) in [
            ("r-1", RequirementStatus::NotMet),
            ("r-2", RequirementStatus::Partly),
        ] {
            r.requirements.push(RequirementOutcome {
                id: id.into(),
                status: Some(status),
                reported_by: vec![ReportedStatus {
                    agent: "ada".into(),
                    status,
                }],
            });
        }
        let mut i = inputs(r);
        i.requirement_text
            .insert("r-1".into(), "Users can log in".into());
        let g = guidance::build(&i);
        assert_eq!(g.headline.kind, HeadlineKind::NeedsFix);
        let ids: Vec<&str> = g.findings.iter().map(|f| f.finding_id.as_str()).collect();
        assert_eq!(ids, ["r:r-1", "r:r-2"]);
        assert_eq!(
            g.findings[0].plain,
            "Requirement r-1 (\"Users can log in\") is not met."
        );
        let message = g.actions[0].message.as_deref().unwrap();
        assert_eq!(g.actions[0].agent_id.as_deref(), Some("ada"));
        assert!(message.contains("(\"Users can log in\")"), "{message}");
        assert!(message.contains("Your report says not met: the API isn't there yet"));
        assert!(g.actions[1]
            .message
            .as_deref()
            .unwrap()
            .contains("Your report says partly."));
    }

    #[test]
    fn foundry_file_changed_is_ignored() {
        let g = guidance::build(&inputs(review(
            true,
            vec![at(
                mk(code::FOUNDRY_FILE_CHANGED, FindingSeverity::Info),
                "vsm_data.js",
            )],
        )));
        assert_eq!(g.findings[0].group, FindingGroup::Ignore);
        assert_eq!(g.findings[0].plain, "Foundry rewrites this file itself.");
        assert_eq!(g.headline.kind, HeadlineKind::Passed);
    }

    #[test]
    fn the_codes_without_their_own_spec_rule_get_the_agreed_defaults() {
        let group = |code: &str, severity| {
            guidance::build(&inputs(review(false, vec![mk(code, severity)]))).findings[0].group
        };
        use FindingSeverity::{Danger, Info, Warn};
        assert_eq!(
            group(code::UNREPORTED_REQUIREMENT, Danger),
            FindingGroup::Fix
        );
        assert_eq!(group(code::CONTRADICTION, Danger), FindingGroup::Fix);
        assert_eq!(
            group(code::UNKNOWN_REQUIREMENT, Warn),
            FindingGroup::FollowUp
        );
        assert_eq!(group(code::SLICE_MISMATCH, Warn), FindingGroup::FollowUp);
        assert_eq!(group(code::NO_TEST_COMMAND, Warn), FindingGroup::FollowUp);
        assert_eq!(group(code::BOUNDARY_SKIPPED, Warn), FindingGroup::FollowUp);
        assert_eq!(group(code::REPORTED_BLOCKER, Warn), FindingGroup::Decide);
        assert_eq!(group(code::INVALID_BUG, Warn), FindingGroup::FollowUp);
        assert_eq!(group(code::PARTLY_REPORTED, Info), FindingGroup::Info);
        assert_eq!(
            group(code::AMBIGUOUS_SHELL_CHANGE, Info),
            FindingGroup::Info
        );
        assert_eq!(group(code::OWNED_BY_OTHER_SLICE, Info), FindingGroup::Info);
    }

    #[test]
    fn a_fix_and_an_override_together_need_a_fix_first() {
        let mut i = test_failure("assertion failed");
        let r = i.review.as_mut().unwrap();
        r.findings.push(at(
            mk(code::OUTSIDE_WRITE_GLOBS, FindingSeverity::Warn),
            "turbo.json",
        ));
        i.paths.insert(
            "turbo.json".into(),
            PathFacts {
                changed_lines: Some(2),
                ..PathFacts::default()
            },
        );
        let g = guidance::build(&i);
        assert_eq!(g.headline.kind, HeadlineKind::NeedsFix);
        let groups: Vec<FindingGroup> = g.findings.iter().map(|f| f.group).collect();
        assert_eq!(groups, [FindingGroup::Fix, FindingGroup::Override]);
        assert_eq!(
            kinds(&g),
            [
                GuidanceActionKind::SendToAgent,
                GuidanceActionKind::Reverify
            ],
            "no override while something needs a fix"
        );
    }

    #[test]
    fn a_failed_review_with_only_info_findings_is_passed_with_a_note() {
        let g = guidance::build(&inputs(review(
            false,
            vec![mk(code::PARTLY_REPORTED, FindingSeverity::Info)],
        )));
        assert_eq!(g.headline.kind, HeadlineKind::Passed);
        assert!(g
            .headline
            .text
            .starts_with("Passed (the saved result says failed"));
        assert_eq!(
            kinds(&g),
            [GuidanceActionKind::MarkDone, GuidanceActionKind::Commit]
        );
    }

    #[test]
    fn a_setup_skip_offers_commit_setup() {
        let mut i = inputs(review(true, Vec::new()));
        i.skip = Some(VerifySkip {
            slice_id: S.into(),
            reason: SkipReason::SetupNotCommitted,
            code: ErrorCode::SetupNotCommitted,
            message: "\"Feed\" has no start commit, and Foundry's setup isn't committed.".into(),
            at: 1,
        });
        let g = guidance::build(&i);
        assert_eq!(g.headline.kind, HeadlineKind::Skipped);
        assert_eq!(g.headline.text, i.skip.as_ref().unwrap().message);
        assert_eq!(g.actions[0].kind, GuidanceActionKind::CommitSetup);
    }

    #[test]
    fn headline_priority_is_waiting_skipped_stale_then_fix_decision_override_passed() {
        let all = || {
            let mut i = inputs(review(
                false,
                vec![
                    at(mk(code::SHARED_FILE_TOUCHED, FindingSeverity::Warn), "a.ts"),
                    at(mk(code::OUTSIDE_WRITE_GLOBS, FindingSeverity::Warn), "b.ts"),
                    mk(code::UNREPORTED_REQUIREMENT, FindingSeverity::Danger),
                ],
            ));
            i.paths.insert(
                "b.ts".into(),
                PathFacts {
                    elsewhere: Some("bo on slice s-web".into()),
                    ..PathFacts::default()
                },
            );
            i.on_slice.push("bo".into());
            i.stale = true;
            i.skip = Some(VerifySkip {
                slice_id: S.into(),
                reason: SkipReason::TestCommandsUnconfirmed,
                code: ErrorCode::TestCommandsUnconfirmed,
                message: "confirm them".into(),
                at: 1,
            });
            i
        };
        let kind = |i: &Inputs| guidance::build(i).headline.kind;
        let mut i = all();
        assert_eq!(kind(&i), HeadlineKind::Waiting);
        i.on_slice.pop();
        assert_eq!(kind(&i), HeadlineKind::Skipped);
        i.skip = None;
        assert_eq!(kind(&i), HeadlineKind::Stale);
        i.stale = false;
        assert_eq!(kind(&i), HeadlineKind::NeedsFix);
        i.review.as_mut().unwrap().findings.pop();
        assert_eq!(kind(&i), HeadlineKind::NeedsDecision);
        i.review.as_mut().unwrap().findings.remove(0);
        assert_eq!(kind(&i), HeadlineKind::SafeToOverride);
        i.review.as_mut().unwrap().findings.clear();
        assert_eq!(kind(&i), HeadlineKind::Passed);
        // While not final, findings are described but carry no actions.
        let waiting = guidance::build(&all());
        assert!(waiting.actions.is_empty());
        assert_eq!(waiting.findings.len(), 3);
    }
}

// End to end, through the store.

#[test]
fn guidance_passed_offers_mark_done_then_commit() {
    let env = Env::new();
    env.commands(&[]);
    env.write_agent_report(
        "ada",
        &report_by("ada", json!([met("r-1", "test a"), met("r-2", "test b")])),
    );
    env.verify().unwrap();
    let g = env.store.guidance(P, S).unwrap();
    assert_eq!(
        (g.headline.kind, g.headline.text.as_str()),
        (HeadlineKind::Passed, "Passed")
    );
    let kinds: Vec<GuidanceActionKind> = g.actions.iter().map(|a| a.kind).collect();
    // `no_test_command` is a follow-up after them.
    assert_eq!(
        kinds[..2],
        [GuidanceActionKind::MarkDone, GuidanceActionKind::Commit]
    );
}

#[test]
fn guidance_waits_for_the_slices_agents_from_task_prompts_and_ledger_sessions() {
    let env = Env::new();
    env.commands(&[]);
    env.repo
        .write(&format!(".foundry/prompts/{S}-ada.md"), "# task");
    env.repo
        .write(&format!(".foundry/prompts/{S}-bo.md"), "# task");
    env.repo.write(".foundry/prompts/handoff.md", "# handoff");
    env.ledger(writes::EntryKind::Start, "sess-cy", S, "cy", 100);
    env.write_agent_report(
        "ada",
        &report_by("ada", json!([met("r-1", "test a"), met("r-2", "test b")])),
    );
    env.verify().unwrap();
    let g = env.store.guidance(P, S).unwrap();
    assert_eq!(g.headline.kind, HeadlineKind::Waiting);
    assert_eq!(
        g.headline.text,
        "Waiting for bo and cy. Earlier results aren't final."
    );
    assert!(g.actions.is_empty());
}

#[test]
fn guidance_for_a_stale_verification_is_reverify() {
    let env = Env::new();
    env.commands(&[]);
    env.write_agent_report(
        "ada",
        &report_by("ada", json!([met("r-1", "test a"), met("r-2", "test b")])),
    );
    env.verify().unwrap();
    env.write_agent_report(
        "ada",
        &report_by("ada", json!([met("r-1", "test a2"), met("r-2", "test b")])),
    );
    let g = env.store.guidance(P, S).unwrap();
    assert_eq!(g.headline.kind, HeadlineKind::Stale);
    let kinds: Vec<GuidanceActionKind> = g.actions.iter().map(|a| a.kind).collect();
    assert_eq!(kinds, [GuidanceActionKind::Reverify]);
}

#[test]
fn guidance_for_a_skip_gives_its_plain_reason_and_fix() {
    let env = Env::new();
    env.store
        .set_test_commands(P, vec!["cargo test".into()])
        .unwrap();
    env.write_agent_report(
        "ada",
        &report_by("ada", json!([met("r-1", "test a"), met("r-2", "test b")])),
    );
    let refused = env.verify().unwrap_err();
    let g = env.store.guidance(P, S).unwrap();
    assert_eq!(g.headline.kind, HeadlineKind::Skipped);
    assert_eq!(g.headline.text, refused.message);
    assert_eq!(g.actions[0].id, "confirm_test_commands");
    assert_eq!(g.actions[1].kind, GuidanceActionKind::Reverify);
}

#[test]
fn guidance_sees_another_slices_folders_and_a_small_untracked_edit() {
    let env = Env::new();
    env.commands(&[]);
    env.plan_two(&env.repo.git(&["rev-parse", "HEAD"]));
    env.repo
        .write(&format!(".foundry/prompts/{S}-ada.md"), "# task");
    env.repo
        .write(&format!(".foundry/prompts/{OTHER}-cy.md"), "# task");
    // Legacy run (no ledger): protected for ada, but cy's folder on s-other.
    env.repo.write("src/secret/key.rs", "// cy's\n");
    env.write_agent_report(
        "ada",
        &report_by("ada", json!([met("r-1", "test a"), met("r-2", "test b")])),
    );
    env.verify().unwrap();
    let g = env.store.guidance(P, S).unwrap();
    assert_eq!(g.headline.kind, HeadlineKind::SafeToOverride);
    let template = g.actions[0].reason_template.as_deref().unwrap();
    assert!(
        template.contains("cy's folders (slice s-other)"),
        "{template}"
    );

    // One small file outside every glob, nobody else's.
    let env = Env::new();
    env.commands(&[]);
    env.repo.write("docs/notes.md", "one\ntwo\n");
    env.write_agent_report(
        "ada",
        &report_by("ada", json!([met("r-1", "test a"), met("r-2", "test b")])),
    );
    env.verify().unwrap();
    let g = env.store.guidance(P, S).unwrap();
    assert_eq!(g.headline.kind, HeadlineKind::SafeToOverride);
    assert_eq!(
        g.actions[0].reason_template.as_deref(),
        Some("Edit to docs/notes.md needed for this slice; <reason>")
    );
}

// ---------- requirement checks ----------

mod requirement_checks {
    use super::*;
    use crate::core::checks::Limits;

    impl Env {
        /// Sets fields on requirement `id` in plan.json (adding it, project-wide, if missing).
        fn requirement_fields(&self, id: &str, fields: Value) {
            let mut plan: Value =
                serde_json::from_str(&self.repo.read(".foundry/plan.json")).unwrap();
            let requirements = plan["requirements"].as_array_mut().unwrap();
            if !requirements.iter().any(|r| r["id"] == id) {
                requirements.push(json!({ "id": id, "sliceId": null, "projectWide": true }));
            }
            let r = requirements.iter_mut().find(|r| r["id"] == id).unwrap();
            for (key, value) in fields.as_object().unwrap() {
                r[key] = value.clone();
            }
            self.repo.write(".foundry/plan.json", &plan.to_string());
        }

        /// A slice ready to verify: no test commands, both requirements met with evidence.
        fn ready(&self) {
            self.commands(&[]);
            self.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
        }
    }

    fn results(v: &Verification) -> Vec<(String, bool)> {
        v.review
            .check_results
            .iter()
            .map(|r| (crate::core::checks::describe(&r.check), r.passed))
            .collect()
    }

    fn failed(v: &Verification) -> Vec<&Finding> {
        v.review
            .findings
            .iter()
            .filter(|f| f.code == code::CHECK_FAILED)
            .collect()
    }

    fn all_five() -> Value {
        json!([
            { "type": "file_exists", "path": "components.json" },
            { "type": "file_absent", "path": "src/legacy-sidebar.tsx" },
            { "type": "dependency_present", "name": "@base-ui-components/*" },
            { "type": "dependency_absent", "name": "@radix-ui/*" },
            { "type": "text_absent", "glob": "src/**/*.tsx", "pattern": r#"from\s+["']@radix-ui/"# }
        ])
    }

    #[test]
    fn every_check_type_passes_then_fails_with_the_first_file_and_line() {
        let env = Env::new();
        env.ready();
        env.requirement_fields("r-1", json!({ "checks": all_five() }));
        env.repo.write("components.json", "{}");
        env.repo.write(
            "package.json",
            r#"{ "dependencies": { "react": "19", "@base-ui-components/react": "1" } }"#,
        );
        env.repo.write(
            "src/app.tsx",
            "import { Menu } from \"@base-ui-components/react\";\n",
        );

        let v = env.verify().unwrap();
        assert!(
            results(&v).iter().all(|(_, passed)| *passed),
            "{:?}",
            v.review.check_results
        );
        assert_eq!(v.review.check_results.len(), 5);
        assert!(v
            .review
            .check_results
            .iter()
            .all(|r| r.requirement_id == "r-1"));
        assert!(failed(&v).is_empty());
        assert!(v.review.passed, "{:?}", v.review.findings);

        // Now break every one of them.
        fs::remove_file(env.repo.path().join("components.json")).unwrap();
        env.repo.write("src/legacy-sidebar.tsx", "export {}\n");
        env.repo.write(
            "package.json",
            "{\n  \"dependencies\": {\n    \"react\": \"19\",\n    \"@radix-ui/react-dialog\": \"1\"\n  }\n}\n",
        );
        env.repo.write(
            "src/ui/dialog.tsx",
            "// a dialog\n\nimport * as Dialog from '@radix-ui/react-dialog';\n",
        );

        let v = env.verify().unwrap();
        assert!(
            results(&v).iter().all(|(_, passed)| !*passed),
            "{:?}",
            results(&v)
        );
        assert!(!v.review.passed);
        let failed = failed(&v);
        assert_eq!(failed.len(), 5);
        assert!(failed
            .iter()
            .all(|f| f.severity == FindingSeverity::Danger
                && f.requirement_id.as_deref() == Some("r-1")));
        let by_check = |prefix: &str| {
            *failed
                .iter()
                .find(|f| f.message.contains(prefix))
                .unwrap_or_else(|| panic!("no {prefix} in {failed:?}"))
        };
        let dep = by_check("dependency_absent @radix-ui/*");
        assert!(
            dep.message
                .contains("@radix-ui/react-dialog is in package.json:4"),
            "{}",
            dep.message
        );
        assert_eq!(dep.path.as_deref(), Some("package.json"));
        let text = by_check("text_absent");
        assert!(
            text.message.contains("src/ui/dialog.tsx:3"),
            "{}",
            text.message
        );
        assert_eq!(text.path.as_deref(), Some("src/ui/dialog.tsx"));
        assert!(by_check("dependency_present")
            .message
            .contains("none of the project's 1"));
        assert!(by_check("file_exists components.json")
            .message
            .contains("doesn't exist"));
        assert_eq!(
            by_check("file_absent").path.as_deref(),
            Some("src/legacy-sidebar.tsx")
        );
        // Stored with the verification.
        let saved = env.store.get_verification(P, S).unwrap().unwrap();
        assert_eq!(saved.review.check_results, v.review.check_results);
        assert!(env
            .repo
            .read(&format!(".foundry/reviews/{S}.json"))
            .contains("\"checkResults\""));
    }

    #[test]
    fn a_dependency_name_ending_in_star_is_a_prefix_and_otherwise_exact() {
        let env = Env::new();
        env.ready();
        env.repo.write(
            "web/package.json",
            r#"{ "devDependencies": { "react-dom": "19", "@radix-ui/react-slot": "1" } }"#,
        );
        env.requirement_fields(
            "r-1",
            json!({ "checks": [
                { "type": "dependency_present", "name": "react" },
                { "type": "dependency_present", "name": "react*" },
                { "type": "dependency_absent", "name": "@radix-ui/*" },
                { "type": "dependency_absent", "name": "@radix-ui/react-dialog" }
            ]}),
        );
        let v = env.verify().unwrap();
        assert_eq!(
            results(&v),
            [
                ("dependency_present react".to_string(), false),
                ("dependency_present react*".to_string(), true),
                ("dependency_absent @radix-ui/*".to_string(), false),
                ("dependency_absent @radix-ui/react-dialog".to_string(), true),
            ]
        );
        assert!(failed(&v).iter().any(|f| f
            .message
            .contains("@radix-ui/react-slot is in web/package.json:1")));
    }

    #[test]
    fn node_modules_and_other_skipped_folders_are_never_looked_into() {
        let env = Env::new();
        env.ready();
        env.repo
            .write("package.json", r#"{ "dependencies": { "react": "19" } }"#);
        for dir in ["node_modules", "src/node_modules", "dist"] {
            env.repo.write(
                &format!("{dir}/@radix-ui/react-dialog/package.json"),
                r#"{ "name": "@radix-ui/react-dialog", "dependencies": { "@radix-ui/primitive": "1" } }"#,
            );
            env.repo.write(
                &format!("{dir}/@radix-ui/react-dialog/index.tsx"),
                "import x from '@radix-ui/primitive';\n",
            );
        }
        env.requirement_fields(
            "r-1",
            json!({ "checks": [
                { "type": "dependency_absent", "name": "@radix-ui/*" },
                { "type": "text_absent", "glob": "**/*.tsx", "pattern": "@radix-ui/" }
            ]}),
        );
        let v = env.verify().unwrap();
        assert!(
            v.review.check_results.iter().all(|r| r.passed),
            "{:?}",
            v.review.check_results
        );
        assert!(
            v.review.check_results[0].detail.contains("1 package.json"),
            "{}",
            v.review.check_results[0].detail
        );
    }

    #[test]
    fn a_passed_check_is_evidence_and_a_failed_one_blocks() {
        let env = Env::new();
        env.commands(&[]);
        env.write_report(&report(json!([
            met("r-1", "it uses Base UI"),
            met("r-2", "test b")
        ])));
        let v = env.verify().unwrap();
        assert_eq!(
            finding(&v, code::MISSING_EVIDENCE)
                .requirement_id
                .as_deref(),
            Some("r-1")
        );

        env.repo.write("components.json", "{}");
        env.requirement_fields(
            "r-1",
            json!({ "checks": [{ "type": "file_exists", "path": "components.json" }] }),
        );
        let v = env.verify().unwrap();
        assert!(
            !codes(&v).contains(&code::MISSING_EVIDENCE),
            "{:?}",
            codes(&v)
        );
        assert!(v.review.passed, "{:?}", v.review.findings);

        fs::remove_file(env.repo.path().join("components.json")).unwrap();
        let v = env.verify().unwrap();
        assert!(!v.review.passed, "a failed check blocks");
        assert_eq!(failed(&v).len(), 1);
        assert!(
            codes(&v).contains(&code::MISSING_EVIDENCE),
            "no longer proven"
        );
        let guidance = env.store.guidance(P, S).unwrap();
        assert_eq!(guidance.headline.kind, HeadlineKind::NeedsFix);
    }

    #[test]
    fn project_wide_requirements_checks_run_on_every_slice() {
        let env = Env::new();
        env.ready();
        env.requirement_fields(
            "r-rule",
            json!({ "checks": [{ "type": "file_absent", "path": "src/lib.rs" }] }),
        );
        env.requirement_fields(
            "r-9",
            json!({ "checks": [{ "type": "file_absent", "path": "src/lib.rs" }] }),
        );
        let v = env.verify().unwrap();
        assert_eq!(v.review.check_results.len(), 1, "not another slice's");
        assert_eq!(v.review.check_results[0].requirement_id, "r-rule");
        assert_eq!(failed(&v)[0].requirement_id.as_deref(), Some("r-rule"));
    }

    #[test]
    fn a_hand_edited_check_that_escapes_or_is_over_the_limits_fails_closed() {
        let env = Env::new();
        env.ready();
        env.requirement_fields(
            "r-1",
            json!({ "checks": [
                { "type": "file_absent", "path": "../outside.txt" },
                { "type": "text_absent", "glob": "src/../../**", "pattern": "x" },
                { "type": "text_absent", "glob": "src/**", "pattern": r"(\w{500}){500}" }
            ]}),
        );
        let v = env.verify().unwrap();
        assert!(v.review.check_results.iter().all(|r| !r.passed));
        let details: Vec<&str> = v
            .review
            .check_results
            .iter()
            .map(|r| r.detail.as_str())
            .collect();
        assert!(details[0].contains(".."), "{details:?}");
        assert!(details[1].contains(".."), "{details:?}");
        assert!(details[2].contains("size limit"), "{details:?}");
        assert!(!v.review.passed);
    }

    #[test]
    fn suggested_checks_are_never_run() {
        let env = Env::new();
        env.ready();
        env.requirement_fields(
            "r-1",
            json!({ "suggestedChecks": [{ "type": "file_exists", "path": "nowhere.json" }] }),
        );
        let v = env.verify().unwrap();
        assert!(v.review.check_results.is_empty());
        assert!(v.review.passed, "{:?}", v.review.findings);
    }

    #[test]
    fn older_plans_and_reviews_without_checks_still_load() {
        let env = Env::new();
        env.ready();
        // Env's plan has no checks fields at all.
        let v = env.verify().unwrap();
        assert!(v.review.check_results.is_empty());
        let path = format!(".foundry/reviews/{S}.json");
        let mut old: Value = serde_json::from_str(&env.repo.read(&path)).unwrap();
        old.as_object_mut().unwrap().remove("checkResults");
        env.repo.write(&path, &old.to_string());
        let loaded = env.store.get_verification(P, S).unwrap().unwrap();
        assert!(loaded.review.check_results.is_empty());
    }

    // The runner's own limits, which the commands can't reach.

    #[test]
    fn the_time_limit_and_the_file_cap_fail_a_check_closed() {
        let repo = TestRepo::new();
        repo.write("src/a.tsx", "fine\n");
        let check = RequirementCheck::TextAbsent {
            glob: "**".into(),
            pattern: "@radix-ui/".into(),
        };
        let out = checks::run(
            repo.path(),
            &check,
            &Limits {
                time_limit: Duration::ZERO,
                ..Limits::default()
            },
        );
        assert!(!out.passed);
        assert!(out.detail.contains("time limit"), "{}", out.detail);

        repo.write("src/b.tsx", "fine\n");
        let out = checks::run(
            repo.path(),
            &check,
            &Limits {
                max_files: 1,
                ..Limits::default()
            },
        );
        assert!(!out.passed);
        assert!(
            out.detail.contains("stopped after 1 files"),
            "{}",
            out.detail
        );
        assert!(checks::run(repo.path(), &check, &Limits::default()).passed);
    }

    #[test]
    fn matches_are_capped_and_large_files_are_skipped() {
        let repo = TestRepo::new();
        repo.write("src/many.ts", &"use @radix-ui/x\n".repeat(50));
        repo.write("src/big.ts", &"x".repeat(2048));
        let limits = Limits {
            max_matches: 3,
            max_file_bytes: 1024,
            ..Limits::default()
        };
        let radix = RequirementCheck::TextAbsent {
            glob: "src/many.ts".into(),
            pattern: "@radix-ui/".into(),
        };
        let out = checks::run(repo.path(), &radix, &limits);
        assert!(!out.passed);
        assert_eq!(
            out.detail,
            "\"@radix-ui/\" matches src/many.ts:1 (and 2 or more others)"
        );
        let big = RequirementCheck::TextAbsent {
            glob: "src/big.ts".into(),
            pattern: "x".into(),
        };
        let out = checks::run(repo.path(), &big, &limits);
        assert!(out.passed);
        assert!(
            out.detail.contains("1 file(s) over 1 KB not searched"),
            "{}",
            out.detail
        );
    }
}

#[test]
fn requirement_checks_run_after_the_test_commands() {
    let env = Env::new();
    // The platform shell's own redirect: only a check that runs after it sees the file.
    env.commands(&["echo {} > components.json".to_string()]);
    env.write_report(&report(json!([met("r-1", "test a"), met("r-2", "test b")])));
    let mut plan: Value = serde_json::from_str(&env.repo.read(".foundry/plan.json")).unwrap();
    plan["requirements"][0]["checks"] =
        json!([{ "type": "file_exists", "path": "components.json" }]);
    env.repo.write(".foundry/plan.json", &plan.to_string());

    let v = env.verify().unwrap();

    assert_eq!(v.review.test_runs[0].exit_code, Some(0));
    assert!(
        v.review.check_results[0].passed,
        "{:?}",
        v.review.check_results
    );
}
