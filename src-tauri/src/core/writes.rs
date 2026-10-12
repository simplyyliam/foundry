//! Who wrote which file: the writes ledger and the per-session file snapshots.
//!
//! - `.foundry/events/writes.jsonl`: append-only, one [`LedgerEntry`] per line. The
//!   agent-events feature records a `start` and an `end` per Claude Code session, a `write` per
//!   successful edit-tool call (`Write`, `Edit`, `MultiEdit`, `NotebookEdit`), and a `snapshot`
//!   whenever it saves an end snapshot. Verification reads it to tell which slice wrote what.
//! - `.foundry/events/snapshots/<sessionId>.start.json` and `.end.json`: a hash of every
//!   non-ignored file (tracked and untracked) when the session started, and when it last ended
//!   or reported. A path whose hash differs between the two changed during the session, by
//!   whatever tool (shell commands included).
//!
//! Both live under `.foundry/events/`, which is git-excluded. Reads are tolerant: a torn or
//! hand-mangled line is skipped, a missing file is empty.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::git::{self, Git, GitRun};
use super::{paths, repo_write, AppError};

pub const LEDGER_FILE: &str = ".foundry/events/writes.jsonl";
pub const SNAPSHOTS_DIR: &str = ".foundry/events/snapshots";
/// Files bigger than this aren't read: their fingerprint is their size and modified time.
pub const HASH_CAP: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    /// The session started (Claude Code's `SessionStart`).
    Start,
    /// An edit tool wrote `path`.
    Write,
    /// An end snapshot was saved (the session reported, or ended).
    Snapshot,
    /// The session ended (`SessionEnd`).
    End,
}

/// One ledger line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LedgerEntry {
    pub kind: EntryKind,
    /// Claude Code's `session_id`.
    pub session_id: String,
    pub slice_id: Option<String>,
    pub agent_id: String,
    /// Repo-relative with `/`; set on `write`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Epoch ms.
    pub ts: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moment {
    Start,
    End,
}

/// Every non-ignored file's fingerprint at one moment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    /// Epoch ms.
    pub ts: u64,
    /// Repo-relative path → fingerprint. A file missing here didn't exist.
    pub files: BTreeMap<String, String>,
}

impl Snapshot {
    /// The paths whose fingerprint differs between `self` (earlier) and `later`: changed,
    /// created or deleted.
    pub fn changed(&self, later: &Snapshot) -> BTreeSet<String> {
        self.files
            .keys()
            .chain(later.files.keys())
            .filter(|p| self.files.get(*p) != later.files.get(*p))
            .cloned()
            .collect()
    }
}

/// Claude Code session ids name files: 1–128 ASCII letters, digits, `-` or `_`.
pub fn valid_session_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn check_session_id(id: &str) -> Result<(), AppError> {
    if valid_session_id(id) {
        Ok(())
    } else {
        Err(AppError::invalid_input(format!(
            "sessionId must be 1-128 letters, digits, '-' or '_': {id:?}"
        )))
    }
}

pub fn snapshot_file(session_id: &str, moment: Moment) -> String {
    let which = match moment {
        Moment::Start => "start",
        Moment::End => "end",
    };
    format!("{SNAPSHOTS_DIR}/{session_id}.{which}.json")
}

/// Whether the ledger ends mid-line (a torn write), so the next append starts a fresh line.
fn ledger_torn(repo: &Path) -> Result<bool, AppError> {
    let path = paths::resolve(repo, LEDGER_FILE)?;
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(err) => return Err(err.into()),
    };
    if file.seek(SeekFrom::End(0))? == 0 {
        return Ok(false);
    }
    file.seek(SeekFrom::End(-1))?;
    let mut last = [0u8; 1];
    file.read_exact(&mut last)?;
    Ok(last[0] != b'\n')
}

/// Appends one entry to the ledger.
pub fn record(repo: &Path, entry: &LedgerEntry) -> Result<(), AppError> {
    check_session_id(&entry.session_id)?;
    let mut line = serde_json::to_string(entry)
        .map_err(|err| AppError::internal(format!("serialize ledger entry: {err}")))?;
    line.push('\n');
    if ledger_torn(repo)? {
        line.insert(0, '\n');
    }
    repo_write::append(repo, LEDGER_FILE, line)
}

/// Every ledger entry, in file order. No ledger: empty.
pub fn read_ledger(repo: &Path) -> Result<Vec<LedgerEntry>, AppError> {
    let Some(text) = repo_write::read_to_string(repo, LEDGER_FILE)? else {
        return Ok(Vec::new());
    };
    Ok(text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<LedgerEntry>(line).ok())
        .filter(|e| valid_session_id(&e.session_id))
        .collect())
}

pub fn save_snapshot(
    repo: &Path,
    session_id: &str,
    moment: Moment,
    snapshot: &Snapshot,
) -> Result<(), AppError> {
    check_session_id(session_id)?;
    let text = serde_json::to_string(snapshot)
        .map_err(|err| AppError::internal(format!("serialize snapshot: {err}")))?;
    repo_write::write(repo, &snapshot_file(session_id, moment), text)
}

