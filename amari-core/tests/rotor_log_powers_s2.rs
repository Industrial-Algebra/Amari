//! CORE-W04 stage 2 law batteries (contract §2): domain-aware logarithm and
//! powers.
//!
//! These are the public-API fixtures the S2 dispatch enumerates. Batteries that
//! need to store a rotor value outside the constructor closure (scalar-only
//! non-unit values, the zero rotor, the negative sheet, the range extremes,
//! the exact dyadic `R^32` chain, and the R8 round-trip witness) live as
//! private-stored-field unit tests in `src/rotor.rs`.

use amari_core::{Bivector, Multivector, Rotor, RotorError};

/// Monotonic key of an `f64` (finite inputs), so `abs_diff` is an ulp distance.
fn total_order_key(x: f64) -> u64 {
    let bits = x.to_bits();
    if bits & (1u64 << 63) != 0 {
        !bits
    } else {
        bits | (1u64 << 63)
    }
}

/// Largest coefficient-wise ulp distance between two slices.
fn ulp_distance(a: &[f64], b: &[f64]) -> u64 {
    a.iter()
        .zip(b.iter())
        .map(|(&x, &y)| total_order_key(x).abs_diff(total_order_key(y)))
        .max()
        .unwrap_or(0)
}

// ======================================================================
// §2d powers — bit-exact endpoints.
// ======================================================================

#[test]
fn boost_fixture_power_integer_one_is_bit_exact_clone() {
    let bivector = Bivector::<1, 1, 0>::e12();
    let stored = Rotor::from_bivector(&bivector, 1.0).expect("bounded boost exponential");
    assert_eq!(stored.as_slice()[0], 1.127625965206381);
    assert_eq!(stored.as_slice()[3], -0.5210953054937475);

    let clone = stored.power_integer(1).expect("n = 1 direct clone");
    assert_eq!(
        clone.as_slice()[0].to_bits(),
        stored.as_slice()[0].to_bits()
    );
    assert_eq!(
        clone.as_slice()[3].to_bits(),
        stored.as_slice()[3].to_bits()
    );
}

#[test]
fn two_pi_rotor_power_integer_one_clones_residual_bivector() {
    let bivector = Bivector::<3, 0, 0>::e12();
    let rotor =
        Rotor::from_bivector(&bivector, 2.0 * std::f64::consts::PI).expect("bounded 2π rotor");
    // The stored scalar is the −1 double-cover representative with a nonzero
    // residual bivector; the pin is the bit-exact clone, not mathematical zero.
    assert!(rotor.as_slice()[0] < 0.0);

    let clone = rotor.power_integer(1).expect("n = 1 direct clone");
    assert_eq!(clone.as_slice(), rotor.as_slice());
}

#[test]
fn power_integer_zero_one_negative_one_laws() {
    let bivector = Bivector::<1, 1, 0>::e12();
    let rotor = Rotor::from_bivector(&bivector, 1.0).expect("bounded boost");
    let identity = Rotor::<1, 1, 0>::identity();

    assert_eq!(
        rotor.power_integer(0).expect("identity").as_slice(),
        identity.as_slice()
    );
    assert_eq!(
        rotor.power_integer(1).expect("clone").as_slice(),
        rotor.as_slice()
    );
    let inverse = rotor.inverse().expect("stored N > 0");
    assert_eq!(
        rotor.power_integer(-1).expect("inverse").as_slice(),
        inverse.as_slice()
    );
}

#[test]
fn power_integer_minus_two_boost_matches_exact_oracle() {
    // log R = -½e12 for the angle-1 boost, so R^(-2) = exp(e12) = cosh(1) + sinh(1)e12.
    let bivector = Bivector::<1, 1, 0>::e12();
    let rotor = Rotor::from_bivector(&bivector, 1.0).expect("bounded boost");
    let squared = rotor.power_integer(-2).expect("certified log route");
    let expected = 1.5430806348152437_f64; // cosh(1), correctly rounded exact oracle
    let distance = total_order_key(squared.as_slice()[0]).abs_diff(total_order_key(expected));
    assert!(
        distance <= 2,
        "power_integer(-2) scalar {distance} ulp from cosh(1)"
    );
}

