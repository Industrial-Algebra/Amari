//! Experimental epsilon rational functions.
//!
//! Polynomials and rational functions in a formal positive infinitesimal
//! `ε`, ordered by asymptotic behaviour as `ε → 0⁺`.
//!
//! The coefficient field is [`RationalSurreal`].
//! A polynomial in ε is stored as a map from integer exponents to
//! nonzero rational surreal coefficients.  A rational function is a
//! pair of such polynomials with the denominator normalised to have a
//! positive leading coefficient.
//!
//! # Design notes
//!
//! - The exponent type is a newtype wrapper (`EpsilonExponent`) so that
//!   future Puiseux / Hahn extensions can replace the exponent without
//!   changing the public API shape.
//! - `ε²` is a *positive* infinitesimal smaller than `ε` — this is **not**
//!   a nilpotent-dual-number system.
//! - No polynomial GCD normalisation is attempted; zero removal and
//!   denominator-sign normalisation are enough to keep display and
//!   testing stable for the current feature set.

use crate::error::SurrealError;
use crate::rational::RationalSurreal;

/// Maximum |exponent| for stored epsilon terms (2^20) — the
/// representation invariant of the epsilon layer (symmetric in the
/// exponent sign; negative exponents are Laurent terms and stay
/// supported). Comparisons and cross-products use unbounded transients;
/// construction of stored values enforces the bound.
pub const MAX_EPSILON_EXPONENT: i32 = 1 << 20;

fn check_exponent(exp: i32) -> Result<(), SurrealError> {
    if exp.unsigned_abs() > MAX_EPSILON_EXPONENT.unsigned_abs() {
        return Err(SurrealError::ExponentOverflow {
            requested: exp.unsigned_abs(),
            bound: MAX_EPSILON_EXPONENT.unsigned_abs(),
        });
    }
    Ok(())
}
use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::fmt;
use std::ops::{Add, Mul, Neg, Sub};

// ---------------------------------------------------------------------------
// Exponent newtype
// ---------------------------------------------------------------------------

/// Exponent for epsilon terms.
///
/// Wraps `i32` to leave room for non-integer exponent types in future
/// Puiseux-series or Hahn-series extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EpsilonExponent(pub i32);

// ---------------------------------------------------------------------------
// EpsilonPolynomial
// ---------------------------------------------------------------------------

/// A polynomial in a formal positive infinitesimal `ε` with rational
/// surreal coefficients.
///
/// # Internal representation
///
/// Terms are stored in a [`BTreeMap`] keyed by [`EpsilonExponent`].
/// Zero coefficients are removed during normalisation so that every
/// stored term carries a nonzero coefficient.  The polynomial is
/// ordered by increasing exponent.
///
/// # Asymptotic ordering (ε → 0⁺)
///
/// Terms with **smaller** exponents dominate — the leading term for
/// asymptotic comparison is the smallest exponent with nonzero
/// coefficient.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpsilonPolynomial {
    terms: BTreeMap<EpsilonExponent, RationalSurreal>,
}

// -- constructors -----------------------------------------------------------

impl EpsilonPolynomial {
    /// The zero polynomial.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            terms: BTreeMap::new(),
        }
    }

    /// The constant polynomial `1`.
    #[must_use]
    pub fn one() -> Self {
        Self::from_scalar(RationalSurreal::one())
    }

    /// The monomial `ε` (coefficient 1 at exponent 1).
    #[must_use]
    pub fn epsilon() -> Self {
        Self::monomial(RationalSurreal::one(), 1)
    }

    /// A single monomial `coeff · ε^exp`.
    ///
    /// Returns the zero polynomial when `coeff` is zero.
    ///
    /// # Panics
    ///
    /// Panics when `|exp| > MAX_EPSILON_EXPONENT` (representation
    /// bound). Use [`EpsilonPolynomial::try_monomial`] for the fallible
    /// path.
    #[must_use]
    pub fn monomial(coeff: RationalSurreal, exp: i32) -> Self {
        Self::try_monomial(coeff, exp).expect("exponent exceeds MAX_EPSILON_EXPONENT")
    }

    /// Fallible monomial constructor enforcing the representation bound.
    pub fn try_monomial(coeff: RationalSurreal, exp: i32) -> Result<Self, SurrealError> {
        if coeff.is_zero() {
            return Ok(Self::zero());
        }
        check_exponent(exp)?;
        let mut terms = BTreeMap::new();
        terms.insert(EpsilonExponent(exp), coeff);
        Ok(Self { terms })
    }

    /// Build a polynomial from a scalar (constant term).
    #[must_use]
    pub fn from_scalar(scalar: RationalSurreal) -> Self {
        if scalar.is_zero() {
            Self::zero()
        } else {
            let mut terms = BTreeMap::new();
            terms.insert(EpsilonExponent(0), scalar);
            Self { terms }
        }
    }

    /// Build a polynomial from a coefficient vector.
    ///
    /// The coefficient at index `k` is the coefficient of `ε^k`.
    /// Zero coefficients are skipped.
    #[must_use]
    pub fn from_coefficients(coeffs: Vec<RationalSurreal>) -> Self {
        let mut terms = BTreeMap::new();
        for (k, coeff) in coeffs.into_iter().enumerate() {
            if !coeff.is_zero() {
                assert!(
                    k <= i32::MAX as usize,
                    "coefficient index {k} exceeds i32 exponent range"
                );
                terms.insert(EpsilonExponent(k as i32), coeff);
            }
        }
        Self { terms }
    }
}

