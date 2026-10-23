// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: Apache-2.0
//! CORE-W03 red-first regressions: general inverse and exponential domains.
//!
//! Contract: docs/plans/2026-10-10-amari-core-w03-inverse-exp-contract.md
//! (rounds 1–7 review loop; erratum 2026-10-12 disclosed in-contract).
//!
//! Items 1–3 are behavior-red against current develop (the `inverse()` API
//! exists but is wrong for these inputs). Items 4–17 are API-red: they use the
//! contract's `Result`-returning `exp()` and `ExpError`, which do not exist
//! yet — the red state is the compile failure. Item 8 is a contract pin
//! (green today via different internals; pins the `f64::exp` route and
//! exact `exp(0)`). Item 16 (bignum oracle vs num-bigint) is a unit test that
//! lands with the implementation module.

use amari_core::{ExpError, Multivector};

/// Blade index from generator bits: e12 -> 0b0011 = 3, e13 -> 5, e23 -> 6,
/// e34 -> 12, e1234 -> 15.
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
    for r in [m.geometric_product(x), x.geometric_product(m)] {
        let mut identity = Multivector::zero();
        identity.set(0, 1.0);
        let d = r + identity * -1.0;
        assert!(
            d.coefficient_norm() <= tol,
            "residual {} > {tol}",
            d.coefficient_norm()
        );
    }
}

// ============================================================================
// Items 1–3: inverse (behavior-red)
// ============================================================================

#[test]
fn non_versor_inverse_solves_both_orders() {
    // CORE-01: 1+e1+e2 in Cl(2,0,0); M·rev(M) has grade-1 and grade-2 parts,
    // so the scalar-part formula returns a wrong inverse (M/3).
    let m = mv::<2, 0, 0>(&[(1.0, &[]), (1.0, &[1]), (1.0, &[2])]);
    let x = m
        .inverse()
        .expect("non-versor invertible element has an inverse");
    assert_inverse_acceptable(&m, &x);
}

#[test]
fn small_scale_versor_inverse_not_rejected() {
    // The absolute 1e-14 threshold rejects the exact inverse of scalar 1e-8.
    let m = mv::<2, 0, 0>(&[(1e-8, &[])]);
    let x = m.inverse().expect("small-scale scalar is invertible");
    assert_inverse_acceptable(&m, &x);
}

#[test]
fn scalar_overflow_scale_inverse() {
    // Fast-path overflow guard: scalar 1e200 has computed lambda = inf;
    // the inverse 1e-200 is representable (LU route).
    let m = mv::<2, 0, 0>(&[(1e200, &[])]);
    let x = m
        .inverse()
        .expect("1e200 scalar is invertible; 1e-200 representable");
    assert_inverse_acceptable(&m, &x);
}

// ============================================================================
// Items 4–17: exponential (API-red against the contract API)
// ============================================================================

#[test]
fn nilpotent_bivector_exponential_is_exact() {
    // CORE-03: B = e12+e13 in Cl(2,1,0) has exact square 0; exp(B) = 1+B.
    let b = mv::<2, 1, 0>(&[(1.0, &[1, 2]), (1.0, &[1, 3])]);
    let e = b.exp().expect("exact nilpotent bivector");
    let expect = mv::<2, 1, 0>(&[(1.0, &[]), (1.0, &[1, 2]), (1.0, &[1, 3])]);
    assert_eq!(e.as_slice(), expect.as_slice());
}

#[test]
fn square_zero_vector_exponential_survives() {
    // CORE-05 dispatch defect: v = 1e-11(e1+e2) in Cl(1,1,0) is a square-zero
    // VECTOR (not a bivector); the absolute 1e-10 membership test currently
    // deletes it. True exp = 1 + v exactly (series route).
    let v = mv::<1, 1, 0>(&[(1e-11, &[1]), (1e-11, &[2])]);
    let e = v.exp().expect("square-zero vector");
    let expect = mv::<1, 1, 0>(&[(1.0, &[]), (1e-11, &[1]), (1e-11, &[2])]);
    let d = e + expect.clone() * -1.0;
    assert!(
        d.coefficient_norm() <= 1e-11 * 1e-6,
        "{}",
        d.coefficient_norm()
    );
}

#[test]
fn large_norm_scalar_vector_exponential_converges() {
    // CORE-06: exp(1+9e1) in Cl(2,0,0). Reference: e·(cosh 9 + sinh 9·e1).
    let m = mv::<2, 0, 0>(&[(1.0, &[]), (9.0, &[1])]);
    let e = m.exp().expect("scaling-and-squaring converges");
    let expect = mv::<2, 0, 0>(&[
        (9f64.cosh() * 1f64.exp(), &[]),
        (9f64.sinh() * 1f64.exp(), &[1]),
    ]);
    let d = e + expect.clone() * -1.0;
    assert!(d.coefficient_norm() <= 1e-6 * expect.coefficient_norm());
}

#[test]
fn non_simple_bivector_euclidean() {
    // CORE-04: e12+e34 in Cl(4,0,0) — B^2 = -2 + 2e1234.
    // Reference: (cos1+sin1 e12)(cos1+sin1 e34).
    let b = mv::<4, 0, 0>(&[(1.0, &[1, 2]), (1.0, &[3, 4])]);
    let e = b.exp().expect("non-simple bivector -> series");
    let c = 1f64.cos();
    let s = 1f64.sin();
    let expect = mv::<4, 0, 0>(&[
        (c * c, &[]),
        (c * s, &[1, 2]),
        (c * s, &[3, 4]),
        (s * s, &[1, 2, 3, 4]),
    ]);
    let d = e + expect.clone() * -1.0;
    assert!(d.coefficient_norm() <= 1e-9 * expect.coefficient_norm());
}

