use crate::game::GameId;
use thiserror::Error;

/// Result type for `amari-cgt` operations.
pub type Result<T> = core::result::Result<T, CgtError>;

/// Errors produced by `amari-cgt`.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CgtError {
    /// Returned when a `GameId` does not refer to a valid arena node.
    #[error("invalid game id: {0:?}")]
    InvalidGameId(GameId),

    /// Returned when an impartial-only operation is requested on a partizan game.
    #[error("game is not impartial: {0:?}")]
    NotImpartial(GameId),

    /// Returned when a numeric-validation witness is requested for a non-numeric game.
    #[error("game is not numeric: {0:?}")]
    NotNumericGame(GameId),

    /// Returned when an exhaustive bounded generation request becomes too large.
    #[error("generation universe too large for exhaustive construction: {0}")]
    GenerationUniverseTooLarge(usize),
}
