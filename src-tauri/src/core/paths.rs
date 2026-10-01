//! Validating untrusted paths, and joining relative ones onto a trusted root.

use std::io;
use std::path::{Component, Path, PathBuf};

use super::AppError;

/// The kind of path a caller accepts from outside the backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathKind {
    /// Relative to a root the caller supplies (see [`resolve`]).
    Relative,
    /// A full path that stands on its own, e.g. a folder the user browses to.
    Absolute,
}

/// Lexical checks shared by every untrusted path. Never touches the filesystem.
///
/// - `Relative`: rejects absolute paths (including drive and UNC prefixes) and any `..`
///   component, both as `path_escape` since either would leave the root. Returns only the
///   normal components (`.` dropped).
/// - `Absolute`: requires an absolute path and rejects any `..` component, both as
///   `invalid_path` since there is no root to escape. Returns the path with `.` and trailing
///   separators dropped.
pub fn check(path: &Path, kind: PathKind) -> Result<PathBuf, AppError> {
    let parent_dir = || format!("path must not contain '..': {}", path.display());
    match kind {
        PathKind::Relative => {
            let mut parts = PathBuf::new();
            for component in path.components() {
                match component {
                    Component::Normal(part) => parts.push(part),
                    Component::CurDir => {}
                    Component::ParentDir => return Err(AppError::path_escape(parent_dir())),
                    Component::RootDir | Component::Prefix(_) => {
                        return Err(AppError::path_escape(format!(
                            "path must be relative: {}",
                            path.display()
                        )))
                    }
                }
            }
            Ok(parts)
        }
        PathKind::Absolute => {
            if !path.is_absolute() {
                return Err(AppError::invalid_path(format!(
                    "path must be absolute: {}",
                    path.display()
                )));
            }
            if path.components().any(|c| c == Component::ParentDir) {
                return Err(AppError::invalid_path(parent_dir()));
            }
            Ok(path.components().collect())
        }
    }
}

