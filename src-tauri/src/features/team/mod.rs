//! The Role Handbook (app-level, reusable roles), composing a project's team, and writing the
//! confirmed roster: `agents/roles.json` and `agents/roles/<agentId>.md`.
//!
//! Nothing an LLM proposes becomes a permission on disk without the human's confirm step:
//! `compose_team` only reads and proposes; `confirm_team` and `add_agent_manually` are the
//! only writers, and every agent they write carries the protected floor.

pub mod commands;
mod handbook;
mod import;
mod markdown;

use std::collections::{BTreeSet, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use specta::Type;

use crate::core::image;
use crate::core::llm::{LlmRunner, RunControl};
use crate::core::paths::{self, PathKind};
use crate::core::repos::RepoIndex;
use crate::core::slug::slugify;
use crate::core::{fsx, repo_write, AppError, ErrorCode};

/// Every confirmed agent is protected from writing these, whatever a proposal or an edit
/// says. Enforced here, not negotiable.
pub const PROTECTED_FLOOR: &[&str] = &[
    "AGENTS.md",
    "agents/**",
    "docs/design/**",
    ".foundry/plan.json",
    // Holds `testCommands`, which Verify runs with the human's rights.
    ".foundry/project.json",
    // What Foundry writes there and the plan trusts to mark a slice done. The rest of
    // `.foundry/reviews/` stays writable (e.g. a Manager role's review notes).
    ".foundry/reviews/*.json",
    ".foundry/reviews/digest.md",
    ".claude/**",
];

/// Built-in avatars, embedded: `(id, PNG bytes)`. Empty until real images are added to
/// `resources/avatars/` and listed here with `include_bytes!`.
#[cfg(not(test))]
const BUILTIN_AVATARS: &[(&str, &[u8])] = &[];
/// Tests only: one fixture, deliberately not a decodable image, to prove a built-in choice
/// is never processed. Never compiled into the app.
#[cfg(test)]
const BUILTIN_AVATARS: &[(&str, &[u8])] = &[("test-fixture", b"not an image")];

const HANDBOOK_DIR: &str = "role-handbook";
const HANDBOOK_FILE: &str = "index.json";
const ROLES_FILE: &str = "agents/roles.json";
const ROLE_DIR: &str = "agents/roles";
const AVATAR_DIR: &str = ".foundry/agents";
/// `agent_avatar` returns copies at most this many pixels on their long edge.
pub const AVATAR_THUMB_EDGE: u32 = 256;
const PLAN_FILE: &str = ".foundry/plan.json";
const PROMPT: &str = include_str!("prompt.md");

// ---------- types ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HandbookRole {
    pub id: String,
    pub title: String,
    pub description: String,
    pub typical_write_globs: Vec<String>,
    pub typical_protected_globs: Vec<String>,
    /// Who they are and what they provide: merged into the role file of agents made from it.
    pub skill_template: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewHandbookRole {
    pub title: String,
    pub description: String,
    pub typical_write_globs: Vec<String>,
    pub typical_protected_globs: Vec<String>,
    pub skill_template: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HandbookRolePatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub typical_write_globs: Option<Vec<String>>,
    pub typical_protected_globs: Option<Vec<String>>,
    pub skill_template: Option<String>,
}

/// What `role_handbook_add_from_paths` did: role ids.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RolesImported {
    pub added: Vec<String>,
    /// Existing roles that kept their id and took the file's fields (`replace: true`).
    pub replaced: Vec<String>,
}

/// What `compose_team` tells the model about each handbook role: never the skill body.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RoleSummary<'a> {
    id: &'a str,
    title: &'a str,
    description: &'a str,
    typical_write_globs: &'a [String],
}

/// A role that isn't in the handbook.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CustomRole {
    pub title: String,
    pub description: String,
    pub skill_template: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProposedAgent {
    pub temp_id: String,
    pub handbook_role_id: Option<String>,
    pub custom_role: Option<CustomRole>,
    pub display_name: String,
    pub title: String,
    pub rationale: String,
    pub related_slice_ids: Vec<String>,
}

/// What `compose_team` returns. Nothing is written until `confirm_team`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamProposal {
    pub agents: Vec<ProposedAgent>,
}

/// One agent as the human confirms it (also `add_agent_manually`'s input).
#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RosterAgent {
    /// Exactly one of `handbookRoleId` and `customRole`.
    pub handbook_role_id: Option<String>,
    pub custom_role: Option<CustomRole>,
    pub display_name: String,
    /// Defaults to the role's title.
    pub title: Option<String>,
    pub related_slice_ids: Vec<String>,
    /// A built-in avatar id (see `builtin_avatar_list`).
    pub avatar_builtin_id: Option<String>,
    /// Defaults to the handbook role's typical write globs (none for a custom role).
    pub write_globs: Option<Vec<String>>,
    /// Defaults to the handbook role's typical protected globs. The floor is always added.
    pub protected_globs: Option<Vec<String>>,
}

