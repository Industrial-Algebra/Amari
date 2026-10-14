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

// ======================================================================
// CORE-W04 error surface (contract §5)
// ======================================================================
//
// The W04 enums are defined once, independent of the `std` feature, with a
// manual `Display` implementation and (under `std`) an `std::error::Error`
// impl. This avoids the historical std/no_std duplication while keeping the
// existing `CoreError`/`ExpError` layout untouched.

/// Errors from rotor construction and domain-checked rotor operations.
///
/// Contract: `docs/plans/2026-10-13-amari-core-w04-rotor-duality-contract.md`
/// §5. `Exp(ExpError)` is a lossless carrier for the frozen W03 exponential
/// error surface; the `From<ExpError>` impl is the only conversion the
/// rotor constructors need.
#[derive(Debug, Clone, PartialEq)]
pub enum RotorError {
    /// An input to `from_vectors` / `reflect` was not a pure grade-1 element.
    NotAVector,
    /// `from_multivector_bivector` received a nonzero foreign-grade part.
    NotABivector,
    /// A normalization step had no positive square to scale by.
    NotNormalizable,
    /// The scalar part is outside the certified logarithm domain.
    NotALogDomain,
    /// A negative scalar cannot select a log branch plane.
    BranchAmbiguous,
    /// The logarithm class is not certified (multi-plane / non-simple).
    Uncertified,
    /// A null (zero-square) input vector.
    NullVector,
    /// Antipodal inputs that are not resolvable by a unique Euclidean plane.
    Antipodal,
    /// The two vectors have opposite causal type.
    CausalTypeMismatch,
    /// The connecting even versor is a zero divisor.
    NullCandidate,
    /// The connecting even versor lies off the identity component.
    SheetChange,
    /// A reflection normal is not a certified unit vector of square `±1`.
    NotUnitNormal,
    /// An odd-grade element was supplied where an even rotor is required.
    NotEven,
    /// The certified stored reverse norm is non-positive or non-scalar.
    Degenerate,
    /// Overflow inside an unnormalized composition step.
    Composition,
    /// The underlying exponential map failed; carried losslessly.
    Exp(ExpError),
}

impl From<ExpError> for RotorError {
    fn from(error: ExpError) -> Self {
        RotorError::Exp(error)
    }
}

impl core::fmt::Display for RotorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RotorError::NotAVector => write!(f, "expected a pure vector (grade 1)"),
            RotorError::NotABivector => write!(f, "expected a pure bivector (grade 2)"),
            RotorError::NotNormalizable => {
                write!(f, "element has no positive square to normalize by")
            }
            RotorError::NotALogDomain => write!(f, "scalar part is outside the log domain"),
            RotorError::BranchAmbiguous => {
                write!(f, "negative scalar selects no log branch plane")
            }
            RotorError::Uncertified => write!(f, "logarithm class is not certified"),
            RotorError::NullVector => write!(f, "null vector has zero square"),
            RotorError::Antipodal => write!(f, "antipodal vectors are not resolvable"),
            RotorError::CausalTypeMismatch => {
                write!(f, "vectors have opposite causal type")
            }
            RotorError::NullCandidate => {
                write!(f, "connecting even versor is a zero divisor")
            }
            RotorError::SheetChange => {
                write!(f, "connecting even versor lies off the identity component")
            }
            RotorError::NotUnitNormal => {
                write!(f, "reflection normal is not a unit vector of square ±1")
            }
            RotorError::NotEven => write!(f, "rotor must have even grade"),
            RotorError::Degenerate => {
                write!(f, "stored reverse norm is non-positive or non-scalar")
            }
            RotorError::Composition => write!(f, "composition overflowed the finite domain"),
            RotorError::Exp(error) => write!(f, "exponential failed: {error}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for RotorError {}

/// Errors from the metric Hodge dual operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DualError {
    /// The pseudoscalar is not invertible (degenerate signature).
    DegeneratePseudoscalar,
}

impl core::fmt::Display for DualError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DualError::DegeneratePseudoscalar => {
                write!(f, "pseudoscalar is not invertible (degenerate signature)")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DualError {}

/// Errors from checked `Multivector`-to-`Scalar`/`Vector`/`Bivector`
/// conversions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradeError {
    /// A coefficient outside the wrapper's grade is nonzero.
    ForeignGrade,
}

impl core::fmt::Display for GradeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            GradeError::ForeignGrade => {
                write!(f, "multivector has a nonzero foreign-grade coefficient")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for GradeError {}

/// Errors from checked bivector-component construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentError {
    /// The requested component index is out of range.
    OutOfRange,
}

impl core::fmt::Display for ComponentError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ComponentError::OutOfRange => write!(f, "component index out of range"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for ComponentError {}
