//! Checked binding keys. General inversion is not admission to the retrieval-key class.
use super::{AlgebraError, AlgebraResult};
use crate::memory::MemoryElement;

fn validate<A: MemoryElement>(x: &A) -> AlgebraResult<()> {
    let c = x.full_coefficients();
    if c.is_empty() {
        return Err(AlgebraError::KeyDomain);
    }
    if c.iter().any(|v| !v.is_finite()) {
        return Err(AlgebraError::NonFinite);
    }
    Ok(())
}
pub(super) fn certify<A: MemoryElement>(key: &A, inverse: A) -> AlgebraResult<A> {
    validate(key)?;
    validate(&inverse)?;
    let one = A::identity().full_coefficients();
    for product in [key.bind(&inverse), inverse.bind(key)] {
        let actual = product.full_coefficients();
        if actual.len() != one.len()
            || !actual.iter().zip(&one).all(|(x, y)| (x - y).abs() <= 1e-12)
        {
            return Err(AlgebraError::KeyDomain);
        }
    }
    Ok(inverse)
}
fn checked_product<A: MemoryElement>(key: &A, value: &A) -> AlgebraResult<A> {
    validate(value)?;
    let result = key.bind(value);
    validate(&result)?;
    Ok(result)
}
/// Key together with its certified two-sided inverse. Fields cannot be replaced independently.
#[derive(Clone, Debug)]
pub struct TypedKey<A> {
    key: A,
    inverse: A,
}
impl<A: MemoryElement> TypedKey<A> {
    /// Admit the backend's retrieval-key class, not every invertible element.
    pub fn try_new(key: A) -> AlgebraResult<Self> {
        validate(&key)?;
        if !key.admissible_key() {
            return Err(AlgebraError::KeyDomain);
        }
        let inverse = certify(&key, key.inverse()?)?;
        Ok(Self { key, inverse })
    }
    /// Original validated element (including every FHRR imaginary coefficient).
    pub fn as_element(&self) -> &A {
        &self.key
    }
    /// Bind with this key on the left.
    pub fn bind_key(&self, value: &A) -> AlgebraResult<A> {
        checked_product(&self.key, value)
    }
    /// Recover via the cached, certified left inverse.
    pub fn unbind_key(&self, bound: &A) -> AlgebraResult<A> {
        checked_product(&self.inverse, bound)
    }
}
/// General left division by a finite element with a representable two-sided inverse.
pub fn unbind_general<A: MemoryElement>(key: &A, bound: &A) -> AlgebraResult<A> {
    validate(key)?;
    checked_product(&certify(key, key.inverse()?)?, bound)
}
