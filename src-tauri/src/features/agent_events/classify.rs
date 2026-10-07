//! The classifier: a pure function from one hook POST to the events it means. No I/O. What
//! it needs beyond the body (the agent's protected globs, the repo root, the failure streak)
//! comes in through [`Context`]; the next streak comes back in [`Classified`].

use std::path::Path;

use super::payload::{self, Hook, HookPost, Outcome, ToolCall};
use super::{EventKind, Severity};
use crate::core::glob;

/// Longest `command` kept on an event (characters).
pub const COMMAND_CAP: usize = 2000;
/// Longest `detail` kept on an event (characters), including Stop's last assistant message.
pub const DETAIL_CAP: usize = 300;
/// How many failures of the same command in a row make an agent `stuck`.
pub const STUCK_AFTER: u32 = 3;

/// Dependency manifests and lockfiles, by file name, at any depth.
const MANIFESTS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "package.json",
    "package-lock.json",
    "npm-shrinkwrap.json",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
    "deno.json",
    "deno.jsonc",
    "deno.lock",
    "pyproject.toml",
    "poetry.lock",
    "uv.lock",
    "Pipfile",
    "Pipfile.lock",
    "requirements.txt",
    "go.mod",
    "go.sum",
    "Gemfile",
    "Gemfile.lock",
    "composer.json",
    "composer.lock",
];

/// Notification types that are only informational: no `needs_attention` for these.
const QUIET_NOTIFICATIONS: &[&str] = &[
    "auth_success",
    "elicitation_complete",
    "elicitation_response",
];

const EDIT_TOOLS: &[&str] = &["Write", "Edit", "MultiEdit", "NotebookEdit"];
const COMMAND_TOOLS: &[&str] = &["Bash", "PowerShell"];

/// An event before the store gives it an id, a timestamp, a project and an agent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Draft {
    pub slice_id: Option<String>,
    pub kind: EventKind,
    pub severity: Severity,
    pub title: String,
    pub detail: Option<String>,
    pub path: Option<String>,
    pub command: Option<String>,
}

/// The same command failing in a row, per agent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Streak {
    pub command: Option<String>,
    pub failures: u32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Context<'a> {
    /// The project's repo folder, to make absolute tool paths repo-relative.
    pub repo_root: Option<&'a Path>,
    /// The agent's `protectedGlobs` from `agents/roles.json`.
    pub protected_globs: &'a [String],
    pub streak: Option<&'a Streak>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Classified {
    pub drafts: Vec<Draft>,
    pub streak: Streak,
}

