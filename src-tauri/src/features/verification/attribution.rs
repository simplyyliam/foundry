//! Which session, and so which slice, wrote each file: from the writes ledger and the
//! per-session snapshots (`core::writes`).
//!
//! A session spans its first to its last ledger entry; two sessions overlap when their spans
//! do. A session owns the paths its edit tools wrote (the ledger). A path that changed between
//! its start and end snapshots (a shell write, say) is also its own, unless another session
//! that overlapped it wrote that path with an edit tool (then it's that one's), or a session
//! of another slice overlapped it (then nobody can tell: ambiguous).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::core::writes::{self, EntryKind, Moment};
use crate::core::AppError;

#[derive(Debug, Clone)]
pub struct Session {
    pub id: String,
    pub slice_id: Option<String>,
    pub agent_id: String,
    /// Epoch ms of its first and last ledger entry.
    pub first: u64,
    pub last: u64,
    /// Paths its edit tools wrote.
    pub written: BTreeSet<String>,
    /// Paths its snapshots show changed and that are its own (see the module docs).
    pub shell: BTreeSet<String>,
    /// Paths its snapshots show changed that can't be told apart: path → the other
    /// sessions' indexes.
    pub ambiguous: BTreeMap<String, Vec<usize>>,
}

impl Session {
    pub fn overlaps(&self, other: &Session) -> bool {
        self.first <= other.last && other.first <= self.last
    }

    pub fn owns(&self, path: &str) -> bool {
        self.written.contains(path) || self.shell.contains(path)
    }

    pub fn on(&self, slice_id: &str) -> bool {
        self.slice_id.as_deref() == Some(slice_id)
    }
}

/// Every session in the ledger, ordered by first entry.
pub fn load(repo: &Path) -> Result<Vec<Session>, AppError> {
    let mut by_id: BTreeMap<String, Session> = BTreeMap::new();
    for entry in writes::read_ledger(repo)? {
        let session = by_id
            .entry(entry.session_id.clone())
            .or_insert_with(|| Session {
                id: entry.session_id.clone(),
                slice_id: None,
                agent_id: entry.agent_id.clone(),
                first: entry.ts,
                last: entry.ts,
                written: BTreeSet::new(),
                shell: BTreeSet::new(),
                ambiguous: BTreeMap::new(),
            });
        if session.slice_id.is_none() {
            session.slice_id = entry.slice_id.clone();
        }
        session.first = session.first.min(entry.ts);
        session.last = session.last.max(entry.ts);
        if let (EntryKind::Write, Some(path)) = (entry.kind, entry.path) {
            session.written.insert(path);
        }
    }
    let mut sessions: Vec<Session> = by_id.into_values().collect();
    sessions.sort_by(|a, b| (a.first, &a.id).cmp(&(b.first, &b.id)));

    for i in 0..sessions.len() {
        let start = writes::read_snapshot(repo, &sessions[i].id, Moment::Start)?;
        let end = writes::read_snapshot(repo, &sessions[i].id, Moment::End)?;
        let (Some(start), Some(end)) = (start, end) else {
            continue;
        };
        let mut shell = BTreeSet::new();
        let mut ambiguous = BTreeMap::new();
        let me = &sessions[i];
        for path in start.changed(&end) {
            if me.written.contains(&path) {
                continue;
            }
            let others = || {
                sessions
                    .iter()
                    .enumerate()
                    .filter(move |(j, other)| *j != i && other.overlaps(me))
            };
            if others().any(|(_, other)| other.written.contains(&path)) {
                continue;
            }
            let rivals: Vec<usize> = others()
                .filter(|(_, other)| other.slice_id != me.slice_id)
                .map(|(j, _)| j)
                .collect();
            if rivals.is_empty() {
                shell.insert(path);
            } else {
                ambiguous.insert(path, rivals);
            }
        }
        sessions[i].shell = shell;
        sessions[i].ambiguous = ambiguous;
    }
    Ok(sessions)
}