// -- queries ----------------------------------------------------------------

impl EpsilonPolynomial {
    /// The degree (largest exponent with a nonzero coefficient) or
    /// `None` for the zero polynomial.
    #[must_use]
    pub fn degree(&self) -> Option<i32> {
        self.terms.keys().last().map(|e| e.0)
    }

    /// The valuation (smallest exponent with a nonzero coefficient) or
    /// `None` for the zero polynomial.
    #[must_use]
    pub fn valuation(&self) -> Option<i32> {
        self.terms.keys().next().map(|e| e.0)
    }

    /// Whether the polynomial is identically zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.terms.is_empty()
    }

    /// Whether the polynomial is the constant `1`.
    ///
    /// Requires a SINGLE constant term (S9): a Laurent polynomial such
    /// as `1 + ε^-1` has `degree() == Some(0)` and term(0) == 1 but is
    /// not the constant one.
    #[must_use]
    pub fn is_one(&self) -> bool {
        self.is_constant() && self.terms.get(&EpsilonExponent(0)) == Some(&RationalSurreal::one())
    }

    /// Whether the polynomial is a single constant term.
    #[must_use]
    pub fn is_constant(&self) -> bool {
        self.valuation() == Some(0) && self.degree() == Some(0)
    }

    /// Sign of the polynomial as `ε → 0⁺`.
    ///
    /// Returns `Some(Ordering::Less)` for negative, `Some(Ordering::Greater)`
    /// for positive, and `None` for the zero polynomial.
    #[must_use]
    pub fn sign_epsilon(&self) -> Option<Ordering> {
        self.valuation()
            .map(|v| self.terms[&EpsilonExponent(v)].cmp(&RationalSurreal::zero()))
    }

    /// Reference to the internal term map (for tests / inspection).
    #[must_use]
    pub fn terms(&self) -> &BTreeMap<EpsilonExponent, RationalSurreal> {
        &self.terms
    }
}

// -- arithmetic -------------------------------------------------------------

impl Add for EpsilonPolynomial {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let mut terms = self.terms;
        for (exp, coeff) in rhs.terms {
            let entry = terms.entry(exp).or_insert_with(RationalSurreal::zero);
            *entry = entry.clone() + coeff;
        }
        terms.retain(|_, v| !v.is_zero());
        Self { terms }
    }
}

impl Sub for EpsilonPolynomial {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self + (-rhs)
    }
}

impl EpsilonPolynomial {
    /// TRANSIENT multiplication: no representation bound is enforced on
    /// the output (used by `Ord`/`PartialEq` cross-products and by the
    /// checked constructors, which enforce the bound themselves before
    /// returning). Exponent addition uses `checked_add` so even
    /// transients can never wrap i32 (double cross-products stay ≤ 2^22,
    /// far inside the range).
    fn mul_transient(&self, rhs: &Self) -> Self {
        let mut terms: BTreeMap<EpsilonExponent, RationalSurreal> = BTreeMap::new();
        for (&exp_a, coeff_a) in &self.terms {
            for (&exp_b, coeff_b) in &rhs.terms {
                let exp =
                    EpsilonExponent(exp_a.0.checked_add(exp_b.0).expect(
                        "transient exponent sum exceeds i32 (impossible from bounded inputs)",
                    ));
                let prod = coeff_a.clone() * coeff_b.clone();
                let entry = terms.entry(exp).or_insert_with(RationalSurreal::zero);
                *entry = entry.clone() + prod;
            }
        }
        terms.retain(|_, v| !v.is_zero());
        Self { terms }
    }

