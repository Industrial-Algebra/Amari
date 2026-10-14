//! CORE-W04 stage 4 batteries (contract §4.1–§4.4): the formal complement vs.
//! the metric Hodge dual, the typed 3D duals, and the dimension-general
//! bivector-component API.
//!
//! # Compile-error pin for the deleted generic dual
//!
//! The generic `Vector::<P,Q,R>::hodge_dual() -> Bivector` is deleted (CORE-09):
//! only `Vector<3,0,0>` and `Vector<0,3,0>` have typed duals. There is no
//! `trybuild` infrastructure in this workspace, so the compile error is pinned
//! as a `compile_fail` doctest on the `Vector` type in
//! `amari-core/src/lib.rs` (run by `cargo test -p amari-core`), and the deletion
//! is further verified by grepping the public API for the removed method.

use amari_core::{Bivector, ComponentError, DualError, Multivector, Vector};

/// Coefficient comparison helper.
fn assert_coeffs<const P: usize, const Q: usize, const R: usize>(
    actual: &Multivector<P, Q, R>,
    expected: &Multivector<P, Q, R>,
) {
    for (i, (&a, &b)) in actual
        .as_slice()
        .iter()
        .zip(expected.as_slice())
        .enumerate()
    {
        assert!(
            (a - b).abs() < 1e-14,
            "blade mask {i}: actual={a}, expected={b}"
        );
    }
}

// ==================================================================
// §4.1 formal_complement — permutation sign only, all signatures.
// ==================================================================

#[test]
fn formal_complement_cl_2_0_1_maps_e3_to_e12() {
    // CORE-08 pin: the metric dual of e3 is a null-generator zero divisor,
    // but the formal complement is the permutation-only map e3 ↦ e12.
    let mut e3 = Multivector::<2, 0, 1>::zero();
    e3.set(0b100, 1.0); // e3 (third basis vector)

    let complement = e3.formal_complement();
    let mut e12 = Multivector::<2, 0, 1>::zero();
    e12.set(0b011, 1.0); // e12

    assert_coeffs(&complement, &e12);
    assert_eq!(complement.get(0b011), 1.0);
}

#[test]
fn formal_complement_euclidean_3d_basis() {
    let mut e1 = Multivector::<3, 0, 0>::zero();
    e1.set(1, 1.0);
    let mut e2 = Multivector::<3, 0, 0>::zero();
    e2.set(2, 1.0);
    let mut e3 = Multivector::<3, 0, 0>::zero();
    e3.set(4, 1.0);
    let mut e12 = Multivector::<3, 0, 0>::zero();
    e12.set(3, 1.0);

    // ⋆form: e1 ↦ e23, e2 ↦ −e13, e3 ↦ e12, e12 ↦ e3.
    assert_eq!(e1.formal_complement().get(0b110), 1.0);
    assert_eq!(e2.formal_complement().get(0b101), -1.0);
    assert_eq!(e3.formal_complement().get(0b011), 1.0);
    assert_eq!(e12.formal_complement().get(0b100), 1.0);
}

#[test]
fn formal_complement_squares_to_the_grade_sign_law_4d() {
    // On a pure grade-k element in n = 4 dimensions,
    // formal_complement² = (−1)^{k(n−k)}.
    type Cl4 = Multivector<4, 0, 0>;

    // grade 0: (−1)^0 = +1
    let scalar = Cl4::scalar(3.0);
    assert_coeffs(&scalar.formal_complement().formal_complement(), &scalar);

    // grade 1: (−1)^3 = −1
    let mut e1 = Cl4::zero();
    e1.set(1, 2.0);
    assert_coeffs(
        &e1.formal_complement().formal_complement(),
        &(e1.clone() * -1.0),
    );

    // grade 2: (−1)^4 = +1
    let mut e12 = Cl4::zero();
    e12.set(3, 5.0);
    assert_coeffs(&e12.formal_complement().formal_complement(), &e12);

    // grade 3: (−1)^3 = −1
    let mut e123 = Cl4::zero();
    e123.set(0b0111, 7.0);
    assert_coeffs(
        &e123.formal_complement().formal_complement(),
        &(e123.clone() * -1.0),
    );

    // grade 4: (−1)^0 = +1
    let mut e1234 = Cl4::zero();
    e1234.set(0b1111, 11.0);
    assert_coeffs(&e1234.formal_complement().formal_complement(), &e1234);
}

