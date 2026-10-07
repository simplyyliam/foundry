//! Slice reports, verification and the manager digest (P9).
//!
//! Agents only write `.foundry/reports/<sliceId>.json` (`core::report`). `verify_slice` checks
//! that report against the plan, runs the project's confirmed test commands, compares the
//! files changed since the slice started with the agent's globs, and saves the result to
//! `.foundry/reviews/<sliceId>.json`. Everything else here only reads.
//!
//! Test commands come from `.foundry/project.json`, which an agent might edit, and run with
//! the human's rights. So `verify_slice` runs them only when they equal, exactly, the list the
//! human last confirmed, which is kept in app data, never in the repo.

pub mod commands;
mod digest;
mod disk;
mod run;
#[cfg(test)]
mod tests;

use std::collections::{BTreeSet, HashMap};
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
use crate::core::report::{report_path, RequirementStatus, SliceReport};
use crate::core::repos::RepoIndex;
use crate::core::sessions::{now_ms, SessionRegistry};
use crate::core::{glob, hook_policy, repo_write, AppError, ErrorCode};

pub use digest::{
    AgentDigest, AttentionItem, AttentionKind, ManagerDigest, SliceDigest, VerificationState,
};

/// Each test command gets this long before its process tree is killed.
pub const TEST_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Left out of the boundary check: `project.json`, because changing the test commands
/// through Foundry rewrites it after a slice's start commit, and an agent's edit of them can't
/// run anyway (the confirmation gate).
const IGNORED_FILES: &[&str] = &[".foundry/project.json"];

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
}

impl Finding {
    fn new(severity: FindingSeverity, code: &str, message: impl Into<String>) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            requirement_id: None,
            path: None,
        }
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

/// `.foundry/reviews/<sliceId>.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Review {
    pub slice_id: String,
    /// No `danger` finding and no unreported requirement.
    pub passed: bool,
    pub findings: Vec<Finding>,
    pub test_runs: Vec<CommandRun>,
    /// The exact report that was verified, as it was on disk.
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
    /// The report on disk no longer equals the one verified (or is gone).
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

/// Where progress events go: the app emits them, tests record them.
pub trait VerifyEvents: Send + Sync + 'static {
    fn progress(&self, event: VerifyProgress);
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
/// backticks, or contains one of the test commands (confirmed, or run by the agent).
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

