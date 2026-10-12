//! Pure text assembly: the same inputs always give the same bytes (LF line endings).

use std::collections::BTreeMap;

use super::disk::{Agent, Plan, Slice, BAKERY_MANIFEST, DESIGN_MANIFEST};
use crate::core::hook_policy;
use crate::core::report::{
    self, Bug, BugSeverity, Bugs, FailingTest, RequirementReport, RequirementStatus, SliceReport,
    TestRun,
};
use crate::core::text::normalize_ws;
use crate::core::AppError;

const HANDOFF_TEMPLATE: &str =
    include_str!("../../../resources/templates/handoff-prompt.template.md");
pub const MARKER: &str = "[[ FILL IN";
const MARKER_CLOSE: &str = "]]";
/// Starts the line a forced task prompt carries; `launch_message` looks for it.
pub const FORCED_PREFIX: &str = "Started with unfinished dependencies:";
/// Heads the hand-off's project rules, under the per-slice requirements.
pub const PROJECT_RULES_HEADING: &str =
    "PROJECT RULES (apply to everything built, in every slice):";
/// Follows a task prompt's requirement list.
pub const PROJECT_RULES_SENTENCE: &str = "Project rules (in the hand-off) apply to everything you \
     build; you are not asked to report on them.";
/// Closes the Boundaries section of a task prompt for a slice with `workspaceAccess`.
pub const WORKSPACE_SENTENCE: &str = "For this slice you may run package installs and edit the \
     root files listed above; edit them only as much as the slice needs.";

// ---------- hand-off ----------

/// The hand-off template's fields, by the label in front of each marker. A list field is a
/// `- [[ FILL IN … ]]` bullet, keyed by the heading line above it (without its colon).
pub mod field {
    pub const APPLICATION_PROFILE: &str = "APPLICATION PROFILE";
    pub const WORKSPACE: &str = "WORKSPACE ARCHITECTURE";
    pub const BACKEND: &str = "BACKEND TIERS";
    pub const FRONTEND: &str = "FRONTEND TIER";
    pub const DESIGN_FAMILY: &str = "DESIGN FAMILY";
    pub const PERSISTENCE: &str = "PERSISTENCE LAYER";
    pub const REQUIREMENTS: &str = "CORE MVP USER CONSTRAINTS & FEATURE PRIMITIVES";
    pub const HORIZON: &str = "FUTURE HORIZON ROADMAP PRIMITIVES";
}

/// The template without what's only there for filling it in by hand: the usage note (the
/// blockquote under the title, which itself mentions the marker) and the title's
/// " Template" suffix.
fn handoff_body() -> String {
    let template = HANDOFF_TEMPLATE.replace("\r\n", "\n");
    let mut lines = template.lines().peekable();
    let mut out: Vec<String> = Vec::new();
    if let Some(title) = lines.next() {
        out.push(title.trim_end().trim_end_matches(" Template").to_string());
    }
    while lines.peek().is_some_and(|l| l.trim().is_empty()) {
        lines.next();
    }
    while lines.peek().is_some_and(|l| l.starts_with('>')) {
        lines.next();
    }
    while lines.peek().is_some_and(|l| l.trim().is_empty()) {
        lines.next();
    }
    out.push(String::new());
    out.extend(lines.map(str::to_string));
    let mut text = out.join("\n");
    text.push('\n');
    text
}

