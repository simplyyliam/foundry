//! Slice reports, verification and the manager digest (P9).
//!
//! Agents only write their own `.foundry/reports/<sliceId>.<agentId>.json` (`core::report`);
//! a project made before per-agent reports may hold a legacy `<sliceId>.json` too, read the
//! same way. `verify_slice` checks every report of the slice against the plan, runs the
//! project's confirmed test commands, compares the files changed since the slice started
//! with the reporting agents' globs, and saves the result to `.foundry/reviews/<sliceId>.json`.
//! Everything else here only reads.
//!
//! Test commands come from `.foundry/project.json`, which an agent might edit, and run with
//! the human's rights. So `verify_slice` runs them only when they equal, exactly, the list the
//! human last confirmed, which is kept in app data, never in the repo.

mod attribution;
pub mod commands;
mod digest;
mod disk;
mod guidance;
mod run;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;
use tauri_specta::Event;

use crate::core::events::EventMeta;
use crate::core::git::{self, Git, GitRun};
use crate::core::report::{self, RequirementStatus, SliceReport};
use crate::core::repos::RepoIndex;
use crate::core::sessions::{now_ms, SessionRegistry};
use crate::core::{glob, hook_policy, repo_write, setup, AppError, ErrorCode};

pub use digest::{
    AgentDigest, AttentionItem, AttentionKind, ManagerDigest, SliceDigest, VerificationState,
};
pub use guidance::{
    FindingGroup, GuidanceAction, GuidanceActionKind, GuidedFinding, Headline, HeadlineKind,
    VerificationGuidance,
};

/// Each test command gets this long before its process tree is killed.
pub const TEST_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Left out of the boundary check: `project.json`, because changing the test commands
/// through Foundry rewrites it after a slice's start commit, and an agent's edit of them can't
/// run anyway (the confirmation gate).
const IGNORED_FILES: &[&str] = &[".foundry/project.json"];

/// Where task prompts live (`<sliceId>-<agentId>.md`), written by the prompts feature.
const PROMPTS_DIR: &str = ".foundry/prompts";

/// Foundry's own folders, left out of the boundary check.
const IGNORED_DIRS: &[&str] = &[
    ".foundry/reports/",
    ".foundry/events/",
    ".foundry/reviews/",
    ".foundry/prompts/",
];

/// Files Foundry rewrites itself while a slice runs (every status change rewrites the plan
/// and its mirror), so a change to them can't be pinned on the agent: reported as `info`.
const FOUNDRY_WRITTEN: &[&str] = &[".foundry/plan.json", "vsm_data.js"];

/// Finding codes. Stable: the UI may key on them.
pub mod code {
    pub const UNREPORTED_REQUIREMENT: &str = "unreported_requirement";
    pub const UNKNOWN_REQUIREMENT: &str = "unknown_requirement";
    pub const SLICE_MISMATCH: &str = "slice_mismatch";
    pub const MISSING_EVIDENCE: &str = "missing_evidence";
    pub const NO_TEST_COMMAND: &str = "no_test_command";
    pub const TEST_FAILED: &str = "test_failed";
    pub const TEST_TIMED_OUT: &str = "test_timed_out";
    pub const PROTECTED_FILE_CHANGED: &str = "protected_file_changed";
    pub const OUTSIDE_WRITE_GLOBS: &str = "outside_write_globs";
    pub const FOUNDRY_FILE_CHANGED: &str = "foundry_file_changed";
    pub const BOUNDARY_SKIPPED: &str = "boundary_skipped";
    pub const CONTRADICTION: &str = "contradiction";
    /// One agent says `partly` where another says `met` (info; the requirement counts as met).
    pub const PARTLY_REPORTED: &str = "partly_reported";
    /// A blocker an agent listed in its report.
    pub const REPORTED_BLOCKER: &str = "reported_blocker";
    /// A changed file that only another slice's session wrote (info).
    pub const OWNED_BY_OTHER_SLICE: &str = "owned_by_other_slice";
    /// A file this slice wrote that an overlapping session of another slice also wrote (warn).
    pub const SHARED_FILE_TOUCHED: &str = "shared_file_touched";
    /// A file that changed during this slice's session without an edit tool while another
    /// slice's session was running too, so it can't be told whose it is (info).
    pub const AMBIGUOUS_SHELL_CHANGE: &str = "ambiguous_shell_change";
}

// ---------- types ----------

/// Ordered: `info < warn < danger`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Type,
)]
#[serde(rename_all = "snake_case")]
pub enum FindingSeverity {
    Info,
    Warn,
    Danger,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub severity: FindingSeverity,
    /// One of the stable codes, e.g. `unreported_requirement` (see CONTRACT_CHANGELOG.md).
    pub code: String,
    pub message: String,
    pub requirement_id: Option<String>,
    /// The repo-relative file a boundary finding is about.
    pub path: Option<String>,
    /// The agent whose report a finding is about; `null` for findings about the slice as a
    /// whole (tests, boundary, coverage).
    pub agent: Option<String>,
}

impl Finding {
    fn new(severity: FindingSeverity, code: &str, message: impl Into<String>) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            requirement_id: None,
            path: None,
            agent: None,
        }
    }

    fn agent(mut self, agent: &str) -> Self {
        self.agent = Some(agent.into());
        self
    }

    fn requirement(mut self, id: &str) -> Self {
        self.requirement_id = Some(id.into());
        self
    }

    fn path(mut self, path: &str) -> Self {
        self.path = Some(path.into());
        self
    }
}

/// One confirmed test command, as run by `verify_slice`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CommandRun {
    pub command: String,
    /// `null` when it was killed at the time limit or couldn't start.
    pub exit_code: Option<i32>,
    // u32 stands in for the exported type only (TS `number`), as with EventMeta.ts.
    #[specta(type = u32)]
    pub duration_ms: u64,
    /// The last 8 KB of its output (stdout and stderr together).
    pub tail: String,
    pub timed_out: bool,
}

/// One report file a verification read, exactly as it was on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VerifiedReport {
    /// Repo-relative, e.g. `.foundry/reports/s-feed.qa.json`.
    pub path: String,
    /// From the file name; for a legacy `<sliceId>.json`, the report's `agent` field.
    pub agent: String,
    #[specta(type = SliceReport)]
    pub report: Value,
}

/// A slice report with the file and agent it belongs to (`list_reports`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentReport {
    pub path: String,
    /// From the file name; for a legacy `<sliceId>.json`, the report's `agent` field.
    pub agent: String,
    pub report: SliceReport,
    /// When the report file was last written (its modified time), ISO 8601 UTC, e.g.
    /// `2026-10-07T14:03:05.123Z`. Legacy `<sliceId>.json` reports have one too.
    pub written_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReportedStatus {
    pub agent: String,
    pub status: RequirementStatus,
}

/// One requirement across every report of the slice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RequirementOutcome {
    pub id: String,
    /// `not_met` if any report says so; else `met` if any report says so; else `partly`.
    /// `null` when no report lists it.
    pub status: Option<RequirementStatus>,
    /// What each report that lists it says, in report order.
    pub reported_by: Vec<ReportedStatus>,
}

/// `.foundry/reviews/<sliceId>.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub slice_id: String,
    /// No `danger` finding and no unreported requirement.
    pub passed: bool,
    pub findings: Vec<Finding>,
    pub test_runs: Vec<CommandRun>,
    /// Every report file that was verified (legacy first, then by agent). Empty in a review
    /// saved before per-agent reports, which verified only `report`.
    #[serde(default)]
    pub reports: Vec<VerifiedReport>,
    /// The slice's requirements (the plan's, then any others a report lists), with each one's
    /// status across the reports. Empty in a review saved before per-agent reports.
    #[serde(default)]
    pub requirements: Vec<RequirementOutcome>,
    /// The first of `reports`, kept for the shape from before per-agent reports.
    #[specta(type = SliceReport)]
    pub report: Value,
    /// Epoch ms.
    #[specta(type = u32)]
    pub verified_at: u64,
}

