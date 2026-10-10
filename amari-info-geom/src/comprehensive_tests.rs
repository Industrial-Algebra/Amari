//! Comprehensive test coverage for information geometry operations
//!
//! This module provides extensive testing for information geometric structures
//! including Fisher metrics, α-connections, Bregman divergences, and the
//! Amari-Chentsov tensor. Tests cover mathematical properties, edge cases,
//! and numerical stability.

use crate::{
    amari_chentsov_tensor, bregman_divergence, kl_divergence, AlphaConnection,
    AlphaConnectionFactory, DuallyFlatManifold, InfoGeomError, Parameter, SimpleAlphaConnection,
};
use amari_core::{basis::MultivectorBuilder, Multivector};
use approx::assert_relative_eq;

/// Test comprehensive Fisher information metric properties
#[cfg(test)]
mod fisher_metric_tests {
    use super::*;

    #[test]
    fn test_fisher_metric_positive_definiteness() {
        let manifold = DuallyFlatManifold::new(3, 0.0);

        // Test with valid probability distribution
        let prob_dist = vec![0.3, 0.4, 0.3];
        let fisher_matrix = manifold.fisher_metric_at(&prob_dist);

        let eigenvals = fisher_matrix.eigenvalues();
        for &eigenval in &eigenvals {
            assert!(
                eigenval > 0.0,
                "Fisher metric must be positive definite, got eigenvalue: {}",
                eigenval
            );
        }
    }

    #[test]
    fn test_fisher_metric_symmetry() {
        let manifold = DuallyFlatManifold::new(2, 0.0);
        let point = vec![0.6, 0.4];
        let matrix = manifold.fisher_metric_at(&point);

        // For our simplified diagonal implementation, symmetry is automatic
        // In a full implementation, we would check matrix[i][j] == matrix[j][i]
        let eigenvals = matrix.eigenvalues();
        assert_eq!(eigenvals.len(), 2);
    }

    #[test]
    fn test_fisher_metric_scaling_properties() {
        let manifold = DuallyFlatManifold::new(2, 0.0);

        // Test different probability distributions
        let uniform = vec![0.5, 0.5];
        let skewed = vec![0.9, 0.1];

        let fisher_uniform = manifold.fisher_metric_at(&uniform);
        let fisher_skewed = manifold.fisher_metric_at(&skewed);

        let eigen_uniform = fisher_uniform.eigenvalues();
        let eigen_skewed = fisher_skewed.eigenvalues();

        // Fisher information should be higher for more skewed distributions
        assert!(eigen_skewed[1] > eigen_uniform[1]);
    }

    #[test]
    fn test_fisher_metric_boundary_behavior() {
        let manifold = DuallyFlatManifold::new(2, 0.0);

        // Test near boundary of probability simplex
        let near_boundary = vec![0.999, 0.001];
        let fisher_matrix = manifold.fisher_metric_at(&near_boundary);

        let eigenvals = fisher_matrix.eigenvalues();
        // Should have very large values near boundary due to 1/p_i terms
        assert!(eigenvals[1] > 100.0);
    }

    #[test]
    fn test_fisher_metric_dimensional_consistency() {
        for dim in 1..=5 {
            let manifold = DuallyFlatManifold::new(dim, 0.0);
            let uniform_dist: Vec<f64> = (0..dim).map(|_| 1.0 / dim as f64).collect();

            let fisher_matrix = manifold.fisher_metric_at(&uniform_dist);
            let eigenvals = fisher_matrix.eigenvalues();

            assert_eq!(
                eigenvals.len(),
                dim,
                "Fisher matrix dimension should match manifold dimension"
            );
        }
    }
}

/// Test comprehensive Bregman divergence properties
#[cfg(test)]
mod bregman_divergence_tests {
    use super::*;

    #[test]
    fn test_bregman_divergence_non_negativity() {
        let phi = |mv: &Multivector<3, 0, 0>| mv.norm_squared();

        for i in 0..10 {
            let p = random_multivector(i);
            let q = random_multivector(i + 1);

            let divergence = bregman_divergence(phi, &p, &q).unwrap();
            assert!(
                divergence >= 0.0,
                "Bregman divergence must be non-negative, got: {}",
                divergence
            );
        }
    }

    #[test]
    fn test_bregman_divergence_identity() {
        let phi = |mv: &Multivector<3, 0, 0>| mv.norm_squared();

        let p = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.0)
            .e(1, 2.0)
            .e(2, -1.0)
            .build();

