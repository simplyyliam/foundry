//! What the `foundry-hook` helper allows, warns about or blocks before a tool runs.
//!
//! Std, serde and serde_json only: the helper binary includes this file (and `glob.rs`) with
//! `#[path]`, so it must never use `crate::`. The decision is local and never touches the
//! network.
//!
//! - Edit tools (`Write`, `Edit`, `MultiEdit`, `NotebookEdit`): the target, normalized and
//!   resolved through links, must sit inside the repo, must not match the protected floor or
//!   the agent's `protectedGlobs`, and must match its `writeGlobs`, except for the report
//!   carve-out (`.foundry/reports/<name>.json`, a direct child). No `roles.json`, one that
//!   doesn't parse, or no entry for the agent: blocked (fail closed).
//! - Command tools (`Bash`, `PowerShell`): clearly destructive commands are blocked; `git
//!   push` and dependency installs are allowed with a warning.

use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use super::glob;

pub const EDIT_TOOLS: &[&str] = &["Write", "Edit", "MultiEdit", "NotebookEdit"];
pub const COMMAND_TOOLS: &[&str] = &["Bash", "PowerShell"];
pub const ROLES_FILE: &str = "agents/roles.json";

/// Protected for every agent, whatever `roles.json` says. Kept equal to the team feature's
/// floor (a test checks).
pub const PROTECTED_FLOOR: &[&str] = &[
    "AGENTS.md",
    "agents/**",
    "docs/design/**",
    ".foundry/plan.json",
    // Holds `testCommands`, which Verify runs with the human's rights.
    ".foundry/project.json",
    // Verification results: the plan trusts them to mark a slice done.
    ".foundry/reviews/**",
    ".claude/**",
];

/// Rule ids, as sent in the event's `decision.rule` and named in the agent's message.
pub mod rule {
    pub const ROLES_MISSING: &str = "roles_missing";
    pub const ROLES_INVALID: &str = "roles_invalid";
    pub const UNKNOWN_AGENT: &str = "unknown_agent";
    pub const NO_TARGET: &str = "no_target_path";
    pub const PATH_ESCAPE: &str = "path_escape";
    pub const OUTSIDE_REPO: &str = "outside_repo";
    pub const PROTECTED: &str = "protected_path";
    pub const OUTSIDE_WRITE_GLOBS: &str = "outside_write_globs";
    pub const GIT_PUSH_FORCE: &str = "git_push_force";
    pub const GIT_RESET_HARD: &str = "git_reset_hard";
    pub const GIT_CLEAN_FORCE: &str = "git_clean_force";
    pub const RM_RF_OUTSIDE: &str = "rm_rf_outside";
    pub const GIT_PUSH: &str = "git_push";
    pub const DEPENDENCY_INSTALL: &str = "dependency_install";
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    /// Allowed; the agent and the human are told.
    Warn {
        rule: &'static str,
        message: String,
    },
    Block {
        rule: &'static str,
        message: String,
    },
}

impl Verdict {
    pub fn rule(&self) -> Option<&'static str> {
        match self {
            Self::Allow => None,
            Self::Warn { rule, .. } | Self::Block { rule, .. } => Some(rule),
        }
    }
}

const ASK: &str = "Don't work around this: stop and ask the human if you need it.";

fn block(rule: &'static str, what: String) -> Verdict {
    Verdict::Block {
        rule,
        message: format!("Foundry blocked this: {what} (rule: {rule}). {ASK}"),
    }
}

