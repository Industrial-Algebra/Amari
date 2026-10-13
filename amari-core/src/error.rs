//! Error types for core geometric algebra operations

#[cfg(feature = "std")]
use thiserror::Error;

#[cfg(not(feature = "std"))]
use core::fmt;

/// Error types for core geometric algebra operations
#[cfg(feature = "std")]
#[derive(Error, Debug, Clone, PartialEq)]
pub enum CoreError {
    /// Invalid dimension for operation
    #[error("Invalid dimension: expected {expected}, got {actual}")]
    InvalidDimension { expected: usize, actual: usize },

    /// Division by zero
    #[error("Division by zero")]
    DivisionByZero,

    /// Numerical instability detected
    #[error("Numerical instability detected")]
    NumericalInstability,

    /// Invalid basis vector index
    #[error("Invalid basis vector index: {0} (max: {1})")]
    InvalidBasisIndex(usize, usize),

    /// Matrix is singular (non-invertible)
    #[error("Matrix is singular and cannot be inverted")]
    SingularMatrix,

    /// Invalid metric signature
    #[error("Invalid metric signature: positive {positive} + negative {negative} + zero {zero} != dimension {dimension}")]
    InvalidSignature {
        positive: usize,
        negative: usize,
        zero: usize,
        dimension: usize,
    },

    /// GF(2) dimension mismatch
    #[error("GF(2) dimension mismatch: expected {expected}, got {got}")]
    GF2DimensionMismatch { expected: usize, got: usize },

    /// GF(2) matrix is not square
    #[error("GF(2) matrix is not square: {rows}x{cols}")]
    GF2NotSquare { rows: usize, cols: usize },

    /// GF(2) index out of bounds
    #[error("GF(2) index {index} out of bounds for dimension {dim}")]
    GF2IndexOutOfBounds { index: usize, dim: usize },
}

/// Stage of the exponential route at which a non-finite value appeared.
///
/// Contract: `docs/plans/2026-10-10-amari-core-w03-inverse-exp-contract.md`
/// (D02.3). Each variant names the point of the frozen `exp()` routing order.
#[cfg(feature = "std")]
#[derive(Error, Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonFiniteStage {
    /// Pure-scalar `f64::exp` route.
    #[error("scalar route")]
    ScalarRoute,
    /// Simple-bivector closed form.
    #[error("closed form")]
    ClosedForm,
    /// Scaling step of the series route.
    #[error("series scaling")]
    SeriesScaling,
    /// Term accumulation of the series route.
    #[error("series accumulation")]
    SeriesAccumulation,
    /// Repeated squaring of the series route.
    #[error("squaring")]
    Squaring,
}

/// Failure of the exponential map `exp()`.
///
/// Contract: `docs/plans/2026-10-10-amari-core-w03-inverse-exp-contract.md`
/// (D02.3/D02.4). `NoConvergence` reports the bounded `exp_series()` loop
/// exhausting its hard cap; `NonFiniteResult` reports a non-finite value at the
/// named routing stage. Both are honest numerical accept/reject results, not
/// exact-arithmetic certificates.
#[cfg(feature = "std")]
#[derive(Error, Debug, Clone, Copy, PartialEq)]
pub enum ExpError {
    /// The series route hit its hard term cap without meeting tolerance.
    #[error(
        "exponential series failed to converge after {terms} terms \
         (last term norm {last_term_norm}, result scale {result_scale})"
    )]
    NoConvergence {
        /// Number of terms evaluated before the hard cap.
        terms: usize,
        /// Coefficient norm of the final term.
        last_term_norm: f64,
        /// Coefficient norm of the accumulated result.
        result_scale: f64,
    },
    /// A non-finite value was produced during the named routing stage.
    #[error("exponential produced a non-finite result during {stage}")]
    NonFiniteResult {
        /// The stage where the non-finite value appeared.
        stage: NonFiniteStage,
    },
}

/// Error types for core geometric algebra operations (no_std version)
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, PartialEq)]
pub enum CoreError {
    /// Invalid dimension for operation
    InvalidDimension { expected: usize, actual: usize },

    /// Division by zero
    DivisionByZero,

    /// Numerical instability detected
    NumericalInstability,

