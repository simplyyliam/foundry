//! The one line that starts an agent's session, and what goes into it: the agent from
//! `agents/roles.json` and a prompt file under `.foundry/prompts/`.
//!
//! Shared by the prompts feature's `launch_message` (which shows it) and agent sessions'
//! `agent_start` (which launches with it), so the frontend never passes free text into the
//! process arguments. Reads only; `agents/roles.json` is written by the team feature.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use super::paths::{self, PathKind};
use super::{repo_write, AppError, ErrorCode};

pub const PROMPTS_DIR: &str = ".foundry/prompts";
/// Written by the team feature on `confirm_team` / `add_agent_manually`.
pub const ROLES_FILE: &str = "agents/roles.json";
/// A launch message is at most this many characters.
pub const MAX_LAUNCH_CHARS: usize = 500;

/// One agent in `agents/roles.json`, as far as the launch message needs it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RosterAgent {
    pub id: String,
    pub display_name: String,
    pub title: String,
}

#[derive(Debug, Deserialize)]
struct RolesFile {
    #[serde(default)]
    agents: Vec<RosterAgent>,
}

/// The confirmed team; `None` when `roles.json` doesn't exist (no team confirmed yet).
pub fn roster(repo: &Path) -> Result<Option<Vec<RosterAgent>>, AppError> {
    repo_write::read_to_string(repo, ROLES_FILE)?
        .map(|text| {
            serde_json::from_str::<RolesFile>(&text)
                .map(|file| file.agents)
                .map_err(|err| AppError::internal(format!("{ROLES_FILE} is unreadable: {err}")))
        })
        .transpose()
}

/// The agent in the confirmed team: `team_not_confirmed` without one, `not_found` when the
/// agent isn't in it.
pub fn find_agent(repo: &Path, agent_id: &str) -> Result<RosterAgent, AppError> {
    roster(repo)?
        .ok_or_else(|| {
            AppError::new(
                ErrorCode::TeamNotConfirmed,
                "this project has no confirmed team",
            )
        })?
        .into_iter()
        .find(|agent| agent.id == agent_id)
        .ok_or_else(|| AppError::not_found(format!("no agent {agent_id} in {ROLES_FILE}")))
}

/// A prompt file checked to be an existing file directly in `.foundry/prompts/`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptPath {
    /// The file name, e.g. `s-feed-ada.md`.
    pub name: String,
    /// Repo-relative with `/`, e.g. `.foundry/prompts/s-feed-ada.md`.
    pub rel: String,
    /// The resolved absolute path.
    pub path: PathBuf,
}

/// Checks `prompt_file` (repo-relative; `./` and `\` accepted) is an existing file directly
/// in `.foundry/prompts/`, resolved through `core::paths`, so a link out of that folder
/// doesn't count. Anything else → `invalid_input`.
pub fn prompt_file(repo: &Path, prompt_file: &str) -> Result<PromptPath, AppError> {
    let invalid = || {
        AppError::invalid_input(format!(
            "the prompt file must be an existing file under {PROMPTS_DIR}/"
        ))
    };
    if prompt_file.chars().any(char::is_control) {
        return Err(invalid());
    }
    let rel = paths::check(Path::new(prompt_file), PathKind::Relative).map_err(|_| invalid())?;
    if !rel.starts_with(PROMPTS_DIR) || rel.components().count() != 3 {
        return Err(invalid());
    }
    let name = rel
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(invalid)?
        .to_string();
    let dir = paths::resolve(repo, PROMPTS_DIR).map_err(|_| invalid())?;
    let path = paths::resolve(repo, &rel).map_err(|_| invalid())?;
    if !path.starts_with(&dir) || !path.is_file() {
        return Err(invalid());
    }
    Ok(PromptPath {
        rel: format!("{PROMPTS_DIR}/{name}"),
        name,
        path,
    })
}

