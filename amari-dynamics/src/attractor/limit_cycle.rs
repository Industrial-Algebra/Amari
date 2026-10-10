//! Limit cycle detection and Poincaré sections
//!
//! This module provides tools for detecting and analyzing periodic orbits
//! (limit cycles) in dynamical systems using Poincaré section techniques.
//!
//! # Poincaré Sections
//!
//! A Poincaré section is a lower-dimensional slice through phase space.
//! By recording where trajectories cross this section, we can:
//!
//! - Detect periodic orbits (fixed points of the Poincaré map)
//! - Analyze stability via Floquet multipliers
//! - Distinguish quasiperiodic from chaotic motion
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::attractor::{PoincareSection, detect_limit_cycle};
//!
//! // Define a section: x₁ = 0, crossing from negative to positive
//! let section = PoincareSection::hyperplane(1, 0.0, true);
//!
//! // Detect limit cycle
//! let cycle = detect_limit_cycle(&system, initial, &section, &config)?;
//! println!("Period: {:.4}", cycle.period);
//! ```

use amari_core::Multivector;

use crate::error::{DynamicsError, Result};
use crate::flow::DynamicalSystem;
use crate::solver::{DormandPrince, ODESolver, Trajectory};

use super::traits::{Attractor, AttractorConfig, LimitCycleAttractor};

/// A Poincaré section definition
///
/// Defines a hyperplane in phase space where trajectory crossings are recorded.
#[derive(Debug, Clone)]
pub struct PoincareSection {
    /// Index of the coordinate defining the section
    pub coordinate_index: usize,

    /// Value of the coordinate at the section
    pub section_value: f64,

    /// Direction of crossing (true = positive direction, false = negative)
    pub positive_crossing: bool,
}

impl PoincareSection {
    /// Create a hyperplane section at x_i = value
    ///
    /// # Arguments
    ///
    /// * `coordinate_index` - Which coordinate defines the section
    /// * `value` - The value of that coordinate at the section
    /// * `positive_crossing` - If true, only record crossings from x_i < value to x_i > value
    pub fn hyperplane(coordinate_index: usize, value: f64, positive_crossing: bool) -> Self {
        Self {
            coordinate_index,
            section_value: value,
            positive_crossing,
        }
    }

    /// Create a section at the origin for coordinate i
    pub fn origin(coordinate_index: usize) -> Self {
        Self::hyperplane(coordinate_index, 0.0, true)
    }

    /// Check if a crossing occurred between two states
    pub fn check_crossing<const P: usize, const Q: usize, const R: usize>(
        &self,
        state_before: &Multivector<P, Q, R>,
        state_after: &Multivector<P, Q, R>,
    ) -> bool {
        let x_before = state_before.get(self.coordinate_index);
        let x_after = state_after.get(self.coordinate_index);

        if self.positive_crossing {
            x_before < self.section_value && x_after >= self.section_value
        } else {
            x_before > self.section_value && x_after <= self.section_value
        }
    }

    /// Interpolate to find the exact crossing point
    ///
    /// Uses linear interpolation between the two states.
    pub fn interpolate_crossing<const P: usize, const Q: usize, const R: usize>(
        &self,
        state_before: &Multivector<P, Q, R>,
        state_after: &Multivector<P, Q, R>,
        t_before: f64,
        t_after: f64,
    ) -> (Multivector<P, Q, R>, f64) {
        let x_before = state_before.get(self.coordinate_index);
        let x_after = state_after.get(self.coordinate_index);

        // Linear interpolation parameter
        let alpha = (self.section_value - x_before) / (x_after - x_before);
        let alpha = alpha.clamp(0.0, 1.0);

        // Interpolate time
        let t_crossing = t_before + alpha * (t_after - t_before);

        // Interpolate state
        let state_crossing = &(state_before * (1.0 - alpha)) + &(state_after * alpha);

        (state_crossing, t_crossing)
    }
}

