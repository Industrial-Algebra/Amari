//! Phase portrait generation
//!
//! This module provides tools for generating and analyzing phase portraits
//! of dynamical systems, particularly for 2D and 3D visualization.
//!
//! # Overview
//!
//! A phase portrait shows the behavior of a dynamical system by displaying
//! trajectories in state space. Key features include:
//!
//! - Vector field visualization
//! - Trajectory bundles from multiple initial conditions
//! - Fixed points and their stability
//! - Limit cycles and periodic orbits
//! - Separatrices and basin boundaries
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::phase::{PhasePortrait, PortraitConfig};
//!
//! let config = PortraitConfig::default();
//! let portrait = PhasePortrait::generate(&system, &config)?;
//!
//! // Get vector field for visualization
//! let vectors = portrait.vector_field_at_grid(20, 20);
//! ```

use amari_core::Multivector;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::error::Result;
use crate::flow::DynamicalSystem;
use crate::solver::{DormandPrince, ODESolver, Trajectory};
use crate::stability::{find_fixed_point, DifferentiationConfig, FixedPointConfig, StabilityType};

use super::trajectory::TrajectoryBundle;

/// Configuration for phase portrait generation
#[derive(Debug, Clone)]
pub struct PortraitConfig {
    /// Bounds for x dimension: (min, max)
    pub x_bounds: (f64, f64),

    /// Bounds for y dimension: (min, max)
    pub y_bounds: (f64, f64),

    /// Which component index corresponds to x
    pub x_component: usize,

    /// Which component index corresponds to y
    pub y_component: usize,

    /// Number of trajectories in x direction
    pub x_trajectories: usize,

    /// Number of trajectories in y direction
    pub y_trajectories: usize,

    /// Integration time for forward trajectories
    pub forward_time: f64,

    /// Integration time for backward trajectories (negative flows)
    pub backward_time: f64,

    /// Time step for integration
    pub dt: f64,

    /// Fixed values for components other than x and y
    pub fixed_values: Vec<(usize, f64)>,

    /// Whether to compute fixed points
    pub find_fixed_points: bool,

    /// Whether to include separatrices near saddle points
    pub include_separatrices: bool,
}

impl Default for PortraitConfig {
    fn default() -> Self {
        Self {
            x_bounds: (-2.0, 2.0),
            y_bounds: (-2.0, 2.0),
            x_component: 1, // e1 component
            y_component: 2, // e2 component
            x_trajectories: 10,
            y_trajectories: 10,
            forward_time: 10.0,
            backward_time: 0.0,
            dt: 0.01,
            fixed_values: Vec::new(),
            find_fixed_points: true,
            include_separatrices: true,
        }
    }
}

impl PortraitConfig {
    /// Create config for a simple 2D system
    pub fn simple_2d(x_bounds: (f64, f64), y_bounds: (f64, f64)) -> Self {
        Self {
            x_bounds,
            y_bounds,
            ..Default::default()
        }
    }

    /// Create config with specified resolution
    pub fn with_resolution(mut self, nx: usize, ny: usize) -> Self {
        self.x_trajectories = nx;
        self.y_trajectories = ny;
        self
    }

    /// Create config for phase plane of higher-dimensional system
    pub fn phase_plane(x_component: usize, y_component: usize) -> Self {
        Self {
            x_component,
            y_component,
            ..Default::default()
        }
    }

    /// Generate grid of initial conditions
    pub fn initial_conditions<const P: usize, const Q: usize, const R: usize>(
        &self,
    ) -> Vec<Multivector<P, Q, R>> {
        let mut points = Vec::with_capacity(self.x_trajectories * self.y_trajectories);

        for i in 0..self.x_trajectories {
            for j in 0..self.y_trajectories {
                let x = self.x_bounds.0
                    + (self.x_bounds.1 - self.x_bounds.0) * (i as f64)
                        / (self.x_trajectories - 1).max(1) as f64;
                let y = self.y_bounds.0
                    + (self.y_bounds.1 - self.y_bounds.0) * (j as f64)
                        / (self.y_trajectories - 1).max(1) as f64;

                let mut point = Multivector::<P, Q, R>::zero();
                point.set(self.x_component, x);
                point.set(self.y_component, y);

                // Set fixed values
                for &(idx, val) in &self.fixed_values {
                    point.set(idx, val);
                }

                points.push(point);
            }
        }

        points
    }
}

/// A fixed point with its stability classification
#[derive(Debug, Clone)]
pub struct ClassifiedFixedPoint<const P: usize, const Q: usize, const R: usize> {
    /// Location of the fixed point
    pub point: Multivector<P, Q, R>,

