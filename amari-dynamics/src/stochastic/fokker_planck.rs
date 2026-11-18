//! Fokker-Planck equation solver
//!
//! The Fokker-Planck equation describes the time evolution of the probability
//! density function P(x, t) of a stochastic process.
//!
//! # Equation
//!
//! For an Itô SDE: dX = f(X)dt + √(2D) dW
//!
//! The corresponding Fokker-Planck equation is:
//! ```text
//! ∂P/∂t = -∇·(f(x)P) + D∇²P
//!       = -∑_i ∂(f_i P)/∂x_i + D ∑_i ∂²P/∂x_i²
//! ```
//!
//! # Steady State
//!
//! The stationary distribution P_s(x) satisfies:
//! ```text
//! -∇·(f(x)P_s) + D∇²P_s = 0
//! ```
//!
//! For gradient systems f(x) = -∇V(x), the stationary distribution is:
//! ```text
//! P_s(x) ∝ exp(-V(x)/D)
//! ```
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::stochastic::{FokkerPlanckSolver, FokkerPlanckConfig};
//!
//! let config = FokkerPlanckConfig::new_1d(-2.0, 2.0, 100, 0.1);
//! let solver = FokkerPlanckSolver::new(config);
//!
//! // Define drift (e.g., double-well potential)
//! let drift = |x: f64| x - x.powi(3);
//!
//! // Evolve probability density
//! let p = solver.evolve_1d(&drift, &initial_density, dt, steps)?;
//! ```

use crate::error::{DynamicsError, Result};
use crate::flow::DynamicalSystem;

/// Configuration for Fokker-Planck solver
#[derive(Debug, Clone)]
pub struct FokkerPlanckConfig {
    /// Domain bounds (for each dimension)
    pub bounds: Vec<(f64, f64)>,
    /// Number of grid points per dimension
    pub grid_points: Vec<usize>,
    /// Diffusion coefficient D
    pub diffusion: f64,
    /// Boundary conditions
    pub boundary: BoundaryCondition,
}

/// Boundary condition types
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum BoundaryCondition {
    /// Zero probability at boundaries (absorbing)
    #[default]
    Dirichlet,
    /// Zero flux at boundaries (reflecting)
    Neumann,
    /// Periodic boundaries
    Periodic,
}

impl FokkerPlanckConfig {
    /// Create 1D configuration
    pub fn new_1d(x_min: f64, x_max: f64, n_points: usize, diffusion: f64) -> Self {
        Self {
            bounds: vec![(x_min, x_max)],
            grid_points: vec![n_points],
            diffusion,
            boundary: BoundaryCondition::default(),
        }
    }

    /// Create 2D configuration
    pub fn new_2d(
        x_bounds: (f64, f64),
        y_bounds: (f64, f64),
        nx: usize,
        ny: usize,
        diffusion: f64,
    ) -> Self {
        Self {
            bounds: vec![x_bounds, y_bounds],
            grid_points: vec![nx, ny],
            diffusion,
            boundary: BoundaryCondition::default(),
        }
    }

    /// Set boundary condition
    pub fn with_boundary(mut self, bc: BoundaryCondition) -> Self {
        self.boundary = bc;
        self
    }

    /// Get grid spacing for dimension i
    pub fn dx(&self, dim: usize) -> f64 {
        let (min, max) = self.bounds[dim];
        (max - min) / (self.grid_points[dim] - 1) as f64
    }

    /// Get total number of grid points
    pub fn total_points(&self) -> usize {
        self.grid_points.iter().product()
    }

    /// Get dimensionality
    pub fn dimension(&self) -> usize {
        self.bounds.len()
    }
}

/// 1D Fokker-Planck solver using finite differences
#[derive(Debug, Clone)]
pub struct FokkerPlanck1D {
    /// Configuration
    pub config: FokkerPlanckConfig,
    /// Grid positions
    pub x: Vec<f64>,
    /// Current probability density
    pub density: Vec<f64>,
}

