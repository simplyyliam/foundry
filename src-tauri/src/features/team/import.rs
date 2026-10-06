//! Reading a dropped `.md` file as one handbook role. Only reads: the file is never modified
//! or copied.
//!
//! Optional frontmatter between `---` lines at the very top, with the keys `title`,
//! `description`, `writes` and `protected`. Lists are inline (`[a, b]`) or `- item` lines.
//! That small subset is parsed here by hand; anything else is an error naming the line.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use super::markdown;
use super::NewHandbookRole;
use crate::core::paths::{self, PathKind};
use crate::core::{AppError, ErrorCode};

/// Each file's cap.
pub const MAX_FILE_BYTES: u64 = 100 * 1024;
/// Files per call.
pub const MAX_FILES: usize = 50;
/// A description taken from the body is cut to this many characters.
const DESCRIPTION_CHARS: usize = 200;
const KEYS: [&str; 4] = ["title", "description", "writes", "protected"];

/// Reads and parses one dropped file (an absolute path). Returns the file name (for errors)
/// and the role, not yet validated.
pub fn read_role(source: &str) -> Result<(String, NewHandbookRole), AppError> {
    let path = paths::check(Path::new(source), PathKind::Absolute)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .ok_or_else(|| AppError::invalid_path(format!("not a file: {source}")))?;
    let is_md = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("md"));
    if !is_md {
        return Err(AppError::new(
            ErrorCode::UnsupportedFile,
            format!("{name}: only .md files can become roles"),
        ));
    }
    let meta = fs::metadata(&path)
        .map_err(|err| AppError::new(ErrorCode::Io, format!("{name}: {err}")))?;
    if !meta.is_file() {
        return Err(AppError::invalid_path(format!("{name}: not a file")));
    }
    let too_large = || {
        AppError::new(
            ErrorCode::FileTooLarge,
            format!("{name} is over the 100 KB limit"),
        )
    };
    if meta.len() > MAX_FILE_BYTES {
        return Err(too_large());
    }
    let bytes =
        fs::read(&path).map_err(|err| AppError::new(ErrorCode::Io, format!("{name}: {err}")))?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(too_large());
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| AppError::invalid_input(format!("{name}: not UTF-8 text")))?;
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let role = parse(&name, &stem, &text)?;
    Ok((name, role))
}

#[derive(Default)]
struct Frontmatter {
    title: Option<String>,
    description: Option<String>,
    writes: Option<Vec<String>>,
    protected: Option<Vec<String>>,
}

/// One role from a file's text. `name` labels errors; `stem` is the last-resort title.
pub fn parse(name: &str, stem: &str, text: &str) -> Result<NewHandbookRole, AppError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = text.split('\n').collect();

    let (front, body) = if lines.first().map(|l| l.trim_end()) == Some("---") {
        let close = lines
            .iter()
            .skip(1)
            .position(|l| l.trim_end() == "---")
            .map(|i| i + 1)
            .ok_or_else(|| {
                AppError::invalid_input(format!(
                    "{name}, line 1: the frontmatter opened here is never closed with ---"
                ))
            })?;
        (
            parse_frontmatter(name, &lines[1..close])?,
            lines[close + 1..].join("\n"),
        )
    } else {
        (Frontmatter::default(), text.clone())
    };

    let title = front
        .title
        .or_else(|| markdown::first_title(&body))
        .unwrap_or_else(|| stem.to_string());
    let description = front.description.unwrap_or_else(|| {
        markdown::first_paragraph(&body)
            .map(|p| p.chars().take(DESCRIPTION_CHARS).collect::<String>())
            .unwrap_or_default()
    });
    Ok(NewHandbookRole {
        title,
        description,
        typical_write_globs: front.writes.unwrap_or_default(),
        typical_protected_globs: front.protected.unwrap_or_default(),
        skill_template: body,
    })
}

