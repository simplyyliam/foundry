//! Which folder each registered project lives in. The projects feature keeps it current;
//! other features look project ids up here instead of reaching into projects.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use super::{AppError, ErrorCode};

/// Project id → repo folder. Share it as `Arc<RepoIndex>`.
#[derive(Debug, Default)]
pub struct RepoIndex {
    repos: RwLock<HashMap<String, PathBuf>>,
    /// Ids of archived projects (agents can't start in them).
    archived: RwLock<HashSet<String>>,
}

impl RepoIndex {
    /// Replaces the whole index (after the registry was saved).
    pub fn replace_all<'a>(&self, entries: impl IntoIterator<Item = (&'a str, &'a Path)>) {
        let next = entries
            .into_iter()
            .map(|(id, path)| (id.to_string(), path.to_path_buf()))
            .collect();
        match self.repos.write() {
            Ok(mut repos) => *repos = next,
            Err(poisoned) => *poisoned.into_inner() = next,
        }
    }

    /// Replaces the set of archived project ids.
    pub fn set_archived<'a>(&self, ids: impl IntoIterator<Item = &'a str>) {
        let next: HashSet<String> = ids.into_iter().map(str::to_string).collect();
        match self.archived.write() {
            Ok(mut archived) => *archived = next,
            Err(poisoned) => *poisoned.into_inner() = next,
        }
    }

    pub fn is_archived(&self, id: &str) -> bool {
        match self.archived.read() {
            Ok(archived) => archived.contains(id),
            Err(poisoned) => poisoned.into_inner().contains(id),
        }
    }

    /// The project's repo folder: `not_found` for an unknown id, `folder_missing` if the
    /// folder is gone.
    pub fn repo(&self, id: &str) -> Result<PathBuf, AppError> {
        let repos = self
            .repos
            .read()
            .map_err(|_| AppError::internal("repo index is poisoned"))?;
        let path = repos
            .get(id)
            .cloned()
            .ok_or_else(|| AppError::not_found(format!("no project with id {id}")))?;
        if !path.is_dir() {
            return Err(AppError::new(
                ErrorCode::FolderMissing,
                format!("project folder is missing: {}", path.display()),
            ));
        }
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRepo;

    #[test]
    fn looks_up_known_ids_and_reports_unknown_or_missing() {
        let repo = TestRepo::new();
        let gone = repo.path().join("gone");
        let index = RepoIndex::default();
        index.replace_all([("a", repo.path()), ("b", gone.as_path())]);

        assert_eq!(index.repo("a").unwrap(), repo.path());
        assert_eq!(index.repo("b").unwrap_err().code, ErrorCode::FolderMissing);
        assert_eq!(index.repo("c").unwrap_err().code, ErrorCode::NotFound);

        index.replace_all([]);
        assert_eq!(index.repo("a").unwrap_err().code, ErrorCode::NotFound);
    }

    #[test]
    fn tracks_archived_ids() {
        let index = RepoIndex::default();
        assert!(!index.is_archived("a"));
        index.set_archived(["a", "b"]);
        assert!(index.is_archived("a") && index.is_archived("b"));
        index.set_archived(["b"]);
        assert!(!index.is_archived("a"));
    }
}
