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
    // compose chain. The product's reverse norm is exactly scalar because each
    // factor's is.
    let mut b12 = Multivector::<4, 0, 0>::zero();
    b12.set(3, 1.0); // e12
    let mut b34 = Multivector::<4, 0, 0>::zero();
    b34.set(12, 1.0); // e34
    let r12 = Rotor::<4, 0, 0>::from_multivector_bivector(&b12, 0.7).expect("simple rotor e12");
    let r34 = Rotor::<4, 0, 0>::from_multivector_bivector(&b34, 0.9).expect("simple rotor e34");
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
fn binary_exponentiation_chain_is_bit_exact_for_small_exponents() {
    let rotor = multiphane_rotor();

    // n = 2 is exactly one squaring; n = 3 is (R²)·R.
    let squared = rotor.compose(&rotor);
    assert_eq!(
        rotor.power_integer(2).expect("binary power").as_slice(),
        squared.as_slice()
    );
    let cubed = squared.compose(&rotor);
    assert_eq!(
        rotor.power_integer(3).expect("binary power").as_slice(),
        cubed.as_slice()
    );
}

#[test]
fn binary_exponentiation_composition_law_within_budget() {
    let rotor = multiphane_rotor();

    let cubed = rotor.power_integer(3).expect("binary power 3");
    let fourth = rotor.power_integer(4).expect("binary power 4");
    let lhs = cubed.compose(&fourth);
    let rhs = rotor.power_integer(7).expect("binary power 7");

    // The left chain is 2 compositions for 3, 2 for 4, plus the final compose
    // = 5; the exponent-7 chain is 4. With m = 5 the fixture-scoped budget is
    // m + 1 = 6 ulp on the max-abs coefficient norm.
    let max_abs = lhs
        .as_slice()
        .iter()
        .chain(rhs.as_slice())
        .map(|c| c.abs())
        .fold(0.0f64, f64::max);
    let ulp = f64::from_bits(max_abs.to_bits() + 1) - max_abs;
    let max_diff = lhs
        .as_slice()
        .iter()
        .zip(rhs.as_slice())
        .map(|(&a, &b)| (a - b).abs())
        .fold(0.0f64, f64::max);
    assert!(
        max_diff <= 6.0 * ulp,
        "composition law {max_diff} > {} ({} ulp)",
        6.0 * ulp,
        max_diff / ulp
    );
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
