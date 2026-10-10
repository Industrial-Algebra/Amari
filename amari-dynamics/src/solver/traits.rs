//! ODE solver traits and trajectory types
//!
//! This module defines the core abstraction for ordinary differential equation
//! solvers operating on geometric algebra state spaces.
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::{
//!     flow::DynamicalSystem,
//!     solver::{ODESolver, RungeKutta4},
//! };
//!
//! let system = MySystem::new();
//! let solver = RungeKutta4::new();
//! let trajectory = solver.solve(&system, initial_state, 0.0, 10.0, 1000)?;
//! ```

use amari_core::Multivector;

use crate::error::{DynamicsError, Result};
use crate::flow::DynamicalSystem;

// Creusot contracts would be applied to implementations, not trait declarations
// See individual solver implementations for contract annotations

// ============================================================================
// Trajectory Types
// ============================================================================

/// A trajectory through phase space
///
/// Stores the time evolution of a dynamical system's state, including
/// timestamps, states, and optional metadata.
#[derive(Debug, Clone)]
pub struct Trajectory<const P: usize, const Q: usize, const R: usize> {
    /// Time values at each point
    pub times: Vec<f64>,
    /// State vectors at each time
    pub states: Vec<Multivector<P, Q, R>>,
    /// Step sizes used (for adaptive methods)
    pub step_sizes: Option<Vec<f64>>,
    /// Local error estimates (for adaptive methods)
    pub error_estimates: Option<Vec<f64>>,
}

impl<const P: usize, const Q: usize, const R: usize> Trajectory<P, Q, R> {
    /// Create a new empty trajectory with pre-allocated capacity
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            times: Vec::with_capacity(capacity),
            states: Vec::with_capacity(capacity),
            step_sizes: None,
            error_estimates: None,
        }
    }

    /// Create a trajectory from time and state vectors
    pub fn from_data(times: Vec<f64>, states: Vec<Multivector<P, Q, R>>) -> Result<Self> {
        if times.len() != states.len() {
            return Err(DynamicsError::DimensionMismatch {
                expected: times.len(),
                actual: states.len(),
            });
        }
        Ok(Self {
            times,
            states,
            step_sizes: None,
            error_estimates: None,
        })
    }

    /// Push a new point to the trajectory
    pub fn push(&mut self, t: f64, state: Multivector<P, Q, R>) {
        self.times.push(t);
        self.states.push(state);
    }

    /// Push a point with step size and error estimate (for adaptive methods)
    pub fn push_with_metadata(&mut self, t: f64, state: Multivector<P, Q, R>, dt: f64, error: f64) {
        self.times.push(t);
        self.states.push(state);

        if self.step_sizes.is_none() {
            self.step_sizes = Some(Vec::with_capacity(self.times.capacity()));
        }
        if self.error_estimates.is_none() {
            self.error_estimates = Some(Vec::with_capacity(self.times.capacity()));
        }

        self.step_sizes.as_mut().unwrap().push(dt);
        self.error_estimates.as_mut().unwrap().push(error);
    }

    /// Number of points in the trajectory
    pub fn len(&self) -> usize {
        self.times.len()
    }

    /// Check if trajectory is empty
    pub fn is_empty(&self) -> bool {
        self.times.is_empty()
    }

    /// Get the initial state
    pub fn initial_state(&self) -> Option<&Multivector<P, Q, R>> {
        self.states.first()
    }

    /// Get the final state
    pub fn final_state(&self) -> Option<&Multivector<P, Q, R>> {
        self.states.last()
    }

    /// Get the initial time
    pub fn initial_time(&self) -> Option<f64> {
        self.times.first().copied()
    }

    /// Get the final time
    pub fn final_time(&self) -> Option<f64> {
        self.times.last().copied()
    }

    /// Get state at a specific index
    pub fn get(&self, index: usize) -> Option<(f64, &Multivector<P, Q, R>)> {
        self.times
            .get(index)
            .and_then(|&t| self.states.get(index).map(|s| (t, s)))
    }

    /// Iterate over (time, state) pairs
    pub fn iter(&self) -> impl Iterator<Item = (f64, &Multivector<P, Q, R>)> {
        self.times.iter().copied().zip(self.states.iter())
    }

    /// Compute the total arc length of the trajectory
    pub fn arc_length(&self) -> f64 {
        if self.states.len() < 2 {
            return 0.0;
        }

        self.states
            .windows(2)
            .map(|w| {
                let diff = &w[1] - &w[0];
                diff.coefficient_norm()
            })
            .sum()
    }

    /// Sample the trajectory at uniform time intervals
    ///
    /// Uses linear interpolation between stored points.
    pub fn resample(&self, num_points: usize) -> Result<Self> {
        if self.is_empty() {
            return Ok(Self::with_capacity(0));
        }

        let t0 = self.times[0];
        let t1 = *self.times.last().unwrap();

        if num_points < 2 {
            return Err(DynamicsError::invalid_parameter(
                "resample requires at least 2 points",
            ));
        }

        let dt = (t1 - t0) / (num_points - 1) as f64;
        let mut result = Self::with_capacity(num_points);

        for i in 0..num_points {
            let t = t0 + i as f64 * dt;
            let state = self.interpolate(t)?;
            result.push(t, state);
        }

        Ok(result)
    }

    /// Interpolate state at a given time using linear interpolation
    pub fn interpolate(&self, t: f64) -> Result<Multivector<P, Q, R>> {
        if self.is_empty() {
            return Err(DynamicsError::invalid_parameter("empty trajectory"));
        }

        // Find the interval containing t
        let t0 = self.times[0];
        let t1 = *self.times.last().unwrap();

        if t < t0 || t > t1 {
            return Err(DynamicsError::invalid_time_interval(t, t));
        }

        // Binary search for the interval
        let idx = self
            .times
            .binary_search_by(|&ti| ti.partial_cmp(&t).unwrap())
            .unwrap_or_else(|i| i.saturating_sub(1));

        if idx >= self.times.len() - 1 {
            return Ok(self.states.last().unwrap().clone());
        }

        // Linear interpolation
        let t_a = self.times[idx];
        let t_b = self.times[idx + 1];
        let alpha = (t - t_a) / (t_b - t_a);

        let state_a = &self.states[idx];
        let state_b = &self.states[idx + 1];

        // Interpolate: (1 - alpha) * a + alpha * b
        let scaled_a = state_a * (1.0 - alpha);
        let scaled_b = state_b * alpha;
        Ok(&scaled_a + &scaled_b)
    }
}

