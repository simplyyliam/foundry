//! Requirement checks: small, read-only facts about a project that verification tests itself,
//! so a rule like "use Base UI, not Radix" doesn't rest on a report's word alone.
//!
//! Stored on a plan requirement (`checks`, run by verification; `suggestedChecks`, never run
//! until the human accepts one). Shared here because the plan stores them, the roadmap
//! suggests them and verification runs them.

use std::path::Path;
use std::time::Duration;

use regex::{Regex, RegexBuilder};
use serde::{Deserialize, Serialize};
use specta::Type;

use super::{paths, AppError};

/// One check. Paths and globs are relative to the project root, `/`-separated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RequirementCheck {
    /// The file or folder exists.
    FileExists { path: String },
    /// The file or folder doesn't exist.
    FileAbsent { path: String },
    /// A dependency of this name is in some `package.json` of the project (any of
    /// `dependencies`, `devDependencies`, `peerDependencies`, `optionalDependencies`). A name
    /// ending in `*` is a prefix, e.g. `@radix-ui/*`.
    DependencyPresent { name: String },
    /// No `package.json` of the project has a dependency of this name (same rules).
    DependencyAbsent { name: String },
    /// No file matching `glob` (the role-glob syntax, `core::glob`) has a match for
    /// `pattern`, a regular expression.
    TextAbsent { glob: String, pattern: String },
}

/// At most this many checks per requirement (and suggestions).
pub const MAX_CHECKS: usize = 20;
/// Longest path, glob, name or pattern accepted.
pub const MAX_FIELD_CHARS: usize = 500;
/// Compiled size limit for a pattern (and for its lazy DFA).
pub const REGEX_SIZE_LIMIT: usize = 256 * 1024;

/// What one run of checks may cost. Verification uses [`Limits::default`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Larger files aren't read (they're counted as skipped).
    pub max_file_bytes: u64,
    /// More files than this visited: the check stops and fails (fail closed).
    pub max_files: usize,
    /// Matches recorded per check before it stops looking.
    pub max_matches: usize,
    /// Wall-clock budget per check: past it, the check stops and fails (fail closed).
    pub time_limit: Duration,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_file_bytes: 1024 * 1024,
            max_files: 50_000,
            max_matches: 20,
            time_limit: Duration::from_secs(10),
        }
    }
}

/// Folders never looked into: dependencies, VCS data, build output, caches, and Foundry's own.
pub const SKIPPED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    ".foundry",
    "target",
    "dist",
    "build",
    "out",
    "coverage",
    ".next",
    ".nuxt",
    ".svelte-kit",
    ".turbo",
    ".cache",
    ".vite",
    ".venv",
    "venv",
    "__pycache__",
    "test-results",
    "playwright-report",
];

fn invalid(message: impl Into<String>) -> AppError {
    AppError::invalid_input(message)
}

/// A repo-relative path or glob: not empty, not absolute, no `..` segment (`path_escape`).
/// Backslashes count as separators. Returns it trimmed, `/`-separated, without `./`.
fn relative(what: &str, value: &str) -> Result<String, AppError> {
    let value = value.trim().replace('\\', "/");
    if value.is_empty() {
        return Err(invalid(format!("{what} can't be empty")));
    }
    if value.chars().count() > MAX_FIELD_CHARS {
        return Err(invalid(format!(
            "{what} must be {MAX_FIELD_CHARS} characters or fewer"
        )));
    }
    // A drive (`C:`) or a root is absolute on some platform: never inside the project.
    if value.starts_with('/') || value.contains(':') {
        return Err(AppError::path_escape(format!(
            "{what} must be relative to the project: {value}"
        )));
    }
    if value.split('/').any(|segment| segment == "..") {
        return Err(AppError::path_escape(format!(
            "{what} must not contain '..': {value}"
        )));
    }
    paths::check(Path::new(&value), paths::PathKind::Relative)?;
    let parts: Vec<&str> = value
        .split('/')
        .filter(|s| !s.is_empty() && *s != ".")
        .collect();
    if parts.is_empty() {
        return Err(invalid(format!("{what} can't be the project root")));
    }
    let mut out = parts.join("/");
    if value.ends_with('/') {
        out.push('/');
    }
    Ok(out)
}

fn dependency_name(name: &str) -> Result<String, AppError> {
    let name = name.trim();
    if name.is_empty() || name == "*" {
        return Err(invalid("a dependency name can't be empty"));
    }
    if name.chars().count() > MAX_FIELD_CHARS || name.chars().any(char::is_whitespace) {
        return Err(invalid(format!("not a dependency name: {name:?}")));
    }
    if name.trim_end_matches('*').contains('*') || name.ends_with("**") {
        return Err(invalid(format!(
            "a dependency name may only end with one '*': {name:?}"
        )));
    }
    Ok(name.to_string())
}

