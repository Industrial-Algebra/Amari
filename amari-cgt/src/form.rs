use crate::game::Birthday;
use std::fmt;

/// Arena-independent structural form of a short combinatorial game.
///
/// `GameForm` stores the recursive left and right option sets directly, without
/// requiring a [`crate::arena::GameArena`]. It is intended for:
///
/// - import/export of small short games
/// - documentation and examples
/// - tests that should not depend on specific arena identities
/// - optional serialization behind the crate `serialize` feature
///
/// Importing a `GameForm` into a [`crate::arena::GameArena`] via
/// [`crate::arena::GameArena::intern_form`] re-interns any
/// repeated subgames. Exporting from a `GameArena` into a `GameForm` expands the
/// arena-backed DAG into a purely structural recursive representation.
///
/// Display formatting recognizes a few small named games directly:
///
/// - `0`
/// - `*`
/// - short integers like `1`, `-1`, `2`, ...
///
/// Other values are rendered as recursive cuts such as `{0 | 1}`.
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GameForm {
    left: Vec<GameForm>,
    right: Vec<GameForm>,
}

impl GameForm {
    /// Creates a structural game form from recursive left and right option sets.
    ///
    /// The option sets are normalized recursively: options are normalized,
    /// sorted, and deduplicated so the resulting `GameForm` behaves like a true
    /// finite set-based short-game cut.
    #[must_use]
    pub fn new<L, R>(left: L, right: R) -> Self
    where
        L: IntoIterator<Item = GameForm>,
        R: IntoIterator<Item = GameForm>,
    {
        let mut form = Self {
            left: left.into_iter().map(Self::normalized).collect(),
            right: right.into_iter().map(Self::normalized).collect(),
        };
        form.normalize_options();
        form
    }

    /// Returns the zero game `{ | }`.
    #[must_use]
    pub fn zero() -> Self {
        Self::default()
    }

    /// Returns the star game `{ 0 | 0 }`.
    #[must_use]
    pub fn star() -> Self {
        let zero = Self::zero();
        Self::new([zero.clone()], [zero])
    }

    /// Returns the game `1 = { 0 | }`.
    #[must_use]
    pub fn one() -> Self {
        Self::new([Self::zero()], std::iter::empty())
    }

    /// Returns the game `-1 = { | 0 }`.
    #[must_use]
    pub fn minus_one() -> Self {
        Self::new(std::iter::empty(), [Self::zero()])
    }

    /// Returns the left options of the structural game.
    #[must_use]
    pub fn left_options(&self) -> &[GameForm] {
        &self.left
    }

    /// Returns the right options of the structural game.
    #[must_use]
    pub fn right_options(&self) -> &[GameForm] {
        &self.right
    }

    /// Returns whether this structural form is the zero game.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.left.is_empty() && self.right.is_empty()
    }

    /// Computes the birthday of the structural form.
    #[must_use]
    pub fn birthday(&self) -> Birthday {
        let max_birthday = self
            .left
            .iter()
            .chain(self.right.iter())
            .map(Self::birthday)
            .map(|birthday| birthday.0)
            .max()
            .unwrap_or(0);

        if self.is_zero() {
            Birthday(0)
        } else {
            Birthday(max_birthday + 1)
        }
    }

    /// Returns a recursively normalized structural form.
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.left = self.left.into_iter().map(Self::normalized).collect();
        self.right = self.right.into_iter().map(Self::normalized).collect();
        self.normalize_options();
        self
    }

    fn normalize_options(&mut self) {
        self.left.sort_unstable();
        self.left.dedup();
        self.right.sort_unstable();
        self.right.dedup();
    }

    fn integer_value(&self) -> Option<i64> {
        if self.is_zero() {
            Some(0)
        } else if self.right.is_empty() && self.left.len() == 1 {
            self.left[0].integer_value()?.checked_add(1)
        } else if self.left.is_empty() && self.right.len() == 1 {
            self.right[0].integer_value()?.checked_sub(1)
        } else {
            None
        }
    }

    fn is_star(&self) -> bool {
        self.left.len() == 1
            && self.right.len() == 1
            && self.left[0].is_zero()
            && self.right[0].is_zero()
    }
}

impl fmt::Display for GameForm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(integer) = self.integer_value() {
            return write!(f, "{integer}");
        }
        if self.is_star() {
            return write!(f, "*");
        }

        write!(f, "{{")?;
        for (index, option) in self.left.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{option}")?;
        }
        write!(f, " | ")?;
        for (index, option) in self.right.iter().enumerate() {
            if index > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{option}")?;
        }
        write!(f, "}}")
    }
}
