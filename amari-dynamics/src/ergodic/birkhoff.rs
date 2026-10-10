//! Birkhoff ergodic averages
//!
//! This module provides algorithms for computing time averages along trajectories,
//! which converge to space averages for ergodic systems (Birkhoff Ergodic Theorem).
//!
//! # Overview
//!
//! For an ergodic system with invariant measure μ, the Birkhoff average:
//!
//! ```text
//! lim (1/T) ∫₀ᵀ f(φₜ(x)) dt = ∫ f dμ
//! ```
//!
//! converges almost everywhere. This module computes these averages numerically.
//!
//! # Applications
//!
//! - Computing mean values of observables
//! - Estimating invariant measures
//! - Testing ergodicity
//! - Computing Lyapunov exponents (special case)
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::ergodic::{birkhoff_average, BirkhoffConfig};
//!
//! // Compute time average of energy
//! let energy = |state: &Multivector<2, 0, 0>| {
//!     let x = state.get(1);
//!     let v = state.get(2);
//!     0.5 * v * v + 0.5 * x * x
//! };
//!
//! let avg = birkhoff_average(&system, &initial, energy, &config)?;
//! ```

use amari_core::Multivector;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

use crate::error::{DynamicsError, Result};
use crate::flow::DynamicalSystem;
use crate::solver::{DormandPrince, ODESolver, Trajectory};

/// Configuration for Birkhoff average computation
#[derive(Debug, Clone)]
pub struct BirkhoffConfig {
    /// Total integration time
    pub total_time: f64,

    /// Integration time step
    pub dt: f64,

    /// Transient time to discard (before averaging)
    pub transient_time: f64,

    /// Check for convergence every this many steps
    pub convergence_check_interval: usize,

    /// Relative tolerance for convergence
    pub convergence_tolerance: f64,

    /// Maximum number of convergence checks before stopping
    pub max_convergence_checks: usize,
}

impl Default for BirkhoffConfig {
    fn default() -> Self {
        Self {
            total_time: 1000.0,
            dt: 0.01,
            transient_time: 100.0,
            convergence_check_interval: 1000,
            convergence_tolerance: 1e-6,
            max_convergence_checks: 100,
        }
    }
}

impl BirkhoffConfig {
    /// Create a fast configuration for quick estimates
    pub fn fast() -> Self {
        Self {
            total_time: 100.0,
            dt: 0.01,
            transient_time: 10.0,
            convergence_check_interval: 100,
            convergence_tolerance: 1e-4,
            max_convergence_checks: 20,
        }
    }

    /// Create an accurate configuration for precise averages
    pub fn accurate() -> Self {
        Self {
            total_time: 10000.0,
            dt: 0.001,
            transient_time: 1000.0,
            convergence_check_interval: 5000,
            convergence_tolerance: 1e-8,
            max_convergence_checks: 200,
        }
    }

    /// Get the number of steps for the main averaging phase
    pub fn averaging_steps(&self) -> usize {
        ((self.total_time - self.transient_time) / self.dt) as usize
    }

    /// Get the number of transient steps to discard
    pub fn transient_steps(&self) -> usize {
        (self.transient_time / self.dt) as usize
    }
}

/// Result of Birkhoff average computation
#[derive(Debug, Clone)]
pub struct BirkhoffResult {
    /// The computed average value
    pub average: f64,

    /// Estimated standard error
    pub standard_error: f64,

    /// Total number of samples used
    pub num_samples: usize,

    /// Whether the average converged
    pub converged: bool,

    /// Convergence history (average values at check points)
    pub convergence_history: Vec<f64>,

    /// Autocorrelation time estimate (if computed)
    pub autocorrelation_time: Option<f64>,
}

impl BirkhoffResult {
    /// Get confidence interval at given level (e.g., 0.95 for 95%)
    pub fn confidence_interval(&self, level: f64) -> (f64, f64) {
        // Z-score for given confidence level (approximate)
        let z = if level >= 0.99 {
            2.576
        } else if level >= 0.95 {
            1.96
        } else if level >= 0.90 {
            1.645
        } else {
            1.0
        };

        let margin = z * self.standard_error;
        (self.average - margin, self.average + margin)
    }

    /// Get effective number of independent samples
    pub fn effective_samples(&self) -> f64 {
        match self.autocorrelation_time {
            Some(tau) if tau > 0.0 => self.num_samples as f64 / (2.0 * tau),
            _ => self.num_samples as f64,
        }
    }
}