impl FokkerPlanck1D {
    /// Create a new 1D Fokker-Planck solver
    pub fn new(config: FokkerPlanckConfig) -> Result<Self> {
        if config.dimension() != 1 {
            return Err(DynamicsError::invalid_parameter(
                "FokkerPlanck1D requires 1D configuration",
            ));
        }

        let n = config.grid_points[0];
        let (x_min, x_max) = config.bounds[0];
        let dx = (x_max - x_min) / (n - 1) as f64;

        let x: Vec<f64> = (0..n).map(|i| x_min + i as f64 * dx).collect();
        let density = vec![0.0; n];

        Ok(Self { config, x, density })
    }

    /// Initialize with Gaussian distribution
    pub fn init_gaussian(&mut self, mean: f64, std: f64) {
        let norm = 1.0 / (std * (2.0 * std::f64::consts::PI).sqrt());
        for (i, &xi) in self.x.iter().enumerate() {
            let z = (xi - mean) / std;
            self.density[i] = norm * (-0.5 * z * z).exp();
        }
        self.normalize();
    }

    /// Initialize with delta-like distribution (narrow Gaussian)
    pub fn init_delta(&mut self, x0: f64) {
        let dx = self.config.dx(0);
        self.init_gaussian(x0, dx * 2.0);
    }

    /// Initialize with uniform distribution
    pub fn init_uniform(&mut self) {
        let n = self.density.len();
        let value = 1.0 / (n as f64 * self.config.dx(0));
        self.density.fill(value);
    }

    /// Normalize the density
    pub fn normalize(&mut self) {
        let dx = self.config.dx(0);
        let total: f64 = self.density.iter().sum::<f64>() * dx;
        if total > 1e-15 {
            for p in &mut self.density {
                *p /= total;
            }
        }
    }

    /// Evolve for one time step using explicit Euler
    ///
    /// Uses FTCS (Forward Time Centered Space) scheme
    #[allow(clippy::needless_range_loop)]
    pub fn step<F>(&mut self, drift: &F, dt: f64) -> Result<()>
    where
        F: Fn(f64) -> f64,
    {
        let n = self.density.len();
        let dx = self.config.dx(0);
        let d = self.config.diffusion;

        // CFL stability condition
        let cfl = d * dt / (dx * dx);
        if cfl > 0.5 {
            return Err(DynamicsError::numerical_instability(
                "Fokker-Planck step",
                format!("CFL condition violated: {} > 0.5", cfl),
            ));
        }

        let mut new_density = vec![0.0; n];

        // Interior points
        for i in 1..n - 1 {
            let xi = self.x[i];
            let f = drift(xi);

            // Central differences for diffusion: D * (P[i+1] - 2P[i] + P[i-1]) / dx²
            let diffusion_term =
                d * (self.density[i + 1] - 2.0 * self.density[i] + self.density[i - 1]) / (dx * dx);

            // Upwind scheme for advection: -∂(fP)/∂x
            let advection_term = if f >= 0.0 {
                // Backward difference when f > 0
                -(f * self.density[i] - drift(self.x[i - 1]) * self.density[i - 1]) / dx
            } else {
                // Forward difference when f < 0
                -(drift(self.x[i + 1]) * self.density[i + 1] - f * self.density[i]) / dx
            };

            new_density[i] = self.density[i] + dt * (diffusion_term + advection_term);
        }

        // Boundary conditions
        match self.config.boundary {
            BoundaryCondition::Dirichlet => {
                new_density[0] = 0.0;
                new_density[n - 1] = 0.0;
            }
            BoundaryCondition::Neumann => {
                // Zero flux: dP/dx = 0
                new_density[0] = new_density[1];
                new_density[n - 1] = new_density[n - 2];
            }
            BoundaryCondition::Periodic => {
                new_density[0] = new_density[n - 2];
                new_density[n - 1] = new_density[1];
            }
        }

        // Check for negative densities (can happen with explicit schemes)
        for p in &mut new_density {
            if *p < 0.0 {
                *p = 0.0;
            }
        }

        self.density = new_density;
        Ok(())
    }

    /// Evolve for multiple time steps
    pub fn evolve<F>(&mut self, drift: F, dt: f64, steps: usize) -> Result<()>
    where
        F: Fn(f64) -> f64,
    {
        for _ in 0..steps {
            self.step(&drift, dt)?;
        }
        Ok(())
    }

