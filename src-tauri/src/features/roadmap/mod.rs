//! Turning `docs/brain-dump.md` into a roadmap with `claude -p`, then checking the result
//! mechanically before anything is saved (see `validate`).
//!
//! Saved to `.foundry/roadmap.json`. Loading it into the plan as `proposed` waits for the
//! plan store (P2).

pub mod commands;
mod validate;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

use crate::core::checks::RequirementCheck;
use crate::core::claude::ClaudeStatus;
use crate::core::events::EventMeta;
pub use crate::core::llm::{ClaudeRunner, LlmRunner, RunControl};
use crate::core::repos::RepoIndex;
use crate::core::{repo_write, AppError, ErrorCode};

const BRAIN_DUMP: &str = "docs/brain-dump.md";
const ROADMAP_FILE: &str = ".foundry/roadmap.json";
const ERROR_FILE: &str = ".foundry/last-generation-error.txt";
const PROMPT: &str = include_str!("prompt.md");
/// How long one generation may take when no `generationTimeoutMs` is configured.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(5 * 60);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TargetUser {
    pub name: String,
    pub description: String,
    pub pain_points: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Feature {
    pub family: String,
    pub description: String,
    pub features: Vec<String>,
    pub relates_to: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Slice {
    pub name: String,
    pub delivers: Vec<String>,
    /// Names of other slices.
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    pub text: String,
    pub kind: String,
    pub source_excerpt: String,
    pub slice_name: Option<String>,
    pub verified_by: String,
    /// The excerpt isn't in the brain dump (whitespace-normalized): kept, but flagged.
    pub unsourced: bool,
    /// Checks the model proposes for a `constraint` (always empty for other kinds). Invalid
    /// ones are dropped, not retried. Carried into the plan's `suggestedChecks`; never run.
    /// Missing in older roadmaps: none.
    #[serde(default)]
    pub suggested_checks: Vec<RequirementCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Deferred {
    pub label: String,
    pub anchored_to: String,
}

/// What the model produced, after validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Roadmap {
    pub goal: String,
    pub target_users: Vec<TargetUser>,
    pub feature_map: Vec<Feature>,
    pub slices: Vec<Slice>,
    /// Requirements whose `sliceName` names a slice.
    pub requirements: Vec<Requirement>,
    pub deferred: Vec<Deferred>,
    pub open_questions: Vec<String>,
}

/// `.foundry/roadmap.json`, and what `generate_roadmap` / `get_roadmap` return.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RoadmapResult {
    pub roadmap: Roadmap,
    /// Requirements whose `sliceName` is missing or names no slice.
    pub unassigned: Vec<Requirement>,
    /// Brain-dump paragraphs no requirement's excerpt falls inside: what the roadmap missed.
    pub uncovered_paragraphs: Vec<String>,
    /// 1, or 2 when the first reply was rejected.
    pub attempts: u32,
    // u32 stands in for the exported type only (TS `number`), as with EventMeta.ts.
    #[specta(type = u32)]
    pub generated_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Preflight,
    Running,
    Validating,
    Retrying,
    Done,
    Failed,
}

/// `roadmap:progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "roadmap:progress")]
pub struct RoadmapProgress {
    #[serde(flatten)]
    pub meta: EventMeta,
    pub project_id: String,
    pub stage: Stage,
    /// Since `generate_roadmap` started.
    #[specta(type = u32)]
    pub elapsed_ms: u64,
}

/// Where progress events go: the app emits them, tests record them.
pub trait ProgressSink: Send + Sync + 'static {
    fn progress(&self, event: RoadmapProgress);
}

type RunnerSource = Box<dyn Fn() -> Result<Arc<dyn LlmRunner>, AppError> + Send + Sync>;
type StatusSource = Box<dyn Fn() -> ClaudeStatus + Send + Sync>;
type TimeoutSource = Box<dyn Fn() -> Duration + Send + Sync>;

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Removes a project's entry from the running map however the generation ends.
struct Running<'a> {
    map: &'a Mutex<HashMap<String, Arc<RunControl>>>,
    project_id: &'a str,
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        lock(self.map).remove(self.project_id);
    }
}

/// Owns roadmap generation for every project. Share it as `Arc<RoadmapStore>`.
pub struct RoadmapStore {
    repos: Arc<RepoIndex>,
    /// Runs the preflight and returns a runner (or `claude_not_found`, `not_signed_in`…).
    runner: RunnerSource,
    status: StatusSource,
    events: Arc<dyn ProgressSink>,
    /// Read at the start of every generation (the `generationTimeoutMs` setting).
    timeout: TimeoutSource,
    running: Mutex<HashMap<String, Arc<RunControl>>>,
}