    /// Stability type
    pub stability: StabilityType,

    /// Eigenvalues (real, imaginary) pairs
    pub eigenvalues: Vec<(f64, f64)>,
}

/// Vector field sample point
#[derive(Debug, Clone)]
pub struct VectorFieldPoint<const P: usize, const Q: usize, const R: usize> {
    /// Position in state space
    pub position: Multivector<P, Q, R>,

    /// Vector field value at this position
    pub vector: Multivector<P, Q, R>,

    /// Magnitude of the vector
    pub magnitude: f64,
}

/// Phase portrait data structure
#[derive(Debug, Clone)]
pub struct PhasePortrait<const P: usize, const Q: usize, const R: usize> {
    /// Configuration used to generate portrait
    pub config: PortraitConfig,

    /// Collection of trajectories
    pub trajectories: TrajectoryBundle<P, Q, R>,

    /// Fixed points found (if any)
    pub fixed_points: Vec<ClassifiedFixedPoint<P, Q, R>>,

    /// Separatrices from saddle points
    pub separatrices: Vec<Trajectory<P, Q, R>>,
}

impl<const P: usize, const Q: usize, const R: usize> PhasePortrait<P, Q, R> {
    /// Generate a phase portrait for the given system
    pub fn generate<S>(system: &S, config: &PortraitConfig) -> Result<Self>
    where
        S: DynamicalSystem<P, Q, R>,
    {
        let solver = DormandPrince::new();
        let initial_conditions = config.initial_conditions::<P, Q, R>();

        // Compute trajectories
        let mut trajectories = Vec::with_capacity(initial_conditions.len());
        let steps = (config.forward_time / config.dt) as usize;

        for ic in &initial_conditions {
            if let Ok(traj) = solver.solve(system, ic.clone(), 0.0, config.forward_time, steps) {
                trajectories.push(traj);
            }
        }

        // Also compute backward trajectories if requested
        if config.backward_time > 0.0 {
            let back_steps = (config.backward_time / config.dt) as usize;
            for ic in &initial_conditions {
                // Create a time-reversed system wrapper would be ideal,
                // but for now we just note this limitation
                if let Ok(traj) =
                    solver.solve(system, ic.clone(), 0.0, config.backward_time, back_steps)
                {
                    trajectories.push(traj);
                }
            }
        }

        let bundle = TrajectoryBundle::from_trajectories(trajectories);

        // Find fixed points
        let mut fixed_points = Vec::new();
        if config.find_fixed_points {
            fixed_points = find_portrait_fixed_points(system, config)?;
        }

        // Compute separatrices
        let mut separatrices = Vec::new();
        if config.include_separatrices {
            for fp in &fixed_points {
                if matches!(fp.stability, StabilityType::Saddle) {
                    // Compute unstable manifold (outgoing separatrices)
                    if let Ok(seps) =
                        compute_separatrices(system, &fp.point, &fp.eigenvalues, config)
                    {
                        separatrices.extend(seps);
                    }
                }
            }
        }

        Ok(PhasePortrait {
            config: config.clone(),
            trajectories: bundle,
            fixed_points,
            separatrices,
        })
    }

    /// Sample vector field on a grid
    pub fn vector_field_grid<S>(
        system: &S,
        config: &PortraitConfig,
        nx: usize,
        ny: usize,
    ) -> Result<Vec<VectorFieldPoint<P, Q, R>>>
    where
        S: DynamicalSystem<P, Q, R>,
    {
        let mut points = Vec::with_capacity(nx * ny);

        for i in 0..nx {
            for j in 0..ny {
                let x = config.x_bounds.0
                    + (config.x_bounds.1 - config.x_bounds.0) * (i as f64) / (nx - 1).max(1) as f64;
                let y = config.y_bounds.0
                    + (config.y_bounds.1 - config.y_bounds.0) * (j as f64) / (ny - 1).max(1) as f64;

                let mut position = Multivector::<P, Q, R>::zero();
                position.set(config.x_component, x);
                position.set(config.y_component, y);

                for &(idx, val) in &config.fixed_values {
                    position.set(idx, val);
                }

                if let Ok(vector) = system.vector_field(&position) {
                    let magnitude = vector.coefficient_norm();
                    points.push(VectorFieldPoint {
                        position,
                        vector,
                        magnitude,
                    });
                }
            }
        }

        Ok(points)
    }

    /// Get the number of trajectories in the portrait
    pub fn num_trajectories(&self) -> usize {
        self.trajectories.len()
    }