/// Compute Birkhoff average of an observable along a trajectory
pub fn birkhoff_average<S, F, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    initial: &Multivector<P, Q, R>,
    observable: F,
    config: &BirkhoffConfig,
) -> Result<BirkhoffResult>
where
    S: DynamicalSystem<P, Q, R>,
    F: Fn(&Multivector<P, Q, R>) -> f64,
{
    let solver = DormandPrince::new();

    // First, integrate through transient
    let transient_steps = config.transient_steps();
    let post_transient = if transient_steps > 0 {
        let trajectory = solver.solve(
            system,
            initial.clone(),
            0.0,
            config.transient_time,
            transient_steps,
        )?;
        trajectory.final_state().cloned().ok_or_else(|| {
            DynamicsError::numerical_instability("Birkhoff average", "Empty transient trajectory")
        })?
    } else {
        initial.clone()
    };

    // Now compute the average
    let averaging_steps = config.averaging_steps();
    let trajectory = solver.solve(
        system,
        post_transient,
        0.0,
        config.total_time - config.transient_time,
        averaging_steps,
    )?;

    compute_average_from_trajectory(&trajectory, observable, config)
}

/// Compute average from an existing trajectory
pub fn compute_average_from_trajectory<F, const P: usize, const Q: usize, const R: usize>(
    trajectory: &Trajectory<P, Q, R>,
    observable: F,
    config: &BirkhoffConfig,
) -> Result<BirkhoffResult>
where
    F: Fn(&Multivector<P, Q, R>) -> f64,
{
    let states = &trajectory.states;
    if states.is_empty() {
        return Err(DynamicsError::invalid_parameter("Empty trajectory"));
    }

    // Compute observable values
    let values: Vec<f64> = states.iter().map(&observable).collect();
    let n = values.len();

    // Running average with convergence checking
    let mut sum = 0.0;
    let mut sum_sq = 0.0;
    let mut convergence_history = Vec::new();
    let mut converged = false;
    let mut last_avg = 0.0;

    for (i, &val) in values.iter().enumerate() {
        sum += val;
        sum_sq += val * val;

        // Check convergence periodically
        if (i + 1) % config.convergence_check_interval == 0 {
            let current_avg = sum / (i + 1) as f64;
            convergence_history.push(current_avg);

            if convergence_history.len() > 1 {
                let rel_change = ((current_avg - last_avg) / (last_avg.abs() + 1e-10)).abs();
                if rel_change < config.convergence_tolerance {
                    converged = true;
                }
            }
            last_avg = current_avg;

            if convergence_history.len() >= config.max_convergence_checks && converged {
                break;
            }
        }
    }

    let average = sum / n as f64;
    let variance = sum_sq / n as f64 - average * average;

    // Estimate autocorrelation time for error estimation
    let autocorrelation_time = estimate_autocorrelation_time(&values);

    // Standard error with autocorrelation correction
    let effective_n = match autocorrelation_time {
        Some(tau) if tau > 0.0 => n as f64 / (2.0 * tau),
        _ => n as f64,
    };
    let standard_error = (variance / effective_n).sqrt();

    Ok(BirkhoffResult {
        average,
        standard_error,
        num_samples: n,
        converged,
        convergence_history,
        autocorrelation_time,
    })
}

/// Estimate autocorrelation time from a time series
fn estimate_autocorrelation_time(values: &[f64]) -> Option<f64> {
    let n = values.len();
    if n < 100 {
        return None;
    }

    let mean = values.iter().sum::<f64>() / n as f64;
    let var = values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n as f64;

    if var < 1e-15 {
        return Some(1.0);
    }

    // Compute autocorrelation function
    let max_lag = (n / 10).min(1000);
    let mut tau = 0.5; // Initial value (C(0) contributes 0.5)

    for lag in 1..max_lag {
        let mut c_lag = 0.0;
        for i in 0..(n - lag) {
            c_lag += (values[i] - mean) * (values[i + lag] - mean);
        }
        c_lag /= (n - lag) as f64 * var;

        // Stop when autocorrelation becomes negligible
        if c_lag < 0.05 {
            break;
        }
        tau += c_lag;
    }

    Some(tau.max(1.0))
}