#[test]
fn formal_complement_squares_to_the_grade_sign_law_3d() {
    type Cl3 = Multivector<3, 0, 0>;

    // grade 1: (−1)^{1·2} = +1; grade 2: (−1)^{2·1} = +1.
    let mut e1 = Cl3::zero();
    e1.set(1, 4.0);
    assert_coeffs(&e1.formal_complement().formal_complement(), &e1);

    let mut e12 = Cl3::zero();
    e12.set(3, -6.0);
    assert_coeffs(&e12.formal_complement().formal_complement(), &e12);

    // grade 3: (−1)^{3·0} = +1.
    let mut e123 = Cl3::zero();
    e123.set(7, 2.0);
    assert_coeffs(&e123.formal_complement().formal_complement(), &e123);
}

#[test]
fn formal_complement_is_linear_on_mixed_grades() {
    type Cl4 = Multivector<4, 0, 0>;

    let mut a = Cl4::scalar(1.5);
    a.set(1, -2.0); // e1
    a.set(6, 0.75); // e23
    let mut b = Cl4::scalar(-0.5);
    b.set(2, 3.25); // e2
    b.set(12, 4.0); // e34

    // Additivity.
    let left = (a.clone() + b.clone()).formal_complement();
    let right = a.formal_complement() + b.formal_complement();
    assert_coeffs(&left, &right);

    // Homogeneity.
    let scaled = (a.clone() * 2.5).formal_complement();
    assert_coeffs(&scaled, &(a.formal_complement() * 2.5));
}

#[test]
fn formal_complement_maps_grade_to_complement_grade() {
    type Cl4 = Multivector<4, 0, 0>;

    // grade 0 ↦ grade 4
    assert_eq!(Cl4::scalar(1.0).formal_complement().grade(), 4);
    // grade 1 ↦ grade 3
    let mut e1 = Cl4::zero();
    e1.set(1, 1.0);
    assert_eq!(e1.formal_complement().grade(), 3);
    // grade 2 ↦ grade 2
    let mut e12 = Cl4::zero();
    e12.set(3, 1.0);
    assert_eq!(e12.formal_complement().grade(), 2);
}

// ==================================================================
// §4.2 metric hodge_dual — reverse(A)·I, degenerate rejection.
// ==================================================================

#[test]
fn hodge_dual_cl_2_0_1_rejects_the_degenerate_pseudoscalar() {
    // CORE-08 pin: with a null generator the pseudoscalar is a zero divisor,
    // so the metric dual is rejected rather than silently zeroed.
    let mut e3 = Multivector::<2, 0, 1>::zero();
    e3.set(0b100, 1.0);
    assert_eq!(
        e3.hodge_dual(),
        Err(DualError::DegeneratePseudoscalar),
        "null-generator signature must reject the metric dual"
    );
}