    /// Whether any stored term exceeds the representation bound.
    fn within_bound(&self) -> bool {
        self.terms
            .keys()
            .all(|e| e.0.unsigned_abs() <= MAX_EPSILON_EXPONENT.unsigned_abs())
    }

    /// Checked multiplication: enforces the representation bound on the
    /// OUTPUT. Addition is union-of-terms and cannot grow exponents;
    /// multiplication is the growth path.
    pub fn checked_mul(&self, rhs: &Self) -> Result<Self, SurrealError> {
        let product = self.mul_transient(rhs);
        if product.within_bound() {
            Ok(product)
        } else {
            Err(SurrealError::ExponentOverflow {
                requested: (MAX_EPSILON_EXPONENT as u32) + 1,
                bound: MAX_EPSILON_EXPONENT as u32,
            })
        }
    }
}

impl Mul for EpsilonPolynomial {
    type Output = Self;

    /// # Panics
    ///
    /// Panics when the result has any term beyond
    /// `MAX_EPSILON_EXPONENT` (documented honest panic; see
    /// [`EpsilonPolynomial::checked_mul`]).
    fn mul(self, rhs: Self) -> Self::Output {
        self.checked_mul(&rhs)
            .expect("exponent exceeds MAX_EPSILON_EXPONENT")
    }
}

impl Neg for EpsilonPolynomial {
    type Output = Self;

    fn neg(self) -> Self::Output {
        let terms: BTreeMap<_, _> = self.terms.into_iter().map(|(k, v)| (k, -v)).collect();
        Self { terms }
    }
}

// -- formatting -------------------------------------------------------------

impl fmt::Display for EpsilonPolynomial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return write!(f, "0");
        }
        let mut first = true;
        for (exp, coeff) in &self.terms {
            let negative = coeff.is_negative();
            let magnitude = coeff.clone().abs();
            if first {
                if negative {
                    write!(f, "-")?;
                }
            } else if negative {
                write!(f, " - ")?;
            } else {
                write!(f, " + ")?;
            }
            first = false;
            match exp.0 {
                0 => write!(f, "{magnitude}")?,
                1 => write!(f, "{magnitude}·ε")?,
                e => write!(f, "{magnitude}·ε^{e}")?,
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// EpsilonRational
// ---------------------------------------------------------------------------

/// A rational function of a formal positive infinitesimal `ε` with
/// rational surreal coefficients.
///
/// Internally represented as a numerator and denominator polynomial.
/// The denominator is normalised so that its leading coefficient
/// (as `ε → 0⁺`) is positive, which makes comparison straightforward.
#[derive(Debug, Clone)]
pub struct EpsilonRational {
    numer: EpsilonPolynomial,
    denom: EpsilonPolynomial,
}

// -- constructors -----------------------------------------------------------

impl EpsilonRational {
    /// The rational function `0 / 1`.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            numer: EpsilonPolynomial::zero(),
            denom: EpsilonPolynomial::one(),
        }
    }

    /// The rational function `1 / 1`.
    #[must_use]
    pub fn one() -> Self {
        Self {
            numer: EpsilonPolynomial::one(),
            denom: EpsilonPolynomial::one(),
        }
    }

    /// The rational function `ε / 1`.
    #[must_use]
    pub fn epsilon() -> Self {
        Self::monomial(RationalSurreal::one(), 1)
    }

    /// A monomial rational function `coeff · ε^exp / 1`.
    ///
    /// # Panics
    ///
    /// Panics when `|exp| > MAX_EPSILON_EXPONENT`. Use
    /// [`EpsilonRational::try_monomial`] for the fallible path.
    #[must_use]
    pub fn monomial(coeff: RationalSurreal, exp: i32) -> Self {
        Self {
            numer: EpsilonPolynomial::monomial(coeff, exp),
            denom: EpsilonPolynomial::one(),
        }
    }

    /// Fallible monomial constructor enforcing the representation bound.
    pub fn try_monomial(coeff: RationalSurreal, exp: i32) -> Result<Self, SurrealError> {
        Ok(Self {
            numer: EpsilonPolynomial::try_monomial(coeff, exp)?,
            denom: EpsilonPolynomial::one(),
        })
    }

    /// Build from a scalar constant: `scalar / 1`.
    #[must_use]
    pub fn from_scalar(scalar: RationalSurreal) -> Self {
        Self {
            numer: EpsilonPolynomial::from_scalar(scalar),
            denom: EpsilonPolynomial::one(),
        }
    }

    /// Build a rational function from a coefficient vector.
    ///
    /// The coefficient at index `k` is the coefficient of `ε^k` in the
    /// numerator; the denominator is `1`.
    #[must_use]
    pub fn from_polynomial(coeffs: Vec<RationalSurreal>) -> Self {
        Self {
            numer: EpsilonPolynomial::from_coefficients(coeffs),
            denom: EpsilonPolynomial::one(),
        }
    }

    /// Build directly from numerator and denominator polynomials.
    ///
    /// Returns `SurrealError::DivisionByZero` when the denominator is
    /// the zero polynomial.
    pub fn from_parts(
        numer: EpsilonPolynomial,
        denom: EpsilonPolynomial,
    ) -> Result<Self, SurrealError> {
        if denom.is_zero() {
            return Err(SurrealError::DivisionByZero);
        }
        Ok(Self { numer, denom }.normalize())
    }
}