/// A crossing through a Poincaré section
#[derive(Debug, Clone)]
pub struct PoincareCrossing<const P: usize, const Q: usize, const R: usize> {
    /// State at the crossing
    pub state: Multivector<P, Q, R>,

    /// Time of the crossing
    pub time: f64,

    /// Crossing number (1st, 2nd, etc.)
    pub index: usize,
}

/// Result of Poincaré section analysis
#[derive(Debug, Clone)]
pub struct PoincareResult<const P: usize, const Q: usize, const R: usize> {
    /// All crossings detected
    pub crossings: Vec<PoincareCrossing<P, Q, R>>,

    /// Return times between consecutive crossings
    pub return_times: Vec<f64>,

    /// Average return time
    pub mean_return_time: f64,

    /// Standard deviation of return times
    pub return_time_std: f64,
}

impl<const P: usize, const Q: usize, const R: usize> PoincareResult<P, Q, R> {
    /// Compute from a list of crossings
    pub fn from_crossings(crossings: Vec<PoincareCrossing<P, Q, R>>) -> Self {
        let return_times: Vec<f64> = crossings
            .windows(2)
            .map(|w| w[1].time - w[0].time)
            .collect();

        let n = return_times.len() as f64;
        let mean_return_time = if n > 0.0 {
            return_times.iter().sum::<f64>() / n
        } else {
            0.0
        };

        let return_time_std = if n > 1.0 {
            let variance: f64 = return_times
                .iter()
                .map(|t| (t - mean_return_time).powi(2))
                .sum::<f64>()
                / (n - 1.0);
            variance.sqrt()
        } else {
            0.0
        };

        Self {
            crossings,
            return_times,
            mean_return_time,
            return_time_std,
        }
    }

    /// Check if the motion appears periodic
    ///
    /// Returns true if return times have low variance relative to mean.
    pub fn is_periodic(&self, tolerance: f64) -> bool {
        if self.mean_return_time > 0.0 && self.return_times.len() > 2 {
            self.return_time_std / self.mean_return_time < tolerance
        } else {
            false
        }
    }

    /// Get the estimated period
    pub fn estimated_period(&self) -> Option<f64> {
        if self.mean_return_time > 0.0 {
            Some(self.mean_return_time)
        } else {
            None
        }
    }
}

/// Collect Poincaré section crossings from a trajectory
pub fn collect_crossings<const P: usize, const Q: usize, const R: usize>(
    trajectory: &Trajectory<P, Q, R>,
    section: &PoincareSection,
) -> PoincareResult<P, Q, R> {
    let mut crossings = Vec::new();
    let states = &trajectory.states;
    let times = &trajectory.times;

    for i in 0..states.len().saturating_sub(1) {
        if section.check_crossing(&states[i], &states[i + 1]) {
            let (crossing_state, crossing_time) =
                section.interpolate_crossing(&states[i], &states[i + 1], times[i], times[i + 1]);

            crossings.push(PoincareCrossing {
                state: crossing_state,
                time: crossing_time,
                index: crossings.len(),
            });
        }
    }

    PoincareResult::from_crossings(crossings)
}