    /// Compute mean of the distribution
    pub fn mean(&self) -> f64 {
        let dx = self.config.dx(0);
        self.x
            .iter()
            .zip(self.density.iter())
            .map(|(&xi, &pi)| xi * pi)
            .sum::<f64>()
            * dx
    }

    /// Compute variance of the distribution
    pub fn variance(&self) -> f64 {
        let dx = self.config.dx(0);
        let mean = self.mean();
        self.x
            .iter()
            .zip(self.density.iter())
            .map(|(&xi, &pi)| (xi - mean).powi(2) * pi)
            .sum::<f64>()
            * dx
    }

    /// Compute entropy of the distribution
    pub fn entropy(&self) -> f64 {
        let dx = self.config.dx(0);
        -self
            .density
            .iter()
            .filter(|&&p| p > 1e-15)
            .map(|&p| p * p.ln())
            .sum::<f64>()
            * dx
    }

    /// Find the stationary distribution (if it exists)
    ///
    /// Iterates until convergence or max_iter
    pub fn find_stationary<F>(
        &mut self,
        drift: F,
        dt: f64,
        tolerance: f64,
        max_iter: usize,
    ) -> Result<usize>
    where
        F: Fn(f64) -> f64,
    {
        for iter in 0..max_iter {
            let old_density = self.density.clone();

            self.step(&drift, dt)?;
            self.normalize();

            // Check convergence
            let max_diff = old_density
                .iter()
                .zip(self.density.iter())
                .map(|(a, b)| (a - b).abs())
                .fold(0.0, f64::max);

            if max_diff < tolerance {
                return Ok(iter);
            }
        }

        Err(DynamicsError::numerical_instability(
            "Stationary distribution",
            "Did not converge within max iterations",
        ))
    }

    /// Compute stationary distribution analytically for gradient systems
    ///
    /// For f(x) = -dV/dx, P_s ∝ exp(-V(x)/D)
    pub fn boltzmann_distribution<V>(&mut self, potential: V)
    where
        V: Fn(f64) -> f64,
    {
        let d = self.config.diffusion;
        for (i, &xi) in self.x.iter().enumerate() {
            self.density[i] = (-potential(xi) / d).exp();
        }
        self.normalize();
    }

    /// Get probability at given position (interpolated)
    pub fn probability_at(&self, x: f64) -> f64 {
        let (x_min, x_max) = self.config.bounds[0];
        if x < x_min || x > x_max {
            return 0.0;
        }

        let dx = self.config.dx(0);
        let idx_f = (x - x_min) / dx;
        let idx = idx_f.floor() as usize;

        if idx >= self.density.len() - 1 {
            return self.density[self.density.len() - 1];
        }

        // Linear interpolation
        let frac = idx_f - idx as f64;
        self.density[idx] * (1.0 - frac) + self.density[idx + 1] * frac
    }
}

/// 2D Fokker-Planck solver
#[derive(Debug, Clone)]
pub struct FokkerPlanck2D {
    /// Configuration
    pub config: FokkerPlanckConfig,
    /// Grid positions x
    pub x: Vec<f64>,
    /// Grid positions y
    pub y: Vec<f64>,
    /// Current probability density (row-major: density[i*ny + j] = P(x_i, y_j))
    pub density: Vec<f64>,
}

impl FokkerPlanck2D {
    /// Create a new 2D Fokker-Planck solver
    pub fn new(config: FokkerPlanckConfig) -> Result<Self> {
        if config.dimension() != 2 {
            return Err(DynamicsError::invalid_parameter(
                "FokkerPlanck2D requires 2D configuration",
            ));
        }

        let nx = config.grid_points[0];
        let ny = config.grid_points[1];

        let (x_min, x_max) = config.bounds[0];
        let (y_min, y_max) = config.bounds[1];

        let dx = (x_max - x_min) / (nx - 1) as f64;
        let dy = (y_max - y_min) / (ny - 1) as f64;

        let x: Vec<f64> = (0..nx).map(|i| x_min + i as f64 * dx).collect();
        let y: Vec<f64> = (0..ny).map(|j| y_min + j as f64 * dy).collect();
        let density = vec![0.0; nx * ny];

        Ok(Self {
            config,
            x,
            y,
            density,
        })
    }

