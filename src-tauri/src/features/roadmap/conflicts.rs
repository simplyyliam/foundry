//! The step after generation: asking the model where the brain dump contradicts the design
//! family files and the Bakery's Take and Skip notes, then keeping only conflicts whose
//! excerpts really are in their sources.
//!
//! Reads `.foundry/design-files.json` (the copies the design feature made in `docs/design/`)
//! and `.foundry/bakery.json` from disk; neither feature is imported.

use std::path::Path;

use serde::Deserialize;
use serde_json::Value;

use super::validate::{normalize, parse};
use crate::core::conflicts::{
    ConflictSource, ConflictSourceKind, RoadmapConflict, RulingKind, MAX_CONFLICTS,
};
use crate::core::{ids, repo_write, AppError};

const PROMPT: &str = include_str!("conflicts.md");
const DESIGN_FILES: &str = ".foundry/design-files.json";
const DESIGN_DIR: &str = "docs/design";
const BAKERY_FILE: &str = ".foundry/bakery.json";
/// Larger design files are left out of the comparison.
const MAX_SOURCE_BYTES: usize = 200 * 1000;

/// One reference the brain dump is compared with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub kind: ConflictSourceKind,
    pub reference: String,
    /// What an excerpt must be found in: a design file's text, or a Bakery entry's Take and
    /// Skip notes.
    pub texts: Vec<String>,
    /// How it's shown to the model.
    pub shown: String,
}

#[derive(Deserialize)]
struct BakeryItem {
    file: String,
    #[serde(default)]
    label: String,
    #[serde(default)]
    take: String,
    #[serde(default)]
    skip: String,
}

#[derive(Deserialize, Default)]
struct BakeryFile {
    #[serde(default)]
    items: Vec<BakeryItem>,
}

/// The design family files and Bakery entries with something to compare. A file that's
/// unreadable or missing is left out, not an error: it just can't conflict.
pub fn sources(repo: &Path) -> Result<Vec<Source>, AppError> {
    let mut out = Vec::new();
    let names: Vec<String> = repo_write::read_to_string(repo, DESIGN_FILES)?
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    for name in names {
        if name.eq_ignore_ascii_case("manifest.md") || name.contains(['/', '\\']) {
            continue;
        }
        let text = match repo_write::read_to_string(repo, &format!("{DESIGN_DIR}/{name}")) {
            Ok(Some(text)) if !text.trim().is_empty() && text.len() <= MAX_SOURCE_BYTES => text,
            _ => continue,
        };
        out.push(Source {
            kind: ConflictSourceKind::DesignFamily,
            shown: format!(
                "<design_family ref=\"{name}\">\n{}\n</design_family>",
                text.trim()
            ),
            reference: name,
            texts: vec![text],
        });
    }
    let bakery: BakeryFile = repo_write::read_to_string(repo, BAKERY_FILE)?
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    for item in bakery.items {
        let (take, skip) = (item.take.trim(), item.skip.trim());
        if take.is_empty() && skip.is_empty() {
            continue;
        }
        out.push(Source {
            kind: ConflictSourceKind::Bakery,
            shown: format!(
                "<bakery ref=\"{}\" label=\"{}\">\nTake: {}\nSkip: {}\n</bakery>",
                item.file,
                normalize(&item.label),
                if take.is_empty() { "(none)" } else { take },
                if skip.is_empty() { "(none)" } else { skip },
            ),
            reference: item.file,
            texts: vec![item.take, item.skip],
        });
    }
    Ok(out)
}

/// The detection prompt (LF line endings, as with the roadmap prompt).
pub fn prompt(brain_dump: &str, sources: &[Source]) -> String {
    let references = sources
        .iter()
        .map(|s| s.shown.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");
    PROMPT
        .replace("\r\n", "\n")
        .replacen("{{BRAIN_DUMP}}", brain_dump.trim(), 1)
        .replacen("{{REFERENCES}}", &references, 1)
}

fn string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn kind(value: &str) -> Option<ConflictSourceKind> {
    match value {
        "design_family" => Some(ConflictSourceKind::DesignFamily),
        "bakery" => Some(ConflictSourceKind::Bakery),
        _ => None,
    }
}

fn found_in(haystack: &str, excerpt: &str) -> bool {
    let excerpt = normalize(excerpt);
    !excerpt.is_empty() && normalize(haystack).contains(&excerpt)
}

/// The valid conflicts in `output`, at most `MAX_CONFLICTS`, each with a fresh id and
/// `suggested: reference_wins`. A conflict is dropped when a field is missing, its source
/// isn't one of `sources`, an excerpt isn't in its source, or it repeats an earlier one.
/// `Err` only when the reply isn't a JSON object with a `conflicts` array.
pub fn check(
    output: &str,
    brain_dump: &str,
    sources: &[Source],
) -> Result<Vec<RoadmapConflict>, String> {
    let value = parse(output).map_err(|errors| errors.join("; "))?;
    let Some(items) = value.get("conflicts").and_then(Value::as_array) else {
        return Err("the reply has no \"conflicts\" array".into());
    };
    let mut out: Vec<RoadmapConflict> = Vec::new();
    for item in items {
        let conflict = (|| {
            let topic = string(item, "topic")?;
            let brain_dump_excerpt = string(item, "brainDumpExcerpt")?;
            let other_excerpt = string(item, "otherExcerpt")?;
            let other = item.get("otherSource")?;
            let kind = kind(&string(other, "kind")?)?;
            let reference = string(other, "ref")?;
            let source = sources
                .iter()
                .find(|s| s.kind == kind && s.reference == reference)?;
            if !found_in(brain_dump, &brain_dump_excerpt)
                || !source.texts.iter().any(|t| found_in(t, &other_excerpt))
            {
                return None;
            }
            Some(RoadmapConflict {
                id: ids::new_id(),
                topic,
                brain_dump_excerpt,
                other_source: ConflictSource { kind, reference },
                other_excerpt,
                suggested: RulingKind::ReferenceWins,
                ruling: None,
                rule_requirement_id: None,
            })
        })();
        let Some(conflict) = conflict else {
            continue;
        };
        let repeated = out.iter().any(|c| {
            c.other_source == conflict.other_source
                && normalize(&c.brain_dump_excerpt) == normalize(&conflict.brain_dump_excerpt)
                && normalize(&c.other_excerpt) == normalize(&conflict.other_excerpt)
        });
        if !repeated {
            out.push(conflict);
        }
        if out.len() == MAX_CONFLICTS {
            break;
        }
    }
    Ok(out)
}