/// One entry in `agents/roles.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamAgent {
    pub id: String,
    pub display_name: String,
    pub title: String,
    /// `builtin:<id>`, a repo-relative path under `.foundry/agents/`, or null.
    pub avatar_path: Option<String>,
    pub reads: Vec<String>,
    pub write_globs: Vec<String>,
    pub protected_globs: Vec<String>,
    pub source_handbook_role_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Team {
    pub agents: Vec<TeamAgent>,
}

/// Protected globs that had to be put back on an agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FloorRestored {
    pub agent_id: String,
    pub globs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TeamWritten {
    pub team: Team,
    /// Ids of the agents this call created.
    pub created: Vec<String>,
    /// Where the protected floor was missing from what was asked for, and was restored.
    pub floor_restored: Vec<FloorRestored>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AvatarChoice {
    /// Exactly one of `builtinId` and `bytes`.
    pub builtin_id: Option<String>,
    pub bytes: Option<Vec<u8>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct HandbookFile {
    #[serde(default)]
    roles: Vec<HandbookRole>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct RolesFile {
    version: u32,
    agents: Vec<TeamAgent>,
}

/// A slice from `.foundry/plan.json`, as much as the team needs.
#[derive(Debug, Clone, Serialize)]
struct PlanSlice {
    id: String,
    name: String,
    delivers: Vec<String>,
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
        Err(AppError::invalid_input(format!("{what} can't be empty")))
    } else {
        Ok(text.to_string())
    }
}

/// What `role_handbook_add` checks and cleans, with the id it starts from (`slug(title)`,
/// or `role`). Making the id unique is the caller's job.
fn validated_role(role: NewHandbookRole) -> Result<HandbookRole, AppError> {
    let title = required("role title", &role.title)?;
    let id = Some(slugify(&title))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "role".into());
    Ok(HandbookRole {
        id,
        title,
        description: role.description.trim().to_string(),
        typical_write_globs: clean_globs(role.typical_write_globs),
        typical_protected_globs: clean_globs(role.typical_protected_globs),
        skill_template: role.skill_template.trim().to_string(),
    })
}

fn clean_globs(globs: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    globs
        .into_iter()
        .map(|g| g.trim().replace('\\', "/"))
        .filter(|g| !g.is_empty() && seen.insert(g.clone()))
        .collect()
}

/// `protected` with the floor guaranteed (floor first, then the rest in order), and which
/// floor globs were missing.
fn with_floor(protected: Vec<String>) -> (Vec<String>, Vec<String>) {
    let protected = clean_globs(protected);
    let missing: Vec<String> = PROTECTED_FLOOR
        .iter()
        .filter(|g| !protected.iter().any(|p| p == *g))
        .map(|g| g.to_string())
        .collect();
    let mut out: Vec<String> = PROTECTED_FLOOR.iter().map(|g| g.to_string()).collect();
    out.extend(
        protected
            .into_iter()
            .filter(|p| !PROTECTED_FLOOR.contains(&p.as_str())),
    );
    (out, missing)
}

/// What every agent reads (and its own role file).
fn reads_for(agent_id: &str) -> Vec<String> {
    vec![
        "AGENTS.md".into(),
        "agents/roles.json".into(),
        format!("{ROLE_DIR}/{agent_id}.md"),
        "vsm_data.js".into(),
        "docs/design/manifest.md".into(),
        "docs/bakery/manifest.md".into(),
    ]
}

fn unfence(text: &str) -> &str {
    let text = text.trim();
    let Some(start) = text.find("```") else {
        return text;
    };
    let after = &text[start + 3..];
    let body = &after[after.find('\n').map_or(after.len(), |i| i + 1)..];
    body.rfind("```").map_or(body, |end| &body[..end]).trim()
}

fn load_plan_slices(repo: &Path) -> Result<Vec<PlanSlice>, AppError> {
    let text = repo_write::read_to_string(repo, PLAN_FILE)?.ok_or_else(|| {
        AppError::invalid_input("this project has no plan yet: confirm the roadmap first")
    })?;
    let value: Value = serde_json::from_str(&text)
        .map_err(|err| AppError::invalid_input(format!("{PLAN_FILE} is unreadable: {err}")))?;
    let slices = value
        .get("slices")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::invalid_input(format!("{PLAN_FILE} has no slices list")))?;
    slices
        .iter()
        .map(|s| {
            let text = |key: &str| s.get(key).and_then(Value::as_str).map(str::to_string);
            Ok(PlanSlice {
                id: text("id").ok_or_else(|| AppError::invalid_input("a plan slice has no id"))?,
                name: text("name").unwrap_or_default(),
                delivers: s
                    .get("delivers")
                    .and_then(Value::as_array)
                    .map(|d| {
                        d.iter()
                            .filter_map(|x| x.as_str().map(str::to_string))
                            .collect()
                    })
                    .unwrap_or_default(),
            })
        })
        .collect()
}