    /// Get index in flattened array
    fn index(&self, i: usize, j: usize) -> usize {
        let ny = self.config.grid_points[1];
        i * ny + j
    }

    /// Initialize with 2D Gaussian
    pub fn init_gaussian(&mut self, mean_x: f64, mean_y: f64, std_x: f64, std_y: f64) {
        let nx = self.config.grid_points[0];
        let ny = self.config.grid_points[1];

        for i in 0..nx {
            for j in 0..ny {
                let zx = (self.x[i] - mean_x) / std_x;
                let zy = (self.y[j] - mean_y) / std_y;
                let idx = self.index(i, j);
                self.density[idx] = (-0.5 * (zx * zx + zy * zy)).exp();
            }
        }
        self.normalize();
    }

    /// Normalize the density
    pub fn normalize(&mut self) {
        let dx = self.config.dx(0);
        let dy = self.config.dx(1);
        let total: f64 = self.density.iter().sum::<f64>() * dx * dy;
        if total > 1e-15 {
            for p in &mut self.density {
                *p /= total;
            }
        }
    }

    /// Evolve for one time step
    #[allow(clippy::needless_range_loop)]
    pub fn step<F>(&mut self, drift: &F, dt: f64) -> Result<()>
    where
        F: Fn(f64, f64) -> (f64, f64),
    {
        let nx = self.config.grid_points[0];
        let ny = self.config.grid_points[1];
        let dx = self.config.dx(0);
        let dy = self.config.dx(1);
        let d = self.config.diffusion;

        // CFL condition
        let cfl = d * dt * (1.0 / (dx * dx) + 1.0 / (dy * dy));
        if cfl > 0.5 {
            return Err(DynamicsError::numerical_instability(
                "Fokker-Planck 2D step",
                format!("CFL condition violated: {} > 0.5", cfl),
            ));
        }

        let mut new_density = vec![0.0; nx * ny];

        // Interior points
        for i in 1..nx - 1 {
            for j in 1..ny - 1 {
                let idx = self.index(i, j);
                let (fx, fy) = drift(self.x[i], self.y[j]);

                // Diffusion terms (central differences)
                let laplacian_x = (self.density[self.index(i + 1, j)] - 2.0 * self.density[idx]
                    + self.density[self.index(i - 1, j)])
                    / (dx * dx);
                let laplacian_y = (self.density[self.index(i, j + 1)] - 2.0 * self.density[idx]
                    + self.density[self.index(i, j - 1)])
                    / (dy * dy);
                let diffusion_term = d * (laplacian_x + laplacian_y);

                // Advection terms (upwind scheme)
                let advection_x = if fx >= 0.0 {
                    let (fx_m, _) = drift(self.x[i - 1], self.y[j]);
                    -(fx * self.density[idx] - fx_m * self.density[self.index(i - 1, j)]) / dx
                } else {
                    let (fx_p, _) = drift(self.x[i + 1], self.y[j]);
                    -(fx_p * self.density[self.index(i + 1, j)] - fx * self.density[idx]) / dx
                };

                let advection_y = if fy >= 0.0 {
                    let (_, fy_m) = drift(self.x[i], self.y[j - 1]);
                    -(fy * self.density[idx] - fy_m * self.density[self.index(i, j - 1)]) / dy
                } else {
                    let (_, fy_p) = drift(self.x[i], self.y[j + 1]);
                    -(fy_p * self.density[self.index(i, j + 1)] - fy * self.density[idx]) / dy
                };

                new_density[idx] =
                    self.density[idx] + dt * (diffusion_term + advection_x + advection_y);
            }
        }

        // Boundary conditions (Dirichlet by default)
        for p in &mut new_density {
            if *p < 0.0 {
                *p = 0.0;
            }
        }

        self.density = new_density;
        Ok(())
    }

