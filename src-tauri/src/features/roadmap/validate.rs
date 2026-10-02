//! Mechanical checks on a generated roadmap. Nothing here trusts the model: every field is
//! type-checked, every excerpt is looked up in the brain dump, every dependency is resolved.

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use super::{Deferred, Feature, Requirement, Roadmap, Slice, TargetUser};

/// Why an output was rejected. These go back to the model in the retry prompt.
pub type Errors = Vec<String>;

/// A validated roadmap, before it's saved.
#[derive(Debug, Clone, PartialEq)]
pub struct Checked {
    pub roadmap: Roadmap,
    /// Requirements whose `sliceName` names no slice.
    pub unassigned: Vec<Requirement>,
    pub uncovered_paragraphs: Vec<String>,
}

/// Parses, checks and cross-checks `output` against `brain_dump`.
pub fn check(output: &str, brain_dump: &str) -> Result<Checked, Errors> {
    let value = parse(output)?;
    let mut roadmap = schema(&value)?;
    graph(&roadmap.slices)?;

    let source = normalize(brain_dump);
    for requirement in &mut roadmap.requirements {
        let excerpt = normalize(&requirement.source_excerpt);
        requirement.unsourced = excerpt.is_empty() || !source.contains(&excerpt);
    }
    let names: HashSet<&str> = roadmap.slices.iter().map(|s| s.name.as_str()).collect();
    let (assigned, unassigned): (Vec<Requirement>, Vec<Requirement>) = roadmap
        .requirements
        .drain(..)
        .partition(|r| r.slice_name.as_deref().is_some_and(|n| names.contains(n)));
    roadmap.requirements = assigned;
    let uncovered_paragraphs =
        uncovered(brain_dump, roadmap.requirements.iter().chain(&unassigned));
    Ok(Checked {
        roadmap,
        unassigned,
        uncovered_paragraphs,
    })
}

/// The JSON object in `output`, tolerating a surrounding code fence (```json … ```).
pub fn parse(output: &str) -> Result<Value, Errors> {
    let text = unfence(output.trim());
    match serde_json::from_str::<Value>(text) {
        Ok(value) if value.is_object() => Ok(value),
        Ok(_) => Err(vec!["the reply must be a single JSON object".into()]),
        Err(err) => Err(vec![format!("the reply is not valid JSON: {err}")]),
    }
}

fn unfence(text: &str) -> &str {
    let Some(start) = text.find("```") else {
        return text;
    };
    let after = &text[start + 3..];
    // Skip the info string (`json`) up to the end of the opening line.
    let body_start = after.find('\n').map_or(after.len(), |i| i + 1);
    let body = &after[body_start..];
    match body.rfind("```") {
        Some(end) => body[..end].trim(),
        None => body.trim(),
    }
}

/// Collects every schema problem (not just the first), with its path.
struct Schema {
    errors: Errors,
}

