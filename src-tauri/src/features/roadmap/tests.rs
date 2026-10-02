use std::collections::VecDeque;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::validate::{check, paragraphs};
use super::*;
use crate::core::process_tree::pid_alive;
use crate::testing::{install_fake_claude, TestRepo};

const P: &str = "project-1";

const BRAIN: &str = "# Orbit

Users can save a reading list
and sync it across devices.

The app must work offline for at least a week.

---

We might add social sharing later.

Pricing is still undecided.
";

// ---------- fixtures ----------

enum Reply {
    Text(String),
    /// Wait until the run is cancelled or times out.
    Block,
}

#[derive(Default)]
struct FakeRunner {
    replies: Mutex<VecDeque<Reply>>,
    prompts: Mutex<Vec<String>>,
}

impl FakeRunner {
    fn with(replies: Vec<Reply>) -> Arc<Self> {
        Arc::new(Self {
            replies: Mutex::new(replies.into()),
            prompts: Mutex::default(),
        })
    }

    fn prompts(&self) -> Vec<String> {
        self.prompts.lock().unwrap().clone()
    }
}

impl LlmRunner for FakeRunner {
    fn run(&self, prompt: &str, control: &RunControl) -> Result<String, AppError> {
        self.prompts.lock().unwrap().push(prompt.to_string());
        match self.replies.lock().unwrap().pop_front() {
            Some(Reply::Text(text)) => Ok(text),
            Some(Reply::Block) => loop {
                control.check()?;
                thread::sleep(Duration::from_millis(10));
            },
            None => Err(AppError::internal("FakeRunner ran out of replies")),
        }
    }
}

#[derive(Default)]
struct Stages(Mutex<Vec<Stage>>);

impl ProgressSink for Stages {
    fn progress(&self, event: RoadmapProgress) {
        assert_eq!(event.project_id, P);
        self.0.lock().unwrap().push(event.stage);
    }
}

impl Stages {
    fn all(&self) -> Vec<Stage> {
        self.0.lock().unwrap().clone()
    }
}

struct Env {
    repo: TestRepo,
    runner: Arc<FakeRunner>,
    stages: Arc<Stages>,
    store: Arc<RoadmapStore>,
}

fn status_stub() -> ClaudeStatus {
    ClaudeStatus {
        path: None,
        version: None,
        logged_in: true,
        auth_method: Some("claude.ai".into()),
        api_provider: Some("firstParty".into()),
        subscription_type: None,
        problem: None,
    }
}

impl Env {
    fn new(replies: Vec<Reply>) -> Self {
        Self::with_timeout(replies, DEFAULT_TIMEOUT)
    }

    fn with_timeout(replies: Vec<Reply>, timeout: Duration) -> Self {
        let repo = TestRepo::new();
        repo.write("docs/brain-dump.md", BRAIN);
        let repos = Arc::new(RepoIndex::default());
        repos.replace_all([(P, repo.path())]);
        let runner = FakeRunner::with(replies);
        let stages = Arc::new(Stages::default());
        let source = Arc::clone(&runner);
        let store = RoadmapStore::new(
            repos,
            move || Ok(Arc::clone(&source) as Arc<dyn LlmRunner>),
            status_stub,
            Arc::clone(&stages) as Arc<dyn ProgressSink>,
        )
        .with_timeout(timeout);
        Self {
            repo,
            runner,
            stages,
            store: Arc::new(store),
        }
    }

    fn file(&self, rel: &str) -> Option<String> {
        fs::read_to_string(self.repo.path().join(rel)).ok()
    }
}

fn text(value: &Value) -> Reply {
    Reply::Text(value.to_string())
}

/// A valid roadmap for BRAIN: two sourced requirements, one per slice.
fn valid() -> Value {
    json!({
        "goal": "Read anywhere",
        "targetUsers": [{ "name": "Reader", "description": "Reads a lot", "painPoints": ["loses links"] }],
        "featureMap": [{ "family": "Library", "description": "Saved items", "features": ["reading list", "sync"], "relatesTo": [] }],
        "slices": [
            { "name": "Core", "delivers": ["save a list"], "dependsOn": [] },
            { "name": "Offline", "delivers": ["read offline"], "dependsOn": ["Core"] }
        ],
        "requirements": [
            { "text": "Save a reading list", "kind": "behavior",
              "sourceExcerpt": "Users can save a reading list and sync it across devices.",
              "sliceName": "Core", "verifiedBy": "add an item, see it on a second device" },
            { "text": "Works offline for a week", "kind": "constraint",
              "sourceExcerpt": "must work offline for at least a week",
              "sliceName": "Offline", "verifiedBy": "airplane mode for 7 days" }
        ],
        "deferred": [{ "label": "Social sharing", "anchoredTo": "Library" }],
        "openQuestions": ["What does it cost?"]
    })
}

