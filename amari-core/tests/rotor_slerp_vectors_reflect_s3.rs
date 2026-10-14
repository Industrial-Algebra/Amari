//! CORE-W04 stage 3 batteries (contract §2e, §3): sign-selected slerp,
//! `from_vectors` direction construction, and the reflection pair.
//!
//! Exact fixtures whose coefficients cannot be produced by the public
//! constructors (the exact coefficient-dot tie, the candidate `Câ = b̂C`
//! identities on the private candidate builder) live as private-stored-field
//! unit tests in `src/rotor.rs`; the public-API batteries here use the
//! deterministic `from_bivector`/`from_vectors` constructions.

use amari_core::rotor::{reflect, reflect_line};
use amari_core::{Bivector, Multivector, Rotor, RotorError, Vector};

/// Coefficient-wise dot product over the full basis (the §2e sign rule).
fn slice_dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b.iter()).map(|(&x, &y)| x * y).sum()
}

/// Whether two positive `f64` values are within one ulp (bit-distance).
fn within_one_ulp(a: f64, b: f64) -> bool {
    a == b || ((a.to_bits() as i64) - (b.to_bits() as i64)).unsigned_abs() <= 1
}

/// A `Cl(2,0,0)` rotor `√(1 − b²) + b·e12` via `from_bivector`
/// (`exp(−angle/2·e12) = cos(angle/2) − sin(angle/2)e12`).
fn planar_rotor(bivector_coefficient: f64) -> Rotor<2, 0, 0> {
    let angle = -2.0 * bivector_coefficient.asin();
    let e12 = Bivector::<2, 0, 0>::e12();
    Rotor::from_bivector(&e12, angle).expect("bounded planar rotor")
}

/// The F4 twenty-squared rotor composed with its opposite-angle counterpart: a
/// positive scalar `1.0000000000380618`, whose certified `ln4/ln(scale)` power
/// is exactly `[4, 0]` (CORE-W04 R2-F8/F9 fixture).
fn stored_scale_scalar_four() -> Rotor<2, 0, 0> {
    let bivector = Bivector::<2, 0, 0>::e12();
    let mut forward = Rotor::<2, 0, 0>::from_bivector(&bivector, 1e-8).expect("bounded rotor");
    for _ in 0..20 {
        forward = forward.compose(&forward);
    }
    let mut backward = Rotor::<2, 0, 0>::from_bivector(&bivector, -1e-8).expect("bounded rotor");
    for _ in 0..20 {
        backward = backward.compose(&backward);
    }
    let scale = forward.compose(&backward);
    scale
        .power(4f64.ln() / scale.scalar_part().ln())
        .expect("certified exp-route power")
}

// ==================================================================
// §2e slerp — endpoints, identity, sign selection.
// ==================================================================

#[test]
fn slerp_identity_to_identity_2d_is_ok_identity() {
    let identity = Rotor::<2, 0, 0>::identity();

    let at_0 = identity.slerp(&identity, 0.0).expect("t = 0 endpoint");
    assert_eq!(at_0.as_slice(), identity.as_slice());
    let at_half = identity.slerp(&identity, 0.5).expect("interior");
    assert_eq!(at_half.as_slice(), identity.as_slice());
    let at_1 = identity.slerp(&identity, 1.0).expect("t = 1 endpoint");
    assert_eq!(at_1.as_slice(), identity.as_slice());
}

#[test]
fn slerp_identity_endpoint_returns_single_plane_4d_rotor_exactly() {
    // A single-plane Cl(4,0,0) rotor from e34, angle 1.
    let mut b34 = Multivector::<4, 0, 0>::zero();
    b34.set(12, 1.0); // e34
    let rotor =
        Rotor::<4, 0, 0>::from_multivector_bivector(&b34, 1.0).expect("single-plane e34 rotor");

    let identity = Rotor::<4, 0, 0>::identity();
    let at_1 = identity.slerp(&rotor, 1.0).expect("t = 1 endpoint");
    assert_eq!(at_1.as_slice(), rotor.as_slice());
}

