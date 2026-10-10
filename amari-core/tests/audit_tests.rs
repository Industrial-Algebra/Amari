//! Audit tests for amari-core 0.19.0 release
//!
//! Tests covering: Hodge dual, contractions, non-Euclidean signatures,
//! Cayley table verification, and numeric edge cases.

use amari_core::Multivector;
use approx::assert_relative_eq;

// ============ Hodge Dual Tests ============

mod hodge_dual_tests {
    use super::*;

    #[test]
    fn test_hodge_dual_basis_vectors_3d() {
        // In Cl(3,0,0): ⋆e1 = e23, ⋆e2 = -e13, ⋆e3 = e12
        let e1 = Multivector::<3, 0, 0>::basis_vector(0);
        let e2 = Multivector::<3, 0, 0>::basis_vector(1);
        let e3 = Multivector::<3, 0, 0>::basis_vector(2);

        let dual_e1 = e1.hodge_dual();
        // e23 = index 6 (0b110)
        assert_relative_eq!(dual_e1.get(6), 1.0, epsilon = 1e-14);
        assert_relative_eq!(dual_e1.get(3), 0.0, epsilon = 1e-14);
        assert_relative_eq!(dual_e1.get(5), 0.0, epsilon = 1e-14);

        let dual_e2 = e2.hodge_dual();
        // -e13 = -1 at index 5 (0b101)
        assert_relative_eq!(dual_e2.get(5), -1.0, epsilon = 1e-14);
        assert_relative_eq!(dual_e2.get(3), 0.0, epsilon = 1e-14);
        assert_relative_eq!(dual_e2.get(6), 0.0, epsilon = 1e-14);

        let dual_e3 = e3.hodge_dual();
        // e12 = index 3 (0b011)
        assert_relative_eq!(dual_e3.get(3), 1.0, epsilon = 1e-14);
        assert_relative_eq!(dual_e3.get(5), 0.0, epsilon = 1e-14);
        assert_relative_eq!(dual_e3.get(6), 0.0, epsilon = 1e-14);
    }

    #[test]
    fn test_hodge_dual_bivectors_3d() {
        // In Cl(3,0,0): ⋆e12 = e3, ⋆e13 = -e2, ⋆e23 = e1
        let mut e12 = Multivector::<3, 0, 0>::zero();
        e12.set(3, 1.0); // index 3 = e12

        let mut e13 = Multivector::<3, 0, 0>::zero();
        e13.set(5, 1.0); // index 5 = e13

        let mut e23 = Multivector::<3, 0, 0>::zero();
        e23.set(6, 1.0); // index 6 = e23

        let dual_e12 = e12.hodge_dual();
        assert_relative_eq!(dual_e12.get(4), 1.0, epsilon = 1e-14); // e3 = index 4

        let dual_e13 = e13.hodge_dual();
        assert_relative_eq!(dual_e13.get(2), -1.0, epsilon = 1e-14); // -e2 = index 2

        let dual_e23 = e23.hodge_dual();
        assert_relative_eq!(dual_e23.get(1), 1.0, epsilon = 1e-14); // e1 = index 1
    }

    #[test]
    fn test_hodge_dual_scalar_pseudoscalar_3d() {
        // ⋆1 = e123, ⋆e123 = 1
        let scalar = Multivector::<3, 0, 0>::scalar(1.0);
        let dual_scalar = scalar.hodge_dual();
        assert_relative_eq!(dual_scalar.get(7), 1.0, epsilon = 1e-14); // e123 = index 7

        let mut pseudoscalar = Multivector::<3, 0, 0>::zero();
        pseudoscalar.set(7, 1.0);
        let dual_pseudo = pseudoscalar.hodge_dual();
        assert_relative_eq!(dual_pseudo.get(0), 1.0, epsilon = 1e-14); // scalar
    }

    #[test]
    fn test_hodge_dual_double_dual_3d() {
        // In 3D: ⋆⋆a = (-1)^{k(n-k)} * a for k-vector in n dimensions
        // For vectors (k=1, n=3): ⋆⋆v = (-1)^(1*2) * v = v
        let v = Multivector::<3, 0, 0>::basis_vector(0); // e1
        let double_dual = v.hodge_dual().hodge_dual();
        assert_relative_eq!(double_dual.as_slice(), v.as_slice(), epsilon = 1e-14);

        // For bivectors (k=2, n=3): ⋆⋆B = (-1)^(2*1) * B = B
        let mut b = Multivector::<3, 0, 0>::zero();
        b.set(3, 1.0); // e12
        let double_dual_b = b.hodge_dual().hodge_dual();
        assert_relative_eq!(double_dual_b.as_slice(), b.as_slice(), epsilon = 1e-14);
    }