#[test]
fn non_simple_bivector_scalar_zero_grade4_nonzero() {
    // Item 15 / precedence pin: e12+e34 in Cl(3,1,0) — scalar square EXACTLY 0,
    // B^2 = 2e1234. Must route to series, never 1+B.
    // Reference: (cos1+sin1 e12)(cosh1+sinh1 e34).
    let b = mv::<3, 1, 0>(&[(1.0, &[1, 2]), (1.0, &[3, 4])]);
    let e = b.exp().expect("scalar-zero non-nilpotent -> series");
    let c = 1f64.cos();
    let s = 1f64.sin();
    let ch = 1f64.cosh();
    let sh = 1f64.sinh();
    let expect = mv::<3, 1, 0>(&[
        (c * ch, &[]),
        (s * ch, &[1, 2]),
        (c * sh, &[3, 4]),
        (s * sh, &[1, 2, 3, 4]),
    ]);
    let d = e + expect.clone() * -1.0;
    assert!(d.coefficient_norm() <= 1e-9 * expect.coefficient_norm());
}

#[test]
fn exp_zero_is_identity_and_scalar_uses_f64() {
    // Item 8 (contract pin): exp(0) = 1 exactly; scalars use f64::exp.
    let z = Multivector::<2, 0, 0>::zero();
    let e = z.exp().expect("exp(0)");
    let mut one = Multivector::<2, 0, 0>::zero();
    one.set(0, 1.0);
    assert_eq!(e.as_slice(), one.as_slice());

    let s = mv::<2, 0, 0>(&[(0.5, &[])]);
    let e = s.exp().expect("scalar exp");
    let expect = mv::<2, 0, 0>(&[(0.5f64.exp(), &[])]);
    assert_eq!(
        e.as_slice(),
        expect.as_slice(),
        "scalar route must be f64::exp"
    );
}

#[test]
fn huge_circular_single_blade_bounded() {
    // Item 9a: 1e200·e12 in Cl(2,0,0). Single blade: angle = |c| bit-for-bit;
    // bounded correct result via the signature-aware route.
    let b = mv::<2, 0, 0>(&[(1e200, &[1, 2])]);
    let e = b.exp().expect("bounded circular exponential");
    let a = 1e200f64;
    let expect = mv::<2, 0, 0>(&[(a.cos(), &[]), (a.sin(), &[1, 2])]);
    let d = e + expect.clone() * -1.0;
    assert!(d.coefficient_norm() <= 1e-15 * expect.coefficient_norm());
}

#[test]
fn hyperbolic_overflow_is_error() {
    // Item 9b: 1000·e12 in Cl(1,1,0) — cosh(1000) overflows: a true
    // representability failure, reported as NonFiniteResult.
    let b = mv::<1, 1, 0>(&[(1000.0, &[1, 2])]);
    match b.exp() {
        Err(ExpError::NonFiniteResult { .. }) => {}
        other => panic!("expected NonFiniteResult, got {other:?}"),
    }
}

#[test]
fn huge_exact_nilpotent_returns_exactly_one_plus_b() {
    // Item 9c: 1e200·(e12+e13) in Cl(2,1,0) — exact square 0; exp = 1+B
    // exactly, no series, no squaring.
    let b = mv::<2, 1, 0>(&[(1e200, &[1, 2]), (1e200, &[1, 3])]);
    let e = b.exp().expect("certified exact nilpotent");
    let expect = mv::<2, 1, 0>(&[(1.0, &[]), (1e200, &[1, 2]), (1e200, &[1, 3])]);
    assert_eq!(e.as_slice(), expect.as_slice());
}

#[test]
fn erratum_case_huge_simple_circular() {
    // Item 9d (erratum): 1e200·(e12+e13) in Cl(3,0,0) — exact scalar square
    // -2·2^400·(mantissa-adjusted), grade-4 zero: simple circular, closed form
    // Ok via the exact route. Pin by the Pythagorean identity (the extracted
    // angle is within 1 ulp of √2·1e200, which is beyond direct referencing).
    let b = mv::<3, 0, 0>(&[(1e200, &[1, 2]), (1e200, &[1, 3])]);
    let e = b.exp().expect("huge simple circular via exact route");
    let scalar = e.as_slice()[0];
    let c12 = e.as_slice()[blade(&[1, 2])];
    let c13 = e.as_slice()[blade(&[1, 3])];
    assert!(scalar.is_finite() && c12.is_finite() && c13.is_finite());
    assert!((c12 - c13).abs() <= 1e-184, "symmetry: {c12} vs {c13}");
    let s = std::f64::consts::SQRT_2 * c12; // sin(angle)/sqrt(2)
    assert!(
        (scalar * scalar + s * s - 1.0).abs() <= 1e-9,
        "cos²+sin² = 1 violated: {}",
        scalar * scalar + s * s
    );
}

#[test]
fn near_zero_circular_and_hyperbolic() {
    // Item 10: tiny nonzero square, both signs; sinh(x)/x and sin(x)/x safe.
    let b = mv::<2, 0, 0>(&[(1e-8, &[1, 2])]);
    let e = b.exp().expect("tiny circular");
    assert_eq!(e.as_slice()[0], 1.0); // cos(1e-8) rounds to 1
    assert!((e.as_slice()[blade(&[1, 2])] - 1e-8).abs() <= 1e-23);

    let b = mv::<1, 1, 0>(&[(1e-8, &[1, 2])]);
    let e = b.exp().expect("tiny hyperbolic");
    assert_eq!(e.as_slice()[0], 1.0); // cosh(1e-8) rounds to 1
    assert!((e.as_slice()[blade(&[1, 2])] - 1e-8).abs() <= 1e-23);
}

#[test]
fn mixed_signature_circular_plane() {
    // Item 11a: 2e12+e13 in Cl(2,1,0) — exact square -3, angle √3 (NOT the
    // coefficient norm √5). Extraction of the exact integer 3 is bit-exact.
    let b = mv::<2, 1, 0>(&[(2.0, &[1, 2]), (1.0, &[1, 3])]);
    let e = b.exp().expect("mixed-signature circular");
    let a = 3f64.sqrt();
    let f = a.sin() / a;
    let expect = mv::<2, 1, 0>(&[(a.cos(), &[]), (2.0 * f, &[1, 2]), (f, &[1, 3])]);
    let d = e + expect.clone() * -1.0;
    assert!(d.coefficient_norm() <= 1e-15 * expect.coefficient_norm());
}