/// What `verify_slice` and `get_verification` return.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Verification {
    #[serde(flatten)]
    pub review: Review,
    /// The slice's report files on disk no longer equal the ones verified: one changed, is
    /// gone, or a new one appeared.
    pub stale: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TestCommands {
    /// `testCommands` from `.foundry/project.json`.
    pub commands: Vec<String>,
    /// They equal, exactly, the list last confirmed with `confirm_test_commands`, or there
    /// are none (an empty list needs no confirmation).
    pub confirmed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum VerifyStage {
    Starting,
    Checking,
    RunningTests,
    /// Finished and saved; `passed` is in the result.
    Done,
    /// Stopped by an error; nothing saved.
    Failed,
    /// Stopped by `cancel_verification`; nothing saved.
    Cancelled,
}

/// `verify:progress`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "verify:progress")]
pub struct VerifyProgress {
    #[serde(flatten)]
    pub meta: EventMeta,
    pub project_id: String,
    pub slice_id: String,
    pub stage: VerifyStage,
    /// The test command starting, with `running_tests`.
    pub command: Option<String>,
    /// Since `verify_slice` started.
    #[specta(type = u32)]
    pub elapsed_ms: u64,
}

/// Why a wanted verification couldn't run. One per refusal of `verify_slice` (auto mode's or
/// a click's), next to the error code it returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SkipReason {
    /// `test_commands_unconfirmed`.
    TestCommandsUnconfirmed,
    /// `not_found`: the slice has no report.
    NoReport,
    /// `invalid_json`: a report doesn't parse as one.
    ReportIncomplete,
    /// `no_start_commit`: the slice has no valid start commit.
    NoStartCommit,
    /// `verification_running`.
    AnotherVerificationRunning,
    /// `setup_not_committed`: no valid start commit, and the setup files aren't committed.
    SetupNotCommitted,
    /// Anything else (e.g. no such slice, an unreadable plan); `message` says what.
    Other,
}

impl SkipReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TestCommandsUnconfirmed => "test_commands_unconfirmed",
            Self::NoReport => "no_report",
            Self::ReportIncomplete => "report_incomplete",
            Self::NoStartCommit => "no_start_commit",
            Self::AnotherVerificationRunning => "another_verification_running",
            Self::SetupNotCommitted => "setup_not_committed",
            Self::Other => "other",
        }
    }
}

/// The latest time verifying a slice was wanted but refused, kept while its cause holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VerifySkip {
    pub slice_id: String,
    pub reason: SkipReason,
    /// The error code `verify_slice` returned.
    pub code: ErrorCode,
    /// Its message, as-is.
    pub message: String,
    /// Epoch ms.
    #[specta(type = u32)]
    pub at: u64,
}

impl VerifySkip {
    fn new(slice_id: &str, reason: SkipReason, error: &AppError) -> Self {
        Self {
            slice_id: slice_id.to_string(),
            reason,
            code: error.code,
            message: error.message.clone(),
            at: now_ms(),
        }
    }
}

/// `verify:skipped`: a verification was wanted but refused; the slice's skip is now `skip`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "verify:skipped")]
pub struct VerifySkipped {
    #[serde(flatten)]
    pub meta: EventMeta,
    pub project_id: String,
    pub skip: VerifySkip,
}

/// `verify:skip_cleared`: the slice's skip is gone (it was verified, or the cause went away).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "verify:skip_cleared")]
pub struct VerifySkipCleared {
    #[serde(flatten)]
    pub meta: EventMeta,
    pub project_id: String,
    pub slice_id: String,
}

/// Where verification events go: the app emits them, tests record them.
pub trait VerifyEvents: Send + Sync + 'static {
    fn progress(&self, event: VerifyProgress);
    fn skipped(&self, event: VerifySkipped);
    fn skip_cleared(&self, event: VerifySkipCleared);
}

// ---------- helpers ----------

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Slice ids name files: 1–128 ASCII letters, digits, `-` or `_`.
fn check_slice_id(id: &str) -> Result<(), AppError> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(AppError::invalid_input(format!(
            "sliceId must be 1-128 letters, digits, '-' or '_': {id:?}"
        )))
    }
}

fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// Evidence names a test or a command: it mentions a test or spec, quotes something in
/// backticks, or contains one of the test commands (confirmed, or run by the agent whose
/// report it is).
fn names_test_or_command(evidence: &str, commands: &[String], report: &SliceReport) -> bool {
    let evidence = evidence.trim();
    if evidence.is_empty() {
        return false;
    }
    let lower = evidence.to_lowercase();
    lower.contains("test")
        || lower.contains("spec")
        || evidence.contains('`')
        || commands
            .iter()
            .chain(report.tests_run.iter().map(|t| &t.command))
            .map(|c| c.trim())
            .any(|c| !c.is_empty() && evidence.contains(c))
}

/// A full or abbreviated commit id, so it can't be taken for a git option.
fn is_commit_id(text: &str) -> bool {
    (7..=64).contains(&text.len()) && text.bytes().all(|b| b.is_ascii_hexdigit())
}

/// One of the slice's report files, parsed.
struct LoadedReport {
    path: String,
    agent: String,
    value: Value,
    report: SliceReport,
}

/// Every report of the slice, legacy first then by agent. `invalid_json` for one that isn't a
/// report. A per-agent file's agent is its name's; a legacy file's is its `agent` field.
fn load_reports(repo: &Path, slice_id: &str) -> Result<Vec<LoadedReport>, AppError> {
    let mut out = Vec::new();
    for path in report::slice_report_paths(repo, slice_id)? {
        // Gone since it was listed: as if it never was.
        let Some(value) = disk::read_json(repo, &path)? else {
            continue;
        };
        let parsed = parse_report(&path, value.clone())?;
        let agent = path
            .rsplit('/')
            .next()
            .and_then(report::parse_report_name)
            .and_then(|(_, agent)| agent)
            .unwrap_or(&parsed.agent)
            .to_string();
        out.push(LoadedReport {
            path,
            agent,
            value,
            report: parsed,
        });
    }
    Ok(out)
}

/// Whether the slice's report files on disk differ from the ones `review` verified.
fn is_stale(repo: &Path, review: &Review) -> Result<bool, AppError> {
    let verified = report::verified_snapshot(
        &review.slice_id,
        review
            .reports
            .iter()
            .map(|r| (r.path.clone(), r.report.clone())),
        review.report.clone(),
    );
    Ok(report::snapshot(repo, &review.slice_id)? != verified)
}

/// Each requirement's status across the reports: the plan's ids in plan order, then any
/// others the reports list, in the order first listed.
fn outcomes(expected: &[&str], reports: &[LoadedReport]) -> Vec<RequirementOutcome> {
    let mut ids: Vec<&str> = Vec::new();
    let listed = reports
        .iter()
        .flat_map(|l| &l.report.requirements)
        .map(|r| r.id.as_str());
    for id in expected.iter().copied().chain(listed) {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    ids.into_iter()
        .map(|id| {
            let reported_by: Vec<ReportedStatus> = reports
                .iter()
                .flat_map(|l| {
                    l.report
                        .requirements
                        .iter()
                        .filter(|r| r.id == id)
                        .map(|r| ReportedStatus {
                            agent: l.agent.clone(),
                            status: r.status,
                        })
                })
                .collect();
            let says = |status| reported_by.iter().any(|a| a.status == status);
            let status = if reported_by.is_empty() {
                None
            } else if says(RequirementStatus::NotMet) {
                Some(RequirementStatus::NotMet)
            } else if says(RequirementStatus::Met) {
                Some(RequirementStatus::Met)
            } else {
                Some(RequirementStatus::Partly)
            };
            RequirementOutcome {
                id: id.to_string(),
                status,
                reported_by,
            }
        })
        .collect()
}

/// What a verification needs, gathered and checked before it starts.
struct Ready {
    repo: PathBuf,
    commands: Vec<String>,
    /// At least one.
    reports: Vec<LoadedReport>,
    /// A commit id.
    start_commit: String,
    /// The slice's requirement ids, in plan order.
    expected: Vec<String>,
    workspace_access: bool,
}

/// Why `verify_slice` didn't run, or failed: the error it returns, and the skip reason to
/// record (`None`: nothing to record, e.g. a bad slice id, or a run that started and failed).
struct Refusal {
    reason: Option<SkipReason>,
    error: AppError,
}

impl Refusal {
    fn skip(reason: SkipReason, error: AppError) -> Self {
        Self {
            reason: Some(reason),
            error,
        }
    }

    fn unrecorded(error: AppError) -> Self {
        Self {
            reason: None,
            error,
        }
    }
}

fn running_error() -> AppError {
    AppError::new(
        ErrorCode::VerificationRunning,
        "a verification is already running for this project",
    )
}

/// Removes a project's entry from the running map however the verification ends.
struct Running<'a> {
    map: &'a Mutex<HashMap<String, Arc<AtomicBool>>>,
    project_id: &'a str,
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        lock(self.map).remove(self.project_id);
    }
}

