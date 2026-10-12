//! The slice report an agent writes when it finishes a task:
//! `.foundry/reports/<sliceId>.<agentId>.json`, one file per agent, so two agents on one slice
//! never overwrite each other. Projects made before that wrote `.foundry/reports/<sliceId>.json`,
//! which is still read (its agent comes from its `agent` field) but never asked for.
//! Task prompts (P6) tell agents this shape; reading reports back (P9) parses it. Both use
//! this one type, so the format an agent is asked for and the format Foundry reads can't drift.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

use super::{paths, repo_write, time, AppError};

pub const REPORTS_DIR: &str = ".foundry/reports";

/// How the decision recorded when a slice is marked done without a passed verification
/// begins. The full text is `<prefix>; it <why>): <reason>`. Plan writes it; the slice commit
/// message reads the reason back.
pub const DONE_OVERRIDE_PREFIX: &str = "Marked done without a passed verification (override";

/// The reason of a [`DONE_OVERRIDE_PREFIX`] decision; `None` for any other decision.
pub fn done_override_reason(decision: &str) -> Option<&str> {
    let rest = decision.strip_prefix(DONE_OVERRIDE_PREFIX)?;
    Some(rest.split_once("): ")?.1.trim())
}

/// Where an agent writes its report for a slice, repo-relative.
pub fn report_path(slice_id: &str, agent_id: &str) -> String {
    format!("{REPORTS_DIR}/{slice_id}.{agent_id}.json")
}

/// The one-report-per-slice name from before per-agent reports. Read, never written.
pub fn legacy_report_path(slice_id: &str) -> String {
    format!("{REPORTS_DIR}/{slice_id}.json")
}

