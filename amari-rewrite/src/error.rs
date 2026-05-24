use alloc::string::String;
use thiserror::Error;

/// Errors returned by rewrite operations.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RewriteError {
    /// A child index is outside the valid range for a node.
    #[error("invalid child index {index}")]
    InvalidChildIndex { index: usize },
    /// A path does not identify a valid subterm.
    #[error("invalid path")]
    InvalidPath,
    /// A rewrite or normalization step limit was reached.
    #[error("rewrite step limit reached")]
    StepLimitReached,
    /// A bounded search node limit was reached.
    #[error("node limit reached")]
    NodeLimitReached,
    /// A rewrite rule failed validation.
    #[error("invalid rule: {message}")]
    InvalidRule { message: String },
}

/// Result type used throughout `amari-rewrite`.
pub type RewriteResult<T> = Result<T, RewriteError>;
