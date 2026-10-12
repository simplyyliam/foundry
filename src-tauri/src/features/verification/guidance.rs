//! What to do after a verification, in plain words (`verification_guidance`). Read-only: it
//! suggests actions, the UI performs them.
//!
//! [`build`] is pure: the store gathers [`Inputs`] (the saved review, the skip, who's on the
//! slice, who wrote which file) and this turns them into a headline, actions and one entry
//! per finding. Each finding code has one rule in [`RULES`]; requirement outcomes and the
//! headline have their own rule functions below.
//!
//! Not built: "a requirement needs a layer outside the slice's owners" (headline
//! needs_decision, defer to another slice). It needs plan data that doesn't exist yet (slices
//! have no paths or layers), and report text isn't matched instead.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use specta::Type;

use super::SkipReason;
use super::{code, CommandRun, Finding, FindingSeverity, RequirementOutcome, Review, VerifySkip};
use crate::core::glob;
use crate::core::report::RequirementStatus;

/// How many output lines of a failed test command are shown and sent.
pub const OUTPUT_LINES: usize = 40;
/// An out-of-scope file with at most this many changed lines counts as a small edit.
pub const SMALL_DIFF_LINES: u32 = 20;
/// Files to revert rather than override: the team's rules, the design, Claude settings and
/// the plan.
const REVERT_PATHS: &[&str] = &[
    "AGENTS.md",
    "agents/**",
    "docs/design/**",
    ".claude/**",
    ".foundry/plan.json",
];
const FIX_LINE: &str = "Fix the code, don't weaken tests, run the command, report the result.";

// ---------- the contract ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum HeadlineKind {
    Passed,
    NeedsFix,
    SafeToOverride,
    NeedsDecision,
    Stale,
    Waiting,
    Skipped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Headline {
    pub kind: HeadlineKind,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum GuidanceActionKind {
    MarkDone,
    Commit,
    SendToAgent,
    Override,
    Reverify,
    CommitSetup,
    StopDevServer,
    Manual,
}

/// One thing the user can do. Foundry never does it on its own.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GuidanceAction {
    /// Unique within one guidance, e.g. `reverify` or `send:f2`.
    pub id: String,
    pub label: String,
    pub kind: GuidanceActionKind,
    /// `send_to_agent`: who to send `message` to; `null` when the slice has several agents
    /// and the user picks.
    pub agent_id: Option<String>,
    /// `send_to_agent`: the message to send. `manual` and `stop_dev_server`: what to do.
    pub message: Option<String>,
    /// `override`: a reason to start from; `<reason>` is for the user to fill in.
    pub reason_template: Option<String>,
}

/// How a finding weighs on the slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum FindingGroup {
    /// Must be fixed (headline `needs_fix`).
    Fix,
    /// The user must decide (headline `needs_decision`).
    Decide,
    /// Fine to mark done with an override reason (headline `safe_to_override`).
    Override,
    /// Worth doing, doesn't block.
    FollowUp,
    /// For information.
    Info,
    /// Nothing to do.
    Ignore,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GuidedFinding {
    /// `f<index>` for the review's findings (index into `findings`), `r:<requirementId>` for a
    /// requirement reported not met or partly.
    pub finding_id: String,
    pub plain: String,
    pub what_to_do: String,
    pub group: FindingGroup,
    /// A failed test command's last 40 output lines.
    pub output: Option<String>,
    /// The ids of the actions in `actions` that belong to this finding.
    pub action_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VerificationGuidance {
    pub headline: Headline,
    pub actions: Vec<GuidanceAction>,
    pub findings: Vec<GuidedFinding>,
}

// ---------- inputs ----------

/// Who touched a file a boundary finding is about.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PathFacts {
    /// This slice's agent that wrote it (writes ledger).
    pub writer: Option<String>,
    /// Another slice or agent it belongs to, e.g. `bo on slice s-web`: written by another
    /// slice's session, or inside the folders of an agent that is only on other slices.
    pub elsewhere: Option<String>,
    /// Lines added plus removed since the slice started; `None` when unknown (binary).
    pub changed_lines: Option<u32>,
}