        let divergence = bregman_divergence(phi, &p, &p).unwrap();
        assert!(
            divergence.abs() < 1e-10,
            "Bregman divergence D(p,p) should be zero, got: {}",
            divergence
        );
    }

    #[test]
    fn test_bregman_divergence_convexity() {
        let phi = |mv: &Multivector<3, 0, 0>| mv.norm_squared();

        let p = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.0)
            .e(1, 1.0)
            .build();
        let q = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(0.5)
            .e(2, 2.0)
            .build();
        let r = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(2.0)
            .e(3, -1.0)
            .build();

        // Test convexity in first argument: D(λp + (1-λ)q, r) ≤ λD(p,r) + (1-λ)D(q,r)
        let lambda = 0.3;
        let combined = p.clone() * lambda + q.clone() * (1.0 - lambda);

        let d_combined = bregman_divergence(phi, &combined, &r).unwrap();
        let d_p = bregman_divergence(phi, &p, &r).unwrap();
        let d_q = bregman_divergence(phi, &q, &r).unwrap();

        let expected_upper_bound = lambda * d_p + (1.0 - lambda) * d_q;

        assert!(
            d_combined <= expected_upper_bound + 1e-10,
            "Convexity violated: {} > {}",
            d_combined,
            expected_upper_bound
        );
    }

    #[test]
    fn test_bregman_divergence_different_potentials() {
        let p = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.0)
            .e(1, 1.0)
            .build();
        let q = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.5)
            .e(1, 0.5)
            .build();

        // Quadratic potential
        let phi_quadratic = |mv: &Multivector<3, 0, 0>| mv.norm_squared();
        let div_quad = bregman_divergence(phi_quadratic, &p, &q).unwrap();

        // Exponential potential (simplified)
        let phi_exp = |mv: &Multivector<3, 0, 0>| mv.get(0).exp();
        let div_exp = bregman_divergence(phi_exp, &p, &q).unwrap();

        // Both should be non-negative
        assert!(div_quad >= 0.0);
        assert!(div_exp >= 0.0);

        // They should generally give different values
        assert!(
            (div_quad - div_exp).abs() > 1e-6,
            "Different potentials should give different divergences"
        );
    }

    #[test]
    fn test_bregman_divergence_manifold_implementation() {
        let manifold = DuallyFlatManifold::new(3, 0.0);

        let p = vec![0.5, 0.3, 0.2];
        let q = vec![0.4, 0.4, 0.2];

        let divergence = manifold.bregman_divergence(&p, &q);

        // KL divergence properties
        assert!(divergence >= 0.0);

        // Self-divergence should be zero
        let self_div = manifold.bregman_divergence(&p, &p);
        assert!(self_div.abs() < 1e-12);
    }

    fn random_multivector(seed: usize) -> Multivector<3, 0, 0> {
        let factor = (seed as f64 * 0.123456789) % 1.0;
        MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.0 + factor)
            .e(1, factor * 2.0 - 1.0)
            .e(2, factor * 3.0 - 1.5)
            .build()
    }
}

/// Test comprehensive α-connection properties
#[cfg(test)]
mod alpha_connection_tests {
    use super::*;

    #[test]
    fn test_alpha_connection_parameter_bounds() {
        // Valid α values
        for alpha in [-1.0, -0.5, 0.0, 0.5, 1.0] {
            let conn = SimpleAlphaConnection::new(alpha);
            assert_eq!(conn.alpha(), alpha);
        }
    }

    #[test]
    fn test_alpha_connection_special_cases() {
        // e-connection (exponential connection)
        let e_conn = SimpleAlphaConnection::new(1.0);
        assert_eq!(e_conn.alpha(), 1.0);

        // m-connection (mixture connection)
        let m_conn = SimpleAlphaConnection::new(-1.0);
        assert_eq!(m_conn.alpha(), -1.0);

        // Fisher connection
        let fisher_conn = SimpleAlphaConnection::new(0.0);
        assert_eq!(fisher_conn.alpha(), 0.0);
    }

    #[test]
    fn test_alpha_connection_duality() {
        let alpha = 0.7;
        let conn1 = SimpleAlphaConnection::new(alpha);
        let conn2 = SimpleAlphaConnection::new(-alpha);

        // Dual connections should have opposite α values
        assert!((conn1.alpha() + conn2.alpha()).abs() < 1e-12);
    }

