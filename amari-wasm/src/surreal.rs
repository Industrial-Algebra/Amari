use crate::cgt::{WasmCgtArena, WasmGameId};
use amari_surreal::epsilon::EpsilonRational;
use amari_surreal::{Dyadic, RationalSurreal, ShortSurreal};
use wasm_bindgen::prelude::*;

fn surreal_error(error: amari_surreal::SurrealError) -> JsValue {
    JsValue::from_str(&error.to_string())
}

/// Exact dyadic rational `numerator / 2^exponent` for short surreal values.
#[wasm_bindgen]
#[derive(Clone)]
pub struct WasmDyadic {
    inner: Dyadic,
}

#[wasm_bindgen]
impl WasmDyadic {
    /// Create and normalize a dyadic rational `numerator / 2^exponent`.
    #[wasm_bindgen(constructor)]
    pub fn new(numerator: i32, exponent: u32) -> Self {
        Self {
            inner: Dyadic::new(numerator, exponent),
        }
    }

    /// Create a dyadic integer.
    #[wasm_bindgen(js_name = fromInteger)]
    pub fn from_integer(value: i32) -> Self {
        Self {
            inner: Dyadic::from_integer(value),
        }
    }

    /// Return zero.
    pub fn zero() -> Self {
        Self {
            inner: Dyadic::zero(),
        }
    }

    /// Return one.
    pub fn one() -> Self {
        Self {
            inner: Dyadic::one(),
        }
    }

    /// Numerator as a base-10 string.
    #[wasm_bindgen(js_name = numeratorString)]
    pub fn numerator_string(&self) -> String {
        self.inner.numer().to_string()
    }

    /// Power-of-two denominator exponent.
    pub fn exponent(&self) -> u32 {
        self.inner.exponent()
    }

    /// Format as an integer or rational string.
    pub fn format(&self) -> String {
        self.inner.to_string()
    }

    /// Return whether the dyadic is zero.
    #[wasm_bindgen(js_name = isZero)]
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// Return whether the dyadic is positive.
    #[wasm_bindgen(js_name = isPositive)]
    pub fn is_positive(&self) -> bool {
        self.inner.is_positive()
    }

    /// Return whether the dyadic is negative.
    #[wasm_bindgen(js_name = isNegative)]
    pub fn is_negative(&self) -> bool {
        self.inner.is_negative()
    }

    /// Return the sign as `negative`, `zero`, or `positive`.
    pub fn sign(&self) -> String {
        sign_name(self.inner.is_negative(), self.inner.is_zero()).to_string()
    }

    /// Add two dyadics exactly.
    pub fn add(&self, rhs: &WasmDyadic) -> WasmDyadic {
        WasmDyadic {
            inner: self.inner.clone() + rhs.inner.clone(),
        }
    }

    /// Subtract two dyadics exactly.
    pub fn sub(&self, rhs: &WasmDyadic) -> WasmDyadic {
        WasmDyadic {
            inner: self.inner.clone() - rhs.inner.clone(),
        }
    }

    /// Multiply two dyadics exactly.
    pub fn mul(&self, rhs: &WasmDyadic) -> WasmDyadic {
        WasmDyadic {
            inner: self.inner.clone() * rhs.inner.clone(),
        }
    }

    /// Negate the dyadic.
    pub fn neg(&self) -> WasmDyadic {
        WasmDyadic {
            inner: -self.inner.clone(),
        }
    }

    /// Absolute value.
    pub fn abs(&self) -> WasmDyadic {
        WasmDyadic {
            inner: self.inner.abs(),
        }
    }

    /// Checked reciprocal within the dyadic short-surreal layer.
    #[wasm_bindgen(js_name = checkedReciprocal)]
    pub fn checked_reciprocal(&self) -> Result<WasmDyadic, JsValue> {
        Ok(WasmDyadic {
            inner: self.inner.checked_reciprocal().map_err(surreal_error)?,
        })
    }

    /// Checked division; fails if the quotient leaves the dyadic layer.
    #[wasm_bindgen(js_name = checkedDiv)]
    pub fn checked_div(&self, rhs: &WasmDyadic) -> Result<WasmDyadic, JsValue> {
        Ok(WasmDyadic {
            inner: self.inner.checked_div(&rhs.inner).map_err(surreal_error)?,
        })
    }
}

impl WasmDyadic {
    fn wrap(inner: Dyadic) -> Self {
        Self { inner }
    }
}