// ---------- the store ----------

/// Owns verification for every project. Share it as `Arc<VerificationStore>`.
pub struct VerificationStore {
    repos: Arc<RepoIndex>,
    sessions: Arc<SessionRegistry>,
    git: Git,
    /// App data: where the confirmed test commands live.
    data_dir: PathBuf,
    events: Arc<dyn VerifyEvents>,
    timeout: Duration,
    /// Project id → its running verification's cancel flag.
    running: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// Serializes setting, confirming and checking test commands.
    commands_lock: Mutex<()>,
    /// Project id → slice id → its latest skip. In memory: after a restart the next refusal
    /// records it again.
    skips: Mutex<HashMap<String, BTreeMap<String, VerifySkip>>>,
}

impl VerificationStore {
    pub fn new(
        repos: Arc<RepoIndex>,
        sessions: Arc<SessionRegistry>,
        git: Git,
        data_dir: impl Into<PathBuf>,
        events: Arc<dyn VerifyEvents>,
    ) -> Self {
        Self {
            repos,
            sessions,
            git,
            data_dir: data_dir.into(),
            events,
            timeout: TEST_TIMEOUT,
            running: Mutex::new(HashMap::new()),
            commands_lock: Mutex::new(()),
            skips: Mutex::new(HashMap::new()),
        }
    }

    /// The time limit for each test command (tests shorten it).
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    // ---------- test commands ----------

    fn test_commands_locked(
        &self,
        project_id: &str,
        repo: &Path,
    ) -> Result<TestCommands, AppError> {
        let commands = disk::test_commands(repo)?;
        // An empty list runs nothing, so it needs no confirmation.
        let confirmed = commands.is_empty()
            || disk::read_confirmed(&self.data_dir)?.get(project_id) == Some(&commands);
        Ok(TestCommands {
            commands,
            confirmed,
        })
    }

    pub fn get_test_commands(&self, project_id: &str) -> Result<TestCommands, AppError> {
        let repo = self.repos.repo(project_id)?;
        let _guard = lock(&self.commands_lock);
        self.test_commands_locked(project_id, &repo)
    }

    /// Replaces `testCommands` in `.foundry/project.json` (each trimmed; blank ones and line
    /// breaks refused) and drops the confirmation: they must be confirmed again.
    pub fn set_test_commands(
        &self,
        project_id: &str,
        commands: Vec<String>,
    ) -> Result<TestCommands, AppError> {
        let repo = self.repos.repo(project_id)?;
        let commands: Vec<String> = commands.iter().map(|c| c.trim().to_string()).collect();
        if let Some(bad) = commands
            .iter()
            .find(|c| c.is_empty() || c.contains(['\n', '\r', '\0']))
        {
            return Err(AppError::invalid_input(format!(
                "a test command must be one non-empty line: {bad:?}"
            )));
        }
        let _guard = lock(&self.commands_lock);
        let mut confirmed = disk::read_confirmed(&self.data_dir)?;
        if confirmed.remove(project_id).is_some() {
            disk::write_confirmed(&self.data_dir, &confirmed)?;
        }
        disk::set_test_commands(&repo, &commands)?;
        tracing::info!(project_id, count = commands.len(), "test commands set");
        let result = self.test_commands_locked(project_id, &repo);
        drop(_guard);
        self.recheck_skips(project_id);
        result
    }

    /// Confirms `shown`, the list the human reviewed, verbatim. If `testCommands` on disk no
    /// longer equals it exactly, refuses with `test_commands_changed` and confirms nothing.
    pub fn confirm_test_commands(
        &self,
        project_id: &str,
        shown: Vec<String>,
    ) -> Result<TestCommands, AppError> {
        let repo = self.repos.repo(project_id)?;
        let _guard = lock(&self.commands_lock);
        let commands = disk::test_commands(&repo)?;
        if commands != shown {
            tracing::warn!(project_id, "test commands changed while being reviewed");
            return Err(AppError::new(
                ErrorCode::TestCommandsChanged,
                "The test commands changed while you were reviewing them. Review them again.",
            ));
        }
        let mut confirmed = disk::read_confirmed(&self.data_dir)?;
        confirmed.insert(project_id.to_string(), commands);
        disk::write_confirmed(&self.data_dir, &confirmed)?;
        tracing::info!(project_id, "test commands confirmed");
        let result = self.test_commands_locked(project_id, &repo);
        drop(_guard);
        self.recheck_skips(project_id);
        result
    }

    // ---------- reports and verifications ----------

    /// The slice's first report (the legacy `<sliceId>.json`, else the first agent's by id),
    /// or `None` if there's none. Kept for its old shape; [`Self::list_reports`] gives them
    /// all. `invalid_json` when a report doesn't parse as one.
    pub fn get_report(
        &self,
        project_id: &str,
        slice_id: &str,
    ) -> Result<Option<SliceReport>, AppError> {
        Ok(self
            .list_reports(project_id, slice_id)?
            .into_iter()
            .next()
            .map(|r| r.report))
    }

    /// Every report of the slice, each with its file and agent: the legacy `<sliceId>.json`
    /// first, then one per agent by agent id. `invalid_json` when one doesn't parse as a
    /// report.
    pub fn list_reports(
        &self,
        project_id: &str,
        slice_id: &str,
    ) -> Result<Vec<AgentReport>, AppError> {
        check_slice_id(slice_id)?;
        let repo = self.repos.repo(project_id)?;
        let mut out = Vec::new();
        for l in load_reports(&repo, slice_id)? {
            // Gone since it was read: as if it never was, like `load_reports`.
            let Some(written_at) = report::written_at(&repo, &l.path)? else {
                continue;
            };
            out.push(AgentReport {
                path: l.path,
                agent: l.agent,
                report: l.report,
                written_at,
            });
        }
        Ok(out)
    }

    /// The slice's last verification, with `stale` worked out now; `None` if never verified.
    pub fn get_verification(
        &self,
        project_id: &str,
        slice_id: &str,
    ) -> Result<Option<Verification>, AppError> {
        check_slice_id(slice_id)?;
        let repo = self.repos.repo(project_id)?;
        let rel = disk::review_path(slice_id);
        let Some(value) = disk::read_json(&repo, &rel)? else {
            return Ok(None);
        };
        let review: Review = serde_json::from_value(value).map_err(|err| {
            AppError::new(
                ErrorCode::InvalidJson,
                format!("{rel} is unreadable ({err})"),
            )
        })?;
        let stale = is_stale(&repo, &review)?;
        Ok(Some(Verification { review, stale }))
    }

    /// Stops the project's running verification; it then fails with `cancelled` and saves
    /// nothing. Returns whether one was running.
    pub fn cancel(&self, project_id: &str) -> bool {
        match lock(&self.running).get(project_id) {
            Some(flag) => {
                flag.store(true, Ordering::SeqCst);
                tracing::info!(project_id, "verification cancel requested");
                true
            }
            None => false,
        }
    }

    /// What verifying the slice would hit right now, without recording or running anything:
    /// `None` when it would run. Errors only for a bad slice id or an unknown project.
    pub fn check_verify(
        &self,
        project_id: &str,
        slice_id: &str,
    ) -> Result<Option<VerifySkip>, AppError> {
        match self.current_refusal(project_id, slice_id) {
            Ok(None) => Ok(None),
            Ok(Some((reason, error))) => Ok(Some(VerifySkip::new(slice_id, reason, &error))),
            Err(error) => Err(error),
        }
    }