// ---------- R-3-1 (roadmap half; loading into the plan waits for P2) ----------

#[test]
fn valid_output_is_validated_and_saved_to_roadmap_json() {
    let env = Env::new(vec![text(&valid())]);

    let result = env.store.generate(P).unwrap();

    assert_eq!(result.attempts, 1);
    assert_eq!(result.roadmap.goal, "Read anywhere");
    assert_eq!(result.roadmap.slices.len(), 2);
    assert_eq!(result.roadmap.requirements.len(), 2);
    assert!(result.roadmap.requirements.iter().all(|r| !r.unsourced));
    assert!(result.unassigned.is_empty());
    let saved: RoadmapResult = serde_json::from_str(&env.file(ROADMAP_FILE).unwrap()).unwrap();
    assert_eq!(saved, result);
    assert_eq!(env.store.get(P).unwrap(), Some(result));
    assert_eq!(
        env.stages.all(),
        [
            Stage::Preflight,
            Stage::Running,
            Stage::Validating,
            Stage::Done
        ]
    );
    let prompt = &env.runner.prompts()[0];
    assert!(
        prompt.contains("<brain_dump>\n# Orbit"),
        "brain dump embedded"
    );
    assert!(!prompt.contains("{{BRAIN_DUMP}}"));
}

// ---------- R-3-2 ----------

#[test]
fn json_inside_code_fences_parses() {
    for wrapped in [
        format!("```json\n{}\n```", valid()),
        format!("```\n{}\n```\n", valid()),
        format!("Here is the roadmap:\n\n```json\n{:#}\n```\nDone.", valid()),
    ] {
        let checked = check(&wrapped, BRAIN).unwrap_or_else(|e| panic!("{e:?} for {wrapped}"));
        assert_eq!(checked.roadmap.goal, "Read anywhere");
    }
    let env = Env::new(vec![Reply::Text(format!("```json\n{}\n```", valid()))]);
    assert_eq!(env.store.generate(P).unwrap().attempts, 1);
}

// ---------- R-3-3 ----------

#[test]
fn invalid_then_valid_succeeds_after_one_retry_with_the_errors_in_the_prompt() {
    let mut broken = valid();
    broken.as_object_mut().unwrap().remove("goal");
    broken["slices"][0]["delivers"] = json!("not a list");
    let env = Env::new(vec![text(&broken), text(&valid())]);

    let result = env.store.generate(P).unwrap();

    assert_eq!(result.attempts, 2);
    let prompts = env.runner.prompts();
    assert_eq!(prompts.len(), 2);
    assert!(
        prompts[1].starts_with(&prompts[0]),
        "the retry repeats the task"
    );
    assert!(
        prompts[1].contains("roadmap.goal is missing"),
        "{}",
        prompts[1]
    );
    assert!(prompts[1].contains("roadmap.slices[0].delivers must be an array of strings"));
    assert_eq!(
        env.stages.all(),
        [
            Stage::Preflight,
            Stage::Running,
            Stage::Validating,
            Stage::Retrying,
            Stage::Validating,
            Stage::Done
        ]
    );
}

// ---------- R-3-4 (roadmap half; the plan doesn't exist yet) ----------

#[test]
fn invalid_twice_fails_saves_the_raw_output_and_changes_nothing() {
    let earlier = Env::new(vec![text(&valid())]);
    earlier.store.generate(P).unwrap();
    let before = earlier.file(ROADMAP_FILE).unwrap();
    let env = Env {
        runner: FakeRunner::with(vec![
            Reply::Text("not json at all".into()),
            Reply::Text("{\"goal\": 5}".into()),
        ]),
        ..earlier
    };
    let source = Arc::clone(&env.runner);
    let repos = Arc::new(RepoIndex::default());
    repos.replace_all([(P, env.repo.path())]);
    let store = RoadmapStore::new(
        repos,
        move || Ok(Arc::clone(&source) as Arc<dyn LlmRunner>),
        status_stub,
        Arc::new(Stages::default()) as Arc<dyn ProgressSink>,
    );

    let err = store.generate(P).unwrap_err();

    assert_eq!(err.code, ErrorCode::GenerationFailed);
    assert!(
        err.message.contains("roadmap.goal must be a string"),
        "{}",
        err.message
    );
    assert!(
        err.message.contains("roadmap.slices is missing"),
        "{}",
        err.message
    );
    assert_eq!(
        env.file(ERROR_FILE).as_deref(),
        Some("{\"goal\": 5}"),
        "raw second output"
    );
    assert_eq!(
        env.file(ROADMAP_FILE).unwrap(),
        before,
        "roadmap.json untouched"
    );
}