/// Exact short surreal value backed by a dyadic rational.
#[wasm_bindgen]
#[derive(Clone)]
pub struct WasmShortSurreal {
    inner: ShortSurreal,
}

#[wasm_bindgen]
impl WasmShortSurreal {
    /// Return zero.
    pub fn zero() -> Self {
        Self {
            inner: ShortSurreal::zero(),
        }
    }

    /// Return one.
    pub fn one() -> Self {
        Self {
            inner: ShortSurreal::one(),
        }
    }

    /// Create a short surreal from an integer.
    #[wasm_bindgen(js_name = fromInteger)]
    pub fn from_integer(value: i32) -> Self {
        Self {
            inner: ShortSurreal::from_integer(i64::from(value)),
        }
    }

    /// Create a short surreal from an exact dyadic value.
    #[wasm_bindgen(js_name = fromDyadic)]
    pub fn from_dyadic(value: &WasmDyadic) -> Self {
        Self {
            inner: ShortSurreal::from_dyadic(value.inner.clone()),
        }
    }

    /// Convert a numeric CGT game to a short surreal.
    #[wasm_bindgen(js_name = fromGame)]
    pub fn from_game(
        arena: &mut WasmCgtArena,
        game: &WasmGameId,
    ) -> Result<WasmShortSurreal, JsValue> {
        let game = arena.game_id(game)?;
        Ok(WasmShortSurreal {
            inner: ShortSurreal::from_game(&mut arena.inner, game).map_err(surreal_error)?,
        })
    }

    /// Return the exact dyadic value.
    #[wasm_bindgen(js_name = toDyadic)]
    pub fn to_dyadic(&self) -> WasmDyadic {
        WasmDyadic::wrap(self.inner.to_dyadic())
    }

    /// Format as an integer or dyadic rational string.
    pub fn format(&self) -> String {
        self.inner.to_string()
    }

    /// Birthday of the short surreal value.
    pub fn birthday(&self) -> u32 {
        self.inner.birthday().0
    }

    /// Whether the value was converted from a CGT game handle.
    #[wasm_bindgen(js_name = hasProvenance)]
    pub fn has_provenance(&self) -> bool {
        self.inner.provenance().is_some()
    }

    /// Return whether the value is zero.
    #[wasm_bindgen(js_name = isZero)]
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// Return whether the value is positive.
    #[wasm_bindgen(js_name = isPositive)]
    pub fn is_positive(&self) -> bool {
        self.inner.is_positive()
    }

    /// Return whether the value is negative.
    #[wasm_bindgen(js_name = isNegative)]
    pub fn is_negative(&self) -> bool {
        self.inner.is_negative()
    }

    /// Return the sign as `negative`, `zero`, or `positive`.
    pub fn sign(&self) -> String {
        sign_name(self.inner.is_negative(), self.inner.is_zero()).to_string()
    }

    /// Absolute value.
    pub fn abs(&self) -> WasmShortSurreal {
        WasmShortSurreal {
            inner: self.inner.abs(),
        }
    }

    /// Add two short surreals exactly.
    pub fn add(&self, rhs: &WasmShortSurreal) -> WasmShortSurreal {
        WasmShortSurreal {
            inner: self.inner.clone() + rhs.inner.clone(),
        }
    }

    /// Subtract two short surreals exactly.
    pub fn sub(&self, rhs: &WasmShortSurreal) -> WasmShortSurreal {
        WasmShortSurreal {
            inner: self.inner.clone() - rhs.inner.clone(),
        }
    }

    /// Multiply two short surreals exactly.
    pub fn mul(&self, rhs: &WasmShortSurreal) -> WasmShortSurreal {
        WasmShortSurreal {
            inner: self.inner.clone() * rhs.inner.clone(),
        }
    }

    /// Negate the value.
    pub fn neg(&self) -> WasmShortSurreal {
        WasmShortSurreal {
            inner: -self.inner.clone(),
        }
    }

    /// Compare two short surreal values.
    pub fn compare(&self, rhs: &WasmShortSurreal) -> String {
        match self.inner.cmp(&rhs.inner) {
            std::cmp::Ordering::Less => "less",
            std::cmp::Ordering::Equal => "equal",
            std::cmp::Ordering::Greater => "greater",
        }
        .to_string()
    }

