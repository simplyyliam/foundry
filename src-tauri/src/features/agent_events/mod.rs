//! Agent events (P8a): what each agent is doing, from Claude Code hooks.
//!
//! The hook helper (P8b) POSTs every hook call to the local [`server`]; the pure
//! [`classify`] turns it into events; the store appends them to
//! `.foundry/events/<agentId>.jsonl` and emits `agent:event`. Everything is per project:
//! state is keyed by `(projectId, agentId)` and files live in each project's repo, so two
//! projects with an agent of the same id never mix. A timer adds one `quiet` event per quiet
//! stretch of a running session.

pub mod classify;
pub mod commands;
mod glob;
pub mod payload;
pub mod server;
mod store;
#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

use crate::core::repos::RepoIndex;
use crate::core::sessions::{SessionRegistry, SessionStatus};
use crate::core::{repo_write, AppError};
use classify::{Classified, Context, Draft, Streak};
pub use payload::{HookDecision, HookPost};
pub use store::{READ_FILE, ROTATE_AT};

/// A running session with no events for this long gets one `quiet` event.
pub const QUIET_AFTER: Duration = Duration::from_secs(10 * 60);
/// How often the timer looks for quiet sessions.
pub const QUIET_CHECK_EVERY: Duration = Duration::from_secs(30);
/// `agent_events` returns at most this many events (and this many when `limit` is absent).
pub const MAX_LIMIT: u32 = 500;
pub const DEFAULT_LIMIT: u32 = 100;

const ROLES_FILE: &str = "agents/roles.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    SessionStarted,
    Edit,
    Command,
    TestsRan,
    Blocked,
    NeedsAttention,
    TurnComplete,
    SessionEnded,
    Stuck,
    Quiet,
    Danger,
    ReportReady,
}

/// Ordered: `info < warn < danger` (`minSeverity` filters on it).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, Type,
)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warn,
    Danger,
}

/// One thing an agent did, as stored (one JSONL line) and as emitted on `agent:event`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "agent:event")]
pub struct AgentEvent {
    /// 1, 2, 3, … per project and agent; never reused, also across rotation.
    // u32 stands in for the exported type only (TS `number`), as with EventMeta.ts.
    #[specta(type = u32)]
    pub id: u64,
    /// Epoch ms.
    #[specta(type = u32)]
    pub ts: u64,
    pub project_id: String,
    pub agent_id: String,
    pub slice_id: Option<String>,
    pub kind: EventKind,
    pub severity: Severity,
    pub title: String,
    pub detail: Option<String>,
    /// Repo-relative with `/` when the file is inside the repo.
    pub path: Option<String>,
    pub command: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventsQuery {
    /// Only events with a smaller id (paging back from the oldest one shown).
    pub before_id: Option<u32>,
    /// 1–500, default 100.
    pub limit: Option<u32>,
    pub min_severity: Option<Severity>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AgentEventCounts {
    pub agent_id: String,
    /// Events after the agent's read marker.
    pub unread: u32,
    /// Of those, the ones with severity `danger`.
    pub danger: u32,
}

/// Where emitted events go (the webview, in the app).
pub trait AgentEventSink: Send + Sync {
    fn event(&self, event: AgentEvent);
}

/// Epoch ms. Injectable so tests can move time.
pub trait Clock: Send + Sync {
    fn now_ms(&self) -> u64;
}

pub struct SystemClock;

impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        crate::core::sessions::now_ms()
    }
}

/// Agent ids name files: 1–64 ASCII letters, digits, `-` or `_`.
pub fn valid_agent_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn check_agent_id(id: &str) -> Result<(), AppError> {
    if valid_agent_id(id) {
        Ok(())
    } else {
        Err(AppError::invalid_input(format!(
            "agentId must be 1-64 letters, digits, '-' or '_': {id:?}"
        )))
    }
}

/// In-memory state of one `(projectId, agentId)`.
#[derive(Debug, Default)]
struct AgentState {
    /// Whether `last_id` and `torn` were read from disk yet.
    loaded: bool,
    last_id: u64,
    torn: bool,
    streak: Streak,
    /// Epoch ms of the last hook call accepted for this agent.
    last_activity: Option<u64>,
    /// The start of the quiet stretch that already got its `quiet` event.
    quiet_for: Option<u64>,
    slice_id: Option<String>,
}

type Key = (String, String);

pub struct AgentEventsStore {
    repos: Arc<RepoIndex>,
    sessions: Arc<SessionRegistry>,
    sink: Arc<dyn AgentEventSink>,
    clock: Arc<dyn Clock>,
    rotate_at: u64,
    quiet_after_ms: u64,
    /// One lock for every agent: appends, id assignment and markers stay in order.
    state: Mutex<HashMap<Key, AgentState>>,
}

