//! Filesystem helpers.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::{ids, AppError};

/// Writes `contents` to `path` atomically: a temp file in the same directory, fsync, rename.
pub fn write_atomic(path: &Path, contents: impl AsRef<[u8]>) -> Result<(), AppError> {
    write_atomic_with(path, |file| file.write_all(contents.as_ref()))
}

/// Like [`write_atomic`], but `fill` streams the contents into the temp file.
/// If `fill` (or anything after it) fails, the temp file is removed and `path` is untouched.
pub fn write_atomic_with(
    path: &Path,
    fill: impl FnOnce(&mut File) -> io::Result<()>,
) -> Result<(), AppError> {
    let tmp = temp_path(path)?;
    let result = write_and_rename(&tmp, path, fill);
    if result.is_err() {
        // Best effort: the temp file may never have been created.
        let _ = fs::remove_file(&tmp);
    }
    result.map_err(AppError::from)
}

fn write_and_rename(
    tmp: &Path,
    path: &Path,
    fill: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let mut file = File::options().write(true).create_new(true).open(tmp)?;
    fill(&mut file)?;
    file.sync_all()?;
    drop(file);
    // std::fs::rename replaces an existing destination file on Windows and Unix.
    fs::rename(tmp, path)
}

fn temp_path(path: &Path) -> Result<PathBuf, AppError> {
    let name = path
        .file_name()
        .ok_or_else(|| AppError::invalid_path(format!("not a file path: {}", path.display())))?;
    let mut tmp_name = std::ffi::OsString::from(".");
    tmp_name.push(name);
    tmp_name.push(format!(".{}.tmp", ids::new_id()));
    Ok(path.with_file_name(tmp_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorCode;
    use crate::testing::TestRepo;

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn writes_new_file() {
        let repo = TestRepo::new();
        let dest = repo.path().join("a.json");
        write_atomic(&dest, b"{}").unwrap();
        assert_eq!(fs::read_to_string(&dest).unwrap(), "{}");
        assert_eq!(entries(repo.path()), ["a.json"]);
    }

    #[test]
    fn replaces_existing_file() {
        let repo = TestRepo::new();
        repo.write("a.txt", "old");
        write_atomic(&repo.path().join("a.txt"), "new").unwrap();
        assert_eq!(repo.read("a.txt"), "new");
        assert_eq!(entries(repo.path()), ["a.txt"]);
    }

    #[test]
    fn failed_write_leaves_existing_destination_untouched() {
        let repo = TestRepo::new();
        repo.write("a.txt", "original");
        let err = write_atomic_with(&repo.path().join("a.txt"), |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("forced failure"))
        })
        .unwrap_err();

        assert_eq!(err.code, ErrorCode::Io);
        assert_eq!(repo.read("a.txt"), "original");
        assert_eq!(
            entries(repo.path()),
            ["a.txt"],
            "temp file was not cleaned up"
        );
    }

    #[test]
    fn failed_write_creates_no_destination() {
        let repo = TestRepo::new();
        let dest = repo.path().join("new.txt");
        write_atomic_with(&dest, |file| {
            file.write_all(b"partial")?;
            Err(io::Error::other("forced failure"))
        })
        .unwrap_err();

        assert!(!dest.exists());
        assert!(entries(repo.path()).is_empty());
    }

    #[test]
    fn failed_rename_leaves_no_partial_file() {
        // The destination is a non-empty directory, so the final rename fails.
        let repo = TestRepo::new();
        repo.write("dest/keep.txt", "k");
        let err = write_atomic(&repo.path().join("dest"), "data").unwrap_err();

        assert_eq!(err.code, ErrorCode::Io);
        assert!(repo.path().join("dest").is_dir());
        assert_eq!(entries(repo.path()), ["dest"]);
    }

    #[test]
    fn missing_parent_dir_is_an_io_error() {
        let repo = TestRepo::new();
        let err = write_atomic(&repo.path().join("missing/a.txt"), "x").unwrap_err();
        assert_eq!(err.code, ErrorCode::Io);
        assert!(entries(repo.path()).is_empty());
    }
}
