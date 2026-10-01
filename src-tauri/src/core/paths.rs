//! Joining untrusted relative paths onto a trusted root.

use std::io;
use std::path::{Component, Path, PathBuf};

use super::AppError;

/// Joins `rel` onto `root` and returns the resolved path, guaranteed to sit inside `root`.
///
/// Rejects absolute paths (including drive and UNC prefixes), any `..` component, and any
/// existing symlink or junction along the way that resolves outside `root` (or does not
/// resolve at all). Components that do not exist yet are joined as-is. `root` must exist.
pub fn resolve(root: &Path, rel: impl AsRef<Path>) -> Result<PathBuf, AppError> {
    let rel = rel.as_ref();
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(part) => parts.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                return Err(AppError::path_escape(format!(
                    "path must not contain '..': {}",
                    rel.display()
                )))
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err(AppError::path_escape(format!(
                    "path must be relative: {}",
                    rel.display()
                )))
            }
        }
    }

    let root = root.canonicalize()?;
    let mut current = root.clone();
    let mut parts = parts.into_iter();
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

    #[test]
    fn missing_root_is_an_io_error() {
        let repo = TestRepo::new();
        let err = resolve(&repo.path().join("missing"), "a.txt").unwrap_err();
        assert_eq!(err.code, ErrorCode::Io);
    }
}
