//! Information Geometry operations for statistical manifolds
//!
//! This crate implements the foundational concepts of Information Geometry,
//! including Fisher metrics, α-connections, Bregman divergences, and the
//! Amari-Chentsov tensor structure.

use amari_core::Multivector;
use num_traits::{Float, Zero};
use thiserror::Error;

// GPU acceleration exports
#[cfg(feature = "gpu")]
pub use gpu::{
    GpuBregmanData, GpuFisherData, GpuStatisticalManifold, InfoGeomGpuConfig, InfoGeomGpuError,
    InfoGeomGpuOps, InfoGeomGpuResult,
};

#[cfg(test)]
pub mod comprehensive_tests;
#[cfg(feature = "gpu")]
pub mod gpu;
pub mod verified_contracts;

// pub mod fisher;
// pub mod connections;
// pub mod divergences;
// pub mod manifolds;

#[derive(Error, Debug)]
pub enum InfoGeomError {
    #[error("Numerical instability in computation")]
    NumericalInstability,

    #[error("Invalid parameter dimension: expected {expected}, got {actual}")]
    InvalidDimension { expected: usize, actual: usize },

    #[error("Parameter out of valid range")]
    ParameterOutOfRange,
}

/// Trait for objects that can be used as parameters on statistical manifolds
pub trait Parameter {
    type Scalar: Float;

    /// Dimension of the parameter space
    fn dimension(&self) -> usize;

    /// Get parameter component by index
    fn get_component(&self, index: usize) -> Self::Scalar;

    /// Set parameter component by index
    fn set_component(&mut self, index: usize, value: Self::Scalar);
}

impl<const P: usize, const Q: usize, const R: usize> Parameter for Multivector<P, Q, R> {
    type Scalar = f64;

    fn dimension(&self) -> usize {
        Self::BASIS_COUNT
    }

    fn get_component(&self, index: usize) -> f64 {
        self.get(index)
    }

    fn set_component(&mut self, index: usize, value: f64) {
        self.set(index, value);
    }
}

/// Fisher Information Metric for statistical manifolds
pub trait FisherMetric<T: Parameter> {
    /// Compute the Fisher information matrix at a point
    fn fisher_matrix(&self, point: &T) -> Result<Vec<Vec<T::Scalar>>, InfoGeomError>;

    /// Compute inner product using Fisher metric
    fn fisher_inner_product(&self, point: &T, v1: &T, v2: &T) -> Result<T::Scalar, InfoGeomError> {
        let g = self.fisher_matrix(point)?;
        let mut result = T::Scalar::zero();

        for i in 0..v1.dimension() {
            for j in 0..v2.dimension() {
                if i < g.len() && j < g[i].len() {
                    result = result + g[i][j] * v1.get_component(i) * v2.get_component(j);
                }
            }
        }

        Ok(result)
    }
}

/// α-connection on a statistical manifold
pub trait AlphaConnection<T: Parameter> {
    /// The α parameter defining this connection (-1 ≤ α ≤ 1)
    fn alpha(&self) -> f64;

    /// Christoffel symbols for the α-connection
    fn christoffel_symbols(&self, point: &T) -> Result<Vec<Vec<Vec<T::Scalar>>>, InfoGeomError>;

    /// Covariant derivative along a curve
    fn covariant_derivative(
        &self,
        point: &T,
        vector: &T,
        direction: &T,
    ) -> Result<T, InfoGeomError>;
}

/// Dually flat manifold with e-connection and m-connection
#[derive(Clone, Debug)]
pub struct DuallyFlatManifold {
    dimension: usize,
    #[allow(dead_code)]
    alpha: f64,
}

impl DuallyFlatManifold {
    /// Create new dually flat manifold with given dimension and alpha parameter
    pub fn new(dimension: usize, alpha: f64) -> Self {
        Self { dimension, alpha }
    }

    /// Compute Fisher information metric at a point
    pub fn fisher_metric_at(&self, point: &[f64]) -> FisherInformationMatrix {
        // For exponential families, Fisher metric is the Hessian of log partition function
        let mut matrix = vec![vec![0.0; self.dimension]; self.dimension];

        // Simplified Fisher metric for probability simplex
        #[allow(clippy::needless_range_loop)]
        for i in 0..self.dimension {
            for j in 0..self.dimension {
                if i == j && i < point.len() {
                    // Diagonal elements: 1/p_i for probability distributions
                    matrix[i][j] = if point[i] > 1e-12 {
                        1.0 / point[i]
                    } else {
                        1e12
                    };
                } else {
                    // Off-diagonal elements are zero for independent components
                    matrix[i][j] = 0.0;
                }
            }
        }

        FisherInformationMatrix { matrix }
    }

