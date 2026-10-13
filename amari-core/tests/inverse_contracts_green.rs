// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: Apache-2.0
//! CORE-W03 stage-3 green verification for [`Multivector::inverse`].
//!
//! Contract: docs/plans/2026-10-10-amari-core-w03-inverse-exp-contract.md
//! (D02.1/D02.2). These tests exercise the general LU inverse and the scalar
//! fast path across default, mixed, and degenerate signatures.
//!
//! ## Conditioning criterion
//!
//! For a deterministic pseudo-random draw `M`, the left-multiplication matrix
//! `A` is rebuilt independently and factorized with partial pivoting. A draw is
//! accepted as *well-conditioned* only when the scale-relative pivot-product
//! ratio
//!
//! ```text
//! |det(A)| / max_abs(A)^N >= 1e-8
//! ```
//!
//! (evaluated in log space to avoid overflow/underflow of `det`). Ill-
//! conditioned or singular draws are skipped under a fixed seed schedule; the
//! acceptance tolerance `1e-6` leaves many orders of headroom for a
//! `1e-8`-conditioned solve.

use amari_core::Multivector;

/// Blade index from generator numbers (1-based): e12 -> 0b0011 = 3.
const fn blade(g: &[usize]) -> usize {
    let mut i = 0;
    let mut n = 0;
    while i < g.len() {
        n |= 1 << (g[i] - 1);
        i += 1;
    }
    n
}

fn mv<const P: usize, const Q: usize, const R: usize>(
    parts: &[(f64, &[usize])],
) -> Multivector<P, Q, R> {
    let mut m = Multivector::zero();
    for (c, g) in parts {
        let idx = if g.is_empty() { 0 } else { blade(g) };
        m.set(idx, *c);
    }
    m
}

/// Contract acceptance: both multiplication orders, sound denominator.
fn assert_inverse_acceptable<const P: usize, const Q: usize, const R: usize>(
    m: &Multivector<P, Q, R>,
    x: &Multivector<P, Q, R>,
) {
    let tol = 1e-6 * (m.coefficient_norm() * x.coefficient_norm()).max(1.0);
    let mut identity = Multivector::<P, Q, R>::zero();
    identity.set(0, 1.0);
    for product in [m.geometric_product(x), x.geometric_product(m)] {
        let d = product + identity.clone() * -1.0;
        assert!(
            d.coefficient_norm() <= tol,
            "residual {} > {tol}",
            d.coefficient_norm()
        );
    }
}

/// `ln(|det(A)| / max_abs(A)^N)` for the left-multiplication matrix of `m`.
///
/// `NEG_INFINITY` marks a singular matrix. This is the independent condition
/// measure used to decide whether a random draw is well-conditioned.
fn log_det_ratio<const P: usize, const Q: usize, const R: usize>(m: &Multivector<P, Q, R>) -> f64 {
    let n = Multivector::<P, Q, R>::BASIS_COUNT;
    let mut a = vec![0.0f64; n * n];
    let mut max_abs = 0.0f64;
    for j in 0..n {
        let mut basis = Multivector::<P, Q, R>::zero();
        basis.set(j, 1.0);
        let col = m.geometric_product(&basis);
        for (i, &v) in col.as_slice().iter().enumerate() {
            a[i * n + j] = v;
            let av = v.abs();
            if av > max_abs {
                max_abs = av;
            }
        }
    }
    if max_abs == 0.0 || !max_abs.is_finite() {
        return f64::NEG_INFINITY;
    }
    let mut log_sum = 0.0f64;
    for k in 0..n {
        let mut pivot_row = k;
        let mut pivot_mag = a[k * n + k].abs();
        for r in (k + 1)..n {
            let av = a[r * n + k].abs();
            if av > pivot_mag {
                pivot_mag = av;
                pivot_row = r;
            }
        }
        if pivot_mag == 0.0 {
            return f64::NEG_INFINITY;
        }
        log_sum += pivot_mag.ln();
        if pivot_row != k {
            for c in 0..n {
                a.swap(k * n + c, pivot_row * n + c);
            }
        }
        let pivot = a[k * n + k];
        for r in (k + 1)..n {
            let factor = a[r * n + k] / pivot;
            for c in (k + 1)..n {
                a[r * n + c] -= factor * a[k * n + c];
            }
        }
    }
    log_sum - (n as f64) * max_abs.ln()
}