// ---------- R-3-5 ----------

#[test]
fn a_hallucinated_source_excerpt_is_flagged_unsourced_not_dropped() {
    let mut output = valid();
    output["requirements"][0]["sourceExcerpt"] = json!("Users can share lists with friends");
    output["requirements"].as_array_mut().unwrap().push(
        json!({ "text": "No source", "kind": "constraint", "sourceExcerpt": "",
                      "sliceName": "Core", "verifiedBy": "-" }),
    );
    // Different line breaks and spacing from the brain dump still count as verbatim.
    output["requirements"][1]["sourceExcerpt"] = json!("must   work offline\nfor at least a week");

    let checked = check(&output.to_string(), BRAIN).unwrap();

    let requirements = &checked.roadmap.requirements;
    assert_eq!(requirements.len(), 3, "nothing dropped");
    assert!(requirements[0].unsourced, "made-up excerpt");
    assert!(!requirements[1].unsourced, "whitespace-normalized match");
    assert!(requirements[2].unsourced, "an empty excerpt is no source");
    // An unsourced requirement doesn't cover its paragraph.
    assert!(checked
        .uncovered_paragraphs
        .contains(&"Users can save a reading list\nand sync it across devices.".to_string()));
}

/// Requirement kinds must be the plan's, so a roadmap can be loaded into it as is.
#[test]
fn requirement_kinds_must_be_the_plans() {
    for kind in ["behavior", "interaction", "constraint"] {
        let mut output = valid();
        output["requirements"][0]["kind"] = json!(kind);
        assert!(check(&output.to_string(), BRAIN).is_ok(), "{kind}");
    }
    for kind in ["functional", "non-functional", "Behavior", ""] {
        let mut output = valid();
        output["requirements"][0]["kind"] = json!(kind);
        let errors = check(&output.to_string(), BRAIN).unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| e.starts_with("roadmap.requirements[0].kind must be one of")),
            "{kind}: {errors:?}"
        );
    }

    // The old kinds trigger the retry, and the retry prompt says what's allowed.
    let mut old = valid();
    old["requirements"][0]["kind"] = json!("functional");
    let env = Env::new(vec![text(&old), text(&valid())]);
    assert_eq!(env.store.generate(P).unwrap().attempts, 2);
    let prompts = env.runner.prompts();
    assert!(prompts[0].contains(r#""kind": "behavior | interaction | constraint""#));
    assert!(prompts[1]
        .contains(r#"must be one of "behavior", "interaction", "constraint" (got "functional")"#));
}

// ---------- R-3-6 ----------

#[test]
fn a_dependency_cycle_is_rejected() {
    let mut cyclic = valid();
    cyclic["slices"] = json!([
        { "name": "A", "delivers": [], "dependsOn": ["C"] },
        { "name": "B", "delivers": [], "dependsOn": ["A"] },
        { "name": "C", "delivers": [], "dependsOn": ["B"] }
    ]);
    let errors = check(&cyclic.to_string(), BRAIN).unwrap_err();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("cycle"), "{errors:?}");
    assert!(errors[0].contains("A → C → B → A"), "{errors:?}");

    let env = Env::new(vec![text(&cyclic), text(&cyclic)]);
    let err = env.store.generate(P).unwrap_err();
    assert_eq!(err.code, ErrorCode::GenerationFailed);
    assert!(err.message.contains("cycle"));
    assert!(env.file(ROADMAP_FILE).is_none());
}

#[test]
fn unresolvable_dependencies_are_errors_and_unresolvable_requirements_are_unassigned() {
    let mut output = valid();
    output["slices"][1]["dependsOn"] = json!(["Core", "Ghost"]);
    let errors = check(&output.to_string(), BRAIN).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|e| e.contains("\"Ghost\", which is not a slice")),
        "{errors:?}"
    );

    let mut self_dep = valid();
    self_dep["slices"][0]["dependsOn"] = json!(["Core"]);
    assert!(check(&self_dep.to_string(), BRAIN).unwrap_err()[0].contains("depends on itself"));

    let mut duplicate = valid();
    duplicate["slices"][1]["name"] = json!("Core");
    duplicate["slices"][1]["dependsOn"] = json!([]);
    assert!(check(&duplicate.to_string(), BRAIN).unwrap_err()[0].contains("used twice"));

    let mut stray = valid();
    stray["requirements"][1]["sliceName"] = json!("Nowhere");
    stray["requirements"][0]["sliceName"] = Value::Null;
    let checked = check(&stray.to_string(), BRAIN).unwrap();
    assert!(checked.roadmap.requirements.is_empty());
    assert_eq!(checked.unassigned.len(), 2, "kept, as unassigned");
}

