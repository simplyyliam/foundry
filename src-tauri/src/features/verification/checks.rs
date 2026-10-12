//! Running a requirement's checks (`core::checks`) against the project. Read-only: nothing
//! here writes, follows a symlink, or looks into `core::checks::SKIPPED_DIRS`. Every limit
//! that stops a check early fails it (fail closed), with the reason in `detail`.

use std::fs;
use std::path::Path;
use std::time::Instant;

use serde_json::Value;

use crate::core::checks::{self, Limits, RequirementCheck};
use crate::core::{glob, paths};

const PACKAGE_JSON: &str = "package.json";
const DEPENDENCY_KEYS: &[&str] = &[
    "dependencies",
    "devDependencies",
    "peerDependencies",
    "optionalDependencies",
];

/// One check's result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub passed: bool,
    /// What was found, for people: on a failure, the first matching `file:line` when there is
    /// one.
    pub detail: String,
    /// The first file a failure is about.
    pub path: Option<String>,
}

impl Outcome {
    fn pass(detail: impl Into<String>) -> Self {
        Self {
            passed: true,
            detail: detail.into(),
            path: None,
        }
    }

    fn fail(detail: impl Into<String>, path: Option<String>) -> Self {
        Self {
            passed: false,
            detail: detail.into(),
            path,
        }
    }
}

/// Why a walk stopped early.
enum Stop {
    TooManyFiles,
    TimeLimit,
    /// The visitor has what it needs.
    Done,
}

/// Every file under `repo` (not symlinks, not in skipped folders), depth first in name order,
/// as a `/`-separated relative path.
struct Walk<'a> {
    limits: &'a Limits,
    started: Instant,
    visited: usize,
}

impl Walk<'_> {
    fn dir(
        &mut self,
        abs: &Path,
        rel: &str,
        visit: &mut dyn FnMut(&str, &Path, u64) -> bool,
    ) -> Result<(), Stop> {
        let Ok(entries) = fs::read_dir(abs) else {
            return Ok(());
        };
        let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            if self.started.elapsed() >= self.limits.time_limit {
                return Err(Stop::TimeLimit);
            }
            let Some(name) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };
            // Not followed: a link could lead out of the project.
            let Ok(meta) = entry.path().symlink_metadata() else {
                continue;
            };
            let child = if rel.is_empty() {
                name.clone()
            } else {
                format!("{rel}/{name}")
            };
            if meta.is_dir() {
                if !checks::SKIPPED_DIRS.contains(&name.as_str()) {
                    self.dir(&entry.path(), &child, visit)?;
                }
            } else if meta.is_file() {
                self.visited += 1;
                if self.visited > self.limits.max_files {
                    return Err(Stop::TooManyFiles);
                }
                if !visit(&child, &entry.path(), meta.len()) {
                    return Err(Stop::Done);
                }
            }
        }
        Ok(())
    }
}

/// Walks `repo`; `visit` returns `false` to stop. `Some(reason)` when a limit stopped it.
fn walk(
    repo: &Path,
    limits: &Limits,
    started: Instant,
    visit: &mut dyn FnMut(&str, &Path, u64) -> bool,
) -> Option<String> {
    let mut w = Walk {
        limits,
        started,
        visited: 0,
    };
    match w.dir(repo, "", visit) {
        Ok(()) | Err(Stop::Done) => None,
        Err(Stop::TooManyFiles) => Some(format!(
            "stopped after {} files without finishing; narrow the check",
            limits.max_files
        )),
        Err(Stop::TimeLimit) => Some(format!(
            "stopped at the {} s time limit without finishing; narrow the check",
            limits.time_limit.as_secs_f32()
        )),
    }
}