pub fn classify(post: &HookPost, ctx: &Context<'_>) -> Classified {
    let slice_id = post
        .slice_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let draft = |kind, severity, title: String| Draft {
        slice_id: slice_id.clone(),
        kind,
        severity,
        title,
        detail: None,
        path: None,
        command: None,
    };
    let mut streak = ctx.streak.cloned().unwrap_or_default();
    let hook = payload::parse(post);
    let mut drafts = Vec::new();

    if let Some(decision) = post.decision.as_ref().filter(|d| d.blocked) {
        let call = hook.tool_call().cloned().unwrap_or_default();
        let path = call
            .file_path
            .as_deref()
            .map(|p| repo_relative(ctx.repo_root, p));
        let command = call.command.as_deref().map(|c| cut(c, COMMAND_CAP));
        let title = match (&command, &path) {
            (Some(command), _) => format!("Blocked a command: {}", cut(command, 80)),
            (None, Some(path)) => format!("Blocked a write to {path}"),
            (None, None) => "Blocked an action".to_string(),
        };
        drafts.push(Draft {
            detail: decision.rule.as_deref().map(|r| cut(r, DETAIL_CAP)),
            path,
            command,
            ..draft(EventKind::Blocked, Severity::Danger, title)
        });
        return Classified { drafts, streak };
    }

    match &hook {
        Hook::SessionStart { source } => drafts.push(Draft {
            detail: source.clone(),
            ..draft(
                EventKind::SessionStarted,
                Severity::Info,
                "Session started".into(),
            )
        }),
        Hook::SessionEnd { reason } => drafts.push(Draft {
            detail: reason.clone(),
            ..draft(
                EventKind::SessionEnded,
                Severity::Info,
                "Session ended".into(),
            )
        }),
        Hook::Notification { kind, message } => {
            if !kind
                .as_deref()
                .is_some_and(|k| QUIET_NOTIFICATIONS.contains(&k))
            {
                let title = match kind.as_deref() {
                    Some("permission_prompt") => "Needs permission",
                    Some("idle_prompt") => "Waiting for input",
                    _ => "Needs attention",
                };
                drafts.push(Draft {
                    detail: message.as_deref().map(|m| cut(m, DETAIL_CAP)),
                    ..draft(EventKind::NeedsAttention, Severity::Warn, title.into())
                });
            }
        }
        Hook::Stop { last_message } => drafts.push(Draft {
            detail: last_message.as_deref().map(|m| cut(m, DETAIL_CAP)),
            ..draft(
                EventKind::TurnComplete,
                Severity::Info,
                "Turn complete".into(),
            )
        }),
        Hook::PostToolUse(call, outcome) => {
            if is_tool(call, EDIT_TOOLS) {
                drafts.push(edit(
                    call,
                    ctx,
                    draft(EventKind::Edit, Severity::Info, String::new()),
                ));
            } else if let (true, Some(command)) = (is_tool(call, COMMAND_TOOLS), &call.command) {
                let base = Draft {
                    command: Some(cut(command, COMMAND_CAP)),
                    detail: (*outcome == Outcome::Failed)
                        .then(|| call.error.as_deref().map(|e| cut(e, DETAIL_CAP)))
                        .flatten(),
                    ..draft(EventKind::Command, Severity::Info, String::new())
                };
                drafts.push(if is_test_command(command) {
                    let title = match outcome {
                        Outcome::Passed => "Tests passed",
                        Outcome::Failed => "Tests failed",
                        Outcome::Interrupted => "Tests interrupted",
                    };
                    Draft {
                        kind: EventKind::TestsRan,
                        title: title.into(),
                        ..base.clone()
                    }
                } else {
                    let title = match outcome {
                        Outcome::Passed => "Ran a command",
                        Outcome::Failed => "Command failed",
                        Outcome::Interrupted => "Command interrupted",
                    };
                    Draft {
                        title: title.into(),
                        ..base.clone()
                    }
                });
                streak = next_streak(streak, command, *outcome);
                if *outcome == Outcome::Failed && streak.failures == STUCK_AFTER {
                    drafts.push(Draft {
                        kind: EventKind::Stuck,
                        severity: Severity::Warn,
                        title: format!("The same command failed {STUCK_AFTER} times in a row"),
                        ..base
                    });
                }
            }
        }
        // The helper allowed it with a warning (`git push`, a dependency install).
        Hook::PreToolUse(call) => {
            if let Some(rule) = post.decision.as_ref().and_then(|d| d.rule.as_deref()) {
                let path = call
                    .file_path
                    .as_deref()
                    .map(|p| repo_relative(ctx.repo_root, p));
                let command = call.command.as_deref().map(|c| cut(c, COMMAND_CAP));
                let kind = if command.is_some() {
                    EventKind::Command
                } else {
                    EventKind::Edit
                };
                drafts.push(Draft {
                    detail: Some(cut(rule, DETAIL_CAP)),
                    path,
                    command,
                    ..draft(
                        kind,
                        Severity::Warn,
                        format!("Allowed with a warning: {}", cut(rule, 80)),
                    )
                });
            }
            // Otherwise a tool about to run means nothing until it finishes.
        }
        Hook::Other(_) => {}
    }
    Classified { drafts, streak }
}

fn is_tool(call: &ToolCall, tools: &[&str]) -> bool {
    call.tool.as_deref().is_some_and(|t| tools.contains(&t))
}

fn edit(call: &ToolCall, ctx: &Context<'_>, base: Draft) -> Draft {
    let Some(path) = call
        .file_path
        .as_deref()
        .map(|p| repo_relative(ctx.repo_root, p))
    else {
        return Draft {
            title: "Edited a file".into(),
            ..base
        };
    };
    if let Some(slice) = report_slice(&path) {
        return Draft {
            slice_id: Some(slice.to_string()),
            kind: EventKind::ReportReady,
            title: format!("Report ready for {slice}"),
            path: Some(path),
            ..base
        };
    }
    let inside = !is_absolute(&path);
    let file_name = path.rsplit('/').next().unwrap_or(&path);
    let (severity, title) = if inside && ctx.protected_globs.iter().any(|g| glob::matches(g, &path))
    {
        (Severity::Warn, format!("Edited a protected file: {path}"))
    } else if MANIFESTS.contains(&file_name) {
        (Severity::Warn, format!("Edited a dependency file: {path}"))
    } else {
        (Severity::Info, format!("Edited {path}"))
    };
    Draft {
        severity,
        title,
        path: Some(path),
        ..base
    }
}