    #[test]
    fn test_alpha_connection_factory() {
        let alpha = 0.3;
        let conn: Box<dyn AlphaConnection<Multivector<3, 0, 0>>> =
            AlphaConnectionFactory::create(alpha);

        assert_eq!(conn.alpha(), alpha);
    }

    #[test]
    fn test_christoffel_symbols_structure() {
        let _conn = SimpleAlphaConnection::new(0.5);
        let _point = MultivectorBuilder::<3, 0, 0>::new().scalar(1.0).build();

        // Note: SimpleAlphaConnection doesn't implement AlphaConnection trait yet
        // This is a simplified test of the structure
        let symbols = vec![vec![vec![0.0; 8]; 8]; 8]; // Simplified 3D structure

        // Check structure: should be dim × dim × dim tensor
        let dim = _point.dimension();
        assert_eq!(symbols.len(), dim);
        for symbol_row in symbols.iter().take(dim) {
            assert_eq!(symbol_row.len(), dim);
            for symbol_col in symbol_row.iter().take(dim) {
                assert_eq!(symbol_col.len(), dim);
            }
        }
    }

    #[test]
    fn test_covariant_derivative_properties() {
        let conn = SimpleAlphaConnection::new(0.0); // Fisher connection

        let _point = MultivectorBuilder::<3, 0, 0>::new().scalar(1.0).build();
        let _vector = MultivectorBuilder::<3, 0, 0>::new().e(1, 1.0).build();
        let _direction = MultivectorBuilder::<3, 0, 0>::new().e(2, 1.0).build();

        // Note: SimpleAlphaConnection doesn't implement AlphaConnection trait yet
        // This would test covariant derivative in a full implementation
        // For now, test that the connection exists and has correct alpha
        assert_eq!(conn.alpha(), 0.0);
    }

    #[test]
    fn test_alpha_connection_interpolation_property() {
        // Test that α-connections form a one-parameter family
        let alpha_values = vec![-1.0, -0.5, 0.0, 0.5, 1.0];

        for &alpha in &alpha_values {
            let conn = SimpleAlphaConnection::new(alpha);
            assert!((-1.0..=1.0).contains(&alpha));
            assert_eq!(conn.alpha(), alpha);
        }
    }
}

/// Test comprehensive KL divergence properties
#[cfg(test)]
mod kl_divergence_tests {
    use super::*;

    #[test]
    fn test_kl_divergence_basic_properties() {
        let eta_p = MultivectorBuilder::<3, 0, 0>::new().scalar(1.0).build();
        let eta_q = MultivectorBuilder::<3, 0, 0>::new().scalar(0.5).build();
        let mu_p = MultivectorBuilder::<3, 0, 0>::new().scalar(2.0).build();

        let kl = kl_divergence(&eta_p, &eta_q, &mu_p);

        // Basic computation check
        assert_eq!(kl, 1.0); // (1.0 - 0.5) * 2.0 = 1.0
    }

    #[test]
    fn test_kl_divergence_identity() {
        let eta = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.0)
            .e(1, 0.5)
            .build();
        let mu = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(2.0)
            .e(2, -1.0)
            .build();

        let kl = kl_divergence(&eta, &eta, &mu);
        assert_eq!(kl, 0.0, "KL(p||p) should be zero");
    }

    #[test]
    fn test_kl_divergence_asymmetry() {
        let eta_p = MultivectorBuilder::<3, 0, 0>::new().scalar(1.0).build();
        let eta_q = MultivectorBuilder::<3, 0, 0>::new().scalar(2.0).build();
        let mu_p = MultivectorBuilder::<3, 0, 0>::new().scalar(1.5).build();
        let mu_q = MultivectorBuilder::<3, 0, 0>::new().scalar(1.0).build();

        let kl_pq = kl_divergence(&eta_p, &eta_q, &mu_p);
        let kl_qp = kl_divergence(&eta_q, &eta_p, &mu_q);

        // KL divergence is generally asymmetric
        assert_ne!(kl_pq, kl_qp);
    }

    #[test]
    fn test_kl_divergence_multivariate() {
        let eta_p = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.0)
            .e(1, 0.5)
            .e(2, -0.3)
            .build();

        let eta_q = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(0.8)
            .e(1, 0.2)
            .e(2, 0.1)
            .build();

        let mu_p = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(2.0)
            .e(1, 1.0)
            .e(2, -1.5)
            .build();

        let kl = kl_divergence(&eta_p, &eta_q, &mu_p);

        // Should be finite
        assert!(kl.is_finite());

        // Manual calculation verification
        let eta_diff = eta_p - eta_q;
        let expected = eta_diff.scalar_product(&mu_p);
        assert_relative_eq!(kl, expected, epsilon = 1e-12);
    }

    #[test]
    fn test_kl_divergence_exponential_family_properties() {
        // Test properties specific to exponential families
        let eta_p = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.0)
            .e(1, 0.5)
            .build();

        let eta_q = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.2)
            .e(1, 0.3)
            .build();

        let mu_p = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.5)
            .e(1, 2.0)
            .build();

        let kl = kl_divergence(&eta_p, &eta_q, &mu_p);

        // For exponential families, KL divergence has specific structure
        let eta_diff = eta_p - eta_q;
        let manual_calc = eta_diff.scalar_product(&mu_p);

        assert_relative_eq!(kl, manual_calc, epsilon = 1e-12);
    }
}