    /// The slice's recorded skip: the latest time verifying it was refused, while that cause
    /// still holds.
    pub fn verify_skip(&self, project_id: &str, slice_id: &str) -> Option<VerifySkip> {
        lock(&self.skips)
            .get(project_id)
            .and_then(|slices| slices.get(slice_id))
            .cloned()
    }

    /// `preflight` plus the running check, for a slice that isn't being verified by us.
    fn current_refusal(
        &self,
        project_id: &str,
        slice_id: &str,
    ) -> Result<Option<(SkipReason, AppError)>, AppError> {
        match self.preflight(project_id, slice_id) {
            Ok(_) if lock(&self.running).contains_key(project_id) => Ok(Some((
                SkipReason::AnotherVerificationRunning,
                running_error(),
            ))),
            Ok(_) => Ok(None),
            Err(Refusal {
                reason: Some(reason),
                error,
            }) => Ok(Some((reason, error))),
            Err(Refusal { error, .. }) => Err(error),
        }
    }

    /// Everything a verification needs, or why it can't run. Refusals carry the skip reason,
    /// except a bad slice id or an unknown project (nothing to record it against).
    fn preflight(&self, project_id: &str, slice_id: &str) -> Result<Ready, Refusal> {
        check_slice_id(slice_id).map_err(Refusal::unrecorded)?;
        let repo = self.repos.repo(project_id).map_err(Refusal::unrecorded)?;
        let other = |error: AppError| Refusal::skip(SkipReason::Other, error);
        let commands = {
            let _guard = lock(&self.commands_lock);
            let current = self
                .test_commands_locked(project_id, &repo)
                .map_err(other)?;
            if !current.confirmed {
                return Err(Refusal::skip(
                    SkipReason::TestCommandsUnconfirmed,
                    AppError::new(
                        ErrorCode::TestCommandsUnconfirmed,
                        "the project's test commands changed or were never confirmed: review \
                         and confirm them before verifying",
                    ),
                ));
            }
            current.commands
        };
        let reports = load_reports(&repo, slice_id).map_err(|error| {
            if error.code == ErrorCode::InvalidJson {
                Refusal::skip(SkipReason::ReportIncomplete, error)
            } else {
                other(error)
            }
        })?;
        if reports.is_empty() {
            return Err(Refusal::skip(
                SkipReason::NoReport,
                AppError::not_found(format!(
                    "no report for slice {slice_id} in {}",
                    report::REPORTS_DIR
                )),
            ));
        }
        let plan = disk::plan(&repo).map_err(other)?;
        let slice = plan
            .slices
            .iter()
            .find(|s| s.id == slice_id)
            .ok_or_else(|| other(AppError::not_found(format!("no slice with id {slice_id}"))))?;
        let start_commit = slice
            .start_commit
            .as_deref()
            .map(str::trim)
            .filter(|s| is_commit_id(s))
            .map(str::to_string);
        let Some(start_commit) = start_commit else {
            return Err(self.no_start_commit(&repo, slice));
        };
        let expected = plan
            .requirements
            .iter()
            .filter(|r| r.slice_id.as_deref() == Some(slice_id))
            .map(|r| r.id.clone())
            .collect();
        // Project rules belong to no slice: a report that lists one anyway isn't held to it,
        // and the entry isn't checked. The stored report (`value`) keeps it as written.
        let project_wide: HashSet<&str> = plan
            .requirements
            .iter()
            .filter(|r| r.project_wide)
            .map(|r| r.id.as_str())
            .collect();
        let mut reports = reports;
        for l in &mut reports {
            l.report
                .requirements
                .retain(|r| !project_wide.contains(r.id.as_str()));
        }
        Ok(Ready {
            repo,
            commands,
            reports,
            start_commit,
            expected,
            workspace_access: slice.workspace_access,
        })
    }

    /// The slice has no start commit, or one that isn't a commit id: `setup_not_committed`
    /// when Foundry's setup files aren't committed (the usual reason none was recorded),
    /// otherwise `no_start_commit`.
    fn no_start_commit(&self, repo: &Path, slice: &disk::PlanSlice) -> Refusal {
        let name = &slice.name;
        let what = match slice.start_commit.as_deref().map(str::trim) {
            None | Some("") => "has no start commit".to_string(),
            Some(bad) => format!("has a start commit that isn't one ({bad:?})"),
        };
        let again = "move it back to queued and start it again";
        let uncommitted = setup::uncommitted(&self.git, repo);
        if uncommitted.is_empty() {
            Refusal::skip(
                SkipReason::NoStartCommit,
                AppError::new(
                    ErrorCode::NoStartCommit,
                    format!(
                        "\"{name}\" {what}, so its changes can't be told from what was there \
                         before. It needs a git repo with at least one commit: {again}."
                    ),
                ),
            )
        } else {
            Refusal::skip(
                SkipReason::SetupNotCommitted,
                AppError::new(
                    ErrorCode::SetupNotCommitted,
                    format!(
                        "\"{name}\" {what}, and Foundry's setup isn't committed: {} {} not \
                         committed. Commit your setup, then {again}.",
                        setup::list_some(&uncommitted),
                        if uncommitted.len() == 1 { "is" } else { "are" }
                    ),
                ),
            )
        }
    }

    fn record_skip(&self, project_id: &str, slice_id: &str, reason: SkipReason, error: &AppError) {
        let skip = VerifySkip::new(slice_id, reason, error);
        lock(&self.skips)
            .entry(project_id.to_string())
            .or_default()
            .insert(slice_id.to_string(), skip.clone());
        tracing::info!(
            project_id,
            slice_id,
            reason = reason.as_str(),
            "verification skipped"
        );
        self.events.skipped(VerifySkipped {
            meta: EventMeta::new(),
            project_id: project_id.to_string(),
            skip,
        });
    }

    fn clear_skip(&self, project_id: &str, slice_id: &str) {
        let removed = lock(&self.skips)
            .get_mut(project_id)
            .and_then(|slices| slices.remove(slice_id))
            .is_some();
        if removed {
            tracing::info!(project_id, slice_id, "verification skip cleared");
            self.events.skip_cleared(VerifySkipCleared {
                meta: EventMeta::new(),
                project_id: project_id.to_string(),
                slice_id: slice_id.to_string(),
            });
        }
    }

    /// Looks again at every recorded skip of the project: one whose cause is gone is cleared,
    /// one whose cause changed is recorded again with the new reason.
    fn recheck_skips(&self, project_id: &str) {
        let recorded: Vec<(String, SkipReason, String)> = lock(&self.skips)
            .get(project_id)
            .map(|slices| {
                slices
                    .values()
                    .map(|s| (s.slice_id.clone(), s.reason, s.message.clone()))
                    .collect()
            })
            .unwrap_or_default();
        for (slice_id, reason, message) in recorded {
            match self.current_refusal(project_id, &slice_id) {
                Ok(Some((now, error))) if now == reason && error.message == message => {}
                Ok(Some((now, error))) => self.record_skip(project_id, &slice_id, now, &error),
                Ok(None) | Err(_) => self.clear_skip(project_id, &slice_id),
            }
        }
    }

    /// Verifies every report of the slice and saves the result to
    /// `.foundry/reviews/<sliceId>.json`.
    ///
    /// Refused before anything runs, each refusal recorded as the slice's skip (see
    /// [`SkipReason`]) and sent as `verify:skipped`: `test_commands_unconfirmed` (unless there
    /// are no test commands, which need no confirmation), `not_found` (no report at all, or no
    /// such slice in the plan), `invalid_json` (a report that isn't one), `no_start_commit` or
    /// `setup_not_committed` (no valid start commit), `verification_running`. A bad slice id
    /// (`invalid_input`) or unknown project isn't recorded. A verification that runs clears
    /// the slice's skip; when one ends, the project's other skips are looked at again.
    pub fn verify(&self, project_id: &str, slice_id: &str) -> Result<Verification, AppError> {
        let result = self.verify_once(project_id, slice_id);
        match &result {
            Ok(_) => self.clear_skip(project_id, slice_id),
            Err(Refusal {
                reason: Some(reason),
                error,
            }) => self.record_skip(project_id, slice_id, *reason, error),
            Err(_) => {}
        }
        let busy = matches!(
            &result,
            Err(Refusal {
                reason: Some(SkipReason::AnotherVerificationRunning),
                ..
            })
        );
        if !busy {
            self.recheck_skips(project_id);
        }
        result.map_err(|refusal| refusal.error)
    }