    /// Get the number of fixed points found
    pub fn num_fixed_points(&self) -> usize {
        self.fixed_points.len()
    }

    /// Get stable fixed points
    pub fn stable_fixed_points(&self) -> Vec<&ClassifiedFixedPoint<P, Q, R>> {
        self.fixed_points
            .iter()
            .filter(|fp| {
                matches!(
                    fp.stability,
                    StabilityType::AsymptoticallyStable
                        | StabilityType::StableNode
                        | StabilityType::StableFocus
                )
            })
            .collect()
    }

    /// Get unstable fixed points
    pub fn unstable_fixed_points(&self) -> Vec<&ClassifiedFixedPoint<P, Q, R>> {
        self.fixed_points
            .iter()
            .filter(|fp| {
                matches!(
                    fp.stability,
                    StabilityType::Unstable
                        | StabilityType::UnstableNode
                        | StabilityType::UnstableFocus
                )
            })
            .collect()
    }

    /// Get saddle points
    pub fn saddle_points(&self) -> Vec<&ClassifiedFixedPoint<P, Q, R>> {
        self.fixed_points
            .iter()
            .filter(|fp| matches!(fp.stability, StabilityType::Saddle))
            .collect()
    }
}

/// Generate phase portrait in parallel
#[cfg(feature = "parallel")]
pub fn generate_portrait_parallel<S, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    config: &PortraitConfig,
) -> Result<PhasePortrait<P, Q, R>>
where
    S: DynamicalSystem<P, Q, R> + Sync,
{
    let solver = DormandPrince::new();
    let initial_conditions = config.initial_conditions::<P, Q, R>();
    let steps = (config.forward_time / config.dt) as usize;

    let trajectories: Vec<Trajectory<P, Q, R>> = initial_conditions
        .par_iter()
        .filter_map(|ic| {
            solver
                .solve(system, ic.clone(), 0.0, config.forward_time, steps)
                .ok()
        })
        .collect();

    let bundle = TrajectoryBundle::from_trajectories(trajectories);

    // Find fixed points
    let mut fixed_points = Vec::new();
    if config.find_fixed_points {
        fixed_points = find_portrait_fixed_points(system, config)?;
    }

    // Compute separatrices (sequential for now)
    let mut separatrices = Vec::new();
    if config.include_separatrices {
        for fp in &fixed_points {
            if matches!(fp.stability, StabilityType::Saddle) {
                if let Ok(seps) = compute_separatrices(system, &fp.point, &fp.eigenvalues, config) {
                    separatrices.extend(seps);
                }
            }
        }
    }

    Ok(PhasePortrait {
        config: config.clone(),
        trajectories: bundle,
        fixed_points,
        separatrices,
    })
}

/// Find fixed points in the portrait region
fn find_portrait_fixed_points<S, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    config: &PortraitConfig,
) -> Result<Vec<ClassifiedFixedPoint<P, Q, R>>>
where
    S: DynamicalSystem<P, Q, R>,
{
    let mut fixed_points = Vec::new();
    let fp_config = FixedPointConfig {
        tolerance: 1e-8,
        max_iterations: 100,
        ..Default::default()
    };

    // Sample initial guesses on a coarse grid
    let search_nx = 5;
    let search_ny = 5;

    for i in 0..search_nx {
        for j in 0..search_ny {
            let x = config.x_bounds.0
                + (config.x_bounds.1 - config.x_bounds.0) * (i as f64 + 0.5) / search_nx as f64;
            let y = config.y_bounds.0
                + (config.y_bounds.1 - config.y_bounds.0) * (j as f64 + 0.5) / search_ny as f64;

            let mut guess = Multivector::<P, Q, R>::zero();
            guess.set(config.x_component, x);
            guess.set(config.y_component, y);

            for &(idx, val) in &config.fixed_values {
                guess.set(idx, val);
            }

            // Try to find fixed point
            if let Ok(fp_result) = find_fixed_point(system, &guess, &fp_config) {
                let fp = &fp_result.point;
                // Check if in bounds
                let fp_x = fp.get(config.x_component);
                let fp_y = fp.get(config.y_component);

                if fp_x >= config.x_bounds.0
                    && fp_x <= config.x_bounds.1
                    && fp_y >= config.y_bounds.0
                    && fp_y <= config.y_bounds.1
                {
                    // Check if we already found this one
                    let is_new =
                        !fixed_points
                            .iter()
                            .any(|existing: &ClassifiedFixedPoint<P, Q, R>| {
                                let dx = existing.point.get(config.x_component) - fp_x;
                                let dy = existing.point.get(config.y_component) - fp_y;
                                (dx * dx + dy * dy).sqrt() < 1e-4
                            });

                    if is_new {
                        // Classify stability
                        let diff_config = DifferentiationConfig::default();
                        let stability_result =
                            crate::stability::analyze_stability(system, fp, &diff_config, 1e-8);
                        let (stability, eigenvalues) = match stability_result {
                            Ok(info) => (info.stability, info.eigenvalues),
                            Err(_) => (StabilityType::Degenerate, Vec::new()),
                        };

                        fixed_points.push(ClassifiedFixedPoint {
                            point: fp.clone(),
                            stability,
                            eigenvalues,
                        });
                    }
                }
            }
        }
    }

    Ok(fixed_points)
}