// ---------- R-3-7 ----------

#[test]
fn uncovered_paragraphs_are_the_ones_no_excerpt_falls_inside() {
    let checked = check(&valid().to_string(), BRAIN).unwrap();
    assert_eq!(
        checked.uncovered_paragraphs,
        [
            "We might add social sharing later.",
            "Pricing is still undecided."
        ],
        "the heading and the rule aren't paragraphs; the two sourced ones are covered"
    );
    assert_eq!(
        paragraphs(BRAIN),
        [
            "Users can save a reading list\nand sync it across devices.",
            "The app must work offline for at least a week.",
            "We might add social sharing later.",
            "Pricing is still undecided."
        ]
    );
    assert_eq!(paragraphs("one\r\n\r\ntwo\r\n"), ["one", "two"]);
}

// ---------- R-3-9 (run half; the plan doesn't exist yet) ----------

#[test]
fn cancel_stops_the_run_and_saves_nothing() {
    let env = Env::new(vec![Reply::Block]);
    let store = Arc::clone(&env.store);
    let run = thread::spawn(move || store.generate(P));
    let deadline = Instant::now() + Duration::from_secs(10);
    while !env.stages.all().contains(&Stage::Running) {
        assert!(Instant::now() < deadline, "never started");
        thread::sleep(Duration::from_millis(5));
    }
    assert!(!env.store.cancel("other-project"));

    assert!(env.store.cancel(P));

    assert_eq!(run.join().unwrap().unwrap_err().code, ErrorCode::Cancelled);
    assert_eq!(env.stages.all().last(), Some(&Stage::Failed));
    assert!(env.file(ROADMAP_FILE).is_none());
    assert!(env.file(ERROR_FILE).is_none());
    assert!(!env.store.cancel(P), "nothing left running");
}

#[test]
fn timeout_stops_the_run_and_saves_nothing() {
    let env = Env::with_timeout(vec![Reply::Block], Duration::from_millis(150));
    let started = Instant::now();

    let err = env.store.generate(P).unwrap_err();

    assert_eq!(err.code, ErrorCode::TimedOut);
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(env.file(ROADMAP_FILE).is_none());
}

#[test]
fn one_generation_per_project_at_a_time() {
    let env = Env::new(vec![Reply::Block]);
    let store = Arc::clone(&env.store);
    let run = thread::spawn(move || store.generate(P));
    while !env.stages.all().contains(&Stage::Running) {
        thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        env.store.generate(P).unwrap_err().code,
        ErrorCode::GenerationRunning
    );
    env.store.cancel(P);
    run.join().unwrap().unwrap_err();
}

#[test]
fn preflight_failures_and_an_empty_brain_dump_start_nothing() {
    let repo = TestRepo::new();
    let repos = Arc::new(RepoIndex::default());
    repos.replace_all([(P, repo.path())]);
    let stages = Arc::new(Stages::default());
    let store = RoadmapStore::new(
        Arc::clone(&repos),
        || Err(AppError::new(ErrorCode::NotSignedIn, "sign in")),
        status_stub,
        Arc::clone(&stages) as Arc<dyn ProgressSink>,
    );
    assert_eq!(store.generate(P).unwrap_err().code, ErrorCode::NotSignedIn);
    assert_eq!(stages.all(), [Stage::Preflight, Stage::Failed]);

    let env = Env::new(vec![text(&valid())]);
    env.repo.write("docs/brain-dump.md", "  \n");
    assert_eq!(
        env.store.generate(P).unwrap_err().code,
        ErrorCode::InvalidInput
    );
    assert!(
        env.runner.prompts().is_empty(),
        "the model was never called"
    );
    assert_eq!(
        env.store.generate("nope").unwrap_err().code,
        ErrorCode::NotFound
    );
}