/// The report on disk as JSON, `None` when it's missing or doesn't parse.
fn current_report(repo: &Path, slice_id: &str) -> Result<Option<Value>, AppError> {
    disk::read_json_lenient(repo, &report_path(slice_id))
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
        self.test_commands_locked(project_id, &repo)
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
        self.test_commands_locked(project_id, &repo)
    }

    // ---------- reports and verifications ----------

    /// The slice's report, or `None` if the agent hasn't written one. `invalid_json` when it
    /// doesn't parse as a report.
    pub fn get_report(
        &self,
        project_id: &str,
        slice_id: &str,
    ) -> Result<Option<SliceReport>, AppError> {
        check_slice_id(slice_id)?;
        let repo = self.repos.repo(project_id)?;
        let rel = report_path(slice_id);
        disk::read_json(&repo, &rel)?
            .map(|value| parse_report(&rel, value))
            .transpose()
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
        let stale = current_report(&repo, slice_id)?.as_ref() != Some(&review.report);
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

    /// Verifies the slice's report and saves the result to `.foundry/reviews/<sliceId>.json`.
    ///
    /// Refused before anything runs: `test_commands_unconfirmed` (unless there are no test
    /// commands, which need no confirmation), `not_found` (no report, or
    /// no such slice in the plan), `invalid_json` (a report that isn't one),
    /// `verification_running`.
    pub fn verify(&self, project_id: &str, slice_id: &str) -> Result<Verification, AppError> {
        check_slice_id(slice_id)?;
        let repo = self.repos.repo(project_id)?;
        let commands = {
            let _guard = lock(&self.commands_lock);
            let current = self.test_commands_locked(project_id, &repo)?;
            if !current.confirmed {
                return Err(AppError::new(
                    ErrorCode::TestCommandsUnconfirmed,
                    "the project's test commands changed or were never confirmed: review and \
                     confirm them before verifying",
                ));
            }
            current.commands
        };
        let rel = report_path(slice_id);
        let report_value = disk::read_json(&repo, &rel)?.ok_or_else(|| {
            AppError::not_found(format!("no report for slice {slice_id} ({rel})"))
        })?;
        let report = parse_report(&rel, report_value.clone())?;
        let plan = disk::plan(&repo)?;
        let slice = plan
            .slices
            .iter()
            .find(|s| s.id == slice_id)
            .ok_or_else(|| AppError::not_found(format!("no slice with id {slice_id}")))?;

        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut running = lock(&self.running);
            if running.contains_key(project_id) {
                return Err(AppError::new(
                    ErrorCode::VerificationRunning,
                    "a verification is already running for this project",
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
            start_commit: slice.start_commit.as_deref(),
            expected: plan
                .requirements
                .iter()
                .filter(|r| r.slice_id.as_deref() == Some(slice_id))
                .map(|r| r.id.as_str())
                .collect(),
            report: &report,
            commands: &commands,
            cancel: &cancel,
        };
        let result = self
            .run_checks(&checked, &emit)
            .and_then(|(findings, runs)| {
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
                    report: report_value,
                    verified_at: now_ms(),
                };
                let text = serde_json::to_string_pretty(&review)
                    .map_err(|err| AppError::internal(format!("serialize review: {err}")))?;
                repo_write::write(&repo, &disk::review_path(slice_id), text + "\n")?;
                let stale = current_report(&repo, slice_id)?.as_ref() != Some(&review.report);
                Ok(Verification { review, stale })
            });
        match &result {
            Ok(done) => {
                emit(VerifyStage::Done, None);
                tracing::info!(
                    project_id,
                    slice_id,
                    passed = done.review.passed,
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
        result
    }

    fn run_checks(
        &self,
        c: &Checked<'_>,
        emit: &dyn Fn(VerifyStage, Option<&str>),
    ) -> Result<(Vec<Finding>, Vec<CommandRun>), AppError> {
        use FindingSeverity::{Danger, Warn};
        emit(VerifyStage::Checking, None);
        let mut findings = Vec::new();
        let report = c.report;

        if report.slice_id != c.slice_id {
            findings.push(Finding::new(
                Warn,
                code::SLICE_MISMATCH,
                format!(
                    "the report names slice {:?}, but it's the report file for {}",
                    report.slice_id, c.slice_id
                ),
            ));
        }

        // 1. The report covers exactly the slice's requirements.
        let reported: BTreeSet<&str> = report.requirements.iter().map(|r| r.id.as_str()).collect();
        for id in c.expected.iter().filter(|id| !reported.contains(*id)) {
            findings.push(
                Finding::new(
                    Danger,
                    code::UNREPORTED_REQUIREMENT,
                    format!("requirement {id} is in the plan for this slice but not in the report"),
                )
                .requirement(id),
            );
        }
        for id in reported.iter().filter(|id| !c.expected.contains(*id)) {
            findings.push(
                Finding::new(
                    Warn,
                    code::UNKNOWN_REQUIREMENT,
                    format!(
                        "the report lists requirement {id}, which the plan doesn't give this slice"
                    ),
                )
                .requirement(id),
            );
        }

        // 2. Every met requirement names its proof.
        for r in report
            .requirements
            .iter()
            .filter(|r| r.status == RequirementStatus::Met)
        {
            if !names_test_or_command(&r.evidence, c.commands, report) {
                let what = if r.evidence.trim().is_empty() {
                    "no evidence"
                } else {
                    "evidence that names no test or command"
                };
                findings.push(
                    Finding::new(
                        Warn,
                        code::MISSING_EVIDENCE,
                        format!("requirement {} is reported met with {what}", r.id),
                    )
                    .requirement(&r.id),
                );
            }
        }

        // 3. Run the confirmed test commands.
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

        // 4. Files changed since the slice started, against the agent's globs.
        emit(VerifyStage::Checking, None);
        findings.extend(self.boundary(c.repo, c.start_commit, &report.agent));

        // 5. "Everything met" while a test failed.
        let all_met = !report.requirements.is_empty()
            && report
                .requirements
                .iter()
                .all(|r| r.status == RequirementStatus::Met);
        if all_met && runs.iter().any(|r| r.exit_code != Some(0) || r.timed_out) {
            findings.push(Finding::new(
                Danger,
                code::CONTRADICTION,
                "the report says every requirement is met, but a test command failed",
            ));
        }
        Ok((findings, runs))
    }

    /// Changed files since `start_commit`: committed, staged, unstaged and untracked (not
    /// ignored). A protected one is `danger`; one outside the write globs only `warn`, since
    /// others' edits in the same period look the same.
    fn boundary(&self, repo: &Path, start_commit: Option<&str>, agent: &str) -> Vec<Finding> {
        use FindingSeverity::{Danger, Info, Warn};
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
        let Some(role) = roles.iter().find(|r| r.id == agent) else {
            return skipped(format!("agent {agent:?} isn't in {}", disk::ROLES_FILE));
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

        let mut findings = Vec::new();
        for path in changed
            .iter()
            .filter(|p| !IGNORED_DIRS.iter().any(|dir| p.starts_with(dir)))
            .filter(|p| !IGNORED_FILES.contains(&p.as_str()))
        {
            let protected = hook_policy::PROTECTED_FLOOR
                .iter()
                .copied()
                .chain(role.protected_globs.iter().map(String::as_str))
                .find(|g| glob::matches(g, path));
            let finding = if FOUNDRY_WRITTEN.contains(&path.as_str()) {
                Finding::new(
                    Info,
                    code::FOUNDRY_FILE_CHANGED,
                    format!(
                        "{path} changed; Foundry rewrites it too, so this may not be the agent"
                    ),
                )
            } else if let Some(hit) = protected {
                Finding::new(
                    Danger,
                    code::PROTECTED_FILE_CHANGED,
                    format!("{path} changed, and it's protected for {agent} (\"{hit}\")"),
                )
            } else if !role.write_globs.iter().any(|g| glob::matches(g, path)) {
                Finding::new(
                    Warn,
                    code::OUTSIDE_WRITE_GLOBS,
                    format!(
                        "{path} changed outside {agent}'s write globs (someone else may have \
                         changed it)"
                    ),
                )
            } else {
                continue;
            };
            findings.push(finding.path(path));
        }
        findings
    }

    // ---------- digest ----------

    pub fn manager_digest(&self, project_id: &str) -> Result<ManagerDigest, AppError> {
        let repo = self.repos.repo(project_id)?;
        digest::build(&repo, project_id, &self.sessions, now_ms())
    }

    /// Writes the digest to `.foundry/reviews/digest.md` as markdown and returns it.
    pub fn write_digest_file(&self, project_id: &str) -> Result<ManagerDigest, AppError> {
        let repo = self.repos.repo(project_id)?;
        let digest = digest::build(&repo, project_id, &self.sessions, now_ms())?;
        repo_write::write(&repo, disk::DIGEST_FILE, digest::markdown(&digest))?;
        tracing::info!(project_id, "digest written");
        Ok(digest)
    }
}

/// What one verification checks, gathered before it starts.
struct Checked<'a> {
    repo: &'a Path,
    slice_id: &'a str,
    start_commit: Option<&'a str>,
    /// The slice's requirement ids, from the plan.
    expected: BTreeSet<&'a str>,
    report: &'a SliceReport,
    commands: &'a [String],
    cancel: &'a AtomicBool,
}

fn parse_report(rel: &str, value: Value) -> Result<SliceReport, AppError> {
    serde_json::from_value(value).map_err(|err| {
        AppError::new(
            ErrorCode::InvalidJson,
            format!("{rel} isn't a valid slice report ({err})"),
        )
    })
}