/// Compute multiple Birkhoff averages simultaneously (more efficient)
#[allow(clippy::type_complexity)]
pub fn birkhoff_averages<S, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    initial: &Multivector<P, Q, R>,
    observables: &[Box<dyn Fn(&Multivector<P, Q, R>) -> f64>],
    config: &BirkhoffConfig,
) -> Result<Vec<BirkhoffResult>>
where
    S: DynamicalSystem<P, Q, R>,
{
    let solver = DormandPrince::new();

    // First, integrate through transient
    let transient_steps = config.transient_steps();
    let post_transient = if transient_steps > 0 {
        let trajectory = solver.solve(
            system,
            initial.clone(),
            0.0,
            config.transient_time,
            transient_steps,
        )?;
        trajectory.final_state().cloned().ok_or_else(|| {
            DynamicsError::numerical_instability("Birkhoff average", "Empty transient trajectory")
        })?
    } else {
        initial.clone()
    };

    // Compute trajectory once
    let averaging_steps = config.averaging_steps();
    let trajectory = solver.solve(
        system,
        post_transient,
        0.0,
        config.total_time - config.transient_time,
        averaging_steps,
    )?;

    // Compute averages for all observables
    observables
        .iter()
        .map(|obs| compute_average_from_trajectory(&trajectory, |s| obs(s), config))
        .collect()
}

/// Compute Birkhoff average from multiple initial conditions
#[cfg(feature = "parallel")]
pub fn birkhoff_average_ensemble<S, F, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    initial_conditions: &[Multivector<P, Q, R>],
    observable: F,
    config: &BirkhoffConfig,
) -> Result<Vec<BirkhoffResult>>
where
    S: DynamicalSystem<P, Q, R> + Sync,
    F: Fn(&Multivector<P, Q, R>) -> f64 + Sync,
{
    initial_conditions
        .par_iter()
        .map(|ic| birkhoff_average(system, ic, &observable, config))
        .collect()
}

/// Sequential version of ensemble average
pub fn birkhoff_average_ensemble_seq<S, F, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    initial_conditions: &[Multivector<P, Q, R>],
    observable: F,
    config: &BirkhoffConfig,
) -> Result<Vec<BirkhoffResult>>
where
    S: DynamicalSystem<P, Q, R>,
    F: Fn(&Multivector<P, Q, R>) -> f64,
{
    initial_conditions
        .iter()
        .map(|ic| birkhoff_average(system, ic, &observable, config))
        .collect()
}

/// Test for ergodicity by comparing averages from different initial conditions
pub fn test_ergodicity<S, F, const P: usize, const Q: usize, const R: usize>(
    system: &S,
    initial_conditions: &[Multivector<P, Q, R>],
    observable: F,
    config: &BirkhoffConfig,
    tolerance: f64,
) -> Result<ErgodicityTest>
where
    S: DynamicalSystem<P, Q, R>,
    F: Fn(&Multivector<P, Q, R>) -> f64 + Clone,
{
    if initial_conditions.len() < 2 {
        return Err(DynamicsError::invalid_parameter(
            "Need at least 2 initial conditions for ergodicity test",
        ));
    }

    let results = birkhoff_average_ensemble_seq(system, initial_conditions, observable, config)?;

    let averages: Vec<f64> = results.iter().map(|r| r.average).collect();
    let mean_average = averages.iter().sum::<f64>() / averages.len() as f64;
    let variance = averages
        .iter()
        .map(|a| (a - mean_average).powi(2))
        .sum::<f64>()
        / averages.len() as f64;
    let std_dev = variance.sqrt();

    // Coefficient of variation
    let cv = std_dev / (mean_average.abs() + 1e-10);

    // Check if all averages are within tolerance of mean
    let max_deviation = averages
        .iter()
        .map(|a| ((a - mean_average) / (mean_average.abs() + 1e-10)).abs())
        .fold(0.0, f64::max);

    let is_ergodic = max_deviation < tolerance;

    Ok(ErgodicityTest {
        is_ergodic,
        mean_average,
        standard_deviation: std_dev,
        coefficient_of_variation: cv,
        max_relative_deviation: max_deviation,
        individual_averages: averages,
        num_initial_conditions: initial_conditions.len(),
    })
}

/// Result of ergodicity test
#[derive(Debug, Clone)]
pub struct ErgodicityTest {
    /// Whether the system appears ergodic
    pub is_ergodic: bool,

    /// Mean of all time averages
    pub mean_average: f64,

    /// Standard deviation of time averages
    pub standard_deviation: f64,

    /// Coefficient of variation
    pub coefficient_of_variation: f64,

    /// Maximum relative deviation from mean
    pub max_relative_deviation: f64,

    /// Individual averages from each initial condition
    pub individual_averages: Vec<f64>,

    /// Number of initial conditions tested
    pub num_initial_conditions: usize,
}

/// Common observables for dynamical systems
pub mod observables {
    use amari_core::Multivector;

    /// Kinetic energy observable (assumes velocity in component 2)
    pub fn kinetic_energy<const P: usize, const Q: usize, const R: usize>(
        velocity_component: usize,
    ) -> impl Fn(&Multivector<P, Q, R>) -> f64 {
        move |state: &Multivector<P, Q, R>| {
            let v = state.get(velocity_component);
            0.5 * v * v
        }
    }