// -- accessors --------------------------------------------------------------

impl EpsilonRational {
    /// Reference to the numerator polynomial.
    #[must_use]
    pub fn numer(&self) -> &EpsilonPolynomial {
        &self.numer
    }

    /// Reference to the denominator polynomial.
    #[must_use]
    pub fn denom(&self) -> &EpsilonPolynomial {
        &self.denom
    }

    /// Whether the rational function is identically zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.numer.is_zero()
    }
}

// -- arithmetic -------------------------------------------------------------

impl EpsilonRational {
    /// Checked reciprocal `1 / self`.
    ///
    /// Returns `SurrealError::DivisionByZero` when the numerator is zero
    /// (i.e. `self` is the zero rational function).
    pub fn checked_reciprocal(&self) -> Result<Self, SurrealError> {
        if self.is_zero() {
            return Err(SurrealError::DivisionByZero);
        }
        let value = Self {
            numer: self.denom.clone(),
            denom: self.numer.clone(),
        }
        .normalize();
        if !value.numer.within_bound() || !value.denom.within_bound() {
            return Err(SurrealError::ExponentOverflow {
                requested: (MAX_EPSILON_EXPONENT as u32) + 1,
                bound: MAX_EPSILON_EXPONENT as u32,
            });
        }
        Ok(value)
    }

    /// Checked division `self / rhs`.
    ///
    /// Returns `SurrealError::DivisionByZero` when `rhs` is zero.
    pub fn checked_div(&self, rhs: &Self) -> Result<Self, SurrealError> {
        let recip = rhs.checked_reciprocal()?;
        // TRANSIENT cross-multiplication, then bound the stored result.
        let numer = self.numer.mul_transient(&recip.numer);
        let denom = self.denom.mul_transient(&recip.denom);
        let value = Self { numer, denom }.normalize();
        if !value.numer.within_bound() || !value.denom.within_bound() {
            return Err(SurrealError::ExponentOverflow {
                requested: (MAX_EPSILON_EXPONENT as u32) + 1,
                bound: MAX_EPSILON_EXPONENT as u32,
            });
        }
        Ok(value)
    }
}

impl EpsilonRational {
    fn checked_add_impl(&self, rhs: &Self) -> Result<Self, SurrealError> {
        // N1/D1 + N2/D2 = (N1*D2 + N2*D1) / (D1*D2) — denominator
        // cross-products GROW exponents (R4): compute as transients,
        // then enforce the bound on the stored result.
        let numer = self
            .numer
            .mul_transient(&rhs.denom)
            .add(rhs.numer.clone().mul_transient(&self.denom));
        let denom = self.denom.mul_transient(&rhs.denom);
        let value = Self { numer, denom }.normalize();
        if !value.numer.within_bound() || !value.denom.within_bound() {
            return Err(SurrealError::ExponentOverflow {
                requested: (MAX_EPSILON_EXPONENT as u32) + 1,
                bound: MAX_EPSILON_EXPONENT as u32,
            });
        }
        Ok(value)
    }
}

impl Add for EpsilonRational {
    type Output = Self;

