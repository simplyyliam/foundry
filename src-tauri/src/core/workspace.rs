//! A slice's `workspaceAccess`, read from `.foundry/plan.json` on disk (the plan feature owns
//! that file; the launch only reads this one flag). The files it opens up are
//! `hook_policy::WORKSPACE_FILES`.

use std::path::Path;

use serde::Deserialize;

use super::repo_write;

const PLAN_FILE: &str = ".foundry/plan.json";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Plan {
    #[serde(default)]
    slices: Vec<Slice>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Slice {
    id: String,
    #[serde(default)]
    workspace_access: bool,
}

/// Whether the plan gives `slice_id` workspace access right now. No plan, an unreadable one,
/// no such slice, or no flag: `false` (fail closed).
pub fn slice_has_access(repo: &Path, slice_id: &str) -> bool {
    let text = match repo_write::read_to_string(repo, PLAN_FILE) {
        Ok(Some(text)) => text,
        Ok(None) => return false,
        Err(err) => {
            tracing::warn!(error = %err, "plan unreadable; no workspace access");
            return false;
        }
    };
    serde_json::from_str::<Plan>(text.trim_start_matches('\u{feff}'))
        .map(|plan| {
            plan.slices
                .iter()
                .any(|s| s.id == slice_id && s.workspace_access)
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRepo;

    #[test]
    fn only_a_slice_with_the_flag_on_has_access() {
        let repo = TestRepo::new();
        assert!(!slice_has_access(repo.path(), "s-1"), "no plan");
        repo.write(PLAN_FILE, "{ nope");
        assert!(!slice_has_access(repo.path(), "s-1"), "unreadable plan");
        repo.write(
            PLAN_FILE,
            r#"{ "slices": [
                { "id": "s-1", "workspaceAccess": true },
                { "id": "s-2", "workspaceAccess": false },
                { "id": "s-3" }
            ] }"#,
        );
        assert!(slice_has_access(repo.path(), "s-1"));
        for off in ["s-2", "s-3", "s-9"] {
            assert!(!slice_has_access(repo.path(), off), "{off}");
        }
    }
}