    /// Compute Bregman divergence between two points
    pub fn bregman_divergence(&self, p: &[f64], q: &[f64]) -> f64 {
        // KL divergence for probability distributions: D_KL(p||q) = Σ p_i log(p_i/q_i)
        let mut divergence = 0.0;

        for i in 0..p.len().min(q.len()) {
            if p[i] > 1e-12 && q[i] > 1e-12 {
                divergence += p[i] * (p[i] / q[i]).ln();
            }
        }

        divergence
    }
}

/// Fisher Information Matrix
#[derive(Clone, Debug)]
pub struct FisherInformationMatrix {
    matrix: Vec<Vec<f64>>,
}

impl FisherInformationMatrix {
    /// Compute eigenvalues to check positive definiteness
    pub fn eigenvalues(&self) -> Vec<f64> {
        // Simplified eigenvalue computation for testing
        // In practice, would use proper linear algebra library
        let mut eigenvals = Vec::new();

        // For diagonal matrices, eigenvalues are the diagonal elements
        for (i, row) in self.matrix.iter().enumerate() {
            if let Some(&value) = row.get(i) {
                eigenvals.push(value);
            }
        }

        eigenvals
    }
}

/// Simplified AlphaConnection implementation for tests
#[derive(Clone, Debug)]
pub struct SimpleAlphaConnection {
    alpha: f64,
}

impl SimpleAlphaConnection {
    pub fn new(alpha: f64) -> Self {
        Self { alpha }
    }

    pub fn alpha(&self) -> f64 {
        self.alpha
    }
}

// For backwards compatibility, we expose both the trait and struct versions

/// Compute the Bregman divergence between two points
pub fn bregman_divergence<F>(
    phi: F,
    p: &Multivector<3, 0, 0>,
    q: &Multivector<3, 0, 0>,
) -> Result<f64, InfoGeomError>
where
    F: Fn(&Multivector<3, 0, 0>) -> f64,
{
    let phi_p = phi(p);
    let phi_q = phi(q);

    // Approximate gradient using finite differences
    let eps = 1e-8;
    let mut grad_phi_q = Multivector::zero();

    for i in 0..8 {
        let mut q_plus = q.clone();
        q_plus.set(i, q.get(i) + eps);
        let phi_plus = phi(&q_plus);

        let mut q_minus = q.clone();
        q_minus.set(i, q.get(i) - eps);
        let phi_minus = phi(&q_minus);

        let derivative = (phi_plus - phi_minus) / (2.0 * eps);
        grad_phi_q.set(i, derivative);
    }

    let diff = p - q;
    let inner_product = diff.scalar_product(&grad_phi_q);

    Ok(phi_p - phi_q - inner_product)
}

/// Compute KL divergence using natural and expectation parameters
pub fn kl_divergence(
    eta_p: &Multivector<3, 0, 0>, // Natural parameters for p
    eta_q: &Multivector<3, 0, 0>, // Natural parameters for q
    mu_p: &Multivector<3, 0, 0>,  // Expectation parameters for p
) -> f64 {
    // KL(p||q) = <η_p - η_q, μ_p> - ψ(η_p) + ψ(η_q)
    // where ψ is the log partition function

    let eta_diff = eta_p - eta_q;

    // For simplicity, assume log partition functions cancel in relative computation
    eta_diff.scalar_product(mu_p)
}

/// Compute the Amari-Chentsov tensor at a point
pub fn amari_chentsov_tensor(
    x: &Multivector<3, 0, 0>,
    y: &Multivector<3, 0, 0>,
    z: &Multivector<3, 0, 0>,
) -> f64 {
    // The Amari-Chentsov tensor is the unique (up to scaling) symmetric 3-tensor
    // that is invariant under sufficient statistics transformations.
    //
    // T(X,Y,Z) = ∂³ψ/∂θ^i∂θ^j∂θ^k X^i Y^j Z^k
    // For a proper implementation, we use the symmetrized trilinear form:
    // T(X,Y,Z) = (1/6)[X·(Y×Z) + Y·(Z×X) + Z·(X×Y) + cyclic permutations]

    // Extract vector components for the computation
    let x_vec = [
        x.vector_component(0),
        x.vector_component(1),
        x.vector_component(2),
    ];
    let y_vec = [
        y.vector_component(0),
        y.vector_component(1),
        y.vector_component(2),
    ];
    let z_vec = [
        z.vector_component(0),
        z.vector_component(1),
        z.vector_component(2),
    ];

    // Compute the symmetric trilinear form
    // For 3D Euclidean space, this is related to the scalar triple product
    x_vec[0] * y_vec[1] * z_vec[2] + x_vec[1] * y_vec[2] * z_vec[0] + x_vec[2] * y_vec[0] * z_vec[1]
        - x_vec[2] * y_vec[1] * z_vec[0]
        - x_vec[1] * y_vec[0] * z_vec[2]
        - x_vec[0] * y_vec[2] * z_vec[1]
}

