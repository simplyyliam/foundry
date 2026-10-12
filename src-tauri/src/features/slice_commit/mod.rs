//! Committing from the app, so the user never needs a code editor or a terminal to commit:
//! `commit_status` shows every uncommitted file, `commit_slice` stages them all
//! (`git add -A`, which respects `.gitignore`) and commits them with a message built from the
//! slice. Committing is the user's own explicit click, so agent work is included, in any
//! slice state (a multi-agent slice is committed between agents). Never pushes, never
//! creates a branch, never amends. Commits run under the session registry's lock, like
//! `commit_setup`, and are refused while the project has a live (or launching) session.

pub mod commands;
mod disk;
mod message;

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::core::git::{self, Git};
use crate::core::repos::RepoIndex;
use crate::core::sessions::SessionRegistry;
use crate::core::{setup, AppError, ErrorCode};

pub use message::{commit_type, CommitTypeChoice};

/// At most this many files in `commit_status`; `truncated` says when there were more.
pub const MAX_STATUS_FILES: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum FileChange {
    New,
    Modified,
    Deleted,
    Renamed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    /// Repo-relative, `/`-separated. For a rename, the new path.
    pub path: String,
    pub change: FileChange,
    /// A setup file (what `commit_setup` commits), a report (`.foundry/reports/**`) or a
    /// verification (`.foundry/reviews/*.json`).
    pub foundry_owned: bool,
}

/// The conventional-commit type of a slice commit, picked by fixed rules (never by a model).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CommitType {
    Feat,
    Fix,
    Refactor,
    Test,
    Docs,
    Chore,
}

