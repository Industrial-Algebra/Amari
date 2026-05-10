use crate::surreal::WasmRationalSurreal;
use amari_surcomplex::{RationalSurcomplex, SurcomplexError};
use wasm_bindgen::prelude::*;

fn surcomplex_error(error: SurcomplexError) -> JsValue {
    match error {
        SurcomplexError::DivisionByZero => JsValue::from_str("division by zero"),
        SurcomplexError::Surreal(e) => JsValue::from_str(&e.to_string()),
    }
}

/// Exact rational surcomplex number `a + bi` with `a, b ∈ Q`.
///
/// `WasmRationalSurcomplex` wraps exact complex numbers backed by
/// [`WasmRationalSurreal`] for the real and imaginary parts.
#[wasm_bindgen]
#[derive(Clone)]
pub struct WasmRationalSurcomplex {
    inner: RationalSurcomplex,
}

#[wasm_bindgen]
impl WasmRationalSurcomplex {
    /// Return zero (`0 + 0i`).
    pub fn zero() -> Self {
        Self {
            inner: RationalSurcomplex::zero(),
        }
    }

    /// Return one (`1 + 0i`).
    pub fn one() -> Self {
        Self {
            inner: RationalSurcomplex::one(),
        }
    }

    /// Return the imaginary unit `i` (`0 + 1i`).
    pub fn i() -> Self {
        Self {
            inner: RationalSurcomplex::i(),
        }
    }

    /// Create a real surcomplex from an integer.
    #[wasm_bindgen(js_name = fromInteger)]
    pub fn from_integer(value: i32) -> Self {
        Self {
            inner: RationalSurcomplex::from_integer(value),
        }
    }

    /// Create a real surcomplex from a rational surreal.
    #[wasm_bindgen(js_name = fromReal)]
    pub fn from_real(real: &WasmRationalSurreal) -> Self {
        Self {
            inner: RationalSurcomplex::from_real(real.inner_ref().clone()),
        }
    }

    /// Create a surcomplex from real and imaginary parts.
    #[wasm_bindgen(js_name = fromParts)]
    pub fn from_parts(real: &WasmRationalSurreal, imag: &WasmRationalSurreal) -> Self {
        Self {
            inner: RationalSurcomplex::from_parts(
                real.inner_ref().clone(),
                imag.inner_ref().clone(),
            ),
        }
    }

    /// Return the real part as a rational surreal.
    pub fn real(&self) -> WasmRationalSurreal {
        WasmRationalSurreal::wrap(self.inner.real().clone())
    }

    /// Return the imaginary part as a rational surreal.
    pub fn imag(&self) -> WasmRationalSurreal {
        WasmRationalSurreal::wrap(self.inner.imag().clone())
    }

    /// Format the surcomplex as a string.
    pub fn format(&self) -> String {
        self.inner.to_string()
    }

    /// Return the complex conjugate `a - bi`.
    pub fn conjugate(&self) -> WasmRationalSurcomplex {
        WasmRationalSurcomplex {
            inner: self.inner.conjugate(),
        }
    }

    /// Return the squared norm `a² + b²` as a rational surreal.
    #[wasm_bindgen(js_name = normSq)]
    pub fn norm_sq(&self) -> WasmRationalSurreal {
        WasmRationalSurreal::wrap(self.inner.norm_sq())
    }

    /// Return whether the surcomplex is zero.
    #[wasm_bindgen(js_name = isZero)]
    pub fn is_zero(&self) -> bool {
        self.inner.is_zero()
    }

    /// Add two surcomplex numbers.
    pub fn add(&self, rhs: &WasmRationalSurcomplex) -> WasmRationalSurcomplex {
        WasmRationalSurcomplex {
            inner: self.inner.clone() + rhs.inner.clone(),
        }
    }

    /// Subtract two surcomplex numbers.
    pub fn sub(&self, rhs: &WasmRationalSurcomplex) -> WasmRationalSurcomplex {
        WasmRationalSurcomplex {
            inner: self.inner.clone() - rhs.inner.clone(),
        }
    }

    /// Multiply two surcomplex numbers.
    pub fn mul(&self, rhs: &WasmRationalSurcomplex) -> WasmRationalSurcomplex {
        WasmRationalSurcomplex {
            inner: self.inner.clone() * rhs.inner.clone(),
        }
    }

