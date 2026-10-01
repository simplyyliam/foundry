//! The only way to write into a user's project repo. Enforces the AGENTS.md allowlist on top
//! of [`paths::resolve`] (no `..`, no absolute paths, no symlink escapes), and writes atomically.
//!
//! Not covered here, by design: creating the repo folder itself and running `git` in it, which
//! only `create_repository` does (see the AGENTS.md exception).

use std::fs;
use std::path::{Path, PathBuf};

use super::paths::{self, PathKind};
use super::{fsx, AppError, ErrorCode};

/// Folders whose whole contents Foundry may write.
const ALLOWED_DIRS: &[&str] = &[".foundry", "docs/bakery", "docs/design", "agents"];

/// Single files Foundry may write.
const ALLOWED_FILES: &[&str] = &[
    "docs/brain-dump.md",
    "AGENTS.md",
    "vsm_data.js",
    "vsm_viewer.html",
];

/// Allowed only as a JSON merge, never a plain overwrite (no merge writer exists yet).
const MERGE_ONLY: &[&str] = &[".claude/settings.local.json"];

/// Whether a repo-relative path is on the allowlist for plain writes.
/// Matching is exact and case-sensitive, on normalized components.
pub fn is_allowed(rel: &Path) -> bool {
    let Ok(rel) = paths::check(rel, PathKind::Relative) else {
        return false;
    };
    let in_dir = ALLOWED_DIRS.iter().any(|dir| {
        let dir = Path::new(dir);
        rel.starts_with(dir) && rel.components().count() > dir.components().count()
    });
    in_dir || ALLOWED_FILES.iter().any(|file| rel == Path::new(file))
}

fn guard(repo: &Path, rel: &str) -> Result<PathBuf, AppError> {
    let rel_path = Path::new(rel);
    if MERGE_ONLY
        .iter()
        .any(|file| paths::check(rel_path, PathKind::Relative).is_ok_and(|r| r == Path::new(file)))
    {
        return Err(AppError::new(
            ErrorCode::WriteNotAllowed,
            format!("{rel} may only be merged, never overwritten"),
        ));
    }
    // Path checks first, so `..` and absolute paths report as escapes, not as allowlist misses.
    let target = paths::resolve(repo, rel_path)?;
    if !is_allowed(rel_path) {
        return Err(AppError::new(
            ErrorCode::WriteNotAllowed,
            format!("Foundry doesn't write {rel} in a project repo"),
        ));
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(target)
}

/// Writes (or replaces) an allowlisted file in `repo`, atomically.
pub fn write(repo: &Path, rel: &str, contents: impl AsRef<[u8]>) -> Result<(), AppError> {
    let target = guard(repo, rel)?;
    fsx::write_atomic(&target, contents)
}

/// Writes an allowlisted file only if it doesn't exist yet. Returns whether it was written.
pub fn write_if_missing(
    repo: &Path,
    rel: &str,
    contents: impl AsRef<[u8]>,
) -> Result<bool, AppError> {
    let target = guard(repo, rel)?;
    fsx::write_atomic_new(&target, contents)
}

/// Reads an allowlisted file through the same path checks as writes.
/// Returns `None` when it doesn't exist.
pub fn read_to_string(repo: &Path, rel: &str) -> Result<Option<String>, AppError> {
    let target = paths::resolve(repo, rel)?;
    match fs::read_to_string(target) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRepo;

    fn assert_not_allowed(result: Result<(), AppError>) {
        assert_eq!(result.unwrap_err().code, ErrorCode::WriteNotAllowed);
    }

    #[test]
    fn allows_every_allowlisted_location() {
        let repo = TestRepo::new();
        for rel in [
            ".foundry/project.json",
            ".foundry/runs/1/log.jsonl",
            "docs/bakery/a.md",
            "docs/design/tokens/colors.json",
            "docs/brain-dump.md",
            "agents/backend.md",
            "AGENTS.md",
            "vsm_data.js",
            "vsm_viewer.html",
        ] {
            write(repo.path(), rel, "x").unwrap_or_else(|e| panic!("{rel}: {e}"));
            assert_eq!(repo.read(rel), "x", "{rel}");
        }
    }

    #[test]
    fn rejects_writes_outside_the_allowlist_and_touches_nothing() {
        let repo = TestRepo::with_git();
        repo.write("src/main.rs", "fn main() {}");
        let before = std::fs::read(repo.path().join(".git/config")).unwrap();

        for rel in [
            "src/main.rs",
            "README.md",
            ".gitignore",
            ".git/config",
            ".git/hooks/pre-commit",
            "docs/other.md",
            "docs/bakery",
            "docs",
            ".foundry",
            "agents",
            "agents.md",
            "Agents.md",
            ".claude/settings.json",
        ] {
            assert_not_allowed(write(repo.path(), rel, "x"));
        }
        assert_eq!(repo.read("src/main.rs"), "fn main() {}");
        assert_eq!(
            std::fs::read(repo.path().join(".git/config")).unwrap(),
            before
        );
        assert!(!repo.path().join("README.md").exists());
        assert!(!repo.path().join("docs").exists(), "no parent dirs created");
    }

    #[test]
    fn settings_local_is_merge_only() {
        let repo = TestRepo::new();
        repo.write(".claude/settings.local.json", "{\"keep\":true}");
        assert_not_allowed(write(repo.path(), ".claude/settings.local.json", "{}"));
        assert_eq!(repo.read(".claude/settings.local.json"), "{\"keep\":true}");
    }

    #[test]
    fn path_escapes_are_rejected_before_the_allowlist() {
        let repo = TestRepo::new();
        for rel in ["../AGENTS.md", ".foundry/../../x", "/tmp/AGENTS.md"] {
            assert_eq!(
                write(repo.path(), rel, "x").unwrap_err().code,
                ErrorCode::PathEscape,
                "{rel}"
            );
        }
    }

    #[test]
    fn symlinked_allowlisted_dir_cannot_escape() {
        let repo = TestRepo::new();
        let outside = tempfile::tempdir().unwrap();
        crate::testing::link_dir(outside.path(), &repo.path().join(".foundry"));
        assert_eq!(
            write(repo.path(), ".foundry/project.json", "x")
                .unwrap_err()
                .code,
            ErrorCode::PathEscape
        );
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
    }

    #[test]
    fn write_if_missing_never_replaces() {
        let repo = TestRepo::new();
        assert!(write_if_missing(repo.path(), "vsm_viewer.html", "a").unwrap());
        assert!(!write_if_missing(repo.path(), "vsm_viewer.html", "b").unwrap());
        assert_eq!(repo.read("vsm_viewer.html"), "a");
    }

    #[test]
    fn reads_return_none_for_missing_files() {
        let repo = TestRepo::new();
        assert_eq!(
            read_to_string(repo.path(), "docs/brain-dump.md").unwrap(),
            None
        );
        repo.write("docs/brain-dump.md", "hi");
        assert_eq!(
            read_to_string(repo.path(), "docs/brain-dump.md").unwrap(),
            Some("hi".into())
        );
    }
}
