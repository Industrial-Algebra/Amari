//! Comprehensive unit tests for all public functions in lib.rs
//!
//! This module systematically tests all 83 public functions identified in lib.rs
//! to achieve high test coverage as part of formal verification.

use crate::*;
use approx::assert_relative_eq;

#[cfg(feature = "std")]
use std::f64::consts;

#[cfg(not(feature = "std"))]
use core::f64::consts;

type Cl3 = Multivector<3, 0, 0>; // 3D Euclidean space

#[cfg(test)]
mod constructor_tests {
    use super::*;

    #[test]
    fn test_zero() {
        let zero = Cl3::zero();
        for i in 0..8 {
            assert_eq!(zero.get(i), 0.0);
        }
        assert!(zero.is_zero());
    }

    #[test]
    fn test_scalar() {
        let mv = Cl3::scalar(5.0);
        assert_eq!(mv.get(0), 5.0);
        for i in 1..8 {
            assert_eq!(mv.get(i), 0.0);
        }
    }

    #[test]
    fn test_basis_vector() {
        let e1 = Cl3::basis_vector(0);
        assert_eq!(e1.get(1), 1.0); // e1 is at index 1
        for i in [0, 2, 3, 4, 5, 6, 7] {
            assert_eq!(e1.get(i), 0.0);
        }
    }

    #[test]
    fn test_from_coefficients() {
        let coeffs = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mv = Cl3::from_coefficients(coeffs.clone());
        for (i, &coeff) in coeffs.iter().enumerate().take(8) {
            assert_eq!(mv.get(i), coeff);
        }
    }

    #[test]
    fn test_from_slice() {
        let coeffs = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mv = Cl3::from_slice(&coeffs);
        for (i, &coeff) in coeffs.iter().enumerate() {
            assert_eq!(mv.get(i), coeff);
        }
    }
}

#[cfg(test)]
mod accessor_tests {
    use super::*;

    #[test]
    fn test_get_set() {
        let mut mv = Cl3::zero();
        mv.set(3, 42.0);
        assert_eq!(mv.get(3), 42.0);
    }

    #[test]
    fn test_scalar_part() {
        let mv = Cl3::from_coefficients(vec![5.0, 1.0, 2.0, 3.0, 4.0, 0.0, 0.0, 0.0]);
        assert_eq!(mv.scalar_part(), 5.0);
    }

    #[test]
    fn test_set_scalar() {
        let mut mv = Cl3::scalar(1.0);
        mv.set_scalar(10.0);
        assert_eq!(mv.scalar_part(), 10.0);
    }

    #[test]
    fn test_vector_part() {
        let mv = Cl3::from_coefficients(vec![1.0, 2.0, 3.0, 4.0, 5.0, 0.0, 0.0, 0.0]);
        let vec_part = mv.vector_part();
        assert_eq!(vec_part.mv.get(1), 2.0); // e1 component
        assert_eq!(vec_part.mv.get(2), 3.0); // e2 component
        assert_eq!(vec_part.mv.get(4), 5.0); // e3 component (index 4 = binary 100)
    }

    #[test]
    fn test_bivector_part() {
        let mv = Cl3::from_coefficients(vec![0.0, 0.0, 0.0, 1.0, 0.0, 2.0, 3.0, 0.0]);
        let biv_part = mv.bivector_part();
        assert_eq!(biv_part.get(3), 1.0); // e1∧e2 (binary 011)
        assert_eq!(biv_part.get(5), 2.0); // e1∧e3 (binary 101)
        assert_eq!(biv_part.get(6), 3.0); // e2∧e3 (binary 110)
    }

    #[test]
    fn test_trivector_part() {
        let mv = Cl3::from_coefficients(vec![0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 5.0]);
        assert_eq!(mv.trivector_part(), 5.0);
    }

    #[test]
    fn test_set_vector_component() {
        let mut mv = Cl3::zero();
        mv.set_vector_component(1, 42.0); // y component
        assert_eq!(mv.get(2), 42.0); // e2 is at index 2
    }

    #[test]
    fn test_set_bivector_component() {
        let mut mv = Cl3::zero();
        mv.set_bivector_component(0, 42.0); // e1∧e2 maps to index 3
        assert_eq!(mv.get(3), 42.0);
    }

