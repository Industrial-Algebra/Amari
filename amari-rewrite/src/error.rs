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
    /// No rule applies at the requested position.
    #[error("no rule applies: {rule_name}")]
    NoRule {
        /// The rule identity that failed to apply.
        rule_name: String,
    },
    /// A relation limit was zero or exceeded its fixed ceiling.
    #[error("invalid relation limit {resource}: {value} (ceiling {ceiling})")]
    InvalidLimit {
        /// The limit being set.
        resource: &'static str,
        /// The rejected value.
        value: usize,
        /// The fixed ceiling.
        ceiling: usize,
    },
    /// A relation resource budget was exhausted.
    #[error("relation resource exhausted: {resource} (limit {limit})")]
    RelationLimitExceeded {
        /// The exhausted resource.
        resource: &'static str,
        /// The configured limit.
        limit: usize,
    },
    /// A SHA-256 digest failed validation.
    #[error("invalid sha-256 digest: {message}")]
    InvalidDigest {
        /// Validation failure detail.
        message: String,
    },
    /// First-order unification failed (clash, arity mismatch, or
    /// occurs-check violation).
    #[error("unification failed: {reason}")]
    UnificationFailure {
        /// Failure detail.
        reason: String,
    },
    /// A substitution failed checked composition or validation.
    #[error("invalid substitution: {message}")]
    InvalidSubstitution {
        /// Validation failure detail.
        message: String,
    },
    /// A tree automaton failed validation (rank conflict, unknown
    /// state or symbol, arity mismatch, duplicate entry, invalid
    /// final state).
    #[error("malformed tree automaton: {message}")]
    MalformedAutomaton {
        /// Validation failure detail.
        message: String,
    },
    /// A language operation requires a ground term but received one
    /// containing variables.
    #[error("non-ground term: {message}")]
    NonGroundTerm {
        /// Failure detail.
        message: String,
    },
    /// A regular tree grammar failed validation (undeclared
    /// nonterminal, rank mismatch, unrenderable name) or a grammar
    /// text could not be parsed.
    #[error("malformed tree grammar: {message}")]
    MalformedGrammar {
        /// Validation or parse failure detail.
        message: String,
    },
    /// A residual failed validation or its reconstructed source digest
    /// did not match the recorded authority. Always a hard error: no
    /// degraded or warning-only reconstruction is ever returned.
    #[error("residual mismatch: {message}")]
    ResidualMismatch {
        /// Validation failure detail.
        message: String,
    },
}

/// Result type used throughout `amari-rewrite`.
pub type RewriteResult<T> = Result<T, RewriteError>;