#[test]
fn mixed_signature_hyperbolic_plane() {
    // Item 11b: 2e12+e13 in Cl(1,2,0) — exact square +5, angle √5.
    let b = mv::<1, 2, 0>(&[(2.0, &[1, 2]), (1.0, &[1, 3])]);
    let e = b.exp().expect("mixed-signature hyperbolic");
    let a = 5f64.sqrt();
    let f = a.sinh() / a;
    let expect = mv::<1, 2, 0>(&[(a.cosh(), &[]), (2.0 * f, &[1, 2]), (f, &[1, 3])]);
    let d = e + expect.clone() * -1.0;
    assert!(d.coefficient_norm() <= 1e-15 * expect.coefficient_norm());
}

#[test]
fn long_angle_general_multivector() {
    // Item 12: 0.1 + t·e12 in Cl(2,0,0). Reference
    // e^0.1·(cos t + sin t·e12). Round 2 moved this class to the
    // scalar-factored closed form (`y = t·e12` has the exact scalar square
    // `-t²`), which over-performs the frozen condition-aware bound; the loose
    // ≤0.5 contract assertion is intentionally kept here, and the tight
    // achieved accuracy is pinned by `round2_scalar_factored_closed_form_long_angle`.
    for t in [1e4, 1e8, 1e12] {
        let m = mv::<2, 0, 0>(&[(0.1, &[]), (t, &[1, 2])]);
        let e = m.exp().unwrap_or_else(|err| panic!("t={t}: {err:?}"));
        let scale = 0.1f64.exp();
        let expect = mv::<2, 0, 0>(&[(scale * t.cos(), &[]), (scale * t.sin(), &[1, 2])]);
        let d = e + expect.clone() * -1.0;
        let rel = d.coefficient_norm() / expect.coefficient_norm();
        assert!(rel <= 0.5, "t={t}: relative error {rel}");
    }
}

#[test]
fn degenerate_nilpotent_blade() {
    // Item 13a: e12 in Cl(1,0,1) — null generator e3 gives eta(e12) = ... wait:
    // e1 positive, e2 null -> e12^2 = -(+1)(0) = 0. exp = 1+e12 exactly.
    let b = mv::<1, 0, 1>(&[(1.0, &[1, 2])]);
    let e = b.exp().expect("null-blade nilpotent");
    let expect = mv::<1, 0, 1>(&[(1.0, &[]), (1.0, &[1, 2])]);
    assert_eq!(e.as_slice(), expect.as_slice());
}

#[test]
fn degenerate_mixed_hyperbolic() {
    // Item 13b: e12+e13 in Cl(1,1,1) — eta(e12) = +1, eta(e13) = 0 (e3 null):
    // exact square 1, hyperbolic angle 1.
    let b = mv::<1, 1, 1>(&[(1.0, &[1, 2]), (1.0, &[1, 3])]);
    let e = b.exp().expect("degenerate mixed hyperbolic");
    let expect = mv::<1, 1, 1>(&[
        (1f64.cosh(), &[]),
        (1f64.sinh(), &[1, 2]),
        (1f64.sinh(), &[1, 3]),
    ]);
    let d = e + expect.clone() * -1.0;
    assert!(d.coefficient_norm() <= 1e-15 * expect.coefficient_norm());
}

#[test]
fn rounded_false_zero_routes_by_exact_square() {
    // Item 14a: 1e8·e12 + e13 + 1e8·e23 in Cl(1,2,0) — exact square 1
    // (plain f64 of the normalized terms gives a false zero).
    let b = mv::<1, 2, 0>(&[(1e8, &[1, 2]), (1.0, &[1, 3]), (1e8, &[2, 3])]);
    let e = b.exp().expect("exact square 1 routes hyperbolic");
    let expect = mv::<1, 2, 0>(&[
        (1f64.cosh(), &[]),
        (1e8 * 1f64.sinh(), &[1, 2]),
        (1f64.sinh(), &[1, 3]),
        (1e8 * 1f64.sinh(), &[2, 3]),
    ]);
    let d = e + expect.clone() * -1.0;
    assert!(d.coefficient_norm() <= 1e-15 * expect.coefficient_norm());
}

#[test]
fn crafted_dyadic_false_zero() {
    // Item 14b (round-4 crafted): a=2^22 on e12, b=round(sqrt(1023))·2^22 on
    // e13, c=2^27 on e23 in Cl(1,2,0). Plain normalized f64 terms sum to
    // exactly 0; the exact square is 962307637273/2^40.
    let s = (1u64 << 22) as f64;
    let a = s;
    let b = 1023f64.sqrt() * s;
    let c = (32u64 << 22) as f64;
    assert_eq!(
        a * a + b * b - c * c,
        0.0,
        "precondition: plain f64 false zero"
    );

    let m = mv::<1, 2, 0>(&[(a, &[1, 2]), (b, &[1, 3]), (c, &[2, 3])]);
    let e = m.exp().expect("exact square is nonzero: hyperbolic");
    // Reference from exact rational arithmetic (python fractions):
    const ANGLE: f64 = f64::from_bits(0x3FEDEFD99A5D8E5B); // 0x1.defd99a5d8e5bp-1 (stage-1 review corrected the discriminant)
    const COSH: f64 = f64::from_bits(0x3FF7870AD0A00175);
    const SINH_OVER: f64 = f64::from_bits(0x3FF2702D452BDAAE);
    let expect = mv::<1, 2, 0>(&[
        (COSH, &[]),
        (SINH_OVER * a, &[1, 2]),
        (SINH_OVER * b, &[1, 3]),
        (SINH_OVER * c, &[2, 3]),
    ]);
    let d = e.clone() + expect.clone() * -1.0;
    // The extracted angle is within 1 ulp of the exact sqrt(q); the b term of
    // the reference carries the same 1-ulp angle uncertainty amplified by
    // sinh'/sinh ~ 1, so allow a relative tolerance of a few ulp.
    assert!(
        d.coefficient_norm() <= 1e-14 * expect.coefficient_norm(),
        "{} vs {}",
        d.coefficient_norm(),
        expect.coefficient_norm()
    );
    assert_ne!(
        e.as_slice()[0],
        1.0,
        "must NOT route to the nilpotent branch"
    );
    let _ = ANGLE;
}