    /// Checked reciprocal within the dyadic short-surreal layer.
    #[wasm_bindgen(js_name = checkedReciprocal)]
    pub fn checked_reciprocal(&self) -> Result<WasmShortSurreal, JsValue> {
        Ok(WasmShortSurreal {
            inner: self.inner.checked_reciprocal().map_err(surreal_error)?,
        })
    }

    /// Checked division; fails if the quotient leaves the dyadic layer.
    #[wasm_bindgen(js_name = checkedDiv)]
    pub fn checked_div(&self, rhs: &WasmShortSurreal) -> Result<WasmShortSurreal, JsValue> {
        Ok(WasmShortSurreal {
            inner: self.inner.checked_div(&rhs.inner).map_err(surreal_error)?,
        })
    }

    /// Reconstruct this short surreal as a numeric CGT game in the provided arena.
    #[wasm_bindgen(js_name = toGameIn)]
    pub fn to_game_in(&self, arena: &mut WasmCgtArena) -> Result<WasmGameId, JsValue> {
        let game = self
            .inner
            .to_game_in(&mut arena.inner)
            .map_err(surreal_error)?;
        Ok(arena.wrap(game))
    }
}

impl WasmShortSurreal {
    pub(crate) fn inner_ref(&self) -> &ShortSurreal {
        &self.inner
    }
}

/// Exact rational surreal backed by `BigRational`.
///
/// `WasmRationalSurreal` provides an exact rational scalar field for the
/// surcomplex layer (v0.23+). It augments the existing dyadic
/// `WasmShortSurreal` layer with true rational division, supporting
/// denominators that are not powers of two.
#[wasm_bindgen]
#[derive(Clone)]
pub struct WasmRationalSurreal {
    inner: RationalSurreal,
}

#[wasm_bindgen]
impl WasmRationalSurreal {
    /// Return zero.
    pub fn zero() -> Self {
        Self {
            inner: RationalSurreal::zero(),
        }
    }

    /// Return one.
    pub fn one() -> Self {
        Self {
            inner: RationalSurreal::one(),
        }
    }

    /// Create a rational surreal from an integer.
    #[wasm_bindgen(js_name = fromInteger)]
    pub fn from_integer(value: i32) -> Self {
        Self {
            inner: RationalSurreal::from_integer(value),
        }
    }

    /// Create a rational surreal from a numerator and denominator.
    ///
    /// Returns an error when the denominator is zero.
    #[wasm_bindgen(js_name = fromRatio)]
    pub fn from_ratio(numer: i32, denom: i32) -> Result<WasmRationalSurreal, JsValue> {
        Ok(WasmRationalSurreal {
            inner: RationalSurreal::from_ratio(numer, denom).map_err(surreal_error)?,
        })
    }

    /// Convert a short surreal into a rational surreal.
    #[wasm_bindgen(js_name = fromShort)]
    pub fn from_short(value: &WasmShortSurreal) -> WasmRationalSurreal {
        WasmRationalSurreal {
            inner: RationalSurreal::from_short(value.inner_ref().clone()),
        }
    }

    /// Numerator as a base-10 string.
    #[wasm_bindgen(js_name = numeratorString)]
    pub fn numerator_string(&self) -> String {
        self.inner.numer().to_string()
    }

    /// Denominator as a base-10 string.
    #[wasm_bindgen(js_name = denominatorString)]
    pub fn denominator_string(&self) -> String {
        self.inner.denom().to_string()
    }

    /// Format as a rational string.
    pub fn format(&self) -> String {
        self.inner.to_string()
    }

    /// Return whether the rational is zero.
    #[wasm_bindgen(js_name = isZero)]
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// Return whether the rational is positive.
    #[wasm_bindgen(js_name = isPositive)]
    pub fn is_positive(&self) -> bool {
        self.inner.is_positive()
    }

    /// Return whether the rational is negative.
    #[wasm_bindgen(js_name = isNegative)]
    pub fn is_negative(&self) -> bool {
        self.inner.is_negative()
    }

    /// Return the sign as `negative`, `zero`, or `positive`.
    pub fn sign(&self) -> String {
        sign_name(self.inner.is_negative(), self.inner.is_zero()).to_string()
    }

    /// Absolute value.
    pub fn abs(&self) -> WasmRationalSurreal {
        WasmRationalSurreal {
            inner: self.inner.abs(),
        }
    }

