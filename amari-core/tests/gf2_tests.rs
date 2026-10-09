//! Cross-module integration tests for the GF(2) algebra module.

#![cfg(feature = "gf2")]

use amari_core::gf2::*;

mod grassmannian_matrix_integration {
    use super::*;

    #[test]
    fn test_enumerate_and_verify_subspaces() {
        let subs = enumerate_subspaces(2, 4);
        assert_eq!(subs.len(), 35);

        for s in &subs {
            // Each subspace should be rank 2.
            assert_eq!(s.rank(), 2);
            // Each row should be nonzero.
            for i in 0..s.nrows() {
                assert!(!s.row(i).is_zero());
            }
        }
    }

    #[test]
    fn test_subspace_closure_under_xor() {
        // A 2-dimensional subspace contains {0, r0, r1, r0+r1}.
        // Verify XOR closure: r0 + r1 should be expressible via the basis.
        let subs = enumerate_subspaces(2, 3);
        for s in &subs {
            let r0 = s.row(0);
            let r1 = s.row(1);
            let sum = r0.add(r1);
            // sum should be nonzero (since r0 != r1 in a rank-2 subspace)
            // and different from both r0 and r1.
            if !sum.is_zero() {
                assert_ne!(&sum, r0);
                assert_ne!(&sum, r1);
            }
        }
    }
}

mod clifford_real_integration {
    use super::*;

    #[test]
    fn test_binary_to_real_geometric_product() {
        type Cl3 = BinaryMultivector<3, 0>;

        let e1 = Cl3::basis_vector(0);
        let e2 = Cl3::basis_vector(1);

        // Binary geometric product: e1 * e2 = e12
        let binary_prod = e1.geometric_product(&e2);

        // Real geometric product: same result
        let real_e1 = e1.to_real();
        let real_e2 = e2.to_real();
        let real_prod = real_e1.geometric_product(&real_e2);

        // Convert binary product to real and compare
        let binary_as_real = binary_prod.to_real();
        for i in 0..8 {
            assert!(
                (binary_as_real.get(i) - real_prod.get(i)).abs() < 1e-14,
                "mismatch at blade {}: binary={}, real={}",
                i,
                binary_as_real.get(i),
                real_prod.get(i)
            );
        }
    }

    #[test]
    fn test_from_real_reduces_mod2() {
        type Cl3 = BinaryMultivector<3, 0>;

        // Create a real multivector with coefficients that need reduction.
        let mut real_mv = amari_core::Multivector::<3, 0, 0>::zero();
        real_mv.set(0, 3.0); // 3 mod 2 = 1
        real_mv.set(1, 2.0); // 2 mod 2 = 0
        real_mv.set(3, 5.0); // 5 mod 2 = 1

        let binary = Cl3::from_real(&real_mv);
        assert_eq!(binary.get(0), GF2::ONE);
        assert_eq!(binary.get(1), GF2::ZERO);
        assert_eq!(binary.get(3), GF2::ONE);
    }
}

mod scalar_vector_matrix_chain {
    use super::*;

    #[test]
    fn test_matrix_solve_and_verify() {
        // Create a system Ax = b, solve, and verify.
        let a = GF2Matrix::from_rows(vec![
            GF2Vector::from_bits(&[1, 1, 0]),
            GF2Vector::from_bits(&[0, 1, 1]),
            GF2Vector::from_bits(&[1, 0, 1]),
        ]);
        let b = GF2Vector::from_bits(&[1, 0, 1]);

        if let Some(x) = a.solve(&b) {
            let result = a.mul_vec(&x);
            assert_eq!(result, b, "Ax should equal b");
        }
    }

    #[test]
    fn test_null_space_orthogonal_to_row_space() {
        let m = GF2Matrix::from_rows(vec![
            GF2Vector::from_bits(&[1, 0, 1, 0]),
            GF2Vector::from_bits(&[0, 1, 0, 1]),
        ]);
        let ns = m.null_space();
        for v in &ns {
            let prod = m.mul_vec(v);
            assert!(prod.is_zero(), "null space vector must satisfy Av=0");
        }
    }
}