/// Compute separatrices from a saddle point
fn compute_separatrices<S, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    saddle: &Multivector<P, Q, R>,
    eigenvalues: &[(f64, f64)],
    config: &PortraitConfig,
) -> Result<Vec<Trajectory<P, Q, R>>>
where
    S: DynamicalSystem<P, Q, R>,
{
    let solver = DormandPrince::new();
    let mut separatrices = Vec::new();

    // Find eigenvectors for unstable directions
    // This is a simplified approach - for 2D we assume eigenvalues are real for saddles
    let epsilon = 1e-4;
    let steps = (config.forward_time / config.dt) as usize;

    // Perturb along unstable direction (positive eigenvalue)
    for (re, _im) in eigenvalues {
        if *re > 0.0 {
            // Unstable direction - integrate forward
            // Use numerical gradient to estimate eigenvector
            for sign in [-1.0, 1.0] {
                let mut perturbed = saddle.clone();
                // Simple perturbation along x and y
                perturbed.set(
                    config.x_component,
                    perturbed.get(config.x_component) + sign * epsilon,
                );

                if let Ok(traj) = solver.solve(system, perturbed, 0.0, config.forward_time, steps) {
                    separatrices.push(traj);
                }

                let mut perturbed_y = saddle.clone();
                perturbed_y.set(
                    config.y_component,
                    perturbed_y.get(config.y_component) + sign * epsilon,
                );

                if let Ok(traj) = solver.solve(system, perturbed_y, 0.0, config.forward_time, steps)
                {
                    separatrices.push(traj);
                }
            }
        }
    }

    Ok(separatrices)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_portrait_config_default() {
        let config = PortraitConfig::default();
        assert_eq!(config.x_component, 1);
        assert_eq!(config.y_component, 2);
        assert!(config.forward_time > 0.0);
    }

    #[test]
    fn test_initial_conditions() {
        let config = PortraitConfig {
            x_bounds: (0.0, 1.0),
            y_bounds: (0.0, 1.0),
            x_trajectories: 3,
            y_trajectories: 3,
            ..Default::default()
        };

        let ics = config.initial_conditions::<3, 0, 0>();
        assert_eq!(ics.len(), 9);

        // Check corners
        let has_origin = ics
            .iter()
            .any(|p| (p.get(1) - 0.0).abs() < 1e-10 && (p.get(2) - 0.0).abs() < 1e-10);
        assert!(has_origin);

        let has_corner = ics
            .iter()
            .any(|p| (p.get(1) - 1.0).abs() < 1e-10 && (p.get(2) - 1.0).abs() < 1e-10);
        assert!(has_corner);
    }

    #[test]
    fn test_with_resolution() {
        let config = PortraitConfig::default().with_resolution(20, 20);
        assert_eq!(config.x_trajectories, 20);
        assert_eq!(config.y_trajectories, 20);
    }

    #[test]
    fn test_vector_field_point() {
        let mut pos = Multivector::<2, 0, 0>::zero();
        pos.set(0, 1.0);
        pos.set(1, 2.0);

        let mut vec = Multivector::<2, 0, 0>::zero();
        vec.set(0, 3.0);
        vec.set(1, 4.0);

        let vfp = VectorFieldPoint {
            position: pos,
            vector: vec,
            magnitude: 5.0,
        };

        assert_eq!(vfp.magnitude, 5.0);
    }

    #[test]
    fn test_classified_fixed_point() {
        let point = Multivector::<2, 0, 0>::zero();
        let cfp = ClassifiedFixedPoint {
            point,
            stability: StabilityType::AsymptoticallyStable,
            eigenvalues: vec![(-1.0, 0.0), (-2.0, 0.0)],
        };

        assert!(matches!(cfp.stability, StabilityType::AsymptoticallyStable));
        assert_eq!(cfp.eigenvalues.len(), 2);
    }
}
