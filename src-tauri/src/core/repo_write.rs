//! The only way to write into a user's project repo. Enforces the AGENTS.md allowlist on top
//! of [`paths::resolve`] (no `..`, no absolute paths, no symlink escapes), and writes atomically.
//! One more narrow writer, [`add_git_excludes`], only ever adds lines to `.git/info/exclude`.
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

/// Allowed only as a JSON merge through [`merge_json`], never a plain overwrite.
const MERGE_ONLY: &[&str] = &[".claude/settings.local.json"];

/// Where [`merge_json`] keeps the previous version of a file it changes.
pub const BACKUP_DIR: &str = ".foundry/backups";

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

/// Appends to an allowlisted file, creating it if needed. Not atomic like [`write`]: for
/// append-only logs (one JSON line per call), where rewriting the whole file per line would
/// cost O(size). `contents` goes out in a single `write_all`; a crash can leave at most one
/// torn final line, which readers of such logs must skip.
pub fn append(repo: &Path, rel: &str, contents: impl AsRef<[u8]>) -> Result<(), AppError> {
    use std::io::Write as _;
    let target = guard(repo, rel)?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(target)?;
    file.write_all(contents.as_ref())?;
    Ok(())
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

/// Deletes an allowlisted file (never a folder). Returns `false` if it didn't exist.
pub fn remove(repo: &Path, rel: &str) -> Result<bool, AppError> {
    let target = guard(repo, rel)?;
    match fs::symlink_metadata(&target) {
        Ok(meta) if meta.is_file() => {
            fs::remove_file(&target)?;
            Ok(true)
        }
        Ok(_) => Err(AppError::invalid_path(format!("not a file: {rel}"))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => Err(err.into()),
    }
}

/// Renames one allowlisted file to another, never replacing an existing `to`: returns
/// `false` (and changes nothing) if `to` exists. `from` must exist.
pub fn rename_new(repo: &Path, from: &str, to: &str) -> Result<bool, AppError> {
    let source = guard(repo, from)?;
    let target = guard(repo, to)?;
    // A hard link fails if the destination exists, so the check and the move are one step.
    match fs::hard_link(&source, &target) {
        Ok(()) => {
            if let Err(err) = fs::remove_file(&source) {
                let _ = fs::remove_file(&target);
                return Err(err.into());
            }
            Ok(true)
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
        // Filesystems without hard links (FAT, some network drives): check, then rename.
        Err(_) if target.symlink_metadata().is_ok() => Ok(false),
        Err(_) => {
            fs::rename(&source, &target)?;
            Ok(true)
        }
    }
}

/// What [`merge_json`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged {
    /// Whether the file changed. An idempotent merge leaves it (and makes no backup).
    pub changed: bool,
    /// The previous file's copy under [`BACKUP_DIR`] (repo-relative), when one was replaced.
    pub backup: Option<String>,
}

const BOM: &str = "\u{feff}";

/// Merges into a merge-only file (`.claude/settings.local.json`): reads it (a missing file
/// reads as `{}`), lets `merge` change the parsed object, and only if that changed anything,
/// backs the old file up under [`BACKUP_DIR`] and writes the result atomically. A file that
/// isn't valid JSON, or isn't an object, is refused (`invalid_json`) and never touched. An
/// error from `merge` writes nothing. A UTF-8 BOM is kept.
pub fn merge_json(
    repo: &Path,
    rel: &str,
    merge: impl FnOnce(&mut serde_json::Value) -> Result<(), AppError>,
) -> Result<Merged, AppError> {
    let rel_path = paths::check(Path::new(rel), PathKind::Relative)?;
    if !MERGE_ONLY.iter().any(|file| rel_path == Path::new(file)) {
        return Err(AppError::new(
            ErrorCode::WriteNotAllowed,
            format!("{rel} isn't a file Foundry merges into"),
        ));
    }
    let target = paths::resolve(repo, &rel_path)?;
    let original = match fs::read(&target) {
        Ok(bytes) => Some(bytes),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(err.into()),
    };
    let invalid = |why: String| AppError::new(ErrorCode::InvalidJson, why);
    let (current, bom) = match &original {
        None => (serde_json::Value::Object(Default::default()), false),
        Some(bytes) => {
            let text = std::str::from_utf8(bytes)
                .map_err(|_| invalid(format!("{rel} isn't UTF-8 text; it was left untouched")))?;
            let value: serde_json::Value = serde_json::from_str(text.trim_start_matches(BOM))
                .map_err(|err| {
                    invalid(format!(
                        "{rel} isn't valid JSON ({err}); it was left untouched. Fix it by hand first"
                    ))
                })?;
            if !value.is_object() {
                return Err(invalid(format!(
                    "{rel} must hold a JSON object; it was left untouched"
                )));
            }
            (value, text.starts_with(BOM))
        }
    };
    let mut next = current.clone();
    merge(&mut next)?;
    let nothing_new = original.is_none() && next.as_object().is_some_and(|m| m.is_empty());
    if next == current || nothing_new {
        return Ok(Merged {
            changed: false,
            backup: None,
        });
    }
    let backup = match &original {
        Some(bytes) => Some(back_up(repo, &rel_path, bytes)?),
        None => None,
    };
    let mut text = serde_json::to_string_pretty(&next)
        .map_err(|err| AppError::internal(format!("serialize {rel}: {err}")))?;
    text.push('\n');
    if bom {
        text.insert_str(0, BOM);
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fsx::write_atomic(&target, text)?;
    Ok(Merged {
        changed: true,
        backup,
    })
}

/// `.foundry/backups/<name>.<ms>.json` (`-2`, `-3`, … if taken), never replacing a backup.
fn back_up(repo: &Path, rel: &Path, bytes: &[u8]) -> Result<String, AppError> {
    let stem = rel
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "file".into());
    let ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    for n in 1..100 {
        let suffix = if n == 1 {
            String::new()
        } else {
            format!("-{n}")
        };
        let backup = format!("{BACKUP_DIR}/{stem}.{ms}{suffix}.json");
        if write_if_missing(repo, &backup, bytes)? {
            return Ok(backup);
        }
    }
    Err(AppError::internal("could not pick a backup file name"))
}

/// The repo's local, uncommitted ignore list. Never a plain write: only [`add_git_excludes`]
/// touches it, and only by adding lines.
pub const GIT_EXCLUDE: &str = ".git/info/exclude";

/// Adds each of `lines` that isn't already a line of `.git/info/exclude`, creating the file
/// if needed. Existing lines stay byte-identical (and keep their CRLF endings, which new
/// lines then follow). Returns the lines added. A folder whose `.git` isn't a directory (no
/// git, or a worktree's `.git` file) is left alone: nothing is added.
pub fn add_git_excludes(repo: &Path, lines: &[&str]) -> Result<Vec<String>, AppError> {
    if !repo
        .join(".git")
        .symlink_metadata()
        .is_ok_and(|m| m.is_dir())
    {
        return Ok(Vec::new());
    }
    let target = paths::resolve(repo, GIT_EXCLUDE)?;
    let mut text = match fs::read(&target) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| {
            AppError::invalid_input(format!("{GIT_EXCLUDE} isn't UTF-8 text; left untouched"))
        })?,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(err) => return Err(err.into()),
    };
    let missing: Vec<String> = lines
        .iter()
        .filter(|line| !text.lines().any(|have| have.trim_end() == **line))
        .map(|line| line.to_string())
        .collect();
    if missing.is_empty() {
        return Ok(missing);
    }
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    if !text.is_empty() && !text.ends_with('\n') {
        text.push_str(eol);
    }
    for line in &missing {
        text.push_str(line);
        text.push_str(eol);
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    fsx::write_atomic(&target, text)?;
    Ok(missing)
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

    #[test]
    fn remove_and_rename_stay_inside_the_allowlist() {
        let repo = TestRepo::new();
        write(repo.path(), "docs/bakery/a.png", "a").unwrap();
        write(repo.path(), "docs/bakery/b.png", "b").unwrap();
        fs::write(repo.path().join("keep.txt"), "keep").unwrap();

        assert!(!rename_new(repo.path(), "docs/bakery/a.png", "docs/bakery/b.png").unwrap());
        assert_eq!(
            fs::read(repo.path().join("docs/bakery/b.png")).unwrap(),
            b"b"
        );
        assert!(rename_new(repo.path(), "docs/bakery/a.png", "docs/bakery/c.png").unwrap());
        assert!(!repo.path().join("docs/bakery/a.png").exists());
        assert_eq!(
            fs::read(repo.path().join("docs/bakery/c.png")).unwrap(),
            b"a"
        );

        assert!(remove(repo.path(), "docs/bakery/c.png").unwrap());
        assert!(!remove(repo.path(), "docs/bakery/c.png").unwrap());
        assert_eq!(
            remove(repo.path(), "keep.txt").unwrap_err().code,
            ErrorCode::WriteNotAllowed
        );
        assert_eq!(
            remove(repo.path(), "docs/bakery/../../keep.txt")
                .unwrap_err()
                .code,
            ErrorCode::PathEscape
        );
        assert_eq!(
            rename_new(repo.path(), "docs/bakery/b.png", "keep2.txt")
                .unwrap_err()
                .code,
            ErrorCode::WriteNotAllowed
        );
        assert!(repo.path().join("keep.txt").exists());
    }

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
        let rel = ".claude/settings.local.json";
        repo.write(rel, "{\"keep\":true}");
        assert_not_allowed(write(repo.path(), rel, "{}"));
        assert_not_allowed(append(repo.path(), rel, "x"));
        assert_eq!(repo.read(rel), "{\"keep\":true}");

        // The merge writer is the only way in, and it keeps what's there.
        let merged = merge_json(repo.path(), rel, |v| {
            v["added"] = serde_json::json!(1);
            Ok(())
        })
        .unwrap();
        assert!(merged.changed);
        let after: serde_json::Value = serde_json::from_str(&repo.read(rel)).unwrap();
        assert_eq!(after, serde_json::json!({ "keep": true, "added": 1 }));
        let backup = merged.backup.unwrap();
        assert!(
            backup.starts_with(".foundry/backups/settings.local."),
            "{backup}"
        );
        assert_eq!(repo.read(&backup), "{\"keep\":true}");

        // Only merge-only files can be merged.
        for other in ["AGENTS.md", ".claude/settings.json", "src/x.json"] {
            assert_not_allowed(merge_json(repo.path(), other, |_| Ok(())).map(|_| ()));
        }
    }

    #[test]
    fn merge_json_is_idempotent_and_refuses_invalid_json_untouched() {
        let repo = TestRepo::new();
        let rel = ".claude/settings.local.json";
        let set = |v: &mut serde_json::Value| {
            v["a"] = serde_json::json!(1);
            Ok(())
        };
        assert!(merge_json(repo.path(), rel, set).unwrap().changed);
        let first = repo.read(rel);
        let again = merge_json(repo.path(), rel, set).unwrap();
        assert_eq!((again.changed, again.backup), (false, None));
        assert_eq!(repo.read(rel), first);
        assert!(
            !repo.path().join(BACKUP_DIR).exists(),
            "no backup of a file that didn't exist"
        );

        // A missing file and a no-op merge create nothing.
        let empty = TestRepo::new();
        assert!(!merge_json(empty.path(), rel, |_| Ok(())).unwrap().changed);
        assert!(!empty.path().join(".claude").exists());

        for bad in ["{ not json", "[1, 2]", "\"text\""] {
            repo.write(rel, bad);
            let err = merge_json(repo.path(), rel, set).unwrap_err();
            assert_eq!(err.code, ErrorCode::InvalidJson, "{bad}");
            assert_eq!(repo.read(rel), bad, "left untouched");
        }
        // A failing merge writes nothing either.
        repo.write(rel, "{}");
        let err = merge_json(repo.path(), rel, |_| Err(AppError::invalid_input("no"))).unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert_eq!(repo.read(rel), "{}");

        // A BOM survives.
        repo.write(rel, "\u{feff}{\"b\":2}");
        assert!(merge_json(repo.path(), rel, set).unwrap().changed);
        assert!(repo.read(rel).starts_with('\u{feff}'));
    }

    #[test]
    fn merge_json_cannot_escape_through_a_linked_claude_folder() {
        let repo = TestRepo::new();
        let outside = tempfile::tempdir().unwrap();
        crate::testing::link_dir(outside.path(), &repo.path().join(".claude"));
        let err = merge_json(repo.path(), ".claude/settings.local.json", |v| {
            v["a"] = serde_json::json!(1);
            Ok(())
        })
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::PathEscape);
        assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
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
    fn append_adds_to_allowlisted_files_only() {
        let repo = TestRepo::new();
        append(
            repo.path(),
            ".foundry/events/a.jsonl",
            "1
",
        )
        .unwrap();
        append(
            repo.path(),
            ".foundry/events/a.jsonl",
            "2
",
        )
        .unwrap();
        assert_eq!(
            repo.read(".foundry/events/a.jsonl"),
            "1
2
"
        );
        assert_not_allowed(append(repo.path(), "src/log.txt", "x"));
        assert_eq!(
            append(repo.path(), ".foundry/../x", "x").unwrap_err().code,
            ErrorCode::PathEscape
        );
        assert_not_allowed(append(repo.path(), ".claude/settings.local.json", "x"));
    }

    #[test]
    fn write_if_missing_never_replaces() {
        let repo = TestRepo::new();
        assert!(write_if_missing(repo.path(), "vsm_viewer.html", "a").unwrap());
        assert!(!write_if_missing(repo.path(), "vsm_viewer.html", "b").unwrap());
        assert_eq!(repo.read("vsm_viewer.html"), "a");
    }

    const EXCLUDES: &[&str] = &[".foundry/events/", ".foundry/reviews/digest.md"];

    #[test]
    fn git_excludes_are_added_once_and_idempotent() {
        let repo = TestRepo::with_git();
        fs::remove_file(repo.path().join(GIT_EXCLUDE)).ok();
        fs::remove_dir(repo.path().join(".git/info")).ok();

        assert_eq!(add_git_excludes(repo.path(), EXCLUDES).unwrap(), EXCLUDES);
        assert_eq!(
            repo.read(GIT_EXCLUDE),
            ".foundry/events/\n.foundry/reviews/digest.md\n"
        );
        assert!(add_git_excludes(repo.path(), EXCLUDES).unwrap().is_empty());
        assert_eq!(
            repo.read(GIT_EXCLUDE),
            ".foundry/events/\n.foundry/reviews/digest.md\n"
        );
        // git honors it.
        repo.write(".foundry/events/a.jsonl", "{}\n");
        assert_eq!(
            repo.git(&["status", "--porcelain", "--untracked-files=all"]),
            ""
        );
        // Still never a plain write.
        assert_not_allowed(write(repo.path(), GIT_EXCLUDE, "x"));
    }

    #[test]
    fn git_excludes_keep_existing_lines() {
        let repo = TestRepo::with_git();
        let before = "# mine\r\n*.swp\r\n.foundry/events/\r\nbuild";
        repo.write(GIT_EXCLUDE, before);

        assert_eq!(
            add_git_excludes(repo.path(), EXCLUDES).unwrap(),
            [".foundry/reviews/digest.md"]
        );
        assert_eq!(
            repo.read(GIT_EXCLUDE),
            format!("{before}\r\n.foundry/reviews/digest.md\r\n")
        );
    }

    #[test]
    fn git_excludes_skip_folders_without_a_git_dir() {
        let plain = TestRepo::new();
        assert!(add_git_excludes(plain.path(), EXCLUDES).unwrap().is_empty());
        assert!(!plain.path().join(".git").exists());

        let worktree = TestRepo::new();
        worktree.write(".git", "gitdir: elsewhere\n");
        assert!(add_git_excludes(worktree.path(), EXCLUDES)
            .unwrap()
            .is_empty());
        assert_eq!(worktree.read(".git"), "gitdir: elsewhere\n");
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
