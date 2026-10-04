use serde::{Deserialize, Serialize};
use specta::Type;

/// Stable, machine-readable error codes. Serialized as snake_case strings.
/// Adding a variant is an additive contract change; renaming one is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// A path argument was empty or malformed.
    InvalidPath,
    /// A path resolved outside the root it was joined onto.
    PathEscape,
    /// A filesystem or process I/O error.
    Io,
    /// A bug or an unexpected state inside the backend.
    Internal,
    /// An argument other than a path was empty, malformed or out of range.
    InvalidInput,
    /// The thing an id or path refers to doesn't exist.
    NotFound,
    /// A registered project's folder is gone (moved or deleted); relink it.
    FolderMissing,
    /// Refused to create a folder because one already exists there. Nothing was touched.
    FolderExists,
    /// The folder is already registered as a project.
    AlreadyLinked,
    /// A write into a project repo outside the AGENTS.md allowlist.
    WriteNotAllowed,
    /// The action needs the project to be archived first (e.g. `remove_project`).
    NotArchived,
    /// The Bakery already holds its maximum number of images.
    BakeryFull,
    /// Not a PNG, JPEG or WebP image (judged by its contents), or it can't be decoded.
    UnsupportedImage,
    /// An input file is over the size cap.
    FileTooLarge,
    /// Something is still running against it: an agent session (stop it first).
    SessionRunning,
    /// `maxConcurrentAgents` sessions are already running.
    TooManySessions,
    /// The command exists in the contract but its backend isn't built yet.
    NotImplemented,
    /// No `claude` executable on PATH or at `claudePath`.
    ClaudeNotFound,
    /// Claude Code isn't signed in.
    NotSignedIn,
    /// Signed in, but not with a Claude subscription (API key, console or a cloud provider).
    WrongAuthMode,
    /// The run was cancelled.
    Cancelled,
    /// The run hit its time limit and was stopped.
    TimedOut,
    /// Claude Code's output failed validation, even after a retry.
    GenerationFailed,
    /// A generation is already running for this project.
    GenerationRunning,
    /// The project is archived: unarchive it first (e.g. to start an agent in it).
    ProjectArchived,
    /// `claude` resolves to a `.cmd`/`.bat` shim (npm's install), which agent sessions don't
    /// use: they need the native `claude.exe`.
    ClaudeNeedsNative,
    /// `expectedRevision` doesn't match the plan's current revision: reload and retry.
    StaleRevision,
    /// The dependencies would form a cycle (the message names it).
    DependencyCycle,
    /// A slice can't start before its dependencies are done (unless overridden with a reason).
    DependenciesNotDone,
    /// A requirement can't be confirmed while it has no slice.
    RequirementUnassigned,
    /// A slice can't be removed while requirements, dependents or deferred items point at it.
    SliceInUse,
    /// Only `.md` files can go in the Design Library.
    UnsupportedFile,
    /// The project already has a confirmed team: add agents with `add_agent_manually`;
    /// re-proposing a confirmed team is out of scope (AGENTS.md, Law 6).
    TeamConfirmed,
    /// The project has no confirmed team yet (`confirm_team` first).
    TeamNotConfirmed,
    /// AGENTS.md placeholders still need confirming.
    PlaceholdersUnconfirmed,
    /// An AGENTS.md that differs already exists: pass `overwrite: true` (it's backed up).
    AgentsMdExists,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidPath => "invalid_path",
            Self::PathEscape => "path_escape",
            Self::Io => "io",
            Self::Internal => "internal",
            Self::InvalidInput => "invalid_input",
            Self::NotFound => "not_found",
            Self::FolderMissing => "folder_missing",
            Self::FolderExists => "folder_exists",
            Self::AlreadyLinked => "already_linked",
            Self::WriteNotAllowed => "write_not_allowed",
            Self::NotArchived => "not_archived",
            Self::BakeryFull => "bakery_full",
            Self::UnsupportedImage => "unsupported_image",
            Self::FileTooLarge => "file_too_large",
            Self::SessionRunning => "session_running",
            Self::TooManySessions => "too_many_sessions",
            Self::NotImplemented => "not_implemented",
            Self::ClaudeNotFound => "claude_not_found",
            Self::NotSignedIn => "not_signed_in",
            Self::WrongAuthMode => "wrong_auth_mode",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
            Self::GenerationFailed => "generation_failed",
            Self::GenerationRunning => "generation_running",
            Self::ProjectArchived => "project_archived",
            Self::ClaudeNeedsNative => "claude_needs_native",
            Self::StaleRevision => "stale_revision",
            Self::DependencyCycle => "dependency_cycle",
            Self::DependenciesNotDone => "dependencies_not_done",
            Self::RequirementUnassigned => "requirement_unassigned",
            Self::SliceInUse => "slice_in_use",
            Self::UnsupportedFile => "unsupported_file",
            Self::TeamConfirmed => "team_confirmed",
            Self::TeamNotConfirmed => "team_not_confirmed",
            Self::PlaceholdersUnconfirmed => "placeholders_unconfirmed",
            Self::AgentsMdExists => "agents_md_exists",
        }
    }
}

