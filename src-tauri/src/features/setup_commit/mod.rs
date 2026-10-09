//! Commits Foundry's own setup files, so the slice start gate (`setup_not_committed`) passes
//! without the user running git by hand.
//!
//! Only new or modified files under [`COMMIT_PATHS`] are staged and committed, both with
//! explicit literal pathspecs, so the user's other staged and unstaged changes stay exactly
//! as they were. Never pushes, never creates a branch, never amends. Runs under the session
//! registry's lock and refuses while the project has a live (or launching) agent session.

pub mod commands;

use std::path::Path;
use std::process::Output;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::core::git::{self, Git, GitRun};
use crate::core::repos::RepoIndex;
use crate::core::sessions::SessionRegistry;
use crate::core::{AppError, ErrorCode};

/// What `commit_setup` may stage and commit. Folders cover everything under them. Covers
/// every path the start gate checks (`core::setup`), plus `project.json` and the prompts.
pub const COMMIT_PATHS: &[&str] = &[
    "AGENTS.md",
    "agents",
    "docs",
    "vsm_viewer.html",
    "vsm_data.js",
    ".foundry/plan.json",
    ".foundry/project.json",
    ".foundry/prompts",
];

pub const COMMIT_MESSAGE: &str = "foundry setup";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitSetupResult {
    /// HEAD after the call: the new commit, or the current HEAD when there was nothing to
    /// commit (empty in a repo with no commits yet).
    pub commit: String,
    /// Repo-relative paths that were committed, sorted. Empty: nothing to commit.
    pub files: Vec<String>,
}

/// Share it as `Arc<SetupCommitter>`.
pub struct SetupCommitter {
    repos: Arc<RepoIndex>,
    sessions: Arc<SessionRegistry>,
    git: Git,
}

impl SetupCommitter {
    pub fn new(repos: Arc<RepoIndex>, sessions: Arc<SessionRegistry>, git: Git) -> Self {
        Self {
            repos,
            sessions,
            git,
        }
    }

    /// Errors: `not_found` (unknown project), `folder_missing`, `session_running` (an agent
    /// session is live or launching), `git_not_found`, `not_a_git_repo`,
    /// `git_identity_missing`, `commit_failed` (with git's message).
    pub fn commit_setup(&self, project_id: &str) -> Result<CommitSetupResult, AppError> {
        let repo = self.repos.repo(project_id)?;
        self.sessions.unless_project_active(
            project_id,
            "stop this project's agent sessions before committing the setup files",
            || self.commit_in(project_id, &repo),
        )
    }

    fn commit_in(&self, project_id: &str, repo: &Path) -> Result<CommitSetupResult, AppError> {
        self.require_work_tree(repo)?;
        let files = self.changed(repo)?;
        if files.is_empty() {
            tracing::info!(project_id, "setup commit: nothing to commit");
            return Ok(CommitSetupResult {
                commit: self.head(repo),
                files,
            });
        }
        self.require_identity(repo)?;

        let paths: Vec<&str> = files.iter().map(String::as_str).collect();
        let mut add = vec!["--literal-pathspecs", "add", "--"];
        add.extend(&paths);
        self.run_commit_step(repo, &add)?;
        let mut commit = vec![
            "--literal-pathspecs",
            "commit",
            "--quiet",
            "-m",
            COMMIT_MESSAGE,
            "--",
        ];
        commit.extend(&paths);
        self.run_commit_step(repo, &commit)?;

        let commit = self.head(repo);
        tracing::info!(project_id, commit = %commit, files = files.len(), "setup committed");
        Ok(CommitSetupResult { commit, files })
    }

    /// git's output, `Ok` when it exited 0. A missing git is `git_not_found`.
    fn run(&self, repo: &Path, args: &[&str]) -> Result<Result<Output, Output>, AppError> {
        match self.git.run(repo, args) {
            Ok(GitRun::Ok(output)) => Ok(Ok(output)),
            Ok(GitRun::Failed(output)) => Ok(Err(output)),
            Ok(GitRun::Missing) => Err(AppError::new(
                ErrorCode::GitNotFound,
                "git isn't installed (or isn't on PATH)",
            )),
            Err(err) => Err(AppError::internal(format!("could not run git: {err}"))),
        }
    }