    /// Marginal distribution in x
    #[allow(clippy::needless_range_loop)]
    pub fn marginal_x(&self) -> Vec<f64> {
        let nx = self.config.grid_points[0];
        let ny = self.config.grid_points[1];
        let dy = self.config.dx(1);

        let mut marginal = vec![0.0; nx];
        for i in 0..nx {
            for j in 0..ny {
                marginal[i] += self.density[self.index(i, j)];
            }
            marginal[i] *= dy;
        }
        marginal
    }

    /// Marginal distribution in y
    #[allow(clippy::needless_range_loop)]
    pub fn marginal_y(&self) -> Vec<f64> {
        let nx = self.config.grid_points[0];
        let ny = self.config.grid_points[1];
        let dx = self.config.dx(0);

        let mut marginal = vec![0.0; ny];
        for j in 0..ny {
            for i in 0..nx {
                marginal[j] += self.density[self.index(i, j)];
            }
            marginal[j] *= dx;
        }
        marginal
    }
}

/// Create Fokker-Planck solver from a dynamical system
#[allow(dead_code)]
pub fn fokker_planck_from_system<S, const P: usize, const Q: usize, const R: usize>(
    _system: &S,
    _config: FokkerPlanckConfig,
) -> Result<FokkerPlanck1D>
where
    S: DynamicalSystem<P, Q, R>,
{
    // This would extract the drift from the dynamical system
    // For now, just create a basic solver
    Err(DynamicsError::invalid_parameter(
        "Generic system to Fokker-Planck conversion not yet implemented",
    ))
}

