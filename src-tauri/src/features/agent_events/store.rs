//! Event files in the repo: `.foundry/events/<agentId>.jsonl` (append-only, one event per
//! line), the rotated `<agentId>.1.jsonl` before it, and read markers in `read.json`. All
//! writes go through `core::repo_write`; reads through `core::paths`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::AgentEvent;
use crate::core::{paths, repo_write, AppError};

pub const EVENTS_DIR: &str = ".foundry/events";
pub const READ_FILE: &str = ".foundry/events/read.json";
/// The current file is rotated once appending would take it past this size.
pub const ROTATE_AT: u64 = 5 * 1024 * 1024;

pub fn current_file(agent_id: &str) -> String {
    format!("{EVENTS_DIR}/{agent_id}.jsonl")
}

pub fn previous_file(agent_id: &str) -> String {
    format!("{EVENTS_DIR}/{agent_id}.1.jsonl")
}

fn size(repo: &Path, rel: &str) -> Result<u64, AppError> {
    let path = paths::resolve(repo, rel)?;
    match fs::metadata(path) {
        Ok(meta) => Ok(meta.len()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(0),
        Err(err) => Err(err.into()),
    }
}

/// Lines that parse as events. A torn or hand-mangled line is skipped, not an error.
fn parse_lines(text: &str) -> impl Iterator<Item = AgentEvent> + '_ {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str(line).ok())
}

fn read_file(repo: &Path, rel: &str) -> Result<Option<String>, AppError> {
    repo_write::read_to_string(repo, rel)
}

/// Where the agent's log stands: the highest id on disk (0 for none) and whether the current
/// file ends mid-line (a torn write), so the next append starts on a fresh line.
pub fn tail(repo: &Path, agent_id: &str) -> Result<(u64, bool), AppError> {
    let current = read_file(repo, &current_file(agent_id))?.unwrap_or_default();
    let torn = !current.is_empty() && !current.ends_with('\n');
    let mut last = parse_lines(&current).map(|e| e.id).max().unwrap_or(0);
    if last == 0 {
        let previous = read_file(repo, &previous_file(agent_id))?.unwrap_or_default();
        last = parse_lines(&previous).map(|e| e.id).max().unwrap_or(0);
    }
    Ok((last, torn))
}

/// Appends one event as a line, rotating first when the line would take the current file
/// past `rotate_at`: the current file becomes `<agentId>.1.jsonl`, replacing the older one,
/// so at most the last two files are kept.
pub fn append(repo: &Path, event: &AgentEvent, rotate_at: u64, torn: bool) -> Result<(), AppError> {
    let mut line = serde_json::to_string(event)
        .map_err(|err| AppError::internal(format!("serialize event: {err}")))?;
    line.push('\n');
    let current = current_file(&event.agent_id);
    let existing = size(repo, &current)?;
    let rotate = existing > 0 && existing + line.len() as u64 > rotate_at;
    if rotate {
        let previous = previous_file(&event.agent_id);
        repo_write::remove(repo, &previous)?;
        if !repo_write::rename_new(repo, &current, &previous)? {
            return Err(AppError::internal(format!(
                "could not rotate {current}: {previous} reappeared"
            )));
        }
        tracing::info!(agent_id = %event.agent_id, "rotated agent events");
    } else if torn {
        line.insert(0, '\n');
    }
    repo_write::append(repo, &current, line)
}

/// Every stored event of the agent (both files), in file order.
pub fn read_all(repo: &Path, agent_id: &str) -> Result<Vec<AgentEvent>, AppError> {
    let mut events = Vec::new();
    for rel in [previous_file(agent_id), current_file(agent_id)] {
        if let Some(text) = read_file(repo, &rel)? {
            events.extend(parse_lines(&text).filter(|e| e.agent_id == agent_id));
        }
    }
    Ok(events)
}

/// Agent ids with an event file, sorted.
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
            super::valid_agent_id(id).then(|| id.to_string())
        })
        .collect();
    ids.sort();
    ids.dedup();
    Ok(ids)
}

/// Agent id → the highest id the user has read. A missing or unreadable file reads as empty.
pub fn read_markers(repo: &Path) -> Result<BTreeMap<String, u64>, AppError> {
    let Some(text) = read_file(repo, READ_FILE)? else {
        return Ok(BTreeMap::new());
    };
    Ok(serde_json::from_str(&text).unwrap_or_else(|err| {
        tracing::warn!(error = %err, "unreadable read markers; treating every event as unread");
        BTreeMap::new()
    }))
}

pub fn write_markers(repo: &Path, markers: &BTreeMap<String, u64>) -> Result<(), AppError> {
    let mut text = serde_json::to_string_pretty(markers)
        .map_err(|err| AppError::internal(format!("serialize read markers: {err}")))?;
    text.push('\n');
    repo_write::write(repo, READ_FILE, text)
}
