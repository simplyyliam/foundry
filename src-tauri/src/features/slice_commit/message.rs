//! The generated slice commit message and its type, by fixed rules.

use super::disk::{MessageFacts, SliceInfo, VerificationLine};
use super::{ChangedFile, CommitType, FileChange};
use crate::core::slug::slugify;

/// The first line's limit, in characters.
pub const SUBJECT_MAX: usize = 72;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitTypeChoice {
    pub commit_type: CommitType,
    /// One line: why this type.
    pub reason: String,
}

fn choice(commit_type: CommitType, reason: impl Into<String>) -> CommitTypeChoice {
    CommitTypeChoice {
        commit_type,
        reason: reason.into(),
    }
}

/// Foundry's own bookkeeping, which every slice commit may carry: it says nothing about
/// what kind of change the slice made, so the rules skip it.
fn is_bookkeeping(path: &str) -> bool {
    path.starts_with(".foundry/") || path == "vsm_data.js" || path == "vsm_viewer.html"
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// `*.spec.*`, `*.test.*`, or anything under an `e2e/` folder.
fn is_test(path: &str) -> bool {
    let name = file_name(path).to_ascii_lowercase();
    name.contains(".spec.")
        || name.contains(".test.")
        || path.split('/').rev().skip(1).any(|dir| dir == "e2e")
}

/// Under `docs/`, or a markdown file anywhere.
fn is_docs(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("docs/") || lower.ends_with(".md") || lower.ends_with(".mdx")
}

const LOCKFILES: &[&str] = &[
    "package-lock.json",
    "npm-shrinkwrap.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
    "cargo.lock",
    "gemfile.lock",
    "poetry.lock",
    "pipfile.lock",
    "composer.lock",
    "uv.lock",
    "flake.lock",
    "go.sum",
];

const CONFIG_FILES: &[&str] = &[
    "package.json",
    "pnpm-workspace.yaml",
    "cargo.toml",
    "rust-toolchain",
    "rust-toolchain.toml",
    "rustfmt.toml",
    ".rustfmt.toml",
    "clippy.toml",
    "deny.toml",
    "go.mod",
    "pyproject.toml",
    "biome.json",
    "turbo.json",
    "nx.json",
    "lerna.json",
    "renovate.json",
    ".gitignore",
    ".gitattributes",
    ".editorconfig",
    ".npmrc",
    ".nvmrc",
    ".node-version",
    ".tool-versions",
    ".prettierignore",
    ".eslintignore",
    ".env.example",
    "dockerfile",
    "compose.yaml",
    "compose.yml",
    "makefile",
    "justfile",
];

const TOOLING_DIRS: &[&str] = &[
    ".github",
    ".vscode",
    ".husky",
    ".devcontainer",
    ".circleci",
    ".cargo",
    ".claude",
];

/// A lockfile, a config file, or a file in a tooling folder.
fn is_config(path: &str) -> bool {
    let name = file_name(path).to_ascii_lowercase();
    if LOCKFILES.contains(&name.as_str()) || CONFIG_FILES.contains(&name.as_str()) {
        return true;
    }
    if path
        .split('/')
        .rev()
        .skip(1)
        .any(|dir| TOOLING_DIRS.contains(&dir))
    {
        return true;
    }
    let config_ext = [".js", ".cjs", ".mjs", ".ts", ".cts", ".mts", ".json"];
    (name.starts_with("tsconfig") || name.starts_with("jsconfig")) && name.ends_with(".json")
        || name.starts_with(".prettierrc")
        || name.starts_with(".eslintrc")
        || name.starts_with("eslint.config.")
        || name.starts_with("docker-compose")
        || name.contains(".config.") && config_ext.iter().any(|ext| name.ends_with(ext))
}

/// Lowercase words of `text`: runs of letters and digits.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

const FIX_WORDS: &[&str] = &[
    "fix",
    "fixes",
    "fixed",
    "fixing",
    "bug",
    "bugs",
    "bugfix",
    "bugfixes",
    "repair",
    "repairs",
    "repaired",
    "repairing",
    "resolve",
    "resolves",
    "resolved",
    "resolving",
];

fn starts_like_a_fix(text: &str) -> bool {
    words(text)
        .first()
        .is_some_and(|w| FIX_WORDS.contains(&w.as_str()))
}

fn says_refactor(text: &str) -> bool {
    let lower = text.to_lowercase();
    ["refactor", "restructur", "cleanup", "clean up", "clean-up"]
        .iter()
        .any(|w| lower.contains(w))
}

/// A new file inside a `features/` folder.
fn adds_feature_file(file: &ChangedFile) -> bool {
    file.change == FileChange::New && file.path.split('/').rev().skip(1).any(|d| d == "features")
}

/// The type, by the first rule that holds:
/// 1. `test`: only test files changed (`*.spec.*`, `*.test.*`, `e2e/**`);
/// 2. `docs`: only `docs/**` or markdown changed;
/// 3. `chore`: only config, lockfiles or tooling changed, or it's the foundation slice (its
///    name has the word "foundation");
/// 4. `fix`: the slice name or first deliverable starts with fix, bug, repair or resolve;
/// 5. `refactor`: either says refactor, restructure or cleanup, and no new file is added in a
///    `features/` folder;
/// 6. `feat` otherwise.
///
/// Foundry's bookkeeping (`.foundry/**`, `vsm_data.js`, `vsm_viewer.html`) is skipped by the
/// file rules; a commit of nothing else is `chore`.
pub fn commit_type(name: &str, first_deliverable: &str, files: &[ChangedFile]) -> CommitTypeChoice {
    let product: Vec<&ChangedFile> = files.iter().filter(|f| !is_bookkeeping(&f.path)).collect();
    let only =
        |rule: fn(&str) -> bool| !product.is_empty() && product.iter().all(|f| rule(&f.path));
    if only(is_test) {
        return choice(CommitType::Test, "only test files changed");
    }
    if only(is_docs) {
        return choice(CommitType::Docs, "only docs or markdown changed");
    }
    if words(name).iter().any(|w| w == "foundation") {
        return choice(CommitType::Chore, "it's the foundation slice");
    }
    if only(is_config) {
        return choice(
            CommitType::Chore,
            "only config, lockfiles or tooling changed",
        );
    }
    if product.is_empty() {
        return choice(CommitType::Chore, "only Foundry's own files changed");
    }
    for (what, text) in [
        ("slice name", name),
        ("first deliverable", first_deliverable),
    ] {
        if starts_like_a_fix(text) {
            return choice(
                CommitType::Fix,
                format!("the {what} starts with \"{}\"", words(text)[0]),
            );
        }
    }
    let refactor = [
        ("slice name", name),
        ("first deliverable", first_deliverable),
    ]
    .into_iter()
    .find(|(_, text)| says_refactor(text));
    if let Some((what, _)) = refactor {
        if product.iter().any(|f| adds_feature_file(f)) {
            return choice(
                CommitType::Feat,
                format!("the {what} says refactor, but it adds new files in a feature folder"),
            );
        }
        return choice(
            CommitType::Refactor,
            format!("the {what} says refactor and no new files are added in a feature folder"),
        );
    }
    choice(CommitType::Feat, "new or changed product code")
}

/// The text up to the end of its first sentence (`.`, `!` or `?` then a space or the end),
/// without a closing period, on one line.
fn first_sentence(text: &str) -> String {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut end = text.len();
    for (i, c) in text.char_indices() {
        if matches!(c, '.' | '!' | '?') {
            let next = text[i + c.len_utf8()..].chars().next();
            if next.map_or(true, char::is_whitespace) {
                end = if c == '.' { i } else { i + c.len_utf8() };
                break;
            }
        }
    }
    text[..end].trim().to_string()
}

/// `subject` cut to at most [`SUBJECT_MAX`] characters, ending in `…` when cut.
fn fit_subject(subject: &str) -> String {
    if subject.chars().count() <= SUBJECT_MAX {
        return subject.to_string();
    }
    let kept: String = subject.chars().take(SUBJECT_MAX - 1).collect();
    format!("{}…", kept.trim_end())
}

/// A requirement id short enough to list: a UUID becomes its first 8 characters.
fn short_id(id: &str) -> &str {
    if id.chars().count() > 12 {
        id.get(..8).unwrap_or(id)
    } else {
        id
    }
}

/// ```text
/// <type>(<slice-name-in-kebab-case>): <summary>
///
/// Requirements met: <n> of <m> (<met ids>)
/// Verification: passed | overridden (<reason>) | failed | stale | not run
/// Agents: <names>
/// ```
/// The summary is the first sentence of the first deliverable (else the slice name); the
/// first line is cut at [`SUBJECT_MAX`] characters.
pub fn build(commit_type: CommitType, slice: &SliceInfo, facts: &MessageFacts) -> String {
    let scope = Some(slugify(&slice.name))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "slice".into());
    let summary = Some(first_sentence(slice.first_deliverable()))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| first_sentence(&slice.name));
    let subject = fit_subject(&format!("{}({scope}): {summary}", commit_type.as_str()));

    let mut met = format!(
        "Requirements met: {} of {}",
        facts.met.len(),
        facts.required.len()
    );
    if !facts.met.is_empty() {
        let ids: Vec<&str> = facts.met.iter().map(|id| short_id(id)).collect();
        met.push_str(&format!(" ({})", ids.join(", ")));
    }
    let verification = match &facts.verification {
        VerificationLine::Passed => "passed".to_string(),
        VerificationLine::Overridden(reason) => format!(
            "overridden ({})",
            reason.split_whitespace().collect::<Vec<_>>().join(" ")
        ),
        VerificationLine::Failed => "failed".to_string(),
        VerificationLine::Stale => "stale (a report changed after it passed)".to_string(),
        VerificationLine::NotRun => "not run".to_string(),
    };
    let agents = if facts.agents.is_empty() {
        "none".to_string()
    } else {
        facts.agents.join(", ")
    };
    format!("{subject}\n\n{met}\nVerification: {verification}\nAgents: {agents}\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_sentence_stops_at_the_first_sentence_end() {
        assert_eq!(
            first_sentence("Users can log in. Then more."),
            "Users can log in"
        );
        assert_eq!(first_sentence("Is it v1.2 ready? Yes"), "Is it v1.2 ready?");
        assert_eq!(first_sentence("  no   end  "), "no end");
    }

    #[test]
    fn a_long_subject_is_cut_to_72_characters() {
        let long = format!("feat(x): {}", "word ".repeat(30));
        let cut = fit_subject(&long);
        assert!(cut.chars().count() <= SUBJECT_MAX);
        assert!(cut.ends_with('…'));
        assert_eq!(fit_subject("feat(x): short"), "feat(x): short");
    }

    #[test]
    fn uuid_ids_are_shortened_and_short_ids_kept() {
        assert_eq!(short_id("r-1"), "r-1");
        assert_eq!(short_id("1b4e28ba-2fa1-11d2-883f-0016d3cca427"), "1b4e28ba");
    }
}