/// Deterministic SplitMix64 for reproducible random-ish draws.
struct SplitMix64(u64);

impl SplitMix64 {
    const fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform draw in `[-1, 1)`.
    fn next_signed(&mut self) -> f64 {
        let bits = self.next_u64() >> 11;
        (bits as f64) / ((1u64 << 53) as f64) * 2.0 - 1.0
    }
}

/// Draw up to five well-conditioned elements and verify their inverses.
fn check_random_draws<const P: usize, const Q: usize, const R: usize>(seed: u64, label: &str) {
    const THRESHOLD: f64 = 1e-8;
    let mut rng = SplitMix64::new(seed);
    let mut accepted = 0usize;
    let mut attempts = 0usize;
    while accepted < 5 && attempts < 400 {
        attempts += 1;
        let mut m = Multivector::<P, Q, R>::zero();
        for i in 0..Multivector::<P, Q, R>::BASIS_COUNT {
            m.set(i, rng.next_signed());
        }
        let ratio = log_det_ratio(&m);
        // Accept only well-conditioned draws: |det|/max_abs^N >= 1e-8.
        if ratio < THRESHOLD.ln() {
            continue;
        }
        let x = m.inverse().unwrap_or_else(|| {
            panic!("{label}: invertible draw returned None (log ratio {ratio})")
        });
        assert_inverse_acceptable(&m, &x);
        accepted += 1;
    }
    assert!(
        accepted >= 5,
        "{label}: only {accepted} well-conditioned draws in {attempts} attempts"
    );
}

// ============================================================================
// Scalar / vector / versor / non-versor
// ============================================================================

#[test]
fn scalar_inverse_across_signatures() {
    let m = mv::<2, 0, 0>(&[(2.0, &[])]);
    assert_eq!(m.inverse().unwrap().as_slice()[0], 0.5);

    let m = mv::<2, 1, 0>(&[(2.0, &[])]);
    assert_eq!(m.inverse().unwrap().as_slice()[0], 0.5);

    let m = mv::<1, 1, 1>(&[(2.0, &[])]);
    assert_eq!(m.inverse().unwrap().as_slice()[0], 0.5);

    // Cl(1,0,1): the scalar route is unaffected by the null generator.
    let m = mv::<1, 0, 1>(&[(2.0, &[])]);
    assert_eq!(m.inverse().unwrap().as_slice()[0], 0.5);
}

#[test]
fn vector_inverse_is_self_when_square_one() {
    let m = mv::<2, 0, 0>(&[(1.0, &[1])]);
    let x = m.inverse().expect("unit vector invertible");
    assert_eq!(x.as_slice(), m.as_slice());
    assert_inverse_acceptable(&m, &x);

    let m = mv::<2, 1, 0>(&[(1.0, &[2])]);
    let x = m.inverse().expect("unit vector invertible");
    assert_eq!(x.as_slice(), m.as_slice());

    let m = mv::<1, 1, 1>(&[(1.0, &[1])]);
    let x = m.inverse().expect("unit vector invertible");
    assert_eq!(x.as_slice(), m.as_slice());
}

#[test]
fn versor_inverse_across_signatures() {
    // Bivector e12: reverse is -e12 and e12·rev(e12) = 1 (fast path).
    let m = mv::<2, 0, 0>(&[(1.0, &[1, 2])]);
    let x = m.inverse().expect("bivector versor invertible");
    assert_eq!(x.as_slice()[blade(&[1, 2])], -1.0);
    assert_inverse_acceptable(&m, &x);

    // A null-generator versor: (e1+e2)^2 = 1 in Cl(1,0,1) (e2 is null).
    let m = mv::<1, 0, 1>(&[(1.0, &[1]), (1.0, &[2])]);
    let x = m.inverse().expect("null-generator versor invertible");
    assert_inverse_acceptable(&m, &x);
    assert_eq!(x.as_slice()[blade(&[1])], 1.0);
    assert_eq!(x.as_slice()[blade(&[2])], 1.0);
}

