//! Hand-off and task prompts, assembled by code from what's on disk, never by a model: the
//! same inputs always give the same bytes, so no requirement can be dropped between the
//! plan and the agent.
//!
//! Reads `.foundry/plan.json`, `.foundry/roadmap.json`, `agents/roles.json`,
//! `.foundry/agents-placeholders.json`, `.foundry/bakery.json` and `AGENTS.md` (see `disk`;
//! no other feature is imported). Writes only `.foundry/prompts/`, through `repo_write`.
//! Never writes `plan.json`: the plan feature is its only writer.

pub mod commands;
mod disk;
mod fix;
mod render;

use std::collections::BTreeMap;
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::core::paths::{self, PathKind};
use crate::core::repos::RepoIndex;
use crate::core::{launch_message, repo_write, report, AppError, ErrorCode};

pub use fix::FixPrompt;
pub use render::FORCED_PREFIX;

/// `launch_message` returns at most this many characters (built in `core::launch_message`).
pub use crate::core::launch_message::{MAX_LAUNCH_CHARS, PROMPTS_DIR};
const HANDOFF_FILE: &str = ".foundry/prompts/handoff.md";
/// `read_prompt` refuses files over this size (200 KB).
pub const MAX_PROMPT_BYTES: u64 = 200 * 1000;

/// AGENTS.md placeholder keys (the design feature's slugs of the template's labels).
mod key {
    pub const ARCHITECTURE: &str = "architecture";
    pub const FRONTEND: &str = "frontend-engine";
    pub const BACKEND: &str = "backend-engine";
    pub const DATABASE: &str = "database-layer";
    pub const DESIGN_FAMILY: &str = "designFamily";
}