/// Everything [`build`] reads.
#[derive(Debug, Clone, Default)]
pub struct Inputs {
    pub slice_id: String,
    pub review: Option<Review>,
    pub stale: bool,
    pub skip: Option<VerifySkip>,
    /// The slice's agents (task prompts and writes-ledger sessions), by id.
    pub on_slice: Vec<String>,
    /// Agents with a report for the slice.
    pub reported: BTreeSet<String>,
    /// Agent id → display name.
    pub names: BTreeMap<String, String>,
    /// Requirement id → its text.
    pub requirement_text: BTreeMap<String, String>,
    /// Path → facts, for boundary findings.
    pub paths: BTreeMap<String, PathFacts>,
}

impl Inputs {
    fn name(&self, agent: &str) -> String {
        self.names
            .get(agent)
            .cloned()
            .unwrap_or_else(|| agent.to_string())
    }

    /// The agent to send a slice-wide message to: the only one on the slice (or the only
    /// reporter), else `None` (the user picks).
    fn slice_agent(&self) -> Option<String> {
        match (&self.on_slice[..], self.reported.len()) {
            ([one], _) => Some(one.clone()),
            ([], 1) => self.reported.iter().next().cloned(),
            _ => None,
        }
    }
}

// ---------- per-finding rules ----------

/// What one rule makes of one finding.
struct Guided {
    plain: String,
    what_to_do: String,
    group: FindingGroup,
    output: Option<String>,
    actions: Vec<GuidanceAction>,
    /// Joined into the `override` action's template when the headline is `safe_to_override`.
    reason_template: Option<String>,
}

impl Guided {
    fn new(group: FindingGroup, plain: impl Into<String>, what_to_do: impl Into<String>) -> Self {
        Self {
            plain: plain.into(),
            what_to_do: what_to_do.into(),
            group,
            output: None,
            actions: Vec::new(),
            reason_template: None,
        }
    }

    fn action(mut self, action: GuidanceAction) -> Self {
        self.actions.push(action);
        self
    }
}

/// One finding, with what the rule needs around it.
struct Ctx<'a> {
    inputs: &'a Inputs,
    finding: &'a Finding,
    id: &'a str,
    /// Every finding of the review.
    all: &'a [Finding],
    runs: &'a [CommandRun],
}

impl Ctx<'_> {
    fn send(&self, agent: Option<String>, label: &str, message: String) -> GuidanceAction {
        GuidanceAction {
            id: format!("send:{}", self.id),
            label: match &agent {
                Some(a) => format!("{label} ({})", self.inputs.name(a)),
                None => label.to_string(),
            },
            kind: GuidanceActionKind::SendToAgent,
            agent_id: agent,
            message: Some(message),
            reason_template: None,
        }
    }

    fn manual(&self, label: &str, message: impl Into<String>) -> GuidanceAction {
        GuidanceAction {
            id: format!("manual:{}", self.id),
            label: label.to_string(),
            kind: GuidanceActionKind::Manual,
            agent_id: None,
            message: Some(message.into()),
            reason_template: None,
        }
    }

    fn path(&self) -> &str {
        self.finding.path.as_deref().unwrap_or("a file")
    }

    fn facts(&self) -> PathFacts {
        self.finding
            .path
            .as_ref()
            .and_then(|p| self.inputs.paths.get(p))
            .cloned()
            .unwrap_or_default()
    }

    /// The finding's agent, else the slice's.
    fn agent(&self) -> Option<String> {
        self.finding
            .agent
            .clone()
            .or_else(|| self.inputs.slice_agent())
    }
}

