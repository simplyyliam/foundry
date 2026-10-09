//! Projects: the app-level registry, settings, and the Foundry files inside each repo.
//!
//! Repo writes go through `core::repo_write` (allowlist + safe paths + atomic). The only
//! writes outside it are `create_repository` making the new folder, running git in it and
//! writing its starter `.gitignore`.

pub mod commands;
mod scaffold;
mod storage;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::core::git::{self, Git, GitRun};
use crate::core::paths::{self, PathKind};
use crate::core::repos::RepoIndex;
use crate::core::sessions::SessionRegistry;
use crate::core::{fsx, ids, repo_write, trash, AppError, ErrorCode};
use scaffold::ProjectFile;
use storage::{now_ms, RegistryEntry, RegistryFile};

pub use scaffold::VSM_VIEWER;

/// Foundry's runtime files, kept out of `git status` through `.git/info/exclude` (local,
/// never committed). The verdict files (`.foundry/reports/`, the rest of `.foundry/reviews/`)
/// stay visible: the boundary check needs git to see them.
pub const GIT_EXCLUDES: &[&str] = &[".foundry/events/", ".foundry/reviews/digest.md"];

/// A new repo's root `.gitignore`. A linked repo's own `.gitignore` is never touched.
pub const STARTER_GITIGNORE: &str =
    "node_modules/\ndist/\n.env\n*.db\ntest-results/\nplaywright-report/\n*.log\n";

/// A registered project, as the frontend sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub tagline: String,
    pub repo_path: String,
    /// Epoch ms of the last `open_project`, or null if never opened.
    // u32 stands in for the exported type only (TS `number`), as with EventMeta.ts.
    #[specta(type = Option<u32>)]
    pub last_opened: Option<u64>,
    /// False when the folder was moved or deleted: offer `relink_project`.
    pub folder_exists: bool,
    /// Archived projects stay registered and on disk; the UI lists them separately.
    pub archived: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub max_concurrent_agents: u32,
    /// Where `create_repository` puts new repos when no `parentDir` is given.
    pub default_projects_dir: Option<String>,
    /// Use this `claude` executable instead of the one on PATH.
    pub claude_path: Option<String>,
    /// Passed to `claude -p` as `--model` when set (an alias like `sonnet`, or a full name).
    pub default_model: Option<String>,
    /// How long one roadmap generation may run. At least 1.
    pub generation_timeout_ms: u32,
    /// Which embedded AGENTS.md template `generate_agents` fills (one exists today).
    pub agents_template_id: String,
    // Stored and returned only: the frontend and later work read the fields below.
    pub theme: Theme,
    pub reduced_motion_override: bool,
    pub auto_install_hooks: bool,
    pub default_verify_mode: VerifyMode,
    pub os_notifications_enabled: bool,
    pub os_notification_severity: NotificationSeverity,
}

