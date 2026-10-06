//! The slice report an agent writes when it finishes a task: `.foundry/reports/<sliceId>.json`.
//! Task prompts (P6) tell agents this shape; reading reports back (P9) parses it. Both use
//! this one type, so the format an agent is asked for and the format Foundry reads can't drift.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Where an agent writes its report for a slice, repo-relative.
pub fn report_path(slice_id: &str) -> String {
    format!(".foundry/reports/{slice_id}.json")
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

/// `.foundry/reports/<sliceId>.json`.
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
    fn report_path_is_under_reports() {
        assert_eq!(report_path("s-1"), ".foundry/reports/s-1.json");
    }
}