type Rule = fn(&Ctx<'_>) -> Guided;

/// Whether `code` has its own rule (the rest fall back to their severity).
#[cfg(test)]
pub fn has_rule(code: &str) -> bool {
    RULES.iter().any(|(c, _)| *c == code)
}

/// One rule per finding code. A test fails when a code in `code` has no rule here.
const RULES: &[(&str, Rule)] = &[
    (code::UNREPORTED_REQUIREMENT, unreported_requirement),
    (code::UNKNOWN_REQUIREMENT, unknown_requirement),
    (code::SLICE_MISMATCH, slice_mismatch),
    (code::MISSING_EVIDENCE, missing_evidence),
    (code::NO_TEST_COMMAND, no_test_command),
    (code::TEST_FAILED, test_failed),
    (code::TEST_TIMED_OUT, test_failed),
    (code::PROTECTED_FILE_CHANGED, out_of_bounds),
    (code::OUTSIDE_WRITE_GLOBS, out_of_bounds),
    (code::FOUNDRY_FILE_CHANGED, foundry_file_changed),
    (code::BOUNDARY_SKIPPED, boundary_skipped),
    (code::CONTRADICTION, contradiction),
    (code::PARTLY_REPORTED, partly_reported),
    (code::REPORTED_BLOCKER, reported_blocker),
    (code::OWNED_BY_OTHER_SLICE, owned_by_other_slice),
    (code::SHARED_FILE_TOUCHED, shared_file_touched),
    (code::AMBIGUOUS_SHELL_CHANGE, ambiguous_shell_change),
    (code::INVALID_BUG, invalid_bug),
    (code::CHECK_FAILED, check_failed),
];

fn unreported_requirement(c: &Ctx<'_>) -> Guided {
    let id = c.finding.requirement_id.as_deref().unwrap_or("?");
    let text = requirement_line(c.inputs, id);
    Guided::new(
        FindingGroup::Fix,
        format!("No report says anything about requirement {id}{text}."),
        "Ask the agent to add it to its report.",
    )
    .action(c.send(
        c.agent(),
        "Ask for the missing requirement",
        format!(
            "Your report doesn't list requirement {id}{text}. Add it with its status (met, \
             partly or not_met) and the test and command that cover it."
        ),
    ))
}

fn check_failed(c: &Ctx<'_>) -> Guided {
    let id = c.finding.requirement_id.as_deref().unwrap_or("?");
    let text = requirement_line(c.inputs, id);
    let what = c
        .finding
        .message
        .split_once(": ")
        .map_or(c.finding.message.as_str(), |(_, rest)| rest);
    Guided::new(
        FindingGroup::Fix,
        format!("Requirement {id}{text} has a check that failed: {what}."),
        "Ask the agent to fix the code so the check passes, then verify again.",
    )
    .action(c.send(
        c.agent(),
        "Ask to fix it",
        format!(
            "Requirement {id}{text} has a check Foundry runs itself, and it failed: {what}.              Change the code so the check passes (don't change the check), then update your              report."
        ),
    ))
}

fn unknown_requirement(c: &Ctx<'_>) -> Guided {
    let id = c.finding.requirement_id.as_deref().unwrap_or("?");
    Guided::new(
        FindingGroup::FollowUp,
        format!("A report lists requirement {id}, which isn't part of this slice."),
        "Doesn't block. Ask the agent to remove it from the report.",
    )
    .action(c.send(
        c.agent(),
        "Ask to fix the report",
        format!(
            "Your report lists requirement {id}, which isn't part of slice {}. Remove it from \
             your report. Don't change any code.",
            c.inputs.slice_id
        ),
    ))
}

fn slice_mismatch(c: &Ctx<'_>) -> Guided {
    Guided::new(
        FindingGroup::FollowUp,
        "A report names a different slice than its file name.",
        "Doesn't block. Ask the agent to fix the sliceId in its report.",
    )
    .action(c.send(
        c.agent(),
        "Ask to fix the report",
        format!(
            "Your report file is for slice {}, but its sliceId says otherwise. Set \"sliceId\" \
             to \"{}\". Don't change any code.",
            c.inputs.slice_id, c.inputs.slice_id
        ),
    ))
}

fn missing_evidence(c: &Ctx<'_>) -> Guided {
    let id = c.finding.requirement_id.as_deref().unwrap_or("?");
    Guided::new(
        FindingGroup::FollowUp,
        format!(
            "Requirement {id} is reported met, but the report doesn't say which test proves it."
        ),
        "Doesn't block. Ask the agent to name the test and command.",
    )
    .action(c.send(
        c.agent(),
        "Ask for evidence",
        format!(
            "Update your report for requirement {id} to name the test and command that cover \
             it. Don't change any code."
        ),
    ))
}

fn no_test_command(c: &Ctx<'_>) -> Guided {
    Guided::new(
        FindingGroup::FollowUp,
        "The project has no test commands, so no tests ran.",
        "Add the project's test command in the verification settings, then verify again.",
    )
    .action(c.manual(
        "Add a test command",
        "Add the project's test command (for example `pnpm test`), confirm it, then verify \
         again.",
    ))
}

/// The run a test finding is about: the one its message quotes, else the first that failed.
fn failed_run<'a>(c: &Ctx<'a>) -> Option<&'a CommandRun> {
    let quoted = c.finding.message.split('`').nth(1);
    c.runs
        .iter()
        .find(|r| Some(r.command.as_str()) == quoted)
        .or_else(|| {
            c.runs
                .iter()
                .find(|r| r.timed_out || r.exit_code != Some(0))
        })
}

