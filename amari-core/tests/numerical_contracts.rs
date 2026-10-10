// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: Apache-2.0
//! CORE-W02 numerical-contract regressions (D01 decisions 1-4).

use amari_core::{Multivector, Vector};

type E3 = Multivector<3, 0, 0>;
type Mink = Multivector<1, 1, 0>;
type Deg = Multivector<1, 0, 1>;

#[test]
fn exact_zero_rejects_tiny_but_nonzero_coefficients() {
    assert!(!E3::scalar(1e-15).is_zero());
    assert!(!E3::basis_vector(0).is_zero());
    assert!(E3::zero().is_zero());
}

#[test]
fn approx_eq_rejects_nonzero_null_difference() {
    // In Cl(1, 0, 1) the generator at index 1 is the null (r) direction.
    let null = Deg::basis_vector(1);
    assert!(!null.approx_eq(&Deg::zero(), 1e-10));
}

#[test]
fn approx_eq_nonfinite_is_false_unless_exactly_equal() {
    let nan = E3::scalar(f64::NAN);
    assert!(!nan.approx_eq(&E3::scalar(f64::NAN), 1e-10));
    assert!(E3::scalar(f64::INFINITY).approx_eq(&E3::scalar(f64::INFINITY), 1e-10));
}

#[test]
fn approx_eq_accepts_relative_scales() {
    let a = E3::scalar(1.0);
    let b = E3::scalar(1.0 + 1e-9);
    assert!(a.approx_eq(&b, 1e-8));
    assert!(!a.approx_eq(&b, 1e-12));
}

#[test]
fn vector_normalize_requires_strictly_positive_square() {
    let timelike = Vector::<1, 1, 0>::from_multivector(&Mink::basis_vector(1));
    assert_eq!(timelike.normalize(), None);
    let null = Vector::<1, 0, 1>::from_multivector(&Deg::basis_vector(1));
    assert_eq!(null.normalize(), None);
    let tiny = Vector::<3, 0, 0>::from_multivector(&(E3::basis_vector(0) * 1e-15));
    let unit = tiny
        .normalize()
        .expect("tiny positive-square vector normalizes exactly");
    assert!((unit.norm_squared() - 1.0).abs() < 1e-12);
}

// ==== Review-round-1 additions (W02 R1 P1-3/P2 remediation) ====

#[test]
fn coefficient_norm_preserves_representable_extremes() {
    assert_eq!(
        Multivector::<1, 0, 0>::scalar(1e-200).coefficient_norm(),
        1e-200
    );
    assert_eq!(
        Multivector::<1, 0, 0>::scalar(1e200).coefficient_norm(),
        1e200
    );
    let mixed = {
        let mut m = Multivector::<2, 0, 0>::zero();
        m.set(0, 3e-200);
        m.set(1, 4e-200);
        m
    };
    assert!((mixed.coefficient_norm() - 5e-200).abs() < 1e-216);
}

#[test]
fn coefficient_norm_nonfinite_policy() {
    assert!(Multivector::<1, 0, 0>::scalar(f64::NAN)
        .coefficient_norm()
        .is_nan());
    assert_eq!(
        Multivector::<1, 0, 0>::scalar(f64::INFINITY).coefficient_norm(),
        f64::INFINITY
    );
}

#[test]
fn coefficient_norm_of_nonzero_null_element_is_nonzero() {
    // e_null in Cl(1, 0, 1): metric square zero, coefficient l2 positive.
    let null = Deg::basis_vector(1);
    assert_eq!(null.norm_squared(), 0.0);
    assert_eq!(null.coefficient_norm(), 1.0);
}

#[test]
fn is_approx_zero_matches_approx_eq_against_zero() {
    let tiny = E3::scalar(1e-12);
    assert!(tiny.is_approx_zero(1e-9));
    assert!(!tiny.is_approx_zero(1e-15));
    assert!(E3::zero().is_approx_zero(0.0));
    assert!(!E3::scalar(1.0).is_approx_zero(1e-9));
}

#[test]
fn approx_eq_relative_component_at_large_scale() {
    // Absolute-only comparison at these tolerances fails; relative passes.
    let a = E3::scalar(1e12);
    let b = E3::scalar(1e12 + 1e4);
    assert!(a.approx_eq(&b, 1e-6));
    assert!(!a.approx_eq(&b, 1e-9));
}

#[test]
fn zero_trait_delegates_exact_semantics() {
    use num_traits::Zero;
    let tiny = E3::scalar(1e-300);
    assert!(!Zero::is_zero(&tiny));
    assert!(Zero::is_zero(&E3::zero()));
    let z: E3 = Zero::zero();
    assert!(z.is_zero());
}

#[test]
fn vector_normalize_handles_subnormal_metric_square() {
    // Coverage: normalization in the subnormal metric-square regime
    // (q = 1e-320). NOTE: this does NOT distinguish division from
    // reciprocal-multiplication scaling — for any representable
    // positive q, 1/sqrt(q) is finite — both paths carry the ~5e-6
    // rounding error of the subnormal q itself. The scaling method is
    // pinned by the Div<f64> regression below and the macro regression.
    let v = Vector::<3, 0, 0>::from_multivector(&(E3::basis_vector(0) * 1e-160));
    let unit = v
        .normalize()
        .expect("subnormal positive-square vector normalizes");
    assert!((unit.coefficient_norm() - 1.0).abs() < 1e-3);
    // Vectors whose metric square rounds to zero outright (single
    // Euclidean component at scale ~<1e-162) return None: a documented
    // norm_squared limitation deferred with the metric-form computation.
}

#[test]
fn div_f64_distinguishes_division_from_reciprocal_scaling() {
    // Division by a subnormal norm is exact here; multiplication by the
    // reciprocal (1.0 / 1e-320 = inf) would destroy the result. This
    // pins the scaling method used by unit!/normalize guards.
    let a = Multivector::<1, 0, 0>::scalar(1e-320);
    let owned = a.clone() / 1e-320;
    let borrowed = &a / 1e-320;
    let expect = Multivector::<1, 0, 0>::scalar(1.0);
    assert_eq!(owned.as_slice(), expect.as_slice());
    assert_eq!(borrowed.as_slice(), expect.as_slice());
    assert!(owned.coefficient_norm().is_finite());
}