#[test]
fn non_versor_inverse_across_signatures() {
    // 1+e1+e2 in Cl(2,0,0): M·rev(M) = 3+2e1+2e2, so the general route runs.
    let m = mv::<2, 0, 0>(&[(1.0, &[]), (1.0, &[1]), (1.0, &[2])]);
    let x = m.inverse().expect("non-versor invertible");
    assert_inverse_acceptable(&m, &x);

    // Cl(2,1,0): (1+e1+e2)(1-e1-e2) = -1 -> inverse is -(1-e1-e2).
    let m = mv::<2, 1, 0>(&[(1.0, &[]), (1.0, &[1]), (1.0, &[2])]);
    let x = m.inverse().expect("non-versor invertible");
    assert_inverse_acceptable(&m, &x);

    // Cl(1,1,1): e1+e2 is nilpotent, so (1+e1+e2)^{-1} = 1-e1-e2.
    let m = mv::<1, 1, 1>(&[(1.0, &[]), (1.0, &[1]), (1.0, &[2])]);
    let x = m.inverse().expect("non-versor invertible");
    assert_inverse_acceptable(&m, &x);
    assert_eq!(x.as_slice()[blade(&[1])], -1.0);
    assert_eq!(x.as_slice()[blade(&[2])], -1.0);
}

// ============================================================================
// Non-invertible elements
// ============================================================================

#[test]
fn null_generator_is_not_invertible() {
    // In Cl(1,0,1) the second generator e2 is the null one; it squares to zero.
    let m = mv::<1, 0, 1>(&[(1.0, &[2])]);
    assert!(m.inverse().is_none(), "null vector must not be invertible");

    // In Cl(2,0,1) the third generator e3 is null (the dispatch's `e3` case).
    let m = mv::<2, 0, 1>(&[(1.0, &[3])]);
    assert!(
        m.inverse().is_none(),
        "null vector e3 must not be invertible"
    );
}

#[test]
fn nilpotent_and_zero_divisors_return_none() {
    // Square-zero bivector e12+e13 in Cl(2,1,0).
    let m = mv::<2, 1, 0>(&[(1.0, &[1, 2]), (1.0, &[1, 3])]);
    assert!(m.inverse().is_none(), "nilpotent bivector must be None");

    // Zero divisor (1+e1)(1-e1) = 0 in Cl(2,0,0).
    let m = mv::<2, 0, 0>(&[(1.0, &[]), (1.0, &[1])]);
    assert!(m.inverse().is_none(), "zero divisor must be None");

    // The zero multivector.
    let z = Multivector::<2, 0, 0>::zero();
    assert!(z.inverse().is_none(), "zero must be None");
}

// ============================================================================
// Fast-path overflow guard (kept from the red-stage regression)
// ============================================================================

#[test]
fn scalar_overflow_scale_inverse() {
    let m = mv::<2, 0, 0>(&[(1e200, &[])]);
    let x = m
        .inverse()
        .expect("1e200 scalar is invertible; 1e-200 representable");
    assert_eq!(x.as_slice()[0], 1e-200);
    assert_inverse_acceptable(&m, &x);

    // A tiny scalar takes the fast path: P = 1e-16, inverse = 1e8.
    let m = mv::<2, 0, 0>(&[(1e-8, &[])]);
    let x = m.inverse().expect("small-scale scalar invertible");
    assert!((x.as_slice()[0] - 1e8).abs() <= 1e-6 * 1e8);
}

// ============================================================================
// Deterministic well-conditioned random draws, all four signatures
// ============================================================================

#[test]
fn well_conditioned_random_inverses_euclidean() {
    check_random_draws::<2, 0, 0>(0x5A17_0001, "Cl(2,0,0)");
}

#[test]
fn well_conditioned_random_inverses_mixed() {
    check_random_draws::<2, 1, 0>(0x5A17_0002, "Cl(2,1,0)");
}

#[test]
fn well_conditioned_random_inverses_degenerate_mixed() {
    check_random_draws::<1, 1, 1>(0x5A17_0003, "Cl(1,1,1)");
}

#[test]
fn well_conditioned_random_inverses_null_generator() {
    check_random_draws::<1, 0, 1>(0x5A17_0004, "Cl(1,0,1)");
}