// ---------- types ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HandoffResult {
    /// Repo-relative: `.foundry/prompts/handoff.md`.
    pub path: String,
    /// Not refusals, e.g. unlabeled Bakery images.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct TaskPromptOptions {
    /// Generate even though some dependencies aren't done.
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskPromptResult {
    /// Repo-relative: `.foundry/prompts/<sliceId>-<agentId>.md`.
    pub path: String,
    /// Names of the dependencies that weren't done (only ever non-empty with `force`).
    pub unfinished_dependencies: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum PromptKind {
    Handoff,
    Task,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PromptFile {
    /// Repo-relative, e.g. `.foundry/prompts/handoff.md`. What `launch_message` takes.
    pub path: String,
    pub kind: PromptKind,
    /// Task prompts only.
    pub slice_id: Option<String>,
    /// Task prompts only.
    pub agent_id: Option<String>,
    // u32 stands in for the exported type only (TS `number`), as with EventMeta.ts.
    #[specta(type = u32)]
    pub size: u64,
    /// Epoch milliseconds.
    #[specta(type = u32)]
    pub modified_ms: u64,
}

// ---------- helpers ----------

fn task_file(slice_id: &str, agent_id: &str) -> String {
    format!("{PROMPTS_DIR}/{slice_id}-{agent_id}.md")
}

/// Ids become file names: one plain component, no separators or control characters.
fn check_file_id(what: &str, id: &str) -> Result<(), AppError> {
    let plain = !id.is_empty()
        && !id.starts_with('.')
        && !id.contains(['/', '\\', ':'])
        && !id.chars().any(char::is_control)
        && paths::check(Path::new(id), PathKind::Relative)
            .is_ok_and(|p| p.components().count() == 1);
    if plain {
        Ok(())
    } else {
        Err(AppError::invalid_input(format!(
            "{what} \"{id}\" can't be used in a file name"
        )))
    }
}

/// Names that go into a prompt's first line or a launch message: one line, no control chars.
fn check_one_line(what: &str, text: &str) -> Result<(), AppError> {
    if text.chars().any(char::is_control) {
        return Err(AppError::invalid_input(format!(
            "the agent's {what} must be one line with no control characters"
        )));
    }
    Ok(())
}

fn check_agent(agent: &disk::Agent) -> Result<(), AppError> {
    check_file_id("agent id", &agent.id)?;
    check_one_line("display name", &agent.display_name)?;
    check_one_line("title", &agent.title)
}

fn require_confirmed_plan(plan: &disk::Plan) -> Result<(), AppError> {
    if plan.is_confirmed() {
        Ok(())
    } else {
        Err(AppError::new(
            ErrorCode::PlanNotConfirmed,
            "confirm the plan first: every requirement assigned (or project-wide) and \
             confirmed, no dependency cycle",
        ))
    }
}

fn require_agents_md(repo: &Path) -> Result<(), AppError> {
    match repo_write::read_to_string(repo, disk::AGENTS_FILE)? {
        None => Err(AppError::new(
            ErrorCode::AgentsMdNotReady,
            "generate AGENTS.md first",
        )),
        Some(text) if text.contains(render::MARKER) => Err(AppError::new(
            ErrorCode::AgentsMdNotReady,
            "AGENTS.md still has a [[ FILL IN marker: fill it in first",
        )),
        Some(_) => Ok(()),
    }
}

fn require_team(repo: &Path) -> Result<Vec<disk::Agent>, AppError> {
    disk::team(repo)?.ok_or_else(|| {
        AppError::new(
            ErrorCode::TeamNotConfirmed,
            "this project has no confirmed team",
        )
    })
}

fn find_agent(team: Vec<disk::Agent>, agent_id: &str) -> Result<disk::Agent, AppError> {
    let agent = team
        .into_iter()
        .find(|a| a.id == agent_id)
        .ok_or_else(|| AppError::not_found(format!("no agent {agent_id}")))?;
    check_agent(&agent)?;
    Ok(agent)
}

fn bakery_warnings(repo: &Path) -> Result<Vec<String>, AppError> {
    let unlabeled = disk::unlabeled_bakery_images(repo)?;
    if unlabeled.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![format!(
        "{} Bakery image(s) have no label or no Take note, so agents will ignore them: {}",
        unlabeled.len(),
        unlabeled.join(", ")
    )])
}

fn dependencies_not_done(names: &[String]) -> AppError {
    AppError::new(
        ErrorCode::DependenciesNotDone,
        format!(
            "these dependencies aren't done yet: {} (generate with force to start anyway)",
            names.join(", ")
        ),
    )
}

/// The slice a task prompt file is for, by matching `<sliceId>-<agentId>.md` against the
/// plan's slice ids (both may contain `-`). The longest matching slice id wins.
fn parse_task_name<'a>(plan: &'a disk::Plan, name: &'a str) -> Option<(&'a str, &'a str)> {
    let stem = name.strip_suffix(".md")?;
    plan.slices
        .iter()
        .filter_map(|s| {
            let agent = stem.strip_prefix(s.id.as_str())?.strip_prefix('-')?;
            (!agent.is_empty()).then_some((s.id.as_str(), agent))
        })
        .max_by_key(|(slice, _)| slice.len())
}

