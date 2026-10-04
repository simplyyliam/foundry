//! The embedded AGENTS.md template: finding its `[[ FILL IN: … ]]` markers and filling them.

use crate::core::slug::slugify;
use crate::core::text::normalize_ws;

/// Embedded templates by id (`agentsTemplateId`). Only one exists today.
const TEMPLATES: &[(&str, &str)] = &[(
    "default",
    include_str!("../../../resources/templates/AGENTS.template.md"),
)];

const OPEN: &str = "[[ FILL IN";
const CLOSE: &str = "]]";

/// One `[[ FILL IN: … ]]` marker in a template.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    /// Stable for a given template: the slug of `label`.
    pub key: String,
    /// What the marker is for, e.g. "Architecture".
    pub label: String,
    /// The marker's own text, e.g. "e.g. Vite / React / TypeScript".
    pub hint: String,
    /// The template says it's optional: an empty value removes its line.
    pub optional: bool,
    start: usize,
    end: usize,
}

/// The template's text with LF line endings (the checkout may have made it CRLF).
pub fn text(template_id: &str) -> Option<String> {
    TEMPLATES
        .iter()
        .find(|(id, _)| *id == template_id)
        .map(|(_, text)| text.replace("\r\n", "\n"))
}

/// The template without what's only there for filling it in by hand: the usage note
/// (the blockquote under the title) and the title's " (TEMPLATE)" suffix.
pub fn body(template: &str) -> String {
    let mut out = Vec::new();
    let mut lines = template.lines().peekable();
    if let Some(title) = lines.next() {
        out.push(title.replace(" (TEMPLATE)", ""));
    }
    // Skip blank lines, then the blockquote, then the blank line after it.
    while lines.peek().is_some_and(|l| l.trim().is_empty()) {
        lines.next();
    }
    if lines.peek().is_some_and(|l| l.starts_with('>')) {
        while lines.peek().is_some_and(|l| l.starts_with('>')) {
            lines.next();
        }
    }
    out.push(String::new());
    while lines.peek().is_some_and(|l| l.trim().is_empty()) {
        lines.next();
    }
    out.extend(lines.map(str::to_string));
    let mut text = out.join("\n");
    if template.ends_with('\n') {
        text.push('\n');
    }
    text
}

/// The first `**bold**` run in `line`, if any.
fn bold(line: &str) -> Option<&str> {
    let start = line.find("**")? + 2;
    let len = line[start..].find("**")?;
    Some(&line[start..start + len])
}

/// Every marker in `body`, in order. A marker may wrap onto the next line.
pub fn markers(body: &str) -> Vec<Marker> {
    let mut out: Vec<Marker> = Vec::new();
    let mut at = 0;
    while let Some(found) = body[at..].find(OPEN) {
        let start = at + found;
        let Some(close) = body[start..].find(CLOSE) else {
            break;
        };
        let end = start + close + CLOSE.len();
        let inner = &body[start + OPEN.len()..end - CLOSE.len()];
        let hint = normalize_ws(inner.trim_start_matches(':'));
        let line_start = body[..start].rfind('\n').map_or(0, |i| i + 1);
        let line = &body[line_start..body[start..].find('\n').map_or(body.len(), |i| start + i)];
        let before = &body[line_start..start];
        let label = bold(line)
            .filter(|b| !b.contains(OPEN))
            .map(str::to_string)
            .or_else(|| {
                let prefix = before
                    .trim()
                    .trim_start_matches(['-', '*', ' '])
                    .trim_end_matches(':')
                    .trim();
                (!prefix.is_empty()).then(|| prefix.to_string())
            })
            .unwrap_or_else(|| {
                hint.split([':', '—'])
                    .next()
                    .unwrap_or(&hint)
                    .trim()
                    .to_string()
            });
        let label = label.trim_end_matches(':').trim().to_string();
        let mut key = slugify(&label);
        if key.is_empty() {
            key = "placeholder".into();
        }
        let base = key.clone();
        let mut n = 2;
        while out.iter().any(|m| m.key == key) {
            key = format!("{base}-{n}");
            n += 1;
        }
        out.push(Marker {
            key,
            optional: hint.to_ascii_lowercase().contains("optional"),
            label,
            hint,
            start,
            end,
        });
        at = end;
    }
    out
}

/// Fills every marker with its value. An optional marker with an empty value takes its whole
/// line with it (the template says to delete that bullet when it doesn't apply).
pub fn fill(body: &str, value: impl Fn(&Marker) -> String) -> String {
    let found = markers(body);
    let mut out = String::with_capacity(body.len());
    let mut at = 0;
    for marker in &found {
        let value = value(marker);
        if marker.optional && value.trim().is_empty() {
            let line_start = body[..marker.start].rfind('\n').map_or(0, |i| i + 1);
            let line_end = body[marker.end..]
                .find('\n')
                .map_or(body.len(), |i| marker.end + i + 1);
            out.push_str(&body[at..line_start.max(at)]);
            at = line_end;
            continue;
        }
        out.push_str(&body[at..marker.start]);
        out.push_str(value.trim());
        at = marker.end;
    }
    out.push_str(&body[at..]);
    out
}

pub fn contains_marker(text: &str) -> bool {
    text.contains(OPEN)
}