/// Fills every marker: `scalars` by label (one line each), `lists` by heading (one output
/// line per entry, replacing the bullet). A marker no field fills is an error, and so is
/// any marker left afterwards (e.g. inside a value).
pub fn handoff(
    scalars: &BTreeMap<&str, String>,
    lists: &BTreeMap<&str, Vec<String>>,
) -> Result<String, AppError> {
    let body = handoff_body();
    let mut out = String::with_capacity(body.len() * 2);
    let mut heading = String::new();
    for line in body.lines() {
        let Some(at) = line.find(MARKER) else {
            let trimmed = line.trim();
            if let Some(h) = trimmed.strip_suffix(':') {
                heading = h.trim().to_string();
            }
            out.push_str(line);
            out.push('\n');
            continue;
        };
        let before = &line[..at];
        if before.trim() == "-" {
            let entries = lists.get(heading.as_str()).ok_or_else(|| {
                AppError::internal(format!(
                    "the hand-off template has a list Foundry doesn't fill: {heading}"
                ))
            })?;
            for entry in entries {
                out.push_str(entry);
                out.push('\n');
            }
            continue;
        }
        let label = before.trim().trim_end_matches(':').trim();
        let value = scalars.get(label).ok_or_else(|| {
            AppError::internal(format!(
                "the hand-off template has a field Foundry doesn't fill: {label}"
            ))
        })?;
        let end = line[at..]
            .find(MARKER_CLOSE)
            .map_or(line.len(), |i| at + i + MARKER_CLOSE.len());
        out.push_str(before);
        out.push_str(&normalize_ws(value));
        out.push_str(&line[end..]);
        out.push('\n');
    }
    if out.contains(MARKER) {
        return Err(AppError::invalid_input(
            "the hand-off would still contain [[ FILL IN; remove it from the plan or the \
             AGENTS.md values first",
        ));
    }
    Ok(out)
}

/// The requirements grouped by slice, in plan order, text verbatim. Only confirmed ones
/// (in a confirmed plan that's all of them).
pub fn handoff_requirements(plan: &Plan) -> Vec<String> {
    let mut out = Vec::new();
    for slice in &plan.slices {
        out.push(format!("- {}", normalize_ws(&slice.name)));
        let mut any = false;
        for requirement in confirmed_requirements(plan, &slice.id) {
            out.push(format!("  - {}: {}", requirement.id, requirement.text));
            any = true;
        }
        if !any {
            out.push("  - (no requirements assigned)".into());
        }
    }
    out
}

/// The confirmed project-wide requirements, each with its "Verified by" line, after a blank
/// line and `PROJECT_RULES_HEADING`. Nothing at all when there are none, so the hand-off is
/// unchanged for a plan without them.
pub fn handoff_project_rules(plan: &Plan) -> Vec<String> {
    let rules: Vec<&super::disk::Requirement> = plan
        .requirements
        .iter()
        .filter(|r| r.project_wide && r.state == super::disk::RequirementState::Confirmed)
        .collect();
    if rules.is_empty() {
        return Vec::new();
    }
    let mut out = vec![String::new(), PROJECT_RULES_HEADING.to_string()];
    for requirement in rules {
        out.push(format!("- {}: {}", requirement.id, requirement.text));
        out.push(format!(
            "  Verified by: {}",
            normalize_ws(&requirement.verified_by)
        ));
    }
    out
}

/// The deferred items, each with the slice it's anchored to.
pub fn handoff_horizon(plan: &Plan) -> Vec<String> {
    if plan.deferred.is_empty() {
        return vec!["- (none)".into()];
    }
    plan.deferred
        .iter()
        .map(|d| {
            let anchor = plan
                .slice(&d.anchored_to)
                .map_or(d.anchored_to.as_str(), |s| s.name.as_str());
            format!(
                "- {} (anchored to: {})",
                normalize_ws(&d.label),
                normalize_ws(anchor)
            )
        })
        .collect()
}

fn confirmed_requirements<'a>(
    plan: &'a Plan,
    slice_id: &'a str,
) -> impl Iterator<Item = &'a super::disk::Requirement> {
    plan.requirements.iter().filter(move |r| {
        r.slice_id.as_deref() == Some(slice_id)
            && r.state == super::disk::RequirementState::Confirmed
    })
}

// ---------- task prompt ----------