    fn verify_once(&self, project_id: &str, slice_id: &str) -> Result<Verification, Refusal> {
        let Ready {
            repo,
            commands,
            reports,
            start_commit,
            expected,
            workspace_access,
        } = self.preflight(project_id, slice_id)?;

        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut running = lock(&self.running);
            if running.contains_key(project_id) {
                return Err(Refusal::skip(
                    SkipReason::AnotherVerificationRunning,
                    running_error(),
                ));
            }
            running.insert(project_id.to_string(), Arc::clone(&cancel));
        }
        let _running = Running {
            map: &self.running,
            project_id,
        };
        let started = Instant::now();
        let emit = |stage: VerifyStage, command: Option<&str>| {
            self.events.progress(VerifyProgress {
                meta: EventMeta::new(),
                project_id: project_id.to_string(),
                slice_id: slice_id.to_string(),
                stage,
                command: command.map(str::to_string),
                elapsed_ms: millis(started.elapsed()),
            });
        };
        emit(VerifyStage::Starting, None);
        tracing::info!(project_id, slice_id, "verification started");

        let checked = Checked {
            repo: &repo,
            slice_id,
            start_commit: Some(&start_commit),
            workspace_access,
            expected: expected.iter().map(String::as_str).collect(),
            reports: &reports,
            commands: &commands,
            cancel: &cancel,
        };
        let result = self.run_checks(&checked, &emit).and_then(
            |CheckOutcome {
                 findings,
                 runs,
                 requirements,
             }| {
                if cancel.load(Ordering::SeqCst) {
                    return Err(AppError::new(
                        ErrorCode::Cancelled,
                        "verification cancelled",
                    ));
                }
                let unreported = findings
                    .iter()
                    .any(|f| f.code == code::UNREPORTED_REQUIREMENT);
                let review = Review {
                    slice_id: slice_id.to_string(),
                    passed: !unreported
                        && findings
                            .iter()
                            .all(|f| f.severity != FindingSeverity::Danger),
                    findings,
                    test_runs: runs,
                    report: reports[0].value.clone(),
                    reports: reports
                        .iter()
                        .map(|l| VerifiedReport {
                            path: l.path.clone(),
                            agent: l.agent.clone(),
                            report: l.value.clone(),
                        })
                        .collect(),
                    requirements,
                    verified_at: now_ms(),
                };
                let text = serde_json::to_string_pretty(&review)
                    .map_err(|err| AppError::internal(format!("serialize review: {err}")))?;
                repo_write::write(&repo, &disk::review_path(slice_id), text + "\n")?;
                let stale = is_stale(&repo, &review)?;
                Ok(Verification { review, stale })
            },
        );
        match &result {
            Ok(done) => {
                emit(VerifyStage::Done, None);
                tracing::info!(
                    project_id,
                    slice_id,
                    passed = done.review.passed,
                    reports = done.review.reports.len(),
                    "verification saved"
                );
            }
            Err(err) if err.code == ErrorCode::Cancelled => {
                emit(VerifyStage::Cancelled, None);
                tracing::info!(project_id, slice_id, "verification cancelled");
            }
            Err(err) => {
                emit(VerifyStage::Failed, None);
                tracing::warn!(
                    project_id,
                    slice_id,
                    code = err.code.as_str(),
                    "verification failed"
                );
            }
        }
        result.map_err(Refusal::unrecorded)
    }

    fn run_checks(
        &self,
        c: &Checked<'_>,
        emit: &dyn Fn(VerifyStage, Option<&str>),
    ) -> Result<CheckOutcome, AppError> {
        use FindingSeverity::{Danger, Info, Warn};
        emit(VerifyStage::Checking, None);
        let mut findings = Vec::new();

        for l in c.reports {
            if l.report.slice_id != c.slice_id {
                findings.push(
                    Finding::new(
                        Warn,
                        code::SLICE_MISMATCH,
                        format!(
                            "{}'s report names slice {:?}, but it's a report file for {} ({})",
                            l.agent, l.report.slice_id, c.slice_id, l.path
                        ),
                    )
                    .agent(&l.agent)
                    .path(&l.path),
                );
            }
        }

        // 1. Together, the reports cover exactly the slice's requirements.
        let outcomes = outcomes(&c.expected, c.reports);
        for o in &outcomes {
            let expected = c.expected.contains(&o.id.as_str());
            if expected && o.reported_by.is_empty() {
                findings.push(
                    Finding::new(
                        Danger,
                        code::UNREPORTED_REQUIREMENT,
                        format!(
                            "requirement {} is in the plan for this slice but in no report",
                            o.id
                        ),
                    )
                    .requirement(&o.id),
                );
            } else if !expected {
                let mut agents: Vec<&str> =
                    o.reported_by.iter().map(|a| a.agent.as_str()).collect();
                agents.dedup();
                let mut finding = Finding::new(
                    Warn,
                    code::UNKNOWN_REQUIREMENT,
                    format!(
                        "{}'s report lists requirement {}, which the plan doesn't give this slice",
                        agents.join(" and "),
                        o.id
                    ),
                )
                .requirement(&o.id);
                if let [agent] = agents[..] {
                    finding = finding.agent(agent);
                }
                findings.push(finding);
            }
        }

        // 2. Met in one report and partly in another: met, with a note naming who said partly.
        for o in outcomes
            .iter()
            .filter(|o| o.status == Some(RequirementStatus::Met))
        {
            let met_by: Vec<&str> = o
                .reported_by
                .iter()
                .filter(|a| a.status == RequirementStatus::Met)
                .map(|a| a.agent.as_str())
                .collect();
            for partly in o
                .reported_by
                .iter()
                .filter(|a| a.status == RequirementStatus::Partly)
            {
                findings.push(
                    Finding::new(
                        Info,
                        code::PARTLY_REPORTED,
                        format!(
                            "requirement {} counts as met ({} says met), but {} says partly",
                            o.id,
                            met_by.join(" and "),
                            partly.agent
                        ),
                    )
                    .requirement(&o.id)
                    .agent(&partly.agent),
                );
            }
        }

        // 3. Every met requirement names its proof, in the report that says met.
        for l in c.reports {
            for r in l
                .report
                .requirements
                .iter()
                .filter(|r| r.status == RequirementStatus::Met)
            {
                if !names_test_or_command(&r.evidence, c.commands, &l.report) {
                    let what = if r.evidence.trim().is_empty() {
                        "no evidence"
                    } else {
                        "evidence that names no test or command"
                    };
                    findings.push(
                        Finding::new(
                            Warn,
                            code::MISSING_EVIDENCE,
                            format!(
                                "requirement {} is reported met by {} with {what}",
                                r.id, l.agent
                            ),
                        )
                        .requirement(&r.id)
                        .agent(&l.agent),
                    );
                }
            }
        }

        // 4. Every report's blockers.
        for l in c.reports {
            for blocker in l
                .report
                .blockers
                .iter()
                .map(|b| b.trim())
                .filter(|b| !b.is_empty())
            {
                findings.push(
                    Finding::new(
                        Warn,
                        code::REPORTED_BLOCKER,
                        format!("{} reported a blocker: {blocker}", l.agent),
                    )
                    .agent(&l.agent),
                );
            }
        }

        // 5. Run the confirmed test commands.
        emit(VerifyStage::RunningTests, None);
        let mut runs = Vec::new();
        if c.commands.is_empty() {
            findings.push(Finding::new(
                Warn,
                code::NO_TEST_COMMAND,
                "the project has no test commands, so nothing was run",
            ));
        }
        for command in c.commands {
            emit(VerifyStage::RunningTests, Some(command));
            let outcome = run::run(command, c.repo, self.timeout, c.cancel)?;
            if outcome.timed_out {
                findings.push(Finding::new(
                    Danger,
                    code::TEST_TIMED_OUT,
                    format!(
                        "`{command}` didn't finish within {} s and was stopped",
                        self.timeout.as_secs()
                    ),
                ));
            } else if !outcome.succeeded() {
                findings.push(Finding::new(
                    Danger,
                    code::TEST_FAILED,
                    match outcome.exit_code {
                        Some(exit) => format!("`{command}` failed (exit code {exit})"),
                        None => format!("`{command}` didn't run to completion"),
                    },
                ));
            }
            runs.push(CommandRun {
                command: command.clone(),
                exit_code: outcome.exit_code,
                duration_ms: millis(outcome.duration),
                tail: outcome.tail.clone(),
                timed_out: outcome.timed_out,
            });
        }

        // 6. Files changed since the slice started, against the reporting agents' globs.
        emit(VerifyStage::Checking, None);
        let mut agents: Vec<&str> = c.reports.iter().map(|l| l.agent.as_str()).collect();
        agents.sort_unstable();
        agents.dedup();
        findings.extend(self.boundary(
            c.repo,
            c.slice_id,
            c.start_commit,
            &agents,
            c.workspace_access,
        ));

        // 7. "Everything met" while a test failed.
        let all_met = outcomes.iter().any(|o| o.status.is_some())
            && outcomes
                .iter()
                .filter(|o| o.status.is_some())
                .all(|o| o.status == Some(RequirementStatus::Met));
        if all_met && runs.iter().any(|r| r.exit_code != Some(0) || r.timed_out) {
            findings.push(Finding::new(
                Danger,
                code::CONTRADICTION,
                "the reports say every requirement is met, but a test command failed",
            ));
        }
        Ok(CheckOutcome {
            findings,
            runs,
            requirements: outcomes,
        })
    }

    /// The boundary check. Candidates are the files changed since `start_commit`: committed,
    /// staged, unstaged and untracked (not ignored).
    ///
    /// When the writes ledger has sessions on this slice, only the slice's own paths (what its
    /// sessions' edit tools wrote, plus their snapshot changes nobody else can have made) are
    /// held to the rules, each against the agent that wrote it: protected for that agent is
    /// `danger`, outside its write globs (and not a workspace file, with `workspace_access`)
    /// `warn`. An own path that an overlapping session of another slice also wrote is
    /// `shared_file_touched` (`warn`); a snapshot change that can't be told apart is
    /// `ambiguous_shell_change` (`info`); a changed file only other slices' sessions wrote is
    /// `owned_by_other_slice` (`info`). Other changed files aren't the slice's: no finding.
    ///
    /// Without ledger sessions on this slice (a run from before the ledger), every changed
    /// file is checked as before against the reporting `agents` together, except that one only
    /// other slices' sessions wrote is `owned_by_other_slice`.
    fn boundary(
        &self,
        repo: &Path,
        slice_id: &str,
        start_commit: Option<&str>,
        agents: &[&str],
        workspace_access: bool,
    ) -> Vec<Finding> {
        use FindingSeverity::Warn;
        let skipped = |why: String| {
            vec![Finding::new(
                Warn,
                code::BOUNDARY_SKIPPED,
                format!("the boundary check was skipped: {why}"),
            )]
        };
        let Some(start) = start_commit.map(str::trim).filter(|s| !s.is_empty()) else {
            return skipped(
                "the slice has no start commit (not a git repo when it started?)".into(),
            );
        };
        if !is_commit_id(start) {
            return skipped(format!(
                "the slice's start commit {start:?} isn't a commit id"
            ));
        }
        let roles = match disk::roles(repo) {
            Ok(Some(roles)) => roles,
            Ok(None) => return skipped(format!("{} is missing or unreadable", disk::ROLES_FILE)),
            Err(err) => return skipped(err.message),
        };
        let sessions = match attribution::load(repo) {
            Ok(sessions) => sessions,
            Err(err) => {
                return skipped(format!("the writes ledger is unreadable: {}", err.message))
            }
        };
        let diff = [
            "--no-optional-locks",
            "diff",
            "--name-only",
            "--no-renames",
            "-z",
            start,
            "--",
        ];
        let others = [
            "--no-optional-locks",
            "ls-files",
            "--others",
            "--exclude-standard",
            "-z",
        ];
        let mut changed = BTreeSet::new();
        for args in [&diff[..], &others[..]] {
            match self.git.run(repo, args) {
                Ok(GitRun::Ok(output)) => changed.extend(
                    String::from_utf8_lossy(&output.stdout)
                        .split('\0')
                        .filter(|p| !p.is_empty())
                        .map(str::to_string),
                ),
                Ok(GitRun::Failed(output)) => {
                    return skipped(format!(
                        "git {} failed: {}",
                        args[1],
                        git::stderr_line(&output)
                    ))
                }
                Ok(GitRun::Missing) => return skipped("git isn't installed".into()),
                Err(err) => return skipped(format!("git couldn't run: {err}")),
            }
        }

        changed.retain(|p| checked_path(p));

        let rules = Rules {
            roles: &roles,
            workspace_access,
        };
        if sessions.iter().any(|s| s.on(slice_id)) {
            return attributed(&rules, slice_id, &sessions, &changed).unwrap_or_else(skipped);
        }
        let mut team = Vec::new();
        for agent in agents {
            let Some(role) = rules.role(agent) else {
                return skipped(format!("agent {agent:?} isn't in {}", disk::ROLES_FILE));
            };
            team.push(role);
        }
        legacy(&rules, &team, agents, &sessions, &changed)
    }

    // ---------- guidance ----------

    /// What to do after the slice's last verification, in plain words. Read-only: it never
    /// performs an action. Errors: `invalid_input` (bad slice id), `not_found` (project or
    /// slice), `folder_missing`, `invalid_json` (unreadable plan or review).
    pub fn guidance(
        &self,
        project_id: &str,
        slice_id: &str,
    ) -> Result<VerificationGuidance, AppError> {
        check_slice_id(slice_id)?;
        let repo = self.repos.repo(project_id)?;
        let plan = disk::plan(&repo)?;
        let slice = plan
            .slices
            .iter()
            .find(|s| s.id == slice_id)
            .ok_or_else(|| AppError::not_found(format!("no slice with id {slice_id}")))?;
        let verification = self.get_verification(project_id, slice_id)?;
        let skip = match (self.verify_skip(project_id, slice_id), &verification) {
            (Some(skip), _) => Some(skip),
            (None, None) => self.check_verify(project_id, slice_id)?,
            (None, Some(_)) => None,
        };
        let roles = disk::roles(&repo)?.unwrap_or_default();
        let sessions = attribution::load(&repo).unwrap_or_else(|err| {
            tracing::warn!(error = %err, "writes ledger unreadable; guidance without it");
            Vec::new()
        });
        let by_slice = slice_agents(&repo, &plan, &roles, &sessions)?;
        let on_slice: BTreeSet<String> = by_slice.get(slice_id).cloned().unwrap_or_default();
        let start = slice
            .start_commit
            .as_deref()
            .map(str::trim)
            .filter(|s| is_commit_id(s));
        let mut paths = BTreeMap::new();
        if let Some(v) = &verification {
            for path in v.review.findings.iter().filter_map(|f| {
                (f.code == code::PROTECTED_FILE_CHANGED || f.code == code::OUTSIDE_WRITE_GLOBS)
                    .then_some(f.path.as_deref())
                    .flatten()
            }) {
                let facts = guidance::PathFacts {
                    writer: sessions
                        .iter()
                        .find(|s| s.on(slice_id) && s.owns(path))
                        .map(|s| s.agent_id.clone()),
                    elsewhere: elsewhere(slice_id, path, &sessions, &roles, &by_slice),
                    changed_lines: start.and_then(|start| self.changed_lines(&repo, start, path)),
                };
                paths.insert(path.to_string(), facts);
            }
        }
        let (review, stale) = match verification {
            Some(v) => (Some(v.review), v.stale),
            None => (None, false),
        };
        Ok(guidance::build(&guidance::Inputs {
            slice_id: slice_id.to_string(),
            review,
            stale,
            skip,
            on_slice: on_slice.into_iter().collect(),
            reported: reported_agents(&repo, slice_id)?,
            names: roles
                .iter()
                .filter_map(|r| Some((r.id.clone(), r.display_name.clone()?)))
                .collect(),
            requirement_text: plan
                .requirements
                .iter()
                .map(|r| (r.id.clone(), r.text.clone()))
                .collect(),
            paths,
        }))
    }

    /// Lines added plus removed in `path` since `start` (an untracked file: its line count).
    /// `None` for a binary file or when git can't say.
    fn changed_lines(&self, repo: &Path, start: &str, path: &str) -> Option<u32> {
        let args = [
            "--no-optional-locks",
            "diff",
            "--numstat",
            "--no-renames",
            start,
            "--",
            path,
        ];
        let GitRun::Ok(output) = self.git.run(repo, &args).ok()? else {
            return None;
        };
        let text = String::from_utf8_lossy(&output.stdout);
        if let Some(line) = text.lines().next() {
            let mut cols = line.split('\t');
            let added: u32 = cols.next()?.parse().ok()?;
            let removed: u32 = cols.next()?.parse().ok()?;
            return Some(added + removed);
        }
        // Not in the diff: untracked (or unchanged).
        let file = crate::core::paths::resolve(repo, path).ok()?;
        match std::fs::read(file) {
            Ok(bytes) if bytes.contains(&0) => None,
            Ok(bytes) => {
                let lines = bytes
                    .split(|b| *b == b'\n')
                    .filter(|l| !l.is_empty())
                    .count();
                Some(u32::try_from(lines).unwrap_or(u32::MAX))
            }
            Err(_) => Some(0),
        }
    }

    // ---------- digest ----------

    /// The digest. Recorded skips are looked at again first, so each slice's `skip` is current.
    pub fn manager_digest(&self, project_id: &str) -> Result<ManagerDigest, AppError> {
        let repo = self.repos.repo(project_id)?;
        self.recheck_skips(project_id);
        digest::build(
            &repo,
            project_id,
            &self.sessions,
            &self.skips_of(project_id),
            now_ms(),
        )
    }

    fn skips_of(&self, project_id: &str) -> BTreeMap<String, VerifySkip> {
        lock(&self.skips)
            .get(project_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Writes the digest to `.foundry/reviews/digest.md` as markdown and returns it.
    pub fn write_digest_file(&self, project_id: &str) -> Result<ManagerDigest, AppError> {
        let repo = self.repos.repo(project_id)?;
        self.recheck_skips(project_id);
        let digest = digest::build(
            &repo,
            project_id,
            &self.sessions,
            &self.skips_of(project_id),
            now_ms(),
        )?;
        repo_write::write(&repo, disk::DIGEST_FILE, digest::markdown(&digest))?;
        tracing::info!(project_id, "digest written");
        Ok(digest)
    }
}

/// What `run_checks` found: the findings, the test command runs, and each requirement's
/// status across the reports.
struct CheckOutcome {
    findings: Vec<Finding>,
    runs: Vec<CommandRun>,
    requirements: Vec<RequirementOutcome>,
}

/// What one verification checks, gathered before it starts.
struct Checked<'a> {
    repo: &'a Path,
    slice_id: &'a str,
    start_commit: Option<&'a str>,
    /// The slice has `workspaceAccess`: the root workspace files are writable too.
    workspace_access: bool,
    /// The slice's requirement ids, from the plan, in plan order.
    expected: Vec<&'a str>,
    /// At least one.
    reports: &'a [LoadedReport],
    commands: &'a [String],
    cancel: &'a AtomicBool,
}

