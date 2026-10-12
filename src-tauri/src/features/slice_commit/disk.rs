//! What the commit message needs, read from files on disk: the plan, the slice's latest
//! verification, its reports and the team's names.

use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value;

use crate::core::report::{self, RequirementStatus};
use crate::core::{repo_write, AppError, ErrorCode};

const PLAN_FILE: &str = ".foundry/plan.json";
const ROLES_FILE: &str = "agents/roles.json";

/// The parts of a plan slice the commit needs.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SliceInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub delivers: Vec<String>,
    #[serde(default)]
    pub decisions: Vec<String>,
}

impl SliceInfo {
    pub fn first_deliverable(&self) -> &str {
        self.delivers.first().map_or("", String::as_str)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanRequirement {
    id: String,
    #[serde(default)]
    slice_id: Option<String>,
    #[serde(default)]
    project_wide: bool,
}

#[derive(Debug, Deserialize)]
struct PlanFile {
    #[serde(default)]
    slices: Vec<SliceInfo>,
    #[serde(default)]
    requirements: Vec<PlanRequirement>,
}

/// How the slice's latest verification stands, for the `Verification:` line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationLine {
    Passed,
    /// Marked done without a passed verification; the override's reason.
    Overridden(String),
    Failed,
    /// It passed, but a report changed, appeared or went away since.
    Stale,
    NotRun,
}

/// The facts in a generated message's body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageFacts {
    /// The plan's requirement ids for the slice (project-wide ones aren't), in plan order.
    pub required: Vec<String>,
    /// Those the latest verification found met, in plan order.
    pub met: Vec<String>,
    pub verification: VerificationLine,
    /// Who wrote the slice's reports: display names from `roles.json`, else agent ids.
    pub agents: Vec<String>,
}

fn read<T: DeserializeOwned>(repo: &Path, rel: &str) -> Result<Option<T>, AppError> {
    let Some(text) = repo_write::read_to_string(repo, rel)? else {
        return Ok(None);
    };
    serde_json::from_str(text.trim_start_matches('\u{feff}'))
        .map(Some)
        .map_err(|err| {
            AppError::new(
                ErrorCode::InvalidJson,
                format!("{rel} is unreadable ({err})"),
            )
        })
}

/// Like [`read`], but a file that doesn't parse reads as missing.
fn read_lenient<T: DeserializeOwned>(repo: &Path, rel: &str) -> Result<Option<T>, AppError> {
    Ok(repo_write::read_to_string(repo, rel)?
        .and_then(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')).ok()))
}

fn plan(repo: &Path) -> Result<PlanFile, AppError> {
    read(repo, PLAN_FILE)?.ok_or_else(|| AppError::not_found("this project has no plan yet"))
}

/// The slice, or `not_found`.
pub fn slice(repo: &Path, slice_id: &str) -> Result<SliceInfo, AppError> {
    plan(repo)?
        .slices
        .into_iter()
        .find(|s| s.id == slice_id)
        .ok_or_else(|| AppError::not_found(format!("no slice with id {slice_id}")))
}

#[derive(Debug, Deserialize)]
struct StoredReport {
    path: String,
    report: Value,
}

#[derive(Debug, Deserialize)]
struct StoredOutcome {
    id: String,
    #[serde(default)]
    status: Option<RequirementStatus>,
}

#[derive(Debug, Deserialize)]
struct StoredReview {
    passed: bool,
    #[serde(default)]
    report: Value,
    #[serde(default)]
    reports: Vec<StoredReport>,
    #[serde(default)]
    requirements: Vec<StoredOutcome>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Role {
    id: String,
    #[serde(default)]
    display_name: String,
}

#[derive(Debug, Deserialize)]
struct RolesFile {
    #[serde(default)]
    agents: Vec<Role>,
}

pub fn message_facts(repo: &Path, slice: &SliceInfo) -> Result<MessageFacts, AppError> {
    let required: Vec<String> = plan(repo)?
        .requirements
        .into_iter()
        .filter(|r| !r.project_wide && r.slice_id.as_deref() == Some(slice.id.as_str()))
        .map(|r| r.id)
        .collect();

    let review: Option<StoredReview> =
        read_lenient(repo, &format!(".foundry/reviews/{}.json", slice.id))?;
    let met: Vec<String> = match &review {
        Some(review) => required
            .iter()
            .filter(|id| {
                review
                    .requirements
                    .iter()
                    .any(|o| &o.id == *id && o.status == Some(RequirementStatus::Met))
            })
            .cloned()
            .collect(),
        None => Vec::new(),
    };

    let current = match &review {
        Some(review) if review.passed => {
            let verified = report::verified_snapshot(
                &slice.id,
                review
                    .reports
                    .iter()
                    .map(|r| (r.path.clone(), r.report.clone())),
                review.report.clone(),
            );
            Some(report::snapshot(repo, &slice.id)? == verified)
        }
        _ => None,
    };
    let overridden = slice
        .decisions
        .iter()
        .rev()
        .find_map(|d| report::done_override_reason(d));
    let verification = match (&review, current, overridden) {
        (_, Some(true), _) => VerificationLine::Passed,
        (_, _, Some(reason)) => VerificationLine::Overridden(reason.to_string()),
        (Some(_), Some(false), None) => VerificationLine::Stale,
        (Some(_), None, None) => VerificationLine::Failed,
        (None, _, None) => VerificationLine::NotRun,
    };

    Ok(MessageFacts {
        required,
        met,
        verification,
        agents: agents(repo, &slice.id)?,
    })
}

/// The slice's report authors in report order (legacy first, then by agent id), named.
fn agents(repo: &Path, slice_id: &str) -> Result<Vec<String>, AppError> {
    let roles = read_lenient::<RolesFile>(repo, ROLES_FILE)?
        .map(|f| f.agents)
        .unwrap_or_default();
    let mut names = Vec::new();
    for rel in report::slice_report_paths(repo, slice_id)? {
        let from_name = rel
            .rsplit('/')
            .next()
            .and_then(report::parse_report_name)
            .and_then(|(_, agent)| agent)
            .map(str::to_string);
        let id = match from_name {
            Some(id) => id,
            None => match read_lenient::<Value>(repo, &rel)?
                .as_ref()
                .and_then(|v| v.get("agent"))
                .and_then(Value::as_str)
            {
                Some(id) if !id.trim().is_empty() => id.trim().to_string(),
                _ => continue,
            },
        };
        let name = roles
            .iter()
            .find(|r| r.id == id && !r.display_name.trim().is_empty())
            .map_or(id, |r| r.display_name.trim().to_string());
        if !names.contains(&name) {
            names.push(name);
        }
    }
    Ok(names)
}