/// The last [`OUTPUT_LINES`] lines.
pub fn last_lines(text: &str) -> String {
    let lines: Vec<&str> = text.trim_end().lines().collect();
    lines[lines.len().saturating_sub(OUTPUT_LINES)..].join("\n")
}

/// A known cause in the environment, not the code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvCause {
    /// A running dev server or Prisma Studio holds a native file (EPERM/EBUSY on Prisma's
    /// query engine, a `.dll` or a `.node`).
    LockedFile,
    /// EADDRINUSE.
    PortInUse,
}

pub fn env_cause(output: &str) -> Option<EnvCause> {
    let locked = output.lines().any(|line| {
        let lower = line.to_lowercase();
        (line.contains("EPERM") || line.contains("EBUSY"))
            && (lower.contains("query_engine") || lower.contains(".dll") || lower.contains(".node"))
    });
    if locked {
        Some(EnvCause::LockedFile)
    } else if output.contains("EADDRINUSE") {
        Some(EnvCause::PortInUse)
    } else {
        None
    }
}

fn test_failed(c: &Ctx<'_>) -> Guided {
    let run = failed_run(c);
    let command = run.map_or("the test command", |r| r.command.as_str());
    let output = run.map(|r| last_lines(&r.tail));
    let how = match run {
        Some(r) if r.timed_out => "didn't finish in time".to_string(),
        Some(CommandRun {
            exit_code: Some(code),
            ..
        }) => format!("failed (exit code {code})"),
        _ => "failed".to_string(),
    };
    let plain = format!("`{command}` {how}.");
    let mut guided = match output.as_deref().and_then(env_cause) {
        Some(EnvCause::LockedFile) => Guided::new(
            FindingGroup::Fix,
            plain,
            "Stop the dev server and Prisma Studio, then re-verify.",
        )
        .action(GuidanceAction {
            id: format!("stop_dev_server:{}", c.id),
            label: "Stop the dev server and Prisma Studio".into(),
            kind: GuidanceActionKind::StopDevServer,
            agent_id: None,
            message: Some(
                "Stop the dev server and Prisma Studio: they hold a file the tests need to \
                 replace. Then verify again."
                    .into(),
            ),
            reason_template: None,
        }),
        Some(EnvCause::PortInUse) => Guided::new(
            FindingGroup::Fix,
            plain,
            "A port is in use: stop whatever is using it, or change the port, then re-verify.",
        )
        .action(c.manual(
            "Free the port",
            "A port is in use: stop whatever is using it, or change the port. Then verify again.",
        )),
        None => {
            let tail = output.as_deref().unwrap_or("");
            Guided::new(
                FindingGroup::Fix,
                plain,
                "Send the failure to the agent to fix.",
            )
            .action(c.send(
                c.inputs.slice_agent(),
                "Send the failure to the agent",
                format!(
                    "`{command}` {how}. The last lines of its output:\n```\n{tail}\n```\n{FIX_LINE}"
                ),
            ))
        }
    };
    guided.output = output;
    guided
}

fn reverts(path: &str) -> bool {
    REVERT_PATHS.iter().any(|g| glob::matches(g, path))
}

