use crate::cgt::{WasmCgtArena, WasmGameId};
use amari_surreal::{Dyadic, ShortSurreal};
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
}
