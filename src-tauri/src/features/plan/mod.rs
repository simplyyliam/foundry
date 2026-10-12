//! The plan: the value stream map as data, in `.foundry/plan.json`.
//!
//! Every mutation runs under one per-project lock: load, check `expectedRevision`, apply,
//! bump `revision`, save `plan.json`, regenerate `vsm_data.js`, emit `plan:changed`. If the
//! mirror can't be written, `plan.json` is restored, so the two never disagree.

pub mod commands;
mod graph;
mod mirror;
mod roadmap_file;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri_specta::Event;

use crate::core::checks::{self, RequirementCheck};
use crate::core::conflicts::{ConflictRuling, ConflictSourceKind, RoadmapConflict, RulingKind};
use crate::core::events::EventMeta;
use crate::core::git::{Git, GitRun};
use crate::core::repos::RepoIndex;
use crate::core::setup::{self, list_some};
use crate::core::text::normalize_ws;
use crate::core::{ids, repo_write, report, AppError, ErrorCode};

pub use graph::Derived;

const PLAN_FILE: &str = ".foundry/plan.json";
const PROJECT_FILE: &str = ".foundry/project.json";
const BRAIN_DUMP: &str = "docs/brain-dump.md";
/// Written by roadmap generation (P3); read here, never written.
const ROADMAP_FILE: &str = ".foundry/roadmap.json";
/// Written by verification (P9) and by agents; read here, never written.
const REVIEWS_DIR: &str = ".foundry/reviews";
const PLAN_VERSION: u32 = 1;
const MAX_NAME_CHARS: usize = 200;