impl Schema {
    fn field<'a>(&mut self, obj: &'a Value, path: &str, key: &str) -> Option<&'a Value> {
        let found = obj.get(key).filter(|v| !v.is_null());
        if found.is_none() {
            self.errors.push(format!("{path}.{key} is missing"));
        }
        found
    }

    fn string(&mut self, obj: &Value, path: &str, key: &str) -> String {
        match self.field(obj, path, key) {
            Some(Value::String(s)) => s.trim().to_string(),
            Some(_) => {
                self.errors.push(format!("{path}.{key} must be a string"));
                String::new()
            }
            None => String::new(),
        }
    }

    fn nonempty(&mut self, obj: &Value, path: &str, key: &str) -> String {
        let value = self.string(obj, path, key);
        if value.is_empty() && obj.get(key).is_some_and(Value::is_string) {
            self.errors.push(format!("{path}.{key} must not be empty"));
        }
        value
    }

    /// An optional string: null or absent is `None`.
    fn optional(&mut self, obj: &Value, path: &str, key: &str) -> Option<String> {
        match obj.get(key) {
            None | Some(Value::Null) => None,
            Some(Value::String(s)) => Some(s.trim().to_string()).filter(|s| !s.is_empty()),
            Some(_) => {
                self.errors
                    .push(format!("{path}.{key} must be a string or null"));
                None
            }
        }
    }

    fn strings(&mut self, obj: &Value, path: &str, key: &str) -> Vec<String> {
        match self.field(obj, path, key) {
            Some(Value::Array(items)) => {
                let mut out = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    match item {
                        Value::String(s) => out.push(s.trim().to_string()),
                        _ => self
                            .errors
                            .push(format!("{path}.{key}[{i}] must be a string")),
                    }
                }
                out
            }
            Some(_) => {
                self.errors
                    .push(format!("{path}.{key} must be an array of strings"));
                Vec::new()
            }
            None => Vec::new(),
        }
    }

    fn objects<'a>(&mut self, obj: &'a Value, path: &str, key: &str) -> Vec<(String, &'a Value)> {
        match self.field(obj, path, key) {
            Some(Value::Array(items)) => items
                .iter()
                .enumerate()
                .filter_map(|(i, item)| {
                    let item_path = format!("{path}.{key}[{i}]");
                    if item.is_object() {
                        Some((item_path, item))
                    } else {
                        self.errors.push(format!("{item_path} must be an object"));
                        None
                    }
                })
                .collect(),
            Some(_) => {
                self.errors.push(format!("{path}.{key} must be an array"));
                Vec::new()
            }
            None => Vec::new(),
        }
    }
}

fn schema(value: &Value) -> Result<Roadmap, Errors> {
    let mut s = Schema { errors: Vec::new() };
    let root = "roadmap";
    let goal = s.nonempty(value, root, "goal");
    let target_users = s
        .objects(value, root, "targetUsers")
        .into_iter()
        .map(|(p, v)| TargetUser {
            name: s.nonempty(v, &p, "name"),
            description: s.string(v, &p, "description"),
            pain_points: s.strings(v, &p, "painPoints"),
        })
        .collect();
    let feature_map = s
        .objects(value, root, "featureMap")
        .into_iter()
        .map(|(p, v)| Feature {
            family: s.nonempty(v, &p, "family"),
            description: s.string(v, &p, "description"),
            features: s.strings(v, &p, "features"),
            relates_to: s.strings(v, &p, "relatesTo"),
        })
        .collect();
    let slices = s
        .objects(value, root, "slices")
        .into_iter()
        .map(|(p, v)| Slice {
            name: s.nonempty(v, &p, "name"),
            delivers: s.strings(v, &p, "delivers"),
            depends_on: s.strings(v, &p, "dependsOn"),
        })
        .collect();
    let requirements = s
        .objects(value, root, "requirements")
        .into_iter()
        .map(|(p, v)| Requirement {
            text: s.nonempty(v, &p, "text"),
            kind: s.nonempty(v, &p, "kind"),
            source_excerpt: s.string(v, &p, "sourceExcerpt"),
            slice_name: s.optional(v, &p, "sliceName"),
            verified_by: s.string(v, &p, "verifiedBy"),
            unsourced: false,
        })
        .collect();
    let deferred = s
        .objects(value, root, "deferred")
        .into_iter()
        .map(|(p, v)| Deferred {
            label: s.nonempty(v, &p, "label"),
            anchored_to: s.string(v, &p, "anchoredTo"),
        })
        .collect();
    let open_questions = s.strings(value, root, "openQuestions");
    if s.errors.is_empty() {
        Ok(Roadmap {
            goal,
            target_users,
            feature_map,
            slices,
            requirements,
            deferred,
            open_questions,
        })
    } else {
        Err(s.errors)
    }
}