#[test]
fn integral_power_delegates_to_power_integer() {
    let bivector = Bivector::<1, 1, 0>::e12();
    let rotor = Rotor::from_bivector(&bivector, 1.0).expect("bounded boost");
    let identity = Rotor::<1, 1, 0>::identity();

    assert_eq!(
        rotor.power(1.0).expect("clone").as_slice(),
        rotor.as_slice()
    );
    assert_eq!(
        rotor.power(0.0).expect("identity").as_slice(),
        identity.as_slice()
    );
    assert_eq!(
        rotor.power(-1.0).expect("inverse").as_slice(),
        rotor.inverse().expect("stored N > 0").as_slice()
    );
}

// ======================================================================
// §2c elliptic evaluation.
// ======================================================================

#[test]
fn small_angle_bivector_is_preserved_by_the_atan2_route() {
    let bivector = Bivector::<3, 0, 0>::e12();
    let rotor = Rotor::from_bivector(&bivector, 1e-8).expect("bounded small-angle rotor");
    let log = rotor.logarithm().expect("elliptic certified");
    // `from_bivector` stores `exp(-angle/2 · e12)`, so the log's e12 coefficient
    // is `-angle/2 = -5e-9`; the old threshold route dropped it.
    assert!(
        (log.get(3) + 5e-9).abs() < 1e-22,
        "log e12 = {}",
        log.get(3)
    );
    assert!(log.get(3).abs() > 1e-9);
    // The scalar scale is ½·ln(1) up to the stored-norm rounding.
    assert!(log.get(0).abs() < 1e-15);
}

#[test]
fn elliptic_round_trip_is_fixture_bounded() {
    let bivector = Bivector::<3, 0, 0>::e12();
    for angle in [0.3, 1.0, 2.5, -1.7, std::f64::consts::PI] {
        let rotor = Rotor::from_bivector(&bivector, angle).expect("bounded rotor");
        let log = rotor.logarithm().expect("elliptic certified");
        let recovered = log.exp().expect("W03 exponential");
        let distance = ulp_distance(recovered.as_slice(), rotor.as_slice());
        assert!(distance <= 8, "angle {angle}: {distance} ulp");
    }
}

#[test]
fn log_exp_selects_the_principal_branch_for_two_pi_plane() {
    // exp(2π·e12) = 1 exactly in exact arithmetic; `from_bivector` reaches it
    // as `exp(-angle/2 · e12)` with `angle = -4π`, and its log is the principal
    // branch result (approximately zero, not 2π).
    let bivector = Bivector::<3, 0, 0>::e12();
    let rotor =
        Rotor::from_bivector(&bivector, -4.0 * std::f64::consts::PI).expect("bounded 2π rotor");
    let log = rotor.logarithm().expect("elliptic certified");
    assert!(log.get(0).abs() < 1e-14);
    assert!(log.get(3).abs() < 1e-14);
}

// ======================================================================
// §2c hyperbolic evaluation.
// ======================================================================

#[test]
fn hyperbolic_small_boost_rapidity_is_two_pow_minus_54() {
    // R = exp(2⁻⁵⁴e12) = 1 + 2⁻⁵⁴e12 (w = 1, β = 2⁻⁵⁴; gate w>0 ∧ N>0 alone).
    let bivector = Bivector::<1, 1, 0>::e12();
    let rotor = Rotor::from_bivector(&bivector, -2f64.powi(-53)).expect("certified small boost");
    let log = rotor.logarithm().expect("hyperbolic certified");

    let expected = 5.551115123125783e-17; // 2⁻⁵⁴
    assert_eq!(log.get(3), expected);
}

#[test]
fn hyperbolic_round_trip_is_fixture_bounded() {
    let bivector = Bivector::<1, 1, 0>::e12();
    for angle in [0.5, 1.0, 2.0, -1.5] {
        let rotor = Rotor::from_bivector(&bivector, angle).expect("bounded boost");
        let log = rotor.logarithm().expect("hyperbolic certified");
        let recovered = log.exp().expect("W03 exponential");
        let distance = ulp_distance(recovered.as_slice(), rotor.as_slice());
        assert!(distance <= 8, "angle {angle}: {distance} ulp");
    }
}

// ======================================================================
// §2d binary exponentiation for uncertified multi-plane rotors.
// ======================================================================

fn multiphane_rotor() -> Rotor<4, 0, 0> {
    // Cl(4,0,0): the product of the two orthogonal simple rotors in e12 and e34
    // has grade-4 content, so the logarithm is uncertified and powers use the
    // compose chain. R1-F5 fixture: r12(0.3) · r34(0.7).
    let mut b12 = Multivector::<4, 0, 0>::zero();
    b12.set(3, 1.0); // e12
    let mut b34 = Multivector::<4, 0, 0>::zero();
    b34.set(12, 1.0); // e34
    let r12 = Rotor::<4, 0, 0>::from_multivector_bivector(&b12, 0.3).expect("simple rotor e12");
    let r34 = Rotor::<4, 0, 0>::from_multivector_bivector(&b34, 0.7).expect("simple rotor e34");
    r12.compose(&r34)
}