// ---------- stored types ----------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum SliceStatus {
    Queued,
    InProgress,
    Done,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RequirementKind {
    Behavior,
    Interaction,
    Constraint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RequirementState {
    Proposed,
    Confirmed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OpenQuestion {
    pub id: String,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
// Exported as PlanSlice: P3's roadmap already exports a draft `Slice`.
#[serde(rename = "PlanSlice")] // the type name only: struct JSON is unchanged
pub struct Slice {
    pub id: String,
    pub name: String,
    pub delivers: Vec<String>,
    /// Ids of slices this one depends on.
    pub depends_on: Vec<String>,
    pub status: SliceStatus,
    pub decisions: Vec<String>,
    pub open_questions: Vec<OpenQuestion>,
    /// HEAD when the slice moved to `in_progress` (null outside a git repo or before then).
    pub start_commit: Option<String>,
    /// An agent launched on this slice may also write the root workspace files
    /// (`core::hook_policy::WORKSPACE_FILES`). Set only by the human
    /// (`set_slice_workspace_access`); missing in older plans, which read as `false`.
    #[serde(default)]
    pub workspace_access: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
// Exported as PlanRequirement: P3's roadmap already exports a draft `Requirement`.
#[serde(rename = "PlanRequirement")] // the type name only: struct JSON is unchanged
pub struct Requirement {
    pub id: String,
    pub text: String,
    pub kind: RequirementKind,
    pub slice_id: Option<String>,
    pub source_excerpt: String,
    pub verified_by: String,
    pub state: RequirementState,
    /// The excerpt isn't in `docs/brain-dump.md` (whitespace-normalized), or is empty.
    pub unsourced: bool,
    /// A rule for the whole project, not one slice: `sliceId` is always null while it's
    /// set, and only a `constraint` can have it. Set only by `set_requirement_project_wide`;
    /// missing in older plans, which read as `false`.
    #[serde(default)]
    pub project_wide: bool,
    /// Checks verification runs itself (`set_requirement_checks`, `accept_requirement_check`).
    /// Missing in older plans: none.
    #[serde(default)]
    pub checks: Vec<RequirementCheck>,
    /// Checks the roadmap proposed. Never run: `accept_requirement_check` moves one into
    /// `checks`. Missing in older plans: none.
    #[serde(default)]
    pub suggested_checks: Vec<RequirementCheck>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
// Exported as PlanDeferred: P3's roadmap already exports a draft `Deferred`.
#[serde(rename = "PlanDeferred")] // the type name only: struct JSON is unchanged
pub struct Deferred {
    pub id: String,
    pub label: String,
    /// A slice id.
    pub anchored_to: String,
}

/// `.foundry/plan.json`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanFile {
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub revision: u32,
    #[serde(default)]
    pub slices: Vec<Slice>,
    #[serde(default)]
    pub requirements: Vec<Requirement>,
    #[serde(default)]
    pub deferred: Vec<Deferred>,
    /// Project-wide open questions (a slice's own live on the slice).
    #[serde(default)]
    pub open_questions: Vec<OpenQuestion>,
    /// UI conflicts between the brain dump and the references, from the loaded roadmap.
    /// Missing in older plans: none.
    #[serde(default)]
    pub conflicts: Vec<RoadmapConflict>,
}

// ---------- what the UI gets ----------

/// A slice plus what's derived from the graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SliceView {
    #[serde(flatten)]
    pub slice: Slice,
    /// Longest dependency path from a root (0 = no dependencies); same layer = parallel.
    pub layer: u32,
    /// How many slices depend on this one.
    pub used_by: u32,
    /// Queued, and every dependency is done.
    pub ready: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Plan {
    pub revision: u32,
    pub slices: Vec<SliceView>,
    pub requirements: Vec<Requirement>,
    pub deferred: Vec<Deferred>,
    /// Project-wide open questions.
    pub open_questions: Vec<OpenQuestion>,
    /// Roadmap conflicts with no ruling yet (`list_roadmap_conflicts`). Never blocks
    /// confirming: it's for the UI to warn.
    pub unresolved_conflicts: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LoadRoadmapOptions {
    /// Replace the plan even though some requirements are confirmed.
    pub discard_confirmed: bool,
}

/// What `load_roadmap` loaded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RoadmapLoaded {
    pub plan: Plan,
    pub slices: u32,
    pub requirements: u32,
    pub deferred: u32,
    pub open_questions: u32,
    /// Labels of deferred items whose `anchoredTo` names no slice (e.g. a feature family),
    /// so they couldn't be anchored and weren't loaded.
    pub unanchored_deferred: Vec<String>,
}

/// What an `add_*` command returns: the new item's id and the plan after it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanAdded {
    pub id: String,
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlanSummary {
    /// Requirement ids with no slice that aren't project-wide.
    pub unassigned: Vec<String>,
    /// Requirement ids still `proposed`.
    pub unconfirmed: Vec<String>,
    /// Queued slice ids whose dependencies are all done.
    pub ready: Vec<String>,
    /// Queued slice ids waiting on a dependency.
    pub blocked: Vec<String>,
    pub acyclic: bool,
    /// At least one slice, the graph is acyclic, and every requirement is handled (assigned
    /// to a slice, or project-wide) and confirmed.
    pub confirmed: bool,
}

/// `plan:changed`, after every successful mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
#[tauri_specta(event_name = "plan:changed")]
pub struct PlanChanged {
    #[serde(flatten)]
    pub meta: EventMeta,
    pub project_id: String,
    pub revision: u32,
}

pub trait PlanEvents: Send + Sync + 'static {
    fn changed(&self, event: PlanChanged);
}

// ---------- inputs ----------

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewSlice {
    pub name: String,
    pub delivers: Vec<String>,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SlicePatch {
    pub name: Option<String>,
    pub delivers: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewRequirement {
    pub text: String,
    pub kind: RequirementKind,
    pub slice_id: Option<String>,
    pub source_excerpt: String,
    pub verified_by: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RequirementPatch {
    pub text: Option<String>,
    pub kind: Option<RequirementKind>,
    pub source_excerpt: Option<String>,
    pub verified_by: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StatusOptions {
    /// Start before the dependencies are done, or mark done without a passed, current
    /// verification. Needs a `reason`, recorded as a decision.
    #[serde(rename = "override")]
    pub override_gate: bool,
    pub reason: Option<String>,
}

/// What `resolve_roadmap_conflict` returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictResolved {
    pub conflict: RoadmapConflict,
    /// The project-wide constraint stating the ruling, when there is one (`addAsRule`, or
    /// one made by an earlier ruling, which is rewritten to match).
    pub rule_requirement_id: Option<String>,
    pub plan: Plan,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewDeferred {
    pub label: String,
    pub anchored_to: String,
}

// ---------- helpers ----------

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn required(what: &str, text: &str) -> Result<String, AppError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::invalid_input(format!("{what} can't be empty")));
    }
    if text.chars().count() > MAX_NAME_CHARS && what.ends_with("name") {
        return Err(AppError::invalid_input(format!(
            "{what} must be {MAX_NAME_CHARS} characters or fewer"
        )));
    }
    Ok(text.to_string())
}

fn clean_list(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect()
}

fn slice_index(plan: &PlanFile, id: &str) -> Result<usize, AppError> {
    plan.slices
        .iter()
        .position(|s| s.id == id)
        .ok_or_else(|| AppError::not_found(format!("no slice with id {id}")))
}

fn requirement_index(plan: &PlanFile, id: &str) -> Result<usize, AppError> {
    plan.requirements
        .iter()
        .position(|r| r.id == id)
        .ok_or_else(|| AppError::not_found(format!("no requirement with id {id}")))
}

/// Has a home: a slice, or the whole project.
fn is_handled(r: &Requirement) -> bool {
    r.slice_id.is_some() || r.project_wide
}

fn slice_name<'a>(plan: &'a PlanFile, id: &'a str) -> &'a str {
    plan.slices
        .iter()
        .find(|s| s.id == id)
        .map_or(id, |s| s.name.as_str())
}

/// Checks a dependency list for `slice_id`: known ids, no self-dependency, no cycle.
/// Returns it de-duplicated.
fn checked_dependencies(
    plan: &PlanFile,
    slice_id: &str,
    depends_on: Vec<String>,
) -> Result<Vec<String>, AppError> {
    let mut seen = HashSet::new();
    let mut deps = Vec::new();
    for dep in depends_on.into_iter().map(|d| d.trim().to_string()) {
        if dep == slice_id {
            return Err(AppError::invalid_input(format!(
                "slice \"{}\" can't depend on itself",
                slice_name(plan, slice_id)
            )));
        }
        slice_index(plan, &dep)?;
        if seen.insert(dep.clone()) {
            deps.push(dep);
        }
    }
    let mut trial = plan.slices.clone();
    if let Some(slice) = trial.iter_mut().find(|s| s.id == slice_id) {
        slice.depends_on = deps.clone();
    }
    if let Some(cycle) = graph::find_cycle(&trial) {
        let names: Vec<&str> = cycle.iter().map(|id| slice_name(plan, id)).collect();
        return Err(AppError::new(
            ErrorCode::DependencyCycle,
            format!(
                "these dependencies would form a cycle: {}",
                names.join(" → ")
            ),
        ));
    }
    Ok(deps)
}

/// `stale_revision` unless `expected` is absent or the plan's revision.
fn check_revision(plan: &PlanFile, expected: Option<u32>) -> Result<(), AppError> {
    match expected {
        Some(expected) if expected != plan.revision => Err(AppError::new(
            ErrorCode::StaleRevision,
            format!(
                "the plan changed (revision {} now, {expected} expected): reload it",
                plan.revision
            ),
        )),
        _ => Ok(()),
    }
}

/// Whether `excerpt` appears in the brain dump, whitespace-normalized.
fn is_sourced(repo: &Path, excerpt: &str) -> Result<bool, AppError> {
    let excerpt = normalize_ws(excerpt);
    if excerpt.is_empty() {
        return Ok(false);
    }
    let brain_dump = repo_write::read_to_string(repo, BRAIN_DUMP)?.unwrap_or_default();
    Ok(normalize_ws(&brain_dump).contains(&excerpt))
}

/// The plan as the UI sees it.
pub fn view(plan: &PlanFile) -> Plan {
    let derived = graph::derive(&plan.slices);
    let done: HashSet<&str> = plan
        .slices
        .iter()
        .filter(|s| s.status == SliceStatus::Done)
        .map(|s| s.id.as_str())
        .collect();
    Plan {
        revision: plan.revision,
        slices: plan
            .slices
            .iter()
            .map(|s| SliceView {
                layer: derived.layer.get(&s.id).copied().unwrap_or(0),
                used_by: derived.used_by.get(&s.id).copied().unwrap_or(0),
                ready: s.status == SliceStatus::Queued
                    && s.depends_on.iter().all(|d| done.contains(d.as_str())),
                slice: s.clone(),
            })
            .collect(),
        requirements: plan.requirements.clone(),
        deferred: plan.deferred.clone(),
        open_questions: plan.open_questions.clone(),
        unresolved_conflicts: u32::try_from(
            plan.conflicts.iter().filter(|c| c.ruling.is_none()).count(),
        )
        .unwrap_or(u32::MAX),
    }
}

pub fn summary(plan: &PlanFile) -> PlanSummary {
    let derived = graph::derive(&plan.slices);
    let view = view(plan);
    let unassigned: Vec<String> = plan
        .requirements
        .iter()
        .filter(|r| !is_handled(r))
        .map(|r| r.id.clone())
        .collect();
    let unconfirmed: Vec<String> = plan
        .requirements
        .iter()
        .filter(|r| r.state != RequirementState::Confirmed)
        .map(|r| r.id.clone())
        .collect();
    let queued = view
        .slices
        .iter()
        .filter(|s| s.slice.status == SliceStatus::Queued);
    let (ready, blocked): (Vec<&SliceView>, Vec<&SliceView>) = queued.partition(|s| s.ready);
    PlanSummary {
        confirmed: !plan.slices.is_empty()
            && derived.acyclic
            && unassigned.is_empty()
            && unconfirmed.is_empty(),
        unassigned,
        unconfirmed,
        ready: ready.iter().map(|s| s.slice.id.clone()).collect(),
        blocked: blocked.iter().map(|s| s.slice.id.clone()).collect(),
        acyclic: derived.acyclic,
    }
}

// ---------- the store ----------

/// Owns every project's plan. Share it as `Arc<PlanStore>`.
pub struct PlanStore {
    repos: Arc<RepoIndex>,
    git: Git,
    events: Arc<dyn PlanEvents>,
    /// One writer at a time, per project.
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl PlanStore {
    pub fn new(repos: Arc<RepoIndex>, git: Git, events: Arc<dyn PlanEvents>) -> Self {
        Self {
            repos,
            git,
            events,
            locks: Mutex::new(HashMap::new()),
        }
    }

    fn project_lock(&self, project_id: &str) -> Arc<Mutex<()>> {
        Arc::clone(lock(&self.locks).entry(project_id.to_string()).or_default())
    }

    fn load(repo: &Path) -> Result<PlanFile, AppError> {
        match repo_write::read_to_string(repo, PLAN_FILE)? {
            None => Ok(PlanFile {
                version: PLAN_VERSION,
                ..PlanFile::default()
            }),
            // Never reset a plan Foundry can't read: it's the source of truth.
            Some(text) => serde_json::from_str(&text).map_err(|err| {
                AppError::internal(format!(
                    "{PLAN_FILE} is unreadable ({err}); fix or remove it to continue"
                ))
            }),
        }
    }

    pub fn get(&self, project_id: &str) -> Result<Plan, AppError> {
        let repo = self.repos.repo(project_id)?;
        let guard = self.project_lock(project_id);
        let _guard = lock(&guard);
        Ok(view(&Self::load(&repo)?))
    }

    pub fn summary(&self, project_id: &str) -> Result<PlanSummary, AppError> {
        let repo = self.repos.repo(project_id)?;
        let guard = self.project_lock(project_id);
        let _guard = lock(&guard);
        Ok(summary(&Self::load(&repo)?))
    }

    /// Load → check revision → apply → bump → save plan.json → write vsm_data.js → emit,
    /// all under the project's lock. `apply` failing changes nothing.
    fn mutate<T>(
        &self,
        project_id: &str,
        expected_revision: Option<u32>,
        apply: impl FnOnce(&mut PlanFile, &Path) -> Result<T, AppError>,
    ) -> Result<(T, Plan), AppError> {
        let repo = self.repos.repo(project_id)?;
        let guard = self.project_lock(project_id);
        let _guard = lock(&guard);

        let previous = repo_write::read_to_string(&repo, PLAN_FILE)?;
        let mut plan = Self::load(&repo)?;
        check_revision(&plan, expected_revision)?;
        let out = apply(&mut plan, &repo)?;
        plan.version = PLAN_VERSION;
        plan.revision = plan
            .revision
            .checked_add(1)
            .ok_or_else(|| AppError::internal("plan revision overflowed"))?;

        let json = serde_json::to_string_pretty(&plan)
            .map_err(|err| AppError::internal(format!("serialize plan: {err}")))?;
        repo_write::write(&repo, PLAN_FILE, json + "\n")?;
        if let Err(err) = self.write_mirror(&repo, &plan) {
            // Keep the pair consistent: put plan.json back as it was.
            let restored = match &previous {
                Some(text) => repo_write::write(&repo, PLAN_FILE, text),
                None => repo_write::remove(&repo, PLAN_FILE).map(|_| ()),
            };
            if let Err(restore) = restored {
                tracing::error!(error = %restore, "could not restore plan.json after a failed mirror write");
            }
            return Err(err);
        }
        self.events.changed(PlanChanged {
            meta: EventMeta::new(),
            project_id: project_id.to_string(),
            revision: plan.revision,
        });
        tracing::info!(project_id, revision = plan.revision, "plan changed");
        Ok((out, view(&plan)))
    }

    fn write_mirror(&self, repo: &Path, plan: &PlanFile) -> Result<(), AppError> {
        let (title, subtitle) = project_meta(repo);
        let text = mirror::render(plan, &graph::derive(&plan.slices), &title, &subtitle);
        if repo_write::read_to_string(repo, mirror::FILE)?.as_deref() == Some(text.as_str()) {
            return Ok(());
        }
        repo_write::write(repo, mirror::FILE, text)
    }

    fn head(&self, repo: &Path) -> Option<String> {
        match self.git.run(repo, &["rev-parse", "HEAD"]) {
            Ok(GitRun::Ok(output)) => {
                Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
                    .filter(|sha| !sha.is_empty())
            }
            _ => None,
        }
    }

    /// Setup files that are untracked or differ from HEAD (`core::setup`). Not a git repo,
    /// or git failing: none (there's no start commit to compare against, and verification
    /// refuses the slice for that).
    fn uncommitted_setup(&self, repo: &Path) -> Vec<String> {
        setup::uncommitted(&self.git, repo)
    }

    // ---------- slices ----------

    pub fn add_slice(
        &self,
        project_id: &str,
        input: NewSlice,
        expected: Option<u32>,
    ) -> Result<PlanAdded, AppError> {
        let name = required("slice name", &input.name)?;
        let id = ids::new_id();
        let new_id = id.clone();
        let ((), plan) = self.mutate(project_id, expected, move |plan, _| {
            plan.slices.push(Slice {
                id: new_id.clone(),
                name,
                delivers: clean_list(input.delivers),
                depends_on: Vec::new(),
                status: SliceStatus::Queued,
                decisions: Vec::new(),
                open_questions: Vec::new(),
                start_commit: None,
                workspace_access: false,
            });
            let deps = checked_dependencies(plan, &new_id, input.depends_on)?;
            if let Some(slice) = plan.slices.last_mut() {
                slice.depends_on = deps;
            }
            Ok(())
        })?;
        Ok(PlanAdded { id, plan })
    }

    pub fn update_slice(
        &self,
        project_id: &str,
        slice_id: &str,
        patch: SlicePatch,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let name = patch
            .name
            .as_deref()
            .map(|n| required("slice name", n))
            .transpose()?;
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let index = slice_index(plan, slice_id)?;
            let slice = &mut plan.slices[index];
            if let Some(name) = name {
                slice.name = name;
            }
            if let Some(delivers) = patch.delivers {
                slice.delivers = clean_list(delivers);
            }
            Ok(())
        })?;
        Ok(plan)
    }

    /// Refused (`slice_in_use`) while requirements are assigned to it, other slices depend
    /// on it, or deferred items are anchored to it.
    pub fn remove_slice(
        &self,
        project_id: &str,
        slice_id: &str,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let index = slice_index(plan, slice_id)?;
            let in_use = |what: String| {
                AppError::new(
                    ErrorCode::SliceInUse,
                    format!("\"{}\" still has {what}", plan.slices[index].name),
                )
            };
            let assigned = plan
                .requirements
                .iter()
                .filter(|r| r.slice_id.as_deref() == Some(slice_id))
                .count();
            if assigned > 0 {
                return Err(in_use(format!(
                    "{assigned} requirement(s) assigned: reassign them first"
                )));
            }
            let dependents: Vec<&str> = plan
                .slices
                .iter()
                .filter(|s| s.depends_on.iter().any(|d| d == slice_id))
                .map(|s| s.name.as_str())
                .collect();
            if !dependents.is_empty() {
                return Err(in_use(format!(
                    "dependents ({}): change their dependencies first",
                    dependents.join(", ")
                )));
            }
            if plan.deferred.iter().any(|d| d.anchored_to == slice_id) {
                return Err(in_use(
                    "deferred items anchored to it: move or remove them first".into(),
                ));
            }
            plan.slices.remove(index);
            Ok(())
        })?;
        Ok(plan)
    }

    pub fn set_dependencies(
        &self,
        project_id: &str,
        slice_id: &str,
        depends_on: Vec<String>,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let index = slice_index(plan, slice_id)?;
            let deps = checked_dependencies(plan, slice_id, depends_on)?;
            plan.slices[index].depends_on = deps;
            Ok(())
        })?;
        Ok(plan)
    }

    /// Turns the slice's `workspaceAccess` on or off. Only the human calls this; nothing an
    /// agent or a roadmap proposal writes reaches it. Takes effect for sessions launched on
    /// the slice afterwards (a running one keeps what it was launched with) and for its next
    /// verification.
    pub fn set_slice_workspace_access(
        &self,
        project_id: &str,
        slice_id: &str,
        value: bool,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let index = slice_index(plan, slice_id)?;
            plan.slices[index].workspace_access = value;
            Ok(())
        })?;
        tracing::info!(project_id, slice_id, value, "slice workspace access set");
        Ok(plan)
    }

    /// `queued → in_progress` needs every dependency `done`, and moving to `done` needs a
    /// passed, non-stale verification (`not_verified`), unless `override` is set with a
    /// non-empty reason (recorded as a decision). Moving to `in_progress` records HEAD.
    pub fn set_slice_status(
        &self,
        project_id: &str,
        slice_id: &str,
        status: SliceStatus,
        options: StatusOptions,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let reason = options
            .reason
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty());
        if options.override_gate && reason.is_none() {
            return Err(AppError::invalid_input("an override needs a reason"));
        }
        let (_, plan) = self.mutate(project_id, expected, |plan, repo| {
            let index = slice_index(plan, slice_id)?;
            let current = plan.slices[index].status;
            if current == SliceStatus::Queued && status == SliceStatus::InProgress {
                let pending: Vec<String> = plan.slices[index]
                    .depends_on
                    .iter()
                    .filter(|d| {
                        plan.slices
                            .iter()
                            .find(|s| &s.id == *d)
                            .is_some_and(|s| s.status != SliceStatus::Done)
                    })
                    .map(|d| slice_name(plan, d).to_string())
                    .collect();
                if !pending.is_empty() {
                    let Some(reason) = reason.as_deref().filter(|_| options.override_gate) else {
                        return Err(AppError::new(
                            ErrorCode::DependenciesNotDone,
                            format!(
                                "\"{}\" depends on slices that aren't done: {}",
                                plan.slices[index].name,
                                pending.join(", ")
                            ),
                        ));
                    };
                    plan.slices[index].decisions.push(format!(
                        "Started before {} {} done (override): {reason}",
                        pending.join(", "),
                        if pending.len() == 1 { "was" } else { "were" }
                    ));
                }
            }
            if status == SliceStatus::Done && current != SliceStatus::Done {
                if let Err(why) = verified(repo, slice_id)? {
                    let Some(reason) = reason.as_deref().filter(|_| options.override_gate) else {
                        return Err(AppError::new(
                            ErrorCode::NotVerified,
                            format!("\"{}\" {why}", plan.slices[index].name),
                        ));
                    };
                    plan.slices[index].decisions.push(format!(
                        "{}; it {why}): {reason}",
                        report::DONE_OVERRIDE_PREFIX
                    ));
                }
            }
            if status == SliceStatus::InProgress && current != SliceStatus::InProgress {
                let uncommitted = self.uncommitted_setup(repo);
                if !uncommitted.is_empty() {
                    return Err(AppError::new(
                        ErrorCode::SetupNotCommitted,
                        format!(
                            "Commit your setup first: {} {} not committed. Verification \
                             compares a slice against the commit it starts from, so these \
                             would count as the agent's changes.",
                            list_some(&uncommitted),
                            if uncommitted.len() == 1 { "is" } else { "are" }
                        ),
                    ));
                }
                plan.slices[index].start_commit = self.head(repo);
            }
            plan.slices[index].status = status;
            Ok(())
        })?;
        Ok(plan)
    }

    pub fn add_decision(
        &self,
        project_id: &str,
        slice_id: &str,
        text: &str,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let text = required("decision", text)?;
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let index = slice_index(plan, slice_id)?;
            plan.slices[index].decisions.push(text);
            Ok(())
        })?;
        Ok(plan)
    }

    pub fn add_open_question(
        &self,
        project_id: &str,
        slice_id: &str,
        text: &str,
        expected: Option<u32>,
    ) -> Result<PlanAdded, AppError> {
        let text = required("question", text)?;
        let id = ids::new_id();
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let index = slice_index(plan, slice_id)?;
            plan.slices[index].open_questions.push(OpenQuestion {
                id: id.clone(),
                text,
            });
            Ok(())
        })?;
        Ok(PlanAdded { id, plan })
    }

    /// Removes the question and records "question → resolution" as a decision.
    pub fn resolve_open_question(
        &self,
        project_id: &str,
        slice_id: &str,
        question_id: &str,
        resolution: &str,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let resolution = required("resolution", resolution)?;
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let index = slice_index(plan, slice_id)?;
            let slice = &mut plan.slices[index];
            let at = slice
                .open_questions
                .iter()
                .position(|q| q.id == question_id)
                .ok_or_else(|| AppError::not_found(format!("no open question {question_id}")))?;
            let question = slice.open_questions.remove(at);
            slice
                .decisions
                .push(format!("{} → {resolution}", question.text));
            Ok(())
        })?;
        Ok(plan)
    }

    // ---------- requirements ----------

    pub fn add_requirement(
        &self,
        project_id: &str,
        input: NewRequirement,
        expected: Option<u32>,
    ) -> Result<PlanAdded, AppError> {
        let text = required("requirement text", &input.text)?;
        let id = ids::new_id();
        let (_, plan) = self.mutate(project_id, expected, |plan, repo| {
            if let Some(slice_id) = &input.slice_id {
                slice_index(plan, slice_id)?;
            }
            plan.requirements.push(Requirement {
                id: id.clone(),
                text,
                kind: input.kind,
                slice_id: input.slice_id,
                unsourced: !is_sourced(repo, &input.source_excerpt)?,
                source_excerpt: input.source_excerpt.trim().to_string(),
                verified_by: input.verified_by.trim().to_string(),
                state: RequirementState::Proposed,
                project_wide: false,
                checks: Vec::new(),
                suggested_checks: Vec::new(),
            });
            Ok(())
        })?;
        Ok(PlanAdded { id, plan })
    }

    /// Changing what a confirmed requirement says (text, kind, excerpt) makes it `proposed`
    /// again: it was confirmed as it was.
    pub fn update_requirement(
        &self,
        project_id: &str,
        requirement_id: &str,
        patch: RequirementPatch,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let text = patch
            .text
            .as_deref()
            .map(|t| required("requirement text", t))
            .transpose()?;
        let (_, plan) = self.mutate(project_id, expected, |plan, repo| {
            let index = requirement_index(plan, requirement_id)?;
            let sourced = patch
                .source_excerpt
                .as_deref()
                .map(|e| is_sourced(repo, e))
                .transpose()?;
            let r = &mut plan.requirements[index];
            let mut changed_meaning = false;
            if let Some(text) = text {
                changed_meaning |= text != r.text;
                r.text = text;
            }
            if let Some(kind) = patch.kind {
                if r.project_wide && kind != RequirementKind::Constraint {
                    return Err(AppError::invalid_input(
                        "a project-wide requirement must stay a constraint: turn project-wide \
                         off first",
                    ));
                }
                changed_meaning |= kind != r.kind;
                r.kind = kind;
            }
            if let (Some(excerpt), Some(sourced)) = (patch.source_excerpt, sourced) {
                let excerpt = excerpt.trim().to_string();
                changed_meaning |= excerpt != r.source_excerpt;
                r.source_excerpt = excerpt;
                r.unsourced = !sourced;
            }
            if let Some(verified_by) = patch.verified_by {
                r.verified_by = verified_by.trim().to_string();
            }
            if changed_meaning {
                r.state = RequirementState::Proposed;
            }
            Ok(())
        })?;
        Ok(plan)
    }

    /// `slice_id: null` unassigns it (and so unconfirms it). Assigning a project-wide
    /// requirement to a slice turns project-wide off: it belongs to that slice now.
    pub fn assign_requirement(
        &self,
        project_id: &str,
        requirement_id: &str,
        slice_id: Option<String>,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            if let Some(slice_id) = &slice_id {
                slice_index(plan, slice_id)?;
            }
            let index = requirement_index(plan, requirement_id)?;
            let r = &mut plan.requirements[index];
            if slice_id.is_none() {
                r.state = RequirementState::Proposed;
            } else {
                r.project_wide = false;
            }
            r.slice_id = slice_id;
            Ok(())
        })?;
        Ok(plan)
    }

    /// Turns the requirement's `projectWide` on or off. Only a `constraint` can be
    /// project-wide (`invalid_input` otherwise, either way). On clears its slice; off leaves it
    /// unassigned. Its state (proposed or confirmed) is left as it is.
    pub fn set_requirement_project_wide(
        &self,
        project_id: &str,
        requirement_id: &str,
        value: bool,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let index = requirement_index(plan, requirement_id)?;
            let r = &mut plan.requirements[index];
            if r.kind != RequirementKind::Constraint {
                return Err(AppError::invalid_input(
                    "only a constraint can be project-wide",
                ));
            }
            r.project_wide = value;
            r.slice_id = None;
            Ok(())
        })?;
        tracing::info!(
            project_id,
            requirement_id,
            value,
            "requirement project-wide set"
        );
        Ok(plan)
    }

    /// Replaces the requirement's `checks` (each validated: `invalid_input`, `path_escape`;
    /// exact duplicates dropped). Its state is left as it is: checks don't change what it says.
    pub fn set_requirement_checks(
        &self,
        project_id: &str,
        requirement_id: &str,
        checks: Vec<RequirementCheck>,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let checks = checks::validate_all(&checks)?;
        let count = checks.len();
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let index = requirement_index(plan, requirement_id)?;
            plan.requirements[index].checks = checks;
            Ok(())
        })?;
        tracing::info!(project_id, requirement_id, count, "requirement checks set");
        Ok(plan)
    }

    /// Moves `suggestedChecks[index]` into `checks` (validated first; already there: just
    /// removed from the suggestions). `invalid_input` for an index out of range.
    pub fn accept_requirement_check(
        &self,
        project_id: &str,
        requirement_id: &str,
        index: u32,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let at = requirement_index(plan, requirement_id)?;
            let r = &mut plan.requirements[at];
            let i = usize::try_from(index).unwrap_or(usize::MAX);
            let Some(suggested) = r.suggested_checks.get(i) else {
                return Err(AppError::invalid_input(format!(
                    "requirement {requirement_id} has no suggested check {index}"
                )));
            };
            let check = checks::validate(suggested)?;
            if !r.checks.contains(&check) {
                if r.checks.len() >= checks::MAX_CHECKS {
                    return Err(AppError::invalid_input(format!(
                        "a requirement can have at most {} checks",
                        checks::MAX_CHECKS
                    )));
                }
                r.checks.push(check);
            }
            r.suggested_checks.remove(i);
            Ok(())
        })?;
        tracing::info!(
            project_id,
            requirement_id,
            index,
            "suggested check accepted"
        );
        Ok(plan)
    }

    pub fn remove_requirement(
        &self,
        project_id: &str,
        requirement_id: &str,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let index = requirement_index(plan, requirement_id)?;
            plan.requirements.remove(index);
            Ok(())
        })?;
        Ok(plan)
    }

    pub fn confirm_requirement(
        &self,
        project_id: &str,
        requirement_id: &str,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        self.set_state(
            project_id,
            requirement_id,
            RequirementState::Confirmed,
            expected,
        )
    }

    pub fn unconfirm_requirement(
        &self,
        project_id: &str,
        requirement_id: &str,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        self.set_state(
            project_id,
            requirement_id,
            RequirementState::Proposed,
            expected,
        )
    }

    fn set_state(
        &self,
        project_id: &str,
        requirement_id: &str,
        state: RequirementState,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let index = requirement_index(plan, requirement_id)?;
            let r = &mut plan.requirements[index];
            if state == RequirementState::Confirmed && !is_handled(r) {
                return Err(AppError::new(
                    ErrorCode::RequirementUnassigned,
                    "assign this requirement to a slice, or make it project-wide, before \
                     confirming it",
                ));
            }
            r.state = state;
            Ok(())
        })?;
        Ok(plan)
    }

    /// Confirms every assigned or project-wide requirement. Unassigned ones stay proposed
    /// (see `unassigned` in `plan_summary`).
    pub fn confirm_all(&self, project_id: &str, expected: Option<u32>) -> Result<Plan, AppError> {
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            for r in plan.requirements.iter_mut().filter(|r| is_handled(r)) {
                r.state = RequirementState::Confirmed;
            }
            Ok(())
        })?;
        Ok(plan)
    }

    // ---------- loading a generated roadmap ----------

    /// Replaces the plan with `.foundry/roadmap.json`, everything `proposed`: one mutation
    /// (one revision, one `vsm_data.js` write, one `plan:changed`). Refused
    /// (`confirmed_requirements`) while any requirement is confirmed, unless
    /// `discardConfirmed`. Any problem with the roadmap fails the whole load and changes
    /// nothing.
    pub fn load_roadmap(
        &self,
        project_id: &str,
        options: LoadRoadmapOptions,
        expected: Option<u32>,
    ) -> Result<RoadmapLoaded, AppError> {
        let (counts, plan) = self.mutate(project_id, expected, |plan, repo| {
            let confirmed = plan
                .requirements
                .iter()
                .filter(|r| r.state == RequirementState::Confirmed)
                .count();
            if confirmed > 0 && !options.discard_confirmed {
                return Err(AppError::new(
                    ErrorCode::ConfirmedRequirements,
                    format!(
                        "loading the roadmap would replace {confirmed} confirmed \
                         requirement(s); load with discardConfirmed to do it anyway"
                    ),
                ));
            }
            let text = repo_write::read_to_string(repo, ROADMAP_FILE)?.ok_or_else(|| {
                AppError::not_found("there's no generated roadmap yet: generate one first")
            })?;
            let roadmap: roadmap_file::RoadmapFile =
                serde_json::from_str(&text).map_err(|err| {
                    AppError::invalid_input(format!("{ROADMAP_FILE} is unreadable: {err}"))
                })?;
            let (loaded, unanchored) = roadmap_file::to_plan(roadmap)?;
            let counts = (
                loaded.slices.len(),
                loaded.requirements.len(),
                loaded.deferred.len(),
                loaded.open_questions.len(),
                unanchored,
            );
            *plan = PlanFile {
                revision: plan.revision,
                ..loaded
            };
            Ok(counts)
        })?;
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Ok(RoadmapLoaded {
            plan,
            slices: count(counts.0),
            requirements: count(counts.1),
            deferred: count(counts.2),
            open_questions: count(counts.3),
            unanchored_deferred: counts.4,
        })
    }

    // ---------- roadmap conflicts ----------

    /// The plan's roadmap conflicts, resolved or not, in the order they were found.
    pub fn list_conflicts(&self, project_id: &str) -> Result<Vec<RoadmapConflict>, AppError> {
        let repo = self.repos.repo(project_id)?;
        let guard = self.project_lock(project_id);
        let _guard = lock(&guard);
        Ok(Self::load(&repo)?.conflicts)
    }

    /// Records the ruling (again, if it had one). With `add_as_rule`, also adds a confirmed,
    /// project-wide constraint stating it; a rule made by an earlier ruling is rewritten
    /// instead, so there's never more than one per conflict. One mutation either way.
    pub fn resolve_conflict(
        &self,
        project_id: &str,
        conflict_id: &str,
        ruling: ConflictRuling,
        add_as_rule: bool,
        expected: Option<u32>,
    ) -> Result<ConflictResolved, AppError> {
        let text = ruling
            .text
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(str::to_string);
        if ruling.kind == RulingKind::Custom && text.is_none() {
            return Err(AppError::invalid_input("a custom ruling needs its text"));
        }
        let ruling = ConflictRuling {
            kind: ruling.kind,
            text,
        };
        let ((conflict, rule_id), plan) = self.mutate(project_id, expected, |plan, repo| {
            let index = plan
                .conflicts
                .iter()
                .position(|c| c.id == conflict_id)
                .ok_or_else(|| AppError::not_found(format!("no conflict with id {conflict_id}")))?;
            let mut conflict = plan.conflicts[index].clone();
            conflict.ruling = Some(ruling);
            let rule_text = rule_text(&conflict);
            let existing = conflict
                .rule_requirement_id
                .as_deref()
                .and_then(|id| plan.requirements.iter().position(|r| r.id == id));
            let rule_id = match existing {
                Some(at) => {
                    let r = &mut plan.requirements[at];
                    r.text = rule_text;
                    Some(r.id.clone())
                }
                None if add_as_rule => {
                    let id = ids::new_id();
                    plan.requirements.push(Requirement {
                        id: id.clone(),
                        text: rule_text,
                        kind: RequirementKind::Constraint,
                        slice_id: None,
                        unsourced: !is_sourced(repo, &conflict.brain_dump_excerpt)?,
                        source_excerpt: conflict.brain_dump_excerpt.clone(),
                        verified_by: "Review the UI against this ruling.".into(),
                        // The human's own ruling: nothing left to confirm.
                        state: RequirementState::Confirmed,
                        project_wide: true,
                        checks: Vec::new(),
                        suggested_checks: Vec::new(),
                    });
                    Some(id)
                }
                None => None,
            };
            conflict.rule_requirement_id = rule_id.clone();
            plan.conflicts[index] = conflict.clone();
            Ok((conflict, rule_id))
        })?;
        tracing::info!(
            project_id,
            conflict_id,
            rule = rule_id.is_some(),
            "roadmap conflict resolved"
        );
        Ok(ConflictResolved {
            conflict,
            rule_requirement_id: rule_id,
            plan,
        })
    }

    /// Clears the conflict's ruling and removes the rule it created, unless the human has
    /// edited that rule since (then it stays, unlinked); a rule that's gone just loses its
    /// link. A conflict with no ruling: nothing is written, the current plan is returned.
    pub fn unresolve_conflict(
        &self,
        project_id: &str,
        conflict_id: &str,
        expected: Option<u32>,
    ) -> Result<ConflictResolved, AppError> {
        let find = |plan: &PlanFile| {
            plan.conflicts
                .iter()
                .position(|c| c.id == conflict_id)
                .ok_or_else(|| AppError::not_found(format!("no conflict with id {conflict_id}")))
        };
        {
            let repo = self.repos.repo(project_id)?;
            let guard = self.project_lock(project_id);
            let _guard = lock(&guard);
            let plan = Self::load(&repo)?;
            check_revision(&plan, expected)?;
            let conflict = &plan.conflicts[find(&plan)?];
            if conflict.ruling.is_none() {
                return Ok(ConflictResolved {
                    conflict: conflict.clone(),
                    rule_requirement_id: None,
                    plan: view(&plan),
                });
            }
        }
        let ((conflict, removed), plan) = self.mutate(project_id, expected, |plan, _| {
            let index = find(plan)?;
            let mut conflict = plan.conflicts[index].clone();
            let expected_text = rule_text(&conflict);
            let mut removed = false;
            if let Some(rule_id) = conflict.rule_requirement_id.take() {
                let unedited = plan.requirements.iter().position(|r| {
                    r.id == rule_id
                        && r.text == expected_text
                        && r.project_wide
                        && r.kind == RequirementKind::Constraint
                });
                if let Some(at) = unedited {
                    plan.requirements.remove(at);
                    removed = true;
                }
            }
            conflict.ruling = None;
            plan.conflicts[index] = conflict.clone();
            Ok((conflict, removed))
        })?;
        tracing::info!(
            project_id,
            conflict_id,
            rule_removed = removed,
            "roadmap conflict unresolved"
        );
        Ok(ConflictResolved {
            conflict,
            rule_requirement_id: None,
            plan,
        })
    }

    // ---------- deferred ----------

    pub fn add_deferred(
        &self,
        project_id: &str,
        input: NewDeferred,
        expected: Option<u32>,
    ) -> Result<PlanAdded, AppError> {
        let label = required("deferred label", &input.label)?;
        let id = ids::new_id();
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            slice_index(plan, &input.anchored_to)?;
            plan.deferred.push(Deferred {
                id: id.clone(),
                label,
                anchored_to: input.anchored_to,
            });
            Ok(())
        })?;
        Ok(PlanAdded { id, plan })
    }

    pub fn remove_deferred(
        &self,
        project_id: &str,
        deferred_id: &str,
        expected: Option<u32>,
    ) -> Result<Plan, AppError> {
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            let index = plan
                .deferred
                .iter()
                .position(|d| d.id == deferred_id)
                .ok_or_else(|| AppError::not_found(format!("no deferred item {deferred_id}")))?;
            plan.deferred.remove(index);
            Ok(())
        })?;
        Ok(plan)
    }
}

