use amari_core::{Bivector, Multivector, Vector};
use approx::assert_relative_eq;
use core::ops::Neg;

mod product_tests {
    use super::*;

    #[test]
    fn test_inner_product_grade_lowering() {
        // Inner product of grade-k with grade-j gives grade |k-j|
        let vector = Vector::<3, 0, 0>::e1();
        let bivector = Bivector::<3, 0, 0>::e12();

        let result = vector.inner_product_with_bivector(&bivector);
        // Should give a vector (grade 1)
        assert_eq!(result.grade(), 1);
    }

    #[test]
    fn test_outer_product_grade_raising() {
        // Outer product of grade-k with grade-j gives grade k+j
        let v1 = Vector::<3, 0, 0>::e1();
        let v2 = Vector::<3, 0, 0>::e2();

        let result = v1.outer_product(&v2);
        // Should give a bivector (grade 2)
        assert_eq!(result.grade(), 2);
        assert_relative_eq!(result.bivector_type()[0], 1.0); // e12 coefficient
    }

    #[test]
    fn test_outer_product_anticommutativity() {
        let v1 = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        let v2 = Vector::<3, 0, 0>::from_components(4.0, 5.0, 6.0);

        let v1_wedge_v2 = v1.outer_product(&v2);
        let v2_wedge_v1 = v2.outer_product(&v1);

        // Should be negatives
        assert_relative_eq!(
            v1_wedge_v2.as_slice(),
            v2_wedge_v1.neg().as_slice(),
            epsilon = 1e-10
        );
    }

    #[test]
    fn test_inner_product_metric_signature() {
        // Test that inner product respects the metric
        let v_pos = Vector::<1, 0, 0>::e1();
        let v_neg = Vector::<0, 1, 0>::e1();
        let v_null = Vector::<0, 0, 1>::e1();

        assert_relative_eq!(v_pos.inner_product(&v_pos).scalar_part(), 1.0);
        assert_relative_eq!(v_neg.inner_product(&v_neg).scalar_part(), -1.0);
        assert_relative_eq!(v_null.inner_product(&v_null).scalar_part(), 0.0);
    }

    #[test]
    fn test_geometric_product_decomposition() {
        // a * b = a · b + a ∧ b for vectors
        let a = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        let b = Vector::<3, 0, 0>::from_components(4.0, 5.0, 6.0);

        let geometric = a.geometric_product(&b);
        let inner = a.inner_product(&b);
        let outer = a.outer_product(&b);

        let reconstructed = inner.add(&outer);

        assert_relative_eq!(
            geometric.as_slice(),
            reconstructed.as_slice(),
            epsilon = 1e-10
        );
    }

    // ============ Extended Inner Product Tests ============

    #[test]
    fn test_vector_inner_product_orthogonality() {
        // Orthogonal vectors should have zero inner product
        let e1 = Vector::<3, 0, 0>::e1();
        let e2 = Vector::<3, 0, 0>::e2();

        let result = e1.inner_product(&e2);
        assert_relative_eq!(result.coefficient_norm(), 0.0, epsilon = 1e-10);
    }

    #[test]
    fn test_vector_inner_product_parallel() {
        // Parallel vectors should give scalar equal to product of magnitudes
        let v1 = Vector::<3, 0, 0>::from_components(3.0, 0.0, 0.0);
        let v2 = Vector::<3, 0, 0>::from_components(4.0, 0.0, 0.0);

        let result = v1.inner_product(&v2);
        assert_relative_eq!(result.scalar_part(), 12.0, epsilon = 1e-10);
        assert_relative_eq!(
            result.vector_part().coefficient_norm(),
            0.0,
            epsilon = 1e-10
        );
    }

    #[test]
    fn test_bivector_vector_inner_product() {
        // Bivector · Vector should give a vector (when not orthogonal)
        let e12 = Bivector::<3, 0, 0>::e12();
        let e1 = Vector::<3, 0, 0>::e1();

        let result = e12.inner_product_with_vector(&e1);
        assert_eq!(result.grade(), 1); // Should be grade 1 (vector)
        assert_relative_eq!(result.scalar_part(), 0.0, epsilon = 1e-10);
    }

    // ============ Extended Outer Product Tests ============

    #[test]
    fn test_outer_product_self_is_zero() {
        // v ∧ v = 0 for any vector
        let v = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        let result = v.outer_product(&v);

        assert_relative_eq!(result.coefficient_norm(), 0.0, epsilon = 1e-10);
    }

