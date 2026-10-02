//! The environment for anything that runs Claude Code on the user's subscription.

use std::ffi::{OsStr, OsString};

/// Variables that would move Claude Code off the subscription: API keys and base URLs
/// (`ANTHROPIC_*`) and third-party providers (`CLAUDE_CODE_USE_BEDROCK`, `…_VERTEX`, …).
/// Case-insensitive, as Windows treats variable names.
pub fn is_provider_var(key: &OsStr) -> bool {
    key.to_str().is_some_and(|key| {
        let key = key.to_ascii_uppercase();
        key.starts_with("ANTHROPIC_") || key.starts_with("CLAUDE_CODE_USE_")
    })
}

/// Foundry's own environment without the provider variables. Spawn with `env_clear()` and
/// then exactly these.
pub fn subscription_env() -> Vec<(OsString, OsString)> {
    std::env::vars_os()
        .filter(|(key, _)| !is_provider_var(key))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_variables_are_recognised_case_insensitively() {
        for key in [
            "ANTHROPIC_API_KEY",
            "anthropic_base_url",
            "ANTHROPIC_",
            "CLAUDE_CODE_USE_BEDROCK",
            "Claude_Code_Use_Vertex",
        ] {
            assert!(is_provider_var(OsStr::new(key)), "{key}");
        }
        for key in [
            "PATH",
            "CLAUDE_CONFIG_DIR",
            "MY_ANTHROPIC_KEY",
            "CLAUDE_CODE_SIMPLE",
        ] {
            assert!(!is_provider_var(OsStr::new(key)), "{key}");
        }
    }

    #[test]
    fn subscription_env_drops_only_provider_variables() {
        std::env::set_var("ANTHROPIC_FOUNDRY_ENV_TEST", "x");
        std::env::set_var("FOUNDRY_ENV_TEST_KEEP", "y");
        let env = subscription_env();
        assert!(env.iter().all(|(key, _)| !is_provider_var(key)));
        assert!(env
            .iter()
            .any(|(key, value)| key == "FOUNDRY_ENV_TEST_KEEP" && value == "y"));
    }
}