    /// Add two rational surreals exactly.
    pub fn add(&self, rhs: &WasmRationalSurreal) -> WasmRationalSurreal {
        WasmRationalSurreal {
            inner: self.inner.clone() + rhs.inner.clone(),
        }
    }

    /// Subtract two rational surreals exactly.
    pub fn sub(&self, rhs: &WasmRationalSurreal) -> WasmRationalSurreal {
        WasmRationalSurreal {
            inner: self.inner.clone() - rhs.inner.clone(),
        }
    }

    /// Multiply two rational surreals exactly.
    pub fn mul(&self, rhs: &WasmRationalSurreal) -> WasmRationalSurreal {
        WasmRationalSurreal {
            inner: self.inner.clone() * rhs.inner.clone(),
        }
    }

    /// Negate the value.
    pub fn neg(&self) -> WasmRationalSurreal {
        WasmRationalSurreal {
            inner: -self.inner.clone(),
        }
    }

    /// Checked reciprocal.
    #[wasm_bindgen(js_name = checkedReciprocal)]
    pub fn checked_reciprocal(&self) -> Result<WasmRationalSurreal, JsValue> {
        Ok(WasmRationalSurreal {
            inner: self.inner.checked_reciprocal().map_err(surreal_error)?,
        })
    }

    /// Checked division.
    #[wasm_bindgen(js_name = checkedDiv)]
    pub fn checked_div(&self, rhs: &WasmRationalSurreal) -> Result<WasmRationalSurreal, JsValue> {
        Ok(WasmRationalSurreal {
            inner: self.inner.checked_div(&rhs.inner).map_err(surreal_error)?,
        })
    }

    /// Compare two rational surreal values.
    pub fn compare(&self, rhs: &WasmRationalSurreal) -> String {
        match self.inner.cmp(&rhs.inner) {
            std::cmp::Ordering::Less => "less",
            std::cmp::Ordering::Equal => "equal",
            std::cmp::Ordering::Greater => "greater",
        }
        .to_string()
    }

    /// Convert to a short surreal when the value is dyadic.
    ///
    /// Returns `Some` only when the normalized denominator is a power
    /// of two (dyadic rational); returns `None` for non-dyadic values.
    #[wasm_bindgen(js_name = toShortIfDyadic)]
    pub fn to_short_if_dyadic(&self) -> Option<WasmShortSurreal> {
        self.inner
            .to_short_if_dyadic()
            .map(|s| WasmShortSurreal { inner: s })
    }
}

impl WasmRationalSurreal {
    pub(crate) fn wrap(inner: RationalSurreal) -> Self {
        Self { inner }
    }

    pub(crate) fn inner_ref(&self) -> &RationalSurreal {
        &self.inner
    }
}

/// Experimental epsilon rational function.
///
/// A rational function in a formal positive infinitesimal `ε` with
/// rational surreal coefficients.  Ordered by asymptotic behaviour as
/// `ε → 0⁺`.
///
/// **Experimental**: this API is behind the `experimental-epsilon`
/// feature flag and may change without semver guarantees.
#[wasm_bindgen]
pub struct WasmExperimentalEpsilonRational {
    inner: EpsilonRational,
}

#[wasm_bindgen]
impl WasmExperimentalEpsilonRational {
    /// Return zero (`0 / 1`).
    pub fn zero() -> Self {
        Self {
            inner: EpsilonRational::zero(),
        }
    }

    /// Return one (`1 / 1`).
    pub fn one() -> Self {
        Self {
            inner: EpsilonRational::one(),
        }
    }

    /// Return the formal infinitesimal `ε`.
    pub fn epsilon() -> Self {
        Self {
            inner: EpsilonRational::epsilon(),
        }
    }

    /// Build from a scalar constant: `scalar / 1`.
    #[wasm_bindgen(js_name = fromScalar)]
    pub fn from_scalar(scalar: &WasmRationalSurreal) -> Self {
        Self {
            inner: EpsilonRational::from_scalar(scalar.inner_ref().clone()),
        }
    }

    /// A monomial rational function `coeff · ε^exp / 1`.
    pub fn monomial(coeff: &WasmRationalSurreal, exponent: i32) -> Self {
        Self {
            inner: EpsilonRational::monomial(coeff.inner_ref().clone(), exponent),
        }
    }

    /// Format the epsilon rational.
    pub fn format(&self) -> String {
        self.inner.to_string()
    }

