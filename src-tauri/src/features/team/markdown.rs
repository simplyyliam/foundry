//! Just enough Markdown structure for role skills: ATX (`## x`) and setext (`x` over `===` or
//! `---`) headings, paragraphs, and fenced code (whose lines are never headings). Input is
//! LF-only text.

/// The level the role file's five required headings sit at (`## Reads` …).
const REQUIRED_LEVEL: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// A fence line, or a line inside a fenced block.
    Code,
    Blank,
    /// Paragraph text (including list items).
    Text,
    Atx(usize),
    /// A paragraph line that a setext underline turned into a heading.
    SetextText(usize),
    SetextRule,
}

/// The line without up to three leading spaces; `None` when it's indented further.
fn unindented(line: &str) -> Option<&str> {
    let spaces = line.len() - line.trim_start_matches(' ').len();
    (spaces <= 3).then(|| &line[spaces..])
}

/// A fence opener or closer: its character and run length.
fn fence(line: &str) -> Option<(char, usize, &str)> {
    let rest = unindented(line)?;
    let ch = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let run = rest.len() - rest.trim_start_matches(ch).len();
    (run >= 3).then(|| (ch, run, &rest[run..]))
}

fn atx_level(line: &str) -> Option<usize> {
    let rest = unindented(line)?;
    let hashes = rest.len() - rest.trim_start_matches('#').len();
    let after = &rest[hashes..];
    ((1..=6).contains(&hashes) && (after.is_empty() || after.starts_with([' ', '\t'])))
        .then_some(hashes)
}

fn setext_level(line: &str) -> Option<usize> {
    let rest = unindented(line)?.trim_end();
    if rest.is_empty() {
        None
    } else if rest.chars().all(|c| c == '=') {
        Some(1)
    } else if rest.chars().all(|c| c == '-') {
        Some(2)
    } else {
        None
    }
}

fn is_list_item(line: &str) -> bool {
    let rest = line.trim_start();
    if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
        return true;
    }
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    digits > 0 && (rest[digits..].starts_with(". ") || rest[digits..].starts_with(") "))
}

fn classify(lines: &[&str]) -> Vec<Kind> {
    let mut kinds: Vec<Kind> = Vec::with_capacity(lines.len());
    let mut open: Option<(char, usize)> = None;
    for line in lines {
        if let Some((ch, len)) = open {
            if let Some((c, run, rest)) = fence(line) {
                if c == ch && run >= len && rest.trim().is_empty() {
                    open = None;
                }
            }
            kinds.push(Kind::Code);
            continue;
        }
        if let Some((ch, run, _)) = fence(line) {
            open = Some((ch, run));
            kinds.push(Kind::Code);
            continue;
        }
        let kind = if line.trim().is_empty() {
            Kind::Blank
        } else if let Some(level) = atx_level(line) {
            Kind::Atx(level)
        } else if let Some(level) = setext_level(line).filter(|_| {
            // The paragraph above must be plain text: `- item` over `---` stays a list.
            let start = kinds
                .iter()
                .rposition(|k| *k != Kind::Text)
                .map_or(0, |i| i + 1);
            start < kinds.len() && !is_list_item(lines[start])
        }) {
            for k in kinds.iter_mut().rev().take_while(|k| **k == Kind::Text) {
                *k = Kind::SetextText(level);
            }
            Kind::SetextRule
        } else {
            Kind::Text
        };
        kinds.push(kind);
    }
    kinds
}

/// The text of an ATX heading line, without its markers or closing hashes.
fn atx_text(line: &str) -> &str {
    let text = line.trim().trim_start_matches('#').trim();
    let closed = text.trim_end_matches('#');
    if closed.is_empty() || closed.ends_with([' ', '\t']) {
        closed.trim_end()
    } else {
        text
    }
}

/// Pushes every heading in `text` below the role file's required headings (level 3 and
/// deeper), keeping their relative levels and capping at 6. Setext headings become ATX ones.
/// Text with no heading above level 3 comes back unchanged.
pub fn demote_headings(text: &str) -> String {
    let lines: Vec<&str> = text.split('\n').collect();
    let kinds = classify(&lines);
    let top = kinds
        .iter()
        .filter_map(|k| match k {
            Kind::Atx(l) | Kind::SetextText(l) => Some(*l),
            _ => None,
        })
        .min();
    let Some(top) = top.filter(|t| *t <= REQUIRED_LEVEL) else {
        return text.to_string();
    };
    let shift = REQUIRED_LEVEL + 1 - top;
    let hashes = |level: usize| "#".repeat((level + shift).min(6));

    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    let mut i = 0;
    while i < lines.len() {
        match kinds[i] {
            Kind::Atx(level) => {
                let text = atx_text(lines[i]);
                out.push(if text.is_empty() {
                    hashes(level)
                } else {
                    format!("{} {text}", hashes(level))
                });
                i += 1;
            }
            Kind::SetextText(level) => {
                let start = i;
                while kinds.get(i) == Some(&Kind::SetextText(level)) {
                    i += 1;
                }
                let joined: Vec<&str> = lines[start..i].iter().map(|l| l.trim()).collect();
                out.push(format!("{} {}", hashes(level), joined.join(" ")));
                // Skip the underline.
                i += 1;
            }
            _ => {
                out.push(lines[i].to_string());
                i += 1;
            }
        }
    }
    out.join("\n")
}

/// The text of the first `# ` heading, if any.
pub fn first_title(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.split('\n').collect();
    let kinds = classify(&lines);
    lines
        .iter()
        .zip(&kinds)
        .filter(|(_, k)| **k == Kind::Atx(1))
        .map(|(l, _)| atx_text(l).to_string())
        .find(|t| !t.is_empty())
}

/// The first paragraph that isn't a heading (or code), its lines joined by single spaces.
pub fn first_paragraph(text: &str) -> Option<String> {
    let lines: Vec<&str> = text.split('\n').collect();
    let kinds = classify(&lines);
    let mut paragraph: Vec<&str> = Vec::new();
    for (line, kind) in lines.iter().zip(&kinds) {
        if *kind == Kind::Text {
            paragraph.push(line.trim());
        } else if !paragraph.is_empty() {
            break;
        }
    }
    (!paragraph.is_empty()).then(|| paragraph.join(" "))
}
