//! Typed events. Every event payload carries `{ id, ts, ... }` via a flattened [`EventMeta`].

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{Emitter, Manager, Runtime};

use super::{ids, AppError};

/// Identity and timestamp shared by every event. Flatten it into the payload struct.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct EventMeta {
    /// Unique per emitted event.
    pub id: String,
    /// Milliseconds since the Unix epoch.
    // Exported as TS `number`: specta refuses u64 (bigint) and maps f64 to `number | null`, so
    // u32 stands in for the type only. Epoch millis stay exact in a JS number until year 287396.
    #[specta(type = u32)]
    pub ts: u64,
}

impl EventMeta {
    pub fn new() -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        Self {
            id: ids::new_id(),
            ts,
        }
    }
}

impl Default for EventMeta {
    fn default() -> Self {
        Self::new()
    }
}

/// Emits a typed event to every target and logs it (name only, never the payload).
/// The event must be registered with `collect_events!` and mounted at setup.
pub fn emit<R, H, E>(handle: &H, event: &E) -> Result<(), AppError>
where
    R: Runtime,
    H: Emitter<R> + Manager<R>,
    E: tauri_specta::Event + Serialize + Clone,
{
    tracing::debug!(event = E::NAME, "emit");
    event.emit(handle).map_err(|err| {
        tracing::warn!(event = E::NAME, error = %err, "emit failed");
        AppError::internal(format!("failed to emit {}: {err}", E::NAME))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meta_has_fresh_id_and_current_ts() {
        let a = EventMeta::new();
        let b = EventMeta::new();
        assert_ne!(a.id, b.id);
        // 2020-01-01 in ms: a sanity floor that catches seconds-vs-millis mistakes.
        assert!(a.ts > 1_577_836_800_000);
    }

    #[test]
    fn meta_flattens_into_payload() {
        #[derive(Serialize)]
        struct Payload {
            #[serde(flatten)]
            meta: EventMeta,
            n: u8,
        }
        let value = serde_json::to_value(Payload {
            meta: EventMeta {
                id: "x".into(),
                ts: 5,
            },
            n: 1,
        })
        .unwrap();
        assert_eq!(value, serde_json::json!({ "id": "x", "ts": 5, "n": 1 }));
    }
}
