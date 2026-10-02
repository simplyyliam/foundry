//! Test helpers. Compiled into unit tests via `#[cfg(test)] mod testing` and into
//! integration tests via `#[path = "../src/testing.rs"] mod testing;`, so it must only
//! use std and dev-dependencies, never `crate::`.
#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tempfile::TempDir;

/// A throwaway directory standing in for a user's linked repo. Deleted on drop.
pub struct TestRepo {
    dir: TempDir,
}

impl TestRepo {
    /// An empty temp directory (no git).
    pub fn new() -> Self {
        Self {
            dir: tempfile::tempdir().expect("create temp dir"),
        }
    }

    /// A temp directory with `git init` run in it and a local identity configured.
    pub fn with_git() -> Self {
        let repo = Self::new();
        repo.git(&["init", "--quiet"]);
        repo.git(&["config", "user.name", "Foundry Test"]);
        repo.git(&["config", "user.email", "test@foundry.invalid"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Writes a file relative to the repo root, creating parent directories.
    pub fn write(&self, rel: &str, contents: &str) -> PathBuf {
        let path = self.path().join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dirs");
        }
        fs::write(&path, contents).expect("write test file");
        path
    }

    pub fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.path().join(rel)).expect("read test file")
    }

    /// Runs git in the repo and returns trimmed stdout. Panics on failure.
    pub fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .args(args)
            .current_dir(self.path())
            .output()
            .expect("run git (is it installed?)");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }
}

impl Default for TestRepo {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates a directory link at `link` pointing to `target`.
/// Unix: a symlink. Windows: a symlink if allowed (Developer Mode or admin), else a junction,
/// which needs no privilege and escapes a root just like a symlink.
pub fn link_dir(target: &Path, link: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, link).expect("create symlink");

    #[cfg(windows)]
    if std::os::windows::fs::symlink_dir(target, link).is_err() {
        let status = Command::new("cmd")
            .arg("/C")
            .arg("mklink")
            .arg("/J")
            .arg(link)
            .arg(target)
            .stdout(std::process::Stdio::null())
            .status()
            .expect("run mklink");
        assert!(status.success(), "mklink /J failed");
    }
}

/// Path to the built `fake-agent` binary.
pub fn fake_agent_path() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_fake-agent") {
        return PathBuf::from(path);
    }
    built_bin("fake-agent")
}

/// Path to the built `fake-claude` binary (copy it before use; see that file).
pub fn fake_claude_path() -> PathBuf {
    if let Some(path) = option_env!("CARGO_BIN_EXE_fake-claude") {
        return PathBuf::from(path);
    }
    built_bin("fake-claude")
}

/// Installs a copy of `fake-claude` as `claude(.exe)` in `dir` and returns its path. Files
/// next to it (`auth.json`, `reply.txt`) configure it for that test alone.
pub fn install_fake_claude(dir: &Path) -> PathBuf {
    let target = dir.join(format!("claude{}", std::env::consts::EXE_SUFFIX));
    fs::copy(fake_claude_path(), &target).expect("copy fake-claude");
    target
}

/// Unit tests don't get CARGO_BIN_EXE_*: the binary sits next to the `deps` directory.
fn built_bin(name: &str) -> PathBuf {
    let exe = std::env::current_exe().expect("current exe");
    let dir = exe.parent().and_then(Path::parent).expect("target dir");
    let path = dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    assert!(
        path.exists(),
        "{name} not built at {}; run `cargo test` (not `--lib`) or `cargo build --bin {name}`",
        path.display()
    );
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repo_is_an_empty_temp_dir_removed_on_drop() {
        let repo = TestRepo::new();
        let path = repo.path().to_path_buf();
        assert!(path.is_dir());
        assert_eq!(fs::read_dir(&path).unwrap().count(), 0);
        repo.write("a/b.txt", "hi");
        assert_eq!(repo.read("a/b.txt"), "hi");
        drop(repo);
        assert!(!path.exists());
    }

    #[test]
    fn test_repo_with_git_can_commit() {
        let repo = TestRepo::with_git();
        assert!(repo.path().join(".git").is_dir());
        repo.git(&["commit", "--allow-empty", "--quiet", "-m", "init"]);
        assert_eq!(repo.git(&["rev-list", "--count", "HEAD"]), "1");
    }
}