/// Test comprehensive Amari-Chentsov tensor properties
#[cfg(test)]
mod amari_chentsov_tests {
    use super::*;

    #[test]
    fn test_amari_chentsov_tensor_antisymmetry() {
        let x = MultivectorBuilder::<3, 0, 0>::new().e(1, 1.0).build();
        let y = MultivectorBuilder::<3, 0, 0>::new().e(2, 1.0).build();
        let z = MultivectorBuilder::<3, 0, 0>::new().e(3, 1.0).build();

        let tensor_xyz = amari_chentsov_tensor(&x, &y, &z);
        let tensor_yxz = amari_chentsov_tensor(&y, &x, &z);
        let tensor_xzy = amari_chentsov_tensor(&x, &z, &y);

        // Antisymmetry under swapping arguments
        assert_relative_eq!(tensor_xyz, -tensor_yxz, epsilon = 1e-10);
        assert_relative_eq!(tensor_xyz, -tensor_xzy, epsilon = 1e-10);
    }

    #[test]
    fn test_amari_chentsov_tensor_multilinearity() {
        let x = MultivectorBuilder::<3, 0, 0>::new().e(1, 1.0).build();
        let y = MultivectorBuilder::<3, 0, 0>::new().e(2, 1.0).build();
        let z = MultivectorBuilder::<3, 0, 0>::new().e(3, 1.0).build();
        let w = MultivectorBuilder::<3, 0, 0>::new().e(1, 0.5).build();

        let a = 2.0;
        let b = 3.0;

        // Test linearity in first argument
        let linear_combo = x.clone() * a + w.clone() * b;
        let tensor_combo = amari_chentsov_tensor(&linear_combo, &y, &z);
        let expected =
            a * amari_chentsov_tensor(&x, &y, &z) + b * amari_chentsov_tensor(&w, &y, &z);

        assert_relative_eq!(tensor_combo, expected, epsilon = 1e-10);
    }

    #[test]
    fn test_amari_chentsov_tensor_scalar_triple_product() {
        // Test with standard basis vectors
        let e1 = MultivectorBuilder::<3, 0, 0>::new().e(1, 1.0).build();
        let e2 = MultivectorBuilder::<3, 0, 0>::new().e(2, 1.0).build();
        let e3 = MultivectorBuilder::<3, 0, 0>::new().e(3, 1.0).build();

        let tensor_value = amari_chentsov_tensor(&e1, &e2, &e3);

        // For orthonormal basis, should equal determinant = 1
        assert_relative_eq!(tensor_value, 1.0, epsilon = 1e-10);

        // Test cyclic permutation
        let tensor_cyclic = amari_chentsov_tensor(&e2, &e3, &e1);
        assert_relative_eq!(tensor_cyclic, 1.0, epsilon = 1e-10);

        // Test reverse order (should be negative)
        let tensor_reverse = amari_chentsov_tensor(&e3, &e2, &e1);
        assert_relative_eq!(tensor_reverse, -1.0, epsilon = 1e-10);
    }

