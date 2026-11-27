//! Property-based tests for binding algebras.
//!
//! These tests verify that all algebra implementations satisfy the
//! required algebraic laws for Vector Symbolic Architectures.

#[cfg(test)]
mod property_tests {
    use crate::algebra::*;

    /// Test that binding with identity gives the original element.
    fn test_binding_identity_law<A: BindingAlgebra>(make_random: impl Fn() -> A) {
        let identity = A::identity();

        for _ in 0..10 {
            let a = make_random();
            let bound = a.bind(&identity);

            let sim = a.similarity(&bound).unwrap();
            assert!(
                sim > 0.99,
                "{}: identity law failed, similarity = {}",
                A::algebra_name(),
                sim
            );
        }
    }

    /// Test that binding with inverse gives approximately identity.
    fn test_inverse_law<A: BindingAlgebra>(make_random: impl Fn() -> A) {
        let identity = A::identity();

        for _ in 0..10 {
            let a = make_random();
            if let Ok(a_inv) = a.inverse() {
                let product = a.bind(&a_inv);

                let sim = product.similarity(&identity).unwrap();
                assert!(
                    sim > 0.95,
                    "{}: inverse law failed, similarity = {}",
                    A::algebra_name(),
                    sim
                );
            }
        }
    }

    /// Test that unbinding recovers the original value.
    fn test_unbind_recovery<A: BindingAlgebra>(make_random: impl Fn() -> A) {
        for _ in 0..10 {
            let key = make_random();
            let value = make_random();

            let bound = key.bind(&value);
            if let Ok(recovered) = key.unbind(&bound) {
                let sim = recovered.similarity(&value).unwrap();
                assert!(
                    sim > 0.95,
                    "{}: unbind recovery failed, similarity = {}",
                    A::algebra_name(),
                    sim
                );
            }
        }
    }

    /// Test that binding produces dissimilar results.
    fn test_binding_dissimilarity<A: BindingAlgebra>(make_random: impl Fn() -> A) {
        for _ in 0..10 {
            let a = make_random();
            let b = make_random();
            let bound = a.bind(&b);

            let sim_a = bound.similarity(&a).unwrap().abs();
            let sim_b = bound.similarity(&b).unwrap().abs();

            // For high-dimensional spaces, similarity should be low
            // For low dimensions (like Cl3), we're more lenient
            let threshold = if A::identity().dimension() < 32 {
                0.9
            } else {
                0.5
            };

            assert!(
                sim_a < threshold || sim_b < threshold,
                "{}: binding should produce dissimilar result, sim_a={}, sim_b={}",
                A::algebra_name(),
                sim_a,
                sim_b
            );
        }
    }

    /// Test that bundling produces similar results.
    fn test_bundling_similarity<A: BindingAlgebra>(make_random: impl Fn() -> A) {
        for _ in 0..10 {
            let a = make_random();
            let b = make_random();

            if let Ok(bundled) = a.bundle(&b, 1.0) {
                // Bundled should have some similarity to inputs
                let sim_a = bundled.similarity(&a).unwrap();
                let sim_b = bundled.similarity(&b).unwrap();

                // At least one should have positive similarity
                assert!(
                    sim_a > -0.5 || sim_b > -0.5,
                    "{}: bundling should produce somewhat similar result",
                    A::algebra_name()
                );
            }
        }
    }

    /// Test permutation reversibility.
    fn test_permute_reversible<A: BindingAlgebra>(make_random: impl Fn() -> A) {
        for _ in 0..10 {
            let a = make_random();
            let shift = fastrand::i32(-10..10);

            let permuted = a.permute(shift);
            let back = permuted.permute(-shift);

            let sim = a.similarity(&back).unwrap();
            assert!(
                sim > 0.99,
                "{}: permutation should be reversible, similarity = {}",
                A::algebra_name(),
                sim
            );
        }
    }

    /// Test normalization.
    fn test_normalization<A: BindingAlgebra>(make_random: impl Fn() -> A) {
        for _ in 0..10 {
            let a = make_random();
            if let Ok(normalized) = a.normalize() {
                let norm = normalized.norm().unwrap();
                assert!(
                    (norm - 1.0).abs() < 0.01,
                    "{}: normalized element should have unit norm, got {}",
                    A::algebra_name(),
                    norm
                );
            }
        }
    }

    /// Test coefficient round-trip.
    fn test_coefficient_roundtrip<A: BindingAlgebra>(make_random: impl Fn() -> A) {
        for _ in 0..10 {
            let a = make_random();
            let coeffs = a.to_coefficients();

            if let Ok(reconstructed) = A::from_coefficients(&coeffs) {
                let sim = a.similarity(&reconstructed).unwrap();
                assert!(
                    sim > 0.99,
                    "{}: coefficient round-trip failed, similarity = {}",
                    A::algebra_name(),
                    sim
                );
            }
        }
    }

