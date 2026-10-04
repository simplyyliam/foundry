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

use crate::core::events::EventMeta;
use crate::core::git::{Git, GitRun};
use crate::core::repos::RepoIndex;
use crate::core::text::normalize_ws;
use crate::core::{ids, repo_write, AppError, ErrorCode};

pub use graph::Derived;

const PLAN_FILE: &str = ".foundry/plan.json";
const PROJECT_FILE: &str = ".foundry/project.json";
const BRAIN_DUMP: &str = "docs/brain-dump.md";
/// Written by roadmap generation (P3); read here, never written.
const ROADMAP_FILE: &str = ".foundry/roadmap.json";
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
    /// Requirement ids with no slice.
    pub unassigned: Vec<String>,
    /// Requirement ids still `proposed`.
    pub unconfirmed: Vec<String>,
    /// Queued slice ids whose dependencies are all done.
    pub ready: Vec<String>,
    /// Queued slice ids waiting on a dependency.
    pub blocked: Vec<String>,
    pub acyclic: bool,
    /// At least one slice, the graph is acyclic, and every requirement is assigned and
    /// confirmed.
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
    /// Start before the dependencies are done. Needs a `reason`, recorded as a decision.
    #[serde(rename = "override")]
    pub override_gate: bool,
    pub reason: Option<String>,
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
    }
}

pub fn summary(plan: &PlanFile) -> PlanSummary {
    let derived = graph::derive(&plan.slices);
    let view = view(plan);
    let unassigned: Vec<String> = plan
        .requirements
        .iter()
        .filter(|r| r.slice_id.is_none())
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
        if let Some(expected) = expected_revision {
            if expected != plan.revision {
                return Err(AppError::new(
                    ErrorCode::StaleRevision,
                    format!(
                        "the plan changed (revision {} now, {expected} expected): reload it",
                        plan.revision
                    ),
                ));
            }
        }
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

    /// `queued → in_progress` needs every dependency `done`, unless `override` is set with a
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
            if status == SliceStatus::InProgress && current != SliceStatus::InProgress {
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

    /// `slice_id: null` unassigns it (and so unconfirms it).
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
            }
            r.slice_id = slice_id;
            Ok(())
        })?;
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
            if state == RequirementState::Confirmed && r.slice_id.is_none() {
                return Err(AppError::new(
                    ErrorCode::RequirementUnassigned,
                    "assign this requirement to a slice before confirming it",
                ));
            }
            r.state = state;
            Ok(())
        })?;
        Ok(plan)
    }

    /// Confirms every assigned requirement. Unassigned ones stay proposed (see `unassigned`
    /// in `plan_summary`).
    pub fn confirm_all(&self, project_id: &str, expected: Option<u32>) -> Result<Plan, AppError> {
        let (_, plan) = self.mutate(project_id, expected, |plan, _| {
            for r in plan
                .requirements
                .iter_mut()
                .filter(|r| r.slice_id.is_some())
            {
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

/// `meta.title` / `meta.subtitle` for the mirror: the project's name and tagline from
/// `.foundry/project.json` (a file on disk; this feature doesn't import projects).
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
