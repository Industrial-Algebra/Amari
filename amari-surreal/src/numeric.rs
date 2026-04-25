use crate::error::{Result, SurrealError};
use amari_cgt::{GameArena, GameId};

/// Validated numeric game handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NumericGame {
    game: GameId,
}

impl NumericGame {
    /// Validates that `game` is numeric and wraps it.
    pub fn from_game(arena: &mut GameArena, game: GameId) -> Result<Self> {
        if arena.is_numeric(game)? {
            Ok(Self { game })
        } else {
            Err(SurrealError::NotNumericGame(game))
        }
    }

    /// Returns the underlying game id.
    #[must_use]
    pub fn game_id(self) -> GameId {
        self.game
    }
}