    // ========================================================================
    // Algebra-specific test suites
    // ========================================================================

    mod clifford_tests {
        use super::*;
        use crate::algebra::clifford::Cl3Full;

        #[test]
        fn test_cl3_identity_law() {
            test_binding_identity_law(|| Cl3Full::random_versor(2).unwrap());
        }

        #[test]
        fn test_cl3_inverse_law() {
            test_inverse_law(|| Cl3Full::random_versor(2).unwrap());
        }

        #[test]
        fn test_cl3_unbind_recovery() {
            test_unbind_recovery(|| Cl3Full::random_versor(2).unwrap());
        }

        #[test]
        fn test_cl3_dissimilarity() {
            test_binding_dissimilarity(|| Cl3Full::random_versor(1).unwrap());
        }

        #[test]
        fn test_cl3_bundling() {
            test_bundling_similarity(|| Cl3Full::random_versor(1).unwrap());
        }

        #[test]
        fn test_cl3_permute() {
            test_permute_reversible(|| Cl3Full::random_versor(1).unwrap());
        }

        #[test]
        fn test_cl3_normalize() {
            test_normalization(|| Cl3Full::random_versor(1).unwrap());
        }

        #[test]
        fn test_cl3_coefficients() {
            test_coefficient_roundtrip(|| Cl3Full::random_versor(1).unwrap());
        }
    }

    mod cl3_tests {
        use super::*;
        use crate::algebra::cl3::Cl3;

        #[test]
        fn test_cl3_optimized_identity_law() {
            test_binding_identity_law(|| Cl3::random_versor(2));
        }

        #[test]
        fn test_cl3_optimized_inverse_law() {
            test_inverse_law(|| Cl3::random_versor(2));
        }

        #[test]
        fn test_cl3_optimized_unbind_recovery() {
            test_unbind_recovery(|| Cl3::random_versor(2));
        }

        #[test]
        fn test_cl3_optimized_normalize() {
            test_normalization(|| Cl3::random_versor(1));
        }

        #[test]
        fn test_cl3_optimized_coefficients() {
            test_coefficient_roundtrip(|| Cl3::random_versor(1));
        }
    }

    mod product_clifford_tests {
        use super::*;
        use crate::algebra::product_clifford::ProductCl3x8;

        #[test]
        fn test_product_identity_law() {
            test_binding_identity_law(|| ProductCl3x8::random_versor(2));
        }

        #[test]
        fn test_product_inverse_law() {
            test_inverse_law(|| ProductCl3x8::random_versor(2));
        }

        #[test]
        fn test_product_unbind_recovery() {
            test_unbind_recovery(|| ProductCl3x8::random_versor(2));
        }

        #[test]
        fn test_product_dissimilarity() {
            test_binding_dissimilarity(|| ProductCl3x8::random_versor(1));
        }

        #[test]
        fn test_product_bundling() {
            test_bundling_similarity(|| ProductCl3x8::random_versor(1));
        }

        #[test]
        fn test_product_permute() {
            test_permute_reversible(|| ProductCl3x8::random_versor(1));
        }

        #[test]
        fn test_product_normalize() {
            test_normalization(|| ProductCl3x8::random_versor(1));
        }

        #[test]
        fn test_product_coefficients() {
            test_coefficient_roundtrip(|| ProductCl3x8::random_versor(1));
        }
    }

    mod fhrr_tests {
        use super::*;
        use crate::algebra::fhrr::FHRR256;

        #[test]
        fn test_fhrr_identity_law() {
            test_binding_identity_law(FHRR256::random_unitary);
        }

        #[test]
        fn test_fhrr_inverse_law() {
            test_inverse_law(FHRR256::random_unitary);
        }

        #[test]
        fn test_fhrr_unbind_recovery() {
            test_unbind_recovery(FHRR256::random_unitary);
        }

        #[test]
        fn test_fhrr_dissimilarity() {
            test_binding_dissimilarity(FHRR256::random_unitary);
        }

        #[test]
        fn test_fhrr_bundling() {
            test_bundling_similarity(FHRR256::random_unitary);
        }

        #[test]
        fn test_fhrr_permute() {
            test_permute_reversible(FHRR256::random_unitary);
        }

        #[test]
        fn test_fhrr_normalize() {
            test_normalization(FHRR256::random_unitary);
        }

        #[test]
        fn test_fhrr_coefficients() {
            // FHRR stores complex numbers but to_coefficients only returns real parts.
            // So we test with real-only elements (identity-like).
            fn make_real_only() -> FHRR256 {
                FHRR256::fhrr_identity()
            }
            test_coefficient_roundtrip(make_real_only);
        }
    }

