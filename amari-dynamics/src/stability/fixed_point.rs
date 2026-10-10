//! Fixed point finding algorithms
//!
//! This module provides methods for finding fixed points (equilibria) of
//! dynamical systems, where dx/dt = f(x) = 0.
//!
//! # Algorithms
//!
//! - Newton's method for fast convergence near fixed points
//! - Newton with damping for improved global convergence
//! - Multiple initial conditions search for finding all fixed points
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::stability::{find_fixed_point, FixedPointConfig};
//!
//! let system = MySystem::new();
//! let initial_guess = Multivector::zero();
//! let fp = find_fixed_point(&system, &initial_guess, &FixedPointConfig::default())?;
//! ```

use amari_core::Multivector;
use nalgebra::{DMatrix, DVector};

use crate::error::{DynamicsError, Result};
use crate::flow::DynamicalSystem;

/// Configuration for fixed point finding algorithms
#[derive(Debug, Clone)]
pub struct FixedPointConfig {
    /// Maximum number of Newton iterations
    pub max_iterations: usize,
    /// Convergence tolerance for |f(x)|
    pub tolerance: f64,
    /// Step size damping factor (0 < damping <= 1)
    pub damping: f64,
    /// Minimum step size before declaring failure
    pub min_step: f64,
    /// Whether to use line search for step size
    pub line_search: bool,
}

impl Default for FixedPointConfig {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            tolerance: 1e-10,
            damping: 1.0,
            min_step: 1e-14,
            line_search: false,
        }
    }
}

impl FixedPointConfig {
    /// Create configuration for high precision fixed point finding
    pub fn high_precision() -> Self {
        Self {
            max_iterations: 200,
            tolerance: 1e-14,
            damping: 1.0,
            min_step: 1e-16,
            line_search: false,
        }
    }

    /// Create configuration with damping for difficult systems
    pub fn with_damping(damping: f64) -> Self {
        Self {
            damping: damping.clamp(0.01, 1.0),
            line_search: true,
            ..Default::default()
        }
    }
}

/// Result of a fixed point search
#[derive(Debug, Clone)]
pub struct FixedPointResult<const P: usize, const Q: usize, const R: usize> {
    /// The fixed point location
    pub point: Multivector<P, Q, R>,
    /// Residual |f(x*)|
    pub residual: f64,
    /// Number of iterations used
    pub iterations: usize,
    /// Whether the algorithm converged
    pub converged: bool,
}

impl<const P: usize, const Q: usize, const R: usize> FixedPointResult<P, Q, R> {
    /// Check if this is a valid fixed point (converged with small residual)
    pub fn is_valid(&self, tolerance: f64) -> bool {
        self.converged && self.residual < tolerance
    }
}

/// Find a fixed point of a dynamical system using Newton's method
///
/// Solves f(x) = 0 using Newton iteration:
/// ```text
/// x_{n+1} = x_n - J^{-1}(x_n) * f(x_n)
/// ```
/// where J is the Jacobian matrix.
///
/// # Arguments
///
/// * `system` - The dynamical system
/// * `initial` - Initial guess for the fixed point
/// * `config` - Configuration parameters
///
/// # Returns
///
/// Result containing the fixed point and convergence information.
///
/// # Errors
///
/// Returns an error if:
/// - The Jacobian is singular
/// - Maximum iterations exceeded without convergence
/// - Numerical issues occur
pub fn find_fixed_point<S, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    initial: &Multivector<P, Q, R>,
    config: &FixedPointConfig,
) -> Result<FixedPointResult<P, Q, R>>
where
    S: DynamicalSystem<P, Q, R>,
{
    let dim = S::DIM;
    let mut x = initial.clone();

    for iteration in 0..config.max_iterations {
        // Evaluate vector field
        let f = system.vector_field(&x)?;
        let f_vec = multivector_to_dvector(&f);
        let residual = f_vec.norm();

        // Check convergence
        if residual < config.tolerance {
            return Ok(FixedPointResult {
                point: x,
                residual,
                iterations: iteration,
                converged: true,
            });
        }

        // Compute Jacobian
        let jac = system.jacobian(&x)?;
        let jac_matrix = DMatrix::from_row_slice(dim, dim, &jac);

        // Solve J * delta = -f for delta
        // Use SVD-based pseudo-inverse for robustness with near-singular Jacobians
        // This handles systems where dynamics are confined to a subspace
        let neg_f = -&f_vec;

        let svd = jac_matrix.clone().svd(true, true);
        let tolerance = 1e-10 * dim as f64;

        let delta = match svd.solve(&neg_f, tolerance) {
            Ok(d) => d,
            Err(_) => {
                return Err(DynamicsError::jacobian_error("Jacobian is singular"));
            }
        };

        // Apply damping and update
        let step_size = if config.line_search {
            line_search(system, &x, &delta, &f_vec, config.damping)?
        } else {
            config.damping
        };

        // Compute scaled delta
        let scaled_delta = &delta * step_size;
        let step_norm = scaled_delta.norm();

        // Update state
        let delta_mv = dvector_to_multivector::<P, Q, R>(&scaled_delta);
        x = &x + &delta_mv;

        // Check for very small steps
        if step_norm < config.min_step {
            let final_f = system.vector_field(&x)?;
            let final_residual = multivector_to_dvector(&final_f).norm();

            return Ok(FixedPointResult {
                point: x,
                residual: final_residual,
                iterations: iteration + 1,
                converged: final_residual < config.tolerance,
            });
        }
    }

    // Max iterations reached
    let final_f = system.vector_field(&x)?;
    let final_residual = multivector_to_dvector(&final_f).norm();

    Ok(FixedPointResult {
        point: x,
        residual: final_residual,
        iterations: config.max_iterations,
        converged: false,
    })
}

