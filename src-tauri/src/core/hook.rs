//! Where the local hook server listens, for this run of the app.
//!
//! The agent-events feature publishes its port and per-run token here once the server is
//! bound; any feature (agent sessions, for `FOUNDRY_HOOK_PORT` and `FOUNDRY_HOOK_TOKEN`) reads
//! them from here, never from that feature. Pure state: no sockets here.

use std::sync::RwLock;

/// The hook server's address on 127.0.0.1 and the token every request must carry.
#[derive(Clone, PartialEq, Eq)]
pub struct HookEndpoint {
    pub port: u16,
    /// Sent by the helper in the [`TOKEN_HEADER`] header. Never log it.
    pub token: String,
}

/// The header that carries [`HookEndpoint::token`].
pub const TOKEN_HEADER: &str = "x-foundry-token";

impl std::fmt::Debug for HookEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookEndpoint")
            .field("port", &self.port)
            .field("token", &"<redacted>")
            .finish()
    }
}

/// The published endpoint, empty until the server is up. Share it as `Arc<HookEndpointCell>`.
#[derive(Debug, Default)]
pub struct HookEndpointCell {
    inner: RwLock<Option<HookEndpoint>>,
}

impl HookEndpointCell {
    pub fn publish(&self, endpoint: HookEndpoint) {
        match self.inner.write() {
            Ok(mut inner) => *inner = Some(endpoint),
            Err(poisoned) => *poisoned.into_inner() = Some(endpoint),
        }
    }

    /// Withdraws the endpoint (the server stopped).
    pub fn clear(&self) {
        match self.inner.write() {
            Ok(mut inner) => *inner = None,
            Err(poisoned) => *poisoned.into_inner() = None,
        }
    }

    /// The endpoint, or `None` while no server is running.
    pub fn get(&self) -> Option<HookEndpoint> {
        match self.inner.read() {
            Ok(inner) => inner.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publishes_and_clears_and_never_debug_prints_the_token() {
        let cell = HookEndpointCell::default();
        assert_eq!(cell.get(), None);
        cell.publish(HookEndpoint {
            port: 4567,
            token: "secret-token".into(),
        });
        let got = cell.get().unwrap();
        assert_eq!((got.port, got.token.as_str()), (4567, "secret-token"));
        assert!(!format!("{got:?}").contains("secret-token"));
        cell.clear();
        assert_eq!(cell.get(), None);
    }
}
