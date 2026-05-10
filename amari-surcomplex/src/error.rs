use amari_surreal::SurrealError;
use thiserror::Error;

/// Result type for `amari-surcomplex` operations.
pub type Result<T> = core::result::Result<T, SurcomplexError>;

/// Errors produced by `amari-surcomplex`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SurcomplexError {
    /// Returned when division by zero is attempted.
    #[error("division by zero")]
    DivisionByZero,

    /// Error propagated from `amari-surreal`.
    #[error(transparent)]
    Surreal(#[from] SurrealError),
}