#[test]
fn subnormal_square_extraction() {
    // Item 17a: 2^-1074·e12 in Cl(2,0,0) — exact square -2^-2148; the angle
    // 2^-1074 (the minimum subnormal) must survive exponent composition.
    let c = f64::from_bits(1);
    let b = mv::<2, 0, 0>(&[(c, &[1, 2])]);
    let e = b.exp().expect("subnormal circular");
    assert_eq!(e.as_slice()[0], 1.0);
    assert_eq!(
        e.as_slice()[blade(&[1, 2])],
        c,
        "angle survives as min subnormal"
    );
}

// ============================================================================
// Round-1 review remediation regressions (P1-1, P1-2, P2, detector)
// ============================================================================

#[test]
fn round1_square_zero_vector_at_scale_is_exact() {
    // P1-1: exp(1e9·(e1+e2)) in Cl(1,1,0). `(e1+e2)^2 = 0` is an exact scalar
    // square, so the generalized scalar-square closed form returns 1 + v with
    // every coefficient untouched. The old series route scaled and squared and
    // silently returned all-zero.
    let v = mv::<1, 1, 0>(&[(1e9, &[1]), (1e9, &[2])]);
    let e = v.exp().expect("square-zero vector at scale");
    let expect = mv::<1, 1, 0>(&[(1.0, &[]), (1e9, &[1]), (1e9, &[2])]);
    assert_eq!(e.as_slice(), expect.as_slice());
}

#[test]
fn round1_circular_vector_at_scale_bit_exact() {
    // P1-1 companion: 1e9·e1 in Cl(0,1,0) is a single-blade vector with
    // eta = -1, so the exact square is -1e18 and the single-blade short-circuit
    // gives the angle |c| = 1e9 bit-for-bit. exp = cos(1e9) + sin(1e9)·e1.
    let v = mv::<0, 1, 0>(&[(1e9, &[1])]);
    let e = v.exp().expect("circular vector at scale");
    assert_eq!(e.as_slice()[0].to_bits(), 1e9f64.cos().to_bits());
    assert_eq!(e.as_slice()[1].to_bits(), 1e9f64.sin().to_bits());
}

#[test]
fn round1_disjoint_null_pairs_scalar_square_is_exact() {
    // The literal round-1 remediation input `1e9·(e1+e2) + 1e9·(e3+e4)` in
    // Cl(2,2,0) is a VECTOR, so its exact square is the scalar
    // `1e18·(1+1-1-1) = 0`. The correct behavior of the generalized
    // scalar-square route is therefore `Ok(1 + v)` exactly.
    //
    // Plan-defect deviation: the dispatch enumerated this input as a
    // squaring-detector error, but a vector's square is always scalar, so the
    // scalar-square class (precedence 3) must consume it. Asserting the exact
    // `Ok(1+v)` is the faithful, stronger version; the detector itself is
    // exercised by the test below on a non-scalar-square input.
    let v = mv::<2, 2, 0>(&[(1e9, &[1]), (1e9, &[2]), (1e9, &[3]), (1e9, &[4])]);
    let e = v.exp().expect("null vector is a scalar-square input");
    let expect = mv::<2, 2, 0>(&[
        (1.0, &[]),
        (1e9, &[1]),
        (1e9, &[2]),
        (1e9, &[3]),
        (1e9, &[4]),
    ]);
    assert_eq!(e.as_slice(), expect.as_slice());
}

#[test]
fn round1_squaring_cancellation_detector_fires() {
    // Detector: two disjoint null vectors (1e9·(e1+e3) and 1e9·(e2+e4), whose
    // sum is the null vector 1e9·(e1+e2+e3+e4)) plus a circular bivector
    // 1e9·e34 in Cl(2,2,0). The input is not scalar-square, so it reaches the
    // series. The true exponential is representable (the nilpotent part grows
    // linearly and the bivector part is circular), but repeated squaring
    // cancels the scalar channel; the detector converts that silent collapse
    // into an honest non-finite-stage error.
    let m = mv::<2, 2, 0>(&[
        (1e9, &[1]),
        (1e9, &[2]),
        (1e9, &[3]),
        (1e9, &[4]),
        (1e9, &[3, 4]),
    ]);
    match m.exp() {
        Err(ExpError::NonFiniteResult {
            stage: amari_core::NonFiniteStage::Squaring,
        }) => {}
        other => panic!("expected NonFiniteResult{{Squaring}}, got {other:?}"),
    }
}

#[test]
fn round1_inverse_fast_path_rejects_non_finite_quotient() {
    // P1-2: M·rev(M) = 1e-300 is a computed finite nonzero scalar, so the old
    // fast path returned rev/λ with an infinite e12 coefficient (1e200/1e-300
    // = 1e500 = inf). The true inverse is unrepresentable -> None.
    let m = mv::<1, 0, 1>(&[(1e-150, &[]), (1e200, &[1, 2])]);
    assert!(
        m.inverse().is_none(),
        "non-finite fast-path quotient must be rejected"
    );
}

#[test]
fn round1_scaling_boundary_norm_errors_honestly() {
    // P2 (behavioral): the reviewer's next-above-power-of-two norm as the
    // dominant coefficient of a general-route input (scalar + vector, whose
    // exact square carries a grade-1 part, so it routes to the series).
    // cosh of this scale overflows, so the honest outcome is a non-finite
    // error, never a silent truncation. Minimality and the `<= 0.5` target of
    // the corrected `k` are unit-tested in `amari_core::tests`
    // (`series_scaling_meets_frozen_target_for_boundary_norm`).
    let norm = f64::from_bits((1123u64 << 52) + 1);
    let m = mv::<2, 0, 0>(&[(1.0, &[]), (norm, &[1])]);
    match m.exp() {
        Err(ExpError::NonFiniteResult { .. }) => {}
        Ok(x) => assert!(x.as_slice().iter().all(|c| c.is_finite())),
        other => panic!("unexpected {other:?}"),
    }
}

// ============================================================================
// Round-2 review remediation regressions: scalar factoring, exact any-index
// nilpotency, overflow-safe detector.
//
// The overflow-safe detector-threshold check is a unit test of the private
// `squaring_is_collapse` helper and lives inline in `amari-core/src/lib.rs`
// (`squaring_detector_threshold_is_overflow_safe`); an integration test cannot
// reach that private helper.
// ============================================================================

