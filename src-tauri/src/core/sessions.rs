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

/// A session as `agent_list` reports it: a live one (`starting` or `running`, held here), or
/// an agent's last finished one (`exited`, held by the agent-sessions feature until the agent
/// starts again or the app exits). This registry only ever holds live sessions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    pub agent_id: String,
    pub project_id: String,
    pub status: SessionStatus,
    /// The process id while it runs; `null` once it has exited.
    pub pid: Option<u32>,
    /// Set once `exited`, when the process reported a code; `null` while it runs, and when it
    /// was killed without one.
    pub exit_code: Option<u32>,
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

/// Identifies a session. Agent ids are slugs of display names, so two projects can have an
/// agent with the same id: a session is always the pair.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SessionKey {
    pub project_id: String,
    pub agent_id: String,
}

impl SessionKey {
    pub fn new(project_id: &str, agent_id: &str) -> Self {
        Self {
            project_id: project_id.to_string(),
            agent_id: agent_id.to_string(),
        }
    }
}

/// Live sessions by (project id, agent id). Share it as `Arc<SessionRegistry>`.
#[derive(Debug, Default)]
pub struct SessionRegistry {
    live: RwLock<HashMap<SessionKey, SessionInfo>>,
}

impl SessionRegistry {
    fn read(&self) -> RwLockReadGuard<'_, HashMap<SessionKey, SessionInfo>> {
        // State stays consistent under a poisoned lock: every write is a single map operation.
        self.live
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write(&self) -> RwLockWriteGuard<'_, HashMap<SessionKey, SessionInfo>> {
        self.live
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Claims a slot for the agent as `starting`, in one step with the checks:
    /// `session_running` if that agent already has a session in that project,
    /// `too_many_sessions` if `max` sessions are live across all projects.
    pub fn reserve(&self, key: &SessionKey, max: u32) -> Result<(), AppError> {
        self.reserve_if(key, max, || Ok(()))
    }

