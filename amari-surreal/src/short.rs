use crate::dyadic::Dyadic;
use crate::error::{Result, SurrealError};
use crate::numeric::NumericGame;
use amari_cgt::{Birthday, GameArena, GameId};
use num_bigint::BigInt;
use num_integer::Integer;
use num_traits::One;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::ops::{Add, Mul, Neg, Sub};

/// Validated short surreal number backed by an exact dyadic value.
///
/// Equality, hashing, and ordering are determined by the dyadic value.
/// Birthday and provenance are retained as metadata and can differ across
/// equal-valued constructions.
#[derive(Debug, Clone)]
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

    /// Returns whether the value is zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.value.is_zero()
    }

    /// Returns whether the value is positive.
    #[must_use]
    pub fn is_positive(&self) -> bool {
        self.value.is_positive()
    }

    /// Returns whether the value is negative.
    #[must_use]
    pub fn is_negative(&self) -> bool {
        self.value.is_negative()
    }

    /// Returns the absolute value of the short surreal.
    #[must_use]
    pub fn abs(&self) -> Self {
        Self::from_dyadic(self.value.abs())
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
                    let candidate = Self::floor_scaled_numer(&left, exponent) + BigInt::one();
                    let right_ceiling = Self::ceil_scaled_numer(&right, exponent);
                    if candidate < right_ceiling {
                        found = Some(Dyadic::new(candidate, exponent));
                        break;
                    }
                }

                found.ok_or(SurrealError::InvalidCut)?
            }
        };

        Ok(Self::from_dyadic(value))
    }

    /// Returns a checked reciprocal within the short-surreal dyadic layer.
    pub fn checked_reciprocal(&self) -> Result<Self> {
        Ok(Self::from_dyadic(self.value.checked_reciprocal()?))
    }

    /// Returns `self / rhs` when the result remains dyadic.
    pub fn checked_div(&self, rhs: &Self) -> Result<Self> {
        Ok(Self::from_dyadic(self.value.checked_div(&rhs.value)?))
    }

    /// Reconstructs this short surreal as a canonical numeric short game in the provided arena.
    ///
    /// This reconstruction is value-based: it rebuilds the canonical short-game
    /// representative for the dyadic value, rather than attempting to preserve
    /// the birthday or provenance metadata of the original source game.
    pub fn to_game_in(&self, arena: &mut GameArena) -> Result<GameId> {
        let mut cache = HashMap::new();
        Self::to_game_cached(&self.value, arena, &mut cache)
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

    fn floor_scaled_numer(value: &Dyadic, exponent: u32) -> BigInt {
        if exponent >= value.exponent() {
            value.numer().clone() << ((exponent - value.exponent()) as usize)
        } else {
            value
                .numer()
                .div_floor(&(BigInt::one() << ((value.exponent() - exponent) as usize)))
        }
    }

    fn ceil_scaled_numer(value: &Dyadic, exponent: u32) -> BigInt {
        if exponent >= value.exponent() {
            value.numer().clone() << ((exponent - value.exponent()) as usize)
        } else {
            value
                .numer()
                .div_ceil(&(BigInt::one() << ((value.exponent() - exponent) as usize)))
        }
    }

    fn to_game_cached(
        value: &Dyadic,
        arena: &mut GameArena,
        cache: &mut HashMap<Dyadic, GameId>,
    ) -> Result<GameId> {
        if let Some(game) = cache.get(value) {
            return Ok(*game);
        }

        let game = if value.is_zero() {
            arena.zero()
        } else if value.exponent() == 0 {
            if value.is_positive() {
                let predecessor = Dyadic::from_integer(value.numer().clone() - BigInt::one());
                let predecessor = Self::to_game_cached(&predecessor, arena, cache)?;
                arena.from_options([predecessor], [])?
            } else {
                let successor = Dyadic::from_integer(value.numer().clone() + BigInt::one());
                let successor = Self::to_game_cached(&successor, arena, cache)?;
                arena.from_options([], [successor])?
            }
        } else {
            let left = Dyadic::new(value.numer().clone() - BigInt::one(), value.exponent());
            let right = Dyadic::new(value.numer().clone() + BigInt::one(), value.exponent());
            let left = Self::to_game_cached(&left, arena, cache)?;
            let right = Self::to_game_cached(&right, arena, cache)?;
            arena.from_options([left], [right])?
        };

        cache.insert(value.clone(), game);
        Ok(game)
    }
}

impl PartialEq for ShortSurreal {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl Eq for ShortSurreal {}

impl Hash for ShortSurreal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.hash(state);
    }
}

impl PartialOrd for ShortSurreal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ShortSurreal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
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