// ============================================================================
// Solver Step Result
// ============================================================================

/// Result of a single solver step, including error estimate for adaptive methods
#[derive(Debug, Clone)]
pub struct StepResult<const P: usize, const Q: usize, const R: usize> {
    /// The new state after the step
    pub state: Multivector<P, Q, R>,
    /// Estimated local error (for adaptive methods)
    pub error_estimate: Option<f64>,
    /// Suggested next step size (for adaptive methods)
    pub suggested_dt: Option<f64>,
}

impl<const P: usize, const Q: usize, const R: usize> StepResult<P, Q, R> {
    /// Create a simple step result without error estimate
    pub fn new(state: Multivector<P, Q, R>) -> Self {
        Self {
            state,
            error_estimate: None,
            suggested_dt: None,
        }
    }

    /// Create a step result with error estimate and suggested step size
    pub fn with_error(state: Multivector<P, Q, R>, error: f64, suggested_dt: f64) -> Self {
        Self {
            state,
            error_estimate: Some(error),
            suggested_dt: Some(suggested_dt),
        }
    }
}

// ============================================================================
// ODE Solver Trait
// ============================================================================

/// Trait for ordinary differential equation solvers
///
/// Solvers implement numerical integration of autonomous dynamical systems
/// defined by the [`DynamicalSystem`] trait.
///
/// # Creusot Contracts
///
/// When the `contracts` feature is enabled, implementations should satisfy:
/// - `step`: Given a valid state in the domain, produces a new state (which
///   should remain in the domain for well-behaved systems)
/// - `solve`: Produces a trajectory with monotonically increasing times
///
/// # Example
///
/// ```ignore
/// use amari_dynamics::solver::{ODESolver, RungeKutta4};
///
/// let solver = RungeKutta4::new();
/// let result = solver.step(&system, &state, 0.0, 0.01)?;
/// ```
pub trait ODESolver<const P: usize, const Q: usize, const R: usize> {
    /// Perform a single integration step
    ///
    /// # Arguments
    ///
    /// * `system` - The dynamical system to integrate
    /// * `state` - Current state
    /// * `t` - Current time
    /// * `dt` - Time step size (must be positive)
    ///
    /// # Returns
    ///
    /// The state after advancing by time `dt`.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `dt` is not positive
    /// - The state leaves the valid domain
    /// - Numerical issues occur (NaN, overflow, etc.)
    ///
    /// # Contracts (when `contracts` feature is enabled)
    ///
    /// - Requires: `dt > 0.0`
    /// - Requires: `system.in_domain(state)`
    fn step<S: DynamicalSystem<P, Q, R>>(
        &self,
        system: &S,
        state: &Multivector<P, Q, R>,
        t: f64,
        dt: f64,
    ) -> Result<StepResult<P, Q, R>>;

