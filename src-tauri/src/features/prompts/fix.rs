//! Fix prompts (`generate_fix_prompts`): the bugs of a slice's latest run, routed to the agents
//! who own the code and batched into one ready-to-send prompt per agent. Pure: the store
//! reads the reports and the team, this turns them into text. Nothing is written or sent.

use serde::{Deserialize, Serialize};
use specta::Type;

use super::disk::{Agent, Slice};
use super::render::role_line;
use crate::core::glob;
use crate::core::report::{self, Bug, BugSeverity};
use crate::core::text::normalize_ws;
use crate::core::{AppError, ErrorCode};

/// One agent's fix prompt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FixPrompt {
    pub agent_id: String,
    pub agent_name: String,
    /// The bugs in the prompt, in its numbered order.
    pub bug_ids: Vec<String>,
    /// Self-contained text, ready to send as the agent's next message.
    pub prompt: String,
}

/// The fixed rules every fix prompt ends with (the write-path rule follows them).
pub const RULE_PRODUCT_CODE: &str = "Fix the product code, not the tests.";
pub const RULE_ASSERTIONS: &str =
    "Do not weaken or delete any assertion, and don't skip or disable a test.";
pub const RULE_RUN: &str =
    "Run the failing tests above, then the full unit and e2e suites, and make them pass.";

/// A location's path: `\` as `/`, without a leading `./`, a `#L12` anchor or a `:12` /
/// `:12:5` line suffix.
fn location_path(location: &str) -> String {
    let mut path = location.trim().replace('\\', "/");
    if let Some(at) = path.find("#L") {
        path.truncate(at);
    }
    for _ in 0..2 {
        if let Some((head, tail)) = path.rsplit_once(':') {
            if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) {
                path = head.to_string();
            }
        }
    }
    path.trim_start_matches("./").to_string()
}

/// How specific a glob is: the length of its text before the first wildcard.
fn literal_len(glob: &str) -> usize {
    let glob = glob.trim();
    glob.find(['*', '?', '[']).unwrap_or(glob.len())
}

/// The agent whose write globs best match `location`: the matching glob with the longest
/// literal prefix wins; ties go to the agent listed first in the team.
fn owner_of(team: &[Agent], location: &str) -> Option<usize> {
    let path = location_path(location);
    if path.is_empty() {
        return None;
    }
    let mut best: Option<(usize, usize)> = None;
    for (index, agent) in team.iter().enumerate() {
        let score = agent
            .write_globs
            .iter()
            .filter(|g| glob::matches(g, &path))
            .map(|g| literal_len(g))
            .max();
        if let Some(score) = score {
            if best.is_none_or(|(_, top)| score > top) {
                best = Some((index, score));
            }
        }
    }
    best.map(|(index, _)| index)
}

/// One bug as one agent gets it: the locations that are theirs and the rest.
struct Entry<'a> {
    bug: &'a Bug,
    mine: Vec<&'a str>,
    /// Locations owned by another agent (with their name) or by no one.
    others: Vec<(&'a str, Option<&'a str>)>,
}

fn severity_label(severity: BugSeverity) -> &'static str {
    match severity {
        BugSeverity::High => "high",
        BugSeverity::Medium => "medium",
        BugSeverity::Low => "low",
    }
}

/// Routes every bug and builds one prompt per agent, in team order. A bug with locations
/// under two owners goes to both, each with their own locations. A bug no location places
/// goes to its `ownerHint` (when that's a team agent), else to the Product Lead; with neither,
/// `invalid_input` names the bug.
pub fn build(slice: &Slice, team: &[Agent], bugs: &[Bug]) -> Result<Vec<FixPrompt>, AppError> {
    let lead = team.iter().position(Agent::is_product_lead);
    let mut per_agent: Vec<Vec<Entry<'_>>> = team.iter().map(|_| Vec::new()).collect();
    for bug in bugs {
        let routed: Vec<(&str, Option<usize>)> = bug
            .suspected_locations
            .iter()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| (l, owner_of(team, l)))
            .collect();
        let mut owners: Vec<usize> = Vec::new();
        for owner in routed.iter().filter_map(|(_, o)| *o) {
            if !owners.contains(&owner) {
                owners.push(owner);
            }
        }
        if owners.is_empty() {
            let hinted = bug
                .owner_hint
                .as_deref()
                .and_then(|hint| team.iter().position(|a| a.id == hint.trim()));
            let owner = hinted.or(lead).ok_or_else(|| {
                AppError::new(
                    ErrorCode::InvalidInput,
                    format!(
                        "bug {} has no owner: no agent's write paths match its locations, its \
                         ownerHint isn't a team agent, and the team has no Product Lead",
                        bug.id
                    ),
                )
            })?;
            per_agent[owner].push(Entry {
                bug,
                mine: routed.iter().map(|(l, _)| *l).collect(),
                others: Vec::new(),
            });
            continue;
        }
        for &owner in &owners {
            let (mine, others): (Vec<_>, Vec<_>) =
                routed.iter().copied().partition(|(_, o)| *o == Some(owner));
            per_agent[owner].push(Entry {
                bug,
                mine: mine.into_iter().map(|(l, _)| l).collect(),
                others: others
                    .into_iter()
                    .map(|(l, o)| (l, o.map(|i| team[i].display_name.as_str())))
                    .collect(),
            });
        }
    }

    let mut out = Vec::new();
    for (agent, mut entries) in team.iter().zip(per_agent) {
        if entries.is_empty() {
            continue;
        }
        entries.sort_by_key(|e| e.bug.severity);
        out.push(FixPrompt {
            agent_id: agent.id.clone(),
            agent_name: agent.display_name.clone(),
            bug_ids: entries.iter().map(|e| e.bug.id.clone()).collect(),
            prompt: prompt(slice, agent, &entries),
        });
    }
    Ok(out)
}

