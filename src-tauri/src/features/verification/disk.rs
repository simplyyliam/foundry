//! What verification reads from files other features own, each through a minimal local struct
//! (never an import of that feature), plus the confirmed test commands kept in app data.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::core::{fsx, paths, repo_write, AppError, ErrorCode};

pub const PROJECT_FILE: &str = ".foundry/project.json";
pub const PLAN_FILE: &str = ".foundry/plan.json";
pub const ROLES_FILE: &str = "agents/roles.json";
pub const REVIEWS_DIR: &str = ".foundry/reviews";
pub const DIGEST_FILE: &str = ".foundry/reviews/digest.md";
pub const EVENTS_DIR: &str = ".foundry/events";
pub const READ_FILE: &str = ".foundry/events/read.json";
const TEST_COMMANDS_KEY: &str = "testCommands";
/// In app data, never in a repo.
const CONFIRMED_FILE: &str = "verification/confirmed-test-commands.json";

pub fn review_path(slice_id: &str) -> String {
    format!("{REVIEWS_DIR}/{slice_id}.json")
}

fn parse(text: &str) -> Result<Value, serde_json::Error> {
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
}

/// A repo file as JSON: `None` when it's missing, `invalid_json` when it doesn't parse.
pub fn read_json(repo: &Path, rel: &str) -> Result<Option<Value>, AppError> {
    repo_write::read_to_string(repo, rel)?
        .map(|text| {
            parse(&text).map_err(|err| {
                AppError::new(
                    ErrorCode::InvalidJson,
                    format!("{rel} isn't valid JSON ({err})"),
                )
            })
        })
        .transpose()
}

/// Like [`read_json`], but a file that doesn't parse reads as missing.
pub fn read_json_lenient(repo: &Path, rel: &str) -> Result<Option<Value>, AppError> {
    Ok(repo_write::read_to_string(repo, rel)?.and_then(|text| parse(&text).ok()))
}

// ---------- .foundry/project.json ----------

/// `testCommands` from `.foundry/project.json`, exactly as stored. Missing file or key: none.
pub fn test_commands(repo: &Path) -> Result<Vec<String>, AppError> {
    let Some(project) = read_json(repo, PROJECT_FILE)? else {
        return Ok(Vec::new());
    };
    match project.get(TEST_COMMANDS_KEY) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(value) => serde_json::from_value(value.clone()).map_err(|_| {
            AppError::new(
                ErrorCode::InvalidJson,
                format!("{TEST_COMMANDS_KEY} in {PROJECT_FILE} must be a list of strings"),
            )
        }),
    }
}

/// Sets `testCommands` in `.foundry/project.json`, keeping every other field.
pub fn set_test_commands(repo: &Path, commands: &[String]) -> Result<(), AppError> {
    let mut project = read_json(repo, PROJECT_FILE)?
        .ok_or_else(|| AppError::not_found(format!("{PROJECT_FILE} is missing")))?;
    let Some(object) = project.as_object_mut() else {
        return Err(AppError::new(
            ErrorCode::InvalidJson,
            format!("{PROJECT_FILE} must hold a JSON object"),
        ));
    };
    object.insert(TEST_COMMANDS_KEY.into(), commands.into());
    let text = serde_json::to_string_pretty(&project)
        .map_err(|err| AppError::internal(format!("serialize {PROJECT_FILE}: {err}")))?;
    repo_write::write(repo, PROJECT_FILE, text + "\n")
}

// ---------- confirmed test commands (app data) ----------

fn confirmed_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CONFIRMED_FILE)
}

/// Project id → the command list the human last confirmed, verbatim. Missing reads as empty;
/// so does an unreadable file, which leaves every project unconfirmed (fail closed).
pub fn read_confirmed(data_dir: &Path) -> Result<BTreeMap<String, Vec<String>>, AppError> {
    match fs::read_to_string(confirmed_path(data_dir)) {
        Ok(text) => Ok(serde_json::from_str(&text).unwrap_or_else(|err| {
            tracing::warn!(error = %err, "unreadable confirmed test commands; none are confirmed");
            BTreeMap::new()
        })),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(BTreeMap::new()),
        Err(err) => Err(err.into()),
    }
}

