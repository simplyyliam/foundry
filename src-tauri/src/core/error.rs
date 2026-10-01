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
