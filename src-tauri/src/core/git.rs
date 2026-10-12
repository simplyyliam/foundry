//! Running the `git` CLI. Callers decide what a failure means; nothing here panics.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use super::{AppError, ErrorCode};

/// How to invoke git: which program, plus environment overrides. `Git::system()` in the app;
/// tests point it at a missing program or an isolated config to control identity.
#[derive(Debug, Clone)]
pub struct Git {
    program: OsString,
    envs: Vec<(OsString, Option<OsString>)>,
}

/// Result of one git invocation.
#[derive(Debug)]
pub enum GitRun {
    /// git ran and exited 0.
    Ok(Output),
    /// git ran and exited non-zero (e.g. no identity configured for a commit).
    Failed(Output),
    /// The git program couldn't be found.
    Missing,
}

impl GitRun {
    pub fn succeeded(&self) -> bool {
        matches!(self, GitRun::Ok(_))
    }
}

impl Git {
    /// `git` from PATH with the user's own config.
    pub fn system() -> Self {
        Self::new("git")
    }

    pub fn new(program: impl Into<OsString>) -> Self {
        Self {
            program: program.into(),
            envs: Vec::new(),
        }
    }

    /// Sets an environment variable for every invocation.
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.envs
            .push((key.as_ref().to_owned(), Some(value.as_ref().to_owned())));
        self
    }

    /// Removes an inherited environment variable for every invocation.
    pub fn env_remove(mut self, key: impl AsRef<OsStr>) -> Self {
        self.envs.push((key.as_ref().to_owned(), None));
        self
    }

    /// Runs `git <args>` in `dir`. Never reads stdin, so a prompt can't hang it.
    pub fn run(&self, dir: &Path, args: &[&str]) -> io::Result<GitRun> {
        let mut command = Command::new(&self.program);
        command
            .args(args)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in &self.envs {
            match value {
                Some(value) => command.env(key, value),
                None => command.env_remove(key),
            };
        }
        hide_console_window(&mut command);

        match command.output() {
            Ok(output) if output.status.success() => Ok(GitRun::Ok(output)),
            Ok(output) => Ok(GitRun::Failed(output)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(GitRun::Missing),
            Err(err) => Err(err),
        }
    }
}

/// A GUI app spawning a console program flashes a console window on Windows unless told not to.
fn hide_console_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}

/// The first line of git's stderr, for logs. Never includes file contents.
pub fn stderr_line(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .trim()
        .to_string()
}

/// git's output, `Ok` when it exited 0. A missing git is `git_not_found`.
pub fn run_checked(
    git: &Git,
    repo: &Path,
    args: &[&str],
) -> Result<Result<Output, Output>, AppError> {
    match git.run(repo, args) {
        Ok(GitRun::Ok(output)) => Ok(Ok(output)),
        Ok(GitRun::Failed(output)) => Ok(Err(output)),
        Ok(GitRun::Missing) => Err(AppError::new(
            ErrorCode::GitNotFound,
            "git isn't installed (or isn't on PATH)",
        )),
        Err(err) => Err(AppError::internal(format!("could not run git: {err}"))),
    }
}

/// `not_a_git_repo` unless `repo` is inside a git work tree.
pub fn require_work_tree(git: &Git, repo: &Path) -> Result<(), AppError> {
    let inside = match run_checked(git, repo, &["rev-parse", "--is-inside-work-tree"])? {
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

/// `git_identity_missing` unless git has a `user.name` and a `user.email` for `repo`. Says
/// how to set them; never sets them.
pub fn require_identity(git: &Git, repo: &Path) -> Result<(), AppError> {
    let missing: Vec<&str> = ["user.name", "user.email"]
        .into_iter()
        .filter(
            |key| match run_checked(git, repo, &["config", "--get", key]) {
                Ok(Ok(output)) => String::from_utf8_lossy(&output.stdout).trim().is_empty(),
                _ => true,
            },
        )
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

/// HEAD's commit id; empty when there is none yet (or git fails).
pub fn head(git: &Git, repo: &Path) -> String {
    match git.run(repo, &["rev-parse", "--verify", "--quiet", "HEAD"]) {
        Ok(GitRun::Ok(output)) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        _ => String::new(),
    }
}

/// git's whole stderr (or stdout, which some failures use), trimmed.
pub fn message(output: &Output) -> String {
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
mod tests {
    use super::*;
    use crate::testing::TestRepo;

    #[test]
    fn missing_program_is_reported_not_an_error() {
        let repo = TestRepo::new();
        let git = Git::new("foundry-definitely-not-git");
        assert!(matches!(
            git.run(repo.path(), &["--version"]).unwrap(),
            GitRun::Missing
        ));
    }

    #[test]
    fn runs_git_and_separates_success_from_failure() {
        let repo = TestRepo::new();
        let git = Git::system();
        assert!(git.run(repo.path(), &["--version"]).unwrap().succeeded());
        let failed = git.run(repo.path(), &["no-such-subcommand"]).unwrap();
        assert!(matches!(failed, GitRun::Failed(_)));
    }

    #[test]
    fn env_overrides_apply() {
        let repo = TestRepo::new();
        let git = Git::system().env("GIT_CONFIG_PARAMETERS", "'foundry.probe=yes'");
        let GitRun::Ok(output) = git
            .run(repo.path(), &["config", "--get", "foundry.probe"])
            .unwrap()
        else {
            panic!("git config failed");
        };
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "yes");
    }
}
