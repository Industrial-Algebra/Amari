//! Checked Euclidean geometry on complete real coefficient vectors.
use super::{AlgebraError, AlgebraResult};
use alloc::vec::Vec;
fn scale(c: &[f64]) -> AlgebraResult<f64> {
    let mut m = 0_f64;
    for &x in c {
        if !x.is_finite() {
            return Err(AlgebraError::NonFinite);
        }
        m = m.max(x.abs());
    }
    if m.is_subnormal() {
        return Err(AlgebraError::Unrepresentable);
    }
    Ok(m)
}
pub(super) fn norm(c: &[f64]) -> AlgebraResult<f64> {
    let m = scale(c)?;
    if m == 0. {
        return Ok(0.);
    }
    let n = m * c.iter().map(|x| (x / m).powi(2)).sum::<f64>().sqrt();
    if !n.is_finite() {
        return Err(AlgebraError::Unrepresentable);
    }
    Ok(n)
}
pub(super) fn unit(mut c: Vec<f64>) -> AlgebraResult<Vec<f64>> {
    let m = scale(&c)?;
    if m == 0. {
        return Err(AlgebraError::NormalizationFailed { norm: 0. });
    }
    let n = c.iter().map(|x| (x / m).powi(2)).sum::<f64>().sqrt();
    for x in &mut c {
        let v = (*x / m) / n;
        if *x != 0. && (v == 0. || v.is_subnormal()) {
            return Err(AlgebraError::Unrepresentable);
        }
        *x = v;
    }
    Ok(c)
}
pub(super) fn cosine(a: Vec<f64>, b: Vec<f64>) -> AlgebraResult<f64> {
    if a.len() != b.len() {
        return Err(AlgebraError::DimensionMismatch {
            expected: a.len(),
            actual: b.len(),
        });
    } // Exact on the diagonal, not on a neighborhood around it. Validate before taking the identity.
    if a.iter().zip(&b).all(|(x, y)| x.to_bits() == y.to_bits()) {
        if scale(&a)? == 0. {
            return Err(AlgebraError::NormalizationFailed { norm: 0. });
        }
        return Ok(1.);
    }
    let a = unit(a)?;
    let b = unit(b)?;
    Ok(a.iter()
        .zip(b)
        .map(|(x, y)| x * y)
        .sum::<f64>()
        .clamp(-1., 1.))
}