#[test]
fn slerp_endpoints_are_exact_direct_returns() {
    let bivector = Bivector::<3, 0, 0>::e12();
    let r1 = Rotor::from_bivector(&bivector, core::f64::consts::PI / 6.0).expect("bounded rotor");
    let r2 = Rotor::from_bivector(&bivector, core::f64::consts::PI / 2.0).expect("bounded rotor");

    assert_eq!(r1.slerp(&r2, 0.0).expect("t = 0").as_slice(), r1.as_slice());
    assert_eq!(r1.slerp(&r2, 1.0).expect("t = 1").as_slice(), r2.as_slice());
}

#[test]
fn slerp_interior_preserves_the_stored_element_scale() {
    // R2-F8: the interior `A·(A⁻¹B*)^t` must certify without normalizing, so a
    // stored-scale scalar rotor keeps its magnitude (the former normalization
    // returned `[1, 0]` for every power of the same scalar).
    let identity = Rotor::<2, 0, 0>::identity();
    let scalar_four = stored_scale_scalar_four();
    assert_eq!(scalar_four.as_slice()[0], 4.0);
    assert_eq!(scalar_four.as_slice()[3], 0.0);

    let half = scalar_four.power(0.5).expect("certified half power");
    assert_eq!(half.as_slice()[0], 2.0);
    assert_eq!(half.as_slice()[3], 0.0);

    let midpoint = identity.slerp(&scalar_four, 0.5).expect("interior");
    assert_eq!(midpoint.as_slice()[0], 2.0);
    assert_eq!(midpoint.as_slice()[3], 0.0);

    // Endpoints stay literal direct returns (bit-exact).
    assert_eq!(
        identity.slerp(&scalar_four, 0.0).expect("t = 0").as_slice(),
        identity.as_slice()
    );
    assert_eq!(
        identity.slerp(&scalar_four, 1.0).expect("t = 1").as_slice(),
        scalar_four.as_slice()
    );
}

#[test]
fn slerp_differing_sign_fixture_selects_the_same_hemisphere() {
    // A ≈ 0.6 + 0.8 e12, B ≈ 0.28 + 0.96 e12: the coefficient dot is +0.936
    // (selects B) while the scalar part of `inner_product` is −0.6 (which would
    // select −B).
    let a = planar_rotor(0.8);
    let b = planar_rotor(0.96);

    let dot = slice_dot(a.as_slice(), b.as_slice());
    assert!(dot > 0.9, "coefficient dot {dot}");

    let out = a.slerp(&b, 0.9).expect("interior");
    // The result stays in B's hemisphere: closer to B than to −B.
    assert!(
        slice_dot(out.as_slice(), b.as_slice()) > 0.0,
        "slerp selected the wrong representative"
    );
    let to_b = (out.as_multivector() - b.as_multivector()).coefficient_norm();
    let to_neg_b = (out.as_multivector() + b.as_multivector()).coefficient_norm();
    assert!(to_b < to_neg_b, "result is closer to −B than to B");
}

#[test]
fn slerp_negative_coefficient_dot_selects_the_negated_representative() {
    // A = identity, B ≈ −1: the coefficient dot is negative, so B* = −B ≈ +1
    // and the interior path stays near the identity.
    let identity = Rotor::<3, 0, 0>::identity();
    let bivector = Bivector::<3, 0, 0>::e12();
    let b = Rotor::from_bivector(&bivector, 2.0 * core::f64::consts::PI).expect("2π rotor");

    let dot = slice_dot(identity.as_slice(), b.as_slice());
    assert!(dot < 0.0, "coefficient dot {dot}");

    let out = identity.slerp(&b, 0.5).expect("interior");
    assert!(
        slice_dot(out.as_slice(), b.as_slice()) < 0.0,
        "slerp kept B instead of the negated representative"
    );
    assert!(out
        .as_multivector()
        .approx_eq(identity.as_multivector(), 1e-10));
}

// ==================================================================
// §3a from_vectors — causal type, direction pre-test, trichotomy.
// ==================================================================

#[test]
fn from_vectors_cl0_2_0_maps_e1_to_e2() {
    let a = Vector::<0, 2, 0>::e1();
    let b = Vector::<0, 2, 0>::e2();
    let rotor = Rotor::from_vectors(&a, &b).expect("same negative causal type");
    let acted = rotor.apply_to_vector(&a).expect("certified rotor acts");
    assert!(acted.as_multivector().approx_eq(b.as_multivector(), 1e-12));
}

