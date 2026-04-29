//! Arena-backed ordinals below `ε₀` in Cantor normal form.
//!
//! This module provides a bounded, computationally honest ordinal substrate for
//! optimization-oriented tropical work.
//!
//! The representation is:
//!
//! - ordinals below `ε₀`
//! - canonical Cantor normal form
//! - arena interning via [`OrdinalArena`]
//! - arena-local identifiers via [`OrdinalId`]
//! - a bottom-extended optimization carrier via [`OrdinalWeight`]
//!
//! # Example
//!
//! ```
//! use amari_tropical::{CnfTerm, OrdinalArena, OrdinalWeight};
//!
//! let mut arena = OrdinalArena::new();
//! let one = arena.one();
//! let omega = arena.omega();
//! let omega_plus_one = arena.add(omega, one).unwrap();
//! let omega_squared = arena.intern_cnf(vec![CnfTerm::new(omega, 1)]).unwrap();
//!
//! assert_eq!(arena.format_ordinal(omega).unwrap(), "ω");
//! assert_eq!(arena.format_ordinal(omega_plus_one).unwrap(), "ω + 1");
//! assert_eq!(arena.format_ordinal(omega_squared).unwrap(), "ω^ω");
//!
//! let weight = OrdinalWeight::from_ordinal(omega_plus_one);
//! assert_eq!(arena.format_weight(weight).unwrap(), "ω + 1");
//! ```

use alloc::{
    collections::BTreeMap,
    format,
    string::{String, ToString},
    vec::Vec,
};
use core::cmp::Ordering;

use crate::{TropicalError, TropicalResult};

/// An arena-local identifier for an interned ordinal node.
///
/// `OrdinalId` values are only meaningful with the [`OrdinalArena`] that
/// created them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct OrdinalId(usize);

impl OrdinalId {
    /// The reserved arena slot for the zero ordinal.
    pub const ZERO: Self = Self(0);

    /// Return the raw arena index.
    #[inline]
    pub const fn index(self) -> usize {
        self.0
    }
}

/// A Cantor-normal-form term `ω^exponent * coefficient`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CnfTerm {
    /// Exponent of the `ω` power.
    pub exponent: OrdinalId,
    /// Positive natural coefficient.
    pub coefficient: u64,
}

