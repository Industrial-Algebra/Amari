//! Connection and Christoffel symbols

use super::MetricTensor;

/// Connection on a Riemannian manifold (Christoffel symbols)
///
/// The connection Γ^k_ij encodes how vectors change under parallel transport:
/// Γ^k_ij = (1/2) g^kl (∂_i g_jl + ∂_j g_il - ∂_l g_ij)
pub struct Connection<const DIM: usize> {
    /// Reference to the metric tensor
    metric: MetricTensor<DIM>,
    /// Step size for numerical differentiation
    h: f64,
}

impl<const DIM: usize> Connection<DIM> {
    /// Create connection from metric tensor
    ///
    /// Computes Christoffel symbols from the metric using:
    /// Γ^k_ij = (1/2) g^kl (∂_i g_jl + ∂_j g_il - ∂_l g_ij)
    pub fn from_metric(metric: &MetricTensor<DIM>) -> Self
    where
        [(); DIM]:,
    {
        Self {
            // M-REP-D D2: MetricTensor now derives Clone (ScalarField is a
            // fn-pointer + usize); the former `ptr::read` bitwise "clone
            // hack" was sound only while MetricTensor had no owning fields
            // and is replaced by the real Clone impl.
            metric: metric.clone(),
            h: 1e-5,
        }
    }

    /// Get reference to the underlying metric tensor
    pub fn metric(&self) -> &MetricTensor<DIM> {
        &self.metric
    }

    /// Compute Christoffel symbol Γ^k_ij at a point
    ///
    /// Uses the formula:
    /// Γ^k_ij = (1/2) g^kl (∂_i g_jl + ∂_j g_il - ∂_l g_ij)
    pub fn christoffel(&self, k: usize, i: usize, j: usize, coords: &[f64]) -> f64 {
        let mut gamma = 0.0;

        // Get inverse metric at this point
        let g_inv = self.metric.inverse(coords);

        // Sum over l: Γ^k_ij = (1/2) g^kl (∂_i g_jl + ∂_j g_il - ∂_l g_ij)
        #[allow(clippy::needless_range_loop)]
        for l in 0..DIM {
            let d_i_gjl = self.partial_derivative_metric(j, l, i, coords);
            let d_j_gil = self.partial_derivative_metric(i, l, j, coords);
            let d_l_gij = self.partial_derivative_metric(i, j, l, coords);

            gamma += 0.5 * g_inv[k][l] * (d_i_gjl + d_j_gil - d_l_gij);
        }

        gamma
    }

    /// Compute partial derivative ∂_k g_ij numerically
    fn partial_derivative_metric(&self, i: usize, j: usize, k: usize, coords: &[f64]) -> f64 {
        let mut coords_plus = coords.to_vec();
        let mut coords_minus = coords.to_vec();

        coords_plus[k] += self.h;
        coords_minus[k] -= self.h;

        let g_plus = self.metric.component(i, j, &coords_plus);
        let g_minus = self.metric.component(i, j, &coords_minus);

        (g_plus - g_minus) / (2.0 * self.h)
    }
}

impl<const DIM: usize> Clone for Connection<DIM>
where
    [(); DIM]:,
{
    fn clone(&self) -> Self {
        // M-REP-D D2: real Clone delegation (see from_metric).
        Self {
            metric: self.metric.clone(),
            h: self.h,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_euclidean_connection() {
        let metric = MetricTensor::<2>::euclidean();
        let connection = Connection::from_metric(&metric);

        // All Christoffel symbols should be zero for flat space
        for k in 0..2 {
            for i in 0..2 {
                for j in 0..2 {
                    let gamma = connection.christoffel(k, i, j, &[1.0, 2.0]);
                    assert!(
                        gamma.abs() < 1e-8,
                        "Γ^{}_{}{} should be 0, got {}",
                        k,
                        i,
                        j,
                        gamma
                    );
                }
            }
        }
    }

    #[test]
    fn test_sphere_connection() {
        let r = 1.0;
        let metric = MetricTensor::<2>::sphere(r);
        let connection = Connection::from_metric(&metric);

        // At θ=π/4, check a known Christoffel symbol
        let theta = std::f64::consts::PI / 4.0;
        let phi = 0.0;

        // Γ^θ_φφ = -sin(θ)cos(θ)
        let gamma_theta_phi_phi = connection.christoffel(0, 1, 1, &[theta, phi]);
        let expected = -(theta.sin() * theta.cos());

        assert!(
            (gamma_theta_phi_phi - expected).abs() < 0.05,
            "Γ^θ_φφ expected {}, got {}",
            expected,
            gamma_theta_phi_phi
        );

        // Γ^φ_θφ = cot(θ)
        let gamma_phi_theta_phi = connection.christoffel(1, 0, 1, &[theta, phi]);
        let expected_cot = theta.cos() / theta.sin();

        assert!(
            (gamma_phi_theta_phi - expected_cot).abs() < 0.05,
            "Γ^φ_θφ expected {}, got {}",
            expected_cot,
            gamma_phi_theta_phi
        );
    }
}