    /// # Panics
    ///
    /// Panics when the result leaves the representation bound (the
    /// denominator cross-product grows exponents; see
    /// [`EpsilonRational::checked_add`]).
    fn add(self, rhs: Self) -> Self::Output {
        self.checked_add(&rhs)
            .expect("exponent exceeds MAX_EPSILON_EXPONENT")
    }
}

impl Sub for EpsilonRational {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        self + (-rhs)
    }
}

impl Mul for EpsilonRational {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let numer = self.numer * rhs.numer;
        let denom = self.denom * rhs.denom;
        Self { numer, denom }.normalize()
    }
}

impl EpsilonRational {
    /// Checked addition enforcing the representation bound on the
    /// output (denominator cross-products can grow exponents).
    pub fn checked_add(&self, rhs: &Self) -> Result<Self, SurrealError> {
        self.checked_add_impl(rhs)
    }

    /// Checked subtraction (additive inverse of checked addition).
    pub fn checked_sub(&self, rhs: &Self) -> Result<Self, SurrealError> {
        self.checked_add_impl(&(-rhs.clone()))
    }

    /// Checked multiplication enforcing the representation bound.
    pub fn checked_mul(&self, rhs: &Self) -> Result<Self, SurrealError> {
        let value = Self {
            numer: self.numer.mul_transient(&rhs.numer),
            denom: self.denom.mul_transient(&rhs.denom),
        }
        .normalize();
        if !value.numer.within_bound() || !value.denom.within_bound() {
            return Err(SurrealError::ExponentOverflow {
                requested: (MAX_EPSILON_EXPONENT as u32) + 1,
                bound: MAX_EPSILON_EXPONENT as u32,
            });
        }
        Ok(value)
    }
}
impl Neg for EpsilonRational {
    type Output = Self;

    fn neg(self) -> Self::Output {
        Self {
            numer: -self.numer,
            denom: self.denom,
        }
    }
}

// -- equality (cross-multiplication) --------------------------------------
//
// IMPORTANT: Hash footgun warning.
//
// `PartialEq` / `Eq` compare rational functions via cross-multiplication
//   N₁/D₁ == N₂/D₂  ⇔  N₁·D₂ == N₂·D₁
// without first reducing to a canonical normal form (no polynomial GCD
// normalisation is performed).  Equality is therefore representation-
// independent, but the internal `numer` / `denom` fields may differ for
// equal values.
//
// **Do NOT derive `Hash` structurally** on `EpsilonRational` unless a
// canonical normal form is introduced.  A structural hash would violate
// the `Hash` / `Eq` contract: two equal values with different internal
// representations would produce different hashes, breaking `HashSet` and
// `HashMap` correctness.

impl PartialEq for EpsilonRational {
    fn eq(&self, other: &Self) -> bool {
        // N₁/D₁ == N₂/D₂  ⇔  N₁·D₂ == N₂·D₁ — TRANSIENT cross-products,
        // no representation bound, never panics (totality by design).
        let left = self.numer.mul_transient(&other.denom);
        let right = other.numer.mul_transient(&self.denom);
        left == right
    }
}

impl Eq for EpsilonRational {}

// -- ordering ---------------------------------------------------------------

impl PartialOrd for EpsilonRational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for EpsilonRational {
    fn cmp(&self, other: &Self) -> Ordering {
        // self - other = (N1*D2 - N2*D1) / (D1*D2).
        //
        // Both denominators are normalised so that their leading
        // coefficients are positive, therefore D1*D2 also has a
        // positive leading coefficient and the sign of the difference
        // is exactly the sign of the numerator polynomial.
        let diff_num = self
            .numer
            .mul_transient(&other.denom)
            .sub(other.numer.mul_transient(&self.denom));
        diff_num.sign_epsilon().unwrap_or(Ordering::Equal)
    }
}

// -- internal normalisation -------------------------------------------------