#[test]
fn from_vectors_cl0_2_0_same_vector_is_identity_not_antipodal() {
    let a = Vector::<0, 2, 0>::e1();
    let rotor = Rotor::from_vectors(&a, &a).expect("identity maps any vector to itself");
    assert!(rotor
        .as_multivector()
        .approx_eq(Rotor::<0, 2, 0>::identity().as_multivector(), 1e-12));
}

#[test]
fn from_vectors_cl0_2_0_antipodal_resolves_in_two_dimensions() {
    let a = Vector::<0, 2, 0>::e1();
    let b = Vector::<0, 2, 0>::from_components(-1.0, 0.0, 0.0); // −e1
    let rotor = Rotor::from_vectors(&a, &b).expect("Euclidean 2D antipodal resolution");
    let acted = rotor.apply_to_vector(&a).expect("certified rotor acts");
    assert!(acted.as_multivector().approx_eq(b.as_multivector(), 1e-12));
}

#[test]
fn from_vectors_euclidean_2d_non_axis_antipodal_maps_direction() {
    // a = e1 + e2, b = −a in Cl(2,0,0): the naive candidate is ≈ 1.77e−16
    // (a fixing scalar); the exact 2×2-minor pre-test routes to the plane.
    let a = Vector::<2, 0, 0>::from_components(1.0, 1.0, 0.0);
    let b = Vector::<2, 0, 0>::from_components(-1.0, -1.0, 0.0);
    let rotor = Rotor::from_vectors(&a, &b).expect("resolved antipodal pair");
    let acted = rotor.apply_to_vector(&a).expect("certified rotor acts");
    assert!(acted.as_multivector().approx_eq(b.as_multivector(), 1e-12));
}

#[test]
fn from_vectors_subnormal_direction_is_normalized_symbolically() {
    // R3-F15: `b = 2^-1074(e1+e2)` in Cl(0,2,0) has `σ = -2^-2147`, whose root
    // `√2·2^-1074` rounds to the non-unit `2^-1074` if materialized (producing
    // the spurious `[0.6, 0.8]` action). The symbolic root keeps the direction
    // unit and the action at `[1/√2, 1/√2]`. The implementation returns
    // `[0.0, 0.7071067811865476, 0.7071067811865475, 0.0]`; the reference is
    // `[0.0, 0.7071067811865475, 0.7071067811865475, 0.0]`.
    let tiny = f64::from_bits(1);
    let a = Vector::<0, 2, 0>::e1();
    let b = Vector::<0, 2, 0>::from_components(tiny, tiny, 0.0);
    let rotor = Rotor::from_vectors(&a, &b).expect("same negative causal type");
    let acted = rotor.apply_to_vector(&a).expect("certified rotor acts");
    let one_over_sqrt_two = core::f64::consts::FRAC_1_SQRT_2;
    let expected = [0.0, one_over_sqrt_two, one_over_sqrt_two, 0.0];
    for (got, want) in acted
        .as_multivector()
        .as_slice()
        .iter()
        .zip(expected.iter())
    {
        assert!(within_one_ulp(*got, *want), "got {got}, want {want}");
    }
}