    /// Add two epsilon rationals.
    pub fn add(&self, rhs: &WasmExperimentalEpsilonRational) -> WasmExperimentalEpsilonRational {
        WasmExperimentalEpsilonRational {
            inner: self.inner.clone() + rhs.inner.clone(),
        }
    }

    /// Subtract two epsilon rationals.
    pub fn sub(&self, rhs: &WasmExperimentalEpsilonRational) -> WasmExperimentalEpsilonRational {
        WasmExperimentalEpsilonRational {
            inner: self.inner.clone() - rhs.inner.clone(),
        }
    }

    /// Multiply two epsilon rationals.
    pub fn mul(&self, rhs: &WasmExperimentalEpsilonRational) -> WasmExperimentalEpsilonRational {
        WasmExperimentalEpsilonRational {
            inner: self.inner.clone() * rhs.inner.clone(),
        }
    }

    /// Negate the value.
    pub fn neg(&self) -> WasmExperimentalEpsilonRational {
        WasmExperimentalEpsilonRational {
            inner: -self.inner.clone(),
        }
    }

    /// Checked reciprocal.
    #[wasm_bindgen(js_name = checkedReciprocal)]
    pub fn checked_reciprocal(&self) -> Result<WasmExperimentalEpsilonRational, JsValue> {
        Ok(WasmExperimentalEpsilonRational {
            inner: self.inner.checked_reciprocal().map_err(surreal_error)?,
        })
    }

    /// Checked division.
    #[wasm_bindgen(js_name = checkedDiv)]
    pub fn checked_div(
        &self,
        rhs: &WasmExperimentalEpsilonRational,
    ) -> Result<WasmExperimentalEpsilonRational, JsValue> {
        Ok(WasmExperimentalEpsilonRational {
            inner: self.inner.checked_div(&rhs.inner).map_err(surreal_error)?,
        })
    }

    /// Compare two epsilon rationals.
    pub fn compare(&self, rhs: &WasmExperimentalEpsilonRational) -> String {
        match self.inner.cmp(&rhs.inner) {
            std::cmp::Ordering::Less => "less",
            std::cmp::Ordering::Equal => "equal",
            std::cmp::Ordering::Greater => "greater",
        }
        .to_string()
    }
}

