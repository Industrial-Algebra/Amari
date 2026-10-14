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
    let bivector = Bivector::<1, 1, 0>::e12();
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
    let bivector = Bivector::<1, 1, 0>::e12();
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
    let unit = Vector::<3, 0, 0>::e1();
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