/// Compiles a `text_absent` pattern within [`REGEX_SIZE_LIMIT`] (`invalid_input` otherwise).
pub fn compile(pattern: &str) -> Result<Regex, AppError> {
    if pattern.is_empty() {
        return Err(invalid("a pattern can't be empty"));
    }
    if pattern.chars().count() > MAX_FIELD_CHARS {
        return Err(invalid(format!(
            "a pattern must be {MAX_FIELD_CHARS} characters or fewer"
        )));
    }
    RegexBuilder::new(pattern)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_SIZE_LIMIT)
        .build()
        .map_err(|err| match err {
            regex::Error::CompiledTooBig(_) => invalid(format!(
                "the pattern {pattern:?} is over the size limit; make it simpler"
            )),
            other => invalid(format!("the pattern {pattern:?} isn't valid: {other}")),
        })
}

/// The check, checked and tidied: paths and globs inside the project, a valid dependency name,
/// a pattern that compiles within the limits. Errors: `invalid_input`, `path_escape`.
pub fn validate(check: &RequirementCheck) -> Result<RequirementCheck, AppError> {
    use RequirementCheck::*;
    Ok(match check {
        FileExists { path } => FileExists {
            path: relative("a path", path)?,
        },
        FileAbsent { path } => FileAbsent {
            path: relative("a path", path)?,
        },
        DependencyPresent { name } => DependencyPresent {
            name: dependency_name(name)?,
        },
        DependencyAbsent { name } => DependencyAbsent {
            name: dependency_name(name)?,
        },
        TextAbsent { glob, pattern } => {
            compile(pattern)?;
            TextAbsent {
                glob: relative("a glob", glob)?,
                pattern: pattern.clone(),
            }
        }
    })
}

/// Validates a whole list (at most [`MAX_CHECKS`]), dropping exact duplicates.
pub fn validate_all(checks: &[RequirementCheck]) -> Result<Vec<RequirementCheck>, AppError> {
    if checks.len() > MAX_CHECKS {
        return Err(invalid(format!(
            "a requirement can have at most {MAX_CHECKS} checks"
        )));
    }
    let mut out: Vec<RequirementCheck> = Vec::new();
    for check in checks {
        let check = validate(check)?;
        if !out.contains(&check) {
            out.push(check);
        }
    }
    Ok(out)
}

/// Whether dependency `name` matches `wanted` (`*` at the end: a prefix).
pub fn dependency_matches(wanted: &str, name: &str) -> bool {
    match wanted.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => name == wanted,
    }
}

/// One line for people, e.g. `dependency_absent @radix-ui/*`.
pub fn describe(check: &RequirementCheck) -> String {
    use RequirementCheck::*;
    match check {
        FileExists { path } => format!("file_exists {path}"),
        FileAbsent { path } => format!("file_absent {path}"),
        DependencyPresent { name } => format!("dependency_present {name}"),
        DependencyAbsent { name } => format!("dependency_absent {name}"),
        TextAbsent { glob, pattern } => format!("text_absent /{pattern}/ in {glob}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ErrorCode;

    #[test]
    fn paths_and_globs_must_stay_inside_the_project() {
        for path in [
            "../x",
            "a/../../x",
            "/etc/passwd",
            "C:/x",
            "a\\..\\..\\x",
            "",
            ".",
        ] {
            let check = RequirementCheck::FileExists { path: path.into() };
            let err = validate(&check).unwrap_err();
            assert!(
                matches!(err.code, ErrorCode::PathEscape | ErrorCode::InvalidInput),
                "{path}: {err:?}"
            );
        }
        let glob = RequirementCheck::TextAbsent {
            glob: "../**/*.ts".into(),
            pattern: "x".into(),
        };
        assert_eq!(validate(&glob).unwrap_err().code, ErrorCode::PathEscape);
        let ok = RequirementCheck::FileExists {
            path: " ./src\\components.json ".into(),
        };
        assert_eq!(
            validate(&ok).unwrap(),
            RequirementCheck::FileExists {
                path: "src/components.json".into()
            }
        );
    }

    #[test]
    fn patterns_over_the_size_limit_are_refused() {
        let err = compile(r"(\w{500}){500}").unwrap_err();
        assert_eq!(err.code, ErrorCode::InvalidInput);
        assert!(err.message.contains("size limit"), "{}", err.message);
        assert!(compile("(").is_err());
        assert!(compile(&"a".repeat(MAX_FIELD_CHARS + 1)).is_err());
        assert!(compile(r#"from\s+["']@radix-ui/"#).is_ok());
    }

    #[test]
    fn dependency_names_allow_one_trailing_star() {
        assert!(dependency_name("@radix-ui/*").is_ok());
        assert!(dependency_name("a*b").is_err());
        assert!(dependency_name("*").is_err());
        assert!(dependency_name("a b").is_err());
        assert!(dependency_matches("@radix-ui/*", "@radix-ui/react-dialog"));
        assert!(!dependency_matches(
            "@radix-ui/*",
            "@base-ui-components/react"
        ));
        assert!(dependency_matches("react", "react"));
        assert!(!dependency_matches("react", "react-dom"));
    }

    #[test]
    fn checks_serialize_with_a_type_tag() {
        let check = RequirementCheck::TextAbsent {
            glob: "src/**".into(),
            pattern: "x".into(),
        };
        let json = serde_json::to_value(&check).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "type": "text_absent", "glob": "src/**", "pattern": "x" })
        );
        assert_eq!(
            serde_json::from_value::<RequirementCheck>(json).unwrap(),
            check
        );
    }
}