    fn require_work_tree(&self, repo: &Path) -> Result<(), AppError> {
        let inside = match self.run(repo, &["rev-parse", "--is-inside-work-tree"])? {
            Ok(output) => String::from_utf8_lossy(&output.stdout).trim() == "true",
            Err(_) => false,
        };
        if inside {
            Ok(())
        } else {
            Err(AppError::new(
                ErrorCode::NotAGitRepo,
                format!(
                    "{} isn't a git repository. Run `git init` in it first",
                    repo.display()
                ),
            ))
        }
    }

    /// New (untracked or newly added) and modified files under [`COMMIT_PATHS`]. Deleted,
    /// ignored and conflicted files are left out.
    fn changed(&self, repo: &Path) -> Result<Vec<String>, AppError> {
        let mut args = vec![
            "--literal-pathspecs",
            "--no-optional-locks",
            "status",
            "--porcelain=v1",
            "-z",
            "--no-renames",
            "--untracked-files=all",
            "--",
        ];
        args.extend(COMMIT_PATHS);
        let output = match self.run(repo, &args)? {
            Ok(output) => output,
            Err(output) => {
                return Err(AppError::new(
                    ErrorCode::CommitFailed,
                    format!("git status failed: {}", git_message(&output)),
                ))
            }
        };
        let mut files: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .split('\0')
            .filter_map(|entry| Some((entry.get(..2)?, entry.get(3..)?)))
            .filter(|(state, _)| is_new_or_modified(state))
            .map(|(_, path)| path.to_string())
            .collect();
        files.sort();
        files.dedup();
        Ok(files)
    }

    fn require_identity(&self, repo: &Path) -> Result<(), AppError> {
        let missing: Vec<&str> = ["user.name", "user.email"]
            .into_iter()
            .filter(|key| match self.run(repo, &["config", "--get", key]) {
                Ok(Ok(output)) => String::from_utf8_lossy(&output.stdout).trim().is_empty(),
                _ => true,
            })
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        Err(AppError::new(
            ErrorCode::GitIdentityMissing,
            format!(
                "git doesn't know who you are ({} not set). Set it in a terminal, then try \
                 again: git config --global user.name \"Your Name\" and git config --global \
                 user.email \"you@example.com\"",
                missing.join(" and ")
            ),
        ))
    }

    fn run_commit_step(&self, repo: &Path, args: &[&str]) -> Result<(), AppError> {
        match self.run(repo, args)? {
            Ok(_) => Ok(()),
            Err(output) => {
                tracing::warn!(stderr = %git::stderr_line(&output), "setup commit failed");
                Err(AppError::new(
                    ErrorCode::CommitFailed,
                    format!(
                        "git couldn't commit the setup files: {}",
                        git_message(&output)
                    ),
                ))
            }
        }
    }

    /// HEAD's commit id; empty when there is none yet.
    fn head(&self, repo: &Path) -> String {
        match self
            .git
            .run(repo, &["rev-parse", "--verify", "--quiet", "HEAD"])
        {
            Ok(GitRun::Ok(output)) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
            _ => String::new(),
        }
    }
}

/// Porcelain v1 `XY`: untracked, or added/modified/type-changed in the index or the work tree,
/// and not deleted or unmerged.
fn is_new_or_modified(state: &str) -> bool {
    if state == "??" {
        return true;
    }
    let unmerged = state.contains('U') || state == "AA" || state == "DD";
    !unmerged && !state.contains('D') && state.chars().any(|c| matches!(c, 'M' | 'A' | 'T'))
}

/// git's whole stderr (or stdout, which some failures use), trimmed.
fn git_message(output: &Output) -> String {
    let text = if output.stderr.iter().all(u8::is_ascii_whitespace) {
        &output.stdout
    } else {
        &output.stderr
    };
    let text = String::from_utf8_lossy(text).trim().to_string();
    if text.is_empty() {
        format!("git exited with {}", output.status)
    } else {
        text
    }
}

#[cfg(test)]
mod tests;