#[test]
fn from_vectors_direction_normalization_is_scale_invariant() {
    // R3-F15: the symbolic-root normalization is scale-independent across the
    // whole finite `f64` range, in both definite signatures. The implementation
    // returns bit-identical rotors for every scale, pinned here.
    let scales = [
        1.0f64,
        2f64.powi(-100),
        2f64.powi(-600),
        f64::from_bits(1), // 2^-1074, the minimum subnormal
        2f64.powi(500),
        2f64.powi(1020),
    ];
    for &scale in &scales {
        let a3 = Vector::<3, 0, 0>::e1();
        let b3 = Vector::<3, 0, 0>::from_components(scale, scale, 0.0);
        let r3 = Rotor::from_vectors(&a3, &b3).expect("Cl(3,0,0) scaled direction");
        let a0 = Vector::<0, 3, 0>::e1();
        let b0 = Vector::<0, 3, 0>::from_components(scale, scale, 0.0);
        let r0 = Rotor::from_vectors(&a0, &b0).expect("Cl(0,3,0) scaled direction");

        assert_eq!(r3.as_slice()[0], 0.9238795325112867, "cl3 scale {scale:e}");
        assert_eq!(r3.as_slice()[3], -0.3826834323650897, "cl3 scale {scale:e}");
        assert_eq!(r0.as_slice()[0], 0.9238795325112867, "cl0 scale {scale:e}");
        assert_eq!(r0.as_slice()[3], 0.3826834323650897, "cl0 scale {scale:e}");

        let acted3 = r3.apply_to_vector(&a3).expect("cl3 acts");
        let acted0 = r0.apply_to_vector(&a0).expect("cl0 acts");
        assert_eq!(
            acted3.as_multivector().as_slice(),
            acted0.as_multivector().as_slice()
        );
        let one_over_sqrt_two = core::f64::consts::FRAC_1_SQRT_2;
        assert!(
            within_one_ulp(acted3.as_multivector().get(1), one_over_sqrt_two),
            "cl3 action e1"
        );
        assert!(
            within_one_ulp(acted3.as_multivector().get(2), one_over_sqrt_two),
            "cl3 action e2"
        );
    }
}

#[test]
fn from_vectors_cl2_1_0_null_candidate_is_rejected() {
    let a = Vector::<2, 1, 0>::e1();
    let b = Vector::<2, 1, 0>::from_components(-1.0, 1.0, 1.0); // −e1 + e2 + e3
    assert!(matches!(
        Rotor::from_vectors(&a, &b),
        Err(RotorError::NullCandidate)
    ));
}

#[test]
fn from_vectors_cl1_1_0_sheet_change_is_rejected() {
    let a = Vector::<1, 1, 0>::e1();
    let b = Vector::<1, 1, 0>::from_components(-1.25, 0.75, 0.0); // −5/4 e1 + 3/4 e2
    assert!(matches!(
        Rotor::from_vectors(&a, &b),
        Err(RotorError::SheetChange)
    ));
}

#[test]
fn from_vectors_indefinite_2d_antipodal_is_rejected_as_antipodal() {
    // R1-F6: in Cl(1,1,0) the plane e12 has `e12² = +1`, hence a negative
    // reverse norm (`e12·e12† = −1`), so no checked rotor maps `e1 -> −e1`;
    // note `e12·e1·e12 = −e1`, so e12 is not the identity action. The excluded
    // case is `Err(Antipodal)`, not the old normalization-failure `Degenerate`.
    let a = Vector::<1, 1, 0>::e1();
    let b = Vector::<1, 1, 0>::from_components(-1.0, 0.0, 0.0);
    assert!(matches!(
        Rotor::from_vectors(&a, &b),
        Err(RotorError::Antipodal)
    ));
}

#[test]
fn from_vectors_mv_scalar_input_is_not_a_vector() {
    let scalar = Multivector::<3, 0, 0>::scalar(1.0);
    let e1 = Vector::<3, 0, 0>::e1();
    assert!(matches!(
        Rotor::<3, 0, 0>::from_vectors_mv(&scalar, e1.as_multivector()),
        Err(RotorError::NotAVector)
    ));
}

#[test]
fn from_vectors_null_vector_is_rejected() {
    let null = Vector::<1, 0, 1>::e2(); // nonzero, zero square
    assert!(matches!(
        Rotor::from_vectors(&null, &null),
        Err(RotorError::NullVector)
    ));
}

#[test]
fn from_vectors_mixed_causal_types_are_rejected() {
    let a = Vector::<1, 1, 0>::e1(); // σ = +1
    let b = Vector::<1, 1, 0>::e2(); // σ = −1
    assert!(matches!(
        Rotor::from_vectors(&a, &b),
        Err(RotorError::CausalTypeMismatch)
    ));
}