#[test]
fn uncertified_multiphane_log_is_rejected() {
    assert!(matches!(
        multiphane_rotor().logarithm(),
        Err(RotorError::Uncertified)
    ));
}

#[test]
fn binary_power_of_a_rounded_multiphane_rotor_is_degenerate() {
    // R1-F5: the composite has grade-4 content, so its exact `R·R†` is
    // non-scalar; the full positive-scalar certificate rejects every checked
    // binary-power boundary. Previously the scalar-part-sign gate returned
    // `Ok`, while the output's own `inverse()` was `Err(Degenerate)`.
    let rotor = multiphane_rotor();
    assert!(matches!(
        rotor.power_integer(2),
        Err(RotorError::Degenerate)
    ));
    assert!(matches!(
        rotor.power_integer(3),
        Err(RotorError::Degenerate)
    ));

    // Each simple factor stays a certified single-plane rotor and powers
    // through the exact log route.
    let mut b12 = Multivector::<4, 0, 0>::zero();
    b12.set(3, 1.0); // e12
    let mut b34 = Multivector::<4, 0, 0>::zero();
    b34.set(12, 1.0); // e34
    let r12 = Rotor::<4, 0, 0>::from_multivector_bivector(&b12, 0.3).expect("simple rotor e12");
    let r34 = Rotor::<4, 0, 0>::from_multivector_bivector(&b34, 0.7).expect("simple rotor e34");
    assert!(r12.power_integer(2).is_ok());
    assert!(r34.power_integer(2).is_ok());
}

// ======================================================================
// §2d non-integral power through the exp carrier.
// ======================================================================

#[test]
fn non_integral_power_matches_a_half_angle_construction() {
    let bivector = Bivector::<3, 0, 0>::e12();
    let rotor = Rotor::from_bivector(&bivector, std::f64::consts::PI / 2.0).expect("bounded rotor");
    let half = rotor.power(0.5).expect("certified power");
    let quarter =
        Rotor::from_bivector(&bivector, std::f64::consts::PI / 4.0).expect("bounded rotor");
    let distance = ulp_distance(half.as_slice(), quarter.as_slice());
    assert!(distance <= 8, "half power {distance} ulp");
}

#[test]
fn non_integral_power_propagates_log_domain_errors() {
    // A scalar-only negative rotor is not reachable through the public
    // constructors; the multi-plane path is used here to confirm that an
    // exponential carrier error is a `RotorError::Exp`, not a panic.
    let rotor = multiphane_rotor();
    // The multi-plane log is uncertified, so a non-integral power propagates it
    // directly (not through the integer fallback).
    assert!(matches!(rotor.power(0.5), Err(RotorError::Uncertified)));
}

#[test]
fn power_preserves_the_logarithm_scale_of_the_twenty_squared_rotor() {
    // R1-F4: the exp-route power must preserve the element scale the logarithm
    // carries, not renormalize. R^0 = from_bivector(e12, 1e-8) in Cl(2,0,0),
    // squared 20 times, stores [0.9999862561551659, -0.005242855980934775];
    // the true exp(2·log R) is [0.9999450249603883, -0.010485567847871372],
    // while the former normalization returned a renormalized value near
    // [0.9999450249223286, -0.010485567472272].
    let bivector = Bivector::<2, 0, 0>::e12();
    let mut rotor = Rotor::<2, 0, 0>::from_bivector(&bivector, 1e-8).expect("bounded rotor");
    for _ in 0..20 {
        rotor = rotor.compose(&rotor);
    }
    assert_eq!(rotor.as_slice()[0], 0.9999862561551659);
    assert_eq!(rotor.as_slice()[3], -0.005242855980934775);

    let squared = rotor.power_integer(2).expect("exp-route power");
    assert_eq!(squared.as_slice()[0], 0.9999450249603883);
    assert_eq!(squared.as_slice()[3], -0.010485567847871372);

    // The real power routes an integral exponent through the same
    // preserve-scale path.
    let squared_real = rotor.power(2.0).expect("exp-route real power");
    assert_eq!(squared_real.as_slice(), squared.as_slice());
}
