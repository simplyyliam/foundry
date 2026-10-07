//! The `claude` CLI: finding it, checking it's signed in with a subscription, and running it
//! with a deadline and a cancel flag. Shared by every feature that runs Claude Code.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::process_tree::ProcessTree;
use super::subscription::subscription_env;
use super::{AppError, ErrorCode};

/// How long `claude auth status` / `--version` may take.
const QUICK_TIMEOUT: Duration = Duration::from_secs(30);
const POLL: Duration = Duration::from_millis(25);
/// What `claude auth status` reports for a Claude subscription login.
const SUBSCRIPTION_METHOD: &str = "claude.ai";
const FIRST_PARTY: &str = "firstParty";

/// What the UI shows about the CLI. Never includes the account's email or organisation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ClaudeStatus {
    /// The executable Foundry would run (`claudePath`, or the first match on PATH).
    pub path: Option<String>,
    pub version: Option<String>,
    pub logged_in: bool,
    /// `claude.ai` for a subscription login.
    pub auth_method: Option<String>,
    /// `firstParty`, or a cloud provider.
    pub api_provider: Option<String>,
    /// e.g. `pro`, `max`.
    pub subscription_type: Option<String>,
    /// Why Foundry can't run Claude Code right now (`claude_not_found`, `not_signed_in`,
    /// `wrong_auth_mode`, …); null when it's ready.
    pub problem: Option<AppError>,
}

/// The fields Foundry reads from `claude auth status --json`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    #[serde(default)]
    pub logged_in: bool,
    pub auth_method: Option<String>,
    pub api_provider: Option<String>,
    pub subscription_type: Option<String>,
}

fn not_found(message: impl Into<String>) -> AppError {
    AppError::new(ErrorCode::ClaudeNotFound, message)
}

/// Finds the executable: `claude_path` if set (it must exist), otherwise the first `claude`
/// on `path_var` (normally `PATH`). On Windows that tries each `PATHEXT` extension in each
/// folder, so npm's `claude.cmd` shim is found as well as `claude.exe`.
pub fn find(claude_path: Option<&str>, path_var: Option<&OsStr>) -> Result<PathBuf, AppError> {
    if let Some(configured) = claude_path.map(str::trim).filter(|p| !p.is_empty()) {
        let path = PathBuf::from(configured);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(not_found(format!(
                "claudePath points to {configured}, which doesn't exist"
            )))
        };
    }
    let dirs: Vec<PathBuf> = path_var
        .map(|paths| std::env::split_paths(paths).collect())
        .unwrap_or_default();
    for dir in dirs {
        for name in candidate_names() {
            let candidate = dir.join(&name);
            if candidate.is_file() {
                return Ok(candidate);
            }
        }
    }
    Err(not_found(
        "Claude Code isn't installed (no `claude` on PATH). Install it, or set claudePath",
    ))
}

#[cfg(windows)]
fn candidate_names() -> Vec<String> {
    // An extensionless `claude` on Windows is npm's shell script for Git Bash: not runnable.
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    pathext
        .split(';')
        .map(str::trim)
        .filter(|ext| !ext.is_empty())
        .map(|ext| format!("claude{}", ext.to_ascii_lowercase()))
        .collect()
}

#[cfg(not(windows))]
fn candidate_names() -> Vec<String> {
    vec!["claude".into()]
}

/// A `claude` command with the subscription environment, no console window, and `cwd`.
pub fn command(program: &Path, cwd: &Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.current_dir(cwd).env_clear().envs(subscription_env());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

pub struct Captured {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

/// Runs `cmd` with `input` on stdin, capturing stdout and stderr. Kills its whole process
/// tree on `cancel` (→ `cancelled`) or at `deadline` (→ `timed_out`).
pub fn run(
    mut cmd: Command,
    input: &[u8],
    deadline: Instant,
    cancel: &AtomicBool,
) -> Result<Captured, AppError> {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            not_found(format!("could not start Claude Code: {err}"))
        } else {
            AppError::from(err)
        }
    })?;
    let tree = match ProcessTree::attach(&child) {
        Ok(tree) => tree,
        Err(err) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(err);
        }
    };
    // Each pipe on its own thread, so a large prompt or reply can't deadlock.
    let mut stdin = child.stdin.take();
    let input = input.to_vec();
    let writer = thread::spawn(move || {
        if let Some(stdin) = stdin.as_mut() {
            let _ = stdin.write_all(&input);
        }
    });
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());

    let stopped = loop {
        if let Some(status) = child.try_wait()? {
            break Ok(status);
        }
        if cancel.load(Ordering::SeqCst) {
            break Err(AppError::new(ErrorCode::Cancelled, "cancelled"));
        }
        if Instant::now() >= deadline {
            break Err(AppError::new(
                ErrorCode::TimedOut,
                "Claude Code didn't finish in time",
            ));
        }
        thread::sleep(POLL);
    };
    if stopped.is_err() {
        let _ = tree.kill();
        let _ = child.wait();
    }
    let _ = writer.join();
    let stdout = stdout.join().unwrap_or_default();
    let stderr = stderr.join().unwrap_or_default();
    Ok(Captured {
        status: stopped?,
        stdout,
        stderr,
    })
}