impl CommitType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feat => "feat",
            Self::Fix => "fix",
            Self::Refactor => "refactor",
            Self::Test => "test",
            Self::Docs => "docs",
            Self::Chore => "chore",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitStatus {
    /// HEAD's commit id; empty before the first commit.
    pub head: String,
    /// The checked-out branch; empty when HEAD is detached.
    pub branch: String,
    /// Every uncommitted file (staged, unstaged or untracked; ignored ones never), sorted by
    /// path, at most [`MAX_STATUS_FILES`].
    pub files: Vec<ChangedFile>,
    /// How many uncommitted files there are in all.
    pub total: u32,
    /// `files` was cut at [`MAX_STATUS_FILES`].
    pub truncated: bool,
    /// The type `commit_slice` would pick for `sliceId` now; `null` without a `sliceId` or
    /// with nothing to commit.
    pub commit_type: Option<CommitType>,
    /// Why that type, in one line; `null` when `commitType` is.
    pub type_reason: Option<String>,
    /// The message `commit_slice` would generate for `sliceId` now (no caller's message);
    /// `null` when `commitType` is.
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitSliceResult {
    /// HEAD after the call: the new commit, or the current HEAD when there was nothing to
    /// commit (empty in a repo with no commits yet).
    pub commit: String,
    /// Repo-relative paths in the commit (deleted ones too), sorted. Empty: nothing to
    /// commit.
    pub files: Vec<String>,
    /// The commit message used; empty when nothing was committed.
    pub message: String,
    /// The type Foundry picked; `null` when the caller's message was used or nothing was
    /// committed.
    pub commit_type: Option<CommitType>,
    /// Why that type, in one line; `null` when `commitType` is.
    pub type_reason: Option<String>,
}

/// Share it as `Arc<SliceCommitter>`.
pub struct SliceCommitter {
    repos: Arc<RepoIndex>,
    sessions: Arc<SessionRegistry>,
    git: Git,
}

impl SliceCommitter {
    pub fn new(repos: Arc<RepoIndex>, sessions: Arc<SessionRegistry>, git: Git) -> Self {
        Self {
            repos,
            sessions,
            git,
        }
    }

    /// Every uncommitted file in the repo. Read-only. With a `slice_id`, also the type and
    /// message `commit_slice` would generate. Errors: `not_found` (unknown project or slice),
    /// `folder_missing`, `invalid_json` (unreadable plan), `git_not_found`, `not_a_git_repo`,
    /// `commit_failed` (git status failed).
    pub fn commit_status(
        &self,
        project_id: &str,
        slice_id: Option<&str>,
    ) -> Result<CommitStatus, AppError> {
        let repo = self.repos.repo(project_id)?;
        let slice = match slice_id {
            Some(id) => Some(disk::slice(&repo, id)?),
            None => None,
        };
        git::require_work_tree(&self.git, &repo)?;
        let all = self.changed(&repo)?;
        let generated = match (&slice, all.is_empty()) {
            (Some(slice), false) => Some(generated(&repo, slice, &all)?),
            _ => None,
        };
        let (choice, message) = generated.unzip();
        let total = all.len();
        Ok(CommitStatus {
            head: git::head(&self.git, &repo),
            branch: self.branch(&repo),
            files: all.into_iter().take(MAX_STATUS_FILES).collect(),
            total: u32::try_from(total).unwrap_or(u32::MAX),
            truncated: total > MAX_STATUS_FILES,
            commit_type: choice.as_ref().map(|c| c.commit_type),
            type_reason: choice.map(|c| c.reason),
            message,
        })
    }

    /// Stages everything (`git add -A`) and commits it, with `message` as given or one built
    /// from the slice. Nothing to commit is not an error: `files` is empty and `commit` is
    /// the current HEAD. Errors: `not_found` (unknown project or slice), `folder_missing`,
    /// `invalid_json` (unreadable plan), `session_running`, `git_not_found`,
    /// `not_a_git_repo`, `git_identity_missing`, `commit_failed` (with git's message).
    pub fn commit_slice(
        &self,
        project_id: &str,
        slice_id: &str,
        message: Option<&str>,
    ) -> Result<CommitSliceResult, AppError> {
        let repo = self.repos.repo(project_id)?;
        self.sessions.unless_project_active(
            project_id,
            "stop this project's agent sessions before committing",
            || self.commit_in(project_id, slice_id, message, &repo),
        )
    }

    fn commit_in(
        &self,
        project_id: &str,
        slice_id: &str,
        message: Option<&str>,
        repo: &Path,
    ) -> Result<CommitSliceResult, AppError> {
        let slice = disk::slice(repo, slice_id)?;
        git::require_work_tree(&self.git, repo)?;
        let changed = self.changed(repo)?;
        if changed.is_empty() {
            tracing::info!(project_id, slice_id, "slice commit: nothing to commit");
            return Ok(CommitSliceResult {
                commit: git::head(&self.git, repo),
                files: Vec::new(),
                message: String::new(),
                commit_type: None,
                type_reason: None,
            });
        }
        git::require_identity(&self.git, repo)?;

        let (text, choice) = match message.filter(|m| !m.trim().is_empty()) {
            Some(given) => (given.to_string(), None),
            None => {
                let (choice, text) = generated(repo, &slice, &changed)?;
                (text, Some(choice))
            }
        };

        self.commit_step(repo, &["add", "-A"])?;
        // `--cleanup=verbatim`: the message lands exactly as given (no comment stripping).
        self.commit_step(
            repo,
            &["commit", "--quiet", "--cleanup=verbatim", "-m", &text],
        )?;

        let commit = git::head(&self.git, repo);
        let files = self.committed_files(repo);
        tracing::info!(project_id, slice_id, commit = %commit, files = files.len(), "slice committed");
        Ok(CommitSliceResult {
            commit,
            files,
            message: text,
            commit_type: choice.as_ref().map(|c| c.commit_type),
            type_reason: choice.map(|c| c.reason),
        })
    }

    /// Every uncommitted, non-ignored file, sorted by path.
    fn changed(&self, repo: &Path) -> Result<Vec<ChangedFile>, AppError> {
        let args = [
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
        ];
        let output = match git::run_checked(&self.git, repo, &args)? {
            Ok(output) => output,
            Err(output) => {
                return Err(AppError::new(
                    ErrorCode::CommitFailed,
                    format!("git status failed: {}", git::message(&output)),
                ))
            }
        };
        let mut files = parse_status(&String::from_utf8_lossy(&output.stdout));
        files.sort_by(|a, b| a.path.cmp(&b.path));
        files.dedup_by(|a, b| a.path == b.path);
        Ok(files)
    }

    /// The checked-out branch; empty when detached. Works before the first commit too.
    fn branch(&self, repo: &Path) -> String {
        match git::run_checked(
            &self.git,
            repo,
            &["symbolic-ref", "--quiet", "--short", "HEAD"],
        ) {
            Ok(Ok(output)) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
            _ => String::new(),
        }
    }

    /// The paths in HEAD's commit, sorted. `--root` covers a first commit.
    fn committed_files(&self, repo: &Path) -> Vec<String> {
        let args = [
            "diff-tree",
            "--no-commit-id",
            "--name-only",
            "-r",
            "-z",
            "--no-renames",
            "--root",
            "HEAD",
        ];
        let mut files: Vec<String> = match git::run_checked(&self.git, repo, &args) {
            Ok(Ok(output)) => String::from_utf8_lossy(&output.stdout)
                .split('\0')
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect(),
            _ => Vec::new(),
        };
        files.sort();
        files
    }

    fn commit_step(&self, repo: &Path, args: &[&str]) -> Result<(), AppError> {
        match git::run_checked(&self.git, repo, args)? {
            Ok(_) => Ok(()),
            Err(output) => {
                tracing::warn!(stderr = %git::stderr_line(&output), "slice commit failed");
                Err(AppError::new(
                    ErrorCode::CommitFailed,
                    format!("git couldn't commit: {}", git::message(&output)),
                ))
            }
        }
    }
}

/// The type and message Foundry generates for `slice` over `changed`. The one builder both
/// `commit_status` and `commit_slice` use, so the preview is the commit. Reads only.
fn generated(
    repo: &Path,
    slice: &disk::SliceInfo,
    changed: &[ChangedFile],
) -> Result<(CommitTypeChoice, String), AppError> {
    let choice = commit_type(&slice.name, slice.first_deliverable(), changed);
    let facts = disk::message_facts(repo, slice)?;
    let text = message::build(choice.commit_type, slice, &facts);
    Ok((choice, text))
}

/// `git status --porcelain=v1 -z` → files. A rename or copy entry is followed by its old
/// path, which is skipped.
fn parse_status(text: &str) -> Vec<ChangedFile> {
    let mut out = Vec::new();
    let mut entries = text.split('\0');
    while let Some(entry) = entries.next() {
        let (Some(state), Some(path)) = (entry.get(..2), entry.get(3..)) else {
            continue;
        };
        let change = if state == "??" {
            FileChange::New
        } else if state.contains(['R', 'C']) {
            entries.next();
            FileChange::Renamed
        } else if state.contains('D') && !state.contains('U') {
            FileChange::Deleted
        } else if state.starts_with('A') {
            FileChange::New
        } else {
            FileChange::Modified
        };
        out.push(ChangedFile {
            path: path.to_string(),
            change,
            foundry_owned: is_foundry_owned(path),
        });
    }
    out
}

/// A setup path, a report (`.foundry/reports/**`) or a verification (`.foundry/reviews/*.json`).
pub fn is_foundry_owned(path: &str) -> bool {
    setup::is_commit_path(path)
        || path.starts_with(".foundry/reports/")
        || path
            .strip_prefix(".foundry/reviews/")
            .is_some_and(|name| !name.contains('/') && name.ends_with(".json"))
}

#[cfg(test)]
mod tests;
