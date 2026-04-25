use crate::dyadic::Dyadic;
use crate::error::{Result, SurrealError};
use crate::numeric::NumericGame;
use amari_cgt::{Birthday, GameArena, GameId};
use num_bigint::BigInt;
use num_traits::One;
use std::collections::HashMap;
use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

/// Validated short surreal number backed by an exact dyadic value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShortSurreal {
    value: Dyadic,
    birthday: Birthday,
    provenance: Option<GameId>,
}

impl ShortSurreal {
    /// Returns zero.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            value: Dyadic::zero(),
            birthday: Birthday(0),
            provenance: None,
        }
    }

    /// Returns one.
    #[must_use]
    pub fn one() -> Self {
        Self::from_integer(1)
    }

    /// Creates a short surreal from an integer.
    #[must_use]
    pub fn from_integer(value: i64) -> Self {
        Self::from_dyadic(Dyadic::from_integer(value))
    }

    /// Creates a short surreal from an exact dyadic value.
    #[must_use]
    pub fn from_dyadic(value: Dyadic) -> Self {
        Self {
            birthday: value.short_birthday(),
            value,
            provenance: None,
        }
    }

    /// Converts a numeric game from `amari-cgt` into a short surreal.
    pub fn from_game(arena: &mut GameArena, game: GameId) -> Result<Self> {
        NumericGame::from_game(arena, game)?;
        let mut cache = HashMap::new();
        Self::from_game_cached(arena, game, &mut cache)
    }

    /// Returns the exact dyadic value.
    #[must_use]
    pub fn to_dyadic(&self) -> Dyadic {
        self.value.clone()
    }

    /// Returns the birthday.
    #[must_use]
    pub fn birthday(&self) -> Birthday {
        self.birthday
    }

    /// Returns the originating game id when this surreal was built from `amari-cgt`.
    #[must_use]
    pub fn provenance(&self) -> Option<GameId> {
        self.provenance
    }

    /// Returns the simplest short surreal between the given finite bounds.
    pub fn simplest_between(left: &[ShortSurreal], right: &[ShortSurreal]) -> Result<Self> {
        let left_bound = left.iter().map(|value| value.to_dyadic()).max();
        let right_bound = right.iter().map(|value| value.to_dyadic()).min();

        let value = match (left_bound, right_bound) {
            (None, None) => Dyadic::zero(),
            (Some(left), None) => {
                let next_integer = left.floor_integer() + BigInt::one();
                Dyadic::from_integer(next_integer)
            }
            (None, Some(right)) => {
                let previous_integer = right.ceil_integer() - BigInt::one();
                Dyadic::from_integer(previous_integer)
            }
            (Some(left), Some(right)) => {
                if left >= right {
                    return Err(SurrealError::InvalidCut);
                }

                let max_exponent = left.exponent().max(right.exponent()) + 1;
                let mut found = None;
                for exponent in 0..=max_exponent {
                    let left_scaled =
                        left.numer().clone() << ((exponent - left.exponent()) as usize);
                    let right_scaled =
                        right.numer().clone() << ((exponent - right.exponent()) as usize);
                    let candidate = left_scaled + BigInt::one();
                    if candidate < right_scaled {
                        found = Some(Dyadic::new(candidate, exponent));
                        break;
                    }
                }

                found.ok_or(SurrealError::InvalidCut)?
            }
        };

        Ok(Self::from_dyadic(value))
    }

    /// Returns `self / rhs` when the result remains dyadic.
    pub fn checked_div(&self, rhs: &Self) -> Result<Self> {
        let value = self
            .value
            .checked_div(&rhs.value)
            .ok_or(SurrealError::DivisionByZero)?;
        Ok(Self::from_dyadic(value))
    }

    fn from_game_cached(
        arena: &mut GameArena,
        game: GameId,
        cache: &mut HashMap<GameId, ShortSurreal>,
    ) -> Result<Self> {
        if let Some(value) = cache.get(&game) {
            return Ok(value.clone());
        }

        NumericGame::from_game(arena, game)?;

        let left_ids = arena.left_options(game)?.to_vec();
        let right_ids = arena.right_options(game)?.to_vec();

        let mut left_values = Vec::with_capacity(left_ids.len());
        for option in left_ids {
            left_values.push(Self::from_game_cached(arena, option, cache)?);
        }

        let mut right_values = Vec::with_capacity(right_ids.len());
        for option in right_ids {
            right_values.push(Self::from_game_cached(arena, option, cache)?);
        }

        let mut value = Self::simplest_between(&left_values, &right_values)?;
        value.birthday = arena.birthday(game)?;
        value.provenance = Some(game);
        cache.insert(game, value.clone());
        Ok(value)
    }
}

impl Add for ShortSurreal {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self::from_dyadic(self.value + rhs.value)
    }
}

impl Sub for ShortSurreal {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self::from_dyadic(self.value - rhs.value)
    }
}

impl Mul for ShortSurreal {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self::from_dyadic(self.value * rhs.value)
    }
}

impl Neg for ShortSurreal {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self::from_dyadic(-self.value)
    }
}

impl fmt::Display for ShortSurreal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.value)
    }
}