/// The section a QA or Test Engineer's task prompt carries before "Done means".
pub const QA_SECTION: &str = "## Testing rules\n\n\
- Write tests and report findings. Do not change product code: change only test files \
inside your boundaries.\n\
- Keep every assertion strict, so a failing test stays red until the bug is fixed. Don't \
skip a failing test, loosen it or mark it as expected to fail.\n\
- Commit nothing.\n\
- List every bug you find in the report's `bugs` list, in the shape below: its steps to \
reproduce, the failing test and its file, and the code location you suspect.\n";

/// Follows the report sample in a QA or Test Engineer's "Done means".
pub const QA_BUGS_SENTENCE: &str = "List every bug in `bugs`, one entry each: a unique `id`, \
     `severity` (`high`, `medium` or `low`), a one-line `title`, the `requirementIds` it \
     breaks, the `steps` to reproduce it, its `failingTests` (each with its `file` and test \
     `name`), and its `suspectedLocations` (repo-relative paths, with a line number when you \
     know it, e.g. `src/feed/keys.ts:12`). Add `ownerHint` (an agent id from \
     `agents/roles.json`) only when the locations don't make it clear who should fix it. \
     With no bugs, leave `bugs` out.";

/// A pretty-printed report without its trailing `"bugs": []`: only QA roles list bugs, so the
/// others are shown the shape without it (it's optional when read).
pub fn without_empty_bugs(json: &str) -> String {
    match json.strip_suffix(",\n  \"bugs\": []\n}") {
        Some(head) => format!("{head}\n}}"),
        None => json.to_string(),
    }
}

fn sample_bug() -> Bug {
    Bug {
        id: "b-1".into(),
        severity: BugSeverity::High,
        title: "<what is wrong, in one line>".into(),
        requirement_ids: vec!["<requirement id>".into()],
        steps: vec!["<step 1>".into(), "<step 2>".into()],
        failing_tests: vec![FailingTest {
            file: "<test file>".into(),
            name: "<test name>".into(),
        }],
        suspected_locations: vec!["<path>:<line>".into()],
        owner_hint: Some("<agent id, optional>".into()),
    }
}

pub struct TaskInputs<'a> {
    pub plan: &'a Plan,
    pub slice: &'a Slice,
    pub agent: &'a Agent,
    /// Names of the dependencies that weren't done, when the human forced it.
    pub forced: &'a [String],
    /// At least one of the Bakery and design manifests exists.
    pub design_pointers: bool,
}

/// The line that starts a session, and the first line of every task prompt.
pub fn role_line(agent: &Agent) -> String {
    format!(
        "You are {}, {}. Read agents/roles/{}.md first.",
        agent.display_name, agent.title, agent.id
    )
}

fn bullets(out: &mut String, items: impl IntoIterator<Item = String>, empty: &str) {
    let mut any = false;
    for item in items {
        out.push_str(&format!("- {item}\n"));
        any = true;
    }
    if !any {
        out.push_str(&format!("- {empty}\n"));
    }
}

