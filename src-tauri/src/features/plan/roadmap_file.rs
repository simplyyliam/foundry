//! Reading `.foundry/roadmap.json` (written by roadmap generation, P3) into plan entries.
//! The roadmap feature isn't imported: this is the file's shape as this feature reads it.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

use super::{
    graph, Deferred, OpenQuestion, PlanFile, Requirement, RequirementKind, RequirementState, Slice,
    SliceStatus, PLAN_VERSION,
};
use crate::core::checks::{self, RequirementCheck};
use crate::core::conflicts::RoadmapConflict;
use crate::core::{ids, AppError, ErrorCode};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadmapFile {
    roadmap: Body,
    #[serde(default)]
    unassigned: Vec<Req>,
    /// Older roadmaps have none.
    #[serde(default)]
    conflicts: Vec<RoadmapConflict>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Body {
    #[serde(default)]
    slices: Vec<RoadSlice>,
    #[serde(default)]
    requirements: Vec<Req>,
    #[serde(default)]
    deferred: Vec<RoadDeferred>,
    #[serde(default)]
    open_questions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoadSlice {
    name: String,
    #[serde(default)]
    delivers: Vec<String>,
    #[serde(default)]
    depends_on: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Req {
    text: String,
    kind: String,
    #[serde(default)]
    source_excerpt: String,
    #[serde(default)]
    slice_name: Option<String>,
    #[serde(default)]
    verified_by: String,
    #[serde(default)]
    unsourced: bool,
    /// Older roadmaps have none. Entries that aren't a valid check are dropped: they're only
    /// suggestions.
    #[serde(default)]
    suggested_checks: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RoadDeferred {
    label: String,
    #[serde(default)]
    anchored_to: String,
}

fn bad(message: String) -> AppError {
    AppError::invalid_input(format!("the roadmap can't be loaded: {message}"))
}

fn kind(value: &str) -> Result<RequirementKind, AppError> {
    match value {
        "behavior" => Ok(RequirementKind::Behavior),
        "interaction" => Ok(RequirementKind::Interaction),
        "constraint" => Ok(RequirementKind::Constraint),
        other => Err(bad(format!(
            "requirement kind \"{other}\" isn't behavior, interaction or constraint \
             (regenerate the roadmap)"
        ))),
    }
}

/// The valid, distinct suggestions, at most `checks::MAX_CHECKS`. Never run from here.
fn suggested(values: &[Value]) -> Vec<RequirementCheck> {
    let mut out: Vec<RequirementCheck> = Vec::new();
    for value in values {
        let check = serde_json::from_value::<RequirementCheck>(value.clone())
            .ok()
            .and_then(|c| checks::validate(&c).ok());
        if let Some(check) = check.filter(|c| !out.contains(c)) {
            out.push(check);
        }
    }
    out.truncate(checks::MAX_CHECKS);
    out
}

/// The whole plan built from the roadmap, plus the labels of deferred items that name no
/// slice. Fails without partial results on any inconsistency.
pub fn to_plan(file: RoadmapFile) -> Result<(PlanFile, Vec<String>), AppError> {
    let mut by_name: HashMap<String, String> = HashMap::new();
    let mut slices: Vec<Slice> = Vec::new();
    for s in &file.roadmap.slices {
        let name = s.name.trim().to_string();
        if name.is_empty() {
            return Err(bad("a slice has no name".into()));
        }
        let id = ids::new_id();
        if by_name.insert(name.clone(), id.clone()).is_some() {
            return Err(bad(format!("two slices are named \"{name}\"")));
        }
        slices.push(Slice {
            id,
            name,
            delivers: s
                .delivers
                .iter()
                .map(|d| d.trim().to_string())
                .filter(|d| !d.is_empty())
                .collect(),
            depends_on: Vec::new(),
            status: SliceStatus::Queued,
            decisions: Vec::new(),
            open_questions: Vec::new(),
            start_commit: None,
            // Never from a proposal: only the human turns it on.
            workspace_access: false,
        });
    }
    for (slice, source) in slices.iter_mut().zip(&file.roadmap.slices) {
        for dep in &source.depends_on {
            let id = by_name.get(dep.trim()).ok_or_else(|| {
                bad(format!(
                    "\"{}\" depends on unknown slice \"{dep}\"",
                    slice.name
                ))
            })?;
            if !slice.depends_on.contains(id) {
                slice.depends_on.push(id.clone());
            }
        }
    }
    if let Some(cycle) = graph::find_cycle(&slices) {
        let names: Vec<&str> = cycle
            .iter()
            .map(|id| {
                slices
                    .iter()
                    .find(|s| &s.id == id)
                    .map_or(id.as_str(), |s| s.name.as_str())
            })
            .collect();
        return Err(AppError::new(
            ErrorCode::DependencyCycle,
            format!("the roadmap's slices form a cycle: {}", names.join(" → ")),
        ));
    }

    let mut requirements = Vec::new();
    for r in file.roadmap.requirements.iter().chain(&file.unassigned) {
        let text = r.text.trim().to_string();
        if text.is_empty() {
            return Err(bad("a requirement has no text".into()));
        }
        requirements.push(Requirement {
            id: ids::new_id(),
            text,
            kind: kind(&r.kind)?,
            slice_id: r
                .slice_name
                .as_deref()
                .and_then(|n| by_name.get(n.trim()).cloned()),
            source_excerpt: r.source_excerpt.trim().to_string(),
            verified_by: r.verified_by.trim().to_string(),
            state: RequirementState::Proposed,
            unsourced: r.unsourced,
            project_wide: false,
            checks: Vec::new(),
            suggested_checks: suggested(&r.suggested_checks),
        });
    }

    let mut deferred = Vec::new();
    let mut unanchored = Vec::new();
    for d in &file.roadmap.deferred {
        let label = d.label.trim().to_string();
        match by_name.get(d.anchored_to.trim()) {
            Some(id) if !label.is_empty() => deferred.push(Deferred {
                id: ids::new_id(),
                label,
                anchored_to: id.clone(),
            }),
            _ => unanchored.push(label),
        }
    }

    let open_questions = file
        .roadmap
        .open_questions
        .iter()
        .map(|q| q.trim())
        .filter(|q| !q.is_empty())
        .map(|q| OpenQuestion {
            id: ids::new_id(),
            text: q.to_string(),
        })
        .collect();

    Ok((
        PlanFile {
            version: PLAN_VERSION,
            revision: 0,
            slices,
            requirements,
            deferred,
            open_questions,
            // As found, all unresolved: rulings are made in the plan.
            conflicts: file
                .conflicts
                .into_iter()
                .map(|c| RoadmapConflict {
                    ruling: None,
                    rule_requirement_id: None,
                    ..c
                })
                .collect(),
        },
        unanchored,
    ))
}