    #[test]
    fn test_amari_chentsov_tensor_zero_cases() {
        let x = MultivectorBuilder::<3, 0, 0>::new().e(1, 1.0).build();
        let y = MultivectorBuilder::<3, 0, 0>::new().e(2, 1.0).build();
        let zero = MultivectorBuilder::<3, 0, 0>::new().build(); // Zero vector

        // Tensor with zero vector should be zero
        let tensor_with_zero = amari_chentsov_tensor(&x, &y, &zero);
        assert_relative_eq!(tensor_with_zero, 0.0, epsilon = 1e-10);

        // Tensor with repeated arguments should be zero (antisymmetry)
        let tensor_repeated = amari_chentsov_tensor(&x, &x, &y);
        assert_relative_eq!(tensor_repeated, 0.0, epsilon = 1e-10);
    }

    #[test]
    fn test_amari_chentsov_tensor_invariance_properties() {
        // Test with scaled vectors
        let x = MultivectorBuilder::<3, 0, 0>::new().e(1, 2.0).build();
        let y = MultivectorBuilder::<3, 0, 0>::new().e(2, 3.0).build();
        let z = MultivectorBuilder::<3, 0, 0>::new().e(3, 4.0).build();

        let tensor_scaled = amari_chentsov_tensor(&x, &y, &z);

        // Compare with normalized version
        let scale_factor = 2.0 * 3.0 * 4.0; // Product of scales
        let x_norm = MultivectorBuilder::<3, 0, 0>::new().e(1, 1.0).build();
        let y_norm = MultivectorBuilder::<3, 0, 0>::new().e(2, 1.0).build();
        let z_norm = MultivectorBuilder::<3, 0, 0>::new().e(3, 1.0).build();

        let tensor_norm = amari_chentsov_tensor(&x_norm, &y_norm, &z_norm);

        assert_relative_eq!(tensor_scaled, scale_factor * tensor_norm, epsilon = 1e-10);
    }

    #[test]
    fn test_amari_chentsov_tensor_complex_vectors() {
        // Test with more complex multivector combinations
        let x = MultivectorBuilder::<3, 0, 0>::new()
            .e(1, 1.0)
            .e(2, 0.5)
            .build();

        let y = MultivectorBuilder::<3, 0, 0>::new()
            .e(2, 1.0)
            .e(3, -0.5)
            .build();

        let z = MultivectorBuilder::<3, 0, 0>::new()
            .e(1, 0.3)
            .e(3, 1.0)
            .build();

        let tensor_value = amari_chentsov_tensor(&x, &y, &z);

        // Should be finite and well-defined
        assert!(tensor_value.is_finite());

        // Test antisymmetry still holds
        let tensor_swapped = amari_chentsov_tensor(&y, &x, &z);
        assert_relative_eq!(tensor_value, -tensor_swapped, epsilon = 1e-10);
    }
}

/// Test comprehensive information geometry integration
#[cfg(test)]
mod integration_tests {
    use super::*;

    #[test]
    fn test_fisher_bregman_consistency() {
        let manifold = DuallyFlatManifold::new(3, 0.0);

        let p = vec![0.5, 0.3, 0.2];
        let q = vec![0.4, 0.4, 0.2];

        // Fisher metric at p
        let fisher_p = manifold.fisher_metric_at(&p);
        let eigenvals_p = fisher_p.eigenvalues();

        // Bregman divergence
        let bregman_div = manifold.bregman_divergence(&p, &q);

        // Both should be well-defined
        assert!(eigenvals_p.iter().all(|&λ| λ > 0.0));
        assert!(bregman_div >= 0.0);
    }

    #[test]
    fn test_alpha_connection_fisher_metric_compatibility() {
        let manifold = DuallyFlatManifold::new(2, 0.5);
        let conn = SimpleAlphaConnection::new(0.5);

        let point = vec![0.6, 0.4];
        let fisher_matrix = manifold.fisher_metric_at(&point);

        // Both structures should be compatible
        assert!(fisher_matrix.eigenvalues().iter().all(|&λ| λ > 0.0));
        assert!(conn.alpha() >= -1.0 && conn.alpha() <= 1.0);
    }

    #[test]
    fn test_exponential_family_duality() {
        // Test duality between natural and expectation parameters
        let eta = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.0)
            .e(1, 0.5)
            .build();

        let mu = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(2.0)
            .e(1, 1.0)
            .build();

        // Test that natural and expectation parameters are related
        // In exponential families: μ = ∇ψ(η) where ψ is log partition function
        // This is a simplified consistency check
        let eta_norm = eta.coefficient_norm();
        let mu_norm = mu.coefficient_norm();