    /// Integrate the system from t0 to t1 with fixed step count
    ///
    /// # Arguments
    ///
    /// * `system` - The dynamical system to integrate
    /// * `initial` - Initial state at time t0
    /// * `t0` - Start time
    /// * `t1` - End time (must be > t0 for forward integration)
    /// * `steps` - Number of integration steps
    ///
    /// # Returns
    ///
    /// A trajectory containing the time evolution of the state.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `t1 <= t0`
    /// - `steps == 0`
    /// - The trajectory leaves the valid domain
    /// - Numerical issues occur
    ///
    /// # Contracts (when `contracts` feature is enabled)
    ///
    /// - Requires: `t1 > t0`
    /// - Requires: `steps > 0`
    /// - Requires: `system.in_domain(&initial)`
    fn solve<S: DynamicalSystem<P, Q, R>>(
        &self,
        system: &S,
        initial: Multivector<P, Q, R>,
        t0: f64,
        t1: f64,
        steps: usize,
    ) -> Result<Trajectory<P, Q, R>> {
        if t1 <= t0 {
            return Err(DynamicsError::invalid_time_interval(t0, t1));
        }
        if steps == 0 {
            return Err(DynamicsError::invalid_parameter("steps must be > 0"));
        }

        let dt = (t1 - t0) / steps as f64;
        let mut trajectory = Trajectory::with_capacity(steps + 1);

        let mut state = initial;
        let mut t = t0;

        // Record initial state
        trajectory.push(t, state.clone());

        // Integrate
        for _ in 0..steps {
            let result = self.step(system, &state, t, dt)?;
            t += dt;
            state = result.state;
            trajectory.push(t, state.clone());
        }

        Ok(trajectory)
    }

    /// Get the order of the method (for error analysis)
    ///
    /// Returns the order of accuracy of the numerical method.
    /// E.g., RK4 returns 4, Forward Euler returns 1.
    fn order(&self) -> u32;

    /// Get the name of the solver method
    fn name(&self) -> &'static str;
}

// ============================================================================
// Adaptive Solver Trait
// ============================================================================

/// Configuration for adaptive step size control
#[derive(Debug, Clone)]
pub struct AdaptiveConfig {
    /// Absolute tolerance for error control
    pub atol: f64,
    /// Relative tolerance for error control
    pub rtol: f64,
    /// Minimum allowed step size
    pub min_step: f64,
    /// Maximum allowed step size
    pub max_step: f64,
    /// Safety factor for step size adjustment
    pub safety: f64,
    /// Maximum factor to increase step size
    pub max_factor: f64,
    /// Minimum factor to decrease step size
    pub min_factor: f64,
}

impl Default for AdaptiveConfig {
    fn default() -> Self {
        Self {
            atol: 1e-8,
            rtol: 1e-6,
            min_step: 1e-12,
            max_step: 1.0,
            safety: 0.9,
            max_factor: 5.0,
            min_factor: 0.2,
        }
    }
}