/// Joins `rel` onto `root` and returns the resolved path, guaranteed to sit inside `root`.
///
/// Rejects what [`check`] rejects for [`PathKind::Relative`], plus any existing symlink or
/// junction along the way that resolves outside `root` (or does not resolve at all).
/// Components that do not exist yet are joined as-is. `root` must exist.
pub fn resolve(root: &Path, rel: impl AsRef<Path>) -> Result<PathBuf, AppError> {
    let rel = rel.as_ref();
    let parts = check(rel, PathKind::Relative)?;

    let root = root.canonicalize()?;
    let mut current = root.clone();
    let mut parts = parts.components();
    for part in parts.by_ref() {
        let next = current.join(part);
        match next.symlink_metadata() {
            Ok(_) => {
                let real = next.canonicalize().map_err(|_| {
                    AppError::path_escape(format!("link does not resolve: {}", rel.display()))
                })?;
                if !real.starts_with(&root) {
                    return Err(AppError::path_escape(format!(
                        "path resolves outside the root: {}",
                        rel.display()
                    )));
                }
                current = real;
            }
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                current = next;
                break;
            }
            Err(err) => return Err(err.into()),
        }
    }
    current.extend(parts);
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorCode;
    use crate::testing::{link_dir, TestRepo};

    fn assert_escape(result: Result<PathBuf, AppError>) {
        assert_eq!(result.unwrap_err().code, ErrorCode::PathEscape);
    }

    #[test]
    fn joins_plain_relative_paths() {
        let repo = TestRepo::new();
        repo.write("docs/a.md", "x");
        let root = repo.path().canonicalize().unwrap();
        assert_eq!(
            resolve(repo.path(), "docs/a.md").unwrap(),
            root.join("docs").join("a.md")
        );
        assert_eq!(
            resolve(repo.path(), "./docs/new/b.md").unwrap(),
            root.join("docs").join("new").join("b.md")
        );
        assert_eq!(resolve(repo.path(), "").unwrap(), root);
    }

    #[test]
    fn rejects_parent_dir() {
        let repo = TestRepo::new();
        assert_escape(resolve(repo.path(), ".."));
        assert_escape(resolve(repo.path(), "../outside.txt"));
        assert_escape(resolve(repo.path(), "docs/../../outside.txt"));
        // Rejected even when it would stay inside the root.
        assert_escape(resolve(repo.path(), "docs/../a.txt"));
    }

    #[test]
    fn rejects_absolute_paths() {
        let repo = TestRepo::new();
        let absolute = repo.path().join("a.txt");
        assert_escape(resolve(repo.path(), &absolute));
        assert_escape(resolve(repo.path(), "/etc/passwd"));
        #[cfg(windows)]
        {
            assert_escape(resolve(repo.path(), r"C:\Windows"));
            assert_escape(resolve(repo.path(), r"\Windows"));
            assert_escape(resolve(repo.path(), r"C:relative"));
            assert_escape(resolve(repo.path(), r"\\server\share\x"));
        }
    }

    #[test]
    fn rejects_symlink_escapes() {
        let repo = TestRepo::new();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("secret.txt"), "s").unwrap();
        link_dir(outside.path(), &repo.path().join("escape"));

        assert_escape(resolve(repo.path(), "escape"));
        assert_escape(resolve(repo.path(), "escape/secret.txt"));
        assert_escape(resolve(repo.path(), "escape/new/file.txt"));
    }

    #[test]
    fn rejects_dangling_links() {
        let repo = TestRepo::new();
        let outside = tempfile::tempdir().unwrap();
        let target = outside.path().join("gone");
        std::fs::create_dir(&target).unwrap();
        link_dir(&target, &repo.path().join("dangling"));
        std::fs::remove_dir(&target).unwrap();

        assert_escape(resolve(repo.path(), "dangling/file.txt"));
    }

    #[test]
    fn allows_symlinks_that_stay_inside() {
        let repo = TestRepo::new();
        repo.write("real/a.txt", "a");
        link_dir(&repo.path().join("real"), &repo.path().join("alias"));

        let resolved = resolve(repo.path(), "alias/a.txt").unwrap();
        let root = repo.path().canonicalize().unwrap();
        assert_eq!(resolved, root.join("real").join("a.txt"));
    }

    /// Pins `resolve`'s behavior (codes, messages, canonical output) across the extraction of
    /// its lexical checks into `check`.
    #[test]
    fn resolve_behavior_is_unchanged_after_extracting_check() {
        let repo = TestRepo::new();
        let root = repo.path().canonicalize().unwrap();
        repo.write("inside/a.txt", "a");
        let outside = tempfile::tempdir().unwrap();
        link_dir(outside.path(), &repo.path().join("escape"));
        link_dir(&repo.path().join("inside"), &repo.path().join("alias"));

        // Absolute paths: rejected as escapes, with the same message as before.
        let absolute = repo.path().join("a.txt");
        let err = resolve(repo.path(), &absolute).unwrap_err();
        assert_eq!(err.code, ErrorCode::PathEscape);
        assert_eq!(
            err.message,
            format!("path must be relative: {}", absolute.display())
        );

        // `..`: rejected as an escape, same message.
        let err = resolve(repo.path(), "a/../b").unwrap_err();
        assert_eq!(err.code, ErrorCode::PathEscape);
        assert_eq!(err.message, "path must not contain '..': a/../b");

        // Output is canonical: rooted at the canonical root, `.` dropped, inside links followed.
        assert_eq!(
            resolve(repo.path(), "./new/./b.md").unwrap(),
            root.join("new").join("b.md")
        );
        assert_eq!(
            resolve(repo.path(), "alias/a.txt").unwrap(),
            root.join("inside").join("a.txt")
        );

        // Out-of-root links: rejected as escapes, same message.
        let err = resolve(repo.path(), "escape/secret.txt").unwrap_err();
        assert_eq!(err.code, ErrorCode::PathEscape);
        assert_eq!(
            err.message,
            "path resolves outside the root: escape/secret.txt"
        );
    }

    #[test]
    fn missing_root_is_an_io_error() {
        let repo = TestRepo::new();
        let err = resolve(&repo.path().join("missing"), "a.txt").unwrap_err();
        assert_eq!(err.code, ErrorCode::Io);
    }
}
