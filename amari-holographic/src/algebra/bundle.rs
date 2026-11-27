//! Whole-list reducers shared by the checked serial and Rayon entry points.
use super::{AlgebraError, AlgebraResult, BindingAlgebra};
use alloc::vec::Vec;

pub(super) fn serial<A: BindingAlgebra>(
    items: &[A],
    beta: f64,
    cleanup: impl FnOnce(A) -> A,
) -> AlgebraResult<A> {
    checked(items, beta, || items.iter().map(A::norm).collect(), cleanup)
}

// Validate beta first, then every input in index order (including singletons),
// before either the hard or the soft branch can return a value.
pub(super) fn checked<A: BindingAlgebra>(
    items: &[A],
    beta: f64,
    norms: impl FnOnce() -> AlgebraResult<Vec<f64>>,
    cleanup: impl FnOnce(A) -> A,
) -> AlgebraResult<A> {
    if beta.is_nan() || beta < 0. {
        return Err(AlgebraError::Beta);
    }
    let norms = norms()?;
    match items {
        [] => return Ok(A::zero()),
        [item] => return Ok(item.clone()),
        _ => {}
    }
    if beta == f64::INFINITY {
        return hard_bundle(items);
    }
    let max = norms.iter().copied().fold(0., f64::max);
    let weights: Vec<_> = norms
        .iter()
        .map(|n| {
            if max < 1e-10 || beta == 0. {
                1.
            } else {
                ((n - max) * beta).exp()
            }
        })
        .collect();
    let total: f64 = weights.iter().sum();
    let mut sum = A::zero();
    for (x, w) in items.iter().zip(weights) {
        sum = sum.superpose(&x.scale(w / total)?)?;
    }
    if sum.geometry_coefficients().iter().any(|x| !x.is_finite()) {
        return Err(AlgebraError::Unrepresentable);
    }
    Ok(cleanup(sum))
}

fn hard_bundle<A: BindingAlgebra>(items: &[A]) -> AlgebraResult<A> {
    let first = items[0].geometry_coefficients();
    let mut votes = alloc::vec![0_isize;first.len()];
    for x in items {
        let c = x.geometry_coefficients();
        if c.len() != first.len() {
            return Err(AlgebraError::InvalidShape);
        }
        for (v, a) in votes.iter_mut().zip(c) {
            *v += if a > 0. {
                1
            } else if a < 0. {
                -1
            } else {
                0
            };
        }
    }
    let result: Vec<_> = votes
        .into_iter()
        .zip(first)
        .map(|(v, x)| if v == 0 { x } else { v.signum() as f64 })
        .collect();
    A::from_geometry_coefficients(&result)
}
