//! Running one prompt through Claude Code in print mode (`claude -p`), with a deadline and a
//! cancel flag. Shared by every feature that asks Claude for a one-shot answer (roadmap
//! generation, team composition).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::claude;
use super::{ids, AppError, ErrorCode};

/// Cancel flag and deadline for one generation.
pub struct RunControl {
    cancelled: AtomicBool,
    pub deadline: Instant,
}

impl RunControl {
    pub fn new(timeout: Duration) -> Self {
        Self {
            cancelled: AtomicBool::new(false),
            deadline: Instant::now() + timeout,
        }
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn flag(&self) -> &AtomicBool {
        &self.cancelled
    }

    /// `cancelled` or `timed_out` if the run should stop now.
    pub fn check(&self) -> Result<(), AppError> {
        if self.is_cancelled() {
            Err(AppError::new(ErrorCode::Cancelled, "generation cancelled"))
        } else if Instant::now() >= self.deadline {
            Err(AppError::new(ErrorCode::TimedOut, "generation timed out"))
        } else {
            Ok(())
        }
    }
}

/// Runs one prompt through a model and returns its text reply. Must stop promptly (with
/// `cancelled` / `timed_out`) when `control` says so.
pub trait LlmRunner: Send + Sync {
    fn run(&self, prompt: &str, control: &RunControl) -> Result<String, AppError>;
}

/// The real runner: `claude -p` on the user's subscription.
pub struct ClaudeRunner {
    program: PathBuf,
    model: Option<String>,
}

impl ClaudeRunner {
    /// Print mode with a JSON reply, no built-in tools, no MCP servers, no saved session.
    /// The prompt goes on stdin (no Windows command-line length limit).
    pub const ARGS: &'static [&'static str] = &[
        "-p",
        "--output-format",
        "json",
        "--tools",
        "",
        "--strict-mcp-config",
        "--no-session-persistence",
    ];

    pub fn new(program: PathBuf) -> Self {
        Self {
            program,
            model: None,
        }
    }

    /// Adds `--model <model>` (the `defaultModel` setting) when set.
    pub fn with_model(mut self, model: Option<String>) -> Self {
        self.model = model.filter(|m| !m.trim().is_empty());
        self
    }
}

/// The parts of `claude -p --output-format json` Foundry reads.
#[derive(Deserialize)]
struct PrintReply {
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: String,
    #[serde(default)]
    subtype: String,
}

impl LlmRunner for ClaudeRunner {
    fn run(&self, prompt: &str, control: &RunControl) -> Result<String, AppError> {
        // A scratch folder, not the repo: no project CLAUDE.md or settings, nothing to touch.
        let scratch = std::env::temp_dir().join(format!("foundry-llm-{}", ids::new_id()));
        std::fs::create_dir_all(&scratch)?;
        let mut cmd = claude::command(&self.program, &scratch);
        cmd.args(Self::ARGS);
        if let Some(model) = &self.model {
            cmd.args(["--model", model]);
        }
        let captured = claude::run(cmd, prompt.as_bytes(), control.deadline, control.flag());
        if let Err(err) = std::fs::remove_dir_all(&scratch) {
            tracing::warn!(error = %err, "could not remove the scratch folder");
        }
        let captured = captured?;
        let reply: PrintReply = serde_json::from_slice(&captured.stdout).map_err(|_| {
            let stderr = String::from_utf8_lossy(&captured.stderr);
            AppError::new(
                ErrorCode::GenerationFailed,
                format!(
                    "Claude Code exited with {:?} without a reply: {}",
                    captured.status.code(),
                    stderr
                        .lines()
                        .find(|l| !l.trim().is_empty())
                        .unwrap_or("")
                        .trim()
                ),
            )
        })?;
        if reply.is_error {
            return Err(AppError::new(
                ErrorCode::GenerationFailed,
                format!(
                    "Claude Code reported an error ({}): {}",
                    reply.subtype, reply.result
                ),
            ));
        }
        Ok(reply.result)
    }
}