// ---------- the store ----------

type RunnerSource = Box<dyn Fn() -> Result<Arc<dyn LlmRunner>, AppError> + Send + Sync>;
type TimeoutSource = Box<dyn Fn() -> std::time::Duration + Send + Sync>;

/// Owns the Role Handbook and every project's team. Share it as `Arc<TeamStore>`.
pub struct TeamStore {
    handbook_dir: PathBuf,
    repos: Arc<RepoIndex>,
    runner: RunnerSource,
    timeout: TimeoutSource,
    lock: Mutex<()>,
}

impl TeamStore {
    /// `runner` does the subscription preflight and returns the `claude -p` runner;
    /// `timeout` reads `generationTimeoutMs`.
    pub fn new(
        data_dir: &Path,
        repos: Arc<RepoIndex>,
        runner: impl Fn() -> Result<Arc<dyn LlmRunner>, AppError> + Send + Sync + 'static,
        timeout: impl Fn() -> std::time::Duration + Send + Sync + 'static,
    ) -> Self {
        Self {
            handbook_dir: data_dir.join(HANDBOOK_DIR),
            repos,
            runner: Box::new(runner),
            timeout: Box::new(timeout),
            lock: Mutex::new(()),
        }
    }

    // ---------- handbook ----------

    /// The handbook, seeded with the starter set the first time (a deleted starter role
    /// stays deleted).
    fn load_handbook(&self) -> Result<Vec<HandbookRole>, AppError> {
        match fs::read(self.handbook_dir.join(HANDBOOK_FILE)) {
            Ok(bytes) => serde_json::from_slice::<HandbookFile>(&bytes)
                .map(|f| f.roles)
                .map_err(|err| {
                    AppError::internal(format!("the Role Handbook is unreadable: {err}"))
                }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                let starter = handbook::starter();
                self.save_handbook(&starter)?;
                Ok(starter)
            }
            Err(err) => Err(err.into()),
        }
    }

    fn save_handbook(&self, roles: &[HandbookRole]) -> Result<(), AppError> {
        fs::create_dir_all(&self.handbook_dir)?;
        let json = serde_json::to_vec_pretty(&HandbookFile {
            roles: roles.to_vec(),
        })
        .map_err(|err| AppError::internal(format!("serialize the handbook: {err}")))?;
        fsx::write_atomic(&self.handbook_dir.join(HANDBOOK_FILE), json)
    }

    pub fn handbook(&self) -> Result<Vec<HandbookRole>, AppError> {
        let _guard = lock(&self.lock);
        self.load_handbook()
    }

    pub fn add_role(&self, role: NewHandbookRole) -> Result<Vec<HandbookRole>, AppError> {
        let mut role = validated_role(role)?;
        let _guard = lock(&self.lock);
        let mut roles = self.load_handbook()?;
        let base = role.id.clone();
        let mut n = 2;
        while roles.iter().any(|r| r.id == role.id) {
            role.id = format!("{base}-{n}");
            n += 1;
        }
        roles.push(role);
        self.save_handbook(&roles)?;
        Ok(roles)
    }

