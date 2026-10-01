//! Running the `git` CLI. Callers decide what a failure means; nothing here panics.

use std::ffi::{OsStr, OsString};
use std::io;
use std::path::Path;
use std::process::{Command, Output, Stdio};

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