/// Detect a limit cycle by integrating and analyzing Poincaré crossings
///
/// # Arguments
///
/// * `system` - The dynamical system
/// * `initial` - Initial state (should be near the suspected limit cycle)
/// * `section` - Poincaré section to use
/// * `config` - Detection configuration
///
/// # Returns
///
/// A `LimitCycleAttractor` if periodic motion is detected, error otherwise.
pub fn detect_limit_cycle<S, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    initial: Multivector<P, Q, R>,
    section: &PoincareSection,
    config: &AttractorConfig,
) -> Result<LimitCycleAttractor<P, Q, R>>
where
    S: DynamicalSystem<P, Q, R>,
{
    let solver = DormandPrince::new();

    // First, let transients decay
    let transient_steps = (config.transient_time / config.dt) as usize;
    let transient = solver.solve(system, initial, 0.0, config.transient_time, transient_steps)?;
    let post_transient = transient.final_state().ok_or_else(|| {
        DynamicsError::numerical_instability("detect_limit_cycle", "Transient integration failed")
    })?;

    // Now integrate for analysis
    let sample_steps = (config.sample_time / config.dt) as usize;
    let trajectory = solver.solve(
        system,
        post_transient.clone(),
        config.transient_time,
        config.transient_time + config.sample_time,
        sample_steps,
    )?;

    // Collect Poincaré crossings
    let poincare = collect_crossings(&trajectory, section);

    // Check for periodicity
    if !poincare.is_periodic(config.period_tolerance) {
        return Err(DynamicsError::convergence_failure(
            poincare.crossings.len(),
            "Motion does not appear periodic",
        ));
    }

    let period = poincare
        .estimated_period()
        .ok_or_else(|| DynamicsError::convergence_failure(0, "Could not estimate period"))?;

    // Sample the orbit more finely
    let orbit_steps = (period / config.dt * 10.0) as usize;
    let orbit_trajectory = solver.solve(
        system,
        post_transient.clone(),
        config.transient_time,
        config.transient_time + period,
        orbit_steps,
    )?;

    let orbit_points = orbit_trajectory.states.clone();

    Ok(LimitCycleAttractor::new(orbit_points, period))
}

/// Detect period by finding when trajectory returns close to initial state
pub fn detect_period<S, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    initial: Multivector<P, Q, R>,
    config: &AttractorConfig,
) -> Result<f64>
where
    S: DynamicalSystem<P, Q, R>,
{
    let solver = DormandPrince::new();

    // Integrate forward
    let steps = (config.sample_time / config.dt) as usize;
    let trajectory = solver.solve(system, initial.clone(), 0.0, config.sample_time, steps)?;

    // Find first close return
    let states = &trajectory.states;
    let times = &trajectory.times;

    let initial_norm = initial.coefficient_norm();
    let tolerance = config.period_tolerance * initial_norm.max(1.0);

    for i in 1..states.len() {
        let diff = &states[i] - &initial;
        if diff.coefficient_norm() < tolerance && times[i] > config.dt * 10.0 {
            // Refine estimate using interpolation
            if i > 0 {
                let diff_prev = &states[i - 1] - &initial;
                let d0 = diff_prev.coefficient_norm();
                let d1 = diff.coefficient_norm();

                // Linear interpolation for crossing
                if d0 > d1 {
                    let alpha = d0 / (d0 + d1);
                    return Ok(times[i - 1] + alpha * (times[i] - times[i - 1]));
                }
            }
            return Ok(times[i]);
        }
    }

    Err(DynamicsError::convergence_failure(
        steps,
        "No periodic return found",
    ))
}