/// The project rule a ruling becomes: one line naming both sides and which one wins.
fn rule_text(conflict: &RoadmapConflict) -> String {
    let source = match conflict.other_source.kind {
        ConflictSourceKind::DesignFamily => {
            format!(
                "the design family file `{}`",
                conflict.other_source.reference
            )
        }
        ConflictSourceKind::Bakery => {
            format!("the Bakery reference `{}`", conflict.other_source.reference)
        }
    };
    let topic = normalize_ws(&conflict.topic);
    let brain = normalize_ws(&conflict.brain_dump_excerpt);
    let other = normalize_ws(&conflict.other_excerpt);
    let Some(ruling) = &conflict.ruling else {
        return String::new();
    };
    let mut text = match ruling.kind {
        RulingKind::ReferenceWins => format!(
            "UI ruling ({topic}): follow {source} (\"{other}\"), not the brain dump \
             (\"{brain}\")."
        ),
        RulingKind::BrainDumpWins => format!(
            "UI ruling ({topic}): follow the brain dump (\"{brain}\"), not {source} \
             (\"{other}\")."
        ),
        RulingKind::Custom => format!(
            "UI ruling ({topic}): {}",
            normalize_ws(ruling.text.as_deref().unwrap_or(""))
        ),
    };
    if ruling.kind != RulingKind::Custom {
        if let Some(note) = &ruling.text {
            text.push(' ');
            text.push_str(&normalize_ws(note));
        }
    }
    text
}