#[test]
fn hodge_dual_euclidean_3d_table() {
    type Cl3 = Multivector<3, 0, 0>;

    let e1 = Cl3::basis_vector(0);
    let e2 = Cl3::basis_vector(1);
    let e3 = Cl3::basis_vector(2);

    let dual_e1 = e1.hodge_dual().expect("non-degenerate");
    assert_eq!(dual_e1.get(0b110), 1.0); // e23
    assert_eq!(dual_e1.get(0b011), 0.0);
    assert_eq!(dual_e1.get(0b101), 0.0);

    let dual_e2 = e2.hodge_dual().expect("non-degenerate");
    assert_eq!(dual_e2.get(0b101), -1.0); // −e13

    let dual_e3 = e3.hodge_dual().expect("non-degenerate");
    assert_eq!(dual_e3.get(0b011), 1.0); // e12

    // ⋆e12 = e3, ⋆e13 = −e2, ⋆e23 = e1.
    let mut e12 = Cl3::zero();
    e12.set(0b011, 1.0);
    assert_eq!(e12.hodge_dual().expect("non-degenerate").get(0b100), 1.0);

    let mut e13 = Cl3::zero();
    e13.set(0b101, 1.0);
    assert_eq!(e13.hodge_dual().expect("non-degenerate").get(0b010), -1.0);

    let mut e23 = Cl3::zero();
    e23.set(0b110, 1.0);
    assert_eq!(e23.hodge_dual().expect("non-degenerate").get(0b001), 1.0);

    // ⋆1 = e123, ⋆e123 = 1.
    assert_eq!(Cl3::scalar(1.0).hodge_dual().expect("ok").get(0b111), 1.0);
    let mut e123 = Cl3::zero();
    e123.set(0b111, 1.0);
    assert_eq!(e123.hodge_dual().expect("ok").get(0), 1.0);
}

#[test]
fn hodge_dual_cl_2_0_0_table() {
    // ⋆e1 = e2, ⋆e2 = −e1, ⋆e12 = 1.
    type Cl2 = Multivector<2, 0, 0>;

    let e1 = Cl2::basis_vector(0);
    let e2 = Cl2::basis_vector(1);

    let dual_e1 = e1.hodge_dual().expect("non-degenerate");
    assert_eq!(dual_e1.get(0b10), 1.0); // e2
    assert_eq!(dual_e1.get(0b01), 0.0);

    let dual_e2 = e2.hodge_dual().expect("non-degenerate");
    assert_eq!(dual_e2.get(0b01), -1.0); // −e1

    let mut e12 = Cl2::zero();
    e12.set(0b11, 1.0);
    let dual_e12 = e12.hodge_dual().expect("non-degenerate");
    assert_eq!(dual_e12.get(0), 1.0); // scalar 1
    assert_eq!(dual_e12.get(0b11), 0.0);
}

#[test]
fn hodge_dual_double_dual_is_identity_in_3d() {
    // In 3D the double dual sign is (−1)^{k(3−k)} = +1 for k = 1, 2.
    type Cl3 = Multivector<3, 0, 0>;

    let mut v = Cl3::zero();
    v.set(1, 0.5);
    v.set(2, -1.25);
    let v2 = v.hodge_dual().expect("ok").hodge_dual().expect("ok");
    assert_coeffs(&v2, &v);

    let mut b = Cl3::zero();
    b.set(0b011, 2.0);
    b.set(0b101, -3.0);
    let b2 = b.hodge_dual().expect("ok").hodge_dual().expect("ok");
    assert_coeffs(&b2, &b);
}

// ==================================================================
// §4.3 typed 3D duals.
// ==================================================================

#[test]
fn typed_vector_dual_cl_3_0_0() {
    let e1 = Vector::<3, 0, 0>::e1();
    let dual = e1.hodge_dual().expect("non-degenerate 3D");
    // ⋆e1 = e23, which is component 2 in the lexicographic map.
    assert_eq!(dual.component(2), Some(1.0));
    assert_eq!(dual.component(0), Some(0.0));
    assert_eq!(dual.component(1), Some(0.0));

    // The formal complement of a vector in Cl(3,0,0) is the same map.
    assert_eq!(e1.formal_complement().component(2), Some(1.0));

    let e2 = Vector::<3, 0, 0>::e2();
    assert_eq!(e2.hodge_dual().expect("ok").component(1), Some(-1.0)); // −e13
    let e3 = Vector::<3, 0, 0>::e3();
    assert_eq!(e3.hodge_dual().expect("ok").component(0), Some(1.0)); // e12
}