    /// Harmonic potential energy (assumes position in component 1)
    pub fn harmonic_potential<const P: usize, const Q: usize, const R: usize>(
        position_component: usize,
        omega: f64,
    ) -> impl Fn(&Multivector<P, Q, R>) -> f64 {
        move |state: &Multivector<P, Q, R>| {
            let x = state.get(position_component);
            0.5 * omega * omega * x * x
        }
    }

    /// Euclidean norm of the state
    pub fn state_norm<const P: usize, const Q: usize, const R: usize>(
    ) -> impl Fn(&Multivector<P, Q, R>) -> f64 {
        |state: &Multivector<P, Q, R>| state.coefficient_norm()
    }

    /// Component value
    pub fn component<const P: usize, const Q: usize, const R: usize>(
        index: usize,
    ) -> impl Fn(&Multivector<P, Q, R>) -> f64 {
        move |state: &Multivector<P, Q, R>| state.get(index)
    }

    /// Square of a component
    pub fn component_squared<const P: usize, const Q: usize, const R: usize>(
        index: usize,
    ) -> impl Fn(&Multivector<P, Q, R>) -> f64 {
        move |state: &Multivector<P, Q, R>| {
            let c = state.get(index);
            c * c
        }
    }

    /// Product of two components
    pub fn component_product<const P: usize, const Q: usize, const R: usize>(
        i: usize,
        j: usize,
    ) -> impl Fn(&Multivector<P, Q, R>) -> f64 {
        move |state: &Multivector<P, Q, R>| state.get(i) * state.get(j)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_birkhoff_config_default() {
        let config = BirkhoffConfig::default();
        assert!(config.total_time > config.transient_time);
        assert!(config.dt > 0.0);
        assert!(config.convergence_tolerance > 0.0);
    }

    #[test]
    fn test_birkhoff_config_steps() {
        let config = BirkhoffConfig {
            total_time: 100.0,
            dt: 0.1,
            transient_time: 20.0,
            ..Default::default()
        };

        assert_eq!(config.transient_steps(), 200);
        assert_eq!(config.averaging_steps(), 800);
    }

    #[test]
    fn test_birkhoff_result_confidence() {
        let result = BirkhoffResult {
            average: 1.0,
            standard_error: 0.1,
            num_samples: 1000,
            converged: true,
            convergence_history: vec![0.9, 0.95, 1.0],
            autocorrelation_time: Some(10.0),
        };

        let (low, high) = result.confidence_interval(0.95);
        assert!(low < 1.0 && high > 1.0);
        assert!((high - low - 2.0 * 1.96 * 0.1).abs() < 1e-10);
    }

    #[test]
    fn test_autocorrelation_time() {
        // Test with uncorrelated data (should give ~1)
        let uncorrelated: Vec<f64> = (0..1000).map(|i| (i as f64 * 1.234).sin()).collect();
        let tau = estimate_autocorrelation_time(&uncorrelated);
        assert!(tau.is_some());

        // Test with constant data
        let constant: Vec<f64> = vec![1.0; 1000];
        let tau_const = estimate_autocorrelation_time(&constant);
        assert!(tau_const.is_some());
    }

    #[test]
    fn test_observables() {
        use observables::*;

        let mut state = Multivector::<3, 0, 0>::zero();
        state.set(1, 2.0); // position
        state.set(2, 3.0); // velocity

        let ke = kinetic_energy::<3, 0, 0>(2);
        assert!((ke(&state) - 4.5).abs() < 1e-10);

        let pe = harmonic_potential::<3, 0, 0>(1, 1.0);
        assert!((pe(&state) - 2.0).abs() < 1e-10);

        let norm_obs = state_norm::<3, 0, 0>();
        assert!(norm_obs(&state) > 0.0);

        let comp = component::<3, 0, 0>(1);
        assert!((comp(&state) - 2.0).abs() < 1e-10);

        let comp_sq = component_squared::<3, 0, 0>(2);
        assert!((comp_sq(&state) - 9.0).abs() < 1e-10);

        let prod = component_product::<3, 0, 0>(1, 2);
        assert!((prod(&state) - 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_ergodicity_test_result() {
        let test = ErgodicityTest {
            is_ergodic: true,
            mean_average: 1.0,
            standard_deviation: 0.01,
            coefficient_of_variation: 0.01,
            max_relative_deviation: 0.02,
            individual_averages: vec![0.99, 1.01, 1.0],
            num_initial_conditions: 3,
        };

        assert!(test.is_ergodic);
        assert_eq!(test.num_initial_conditions, 3);
    }
}