fn warn(rule: &'static str, what: String) -> Verdict {
    Verdict::Warn {
        rule,
        message: format!("Foundry warning: {what} (rule: {rule}). Make sure the human expects it."),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RolesFile {
    agents: Vec<RoleEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoleEntry {
    id: String,
    #[serde(default)]
    write_globs: Vec<String>,
    #[serde(default)]
    protected_globs: Vec<String>,
}

/// Checks an edit tool's target. `roles_json` is the text of `agents/roles.json`, `None`
/// when it's missing. `cwd` resolves a relative `target`.
pub fn check_edit(
    repo_root: &Path,
    roles_json: Option<&str>,
    agent_id: &str,
    target: Option<&str>,
    cwd: &Path,
) -> Verdict {
    let Some(roles_json) = roles_json else {
        return block(
            rule::ROLES_MISSING,
            format!("{ROLES_FILE} is missing, so no edits are allowed"),
        );
    };
    let roles: RolesFile = match serde_json::from_str(roles_json.trim_start_matches('\u{feff}')) {
        Ok(roles) => roles,
        Err(err) => {
            return block(
                rule::ROLES_INVALID,
                format!("{ROLES_FILE} doesn't parse ({err}), so no edits are allowed"),
            )
        }
    };
    let Some(role) = roles.agents.iter().find(|a| a.id == agent_id) else {
        return block(
            rule::UNKNOWN_AGENT,
            format!("agent {agent_id} isn't in {ROLES_FILE}, so it may not edit"),
        );
    };
    let Some(target) = target.map(str::trim).filter(|t| !t.is_empty()) else {
        return block(rule::NO_TARGET, "the edit names no file".into());
    };
    let rel = match repo_relative(repo_root, target, cwd) {
        Ok(rel) => rel,
        Err(verdict) => return verdict,
    };
    if let Some(hit) = PROTECTED_FLOOR
        .iter()
        .copied()
        .chain(role.protected_globs.iter().map(String::as_str))
        .find(|g| glob::matches(g, &rel))
    {
        return block(
            rule::PROTECTED,
            format!("{rel} is protected for agent {agent_id} (protectedGlobs \"{hit}\")"),
        );
    }
    if is_report(&rel) || role.write_globs.iter().any(|g| glob::matches(g, &rel)) {
        return Verdict::Allow;
    }
    block(
        rule::OUTSIDE_WRITE_GLOBS,
        format!(
            "{rel} is outside agent {agent_id}'s writeGlobs ({})",
            if role.write_globs.is_empty() {
                "none".to_string()
            } else {
                role.write_globs.join(", ")
            }
        ),
    )
}

/// `.foundry/reports/<name>.json`, a direct child: any agent may write its slice report.
pub fn is_report(rel: &str) -> bool {
    rel.strip_prefix(".foundry/reports/")
        .and_then(|name| name.strip_suffix(".json"))
        .is_some_and(|stem| !stem.is_empty() && !stem.contains('/'))
}

/// The target relative to the repo, `/`-separated, after resolving links: a `..` anywhere is
/// refused outright, and so is anything that lands outside the repo.
pub fn repo_relative(repo_root: &Path, target: &str, cwd: &Path) -> Result<String, Verdict> {
    let raw = Path::new(target);
    if raw.components().any(|c| c == Component::ParentDir)
        || target.split(['/', '\\']).any(|p| p == "..")
    {
        return Err(block(rule::PATH_ESCAPE, format!("{target} contains '..'")));
    }
    let outside = || block(rule::OUTSIDE_REPO, format!("{target} is outside the repo"));
    let root = repo_root.canonicalize().map_err(|_| outside())?;
    let joined = if raw.is_absolute() {
        raw.to_path_buf()
    } else {
        cwd.join(raw)
    };
    let resolved = resolve_existing(&joined).ok_or_else(outside)?;
    let rel = resolved.strip_prefix(&root).map_err(|_| outside())?;
    let parts: Vec<String> = rel
        .components()
        .map(|c| match c {
            Component::Normal(part) => Ok(part.to_string_lossy().into_owned()),
            _ => Err(()),
        })
        .collect::<Result<_, _>>()
        .map_err(|()| outside())?;
    if parts.is_empty() {
        return Err(outside());
    }
    Ok(parts.join("/"))
}

/// Canonicalizes the deepest part of `path` that exists (following every link in it) and
/// appends the rest unchanged.
fn resolve_existing(path: &Path) -> Option<PathBuf> {
    let mut existing = path.to_path_buf();
    let mut rest: Vec<std::ffi::OsString> = Vec::new();
    loop {
        if existing.symlink_metadata().is_ok() {
            let mut real = existing.canonicalize().ok()?;
            for part in rest.iter().rev() {
                real.push(part);
            }
            return Some(real);
        }
        let name = existing.file_name()?.to_os_string();
        rest.push(name);
        existing = existing.parent()?.to_path_buf();
    }
}

// ---------- commands ----------

/// Words of one shell command part, quotes stripped, environment assignments and wrappers
/// (`sudo`, `env`, `command`, `nohup`, `time`) skipped.
fn parts(command: &str) -> Vec<Vec<String>> {
    command
        .split(['\n', ';', '|', '&'])
        .map(|part| {
            let words: Vec<String> = part
                .split_whitespace()
                .map(|w| w.trim_matches(|c| c == '"' || c == '\'').to_string())
                .collect();
            let start = words
                .iter()
                .position(|w| {
                    let assignment = w.contains('=') && !w.starts_with('-');
                    !assignment
                        && !matches!(w.as_str(), "sudo" | "env" | "command" | "nohup" | "time")
                })
                .unwrap_or(words.len());
            words[start..].to_vec()
        })
        .filter(|words| !words.is_empty())
        .collect()
}

fn program(word: &str) -> String {
    let name = word
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(word)
        .to_ascii_lowercase();
    [".exe", ".cmd", ".bat"]
        .iter()
        .find_map(|ext| name.strip_suffix(ext).map(str::to_string))
        .unwrap_or(name)
}

/// A short-flag cluster (`-fd`) containing `flag`.
fn short_has(word: &str, flag: char) -> bool {
    word.starts_with('-') && !word.starts_with("--") && word[1..].contains(flag)
}

/// `git`'s subcommand and its arguments, skipping global options (`-C <dir>`, `-c <k=v>`).
fn git_sub(args: &[String]) -> Option<(&str, &[String])> {
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        if arg == "-C" || arg == "-c" {
            i += 2;
        } else if arg.starts_with('-') {
            i += 1;
        } else {
            return Some((arg, &args[i + 1..]));
        }
    }
    None
}

fn is_outside_path(target: &str) -> bool {
    let t = target.trim();
    let bytes = t.as_bytes();
    t.starts_with('/')
        || t.starts_with('\\')
        || t.starts_with('~')
        || t.starts_with("$HOME")
        || t.starts_with("${HOME}")
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        || t.split(['/', '\\']).any(|p| p == "..")
}

/// Checks a `Bash` or `PowerShell` command. The first block wins; otherwise the first warning.
pub fn check_command(command: &str) -> Verdict {
    let mut warning = None;
    for words in parts(command) {
        let name = program(&words[0]);
        let args = &words[1..];
        let verdict = match name.as_str() {
            "git" => check_git(args, command),
            "rm" => check_rm(args),
            _ => check_install(&name, args),
        };
        match verdict {
            Verdict::Block { .. } => return verdict,
            Verdict::Warn { .. } if warning.is_none() => warning = Some(verdict),
            _ => {}
        }
    }
    warning.unwrap_or(Verdict::Allow)
}

fn check_git(args: &[String], command: &str) -> Verdict {
    let Some((sub, rest)) = git_sub(args) else {
        return Verdict::Allow;
    };
    let short = |fallback: &str| short_command(fallback, 60, command);
    match sub {
        "push" => {
            let force = rest.iter().any(|a| {
                a == "--force"
                    || a.starts_with("--force-with-lease")
                    || a == "--force-if-includes"
                    || short_has(a, 'f')
                    || (a.starts_with('+') && a.len() > 1)
            });
            if force {
                block(
                    rule::GIT_PUSH_FORCE,
                    format!("a force push ({})", short("git push")),
                )
            } else {
                warn(
                    rule::GIT_PUSH,
                    format!("this pushes to a remote ({})", short("git push")),
                )
            }
        }
        "reset" if rest.iter().any(|a| a == "--hard") => block(
            rule::GIT_RESET_HARD,
            "git reset --hard throws away uncommitted work".into(),
        ),
        "clean" => {
            let dry_run = rest.iter().any(|a| a == "--dry-run" || short_has(a, 'n'));
            let force = rest.iter().any(|a| a == "--force" || short_has(a, 'f'));
            if force && !dry_run {
                block(
                    rule::GIT_CLEAN_FORCE,
                    "git clean -f deletes untracked files".into(),
                )
            } else {
                Verdict::Allow
            }
        }
        _ => Verdict::Allow,
    }
}

/// The command, cut for a message.
fn short_command(fallback: &str, max: usize, command: &str) -> String {
    let command = command.trim();
    if command.is_empty() {
        return fallback.into();
    }
    if command.chars().count() <= max {
        command.to_string()
    } else {
        let cut: String = command.chars().take(max - 1).collect();
        format!("{cut}…")
    }
}

fn check_rm(args: &[String]) -> Verdict {
    let mut recursive = false;
    let mut force = false;
    let mut targets = Vec::new();
    let mut only_targets = false;
    for arg in args {
        if only_targets || !arg.starts_with('-') || arg == "-" {
            targets.push(arg.as_str());
        } else if arg == "--" {
            only_targets = true;
        } else if arg == "--recursive" {
            recursive = true;
        } else if arg == "--force" {
            force = true;
        } else if !arg.starts_with("--") {
            recursive |= short_has(arg, 'r') || short_has(arg, 'R');
            force |= short_has(arg, 'f');
        }
    }
    match targets.iter().find(|t| is_outside_path(t)) {
        Some(target) if recursive && force => block(
            rule::RM_RF_OUTSIDE,
            format!("rm -rf on {target}, an absolute path or one with '..'"),
        ),
        _ => Verdict::Allow,
    }
}

fn check_install(name: &str, args: &[String]) -> Verdict {
    let sub = args
        .iter()
        .find(|a| !a.starts_with('-'))
        .map(String::as_str);
    let installs = match name {
        "npm" => matches!(sub, Some("install" | "i" | "add" | "ci")),
        "pnpm" | "bun" => matches!(sub, Some("install" | "i" | "add")),
        "yarn" => matches!(sub, None | Some("install" | "add")),
        "cargo" => matches!(sub, Some("add" | "install")),
        "pip" | "pip3" => matches!(sub, Some("install")),
        "uv" => {
            matches!(sub, Some("add" | "pip")) && args.iter().any(|a| a == "add" || a == "install")
        }
        "python" | "python3" | "py" => args
            .windows(3)
            .any(|w| w[0] == "-m" && w[1].starts_with("pip") && w[2] == "install"),
        "poetry" => matches!(sub, Some("add")),
        "go" => matches!(sub, Some("get" | "install")),
        "gem" => matches!(sub, Some("install")),
        "composer" => matches!(sub, Some("require" | "install")),
        "dotnet" => sub == Some("add") && args.iter().any(|a| a == "package"),
        _ => false,
    };
    if installs {
        warn(
            rule::DEPENDENCY_INSTALL,
            format!("{name} installs dependencies"),
        )
    } else {
        Verdict::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule_of(v: Verdict) -> Option<&'static str> {
        v.rule()
    }

    #[test]
    fn destructive_commands_are_blocked() {
        for (command, expected) in [
            ("git push --force", rule::GIT_PUSH_FORCE),
            ("git push -f origin main", rule::GIT_PUSH_FORCE),
            ("git push -fu origin main", rule::GIT_PUSH_FORCE),
            ("git push --force-with-lease", rule::GIT_PUSH_FORCE),
            ("git push origin +main", rule::GIT_PUSH_FORCE),
            ("git -C repo push --force", rule::GIT_PUSH_FORCE),
            ("cd x && git reset --hard HEAD~1", rule::GIT_RESET_HARD),
            ("git clean -fd", rule::GIT_CLEAN_FORCE),
            ("git clean -f", rule::GIT_CLEAN_FORCE),
            ("git clean -xdf", rule::GIT_CLEAN_FORCE),
            ("rm -rf /", rule::RM_RF_OUTSIDE),
            ("rm -rf ~/projects", rule::RM_RF_OUTSIDE),
            ("rm -fr ../other", rule::RM_RF_OUTSIDE),
            ("rm -r -f build/../../x", rule::RM_RF_OUTSIDE),
            ("rm --recursive --force C:/Users", rule::RM_RF_OUTSIDE),
            ("sudo rm -Rf /var/lib", rule::RM_RF_OUTSIDE),
            ("echo ok; rm -rf \"$HOME\"", rule::RM_RF_OUTSIDE),
            ("FOO=1 git push --force", rule::GIT_PUSH_FORCE),
        ] {
            assert_eq!(rule_of(check_command(command)), Some(expected), "{command}");
            assert!(
                matches!(check_command(command), Verdict::Block { .. }),
                "{command}"
            );
        }
    }

    #[test]
    fn pushes_and_installs_only_warn() {
        for (command, expected) in [
            ("git push", rule::GIT_PUSH),
            ("git push origin main", rule::GIT_PUSH),
            ("npm install", rule::DEPENDENCY_INSTALL),
            ("pnpm add -D vitest", rule::DEPENDENCY_INSTALL),
            ("yarn", rule::DEPENDENCY_INSTALL),
            ("cargo add serde", rule::DEPENDENCY_INSTALL),
            ("pip install requests", rule::DEPENDENCY_INSTALL),
            ("python -m pip install x", rule::DEPENDENCY_INSTALL),
            ("uv add httpx", rule::DEPENDENCY_INSTALL),
            ("go get example.com/x", rule::DEPENDENCY_INSTALL),
        ] {
            let verdict = check_command(command);
            assert!(
                matches!(verdict, Verdict::Warn { .. }),
                "{command}: {verdict:?}"
            );
            assert_eq!(verdict.rule(), Some(expected), "{command}");
        }
    }

    #[test]
    fn ordinary_commands_are_allowed() {
        for command in [
            "cargo test",
            "git status",
            "git commit -m 'push --force later'",
            "git reset --soft HEAD~1",
            "git clean -n",
            "git clean -fdn",
            "rm -rf target",
            "rm -rf ./build",
            "rm /tmp/x",
            "rm -r /tmp/x",
            "ls -la /",
            "npm test",
            "cargo build",
            "",
        ] {
            assert_eq!(check_command(command), Verdict::Allow, "{command}");
        }
    }

    #[test]
    fn report_paths_are_direct_json_children_only() {
        assert!(is_report(".foundry/reports/s-feed.json"));
        for no in [
            ".foundry/reports/sub/x.json",
            ".foundry/reports/.json",
            ".foundry/reports/x.md",
            ".foundry/plan.json",
            "foundry/reports/x.json",
        ] {
            assert!(!is_report(no), "{no}");
        }
    }
}