#[test]
fn typed_bivector_dual_cl_3_0_0() {
    let e12 = Bivector::<3, 0, 0>::e12();
    let dual = e12.hodge_dual().expect("non-degenerate 3D");
    // ⋆e12 = e3 → vector component 2.
    assert_eq!(dual.as_multivector().vector_component(2), 1.0);

    let e13 = Bivector::<3, 0, 0>::e13();
    assert_eq!(
        e13.hodge_dual()
            .expect("ok")
            .as_multivector()
            .vector_component(1),
        -1.0
    );

    let e23 = Bivector::<3, 0, 0>::e23();
    assert_eq!(
        e23.hodge_dual()
            .expect("ok")
            .as_multivector()
            .vector_component(0),
        1.0
    );

    // Formal complement is infallible and returns the same grade image.
    assert_eq!(
        e12.formal_complement().as_multivector().vector_component(2),
        1.0
    );
}

#[test]
fn typed_duals_cl_0_3_0_exist() {
    let e1 = Vector::<0, 3, 0>::e1();
    // The formal complement is the permutation-only map in every signature.
    assert_eq!(e1.formal_complement().component(2), Some(1.0)); // e23
    let e2 = Vector::<0, 3, 0>::e2();
    assert_eq!(e2.formal_complement().component(1), Some(-1.0)); // −e13
    let e3 = Vector::<0, 3, 0>::e3();
    assert_eq!(e3.formal_complement().component(0), Some(1.0)); // e12

    // The metric dual is defined (the signature is non-degenerate).
    assert!(e1.hodge_dual().is_ok());

    let e12 = Bivector::<0, 3, 0>::e12();
    assert_eq!(
        e12.formal_complement().as_multivector().vector_component(2),
        1.0
    );
    assert!(e12.hodge_dual().is_ok());
}

// ==================================================================
// §4.4 dimension-general bivector components.
// ==================================================================

#[test]
fn set_bivector_component_4d_component_3_round_trips_e23() {
    // CORE-15 pin: in Cl(4,0,0) component 3 is e23 (mask 0b0110 = 6).
    let mut mv = Multivector::<4, 0, 0>::zero();
    assert!(mv.set_bivector_component(3, 7.5));
    assert_eq!(mv.get(6), 7.5, "component 3 must write the e23 blade");

    // The old silent-zero layout wrote index 2 to e23; the general map makes
    // index 2 the e14 blade (mask 0b1001 = 9).
    assert!(mv.set_bivector_component(2, -1.0));
    assert_eq!(mv.get(9), -1.0, "component 2 is e14 in 4D");
    assert_eq!(mv.get(6), 7.5, "writing component 2 leaves e23 untouched");
}

#[test]
fn bivector_component_round_trip_4d() {
    let mut bivector = Bivector::<4, 0, 0>::zero();
    assert!(bivector.set_component(3, 2.5)); // e23
    assert_eq!(bivector.component(3), Some(2.5));
    assert_eq!(bivector.as_multivector().get(6), 2.5); // e23 mask

    // 4D lexicographic order: 0↦e12, 1↦e13, 2↦e14, 3↦e23, 4↦e24, 5↦e34.
    let expected_masks = [0b0011u32, 0b0101, 0b1001, 0b0110, 0b1010, 0b1100];
    let mut expected = Bivector::<4, 0, 0>::zero();
    let mut actual = Bivector::<4, 0, 0>::zero();
    for (component, &mask) in expected_masks.iter().enumerate() {
        let value = component as f64 + 1.0;
        assert!(actual.set_component(component, value));
        assert!(expected.set_component(component, value));
        assert_eq!(actual.component(component), Some(value));
        assert_eq!(actual.as_multivector().get(mask as usize), value);
    }
    assert_eq!(actual, expected);
}

#[test]
fn bivector_zero_exists_in_4d() {
    let zero = Bivector::<4, 0, 0>::zero();
    for k in 0..6 {
        assert_eq!(zero.component(k), Some(0.0));
    }
    assert!(zero.as_multivector().is_zero());
}