fn is_id(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A file name in the reports folder → `(sliceId, agentId)`: `<slice>.<agent>.json` gives
/// the agent, the legacy `<slice>.json` gives `None`. Anything else isn't a report.
pub fn parse_report_name(name: &str) -> Option<(&str, Option<&str>)> {
    let stem = name.strip_suffix(".json")?;
    match stem.split_once('.') {
        None => is_id(stem).then_some((stem, None)),
        Some((slice, agent)) => (is_id(slice) && is_id(agent)).then_some((slice, Some(agent))),
    }
}

/// The slice's report files, repo-relative: the legacy one first, then one per agent, by
/// agent id. Empty when there are none (or no reports folder).
pub fn slice_report_paths(repo: &Path, slice_id: &str) -> Result<Vec<String>, AppError> {
    let dir = paths::resolve(repo, REPORTS_DIR)?;
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let mut legacy = false;
    let mut agents = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        if !entry.file_type().is_ok_and(|t| t.is_file()) {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        match parse_report_name(&name) {
            Some((slice, None)) if slice == slice_id => legacy = true,
            Some((slice, Some(agent))) if slice == slice_id => agents.push(agent.to_string()),
            _ => {}
        }
    }
    agents.sort();
    Ok(legacy
        .then(|| legacy_report_path(slice_id))
        .into_iter()
        .chain(agents.iter().map(|agent| report_path(slice_id, agent)))
        .collect())
}

/// When the report file at `rel` was last written, as ISO 8601 UTC (its modified time:
/// agents write reports, so the file is the only record). `None` if it's gone.
pub fn written_at(repo: &Path, rel: &str) -> Result<Option<String>, AppError> {
    let target = paths::resolve(repo, rel)?;
    match fs::metadata(target) {
        Ok(meta) => Ok(Some(time::iso8601(time::epoch_ms(meta.modified()?)))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// Report path → its JSON. What "the report changed since it was verified" compares.
pub type ReportSnapshot = BTreeMap<String, Value>;

/// Every report file of the slice as it is on disk now; one that doesn't parse is `null`.
pub fn snapshot(repo: &Path, slice_id: &str) -> Result<ReportSnapshot, AppError> {
    let mut out = ReportSnapshot::new();
    for rel in slice_report_paths(repo, slice_id)? {
        if let Some(text) = repo_write::read_to_string(repo, &rel)? {
            let value =
                serde_json::from_str(text.trim_start_matches('\u{feff}')).unwrap_or(Value::Null);
            out.insert(rel, value);
        }
    }
    Ok(out)
}

/// What a stored verification read: its `reports` (path → report), or, for one saved before
/// per-agent reports (no `reports`), its single `report` at the legacy path.
pub fn verified_snapshot(
    slice_id: &str,
    reports: impl IntoIterator<Item = (String, Value)>,
    report: Value,
) -> ReportSnapshot {
    let reports: ReportSnapshot = reports.into_iter().collect();
    if reports.is_empty() {
        ReportSnapshot::from([(legacy_report_path(slice_id), report)])
    } else {
        reports
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RequirementStatus {
    Met,
    Partly,
    NotMet,
}

/// One requirement from the task prompt, as the agent reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RequirementReport {
    /// The requirement id from the task prompt.
    pub id: String,
    pub status: RequirementStatus,
    /// What proves it: a test name, a command and its output, a file.
    pub evidence: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TestRun {
    pub command: String,
    /// What happened, e.g. "12 passed" or the failure.
    pub result: String,
}

/// `.foundry/reports/<sliceId>.<agentId>.json` (or the legacy `<sliceId>.json`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SliceReport {
    pub slice_id: String,
    /// The agent id that did the work.
    pub agent: String,
    pub requirements: Vec<RequirementReport>,
    pub tests_run: Vec<TestRun>,
    pub blockers: Vec<String>,
    pub notes: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRepo;
    use serde_json::json;

    fn sample() -> serde_json::Value {
        json!({
            "sliceId": "s-1",
            "agent": "ada",
            "requirements": [
                { "id": "r-1", "status": "met", "evidence": "test login_works" },
                { "id": "r-2", "status": "partly", "evidence": "only the happy path" },
                { "id": "r-3", "status": "not_met", "evidence": "" }
            ],
            "testsRun": [{ "command": "pnpm test", "result": "14 passed" }],
            "blockers": ["needs an API key"],
            "notes": "done otherwise"
        })
    }

    #[test]
    fn sample_report_round_trips() {
        let report: SliceReport = serde_json::from_value(sample()).unwrap();
        assert_eq!(report.requirements[2].status, RequirementStatus::NotMet);
        assert_eq!(report.tests_run[0].result, "14 passed");
        assert_eq!(serde_json::to_value(&report).unwrap(), sample());
    }

    #[test]
    fn unknown_status_is_rejected() {
        let mut value = sample();
        value["requirements"][0]["status"] = json!("done");
        assert!(serde_json::from_value::<SliceReport>(value).is_err());
    }

    #[test]
    fn a_done_override_decision_gives_its_reason() {
        let stale = format!(
            "{DONE_OVERRIDE_PREFIX}; it has a stale verification (its report changed after it \
             was verified)): shipped: by hand"
        );
        assert_eq!(done_override_reason(&stale), Some("shipped: by hand"));
        assert_eq!(
            done_override_reason("Started before a was done (override): x"),
            None
        );
    }

    #[test]
    fn report_path_is_under_reports_and_names_the_agent() {
        assert_eq!(report_path("s-1", "qa"), ".foundry/reports/s-1.qa.json");
        assert_eq!(legacy_report_path("s-1"), ".foundry/reports/s-1.json");
    }

    #[test]
    fn the_hook_allows_exactly_the_path_prompts_ask_for() {
        assert_eq!(
            crate::core::hook_policy::own_report("s-1", "qa"),
            report_path("s-1", "qa")
        );
    }

    #[test]
    fn report_names_parse_to_slice_and_agent() {
        assert_eq!(parse_report_name("s-1.json"), Some(("s-1", None)));
        assert_eq!(parse_report_name("s-1.qa.json"), Some(("s-1", Some("qa"))));
        for no in [
            "s-1.qa.x.json",
            ".json",
            "s-1..json",
            ".qa.json",
            "s-1.md",
            "a b.json",
        ] {
            assert_eq!(parse_report_name(no), None, "{no}");
        }
    }

    #[test]
    fn a_slices_report_files_are_listed_legacy_first_then_by_agent() {
        let repo = TestRepo::new();
        for name in ["s-1.zed", "s-1.ada", "s-1", "s-10.ada", "s-2", "s-1.x.y"] {
            repo.write(&format!(".foundry/reports/{name}.json"), "{}");
        }
        repo.write(".foundry/reports/s-1.notes.md", "x");
        assert_eq!(
            slice_report_paths(repo.path(), "s-1").unwrap(),
            [
                ".foundry/reports/s-1.json",
                ".foundry/reports/s-1.ada.json",
                ".foundry/reports/s-1.zed.json"
            ]
        );
        assert!(slice_report_paths(repo.path(), "s-3").unwrap().is_empty());
        let empty = TestRepo::new();
        assert!(slice_report_paths(empty.path(), "s-1").unwrap().is_empty());
    }

    #[test]
    fn a_review_without_reports_reads_as_the_legacy_file() {
        let legacy = verified_snapshot("s-1", [], json!({ "a": 1 }));
        assert_eq!(
            legacy,
            ReportSnapshot::from([(".foundry/reports/s-1.json".into(), json!({ "a": 1 }))])
        );
        let both = verified_snapshot(
            "s-1",
            [(report_path("s-1", "qa"), json!(2))],
            json!({ "a": 1 }),
        );
        assert_eq!(
            both.keys().collect::<Vec<_>>(),
            [".foundry/reports/s-1.qa.json"]
        );
    }
}