fn code(text: &str) -> String {
    format!("`{}`", normalize_ws(text).replace('`', "'"))
}

fn prompt(slice: &Slice, agent: &Agent, entries: &[Entry<'_>]) -> String {
    let slice_name = normalize_ws(&slice.name);
    let mut out = role_line(agent);
    out.push_str("\n\n");
    out.push_str(&format!(
        "## Fix {} bug{} in slice: {slice_name}\n\n",
        entries.len(),
        if entries.len() == 1 { "" } else { "s" }
    ));
    out.push_str(&format!(
        "Testing found these bugs in slice {slice_name} (`{}`). Each one has a failing test \
         that stays red until the bug is fixed.\n\n",
        slice.id
    ));

    let mut requirement_ids: Vec<String> = Vec::new();
    for (n, entry) in entries.iter().enumerate() {
        let bug = entry.bug;
        out.push_str(&format!(
            "### {}. {} ({})\n\n",
            n + 1,
            normalize_ws(&bug.title),
            bug.id
        ));
        out.push_str(&format!("Severity: {}\n", severity_label(bug.severity)));
        let ids: Vec<String> = bug
            .requirement_ids
            .iter()
            .map(|r| normalize_ws(r))
            .filter(|r| !r.is_empty())
            .collect();
        for id in &ids {
            if !requirement_ids.contains(id) {
                requirement_ids.push(id.clone());
            }
        }
        out.push_str(&format!(
            "Requirements: {}\n\n",
            if ids.is_empty() {
                "none named".to_string()
            } else {
                ids.join(", ")
            }
        ));

        out.push_str("What is wrong (steps to reproduce):\n");
        let steps: Vec<String> = bug
            .steps
            .iter()
            .map(|s| normalize_ws(s))
            .filter(|s| !s.is_empty())
            .collect();
        if steps.is_empty() {
            out.push_str("1. (no steps given: start from the failing tests)\n");
        }
        for (i, step) in steps.iter().enumerate() {
            out.push_str(&format!("{}. {step}\n", i + 1));
        }

        out.push_str("\nWhere:\n");
        if entry.mine.is_empty() {
            out.push_str("- (no location given: start from the failing tests)\n");
        }
        for location in &entry.mine {
            out.push_str(&format!("- {}\n", code(location)));
        }
        if !entry.others.is_empty() {
            out.push_str("\nAlso suspected, outside your write paths (don't change these):\n");
            for (location, owner) in &entry.others {
                let whose = owner.map_or("no agent owns it".to_string(), |name| {
                    format!("{} fixes it", normalize_ws(name))
                });
                out.push_str(&format!("- {} ({whose})\n", code(location)));
            }
        }

        out.push_str("\nFixed when these tests pass:\n");
        let tests: Vec<String> = bug
            .failing_tests
            .iter()
            .filter(|t| !t.file.trim().is_empty() || !t.name.trim().is_empty())
            .map(|t| format!("- {} in {}", code(&t.name), code(&t.file)))
            .collect();
        if tests.is_empty() {
            out.push_str(
                "- (none named: confirm the fix with the steps above and say so in your \
                 report)\n",
            );
        }
        for test in tests {
            out.push_str(&test);
            out.push('\n');
        }
        out.push('\n');
    }

    out.push_str("## Rules\n\n");
    out.push_str(&format!("- {RULE_PRODUCT_CODE}\n"));
    out.push_str(&format!("- {RULE_ASSERTIONS}\n"));
    out.push_str("- Stay inside your own write paths:\n");
    if agent.write_globs.iter().all(|g| g.trim().is_empty()) {
        out.push_str("  - (none)\n");
    }
    for glob in agent.write_globs.iter().filter(|g| !g.trim().is_empty()) {
        out.push_str(&format!("  - {}\n", code(glob)));
    }
    out.push_str(
        "  If a fix needs a file outside them, don't change it: report it as a blocker, \
         naming the file.\n",
    );
    out.push_str(&format!("- {RULE_RUN}\n"));
    let report_path = report::report_path(&slice.id, &agent.id);
    let ids = if requirement_ids.is_empty() {
        "the requirements these bugs touch".to_string()
    } else {
        requirement_ids.join(", ")
    };
    out.push_str(&format!(
        "- Report the results with requirement ids: update `{report_path}`. Set {ids} to \
         their new status (`met`, `partly` or `not_met`) with the test that proves it as \
         evidence, keep your other entries, add each command you ran and its result to \
         `testsRun`, list anything outside your write paths in `blockers`, and name the bug \
         ids you fixed in `notes`. Then stop.\n"
    ));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locations_lose_their_line_numbers() {
        assert_eq!(location_path("src/a.ts:12"), "src/a.ts");
        assert_eq!(location_path("./src/a.ts:12:5"), "src/a.ts");
        assert_eq!(location_path("src\\a.ts#L3-L9"), "src/a.ts");
        assert_eq!(location_path(" src/a.ts "), "src/a.ts");
    }

    #[test]
    fn literal_prefix_measures_specificity() {
        assert_eq!(literal_len("src/feed/**"), 9);
        assert_eq!(literal_len("**/*.test.ts"), 0);
        assert_eq!(literal_len("README.md"), 9);
    }
}