impl EpsilonRational {
    /// Normalise by removing zero coefficients and ensuring the
    /// denominator's leading coefficient is positive.
    fn normalize(mut self) -> Self {
        // Remove any zero coefficients that may have been introduced.
        self.numer.terms.retain(|_, v| !v.is_zero());
        self.denom.terms.retain(|_, v| !v.is_zero());

        // If the denominator's leading coefficient is negative, negate
        // both numerator and denominator to keep the denominator
        // positive.  (The denominator is known to be nonzero.)
        if let Some(ordering) = self.denom.sign_epsilon() {
            if ordering == Ordering::Less {
                self.numer = -self.numer;
                self.denom = -self.denom;
            }
        }

        // If both numerator and denominator are scalar constants,
        // simplify to a / 1 via rational scalar division.
        //
        // Safety of `unwrap` / `expect` below:
        // - The `retain` call above removes every zero coefficient from
        //   the map.  Therefore `degree() == Some(0)` guarantees that
        //   the key `EpsilonExponent(0)` is present with a nonzero
        //   coefficient.
        // - The denominator is known nonzero from the constructor guard,
        //   so `checked_div` on a nonzero denominator succeeds.
        if self.numer.is_constant() && self.denom.is_constant() {
            let num_val = self.numer.terms().get(&EpsilonExponent(0)).unwrap();
            let den_val = self.denom.terms().get(&EpsilonExponent(0)).unwrap();
            let scalar = num_val
                .clone()
                .checked_div(den_val)
                .expect("denominator constant is nonzero");
            self.numer = EpsilonPolynomial::from_scalar(scalar);
            self.denom = EpsilonPolynomial::one();
        }

        self
    }
}

// -- formatting -------------------------------------------------------------

