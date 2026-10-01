//! Shared building blocks. Core imports no feature.

pub mod error;
pub mod events;
pub mod fsx;
pub mod git;
pub mod ids;
pub mod paths;
pub mod repo_write;

pub use error::{AppError, ErrorCode};