// ---------- the real runner, against fake-claude ----------

fn claude_runner(dir: &TestRepo) -> ClaudeRunner {
    ClaudeRunner::new(install_fake_claude(dir.path()))
}

fn result_of(runner: &ClaudeRunner, prompt: &str) -> String {
    runner
        .run(prompt, &RunControl::new(Duration::from_secs(30)))
        .unwrap()
}

// ---------- R-3-10 ----------

#[test]
fn the_spawned_claude_has_no_api_or_provider_variables() {
    std::env::set_var("ANTHROPIC_API_KEY", "sk-should-not-leak");
    std::env::set_var("ANTHROPIC_BASE_URL", "https://example.invalid");
    std::env::set_var("CLAUDE_CODE_USE_VERTEX", "1");
    let dir = TestRepo::new();

    let env = result_of(&claude_runner(&dir), "FAKE:ENV");

    assert!(
        env.lines()
            .any(|l| l.starts_with("PATH=") || l.starts_with("Path=")),
        "{env}"
    );
    for line in env.lines() {
        let key = line.split('=').next().unwrap_or("").to_ascii_uppercase();
        assert!(
            !key.starts_with("ANTHROPIC_") && !key.starts_with("CLAUDE_CODE_USE_"),
            "{line} leaked"
        );
    }
    assert!(!env.contains("sk-should-not-leak"));
}

#[test]
fn claude_runs_in_print_mode_with_no_tools_and_the_prompt_on_stdin() {
    let dir = TestRepo::new();
    let runner = claude_runner(&dir);
    // fake-claude refuses any other flags, and only sees FAKE:ARGS if stdin carried it.
    let args = result_of(&runner, "FAKE:ARGS");
    assert_eq!(args.lines().collect::<Vec<_>>(), ClaudeRunner::ARGS);

    // A prompt far over the Windows command-line limit (32 767 characters).
    fs::write(dir.path().join("reply.txt"), "long ok").unwrap();
    assert_eq!(result_of(&runner, &"x".repeat(200_000)), "long ok");
}

#[test]
fn claude_runs_in_a_scratch_folder_that_is_removed_afterwards() {
    let dir = TestRepo::new();

    let cwd = PathBuf::from(result_of(&claude_runner(&dir), "FAKE:CWD"));

    let name = cwd.file_name().unwrap().to_string_lossy().into_owned();
    assert!(name.starts_with("foundry-llm-"), "{}", cwd.display());
    let temp = std::env::temp_dir().canonicalize().unwrap();
    assert_eq!(
        cwd.parent().unwrap().canonicalize().unwrap(),
        temp,
        "under the temp folder"
    );
    assert!(!cwd.exists(), "scratch folder removed after the run");
}

/// P3 runs `claude -p` through std's `Command`, which escapes arguments for `.cmd` files
/// (or refuses ones it can't). A `--model` value with a cmd.exe special character must reach
/// claude exactly as given, or not start at all: never be reinterpreted on the way.
#[cfg(windows)]
#[test]
fn model_through_a_cmd_shim_arrives_verbatim() {
    let dir = TestRepo::new();
    let exe = install_fake_claude(dir.path());
    let shim = dir.path().join("claude.cmd");
    fs::write(&shim, format!("@\"{}\" %*\r\n", exe.display())).unwrap();

    for model in ["opus[1m]", "a&b"] {
        let runner = ClaudeRunner::new(shim.clone()).with_model(Some(model.into()));
        match runner.run("FAKE:ARGS", &RunControl::new(Duration::from_secs(30))) {
            Ok(args) => {
                eprintln!("{model}: passed verbatim");
                let mut expected: Vec<&str> = ClaudeRunner::ARGS.to_vec();
                expected.extend(["--model", model]);
                assert_eq!(args.lines().collect::<Vec<_>>(), expected, "{model}");
            }
            // Refusing to start is also safe.
            Err(err) => {
                eprintln!("{model}: refused ({err})");
                assert_eq!(err.code, ErrorCode::Io, "{model}: {err}");
            }
        }
    }
}

fn wait_for_pids(dir: &TestRepo) -> Vec<u32> {
    // Generous: each test runs a freshly copied exe, which Windows Defender scans on first run.
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Ok(text) = fs::read_to_string(dir.path().join("pids.txt")) {
            let pids: Vec<u32> = text.split(',').filter_map(|p| p.parse().ok()).collect();
            if pids.len() == 2 && pids[1] != 0 {
                return pids;
            }
        }
        assert!(Instant::now() < deadline, "fake-claude never started");
        thread::sleep(Duration::from_millis(20));
    }
}

