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
    /// A rule for the whole project, in no slice. Missing in older plans: `false`.
    #[serde(default)]
    pub project_wide: bool,
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
    /// no dependency cycle, every requirement assigned (or project-wide) and confirmed.
    pub fn is_confirmed(&self) -> bool {
        !self.slices.is_empty()
            && acyclic(&self.slices)
            && self.requirements.iter().all(|r| {
                (r.slice_id.is_some() || r.project_wide) && r.state == RequirementState::Confirmed
            })
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
    /// The Role Handbook role it was made from; `None` for a custom role.
    #[serde(default)]
    pub source_handbook_role_id: Option<String>,
}

/// The Role Handbook's ids for the roles prompts treat specially (the team feature's
/// starter set).
pub const QA_ROLE: &str = "qa-test-engineer";
pub const PRODUCT_LEAD_ROLE: &str = "product-lead";

impl Agent {
    fn title_words(&self) -> Vec<String> {
        self.title
            .split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_lowercase)
            .collect()
    }

    /// A QA or Test Engineer: made from the handbook's QA role, or a custom role whose title
    /// has the word "QA", or "test" next to "engineer" or "tester".
    pub fn is_qa(&self) -> bool {
        if let Some(role) = &self.source_handbook_role_id {
            return role == QA_ROLE;
        }
        let words = self.title_words();
        words.iter().any(|w| w == "qa" || w == "tester")
            || words
                .windows(2)
                .any(|p| p[0] == "test" && p[1] == "engineer")
    }

    /// Made from the handbook's Product Lead role, or a custom role titled "Product Lead".
    pub fn is_product_lead(&self) -> bool {
        match &self.source_handbook_role_id {
            Some(role) => role == PRODUCT_LEAD_ROLE,
            None => self.title_words() == ["product", "lead"],
        }
    }
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

/// One Bakery entry (`.foundry/bakery.json`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BakeryItem {
    /// Missing in hand-edited files: empty.
    #[serde(default)]
    pub id: String,
    pub file: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub take: String,
    #[serde(default)]
    pub skip: String,
    /// Agents or tasks it applies to; empty means everything.
    #[serde(default)]
    pub applies_to: Vec<String>,
}

impl BakeryItem {
    /// Whether a task prompt for this slice and agent refers to it: `appliesTo` is empty, or
    /// names the agent (id, display name or title) or the slice (id or name), ignoring case.
    pub fn applies(&self, slice: &Slice, agent: &Agent) -> bool {
        let names = [
            agent.id.as_str(),
            agent.display_name.as_str(),
            agent.title.as_str(),
            slice.id.as_str(),
            slice.name.as_str(),
        ];
        let targets: Vec<&str> = self
            .applies_to
            .iter()
            .map(|t| t.trim())
            .filter(|t| !t.is_empty())
            .collect();
        targets.is_empty()
            || targets
                .iter()
                .any(|t| names.iter().any(|n| n.trim().eq_ignore_ascii_case(t)))
    }
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

/// Every Bakery entry, by file name (`NN-` first, so in number order).
pub fn bakery_items(repo: &Path) -> Result<Vec<BakeryItem>, AppError> {
    let mut items = read_json::<BakeryFile>(repo, BAKERY_FILE)?
        .unwrap_or_default()
        .items;
    items.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(items)
}

/// Whether a repo-relative file exists, through the same path checks as reads.
pub fn is_file(repo: &Path, rel: &str) -> Result<bool, AppError> {
    Ok(paths::resolve(repo, rel)?.is_file())
}