/// `protected_file_changed` and `outside_write_globs`.
fn out_of_bounds(c: &Ctx<'_>) -> Guided {
    let path = c.path();
    let facts = c.facts();
    let writer = facts.writer.clone().or_else(|| c.inputs.slice_agent());
    let revert_message = format!(
        "You changed {path}, which isn't yours to change. Don't edit it again. If your task \
         needs it changed, stop and tell me why."
    );
    let revert = |why: &str| {
        Guided::new(
            FindingGroup::Fix,
            format!("{path} changed, and {why}."),
            format!(
                "Revert it (`git checkout -- {path}`, or delete it if it's new) and tell the \
                 agent."
            ),
        )
        .action(c.send(writer.clone(), "Tell the agent", revert_message.clone()))
    };
    if reverts(path) {
        return revert("it holds the project's rules, design or plan");
    }
    if let Some(owner) = &facts.elsewhere {
        let mut guided = Guided::new(
            FindingGroup::Override,
            format!("{path} belongs to {owner}, not this slice."),
            "Safe to override: it's another slice's change.",
        );
        guided.reason_template = Some(format!(
            "{path} was changed by {owner}, not this slice; <reason>"
        ));
        return guided;
    }
    let protected = c.finding.code == code::PROTECTED_FILE_CHANGED;
    if protected {
        return revert("it's protected for this slice's agent");
    }
    let out_of_scope = c
        .all
        .iter()
        .filter(|f| f.code == code::OUTSIDE_WRITE_GLOBS)
        .count();
    let small = facts.changed_lines.is_some_and(|n| n <= SMALL_DIFF_LINES);
    if out_of_scope == 1 && small {
        let mut guided = Guided::new(
            FindingGroup::Override,
            format!("{path} is outside this slice's files, but it's one small edit."),
            "Looks deliberate: if this slice needed it, override with a reason.",
        );
        guided.reason_template = Some(format!("Edit to {path} needed for this slice; <reason>"));
        return guided;
    }
    Guided::new(
        FindingGroup::Decide,
        format!("{path} changed outside this slice's files."),
        "Open it and decide: keep it (override with a reason) or revert it and tell the agent.",
    )
    .action(c.manual(
        &format!("Check {path}"),
        format!("Open {path}. Keep it with an override reason, or revert it and tell the agent."),
    ))
}

fn foundry_file_changed(_: &Ctx<'_>) -> Guided {
    Guided::new(
        FindingGroup::Ignore,
        "Foundry rewrites this file itself.",
        "Nothing to do.",
    )
}

fn boundary_skipped(c: &Ctx<'_>) -> Guided {
    Guided::new(
        FindingGroup::FollowUp,
        "The file check couldn't run, so changed files weren't checked.",
        "Fix the cause, then verify again.",
    )
    .action(c.manual("Check why", c.finding.message.clone()))
}

fn contradiction(_: &Ctx<'_>) -> Guided {
    Guided::new(
        FindingGroup::Fix,
        "The reports say everything is met, but a test failed.",
        "Fix the failing test first; this goes away once it passes.",
    )
}

fn partly_reported(_: &Ctx<'_>) -> Guided {
    Guided::new(
        FindingGroup::Info,
        "One agent says a requirement is only partly done; another says it's met. It counts as met.",
        "Nothing to do unless you doubt it.",
    )
}

fn reported_blocker(c: &Ctx<'_>) -> Guided {
    let blocker = c
        .finding
        .message
        .split_once(": ")
        .map_or(c.finding.message.as_str(), |(_, b)| b);
    Guided::new(
        FindingGroup::Decide,
        format!("An agent is blocked: {blocker}"),
        "Answer it, then ask the agent to continue.",
    )
    .action(c.send(
        c.agent(),
        "Answer the blocker",
        format!("About your blocker \"{blocker}\": <your answer>. Then continue."),
    ))
}

fn invalid_bug(c: &Ctx<'_>) -> Guided {
    let problem = c
        .finding
        .message
        .split_once(": ")
        .map_or(c.finding.message.as_str(), |(_, p)| p);
    Guided::new(
        FindingGroup::FollowUp,
        format!("A report's bugs list needs fixing: {problem}."),
        "Doesn't block. Ask the agent to fix the entry so a fix prompt can be made from it.",
    )
    .action(c.send(
        c.agent(),
        "Ask to fix the bugs list",
        format!(
            "In your report's bugs list, {problem}. Fix that entry: give it an id, severity \
             (high, medium or low), title, requirement ids, steps to reproduce, the failing \
             test and its file, and the code location. Don't change any code."
        ),
    ))
}

fn owned_by_other_slice(_: &Ctx<'_>) -> Guided {
    Guided::new(
        FindingGroup::Info,
        "Another slice changed this file, so it isn't held against this one.",
        "Nothing to do.",
    )
}

fn shared_file_touched(c: &Ctx<'_>) -> Guided {
    let path = c.path();
    Guided::new(
        FindingGroup::Decide,
        format!("Both slices changed {path}: open it and check the merged result."),
        "Open it and check that both changes are there and work together.",
    )
    .action(c.manual(
        &format!("Check {path}"),
        format!("Both slices changed {path}: open it and check the merged result."),
    ))
}