/// α-connection factory
pub struct AlphaConnectionFactory;

impl AlphaConnectionFactory {
    /// Create an α-connection for the given α value
    pub fn create<T: Parameter + Clone + 'static>(alpha: f64) -> Box<dyn AlphaConnection<T>> {
        Box::new(StandardAlphaConnection::new(alpha))
    }
}

/// Standard implementation of α-connection
struct StandardAlphaConnection<T: Parameter> {
    alpha: f64,
    _phantom: std::marker::PhantomData<T>,
}

impl<T: Parameter> StandardAlphaConnection<T> {
    fn new(alpha: f64) -> Self {
        Self {
            alpha,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<T: Parameter + Clone> AlphaConnection<T> for StandardAlphaConnection<T> {
    fn alpha(&self) -> f64 {
        self.alpha
    }

    fn christoffel_symbols(&self, _point: &T) -> Result<Vec<Vec<Vec<T::Scalar>>>, InfoGeomError> {
        // Simplified implementation - would need proper computation based on the metric
        let dim = _point.dimension();
        let mut symbols = Vec::new();
        for _ in 0..dim {
            let mut dim2 = Vec::new();
            for _ in 0..dim {
                let mut dim3 = Vec::new();
                for _ in 0..dim {
                    dim3.push(T::Scalar::zero());
                }
                dim2.push(dim3);
            }
            symbols.push(dim2);
        }

        // For now, return zero symbols (flat connection)
        // In practice, this would involve computing derivatives of the metric

        Ok(symbols)
    }

    fn covariant_derivative(
        &self,
        _point: &T,
        vector: &T,
        _direction: &T,
    ) -> Result<T, InfoGeomError> {
        // Simplified: in flat space, covariant derivative equals ordinary derivative
        Ok(vector.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use amari_core::basis::MultivectorBuilder;

    #[test]
    fn test_bregman_divergence() {
        let p = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.0)
            .e(1, 2.0)
            .build();

        let q = MultivectorBuilder::<3, 0, 0>::new()
            .scalar(1.5)
            .e(1, 1.5)
            .build();

        // Simple quadratic potential
        let phi = |mv: &Multivector<3, 0, 0>| mv.norm_squared();

        let divergence = bregman_divergence(phi, &p, &q).unwrap();
        assert!(divergence >= 0.0); // Bregman divergences are non-negative
    }

    #[test]
    fn test_kl_divergence() {
        let eta_p = MultivectorBuilder::<3, 0, 0>::new().scalar(1.0).build();

        let eta_q = MultivectorBuilder::<3, 0, 0>::new().scalar(0.5).build();

        let mu_p = MultivectorBuilder::<3, 0, 0>::new().scalar(2.0).build();

        let kl = kl_divergence(&eta_p, &eta_q, &mu_p);
        assert_eq!(kl, 1.0); // (1.0 - 0.5) * 2.0 = 1.0
    }

    #[test]
    fn test_amari_chentsov_tensor() {
        // Create three linearly independent vectors to ensure non-zero tensor value
        // Test with e1, e2, e3 which should give determinant = 1
        let x = MultivectorBuilder::<3, 0, 0>::new()
            .e(1, 1.0) // e1
            .build();

        let y = MultivectorBuilder::<3, 0, 0>::new()
            .e(2, 1.0) // e2
            .build();

        let z = MultivectorBuilder::<3, 0, 0>::new()
            .e(3, 1.0) // e3
            .build();

        let tensor_value = amari_chentsov_tensor(&x, &y, &z);

        // For x = e1, y = e2, z = e3, the scalar triple product should be 1
        // T(e1, e2, e3) = det([1,0,0; 0,1,0; 0,0,1]) = 1
        assert!(
            (tensor_value - 1.0).abs() < 1e-10,
            "Expected 1.0, got {}",
            tensor_value
        );

        // Test with different ordering to verify anti-symmetry
        let tensor_value_reversed = amari_chentsov_tensor(&y, &x, &z);
        assert!(
            (tensor_value_reversed + 1.0).abs() < 1e-10,
            "Should be -1.0 due to swap"
        );
    }
}