pub fn task_prompt(input: &TaskInputs<'_>) -> Result<String, AppError> {
    let TaskInputs {
        plan,
        slice,
        agent,
        forced,
        design_pointers,
    } = input;
    let mut out = role_line(agent);
    out.push_str("\n\n");

    // 2. The slice.
    out.push_str(&format!("## Slice: {}\n\n", normalize_ws(&slice.name)));
    out.push_str("It delivers:\n");
    bullets(
        &mut out,
        slice.delivers.iter().map(|d| normalize_ws(d)),
        "(not described)",
    );
    out.push('\n');

    // 3. Dependencies.
    out.push_str("## Depends on\n\n");
    bullets(
        &mut out,
        slice.depends_on.iter().map(|id| match plan.slice(id) {
            Some(dep) => format!("{}: {}", normalize_ws(&dep.name), dep.status.label()),
            None => format!("{id}: unknown slice"),
        }),
        "nothing",
    );
    if !forced.is_empty() {
        out.push_str(&format!(
            "\n{FORCED_PREFIX} the human started this slice before these were done: {}. \
             Don't rely on what they deliver; if you need it, report it as a blocker.\n",
            forced.join(", ")
        ));
    }
    out.push('\n');

    // 4. Requirements, verbatim.
    out.push_str("## Requirements\n\n");
    out.push_str("Each one is checked off individually. Meet every one.\n\n");
    let mut any = false;
    for requirement in confirmed_requirements(plan, &slice.id) {
        out.push_str(&format!("- {}: {}\n", requirement.id, requirement.text));
        out.push_str(&format!(
            "  Verified by: {}\n",
            normalize_ws(&requirement.verified_by)
        ));
        any = true;
    }
    if !any {
        out.push_str("- (none assigned)\n");
    }
    out.push('\n');
    out.push_str(PROJECT_RULES_SENTENCE);
    out.push_str("\n\n");

    // 5. Decisions and open questions.
    out.push_str("## Decisions and open questions\n\n");
    out.push_str("Decisions so far:\n");
    bullets(
        &mut out,
        slice.decisions.iter().map(|d| normalize_ws(d)),
        "(none)",
    );
    out.push_str("\nOpen questions:\n");
    bullets(
        &mut out,
        slice.open_questions.iter().map(|q| normalize_ws(&q.text)),
        "(none)",
    );
    out.push('\n');

    // 6. Boundaries.
    let report_path = report::report_path(&slice.id, &agent.id);
    out.push_str("## Boundaries\n\n");
    out.push_str("You may write only files matching:\n");
    bullets(
        &mut out,
        agent.write_globs.iter().map(|g| format!("`{}`", g.trim())),
        "(nothing beyond your report)",
    );
    out.push_str(&format!("You may also always write `{report_path}`.\n"));
    if slice.workspace_access {
        out.push_str("You may also write these root files:\n");
        bullets(
            &mut out,
            hook_policy::WORKSPACE_FILES
                .iter()
                .map(|f| format!("`{f}`")),
            "",
        );
        out.push_str(WORKSPACE_SENTENCE);
        out.push('\n');
    }
    out.push('\n');

    // 7. Design pointers.
    if *design_pointers {
        out.push_str("## Design pointers\n\n");
        out.push_str(&format!(
            "For UI work, read `{BAKERY_MANIFEST}` and `{DESIGN_MANIFEST}` (where present) \
             first, as AGENTS.md section 6 describes.\n\n"
        ));
    }

    // 7b. QA and Test Engineer roles.
    if agent.is_qa() {
        out.push_str(QA_SECTION);
        out.push('\n');
    }

    // 8. Done means, and the report.
    out.push_str("## Done means\n\n");
    out.push_str(
        "Every requirement above is met and proven the way its \"Verified by\" line says, the \
         tests you ran pass, and every file you changed is inside your boundaries.\n\n",
    );
    out.push_str(&format!(
        "Then write `{report_path}` in exactly this shape, with one entry per requirement \
         above (`status` is `met`, `partly` or `not_met`), and stop:\n\n"
    ));
    let sample = SliceReport {
        slice_id: slice.id.clone(),
        agent: agent.id.clone(),
        requirements: vec![RequirementReport {
            id: "<requirement id>".into(),
            status: RequirementStatus::Met,
            evidence: "<the test or check that proves it>".into(),
        }],
        tests_run: vec![TestRun {
            command: "<command>".into(),
            result: "<result>".into(),
        }],
        blockers: vec![],
        notes: String::new(),
        bugs: if agent.is_qa() {
            Bugs(vec![sample_bug()])
        } else {
            Bugs::default()
        },
    };
    let json = serde_json::to_string_pretty(&sample)
        .map_err(|err| AppError::internal(format!("serialize the sample report: {err}")))?;
    let json = without_empty_bugs(&json);
    out.push_str("```json\n");
    out.push_str(&json);
    out.push_str("\n```\n");
    if agent.is_qa() {
        out.push('\n');
        out.push_str(QA_BUGS_SENTENCE);
        out.push('\n');
    }
    Ok(out)
}
