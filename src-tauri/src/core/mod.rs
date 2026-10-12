//! Shared building blocks. Core imports no feature.

pub mod claude;
pub mod error;
pub mod events;
pub mod fsx;
pub mod git;
pub mod glob;
pub mod hook;
pub mod hook_policy;
pub mod ids;
pub mod image;
pub mod launch_message;
pub mod llm;
pub mod paths;
pub mod process_tree;
pub mod repo_write;
pub mod report;
pub mod repos;
pub mod sessions;
pub mod setup;
pub mod slug;
pub mod subscription;
pub mod text;
pub mod time;
pub mod trash;
pub mod workspace;

pub use error::{AppError, ErrorCode};
