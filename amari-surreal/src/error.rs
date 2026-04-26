use amari_cgt::{CgtError, GameId};
use thiserror::Error;

/// Result type for `amari-surreal` operations.
pub type Result<T> = core::result::Result<T, SurrealError>;

/// Errors produced by `amari-surreal`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SurrealError {
    /// Error propagated from `amari-cgt`.
    #[error(transparent)]
    Cgt(#[from] CgtError),

    /// Returned when a game fails numeric validation.
    #[error("game is not numeric: {0:?}")]
    NotNumericGame(GameId),

    /// Returned when finite cut bounds do not define an open interval.
    #[error("invalid cut: left bound is not strictly less than right bound")]
    InvalidCut,

    /// Returned when division by zero is attempted.
    #[error("division by zero")]
    DivisionByZero,

    /// Returned when an exact quotient leaves the dyadic short-surreal layer.
    #[error("quotient is not dyadic in the short-surreal layer")]
    NonDyadicQuotient,
}