fn ambiguous_shell_change(_: &Ctx<'_>) -> Guided {
    Guided::new(
        FindingGroup::Info,
        "A shell command changed this file while another slice was working, so it can't be told \
         whose change it is.",
        "Nothing to do unless it looks wrong.",
    )
}

/// A code without a rule: by severity.
fn fallback(c: &Ctx<'_>) -> Guided {
    let group = match c.finding.severity {
        FindingSeverity::Danger => FindingGroup::Fix,
        FindingSeverity::Warn => FindingGroup::FollowUp,
        FindingSeverity::Info => FindingGroup::Info,
    };
    Guided::new(group, c.finding.message.clone(), "Read the finding.")
}

// ---------- requirement rule ----------

fn requirement_line(inputs: &Inputs, id: &str) -> String {
    inputs
        .requirement_text
        .get(id)
        .filter(|t| !t.trim().is_empty())
        .map(|t| format!(" (\"{}\")", t.trim()))
        .unwrap_or_default()
}

/// A requirement whose combined status is not met or partly. (Deferring it to another slice
/// is not suggested: see the module docs.)
fn requirement_not_met(inputs: &Inputs, review: &Review, o: &RequirementOutcome) -> Guided {
    let id = &o.id;
    let text = requirement_line(inputs, id);
    let word = |s: RequirementStatus| match s {
        RequirementStatus::Met => "met",
        RequirementStatus::Partly => "partly",
        RequirementStatus::NotMet => "not met",
    };
    let status = o.status.map_or("not met", word);
    let mut guided = Guided::new(
        FindingGroup::Fix,
        format!("Requirement {id}{text} is {status}."),
        "Send it back to the agent to finish.",
    );
    let fid = format!("r:{id}");
    for reported in o
        .reported_by
        .iter()
        .filter(|r| r.status != RequirementStatus::Met)
    {
        let evidence = report_evidence(review, &reported.agent, id);
        let said = match evidence {
            Some(e) => format!("Your report says {}: {e}", word(reported.status)),
            None => format!("Your report says {}.", word(reported.status)),
        };
        guided.actions.push(GuidanceAction {
            id: format!("send:{fid}:{}", reported.agent),
            label: format!("Send back to {}", inputs.name(&reported.agent)),
            kind: GuidanceActionKind::SendToAgent,
            agent_id: Some(reported.agent.clone()),
            message: Some(format!(
                "Requirement {id}{text} isn't met yet. {said}\nFinish it, run the tests, and \
                 update your report."
            )),
            reason_template: None,
        });
    }
    guided
}

/// What `agent`'s verified report says as evidence for `id`.
fn report_evidence(review: &Review, agent: &str, id: &str) -> Option<String> {
    review
        .reports
        .iter()
        .filter(|r| r.agent == agent)
        .filter_map(|r| r.report.get("requirements")?.as_array().cloned())
        .flatten()
        .find(|r| r.get("id").and_then(|v| v.as_str()) == Some(id))
        .and_then(|r| {
            r.get("evidence")?
                .as_str()
                .map(str::trim)
                .map(str::to_string)
        })
        .filter(|e| !e.is_empty())
}

// ---------- headline rules ----------

fn action(id: &str, label: &str, kind: GuidanceActionKind) -> GuidanceAction {
    GuidanceAction {
        id: id.into(),
        label: label.into(),
        kind,
        agent_id: None,
        message: None,
        reason_template: None,
    }
}

fn reverify() -> GuidanceAction {
    action("reverify", "Verify again", GuidanceActionKind::Reverify)
}

