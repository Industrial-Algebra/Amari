use crate::form::GameForm;

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

/// Witness that a game has been validated as numeric in a specific arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NumericGameWitness(pub(crate) GameId);

impl NumericGameWitness {
    /// Returns the validated underlying game id.
    #[must_use]
    pub fn game_id(self) -> GameId {
        self.0
    }
}

/// Public inspection summary for a short game.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameInspection {
    pub(crate) game: GameId,
    pub(crate) birthday: Birthday,
    pub(crate) canonical: CanonicalGame,
    pub(crate) canonical_form: GameForm,
    pub(crate) outcome: OutcomeClass,
    pub(crate) impartial: bool,
    pub(crate) numeric: bool,
    pub(crate) reachable_node_count: usize,
}

impl GameInspection {
    /// Returns the inspected game id.
    #[must_use]
    pub fn game_id(&self) -> GameId {
        self.game
    }

    /// Returns the birthday of the inspected game.
    #[must_use]
    pub fn birthday(&self) -> Birthday {
        self.birthday
    }

    /// Returns the canonical representative.
    #[must_use]
    pub fn canonical_game(&self) -> CanonicalGame {
        self.canonical
    }

    /// Returns the canonical representative id.
    #[must_use]
    pub fn canonical_game_id(&self) -> GameId {
        self.canonical.0
    }

    /// Returns whether the inspected game is already canonical.
    #[must_use]
    pub fn is_canonical(&self) -> bool {
        self.game == self.canonical.0
    }

    /// Returns the canonical representative as an arena-independent structural form.
    #[must_use]
    pub fn canonical_form(&self) -> &GameForm {
        &self.canonical_form
    }

    /// Returns the normal-play outcome class.
    #[must_use]
    pub fn outcome(&self) -> OutcomeClass {
        self.outcome
    }

    /// Returns whether the game is impartial.
    #[must_use]
    pub fn is_impartial(&self) -> bool {
        self.impartial
    }

    /// Returns whether the game is partizan.
    #[must_use]
    pub fn is_partizan(&self) -> bool {
        !self.impartial
    }

    /// Returns whether the game is numeric.
    #[must_use]
    pub fn is_numeric(&self) -> bool {
        self.numeric
    }

    /// Returns whether the game is non-numeric.
    #[must_use]
    pub fn is_non_numeric(&self) -> bool {
        !self.numeric
    }

    /// Returns the number of reachable arena nodes in the game DAG.
    #[must_use]
    pub fn reachable_node_count(&self) -> usize {
        self.reachable_node_count
    }
}