/// `C = 1 + s·b̂â` satisfies `Câ = b̂C` and `C = s·(b̂+â)·â` exactly, for both
/// signs of `s` (public reconstruction of the §3a.3 identity).
fn assert_candidate_identities_public<const P: usize, const Q: usize, const R: usize>(
    a_hat: &Multivector<P, Q, R>,
    b_hat: &Multivector<P, Q, R>,
    s: f64,
) {
    let candidate = Multivector::scalar(1.0) + b_hat.geometric_product(a_hat) * s;
    let left = candidate.geometric_product(a_hat);
    let right = b_hat.geometric_product(&candidate);
    assert_eq!(left.as_slice(), right.as_slice(), "Câ = b̂C");

    let factorized = (b_hat.clone() + a_hat.clone()).geometric_product(a_hat) * s;
    assert_eq!(candidate.as_slice(), factorized.as_slice(), "C = s·(b̂+â)·â");
}

#[test]
fn candidate_identities_hold_for_both_signs() {
    // s = +1: Cl(2,0,0), â = e1, b̂ = e2.
    assert_candidate_identities_public(
        &Multivector::<2, 0, 0>::basis_vector(0),
        &Multivector::<2, 0, 0>::basis_vector(1),
        1.0,
    );
    // s = −1: Cl(0,2,0), â = e1, b̂ = e2.
    assert_candidate_identities_public(
        &Multivector::<0, 2, 0>::basis_vector(0),
        &Multivector::<0, 2, 0>::basis_vector(1),
        -1.0,
    );
}

// ==================================================================
// §3b/§3c reflect and reflect_line.
// ==================================================================

#[test]
fn reflect_negates_the_normal() {
    let e1 = Multivector::<3, 0, 0>::basis_vector(0);
    let reflected = reflect(&e1, &e1).expect("e1 is a certified unit normal");
    assert_eq!(reflected.as_slice(), (-e1.clone()).as_slice());
}

#[test]
fn reflect_fixes_scalars() {
    let e1 = Multivector::<3, 0, 0>::basis_vector(0);
    let one = Multivector::<3, 0, 0>::scalar(1.0);
    let reflected = reflect(&e1, &one).expect("e1 is a certified unit normal");
    assert_eq!(reflected.get(0), 1.0);
    assert!(reflected
        .as_slice()
        .iter()
        .enumerate()
        .all(|(i, &c)| i == 0 || c == 0.0));
}

#[test]
fn reflect_preserves_perpendicular_blades() {
    let e1 = Multivector::<3, 0, 0>::basis_vector(0);
    let e23 = Bivector::<3, 0, 0>::e23();
    let reflected = reflect(&e1, e23.as_multivector()).expect("e1 is a certified unit normal");
    assert_eq!(reflected.as_slice(), e23.as_multivector().as_slice());
}

#[test]
fn reflect_line_fixes_its_own_line() {
    let e1 = Multivector::<3, 0, 0>::basis_vector(0);
    let reflected = reflect_line(&e1, &e1).expect("e1 is a certified unit normal");
    assert_eq!(reflected.as_slice(), e1.as_slice());
}

#[test]
fn reflect_is_an_involution() {
    let n = Multivector::<3, 0, 0>::basis_vector(0);
    let blades = [
        Multivector::<3, 0, 0>::scalar(1.0),
        Multivector::<3, 0, 0>::basis_vector(0),
        Multivector::<3, 0, 0>::basis_vector(1),
        Bivector::<3, 0, 0>::e12().into_multivector(),
        Bivector::<3, 0, 0>::e23().into_multivector(),
        Multivector::<3, 0, 0>::from_slice(&[0.5, 1.0, -2.0, 0.25, 0.0, 0.0, 0.0, 0.0]),
    ];
    for a in &blades {
        let once = reflect(&n, a).expect("e1 is a certified unit normal");
        let twice = reflect(&n, &once).expect("e1 is a certified unit normal");
        assert!(
            twice.approx_eq(a, 1e-12),
            "reflect involution failed for {a:?}"
        );
    }
}

#[test]
fn reflect_normal_errors_follow_not_a_vector_precedence() {
    let e1 = Multivector::<3, 0, 0>::basis_vector(0);
    let scalar = Multivector::<3, 0, 0>::scalar(1.0);
    assert!(matches!(reflect(&scalar, &e1), Err(RotorError::NotAVector)));

    let two_e1 = Multivector::<3, 0, 0>::basis_vector(0) * 2.0;
    assert!(matches!(
        reflect(&two_e1, &e1),
        Err(RotorError::NotUnitNormal)
    ));
}