/// Independent reference exponential of a nilpotent element: the truncated
/// `f64` Taylor sum built by repeated geometric multiplication (never the
/// production exact machinery).
fn truncated_exp<const P: usize, const Q: usize, const R: usize>(
    x: &Multivector<P, Q, R>,
    terms: usize,
) -> Multivector<P, Q, R> {
    let mut result = Multivector::scalar(1.0);
    let mut term = Multivector::scalar(1.0);
    for j in 1..=terms {
        term = term.geometric_product(x) / (j as f64);
        result = result + term.clone();
    }
    result
}

#[test]
fn round2_scalar_plus_square_zero_is_exact() {
    // P1: `0.1 + 1e8·(e1+e2)` in Cl(1,1,0). The non-scalar part `y` is a
    // square-zero vector, so `exp = e^0.1·(1 + y)`. Scalar factoring keeps the
    // scalar channel exact instead of letting the series squaring cancel it.
    for scale in [1e8, 1e20] {
        let x = mv::<1, 1, 0>(&[(0.1, &[]), (scale, &[1]), (scale, &[2])]);
        let e = x.exp().expect("scalar + square-zero factor");
        let f = 0.1f64.exp();
        let expect = mv::<1, 1, 0>(&[(f, &[]), (f * scale, &[1]), (f * scale, &[2])]);
        let d = e + expect.clone() * -1.0;
        let rel = d.coefficient_norm() / expect.coefficient_norm();
        assert!(rel <= 1e-15, "scale={scale}: relative error {rel}");
    }
}

#[test]
fn round2_nilpotent_index_three_exact_at_scale() {
    // P2: `1e10·(e1+e23)` in Cl(0,0,3) is nilpotent of index 3:
    // `x² = 2e20·e123`, `x³ = 0`, so `exp = 1 + x + x²/2` exactly.
    let x = mv::<0, 0, 3>(&[(1e10, &[1]), (1e10, &[2, 3])]);
    let e = x.exp().expect("certified index-3 nilpotent");
    let expect = [1.0, 1e10, 0.0, 0.0, 0.0, 0.0, 1e10, 1e20];
    assert_eq!(e.as_slice(), expect.as_slice());

    // The same class at 1e80 scale (where `y² = 2e160·e123` is still
    // representable, while the old squaring-detector threshold `prev·prev`
    // overflows): the exact nilpotent certificate keeps it exact.
    let x = mv::<0, 0, 3>(&[(1e80, &[1]), (1e80, &[2, 3])]);
    let e = x.exp().expect("certified index-3 nilpotent at 1e80");
    let expect = [1.0, 1e80, 0.0, 0.0, 0.0, 0.0, 1e80, 1e160];
    assert_eq!(e.as_slice(), expect.as_slice());
}

#[test]
fn round2_nilpotent_index_four_polynomial() {
    // Plan defect deviation: the plan asked for an index-4 case in Cl(0,0,4),
    // but that signature has no nilpotency index above 3 (verified
    // exhaustively over all 0/1 elements and by random search over integer
    // coefficients). The faithful closest construction is the index-4 element
    // `x = e1 + e23 + e45` in Cl(0,0,5): `x² = 2(e123+e145+e2345)`,
    // `x³ = 6e12345`, `x⁴ = 0`.
    let x = mv::<0, 0, 5>(&[(1.0, &[1]), (1.0, &[2, 3]), (1.0, &[4, 5])]);
    let e = x.exp().expect("certified index-4 nilpotent");
    let reference = truncated_exp(&x, 4);
    let d = e.clone() + reference.clone() * -1.0;
    assert!(
        d.coefficient_norm() <= 1e-15 * reference.coefficient_norm(),
        "index-4 polynomial mismatch: {}",
        d.coefficient_norm()
    );
    // Pin the exact coefficients: 1 + x + x²/2 + x³/6.
    assert_eq!(e.get(0), 1.0);
    assert_eq!(e.get(blade(&[1])), 1.0);
    assert_eq!(e.get(blade(&[2, 3])), 1.0);
    assert_eq!(e.get(blade(&[4, 5])), 1.0);
    assert_eq!(e.get(blade(&[1, 2, 3])), 1.0);
    assert_eq!(e.get(blade(&[1, 4, 5])), 1.0);
    assert_eq!(e.get(blade(&[2, 3, 4, 5])), 1.0);
    assert_eq!(e.get(blade(&[1, 2, 3, 4, 5])), 1.0);
}

#[test]
fn round2_scalar_factored_closed_form_long_angle() {
    // Scalar-factored closed form: `0.1 + t·e12` in Cl(2,0,0). `y = t·e12`
    // has the exact scalar square `-t²`, so the whole class is closed form via
    // `y` plus scalar factoring; the long-angle series route is superseded and
    // the reference `e^0.1·(cos t + sin t·e12)` is matched to ~1 ulp.
    for t in [1e4, 1e8, 1e12] {
        let x = mv::<2, 0, 0>(&[(0.1, &[]), (t, &[1, 2])]);
        let e = x.exp().unwrap_or_else(|err| panic!("t={t}: {err:?}"));
        let f = 0.1f64.exp();
        let expect = mv::<2, 0, 0>(&[(f * t.cos(), &[]), (f * t.sin(), &[1, 2])]);
        let d = e + expect.clone() * -1.0;
        let rel = d.coefficient_norm() / expect.coefficient_norm();
        assert!(rel <= 1e-15, "t={t}: relative error {rel}");
    }
}

// ============================================================================
// Round-3 review remediation regressions: symbolic scalar scale, exact
// common-denominator polynomial, certification/evaluation separation, and
// the index-8 nilpotency bound.
// ============================================================================