/// The error every command returns. Serializes to exactly `{ code, message }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, thiserror::Error)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.as_str(), self.message)
    }
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn invalid_path(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidPath, message)
    }

    pub fn path_escape(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::PathEscape, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }
}

impl From<std::io::Error> for AppError {
    fn from(err: std::io::Error) -> Self {
        Self::new(ErrorCode::Io, err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn app_error_serializes_to_exactly_code_and_message() {
        let err = AppError::path_escape("nope");
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value, json!({ "code": "path_escape", "message": "nope" }));
        assert_eq!(value.as_object().unwrap().len(), 2);
    }

    #[test]
    fn app_error_round_trips_through_json() {
        let err = AppError::new(ErrorCode::Io, "disk full");
        let text = serde_json::to_string(&err).unwrap();
        assert_eq!(text, r#"{"code":"io","message":"disk full"}"#);
        assert_eq!(serde_json::from_str::<AppError>(&text).unwrap(), err);
    }

    #[test]
    fn error_code_strings_match_serde() {
        for code in [
            ErrorCode::InvalidPath,
            ErrorCode::PathEscape,
            ErrorCode::Io,
            ErrorCode::Internal,
            ErrorCode::InvalidInput,
            ErrorCode::NotFound,
            ErrorCode::FolderMissing,
            ErrorCode::FolderExists,
            ErrorCode::AlreadyLinked,
            ErrorCode::WriteNotAllowed,
            ErrorCode::NotArchived,
            ErrorCode::BakeryFull,
            ErrorCode::UnsupportedImage,
            ErrorCode::FileTooLarge,
            ErrorCode::SessionRunning,
            ErrorCode::TooManySessions,
            ErrorCode::NotImplemented,
            ErrorCode::ClaudeNotFound,
            ErrorCode::NotSignedIn,
            ErrorCode::WrongAuthMode,
            ErrorCode::Cancelled,
            ErrorCode::TimedOut,
            ErrorCode::GenerationFailed,
            ErrorCode::GenerationRunning,
            ErrorCode::ProjectArchived,
            ErrorCode::ClaudeNeedsNative,
            ErrorCode::StaleRevision,
            ErrorCode::DependencyCycle,
            ErrorCode::DependenciesNotDone,
            ErrorCode::RequirementUnassigned,
            ErrorCode::SliceInUse,
            ErrorCode::UnsupportedFile,
            ErrorCode::TeamConfirmed,
            ErrorCode::TeamNotConfirmed,
            ErrorCode::PlaceholdersUnconfirmed,
            ErrorCode::AgentsMdExists,
        ] {
            assert_eq!(serde_json::to_value(code).unwrap(), json!(code.as_str()));
        }
    }

    #[test]
    fn io_errors_map_to_io_code() {
        let err: AppError = std::io::Error::other("boom").into();
        assert_eq!(err.code, ErrorCode::Io);
        assert_eq!(err.message, "boom");
    }
}