    /// Invalid basis vector index
    InvalidBasisIndex(usize, usize),

    /// Matrix is singular (non-invertible)
    SingularMatrix,

    /// Invalid metric signature
    InvalidSignature {
        positive: usize,
        negative: usize,
        zero: usize,
        dimension: usize,
    },

    /// GF(2) dimension mismatch
    GF2DimensionMismatch { expected: usize, got: usize },

    /// GF(2) matrix is not square
    GF2NotSquare { rows: usize, cols: usize },

    /// GF(2) index out of bounds
    GF2IndexOutOfBounds { index: usize, dim: usize },
}

/// Stage of the exponential route at which a non-finite value appeared.
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonFiniteStage {
    /// Pure-scalar `f64::exp` route.
    ScalarRoute,
    /// Simple-bivector closed form.
    ClosedForm,
    /// Scaling step of the series route.
    SeriesScaling,
    /// Term accumulation of the series route.
    SeriesAccumulation,
    /// Repeated squaring of the series route.
    Squaring,
}

/// Failure of the exponential map `exp()` (no_std version).
#[cfg(not(feature = "std"))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExpError {
    /// The series route hit its hard term cap without meeting tolerance.
    NoConvergence {
        /// Number of terms evaluated before the hard cap.
        terms: usize,
        /// Coefficient norm of the final term.
        last_term_norm: f64,
        /// Coefficient norm of the accumulated result.
        result_scale: f64,
    },
    /// A non-finite value was produced during the named routing stage.
    NonFiniteResult {
        /// The stage where the non-finite value appeared.
        stage: NonFiniteStage,
    },
}

#[cfg(not(feature = "std"))]
impl fmt::Display for NonFiniteStage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NonFiniteStage::ScalarRoute => write!(f, "scalar route"),
            NonFiniteStage::ClosedForm => write!(f, "closed form"),
            NonFiniteStage::SeriesScaling => write!(f, "series scaling"),
            NonFiniteStage::SeriesAccumulation => write!(f, "series accumulation"),
            NonFiniteStage::Squaring => write!(f, "squaring"),
        }
    }
}

#[cfg(not(feature = "std"))]
impl fmt::Display for ExpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExpError::NoConvergence {
                terms,
                last_term_norm,
                result_scale,
            } => write!(
                f,
                "exponential series failed to converge after {} terms \
                 (last term norm {}, result scale {})",
                terms, last_term_norm, result_scale
            ),
            ExpError::NonFiniteResult { stage } => {
                write!(
                    f,
                    "exponential produced a non-finite result during {}",
                    stage
                )
            }
        }
    }
}

#[cfg(not(feature = "std"))]
impl fmt::Display for CoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CoreError::InvalidDimension { expected, actual } => {
                write!(
                    f,
                    "Invalid dimension: expected {}, got {}",
                    expected, actual
                )
            }
            CoreError::DivisionByZero => {
                write!(f, "Division by zero")
            }
            CoreError::NumericalInstability => {
                write!(f, "Numerical instability detected")
            }
            CoreError::InvalidBasisIndex(idx, max) => {
                write!(f, "Invalid basis vector index: {} (max: {})", idx, max)
            }
            CoreError::SingularMatrix => {
                write!(f, "Matrix is singular and cannot be inverted")
            }
            CoreError::InvalidSignature {
                positive,
                negative,
                zero,
                dimension,
            } => {
                write!(
                    f,
                    "Invalid metric signature: positive {} + negative {} + zero {} != dimension {}",
                    positive, negative, zero, dimension
                )
            }
            CoreError::GF2DimensionMismatch { expected, got } => {
                write!(
                    f,
                    "GF(2) dimension mismatch: expected {}, got {}",
                    expected, got
                )
            }
            CoreError::GF2NotSquare { rows, cols } => {
                write!(f, "GF(2) matrix is not square: {}x{}", rows, cols)
            }
            CoreError::GF2IndexOutOfBounds { index, dim } => {
                write!(
                    f,
                    "GF(2) index {} out of bounds for dimension {}",
                    index, dim
                )
            }
        }
    }
}

/// Result type for core operations
pub type CoreResult<T> = Result<T, CoreError>;