fn sign_name(is_negative: bool, is_zero: bool) -> &'static str {
    if is_zero {
        "zero"
    } else if is_negative {
        "negative"
    } else {
        "positive"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cgt::WasmCgtArena;

    #[test]
    fn wasm_dyadic_and_short_surreal_arithmetic_are_exact() {
        let half = WasmDyadic::new(1, 1);
        let quarter = WasmDyadic::new(1, 2);
        let three_quarters = half.add(&quarter);

        assert_eq!(three_quarters.numerator_string(), "3");
        assert_eq!(three_quarters.exponent(), 2);
        assert_eq!(three_quarters.format(), "3/4");

        let one = WasmShortSurreal::from_integer(1);
        let value = one.add(&WasmShortSurreal::from_dyadic(&half));
        assert_eq!(value.format(), "3/2");
        assert_eq!(value.sign(), "positive");
        assert_eq!(value.birthday(), 3);
        assert_eq!(
            value
                .checked_div(&WasmShortSurreal::from_integer(3))
                .unwrap()
                .format(),
            "1/2"
        );
    }

    #[test]
    fn wasm_short_surreal_converts_from_and_to_cgt_games() {
        let mut arena = WasmCgtArena::new();
        let zero = arena.zero();
        let one = arena.one().unwrap();
        let half_game = arena.cut(&zero, &one).unwrap();

        let half = WasmShortSurreal::from_game(&mut arena, &half_game).unwrap();
        assert_eq!(half.format(), "1/2");
        assert_eq!(half.birthday(), 2);
        assert!(half.has_provenance());

        let rebuilt = half.to_game_in(&mut arena).unwrap();
        assert!(arena.is_numeric(&rebuilt).unwrap());
        assert_eq!(
            WasmShortSurreal::from_game(&mut arena, &rebuilt)
                .unwrap()
                .format(),
            "1/2"
        );
    }

    // ========================================================================
    // WasmRationalSurreal tests
    // ========================================================================

    #[test]
    fn rational_exact_one_third_plus_one_sixth_is_one_half() {
        let third = WasmRationalSurreal::from_ratio(1, 3).unwrap();
        let sixth = WasmRationalSurreal::from_ratio(1, 6).unwrap();
        let half = WasmRationalSurreal::from_ratio(1, 2).unwrap();
        let sum = third.add(&sixth);
        assert_eq!(sum.format(), "1/2");
        assert_eq!(sum.compare(&half), "equal");
    }

    #[test]
    fn rational_to_short_if_dyadic_one_half_is_some() {
        let half = WasmRationalSurreal::from_ratio(1, 2).unwrap();
        let short = half.to_short_if_dyadic();
        assert!(short.is_some());
        assert_eq!(short.unwrap().format(), "1/2");
    }

    #[test]
    fn rational_to_short_if_dyadic_one_third_is_none() {
        let third = WasmRationalSurreal::from_ratio(1, 3).unwrap();
        let short = third.to_short_if_dyadic();
        assert!(short.is_none());
    }

    #[test]
    fn rational_from_integer_and_sign() {
        let zero = WasmRationalSurreal::zero();
        assert!(zero.is_zero());
        assert_eq!(zero.sign(), "zero");

        let five = WasmRationalSurreal::from_integer(5);
        assert!(five.is_positive());
        assert!(!five.is_negative());
        assert_eq!(five.sign(), "positive");

        let neg_three = WasmRationalSurreal::from_integer(-3);
        assert!(neg_three.is_negative());
        assert!(!neg_three.is_positive());
        assert_eq!(neg_three.sign(), "negative");
    }

    #[test]
    fn rational_arithmetic_and_reciprocal() {
        let two = WasmRationalSurreal::from_integer(2);
        let three = WasmRationalSurreal::from_integer(3);
        assert_eq!(two.add(&three).format(), "5");
        assert_eq!(two.mul(&three).format(), "6");
        assert_eq!(three.sub(&two).format(), "1");
        assert_eq!(two.neg().format(), "-2");
        assert_eq!(two.abs().format(), "2");

        let half = two.checked_reciprocal().unwrap();
        assert_eq!(half.format(), "1/2");

        let six = WasmRationalSurreal::from_integer(6);
        let q = six.checked_div(&three).unwrap();
        assert_eq!(q.format(), "2");
    }

    #[test]
    fn rational_compare() {
        let half = WasmRationalSurreal::from_ratio(1, 2).unwrap();
        let third = WasmRationalSurreal::from_ratio(1, 3).unwrap();
        assert_eq!(half.compare(&third), "greater");
        assert_eq!(third.compare(&half), "less");
        assert_eq!(half.compare(&half), "equal");
    }

    #[test]
    fn rational_numerator_denominator_strings() {
        let r = WasmRationalSurreal::from_ratio(3, 4).unwrap();
        assert_eq!(r.numerator_string(), "3");
        assert_eq!(r.denominator_string(), "4");
    }

    // ========================================================================
    // WasmExperimentalEpsilonRational tests
    // ========================================================================

    #[test]
    fn epsilon_epsilon_gt_zero() {
        let eps = WasmExperimentalEpsilonRational::epsilon();
        let zero = WasmExperimentalEpsilonRational::zero();
        assert_eq!(eps.compare(&zero), "greater");
        assert_eq!(zero.compare(&eps), "less");
    }

    #[test]
    fn epsilon_squared_lt_epsilon() {
        let eps = WasmExperimentalEpsilonRational::epsilon();
        let eps_sq = eps.mul(&eps);
        // ε² < ε (since ε → 0⁺)
        assert_eq!(eps_sq.compare(&eps), "less");
    }

    #[test]
    fn epsilon_reciprocal_larger_than_large_integer() {
        let eps = WasmExperimentalEpsilonRational::epsilon();
        let recip = eps.checked_reciprocal().unwrap();
        let large = WasmExperimentalEpsilonRational::from_scalar(
            &WasmRationalSurreal::from_integer(1_000_000),
        );
        // 1/ε > 1_000_000
        assert_eq!(recip.compare(&large), "greater");
    }

    #[test]
    fn epsilon_arithmetic_basics() {
        let eps = WasmExperimentalEpsilonRational::epsilon();
        let one = WasmExperimentalEpsilonRational::one();
        let sum = one.add(&eps);
        assert!(!sum.format().is_empty());

        let diff = one.sub(&eps);
        assert!(!diff.format().is_empty());

        let neg = eps.neg();
        assert_eq!(
            neg.compare(&WasmExperimentalEpsilonRational::zero()),
            "less"
        );
    }
}