/// Simple backtracking line search
fn line_search<S, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    x: &Multivector<P, Q, R>,
    delta: &DVector<f64>,
    f_current: &DVector<f64>,
    max_step: f64,
) -> Result<f64>
where
    S: DynamicalSystem<P, Q, R>,
{
    let current_norm = f_current.norm_squared();
    let mut step = max_step;
    let c = 1e-4; // Armijo condition constant

    for _ in 0..20 {
        let delta_mv = dvector_to_multivector::<P, Q, R>(&(delta * step));
        let x_new = x + &delta_mv;
        let f_new = system.vector_field(&x_new)?;
        let new_norm = multivector_to_dvector(&f_new).norm_squared();

        // Armijo condition: f(x + step*d)^2 <= f(x)^2 + c * step * 2 * f(x)' * J * d
        // Simplified: just check if we reduced the norm
        if new_norm < current_norm * (1.0 - c * step) {
            return Ok(step);
        }

        step *= 0.5;
    }

    Ok(step) // Return smallest step tried
}

/// Find multiple fixed points by searching from multiple initial conditions
///
/// # Arguments
///
/// * `system` - The dynamical system
/// * `initial_conditions` - Vector of initial guesses
/// * `config` - Configuration parameters
/// * `merge_tolerance` - Distance below which two fixed points are considered the same
///
/// # Returns
///
/// Vector of unique fixed points found
pub fn find_fixed_points<S, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    initial_conditions: &[Multivector<P, Q, R>],
    config: &FixedPointConfig,
    merge_tolerance: f64,
) -> Result<Vec<FixedPointResult<P, Q, R>>>
where
    S: DynamicalSystem<P, Q, R>,
{
    let mut fixed_points: Vec<FixedPointResult<P, Q, R>> = Vec::new();

    for initial in initial_conditions {
        match find_fixed_point(system, initial, config) {
            Ok(result) if result.converged => {
                // Check if this is a new fixed point
                let is_new = fixed_points.iter().all(|fp| {
                    let diff = &result.point - &fp.point;
                    diff.coefficient_norm() > merge_tolerance
                });

                if is_new {
                    fixed_points.push(result);
                }
            }
            Ok(_) => {}  // Didn't converge, skip
            Err(_) => {} // Error, skip
        }
    }

    Ok(fixed_points)
}

/// Generate a grid of initial conditions for fixed point search
///
/// # Arguments
///
/// * `center` - Center of the search region
/// * `radius` - Radius of search region in each dimension
/// * `points_per_dim` - Number of points per dimension
///
/// # Returns
///
/// Vector of initial conditions on a grid
pub fn generate_initial_conditions<const P: usize, const Q: usize, const R: usize>(
    center: &Multivector<P, Q, R>,
    radius: f64,
    points_per_dim: usize,
) -> Vec<Multivector<P, Q, R>> {
    let dim = 1 << (P + Q + R);

    if points_per_dim == 0 {
        return vec![center.clone()];
    }

    // For high dimensions, just sample along each axis
    let mut conditions = Vec::new();
    conditions.push(center.clone());

    let step = if points_per_dim > 1 {
        2.0 * radius / (points_per_dim - 1) as f64
    } else {
        0.0
    };

    for d in 0..dim {
        for i in 0..points_per_dim {
            let offset = -radius + i as f64 * step;
            if offset.abs() > 1e-10 {
                let mut point = center.clone();
                let current = point.get(d);
                point.set(d, current + offset);
                conditions.push(point);
            }
        }
    }

    conditions
}

// Helper functions for nalgebra conversion

fn multivector_to_dvector<const P: usize, const Q: usize, const R: usize>(
    mv: &Multivector<P, Q, R>,
) -> DVector<f64> {
    let dim = 1 << (P + Q + R);
    let mut vec = DVector::zeros(dim);
    for i in 0..dim {
        vec[i] = mv.get(i);
    }
    vec
}