/// The saved snapshot, or `None` when there's none (or it doesn't parse).
pub fn read_snapshot(
    repo: &Path,
    session_id: &str,
    moment: Moment,
) -> Result<Option<Snapshot>, AppError> {
    check_session_id(session_id)?;
    let rel = snapshot_file(session_id, moment);
    let Some(text) = repo_write::read_to_string(repo, &rel)? else {
        return Ok(None);
    };
    Ok(serde_json::from_str(&text).ok())
}

/// FNV-1a, 64 bits: stable across builds and platforms, unlike `std`'s hasher.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, b| {
        (hash ^ u64::from(*b)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

/// A file's fingerprint: `h:<fnv>:<len>`, or `s:<len>:<mtime ns>` over [`HASH_CAP`]. `None`
/// for anything that isn't a readable regular file (gone, a folder or submodule, a link).
fn fingerprint(path: &Path) -> Option<String> {
    let meta = fs::symlink_metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    if meta.len() > HASH_CAP {
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        return Some(format!("s:{}:{modified}", meta.len()));
    }
    let bytes = fs::read(path).ok()?;
    Some(format!("h:{:016x}:{}", fnv1a(&bytes), bytes.len()))
}

/// Fingerprints every non-ignored file (`git ls-files --cached --others --exclude-standard`),
/// leaving out `.foundry/events/` itself. Errors when git can't list the files.
pub fn take_snapshot(git: &Git, repo: &Path, ts: u64) -> Result<Snapshot, AppError> {
    let args = [
        "--no-optional-locks",
        "ls-files",
        "--cached",
        "--others",
        "--exclude-standard",
        "-z",
    ];
    let listed = match git.run(repo, &args) {
        Ok(GitRun::Ok(output)) => output.stdout,
        Ok(GitRun::Failed(output)) => {
            return Err(AppError::internal(format!(
                "git ls-files failed: {}",
                git::stderr_line(&output)
            )))
        }
        Ok(GitRun::Missing) => return Err(AppError::internal("git isn't installed")),
        Err(err) => return Err(AppError::internal(format!("git couldn't run: {err}"))),
    };
    let mut files = BTreeMap::new();
    for rel in String::from_utf8_lossy(&listed)
        .split('\0')
        .filter(|p| !p.is_empty() && !p.starts_with(".foundry/events/"))
    {
        if let Some(print) = fingerprint(&repo.join(rel)) {
            files.insert(rel.to_string(), print);
        }
    }
    Ok(Snapshot { ts, files })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRepo;

    fn entry(kind: EntryKind, path: Option<&str>, ts: u64) -> LedgerEntry {
        LedgerEntry {
            kind,
            session_id: "s-1".into(),
            slice_id: Some("feed".into()),
            agent_id: "ada".into(),
            path: path.map(str::to_string),
            ts,
        }
    }

    #[test]
    fn the_ledger_appends_and_skips_a_torn_line() {
        let repo = TestRepo::new();
        record(repo.path(), &entry(EntryKind::Start, None, 1)).unwrap();
        repo_write::append(repo.path(), LEDGER_FILE, "{\"kind\":\"wri").unwrap();
        record(repo.path(), &entry(EntryKind::Write, Some("src/a.rs"), 2)).unwrap();
        let read = read_ledger(repo.path()).unwrap();
        assert_eq!(
            read,
            [
                entry(EntryKind::Start, None, 1),
                entry(EntryKind::Write, Some("src/a.rs"), 2)
            ]
        );
    }

    #[test]
    fn a_snapshot_sees_tracked_and_untracked_changes_but_not_ignored_ones() {
        let repo = TestRepo::with_git();
        repo.write(".gitignore", "target/\n");
        repo.write("src/a.rs", "a\n");
        repo.write("src/gone.rs", "g\n");
        repo.git(&["add", "-A"]);
        repo.git(&["commit", "--quiet", "-m", "start"]);
        let git = Git::system();
        let before = take_snapshot(&git, repo.path(), 1).unwrap();

        repo.write("src/a.rs", "changed\n");
        repo.write("src/new.rs", "new\n");
        repo.write("target/out.bin", "ignored");
        repo.write(".foundry/events/writes.jsonl", "{}\n");
        fs::remove_file(repo.path().join("src/gone.rs")).unwrap();
        let after = take_snapshot(&git, repo.path(), 2).unwrap();

        assert_eq!(
            before.changed(&after),
            BTreeSet::from(["src/a.rs".into(), "src/gone.rs".into(), "src/new.rs".into()])
        );
        save_snapshot(repo.path(), "s-1", Moment::End, &after).unwrap();
        assert_eq!(
            read_snapshot(repo.path(), "s-1", Moment::End).unwrap(),
            Some(after)
        );
        assert_eq!(
            read_snapshot(repo.path(), "s-1", Moment::Start).unwrap(),
            None
        );
        assert!(save_snapshot(repo.path(), "../x", Moment::End, &before).is_err());
    }
}