// ---------- guidance helpers ----------

/// Slice id → its agents: those with a task prompt for it (`.foundry/prompts/<sliceId>-
/// <agentId>.md`, the agent in `roles.json`) and those with a writes-ledger session on it.
/// A prompt name is matched to the longest slice id it starts with.
fn slice_agents(
    repo: &Path,
    plan: &disk::Plan,
    roles: &[disk::Role],
    sessions: &[attribution::Session],
) -> Result<BTreeMap<String, BTreeSet<String>>, AppError> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut slice_ids: Vec<&str> = plan.slices.iter().map(|s| s.id.as_str()).collect();
    slice_ids.sort_by_key(|id| std::cmp::Reverse(id.len()));
    let dir = crate::core::paths::resolve(repo, PROMPTS_DIR)?;
    let names: Vec<String> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(err) => return Err(err.into()),
    };
    for name in &names {
        let Some(stem) = name.strip_suffix(".md") else {
            continue;
        };
        let found = slice_ids.iter().find_map(|slice| {
            let agent = stem.strip_prefix(slice)?.strip_prefix('-')?;
            Some((*slice, agent))
        });
        if let Some((slice, agent)) = found {
            if roles.iter().any(|r| r.id == agent) {
                out.entry(slice.to_string())
                    .or_default()
                    .insert(agent.to_string());
            }
        }
    }
    for session in sessions {
        if let Some(slice) = &session.slice_id {
            out.entry(slice.clone())
                .or_default()
                .insert(session.agent_id.clone());
        }
    }
    Ok(out)
}