#[test]
fn round3_scalar_factoring_rescues_underflowed_scale() {
    // P1-1: x = -750 + 1e300·(e1+e2) in Cl(1,1,0). The non-scalar part is a
    // square-zero vector (so exp(y) = 1 + y exactly), and the true result is
    // e^-750·[1, 1e300, 1e300, 0]. `f64::exp(-750)` underflows to 0, but the
    // vector coefficients 1.9016849634750064e-26 (== e^-750·1e300) are
    // representable and must survive the symbolic scalar-exponent scaling.
    let x = mv::<1, 1, 0>(&[(-750.0, &[]), (1e300, &[1]), (1e300, &[2])]);
    let e = x
        .exp()
        .expect("symbolic scalar scaling rescues the coefficients");
    // Scalar coefficient e^-750 is below the minimum subnormal: rounds to 0.
    assert_eq!(e.as_slice()[0], 0.0);
    let expected = f64::from_bits(0x3A97_8AAE_2278_9158); // 1.9016849634750064e-26
    for part in [e.as_slice()[1], e.as_slice()[2]] {
        assert!(
            (part.to_bits() as i64 - expected.to_bits() as i64).unsigned_abs() <= 1,
            "coefficient {part} not within 1 ulp of {expected}"
        );
    }
}

#[test]
fn round3_scalar_factoring_overflow_is_honest_error() {
    // P1-1 overflow side: x = +750 + 1e-300·(e1+e2). The naive factor
    // `f64::exp(750)` is +inf, and the symbolic scale keeps the vector
    // coefficients 5.258494541454804e25 representable — but the *scalar*
    // coefficient of the result is e^750 itself (~5.25e325), which exceeds
    // f64::MAX. The honest outcome is a `ScalarRoute` non-finite error, never a
    // silent wrong value. (A fully finite `Ok` is mathematically impossible
    // here: exp(y)'s scalar part is exactly 1.)
    let x = mv::<1, 1, 0>(&[(750.0, &[]), (1e-300, &[1]), (1e-300, &[2])]);
    match x.exp() {
        Err(ExpError::NonFiniteResult {
            stage: amari_core::NonFiniteStage::ScalarRoute,
        }) => {}
        other => panic!("expected ScalarRoute non-finite error, got {other:?}"),
    }
}

#[test]
fn round3_nilpotent_near_cancellation_is_exact() {
    // P1-2: a = nextafter(1e10); x = a·e1 + a·e23 - a²·e123 in Cl(0,0,3) is
    // nilpotent of index 3. The e123 coefficient of exp is the exact residual
    // x[7] + a² = 1478560745259009/274877906944 = 5378.972656250004, which the
    // old per-term f64 conversion cancelled to 0. The common-denominator exact
    // accumulator must preserve it and round once.
    let a = f64::from_bits(1e10f64.to_bits() + 1);
    assert_eq!(a, 10_000_000_000.000_002);
    let x = mv::<0, 0, 3>(&[(a, &[1]), (a, &[2, 3]), (-(a * a), &[1, 2, 3])]);
    let e = x.exp().expect("certified index-3 nilpotent");
    let expected = f64::from_bits(0x40B5_02F9_0000_0004); // 5378.972656250004
    let got = e.get(blade(&[1, 2, 3]));
    let rel = (got - expected).abs() / expected.abs();
    assert!(
        rel <= 1e-12,
        "e123 = {got}, expected {expected} (rel {rel})"
    );
    assert_ne!(got, 0.0, "near-cancellation residual must not vanish");
}

#[test]
fn round3_nilpotent_single_final_rounding_after_division() {
    // P2a: x = 1e154·(e1+e23) in Cl(0,0,3) is a certified index-3 nilpotent
    // with exp = 1 + x + x²/2 and e123 coefficient x²[7]/2 = 1e308. The raw x²
    // coefficient 2e308 overflows f64, so an implementation that converts each
    // power before dividing reports Err; the exact accumulator divides by the
    // common denominator first, so the final coefficient 1e308 is exact and Ok.
    let x = mv::<0, 0, 3>(&[(1e154, &[1]), (1e154, &[2, 3])]);
    let e = x.exp().expect("final coefficient 1e308 is representable");
    assert_eq!(e.get(blade(&[1, 2, 3])), 1e308);
    assert_eq!(e.get(0), 1.0);
    assert_eq!(e.get(blade(&[1])), 1e154);
    assert_eq!(e.get(blade(&[2, 3])), 1e154);
}

#[test]
fn round3_uncertified_input_never_reports_closed_form() {
    // P2b: x = 0.1 + 1e80·(e12+e3) in Cl(2,0,1). The non-scalar part is NOT
    // nilpotent (exp = e^0.1·(cos t + sin t·e12)·(1 + t·e3), finite), so the
    // certification search reaches the power cap with no zero certificate. The
    // round-3 fix requires this to fall through to the residual (series +
    // detector) route; it must never surface the certification path's old
    // `Err(NonFiniteResult {{ stage: ClosedForm }})`.
    //
    // Route served: the bounded scaling-and-squaring series. With the frozen
    // series rules the scaled-domain truncation error (~2^-36) is amplified by
    // 2^k (k ≈ 268) during squaring, the result collapses, and the round-1
    // squaring-cancellation detector fires — an honest evaluation-path error
    // (`Squaring`), never the certification error. The faithful regression here
    // pins the *stage separation* (never ClosedForm); if the series ever
    // returns `Ok`, it is checked against the factorization reference within
    // the contract's condition-aware tolerance.
    let x = mv::<2, 0, 1>(&[(0.1, &[]), (1e80, &[1, 2]), (1e80, &[3])]);
    match x.exp() {
        Err(ExpError::NonFiniteResult {
            stage: amari_core::NonFiniteStage::ClosedForm,
        }) => panic!("uncertified input must not report the certification stage"),
        Err(_) => {} // honest evaluation-path error (series/squaring detector)
        Ok(e) => {
            let t = 1e80f64;
            let f = 0.1f64.exp();
            let expect = mv::<2, 0, 1>(&[
                (f * t.cos(), &[]),
                (f * t.sin(), &[1, 2]),
                (f * t * t.cos(), &[3]),
                (f * t * t.sin(), &[1, 2, 3]),
            ]);
            let d = e + expect.clone() * -1.0;
            // Condition-aware bound: max(1e-6, C·S·u), C = 2e4, S = 1e80.
            let bound = 2e4 * 1e80 * f64::EPSILON;
            let rel = d.coefficient_norm() / expect.coefficient_norm().max(1.0);
            assert!(rel <= bound, "relative error {rel} exceeds {bound}");
        }
    }
}