/// Compute Floquet multipliers for a limit cycle
///
/// The Floquet multipliers are eigenvalues of the monodromy matrix,
/// which is the Jacobian of the Poincaré map at a fixed point.
pub fn compute_floquet_multipliers<S, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    cycle: &LimitCycleAttractor<P, Q, R>,
    section: &PoincareSection,
) -> Result<Vec<(f64, f64)>>
where
    S: DynamicalSystem<P, Q, R>,
{
    use nalgebra::DMatrix;

    let dim = 1 << (P + Q + R);
    let epsilon = 1e-6;

    // Get a point on the cycle
    let base_point = cycle.representative_point().clone();

    // Numerically compute the monodromy matrix by perturbing initial conditions
    let mut monodromy = DMatrix::zeros(dim, dim);
    let solver = DormandPrince::new();
    let steps = (cycle.period / 0.01) as usize;

    for j in 0..dim {
        // Skip the coordinate defining the section (it's constrained)
        if j == section.coordinate_index {
            monodromy[(j, j)] = 1.0;
            continue;
        }

        // Perturb in direction j
        let mut perturbed = base_point.clone();
        perturbed.set(j, perturbed.get(j) + epsilon);

        // Integrate for one period
        let trajectory = solver.solve(system, perturbed, 0.0, cycle.period, steps)?;
        let final_state = trajectory.final_state().ok_or_else(|| {
            DynamicsError::numerical_instability("compute_floquet", "Integration failed")
        })?;

        // Compute column of Jacobian
        for i in 0..dim {
            monodromy[(i, j)] = (final_state.get(i) - base_point.get(i)) / epsilon;
        }
    }

    // Compute eigenvalues of monodromy matrix
    // One multiplier is always 1 (corresponding to motion along the cycle)
    let eigenvalues = crate::stability::compute_eigenvalues(&monodromy)?;

    Ok(eigenvalues)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_poincare_section_creation() {
        let section = PoincareSection::hyperplane(0, 0.5, true);
        assert_eq!(section.coordinate_index, 0);
        assert_eq!(section.section_value, 0.5);
        assert!(section.positive_crossing);

        let origin_section = PoincareSection::origin(1);
        assert_eq!(origin_section.coordinate_index, 1);
        assert_eq!(origin_section.section_value, 0.0);
    }

    #[test]
    fn test_crossing_detection() {
        let section = PoincareSection::origin(0);

        let mut before = Multivector::<2, 0, 0>::zero();
        before.set(0, -0.1);

        let mut after = Multivector::<2, 0, 0>::zero();
        after.set(0, 0.1);

        // Positive crossing
        assert!(section.check_crossing(&before, &after));

        // No crossing (same side)
        let mut same_side = Multivector::<2, 0, 0>::zero();
        same_side.set(0, -0.2);
        assert!(!section.check_crossing(&before, &same_side));

        // Wrong direction
        assert!(!section.check_crossing(&after, &before));
    }

    #[test]
    fn test_crossing_interpolation() {
        let section = PoincareSection::origin(0);

        let mut before = Multivector::<2, 0, 0>::zero();
        before.set(0, -0.5);
        before.set(1, 1.0);

        let mut after = Multivector::<2, 0, 0>::zero();
        after.set(0, 0.5);
        after.set(1, 2.0);

        let (crossing, t) = section.interpolate_crossing(&before, &after, 0.0, 1.0);

        // Should cross at t = 0.5
        assert!((t - 0.5).abs() < 1e-10);

        // x should be at section value (0)
        assert!(crossing.get(0).abs() < 1e-10);

        // y should be interpolated (1.5)
        assert!((crossing.get(1) - 1.5).abs() < 1e-10);
    }

    #[test]
    fn test_poincare_result_periodicity() {
        // Create mock crossings with consistent return times
        let crossings: Vec<PoincareCrossing<2, 0, 0>> = (0..5)
            .map(|i| PoincareCrossing {
                state: Multivector::zero(),
                time: i as f64 * 2.0 * std::f64::consts::PI,
                index: i,
            })
            .collect();

        let result = PoincareResult::from_crossings(crossings);

        assert!(result.is_periodic(0.01));
        assert!((result.mean_return_time - 2.0 * std::f64::consts::PI).abs() < 1e-10);
    }

    #[test]
    fn test_poincare_result_non_periodic() {
        // Create mock crossings with varying return times
        let crossings: Vec<PoincareCrossing<2, 0, 0>> = vec![
            PoincareCrossing {
                state: Multivector::zero(),
                time: 0.0,
                index: 0,
            },
            PoincareCrossing {
                state: Multivector::zero(),
                time: 1.0,
                index: 1,
            },
            PoincareCrossing {
                state: Multivector::zero(),
                time: 3.0, // Different interval
                index: 2,
            },
            PoincareCrossing {
                state: Multivector::zero(),
                time: 4.0, // Different again
                index: 3,
            },
        ];

        let result = PoincareResult::from_crossings(crossings);

        // High variance in return times means not periodic
        assert!(!result.is_periodic(0.01));
    }
}