/// The 1-based line of byte offset `at` in `text`.
fn line_of(text: &str, at: usize) -> usize {
    text.as_bytes()[..at.min(text.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
        + 1
}

/// Runs one check. Never fails: a problem (an escaping path, a bad pattern, a limit) is a
/// failed outcome.
pub fn run(repo: &Path, check: &RequirementCheck, limits: &Limits) -> Outcome {
    // Checks are validated when they're set, but plan.json can be edited by hand.
    let check = match checks::validate(check) {
        Ok(check) => check,
        Err(err) => return Outcome::fail(format!("not a valid check: {}", err.message), None),
    };
    let started = Instant::now();
    match &check {
        RequirementCheck::FileExists { path } => match exists(repo, path) {
            Ok(true) => Outcome::pass(format!("{path} exists")),
            Ok(false) => Outcome::fail(format!("{path} doesn't exist"), Some(path.clone())),
            Err(detail) => Outcome::fail(detail, Some(path.clone())),
        },
        RequirementCheck::FileAbsent { path } => match exists(repo, path) {
            Ok(false) => Outcome::pass(format!("{path} doesn't exist")),
            Ok(true) => Outcome::fail(format!("{path} exists"), Some(path.clone())),
            Err(detail) => Outcome::fail(detail, Some(path.clone())),
        },
        RequirementCheck::DependencyPresent { name } => {
            dependency(repo, name, true, limits, started)
        }
        RequirementCheck::DependencyAbsent { name } => {
            dependency(repo, name, false, limits, started)
        }
        RequirementCheck::TextAbsent { glob, pattern } => {
            text_absent(repo, glob, pattern, limits, started)
        }
    }
}

/// Whether `path` exists inside the project; `Err` (a detail) when it resolves outside it.
fn exists(repo: &Path, path: &str) -> Result<bool, String> {
    let target = paths::resolve(repo, path).map_err(|err| err.message)?;
    Ok(target.symlink_metadata().is_ok())
}

/// Every dependency of a `package.json` whose name matches `wanted`, with its line.
fn matching_dependencies(text: &str, value: &Value, wanted: &str) -> Vec<(String, usize)> {
    let mut found: Vec<(String, usize)> = Vec::new();
    for key in DEPENDENCY_KEYS {
        let Some(Value::Object(deps)) = value.get(key) else {
            continue;
        };
        for name in deps.keys() {
            if checks::dependency_matches(wanted, name) && !found.iter().any(|(n, _)| n == name) {
                let quoted = format!("\"{name}\"");
                let line = text.find(&quoted).map_or(1, |at| line_of(text, at));
                found.push((name.clone(), line));
            }
        }
    }
    found.sort_by_key(|(_, line)| *line);
    found
}

fn dependency(
    repo: &Path,
    wanted: &str,
    present: bool,
    limits: &Limits,
    started: Instant,
) -> Outcome {
    let mut files = 0usize;
    let mut unreadable: Vec<String> = Vec::new();
    // The first package.json with a match: (file, dependency, line).
    let mut hit: Option<(String, String, usize)> = None;
    let mut total = 0usize;
    let stopped = walk(repo, limits, started, &mut |rel, abs, size| {
        if rel.rsplit('/').next() != Some(PACKAGE_JSON) {
            return true;
        }
        files += 1;
        let parsed = (size <= limits.max_file_bytes)
            .then(|| fs::read_to_string(abs).ok())
            .flatten()
            .and_then(|text| {
                let value =
                    serde_json::from_str::<Value>(text.trim_start_matches('\u{feff}')).ok()?;
                Some((text, value))
            });
        let Some((text, value)) = parsed else {
            unreadable.push(rel.to_string());
            return true;
        };
        let found = matching_dependencies(&text, &value, wanted);
        if let Some((name, line)) = found.first() {
            if hit.is_none() {
                hit = Some((rel.to_string(), name.clone(), *line));
            }
            total += found.len();
            // Present needs one; absent names the first and counts the rest, up to the cap.
            if present || total >= limits.max_matches {
                return false;
            }
        }
        true
    });
    let more = match total {
        0 | 1 => String::new(),
        n if n >= limits.max_matches => format!(" (and {} or more others)", n - 1),
        n => format!(" (and {} more)", n - 1),
    };
    match (present, hit, stopped) {
        (true, Some((file, name, line)), _) => Outcome::pass(format!("{name} is in {file}:{line}")),
        (false, Some((file, name, line)), _) => {
            Outcome::fail(format!("{name} is in {file}:{line}{more}"), Some(file))
        }
        (_, None, Some(reason)) => Outcome::fail(reason, None),
        (false, None, None) if !unreadable.is_empty() => Outcome::fail(
            format!("couldn't read {}", unreadable.join(", ")),
            unreadable.first().cloned(),
        ),
        (true, None, None) if files == 0 => Outcome::fail("the project has no package.json", None),
        (true, None, None) => Outcome::fail(
            format!("{wanted} is in none of the project's {files} package.json files"),
            None,
        ),
        (false, None, None) => Outcome::pass(format!(
            "{wanted} is in none of the project's {files} package.json files"
        )),
    }
}

fn text_absent(
    repo: &Path,
    glob_pattern: &str,
    pattern: &str,
    limits: &Limits,
    started: Instant,
) -> Outcome {
    let regex = match checks::compile(pattern) {
        Ok(regex) => regex,
        Err(err) => return Outcome::fail(err.message, None),
    };
    // The glob's literal folders (up to its first wildcard) must not lead out of the project
    // through a link: the walk wouldn't follow it, so the check would pass on nothing.
    let literal: Vec<&str> = glob_pattern
        .split('/')
        .take_while(|s| !s.contains(['*', '?']))
        .filter(|s| !s.is_empty())
        .collect();
    if !literal.is_empty() {
        if let Err(err) = paths::resolve(repo, literal.join("/")) {
            return Outcome::fail(err.message, None);
        }
    }
    let mut searched = 0usize;
    let mut too_big = 0usize;
    let mut matches: Vec<(String, usize)> = Vec::new();
    let stopped = walk(repo, limits, started, &mut |rel, abs, size| {
        if !glob::matches(glob_pattern, rel) {
            return true;
        }
        if size > limits.max_file_bytes {
            too_big += 1;
            return true;
        }
        // Unreadable or not text: nothing to match.
        let Some(text) = fs::read(abs).ok().and_then(|b| String::from_utf8(b).ok()) else {
            return true;
        };
        searched += 1;
        for m in regex
            .find_iter(&text)
            .take(limits.max_matches.saturating_sub(matches.len()))
        {
            matches.push((rel.to_string(), line_of(&text, m.start())));
        }
        matches.len() < limits.max_matches
    });
    let skipped = if too_big > 0 {
        format!(
            "; {too_big} file(s) over {} KB not searched",
            limits.max_file_bytes / 1024
        )
    } else {
        String::new()
    };
    if let Some((file, line)) = matches.first() {
        let more = match matches.len() {
            1 => String::new(),
            n if n >= limits.max_matches => format!(" (and {} or more others)", n - 1),
            n => format!(" (and {} more)", n - 1),
        };
        return Outcome::fail(
            format!("{pattern:?} matches {file}:{line}{more}"),
            Some(file.clone()),
        );
    }
    if let Some(reason) = stopped {
        return Outcome::fail(reason, None);
    }
    Outcome::pass(format!(
        "{pattern:?} matches none of {searched} file(s) in {glob_pattern}{skipped}"
    ))
}