fn epoch_ms(time: std::io::Result<std::time::SystemTime>) -> u64 {
    time.ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

// ---------- the store ----------

/// Share it as `Arc<PromptsStore>`.
pub struct PromptsStore {
    repos: Arc<RepoIndex>,
}

impl PromptsStore {
    pub fn new(repos: Arc<RepoIndex>) -> Self {
        Self { repos }
    }

    /// Fills the hand-off template into `.foundry/prompts/handoff.md`. Gates: the plan is
    /// confirmed, AGENTS.md is ready, every placeholder it uses is confirmed, and the
    /// roadmap has a goal.
    pub fn generate_handoff(&self, project_id: &str) -> Result<HandoffResult, AppError> {
        let repo = self.repos.repo(project_id)?;
        let plan = disk::plan(&repo)?;
        require_confirmed_plan(&plan)?;
        require_agents_md(&repo)?;

        let goal = disk::roadmap_goal(&repo)?.ok_or_else(|| {
            AppError::not_found("the roadmap has no goal: generate a roadmap first")
        })?;
        let values = disk::placeholders(&repo)?;
        let mut missing = Vec::new();
        let mut value = |key: &str, label: &str| match values.get(key) {
            Some(v) if !v.trim().is_empty() => v.trim().to_string(),
            _ => {
                missing.push(label.to_string());
                String::new()
            }
        };
        use render::field;
        let mut scalars: BTreeMap<&str, String> = BTreeMap::new();
        scalars.insert(field::APPLICATION_PROFILE, goal);
        scalars.insert(field::WORKSPACE, value(key::ARCHITECTURE, "Architecture"));
        scalars.insert(field::FRONTEND, value(key::FRONTEND, "Frontend Engine"));
        scalars.insert(field::BACKEND, value(key::BACKEND, "Backend Engine"));
        scalars.insert(field::PERSISTENCE, value(key::DATABASE, "Database Layer"));
        if !missing.is_empty() {
            return Err(AppError::new(
                ErrorCode::PlaceholdersUnconfirmed,
                format!(
                    "confirm these AGENTS.md values first: {}",
                    missing.join(", ")
                ),
            ));
        }
        // Optional: an empty family is a confirmed "none" (AGENTS.md section 6 then asks).
        let family = values
            .get(key::DESIGN_FAMILY)
            .map(|v| v.trim())
            .filter(|v| !v.is_empty())
            .unwrap_or("none");
        scalars.insert(field::DESIGN_FAMILY, family.to_string());

        let mut lists: BTreeMap<&str, Vec<String>> = BTreeMap::new();
        let mut requirements = render::handoff_requirements(&plan);
        requirements.extend(render::handoff_project_rules(&plan));
        lists.insert(field::REQUIREMENTS, requirements);
        lists.insert(field::HORIZON, render::handoff_horizon(&plan));

        let text = render::handoff(&scalars, &lists)?;
        let warnings = bakery_warnings(&repo)?;
        repo_write::write(&repo, HANDOFF_FILE, text)?;
        tracing::info!(project_id, "hand-off prompt generated");
        Ok(HandoffResult {
            path: HANDOFF_FILE.into(),
            warnings,
        })
    }

    /// Writes `.foundry/prompts/<sliceId>-<agentId>.md`. Gates: the plan is confirmed,
    /// AGENTS.md is ready, a team is confirmed, the slice and agent exist, and the slice's
    /// dependencies are done (unless `force`, which names them in the result and the prompt).
    pub fn generate_task_prompt(
        &self,
        project_id: &str,
        slice_id: &str,
        agent_id: &str,
        options: TaskPromptOptions,
    ) -> Result<TaskPromptResult, AppError> {
        let repo = self.repos.repo(project_id)?;
        let plan = disk::plan(&repo)?;
        require_confirmed_plan(&plan)?;
        require_agents_md(&repo)?;
        let team = require_team(&repo)?;
        let slice = plan
            .slice(slice_id)
            .ok_or_else(|| AppError::not_found(format!("no slice with id {slice_id}")))?;
        check_file_id("slice id", slice_id)?;
        let agent = find_agent(team, agent_id)?;

        let unfinished: Vec<String> = plan
            .unfinished_dependencies(slice)
            .iter()
            .map(|s| s.name.clone())
            .collect();
        if !unfinished.is_empty() && !options.force {
            return Err(dependencies_not_done(&unfinished));
        }

        let design_pointers = disk::is_file(&repo, disk::BAKERY_MANIFEST)?
            || disk::is_file(&repo, disk::DESIGN_MANIFEST)?;
        let text = render::task_prompt(&render::TaskInputs {
            plan: &plan,
            slice,
            agent: &agent,
            forced: &unfinished,
            design_pointers,
        })?;
        let warnings = bakery_warnings(&repo)?;
        let path = task_file(slice_id, agent_id);
        repo_write::write(&repo, &path, text)?;
        tracing::info!(
            project_id,
            slice_id,
            agent_id,
            forced = !unfinished.is_empty(),
            "task prompt generated"
        );
        Ok(TaskPromptResult {
            path,
            unfinished_dependencies: unfinished,
            warnings,
        })
    }

    /// One fix prompt per agent for the bugs of the slice's latest QA run (see `fix::build`
    /// for the routing). Gates: a team is confirmed and the slice exists. A report that isn't a
    /// slice report → `invalid_json`. Read-only: writes nothing, sends nothing.
    pub fn generate_fix_prompts(
        &self,
        project_id: &str,
        slice_id: &str,
    ) -> Result<Vec<FixPrompt>, AppError> {
        let repo = self.repos.repo(project_id)?;
        let plan = disk::plan(&repo)?;
        check_file_id("slice id", slice_id)?;
        let slice = plan
            .slice(slice_id)
            .ok_or_else(|| AppError::not_found(format!("no slice with id {slice_id}")))?;
        let team = require_team(&repo)?;
        for agent in &team {
            check_agent(agent)?;
        }
        // The latest run only: of the reports that list a bug, the one(s) with the newest
        // `writtenAt` (the file's modified time, as `list_reports` gives the UI). A report
        // with no bugs never counts as a run, and older runs' bugs are left out.
        let mut latest: Option<String> = None;
        let mut bugs = Vec::new();
        for rel in report::slice_report_paths(&repo, slice_id)? {
            // Gone since it was listed: as if it never was.
            let Some(text) = repo_write::read_to_string(&repo, &rel)? else {
                continue;
            };
            let Some(written_at) = report::written_at(&repo, &rel)? else {
                continue;
            };
            let parsed: report::SliceReport =
                serde_json::from_str(text.trim_start_matches('\u{feff}')).map_err(|err| {
                    AppError::new(
                        ErrorCode::InvalidJson,
                        format!("{rel} isn't a valid slice report ({err})"),
                    )
                })?;
            if parsed.bugs.is_empty() {
                continue;
            }
            match latest.as_deref() {
                Some(newest) if written_at.as_str() < newest => continue,
                Some(newest) if written_at.as_str() == newest => {}
                _ => {
                    bugs.clear();
                    latest = Some(written_at);
                }
            }
            bugs.extend(parsed.bugs.0);
        }
        let prompts = fix::build(slice, &team, &bugs)?;
        tracing::info!(
            project_id,
            slice_id,
            bugs = bugs.len(),
            prompts = prompts.len(),
            "fix prompts generated"
        );
        Ok(prompts)
    }

    /// One line that starts a session. `prompt_file` must be an existing file under
    /// `.foundry/prompts/` (repo-relative). For a task prompt, the slice's dependencies must
    /// be done, unless the prompt was generated with `force` (it says so on a line).
    pub fn launch_message(
        &self,
        project_id: &str,
        agent_id: &str,
        prompt_file: &str,
    ) -> Result<String, AppError> {
        let repo = self.repos.repo(project_id)?;
        let plan = disk::plan(&repo)?;
        require_confirmed_plan(&plan)?;
        require_agents_md(&repo)?;
        let agent = find_agent(require_team(&repo)?, agent_id)?;
        let prompt = launch_message::prompt_file(&repo, prompt_file)?;

        if let Some((slice_id, _)) = parse_task_name(&plan, &prompt.name) {
            if let Some(slice) = plan.slice(slice_id) {
                let unfinished: Vec<String> = plan
                    .unfinished_dependencies(slice)
                    .iter()
                    .map(|s| s.name.clone())
                    .collect();
                if !unfinished.is_empty() {
                    let text = std::fs::read_to_string(&prompt.path)?;
                    let forced = text.lines().any(|l| l.starts_with(FORCED_PREFIX));
                    if !forced {
                        return Err(dependencies_not_done(&unfinished));
                    }
                }
            }
        }

        launch_message::build(
            &launch_message::RosterAgent {
                id: agent.id,
                display_name: agent.display_name,
                title: agent.title,
            },
            Some(&prompt),
        )
    }

    /// The text of one prompt file, as stored. `file` is a repo-relative `.md` file directly in
    /// `.foundry/prompts/`, at most `MAX_PROMPT_BYTES`. Read-only.
    pub fn read_prompt(&self, project_id: &str, file: &str) -> Result<String, AppError> {
        let repo = self.repos.repo(project_id)?;
        if file.chars().any(char::is_control) {
            return Err(AppError::invalid_path(
                "the prompt file path has control characters",
            ));
        }
        let rel = paths::check(Path::new(file), PathKind::Relative)?;
        let outside = || {
            AppError::path_escape(format!(
                "the prompt file must be directly in {PROMPTS_DIR}/"
            ))
        };
        if !rel.starts_with(PROMPTS_DIR) || rel.components().count() != 3 {
            return Err(outside());
        }
        if rel.extension().and_then(|e| e.to_str()) != Some("md") {
            return Err(AppError::new(
                ErrorCode::UnsupportedFile,
                "only .md prompt files can be read",
            ));
        }
        // Through core::paths: a link out of the prompts folder doesn't count.
        let dir = paths::resolve(&repo, PROMPTS_DIR)?;
        let resolved = paths::resolve(&repo, &rel)?;
        if !resolved.starts_with(&dir) {
            return Err(outside());
        }
        let file_not_found = || AppError::not_found(format!("no prompt file {file}"));
        // A folder can't be opened on Windows, so check before opening.
        if !resolved.is_file() {
            return Err(file_not_found());
        }
        let handle = match std::fs::File::open(&resolved) {
            Ok(handle) => handle,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Err(file_not_found()),
            Err(err) => return Err(err.into()),
        };
        // One byte past the cap tells an oversized file from one exactly at it, even if it
        // grows between the open and the read.
        let mut bytes = Vec::new();
        handle.take(MAX_PROMPT_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_PROMPT_BYTES {
            return Err(AppError::new(
                ErrorCode::FileTooLarge,
                format!("prompt files can be at most {} KB", MAX_PROMPT_BYTES / 1000),
            ));
        }
        String::from_utf8(bytes)
            .map_err(|_| AppError::invalid_input("the prompt file isn't valid UTF-8 text"))
    }

    /// The hand-off and task prompts in `.foundry/prompts/`, by file name. Other files
    /// there (temp files, anything not named like a prompt) are left out.
    pub fn list_prompts(&self, project_id: &str) -> Result<Vec<PromptFile>, AppError> {
        let repo = self.repos.repo(project_id)?;
        let dir = paths::resolve(&repo, PROMPTS_DIR)?;
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };
        let plan = disk::plan(&repo).unwrap_or_default();
        let mut out = Vec::new();
        for entry in entries {
            let entry = entry?;
            let meta = entry.metadata()?;
            if !meta.is_file() {
                continue;
            }
            let Ok(name) = entry.file_name().into_string() else {
                continue;
            };
            let (kind, slice_id, agent_id) = if name == "handoff.md" {
                (PromptKind::Handoff, None, None)
            } else if let Some((slice, agent)) = parse_task_name(&plan, &name) {
                (
                    PromptKind::Task,
                    Some(slice.to_string()),
                    Some(agent.to_string()),
                )
            } else {
                continue;
            };
            out.push(PromptFile {
                path: format!("{PROMPTS_DIR}/{name}"),
                kind,
                slice_id,
                agent_id,
                size: meta.len(),
                modified_ms: epoch_ms(meta.modified()),
            });
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }
}

#[cfg(test)]
mod tests;