/// Agents with a report for the slice: from per-agent file names, and a legacy report's
/// `agent` field.
fn reported_agents(repo: &Path, slice_id: &str) -> Result<BTreeSet<String>, AppError> {
    let mut out = BTreeSet::new();
    for path in report::slice_report_paths(repo, slice_id)? {
        let name = path.rsplit('/').next().unwrap_or(&path);
        match report::parse_report_name(name) {
            Some((_, Some(agent))) => {
                out.insert(agent.to_string());
            }
            _ => {
                let agent = disk::read_json(repo, &path)
                    .ok()
                    .flatten()
                    .and_then(|v| v.get("agent")?.as_str().map(str::to_string));
                out.extend(agent);
            }
        }
    }
    Ok(out)
}

/// Who else `path` belongs to: another slice's session wrote it, or an agent that's only on
/// other slices may write it (inside its write globs, not protected for it) and none of this
/// slice's agents may.
fn elsewhere(
    slice_id: &str,
    path: &str,
    sessions: &[attribution::Session],
    roles: &[disk::Role],
    by_slice: &BTreeMap<String, BTreeSet<String>>,
) -> Option<String> {
    let writers: Vec<String> = sessions
        .iter()
        .filter(|s| !s.on(slice_id) && s.owns(path))
        .map(who)
        .collect();
    if !writers.is_empty() {
        return Some(joined(writers));
    }
    let mine = by_slice.get(slice_id);
    let in_globs = |agent: &str| {
        roles.iter().find(|r| r.id == agent).is_some_and(|r| {
            Rules::protected(r, path).is_none()
                && r.write_globs.iter().any(|g| glob::matches(g, path))
        })
    };
    if mine.is_some_and(|agents| agents.iter().any(|a| in_globs(a))) {
        return None;
    }
    let owners: Vec<String> = by_slice
        .iter()
        .filter(|(slice, _)| slice.as_str() != slice_id)
        .flat_map(|(slice, agents)| agents.iter().map(move |a| (slice, a)))
        .filter(|(_, agent)| !mine.is_some_and(|m| m.contains(*agent)) && in_globs(agent))
        .map(|(slice, agent)| format!("{agent}'s folders (slice {slice})"))
        .collect();
    (!owners.is_empty()).then(|| joined(owners))
}

// ---------- the boundary check ----------

/// Not one of Foundry's own files or folders, which the boundary check leaves out.
fn checked_path(path: &str) -> bool {
    !IGNORED_DIRS.iter().any(|dir| path.starts_with(dir)) && !IGNORED_FILES.contains(&path)
}

/// The team's rules, for the boundary check.
struct Rules<'a> {
    roles: &'a [disk::Role],
    workspace_access: bool,
}

