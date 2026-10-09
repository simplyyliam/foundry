//! Foundry's setup files in a linked repo, and which of them aren't committed. A slice's start
//! commit must hold them, or verification would count them as the agent's changes. Starting a
//! slice (plan) and verifying one both ask.

use std::path::Path;

use super::git::{Git, GitRun};

/// Committed before a slice starts; any change to them blocks the start.
pub const SETUP_PATHS: &[&str] = &["AGENTS.md", "agents", "docs", "vsm_viewer.html"];
/// Setup files Foundry rewrites on every plan change (`plan.json` and its mirror): they must
/// be committed once, but later edits don't count.
pub const REWRITTEN_SETUP: &[&str] = &[".foundry/plan.json", "vsm_data.js"];

/// Setup files that are untracked or differ from HEAD. The rewritten ones only count while
/// untracked. Not a git repo, or git failing: none.
pub fn uncommitted(git: &Git, repo: &Path) -> Vec<String> {
    let mut args = vec![
        "--no-optional-locks",
        "status",
        "--porcelain=v1",
        "-z",
        "--no-renames",
        "--untracked-files=all",
        "--",
    ];
    args.extend(SETUP_PATHS.iter().chain(REWRITTEN_SETUP));
    let output = match git.run(repo, &args) {
        Ok(GitRun::Ok(output)) => output,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&output.stdout)
        .split('\0')
        .filter_map(|entry| Some((entry.get(..2)?, entry.get(3..)?)))
        .filter(|(state, path)| *state == "??" || !REWRITTEN_SETUP.contains(path))
        .map(|(_, path)| path.to_string())
        .collect()
}

/// "a, b and c", or the first few and "N more".
pub fn list_some(items: &[String]) -> String {
    const SHOWN: usize = 5;
    match items {
        [] => String::new(),
        [only] => only.clone(),
        _ if items.len() > SHOWN => format!(
            "{} and {} more",
            items[..SHOWN].join(", "),
            items.len() - SHOWN
        ),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}
