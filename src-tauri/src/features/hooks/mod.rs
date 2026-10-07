//! Installing Foundry's Claude Code hooks into a project: entries that run the `foundry-hook`
//! helper, merged into `<repo>/.claude/settings.local.json` through
//! `core::repo_write::merge_json` (backed up first, all user content kept, idempotent).
//! Foundry's entries are the hooks whose `command` contains `foundry-hook`.
//!
//! Entries use exec form (`command` = the helper's path, plus `args`): Claude Code spawns it
//! with no shell, so a path with spaces needs no quoting on any OS. That needs Claude Code
//! [`HOOK_ARGS_MIN_VERSION`] or newer, so `hooks_install` checks `claude --version` first.

pub mod commands;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use specta::Type;

use crate::core::claude::{self, parse_version, HOOK_ARGS_MIN_VERSION};
use crate::core::git::{Git, GitRun};
use crate::core::repos::RepoIndex;
use crate::core::{repo_write, AppError, ErrorCode};

pub const SETTINGS_FILE: &str = ".claude/settings.local.json";
/// What identifies a hook as Foundry's.
pub const MARKER: &str = "foundry-hook";
/// Exec form needs `args` present; the helper ignores them.
pub const HELPER_ARGS: &[&str] = &["hook"];
const TOOL_MATCHER: &str = "Write|Edit|MultiEdit|NotebookEdit|Bash|PowerShell";
/// The hook events Foundry installs, with their matcher (tool events only).
pub const EVENTS: &[(&str, Option<&str>)] = &[
    ("SessionStart", None),
    ("PreToolUse", Some(TOOL_MATCHER)),
    ("PostToolUse", Some(TOOL_MATCHER)),
    ("PostToolUseFailure", Some(TOOL_MATCHER)),
    ("Notification", None),
    ("Stop", None),
    ("SessionEnd", None),
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HooksStatus {
    /// Every Foundry hook is in place, exactly once, running `helperPath`.
    pub installed: bool,
    /// Foundry entries exist but differ from what `hooks_install` writes (another helper path,
    /// a missing event): installing again fixes them.
    pub outdated: bool,
    /// Hook events that have a Foundry entry.
    pub events: Vec<String>,
    /// The helper `hooks_install` points the hooks at.
    pub helper_path: String,
    pub helper_exists: bool,
    /// `false` when `.claude/settings.local.json` isn't valid JSON: install and uninstall refuse
    /// to touch it until it's fixed by hand.
    pub settings_valid: bool,
    /// Whether git ignores `.claude/settings.local.json` (it holds this machine's helper path).
    /// `null` when the folder isn't a git repo or git can't be run.
    pub git_ignored: Option<bool>,
    /// What `claude --version` printed (first line), or `null` when Claude Code isn't found.
    pub claude_version: Option<String>,
    /// The version is at least `minClaudeVersion`.
    pub claude_version_supported: bool,
    pub min_claude_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HooksChange {
    pub status: HooksStatus,
    /// Whether the settings file changed (false: already as asked).
    pub changed: bool,
    /// The previous settings file's copy under `.foundry/backups/`, when it was replaced.
    pub backup: Option<String>,
}

type HelperSource = Box<dyn Fn() -> Option<PathBuf> + Send + Sync>;
type ClaudeSource = Box<dyn Fn() -> Result<PathBuf, AppError> + Send + Sync>;

pub struct HooksStore {
    repos: Arc<RepoIndex>,
    git: Git,
    helper: HelperSource,
    claude: ClaudeSource,
}

/// Claude Code's version as found: the printed line, and whether it's new enough.
struct ClaudeVersion {
    line: Option<String>,
    supported: bool,
    refusal: Option<AppError>,
}

impl HooksStore {
    /// `helper` is where `foundry-hook` is (next to the app); `claude` finds Claude Code (as
    /// agent sessions do, from `claudePath` or PATH).
    pub fn new(
        repos: Arc<RepoIndex>,
        git: Git,
        helper: impl Fn() -> Option<PathBuf> + Send + Sync + 'static,
        claude: impl Fn() -> Result<PathBuf, AppError> + Send + Sync + 'static,
    ) -> Self {
        Self {
            repos,
            git,
            helper: Box::new(helper),
            claude: Box::new(claude),
        }
    }

    fn helper_path(&self) -> Result<PathBuf, AppError> {
        (self.helper)().ok_or_else(|| AppError::internal("can't tell where the app is installed"))
    }

    fn claude_version(&self) -> ClaudeVersion {
        let min = HOOK_ARGS_MIN_VERSION;
        let program = match (self.claude)() {
            Ok(program) => program,
            Err(err) => {
                let refusal = AppError::new(
                    err.code,
                    format!(
                        "{} Foundry's hooks need Claude Code {min} or newer.",
                        err.message
                    ),
                );
                return ClaudeVersion {
                    line: None,
                    supported: false,
                    refusal: Some(refusal),
                };
            }
        };
        let line = claude::version(&program)
            .map(|text| text.lines().next().unwrap_or("").trim().to_string());
        let refusal = match line.as_deref().map(|l| (l, parse_version(l))) {
            Some((_, Some(found))) if found >= min => None,
            Some((_, Some(found))) => Some(format!(
                "Claude Code {found} is too old for Foundry's hooks: they need {min} or newer (hooks run in exec form). Update Claude Code, then install the hooks again"
            )),
            Some((printed, None)) => Some(format!(
                "couldn't read Claude Code's version: `claude --version` printed {printed:?}. Foundry's hooks need {min} or newer"
            )),
            None => Some(format!(
                "couldn't read Claude Code's version: `claude --version` printed nothing or failed. Foundry's hooks need {min} or newer"
            )),
        };
        ClaudeVersion {
            line,
            supported: refusal.is_none(),
            refusal: refusal.map(|m| AppError::new(ErrorCode::ClaudeVersionUnsupported, m)),
        }
    }

    pub fn status(&self, project_id: &str) -> Result<HooksStatus, AppError> {
        let repo = self.repos.repo(project_id)?;
        let helper = self.helper_path()?;
        Ok(self.status_in(&repo, &helper, &self.claude_version()))
    }

    fn status_in(&self, repo: &Path, helper: &Path, claude: &ClaudeVersion) -> HooksStatus {
        let helper_text = helper.to_string_lossy().into_owned();
        let parsed = repo_write::read_to_string(repo, SETTINGS_FILE)
            .ok()
            .map(|text| match text {
                None => Some(json!({})),
                Some(text) => serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}'))
                    .ok()
                    .filter(Value::is_object),
            })
            .unwrap_or(None);
        let (events, installed) = match &parsed {
            Some(settings) => (
                foundry_events(settings),
                is_installed(settings, &helper_text),
            ),
            None => (Vec::new(), false),
        };
        HooksStatus {
            installed,
            outdated: !events.is_empty() && !installed,
            events,
            helper_exists: helper.is_file(),
            helper_path: helper_text,
            settings_valid: parsed.is_some(),
            git_ignored: self.git_ignored(repo),
            claude_version: claude.line.clone(),
            claude_version_supported: claude.supported,
            min_claude_version: HOOK_ARGS_MIN_VERSION.to_string(),
        }
    }

    fn git_ignored(&self, repo: &Path) -> Option<bool> {
        match self
            .git
            .run(repo, &["check-ignore", "-q", SETTINGS_FILE])
            .ok()?
        {
            GitRun::Ok(_) => Some(true),
            GitRun::Failed(out) if out.status.code() == Some(1) => Some(false),
            _ => None,
        }
    }

    /// Merges Foundry's hooks in. Refused, with nothing written, when Claude Code is missing
    /// or older than [`HOOK_ARGS_MIN_VERSION`] (or its version can't be read), when the helper
    /// isn't there, or when the settings file isn't valid JSON.
    pub fn install(&self, project_id: &str) -> Result<HooksChange, AppError> {
        let repo = self.repos.repo(project_id)?;
        let claude = self.claude_version();
        if let Some(refusal) = claude.refusal.clone() {
            return Err(refusal);
        }
        let helper = self.helper_path()?;
        if !helper.is_file() {
            return Err(AppError::not_found(format!(
                "the foundry-hook helper isn't at {} (in development, build it: cargo build --bin foundry-hook)",
                helper.display()
            )));
        }
        let helper_text = helper.to_string_lossy().into_owned();
        let merged = repo_write::merge_json(&repo, SETTINGS_FILE, |settings| {
            install_into(settings, &helper_text)
        })?;
        tracing::info!(%project_id, changed = merged.changed, "hooks installed");
        Ok(HooksChange {
            status: self.status_in(&repo, &helper, &claude),
            changed: merged.changed,
            backup: merged.backup,
        })
    }

    /// Removes Foundry's hooks, and nothing else. Doesn't need Claude Code or the helper.
    pub fn uninstall(&self, project_id: &str) -> Result<HooksChange, AppError> {
        let repo = self.repos.repo(project_id)?;
        let merged = repo_write::merge_json(&repo, SETTINGS_FILE, uninstall_from)?;
        tracing::info!(%project_id, changed = merged.changed, "hooks uninstalled");
        let helper = self.helper_path()?;
        Ok(HooksChange {
            status: self.status_in(&repo, &helper, &self.claude_version()),
            changed: merged.changed,
            backup: merged.backup,
        })
    }
}

/// The group Foundry writes for one event.
pub fn foundry_entry(helper: &str, matcher: Option<&str>) -> Value {
    let hook = json!({ "type": "command", "command": helper, "args": HELPER_ARGS });
    match matcher {
        Some(matcher) => json!({ "matcher": matcher, "hooks": [hook] }),
        None => json!({ "hooks": [hook] }),
    }
}

fn is_foundry_hook(hook: &Value) -> bool {
    hook.get("command")
        .and_then(Value::as_str)
        .is_some_and(|c| c.to_ascii_lowercase().contains(MARKER))
}

fn group_hooks(group: &Value) -> &[Value] {
    group
        .get("hooks")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn has_foundry(group: &Value) -> bool {
    group_hooks(group).iter().any(is_foundry_hook)
}

fn foundry_events(settings: &Value) -> Vec<String> {
    let Some(hooks) = settings.get("hooks").and_then(Value::as_object) else {
        return Vec::new();
    };
    hooks
        .iter()
        .filter(|(_, list)| list.as_array().is_some_and(|l| l.iter().any(has_foundry)))
        .map(|(event, _)| event.clone())
        .collect()
}

fn is_installed(settings: &Value, helper: &str) -> bool {
    let Some(hooks) = settings.get("hooks").and_then(Value::as_object) else {
        return false;
    };
    let exact = EVENTS.iter().all(|(event, matcher)| {
        let groups: Vec<&Value> = hooks
            .get(*event)
            .and_then(Value::as_array)
            .map(|l| l.iter().filter(|g| has_foundry(g)).collect())
            .unwrap_or_default();
        groups.len() == 1 && *groups[0] == foundry_entry(helper, *matcher)
    });
    let elsewhere = hooks
        .iter()
        .filter(|(event, _)| !EVENTS.iter().any(|(e, _)| e == event))
        .any(|(_, list)| list.as_array().is_some_and(|l| l.iter().any(has_foundry)));
    exact && !elsewhere
}

/// Drops Foundry's hooks from an event's groups, and any group that held only Foundry hooks.
/// Returns whether anything was removed.
fn strip_foundry(list: &mut Vec<Value>) -> bool {
    let before = list.clone();
    list.retain_mut(|group| {
        let Some(hooks) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
            return true;
        };
        let had = hooks.len();
        hooks.retain(|h| !is_foundry_hook(h));
        !(hooks.is_empty() && had > 0)
    });
    *list != before
}

fn hooks_object(
    settings: &mut Value,
) -> Result<Option<&mut serde_json::Map<String, Value>>, AppError> {
    let shape = || {
        AppError::new(
            ErrorCode::InvalidJson,
            format!("{SETTINGS_FILE} has a \"hooks\" entry Foundry can't read (it must be an object of arrays); it was left untouched"),
        )
    };
    let Some(hooks) = settings.get_mut("hooks") else {
        return Ok(None);
    };
    let hooks = hooks.as_object_mut().ok_or_else(shape)?;
    if hooks.values().any(|list| !list.is_array()) {
        return Err(shape());
    }
    Ok(Some(hooks))
}

fn install_into(settings: &mut Value, helper: &str) -> Result<(), AppError> {
    if hooks_object(settings)?.is_none() {
        if let Some(map) = settings.as_object_mut() {
            map.insert("hooks".into(), json!({}));
        }
    }
    let Some(hooks) = hooks_object(settings)? else {
        return Err(AppError::internal("settings aren't an object"));
    };
    for (event, matcher) in EVENTS {
        let wanted = foundry_entry(helper, *matcher);
        let list = hooks.entry(*event).or_insert_with(|| json!([]));
        let Some(list) = list.as_array_mut() else {
            continue;
        };
        let ours: Vec<&Value> = list.iter().filter(|g| has_foundry(g)).collect();
        if ours.len() == 1 && *ours[0] == wanted {
            continue;
        }
        strip_foundry(list);
        list.push(wanted);
    }
    let others: Vec<String> = hooks
        .keys()
        .filter(|event| !EVENTS.iter().any(|(e, _)| e == event))
        .cloned()
        .collect();
    for event in others {
        if let Some(list) = hooks.get_mut(&event).and_then(Value::as_array_mut) {
            if strip_foundry(list) && list.is_empty() {
                hooks.remove(&event);
            }
        }
    }
    Ok(())
}

/// Removes Foundry's hooks; an event list (and `hooks` itself) emptied by that goes too.
/// Lists the user left empty stay.
fn uninstall_from(settings: &mut Value) -> Result<(), AppError> {
    let Some(hooks) = hooks_object(settings)? else {
        return Ok(());
    };
    let events: Vec<String> = hooks.keys().cloned().collect();
    let mut emptied_any = false;
    for event in events {
        if let Some(list) = hooks.get_mut(&event).and_then(Value::as_array_mut) {
            if strip_foundry(list) && list.is_empty() {
                hooks.remove(&event);
                emptied_any = true;
            }
        }
    }
    if emptied_any && hooks.is_empty() {
        if let Some(map) = settings.as_object_mut() {
            map.remove("hooks");
        }
    }
    Ok(())
}