    /// One role per `.md` file (absolute paths), validated and given an id exactly as
    /// `add_role` does. All or nothing: any refused file, or (without `replace`) any id the
    /// handbook already has, changes nothing. With `replace`, an existing role keeps its id
    /// and takes the file's fields. The files are only read.
    pub fn add_roles_from_paths(
        &self,
        sources: &[String],
        replace: bool,
    ) -> Result<RolesImported, AppError> {
        if sources.is_empty() {
            return Err(AppError::invalid_input("no files to add"));
        }
        if sources.len() > import::MAX_FILES {
            return Err(AppError::invalid_input(format!(
                "at most {} files at a time, not {}",
                import::MAX_FILES,
                sources.len()
            )));
        }
        let mut incoming: Vec<(String, HandbookRole)> = Vec::new();
        for source in sources {
            let (name, role) = import::read_role(source)?;
            let role = validated_role(role)
                .map_err(|err| AppError::new(err.code, format!("{name}: {}", err.message)))?;
            if let Some((other, _)) = incoming.iter().find(|(_, r)| r.id == role.id) {
                return Err(AppError::invalid_input(format!(
                    "{other} and {name} would both be the role \"{}\"",
                    role.id
                )));
            }
            incoming.push((name, role));
        }

        let _guard = lock(&self.lock);
        let mut roles = self.load_handbook()?;
        let existing: Vec<&str> = roles
            .iter()
            .filter(|r| incoming.iter().any(|(_, new)| new.id == r.id))
            .map(|r| r.title.as_str())
            .collect();
        if !existing.is_empty() && !replace {
            return Err(AppError::new(
                ErrorCode::RoleExists,
                format!(
                    "the Role Handbook already has {}: replace them, or change the files' titles",
                    existing.join(", ")
                ),
            ));
        }
        let mut imported = RolesImported {
            added: Vec::new(),
            replaced: Vec::new(),
        };
        for (_, role) in incoming {
            match roles.iter_mut().find(|r| r.id == role.id) {
                Some(slot) => {
                    imported.replaced.push(role.id.clone());
                    *slot = role;
                }
                None => {
                    imported.added.push(role.id.clone());
                    roles.push(role);
                }
            }
        }
        self.save_handbook(&roles)?;
        tracing::info!(
            added = imported.added.len(),
            replaced = imported.replaced.len(),
            "handbook roles imported"
        );
        Ok(imported)
    }

    pub fn update_role(
        &self,
        id: &str,
        patch: HandbookRolePatch,
    ) -> Result<Vec<HandbookRole>, AppError> {
        let title = patch
            .title
            .as_deref()
            .map(|t| required("role title", t))
            .transpose()?;
        let _guard = lock(&self.lock);
        let mut roles = self.load_handbook()?;
        let role = roles
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or_else(|| AppError::not_found(format!("no handbook role {id}")))?;
        if let Some(title) = title {
            role.title = title;
        }
        if let Some(description) = patch.description {
            role.description = description.trim().to_string();
        }
        if let Some(globs) = patch.typical_write_globs {
            role.typical_write_globs = clean_globs(globs);
        }
        if let Some(globs) = patch.typical_protected_globs {
            role.typical_protected_globs = clean_globs(globs);
        }
        if let Some(skill) = patch.skill_template {
            role.skill_template = skill.trim().to_string();
        }
        self.save_handbook(&roles)?;
        Ok(roles)
    }

    pub fn remove_role(&self, id: &str) -> Result<Vec<HandbookRole>, AppError> {
        let _guard = lock(&self.lock);
        let mut roles = self.load_handbook()?;
        let at = roles
            .iter()
            .position(|r| r.id == id)
            .ok_or_else(|| AppError::not_found(format!("no handbook role {id}")))?;
        roles.remove(at);
        self.save_handbook(&roles)?;
        Ok(roles)
    }

    // ---------- the team on disk ----------

    fn load_team(repo: &Path) -> Result<Option<RolesFile>, AppError> {
        repo_write::read_to_string(repo, ROLES_FILE)?
            .map(|text| {
                serde_json::from_str(&text)
                    .map_err(|err| AppError::internal(format!("{ROLES_FILE} is unreadable: {err}")))
            })
            .transpose()
    }

    pub fn team(&self, project_id: &str) -> Result<Option<Team>, AppError> {
        let repo = self.repos.repo(project_id)?;
        let _guard = lock(&self.lock);
        Ok(Self::load_team(&repo)?.map(|file| Team {
            agents: file.agents,
        }))
    }

    // ---------- compose (read-only) ----------