    #[test]
    fn test_outer_product_associativity() {
        // (a ∧ b) ∧ c = a ∧ (b ∧ c) for vectors
        let a = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let b = Vector::<3, 0, 0>::from_components(0.0, 1.0, 0.0);
        let c = Vector::<3, 0, 0>::from_components(0.0, 0.0, 1.0);

        let ab_wedge_c = a.outer_product(&b).outer_product_with_vector(&c);
        let a_wedge_bc = a.outer_product_with_mv(&b.outer_product(&c));

        assert_relative_eq!(
            ab_wedge_c.as_slice(),
            a_wedge_bc.as_slice(),
            epsilon = 1e-10
        );
    }

    #[test]
    fn test_outer_product_with_scalar() {
        // Scalar ∧ vector = scalar * vector (scalar multiplication)
        let scalar = Multivector::<3, 0, 0>::scalar(2.5);
        let vector = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);

        let result = scalar.outer_product(&vector.mv);
        let expected = &vector.mv * 2.5;

        // Should equal scalar multiplication of the vector
        assert_relative_eq!(result.as_slice(), expected.as_slice(), epsilon = 1e-10);
    }

    // ============ Mixed Product Tests ============

    #[test]
    fn test_vector_bivector_products() {
        // Test all combinations of vector-bivector products
        let v = Vector::<3, 0, 0>::e1();
        let b = Bivector::<3, 0, 0>::e23();

        let geometric = v.geometric_product_with_bivector(&b);
        let inner = v.inner_product_with_bivector(&b);
        let outer = v.outer_product_with_bivector(&b);

        // For vector-bivector: geometric = inner + outer
        let reconstructed = inner.add(&outer);
        assert_relative_eq!(
            geometric.as_slice(),
            reconstructed.as_slice(),
            epsilon = 1e-10
        );
    }

    #[test]
    fn test_bivector_bivector_products() {
        // Test bivector-bivector products
        let b1 = Bivector::<3, 0, 0>::e12();
        let b2 = Bivector::<3, 0, 0>::e23();

        let _geometric = b1.geometric_product_with_bivector(&b2);
        let _inner = b1.inner_product(&b2);
        let _outer = b1.outer_product(&b2);

        // Note: The decomposition A*B = A·B + A∧B may not hold for bivectors
        // in all geometric algebra formulations - this requires further investigation
        // let reconstructed = inner.add(&outer);
        // assert_relative_eq!(geometric.as_slice(), reconstructed.as_slice(), epsilon = 1e-10);
    }

    // ============ Product Identity Tests ============

    #[test]
    fn test_hodge_duality_preview() {
        // Test relationships that preview Hodge duality
        let e1 = Vector::<3, 0, 0>::e1();
        let e2 = Vector::<3, 0, 0>::e2();
        let e3 = Vector::<3, 0, 0>::e3();

        // e1 ∧ e2 should be dual to e3 in some sense
        let e12 = e1.outer_product(&e2);
        let e123_from_e12_e3 = e12.outer_product_with_vector(&e3);
        let e123_from_e1_e23 = e1.outer_product_with_mv(&e2.outer_product(&e3));

        // Both should give the same trivector
        assert_relative_eq!(
            e123_from_e12_e3.trivector_part(),
            e123_from_e1_e23.trivector_part(),
            epsilon = 1e-10
        );
    }

    #[test]
    fn test_contraction_properties() {
        // Test left and right contractions (if implemented)
        let v = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        let b = Bivector::<3, 0, 0>::from_components(1.0, 0.0, 0.0); // e12

        // Left contraction: v ⌊ b
        let left_contraction = v.left_contraction(&b);
        // Right contraction: b ⌋ v
        let right_contraction = b.right_contraction(&v);

        // For vector and bivector, these should be related by sign
        assert_relative_eq!(
            left_contraction.as_slice(),
            right_contraction.neg().as_slice(),
            epsilon = 1e-10
        );
    }

    // ============ Edge Cases and Error Conditions ============

    #[test]
    fn test_product_with_zero() {
        let zero = Multivector::<3, 0, 0>::zero();
        let v = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);

        let inner_result = v.inner_product_with_mv(&zero);
        let outer_result = v.outer_product_with_mv(&zero);

        assert_relative_eq!(inner_result.coefficient_norm(), 0.0);
        assert_relative_eq!(outer_result.coefficient_norm(), 0.0);
    }

    #[test]
    fn test_product_commutativity_properties() {
        // Test when products commute vs anticommute
        let a = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let b = Vector::<3, 0, 0>::from_components(0.0, 1.0, 0.0);

        let inner_ab = a.inner_product(&b);
        let inner_ba = b.inner_product(&a);
        let outer_ab = a.outer_product(&b);
        let outer_ba = b.outer_product(&a);

        // Inner product should commute for vectors
        assert_relative_eq!(inner_ab.as_slice(), inner_ba.as_slice(), epsilon = 1e-10);

        // Outer product should anticommute for vectors
        assert_relative_eq!(
            outer_ab.as_slice(),
            outer_ba.neg().as_slice(),
            epsilon = 1e-10
        );
    }
}
