//! CORE-W04 stage 1 red-battery consolidation (contract §5/§7-S1).
//!
//! These are the batteries the S1 dispatch enumerates: the certified stored
//! reverse-norm gate on `inverse`/`apply`, the deleted silent normalization
//! fallbacks, exact grade validation in `from_multivector_bivector`, the
//! checked `TryFrom` wrapper conversions, and the §1.1 escape round-trips.

use amari_core::{
    Bivector, ExpError, GradeError, Multivector, NonFiniteStage, Rotor, RotorError, Scalar, Vector,
};

type Boost = Rotor<1, 1, 0>;

/// The dyadic boost `5/4 + (3/4)e12` (rapidity `ln 2`) and its `N > 0` chains.
fn dyadic_boost(sign: f64) -> Boost {
    // from_bivector stores exp(-e12·angle/2); angle = ∓2·ln2 gives
    // exp(±ln2·e12) = 5/4 ± 3/4 e12 (both representable exactly).
    let bivector = Bivector::<1, 1, 0>::e12().expect("Cl(1,1,0) has blade e12");
    Boost::from_bivector(&bivector, -sign * 2.0 * 2.0f64.ln())
        .expect("exact dyadic boost has positive reverse norm")
}

/// Five repeated raw squarings: `R -> R² -> R⁴ -> R⁸ -> R¹⁶ -> R³²`.
fn square_chain(mut rotor: Boost) -> Boost {
    for _ in 0..5 {
        rotor = rotor.compose(&rotor);
    }
    rotor
}

