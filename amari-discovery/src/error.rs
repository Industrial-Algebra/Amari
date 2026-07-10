// SPDX-License-Identifier: MIT OR Apache-2.0

//! Structured failures for discovery operations.

use thiserror::Error;

/// A result produced by the discovery engine.
pub type DiscoveryResult<T> = Result<T, DiscoveryError>;

/// A process-level failure produced by the discovery engine.
///
/// Domain outcomes such as insufficient evidence are represented by
/// [`crate::DiscoveryOutcome`], not by this error type.
#[derive(Debug, Error)]
pub enum DiscoveryError {
    /// A protocol identifier is malformed or uses the wrong namespace.
    #[error("invalid identifier `{value}`: {reason}")]
    InvalidId {
        /// The rejected identifier.
        value: String,
        /// The validation failure.
        reason: String,
    },

    /// User or adapter input is malformed.
    #[error("invalid input: {0}")]
    InvalidInput(String),

    /// The generated or curated catalog violates an invariant.
    #[error("catalog corruption: {0}")]
    CatalogCorruption(String),

    /// Project inspection could not be completed safely.
    #[error("inspection failed: {0}")]
    InspectionFailure(String),

    /// A known probe has no executable adapter in this build.
    #[error("probe unavailable: {0}")]
    ProbeUnavailable(String),

    /// A registered probe failed during execution.
    #[error("probe failed: {0}")]
    ProbeFailed(String),

    /// A bounded operation exceeded a declared resource limit.
    #[error("resource limit exceeded: {0}")]
    LimitExceeded(String),

    /// A required read or write operation failed.
    #[error("I/O failure: {0}")]
    Io(#[from] std::io::Error),

    /// JSON protocol encoding or decoding failed.
    #[error("serialization failure: {0}")]
    Serialization(#[from] serde_json::Error),

    /// An internal invariant failed without a more specific classification.
    #[error("internal failure: {0}")]
    Internal(String),
}

impl DiscoveryError {
    /// Creates an invalid-identifier error.
    pub fn invalid_id(value: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidId {
            value: value.into(),
            reason: reason.into(),
        }
    }

    /// Returns the stable machine-readable error kind.
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::InvalidId { .. } => "invalid_id",
            Self::InvalidInput(_) => "invalid_input",
            Self::CatalogCorruption(_) => "catalog_corruption",
            Self::InspectionFailure(_) => "inspection_failure",
            Self::ProbeUnavailable(_) => "probe_unavailable",
            Self::ProbeFailed(_) => "probe_failed",
            Self::LimitExceeded(_) => "limit_exceeded",
            Self::Io(_) => "io",
            Self::Serialization(_) => "serialization",
            Self::Internal(_) => "internal",
        }
    }

    /// Returns the stable process exit code for this failure class.
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::InvalidId { .. } | Self::InvalidInput(_) => 2,
            Self::CatalogCorruption(_) => 3,
            Self::InspectionFailure(_) => 4,
            Self::ProbeUnavailable(_) => 5,
            Self::ProbeFailed(_) => 6,
            Self::LimitExceeded(_) => 7,
            Self::Io(_) => 8,
            Self::Serialization(_) => 9,
            Self::Internal(_) => 70,
        }
    }
}