    /// Asks the Product Lead (`claude -p`) for a roster, from the plan and the handbook.
    /// Validated (one retry with the errors), and never written anywhere: the human edits
    /// it, then `confirm_team` writes it.
    pub fn compose(&self, project_id: &str) -> Result<TeamProposal, AppError> {
        let repo = self.repos.repo(project_id)?;
        let (slices, roles) = {
            let _guard = lock(&self.lock);
            if Self::load_team(&repo)?.is_some() {
                return Err(team_confirmed());
            }
            // Read-only from here: the handbook must already exist to be proposed from.
            (load_plan_slices(&repo)?, self.read_handbook_only()?)
        };
        let runner = (self.runner)()?;
        let control = RunControl::new((self.timeout)());
        let plan_json = serde_json::to_string_pretty(&slices)
            .map_err(|err| AppError::internal(format!("serialize the plan: {err}")))?;
        let summaries: Vec<RoleSummary> = roles
            .iter()
            .map(|r| RoleSummary {
                id: &r.id,
                title: &r.title,
                description: &r.description,
                typical_write_globs: &r.typical_write_globs,
            })
            .collect();
        let handbook_json = serde_json::to_string_pretty(&summaries)
            .map_err(|err| AppError::internal(format!("serialize the handbook: {err}")))?;
        let prompt = PROMPT
            .replace("\r\n", "\n")
            .replacen("{{PLAN}}", &plan_json, 1)
            .replacen("{{HANDBOOK}}", &handbook_json, 1);

        let first = runner.run(&prompt, &control)?;
        match check_proposal(&first, &roles, &slices) {
            Ok(proposal) => Ok(proposal),
            Err(errors) => {
                let retry = format!(
                    "{prompt}\n\nYour previous reply was rejected for these reasons:\n- {}\n\n\
                     Reply again with the corrected JSON object only.",
                    errors.join("\n- ")
                );
                let second = runner.run(&retry, &control)?;
                check_proposal(&second, &roles, &slices).map_err(|errors| {
                    AppError::new(
                        ErrorCode::GenerationFailed,
                        format!(
                            "the team proposal failed validation twice:\n- {}",
                            errors.join("\n- ")
                        ),
                    )
                })
            }
        }
    }