/// The stored-scale scalar-4 rotor from the F4 twenty-squared construction:
/// `r20(+1e-8) ∘ r20(−1e-8)` is a positive scalar, and its certified
/// `ln4/ln(scale)` power is exactly `[4, 0]` (CORE-W04 R2-F8/F9 fixture).
fn stored_scale_scalar_four() -> Rotor<2, 0, 0> {
    let bivector = Bivector::<2, 0, 0>::e12().expect("Cl(2,0,0) has blade e12");
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

/// The same stored-scale scalar-4 construction in Cl(2,1,0) — the R3-F14
/// sandwich fixture: composed with an `e13` boost it stores
/// `4·(cosh 1 + sinh 1·e13)` with certified `N = 16`.
fn stored_scale_scalar_four_cl2_1_0() -> Rotor<2, 1, 0> {
    let bivector = Bivector::<2, 1, 0>::e12().expect("Cl(2,1,0) has blade e12");
    let mut forward = Rotor::<2, 1, 0>::from_bivector(&bivector, 1e-8).expect("bounded rotor");
    for _ in 0..20 {
        forward = forward.compose(&forward);
    }
    let mut backward = Rotor::<2, 1, 0>::from_bivector(&bivector, -1e-8).expect("bounded rotor");
    for _ in 0..20 {
        backward = backward.compose(&backward);
    }
    let scale = forward.compose(&backward);
    scale
        .power(4f64.ln() / scale.scalar_part().ln())
        .expect("certified exp-route power")
}

#[test]
fn dyadic_boost_base_is_exactly_representable() {
    let rotor = dyadic_boost(1.0);
    // Stored exactly: 5/4 + (3/4)e12, N = 1 exactly.
    assert_eq!(rotor.as_slice()[0], 1.25);
    assert_eq!(rotor.as_slice()[3], 0.75);
}

#[test]
fn r32_boost_chain_is_degenerate_for_inverse_and_apply() {
    let r32 = square_chain(dyadic_boost(1.0));
    // Stored coefficients are exactly [2³¹, 2³¹] with certified N = 0.
    assert_eq!(r32.as_slice()[0], 2f64.powi(31));
    assert_eq!(r32.as_slice()[3], 2f64.powi(31));

    assert!(matches!(r32.inverse(), Err(RotorError::Degenerate)));
    assert!(matches!(
        r32.apply(&Multivector::scalar(1.0)),
        Err(RotorError::Degenerate)
    ));
}

#[test]
fn q_plus_times_q_minus_is_exact_zero_rotor() {
    let q_plus = square_chain(dyadic_boost(1.0)); // [2³¹, 2³¹]
    let q_minus = square_chain(dyadic_boost(-1.0)); // [2³¹, −2³¹]
    let zero = q_plus.compose(&q_minus);
    assert!(zero.as_slice().iter().all(|&c| c == 0.0));

    assert!(matches!(zero.inverse(), Err(RotorError::Degenerate)));
}

#[test]
fn angle_one_boost_stored_inverse_matches_exact_stored_element_inverse() {
    let bivector = Bivector::<1, 1, 0>::e12().expect("Cl(1,1,0) has blade e12");
    let rotor = Boost::from_bivector(&bivector, 1.0).expect("bounded boost exponential");
    // Baseline-observed stored coefficients (the constructor normalizes).
    assert_eq!(rotor.as_slice()[0], 1.127625965206381);
    assert_eq!(rotor.as_slice()[3], -0.5210953054937475);

    let inverse = rotor.inverse().expect("stored N > 0");
    // Exact stored-element inverse R†/N (not the bare reverse).
    assert_eq!(inverse.as_slice()[0], 1.1276259652063807);
    assert_eq!(inverse.as_slice()[3], 0.5210953054937474);
}

#[test]
fn exact_n_inverse_rejects_an_unrepresentable_output() {
    // R2-F9: the exact stored `N` can certify positively while the quotient
    // overflows f64. `4^-535 = 2^-1070` has exact `N = 2^-2140`, so the exact
    // inverse `2^1070` is non-finite and must be a checked `Err`, not an `Ok`
    // holding infinity.
    let scalar_four = stored_scale_scalar_four();
    assert_eq!(scalar_four.as_slice()[0], 4.0);
    assert_eq!(scalar_four.as_slice()[3], 0.0);

    let tiny = scalar_four
        .power(-535.0)
        .expect("certified exp-route power");
    assert_eq!(tiny.as_slice()[0].to_bits(), 0x10);
    assert_eq!(tiny.as_slice()[0], 8e-323);

    assert!(matches!(tiny.inverse(), Err(RotorError::Degenerate)));
    assert!(matches!(
        tiny.power_integer(-1),
        Err(RotorError::Degenerate)
    ));

    // The in-range scalar still inverts bit-exactly.
    let inverse = scalar_four.inverse().expect("stored N > 0");
    assert_eq!(inverse.as_slice()[0], 0.25);
    assert_eq!(inverse.as_slice()[3], 0.0);

    // R3-F14 pin revision: a tiny SCALAR rotor's action is the identity map
    // (`R·A·R†/N = λ²A/λ²`), so `apply` returns `Ok(A)` exactly instead of the
    // former nonzero->zero-loss `Err(Degenerate)`. The exact per-coefficient
    // quotient recovers the image even though the floating numerator underflows.
    let scalar = Multivector::<2, 0, 0>::scalar(1.0);
    let acted_scalar = tiny.apply(&scalar).expect("identity action on a scalar");
    assert_eq!(acted_scalar.as_slice(), scalar.as_slice());
    let e1 = Multivector::<2, 0, 0>::basis_vector(0);
    let acted_e1 = tiny.apply(&e1).expect("identity action on a vector");
    assert_eq!(acted_e1.as_slice(), e1.as_slice());
}

#[test]
fn apply_zero_loss_gate_is_post_exact_quotient() {
    // R3-F14: the post-rounding zero-loss gate is deleted. The angle-2
    // Cl(2,1,0) boost `[6.172322539260975, −4.7008047745752055 e13]` maps
    // `2^-1074(e1−e3)` to the exact quotient ≈ 2^-1074/16, which correctly
    // rounds to the exact zero image — `Ok(zero)`, not `Err(Degenerate)`.
    let tiny = f64::from_bits(1);
    let e13 = Bivector::<2, 1, 0>::e13().expect("Cl(2,1,0) has blade e13");
    let scalar_four = stored_scale_scalar_four_cl2_1_0();
    assert_eq!(scalar_four.as_slice()[0], 4.0);
    assert_eq!(scalar_four.as_slice()[3], 0.0);

    let boost_two = Rotor::<2, 1, 0>::from_bivector(&e13, 2.0).expect("bounded angle-2 boost");
    let angle_two = scalar_four.compose(&boost_two);
    // Absolute constants document the glibc-backed bits (std feature);
    // std-off builds route hyperbolic construction through the P-EMBED
    // dispatch's libm arm, which may differ by 1 ULP (see the twin note
    // in inverse_exp_contracts::round5_hyperbolic_symbolic_vector_matches_scalar_bits).
    let c0 = angle_two.as_slice()[0];
    let c5 = angle_two.as_slice()[5];
    assert!(
        (c0 - 6.172322539260975).abs() <= f64::EPSILON * 8.0,
        "c0 within ULPs: {c0}"
    );
    assert!(
        (c5 - (-4.7008047745752055)).abs() <= f64::EPSILON * 8.0,
        "c5 within ULPs: {c5}"
    );

    let mut a = Multivector::<2, 1, 0>::zero();
    a.set(1, tiny);
    a.set(4, -tiny);
    let zero_image = angle_two.apply(&a).expect("correctly-rounded zero image");
    assert!(zero_image
        .as_slice()
        .iter()
        .all(|&coefficient| coefficient == 0.0));

    // The angle-0.5 variant is unchanged: the quotient is representable as
    // minimum-subnormal coefficients (bit 0x1), not rounded to zero.
    let boost_half = Rotor::<2, 1, 0>::from_bivector(&e13, 0.5).expect("bounded angle-0.5 boost");
    let angle_half = scalar_four.compose(&boost_half);
    let subnormal_image = angle_half.apply(&a).expect("min-subnormal image");
    assert_eq!(subnormal_image.get(1).to_bits(), 1);
    assert_eq!(subnormal_image.get(4).to_bits(), 0x8000_0000_0000_0001);
    assert!(subnormal_image
        .as_slice()
        .iter()
        .enumerate()
        .all(|(index, &coefficient)| index == 1 || index == 4 || coefficient == 0.0));
}

#[test]
fn apply_overflowing_quotient_is_degenerate() {
    // The F14 representability gate is constructible: a certified `N > 0`
    // angle-30 Cl(2,1,0) boost (its exact-`N` inverse succeeds) applied to a
    // near-max input produces a non-finite exact quotient -> `Err(Degenerate)`.
    let e13 = Bivector::<2, 1, 0>::e13().expect("Cl(2,1,0) has blade e13");
    let boost = Rotor::<2, 1, 0>::from_bivector(&e13, 30.0).expect("angle-30 boost");
    assert!(boost.inverse().is_ok(), "certified N > 0 input rotor");
    let mut a = Multivector::<2, 1, 0>::zero();
    a.set(1, 1e300);
    assert!(matches!(boost.apply(&a), Err(RotorError::Degenerate)));
}

#[test]
fn from_multivector_bivector_rejects_a_vector() {
    let vector = Multivector::<3, 0, 0>::basis_vector(0);
    let result = Rotor::<3, 0, 0>::from_multivector_bivector(&vector, 1.0);
    assert!(matches!(result, Err(RotorError::NotABivector)));
}

#[test]
fn from_multivector_bivector_rejects_nan_in_a_foreign_grade() {
    let mut mixed = Multivector::<3, 0, 0>::zero();
    mixed.set(3, 1.0); // e12
    mixed.set(1, f64::NAN); // foreign grade 1
    let result = Rotor::<3, 0, 0>::from_multivector_bivector(&mixed, 1.0);
    assert!(matches!(result, Err(RotorError::NotABivector)));
}

#[test]
fn vector_try_from_rejects_mixed_grades_and_nan_foreign_grades() {
    // Scalar + vector + bivector mixture.
    let mixed = Multivector::<3, 0, 0>::from_slice(&[1.0, 2.0, 3.0, 4.0, 5.0, 0.0, 0.0, 0.0]);
    assert_eq!(
        Vector::try_from(mixed.clone()),
        Err(GradeError::ForeignGrade)
    );

    // NaN in a foreign grade is nonzero and rejects.
    let mut nan_foreign = Multivector::<3, 0, 0>::zero();
    nan_foreign.set(1, 1.0); // allowed grade
    nan_foreign.set(3, f64::NAN); // foreign grade
    assert_eq!(Vector::try_from(nan_foreign), Err(GradeError::ForeignGrade));

    // ±0.0 in a foreign grade is zero and is accepted.
    let mut signed_zero = Multivector::<3, 0, 0>::zero();
    signed_zero.set(1, 1.0);
    signed_zero.set(3, -0.0);
    let v = Vector::try_from(signed_zero).expect("foreign ±0.0 is zero");
    assert_eq!(v.as_multivector().get(1), 1.0);
    assert_eq!(v.as_multivector().get(3), 0.0);

    // try_from_multivector (borrowing form) agrees.
    let pure = Multivector::<3, 0, 0>::basis_vector(0) * 2.0;
    assert!(Vector::try_from_multivector(&pure).is_ok());
    assert!(Vector::try_from_multivector(&mixed).is_err());
}

#[test]
fn scalar_and_bivector_try_from_share_the_foreign_grade_predicate() {
    let mixed = Multivector::<3, 0, 0>::from_slice(&[1.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    assert_eq!(
        Scalar::try_from(mixed.clone()),
        Err(GradeError::ForeignGrade)
    );
    assert_eq!(Bivector::try_from(mixed), Err(GradeError::ForeignGrade));

    let s = Scalar::try_from(Multivector::<3, 0, 0>::scalar(2.5)).expect("pure scalar");
    assert_eq!(s.as_multivector().scalar_part(), 2.5);
}

#[test]
fn escape_round_trips_preserve_coefficients() {
    let vector = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
    let vector_mv = vector.clone().into_multivector();
    let vector_back = Vector::try_from(vector_mv).expect("pure vector");
    assert_eq!(
        vector.as_multivector().as_slice(),
        vector_back.as_multivector().as_slice()
    );

    let bivector = Bivector::<3, 0, 0>::from_components(0.5, -1.0, 2.0);
    let bivector_mv = bivector.clone().into_multivector();
    let bivector_back = Bivector::try_from(bivector_mv).expect("pure bivector");
    assert_eq!(
        bivector.as_multivector().as_slice(),
        bivector_back.as_multivector().as_slice()
    );

    let scalar = Scalar::<3, 0, 0>::from(4.0);
    let scalar_mv = scalar.clone().into_multivector();
    let scalar_back = Scalar::try_from(scalar_mv).expect("pure scalar");
    assert_eq!(
        scalar.as_multivector().as_slice(),
        scalar_back.as_multivector().as_slice()
    );
}

#[test]
fn from_vectors_s1_precedence_is_not_a_vector_then_null() {
    let unit = Vector::<3, 0, 0>::e1().expect("Cl(3,0,0) has basis vector e1");
    let scalar = Multivector::<3, 0, 0>::scalar(1.0);
    assert!(matches!(
        Rotor::<3, 0, 0>::from_vectors_mv(&scalar, unit.as_multivector()),
        Err(RotorError::NotAVector)
    ));

    // Cl(1,0,1): e2 is a null generator (nonzero, zero square).
    let null_generator = Multivector::<1, 0, 1>::basis_vector(1);
    assert!(matches!(
        Rotor::<1, 0, 1>::from_vectors_mv(&null_generator, &null_generator),
        Err(RotorError::NullVector)
    ));
}

#[test]
fn exp_error_carrier_is_lossless() {
    let exp = ExpError::NonFiniteResult {
        stage: NonFiniteStage::ClosedForm,
    };
    assert_eq!(RotorError::from(exp), RotorError::Exp(exp));
}

#[test]
fn inverse_uses_exact_reverse_norm_not_a_rounded_denominator() {
    // R1-F2: `from_bivector(e12, 0.3)` in Cl(1,1,0) stores the exact reverse
    // norm N = 0.9999999999999999. Dividing the reverse by the rounded N gives
    // [1.0112711095766707, 0.15056313315161268]; the exact `R†/N` (one rounding
    // per coefficient) gives the correctly rounded values below.
    let bivector = Bivector::<1, 1, 0>::e12().expect("Cl(1,1,0) has blade e12");
    let rotor = Boost::from_bivector(&bivector, 0.3).expect("bounded boost");
    assert_eq!(rotor.as_slice()[0], 1.0112711095766704);
    assert_eq!(rotor.as_slice()[3], -0.15056313315161265);

    let inverse = rotor.inverse().expect("stored N > 0");
    assert_eq!(inverse.as_slice()[0], 1.0112711095766704);
    assert_eq!(inverse.as_slice()[3], 0.15056313315161265);
}

#[test]
fn exact_certification_helpers_return_errors_above_six_dimensions() {
    // R1-F1: the exact machinery's fixed 64-slot table supports n <= 6. Above
    // that the checked routes return errors instead of panicking in the
    // certificate slice. The identity rotor is the minimal witness.
    let identity = Rotor::<7, 0, 0>::identity();
    assert!(matches!(identity.inverse(), Err(RotorError::Degenerate)));
    let scalar = Multivector::<7, 0, 0>::scalar(1.0);
    assert!(matches!(
        identity.apply(&scalar),
        Err(RotorError::Degenerate)
    ));
    assert!(matches!(identity.power(2.0), Err(RotorError::Degenerate)));

    // The scalar-only logarithm does not touch the exact certification
    // machinery, so it keeps the contract's dimension-independent scalar route
    // (`w = 1 -> zero`).
    assert!(identity
        .logarithm()
        .expect("scalar-only log is dimension independent")
        .as_slice()
        .iter()
        .all(|&c| c == 0.0));

    // The privacy/TryFrom path is not exact-machinery dependent.
    let mv = Multivector::<7, 0, 0>::basis_vector(0);
    let vector = Vector::<7, 0, 0>::try_from(mv).expect("pure grade-1 vector");
    assert_eq!(vector.as_multivector().get(1), 1.0);
}

// ==================================================================
// CORE-W04 R4 remediation: F17 exact sandwich, extreme dynamic range.
// ==================================================================

/// The F17 fixture rotor `from_bivector(e12 + 2^-1070·e13, 0.3)` in
/// Cl(3,0,0): its stored coefficients span from ~1 down to `2^-1070`.
fn f17_rotor() -> Rotor<3, 0, 0> {
    let mut b = Multivector::<3, 0, 0>::zero();
    b.set(3, 1.0); // e12
    b.set(5, f64::from_bits(16)); // e13 = 2^-1070
    let bivector = Bivector::<3, 0, 0>::try_from(b).expect("pure grade-2 bivector");
    Rotor::from_bivector(&bivector, 0.3).expect("certified bounded rotor")
}

/// Apply `rotor` to the coefficient vector and return the image coefficient
/// bits (blade-mask order).
fn apply_bits(rotor: &Rotor<3, 0, 0>, coefficients: &[f64; 8]) -> [u64; 8] {
    let a = Multivector::<3, 0, 0>::from_slice(coefficients);
    let image = rotor
        .apply(&a)
        .expect("exact sandwich over finite stored inputs");
    let mut bits = [0u64; 8];
    for (slot, coefficient) in bits.iter_mut().zip(image.as_slice().iter()) {
        *slot = coefficient.to_bits();
    }
    bits
}

#[test]
fn exact_sandwich_extreme_dynamic_range_matches_fraction_oracles() {
    // F17: `R = from_bivector(e12 + 2^-1070·e13, 0.3)` spans a stored
    // magnitude range from ~1 to `2^-1070`. The exact numerator `R·A·R†` of a
    // mixed-magnitude operand (1e308 / 5e-324 / 1.3 patterns) legitimately
    // exceeds the 66-word single-product window, so the old two-product route
    // overflowed and silently substituted the floating sandwich, returning
    // wrong coefficients (e1 off by one ulp, e3 off by one ulp, and a spurious
    // e123). The direct triple accumulation certifies the exact quotient; the
    // expected bit patterns below are generated independently by a Python
    // `Fraction` oracle over the same stored rotor coefficients (provenance:
    // `f17_oracle.py`, Euclidean Cl(3,0,0), exact integer blade products,
    // correctly-rounded `float(Fraction)`).
    let rotor = f17_rotor();

    // The pinned F17 fixture: e1/e3 corrected and the spurious e123 removed.
    let fixture = [0.0, 1e308, f64::from_bits(1), 0.0, 1.3, 0.0, 0.0, 0.0];
    assert_eq!(
        apply_bits(&rotor, &fixture),
        [
            0,
            9214647875884333309,
            9206776761197263025,
            0,
            4608533498688228566,
            0,
            0,
            0,
        ]
    );

    // Mixed-magnitude sweep (the patterns' oracles from the same generator).
    let sweep: [([f64; 8], [u64; 8]); 6] = [
        (
            [0.0, 1.3, f64::from_bits(1), 0.0, 1e308, 0.0, 0.0, 0.0],
            [
                0,
                4608272008146018276,
                4600592324319428312,
                0,
                9214871658872686752,
                0,
                0,
                0,
            ],
        ),
        (
            [0.0, f64::from_bits(1), 1e308, 0.0, 1.3, 0.0, 0.0, 0.0],
            [
                0,
                18430148798052038833,
                9214647875884333309,
                0,
                4608533498688228556,
                0,
                0,
                0,
            ],
        ),
        (
            [0.0, -1e308, 1.3, 0.0, -f64::from_bits(1), 0.0, 0.0, 0.0],
            [
                0,
                18438019912739109117,
                18430148798052038833,
                0,
                13610328633560496871,
                0,
                0,
                0,
            ],
        ),
        (
            [0.0, 1e308, -1e308, 0.0, 1e308, 0.0, 0.0, 0.0],
            [
                0,
                9216128556504168233,
                18435341589783737762,
                0,
                9214871658872686752,
                0,
                0,
                0,
            ],
        ),
        (
            [0.0, 1e308, f64::from_bits(1), 0.0, -1e308, 0.0, 0.0, 0.0],
            [
                0,
                9214647875884333309,
                9206776761197263025,
                0,
                18438243695727462560,
                0,
                0,
                0,
            ],
        ),
        (
            [1e308, f64::from_bits(1), 0.0, 1.3, 0.0, 0.0, 1e308, 0.0],
            [
                9214871658872686752,
                1,
                0,
                4608533498688228566,
                0,
                18430148798052038833,
                9214647875884333309,
                0,
            ],
        ),
    ];
    for (operand, expected) in sweep {
        assert_eq!(
            apply_bits(&rotor, &operand),
            expected,
            "operand {operand:?}"
        );
    }
}

// ==================================================================
// CORE-W04 R4 remediation: F18 fallible `to_rotation_matrix`.
// ==================================================================

#[test]
fn to_rotation_matrix_propagates_a_compose_domain_exit_instead_of_panicking() {
    // F18: the stored-scale scalar-4 rotor (the F4 twenty-squared construction)
    // in Cl(3,0,0); `4^-535 = 2^-1070` is a finite stored element whose
    // self-compose underflows to the exact zero rotor. The zero rotor has
    // `N = 0`, so `apply` is `Err(Degenerate)`; the matrix conversion must
    // propagate that error rather than unwrapping the action.
    let bivector = Bivector::<3, 0, 0>::e12().expect("Cl(3,0,0) has blade e12");
    let mut forward = Rotor::<3, 0, 0>::from_bivector(&bivector, 1e-8).expect("bounded rotor");
    for _ in 0..20 {
        forward = forward.compose(&forward);
    }
    let mut backward = Rotor::<3, 0, 0>::from_bivector(&bivector, -1e-8).expect("bounded rotor");
    for _ in 0..20 {
        backward = backward.compose(&backward);
    }
    let scalar_four = forward.compose(&backward);
    let scalar_four = scalar_four
        .power(4f64.ln() / scalar_four.scalar_part().ln())
        .expect("certified exp-route power");
    assert_eq!(scalar_four.as_slice()[0], 4.0);

    let tiny = scalar_four
        .power_integer(-535)
        .expect("certified exp-route power");
    assert_eq!(tiny.as_slice()[0].to_bits(), 0x10);
    let zero = tiny.compose(&tiny);
    assert!(zero.as_slice().iter().all(|&c| c == 0.0));

    let e1 = Multivector::<3, 0, 0>::basis_vector(0);
    assert!(matches!(zero.apply(&e1), Err(RotorError::Degenerate)));
    assert!(matches!(
        zero.to_rotation_matrix(),
        Err(RotorError::Degenerate)
    ));
}