/// `.foundry/reports/<slice>.json` → `<slice>`.
fn report_slice(path: &str) -> Option<&str> {
    let slice = path
        .strip_prefix(".foundry/reports/")?
        .strip_suffix(".json")?;
    (!slice.is_empty() && !slice.contains('/')).then_some(slice)
}

fn next_streak(streak: Streak, command: &str, outcome: Outcome) -> Streak {
    if outcome != Outcome::Failed {
        return Streak::default();
    }
    let command = command.trim();
    if streak.command.as_deref() == Some(command) {
        Streak {
            failures: streak.failures.saturating_add(1),
            ..streak
        }
    } else {
        Streak {
            command: Some(command.to_string()),
            failures: 1,
        }
    }
}

/// Whether a shell command runs tests: `cargo test`/`nextest`, the `test` script of npm,
/// pnpm, yarn or bun, or `vitest`, `playwright test` or `pytest`, in any `&&`/`;`/`|` part.
pub fn is_test_command(command: &str) -> bool {
    command.split(['\n', ';', '&', '|']).any(|part| {
        let words: Vec<String> = part
            .split_whitespace()
            .map(|w| {
                w.trim_matches(|c| c == '"' || c == '\'')
                    .to_ascii_lowercase()
            })
            .filter(|w| !w.contains('=') || w.starts_with('-'))
            .collect();
        runs_tests(&words)
    })
}

fn tool_name(word: &str) -> &str {
    let name = word.rsplit(['/', '\\']).next().unwrap_or(word);
    [".exe", ".cmd", ".ps1"]
        .iter()
        .find_map(|ext| name.strip_suffix(ext))
        .unwrap_or(name)
}

fn runs_tests(words: &[String]) -> bool {
    let names: Vec<&str> = words.iter().map(|w| tool_name(w)).collect();
    let after_flags = |from: usize| names.iter().skip(from).find(|w| !w.starts_with(['-', '+']));
    match names.first() {
        Some(&"cargo") => matches!(after_flags(1), Some(&"test" | &"nextest")),
        // `npm test`, `pnpm -C web test`, `yarn run test:unit`, `pnpm exec vitest`; never
        // installing a test tool (`pnpm add vitest`).
        Some(&("npm" | "pnpm" | "yarn" | "bun")) => {
            if matches!(
                after_flags(1),
                Some(&("add" | "install" | "i" | "remove" | "uninstall" | "rm"))
            ) {
                return false;
            }
            matches!(after_flags(1), Some(&"t"))
                || names[1..]
                    .iter()
                    .any(|w| *w == "test" || w.starts_with("test:"))
                || names.iter().any(|w| is_runner(w, &names))
        }
        _ => names.iter().any(|w| is_runner(w, &names)),
    }
}

fn is_runner(word: &str, names: &[&str]) -> bool {
    match word {
        "vitest" | "pytest" | "py.test" => true,
        "playwright" => names.contains(&"test"),
        _ => false,
    }
}

/// `path` relative to `root` with `/` separators when it's inside the repo; otherwise as
/// given (with `/` separators). Relative paths are taken as repo-relative already.
pub fn repo_relative(root: Option<&Path>, path: &str) -> String {
    let path = slashes(path);
    if !is_absolute(&path) {
        return path.trim_start_matches("./").to_string();
    }
    if let Some(root) = root {
        let root = slashes(&root.to_string_lossy());
        let root = root.trim_end_matches('/');
        if path.len() > root.len() + 1 && path.as_bytes()[root.len()] == b'/' {
            let (head, rest) = path.split_at(root.len());
            let same = if cfg!(windows) {
                head.eq_ignore_ascii_case(root)
            } else {
                head == root
            };
            if same {
                return rest[1..].to_string();
            }
        }
    }
    path
}

/// Forward slashes, without Windows' `\\?\` verbatim prefix.
fn slashes(path: &str) -> String {
    let path = path.replace('\\', "/");
    path.strip_prefix("//?/")
        .map(str::to_string)
        .unwrap_or(path)
}

fn is_absolute(path: &str) -> bool {
    path.starts_with('/') || path.as_bytes().get(1) == Some(&b':')
}

/// At most `max` characters, with `…` when cut.
pub fn cut(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}