    /// The handbook without seeding it (compose must not write, even the starter set).
    fn read_handbook_only(&self) -> Result<Vec<HandbookRole>, AppError> {
        match fs::read(self.handbook_dir.join(HANDBOOK_FILE)) {
            Ok(bytes) => serde_json::from_slice::<HandbookFile>(&bytes)
                .map(|f| f.roles)
                .map_err(|err| {
                    AppError::internal(format!("the Role Handbook is unreadable: {err}"))
                }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(handbook::starter()),
            Err(err) => Err(err.into()),
        }
    }

    // ---------- confirm / add (the only writers) ----------

    /// Writes the confirmed roster. `team_confirmed` if the project already has a team.
    pub fn confirm(
        &self,
        project_id: &str,
        roster: Vec<RosterAgent>,
    ) -> Result<TeamWritten, AppError> {
        if roster.is_empty() {
            return Err(AppError::invalid_input("a team needs at least one agent"));
        }
        let repo = self.repos.repo(project_id)?;
        let _guard = lock(&self.lock);
        if Self::load_team(&repo)?.is_some() {
            return Err(team_confirmed());
        }
        let roles = self.load_handbook()?;
        let slices = load_plan_slices(&repo).unwrap_or_default();
        let mut taken: BTreeSet<String> = BTreeSet::new();
        let mut built = Vec::new();
        for agent in roster {
            built.push(build_agent(agent, &roles, &slices, &mut taken)?);
        }
        self.write_team(&repo, Vec::new(), built)
    }

    /// Adds one agent to a confirmed team; every existing agent's entry and file is left as
    /// it was. `team_not_confirmed` without a team.
    pub fn add_manually(
        &self,
        project_id: &str,
        agent: RosterAgent,
    ) -> Result<TeamWritten, AppError> {
        let repo = self.repos.repo(project_id)?;
        let _guard = lock(&self.lock);
        let existing = Self::load_team(&repo)?.ok_or_else(|| {
            AppError::new(
                ErrorCode::TeamNotConfirmed,
                "confirm the team before adding to it",
            )
        })?;
        let roles = self.load_handbook()?;
        let slices = load_plan_slices(&repo).unwrap_or_default();
        let mut taken: BTreeSet<String> = existing.agents.iter().map(|a| a.id.clone()).collect();
        let built = build_agent(agent, &roles, &slices, &mut taken)?;
        if repo
            .join(ROLE_DIR)
            .join(format!("{}.md", built.entry.id))
            .exists()
        {
            return Err(AppError::invalid_input(format!(
                "{ROLE_DIR}/{}.md already exists; pick another display name",
                built.entry.id
            )));
        }
        self.write_team(&repo, existing.agents, vec![built])
    }

    /// Writes the new agents' role files, then `roles.json` (existing entries unchanged).
    fn write_team(
        &self,
        repo: &Path,
        existing: Vec<TeamAgent>,
        new: Vec<BuiltAgent>,
    ) -> Result<TeamWritten, AppError> {
        let mut written: Vec<String> = Vec::new();
        for agent in &new {
            let rel = format!("{ROLE_DIR}/{}.md", agent.entry.id);
            if let Err(err) = repo_write::write(repo, &rel, &agent.role_file) {
                for path in &written {
                    let _ = repo_write::remove(repo, path);
                }
                return Err(err);
            }
            written.push(rel);
        }
        let created: Vec<String> = new.iter().map(|a| a.entry.id.clone()).collect();
        let floor_restored: Vec<FloorRestored> = new
            .iter()
            .filter(|a| !a.floor_restored.is_empty())
            .map(|a| FloorRestored {
                agent_id: a.entry.id.clone(),
                globs: a.floor_restored.clone(),
            })
            .collect();
        let mut agents = existing;
        agents.extend(new.into_iter().map(|a| a.entry));
        agents.sort_by(|a, b| a.id.cmp(&b.id));
        if let Err(err) = save_roles(repo, &agents) {
            for path in &written {
                let _ = repo_write::remove(repo, path);
            }
            return Err(err);
        }
        for restored in &floor_restored {
            tracing::warn!(agent_id = %restored.agent_id, globs = ?restored.globs, "protected floor restored");
        }
        tracing::info!(created = created.len(), "team written");
        Ok(TeamWritten {
            team: Team { agents },
            created,
            floor_restored,
        })
    }

    // ---------- avatars ----------

    pub fn builtin_avatars() -> Vec<String> {
        BUILTIN_AVATARS
            .iter()
            .map(|(id, _)| id.to_string())
            .collect()
    }

    /// A built-in choice (no processing) or an uploaded image, processed exactly like a
    /// Bakery image and stored at `.foundry/agents/<agentId>-avatar.<ext>`.
    pub fn set_avatar(
        &self,
        project_id: &str,
        agent_id: &str,
        choice: AvatarChoice,
    ) -> Result<TeamAgent, AppError> {
        let repo = self.repos.repo(project_id)?;
        let _guard = lock(&self.lock);
        let mut file = Self::load_team(&repo)?.ok_or_else(|| {
            AppError::new(
                ErrorCode::TeamNotConfirmed,
                "this project has no confirmed team",
            )
        })?;
        let at = file
            .agents
            .iter()
            .position(|a| a.id == agent_id)
            .ok_or_else(|| AppError::not_found(format!("no agent {agent_id}")))?;
        let path = match (choice.builtin_id, choice.bytes) {
            (Some(id), None) => {
                if !BUILTIN_AVATARS.iter().any(|(b, _)| *b == id) {
                    return Err(AppError::not_found(format!("no built-in avatar {id}")));
                }
                format!("builtin:{id}")
            }
            (None, Some(bytes)) => {
                let processed = image::process(&bytes)?;
                let rel = format!("{AVATAR_DIR}/{agent_id}-avatar.{}", processed.ext);
                repo_write::write(&repo, &rel, &processed.bytes)?;
                for other in ["png", "jpg"].iter().filter(|e| **e != processed.ext) {
                    repo_write::remove(&repo, &format!("{AVATAR_DIR}/{agent_id}-avatar.{other}"))?;
                }
                rel
            }
            _ => {
                return Err(AppError::invalid_input(
                    "choose exactly one of builtinId and bytes",
                ))
            }
        };
        file.agents[at].avatar_path = Some(path);
        save_roles(&repo, &file.agents)?;
        Ok(file.agents[at].clone())
    }

    /// The agent's avatar as a data URL, at most [`AVATAR_THUMB_EDGE`] px on its long edge, or
    /// null when it has none, its built-in id isn't embedded, or its file is missing. An
    /// uploaded file must sit under `.foundry/agents/` (checked through `core::paths`, since
    /// `roles.json` can be hand-edited). Read-only: the stored file is never touched.
    pub fn avatar(&self, project_id: &str, agent_id: &str) -> Result<Option<String>, AppError> {
        let repo = self.repos.repo(project_id)?;
        let avatar_path = {
            let _guard = lock(&self.lock);
            let file = Self::load_team(&repo)?.ok_or_else(|| {
                AppError::new(
                    ErrorCode::TeamNotConfirmed,
                    "this project has no confirmed team",
                )
            })?;
            file.agents
                .into_iter()
                .find(|a| a.id == agent_id)
                .ok_or_else(|| AppError::not_found(format!("no agent {agent_id}")))?
                .avatar_path
        };
        let Some(avatar_path) = avatar_path else {
            return Ok(None);
        };

        if let Some(id) = avatar_path.strip_prefix("builtin:") {
            // Built-ins are shipped already sized, so they're served as embedded.
            return Ok(BUILTIN_AVATARS
                .iter()
                .find(|(b, _)| *b == id)
                .map(|(_, bytes)| data_url("image/png", bytes)));
        }

        let rel = paths::check(Path::new(&avatar_path), PathKind::Relative)?;
        let inside = rel
            .strip_prefix(AVATAR_DIR)
            .ok()
            .filter(|rest| rest.components().next().is_some())
            .ok_or_else(|| {
                AppError::path_escape(format!("avatar path is outside {AVATAR_DIR}/"))
            })?;
        let dir = paths::resolve(&repo, AVATAR_DIR)?;
        if !dir.is_dir() {
            return Ok(None);
        }
        let file = paths::resolve(&dir, inside)?;
        let bytes = match fs::read(&file) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(err.into()),
        };
        let thumb = image::downscale(&bytes, AVATAR_THUMB_EDGE)?;
        let mime = if thumb.ext == "jpg" {
            "image/jpeg"
        } else {
            "image/png"
        };
        Ok(Some(data_url(mime, &thumb.bytes)))
    }
}

fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", STANDARD.encode(bytes))
}

fn team_confirmed() -> AppError {
    AppError::new(
        ErrorCode::TeamConfirmed,
        "this project already has a confirmed team: add agents with add_agent_manually \
         (re-proposing a confirmed team isn't supported)",
    )
}

fn save_roles(repo: &Path, agents: &[TeamAgent]) -> Result<(), AppError> {
    let json = serde_json::to_string_pretty(&RolesFile {
        version: 1,
        agents: agents.to_vec(),
    })
    .map_err(|err| AppError::internal(format!("serialize roles.json: {err}")))?;
    repo_write::write(repo, ROLES_FILE, json + "\n")
}

struct BuiltAgent {
    entry: TeamAgent,
    role_file: String,
    floor_restored: Vec<String>,
}

/// Validates one roster agent and builds its `roles.json` entry and role file.
fn build_agent(
    agent: RosterAgent,
    roles: &[HandbookRole],
    slices: &[PlanSlice],
    taken: &mut BTreeSet<String>,
) -> Result<BuiltAgent, AppError> {
    let display_name = required("displayName", &agent.display_name)?;
    let (source, custom) = match (&agent.handbook_role_id, &agent.custom_role) {
        (Some(id), None) => {
            let role = roles
                .iter()
                .find(|r| &r.id == id)
                .ok_or_else(|| AppError::not_found(format!("no handbook role {id}")))?;
            (Some(role), None)
        }
        (None, Some(custom)) => {
            required("customRole.title", &custom.title)?;
            required("customRole.skillTemplate", &custom.skill_template)?;
            (None, Some(custom))
        }
        _ => {
            return Err(AppError::invalid_input(format!(
                "{display_name}: choose exactly one of handbookRoleId and customRole"
            )))
        }
    };
    let related: Vec<&PlanSlice> = agent
        .related_slice_ids
        .iter()
        .map(|id| {
            slices
                .iter()
                .find(|s| &s.id == id)
                .ok_or_else(|| AppError::not_found(format!("no plan slice {id}")))
        })
        .collect::<Result<_, _>>()?;

    let base = Some(slugify(&display_name))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "agent".into());
    let mut id = base.clone();
    let mut n = 2;
    while taken.contains(&id) {
        id = format!("{base}-{n}");
        n += 1;
    }
    taken.insert(id.clone());

    let role_title = source.map_or_else(
        || {
            custom
                .map(|c| c.title.trim().to_string())
                .unwrap_or_default()
        },
        |r| r.title.clone(),
    );
    let title = agent
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map_or(role_title, str::to_string);
    let skill = source.map_or_else(
        || {
            custom
                .map(|c| c.skill_template.trim().to_string())
                .unwrap_or_default()
        },
        |r| r.skill_template.clone(),
    );
    let write_globs = clean_globs(agent.write_globs.unwrap_or_else(|| {
        source
            .map(|r| r.typical_write_globs.clone())
            .unwrap_or_default()
    }));
    let asked_protected = agent.protected_globs.unwrap_or_else(|| {
        source
            .map(|r| r.typical_protected_globs.clone())
            .unwrap_or_default()
    });
    let (protected_globs, floor_restored) = with_floor(asked_protected);
    let avatar_path = match agent.avatar_builtin_id {
        Some(builtin) => {
            if !BUILTIN_AVATARS.iter().any(|(b, _)| *b == builtin) {
                return Err(AppError::not_found(format!("no built-in avatar {builtin}")));
            }
            Some(format!("builtin:{builtin}"))
        }
        None => None,
    };
    let entry = TeamAgent {
        reads: reads_for(&id),
        id,
        display_name,
        title,
        avatar_path,
        write_globs,
        protected_globs,
        source_handbook_role_id: source.map(|r| r.id.clone()),
    };
    let role_file = role_file(&entry, &markdown::demote_headings(&skill), &related);
    Ok(BuiltAgent {
        entry,
        role_file,
        floor_restored,
    })
}