/// Declares a settings enum with a hidden catch-all variant. The catch-all keeps a bad value
/// parseable (so `update_settings` can name it in an `invalid_input`, instead of Tauri failing
/// to decode the argument) and is skipped in the bindings, so the TS type lists only the real
/// values. It is never stored: `update_settings` rejects it and loading replaces it.
macro_rules! settings_enum {
    ($(#[$doc:meta])* $name:ident, $field:literal, { $($(#[$vdoc:meta])* $variant:ident = $value:literal),+ $(,)? }, default $default:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
        pub enum $name {
            $($(#[$vdoc])* #[serde(rename = $value)] $variant,)+
            #[serde(untagged)]
            #[specta(skip)]
            Invalid(serde_json::Value),
        }

        impl Default for $name {
            fn default() -> Self {
                Self::$default
            }
        }

        impl $name {
            /// `invalid_input` naming the field, the bad value and the allowed ones.
            fn validate(&self) -> Result<(), AppError> {
                match self {
                    Self::Invalid(value) => Err(AppError::invalid_input(format!(
                        "{} must be one of {}, not {value}",
                        $field,
                        [$(concat!("\"", $value, "\"")),+].join(", "),
                    ))),
                    _ => Ok(()),
                }
            }

            /// A value hand-edited into `settings.json` that isn't allowed loads as the default.
            fn or_default(self) -> Self {
                match self {
                    Self::Invalid(_) => Self::default(),
                    valid => valid,
                }
            }
        }
    };
}

settings_enum!(
    /// The app's color theme. `system` follows the OS.
    Theme, "theme", { Light = "light", Dark = "dark", System = "system" }, default System
);
settings_enum!(
    /// How a finished slice is verified by default.
    VerifyMode, "defaultVerifyMode", { Auto = "auto", Manual = "manual" }, default Manual
);
settings_enum!(
    /// Which events raise an OS notification.
    NotificationSeverity,
    "osNotificationSeverity",
    { Danger = "danger", WarnAndDanger = "warn_and_danger", All = "all" },
    default Danger
);

/// Default for `agentsTemplateId`: the one embedded AGENTS.md template.
pub const DEFAULT_AGENTS_TEMPLATE_ID: &str = "default";

/// A model alias or full name (`sonnet`, `claude-opus-4-1`, `opus[1m]`): ASCII letters,
/// digits and `. - _ : / @ [ ]`. Nothing a shell or cmd.exe would interpret.
pub fn is_model_name(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 100
        && model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ".-_:/@[]".contains(c))
}

/// Default for `generationTimeoutMs`: 5 minutes.
pub const DEFAULT_GENERATION_TIMEOUT_MS: u32 = 300_000;

impl Default for Settings {
    fn default() -> Self {
        Self {
            max_concurrent_agents: 3,
            default_projects_dir: None,
            claude_path: None,
            default_model: None,
            generation_timeout_ms: DEFAULT_GENERATION_TIMEOUT_MS,
            agents_template_id: DEFAULT_AGENTS_TEMPLATE_ID.into(),
            theme: Theme::default(),
            reduced_motion_override: false,
            auto_install_hooks: true,
            default_verify_mode: VerifyMode::default(),
            os_notifications_enabled: true,
            os_notification_severity: NotificationSeverity::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateRepositoryInput {
    pub name: String,
    /// Defaults to `defaultProjectsDir`.
    pub parent_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreateRepositoryResult {
    pub project: Project,
    /// False if git isn't installed or `git init` failed.
    pub git_initialized: bool,
    /// False if the empty initial commit failed (usually: no git identity configured).
    /// When it succeeds, a second commit holds only the starter `.gitignore`.
    pub initial_commit: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkRepositoryInput {
    pub folder: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkRepositoryResult {
    pub project: Project,
    pub is_git_repo: bool,
    /// Foundry files that were missing and got added, e.g. `["vsm_viewer.html"]`.
    pub created_files: Vec<String>,
    /// Files under the local git excludes (`.foundry/events/`, `.foundry/reviews/digest.md`)
    /// that git already tracks, sorted. Excluding doesn't untrack a file, so these still show
    /// as modified; Foundry leaves them alone (the user can `git rm --cached` them).
    pub already_tracked: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPatch {
    pub name: Option<String>,
    pub tagline: Option<String>,
}

struct State {
    registry: Vec<RegistryEntry>,
    settings: Settings,
    /// In memory only: null after a restart until `open_project`.
    active: Option<String>,
}

/// Owns the registry and settings files. Share it as `Arc<ProjectsStore>`.
pub struct ProjectsStore {
    data_dir: PathBuf,
    git: Git,
    /// Kept in step with the saved registry, for other features to look repos up in.
    repos: Arc<RepoIndex>,
    /// Live agent sessions; archiving refuses while a project has one.
    sessions: Arc<SessionRegistry>,
    on_released: Option<ReleasedHook>,
    state: Mutex<State>,
}

/// Called with a repo folder's canonical path once it leaves the registry (`remove_project`
/// or `delete_repository`), so the app can drop access it granted there (e.g. the asset
/// protocol), whether or not the files are still on disk.
pub type ReleasedHook = Box<dyn Fn(&Path) + Send + Sync>;

fn validate_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::invalid_input("project name can't be empty"));
    }
    if name.chars().count() > 100 {
        return Err(AppError::invalid_input(
            "project name must be 100 characters or fewer",
        ));
    }
    Ok(name.to_string())
}

fn existing_dir(path: &str) -> Result<PathBuf, AppError> {
    let dir = paths::check(Path::new(path), PathKind::Absolute)?;
    match fs::metadata(&dir) {
        Ok(meta) if meta.is_dir() => Ok(dir),
        Ok(_) => Err(AppError::invalid_path(format!(
            "not a folder: {}",
            dir.display()
        ))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(AppError::not_found(
            format!("folder does not exist: {}", dir.display()),
        )),
        Err(err) => Err(err.into()),
    }
}

/// Same folder on disk? Resolves links where both exist; otherwise compares the paths
/// (case-insensitively on Windows).
fn same_folder(a: &Path, b: &Path) -> bool {
    if let (Ok(a), Ok(b)) = (a.canonicalize(), b.canonicalize()) {
        return a == b;
    }
    let (a, b) = (a.to_string_lossy(), b.to_string_lossy());
    if cfg!(windows) {
        a.eq_ignore_ascii_case(&b)
    } else {
        a == b
    }
}

fn to_project(entry: &RegistryEntry) -> Project {
    Project {
        id: entry.id.clone(),
        name: entry.name.clone(),
        tagline: entry.tagline.clone(),
        repo_path: entry.repo_path.clone(),
        last_opened: entry.last_opened,
        folder_exists: Path::new(&entry.repo_path).is_dir(),
        archived: entry.archived,
    }
}

fn publish(repos: &RepoIndex, registry: &[RegistryEntry]) {
    repos.replace_all(
        registry
            .iter()
            .map(|entry| (entry.id.as_str(), Path::new(&entry.repo_path))),
    );
    repos.set_archived(
        registry
            .iter()
            .filter(|entry| entry.archived)
            .map(|entry| entry.id.as_str()),
    );
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

impl ProjectsStore {
    /// Loads `projects.json` and `settings.json` from `data_dir`, creating it if needed, and
    /// publishes every project's folder to `repos`. `sessions` is the shared live-session
    /// state that `archive_project` checks.
    pub fn load(
        data_dir: impl Into<PathBuf>,
        git: Git,
        repos: Arc<RepoIndex>,
        sessions: Arc<SessionRegistry>,
    ) -> Result<Self, AppError> {
        let data_dir = data_dir.into();
        fs::create_dir_all(&data_dir)?;
        let registry: RegistryFile = storage::load(&data_dir, storage::REGISTRY_FILE)?;
        let settings: storage::SettingsFile = storage::load(&data_dir, storage::SETTINGS_FILE)?;
        tracing::info!(projects = registry.projects.len(), "projects loaded");
        publish(&repos, &registry.projects);
        Ok(Self {
            data_dir,
            git,
            repos,
            sessions,
            on_released: None,
            state: Mutex::new(State {
                registry: registry.projects,
                settings: settings.into(),
                active: None,
            }),
        })
    }

    /// Sets the hook run when a repo folder leaves the registry (see [`ReleasedHook`]).
    pub fn with_released_hook(mut self, hook: impl Fn(&Path) + Send + Sync + 'static) -> Self {
        self.on_released = Some(Box::new(hook));
        self
    }

    fn release(&self, canonical: &Path) {
        if let Some(hook) = &self.on_released {
            hook(canonical);
        }
    }

    fn lock(&self) -> Result<MutexGuard<'_, State>, AppError> {
        self.state
            .lock()
            .map_err(|_| AppError::internal("projects state is poisoned"))
    }

    fn save_registry(&self, state: &State) -> Result<(), AppError> {
        storage::save_registry(&self.data_dir, &state.registry)?;
        publish(&self.repos, &state.registry);
        Ok(())
    }

    fn find<'a>(state: &'a State, id: &str) -> Result<&'a RegistryEntry, AppError> {
        state
            .registry
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| AppError::not_found(format!("no project with id {id}")))
    }

    fn find_mut<'a>(state: &'a mut State, id: &str) -> Result<&'a mut RegistryEntry, AppError> {
        state
            .registry
            .iter_mut()
            .find(|entry| entry.id == id)
            .ok_or_else(|| AppError::not_found(format!("no project with id {id}")))
    }

    /// The project's folder, or `FolderMissing` if it's gone.
    fn repo_root(entry: &RegistryEntry) -> Result<PathBuf, AppError> {
        let root = PathBuf::from(&entry.repo_path);
        if root.is_dir() {
            Ok(root)
        } else {
            Err(AppError::new(
                ErrorCode::FolderMissing,
                format!("project folder is missing: {}", entry.repo_path),
            ))
        }
    }

    fn registered_elsewhere(state: &State, folder: &Path, except: Option<&str>) -> Option<String> {
        state
            .registry
            .iter()
            .filter(|entry| Some(entry.id.as_str()) != except)
            .find(|entry| same_folder(Path::new(&entry.repo_path), folder))
            .map(|entry| entry.name.clone())
    }

    pub fn list(&self) -> Result<Vec<Project>, AppError> {
        Ok(self.lock()?.registry.iter().map(to_project).collect())
    }

    /// Makes `<parentDir>/<slug(name)>`, runs `git init` and an empty initial commit, commits a
    /// starter `.gitignore` on top, adds the Foundry files and the local git excludes, and
    /// registers it. Doesn't open it.
    pub fn create_repository(
        &self,
        input: CreateRepositoryInput,
    ) -> Result<CreateRepositoryResult, AppError> {
        let name = validate_name(&input.name)?;
        let folder_name = scaffold::slug(&name)?;
        let mut state = self.lock()?;

        let parent = input
            .parent_dir
            .or_else(|| state.settings.default_projects_dir.clone())
            .ok_or_else(|| {
                AppError::invalid_path("no parent folder: pass parentDir or set defaultProjectsDir")
            })?;
        let parent = existing_dir(&parent)?;
        // resolve() is the safety check; it returns a canonical (`\\?\C:\…` on Windows) path,
        // so store the plain join, which is the same folder: the slug has no separators.
        paths::resolve(&parent, &folder_name)?;
        let folder = parent.join(&folder_name);

        // Nothing is touched when the folder exists; create_dir also catches a racing creator.
        let exists = || {
            AppError::new(
                ErrorCode::FolderExists,
                format!("a folder already exists at {}", folder.display()),
            )
        };
        if folder.symlink_metadata().is_ok() {
            return Err(exists());
        }
        match fs::create_dir(&folder) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => return Err(exists()),
            Err(err) => return Err(err.into()),
        }

        let result = self.fill_new_repository(&mut state, &folder, name);
        if result.is_err() {
            // Only this call created the folder, moments ago: undo so a retry isn't blocked.
            if let Err(err) = fs::remove_dir_all(&folder) {
                tracing::warn!(error = %err, folder = %folder.display(), "could not roll back new repo folder");
            }
        }
        result
    }

    fn fill_new_repository(
        &self,
        state: &mut State,
        folder: &Path,
        name: String,
    ) -> Result<CreateRepositoryResult, AppError> {
        let git_initialized = self.git_ok(folder, &["init", "--quiet"], "git init");
        // Empty, before the Foundry files exist, so HEAD exists and the files stay untracked.
        let initial_commit = git_initialized
            && self.git_ok(
                folder,
                &["commit", "--allow-empty", "--quiet", "-m", "Initial commit"],
                "initial commit",
            );
        // The starter .gitignore gets its own commit: left untracked, the boundary check
        // would count it as a slice's change. Its own commit keeps the first one empty.
        if !fsx::write_atomic_new(&paths::resolve(folder, ".gitignore")?, STARTER_GITIGNORE)? {
            return Err(AppError::internal(
                "a .gitignore appeared in the new folder",
            ));
        }
        if initial_commit && self.git_ok(folder, &["add", "--", ".gitignore"], "stage .gitignore") {
            self.git_ok(
                folder,
                &[
                    "commit",
                    "--quiet",
                    "-m",
                    "Add .gitignore",
                    "--",
                    ".gitignore",
                ],
                "commit .gitignore",
            );
        }
        if git_initialized {
            self.add_excludes(folder);
        }

        let file = ProjectFile {
            id: ids::new_id(),
            name,
            tagline: String::new(),
            created_at: now_ms(),
            test_commands: Vec::new(),
            design_family: Vec::new(),
        };
        scaffold::ensure(folder, &file)?;

        let entry = RegistryEntry {
            id: file.id,
            name: file.name,
            tagline: file.tagline,
            repo_path: path_string(folder),
            last_opened: None,
            archived: false,
        };
        state.registry.push(entry.clone());
        if let Err(err) = self.save_registry(state) {
            state.registry.pop();
            return Err(err);
        }
        tracing::info!(id = %entry.id, git_initialized, initial_commit, "repository created");
        Ok(CreateRepositoryResult {
            project: to_project(&entry),
            git_initialized,
            initial_commit,
        })
    }

    fn git_ok(&self, dir: &Path, args: &[&str], what: &str) -> bool {
        match self.git.run(dir, args) {
            Ok(GitRun::Ok(_)) => true,
            Ok(GitRun::Failed(output)) => {
                tracing::warn!(step = what, stderr = %git::stderr_line(&output), "git step failed");
                false
            }
            Ok(GitRun::Missing) => {
                tracing::warn!(step = what, "git not found");
                false
            }
            Err(err) => {
                tracing::warn!(step = what, error = %err, "could not run git");
                false
            }
        }
    }

    /// Adds [`GIT_EXCLUDES`] to the repo's `.git/info/exclude`. Best effort: a failure only
    /// costs git noise, so it's logged, not returned.
    fn add_excludes(&self, folder: &Path) {
        match repo_write::add_git_excludes(folder, GIT_EXCLUDES) {
            Ok(added) => tracing::info!(added = added.len(), "git excludes ensured"),
            Err(err) => tracing::warn!(error = %err, "could not add git excludes"),
        }
    }

    /// Files under [`GIT_EXCLUDES`] that git tracks. Not a git repo, or git failing: none.
    fn tracked_excluded(&self, folder: &Path) -> Vec<String> {
        let mut args = vec!["--no-optional-locks", "ls-files", "-z", "--"];
        args.extend(GIT_EXCLUDES);
        let mut files: Vec<String> = match self.git.run(folder, &args) {
            Ok(GitRun::Ok(output)) => String::from_utf8_lossy(&output.stdout)
                .split('\0')
                .filter(|p| !p.is_empty())
                .map(str::to_string)
                .collect(),
            _ => Vec::new(),
        };
        files.sort();
        files
    }

    /// Registers a folder the user set up themselves. Adds only missing Foundry files and the
    /// local git excludes; the only git it runs is a read (`ls-files`). A folder that already
    /// has a `.foundry/project.json` keeps its id and name.
    pub fn link_repository(
        &self,
        input: LinkRepositoryInput,
    ) -> Result<LinkRepositoryResult, AppError> {
        let folder = existing_dir(&input.folder)?;
        let mut state = self.lock()?;
        if let Some(name) = Self::registered_elsewhere(&state, &folder, None) {
            return Err(AppError::new(
                ErrorCode::AlreadyLinked,
                format!("{} is already linked as “{name}”", folder.display()),
            ));
        }

        let file = match scaffold::read(&folder)? {
            Some(file) if Self::find(&state, &file.id).is_err() => file,
            Some(file) => {
                return Err(AppError::new(
                    ErrorCode::AlreadyLinked,
                    format!("this folder's project id {} is already registered", file.id),
                ))
            }
            None => ProjectFile {
                id: ids::new_id(),
                name: folder
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path_string(&folder)),
                tagline: String::new(),
                created_at: now_ms(),
                test_commands: Vec::new(),
                design_family: Vec::new(),
            },
        };
        let created_files = scaffold::ensure(&folder, &file)?;
        let is_git_repo = folder.join(".git").exists();
        let already_tracked = if is_git_repo {
            self.add_excludes(&folder);
            self.tracked_excluded(&folder)
        } else {
            Vec::new()
        };

        let entry = RegistryEntry {
            id: file.id,
            name: file.name,
            tagline: file.tagline,
            repo_path: path_string(&folder),
            last_opened: None,
            archived: false,
        };
        state.registry.push(entry.clone());
        if let Err(err) = self.save_registry(&state) {
            state.registry.pop();
            return Err(err);
        }
        tracing::info!(
            id = %entry.id,
            created = created_files.len(),
            already_tracked = already_tracked.len(),
            "repository linked"
        );
        Ok(LinkRepositoryResult {
            project: to_project(&entry),
            is_git_repo,
            created_files,
            already_tracked,
        })
    }

    /// Changes name and/or tagline in the registry and `.foundry/project.json`. Never renames
    /// or moves the folder.
    pub fn update_project(&self, id: &str, patch: ProjectPatch) -> Result<Project, AppError> {
        let mut state = self.lock()?;
        let mut entry = Self::find(&state, id)?.clone();
        if let Some(name) = patch.name {
            entry.name = validate_name(&name)?;
        }
        if let Some(tagline) = patch.tagline {
            entry.tagline = tagline.trim().to_string();
        }

        // A missing folder still gets the registry change; the file catches up on relink.
        if let Ok(root) = Self::repo_root(&entry) {
            let created_at = scaffold::read(&root)?.map_or_else(now_ms, |f| f.created_at);
            scaffold::update_identity(
                &root,
                &ProjectFile {
                    id: entry.id.clone(),
                    name: entry.name.clone(),
                    tagline: entry.tagline.clone(),
                    created_at,
                    test_commands: Vec::new(),
                    design_family: Vec::new(),
                },
            )?;
        }

        let previous = std::mem::replace(Self::find_mut(&mut state, id)?, entry.clone());
        if let Err(err) = self.save_registry(&state) {
            *Self::find_mut(&mut state, id)? = previous;
            return Err(err);
        }
        tracing::info!(id, "project updated");
        Ok(to_project(&entry))
    }

    /// Makes the project active and records `lastOpened`.
    pub fn open_project(&self, id: &str) -> Result<Project, AppError> {
        let mut state = self.lock()?;
        Self::repo_root(Self::find(&state, id)?)?;
        let entry = Self::find_mut(&mut state, id)?;
        let previous = entry.last_opened.replace(now_ms());
        let project = to_project(entry);
        if let Err(err) = self.save_registry(&state) {
            Self::find_mut(&mut state, id)?.last_opened = previous;
            return Err(err);
        }
        state.active = Some(id.to_string());
        tracing::info!(id, "project opened");
        Ok(project)
    }

    pub fn active_project(&self) -> Result<Option<Project>, AppError> {
        let state = self.lock()?;
        Ok(state
            .active
            .as_deref()
            .and_then(|id| Self::find(&state, id).ok())
            .map(to_project))
    }

    /// Points a project at a new folder (e.g. after the user moved it). Adds missing Foundry
    /// files there; refuses a folder that belongs to a different project.
    pub fn relink_project(&self, id: &str, folder: &str) -> Result<Project, AppError> {
        let folder = existing_dir(folder)?;
        let mut state = self.lock()?;
        let entry = Self::find(&state, id)?.clone();
        if let Some(name) = Self::registered_elsewhere(&state, &folder, Some(id)) {
            return Err(AppError::new(
                ErrorCode::AlreadyLinked,
                format!("{} is already linked as “{name}”", folder.display()),
            ));
        }
        if let Some(file) = scaffold::read(&folder)? {
            if file.id != entry.id {
                return Err(AppError::invalid_input(format!(
                    "{} belongs to a different Foundry project (“{}”)",
                    folder.display(),
                    file.name
                )));
            }
        }
        scaffold::ensure(
            &folder,
            &ProjectFile {
                id: entry.id.clone(),
                name: entry.name.clone(),
                tagline: entry.tagline.clone(),
                created_at: now_ms(),
                test_commands: Vec::new(),
                design_family: Vec::new(),
            },
        )?;

        let target = Self::find_mut(&mut state, id)?;
        let previous = std::mem::replace(&mut target.repo_path, path_string(&folder));
        let project = to_project(target);
        if let Err(err) = self.save_registry(&state) {
            Self::find_mut(&mut state, id)?.repo_path = previous;
            return Err(err);
        }
        tracing::info!(id, "project relinked");
        Ok(project)
    }

    /// Archives the project and, if it's the active one, closes it (active becomes null).
    /// Registry only: the folder is never touched. Idempotent. `session_running` (and nothing
    /// changes) while an agent session is live for it.
    pub fn archive_project(&self, id: &str) -> Result<Project, AppError> {
        let mut state = self.lock()?;
        Self::find(&state, id)?;
        // Checked and saved (which publishes the archived flag) under the session registry's
        // lock, so a session can't be reserved for this project in between.
        let project = self.sessions.unless_project_active(
            id,
            "stop this project's agent session before archiving it",
            || self.set_archived(&mut state, id, true),
        )?;
        if state.active.as_deref() == Some(id) {
            state.active = None;
            tracing::info!(id, "archived project closed");
        }
        Ok(project)
    }

    /// Unarchives (restores) the project. Registry only. Idempotent. Doesn't reopen it.
    pub fn unarchive_project(&self, id: &str) -> Result<Project, AppError> {
        let mut state = self.lock()?;
        self.set_archived(&mut state, id, false)
    }

    fn set_archived(
        &self,
        state: &mut State,
        id: &str,
        archived: bool,
    ) -> Result<Project, AppError> {
        let entry = Self::find_mut(state, id)?;
        let previous = std::mem::replace(&mut entry.archived, archived);
        let project = to_project(entry);
        if previous == archived {
            return Ok(project);
        }
        if let Err(err) = self.save_registry(state) {
            Self::find_mut(state, id)?.archived = previous;
            return Err(err);
        }
        tracing::info!(id, archived, "project archive flag changed");
        Ok(project)
    }

    /// The registry index of an archived project: `not_found` / `not_archived` otherwise.
    fn archived_index(state: &State, id: &str, action: &str) -> Result<usize, AppError> {
        let index = state
            .registry
            .iter()
            .position(|entry| entry.id == id)
            .ok_or_else(|| AppError::not_found(format!("no project with id {id}")))?;
        if !state.registry[index].archived {
            return Err(AppError::new(
                ErrorCode::NotArchived,
                format!("archive this project before {action} it"),
            ));
        }
        Ok(index)
    }

    /// Drops the entry from the registry (and closes it if active).
    fn unregister(&self, state: &mut State, index: usize) -> Result<(), AppError> {
        let removed = state.registry.remove(index);
        if let Err(err) = self.save_registry(state) {
            state.registry.insert(index, removed);
            return Err(err);
        }
        if state.active.as_deref() == Some(removed.id.as_str()) {
            state.active = None;
        }
        Ok(())
    }

    /// Unregisters an archived project. Never deletes or changes repo files.
    /// `not_archived` if it hasn't been archived first.
    pub fn remove_project(&self, id: &str) -> Result<(), AppError> {
        let mut state = self.lock()?;
        let index = Self::archived_index(&state, id, "removing")?;
        let folder = PathBuf::from(&state.registry[index].repo_path);
        self.unregister(&mut state, index)?;
        tracing::info!(id, "project removed");
        // The files stay, but Foundry's access to them goes with the registration.
        self.release(&folder.canonicalize().unwrap_or(folder));
        Ok(())
    }

    /// Moves an archived project's folder to the Recycle Bin, then unregisters it. A folder
    /// that's already gone is just unregistered. If the move fails (`io`, e.g. a locked
    /// file), nothing changes: the folder stays and the project stays registered.
    pub fn delete_repository(&self, id: &str) -> Result<(), AppError> {
        let folder = {
            let state = self.lock()?;
            let index = Self::archived_index(&state, id, "deleting")?;
            PathBuf::from(&state.registry[index].repo_path)
        };
        // Outside the lock: the shell may take a while, or show its own prompt.
        if folder.symlink_metadata().is_ok() {
            // Resolved first: once it's in the Recycle Bin the path can't be canonicalized.
            let canonical = folder.canonicalize().unwrap_or_else(|_| folder.clone());
            trash::move_to_trash(&folder)?;
            tracing::info!(id, "project folder moved to the Recycle Bin");
            self.release(&canonical);
        }
        let mut state = self.lock()?;
        let index = Self::archived_index(&state, id, "deleting")?;
        self.unregister(&mut state, index)?;
        tracing::info!(id, "repository deleted");
        Ok(())
    }

    /// The project's `docs/brain-dump.md`; empty if the file doesn't exist.
    pub fn get_document(&self, id: &str) -> Result<String, AppError> {
        let root = Self::repo_root(Self::find(&*self.lock()?, id)?)?;
        Ok(repo_write::read_to_string(&root, scaffold::BRAIN_DUMP)?.unwrap_or_default())
    }

    /// Replaces `docs/brain-dump.md` atomically.
    pub fn save_document(&self, id: &str, markdown: &str) -> Result<(), AppError> {
        let root = Self::repo_root(Self::find(&*self.lock()?, id)?)?;
        repo_write::write(&root, scaffold::BRAIN_DUMP, markdown)?;
        tracing::debug!(id, bytes = markdown.len(), "document saved");
        Ok(())
    }

    pub fn get_settings(&self) -> Result<Settings, AppError> {
        Ok(self.lock()?.settings.clone())
    }

    /// Replaces the settings (send back the whole object from `get_settings`).
    pub fn update_settings(&self, settings: Settings) -> Result<Settings, AppError> {
        if settings.max_concurrent_agents == 0 {
            return Err(AppError::invalid_input(
                "maxConcurrentAgents must be at least 1",
            ));
        }
        if let Some(model) = settings.default_model.as_deref().map(str::trim) {
            if !model.is_empty() && !is_model_name(model) {
                return Err(AppError::invalid_input(
                    "defaultModel may only contain letters, digits and . - _ : / @ [ ]",
                ));
            }
        }
        if settings.generation_timeout_ms == 0 {
            return Err(AppError::invalid_input(
                "generationTimeoutMs must be at least 1",
            ));
        }
        settings.theme.validate()?;
        settings.default_verify_mode.validate()?;
        settings.os_notification_severity.validate()?;
        let clean = |value: Option<String>| -> Result<Option<String>, AppError> {
            match value
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
            {
                Some(path) => Ok(Some(path_string(&paths::check(
                    Path::new(&path),
                    PathKind::Absolute,
                )?))),
                None => Ok(None),
            }
        };
        let settings = Settings {
            max_concurrent_agents: settings.max_concurrent_agents,
            default_projects_dir: clean(settings.default_projects_dir)?,
            claude_path: clean(settings.claude_path)?,
            default_model: settings
                .default_model
                .map(|model| model.trim().to_string())
                .filter(|model| !model.is_empty()),
            generation_timeout_ms: settings.generation_timeout_ms,
            agents_template_id: Some(settings.agents_template_id.trim().to_string())
                .filter(|id| !id.is_empty())
                .unwrap_or_else(|| DEFAULT_AGENTS_TEMPLATE_ID.into()),
            theme: settings.theme,
            reduced_motion_override: settings.reduced_motion_override,
            auto_install_hooks: settings.auto_install_hooks,
            default_verify_mode: settings.default_verify_mode,
            os_notifications_enabled: settings.os_notifications_enabled,
            os_notification_severity: settings.os_notification_severity,
        };
        let mut state = self.lock()?;
        storage::save_settings(&self.data_dir, &settings)?;
        state.settings = settings.clone();
        tracing::info!("settings updated");
        Ok(settings)
    }
}

#[cfg(test)]
mod tests;