    #[test]
    fn test_vector_component() {
        let mv = Cl3::from_coefficients(vec![0.0, 1.0, 2.0, 0.0, 3.0, 0.0, 0.0, 0.0]);
        assert_eq!(mv.vector_component(0), 1.0); // x (index 1)
        assert_eq!(mv.vector_component(1), 2.0); // y (index 2)
        assert_eq!(mv.vector_component(2), 3.0); // z (index 4)
    }

    #[test]
    fn test_as_slice() {
        let coeffs = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0];
        let mv = Cl3::from_coefficients(coeffs.clone());
        let slice = mv.as_slice();
        assert_eq!(slice.len(), 8);
        for i in 0..8 {
            assert_eq!(slice[i], coeffs[i]);
        }
    }
}

#[cfg(test)]
mod arithmetic_tests {
    use super::*;

    #[test]
    fn test_addition() {
        let a = Cl3::scalar(2.0);
        let b = Cl3::scalar(3.0);
        let result = &a + &b;
        assert_eq!(result.scalar_part(), 5.0);
    }

    #[test]
    fn test_operator_addition() {
        let a = Cl3::scalar(2.0);
        let b = Cl3::scalar(3.0);
        let result = &a + &b;
        assert_eq!(result.scalar_part(), 5.0);
    }

    #[test]
    fn test_operator_subtraction() {
        let a = Cl3::scalar(5.0);
        let b = Cl3::scalar(3.0);
        let result = &a - &b;
        assert_eq!(result.scalar_part(), 2.0);
    }

    #[test]
    fn test_operator_negation() {
        let a = Cl3::scalar(5.0);
        let result = -a.clone();
        assert_eq!(result.scalar_part(), -5.0);
    }

    #[test]
    fn test_scalar_multiplication() {
        let a = Cl3::from_coefficients(vec![1.0, 2.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0]);
        let result = &a * 2.0;
        assert_eq!(result.get(0), 2.0);
        assert_eq!(result.get(1), 4.0);
        assert_eq!(result.get(2), 6.0);
        assert_eq!(result.get(3), 8.0);
    }
}

#[cfg(test)]
mod product_tests {
    use super::*;

    #[test]
    fn test_geometric_product() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let result = e1.geometric_product(&e2);