fn drain(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut out = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = pipe.read_to_end(&mut out);
        }
        out
    })
}

fn quick(program: &Path, args: &[&str]) -> Result<Captured, AppError> {
    let scratch = std::env::temp_dir();
    let mut cmd = command(program, &scratch);
    cmd.args(args);
    run(
        cmd,
        b"",
        Instant::now() + QUICK_TIMEOUT,
        &AtomicBool::new(false),
    )
}

/// `claude --version`, e.g. `2.1.287 (Claude Code)`.
pub fn version(program: &Path) -> Option<String> {
    let out = quick(program, &["--version"]).ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !text.is_empty()).then_some(text)
}

/// A `major.minor.patch` version, as `claude --version` starts its output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u32, pub u32, pub u32);

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// The first Claude Code with exec-form hooks (`args`, no shell), per its CHANGELOG:
/// "2.1.139 — Added hook `args: string[]` field (exec form)".
pub const HOOK_ARGS_MIN_VERSION: Version = Version(2, 1, 139);

/// `2.1.292 (Claude Code)` → `2.1.292`. `None` for anything else.
pub fn parse_version(text: &str) -> Option<Version> {
    let first = text.split_whitespace().next()?;
    let mut parts = first.split('.');
    let mut number = || parts.next()?.parse::<u32>().ok();
    let version = Version(number()?, number()?, number()?);
    parts.next().is_none().then_some(version)
}

/// Parses `claude auth status --json` (it exits non-zero when signed out, but still prints).
pub fn auth_status(program: &Path) -> Result<AuthStatus, AppError> {
    let out = quick(program, &["auth", "status", "--json"])?;
    serde_json::from_slice(&out.stdout).map_err(|err| {
        AppError::internal(format!(
            "couldn't read `claude auth status` (exit {:?}): {err}",
            out.status.code()
        ))
    })
}

/// `not_signed_in` or `wrong_auth_mode` unless signed in with a Claude subscription.
pub fn check_auth(auth: &AuthStatus) -> Result<(), AppError> {
    if !auth.logged_in {
        return Err(AppError::new(
            ErrorCode::NotSignedIn,
            "Claude Code isn't signed in. Run `claude` and sign in with your Claude account",
        ));
    }
    let method = auth.auth_method.as_deref().unwrap_or("unknown");
    let provider = auth.api_provider.as_deref().unwrap_or(FIRST_PARTY);
    if method != SUBSCRIPTION_METHOD || provider != FIRST_PARTY {
        return Err(AppError::new(
            ErrorCode::WrongAuthMode,
            format!(
                "Claude Code is signed in with {method} ({provider}), not a Claude subscription. \
                 Run `claude auth login` and choose your Claude account"
            ),
        ));
    }
    Ok(())
}

/// Finds `claude` and checks the subscription login. Returns the executable to run.
pub fn preflight(claude_path: Option<&str>) -> Result<PathBuf, AppError> {
    let program = find(claude_path, std::env::var_os("PATH").as_deref())?;
    check_auth(&auth_status(&program)?)?;
    Ok(program)
}

/// Everything `claude_status` reports, never failing: problems go in `problem`.
pub fn status(claude_path: Option<&str>) -> ClaudeStatus {
    status_with(claude_path, std::env::var_os("PATH").as_deref())
}

pub fn status_with(claude_path: Option<&str>, path_var: Option<&OsStr>) -> ClaudeStatus {
    let mut status = ClaudeStatus {
        path: None,
        version: None,
        logged_in: false,
        auth_method: None,
        api_provider: None,
        subscription_type: None,
        problem: None,
    };
    let program = match find(claude_path, path_var) {
        Ok(program) => program,
        Err(err) => {
            status.problem = Some(err);
            return status;
        }
    };
    status.path = Some(program.to_string_lossy().into_owned());
    status.version = version(&program);
    match auth_status(&program) {
        Ok(auth) => {
            status.problem = check_auth(&auth).err();
            status.logged_in = auth.logged_in;
            status.auth_method = auth.auth_method;
            status.api_provider = auth.api_provider;
            status.subscription_type = auth.subscription_type;
        }
        Err(err) => status.problem = Some(err),
    }
    status
}

#[cfg(test)]
mod version_tests {
    use super::*;