/// `meta.title` / `meta.subtitle` for the mirror: the project's name and tagline from
/// `.foundry/project.json` (a file on disk; this feature doesn't import projects).
/// The fields of `.foundry/reviews/<sliceId>.json` the done gate needs. The verification
/// feature owns that file; the plan reads it from disk instead of importing the feature.
#[derive(Deserialize)]
struct StoredReview {
    passed: bool,
    /// The first report that was verified; the only one in a review saved before per-agent
    /// reports, which has no `reports`.
    report: serde_json::Value,
    /// Every report file that was verified.
    #[serde(default)]
    reports: Vec<StoredReport>,
}

#[derive(Deserialize)]
struct StoredReport {
    path: String,
    report: serde_json::Value,
}

/// `Ok(())` when the slice has a passed verification whose reports still equal the slice's
/// report files on disk (compared as JSON values, none added or gone); otherwise why not,
/// e.g. "has no verification".
fn verified(repo: &Path, slice_id: &str) -> Result<Result<(), &'static str>, AppError> {
    let Some(review) = repo_write::read_to_string(repo, &format!("{REVIEWS_DIR}/{slice_id}.json"))?
        .and_then(|text| serde_json::from_str(text.trim_start_matches('\u{feff}')).ok())
        .and_then(|value| serde_json::from_value::<StoredReview>(value).ok())
    else {
        return Ok(Err("has no verification"));
    };
    if !review.passed {
        return Ok(Err("failed its last verification"));
    }
    let verified = report::verified_snapshot(
        slice_id,
        review.reports.into_iter().map(|r| (r.path, r.report)),
        review.report,
    );
    if report::snapshot(repo, slice_id)? != verified {
        return Ok(Err(
            "has a stale verification (its report changed after it was verified)",
        ));
    }
    Ok(Ok(()))
}

fn project_meta(repo: &Path) -> (String, String) {
    let file: serde_json::Value = repo_write::read_to_string(repo, PROJECT_FILE)
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    let field = |key: &str| {
        file.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string()
    };
    let title = Some(field("name"))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Value Stream Map".into());
    (title, field("tagline"))
}

#[cfg(test)]
mod tests;