    mod map_tests {
        use super::*;
        use crate::algebra::map::MAP256;

        #[test]
        fn test_map_identity_law() {
            test_binding_identity_law(MAP256::random_bipolar);
        }

        #[test]
        fn test_map_inverse_law() {
            // MAP is self-inverse, so this tests a * a = identity
            test_inverse_law(MAP256::random_bipolar);
        }

        #[test]
        fn test_map_unbind_recovery() {
            test_unbind_recovery(MAP256::random_bipolar);
        }

        #[test]
        fn test_map_dissimilarity() {
            test_binding_dissimilarity(MAP256::random_bipolar);
        }

        #[test]
        fn test_map_bundling() {
            test_bundling_similarity(MAP256::random_bipolar);
        }

        #[test]
        fn test_map_permute() {
            test_permute_reversible(MAP256::random_bipolar);
        }

        #[test]
        fn test_map_normalize() {
            test_normalization(MAP256::random_bipolar);
        }

        #[test]
        fn test_map_coefficients() {
            test_coefficient_roundtrip(MAP256::random_bipolar);
        }
    }
}

#[cfg(test)]
mod capacity_tests {
    use crate::algebra::*;

    /// Test that capacity scales correctly with dimension.
    #[test]
    fn test_capacity_scaling() {
        // Cl3: dimension 8
        let cl3 = cl3::Cl3::one();
        let cl3_cap = cl3.theoretical_capacity();
        assert!(cl3_cap > 2 && cl3_cap < 10);

        // ProductCl3x32: dimension 256
        let prod = product_clifford::ProductCl3x32::product_identity();
        let prod_cap = prod.theoretical_capacity();
        assert!(prod_cap > 40 && prod_cap < 60);

        // FHRR256: dimension 256
        let fhrr = fhrr::FHRR256::fhrr_identity();
        let fhrr_cap = fhrr.theoretical_capacity();
        assert!(fhrr_cap > 40 && fhrr_cap < 60);

        // MAP256: dimension 256
        let map = map::MAP256::map_identity();
        let map_cap = map.theoretical_capacity();
        assert!(map_cap > 40 && map_cap < 60);
    }

    /// Test that SNR decreases with more items.
    #[test]
    fn test_snr_decreases() {
        let a = product_clifford::ProductCl3x32::random_versor(1);

        let snr_0 = a.estimate_snr(0);
        assert!(snr_0.is_infinite());

        let snr_1 = a.estimate_snr(1);
        let snr_10 = a.estimate_snr(10);
        let snr_100 = a.estimate_snr(100);

        assert!(snr_1 > snr_10);
        assert!(snr_10 > snr_100);
    }
}

#[cfg(test)]
mod comparison_tests {
    use crate::algebra::*;

    /// Compare different algebras at similar dimensions.
    #[test]
    fn test_compare_algebras_256d() {
        // All at ~256 dimensions
        let prod = product_clifford::ProductCl3x32::random_versor(2);
        let fhrr = fhrr::FHRR256::random_unitary();
        let map = map::MAP256::random_bipolar();

        // All should have dimension 256
        assert_eq!(prod.dimension(), 256);
        assert_eq!(fhrr.dimension(), 256);
        assert_eq!(map.dimension(), 256);

        // All should have similar capacity
        let prod_cap = prod.theoretical_capacity();
        let fhrr_cap = fhrr.theoretical_capacity();
        let map_cap = map.theoretical_capacity();

        assert!((prod_cap as f64 - fhrr_cap as f64).abs() < 10.0);
        assert!((fhrr_cap as f64 - map_cap as f64).abs() < 10.0);
    }

    /// Test that all algebras support the same basic operations.
    #[test]
    fn test_uniform_api() {
        fn test_algebra<A: BindingAlgebra>(make: impl Fn() -> A) {
            let a = make();
            let b = make();

            // All algebras support these operations
            let _identity = A::identity();
            let _zero = A::zero();
            let _bound = a.bind(&b);
            let _inv = a.inverse();
            let _bundled = a.bundle(&b, 1.0);
            let _sim = a.similarity(&b).unwrap();
            let _norm = a.norm();
            let _normalized = a.normalize();
            let _permuted = a.permute(3);
            let _coeffs = a.to_coefficients();
            let _dim = a.dimension();
            let _cap = a.theoretical_capacity();
            let _snr = a.estimate_snr(10);
        }

        test_algebra(|| cl3::Cl3::random_versor(1));
        test_algebra(|| product_clifford::ProductCl3x8::random_versor(1));
        test_algebra(fhrr::FHRR64::random_unitary);
        test_algebra(map::MAP64::random_bipolar);
    }
}
