//! The Design Library (app-level design guideline files, the user's own) and generating a
//! project's AGENTS.md and `docs/design/` from it.
//!
//! Library: `<app data>/design-library/<filename>.md` plus `design-library/index.json`.
//! Project: `.foundry/agents-placeholders.json` (confirmed values), `AGENTS.md`,
//! `docs/design/*.md` (byte-identical copies), `docs/design/manifest.md`, and
//! `.foundry/design-files.json` (which copies Foundry made, so stale ones can go).

pub mod commands;
mod template;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::core::paths::{self, PathKind};
use crate::core::repos::RepoIndex;
use crate::core::{fsx, ids, repo_write, AppError, ErrorCode};

const LIBRARY_DIR: &str = "design-library";
const INDEX_FILE: &str = "index.json";
const PLACEHOLDERS_FILE: &str = ".foundry/agents-placeholders.json";
const COPIED_FILE: &str = ".foundry/design-files.json";
const AGENTS_FILE: &str = "AGENTS.md";
const DESIGN_DIR: &str = "docs/design";
const MANIFEST_FILE: &str = "docs/design/manifest.md";
const BACKUP_DIR: &str = ".foundry/backups";
/// The placeholder that picks Design Library files. Not a template marker.
pub const DESIGN_FAMILY_KEY: &str = "designFamily";
const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;