/// `agents/roles/<id>.md`: the same five headings for every role. Deterministic. `skill`
/// must already have its headings demoted below them.
fn role_file(agent: &TeamAgent, skill: &str, slices: &[&PlanSlice]) -> String {
    let bullets = |items: &[String]| {
        items
            .iter()
            .map(|i| format!("- `{i}`"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let slice_lines = if slices.is_empty() {
        "- No slices assigned yet: ask the human which slice to take.".to_string()
    } else {
        slices
            .iter()
            .map(|s| {
                let delivers = if s.delivers.is_empty() {
                    String::new()
                } else {
                    format!(": delivers {}", s.delivers.join("; "))
                };
                format!("- {} (`{}`){delivers}", s.name, s.id)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    let writes = if agent.write_globs.is_empty() {
        "Nothing: this role advises and coordinates; it doesn't edit files.".to_string()
    } else {
        format!(
            "Only files matching these patterns:\n\n{}",
            bullets(&agent.write_globs)
        )
    };
    format!(
        "# {name}: {title}\n\n\
         <!-- Generated by Foundry when the team was confirmed. Edit the team in Foundry. -->\n\n\
         {skill}\n\n\
         Your slices:\n\n{slice_lines}\n\n\
         ## Reads\n\n\
         {reads}\n\n\
         Read `AGENTS.md` first. The design and Bakery manifests matter only for UI-touching \
         slices (AGENTS.md, Section 6).\n\n\
         ## Writes\n\n\
         {writes}\n\n\
         Never these, whatever else says so (protected):\n\n{protected}\n\n\
         ## Hands off\n\n\
         When one of your slices is done, hand it to the human with what changed, which of its \
         requirements you checked off (AGENTS.md, Law 9), and what is still open. The rest of \
         the team is in `agents/roles.json`: work that belongs to another role goes to them \
         through the human, never by editing their files.\n\n\
         ## Done means\n\n\
         Every requirement assigned to your slices is checked off individually, their tests \
         pass, physical interactions are verified in a real browser where they apply, and \
         nothing was written outside your Writes list.\n\n\
         ## When blocked\n\n\
         Stop rather than guess or work around it (AGENTS.md, Section 5). Record the blocker \
         as an open question on the slice and tell the human what you need. Never edit a \
         protected file to get unblocked.\n",
        name = agent.display_name,
        title = agent.title,
        reads = bullets(&agent.reads),
        protected = bullets(&agent.protected_globs),
    )
}

/// Parses and checks a proposal: JSON (code fences tolerated), every field, known handbook
/// roles and plan slices, unique temp ids.
fn check_proposal(
    output: &str,
    roles: &[HandbookRole],
    slices: &[PlanSlice],
) -> Result<TeamProposal, Vec<String>> {
    let proposal: TeamProposal = serde_json::from_str(unfence(output))
        .map_err(|err| vec![format!("the reply is not the requested JSON: {err}")])?;
    let mut errors = Vec::new();
    if proposal.agents.is_empty() {
        errors.push("agents must not be empty".into());
    }
    let mut temp_ids = HashSet::new();
    for (i, agent) in proposal.agents.iter().enumerate() {
        let at = format!("agents[{i}]");
        if agent.temp_id.trim().is_empty() || !temp_ids.insert(agent.temp_id.clone()) {
            errors.push(format!("{at}.tempId must be present and unique"));
        }
        if agent.display_name.trim().is_empty() {
            errors.push(format!("{at}.displayName must not be empty"));
        }
        if agent.title.trim().is_empty() {
            errors.push(format!("{at}.title must not be empty"));
        }
        match (&agent.handbook_role_id, &agent.custom_role) {
            (Some(id), None) => {
                if !roles.iter().any(|r| &r.id == id) {
                    errors.push(format!(
                        "{at}.handbookRoleId \"{id}\" is not in the Role Handbook"
                    ));
                }
            }
            (None, Some(custom)) => {
                if custom.title.trim().is_empty() || custom.skill_template.trim().is_empty() {
                    errors.push(format!("{at}.customRole needs a title and a skillTemplate"));
                }
            }
            _ => errors.push(format!(
                "{at} must have exactly one of handbookRoleId and customRole"
            )),
        }
        for id in &agent.related_slice_ids {
            if !slices.iter().any(|s| &s.id == id) {
                errors.push(format!(
                    "{at}.relatedSliceIds: \"{id}\" is not a plan slice"
                ));
            }
        }
    }
    if errors.is_empty() {
        Ok(proposal)
    } else {
        Err(errors)
    }
}

#[cfg(test)]
mod tests;