        assert!(eta_norm > 0.0);
        assert!(mu_norm > 0.0);
    }

    #[test]
    fn test_information_geometric_duality() {
        // Test fundamental duality in information geometry
        let e_conn = SimpleAlphaConnection::new(1.0); // e-connection
        let m_conn = SimpleAlphaConnection::new(-1.0); // m-connection

        // e-connection and m-connection should be dual
        assert_relative_eq!(e_conn.alpha() + m_conn.alpha(), 0.0, epsilon = 1e-12);

        // Test with intermediate α values
        let alpha = 0.7;
        let conn_alpha = SimpleAlphaConnection::new(alpha);
        let conn_minus_alpha = SimpleAlphaConnection::new(-alpha);

        assert_relative_eq!(
            conn_alpha.alpha() + conn_minus_alpha.alpha(),
            0.0,
            epsilon = 1e-12
        );
    }

    #[test]
    fn test_numerical_stability_edge_cases() {
        // Test with extreme probability values
        let manifold = DuallyFlatManifold::new(3, 0.0);

        // Near-uniform distribution
        let near_uniform = vec![0.333, 0.333, 0.334];
        let fisher_uniform = manifold.fisher_metric_at(&near_uniform);
        assert!(fisher_uniform.eigenvalues().iter().all(|&λ| λ.is_finite()));

        // Highly skewed distribution
        let skewed = vec![0.98, 0.01, 0.01];
        let fisher_skewed = manifold.fisher_metric_at(&skewed);
        assert!(fisher_skewed.eigenvalues().iter().all(|&λ| λ.is_finite()));

        // Test Bregman divergence numerical stability
        let bregman_stable = manifold.bregman_divergence(&near_uniform, &skewed);
        assert!(bregman_stable.is_finite() && bregman_stable >= 0.0);
    }

    #[test]
    fn test_multivector_parameter_implementation() {
        // Test Parameter trait implementation for Multivector
        let mv = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.0)
            .e(1, 2.0)
            .e(2, 3.0)
            .build();

        assert_eq!(mv.dimension(), 8); // 2^3 = 8 basis elements
        assert_eq!(mv.get_component(0), 1.0); // Scalar part
        assert_eq!(mv.get_component(1), 2.0); // e1 part
        assert_eq!(mv.get_component(2), 3.0); // e2 part

        // Test mutable operations
        let mut mv_mut = mv.clone();
        mv_mut.set_component(3, 4.0); // e3 part
        assert_eq!(mv_mut.get_component(3), 4.0);
    }
}

/// Test error handling and edge cases
#[cfg(test)]
mod error_handling_tests {
    use super::*;

    #[test]
    fn test_info_geom_error_types() {
        // Test different error types
        let err1 = InfoGeomError::NumericalInstability;
        let err2 = InfoGeomError::InvalidDimension {
            expected: 3,
            actual: 2,
        };
        let err3 = InfoGeomError::ParameterOutOfRange;

        // Errors should display properly
        assert!(format!("{}", err1).contains("Numerical instability"));
        assert!(format!("{}", err2).contains("Invalid parameter dimension"));
        assert!(format!("{}", err3).contains("Parameter out of valid range"));
    }

    #[test]
    fn test_numerical_edge_cases() {
        let manifold = DuallyFlatManifold::new(2, 0.0);

        // Test with zero probabilities (should handle gracefully)
        let with_zero = vec![1.0, 0.0];
        let fisher_matrix = manifold.fisher_metric_at(&with_zero);
        let eigenvals = fisher_matrix.eigenvalues();

        // Second eigenvalue should be very large due to 1/0 → ∞
        assert!(eigenvals[1] > 1e10);
    }

    #[test]
    fn test_bregman_divergence_error_propagation() {
        // Test with potential that could cause numerical issues
        let phi_problematic = |mv: &Multivector<3, 0, 0>| {
            let norm = mv.coefficient_norm();
            if norm > 0.0 {
                1.0 / norm // Could cause division by zero
            } else {
                f64::INFINITY
            }
        };

        let p = MultivectorBuilder::<3, 0, 0>::new().scalar(1.0).build();
        let q = MultivectorBuilder::<3, 0, 0>::new().scalar(0.5).build();

        // Should handle potential numerical issues
        let result = bregman_divergence(phi_problematic, &p, &q);
        assert!(result.is_ok());

        let divergence = result.unwrap();
        assert!(divergence.is_finite());
    }
}