// ---------- types ----------

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignFile {
    pub id: String,
    /// Also its name in a project's `docs/design/`. Unique in the library.
    pub filename: String,
    /// Free-text tags (lowercase). A file with none is never copied into a project.
    pub families: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignLibrary {
    /// Ordered by filename.
    pub files: Vec<DesignFile>,
    pub untagged_count: u32,
    /// Every family tag in use, sorted.
    pub families: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DesignFilePatch {
    pub families: Vec<String>,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Placeholder {
    pub key: String,
    pub label: String,
    /// The template's own guidance for it.
    pub hint: String,
    /// False for `designFamily`, which picks Design Library files rather than filling text.
    pub in_template: bool,
    /// An empty value is allowed (it removes the line, as the template says).
    pub optional: bool,
    pub value: Option<String>,
    pub confirmed: bool,
    /// For `designFamily`: the Design Library's family tags.
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, Type)]
pub struct GenerateOptions {
    /// Replace an existing AGENTS.md that differs (it's backed up first).
    pub overwrite: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GenerateAgentsResult {
    /// False when AGENTS.md already had exactly these bytes.
    pub agents_md_written: bool,
    /// Where the previous AGENTS.md was backed up, if it was replaced.
    pub backup: Option<String>,
    /// The project's confirmed design families.
    pub design_families: Vec<String>,
    /// Library files copied into `docs/design/`, by filename.
    pub copied: Vec<String>,
    /// True when no library file matched, so nothing was copied (not an error).
    pub design_copy_skipped: bool,
    /// Project families no library file has.
    pub unmatched_families: Vec<String>,
    /// Earlier copies removed because they no longer match.
    pub removed: Vec<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    files: Vec<DesignFile>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Confirmed {
    #[serde(default)]
    template_id: String,
    #[serde(default)]
    values: BTreeMap<String, String>,
}

// ---------- helpers ----------

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Trimmed, lowercase, de-duplicated, sorted.
fn clean_families(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn parse_families(value: &str) -> Vec<String> {
    clean_families(value.split(',').map(str::to_string))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// A plain `.md` file name: no folders, not hidden.
fn check_filename(name: &str) -> Result<(), AppError> {
    let plain = paths::check(Path::new(name), PathKind::Relative)
        .is_ok_and(|p| p.components().count() == 1)
        && !name.contains(['/', '\\', ':'])
        && !name.starts_with('.');
    if !plain {
        return Err(AppError::path_escape(format!(
            "not a plain file name: {name}"
        )));
    }
    let is_md = Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md"));
    if !is_md || name.eq_ignore_ascii_case("manifest.md") {
        return Err(AppError::new(
            ErrorCode::UnsupportedFile,
            format!("{name}: the Design Library takes .md files only (not manifest.md)"),
        ));
    }
    Ok(())
}

// ---------- the store ----------

/// Owns the Design Library and AGENTS.md generation. Share it as `Arc<DesignStore>`.
pub struct DesignStore {
    library: PathBuf,
    repos: Arc<RepoIndex>,
    template_id: Box<dyn Fn() -> String + Send + Sync>,
    /// One writer at a time (library and projects alike; these are rare, quick operations).
    lock: Mutex<()>,
}

impl DesignStore {
    /// `template_id` reads the `agentsTemplateId` setting.
    pub fn new(
        data_dir: &Path,
        repos: Arc<RepoIndex>,
        template_id: impl Fn() -> String + Send + Sync + 'static,
    ) -> Self {
        Self {
            library: data_dir.join(LIBRARY_DIR),
            repos,
            template_id: Box::new(template_id),
            lock: Mutex::new(()),
        }
    }

    fn load_index(&self) -> Result<Index, AppError> {
        match fs::read(self.library.join(INDEX_FILE)) {
            Ok(bytes) => serde_json::from_slice(&bytes).map_err(|err| {
                AppError::internal(format!("the Design Library index is unreadable: {err}"))
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Index::default()),
            Err(err) => Err(err.into()),
        }
    }

    fn save_index(&self, index: &mut Index) -> Result<(), AppError> {
        index.files.sort_by(|a, b| a.filename.cmp(&b.filename));
        fs::create_dir_all(&self.library)?;
        let json = serde_json::to_vec_pretty(&*index)
            .map_err(|err| AppError::internal(format!("serialize the library index: {err}")))?;
        fsx::write_atomic(&self.library.join(INDEX_FILE), json)
    }

    fn library_view(index: &Index) -> DesignLibrary {
        let mut files = index.files.clone();
        files.sort_by(|a, b| a.filename.cmp(&b.filename));
        let untagged = files.iter().filter(|f| f.families.is_empty()).count();
        DesignLibrary {
            families: clean_families(files.iter().flat_map(|f| f.families.clone())),
            untagged_count: u32::try_from(untagged).unwrap_or(u32::MAX),
            files,
        }
    }

    pub fn library(&self) -> Result<DesignLibrary, AppError> {
        let _guard = lock(&self.lock);
        Ok(Self::library_view(&self.load_index()?))
    }

    /// Adds `.md` files (absolute paths) to the library, untagged. All or nothing.
    pub fn add_from_paths(&self, sources: &[String]) -> Result<DesignLibrary, AppError> {
        if sources.is_empty() {
            return Err(AppError::invalid_input("no files to add"));
        }
        let _guard = lock(&self.lock);
        let mut index = self.load_index()?;
        let mut incoming: Vec<(String, Vec<u8>)> = Vec::new();
        for source in sources {
            let path = paths::check(Path::new(source), PathKind::Absolute)?;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .ok_or_else(|| AppError::invalid_path(format!("not a file: {source}")))?;
            check_filename(&name)?;
            let taken = |n: &str| {
                index
                    .files
                    .iter()
                    .any(|f| f.filename.eq_ignore_ascii_case(n))
                    || incoming.iter().any(|(f, _)| f.eq_ignore_ascii_case(n))
            };
            if taken(&name) {
                return Err(AppError::invalid_input(format!(
                    "the Design Library already has a file named {name}"
                )));
            }
            let meta = fs::metadata(&path)?;
            if !meta.is_file() {
                return Err(AppError::invalid_path(format!("not a file: {source}")));
            }
            if meta.len() > MAX_FILE_BYTES {
                return Err(AppError::new(
                    ErrorCode::FileTooLarge,
                    format!("{name} is over the 10 MB limit"),
                ));
            }
            incoming.push((name, fs::read(&path)?));
        }
        fs::create_dir_all(&self.library)?;
        let mut written: Vec<PathBuf> = Vec::new();
        for (name, bytes) in &incoming {
            let target = self.library.join(name);
            let result = fsx::write_atomic_new(&target, bytes).and_then(|created| {
                created.then_some(()).ok_or_else(|| {
                    AppError::invalid_input(format!("{name} already exists in the library folder"))
                })
            });
            if let Err(err) = result {
                for path in &written {
                    let _ = fs::remove_file(path);
                }
                return Err(err);
            }
            written.push(target);
        }
        for (name, _) in incoming {
            index.files.push(DesignFile {
                id: ids::new_id(),
                filename: name,
                families: Vec::new(),
                description: String::new(),
            });
        }
        if let Err(err) = self.save_index(&mut index) {
            for path in &written {
                let _ = fs::remove_file(path);
            }
            return Err(err);
        }
        tracing::info!(added = written.len(), "design library files added");
        Ok(Self::library_view(&index))
    }

    pub fn update(&self, id: &str, patch: DesignFilePatch) -> Result<DesignLibrary, AppError> {
        let _guard = lock(&self.lock);
        let mut index = self.load_index()?;
        let file = index
            .files
            .iter_mut()
            .find(|f| f.id == id)
            .ok_or_else(|| AppError::not_found(format!("no Design Library file {id}")))?;
        file.families = clean_families(patch.families);
        file.description = patch.description.trim().to_string();
        self.save_index(&mut index)?;
        Ok(Self::library_view(&index))
    }

    pub fn remove(&self, id: &str) -> Result<DesignLibrary, AppError> {
        let _guard = lock(&self.lock);
        let mut index = self.load_index()?;
        let at = index
            .files
            .iter()
            .position(|f| f.id == id)
            .ok_or_else(|| AppError::not_found(format!("no Design Library file {id}")))?;
        let file = index.files.remove(at);
        self.save_index(&mut index)?;
        match fs::remove_file(self.library.join(&file.filename)) {
            Ok(()) => {}
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => return Err(err.into()),
        }
        Ok(Self::library_view(&index))
    }

    // ---------- placeholders ----------

    fn template(&self) -> Result<(String, String), AppError> {
        let id = (self.template_id)();
        let text = template::text(&id).ok_or_else(|| {
            AppError::invalid_input(format!("agentsTemplateId \"{id}\" isn't a known template"))
        })?;
        Ok((id, template::body(&text)))
    }

    fn load_confirmed(repo: &Path, template_id: &str) -> Result<Confirmed, AppError> {
        let confirmed: Confirmed = match repo_write::read_to_string(repo, PLACEHOLDERS_FILE)? {
            Some(text) => serde_json::from_str(&text).map_err(|err| {
                AppError::internal(format!("{PLACEHOLDERS_FILE} is unreadable: {err}"))
            })?,
            None => Confirmed::default(),
        };
        // Values confirmed for another template don't carry over.
        if confirmed.template_id == template_id {
            Ok(confirmed)
        } else {
            Ok(Confirmed {
                template_id: template_id.into(),
                values: BTreeMap::new(),
            })
        }
    }

    fn placeholder_list(
        &self,
        body: &str,
        confirmed: &Confirmed,
    ) -> Result<Vec<Placeholder>, AppError> {
        let mut list: Vec<Placeholder> = template::markers(body)
            .into_iter()
            .map(|m| Placeholder {
                value: confirmed.values.get(&m.key).cloned(),
                confirmed: confirmed.values.contains_key(&m.key),
                key: m.key,
                label: m.label,
                hint: m.hint,
                in_template: true,
                optional: m.optional,
                suggestions: Vec::new(),
            })
            .collect();
        list.push(Placeholder {
            key: DESIGN_FAMILY_KEY.into(),
            label: "Design family".into(),
            hint: "Comma-separated family tags; Design Library files with any of them are copied \
                   into docs/design/"
                .into(),
            in_template: false,
            optional: true,
            value: confirmed.values.get(DESIGN_FAMILY_KEY).cloned(),
            confirmed: confirmed.values.contains_key(DESIGN_FAMILY_KEY),
            suggestions: Self::library_view(&self.load_index()?).families,
        });
        Ok(list)
    }

    pub fn placeholders(&self, project_id: &str) -> Result<Vec<Placeholder>, AppError> {
        let repo = self.repos.repo(project_id)?;
        let _guard = lock(&self.lock);
        let (template_id, body) = self.template()?;
        let confirmed = Self::load_confirmed(&repo, &template_id)?;
        self.placeholder_list(&body, &confirmed)
    }

    pub fn confirm_placeholder(
        &self,
        project_id: &str,
        key: &str,
        value: &str,
    ) -> Result<Vec<Placeholder>, AppError> {
        let repo = self.repos.repo(project_id)?;
        let _guard = lock(&self.lock);
        let (template_id, body) = self.template()?;
        let mut confirmed = Self::load_confirmed(&repo, &template_id)?;
        let list = self.placeholder_list(&body, &confirmed)?;
        let placeholder = list
            .iter()
            .find(|p| p.key == key)
            .ok_or_else(|| AppError::not_found(format!("no placeholder named {key}")))?;
        let value = if key == DESIGN_FAMILY_KEY {
            parse_families(value).join(", ")
        } else {
            value.trim().to_string()
        };
        if value.is_empty() && !placeholder.optional {
            return Err(AppError::invalid_input(format!(
                "{} needs a value",
                placeholder.label
            )));
        }
        if template::contains_marker(&value) {
            return Err(AppError::invalid_input(
                "a value can't contain a [[ FILL IN marker",
            ));
        }
        confirmed.values.insert(key.to_string(), value);
        let json = serde_json::to_string_pretty(&confirmed)
            .map_err(|err| AppError::internal(format!("serialize placeholders: {err}")))?;
        repo_write::write(&repo, PLACEHOLDERS_FILE, json + "\n")?;
        tracing::info!(project_id, key, "AGENTS.md placeholder confirmed");
        self.placeholder_list(&body, &confirmed)
    }

    // ---------- generate ----------

    /// Writes AGENTS.md, `docs/design/` and its manifest. Refuses (`placeholders_unconfirmed`)
    /// while any placeholder is unconfirmed, and (`agents_md_exists`) when a differing
    /// AGENTS.md exists, unless `overwrite`, in which case it's backed up first.
    pub fn generate(
        &self,
        project_id: &str,
        options: GenerateOptions,
    ) -> Result<GenerateAgentsResult, AppError> {
        let repo = self.repos.repo(project_id)?;
        let _guard = lock(&self.lock);
        let (template_id, body) = self.template()?;
        let confirmed = Self::load_confirmed(&repo, &template_id)?;
        let pending: Vec<String> = self
            .placeholder_list(&body, &confirmed)?
            .into_iter()
            .filter(|p| !p.confirmed)
            .map(|p| p.label)
            .collect();
        if !pending.is_empty() {
            return Err(AppError::new(
                ErrorCode::PlaceholdersUnconfirmed,
                format!("confirm these first: {}", pending.join(", ")),
            ));
        }

        let agents_md = template::fill(&body, |m| {
            confirmed.values.get(&m.key).cloned().unwrap_or_default()
        });
        if template::contains_marker(&agents_md) {
            return Err(AppError::internal(
                "the generated AGENTS.md still contains [[ FILL IN",
            ));
        }
        let existing = repo_write::read_to_string(&repo, AGENTS_FILE)?;
        let differs = existing.as_deref().is_some_and(|e| e != agents_md);
        if differs && !options.overwrite {
            return Err(AppError::new(
                ErrorCode::AgentsMdExists,
                "an AGENTS.md that differs already exists; generate with overwrite to replace \
                 it (it's backed up to .foundry/backups/ first)",
            ));
        }

        // Plan the docs/design copies before writing anything.
        let families = parse_families(
            confirmed
                .values
                .get(DESIGN_FAMILY_KEY)
                .map_or("", String::as_str),
        );
        let library = Self::library_view(&self.load_index()?);
        let matching: Vec<&DesignFile> = library
            .files
            .iter()
            .filter(|f| f.families.iter().any(|tag| families.contains(tag)))
            .collect();
        let unmatched: Vec<String> = families
            .iter()
            .filter(|tag| !library.files.iter().any(|f| f.families.contains(tag)))
            .cloned()
            .collect();
        let mut sources: Vec<(String, Vec<u8>)> = Vec::new();
        for file in &matching {
            check_filename(&file.filename)?;
            sources.push((
                file.filename.clone(),
                fs::read(self.library.join(&file.filename))?,
            ));
        }

        // Write: backup, AGENTS.md, copies, manifest, the record of copies.
        let mut backup = None;
        if differs {
            let rel = format!("{BACKUP_DIR}/AGENTS.{}.md", now_ms());
            repo_write::write(&repo, &rel, existing.as_deref().unwrap_or_default())?;
            backup = Some(rel);
        }
        let agents_md_written = existing.as_deref() != Some(agents_md.as_str());
        if agents_md_written {
            repo_write::write(&repo, AGENTS_FILE, &agents_md)?;
        }
        for (name, bytes) in &sources {
            repo_write::write(&repo, &format!("{DESIGN_DIR}/{name}"), bytes)?;
        }
        let copied: Vec<String> = sources.iter().map(|(n, _)| n.clone()).collect();
        let previously: Vec<String> = repo_write::read_to_string(&repo, COPIED_FILE)?
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let mut removed = Vec::new();
        for stale in previously.iter().filter(|p| !copied.contains(p)) {
            if check_filename(stale).is_ok()
                && repo_write::remove(&repo, &format!("{DESIGN_DIR}/{stale}"))?
            {
                removed.push(stale.clone());
            }
        }
        repo_write::write(&repo, MANIFEST_FILE, manifest(&families, &matching))?;
        let record = serde_json::to_string_pretty(&copied)
            .map_err(|err| AppError::internal(format!("serialize the copy record: {err}")))?;
        repo_write::write(&repo, COPIED_FILE, record + "\n")?;

        tracing::info!(
            project_id,
            copied = copied.len(),
            agents_md_written,
            "AGENTS.md generated"
        );
        Ok(GenerateAgentsResult {
            agents_md_written,
            backup,
            design_copy_skipped: copied.is_empty(),
            design_families: families,
            copied,
            unmatched_families: unmatched,
            removed,
        })
    }
}

/// `docs/design/manifest.md`: what AGENTS.md Section 6 routes through. Deterministic.
fn manifest(families: &[String], files: &[&DesignFile]) -> String {
    let mut out = String::from(
        "# Design manifest\n\n\
         <!-- Generated by Foundry from your Design Library. Edits here are overwritten. -->\n\n\
         Design guideline files for this project, in `docs/design/`. Read this first; open a \
         file only when a UI-touching slice needs what its description says it covers \
         (AGENTS.md, Section 6).\n\n",
    );
    let family_line = if families.is_empty() {
        "(none)".to_string()
    } else {
        families.join(", ")
    };
    out.push_str(&format!("Project design family: {family_line}\n\n"));
    if files.is_empty() {
        out.push_str("No Design Library file matches this project's design family.\n");
        return out;
    }
    for file in files {
        let description = if file.description.is_empty() {
            "(no description)"
        } else {
            &file.description
        };
        out.push_str(&format!(
            "- `{}` (families: {}): {}\n",
            file.filename,
            file.families.join(", "),
            description.split_whitespace().collect::<Vec<_>>().join(" ")
        ));
    }
    out
}

#[cfg(test)]
mod tests;