/// Trait for adaptive step size ODE solvers
///
/// Extends [`ODESolver`] with error estimation and step size control.
pub trait AdaptiveODESolver<const P: usize, const Q: usize, const R: usize>:
    ODESolver<P, Q, R>
{
    /// Perform a step with error estimation
    ///
    /// Returns both the new state and an error estimate that can be used
    /// for step size control.
    fn step_with_error<S: DynamicalSystem<P, Q, R>>(
        &self,
        system: &S,
        state: &Multivector<P, Q, R>,
        t: f64,
        dt: f64,
    ) -> Result<StepResult<P, Q, R>>;

    /// Solve with adaptive step size control
    ///
    /// Automatically adjusts step sizes to maintain error within tolerances.
    fn solve_adaptive<S: DynamicalSystem<P, Q, R>>(
        &self,
        system: &S,
        initial: Multivector<P, Q, R>,
        t0: f64,
        t1: f64,
        config: &AdaptiveConfig,
    ) -> Result<Trajectory<P, Q, R>> {
        if t1 <= t0 {
            return Err(DynamicsError::invalid_time_interval(t0, t1));
        }

        let mut trajectory = Trajectory::with_capacity(1000);
        let mut state = initial;
        let mut t = t0;
        let mut dt = (t1 - t0) / 100.0; // Initial guess

        // Clamp initial step
        dt = dt.clamp(config.min_step, config.max_step);

        trajectory.push(t, state.clone());

        while t < t1 {
            // Don't overshoot
            if t + dt > t1 {
                dt = t1 - t;
            }

            // Try a step
            let result = self.step_with_error(system, &state, t, dt)?;

            // Compute error scale
            let error = result.error_estimate.unwrap_or(0.0);
            let scale = config.atol + config.rtol * state.coefficient_norm();
            let error_ratio = error / scale;

            if error_ratio <= 1.0 {
                // Accept step
                t += dt;
                state = result.state;
                trajectory.push_with_metadata(t, state.clone(), dt, error);

                // Increase step size if error is small
                if error_ratio < 0.5 {
                    dt *=
                        config.safety * (1.0 / error_ratio).powf(1.0 / (self.order() as f64 + 1.0));
                    dt = dt.min(config.max_step).min(dt * config.max_factor);
                }
            } else {
                // Reject step, decrease step size
                dt *= config.safety * (1.0 / error_ratio).powf(1.0 / self.order() as f64);
                dt = dt.max(config.min_step).max(dt * config.min_factor);

                if dt < config.min_step {
                    return Err(DynamicsError::numerical_instability(
                        "adaptive solver",
                        "step size below minimum",
                    ));
                }
            }
        }

        Ok(trajectory)
    }

    /// Get the embedded method order (lower order for error estimation)
    fn embedded_order(&self) -> u32;
}

// ============================================================================
// Implicit Solver Trait
// ============================================================================

/// Trait for implicit ODE solvers (for stiff systems)
///
/// Implicit methods require solving a system of nonlinear equations at each step,
/// making them more expensive but more stable for stiff problems.
pub trait ImplicitODESolver<const P: usize, const Q: usize, const R: usize>:
    ODESolver<P, Q, R>
{
    /// Maximum number of Newton iterations per step
    fn max_newton_iterations(&self) -> usize {
        50
    }

    /// Newton solver tolerance
    fn newton_tolerance(&self) -> f64 {
        1e-10
    }

    /// Perform an implicit step using Newton iteration
    fn implicit_step<S: DynamicalSystem<P, Q, R>>(
        &self,
        system: &S,
        state: &Multivector<P, Q, R>,
        t: f64,
        dt: f64,
    ) -> Result<StepResult<P, Q, R>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_creation() {
        let mut traj: Trajectory<3, 0, 0> = Trajectory::with_capacity(10);
        assert!(traj.is_empty());
        assert_eq!(traj.len(), 0);

        let state = Multivector::<3, 0, 0>::zero();
        traj.push(0.0, state.clone());
        traj.push(1.0, state.clone());

        assert_eq!(traj.len(), 2);
        assert!(!traj.is_empty());
    }

    #[test]
    fn test_trajectory_times() {
        let mut traj: Trajectory<3, 0, 0> = Trajectory::with_capacity(3);
        let state = Multivector::<3, 0, 0>::zero();

        traj.push(0.0, state.clone());
        traj.push(0.5, state.clone());
        traj.push(1.0, state);

        assert_eq!(traj.initial_time(), Some(0.0));
        assert_eq!(traj.final_time(), Some(1.0));
    }

    #[test]
    fn test_adaptive_config_default() {
        let config = AdaptiveConfig::default();
        assert!(config.atol > 0.0);
        assert!(config.rtol > 0.0);
        assert!(config.min_step < config.max_step);
        assert!(config.safety > 0.0 && config.safety < 1.0);
    }

    #[test]
    fn test_step_result() {
        let state = Multivector::<3, 0, 0>::zero();
        let result = StepResult::new(state.clone());
        assert!(result.error_estimate.is_none());
        assert!(result.suggested_dt.is_none());

        let result_with_error = StepResult::with_error(state, 1e-6, 0.01);
        assert_eq!(result_with_error.error_estimate, Some(1e-6));
        assert_eq!(result_with_error.suggested_dt, Some(0.01));
    }
}