fn dvector_to_multivector<const P: usize, const Q: usize, const R: usize>(
    vec: &DVector<f64>,
) -> Multivector<P, Q, R> {
    let dim = 1 << (P + Q + R);
    let mut coeffs = vec![0.0; dim];
    for i in 0..dim.min(vec.len()) {
        coeffs[i] = vec[i];
    }
    Multivector::from_coefficients(coeffs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::HarmonicOscillator;

    // Linear system with fixed point at origin
    // dx/dt = -x, dy/dt = -2y
    // Fixed point at (0, 0)
    struct LinearSystem;

    impl DynamicalSystem<2, 0, 0> for LinearSystem {
        fn vector_field(&self, state: &Multivector<2, 0, 0>) -> Result<Multivector<2, 0, 0>> {
            let x = state.get(1);
            let y = state.get(2);

            let mut result = Multivector::zero();
            result.set(1, -x);
            result.set(2, -2.0 * y);

            Ok(result)
        }
    }

    // Saddle point system
    // dx/dt = x, dy/dt = -y
    // Saddle at origin
    struct SaddleSystem;

    impl DynamicalSystem<2, 0, 0> for SaddleSystem {
        fn vector_field(&self, state: &Multivector<2, 0, 0>) -> Result<Multivector<2, 0, 0>> {
            let x = state.get(1);
            let y = state.get(2);

            let mut result = Multivector::zero();
            result.set(1, x);
            result.set(2, -y);

            Ok(result)
        }
    }

    // System with multiple fixed points
    // dx/dt = x(1-x), dy/dt = -y
    // Fixed points at (0, 0) and (1, 0)
    struct MultipleFixedPoints;

    impl DynamicalSystem<2, 0, 0> for MultipleFixedPoints {
        fn vector_field(&self, state: &Multivector<2, 0, 0>) -> Result<Multivector<2, 0, 0>> {
            let x = state.get(1);
            let y = state.get(2);

            let mut result = Multivector::zero();
            result.set(1, x * (1.0 - x));
            result.set(2, -y);

            Ok(result)
        }
    }

    #[test]
    fn test_find_fixed_point_linear() {
        let system = LinearSystem;
        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0);
        initial.set(2, 1.0);

        let result = find_fixed_point(&system, &initial, &FixedPointConfig::default()).unwrap();

        assert!(result.converged);
        assert!(result.residual < 1e-10);
        assert!(result.point.get(1).abs() < 1e-8);
        assert!(result.point.get(2).abs() < 1e-8);
    }

    #[test]
    fn test_find_fixed_point_saddle() {
        let system = SaddleSystem;
        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 0.1);
        initial.set(2, 0.1);

        let result = find_fixed_point(&system, &initial, &FixedPointConfig::default()).unwrap();

        assert!(result.converged);
        assert!(result.point.get(1).abs() < 1e-8);
        assert!(result.point.get(2).abs() < 1e-8);
    }

    #[test]
    fn test_find_multiple_fixed_points() {
        let system = MultipleFixedPoints;

        let initials = vec![
            {
                let mut m = Multivector::<2, 0, 0>::zero();
                m.set(1, 0.1);
                m
            },
            {
                let mut m = Multivector::<2, 0, 0>::zero();
                m.set(1, 0.9);
                m
            },
        ];

        let results =
            find_fixed_points(&system, &initials, &FixedPointConfig::default(), 0.1).unwrap();

        assert_eq!(results.len(), 2);

        // Check both fixed points were found
        let fp_values: Vec<f64> = results.iter().map(|r| r.point.get(1)).collect();

        assert!(fp_values.iter().any(|&x| x.abs() < 0.01)); // Origin
        assert!(fp_values.iter().any(|&x| (x - 1.0).abs() < 0.01)); // (1, 0)
    }

    #[test]
    fn test_harmonic_oscillator_fixed_point() {
        // Harmonic oscillator has fixed point at origin
        let system = HarmonicOscillator::new(1.0);
        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 0.5);
        initial.set(2, 0.5);

        let result = find_fixed_point(&system, &initial, &FixedPointConfig::default()).unwrap();

        assert!(result.converged);
        assert!(result.point.get(1).abs() < 1e-8);
        assert!(result.point.get(2).abs() < 1e-8);
    }

    #[test]
    fn test_fixed_point_config_default() {
        let config = FixedPointConfig::default();
        assert_eq!(config.max_iterations, 100);
        assert!(config.tolerance > 0.0);
        assert!(config.damping > 0.0 && config.damping <= 1.0);
    }

    #[test]
    fn test_generate_initial_conditions() {
        let center = Multivector::<2, 0, 0>::zero();
        let conditions = generate_initial_conditions(&center, 1.0, 3);

        // Should have center plus samples along each axis
        assert!(!conditions.is_empty());
        assert!(conditions.len() > 1);
    }
}
