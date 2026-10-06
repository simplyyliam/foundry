//! Shared building blocks. Core imports no feature.

pub mod claude;
pub mod error;
pub mod events;
pub mod fsx;
pub mod git;
pub mod ids;
pub mod image;
pub mod llm;
pub mod paths;
pub mod process_tree;
pub mod repo_write;
pub mod report;
pub mod repos;
pub mod sessions;
pub mod slug;
pub mod subscription;
pub mod text;
pub mod trash;

pub use error::{AppError, ErrorCode};