/// Slice names are unique, every `dependsOn` names a slice, and there is no cycle.
fn graph(slices: &[Slice]) -> Result<(), Errors> {
    let mut errors = Vec::new();
    let mut index: HashMap<&str, usize> = HashMap::new();
    for (i, slice) in slices.iter().enumerate() {
        if index.insert(slice.name.as_str(), i).is_some() {
            errors.push(format!("slice name \"{}\" is used twice", slice.name));
        }
    }
    for slice in slices {
        for dep in &slice.depends_on {
            if !index.contains_key(dep.as_str()) {
                errors.push(format!(
                    "slice \"{}\" depends on \"{dep}\", which is not a slice",
                    slice.name
                ));
            } else if dep == &slice.name {
                errors.push(format!("slice \"{}\" depends on itself", slice.name));
            }
        }
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    if let Some(cycle) = find_cycle(slices, &index) {
        return Err(vec![format!(
            "slice dependencies form a cycle: {}",
            cycle.join(" → ")
        )]);
    }
    Ok(())
}

/// A cycle as slice names (first repeated at the end), via depth-first search.
fn find_cycle(slices: &[Slice], index: &HashMap<&str, usize>) -> Option<Vec<String>> {
    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        New,
        Open,
        Done,
    }
    fn visit(
        at: usize,
        slices: &[Slice],
        index: &HashMap<&str, usize>,
        marks: &mut [Mark],
        stack: &mut Vec<usize>,
    ) -> Option<Vec<String>> {
        marks[at] = Mark::Open;
        stack.push(at);
        for dep in &slices[at].depends_on {
            let Some(&next) = index.get(dep.as_str()) else {
                continue; // unknown names were reported before the cycle search
            };
            match marks[next] {
                Mark::Open => {
                    let from = stack.iter().position(|&i| i == next).unwrap_or(0);
                    let mut cycle: Vec<String> = stack[from..]
                        .iter()
                        .map(|&i| slices[i].name.clone())
                        .collect();
                    cycle.push(slices[next].name.clone());
                    return Some(cycle);
                }
                Mark::New => {
                    if let Some(cycle) = visit(next, slices, index, marks, stack) {
                        return Some(cycle);
                    }
                }
                Mark::Done => {}
            }
        }
        stack.pop();
        marks[at] = Mark::Done;
        None
    }
    let mut marks = vec![Mark::New; slices.len()];
    for start in 0..slices.len() {
        if marks[start] == Mark::New {
            if let Some(cycle) = visit(start, slices, index, &mut marks, &mut Vec::new()) {
                return Some(cycle);
            }
        }
    }
    None
}

/// Runs of whitespace (including newlines) become one space; trimmed.
pub fn normalize(text: &str) -> String {
    crate::core::text::normalize_ws(text)
}

/// The brain dump's paragraphs: blocks separated by blank lines. Headings (blocks made only
/// of `#` lines) and horizontal rules aren't paragraphs.
pub fn paragraphs(brain_dump: &str) -> Vec<String> {
    let text = brain_dump.replace("\r\n", "\n");
    let mut out = Vec::new();
    let mut block: Vec<&str> = Vec::new();
    let mut flush = |block: &mut Vec<&str>| {
        let is_heading = block.iter().all(|l| l.trim_start().starts_with('#'));
        let is_rule = block.len() == 1 && {
            let line = block[0].trim();
            line.len() >= 3 && line.chars().all(|c| matches!(c, '-' | '*' | '_' | ' '))
        };
        if !block.is_empty() && !is_heading && !is_rule {
            out.push(block.join("\n").trim().to_string());
        }
        block.clear();
    };
    for line in text.lines() {
        if line.trim().is_empty() {
            flush(&mut block);
        } else {
            block.push(line);
        }
    }
    flush(&mut block);
    out
}

/// Paragraphs that no sourced requirement's excerpt falls inside.
fn uncovered<'a>(
    brain_dump: &str,
    requirements: impl Iterator<Item = &'a Requirement>,
) -> Vec<String> {
    let excerpts: Vec<String> = requirements
        .filter(|r| !r.unsourced)
        .map(|r| normalize(&r.source_excerpt))
        .collect();
    paragraphs(brain_dump)
        .into_iter()
        .filter(|paragraph| {
            let paragraph = normalize(paragraph);
            !excerpts
                .iter()
                .any(|excerpt| paragraph.contains(excerpt.as_str()))
        })
        .collect()
}