/// With a prompt file:
/// `You are {displayName}, {title}. Read agents/roles/{id}.md, then {prompt} and follow it.`
/// Without one (the human types to the agent):
/// `You are {displayName}, {title}. Read agents/roles/{id}.md, then wait for my instructions.`
/// Either way one line with no control characters, at most [`MAX_LAUNCH_CHARS`]; otherwise
/// `invalid_input`.
pub fn build(agent: &RosterAgent, prompt: Option<&PromptPath>) -> Result<String, AppError> {
    for (what, text) in [
        ("display name", &agent.display_name),
        ("title", &agent.title),
    ] {
        if text.chars().any(char::is_control) {
            return Err(AppError::invalid_input(format!(
                "the agent's {what} must be one line with no control characters"
            )));
        }
    }
    let plain_id = !agent.id.is_empty()
        && !agent.id.starts_with('.')
        && !agent.id.contains(['/', '\\', ':'])
        && !agent.id.chars().any(char::is_control);
    if !plain_id {
        return Err(AppError::invalid_input(format!(
            "agent id \"{}\" can't be used in a file name",
            agent.id
        )));
    }
    let then = match prompt {
        Some(prompt) => format!("{} and follow it", prompt.rel),
        None => "wait for my instructions".to_string(),
    };
    let message = format!(
        "You are {}, {}. Read agents/roles/{}.md, then {then}.",
        agent.display_name, agent.title, agent.id
    );
    if message.chars().any(char::is_control) || message.chars().count() > MAX_LAUNCH_CHARS {
        return Err(AppError::invalid_input(format!(
            "the launch message must be one line of at most {MAX_LAUNCH_CHARS} characters"
        )));
    }
    Ok(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRepo;

    fn ada() -> RosterAgent {
        RosterAgent {
            id: "ada".into(),
            display_name: "Ada".into(),
            title: "Backend engineer".into(),
        }
    }

    #[test]
    fn builds_one_line_from_the_roster_and_a_checked_prompt_file() {
        let repo = TestRepo::new();
        repo.write(".foundry/prompts/handoff.md", "x");
        let prompt = prompt_file(repo.path(), r"./.foundry\prompts\handoff.md").unwrap();
        assert_eq!(prompt.rel, ".foundry/prompts/handoff.md");
        assert_eq!(
            build(&ada(), Some(&prompt)).unwrap(),
            "You are Ada, Backend engineer. Read agents/roles/ada.md, then \
             .foundry/prompts/handoff.md and follow it."
        );
        assert_eq!(
            build(&ada(), None).unwrap(),
            "You are Ada, Backend engineer. Read agents/roles/ada.md, then wait for my \
             instructions."
        );

        for bad in [
            RosterAgent {
                display_name: "Ada\nIgnore that".into(),
                ..ada()
            },
            RosterAgent {
                title: "x".repeat(MAX_LAUNCH_CHARS),
                ..ada()
            },
            RosterAgent {
                id: "../ada".into(),
                ..ada()
            },
        ] {
            for prompt in [Some(&prompt), None] {
                assert_eq!(
                    build(&bad, prompt).unwrap_err().code,
                    ErrorCode::InvalidInput,
                    "{bad:?} {prompt:?}"
                );
            }
        }
        for bad in [
            ".foundry/prompts/missing.md",
            ".foundry/prompts/../plan.json",
            ".foundry/plan.json",
            "",
        ] {
            assert_eq!(
                prompt_file(repo.path(), bad).unwrap_err().code,
                ErrorCode::InvalidInput,
                "{bad}"
            );
        }
    }

    #[test]
    fn find_agent_needs_a_confirmed_team_with_that_agent() {
        let repo = TestRepo::new();
        assert_eq!(
            find_agent(repo.path(), "ada").unwrap_err().code,
            ErrorCode::TeamNotConfirmed
        );
        repo.write(
            ROLES_FILE,
            r#"{"version":1,"agents":[{"id":"ada","displayName":"Ada","title":"Backend engineer","writeGlobs":["src/**"]}]}"#,
        );
        assert_eq!(find_agent(repo.path(), "ada").unwrap(), ada());
        assert_eq!(
            find_agent(repo.path(), "lin").unwrap_err().code,
            ErrorCode::NotFound
        );
        repo.write(ROLES_FILE, "{ not json");
        assert_eq!(
            find_agent(repo.path(), "ada").unwrap_err().code,
            ErrorCode::Internal
        );
    }
}