    /// Like [`reserve`](Self::reserve), with `allowed` checked under the same lock first. Pair
    /// it with [`unless_project_active`](Self::unless_project_active) to make "may this
    /// project start a session?" and "may it be archived?" exclusive.
    pub fn reserve_if(
        &self,
        key: &SessionKey,
        max: u32,
        allowed: impl FnOnce() -> Result<(), AppError>,
    ) -> Result<(), AppError> {
        let mut live = self.write();
        allowed()?;
        if live.contains_key(key) {
            return Err(AppError::new(
                ErrorCode::SessionRunning,
                format!(
                    "agent {} already has a running session in this project",
                    key.agent_id
                ),
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
            key.clone(),
            SessionInfo {
                agent_id: key.agent_id.clone(),
                project_id: key.project_id.clone(),
                status: SessionStatus::Starting,
                pid: None,
                exit_code: None,
                since: now_ms(),
            },
        );
        Ok(())
    }

    /// Marks a reserved session running. Returns the updated entry, or `None` if it's gone
    /// (it exited, or was released) — a late call never brings a session back.
    pub fn set_running(&self, key: &SessionKey, pid: Option<u32>) -> Option<SessionInfo> {
        let mut live = self.write();
        let info = live.get_mut(key)?;
        info.status = SessionStatus::Running;
        info.pid = pid;
        info.since = now_ms();
        Some(info.clone())
    }

    /// Drops the session (its process exited, or spawning failed).
    pub fn release(&self, key: &SessionKey) -> Option<SessionInfo> {
        self.write().remove(key)
    }

    /// Runs `f` only if the project has no live session (`session_running`, with
    /// `busy_message`, otherwise), holding the lock throughout, so no session can be reserved
    /// for the project while `f` runs.
    pub fn unless_project_active<T>(
        &self,
        project_id: &str,
        busy_message: &str,
        f: impl FnOnce() -> Result<T, AppError>,
    ) -> Result<T, AppError> {
        let live = self.write();
        if live.values().any(|info| info.project_id == project_id) {
            return Err(AppError::new(ErrorCode::SessionRunning, busy_message));
        }
        let result = f();
        drop(live);
        result
    }

    /// Whether any agent has a starting or running session for this project.
    pub fn is_project_active(&self, project_id: &str) -> bool {
        self.read()
            .values()
            .any(|info| info.project_id == project_id)
    }

    pub fn get(&self, key: &SessionKey) -> Option<SessionInfo> {
        self.read().get(key).cloned()
    }

    /// Every live session, ordered by project id, then agent id.
    pub fn list(&self) -> Vec<SessionInfo> {
        let mut all: Vec<SessionInfo> = self.read().values().cloned().collect();
        all.sort_by(|a, b| (&a.project_id, &a.agent_id).cmp(&(&b.project_id, &b.agent_id)));
        all
    }

    /// One project's live sessions, ordered by agent id.
    pub fn list_project(&self, project_id: &str) -> Vec<SessionInfo> {
        let mut all: Vec<SessionInfo> = self
            .read()
            .values()
            .filter(|info| info.project_id == project_id)
            .cloned()
            .collect();
        all.sort_by(|a, b| a.agent_id.cmp(&b.agent_id));
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(project_id: &str, agent_id: &str) -> SessionKey {
        SessionKey::new(project_id, agent_id)
    }

    #[test]
    fn is_project_active_follows_reserve_running_and_release() {
        let sessions = SessionRegistry::default();
        let builder = key("p1", "builder");
        assert!(!sessions.is_project_active("p1"));

        sessions.reserve(&builder, 3).unwrap();
        assert!(
            sessions.is_project_active("p1"),
            "true from the moment it's reserved"
        );
        assert!(!sessions.is_project_active("p2"));
        assert_eq!(
            sessions.get(&builder).unwrap().status,
            SessionStatus::Starting
        );

        let running = sessions.set_running(&builder, Some(42)).unwrap();
        assert_eq!(
            (running.status, running.pid),
            (SessionStatus::Running, Some(42))
        );
        assert!(sessions.is_project_active("p1"));

        sessions.reserve(&key("p1", "designer"), 3).unwrap();
        sessions.release(&builder);
        assert!(sessions.is_project_active("p1"), "designer still runs");
        sessions.release(&key("p1", "designer"));
        assert!(
            !sessions.is_project_active("p1"),
            "false once the last one is released"
        );
    }

    #[test]
    fn reserve_enforces_the_cap_and_one_session_per_agent() {
        let sessions = SessionRegistry::default();
        sessions.reserve(&key("p1", "a"), 2).unwrap();
        assert_eq!(
            sessions.reserve(&key("p1", "a"), 2).unwrap_err().code,
            ErrorCode::SessionRunning
        );
        sessions.reserve(&key("p2", "b"), 2).unwrap();
        assert_eq!(
            sessions.reserve(&key("p1", "c"), 2).unwrap_err().code,
            ErrorCode::TooManySessions
        );
        assert!(
            sessions.get(&key("p1", "c")).is_none(),
            "a refused reserve leaves nothing behind"
        );

        sessions.release(&key("p1", "a"));
        sessions.reserve(&key("p1", "c"), 2).unwrap();
        let ids: Vec<String> = sessions.list().into_iter().map(|s| s.agent_id).collect();
        assert_eq!(ids, ["c", "b"], "by project, then agent");
    }

    #[test]
    fn the_same_agent_id_in_two_projects_is_two_sessions() {
        let sessions = SessionRegistry::default();
        let (one, two) = (key("p1", "builder"), key("p2", "builder"));
        sessions.reserve(&one, 3).unwrap();
        sessions.reserve(&two, 3).unwrap();
        sessions.set_running(&two, Some(7)).unwrap();
        assert_eq!(sessions.get(&one).unwrap().status, SessionStatus::Starting);
        assert_eq!(sessions.get(&two).unwrap().pid, Some(7));
        assert_eq!(
            sessions.reserve(&two, 3).unwrap_err().code,
            ErrorCode::SessionRunning,
            "still one session per agent in a project"
        );
        assert_eq!(
            sessions.reserve(&key("p3", "builder"), 2).unwrap_err().code,
            ErrorCode::TooManySessions,
            "the cap counts every project"
        );
        let in_p2: Vec<String> = sessions
            .list_project("p2")
            .into_iter()
            .map(|s| s.project_id)
            .collect();
        assert_eq!(in_p2, ["p2"]);

        sessions.release(&one);
        assert!(sessions.get(&one).is_none());
        assert!(
            sessions.get(&two).is_some(),
            "releasing one leaves the other"
        );
        assert!(!sessions.is_project_active("p1"));
        assert!(sessions.is_project_active("p2"));
    }

    #[test]
    fn reserve_if_and_unless_project_active_are_checked_under_one_lock() {
        let sessions = SessionRegistry::default();
        let a = key("p1", "a");
        let refused = sessions
            .reserve_if(&a, 3, || Err(AppError::invalid_input("no")))
            .unwrap_err();
        assert_eq!(refused.code, ErrorCode::InvalidInput);
        assert!(
            sessions.get(&a).is_none(),
            "a refused check reserves nothing"
        );

        assert_eq!(
            sessions
                .unless_project_active("p1", "busy", || Ok(7))
                .unwrap(),
            7
        );
        sessions.reserve_if(&a, 3, || Ok(())).unwrap();
        let busy = sessions
            .unless_project_active("p1", "busy", || -> Result<(), AppError> {
                panic!("must not run while p1 is active")
            })
            .unwrap_err();
        assert_eq!(
            (busy.code, busy.message.as_str()),
            (ErrorCode::SessionRunning, "busy")
        );
        sessions
            .unless_project_active("p2", "busy", || Ok(()))
            .unwrap();
    }

    #[test]
    fn a_late_set_running_never_revives_a_released_session() {
        let sessions = SessionRegistry::default();
        let a = key("p1", "a");
        sessions.reserve(&a, 1).unwrap();
        sessions.release(&a);
        assert!(sessions.set_running(&a, Some(1)).is_none());
        assert!(!sessions.is_project_active("p1"));
    }
}