#[test]
fn out_of_range_component_writes_are_rejected_without_write() {
    let mut bivector = Bivector::<4, 0, 0>::zero();
    // 4D has exactly 6 bivector components (0..=5).
    assert!(!bivector.set_component(6, 1.0));
    assert_eq!(bivector.component(6), None);
    assert!(
        bivector.as_multivector().is_zero(),
        "no write on rejected index"
    );

    // Multivector::set_bivector_component shares the contract.
    let mut mv = Multivector::<4, 0, 0>::zero();
    assert!(!mv.set_bivector_component(6, 1.0));
    assert!(!mv.set_bivector_component(100, 1.0));
    assert!(mv.is_zero());
}

#[test]
fn bivector_get_keeps_out_of_range_zero_and_blade_semantics() {
    // Existing get semantics: in range it is the lexicographic component, out
    // of range it returns 0.0.
    let bivector = Bivector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
    assert_eq!(bivector.get(0), 1.0); // e12
    assert_eq!(bivector.get(1), 2.0); // e13
    assert_eq!(bivector.get(2), 3.0); // e23
    assert_eq!(bivector.get(999), 0.0);
}

#[test]
fn e_ij_constructs_the_named_blades() {
    let e12 = Bivector::<4, 0, 0>::e_ij(1, 2).expect("in range");
    assert_eq!(e12.component(0), Some(1.0));
    let e34 = Bivector::<4, 0, 0>::e_ij(3, 4).expect("in range");
    assert_eq!(e34.component(5), Some(1.0));
    assert_eq!(e34.as_multivector().get(0b1100), 1.0);

    // The named shorthand constructors agree with e_ij.
    assert_eq!(Bivector::<4, 0, 0>::e12().component(0), Some(1.0));
    assert_eq!(Bivector::<4, 0, 0>::e13().component(1), Some(1.0));
    assert_eq!(Bivector::<4, 0, 0>::e23().component(3), Some(1.0));
}

#[test]
fn e_ij_rejects_reversed_equal_and_out_of_range_indices() {
    // Reversed indices are rejected, not silently sign-flipped.
    assert_eq!(
        Bivector::<4, 0, 0>::e_ij(2, 1),
        Err(ComponentError::OutOfRange)
    );
    // Equal indices.
    assert_eq!(
        Bivector::<4, 0, 0>::e_ij(2, 2),
        Err(ComponentError::OutOfRange)
    );
    // Zero is not a valid basis index.
    assert_eq!(
        Bivector::<4, 0, 0>::e_ij(0, 2),
        Err(ComponentError::OutOfRange)
    );
    assert_eq!(
        Bivector::<4, 0, 0>::e_ij(0, 0),
        Err(ComponentError::OutOfRange)
    );
    // j beyond the ambient dimension.
    assert_eq!(
        Bivector::<4, 0, 0>::e_ij(2, 5),
        Err(ComponentError::OutOfRange)
    );
    assert_eq!(
        Bivector::<4, 0, 0>::e_ij(4, 5),
        Err(ComponentError::OutOfRange)
    );
}

#[test]
fn from_components_delegates_to_the_general_map() {
    // First three lexicographic components: e12, e13, and (in 4D) e14.
    let bivector = Bivector::<4, 0, 0>::from_components(1.0, 2.0, 3.0);
    assert_eq!(bivector.component(0), Some(1.0)); // e12
    assert_eq!(bivector.component(1), Some(2.0)); // e13
    assert_eq!(bivector.component(2), Some(3.0)); // e14 in 4D
    assert_eq!(bivector.as_multivector().get(0b1001), 3.0); // e14 mask

    // In 3D the third component is e23, preserving the historical mapping.
    let planar = Bivector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
    assert_eq!(planar.component(0), Some(1.0));
    assert_eq!(planar.component(1), Some(2.0));
    assert_eq!(planar.component(2), Some(3.0));
    assert_eq!(planar.as_multivector().get(0b0110), 3.0); // e23 mask
}