impl AgentEventsStore {
    pub fn new(
        repos: Arc<RepoIndex>,
        sessions: Arc<SessionRegistry>,
        sink: Arc<dyn AgentEventSink>,
    ) -> Self {
        Self {
            repos,
            sessions,
            sink,
            clock: Arc::new(SystemClock),
            rotate_at: ROTATE_AT,
            quiet_after_ms: QUIET_AFTER.as_millis() as u64,
            state: Mutex::new(HashMap::new()),
        }
    }

    pub fn with_clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = clock;
        self
    }

    /// Tests only need small files to see rotation.
    pub fn with_rotate_at(mut self, bytes: u64) -> Self {
        self.rotate_at = bytes;
        self
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<Key, AgentState>> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn loaded<'a>(
        state: &'a mut HashMap<Key, AgentState>,
        repo: &Path,
        project_id: &str,
        agent_id: &str,
    ) -> Result<&'a mut AgentState, AppError> {
        let entry = state
            .entry((project_id.to_string(), agent_id.to_string()))
            .or_default();
        if !entry.loaded {
            let (last_id, torn) = store::tail(repo, agent_id)?;
            entry.last_id = last_id;
            entry.torn = torn;
            entry.loaded = true;
        }
        Ok(entry)
    }

    /// The checks the server makes before it answers: a valid agent id and a known project
    /// (`not_found`, or `folder_missing`).
    pub fn check(&self, project_id: &str, agent_id: &str) -> Result<(), AppError> {
        check_agent_id(agent_id)?;
        self.repos.repo(project_id).map(|_| ())
    }

    /// Classifies one hook POST, stores the events and emits them. Returns them in order.
    pub fn ingest(&self, post: &HookPost) -> Result<Vec<AgentEvent>, AppError> {
        check_agent_id(&post.agent_id)?;
        let repo = self.repos.repo(&post.project_id)?;
        let protected = protected_globs(&repo, &post.agent_id);
        let now = self.clock.now_ms();

        let events = {
            let mut state = self.lock();
            let agent = Self::loaded(&mut state, &repo, &post.project_id, &post.agent_id)?;
            let Classified { drafts, streak } = classify::classify(
                post,
                &Context {
                    repo_root: Some(&repo),
                    protected_globs: &protected,
                    streak: Some(&agent.streak),
                },
            );
            agent.streak = streak;
            agent.last_activity = Some(now);
            if let Some(slice) = post.slice_id.as_deref().filter(|s| !s.trim().is_empty()) {
                agent.slice_id = Some(slice.trim().to_string());
            }
            let mut events = Vec::with_capacity(drafts.len());
            for draft in drafts {
                events.push(self.store(
                    &repo,
                    &post.project_id,
                    &post.agent_id,
                    agent,
                    draft,
                    now,
                )?);
            }
            events
        };
        for event in &events {
            self.sink.event(event.clone());
        }
        Ok(events)
    }

    fn store(
        &self,
        repo: &Path,
        project_id: &str,
        agent_id: &str,
        agent: &mut AgentState,
        draft: Draft,
        now: u64,
    ) -> Result<AgentEvent, AppError> {
        let event = AgentEvent {
            id: agent.last_id + 1,
            ts: now,
            project_id: project_id.to_string(),
            agent_id: agent_id.to_string(),
            slice_id: draft.slice_id,
            kind: draft.kind,
            severity: draft.severity,
            title: draft.title,
            detail: draft.detail,
            path: draft.path,
            command: draft.command,
        };
        store::append(repo, &event, self.rotate_at, agent.torn)?;
        agent.last_id = event.id;
        agent.torn = false;
        tracing::debug!(%project_id, %agent_id, id = event.id, kind = ?event.kind, "agent event");
        Ok(event)
    }

    /// The agent's events, newest first.
    pub fn events(
        &self,
        project_id: &str,
        agent_id: &str,
        query: &AgentEventsQuery,
    ) -> Result<Vec<AgentEvent>, AppError> {
        check_agent_id(agent_id)?;
        let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
        if limit == 0 || limit > MAX_LIMIT {
            return Err(AppError::invalid_input(format!(
                "limit must be 1-{MAX_LIMIT}, not {limit}"
            )));
        }
        let repo = self.repos.repo(project_id)?;
        let _order = self.lock();
        let mut events: Vec<AgentEvent> = store::read_all(&repo, agent_id)?
            .into_iter()
            .filter(|e| {
                query
                    .before_id
                    .map_or(true, |before| e.id < u64::from(before))
            })
            .filter(|e| query.min_severity.map_or(true, |min| e.severity >= min))
            .collect();
        events.sort_by(|a, b| b.id.cmp(&a.id));
        events.dedup_by_key(|e| e.id);
        events.truncate(limit as usize);
        Ok(events)
    }

    /// Unread and unread-danger counts for every agent with events in the project.
    pub fn counts(&self, project_id: &str) -> Result<Vec<AgentEventCounts>, AppError> {
        let repo = self.repos.repo(project_id)?;
        let _order = self.lock();
        let markers = store::read_markers(&repo)?;
        store::agents_with_events(&repo)?
            .into_iter()
            .map(|agent_id| {
                let read = markers.get(&agent_id).copied().unwrap_or(0);
                let unread: Vec<AgentEvent> = store::read_all(&repo, &agent_id)?
                    .into_iter()
                    .filter(|e| e.id > read)
                    .collect();
                let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
                Ok(AgentEventCounts {
                    unread: count(unread.len()),
                    danger: count(
                        unread
                            .iter()
                            .filter(|e| e.severity == Severity::Danger)
                            .count(),
                    ),
                    agent_id,
                })
            })
            .collect()
    }

    /// Marks the agent's events up to `up_to_id` read. Never moves the marker back, and never
    /// past the newest event (so later events still arrive unread).
    pub fn mark_read(
        &self,
        project_id: &str,
        agent_id: &str,
        up_to_id: u64,
    ) -> Result<(), AppError> {
        check_agent_id(agent_id)?;
        let repo = self.repos.repo(project_id)?;
        let mut state = self.lock();
        let last = Self::loaded(&mut state, &repo, project_id, agent_id)?.last_id;
        let mut markers = store::read_markers(&repo)?;
        let current = markers.get(agent_id).copied().unwrap_or(0);
        let next = up_to_id.min(last).max(current);
        if next != current {
            markers.insert(agent_id.to_string(), next);
            store::write_markers(&repo, &markers)?;
            tracing::info!(%project_id, %agent_id, up_to = next, "agent events marked read");
        }
        Ok(())
    }

    /// One timer step: a `quiet` event for each running session whose current quiet stretch
    /// (since its last hook call, or since it started) has lasted [`QUIET_AFTER`] and hasn't
    /// had one yet. Returns what it emitted.
    pub fn check_quiet(&self) -> Vec<AgentEvent> {
        let now = self.clock.now_ms();
        let mut emitted = Vec::new();
        for session in self.sessions.list() {
            if session.status != SessionStatus::Running || !valid_agent_id(&session.agent_id) {
                continue;
            }
            let Ok(repo) = self.repos.repo(&session.project_id) else {
                continue;
            };
            let mut state = self.lock();
            let result = Self::loaded(&mut state, &repo, &session.project_id, &session.agent_id)
                .and_then(|agent| {
                    let start = agent.last_activity.unwrap_or(0).max(session.since);
                    if now.saturating_sub(start) < self.quiet_after_ms
                        || agent.quiet_for == Some(start)
                    {
                        return Ok(None);
                    }
                    let minutes = self.quiet_after_ms / 60_000;
                    let draft = Draft {
                        slice_id: agent.slice_id.clone(),
                        kind: EventKind::Quiet,
                        severity: Severity::Info,
                        title: format!("No activity for {minutes} minutes"),
                        detail: None,
                        path: None,
                        command: None,
                    };
                    let event = self.store(
                        &repo,
                        &session.project_id,
                        &session.agent_id,
                        agent,
                        draft,
                        now,
                    )?;
                    agent.quiet_for = Some(start);
                    Ok(Some(event))
                });
            drop(state);
            match result {
                Ok(Some(event)) => {
                    self.sink.event(event.clone());
                    emitted.push(event);
                }
                Ok(None) => {}
                Err(err) => {
                    tracing::warn!(agent_id = %session.agent_id, error = %err, "quiet check failed")
                }
            }
        }
        emitted
    }

    /// Runs [`check_quiet`](Self::check_quiet) every `every` on the async runtime.
    pub fn spawn_quiet_timer(
        self: &Arc<Self>,
        every: Duration,
    ) -> tauri::async_runtime::JoinHandle<()> {
        let store = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            let mut ticks = tokio::time::interval(every);
            ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                ticks.tick().await;
                let store = Arc::clone(&store);
                let _ = tauri::async_runtime::spawn_blocking(move || store.check_quiet()).await;
            }
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RolesFile {
    #[serde(default)]
    agents: Vec<RoleEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoleEntry {
    id: String,
    #[serde(default)]
    protected_globs: Vec<String>,
}

/// The agent's `protectedGlobs` from `agents/roles.json` (written by the team feature; read
/// here from disk). Missing or unreadable → none.
fn protected_globs(repo: &Path, agent_id: &str) -> Vec<String> {
    let text = match repo_write::read_to_string(repo, ROLES_FILE) {
        Ok(Some(text)) => text,
        Ok(None) => return Vec::new(),
        Err(err) => {
            tracing::warn!(error = %err, "could not read roles.json");
            return Vec::new();
        }
    };
    match serde_json::from_str::<RolesFile>(&text) {
        Ok(roles) => roles
            .agents
            .into_iter()
            .find(|a| a.id == agent_id)
            .map(|a| a.protected_globs)
            .unwrap_or_default(),
        Err(err) => {
            tracing::warn!(error = %err, "roles.json doesn't parse; no protected globs");
            Vec::new()
        }
    }
}