    #[test]
    fn parses_versions_and_rejects_garbage() {
        assert_eq!(
            parse_version("2.1.292 (Claude Code)"),
            Some(Version(2, 1, 292))
        );
        assert_eq!(parse_version("  2.1.139\n"), Some(Version(2, 1, 139)));
        for bad in [
            "",
            "garbage",
            "v2.1.0",
            "2.1",
            "2.1.x",
            "2.1.3.4",
            "Claude Code 2.1.0",
        ] {
            assert_eq!(parse_version(bad), None, "{bad}");
        }
        assert!(Version(2, 1, 138) < HOOK_ARGS_MIN_VERSION);
        assert!(Version(2, 2, 0) > HOOK_ARGS_MIN_VERSION);
        assert_eq!(HOOK_ARGS_MIN_VERSION.to_string(), "2.1.139");
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::testing::{install_fake_claude, TestRepo};

    fn write_auth(dir: &Path, json: &str) {
        fs::write(dir.join("auth.json"), json).unwrap();
    }

    #[test]
    fn find_uses_claude_path_or_the_first_match_on_path() {
        let (a, b) = (TestRepo::new(), TestRepo::new());
        let exe = install_fake_claude(b.path());
        let path = std::env::join_paths([a.path(), b.path()]).unwrap();

        assert_eq!(find(None, Some(&path)).unwrap(), exe);
        assert_eq!(find(Some(exe.to_str().unwrap()), None).unwrap(), exe);
        assert_eq!(
            find(Some("  "), Some(&path)).unwrap(),
            exe,
            "blank claudePath = unset"
        );

        let missing = a.path().join("nope.exe");
        let err = find(Some(missing.to_str().unwrap()), Some(&path)).unwrap_err();
        assert_eq!(err.code, ErrorCode::ClaudeNotFound);
        assert_eq!(
            find(None, Some(a.path().as_os_str())).unwrap_err().code,
            ErrorCode::ClaudeNotFound
        );
        assert_eq!(
            find(None, None).unwrap_err().code,
            ErrorCode::ClaudeNotFound
        );
    }

    /// npm installs `claude.cmd` (and an extensionless shell script, which Windows can't run).
    #[cfg(windows)]
    #[test]
    fn find_and_run_an_npm_cmd_shim() {
        let (shim_dir, real) = (TestRepo::new(), TestRepo::new());
        let exe = install_fake_claude(real.path());
        fs::write(shim_dir.path().join("claude"), "#!/bin/sh\nexit 1\n").unwrap();
        fs::write(
            shim_dir.path().join("claude.cmd"),
            format!("@echo off\r\n\"{}\" %*\r\n", exe.display()),
        )
        .unwrap();

        let found = find(None, Some(shim_dir.path().as_os_str())).unwrap();
        assert_eq!(found, shim_dir.path().join("claude.cmd"));
        assert_eq!(version(&found).as_deref(), Some("9.9.9 (Fake Claude)"));
        check_auth(&auth_status(&found).unwrap()).unwrap();
    }

    #[test]
    fn auth_is_checked_for_a_subscription_login() {
        let dir = TestRepo::new();
        let exe = install_fake_claude(dir.path());
        assert!(
            check_auth(&auth_status(&exe).unwrap()).is_ok(),
            "default: subscription"
        );

        for (json, code) in [
            (
                r#"{"loggedIn":false,"authMethod":"none","apiProvider":"firstParty"}"#,
                ErrorCode::NotSignedIn,
            ),
            (
                r#"{"loggedIn":true,"authMethod":"console","apiProvider":"firstParty"}"#,
                ErrorCode::WrongAuthMode,
            ),
            (
                r#"{"loggedIn":true,"authMethod":"api_key","apiProvider":"firstParty"}"#,
                ErrorCode::WrongAuthMode,
            ),
            (
                r#"{"loggedIn":true,"authMethod":"claude.ai","apiProvider":"bedrock"}"#,
                ErrorCode::WrongAuthMode,
            ),
        ] {
            write_auth(dir.path(), json);
            let auth = auth_status(&exe).unwrap();
            assert_eq!(check_auth(&auth).unwrap_err().code, code, "{json}");
        }
        write_auth(dir.path(), "not json");
        assert_eq!(auth_status(&exe).unwrap_err().code, ErrorCode::Internal);
    }

    #[test]
    fn status_reports_everything_but_never_the_account() {
        let dir = TestRepo::new();
        let exe = install_fake_claude(dir.path());

        let status = status_with(Some(exe.to_str().unwrap()), None);

        assert_eq!(status.path.as_deref(), exe.to_str());
        assert_eq!(status.version.as_deref(), Some("9.9.9 (Fake Claude)"));
        assert!(status.logged_in);
        assert_eq!(status.auth_method.as_deref(), Some("claude.ai"));
        assert_eq!(status.subscription_type.as_deref(), Some("pro"));
        assert_eq!(status.problem, None);
        let json = serde_json::to_string(&status).unwrap();
        assert!(
            !json.contains("example.invalid"),
            "the email never leaves: {json}"
        );

        write_auth(dir.path(), r#"{"loggedIn":false}"#);
        let signed_out = status_with(Some(exe.to_str().unwrap()), None);
        assert_eq!(signed_out.problem.unwrap().code, ErrorCode::NotSignedIn);
        let missing = status_with(None, Some(dir.path().join("empty").as_os_str()));
        assert_eq!(missing.problem.unwrap().code, ErrorCode::ClaudeNotFound);
        assert_eq!(missing.path, None);
    }
}
