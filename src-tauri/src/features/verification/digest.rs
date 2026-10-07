//! The manager digest: per agent, per slice, and what needs the human's attention. Built from
//! files on disk (events, read markers, plan, reports, reviews) and the live sessions.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

use super::{disk, FindingSeverity};
use crate::core::report::report_path;
use crate::core::sessions::{SessionRegistry, SessionStatus};
use crate::core::{repo_write, AppError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentDigest {
    pub agent_id: String,
    /// It has a starting or running session.
    pub running: bool,
    /// Epoch ms of its newest event; `null` if it has none.
    #[specta(type = Option<u32>)]
    pub last_event_at: Option<u64>,
    pub unread: u32,
    pub unread_danger: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum VerificationState {
    Passed,
    Failed,
    /// Never verified.
    None,
    /// The report changed (or is gone) since it was verified.
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SliceDigest {
    pub slice_id: String,
    pub name: String,
    /// The plan's status: `queued`, `in_progress` or `done`.
    pub status: String,
    pub report_present: bool,
    pub verification: VerificationState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum AttentionKind {
    /// An unread `warn` or `danger` agent event.
    UnreadEvent,
    FailedVerification,
    /// A report that was never verified.
    UnverifiedReport,
    StaleVerification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AttentionItem {
    /// `warn` or `danger`.
    pub severity: FindingSeverity,
    pub kind: AttentionKind,
    pub agent_id: Option<String>,
    pub slice_id: Option<String>,
    pub message: String,
    /// The event's id, for `unread_event`.
    #[specta(type = Option<u32>)]
    pub event_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ManagerDigest {
    pub project_id: String,
    /// Epoch ms.
    #[specta(type = u32)]
    pub generated_at: u64,
    /// The team's agents in `roles.json` order, then any others with events, by id.
    pub agents: Vec<AgentDigest>,
    /// In plan order.
    pub slices: Vec<SliceDigest>,
    /// Most severe first; equal severities keep slices before events.
    pub attention: Vec<AttentionItem>,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn verification_state(repo: &Path, slice_id: &str) -> Result<VerificationState, AppError> {
    #[derive(Deserialize)]
    struct Stored {
        passed: bool,
        report: Value,
    }
    let Some(review) = disk::read_json_lenient(repo, &disk::review_path(slice_id))?
        .and_then(|value| serde_json::from_value::<Stored>(value).ok())
    else {
        return Ok(VerificationState::None);
    };
    let current = disk::read_json_lenient(repo, &report_path(slice_id))?;
    Ok(if current.as_ref() != Some(&review.report) {
        VerificationState::Stale
    } else if review.passed {
        VerificationState::Passed
    } else {
        VerificationState::Failed
    })
}

pub fn build(
    repo: &Path,
    project_id: &str,
    sessions: &SessionRegistry,
    now: u64,
) -> Result<ManagerDigest, AppError> {
    let running: BTreeSet<String> = sessions
        .list_project(project_id)
        .into_iter()
        .filter(|s| matches!(s.status, SessionStatus::Starting | SessionStatus::Running))
        .map(|s| s.agent_id)
        .collect();
    let mut agent_ids: Vec<String> = disk::roles(repo)?
        .unwrap_or_default()
        .into_iter()
        .map(|r| r.id)
        .collect();
    let extra: BTreeSet<String> = disk::agents_with_events(repo)?
        .into_iter()
        .chain(running.iter().cloned())
        .filter(|id| !agent_ids.contains(id))
        .collect();
    agent_ids.extend(extra);

    let markers = disk::read_markers(repo)?;
    let mut agents = Vec::new();
    let mut event_items = Vec::new();
    for agent_id in agent_ids {
        let events = disk::events(repo, &agent_id)?;
        let read = markers.get(&agent_id).copied().unwrap_or(0);
        let unread: Vec<_> = events.iter().filter(|e| e.id > read).collect();
        for event in &unread {
            let severity = match event.severity.as_str() {
                "danger" => FindingSeverity::Danger,
                "warn" => FindingSeverity::Warn,
                _ => continue,
            };
            event_items.push(AttentionItem {
                severity,
                kind: AttentionKind::UnreadEvent,
                agent_id: Some(agent_id.clone()),
                slice_id: event.slice_id.clone(),
                message: event.title.clone(),
                event_id: Some(event.id),
            });
        }
        agents.push(AgentDigest {
            running: running.contains(&agent_id),
            last_event_at: events.iter().map(|e| e.ts).max(),
            unread: count(unread.len()),
            unread_danger: count(unread.iter().filter(|e| e.severity == "danger").count()),
            agent_id,
        });
    }

    let mut slices = Vec::new();
    let mut attention = Vec::new();
    for slice in disk::plan(repo)?.slices {
        let report_present = repo_write::read_to_string(repo, &report_path(&slice.id))?.is_some();
        let verification = verification_state(repo, &slice.id)?;
        let label = format!("\"{}\"", slice.name);
        let item = match verification {
            VerificationState::Failed => Some((
                FindingSeverity::Danger,
                AttentionKind::FailedVerification,
                format!("{label} failed verification"),
            )),
            VerificationState::Stale => Some((
                FindingSeverity::Warn,
                AttentionKind::StaleVerification,
                format!("{label}'s report changed since it was verified"),
            )),
            VerificationState::None if report_present => Some((
                FindingSeverity::Warn,
                AttentionKind::UnverifiedReport,
                format!("{label} has a report that hasn't been verified"),
            )),
            _ => None,
        };
        if let Some((severity, kind, message)) = item {
            attention.push(AttentionItem {
                severity,
                kind,
                agent_id: None,
                slice_id: Some(slice.id.clone()),
                message,
                event_id: None,
            });
        }
        slices.push(SliceDigest {
            slice_id: slice.id,
            name: slice.name,
            status: slice.status,
            report_present,
            verification,
        });
    }
    attention.extend(event_items);
    // Stable: equal severities keep their order.
    attention.sort_by(|a, b| b.severity.cmp(&a.severity));

    Ok(ManagerDigest {
        project_id: project_id.to_string(),
        generated_at: now,
        agents,
        slices,
        attention,
    })
}

// ---------- markdown ----------

/// Epoch ms → `2026-10-07 14:03 UTC`.
pub fn utc(ms: u64) -> String {
    let secs = ms / 1000;
    let days = i64::try_from(secs / 86_400).unwrap_or(0);
    let (hour, minute) = ((secs % 86_400) / 3600, (secs % 3600) / 60);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02} UTC")
}

/// One line of untrusted text, safe in a table cell or a list item.
fn cell(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('\\', "\\\\")
        .replace('|', "\\|")
        .replace('`', "'")
}

fn severity_word(severity: FindingSeverity) -> &'static str {
    match severity {
        FindingSeverity::Info => "info",
        FindingSeverity::Warn => "warn",
        FindingSeverity::Danger => "danger",
    }
}

fn state_word(state: VerificationState) -> &'static str {
    match state {
        VerificationState::Passed => "passed",
        VerificationState::Failed => "failed",
        VerificationState::None => "none",
        VerificationState::Stale => "stale",
    }
}

/// The digest as readable markdown (`.foundry/reviews/digest.md`).
pub fn markdown(digest: &ManagerDigest) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Manager digest\n");
    let _ = writeln!(
        out,
        "Project `{}`, generated {}. Written by Foundry; regenerated on request.\n",
        cell(&digest.project_id),
        utc(digest.generated_at)
    );

    let _ = writeln!(out, "## Needs attention\n");
    if digest.attention.is_empty() {
        let _ = writeln!(out, "Nothing needs attention.");
    }
    for item in &digest.attention {
        let mut whose = Vec::new();
        if let Some(agent) = &item.agent_id {
            whose.push(format!("agent {}", cell(agent)));
        }
        if let Some(slice) = &item.slice_id {
            whose.push(format!("slice {}", cell(slice)));
        }
        let _ = writeln!(
            out,
            "- **{}** ({}): {}",
            severity_word(item.severity),
            whose.join(", "),
            cell(&item.message)
        );
    }

    let _ = writeln!(out, "\n## Agents\n");
    if digest.agents.is_empty() {
        let _ = writeln!(out, "No agents yet.");
    } else {
        let _ = writeln!(
            out,
            "| Agent | Running | Last event | Unread | Unread danger |"
        );
        let _ = writeln!(out, "| --- | --- | --- | --- | --- |");
        for agent in &digest.agents {
            let _ = writeln!(
                out,
                "| {} | {} | {} | {} | {} |",
                cell(&agent.agent_id),
                if agent.running { "yes" } else { "no" },
                agent.last_event_at.map_or_else(|| "none".to_string(), utc),
                agent.unread,
                agent.unread_danger
            );
        }
    }

    let _ = writeln!(out, "\n## Slices\n");
    if digest.slices.is_empty() {
        let _ = writeln!(out, "No slices yet.");
    } else {
        let _ = writeln!(out, "| Slice | Status | Report | Verification |");
        let _ = writeln!(out, "| --- | --- | --- | --- |");
        for slice in &digest.slices {
            let _ = writeln!(
                out,
                "| {} ({}) | {} | {} | {} |",
                cell(&slice.name),
                cell(&slice.slice_id),
                cell(&slice.status.replace('_', " ")),
                if slice.report_present { "yes" } else { "no" },
                state_word(slice.verification)
            );
        }
    }
    out
}
