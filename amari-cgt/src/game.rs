/// Opaque identifier for a game stored in a [`crate::arena::GameArena`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GameId(pub(crate) u32);

/// Birthday of a short game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Birthday(pub u32);

/// Outcome classes for short normal-play games.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OutcomeClass {
    /// Left wins regardless of who starts.
    LeftWins,
    /// Right wins regardless of who starts.
    RightWins,
    /// The next player to move wins.
    NextPlayerWins,
    /// The previous player to move wins.
    PreviousPlayerWins,
}

/// Partial comparison result for short partizan games.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameComparison {
    /// Left-hand side is strictly less than right-hand side.
    Less,
    /// Both games are equal.
    Equal,
    /// Left-hand side is strictly greater than right-hand side.
    Greater,
    /// Games are fuzzy / incomparable in the partizan order.
    Fuzzy,
}

/// Marker wrapper for a game treated as canonical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CanonicalGame(pub GameId);
