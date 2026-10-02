//! Which agent sessions are live, and for which project.
//!
//! The agent-sessions feature writes this as sessions start and exit; any feature may read it
//! (archiving a project refuses while it has a live session). Pure state: no processes here.

use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::{AppError, ErrorCode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    /// No session (an agent that isn't running).
    Idle,
    /// Reserved; the process is being spawned.
    Starting,
    Running,
    Exited,
}

/// A live session, as `agent_list` reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub agent_id: String,
    pub project_id: String,
    pub status: SessionStatus,
    pub pid: Option<u32>,
    /// Epoch ms when `status` last changed.
    // u32 stands in for the exported type only (TS `number`), as with EventMeta.ts.
    #[specta(type = u32)]
    pub since: u64,
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Live sessions by agent id. Share it as `Arc<SessionRegistry>`.
#[derive(Debug, Default)]
pub struct SessionRegistry {
    live: RwLock<HashMap<String, SessionInfo>>,
}

impl SessionRegistry {
    fn read(&self) -> RwLockReadGuard<'_, HashMap<String, SessionInfo>> {
        // State stays consistent under a poisoned lock: every write is a single map operation.
        self.live
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, HashMap<String, SessionInfo>> {
        self.live
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Claims a slot for `agent_id` as `starting`, in one step with the checks:
    /// `session_running` if that agent already has a session, `too_many_sessions` if `max`
    /// sessions are live.
    pub fn reserve(&self, agent_id: &str, project_id: &str, max: u32) -> Result<(), AppError> {
        let mut live = self.write();
        if live.contains_key(agent_id) {
            return Err(AppError::new(
                ErrorCode::SessionRunning,
                format!("agent {agent_id} already has a running session"),
            ));
        }
        if live.len() >= max as usize {
            return Err(AppError::new(
                ErrorCode::TooManySessions,
                format!(
                    "{} agent sessions are already running (the limit is {max}); stop one first",
                    live.len()
                ),
            ));
        }
        live.insert(
            agent_id.to_string(),
            SessionInfo {
                agent_id: agent_id.to_string(),
                project_id: project_id.to_string(),
                status: SessionStatus::Starting,
                pid: None,
                since: now_ms(),
            },
        );
        Ok(())
    }

    /// Marks a reserved session running. Returns the updated entry, or `None` if it's gone
    /// (it exited, or was released) — a late call never brings a session back.
    pub fn set_running(&self, agent_id: &str, pid: Option<u32>) -> Option<SessionInfo> {
        let mut live = self.write();
        let info = live.get_mut(agent_id)?;
        info.status = SessionStatus::Running;
        info.pid = pid;
        info.since = now_ms();
        Some(info.clone())
    }

    /// Drops the session (its process exited, or spawning failed).
    pub fn release(&self, agent_id: &str) -> Option<SessionInfo> {
        self.write().remove(agent_id)
    }

    /// Whether any agent has a starting or running session for this project.
    pub fn is_project_active(&self, project_id: &str) -> bool {
        self.read()
            .values()
            .any(|info| info.project_id == project_id)
    }

    pub fn get(&self, agent_id: &str) -> Option<SessionInfo> {
        self.read().get(agent_id).cloned()
    }

    /// Every live session, ordered by agent id.
    pub fn list(&self) -> Vec<SessionInfo> {
        let mut all: Vec<SessionInfo> = self.read().values().cloned().collect();
        all.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_project_active_follows_reserve_running_and_release() {
        let sessions = SessionRegistry::default();
        assert!(!sessions.is_project_active("p1"));

        sessions.reserve("builder", "p1", 3).unwrap();
        assert!(
            sessions.is_project_active("p1"),
            "true from the moment it's reserved"
        );
        assert!(!sessions.is_project_active("p2"));
        assert_eq!(
            sessions.get("builder").unwrap().status,
            SessionStatus::Starting
        );

        let running = sessions.set_running("builder", Some(42)).unwrap();
        assert_eq!(
            (running.status, running.pid),
            (SessionStatus::Running, Some(42))
        );
        assert!(sessions.is_project_active("p1"));

        sessions.reserve("designer", "p1", 3).unwrap();
        sessions.release("builder");
        assert!(sessions.is_project_active("p1"), "designer still runs");
        sessions.release("designer");
        assert!(
            !sessions.is_project_active("p1"),
            "false once the last one is released"
        );
    }

    #[test]
    fn reserve_enforces_the_cap_and_one_session_per_agent() {
        let sessions = SessionRegistry::default();
        sessions.reserve("a", "p1", 2).unwrap();
        assert_eq!(
            sessions.reserve("a", "p1", 2).unwrap_err().code,
            ErrorCode::SessionRunning
        );
        sessions.reserve("b", "p2", 2).unwrap();
        assert_eq!(
            sessions.reserve("c", "p1", 2).unwrap_err().code,
            ErrorCode::TooManySessions
        );
        assert!(
            sessions.get("c").is_none(),
            "a refused reserve leaves nothing behind"
        );

        sessions.release("a");
        sessions.reserve("c", "p1", 2).unwrap();
        let ids: Vec<String> = sessions.list().into_iter().map(|s| s.agent_id).collect();
        assert_eq!(ids, ["b", "c"]);
    }

    #[test]
    fn a_late_set_running_never_revives_a_released_session() {
        let sessions = SessionRegistry::default();
        sessions.reserve("a", "p1", 1).unwrap();
        sessions.release("a");
        assert!(sessions.set_running("a", Some(1)).is_none());
        assert!(!sessions.is_project_active("p1"));
    }
}
