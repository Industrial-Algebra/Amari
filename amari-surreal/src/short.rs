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
    ///
    /// By the Simplicity Theorem (Conway) the value of `{L|R}` is the
    /// minimal-birthday number strictly greater than all `L` and strictly
    /// less than all `R`: among fitting integers, the one closest to zero
    /// (zero itself whenever the interval crosses zero); otherwise the
    /// unique dyadic lattice point at the coarsest fitting scale.
    ///
    /// Never panics: construction routes through [`Dyadic::try_new`], so
    /// a simplest value needing an exponent beyond the representation
    /// bound returns [`SurrealError::ExponentOverflow`].
    pub fn simplest_between(left: &[ShortSurreal], right: &[ShortSurreal]) -> Result<Self> {
        let left_bound = left.iter().map(|value| value.to_dyadic()).max();
        let right_bound = right.iter().map(|value| value.to_dyadic()).min();

        let value = match (left_bound, right_bound) {
            (None, None) => Dyadic::zero(),
            (Some(left), None) => {
                // Simplest number > left: 0 whenever left < 0 (birthday
                // 0); otherwise the smallest integer strictly above left.
                if left.is_negative() {
                    Dyadic::zero()
                } else {
                    let next_integer = left.floor_integer() + BigInt::one();
                    Dyadic::from_integer(next_integer)
                }
            }
            (None, Some(right)) => {
                // Simplest number < right: 0 whenever right > 0.
                if right.is_positive() {
                    Dyadic::zero()
                } else {
                    let previous_integer = right.ceil_integer() - BigInt::one();
                    Dyadic::from_integer(previous_integer)
                }
            }
            (Some(left), Some(right)) => {
                if left >= right {
                    return Err(SurrealError::InvalidCut);
                }

                // Integer scale: the fitting integers are
                // floor(left)+1 ..= ceil(right)-1. The simplest is the
                // one CLOSEST TO ZERO (zero whenever it fits — ties
                // between -k and k are impossible: if both fit, 0 fits).
                let int_low = left.floor_integer() + BigInt::one();
                let int_high = right.ceil_integer() - BigInt::one();
                if int_low <= int_high {
                    if int_low <= BigInt::from(0) && BigInt::from(0) <= int_high {
                        Dyadic::zero()
                    } else if int_high < BigInt::from(0) {
                        Dyadic::from_integer(int_high)
                    } else {
                        Dyadic::from_integer(int_low)
                    }
                } else {
                    // No integer fits: scan scales e >= 1. Within the
                    // enclosing integer cell the birthday of a dyadic is
                    // e + const, so the coarsest fitting scale carries
                    // the minimal birthday, and its fitting point is the
                    // unique ODD lattice point above left (an even one
                    // would be a coarser-scale point that would have fit
                    // earlier) — uniqueness per scale, minimality by scan
                    // order.
                    let max_exponent = left.exponent().max(right.exponent()) + 1;
                    let mut found = None;
                    for exponent in 1..=max_exponent {
                        let candidate = Self::floor_scaled_numer(&left, exponent) + BigInt::one();
                        let right_ceiling = Self::ceil_scaled_numer(&right, exponent);
                        if candidate < right_ceiling {
                            found = Some(Dyadic::try_new(candidate, exponent)?);
                            break;
                        }
                    }

                    found.ok_or(SurrealError::InvalidCut)?
                }
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

    /// Iterative (worklist) option traversal: no Rust recursion in the
    /// surreal layer. NOTE: `NumericGame::from_game` delegates to
    /// `amari-cgt`'s `is_numeric`, whose own option recursion bounds
    /// reverse conversion depth for VERY deep games — that recursion
    /// lives in amari-cgt and is recorded as a cross-crate boundary for
    /// the cgt audit.
    fn from_game_cached(
        arena: &mut GameArena,
        game: GameId,
        cache: &mut HashMap<GameId, ShortSurreal>,
    ) -> Result<Self> {
        let mut stack = vec![game];
        while let Some(current) = stack.pop() {
            if cache.contains_key(&current) {
                continue;
            }

            NumericGame::from_game(arena, current)?;

            let left_ids = arena.left_options(current)?.to_vec();
            let right_ids = arena.right_options(current)?.to_vec();

            let mut pending = Vec::new();
            for option in left_ids.iter().chain(right_ids.iter()) {
                if !cache.contains_key(option) {
                    pending.push(*option);
                }
            }

            if pending.is_empty() {
                let mut left_values = Vec::with_capacity(left_ids.len());
                for option in left_ids {
                    left_values.push(cache[&option].clone());
                }
                let mut right_values = Vec::with_capacity(right_ids.len());
                for option in right_ids {
                    right_values.push(cache[&option].clone());
                }
                let mut value = Self::simplest_between(&left_values, &right_values)?;
                value.birthday = arena.birthday(current)?;
                value.provenance = Some(current);
                cache.insert(current, value);
            } else {
                stack.push(current);
                stack.extend(pending);
            }
        }

        Ok(cache[&game].clone())
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

    /// Work budget for game reconstruction (fully ITERATIVE — no Rust
    /// recursion anywhere; a worklist builds the canonical form
    /// bottom-up). Values needing more arena games than this fail with
    /// [`SurrealError::ReconstructionBudgetExceeded`] — honest error,
    /// never a stack overflow.
    pub const MAX_RECONSTRUCTION_NODES: u32 = 1 << 20;

    fn to_game_cached(
        value: &Dyadic,
        arena: &mut GameArena,
        cache: &mut HashMap<Dyadic, GameId>,
    ) -> Result<GameId> {
        if let Some(game) = cache.get(value) {
            return Ok(*game);
        }

        let mut budget = Self::MAX_RECONSTRUCTION_NODES;
        let mut stack = vec![value.clone()];

        while let Some(current) = stack.pop() {
            if cache.contains_key(&current) {
                continue;
            }

            budget = budget
                .checked_sub(1)
                .ok_or(SurrealError::ReconstructionBudgetExceeded {
                    visited: Self::MAX_RECONSTRUCTION_NODES,
                    budget: Self::MAX_RECONSTRUCTION_NODES,
                })?;

            let game = if current.is_zero() {
                arena.zero()
            } else if current.exponent() == 0 {
                // ITERATIVE integer chain: {n-1|} for n > 0, {|n+1} for
                // n < 0 — a loop, not recursion (the R1 stack-overflow
                // repro was fromInteger(100000)).
                let target = current.numer().clone();
                let one = BigInt::one();
                let positive = current.is_positive();
                let mut game = arena.zero();
                let mut k = BigInt::from(0);
                while if positive { k < target } else { k > target } {
                    if positive {
                        k += &one;
                        game = arena.from_options([game], [])?;
                    } else {
                        k -= &one;
                        game = arena.from_options([], [game])?;
                    }
                    budget = budget.checked_sub(1).ok_or(
                        SurrealError::ReconstructionBudgetExceeded {
                            visited: Self::MAX_RECONSTRUCTION_NODES,
                            budget: Self::MAX_RECONSTRUCTION_NODES,
                        },
                    )?;
                    cache.insert(Dyadic::from_integer(k.clone()), game);
                }
                game
            } else {
                // Fractional: needs both same-scale neighbors (which
                // NORMALIZE to the coarser scale). Defer until cached.
                let left = Dyadic::new(current.numer().clone() - BigInt::one(), current.exponent());
                let right =
                    Dyadic::new(current.numer().clone() + BigInt::one(), current.exponent());
                match (cache.get(&left), cache.get(&right)) {
                    (Some(l), Some(r)) => arena.from_options([*l], [*r])?,
                    _ => {
                        stack.push(current);
                        if !cache.contains_key(&left) {
                            stack.push(left);
                        }
                        if !cache.contains_key(&right) {
                            stack.push(right);
                        }
                        continue;
                    }
                }
            };

            cache.insert(current, game);
        }

        Ok(cache[value])
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cut(l: &[i64], r: &[i64]) -> Result<ShortSurreal> {
        let left: Vec<ShortSurreal> = l.iter().map(|&n| ShortSurreal::from_integer(n)).collect();
        let right: Vec<ShortSurreal> = r.iter().map(|&n| ShortSurreal::from_integer(n)).collect();
        ShortSurreal::simplest_between(&left, &right)
    }

    fn cut_dyadic(left: Dyadic, right: Dyadic) -> Result<ShortSurreal> {
        let l = vec![ShortSurreal::from_dyadic(left)];
        let r = vec![ShortSurreal::from_dyadic(right)];
        ShortSurreal::simplest_between(&l, &r)
    }

    fn value(s: &ShortSurreal) -> String {
        s.to_dyadic().to_string()
    }

    #[test]
    fn simplest_oracle_table() {
        // S1 counterexamples (the four cuts the old code got wrong).
        assert_eq!(
            value(&cut_dyadic(Dyadic::new(-7, 1), Dyadic::new(-3, 1)).unwrap()),
            "-2"
        ); // {−3.5|−1.5}
        assert_eq!(value(&cut(&[-3], &[1]).unwrap()), "0"); // {−3|1}
        assert_eq!(value(&cut(&[-2], &[-1]).unwrap()), "-3/2");
        // Semi-finite: simplest above −3.5 is 0, below 1.5 is 0.
        assert_eq!(value(&cut(&[-7], &[]).unwrap()), "0"); // left only
        assert_eq!(value(&cut(&[3], &[]).unwrap()), "4");
        assert_eq!(value(&cut(&[], &[1]).unwrap()), "0"); // right only: below 1 is 0
        assert_eq!(value(&cut(&[], &[-2]).unwrap()), "-3");
        // Canonical positives.
        assert_eq!(value(&cut(&[0], &[1]).unwrap()), "1/2");
        assert_eq!(value(&cut(&[1], &[2]).unwrap()), "3/2");
        assert_eq!(value(&cut(&[-1], &[1]).unwrap()), "0");
        // Fractions: {1/2|3/4} = 5/8; {0|1/2} = 1/4.
        assert_eq!(
            value(&cut_dyadic(Dyadic::new(1, 1), Dyadic::new(3, 2)).unwrap()),
            "5/8"
        );
        assert_eq!(
            value(&cut_dyadic(Dyadic::zero(), Dyadic::new(1, 1)).unwrap()),
            "1/4"
        );
        // Interval spanning zero with integers on both sides.
        assert_eq!(value(&cut(&[-5], &[3]).unwrap()), "0");
        assert_eq!(value(&cut(&[-5], &[-3]).unwrap()), "-4");
        assert_eq!(value(&cut(&[3], &[5]).unwrap()), "4");
    }

    #[test]
    fn invalid_cut_rejected() {
        assert!(matches!(
            cut(&[1], &[0]).unwrap_err(),
            SurrealError::InvalidCut
        ));
        assert!(matches!(
            cut(&[1], &[1]).unwrap_err(),
            SurrealError::InvalidCut
        ));
    }

    #[test]
    fn from_game_roundtrip_preserves_value() {
        let mut arena = GameArena::new();
        let zero = arena.zero();
        let one = arena.one().unwrap();
        // {0|1} = 1/2; {-1|1} = 0 (the S1 shape).
        let half = arena.from_options([zero], [one]).unwrap();
        let neg_one = arena.from_options([], [zero]).unwrap();
        let mid = arena.from_options([neg_one], [one]).unwrap();
        let three = arena.from_options([one, one], []).unwrap(); // dominated options
        let games = [half, mid, three];
        for game in games {
            let s = ShortSurreal::from_game(&mut arena, game).unwrap();
            let rebuilt = s.to_game_in(&mut arena).unwrap();
            let s2 = ShortSurreal::from_game(&mut arena, rebuilt).unwrap();
            assert_eq!(s.to_dyadic(), s2.to_dyadic());
        }
        // Negative-cut user-built game: {−3|1} has value 0.
        let neg_three = {
            let n2 = arena.from_options([], [neg_one]).unwrap();
            arena.from_options([], [n2]).unwrap()
        };
        let neg_cut = arena.from_options([neg_three], [one]).unwrap();
        let val = ShortSurreal::from_game(&mut arena, neg_cut).unwrap();
        assert_eq!(value(&val), "0");
    }

    #[test]
    fn large_integer_reconstruction_is_iterative_not_recursive() {
        // R1 P1-4 repro: fromInteger(100000).toGameIn previously
        // overflowed the stack (recursion depth 100k); the integer
        // chain is now a LOOP. Roundtrip at a moderate magnitude
        // (reverse conversion recurses inside amari-cgt's is_numeric —
        // cross-crate boundary recorded for the cgt audit).
        let mut arena = GameArena::new();
        let big = ShortSurreal::from_integer(100_000);
        let game = big.to_game_in(&mut arena).unwrap();
        assert!(arena.birthday(game).is_ok());
        let moderate = ShortSurreal::from_integer(2_000);
        let game = moderate.to_game_in(&mut arena).unwrap();
        let back = ShortSurreal::from_game(&mut arena, game).unwrap();
        assert_eq!(back.to_dyadic(), moderate.to_dyadic());
    }

    #[test]
    fn deep_fractional_reconstruction_errors_honestly() {
        // Worklist reconstruction: 1/2^9000 SUCCEEDS (9001 nodes, well
        // under the 2^20 budget) — no stack anywhere; and a value whose
        // chain exceeds the node budget errors honestly instead of
        // looping or overflowing (1/2^MAX needs MAX+1 nodes).
        let mut arena = GameArena::new();
        // to_game builds iteratively: 1/2^9000 succeeds with no stack
        // involvement. (No reverse roundtrip here: from_game validates
        // via amari-cgt's is_numeric, whose option RECURSION bounds the
        // reverse direction — cross-crate boundary for the cgt audit.)
        let deep_ok = ShortSurreal::from_dyadic(Dyadic::new(1, 9_000));
        let game = deep_ok.to_game_in(&mut arena).unwrap();
        assert!(arena.birthday(game).is_ok());
        let too_deep = ShortSurreal::from_dyadic(Dyadic::new(1, crate::dyadic::MAX_EXPONENT));
        assert!(matches!(
            too_deep.to_game_in(&mut arena),
            Err(SurrealError::ReconstructionBudgetExceeded { .. })
        ));
    }

    #[test]
    fn simplest_boundary_errors_beyond_the_bound() {
        // Adjacent dyadics at the representation bound: their simplest
        // value needs exponent MAX+1.
        let lo = Dyadic::new(1, crate::dyadic::MAX_EXPONENT);
        let hi = Dyadic::new(1, crate::dyadic::MAX_EXPONENT)
            + Dyadic::new(1, crate::dyadic::MAX_EXPONENT);
        // 1/2^M and 2/2^M = 1/2^(M-1): simplest between is 3/2^(M+1) —
        // exponent M+1 > bound.
        assert!(matches!(
            cut_dyadic(lo, hi).unwrap_err(),
            SurrealError::ExponentOverflow { .. }
        ));
    }
}
