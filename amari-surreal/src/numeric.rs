use crate::error::{Result, SurrealError};
use amari_cgt::{CgtError, GameArena, GameId, NumericGameWitness};

/// Validated numeric game handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NumericGame {
    witness: NumericGameWitness,
}

impl NumericGame {
    /// Validates that `game` is numeric and wraps it.
    pub fn from_game(arena: &mut GameArena, game: GameId) -> Result<Self> {
        match arena.validate_numeric(game) {
            Ok(witness) => Ok(Self { witness }),
            Err(CgtError::NotNumericGame(game)) => Err(SurrealError::NotNumericGame(game)),
            Err(error) => Err(error.into()),
        }
    }

    /// Returns the underlying game id.
    #[must_use]
    pub fn game_id(self) -> GameId {
        self.witness.game_id()
    }

    /// Returns the underlying `amari-cgt` numeric witness.
    #[must_use]
    pub fn witness(self) -> NumericGameWitness {
        self.witness
    }
}