#[test]
fn round3_index_eight_nilpotent_polynomial() {
    // P3: the old "index <= 7 for dim <= 6" comment was false. In Cl(3,3,0)
    // the element below has x^7 != 0 and x^8 = 0 (the pinned worst observed
    // case for dim = 6); the exact nilpotent route must return sum_{j<8} x^j/j!.
    let mut x = Multivector::<3, 3, 0>::zero();
    for (index, coefficient) in [
        (1usize, 0.5f64),
        (3, 0.25),
        (7, 0.125),
        (8, -0.5),
        (10, -0.25),
        (14, 0.125),
        (17, -0.25),
        (21, -0.125),
        (24, -0.25),
        (28, 0.125),
        (35, -0.125),
        (42, 0.125),
        (49, -0.125),
        (56, -0.125),
    ] {
        x.set(index, coefficient);
    }

    // Pin the nilpotency index: x^7 != 0, x^8 = 0.
    let mut power = x.clone();
    for _ in 1..7 {
        assert!(!power.is_zero(), "unexpected zero power before x^7");
        power = power.geometric_product(&x);
    }
    assert!(!power.is_zero(), "x^7 must be nonzero");
    let eighth = power.geometric_product(&x);
    assert!(eighth.is_zero(), "x^8 must be zero");

    // Exact machinery vs an independent f64 repeated-multiplication Taylor sum.
    let e = x.exp().expect("certified index-8 nilpotent");
    let reference = truncated_exp(&x, 8);
    let d = e.clone() + reference.clone() * -1.0;
    assert!(
        d.coefficient_norm() <= 1e-12 * reference.coefficient_norm(),
        "index-8 polynomial mismatch: {} vs {}",
        d.coefficient_norm(),
        reference.coefficient_norm()
    );
    // The scalar channel of the exact polynomial is cosh/sinh-like and starts
    // at 1; a wrong route (series collapse) would erase it to 0.
    assert_ne!(e.get(0), 0.0);
}

// ============================================================================
// Round-4 review remediation regressions: the unified symbolic-scalar
// composition — one correctly-rounded mantissa-times-exponent product at the
// destination precision — shared by the closed forms and the exact-polynomial
// finalization. The former `scale_by_pow2_rne(c * scale_mantissa, E)` formed
// the mantissa product in `f64` first and both overflowed before the exponent
// could rescue it and double-rounded at the subnormal boundary.
// ============================================================================

#[test]
fn round4_symbolic_scale_overflow_composition_is_finite() {
    // P1-1a: x = -0.5 + f64::MAX·(e1+e2) in Cl(1,1,0). `y` is square-zero, so
    // exp(x) = e^-0.5·(1 + y). Forming `f64::MAX · mantissa` in f64 overflows
    // to +inf before the -1 exponent rescues it; the unified composition must
    // return the finite scalar 0.6065306597126334 and vector coefficients
    // 1.0903560030489124e308.
    let x = mv::<1, 1, 0>(&[(-0.5, &[]), (f64::MAX, &[1]), (f64::MAX, &[2])]);
    let e = x.exp().expect("finite result");
    assert_eq!(e.get(0), 0.6065306597126334);
    let expected = 1.0903560030489124e308;
    assert!(e.get(1).is_finite());
    assert_eq!(e.get(1), expected);
    assert_eq!(e.get(2), expected);
}

#[test]
fn round4_symbolic_scale_subnormal_order_is_correct() {
    // P1-1b: x = 0.4 + 2^-1074·e1 in Cl(0,0,1). `y` is square-zero, so
    // exp = e^0.4·(1 + 2^-1074 e1). Rounding the mantissa product to the
    // subnormal grid before the +1 exponent shift double-rounds and yields
    // from_bits(2); the single destination-precision rounding must yield
    // from_bits(1).
    let x = mv::<0, 0, 1>(&[(0.4, &[]), (f64::from_bits(1), &[1])]);
    let e = x.exp().expect("finite subnormal result");
    assert_eq!(e.get(1).to_bits(), f64::from_bits(1).to_bits());
}

#[test]
fn round4_hyperbolic_closed_form_carries_symbolic_scale() {
    // P1-2a: x = -750 + 750·e1 in Cl(1,0,0). `y = 750 e1` has the exact scalar
    // square 750^2 > 0, but cosh(750)/sinh(750) overflow f64 while
    // e^-750·cosh(750) = (1 + e^-1500)/2 is finite. Evaluating the closed form
    // on the symbolic exponential terms must return [0.5, 0.5].
    let x = mv::<1, 0, 0>(&[(-750.0, &[]), (750.0, &[1])]);
    let e = x.exp().expect("finite hyperbolic result");
    assert_eq!(e.get(0), 0.5);
    assert_eq!(e.get(1), 0.5);
}

#[test]
fn round4_nilpotent_finalization_carries_symbolic_scale() {
    // P1-2b: x = -750 + 1e200·(e1+e23) in Cl(0,0,3) is a certified index-3
    // nilpotent (y^2 = 2e400 e123, y^3 = 0). The unscaled e123 coefficient
    // overflows f64; composing e^-750 into the single final rounding gives the
    // finite scalar 0, e1/e23 = 1.9016849634750064e-126 and
    // e123 = 1.9016849634750064e74 (the symbolic mantissa is within 1 ulp of
    // true e^-750, so the e123 pin admits 1 ulp).
    let x = mv::<0, 0, 3>(&[(-750.0, &[]), (1e200, &[1]), (1e200, &[2, 3])]);
    let e = x.exp().expect("finite nilpotent result");
    assert_eq!(e.get(0), 0.0);
    assert_eq!(e.get(blade(&[1])), 1.9016849634750064e-126);
    assert_eq!(e.get(blade(&[2, 3])), 1.9016849634750064e-126);
    let expected = 1.9016849634750064e74_f64;
    let got = e.get(blade(&[1, 2, 3]));
    assert!(
        (got.to_bits() as i64 - expected.to_bits() as i64).unsigned_abs() <= 1,
        "e123 = {got}, expected within 1 ulp of {expected}"
    );
}