impl RoadmapStore {
    pub fn new(
        repos: Arc<RepoIndex>,
        runner: impl Fn() -> Result<Arc<dyn LlmRunner>, AppError> + Send + Sync + 'static,
        status: impl Fn() -> ClaudeStatus + Send + Sync + 'static,
        events: Arc<dyn ProgressSink>,
    ) -> Self {
        Self {
            repos,
            runner: Box::new(runner),
            status: Box::new(status),
            events,
            timeout: Box::new(|| DEFAULT_TIMEOUT),
            running: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_timeout(self, timeout: Duration) -> Self {
        self.with_timeout_source(move || timeout)
    }

    /// The time limit is read from `source` at the start of each generation.
    pub fn with_timeout_source(
        mut self,
        source: impl Fn() -> Duration + Send + Sync + 'static,
    ) -> Self {
        self.timeout = Box::new(source);
        self
    }

    pub fn claude_status(&self) -> ClaudeStatus {
        (self.status)()
    }

    /// Stops the project's running generation. Returns whether one was running.
    pub fn cancel(&self, project_id: &str) -> bool {
        match lock(&self.running).get(project_id) {
            Some(control) => {
                control.cancel();
                tracing::info!(project_id, "roadmap generation cancel requested");
                true
            }
            None => false,
        }
    }

    /// The last saved roadmap, or `None` if there isn't one.
    pub fn get(&self, project_id: &str) -> Result<Option<RoadmapResult>, AppError> {
        let repo = self.repos.repo(project_id)?;
        let Some(text) = repo_write::read_to_string(&repo, ROADMAP_FILE)? else {
            return Ok(None);
        };
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|err| AppError::internal(format!("{ROADMAP_FILE} is unreadable: {err}")))
    }

    /// Generates, validates (retrying once with the errors), and saves the roadmap. Nothing
    /// is written unless validation passes, except the raw output of a second failure.
    pub fn generate(&self, project_id: &str) -> Result<RoadmapResult, AppError> {
        let repo = self.repos.repo(project_id)?;
        let control = Arc::new(RunControl::new((self.timeout)()));
        {
            let mut running = lock(&self.running);
            if running.contains_key(project_id) {
                return Err(AppError::new(
                    ErrorCode::GenerationRunning,
                    "a roadmap is already being generated for this project",
                ));
            }
            running.insert(project_id.to_string(), Arc::clone(&control));
        }
        let _running = Running {
            map: &self.running,
            project_id,
        };
        let started = Instant::now();
        let emit = |stage: Stage| {
            self.events.progress(RoadmapProgress {
                meta: EventMeta::new(),
                project_id: project_id.to_string(),
                stage,
                elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            });
        };
        let result = self.generate_in(&repo, &control, &emit);
        match &result {
            Ok(saved) => {
                emit(Stage::Done);
                tracing::info!(project_id, attempts = saved.attempts, "roadmap generated");
            }
            Err(err) => {
                emit(Stage::Failed);
                tracing::warn!(
                    project_id,
                    code = err.code.as_str(),
                    "roadmap generation failed"
                );
            }
        }
        result
    }

    fn generate_in(
        &self,
        repo: &Path,
        control: &RunControl,
        emit: &dyn Fn(Stage),
    ) -> Result<RoadmapResult, AppError> {
        emit(Stage::Preflight);
        let runner = (self.runner)()?;
        let brain_dump = repo_write::read_to_string(repo, BRAIN_DUMP)?.unwrap_or_default();
        if brain_dump.trim().is_empty() {
            return Err(AppError::invalid_input(
                "the brain dump is empty: write it before generating a roadmap",
            ));
        }
        // The template's line endings depend on the checkout (git autocrlf): send LF always.
        let prompt = PROMPT
            .replace("\r\n", "\n")
            .replacen("{{BRAIN_DUMP}}", brain_dump.trim(), 1);

        control.check()?;
        emit(Stage::Running);
        let first = runner.run(&prompt, control)?;
        control.check()?;
        emit(Stage::Validating);
        let (checked, attempts) = match validate::check(&first, &brain_dump) {
            Ok(checked) => (checked, 1),
            Err(errors) => {
                emit(Stage::Retrying);
                let second = runner.run(&retry_prompt(&prompt, &errors), control)?;
                control.check()?;
                emit(Stage::Validating);
                match validate::check(&second, &brain_dump) {
                    Ok(checked) => (checked, 2),
                    Err(errors) => {
                        repo_write::write(repo, ERROR_FILE, &second)?;
                        return Err(AppError::new(
                            ErrorCode::GenerationFailed,
                            format!(
                                "the roadmap failed validation twice (raw output saved to \
                                 {ERROR_FILE}):\n- {}",
                                errors.join("\n- ")
                            ),
                        ));
                    }
                }
            }
        };
        let result = RoadmapResult {
            roadmap: checked.roadmap,
            unassigned: checked.unassigned,
            uncovered_paragraphs: checked.uncovered_paragraphs,
            attempts,
            generated_at: now_ms(),
        };
        let json = serde_json::to_string_pretty(&result)
            .map_err(|err| AppError::internal(format!("serialize roadmap: {err}")))?;
        repo_write::write(repo, ROADMAP_FILE, json + "\n")?;
        // TODO(P2): load slices and requirements into the plan as `proposed` (refusing over
        // confirmed requirements unless discardConfirmed).
        Ok(result)
    }
}

/// The original prompt plus why the last reply was rejected.
fn retry_prompt(prompt: &str, errors: &[String]) -> String {
    format!(
        "{prompt}\n\nYour previous reply was rejected for these reasons:\n- {}\n\n\
         Reply again with the corrected JSON object only.",
        errors.join("\n- ")
    )
}

#[cfg(test)]
mod tests;
