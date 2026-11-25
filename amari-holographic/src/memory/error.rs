//! Error types for holographic memory operations.

use core::fmt;

/// Errors that can occur during holographic memory operations.
#[derive(Debug, Clone)]
pub enum HolographicError {
    /// Nonfinite or empty coefficient representation.
    InvalidData,
    /// Outside the checked retrieval-key domain.
    InvalidKey,
    /// Source and destination have different tracking or configuration.
    IncompatibleMemory,
    /// Number of records cannot be represented.
    CountOverflow,
    /// An underlying algebra operation failed.
    Algebra {
        /// The operation that failed.
        operation: String,
        /// The algebra-level cause, preserved for diagnostics.
        source: crate::algebra::AlgebraError,
    },

    /// Memory at capacity (SNR below threshold)
    AtCapacity {
        /// Current signal-to-noise ratio
        snr: f64,
        /// Threshold that was violated
        threshold: f64,
    },

    /// Cannot compute inverse: magnitude too small
    SingularInverse(f64),

    /// Dimension mismatch between operands
    DimensionMismatch {
        /// Expected dimension
        expected: usize,
        /// Actual dimension received
        actual: usize,
    },

    /// Resonator failed to converge
    ResonatorDidNotConverge(usize),

    /// Invalid temperature parameter
    InvalidTemperature(f64),

    /// Empty codebook provided to resonator
    EmptyCodebook,

    /// Numerical instability detected
    NumericalInstability(alloc::string::String),
}

impl fmt::Display for HolographicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidData => write!(f, "invalid memory coefficient data"),
            Self::InvalidKey => write!(f, "invalid retrieval key"),
            Self::IncompatibleMemory => write!(f, "incompatible memory tracking or configuration"),
            Self::CountOverflow => write!(f, "memory record count overflow"),
            Self::Algebra { operation, source } => {
                write!(f, "algebra operation {operation} failed: {source}")
            }
            Self::AtCapacity { snr, threshold } => {
                write!(
                    f,
                    "Memory at capacity (SNR {:.2} below threshold {:.2})",
                    snr, threshold
                )
            }
            Self::SingularInverse(magnitude) => {
                write!(
                    f,
                    "Cannot compute inverse: magnitude {} too small",
                    magnitude
                )
            }
            Self::DimensionMismatch { expected, actual } => {
                write!(
                    f,
                    "Dimension mismatch: expected {}, got {}",
                    expected, actual
                )
            }
            Self::ResonatorDidNotConverge(iterations) => {
                write!(
                    f,
                    "Resonator failed to converge after {} iterations",
                    iterations
                )
            }
            Self::InvalidTemperature(beta) => {
                write!(f, "Invalid temperature parameter: {}", beta)
            }
            Self::EmptyCodebook => {
                write!(f, "Cannot create resonator with empty codebook")
            }
            Self::NumericalInstability(msg) => {
                write!(f, "Numerical instability: {}", msg)
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for HolographicError {}

/// Result type for holographic operations.
pub type HolographicResult<T> = Result<T, HolographicError>;