/// Compute Kramers escape rate from a potential well
///
/// For a particle in potential V(x) with diffusion D,
/// the escape rate from a metastable well is:
///
/// r ≈ (ω_well * ω_barrier) / (2π) * exp(-ΔV/D)
///
/// where ΔV is the barrier height.
pub fn kramers_rate(
    barrier_height: f64,
    diffusion: f64,
    omega_well: f64,
    omega_barrier: f64,
) -> f64 {
    (omega_well * omega_barrier) / (2.0 * std::f64::consts::PI)
        * (-barrier_height / diffusion).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fokker_planck_config_1d() {
        let config = FokkerPlanckConfig::new_1d(-2.0, 2.0, 100, 0.1);
        assert_eq!(config.dimension(), 1);
        assert_eq!(config.total_points(), 100);
        assert!((config.dx(0) - 4.0 / 99.0).abs() < 1e-10);
    }

    #[test]
    fn test_fokker_planck_config_2d() {
        let config = FokkerPlanckConfig::new_2d((-2.0, 2.0), (-1.0, 1.0), 50, 40, 0.1);
        assert_eq!(config.dimension(), 2);
        assert_eq!(config.total_points(), 2000);
    }

    #[test]
    fn test_fokker_planck_1d_creation() {
        let config = FokkerPlanckConfig::new_1d(-2.0, 2.0, 100, 0.1);
        let solver = FokkerPlanck1D::new(config).unwrap();
        assert_eq!(solver.x.len(), 100);
        assert_eq!(solver.density.len(), 100);
    }

    #[test]
    fn test_gaussian_initialization() {
        let config = FokkerPlanckConfig::new_1d(-5.0, 5.0, 200, 0.1);
        let mut solver = FokkerPlanck1D::new(config).unwrap();
        solver.init_gaussian(0.0, 1.0);

        // Check normalization
        let dx = solver.config.dx(0);
        let total: f64 = solver.density.iter().sum::<f64>() * dx;
        assert!((total - 1.0).abs() < 0.01);

        // Check mean is near 0
        assert!(solver.mean().abs() < 0.1);

        // Check variance is near 1
        assert!((solver.variance() - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_fokker_planck_step() {
        let config = FokkerPlanckConfig::new_1d(-3.0, 3.0, 100, 0.1);
        let mut solver = FokkerPlanck1D::new(config).unwrap();
        solver.init_gaussian(0.0, 0.5);

        // Simple drift towards origin
        let drift = |x: f64| -x;

        // Take a few steps
        let dt = 0.001;
        for _ in 0..10 {
            solver.step(&drift, dt).unwrap();
        }

        // Distribution should still be normalized (approximately)
        solver.normalize();
        let dx = solver.config.dx(0);
        let total: f64 = solver.density.iter().sum::<f64>() * dx;
        assert!((total - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_boltzmann_distribution() {
        let config = FokkerPlanckConfig::new_1d(-3.0, 3.0, 200, 0.5);
        let mut solver = FokkerPlanck1D::new(config).unwrap();

        // Harmonic potential V(x) = x²/2
        let potential = |x: f64| 0.5 * x * x;
        solver.boltzmann_distribution(potential);

        // Should be a Gaussian centered at 0 with variance D
        // For V = x²/2 and D = 0.5, variance = D = 0.5
        assert!(solver.mean().abs() < 0.1);
        assert!((solver.variance() - 0.5).abs() < 0.1);
    }

    #[test]
    fn test_probability_interpolation() {
        let config = FokkerPlanckConfig::new_1d(-2.0, 2.0, 100, 0.1);
        let mut solver = FokkerPlanck1D::new(config).unwrap();
        solver.init_gaussian(0.0, 1.0);

        // Check interpolation at grid points
        let p0 = solver.probability_at(0.0);
        assert!(p0 > 0.0);

        // Check boundary handling
        let p_outside = solver.probability_at(-10.0);
        assert_eq!(p_outside, 0.0);
    }

    #[test]
    fn test_entropy() {
        let config = FokkerPlanckConfig::new_1d(-5.0, 5.0, 200, 0.1);
        let mut solver = FokkerPlanck1D::new(config).unwrap();
        solver.init_gaussian(0.0, 1.0);

        let entropy = solver.entropy();

        // Entropy of Gaussian with σ=1: S = ½ + ½ln(2π) ≈ 1.42
        assert!((entropy - 1.42).abs() < 0.2);
    }

    #[test]
    fn test_fokker_planck_2d() {
        let config = FokkerPlanckConfig::new_2d((-2.0, 2.0), (-2.0, 2.0), 50, 50, 0.1);
        let mut solver = FokkerPlanck2D::new(config).unwrap();
        solver.init_gaussian(0.0, 0.0, 0.5, 0.5);

        // Check normalization
        let dx = solver.config.dx(0);
        let dy = solver.config.dx(1);
        let total: f64 = solver.density.iter().sum::<f64>() * dx * dy;
        assert!((total - 1.0).abs() < 0.05);
    }

    #[test]
    fn test_marginal_distributions() {
        let config = FokkerPlanckConfig::new_2d((-3.0, 3.0), (-3.0, 3.0), 60, 60, 0.1);
        let mut solver = FokkerPlanck2D::new(config).unwrap();
        solver.init_gaussian(0.0, 0.0, 1.0, 1.0);

        let marginal_x = solver.marginal_x();
        let marginal_y = solver.marginal_y();

        assert_eq!(marginal_x.len(), 60);
        assert_eq!(marginal_y.len(), 60);

        // Marginals should integrate to 1
        let dx = solver.config.dx(0);
        let dy = solver.config.dx(1);
        let total_x: f64 = marginal_x.iter().sum::<f64>() * dx;
        let total_y: f64 = marginal_y.iter().sum::<f64>() * dy;
        assert!((total_x - 1.0).abs() < 0.1);
        assert!((total_y - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_kramers_rate() {
        // Test Kramers escape rate formula
        let barrier = 1.0;
        let diffusion = 0.1;
        let omega_well = 1.0;
        let omega_barrier = 1.0;

        let rate = kramers_rate(barrier, diffusion, omega_well, omega_barrier);

        // Rate should be positive and small for high barrier
        assert!(rate > 0.0);
        assert!(rate < 1.0); // With barrier/D = 10, rate is very small
    }

    #[test]
    fn test_cfl_violation() {
        let config = FokkerPlanckConfig::new_1d(-1.0, 1.0, 10, 1.0);
        let mut solver = FokkerPlanck1D::new(config).unwrap();
        solver.init_gaussian(0.0, 0.3);

        let drift = |_x: f64| 0.0;

        // Large dt should violate CFL
        let result = solver.step(&drift, 1.0);
        assert!(result.is_err());
    }
}