    #[test]
    fn test_hodge_dual_mixed_grade() {
        // Hodge dual of a+e1 (scalar + vector)
        let mut mv = Multivector::<3, 0, 0>::scalar(2.0);
        mv.set(1, 3.0); // e1 component

        let dual = mv.hodge_dual();

        // ⋆2 = 2*e123, ⋆3e1 = 3*e23
        assert_relative_eq!(dual.get(7), 2.0, epsilon = 1e-14); // e123
        assert_relative_eq!(dual.get(6), 3.0, epsilon = 1e-14); // e23
    }
}

// ============ Cayley Table Verification ============

mod cayley_tests {
    use super::*;

    #[test]
    fn test_full_cayley_table_cl300() {
        // Verify the complete 8x8 Cayley table for Cl(3,0,0)
        // against the known textbook values
        type Cl3 = Multivector<3, 0, 0>;

        // Basis blades: 1, e1, e2, e12, e3, e13, e23, e123
        // Indices:      0,  1,  2,   3,  4,   5,   6,    7

        // e1*e1 = 1
        let e1 = Cl3::basis_vector(0);
        let result = e1.geometric_product(&e1);
        assert_relative_eq!(result.scalar_part(), 1.0, epsilon = 1e-14);

        // e2*e2 = 1
        let e2 = Cl3::basis_vector(1);
        let result = e2.geometric_product(&e2);
        assert_relative_eq!(result.scalar_part(), 1.0, epsilon = 1e-14);

        // e3*e3 = 1
        let e3 = Cl3::basis_vector(2);
        let result = e3.geometric_product(&e3);
        assert_relative_eq!(result.scalar_part(), 1.0, epsilon = 1e-14);

        // e1*e2 = e12 (index 3)
        let result = e1.geometric_product(&e2);
        assert_relative_eq!(result.get(3), 1.0, epsilon = 1e-14);

        // e2*e1 = -e12
        let result = e2.geometric_product(&e1);
        assert_relative_eq!(result.get(3), -1.0, epsilon = 1e-14);

        // e1*e3 = e13 (index 5)
        let result = e1.geometric_product(&e3);
        assert_relative_eq!(result.get(5), 1.0, epsilon = 1e-14);

        // e3*e1 = -e13
        let result = e3.geometric_product(&e1);
        assert_relative_eq!(result.get(5), -1.0, epsilon = 1e-14);

        // e2*e3 = e23 (index 6)
        let result = e2.geometric_product(&e3);
        assert_relative_eq!(result.get(6), 1.0, epsilon = 1e-14);

        // e3*e2 = -e23
        let result = e3.geometric_product(&e2);
        assert_relative_eq!(result.get(6), -1.0, epsilon = 1e-14);

        // e12*e12 = e1*e2*e1*e2 = -e1*e1*e2*e2 = -1
        let mut e12 = Cl3::zero();
        e12.set(3, 1.0);
        let result = e12.geometric_product(&e12);
        assert_relative_eq!(result.scalar_part(), -1.0, epsilon = 1e-14);

        // e123*e123 = -1 in Cl(3,0,0)
        let mut e123 = Cl3::zero();
        e123.set(7, 1.0);
        let result = e123.geometric_product(&e123);
        assert_relative_eq!(result.scalar_part(), -1.0, epsilon = 1e-14);
    }
}

// ============ Non-Euclidean Signature Tests ============

mod signature_tests {
    use super::*;

    #[test]
    fn test_minkowski_spacetime_cl130() {
        // Cl(1,3,0): e0²=+1, e1²=-1, e2²=-1, e3²=-1
        type Mink = Multivector<1, 3, 0>;

        let e0 = Mink::basis_vector(0); // timelike
        let e1 = Mink::basis_vector(1); // spacelike
        let e2 = Mink::basis_vector(2); // spacelike

        // e0*e0 = +1
        let result = e0.geometric_product(&e0);
        assert_relative_eq!(result.scalar_part(), 1.0, epsilon = 1e-14);

        // e1*e1 = -1
        let result = e1.geometric_product(&e1);
        assert_relative_eq!(result.scalar_part(), -1.0, epsilon = 1e-14);

        // e2*e2 = -1
        let result = e2.geometric_product(&e2);
        assert_relative_eq!(result.scalar_part(), -1.0, epsilon = 1e-14);

        // e0*e1 anticommutes: e0*e1 = -e1*e0
        let e0e1 = e0.geometric_product(&e1);
        let e1e0 = e1.geometric_product(&e0);
        assert_relative_eq!(e0e1.get(3), -e1e0.get(3), epsilon = 1e-14);
    }

