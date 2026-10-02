//! The files Foundry puts in a project repo, and the folder name for a new one.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::{repo_write, AppError};

/// Copied byte-for-byte to `<repo>/vsm_viewer.html`, and never over an existing copy.
pub const VSM_VIEWER: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/vsm_viewer.html"
));

pub const PROJECT_FILE: &str = ".foundry/project.json";
pub const BRAIN_DUMP: &str = "docs/brain-dump.md";
pub const VSM_VIEWER_FILE: &str = "vsm_viewer.html";

/// `.foundry/project.json`. Unknown fields written by later phases or agents are preserved
/// by [`update_identity`], which edits the JSON in place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub tagline: String,
    /// Epoch ms.
    pub created_at: u64,
    #[serde(default)]
    pub test_commands: Vec<String>,
    #[serde(default)]
    pub design_family: Vec<String>,
}

fn to_json(file: &ProjectFile) -> Result<String, AppError> {
    serde_json::to_string_pretty(file)
        .map(|json| json + "\n")
        .map_err(|err| AppError::internal(format!("serialize project.json: {err}")))
}

/// Creates whichever of `.foundry/project.json`, `docs/brain-dump.md` and `vsm_viewer.html`
/// are missing, leaving existing ones byte-identical. Returns the files it created.
pub fn ensure(repo: &Path, file: &ProjectFile) -> Result<Vec<String>, AppError> {
    let mut created = Vec::new();
    let files: [(&str, Vec<u8>); 3] = [
        (PROJECT_FILE, to_json(file)?.into_bytes()),
        (BRAIN_DUMP, Vec::new()),
        (VSM_VIEWER_FILE, VSM_VIEWER.to_vec()),
    ];
    for (rel, contents) in files {
        if repo_write::write_if_missing(repo, rel, contents)? {
            created.push(rel.to_string());
        }
    }
    Ok(created)
}

/// Reads `.foundry/project.json`. `None` if it's missing or isn't a Foundry project file.
pub fn read(repo: &Path) -> Result<Option<ProjectFile>, AppError> {
    Ok(repo_write::read_to_string(repo, PROJECT_FILE)?
        .and_then(|text| serde_json::from_str(&text).ok()))
}

/// Sets `name` and `tagline` in `.foundry/project.json`, keeping every other field as is.
/// Recreates the file from `fallback` if it's missing or unreadable.
pub fn update_identity(repo: &Path, fallback: &ProjectFile) -> Result<(), AppError> {
    let existing = repo_write::read_to_string(repo, PROJECT_FILE)?
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .filter(serde_json::Value::is_object);
    let json = match existing {
        Some(mut value) => {
            value["name"] = fallback.name.clone().into();
            value["tagline"] = fallback.tagline.clone().into();
            serde_json::to_string_pretty(&value)
                .map(|json| json + "\n")
                .map_err(|err| AppError::internal(format!("serialize project.json: {err}")))?
        }
        None => to_json(fallback)?,
    };
    repo_write::write(repo, PROJECT_FILE, json)
}

const RESERVED: &[&str] = &[
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

/// Folder name for a new repo: lowercase letters and digits, other runs become `-`.
/// "My Cool App!" → "my-cool-app". Rejects names that leave nothing usable.
pub fn slug(name: &str) -> Result<String, AppError> {
    let slug = crate::core::slug::slugify(name);
    if slug.is_empty() {
        return Err(AppError::invalid_input(format!(
            "“{}” has no letters or digits to name a folder with",
            name.trim()
        )));
    }
    if RESERVED.contains(&slug.as_str()) {
        return Err(AppError::invalid_input(format!(
            "“{slug}” is reserved by Windows; choose another name"
        )));
    }
    Ok(slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs() {
        assert_eq!(slug("My Cool App!").unwrap(), "my-cool-app");
        assert_eq!(slug("  orbit  ").unwrap(), "orbit");
        assert_eq!(slug("a__b--c").unwrap(), "a-b-c");
        assert_eq!(slug("Café 2").unwrap(), "café-2");
        assert!(slug("!!!").is_err());
        assert!(slug("").is_err());
        assert!(slug("CON").is_err());
    }
}