impl fmt::Display for EpsilonRational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.denom.is_one() {
            write!(f, "{}", self.numer)
        } else {
            write!(f, "({}) / ({})", self.numer, self.denom)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RationalSurreal;

    // -- polynomial utilities -----------------------------------------------

    #[test]
    fn zero_polynomial_is_zero() {
        let z = EpsilonPolynomial::zero();
        assert!(z.is_zero());
        assert_eq!(z.degree(), None);
        assert_eq!(z.valuation(), None);
        assert_eq!(z.sign_epsilon(), None);
    }

    #[test]
    fn one_polynomial_has_degree_zero() {
        let one = EpsilonPolynomial::one();
        assert!(one.is_one());
        assert_eq!(one.degree(), Some(0));
        assert_eq!(one.valuation(), Some(0));
    }

    #[test]
    fn epsilon_polynomial_has_degree_one() {
        let eps = EpsilonPolynomial::epsilon();
        assert_eq!(eps.degree(), Some(1));
        assert_eq!(eps.valuation(), Some(1));
    }

    #[test]
    fn polynomial_addition() {
        let a = EpsilonPolynomial::from_coefficients(vec![
            RationalSurreal::from_integer(1),
            RationalSurreal::from_integer(2),
        ]);
        let b = EpsilonPolynomial::from_coefficients(vec![
            RationalSurreal::from_integer(3),
            RationalSurreal::from_integer(4),
        ]);
        let sum = a + b;
        assert_eq!(sum.degree(), Some(1));
        let c0 = sum.terms().get(&EpsilonExponent(0)).unwrap();
        assert_eq!(*c0, RationalSurreal::from_integer(4));
        let c1 = sum.terms().get(&EpsilonExponent(1)).unwrap();
        assert_eq!(*c1, RationalSurreal::from_integer(6));
    }

    #[test]
    fn polynomial_multiplication() {
        // (1 + ε) * (1 - ε) = 1 - ε²
        let a = EpsilonPolynomial::from_coefficients(vec![
            RationalSurreal::one(),
            RationalSurreal::one(),
        ]);
        let neg_one = RationalSurreal::one().neg();
        let b = EpsilonPolynomial::from_coefficients(vec![RationalSurreal::one(), neg_one]);
        let product = a * b;
        assert_eq!(product.degree(), Some(2));
        assert_eq!(
            *product.terms().get(&EpsilonExponent(0)).unwrap(),
            RationalSurreal::one()
        );
        assert_eq!(
            *product.terms().get(&EpsilonExponent(2)).unwrap(),
            RationalSurreal::one().neg()
        );
    }

    #[test]
    fn polynomial_negation() {
        let a = EpsilonPolynomial::from_coefficients(vec![
            RationalSurreal::from_integer(3),
            RationalSurreal::from_integer(-2),
        ]);
        let neg = -a.clone();
        let sum = a + neg;
        assert!(sum.is_zero());
    }

    #[test]
    fn polynomial_sign_epsilon() {
        // 1 - 1000ε: leading term is 1 > 0
        let p = EpsilonPolynomial::from_coefficients(vec![
            RationalSurreal::one(),
            RationalSurreal::from_integer(-1000),
        ]);
        assert_eq!(p.sign_epsilon(), Some(Ordering::Greater));

        // -0.5 + ε: leading term is -0.5 < 0
        let q = EpsilonPolynomial::from_coefficients(vec![
            RationalSurreal::from_ratio(-1, 2).unwrap(),
            RationalSurreal::one(),
        ]);
        assert_eq!(q.sign_epsilon(), Some(Ordering::Less));

        // ε: positive
        let r = EpsilonPolynomial::epsilon();
        assert_eq!(r.sign_epsilon(), Some(Ordering::Greater));
    }

    #[test]
    fn division_and_multiplication_are_consistent() {
        let a = EpsilonRational::from_scalar(RationalSurreal::from_integer(6));
        let b = EpsilonRational::from_scalar(RationalSurreal::from_integer(3));
        let q = a.checked_div(&b).unwrap();
        assert_eq!(
            q,
            EpsilonRational::from_scalar(RationalSurreal::from_integer(2))
        );
    }

    #[test]
    fn reciprocal_of_scalar() {
        let two = EpsilonRational::from_scalar(RationalSurreal::from_integer(2));
        let half = two.checked_reciprocal().unwrap();
        assert_eq!(half.numer().degree(), Some(0));
        assert_eq!(half.denom().degree(), Some(0));
        assert!(half > EpsilonRational::zero() && half < EpsilonRational::one());
    }

    #[test]
    fn addition_commutative() {
        let a = EpsilonRational::from_polynomial(vec![
            RationalSurreal::from_integer(1),
            RationalSurreal::from_integer(2),
        ]);
        let b = EpsilonRational::from_polynomial(vec![
            RationalSurreal::from_integer(3),
            RationalSurreal::one(),
        ]);
        assert_eq!(a.clone() + b.clone(), b + a);
    }

    #[test]
    fn zero_is_additive_identity() {
        let p = EpsilonRational::monomial(RationalSurreal::from_integer(5), 0);
        assert_eq!(p.clone() + EpsilonRational::zero(), p);
    }

    #[test]
    fn negative_is_additive_inverse() {
        let p = EpsilonRational::from_polynomial(vec![
            RationalSurreal::from_integer(1),
            RationalSurreal::from_integer(-2),
            RationalSurreal::one(),
        ]);
        let neg = -p.clone();
        assert_eq!(p + neg, EpsilonRational::zero());
    }

    #[test]
    fn one_is_multiplicative_identity() {
        let p = EpsilonRational::monomial(RationalSurreal::from_integer(7), 2);
        assert_eq!(p.clone() * EpsilonRational::one(), p);
    }

    #[test]
    fn denominator_sign_normalization() {
        // Construct a negative-denom rational via from_parts.
        let numer = EpsilonPolynomial::from_coefficients(vec![RationalSurreal::one()]);
        let denom = EpsilonPolynomial::from_coefficients(vec![RationalSurreal::one().neg()]);
        let r = EpsilonRational::from_parts(numer, denom).unwrap();
        // Denom should have been flipped to 1, numer to -1.
        assert_eq!(r.numer().degree(), Some(0));
        assert!(r < EpsilonRational::zero());
    }

    #[test]
    fn equal_values_compare_equal() {
        // Same rational value, different representation.
        let a = EpsilonRational::from_scalar(RationalSurreal::from_integer(2));
        let b = EpsilonRational::from_parts(
            EpsilonPolynomial::from_scalar(RationalSurreal::from_integer(6)),
            EpsilonPolynomial::from_scalar(RationalSurreal::from_integer(3)),
        )
        .unwrap();
        assert_eq!(a, b);
        // Verify Ord (not just PartialEq) handles equality correctly.
        assert_eq!(a.cmp(&b), Ordering::Equal);
    }

    #[test]
    fn zero_denom_rejected() {
        let r = EpsilonRational::from_parts(EpsilonPolynomial::one(), EpsilonPolynomial::zero());
        assert!(r.is_err());
    }

    // -- S9 regressions (R1/R2): Laurent terms survive arithmetic --------

    #[test]
    fn negative_exponent_plus_one_is_not_collapsed() {
        // ε^-1 + 1 must stay ε^-1 + 1 (was collapsed to 1).
        let eps_inv = EpsilonRational::monomial(RationalSurreal::one(), -1);
        let sum = eps_inv + EpsilonRational::one();
        assert_ne!(sum, EpsilonRational::one());
        assert_ne!(sum, EpsilonRational::monomial(RationalSurreal::one(), -1));
        // Ordering: ε^-1 + 1 > 1 (an infinite-ish positive part).
        assert!(sum > EpsilonRational::one());
        // Deeper: ε^-2 + ε^-1 + 1 keeps all three terms.
        let deep = EpsilonRational::monomial(RationalSurreal::one(), -2)
            + EpsilonRational::monomial(RationalSurreal::one(), -1)
            + EpsilonRational::one();
        assert!(deep > sum && sum > EpsilonRational::one());
    }

    #[test]
    fn is_one_requires_a_single_constant_term() {
        // 1 + ε^-1 is Laurent with degree 0 but NOT the constant one.
        let laurent = EpsilonPolynomial::monomial(RationalSurreal::one(), -1)
            + EpsilonPolynomial::from_scalar(RationalSurreal::one());
        assert_eq!(laurent.degree(), Some(0)); // the old guard's blind spot
        assert!(!laurent.is_one());
        assert!(!laurent.is_constant());
        assert!(EpsilonPolynomial::one().is_one());
        assert!(EpsilonPolynomial::from_scalar(RationalSurreal::from_integer(5)).is_constant());
        // Display: a Laurent denominator renders as a fraction, not bare.
        let r = EpsilonRational::from_parts(
            EpsilonPolynomial::from_scalar(RationalSurreal::one()),
            laurent,
        )
        .unwrap();
        let text = r.to_string();
        assert!(
            text.contains('/'),
            "Laurent denominator must display as a fraction: {text}"
        );
    }

    // -- exponent-bound boundary probes (R3/R4) ---------------------------

    #[test]
    fn checked_ops_error_at_the_bound_and_never_panic() {
        let bound = MAX_EPSILON_EXPONENT;
        // Constructor bound.
        assert!(EpsilonPolynomial::try_monomial(RationalSurreal::one(), bound + 1).is_err());
        assert!(EpsilonRational::try_monomial(RationalSurreal::one(), -(bound + 1)).is_err());
        // Repeated squaring crosses the bound honestly.
        let at_bound = EpsilonPolynomial::monomial(RationalSurreal::one(), bound / 2);
        let mut acc = at_bound.clone();
        let mut crossed = false;
        for _ in 0..3 {
            match acc.checked_mul(&at_bound) {
                Ok(next) => acc = next,
                Err(SurrealError::ExponentOverflow { .. }) => {
                    crossed = true;
                    break;
                }
                Err(_) => panic!("wrong error"),
            }
        }
        assert!(crossed, "squaring from bound/2 must cross the bound");
        // Rational layer: checked_add / checked_div propagate.
        let big = EpsilonRational::monomial(RationalSurreal::one(), bound);
        let recip = big.checked_reciprocal().unwrap(); // ε^-bound
        assert!(matches!(
            recip.checked_mul(&recip.clone()).unwrap_err(), // ε^-(2·bound)
            SurrealError::ExponentOverflow { .. }
        ));
        // Rational Add grows via DENOMINATOR cross-products (R4):
        // 1/ε^-(M/2+1) + itself has denominator ε^-(M+2) — out of bound.
        let tiny_denom = EpsilonRational::from_parts(
            EpsilonPolynomial::from_scalar(RationalSurreal::one()),
            EpsilonPolynomial::monomial(RationalSurreal::one(), -(bound / 2 + 1)),
        )
        .unwrap();
        assert!(matches!(
            tiny_denom.checked_add(&tiny_denom.clone()).unwrap_err(),
            SurrealError::ExponentOverflow { .. }
        ));
    }

    #[test]
    fn comparisons_stay_total_at_the_boundary() {
        let bound = MAX_EPSILON_EXPONENT;
        // Transient cross-products reach 2·bound; Ord never panics.
        let a = EpsilonRational::monomial(RationalSurreal::one(), bound);
        let b = EpsilonRational::monomial(RationalSurreal::from_integer(2), bound);
        let c = EpsilonRational::monomial(RationalSurreal::one(), -(bound / 2));
        assert!(a < b);
        assert!(a < c); // ε^-bound/2 dominates ε^bound
        let laurent = EpsilonRational::monomial(RationalSurreal::one(), -1);
        assert!(laurent > EpsilonRational::one());
        assert_eq!(a.cmp(&a), Ordering::Equal);
    }
}