/// `lines` are the frontmatter's lines; line numbers in errors count from the file's top.
fn parse_frontmatter(name: &str, lines: &[&str]) -> Result<Frontmatter, AppError> {
    let mut front = Frontmatter::default();
    let mut seen: HashSet<&'static str> = HashSet::new();
    // The list key whose `- item` lines may follow.
    let mut open_list: Option<&'static str> = None;
    for (i, raw) in lines.iter().enumerate() {
        let at = i + 2;
        let fail = |reason: &str| {
            AppError::invalid_input(format!("{name}, line {at}: {reason}: \"{}\"", raw.trim()))
        };
        let line = raw.trim_end();
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some(item) = trimmed
            .strip_prefix("- ")
            .or((trimmed == "-").then_some(""))
        {
            let key = open_list
                .ok_or_else(|| fail("a list item must follow \"writes:\" or \"protected:\""))?;
            let item = unquote(item.trim()).map_err(&fail)?;
            if item.is_empty() {
                return Err(fail("empty list item"));
            }
            let list = if key == "writes" {
                &mut front.writes
            } else {
                &mut front.protected
            };
            list.get_or_insert_with(Vec::new).push(item);
            continue;
        }
        if line.starts_with([' ', '\t']) {
            return Err(fail("unexpected indentation"));
        }
        let (key, value) = line
            .split_once(':')
            .ok_or_else(|| fail("expected \"key: value\""))?;
        let value = value.trim();
        let Some(key) = KEYS.iter().copied().find(|k| *k == key.trim()) else {
            return Err(fail(
                "unknown key (expected title, description, writes or protected)",
            ));
        };
        if !seen.insert(key) {
            return Err(fail(&format!("{key} is set twice")));
        }
        open_list = None;
        match key {
            "title" | "description" => {
                let value = unquote(value).map_err(&fail)?;
                // Blank means not given: the fallbacks apply.
                let value = Some(value).filter(|v| !v.trim().is_empty());
                if key == "title" {
                    front.title = value;
                } else {
                    front.description = value;
                }
            }
            _ => {
                let list = if value.is_empty() {
                    open_list = Some(key);
                    Vec::new()
                } else if let Some(inner) = value.strip_prefix('[') {
                    let inner = inner
                        .strip_suffix(']')
                        .ok_or_else(|| fail("an inline list must end with ]"))?;
                    inline_list(inner).map_err(&fail)?
                } else {
                    vec![unquote(value).map_err(&fail)?]
                };
                if key == "writes" {
                    front.writes = Some(list);
                } else {
                    front.protected = Some(list);
                }
            }
        }
    }
    Ok(front)
}

/// `"x"` or `'x'` → `x`; anything else as written.
fn unquote(value: &str) -> Result<String, &'static str> {
    for quote in ['"', '\''] {
        if let Some(rest) = value.strip_prefix(quote) {
            let inner = rest
                .strip_suffix(quote)
                .ok_or("a quoted value must end with the same quote")?;
            return Ok(if quote == '"' {
                inner.replace("\\\"", "\"").replace("\\\\", "\\")
            } else {
                inner.replace("''", "'")
            });
        }
    }
    Ok(value.to_string())
}

/// The inside of `[a, "b, c", d]`: commas inside quotes don't split.
fn inline_list(inner: &str) -> Result<Vec<String>, &'static str> {
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut items = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in inner.chars() {
        match (quote, ch) {
            (None, ',') => {
                items.push(std::mem::take(&mut current));
                continue;
            }
            (None, '"' | '\'') => quote = Some(ch),
            (Some(q), c) if c == q => quote = None,
            _ => {}
        }
        current.push(ch);
    }
    if quote.is_some() {
        return Err("a quoted list item is never closed");
    }
    items.push(current);
    items
        .iter()
        .map(|item| {
            let item = unquote(item.trim())?;
            if item.is_empty() {
                Err("empty list item")
            } else {
                Ok(item)
            }
        })
        .collect()
}