pub fn write_confirmed(
    data_dir: &Path,
    confirmed: &BTreeMap<String, Vec<String>>,
) -> Result<(), AppError> {
    let path = confirmed_path(data_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let text = serde_json::to_string_pretty(confirmed)
        .map_err(|err| AppError::internal(format!("serialize confirmed commands: {err}")))?;
    fsx::write_atomic(&path, text + "\n")
}

// ---------- .foundry/plan.json ----------

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    #[serde(default)]
    pub slices: Vec<PlanSlice>,
    #[serde(default)]
    pub requirements: Vec<PlanRequirement>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSlice {
    pub id: String,
    #[serde(default)]
    pub name: String,
    /// `queued`, `in_progress` or `done`.
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub start_commit: Option<String>,
    /// The root workspace files are writable too (`hook_policy::WORKSPACE_FILES`).
    #[serde(default)]
    pub workspace_access: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanRequirement {
    pub id: String,
    #[serde(default)]
    pub slice_id: Option<String>,
}

/// The plan, or an empty one if there's no `plan.json` yet.
pub fn plan(repo: &Path) -> Result<Plan, AppError> {
    match read_json(repo, PLAN_FILE)? {
        None => Ok(Plan::default()),
        Some(value) => serde_json::from_value(value).map_err(|err| {
            AppError::new(
                ErrorCode::InvalidJson,
                format!("{PLAN_FILE} is unreadable ({err})"),
            )
        }),
    }
}

// ---------- agents/roles.json ----------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Role {
    pub id: String,
    #[serde(default)]
    pub write_globs: Vec<String>,
    #[serde(default)]
    pub protected_globs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RolesFile {
    #[serde(default)]
    agents: Vec<Role>,
}

/// The team's roles; `None` when `roles.json` is missing or doesn't parse.
pub fn roles(repo: &Path) -> Result<Option<Vec<Role>>, AppError> {
    Ok(read_json_lenient(repo, ROLES_FILE)?
        .and_then(|value| serde_json::from_value::<RolesFile>(value).ok())
        .map(|file| file.agents))
}

// ---------- .foundry/events ----------

/// The fields of one stored agent event the digest needs.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredEvent {
    pub id: u64,
    pub ts: u64,
    pub agent_id: String,
    #[serde(default)]
    pub slice_id: Option<String>,
    /// `info`, `warn` or `danger`.
    pub severity: String,
    #[serde(default)]
    pub title: String,
}

fn valid_agent_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Agent ids with an event file (`<id>.jsonl` or the rotated `<id>.1.jsonl`), sorted.
pub fn agents_with_events(repo: &Path) -> Result<Vec<String>, AppError> {
    let dir = paths::resolve(repo, EVENTS_DIR)?;
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let mut ids: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem = name.strip_suffix(".jsonl")?;
            let id = stem.strip_suffix(".1").unwrap_or(stem);
            valid_agent_id(id).then(|| id.to_string())
        })
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// The agent's stored events, older file first. Lines that don't parse are skipped.
pub fn events(repo: &Path, agent_id: &str) -> Result<Vec<StoredEvent>, AppError> {
    if !valid_agent_id(agent_id) {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for rel in [
        format!("{EVENTS_DIR}/{agent_id}.1.jsonl"),
        format!("{EVENTS_DIR}/{agent_id}.jsonl"),
    ] {
        if let Some(text) = repo_write::read_to_string(repo, &rel)? {
            out.extend(
                text.lines()
                    .filter_map(|line| serde_json::from_str::<StoredEvent>(line).ok())
                    .filter(|event| event.agent_id == agent_id),
            );
        }
    }
    Ok(out)
}

/// Agent id → the highest event id the human has read. Missing or unreadable: none read.
pub fn read_markers(repo: &Path) -> Result<BTreeMap<String, u64>, AppError> {
    Ok(read_json_lenient(repo, READ_FILE)?
        .and_then(|value| serde_json::from_value(value).ok())
        .unwrap_or_default())
}