    /// Negate the surcomplex.
    pub fn neg(&self) -> WasmRationalSurcomplex {
        WasmRationalSurcomplex {
            inner: -self.inner.clone(),
        }
    }

    /// Checked reciprocal `1 / self`.
    ///
    /// Returns an error when the value is zero.
    #[wasm_bindgen(js_name = checkedReciprocal)]
    pub fn checked_reciprocal(&self) -> Result<WasmRationalSurcomplex, JsValue> {
        Ok(WasmRationalSurcomplex {
            inner: self.inner.checked_reciprocal().map_err(surcomplex_error)?,
        })
    }

    /// Checked division `self / rhs`.
    ///
    /// Returns an error when `rhs` is zero.
    #[wasm_bindgen(js_name = checkedDiv)]
    pub fn checked_div(
        &self,
        rhs: &WasmRationalSurcomplex,
    ) -> Result<WasmRationalSurcomplex, JsValue> {
        Ok(WasmRationalSurcomplex {
            inner: self
                .inner
                .checked_div(&rhs.inner)
                .map_err(surcomplex_error)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surreal::WasmRationalSurreal;

    #[test]
    fn surcomplex_division_one_over_one_plus_half_i() {
        // 1 / (1 + 1/2 i) = 4/5 - 2/5 i
        let one = WasmRationalSurreal::one();
        let half = WasmRationalSurreal::from_ratio(1, 2).unwrap();
        let z = WasmRationalSurcomplex::from_parts(&one, &half);
        let q = WasmRationalSurcomplex::one().checked_div(&z).unwrap();

        assert_eq!(q.real().format(), "4/5");
        assert_eq!(q.imag().format(), "-2/5");
        assert_eq!(q.format(), "4/5 - 2/5i");
    }

    #[test]
    fn surcomplex_zero_one_and_i() {
        let zero = WasmRationalSurcomplex::zero();
        assert!(zero.is_zero());
        assert_eq!(zero.format(), "0");

        let one = WasmRationalSurcomplex::one();
        assert!(!one.is_zero());
        assert_eq!(one.format(), "1");

        let i = WasmRationalSurcomplex::i();
        assert!(!i.is_zero());
        assert_eq!(i.format(), "i");
    }

    #[test]
    fn surcomplex_conjugate() {
        let r = WasmRationalSurreal::from_ratio(3, 2).unwrap();
        let im = WasmRationalSurreal::from_ratio(4, 2).unwrap();
        let z = WasmRationalSurcomplex::from_parts(&r, &im);
        let conj = z.conjugate();

        assert_eq!(conj.real().format(), "3/2");
        assert_eq!(conj.imag().format(), "-2"); // -4/2 = -2
    }

    #[test]
    fn surcomplex_norm_sq() {
        // |3 + 4i|² = 3² + 4² = 9 + 16 = 25
        let r = WasmRationalSurreal::from_integer(3);
        let im = WasmRationalSurreal::from_integer(4);
        let z = WasmRationalSurcomplex::from_parts(&r, &im);
        let nsq = z.norm_sq();
        assert_eq!(nsq.format(), "25");
    }

    #[test]
    fn surcomplex_arithmetic() {
        let a = WasmRationalSurcomplex::from_integer(2);
        let b = WasmRationalSurcomplex::from_integer(3);
        assert_eq!(a.add(&b).format(), "5");
        assert_eq!(a.mul(&b).format(), "6");
    }

    #[test]
    fn surcomplex_imul() {
        // i * i = -1
        let i = WasmRationalSurcomplex::i();
        let i_sq = i.mul(&i);
        assert_eq!(i_sq.format(), "-1");
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn surcomplex_reciprocal_fails_for_zero() {
        let zero = WasmRationalSurcomplex::zero();
        assert!(zero.checked_reciprocal().is_err());
    }

    #[test]
    #[cfg(target_arch = "wasm32")]
    fn surcomplex_div_fails_for_zero() {
        let one = WasmRationalSurcomplex::one();
        let zero = WasmRationalSurcomplex::zero();
        assert!(one.checked_div(&zero).is_err());
    }
}