#[test]
fn round4_subnormal_division_rounds_once_at_destination() {
    // P1-3: Cl(0,0,3) x[1] = 2^-1074, x[6] = from_bits(1.5.to_bits()+1),
    // x[7] = 2^-1074. The exact e123 coefficient is
    // (5/2 + 2^-52)·2^-1074, which is strictly above the subnormal midpoint;
    // normalized-then-subnormal double rounding yields from_bits(2), while the
    // single destination-precision rounding with the division remainder as
    // sticky yields from_bits(3).
    let mut x = Multivector::<0, 0, 3>::zero();
    x.set(1, f64::from_bits(1));
    x.set(6, f64::from_bits(1.5f64.to_bits() + 1));
    x.set(7, f64::from_bits(1));
    let e = x.exp().expect("finite subnormal finalization");
    assert_eq!(
        e.get(blade(&[1, 2, 3])).to_bits(),
        f64::from_bits(3).to_bits(),
        "exact e123 must round to from_bits(3)"
    );
}

// ============================================================================
// Round-5 review remediation regressions: call-site leaks of the validated
// symbolic pipeline. The reviewer's 20,000 exact-Fraction comparisons and
// 3,000 public subnormal nilpotent cases found zero mismatches in the pipeline
// primitives; these regressions pin the call sites that still materialized a
// coefficient times a closed-form factor (or a coefficient times a symbolic
// ratio) in f64 before the single destination rounding, plus the polynomial
// scale-overflow stage classification.
// ============================================================================

#[test]
fn round5_hyperbolic_factor_scaling_is_symbolic() {
    // P1-1a: Cl(2,1,0) x = -1 + e1 + f64::MAX·(e2+e3). `y` has the exact
    // nonscalar square +1 (hyperbolic, angle 1). The old path formed
    // `f64::MAX · sinhc(1)` in f64 (`> f64::MAX`) and reported Err before
    // e^-1 could rescue the finite product; the three-way symbolic multiply
    // must return e2/e3 = 7.77200912641605e307.
    let x = mv::<2, 1, 0>(&[(-1.0, &[]), (1.0, &[1]), (f64::MAX, &[2]), (f64::MAX, &[3])]);
    let e = x.exp().expect("finite hyperbolic result");
    let expected = f64::from_bits(0x7FDB_AB55_5710_1F8C); // 7.77200912641605e307
    assert_eq!(e.get(blade(&[2])), expected);
    assert_eq!(e.get(blade(&[3])), expected);
}

#[test]
fn round5_circular_factor_scaling_is_symbolic() {
    // P1-1b: Cl(1,2,0) x = 700 + 1e-300·e1 + 1e100·e2 + 1e-300·e3. `y` has
    // the exact scalar square -(1e100)^2 (circular, angle 1e100). The old path
    // formed `1e-300 · sinc` in f64 (underflow to 0) before e^700 could rescue
    // it; the three-way symbolic multiply must return e1/e3 =
    // -3.8605498802690016e-97.
    let x = mv::<1, 2, 0>(&[(700.0, &[]), (1e-300, &[1]), (1e100, &[2]), (1e-300, &[3])]);
    let e = x.exp().expect("finite circular result");
    let expected = f64::from_bits(0xABEA_6331_425E_2D5B); // -3.8605498802690016e-97
    assert_eq!(e.get(blade(&[1])), expected);
    assert_eq!(e.get(blade(&[3])), expected);
}

#[test]
fn round5_hyperbolic_symbolic_fallback_never_forms_two_a() {
    // P1-2a: Cl(1,0,0) x = -f64::MAX + f64::MAX·e1. cosh/sinh overflow, so the
    // symbolic fallback runs; the old `2.0 * angle` overflowed to +inf and the
    // vector coefficient collapsed to 0. The exact e^{s+a}/2 value is 0.5 on
    // both channels.
    let x = mv::<1, 0, 0>(&[(-f64::MAX, &[]), (f64::MAX, &[1])]);
    let e = x.exp().expect("finite hyperbolic symbolic result");
    assert_eq!(e.get(0), 0.5);
    assert_eq!(e.get(1), 0.5);
}

#[test]
fn round5_hyperbolic_symbolic_vector_matches_scalar_bits() {
    // P1-2b: s = -5926.141552874991, a = 5214.592735436862; x = s + a·e1 in
    // Cl(1,0,0). cosh/sinh overflow, and the true scalar/vector channels are
    // both e^{s+a}/2 = 4.75603438903267e-310 (from_bits 96263207714727). The
    // old `a · (m/(2a))` path double-rounded the vector to from_bits
    // 96263207714728; the exact 1/(2a) rational finalization must reproduce the
    // scalar bits.
    let s = -5926.141552874991_f64;
    let a = 5214.592735436862_f64;
    let x = mv::<1, 0, 0>(&[(s, &[]), (a, &[1])]);
    let e = x.exp().expect("finite hyperbolic symbolic result");
    // The test's contract is CHANNEL EQUALITY: the exact 1/(2a) rational
    // finalization must reproduce the scalar channel's bits in the vector
    // channel. The absolute constant documents the glibc-backed bits
    // (std feature); the P-EMBED dispatch routes std-off builds through
    // libm, which may differ by 1 ULP in the last place — so the absolute
    // check allows that seam while channel equality stays exact.
    let scalar = e.get(0).to_bits();
    let vector = e.get(1).to_bits();
    assert_eq!(vector, scalar, "vector channel must match scalar bits");
    let documented = 96263207714727_u64;
    assert!(
        scalar.abs_diff(documented) <= 1,
        "scalar channel bit distance > 1 from the documented bits: {scalar} vs {documented}"
    );
}

#[test]
fn round5_polynomial_scale_overflow_reports_scalar_route() {
    // P2: Cl(0,0,3) x = 700 + 1000·(e1+e23). The exact unscaled polynomial
    // coefficients (1, 1000, 1000, 1e6) are finite; only the e^700 scalar scale
    // pushes e123 (1e6·e^700 ≈ 1e310) out of range. The honest stage is
    // ScalarRoute, not ClosedForm (the round-4 stage distinction).
    let x = mv::<0, 0, 3>(&[(700.0, &[]), (1000.0, &[1]), (1000.0, &[2, 3])]);
    match x.exp() {
        Err(ExpError::NonFiniteResult {
            stage: amari_core::NonFiniteStage::ScalarRoute,
        }) => {}
        other => panic!("expected ScalarRoute non-finite error, got {other:?}"),
    }
}