impl CnfTerm {
    /// Create a new CNF term.
    #[inline]
    pub const fn new(exponent: OrdinalId, coefficient: u64) -> Self {
        Self {
            exponent,
            coefficient,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OrdinalNode {
    terms: Vec<CnfTerm>,
}

/// Arena-backed store for canonical ordinals below `ε₀`.
#[derive(Debug, Clone)]
pub struct OrdinalArena {
    nodes: Vec<OrdinalNode>,
    interner: BTreeMap<Vec<(usize, u64)>, OrdinalId>,
}

impl Default for OrdinalArena {
    fn default() -> Self {
        Self::new()
    }
}

impl OrdinalArena {
    /// Create a new ordinal arena containing the zero ordinal.
    pub fn new() -> Self {
        let mut interner = BTreeMap::new();
        interner.insert(Vec::new(), OrdinalId::ZERO);

        Self {
            nodes: vec![OrdinalNode { terms: Vec::new() }],
            interner,
        }
    }

    /// Return the canonical zero ordinal.
    #[inline]
    pub const fn zero(&self) -> OrdinalId {
        OrdinalId::ZERO
    }

    /// Return `true` if the identifier is in-range for this arena.
    #[inline]
    pub fn contains(&self, ordinal: OrdinalId) -> bool {
        ordinal.index() < self.nodes.len()
    }

    /// Return the number of interned ordinal nodes.
    #[inline]
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Construct the finite natural ordinal `n`.
    pub fn finite(&mut self, n: u64) -> OrdinalId {
        if n == 0 {
            return self.zero();
        }

        self.intern_canonical_terms(vec![CnfTerm::new(self.zero(), n)])
            .expect("finite ordinals are always canonical")
    }

    /// Construct the finite ordinal `1`.
    #[inline]
    pub fn one(&mut self) -> OrdinalId {
        self.finite(1)
    }

    /// Construct the ordinal `ω`.
    pub fn omega(&mut self) -> OrdinalId {
        let one = self.one();
        self.intern_canonical_terms(vec![CnfTerm::new(one, 1)])
            .expect("omega is always canonical")
    }

    /// Access the canonical CNF terms of an ordinal.
    pub fn terms(&self, ordinal: OrdinalId) -> TropicalResult<&[CnfTerm]> {
        Ok(&self.node(ordinal)?.terms)
    }

    /// Intern an ordinal from CNF terms, normalizing order and duplicates.
    pub fn intern_cnf(&mut self, terms: Vec<CnfTerm>) -> TropicalResult<OrdinalId> {
        let normalized = self.normalize_terms(terms)?;
        self.intern_canonical_terms(normalized)
    }

    /// Compare two ordinals.
    pub fn compare(&self, left: OrdinalId, right: OrdinalId) -> TropicalResult<Ordering> {
        self.validate_id(left)?;
        self.validate_id(right)?;
        Ok(self.compare_internal(left, right))
    }

    /// Compute ordinal addition `left + right`.
    pub fn add(&mut self, left: OrdinalId, right: OrdinalId) -> TropicalResult<OrdinalId> {
        self.validate_id(left)?;
        self.validate_id(right)?;

        if left == self.zero() {
            return Ok(right);
        }
        if right == self.zero() {
            return Ok(left);
        }

        let mut left_terms = self.node_unchecked(left).terms.clone();
        let right_terms = self.node_unchecked(right).terms.clone();
        let leading_exponent = right_terms[0].exponent;

        while let Some(last) = left_terms.last() {
            if self.compare_internal(last.exponent, leading_exponent) == Ordering::Less {
                left_terms.pop();
            } else {
                break;
            }
        }

        let mut result = Vec::with_capacity(left_terms.len() + right_terms.len());

        if let Some(last) = left_terms.last().copied() {
            if self.compare_internal(last.exponent, leading_exponent) == Ordering::Equal {
                left_terms.pop();
                let merged = last
                    .coefficient
                    .checked_add(right_terms[0].coefficient)
                    .ok_or(TropicalError::Overflow)?;

                result.extend(left_terms);
                result.push(CnfTerm::new(leading_exponent, merged));
                result.extend_from_slice(&right_terms[1..]);

                return self.intern_canonical_terms(result);
            }
        }

        result.extend(left_terms);
        result.extend(right_terms);
        self.intern_canonical_terms(result)
    }

    /// Return the leading exponent of an ordinal, if nonzero.
    pub fn leading_exponent(&self, ordinal: OrdinalId) -> TropicalResult<Option<OrdinalId>> {
        self.validate_id(ordinal)?;
        Ok(self
            .node_unchecked(ordinal)
            .terms
            .first()
            .map(|term| term.exponent))
    }

    /// Return the leading term `(exponent, coefficient)` of an ordinal, if nonzero.
    pub fn leading_term(&self, ordinal: OrdinalId) -> TropicalResult<Option<(OrdinalId, u64)>> {
        self.validate_id(ordinal)?;
        Ok(self
            .node_unchecked(ordinal)
            .terms
            .first()
            .map(|term| (term.exponent, term.coefficient)))
    }

    /// Format an ordinal in readable Cantor normal form.
    pub fn format_ordinal(&self, ordinal: OrdinalId) -> TropicalResult<String> {
        self.validate_id(ordinal)?;
        Ok(self.format_ordinal_inner(ordinal))
    }

    /// Format an [`OrdinalWeight`].
    pub fn format_weight(&self, weight: OrdinalWeight) -> TropicalResult<String> {
        match weight {
            OrdinalWeight::Bottom => Ok(String::from("Bottom")),
            OrdinalWeight::Ordinal(ordinal) => self.format_ordinal(ordinal),
        }
    }

    fn validate_id(&self, ordinal: OrdinalId) -> TropicalResult<()> {
        if self.contains(ordinal) {
            Ok(())
        } else {
            Err(TropicalError::InvalidOrdinalId(ordinal))
        }
    }

    fn node(&self, ordinal: OrdinalId) -> TropicalResult<&OrdinalNode> {
        self.validate_id(ordinal)?;
        Ok(self.node_unchecked(ordinal))
    }

    fn node_unchecked(&self, ordinal: OrdinalId) -> &OrdinalNode {
        &self.nodes[ordinal.index()]
    }

    fn normalize_terms(&self, mut terms: Vec<CnfTerm>) -> TropicalResult<Vec<CnfTerm>> {
        for term in &terms {
            self.validate_id(term.exponent)?;
        }

        terms.retain(|term| term.coefficient != 0);
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        terms.sort_by(|left, right| self.compare_internal(right.exponent, left.exponent));

        let mut normalized: Vec<CnfTerm> = Vec::with_capacity(terms.len());
        for term in terms {
            if let Some(last) = normalized.last_mut() {
                if self.compare_internal(last.exponent, term.exponent) == Ordering::Equal {
                    last.coefficient = last
                        .coefficient
                        .checked_add(term.coefficient)
                        .ok_or(TropicalError::Overflow)?;
                    continue;
                }
            }
            normalized.push(term);
        }

        Ok(normalized)
    }

    fn intern_canonical_terms(&mut self, terms: Vec<CnfTerm>) -> TropicalResult<OrdinalId> {
        let key = Self::interner_key(&terms);
        if let Some(existing) = self.interner.get(&key).copied() {
            return Ok(existing);
        }

        let ordinal = OrdinalId(self.nodes.len());
        self.nodes.push(OrdinalNode {
            terms: terms.clone(),
        });
        self.interner.insert(key, ordinal);
        Ok(ordinal)
    }

    fn interner_key(terms: &[CnfTerm]) -> Vec<(usize, u64)> {
        terms
            .iter()
            .map(|term| (term.exponent.index(), term.coefficient))
            .collect()
    }

    fn compare_internal(&self, left: OrdinalId, right: OrdinalId) -> Ordering {
        if left == right {
            return Ordering::Equal;
        }

        let left_terms = &self.node_unchecked(left).terms;
        let right_terms = &self.node_unchecked(right).terms;
        let common_len = left_terms.len().min(right_terms.len());

        for index in 0..common_len {
            let left_term = left_terms[index];
            let right_term = right_terms[index];

            let exponent_order = self.compare_internal(left_term.exponent, right_term.exponent);
            if exponent_order != Ordering::Equal {
                return exponent_order;
            }

            let coefficient_order = left_term.coefficient.cmp(&right_term.coefficient);
            if coefficient_order != Ordering::Equal {
                return coefficient_order;
            }
        }

        left_terms.len().cmp(&right_terms.len())
    }

    fn format_ordinal_inner(&self, ordinal: OrdinalId) -> String {
        if let Some(finite) = self.as_finite(ordinal) {
            return finite.to_string();
        }

        self.node_unchecked(ordinal)
            .terms
            .iter()
            .copied()
            .map(|term| self.format_term(term))
            .collect::<Vec<_>>()
            .join(" + ")
    }

    fn format_term(&self, term: CnfTerm) -> String {
        if term.exponent == self.zero() {
            return term.coefficient.to_string();
        }

        let exponent = self.format_ordinal_inner(term.exponent);
        let base = if exponent == "1" {
            String::from("ω")
        } else if exponent.contains(" + ") {
            format!("ω^({exponent})")
        } else {
            format!("ω^{exponent}")
        };

        if term.coefficient == 1 {
            base
        } else {
            format!("{}{base}", term.coefficient)
        }
    }

    fn as_finite(&self, ordinal: OrdinalId) -> Option<u64> {
        let terms = &self.node_unchecked(ordinal).terms;
        match terms.as_slice() {
            [] => Some(0),
            [term] if term.exponent == self.zero() => Some(term.coefficient),
            _ => None,
        }
    }
}

/// Bottom-extended optimization carrier built on interned ordinals.
///
/// `Bottom` is the additive identity for the optimization-facing semiring.
/// `Ordinal(0)` is the multiplicative identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum OrdinalWeight {
    /// Semiring zero / additive identity.
    #[default]
    Bottom,
    /// An actual ordinal value interned in an [`OrdinalArena`].
    Ordinal(OrdinalId),
}

impl From<OrdinalId> for OrdinalWeight {
    fn from(value: OrdinalId) -> Self {
        Self::Ordinal(value)
    }
}

impl OrdinalWeight {
    /// Construct the bottom element.
    #[inline]
    pub const fn bottom() -> Self {
        Self::Bottom
    }

    /// Alias for the semiring zero element.
    #[inline]
    pub const fn zero() -> Self {
        Self::Bottom
    }

    /// Construct the semiring one element, i.e. ordinal zero.
    #[inline]
    pub const fn one() -> Self {
        Self::Ordinal(OrdinalId::ZERO)
    }

    /// Construct a weight from an ordinal identifier.
    #[inline]
    pub const fn from_ordinal(ordinal: OrdinalId) -> Self {
        Self::Ordinal(ordinal)
    }

    /// Return the underlying ordinal identifier, if any.
    #[inline]
    pub const fn ordinal(self) -> Option<OrdinalId> {
        match self {
            Self::Bottom => None,
            Self::Ordinal(ordinal) => Some(ordinal),
        }
    }

    /// Return `true` if this is the bottom element.
    #[inline]
    pub const fn is_bottom(self) -> bool {
        matches!(self, Self::Bottom)
    }

    /// Semiring-style additive combination using `max` on ordinals.
    pub fn oplus(self, other: Self, arena: &OrdinalArena) -> TropicalResult<Self> {
        match (self, other) {
            (Self::Bottom, weight) | (weight, Self::Bottom) => Ok(weight),
            (Self::Ordinal(left), Self::Ordinal(right)) => {
                if arena.compare(left, right)? == Ordering::Less {
                    Ok(Self::Ordinal(right))
                } else {
                    Ok(Self::Ordinal(left))
                }
            }
        }
    }

    /// Semiring-style multiplicative composition using ordinal addition.
    pub fn otimes(self, other: Self, arena: &mut OrdinalArena) -> TropicalResult<Self> {
        match (self, other) {
            (Self::Bottom, _) | (_, Self::Bottom) => Ok(Self::Bottom),
            (Self::Ordinal(left), Self::Ordinal(right)) => {
                Ok(Self::Ordinal(arena.add(left, right)?))
            }
        }
    }

    /// Return the leading-exponent valuation of this weight.
    pub fn valuation(self, arena: &OrdinalArena) -> TropicalResult<Option<OrdinalId>> {
        match self {
            Self::Bottom => Ok(None),
            Self::Ordinal(ordinal) => arena.leading_exponent(ordinal),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finite_ordinals_intern_canonically() {
        let mut arena = OrdinalArena::new();
        let zero = arena.zero();
        let three_a = arena.finite(3);
        let three_b = arena.intern_cnf(vec![CnfTerm::new(zero, 1), CnfTerm::new(zero, 2)]);

        assert_eq!(three_a, three_b.unwrap());
        assert_eq!(arena.format_ordinal(zero).unwrap(), "0");
        assert_eq!(arena.format_ordinal(three_a).unwrap(), "3");
    }

    #[test]
    fn omega_and_basic_formatting_work() {
        let mut arena = OrdinalArena::new();
        let one = arena.one();
        let omega = arena.omega();
        let omega_plus_one = arena.add(omega, one).unwrap();
        let omega_squared = arena.intern_cnf(vec![CnfTerm::new(omega, 1)]).unwrap();

        assert_eq!(arena.format_ordinal(one).unwrap(), "1");
        assert_eq!(arena.format_ordinal(omega).unwrap(), "ω");
        assert_eq!(arena.format_ordinal(omega_plus_one).unwrap(), "ω + 1");
        assert_eq!(arena.format_ordinal(omega_squared).unwrap(), "ω^ω");
    }

    #[test]
    fn unsorted_duplicate_terms_normalize() {
        let mut arena = OrdinalArena::new();
        let zero = arena.zero();
        let one = arena.one();

        let ordinal = arena
            .intern_cnf(vec![
                CnfTerm::new(zero, 2),
                CnfTerm::new(one, 1),
                CnfTerm::new(zero, 3),
            ])
            .unwrap();

        assert_eq!(arena.format_ordinal(ordinal).unwrap(), "ω + 5");
    }

    #[test]
    fn ordinal_comparison_matches_standard_examples() {
        let mut arena = OrdinalArena::new();
        let one = arena.one();
        let two = arena.finite(2);
        let omega = arena.omega();
        let omega_plus_one = arena.add(omega, one).unwrap();
        let two_omega = arena.intern_cnf(vec![CnfTerm::new(one, 2)]).unwrap();

        assert_eq!(arena.compare(one, omega).unwrap(), Ordering::Less);
        assert_eq!(
            arena.compare(omega, omega_plus_one).unwrap(),
            Ordering::Less
        );
        assert_eq!(
            arena.compare(omega_plus_one, two_omega).unwrap(),
            Ordering::Less
        );
        assert_eq!(arena.compare(two, one).unwrap(), Ordering::Greater);
    }

    #[test]
    fn ordinal_addition_matches_standard_examples() {
        let mut arena = OrdinalArena::new();
        let zero = arena.zero();
        let one = arena.one();
        let omega = arena.omega();
        let omega_squared = arena.intern_cnf(vec![CnfTerm::new(omega, 1)]).unwrap();
        let omega_plus_one = arena.add(omega, one).unwrap();

        let one_plus_omega = arena.add(one, omega).unwrap();
        assert_eq!(one_plus_omega, omega);

        let omega_plus_one_again = arena.add(omega, one).unwrap();
        assert_eq!(arena.format_ordinal(omega_plus_one_again).unwrap(), "ω + 1");

        let ordinal = arena
            .intern_cnf(vec![
                CnfTerm::new(omega, 1),
                CnfTerm::new(one, 1),
                CnfTerm::new(zero, 1),
            ])
            .unwrap();
        let sum = arena.add(ordinal, omega).unwrap();

        assert_eq!(arena.format_ordinal(sum).unwrap(), "ω^ω + 2ω");
        assert_eq!(arena.format_ordinal(omega_plus_one).unwrap(), "ω + 1");
        assert_eq!(arena.format_ordinal(omega_squared).unwrap(), "ω^ω");
    }

    #[test]
    fn leading_exponent_and_term_are_reported() {
        let mut arena = OrdinalArena::new();
        let zero = arena.zero();
        let one = arena.one();
        let two = arena.finite(2);
        let omega = arena.omega();
        let ordinal = arena
            .intern_cnf(vec![
                CnfTerm::new(two, 1),
                CnfTerm::new(one, 3),
                CnfTerm::new(zero, 5),
            ])
            .unwrap();

        assert_eq!(arena.leading_exponent(zero).unwrap(), None);
        let leading_exponent = arena.leading_exponent(ordinal).unwrap().unwrap();
        let leading_term = arena.leading_term(ordinal).unwrap().unwrap();

        assert_eq!(arena.format_ordinal(leading_exponent).unwrap(), "2");
        assert_eq!(leading_term.1, 1);
        assert_eq!(arena.format_ordinal(omega).unwrap(), "ω");
        assert_eq!(arena.format_ordinal(ordinal).unwrap(), "ω^2 + 3ω + 5");
    }

    #[test]
    fn ordinal_weight_operations_work() {
        let mut arena = OrdinalArena::new();
        let one = arena.one();
        let omega = arena.omega();
        let omega_plus_one = arena.add(omega, one).unwrap();

        let bottom = OrdinalWeight::bottom();
        let w_omega = OrdinalWeight::from_ordinal(omega);
        let w_one = OrdinalWeight::from_ordinal(one);
        let w_omega_plus_one = OrdinalWeight::from_ordinal(omega_plus_one);

        assert_eq!(bottom.oplus(w_omega, &arena).unwrap(), w_omega);
        assert_eq!(w_omega.oplus(w_one, &arena).unwrap(), w_omega);
        assert_eq!(w_omega.otimes(w_one, &mut arena).unwrap(), w_omega_plus_one);
        assert_eq!(bottom.otimes(w_omega, &mut arena).unwrap(), bottom);
        assert_eq!(arena.format_weight(w_omega_plus_one).unwrap(), "ω + 1");
        assert_eq!(w_omega.valuation(&arena).unwrap(), Some(one));
        assert_eq!(bottom.valuation(&arena).unwrap(), None);
    }

    #[test]
    fn interned_equal_ordinals_share_ids() {
        let mut arena = OrdinalArena::new();
        let one = arena.one();
        let omega_a = arena.omega();
        let omega_b = arena.intern_cnf(vec![CnfTerm::new(one, 1)]).unwrap();

        assert_eq!(omega_a, omega_b);
        assert_eq!(arena.node_count(), 3);
    }
}
