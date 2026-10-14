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

/// A `Cl(2,0,0)` rotor `√(1 − b²) + b·e12` via `from_bivector`
/// (`exp(−angle/2·e12) = cos(angle/2) − sin(angle/2)e12`).
fn planar_rotor(bivector_coefficient: f64) -> Rotor<2, 0, 0> {
    let angle = -2.0 * bivector_coefficient.asin();
    let e12 = Bivector::<2, 0, 0>::e12();
    Rotor::from_bivector(&e12, angle).expect("bounded planar rotor")
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
