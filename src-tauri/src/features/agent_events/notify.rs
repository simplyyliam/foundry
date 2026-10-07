//! Desktop notifications for classified events: only when enabled, only at or above the
//! chosen severity, and at most one per agent (per project) within [`DEBOUNCE`], so a burst
//! shows once. The policy is read on every event, so a settings change applies at once.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::{AgentEvent, Severity};

/// A burst from one agent within this long shows one notification.
pub const DEBOUNCE: Duration = Duration::from_secs(5);

/// `osNotificationsEnabled` and `osNotificationSeverity`, as the store needs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotifyPolicy {
    pub enabled: bool,
    /// The lowest severity that notifies.
    pub min_severity: Severity,
}

/// Shows an OS notification (the notification plugin, in the app).
pub trait OsNotifier: Send + Sync {
    fn show(&self, event: &AgentEvent);
}

type PolicySource = Box<dyn Fn() -> NotifyPolicy + Send + Sync>;

pub(super) struct Notifications {
    policy: PolicySource,
    os: Arc<dyn OsNotifier>,
    /// `(projectId, agentId)` → epoch ms of the last notification shown.
    last: Mutex<HashMap<(String, String), u64>>,
}

impl Notifications {
    pub(super) fn new(
        policy: impl Fn() -> NotifyPolicy + Send + Sync + 'static,
        os: Arc<dyn OsNotifier>,
    ) -> Self {
        Self {
            policy: Box::new(policy),
            os,
            last: Mutex::new(HashMap::new()),
        }
    }

    /// Considers a batch of events from one hook call or timer tick: at most one
    /// notification per agent, for its most severe qualifying event.
    pub(super) fn offer(&self, events: &[AgentEvent], now_ms: u64) {
        let policy = (self.policy)();
        if !policy.enabled {
            return;
        }
        let mut best: HashMap<(String, String), &AgentEvent> = HashMap::new();
        for event in events.iter().filter(|e| e.severity >= policy.min_severity) {
            let key = (event.project_id.clone(), event.agent_id.clone());
            let slot = best.entry(key).or_insert(event);
            if event.severity > slot.severity {
                *slot = event;
            }
        }
        let window = DEBOUNCE.as_millis() as u64;
        for (key, event) in best {
            {
                let mut last = self.last.lock().unwrap_or_else(|p| p.into_inner());
                if last
                    .get(&key)
                    .is_some_and(|shown| now_ms.saturating_sub(*shown) < window)
                {
                    continue;
                }
                last.insert(key, now_ms);
            }
            tracing::debug!(agent_id = %event.agent_id, id = event.id, "os notification");
            self.os.show(event);
        }
    }
}