/// R-3-9 at the process level: cancelling kills `claude` and everything it started.
#[test]
fn cancel_kills_the_claude_process_tree() {
    let dir = TestRepo::new();
    let runner = Arc::new(claude_runner(&dir));
    let control = Arc::new(RunControl::new(Duration::from_secs(60)));
    let (r, c) = (Arc::clone(&runner), Arc::clone(&control));
    let run = thread::spawn(move || r.run("FAKE:SLEEP", &c));
    let pids = wait_for_pids(&dir);
    assert!(pids.iter().all(|&pid| pid_alive(pid)));

    control.cancel();

    assert_eq!(run.join().unwrap().unwrap_err().code, ErrorCode::Cancelled);
    let gone = Instant::now() + Duration::from_secs(3);
    while pids.iter().any(|&pid| pid_alive(pid)) {
        assert!(Instant::now() < gone, "still running: {pids:?}");
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn timeout_kills_the_claude_process_tree() {
    let dir = TestRepo::new();
    let runner = Arc::new(claude_runner(&dir));
    // Long enough for fake-claude to start under a loaded test run; checked before it ends.
    let control = Arc::new(RunControl::new(Duration::from_secs(8)));
    let (r, c) = (Arc::clone(&runner), Arc::clone(&control));
    let run = thread::spawn(move || r.run("FAKE:SLEEP", &c));
    let pids = wait_for_pids(&dir);
    assert!(
        pids.iter().all(|&pid| pid_alive(pid)),
        "running before the deadline"
    );

    let err = run.join().unwrap().unwrap_err();

    assert_eq!(err.code, ErrorCode::TimedOut);
    assert!(
        !control.is_cancelled(),
        "stopped by the deadline, not a cancel"
    );
    let gone = Instant::now() + Duration::from_secs(3);
    while pids.iter().any(|&pid| pid_alive(pid)) {
        assert!(Instant::now() < gone, "still running: {pids:?}");
        thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn error_replies_and_crashes_are_reported() {
    let dir = TestRepo::new();
    let runner = claude_runner(&dir);
    let control = RunControl::new(Duration::from_secs(30));

    let err = runner.run("FAKE:ERROR", &control).unwrap_err();
    assert_eq!(err.code, ErrorCode::GenerationFailed);
    assert!(
        err.message.contains("usage limit reached"),
        "{}",
        err.message
    );

    let err = runner.run("FAKE:CRASH", &control).unwrap_err();
    assert_eq!(err.code, ErrorCode::GenerationFailed);
    assert!(err.message.contains("something broke"), "{}", err.message);
    assert!(err.message.contains("Some(3)"), "{}", err.message);
}

/// The real CLI. Run with `FOUNDRY_LIVE=1 cargo test live_ -- --ignored`.
#[test]
#[ignore = "uses the real claude CLI and your subscription; set FOUNDRY_LIVE=1"]
fn live_claude_generates_a_valid_roadmap() {
    if std::env::var("FOUNDRY_LIVE").as_deref() != Ok("1") {
        eprintln!("skipped: set FOUNDRY_LIVE=1");
        return;
    }
    let repo = TestRepo::new();
    repo.write("docs/brain-dump.md", BRAIN);
    let repos = Arc::new(RepoIndex::default());
    repos.replace_all([(P, repo.path())]);
    let store = RoadmapStore::new(
        repos,
        || {
            let program = crate::core::claude::preflight(None)?;
            Ok(Arc::new(ClaudeRunner::new(program)) as Arc<dyn LlmRunner>)
        },
        || crate::core::claude::status(None),
        Arc::new(Stages::default()) as Arc<dyn ProgressSink>,
    );

    let result = store.generate(P).unwrap();

    assert!(!result.roadmap.slices.is_empty());
    assert!(!result.roadmap.requirements.is_empty() || !result.unassigned.is_empty());
    eprintln!(
        "attempts={} requirements={} unsourced={} uncovered={}",
        result.attempts,
        result.roadmap.requirements.len() + result.unassigned.len(),
        result
            .roadmap
            .requirements
            .iter()
            .chain(&result.unassigned)
            .filter(|r| r.unsourced)
            .count(),
        result.uncovered_paragraphs.len()
    );
}
