//! The user's note on a prompt: plain text the human attaches to a generated prompt, kept
//! beside it in `.foundry/prompts/notes/<name>` so regenerating the prompt never touches it.
//!
//! Written only by the prompts feature's `set_prompt_note` (the folder is in the protected
//! floor). At launch, a prompt with a note is sent as a copy in `.foundry/prompts/sent/<name>`:
//! the generated text, then [`NOTES_HEADING`], then the note exactly as written. Shared by the
//! prompts feature's `launch_message` and agent sessions' `agent_start`, so both name the
//! same file. The generated prompt itself is never changed.

use std::path::Path;

use super::launch_message::PromptPath;
use super::{paths, repo_write, AppError};

pub const NOTES_DIR: &str = ".foundry/prompts/notes";
pub const SENT_DIR: &str = ".foundry/prompts/sent";
/// A note is at most this many characters.
pub const MAX_NOTE_CHARS: usize = 8000;
pub const NOTES_HEADING: &str = "## Notes from the user";

/// Repo-relative path of the note for the prompt file `name` (e.g. `s-feed-ada.md`).
pub fn note_file(name: &str) -> String {
    format!("{NOTES_DIR}/{name}")
}

/// The note for the prompt file `name`; `None` without one (or with only whitespace).
pub fn read(repo: &Path, name: &str) -> Result<Option<String>, AppError> {
    Ok(repo_write::read_to_string(repo, &note_file(name))?.filter(|n| !n.trim().is_empty()))
}

/// Without a note, `prompt` as is. With one, writes the generated text plus the note to
/// `.foundry/prompts/sent/<name>` (replacing an older copy) and returns that file instead.
pub fn for_launch(repo: &Path, prompt: PromptPath) -> Result<PromptPath, AppError> {
    let Some(note) = read(repo, &prompt.name)? else {
        return Ok(prompt);
    };
    let mut text = std::fs::read_to_string(&prompt.path)?;
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push('\n');
    text.push_str(NOTES_HEADING);
    text.push_str("\n\n");
    text.push_str(&note);
    let rel = format!("{SENT_DIR}/{}", prompt.name);
    repo_write::write(repo, &rel, text)?;
    Ok(PromptPath {
        path: paths::resolve(repo, &rel)?,
        name: prompt.name,
        rel,
    })
}
