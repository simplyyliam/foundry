//! Other features' files, as this feature reads them. Nothing here imports those features:
//! each struct is the file's shape as far as prompts need it, and is only ever read.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::Deserialize;

use crate::core::{paths, repo_write, AppError};

/// Written by the plan feature, its only writer.
pub const PLAN_FILE: &str = ".foundry/plan.json";
/// Written by roadmap generation (P3).
pub const ROADMAP_FILE: &str = ".foundry/roadmap.json";
/// Written by the team feature on `confirm_team` / `add_agent_manually`.
pub const ROLES_FILE: &str = "agents/roles.json";
/// Written by the design feature's `confirm_placeholder`.
pub const PLACEHOLDERS_FILE: &str = ".foundry/agents-placeholders.json";
/// Written by the Bakery.
pub const BAKERY_FILE: &str = ".foundry/bakery.json";
pub const AGENTS_FILE: &str = "AGENTS.md";
pub const BAKERY_MANIFEST: &str = "docs/bakery/manifest.md";
pub const DESIGN_MANIFEST: &str = "docs/design/manifest.md";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SliceStatus {
    Queued,
    InProgress,
    Done,
}

impl SliceStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::InProgress => "in progress",
            Self::Done => "done",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenQuestion {
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slice {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub delivers: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<String>,
    pub status: SliceStatus,
    #[serde(default)]
    pub decisions: Vec<String>,
    #[serde(default)]
    pub open_questions: Vec<OpenQuestion>,
    /// The human gave this slice the root workspace files (`hook_policy::WORKSPACE_FILES`).
    #[serde(default)]
    pub workspace_access: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequirementState {
    Proposed,
    Confirmed,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Requirement {
    pub id: String,
    pub text: String,
    pub slice_id: Option<String>,
    #[serde(default)]
    pub verified_by: String,
    pub state: RequirementState,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Deferred {
    pub label: String,
    pub anchored_to: String,
}

/// `.foundry/plan.json`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    #[serde(default)]
    pub slices: Vec<Slice>,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
    #[serde(default)]
    pub deferred: Vec<Deferred>,
}

impl Plan {
    pub fn slice(&self, id: &str) -> Option<&Slice> {
        self.slices.iter().find(|s| s.id == id)
    }

    /// The plan feature's definition of confirmed (its `plan_summary`): at least one slice,
    /// no dependency cycle, every requirement assigned and confirmed.
    pub fn is_confirmed(&self) -> bool {
        !self.slices.is_empty()
            && acyclic(&self.slices)
            && self
                .requirements
                .iter()
                .all(|r| r.slice_id.is_some() && r.state == RequirementState::Confirmed)
    }

    /// The slice's dependencies that aren't done, in its `dependsOn` order.
    pub fn unfinished_dependencies<'a>(&'a self, slice: &'a Slice) -> Vec<&'a Slice> {
        slice
            .depends_on
            .iter()
            .filter_map(|id| self.slice(id))
            .filter(|dep| dep.status != SliceStatus::Done)
            .collect()
    }
}

/// Kahn's algorithm. Dependencies on unknown ids are ignored, as the plan feature does.
fn acyclic(slices: &[Slice]) -> bool {
    let index: HashMap<&str, usize> = slices
        .iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();
    let mut waiting: Vec<usize> = slices
        .iter()
        .map(|s| {
            s.depends_on
                .iter()
                .filter(|d| index.contains_key(d.as_str()))
                .count()
        })
        .collect();
    let mut ready: Vec<usize> = (0..slices.len()).filter(|&i| waiting[i] == 0).collect();
    let mut seen = 0;
    while let Some(i) = ready.pop() {
        seen += 1;
        for (j, other) in slices.iter().enumerate() {
            let hits = other
                .depends_on
                .iter()
                .filter(|d| d.as_str() == slices[i].id)
                .count();
            if hits > 0 {
                waiting[j] = waiting[j].saturating_sub(hits);
                if waiting[j] == 0 {
                    ready.push(j);
                }
            }
        }
    }
    seen == slices.len()
}

#[derive(Debug, Deserialize)]
struct RoadmapFile {
    roadmap: RoadmapBody,
}

#[derive(Debug, Deserialize)]
struct RoadmapBody {
    #[serde(default)]
    goal: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub id: String,
    pub display_name: String,
    pub title: String,
    #[serde(default)]
    pub write_globs: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct RolesFile {
    #[serde(default)]
    agents: Vec<Agent>,
}

#[derive(Debug, Default, Deserialize)]
struct PlaceholdersFile {
    #[serde(default)]
    values: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct BakeryItem {
    file: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    take: String,
}

#[derive(Debug, Default, Deserialize)]
struct BakeryFile {
    #[serde(default)]
    items: Vec<BakeryItem>,
}

fn parse<T: serde::de::DeserializeOwned>(rel: &str, text: &str) -> Result<T, AppError> {
    serde_json::from_str(text)
        .map_err(|err| AppError::internal(format!("{rel} is unreadable: {err}")))
}

fn read_json<T: serde::de::DeserializeOwned>(
    repo: &Path,
    rel: &str,
) -> Result<Option<T>, AppError> {
    repo_write::read_to_string(repo, rel)?
        .map(|text| parse(rel, &text))
        .transpose()
}

/// The plan; an empty one when `plan.json` doesn't exist yet.
pub fn plan(repo: &Path) -> Result<Plan, AppError> {
    Ok(read_json(repo, PLAN_FILE)?.unwrap_or_default())
}

/// The roadmap's goal, if a roadmap exists and has one.
pub fn roadmap_goal(repo: &Path) -> Result<Option<String>, AppError> {
    Ok(read_json::<RoadmapFile>(repo, ROADMAP_FILE)?
        .map(|file| file.roadmap.goal.trim().to_string())
        .filter(|goal| !goal.is_empty()))
}

/// The confirmed team's agents; `None` when no team is confirmed yet.
pub fn team(repo: &Path) -> Result<Option<Vec<Agent>>, AppError> {
    Ok(read_json::<RolesFile>(repo, ROLES_FILE)?.map(|file| file.agents))
}

/// Confirmed AGENTS.md placeholder values by key.
pub fn placeholders(repo: &Path) -> Result<BTreeMap<String, String>, AppError> {
    Ok(read_json::<PlaceholdersFile>(repo, PLACEHOLDERS_FILE)?
        .unwrap_or_default()
        .values)
}

/// Bakery image files with no label or no Take note (the Bakery's own rule), sorted.
pub fn unlabeled_bakery_images(repo: &Path) -> Result<Vec<String>, AppError> {
    let mut files: Vec<String> = read_json::<BakeryFile>(repo, BAKERY_FILE)?
        .unwrap_or_default()
        .items
        .into_iter()
        .filter(|item| item.label.trim().is_empty() || item.take.trim().is_empty())
        .map(|item| item.file)
        .collect();
    files.sort();
    Ok(files)
}

/// Whether a repo-relative file exists, through the same path checks as reads.
pub fn is_file(repo: &Path, rel: &str) -> Result<bool, AppError> {
    Ok(paths::resolve(repo, rel)?.is_file())
}
