//! Property-based tests for geometric algebra using proptest
//!
//! These tests verify fundamental mathematical properties hold for
//! randomly generated inputs, providing broader coverage than
//! example-based tests.

use crate::*;
use proptest::prelude::*;

#[cfg(not(feature = "std"))]
use alloc::format;

#[cfg(feature = "std")]
use std::f64::consts;

#[cfg(not(feature = "std"))]
use core::f64::consts;

/// Strategy for generating valid multivector coefficients
fn multivector_coefficients() -> impl Strategy<Value = Vec<f64>> {
    prop::collection::vec(-100.0..100.0, 8..=8) // For Cl(3,0,0) = 2^3 = 8 coefficients
}

/// Strategy for generating non-zero scalars
fn non_zero_scalar() -> impl Strategy<Value = f64> {
    (-100.0..-0.001).prop_union(0.001..100.0)
}

/// Strategy for generating valid basis vector indices
fn basis_index() -> impl Strategy<Value = usize> {
    0..3usize // For 3D space
}

/// Create a multivector from coefficients
fn create_multivector(coeffs: Vec<f64>) -> Multivector<3, 0, 0> {
    Multivector::from_coefficients(coeffs)
}

proptest! {
    /// Test additive commutativity: a + b = b + a
    #[test]
    fn prop_addition_commutative(
        a_coeffs in multivector_coefficients(),
        b_coeffs in multivector_coefficients()
    ) {
        let a = create_multivector(a_coeffs);
        let b = create_multivector(b_coeffs);

        let ab = &a + &b;
        let ba = &b + &a;

        for i in 0..8 {
            prop_assert!((ab.get(i) - ba.get(i)).abs() < 1e-10);
        }
    }

    /// Test additive associativity: (a + b) + c = a + (b + c)
    #[test]
    fn prop_addition_associative(
        a_coeffs in multivector_coefficients(),
        b_coeffs in multivector_coefficients(),
        c_coeffs in multivector_coefficients()
    ) {
        let a = create_multivector(a_coeffs);
        let b = create_multivector(b_coeffs);
        let c = create_multivector(c_coeffs);

        let ab_c = &(&a + &b) + &c;
        let a_bc = &a + &(&b + &c);

        for i in 0..8 {
            prop_assert!((ab_c.get(i) - a_bc.get(i)).abs() < 1e-10);
        }
    }

    /// Test additive identity: a + 0 = a
    #[test]
    fn prop_additive_identity(a_coeffs in multivector_coefficients()) {
        let a = create_multivector(a_coeffs);
        let zero = Multivector::zero();
        let result = &a + &zero;

        for i in 0..8 {
            prop_assert!((a.get(i) - result.get(i)).abs() < 1e-10);
        }
    }

    /// Test geometric product associativity: (ab)c = a(bc)
    #[test]
    fn prop_geometric_product_associative(
        a_coeffs in multivector_coefficients(),
        b_coeffs in multivector_coefficients(),
        c_coeffs in multivector_coefficients()
    ) {
        let a = create_multivector(a_coeffs);
        let b = create_multivector(b_coeffs);
        let c = create_multivector(c_coeffs);

        let ab_c = a.geometric_product(&b).geometric_product(&c);
        let a_bc = a.geometric_product(&b.geometric_product(&c));

        for i in 0..8 {
            prop_assert!((ab_c.get(i) - a_bc.get(i)).abs() < 1e-8);
        }
    }

    /// Test multiplicative identity: a * 1 = a
    #[test]
    fn prop_multiplicative_identity(a_coeffs in multivector_coefficients()) {
        let a = create_multivector(a_coeffs);
        let one = Multivector::scalar(1.0);
        let result = a.geometric_product(&one);

        for i in 0..8 {
            prop_assert!((a.get(i) - result.get(i)).abs() < 1e-10);
        }
    }

    /// Test scalar multiplication commutativity: s * a = a * s (for scalars)
    #[test]
    fn prop_scalar_multiplication_commutative(
        a_coeffs in multivector_coefficients(),
        scalar in non_zero_scalar()
    ) {
        let a = create_multivector(a_coeffs);
        let s = Multivector::scalar(scalar);

        let sa = s.geometric_product(&a);
        let as_result = a.geometric_product(&s);

        for i in 0..8 {
            prop_assert!((sa.get(i) - as_result.get(i)).abs() < 1e-10);
        }
    }

    /// Test distributivity: a * (b + c) = a * b + a * c
    #[test]
    fn prop_left_distributivity(
        a_coeffs in multivector_coefficients(),
        b_coeffs in multivector_coefficients(),
        c_coeffs in multivector_coefficients()
    ) {
        let a = create_multivector(a_coeffs);
        let b = create_multivector(b_coeffs);
        let c = create_multivector(c_coeffs);

        let a_bc = a.geometric_product(&(&b + &c));
        let ab_ac = &a.geometric_product(&b) + &a.geometric_product(&c);

        for i in 0..8 {
            prop_assert!((a_bc.get(i) - ab_ac.get(i)).abs() < 1e-8);
        }
    }

    /// Test basis vector squares: e_i^2 = +1 for Euclidean signature
    #[test]
    fn prop_basis_vector_squares(i in basis_index()) {
        let ei = Multivector::<3, 0, 0>::basis_vector(i);
        let ei_squared = ei.geometric_product(&ei);

        // Should be scalar 1
        prop_assert!((ei_squared.scalar_part() - 1.0).abs() < 1e-10);

        // All other components should be zero
        for j in 1..8 {
            prop_assert!(ei_squared.get(j).abs() < 1e-10);
        }
    }

    /// Test anticommutativity of distinct basis vectors: e_i * e_j = -e_j * e_i
    #[test]
    fn prop_basis_anticommutativity(i in basis_index(), j in basis_index()) {
        prop_assume!(i != j);

        let ei = Multivector::<3, 0, 0>::basis_vector(i);
        let ej = Multivector::<3, 0, 0>::basis_vector(j);

        let eij = ei.geometric_product(&ej);
        let eji = ej.geometric_product(&ei);

        // Should be negatives of each other
        for k in 0..8 {
            prop_assert!((eij.get(k) + eji.get(k)).abs() < 1e-10);
        }
    }

    /// Test outer product antisymmetry: a ∧ b = -(b ∧ a) for vectors
    #[test]
    fn prop_outer_product_antisymmetric(i in basis_index(), j in basis_index()) {
        prop_assume!(i != j);

        let ei = Multivector::<3, 0, 0>::basis_vector(i);
        let ej = Multivector::<3, 0, 0>::basis_vector(j);

        let ei_ej = ei.outer_product(&ej);
        let ej_ei = ej.outer_product(&ei);

        for k in 0..8 {
            prop_assert!((ei_ej.get(k) + ej_ei.get(k)).abs() < 1e-10);
        }
    }

    /// Test that outer product with self is zero: a ∧ a = 0
    #[test]
    fn prop_outer_product_self_zero(i in basis_index()) {
        let ei = Multivector::<3, 0, 0>::basis_vector(i);
        let ei_ei = ei.outer_product(&ei);

        prop_assert!(ei_ei.is_zero());
    }

    /// Test reverse operation properties: (ab)† = b†a†
    #[test]
    fn prop_reverse_antiautomorphism(
        a_coeffs in multivector_coefficients(),
        b_coeffs in multivector_coefficients()
    ) {
        let a = create_multivector(a_coeffs);
        let b = create_multivector(b_coeffs);

        let ab = a.geometric_product(&b);
        let ab_rev = ab.reverse();

        let a_rev = a.reverse();
        let b_rev = b.reverse();
        let ba_rev = b_rev.geometric_product(&a_rev);

        for i in 0..8 {
            prop_assert!((ab_rev.get(i) - ba_rev.get(i)).abs() < 1e-8);
        }
    }

    /// Test norm properties: ||a||² = a * ã (where ã is reverse)
    #[test]
    fn prop_norm_via_reverse(a_coeffs in multivector_coefficients()) {
        let a = create_multivector(a_coeffs);
        let a_rev = a.reverse();
        let norm_squared_mv = a.geometric_product(&a_rev);

        let norm_squared_direct = a.norm_squared();
        let norm_squared_via_reverse = norm_squared_mv.scalar_part();

        prop_assert!((norm_squared_direct - norm_squared_via_reverse).abs() < 1e-8);
    }

    /// Test that rotors preserve norms
    #[test]
    fn prop_rotor_preserves_norm(
        vector_coeffs in prop::collection::vec(-10.0..10.0, 3..=3), // 3D vector
        angle in -consts::PI..consts::PI
    ) {
        // Create a vector from coefficients
        let v = Multivector::<3, 0, 0>::from_vector(&Vector::from_components(
            vector_coeffs[0], vector_coeffs[1], vector_coeffs[2]
        ));

        // Create a simple rotation in e1-e2 plane
        let e1 = Multivector::<3, 0, 0>::basis_vector(0);
        let e2 = Multivector::<3, 0, 0>::basis_vector(1);
        let bivector = e1.outer_product(&e2) * (angle / 2.0);
        let rotor = bivector.exp();

        // Apply rotation
        let rotated = rotor.geometric_product(&v).geometric_product(&rotor.reverse());

        // Norms should be equal
        let original_norm = v.coefficient_norm();
        let rotated_norm = rotated.coefficient_norm();

        prop_assert!((original_norm - rotated_norm).abs() < 1e-6);
    }
}

#[cfg(test)]
mod unit_property_tests {
    use super::*;

    #[test]
    fn test_property_framework() {
        // Simple smoke test to ensure property test framework works
        let a = Multivector::<3, 0, 0>::scalar(2.0);
        let b = Multivector::<3, 0, 0>::scalar(3.0);
        let sum = &a + &b;
        assert_eq!(sum.scalar_part(), 5.0);
    }
}