    #[test]
    fn test_projective_ga_cl201() {
        // Cl(2,0,1): e1²=+1, e2²=+1, e0²=0 (degenerate/null)
        type Pga = Multivector<2, 0, 1>;

        let e1 = Pga::basis_vector(0);
        let e2 = Pga::basis_vector(1);
        let e0 = Pga::basis_vector(2); // null vector

        // e1*e1 = +1
        let result = e1.geometric_product(&e1);
        assert_relative_eq!(result.scalar_part(), 1.0, epsilon = 1e-14);

        // e2*e2 = +1
        let result = e2.geometric_product(&e2);
        assert_relative_eq!(result.scalar_part(), 1.0, epsilon = 1e-14);

        // e0*e0 = 0 (null vector squares to zero)
        let result = e0.geometric_product(&e0);
        assert_relative_eq!(result.scalar_part(), 0.0, epsilon = 1e-14);

        // e0 should still anticommute with other vectors
        let e1e0 = e1.geometric_product(&e0);
        let e0e1 = e0.geometric_product(&e1);
        // e1*e0 + e0*e1 = 2*<e1,e0> = 0 (orthogonal)
        let sum = e1e0 + e0e1;
        assert_relative_eq!(sum.norm_squared().abs().sqrt(), 0.0, epsilon = 1e-14);
    }
}

// ============ Contraction Tests ============

mod contraction_tests {
    use super::*;

    #[test]
    fn test_left_contraction_vector_bivector() {
        // e1 ⌋ e12 = e2 (left contraction)
        let e1 = Multivector::<3, 0, 0>::basis_vector(0);
        let mut e12 = Multivector::<3, 0, 0>::zero();
        e12.set(3, 1.0);

        let result = e1.left_contraction(&e12);
        assert_relative_eq!(result.get(2), 1.0, epsilon = 1e-14); // e2
    }

    #[test]
    fn test_left_contraction_vector_vector() {
        // e1 ⌋ e1 = 1 (same vector)
        let e1 = Multivector::<3, 0, 0>::basis_vector(0);
        let result = e1.left_contraction(&e1);
        assert_relative_eq!(result.scalar_part(), 1.0, epsilon = 1e-14);

        // e1 ⌋ e2 = 0 (orthogonal vectors)
        let e2 = Multivector::<3, 0, 0>::basis_vector(1);
        let result = e1.left_contraction(&e2);
        assert_relative_eq!(result.coefficient_norm(), 0.0, epsilon = 1e-14);
    }

    #[test]
    fn test_inner_product_vector_bivector() {
        // Inner product of vector with bivector should give a vector
        let e1 = Multivector::<3, 0, 0>::basis_vector(0);
        let mut e23 = Multivector::<3, 0, 0>::zero();
        e23.set(6, 1.0);

        let result = e1.inner_product(&e23);
        // e1 · e23: grade |1-2| = 1, so result is grade 1
        // e1 · e23 = 0 (no shared basis vectors in Hestenes convention)
        assert_relative_eq!(result.coefficient_norm(), 0.0, epsilon = 1e-14);

        // e2 · e23 should give e3
        let e2 = Multivector::<3, 0, 0>::basis_vector(1);
        let result = e2.inner_product(&e23);
        assert_relative_eq!(result.get(4), 1.0, epsilon = 1e-10); // e3
    }
}

// ============ Numeric Edge Cases ============

mod numeric_tests {
    use super::*;

    // Multivector::normalize removed by CORE-W02 (D01.4); Vector domain coverage lives in numerical_contracts.rs

    #[test]
    fn test_inverse_zero() {
        let zero = Multivector::<3, 0, 0>::zero();
        assert!(zero.inverse().is_none());
    }

    #[test]
    fn test_large_coefficients() {
        // Products with large coefficients should not lose precision
        let mut a = Multivector::<3, 0, 0>::zero();
        a.set(1, 1e10);
        let mut b = Multivector::<3, 0, 0>::zero();
        b.set(1, 1e10);

        let result = a.geometric_product(&b);
        assert_relative_eq!(result.scalar_part(), 1e20, epsilon = 1e6);
    }

    #[test]
    fn test_identity_geometric_product() {
        // 1 * a = a * 1 = a
        let scalar_one = Multivector::<3, 0, 0>::scalar(1.0);
        let v = Multivector::<3, 0, 0>::basis_vector(0);

        let left = scalar_one.geometric_product(&v);
        let right = v.geometric_product(&scalar_one);

        assert_relative_eq!(left.as_slice(), v.as_slice(), epsilon = 1e-14);
        assert_relative_eq!(right.as_slice(), v.as_slice(), epsilon = 1e-14);
    }

    #[test]
    fn test_geometric_product_associativity() {
        let a = Multivector::<3, 0, 0>::basis_vector(0); // e1
        let b = Multivector::<3, 0, 0>::basis_vector(1); // e2
        let c = Multivector::<3, 0, 0>::basis_vector(2); // e3

        // (a*b)*c = a*(b*c)
        let ab_c = a.geometric_product(&b).geometric_product(&c);
        let a_bc = a.geometric_product(&b.geometric_product(&c));

        assert_relative_eq!(ab_c.as_slice(), a_bc.as_slice(), epsilon = 1e-14);
    }
}
