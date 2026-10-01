//! Read-only folder browsing for the frontend's folder picker. Never writes.

pub mod commands;

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use specta::Type;

use crate::core::paths::{self, PathKind};
use crate::core::AppError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderEntry {
    pub name: String,
    pub path: String,
    /// Has a `.git` directory or file directly inside it.
    pub is_git_repo: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderListing {
    /// The listed folder, normalized (no trailing separator, no `.`).
    pub path: String,
    /// `None` at a filesystem root.
    pub parent: Option<String>,
    pub is_git_repo: bool,
    /// Visible subfolders, sorted case-insensitively. Files and hidden folders are omitted.
    pub folders: Vec<FolderEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderLocations {
    pub home: String,
    /// Filesystem roots: drive letters on Windows (`C:\`), `/` elsewhere.
    pub roots: Vec<String>,
}

/// Lists the visible subfolders of `path`, or of the home folder when `path` is `None`.
pub fn list(path: Option<&str>) -> Result<FolderListing, AppError> {
    let dir = match path {
        Some(path) => paths::check(Path::new(path), PathKind::Absolute)?,
        None => home_dir()?,
    };

    let mut folders: Vec<FolderEntry> = fs::read_dir(&dir)?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            // Follows symlinks and junctions, so linked folders show up as folders.
            let metadata = fs::metadata(&path).ok()?;
            if !metadata.is_dir() || is_hidden(&entry.file_name().to_string_lossy(), &metadata) {
                return None;
            }
            Some(FolderEntry {
                name: entry.file_name().to_string_lossy().into_owned(),
                is_git_repo: is_git_repo(&path),
                path: path.to_string_lossy().into_owned(),
            })
        })
        .collect();
    folders.sort_by_cached_key(|folder| folder.name.to_lowercase());

    Ok(FolderListing {
        path: dir.to_string_lossy().into_owned(),
        parent: dir.parent().map(|p| p.to_string_lossy().into_owned()),
        is_git_repo: is_git_repo(&dir),
        folders,
    })
}

pub fn locations() -> Result<FolderLocations, AppError> {
    Ok(FolderLocations {
        home: home_dir()?.to_string_lossy().into_owned(),
        roots: roots(),
    })
}

fn home_dir() -> Result<PathBuf, AppError> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| AppError::internal(format!("home folder unknown: {var} is not set")))
}

fn roots() -> Vec<String> {
    if cfg!(windows) {
        ('A'..='Z')
            .map(|letter| format!("{letter}:\\"))
            .filter(|root| Path::new(root).is_dir())
            .collect()
    } else {
        vec!["/".to_string()]
    }
}

fn is_git_repo(dir: &Path) -> bool {
    // A file (not a dir) for worktrees and submodules.
    dir.join(".git").exists()
}

fn is_hidden(name: &str, metadata: &fs::Metadata) -> bool {
    if name.starts_with('.') {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const HIDDEN: u32 = 0x2;
        const SYSTEM: u32 = 0x4;
        if metadata.file_attributes() & (HIDDEN | SYSTEM) != 0 {
            return true;
        }
    }
    #[cfg(not(windows))]
    let _ = metadata;
    false
}

#[cfg(test)]
mod tests;