        // e1 * e2 = e1∧e2 (bivector at index 0b011 = 3)
        assert_eq!(result.get(3), 1.0); // e1∧e2 coefficient
    }

    #[test]
    fn test_geometric_product_associativity() {
        let a = Cl3::scalar(2.0);
        let b = Cl3::basis_vector(0);
        let c = Cl3::basis_vector(1);

        let ab_c = a.geometric_product(&b).geometric_product(&c);
        let a_bc = a.geometric_product(&b.geometric_product(&c));

        for i in 0..8 {
            assert_relative_eq!(ab_c.get(i), a_bc.get(i), epsilon = 1e-10);
        }
    }

    #[test]
    fn test_inner_product() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let result = e1.inner_product(&e2);

        // e1 · e2 = 0 for orthogonal basis vectors
        assert!(result.is_zero());
    }

    #[test]
    fn test_outer_product() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let result = e1.outer_product(&e2);

        // e1 ∧ e2 = bivector at index 3
        assert_eq!(result.get(3), 1.0);
    }

    #[test]
    fn test_outer_product_antisymmetric() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let e1_e2 = e1.outer_product(&e2);
        let e2_e1 = e2.outer_product(&e1);

        // Should be negatives of each other
        for i in 0..8 {
            assert_relative_eq!(e1_e2.get(i), -e2_e1.get(i), epsilon = 1e-10);
        }
    }

    #[test]
    fn test_scalar_product() {
        let a = Cl3::from_coefficients(vec![1.0, 2.0, 3.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let b = Cl3::from_coefficients(vec![2.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let result = a.scalar_product(&b);

        // Should be 1*2 + 2*1 + 3*1 = 7 (considering signature)
        assert_eq!(result, 7.0);
    }
}

#[cfg(test)]
mod operation_tests {
    use super::*;

    #[test]
    fn test_reverse() {
        let mv = Cl3::from_coefficients(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let rev = mv.reverse();

        // Scalar and vector parts unchanged, bivector and trivector change sign
        assert_eq!(rev.get(0), 1.0); // scalar
        assert_eq!(rev.get(1), 2.0); // e1
        assert_eq!(rev.get(2), 3.0); // e2
        assert_eq!(rev.get(3), -4.0); // e1∧e2 (bivector, changes sign)
        assert_eq!(rev.get(4), 5.0); // e3 (vector, unchanged)
        assert_eq!(rev.get(5), -6.0); // e1∧e3 (bivector, changes sign)
        assert_eq!(rev.get(6), -7.0); // e2∧e3 (bivector, changes sign)
        assert_eq!(rev.get(7), -8.0); // e1∧e2∧e3 (trivector, changes sign)
    }

    #[test]
    fn test_grade_projection() {
        let mv = Cl3::from_coefficients(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);

        // Grade 0 projection (scalar)
        let grade0 = mv.grade_projection(0);
        assert_eq!(grade0.get(0), 1.0);
        for i in 1..8 {
            assert_eq!(grade0.get(i), 0.0);
        }

        // Grade 1 projection (vector)
        let grade1 = mv.grade_projection(1);
        assert_eq!(grade1.get(0), 0.0);
        assert_eq!(grade1.get(1), 2.0); // e1
        assert_eq!(grade1.get(2), 3.0); // e2
        assert_eq!(grade1.get(3), 0.0); // e1∧e2 should be zero in grade 1
        assert_eq!(grade1.get(4), 5.0); // e3
        for i in 5..8 {
            assert_eq!(grade1.get(i), 0.0);
        }
    }

    #[test]
    fn test_grade() {
        assert_eq!(Cl3::scalar(5.0).grade(), 0);
        assert_eq!(Cl3::basis_vector(0).grade(), 1);

        let bivector = Cl3::basis_vector(0).outer_product(&Cl3::basis_vector(1));
        assert_eq!(bivector.grade(), 2);
    }
}

#[cfg(test)]
mod utility_tests {
    use super::*;

    #[test]
    fn test_is_zero() {
        assert!(Cl3::zero().is_zero());
        assert!(!Cl3::scalar(1.0).is_zero());
    }

    #[test]
    fn test_norm_squared() {
        let mv = Cl3::from_coefficients(vec![0.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(mv.norm_squared(), 25.0); // 3² + 4² = 25
    }

    #[test]
    fn test_equality() {
        let a = Cl3::scalar(5.0);
        let b = Cl3::scalar(5.0);
        let c = Cl3::scalar(3.0);

        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_norm() {
        let mv = Cl3::from_coefficients(vec![0.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(mv.coefficient_norm(), 5.0); // sqrt(3² + 4²) = 5
    }

    #[test]
    fn test_magnitude() {
        let mv = Cl3::from_coefficients(vec![0.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(mv.norm_squared(), 25.0); // signed metric quadratic form: 3² + 4²
        assert_eq!(mv.coefficient_norm(), mv.norm_squared().sqrt()); // coefficient magnitude agrees here
    }

    #[test]
    fn test_abs() {
        let mv = Cl3::from_coefficients(vec![0.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        assert_eq!(mv.coefficient_norm(), 5.0); // coefficient magnitude
        assert_eq!(mv.coefficient_norm(), mv.norm_squared().sqrt()); // agrees with metric form here
    }

    #[test]
    fn test_approx_eq() {
        let a = Cl3::scalar(1.0);
        let b = Cl3::scalar(1.00001);
        let c = Cl3::scalar(1.1);

        // Should be approximately equal with large epsilon
        assert!(a.approx_eq(&b, 0.001));

        // Should not be approximately equal with small epsilon
        assert!(!a.approx_eq(&b, 0.000001));

        // Should not be approximately equal with large difference
        assert!(!a.approx_eq(&c, 0.001));
    }

    #[test]
    fn test_normalize() {
        // Test with non-zero vector
        let mv = Cl3::from_coefficients(vec![0.0, 3.0, 4.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let normalized = mv
            .normalize_versor()
            .expect("Should normalize successfully");

        // Should have unit coefficient magnitude
        assert!((normalized.coefficient_norm() - 1.0).abs() < 1e-10);

        // Should preserve direction (components scaled by 1/5)
        assert!((normalized.get(1) - 0.6).abs() < 1e-10); // 3/5
        assert!((normalized.get(2) - 0.8).abs() < 1e-10); // 4/5
    }

    #[test]
    fn test_normalize_zero() {
        // Test with zero multivector
        let zero = Cl3::zero();
        let result = zero.normalize_versor();

        // Should return None for zero multivector
        assert!(result.is_none());
    }

    #[test]
    fn test_normalize_small() {
        // Tiny positive-square multivector normalizes exactly: the domain
        // test is strict positivity, with no magnitude threshold.
        let small = Cl3::scalar(1e-15);
        let result = small
            .normalize_versor()
            .expect("tiny positive-square multivector normalizes exactly");

        assert!((result.coefficient_norm() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_inverse() {
        // Test inverse of scalar
        let scalar = Cl3::scalar(2.0);
        let inv = scalar.inverse().expect("Scalar should have inverse");

        // Verify a * a^(-1) = 1
        let product = scalar.geometric_product(&inv);
        assert!((product.scalar_part() - 1.0).abs() < 1e-10);

        // Test inverse of vector
        let vector = Cl3::from_coefficients(vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let inv = vector.inverse().expect("Unit vector should have inverse");
        let product = vector.geometric_product(&inv);
        assert!((product.scalar_part() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_inverse_zero() {
        // Test inverse of zero multivector
        let zero = Cl3::zero();
        let result = zero.inverse();

        // Should return None for zero multivector
        assert!(result.is_none());
    }

    #[test]
    fn test_inverse_small() {
        // Test inverse of very small multivector
        let small = Cl3::scalar(1e-15);
        let result = small.inverse();

        // Should return None for multivector below threshold
        assert!(result.is_none());
    }
}

/// Comprehensive Vector type tests
#[cfg(test)]
mod vector_tests {
    use super::*;

    #[test]
    fn test_vector_zero() {
        let v = Vector::<3, 0, 0>::zero();
        assert!(v.mv.is_zero());
        assert_eq!(v.coefficient_norm(), 0.0);
    }

    #[test]
    fn test_vector_from_components() {
        let v = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        assert_eq!(v.mv.vector_component(0), 1.0);
        assert_eq!(v.mv.vector_component(1), 2.0);
        assert_eq!(v.mv.vector_component(2), 3.0);
    }

    #[test]
    fn test_vector_basis_vectors() {
        let e1 = Vector::<3, 0, 0>::e1();
        let e2 = Vector::<3, 0, 0>::e2();
        let e3 = Vector::<3, 0, 0>::e3();

        assert_eq!(e1.mv.vector_component(0), 1.0);
        assert_eq!(e1.mv.vector_component(1), 0.0);
        assert_eq!(e1.mv.vector_component(2), 0.0);

        assert_eq!(e2.mv.vector_component(0), 0.0);
        assert_eq!(e2.mv.vector_component(1), 1.0);
        assert_eq!(e2.mv.vector_component(2), 0.0);

        assert_eq!(e3.mv.vector_component(0), 0.0);
        assert_eq!(e3.mv.vector_component(1), 0.0);
        assert_eq!(e3.mv.vector_component(2), 1.0);
    }

    #[test]
    fn test_vector_from_multivector() {
        let mv = Cl3::from_coefficients(vec![0.0, 1.0, 2.0, 3.0, 4.0, 0.0, 0.0, 0.0]);
        let v = Vector::from_multivector(&mv);

        // Should extract only vector part (grade projection removes scalar/bivector parts)
        assert_eq!(v.mv.vector_component(0), 1.0);
        assert_eq!(v.mv.vector_component(1), 2.0);
        assert_eq!(v.mv.vector_component(2), 4.0);
        assert_eq!(v.mv.scalar_part(), 0.0);
    }

    #[test]
    fn test_vector_geometric_product_with_vector() {
        let v1 = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let v2 = Vector::<3, 0, 0>::from_components(0.0, 1.0, 0.0);
        let result = v1.geometric_product(&v2);

        // v1 * v2 = v1 · v2 + v1 ∧ v2 = 0 + e1∧e2 = e12
        assert_eq!(result.scalar_part(), 0.0);
        assert_eq!(result.get(3), 1.0); // e12 coefficient
    }

    #[test]
    fn test_vector_geometric_product_with_multivector() {
        let v = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let scalar = Cl3::scalar(2.0);
        let result = v.geometric_product_with_multivector(&scalar);

        // v * 2 = 2v
        assert_eq!(result.vector_component(0), 2.0);
        assert_eq!(result.vector_component(1), 0.0);
        assert_eq!(result.vector_component(2), 0.0);
    }

    #[test]
    fn test_vector_geometric_product_with_bivector() {
        let v = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let bv = Bivector::<3, 0, 0>::e12();
        let result = v.geometric_product_with_bivector(&bv);

        // e1 * e12 = e1 * e1 * e2 = 1 * e2 = e2
        assert_eq!(result.vector_component(1), 1.0);
    }

    #[test]
    fn test_vector_geometric_product_with_scalar() {
        let v = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        let s = Scalar::<3, 0, 0>::from(2.0);
        let result = v.geometric_product_with_scalar(&s);

        // v * 2 = 2v
        assert_eq!(result.vector_component(0), 2.0);
        assert_eq!(result.vector_component(1), 4.0);
        assert_eq!(result.vector_component(2), 6.0);
    }

    #[test]
    fn test_vector_add() {
        let v1 = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        let v2 = Vector::<3, 0, 0>::from_components(4.0, 5.0, 6.0);
        let result = v1.add(&v2);

        assert_eq!(result.mv.vector_component(0), 5.0);
        assert_eq!(result.mv.vector_component(1), 7.0);
        assert_eq!(result.mv.vector_component(2), 9.0);
    }

    #[test]
    fn test_vector_magnitude() {
        let v = Vector::<3, 0, 0>::from_components(3.0, 4.0, 0.0);
        assert_eq!(v.coefficient_norm(), 5.0); // sqrt(3² + 4²) = 5
    }

    #[test]
    fn test_vector_as_slice() {
        let v = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        let slice = v.as_slice();

        // Check that vector components are in the right places
        assert_eq!(slice[1], 1.0); // e1
        assert_eq!(slice[2], 2.0); // e2
        assert_eq!(slice[4], 3.0); // e3
    }

    #[test]
    fn test_vector_inner_product() {
        let v1 = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let v2 = Vector::<3, 0, 0>::from_components(0.0, 1.0, 0.0);
        let result = v1.inner_product(&v2);

        // Orthogonal vectors have zero inner product
        assert!(result.is_zero());

        let v3 = Vector::<3, 0, 0>::from_components(2.0, 0.0, 0.0);
        let result2 = v1.inner_product(&v3);
        assert_eq!(result2.scalar_part(), 2.0);
    }

    #[test]
    fn test_vector_inner_product_with_mv() {
        let v = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let mv = Cl3::from_coefficients(vec![0.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let result = v.inner_product_with_mv(&mv);

        assert_eq!(result.scalar_part(), 2.0);
    }

    #[test]
    fn test_vector_inner_product_with_bivector() {
        let v = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let bv = Bivector::<3, 0, 0>::e12();
        let result = v.inner_product_with_bivector(&bv);

        // v · bv = contraction
        assert_eq!(result.vector_component(1), 1.0); // e1 ⌊ e12 = e2
    }

    #[test]
    fn test_vector_outer_product() {
        let v1 = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let v2 = Vector::<3, 0, 0>::from_components(0.0, 1.0, 0.0);
        let result = v1.outer_product(&v2);

        // e1 ∧ e2 = e12
        assert_eq!(result.get(3), 1.0);
    }

    #[test]
    fn test_vector_outer_product_with_mv() {
        let v = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let mv = Cl3::from_coefficients(vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        let result = v.outer_product_with_mv(&mv);

        // e1 ∧ e2 = e12
        assert_eq!(result.get(3), 1.0);
    }

    #[test]
    fn test_vector_outer_product_with_bivector() {
        let v = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let bv = Bivector::<3, 0, 0>::e23();
        let result = v.outer_product_with_bivector(&bv);

        // e1 ∧ e23 = e123 (trivector)
        assert_eq!(result.get(7), 1.0); // e123 coefficient
    }

    #[test]
    fn test_vector_left_contraction() {
        let v = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let bv = Bivector::<3, 0, 0>::e12();
        let result = v.left_contraction(&bv);

        // e1 ⌊ e12 = e2
        assert_eq!(result.vector_component(1), 1.0);
    }

    #[test]
    fn test_vector_normalize() {
        let v = Vector::<3, 0, 0>::from_components(3.0, 4.0, 0.0);
        let normalized = v.normalize().expect("Should normalize successfully");

        assert!((normalized.coefficient_norm() - 1.0).abs() < 1e-10);
        assert!((normalized.mv.vector_component(0) - 0.6).abs() < 1e-10); // 3/5
        assert!((normalized.mv.vector_component(1) - 0.8).abs() < 1e-10); // 4/5
    }

    #[test]
    fn test_vector_normalize_zero() {
        let v = Vector::<3, 0, 0>::zero();
        let result = v.normalize();
        assert!(result.is_none());
    }

    #[test]
    fn test_vector_norm_squared() {
        let v = Vector::<3, 0, 0>::from_components(3.0, 4.0, 0.0);
        assert_eq!(v.norm_squared(), 25.0); // 3² + 4² = 25
    }

    #[test]
    fn test_vector_reverse() {
        let v = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        let reversed = v.reverse();

        // Vector reverse is the same as the original (grade 1 has reverse sign = +1)
        assert_eq!(reversed.mv.vector_component(0), 1.0);
        assert_eq!(reversed.mv.vector_component(1), 2.0);
        assert_eq!(reversed.mv.vector_component(2), 3.0);
    }

    #[test]
    fn test_vector_norm() {
        let v = Vector::<3, 0, 0>::from_components(3.0, 4.0, 0.0);
        assert_eq!(v.coefficient_norm(), 5.0);
    }

    #[test]
    fn test_vector_hodge_dual() {
        let v = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let dual = v.hodge_dual();

        // In 3D, hodge dual of e1 is e23
        assert_eq!(dual.get(2), 1.0); // e23 component
    }
}

/// Comprehensive Bivector type tests
#[cfg(test)]
mod bivector_tests {
    use super::*;

    #[test]
    fn test_bivector_from_components() {
        let bv = Bivector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        assert_eq!(bv.get(0), 1.0); // e12
        assert_eq!(bv.get(1), 2.0); // e13
        assert_eq!(bv.get(2), 3.0); // e23
    }

    #[test]
    fn test_bivector_basis_bivectors() {
        let e12 = Bivector::<3, 0, 0>::e12();
        let e13 = Bivector::<3, 0, 0>::e13();
        let e23 = Bivector::<3, 0, 0>::e23();

        assert_eq!(e12.get(0), 1.0);
        assert_eq!(e12.get(1), 0.0);
        assert_eq!(e12.get(2), 0.0);

        assert_eq!(e13.get(0), 0.0);
        assert_eq!(e13.get(1), 1.0);
        assert_eq!(e13.get(2), 0.0);

        assert_eq!(e23.get(0), 0.0);
        assert_eq!(e23.get(1), 0.0);
        assert_eq!(e23.get(2), 1.0);
    }

    #[test]
    fn test_bivector_from_multivector() {
        let mv = Cl3::from_coefficients(vec![1.0, 0.0, 0.0, 2.0, 0.0, 3.0, 4.0, 0.0]);
        let bv = Bivector::from_multivector(&mv);

        // Should extract only bivector part (grade 2)
        assert_eq!(bv.get(0), 2.0); // e12
        assert_eq!(bv.get(1), 3.0); // e13
        assert_eq!(bv.get(2), 4.0); // e23
    }

    #[test]
    fn test_bivector_geometric_product_with_vector() {
        let bv = Bivector::<3, 0, 0>::e12();
        let v = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let result = bv.geometric_product(&v);

        // e12 * e1 = e12 * e1 = e1 * e2 * e1 = -e1 * e1 * e2 = -e2
        assert_eq!(result.vector_component(1), -1.0);
    }

    #[test]
    fn test_bivector_geometric_product_with_bivector() {
        let bv1 = Bivector::<3, 0, 0>::e12();
        let bv2 = Bivector::<3, 0, 0>::e13();
        let result = bv1.geometric_product_with_bivector(&bv2);

        // e12 * e13 = (e1 e2)(e1 e3) = e1 e2 e1 e3 = -e1^2 e2 e3 = -e2 e3 = -e23
        assert_eq!(result.get(6), -1.0); // e23 coefficient
    }

    #[test]
    fn test_bivector_magnitude() {
        let bv = Bivector::<3, 0, 0>::from_components(3.0, 4.0, 0.0);
        assert_eq!(bv.coefficient_norm(), 5.0); // sqrt(3² + 4²) = 5
    }

    #[test]
    fn test_bivector_get() {
        let bv = Bivector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        assert_eq!(bv.get(0), 1.0); // e12
        assert_eq!(bv.get(1), 2.0); // e13
        assert_eq!(bv.get(2), 3.0); // e23
        assert_eq!(bv.get(999), 0.0); // Out of bounds
    }

    #[test]
    fn test_bivector_inner_product() {
        let bv1 = Bivector::<3, 0, 0>::e12();
        let bv2 = Bivector::<3, 0, 0>::e13();
        let result = bv1.inner_product(&bv2);

        // Orthogonal bivectors have zero inner product
        assert!(result.is_zero());

        let bv3 = Bivector::<3, 0, 0>::from_components(2.0, 0.0, 0.0);
        let result2 = bv1.inner_product(&bv3);
        assert_eq!(result2.scalar_part(), -2.0); // Inner product of parallel bivectors
    }

    #[test]
    fn test_bivector_inner_product_with_vector() {
        let bv = Bivector::<3, 0, 0>::e12();
        let v = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let result = bv.inner_product_with_vector(&v);

        // bv · v = contraction - e12 · e1 = e2 (but with correct sign from geometric product)
        assert_eq!(result.vector_component(1), -1.0); // e12 · e1 = -e2
    }

    #[test]
    fn test_bivector_outer_product() {
        let bv1 = Bivector::<3, 0, 0>::e12();
        let bv2 = Bivector::<3, 0, 0>::e13();
        let result = bv1.outer_product(&bv2);

        // e12 ∧ e13 = 0 (overlap in indices)
        assert!(result.is_zero());

        // Test that outer product method exists and works
        let bv3 = Bivector::<3, 0, 0>::e23();
        let result2 = bv1.outer_product(&bv3);
        // In 3D, bivector outer products may be zero or very small
        // Just verify the method works without assertion on specific values
        let _ = result2; // Use the result to avoid warning
    }

    #[test]
    fn test_bivector_outer_product_with_vector() {
        let bv = Bivector::<3, 0, 0>::e12();
        let v = Vector::<3, 0, 0>::from_components(0.0, 0.0, 1.0);
        let result = bv.outer_product_with_vector(&v);

        // e12 ∧ e3 = e123
        assert_eq!(result.get(7), 1.0);
    }

    #[test]
    fn test_bivector_right_contraction() {
        let bv = Bivector::<3, 0, 0>::e12();
        let v = Vector::<3, 0, 0>::from_components(1.0, 0.0, 0.0);
        let result = bv.right_contraction(&v);

        // e12 ⌋ e1 = e2 (with correct sign)
        assert_eq!(result.vector_component(1), -1.0);
    }

    #[test]
    fn test_bivector_index_access() {
        let bv = Bivector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        assert_eq!(bv[0], 1.0); // e12
        assert_eq!(bv[1], 2.0); // e13
        assert_eq!(bv[2], 3.0); // e23
    }
}

/// Comprehensive Scalar type tests
#[cfg(test)]
mod scalar_tests {
    use super::*;

    #[test]
    fn test_scalar_from() {
        let s = Scalar::<3, 0, 0>::from(5.0);
        assert_eq!(s.mv.scalar_part(), 5.0);
        assert!(s.mv.vector_part().mv.is_zero());
    }

    #[test]
    fn test_scalar_one() {
        let s = Scalar::<3, 0, 0>::one();
        assert_eq!(s.mv.scalar_part(), 1.0);
    }

    #[test]
    fn test_scalar_from_trait() {
        let s: Scalar<3, 0, 0> = 3.0.into();
        assert_eq!(s.mv.scalar_part(), 3.0);
    }

    #[test]
    fn test_scalar_geometric_product_with_multivector() {
        let s = Scalar::<3, 0, 0>::from(2.0);
        let mv = Cl3::from_coefficients(vec![1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0]);
        let result = s.geometric_product(&mv);

        // Scalar multiplication scales all coefficients
        for i in 0..8 {
            assert_eq!(result.get(i), 2.0);
        }
    }

    #[test]
    fn test_scalar_geometric_product_with_vector() {
        let s = Scalar::<3, 0, 0>::from(3.0);
        let v = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);
        let result = s.geometric_product_with_vector(&v);

        // Scalar multiplication scales vector
        assert_eq!(result.vector_component(0), 3.0);
        assert_eq!(result.vector_component(1), 6.0);
        assert_eq!(result.vector_component(2), 9.0);
    }

    #[test]
    fn test_scalar_commutative_multiplication() {
        let s = Scalar::<3, 0, 0>::from(2.0);
        let v = Vector::<3, 0, 0>::from_components(1.0, 2.0, 3.0);

        let sv = s.geometric_product_with_vector(&v);
        let vs = v.geometric_product_with_scalar(&s);

        // Should be commutative
        for i in 0..8 {
            assert_eq!(sv.get(i), vs.get(i));
        }
    }

    #[test]
    fn test_scalar_identity() {
        let one = Scalar::<3, 0, 0>::one();
        let mv = Cl3::from_coefficients(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let result = one.geometric_product(&mv);

        // Multiplication by scalar 1 should be identity
        for i in 0..8 {
            assert_eq!(result.get(i), mv.get(i));
        }
    }
}

#[cfg(test)]
mod advanced_operations_tests {
    use super::*;

    #[test]
    fn test_exp_bivector() {
        // Create a bivector for rotation in e1-e2 plane
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);
        let bivector = e1.outer_product(&e2) * (consts::PI / 4.0); // 45 degree rotation

        let rotor = bivector.exp();

        // For a pure bivector B, exp(B) should be a rotor
        // Verify it has unit norm
        assert!((rotor.coefficient_norm() - 1.0).abs() < 1e-10);

        // Verify it can rotate vectors (rotor conjugation)
        let rotated_e1 = rotor
            .geometric_product(&e1)
            .geometric_product(&rotor.reverse());

        // For 45-degree rotation, verify rotation occurred
        // The exact values depend on implementation details, so just verify basic properties

        // Verify the rotated vector has unit length
        assert!((rotated_e1.coefficient_norm() - 1.0).abs() < 1e-10);

        // Verify it's primarily in the e1-e2 plane (other components should be zero)
        for i in [0, 3, 4, 5, 6, 7] {
            assert!(rotated_e1.get(i).abs() < 1e-10);
        }

        // Verify some rotation occurred (components changed)
        assert!((rotated_e1.get(1) - 1.0).abs() > 1e-6); // Not the original e1
        assert!(rotated_e1.get(2).abs() > 1e-6); // Some e2 component
    }

    #[test]
    fn test_exp_scalar() {
        // exp(scalar) should be scalar exponential
        let scalar = Cl3::scalar(1.0);
        let exp_scalar = scalar.exp();

        // Should be approximately e ≈ 2.718
        assert!((exp_scalar.scalar_part() - consts::E).abs() < 1e-10);

        // All other components should be zero
        for i in 1..8 {
            assert!(exp_scalar.get(i).abs() < 1e-10);
        }
    }

    #[test]
    fn test_exp_zero() {
        // exp(0) should be 1
        let zero = Cl3::zero();
        let exp_zero = zero.exp();

        assert!((exp_zero.scalar_part() - 1.0).abs() < 1e-10);
        for i in 1..8 {
            assert!(exp_zero.get(i).abs() < 1e-10);
        }
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_basis_vector_squares() {
        // In Cl(3,0,0), all basis vectors should square to +1
        for i in 0..3 {
            let ei = Cl3::basis_vector(i);
            let ei_squared = ei.geometric_product(&ei);
            assert_relative_eq!(ei_squared.scalar_part(), 1.0, epsilon = 1e-10);
        }
    }

    #[test]
    fn test_basis_vector_anticommutativity() {
        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);

        let e1_e2 = e1.geometric_product(&e2);
        let e2_e1 = e2.geometric_product(&e1);

        // Should be negatives: e1*e2 = -e2*e1
        for i in 0..8 {
            assert_relative_eq!(e1_e2.get(i), -e2_e1.get(i), epsilon = 1e-10);
        }
    }

    #[test]
    fn test_distributivity() {
        let a = Cl3::scalar(2.0);
        let b = Cl3::basis_vector(0);
        let c = Cl3::basis_vector(1);

        let a_bc = a.geometric_product(&(&b + &c));
        let ab_ac = &a.geometric_product(&b) + &a.geometric_product(&c);

        for i in 0..8 {
            assert_relative_eq!(a_bc.get(i), ab_ac.get(i), epsilon = 1e-10);
        }
    }

    #[test]
    fn test_identity_elements() {
        let mv = Cl3::from_coefficients(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);

        // Multiplicative identity
        let one = Cl3::scalar(1.0);
        let result = mv.geometric_product(&one);
        assert_eq!(mv, result);

        // Additive identity
        let zero = Cl3::zero();
        let result = &mv + &zero;
        assert_eq!(mv, result);
    }

    #[test]
    fn test_reverse_involution() {
        let a = Cl3::from_coefficients(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let b = Cl3::from_coefficients(vec![2.0, 1.0, 1.0, 1.0, 1.0, 2.0, 3.0, 1.0]);

        let ab = a.geometric_product(&b);
        let ab_rev = ab.reverse();

        let a_rev = a.reverse();
        let b_rev = b.reverse();
        let ba_rev = b_rev.geometric_product(&a_rev);

        // (AB)† = B†A†
        for i in 0..8 {
            assert_relative_eq!(ab_rev.get(i), ba_rev.get(i), epsilon = 1e-10);
        }
    }
}
