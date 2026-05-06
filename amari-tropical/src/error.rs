//! Error types for tropical algebra operations

use crate::ordinal::OrdinalId;
use thiserror::Error;

/// Error types for tropical algebra operations
#[derive(Error, Debug, Clone, PartialEq)]
pub enum TropicalError {
    /// Matrix dimension mismatch
    #[error("Matrix dimension mismatch: {0}")]
    MatrixDimensionMismatch(String),

    /// Invalid matrix dimensions for operation
    #[error("Invalid matrix dimensions for {operation}: ({rows1}x{cols1}) and ({rows2}x{cols2})")]
    InvalidMatrixOperation {
        /// The matrix operation that failed
        operation: String,
        /// Number of rows in the first matrix
        rows1: usize,
        /// Number of columns in the first matrix
        cols1: usize,
        /// Number of rows in the second matrix
        rows2: usize,
        /// Number of columns in the second matrix
        cols2: usize,
    },

    /// Negative infinity arithmetic error
    #[error("Invalid tropical arithmetic: negative infinity operation")]
    NegativeInfinityError,

    /// Invalid tropical polynomial
    #[error("Invalid tropical polynomial: {0}")]
    InvalidPolynomial(String),

    /// Numerical overflow in tropical computation
    #[error("Numerical overflow in tropical computation")]
    Overflow,

    /// Index out of bounds
    #[error("Index out of bounds: {0}")]
    IndexOutOfBounds(String),

    /// Generic dimension mismatch
    #[error("Dimension mismatch: {0}")]
    DimensionMismatch(String),

    /// Invalid ordinal identifier for the current arena
    #[error("Invalid ordinal id: {0:?}")]
    InvalidOrdinalId(OrdinalId),

    /// Invalid ordinal construction or normalization request
    #[error("Invalid ordinal: {0}")]
    InvalidOrdinal(String),
}

/// Result type for tropical operations
pub type TropicalResult<T> = Result<T, TropicalError>;