fn names(inputs: &Inputs, agents: &[String]) -> String {
    let names: Vec<String> = agents.iter().map(|a| inputs.name(a)).collect();
    match names.as_slice() {
        [] => String::new(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// Agents on the slice without a report yet.
fn waiting_rule(inputs: &Inputs) -> Option<(Headline, Vec<GuidanceAction>)> {
    let missing: Vec<String> = inputs
        .on_slice
        .iter()
        .filter(|a| !inputs.reported.contains(*a))
        .cloned()
        .collect();
    (!missing.is_empty()).then(|| {
        (
            Headline {
                kind: HeadlineKind::Waiting,
                text: format!(
                    "Waiting for {}. Earlier results aren't final.",
                    names(inputs, &missing)
                ),
            },
            Vec::new(),
        )
    })
}

/// The recorded refusal, in its own plain words, with its fix.
fn skipped_rule(inputs: &Inputs) -> Option<(Headline, Vec<GuidanceAction>)> {
    let skip = inputs.skip.as_ref()?;
    let manual = |id: &str, label: &str, message: &str| GuidanceAction {
        message: Some(message.into()),
        ..action(id, label, GuidanceActionKind::Manual)
    };
    let actions = match skip.reason {
        SkipReason::TestCommandsUnconfirmed => vec![
            manual(
                "confirm_test_commands",
                "Review and confirm the test commands",
                "Open the test commands, check each one, and confirm them.",
            ),
            reverify(),
        ],
        SkipReason::NoReport => {
            let agents = if inputs.on_slice.is_empty() {
                vec![None]
            } else {
                inputs.on_slice.iter().cloned().map(Some).collect()
            };
            agents
                .into_iter()
                .map(|agent| GuidanceAction {
                    id: format!("send:report:{}", agent.as_deref().unwrap_or("any")),
                    label: match &agent {
                        Some(a) => format!("Ask {} for a report", inputs.name(a)),
                        None => "Ask the agent for a report".into(),
                    },
                    kind: GuidanceActionKind::SendToAgent,
                    message: Some(format!(
                        "Write your report for slice {} to .foundry/reports/{}.<your agent id>.json, \
                         listing every requirement with its status and evidence.",
                        inputs.slice_id, inputs.slice_id
                    )),
                    agent_id: agent,
                    reason_template: None,
                })
                .collect()
        }
        SkipReason::ReportIncomplete => vec![GuidanceAction {
            message: Some(format!(
                "Your report isn't valid: {}. Rewrite it in the report format. Don't change any \
                 code.",
                skip.message
            )),
            agent_id: inputs.slice_agent(),
            ..action(
                "send:report",
                "Ask the agent to fix its report",
                GuidanceActionKind::SendToAgent,
            )
        }],
        SkipReason::NoStartCommit => vec![manual(
            "restart_slice",
            "Restart the slice",
            "Move the slice back to queued, then start it again.",
        )],
        SkipReason::SetupNotCommitted => vec![
            action(
                "commit_setup",
                "Commit the setup",
                GuidanceActionKind::CommitSetup,
            ),
            manual(
                "restart_slice",
                "Restart the slice",
                "Then move the slice back to queued and start it again.",
            ),
        ],
        SkipReason::AnotherVerificationRunning => vec![
            manual(
                "wait",
                "Wait for it",
                "Another verification is running in this project. Wait for it to finish.",
            ),
            reverify(),
        ],
        SkipReason::Other => vec![manual("check", "Check the cause", &skip.message)],
    };
    Some((
        Headline {
            kind: HeadlineKind::Skipped,
            text: skip.message.clone(),
        },
        actions,
    ))
}

fn stale_rule(inputs: &Inputs) -> Option<(Headline, Vec<GuidanceAction>)> {
    let text = match &inputs.review {
        None => "Not verified yet.",
        Some(_) if inputs.stale => "A report changed after this verification. Verify again.",
        Some(_) => return None,
    };
    Some((
        Headline {
            kind: HeadlineKind::Stale,
            text: text.into(),
        },
        vec![reverify()],
    ))
}

fn passed_actions() -> Vec<GuidanceAction> {
    vec![
        action("mark_done", "Mark done", GuidanceActionKind::MarkDone),
        action("commit", "Commit the slice", GuidanceActionKind::Commit),
    ]
}

/// From the findings' groups: needs_fix over needs_decision over safe_to_override over
/// passed. A failed review with nothing but notes left is passed with a note.
fn outcome_rule(review: &Review, guided: &[(String, Guided)]) -> (Headline, Vec<GuidanceAction>) {
    let has = |group| guided.iter().any(|(_, g)| g.group == group);
    if has(FindingGroup::Fix) {
        let fixes = guided
            .iter()
            .filter(|(_, g)| g.group == FindingGroup::Fix)
            .count();
        return (
            Headline {
                kind: HeadlineKind::NeedsFix,
                text: format!(
                    "Needs a fix: {fixes} {} below.",
                    if fixes == 1 { "problem" } else { "problems" }
                ),
            },
            vec![reverify()],
        );
    }
    if has(FindingGroup::Decide) {
        return (
            Headline {
                kind: HeadlineKind::NeedsDecision,
                text: "Needs your decision: see below.".into(),
            },
            vec![reverify()],
        );
    }
    if has(FindingGroup::Override) {
        let template: Vec<&str> = guided
            .iter()
            .filter_map(|(_, g)| g.reason_template.as_deref())
            .collect();
        return (
            Headline {
                kind: HeadlineKind::SafeToOverride,
                text: "Safe to override: what's left isn't this slice's problem.".into(),
            },
            vec![
                GuidanceAction {
                    reason_template: Some(template.join(" ")),
                    ..action(
                        "override",
                        "Mark done with a reason",
                        GuidanceActionKind::Override,
                    )
                },
                action("commit", "Commit the slice", GuidanceActionKind::Commit),
            ],
        );
    }
    let text = if review.passed {
        "Passed".to_string()
    } else {
        "Passed (the saved result says failed, but only notes are left)".to_string()
    };
    (
        Headline {
            kind: HeadlineKind::Passed,
            text,
        },
        passed_actions(),
    )
}

// ---------- build ----------

/// The guidance for one slice. Never performs anything.
pub fn build(inputs: &Inputs) -> VerificationGuidance {
    let mut guided: Vec<(String, Guided)> = Vec::new();
    if let Some(review) = &inputs.review {
        for (index, finding) in review.findings.iter().enumerate() {
            let id = format!("f{index}");
            let rule = RULES
                .iter()
                .find(|(code, _)| *code == finding.code)
                .map_or(fallback as Rule, |(_, rule)| *rule);
            let g = rule(&Ctx {
                inputs,
                finding,
                id: &id,
                all: &review.findings,
                runs: &review.test_runs,
            });
            guided.push((id, g));
        }
        for o in review.requirements.iter().filter(|o| {
            matches!(
                o.status,
                Some(RequirementStatus::NotMet | RequirementStatus::Partly)
            )
        }) {
            guided.push((
                format!("r:{}", o.id),
                requirement_not_met(inputs, review, o),
            ));
        }
    }

    let (headline, lead) = waiting_rule(inputs)
        .or_else(|| skipped_rule(inputs))
        .or_else(|| stale_rule(inputs))
        .unwrap_or_else(|| match &inputs.review {
            Some(review) => outcome_rule(review, &guided),
            // `stale_rule` answers when there's no review.
            None => (
                Headline {
                    kind: HeadlineKind::Stale,
                    text: "Not verified yet.".into(),
                },
                vec![reverify()],
            ),
        });

    // While waiting, skipped or stale, the findings are only shown: their actions would act
    // on results that aren't final.
    let final_result = matches!(
        headline.kind,
        HeadlineKind::Passed
            | HeadlineKind::NeedsFix
            | HeadlineKind::NeedsDecision
            | HeadlineKind::SafeToOverride
    );
    let mut actions: Vec<GuidanceAction> = Vec::new();
    let push = |action: GuidanceAction, actions: &mut Vec<GuidanceAction>| {
        if !actions.iter().any(|a| a.id == action.id) {
            actions.push(action);
        }
    };
    let ends_with_reverify = lead.iter().any(|a| a.kind == GuidanceActionKind::Reverify)
        && matches!(
            headline.kind,
            HeadlineKind::NeedsFix | HeadlineKind::NeedsDecision
        );
    for a in lead
        .iter()
        .filter(|a| !(ends_with_reverify && a.id == "reverify"))
    {
        push(a.clone(), &mut actions);
    }
    let mut findings = Vec::new();
    for (id, g) in guided {
        let mut action_ids = Vec::new();
        if final_result {
            for a in g.actions {
                action_ids.push(a.id.clone());
                push(a, &mut actions);
            }
            if g.reason_template.is_some() && headline.kind == HeadlineKind::SafeToOverride {
                action_ids.push("override".into());
            }
        }
        findings.push(GuidedFinding {
            finding_id: id,
            plain: g.plain,
            what_to_do: g.what_to_do,
            group: g.group,
            output: g.output,
            action_ids,
        });
    }
    if ends_with_reverify {
        push(reverify(), &mut actions);
    }
    VerificationGuidance {
        headline,
        actions,
        findings,
    }
}