impl Rules<'_> {
    fn role(&self, agent: &str) -> Option<&disk::Role> {
        self.roles.iter().find(|r| r.id == agent)
    }

    /// The protected-floor or role glob `path` matches for `role`, if any.
    fn protected<'r>(role: &'r disk::Role, path: &str) -> Option<&'r str> {
        hook_policy::PROTECTED_FLOOR
            .iter()
            .copied()
            .chain(role.protected_globs.iter().map(String::as_str))
            .find(|g| glob::matches(g, path))
    }

    /// Not protected for `role`, and inside its write globs (or, with `workspace_access`, one
    /// of the `hook_policy::WORKSPACE_FILES`).
    fn writable(&self, role: &disk::Role, path: &str) -> bool {
        let workspace_file = self.workspace_access && hook_policy::WORKSPACE_FILES.contains(&path);
        Self::protected(role, path).is_none()
            && (workspace_file || role.write_globs.iter().any(|g| glob::matches(g, path)))
    }
}

fn foundry_written(path: &str) -> Option<Finding> {
    FOUNDRY_WRITTEN.contains(&path).then(|| {
        Finding::new(
            FindingSeverity::Info,
            code::FOUNDRY_FILE_CHANGED,
            format!("{path} changed; Foundry rewrites it too, so this may not be the agent"),
        )
        .path(path)
    })
}

/// `ada on slice s-feed`, or `ada (no slice)`.
fn who(session: &attribution::Session) -> String {
    match &session.slice_id {
        Some(slice) => format!("{} on slice {slice}", session.agent_id),
        None => format!("{} (no slice)", session.agent_id),
    }
}

/// Distinct items, in first-seen order, joined with "and".
fn joined(items: impl IntoIterator<Item = String>) -> String {
    let mut seen: Vec<String> = Vec::new();
    for item in items {
        if !seen.contains(&item) {
            seen.push(item);
        }
    }
    seen.join(" and ")
}

/// `owned_by_other_slice` when sessions wrote `path` and none of them is on `slice_id`.
fn owned_by_others(
    slice_id: Option<&str>,
    sessions: &[attribution::Session],
    path: &str,
) -> Option<Finding> {
    let owners: Vec<&attribution::Session> = sessions.iter().filter(|s| s.owns(path)).collect();
    if owners.is_empty() || slice_id.is_some_and(|id| owners.iter().any(|s| s.on(id))) {
        return None;
    }
    Some(
        Finding::new(
            FindingSeverity::Info,
            code::OWNED_BY_OTHER_SLICE,
            format!(
                "{path} changed, but {} wrote it, not this slice",
                joined(owners.iter().map(|s| who(s)))
            ),
        )
        .path(path),
    )
}

/// The boundary check on the slice's own paths, for a slice with sessions in the ledger.
/// `Err` says why it was skipped.
fn attributed(
    rules: &Rules<'_>,
    slice_id: &str,
    sessions: &[attribution::Session],
    changed: &BTreeSet<String>,
) -> Result<Vec<Finding>, String> {
    use FindingSeverity::{Danger, Info, Warn};
    let own: Vec<&attribution::Session> = sessions.iter().filter(|s| s.on(slice_id)).collect();
    // Each own path → the own sessions that wrote it.
    let mut paths: BTreeMap<&str, Vec<&attribution::Session>> = BTreeMap::new();
    for session in &own {
        for path in session.written.iter().chain(&session.shell) {
            if checked_path(path) {
                paths.entry(path.as_str()).or_default().push(session);
            }
        }
    }
    let mut findings = Vec::new();
    for (path, writers) in &paths {
        if let Some(finding) = foundry_written(path) {
            findings.push(finding);
        } else {
            let mut protected = None;
            let mut outside = None;
            for writer in writers {
                let Some(role) = rules.role(&writer.agent_id) else {
                    return Err(format!(
                        "agent {:?} isn't in {}",
                        writer.agent_id,
                        disk::ROLES_FILE
                    ));
                };
                if let Some(hit) = Rules::protected(role, path) {
                    protected.get_or_insert((role.id.as_str(), hit));
                } else if !rules.writable(role, path) {
                    outside.get_or_insert(role.id.as_str());
                }
            }
            if let Some((agent, hit)) = protected {
                findings.push(
                    Finding::new(
                        Danger,
                        code::PROTECTED_FILE_CHANGED,
                        format!("{path} changed, and it's protected for {agent} (\"{hit}\")"),
                    )
                    .path(path),
                );
            } else if let Some(agent) = outside {
                findings.push(
                    Finding::new(
                        Warn,
                        code::OUTSIDE_WRITE_GLOBS,
                        format!("{agent} wrote {path}, outside {agent}'s write globs"),
                    )
                    .path(path),
                );
            }
        }
        let others: Vec<String> = sessions
            .iter()
            .filter(|other| !other.on(slice_id) && other.owns(path))
            .filter(|other| writers.iter().any(|mine| mine.overlaps(other)))
            .map(who)
            .collect();
        if !others.is_empty() {
            findings.push(
                Finding::new(
                    Warn,
                    code::SHARED_FILE_TOUCHED,
                    format!(
                        "{path} was written by {} and, at the same time, by {}",
                        joined(writers.iter().map(|s| s.agent_id.clone())),
                        joined(others)
                    ),
                )
                .path(path),
            );
        }
    }
    // Path → (own sessions it changed in, the other slices' sessions running then).
    let mut ambiguous: BTreeMap<&str, (Vec<&attribution::Session>, Vec<usize>)> = BTreeMap::new();
    for session in &own {
        for (path, rivals) in &session.ambiguous {
            if checked_path(path) && !paths.contains_key(path.as_str()) {
                let entry = ambiguous.entry(path.as_str()).or_default();
                entry.0.push(session);
                entry.1.extend(rivals);
            }
        }
    }
    for (path, (mine, rivals)) in &ambiguous {
        findings.push(
            Finding::new(
                Info,
                code::AMBIGUOUS_SHELL_CHANGE,
                format!(
                    "{path} changed during {}'s session without an edit tool (a shell \
                     command?) while {} was working too, so it can't be told whose change it is",
                    joined(mine.iter().map(|s| s.agent_id.clone())),
                    joined(rivals.iter().map(|&j| who(&sessions[j])))
                ),
            )
            .path(path),
        );
    }
    for path in changed
        .iter()
        .filter(|p| !paths.contains_key(p.as_str()) && !ambiguous.contains_key(p.as_str()))
    {
        findings.extend(owned_by_others(Some(slice_id), sessions, path));
    }
    Ok(findings)
}

/// The boundary check from before the ledger, for a slice with no sessions in it: every
/// changed file against the reporting agents together. A file one of them may write is fine;
/// otherwise one that only other slices' sessions wrote is `info`, one protected for any of
/// them `danger`, and one outside every write glob only `warn`, since others' edits in the
/// same period look the same.
fn legacy(
    rules: &Rules<'_>,
    team: &[&disk::Role],
    agents: &[&str],
    sessions: &[attribution::Session],
    changed: &BTreeSet<String>,
) -> Vec<Finding> {
    use FindingSeverity::{Danger, Warn};
    let who = agents.join(" and ");
    let mut findings = Vec::new();
    for path in changed {
        if let Some(finding) = foundry_written(path) {
            findings.push(finding);
            continue;
        }
        if team.iter().any(|role| rules.writable(role, path)) {
            continue;
        }
        // The slice has no session in the ledger, so every owner is another slice's.
        if let Some(finding) = owned_by_others(None, sessions, path) {
            findings.push(finding);
            continue;
        }
        let protected = team
            .iter()
            .find_map(|role| Rules::protected(role, path).map(|hit| (role.id.as_str(), hit)));
        let finding = if let Some((agent, hit)) = protected {
            Finding::new(
                Danger,
                code::PROTECTED_FILE_CHANGED,
                format!("{path} changed, and it's protected for {agent} (\"{hit}\")"),
            )
        } else {
            Finding::new(
                Warn,
                code::OUTSIDE_WRITE_GLOBS,
                format!(
                    "{path} changed outside {who}'s write globs (someone else may have \
                     changed it)"
                ),
            )
        };
        findings.push(finding.path(path));
    }
    findings
}

fn parse_report(rel: &str, value: Value) -> Result<SliceReport, AppError> {
    serde_json::from_value(value).map_err(|err| {
        AppError::new(
            ErrorCode::InvalidJson,
            format!("{rel} isn't a valid slice report ({err})"),
        )
    })
}
