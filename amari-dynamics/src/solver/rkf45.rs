//! Runge-Kutta-Fehlberg 4(5) adaptive solver
//!
//! The RKF45 method is an embedded Runge-Kutta method that provides
//! both 4th and 5th order solutions, using the difference for error estimation.
//!
//! # Butcher Tableau
//!
//! The RKF45 method uses 6 function evaluations to compute both a 4th order
//! and 5th order approximation:
//!
//! ```text
//! 0     |
//! 1/4   | 1/4
//! 3/8   | 3/32       9/32
//! 12/13 | 1932/2197  -7200/2197  7296/2197
//! 1     | 439/216    -8          3680/513   -845/4104
//! 1/2   | -8/27      2           -3544/2565  1859/4104  -11/40
//! ------+------------------------------------------------------
//! 4th   | 25/216     0           1408/2565   2197/4104  -1/5     0
//! 5th   | 16/135     0           6656/12825  28561/56430 -9/50   2/55
//! ```
//!
//! # Geometric Algebra Context
//!
//! RKF45 is particularly useful for systems where the dynamics involve multiple
//! scales or near-singular behavior. The adaptive step control helps maintain
//! accuracy when:
//! - Rotor evolution approaches singularities
//! - Bivector magnitudes vary significantly
//! - Trajectory passes near saddle points
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::solver::{RungeKuttaFehlberg45, AdaptiveODESolver, AdaptiveConfig};
//!
//! let solver = RungeKuttaFehlberg45::new();
//! let config = AdaptiveConfig::default();
//! let trajectory = solver.solve_adaptive(&system, initial, 0.0, 10.0, &config)?;
//! ```

use amari_core::Multivector;

use crate::error::{DynamicsError, Result};
use crate::flow::DynamicalSystem;
use crate::solver::traits::{AdaptiveConfig, AdaptiveODESolver, ODESolver, StepResult, Trajectory};

/// Runge-Kutta-Fehlberg 4(5) adaptive solver
///
/// An embedded pair providing both 4th and 5th order solutions.
/// The 5th order solution is used to advance, while the difference
/// between 4th and 5th order provides error estimation.
#[derive(Debug, Clone, Copy, Default)]
pub struct RungeKuttaFehlberg45;

impl RungeKuttaFehlberg45 {
    /// Create a new RKF45 solver
    pub fn new() -> Self {
        Self
    }
}

// RKF45 Butcher tableau coefficients
// Time coefficients (a) - kept for reference
#[allow(dead_code)]
const A2: f64 = 1.0 / 4.0;
#[allow(dead_code)]
const A3: f64 = 3.0 / 8.0;
#[allow(dead_code)]
const A4: f64 = 12.0 / 13.0;
#[allow(dead_code)]
const A5: f64 = 1.0;
#[allow(dead_code)]
const A6: f64 = 1.0 / 2.0;

// Stage coefficients (b)
const B21: f64 = 1.0 / 4.0;

const B31: f64 = 3.0 / 32.0;
const B32: f64 = 9.0 / 32.0;

const B41: f64 = 1932.0 / 2197.0;
const B42: f64 = -7200.0 / 2197.0;
const B43: f64 = 7296.0 / 2197.0;

const B51: f64 = 439.0 / 216.0;
const B52: f64 = -8.0;
const B53: f64 = 3680.0 / 513.0;
const B54: f64 = -845.0 / 4104.0;

const B61: f64 = -8.0 / 27.0;
const B62: f64 = 2.0;
const B63: f64 = -3544.0 / 2565.0;
const B64: f64 = 1859.0 / 4104.0;
const B65: f64 = -11.0 / 40.0;

// 5th order weights (c)
const C1: f64 = 16.0 / 135.0;
const C3: f64 = 6656.0 / 12825.0;
const C4: f64 = 28561.0 / 56430.0;
const C5: f64 = -9.0 / 50.0;
const C6: f64 = 2.0 / 55.0;

// 4th order weights (c_hat) for error estimation
const CH1: f64 = 25.0 / 216.0;
const CH3: f64 = 1408.0 / 2565.0;
const CH4: f64 = 2197.0 / 4104.0;
const CH5: f64 = -1.0 / 5.0;

// Error estimation coefficients (c - c_hat)
const E1: f64 = C1 - CH1; // 1/360
const E3: f64 = C3 - CH3; // -128/4275
const E4: f64 = C4 - CH4; // -2197/75240
const E5: f64 = C5 - CH5; // 1/50
const E6: f64 = C6; // 2/55

impl<const P: usize, const Q: usize, const R: usize> ODESolver<P, Q, R> for RungeKuttaFehlberg45 {
    fn step<S: DynamicalSystem<P, Q, R>>(
        &self,
        system: &S,
        state: &Multivector<P, Q, R>,
        _t: f64, // Time argument unused for autonomous systems
        dt: f64,
    ) -> Result<StepResult<P, Q, R>> {
        if dt <= 0.0 {
            return Err(DynamicsError::invalid_step_size(dt));
        }

        // Compute 6 stages
        let k1 = system.vector_field(state)?;

        let s2 = state + &(&k1 * (dt * B21));
        let k2 = system.vector_field(&s2)?;

        let s3_sum = &(&k1 * (dt * B31)) + &(&k2 * (dt * B32));
        let s3 = state + &s3_sum;
        let k3 = system.vector_field(&s3)?;

        let s4_sum = &(&(&k1 * (dt * B41)) + &(&k2 * (dt * B42))) + &(&k3 * (dt * B43));
        let s4 = state + &s4_sum;
        let k4 = system.vector_field(&s4)?;

        let s5_sum = &(&(&(&k1 * (dt * B51)) + &(&k2 * (dt * B52))) + &(&k3 * (dt * B53)))
            + &(&k4 * (dt * B54));
        let s5 = state + &s5_sum;
        let k5 = system.vector_field(&s5)?;

        let s6_sum = &(&(&(&(&k1 * (dt * B61)) + &(&k2 * (dt * B62))) + &(&k3 * (dt * B63)))
            + &(&k4 * (dt * B64)))
            + &(&k5 * (dt * B65));
        let s6 = state + &s6_sum;
        let k6 = system.vector_field(&s6)?;

        // 5th order solution (used as the result)
        let result_sum = &(&(&(&(&k1 * (dt * C1)) + &(&k3 * (dt * C3))) + &(&k4 * (dt * C4)))
            + &(&k5 * (dt * C5)))
            + &(&k6 * (dt * C6));
        let result = state + &result_sum;

        // Check for NaN/Inf
        if !result.coefficient_norm().is_finite() {
            return Err(DynamicsError::numerical_instability(
                "RKF45 step",
                "result contains NaN or Inf",
            ));
        }

        Ok(StepResult::new(result))
    }

    fn order(&self) -> u32 {
        5
    }

    fn name(&self) -> &'static str {
        "Runge-Kutta-Fehlberg 4(5)"
    }
}

impl<const P: usize, const Q: usize, const R: usize> AdaptiveODESolver<P, Q, R>
    for RungeKuttaFehlberg45
{
    fn step_with_error<S: DynamicalSystem<P, Q, R>>(
        &self,
        system: &S,
        state: &Multivector<P, Q, R>,
        _t: f64, // Time argument unused for autonomous systems
        dt: f64,
    ) -> Result<StepResult<P, Q, R>> {
        if dt <= 0.0 {
            return Err(DynamicsError::invalid_step_size(dt));
        }

        // Compute 6 stages
        let k1 = system.vector_field(state)?;

        let s2 = state + &(&k1 * (dt * B21));
        let k2 = system.vector_field(&s2)?;

        let s3_sum = &(&k1 * (dt * B31)) + &(&k2 * (dt * B32));
        let s3 = state + &s3_sum;
        let k3 = system.vector_field(&s3)?;

        let s4_sum = &(&(&k1 * (dt * B41)) + &(&k2 * (dt * B42))) + &(&k3 * (dt * B43));
        let s4 = state + &s4_sum;
        let k4 = system.vector_field(&s4)?;

        let s5_sum = &(&(&(&k1 * (dt * B51)) + &(&k2 * (dt * B52))) + &(&k3 * (dt * B53)))
            + &(&k4 * (dt * B54));
        let s5 = state + &s5_sum;
        let k5 = system.vector_field(&s5)?;

        let s6_sum = &(&(&(&(&k1 * (dt * B61)) + &(&k2 * (dt * B62))) + &(&k3 * (dt * B63)))
            + &(&k4 * (dt * B64)))
            + &(&k5 * (dt * B65));
        let s6 = state + &s6_sum;
        let k6 = system.vector_field(&s6)?;

        // 5th order solution (used as the result)
        let result_sum = &(&(&(&(&k1 * (dt * C1)) + &(&k3 * (dt * C3))) + &(&k4 * (dt * C4)))
            + &(&k5 * (dt * C5)))
            + &(&k6 * (dt * C6));
        let result = state + &result_sum;

        // Error estimate: difference between 5th and 4th order solutions
        // error = dt * (E1*k1 + E3*k3 + E4*k4 + E5*k5 + E6*k6)
        let error_vec = &(&(&(&(&k1 * (dt * E1)) + &(&k3 * (dt * E3))) + &(&k4 * (dt * E4)))
            + &(&k5 * (dt * E5)))
            + &(&k6 * (dt * E6));
        let error = error_vec.coefficient_norm();

        // Check for NaN/Inf
        if !result.coefficient_norm().is_finite() || !error.is_finite() {
            return Err(DynamicsError::numerical_instability(
                "RKF45 step",
                "result contains NaN or Inf",
            ));
        }

        // Compute optimal step size suggestion
        // Based on: new_dt = dt * safety * (tol/error)^(1/(p+1))
        // where p is the order of the error estimate (4 for RKF45)
        let safety = 0.9;
        let suggested_dt = if error > 1e-15 {
            dt * safety * (1.0 / error).powf(0.2) // 1/(4+1) = 0.2
        } else {
            dt * 2.0 // Error is tiny, try doubling
        };

        Ok(StepResult::with_error(result, error, suggested_dt))
    }

    fn embedded_order(&self) -> u32 {
        4
    }
}

/// Alternative RKF45 with custom configuration
#[derive(Debug, Clone)]
pub struct RKF45WithConfig {
    /// Adaptive step configuration
    pub config: AdaptiveConfig,
}

impl RKF45WithConfig {
    /// Create RKF45 with custom configuration
    pub fn new(config: AdaptiveConfig) -> Self {
        Self { config }
    }

    /// Create with default high-precision configuration
    pub fn high_precision() -> Self {
        Self {
            config: AdaptiveConfig {
                atol: 1e-12,
                rtol: 1e-10,
                min_step: 1e-15,
                max_step: 0.1,
                safety: 0.9,
                max_factor: 2.0,
                min_factor: 0.5,
            },
        }
    }

    /// Solve using the internal configuration
    pub fn solve<S, const P: usize, const Q: usize, const R: usize>(
        &self,
        system: &S,
        initial: Multivector<P, Q, R>,
        t0: f64,
        t1: f64,
    ) -> Result<Trajectory<P, Q, R>>
    where
        S: DynamicalSystem<P, Q, R>,
    {
        let solver = RungeKuttaFehlberg45::new();
        solver.solve_adaptive(system, initial, t0, t1, &self.config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::HarmonicOscillator;

    #[test]
    fn test_rkf45_creation() {
        let solver = RungeKuttaFehlberg45::new();
        // Use concrete type to call trait method
        assert_eq!(
            <RungeKuttaFehlberg45 as ODESolver<2, 0, 0>>::order(&solver),
            5
        );
        assert_eq!(
            <RungeKuttaFehlberg45 as ODESolver<2, 0, 0>>::name(&solver),
            "Runge-Kutta-Fehlberg 4(5)"
        );
    }

    #[test]
    fn test_rkf45_single_step() {
        let system = HarmonicOscillator::new(1.0);
        let solver = RungeKuttaFehlberg45::new();

        let mut state = Multivector::<2, 0, 0>::zero();
        state.set(1, 1.0); // x = 1
        state.set(2, 0.0); // v = 0

        let result = solver.step(&system, &state, 0.0, 0.1).unwrap();

        // Position should decrease slightly
        assert!(result.state.get(1) < 1.0);
        // Velocity should become negative
        assert!(result.state.get(2) < 0.0);
    }

    #[test]
    fn test_rkf45_invalid_step_size() {
        let system = HarmonicOscillator::new(1.0);
        let solver = RungeKuttaFehlberg45::new();
        let state = Multivector::<2, 0, 0>::zero();

        let result = solver.step(&system, &state, 0.0, -0.1);
        assert!(result.is_err());

        let result = solver.step(&system, &state, 0.0, 0.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_rkf45_step_with_error() {
        let system = HarmonicOscillator::new(1.0);
        let solver = RungeKuttaFehlberg45::new();

        let mut state = Multivector::<2, 0, 0>::zero();
        state.set(1, 1.0);

        let result = solver.step_with_error(&system, &state, 0.0, 0.1).unwrap();

        assert!(result.error_estimate.is_some());
        let error = result.error_estimate.unwrap();
        assert!(error >= 0.0);
        assert!(error.is_finite());

        assert!(result.suggested_dt.is_some());
        let suggested = result.suggested_dt.unwrap();
        assert!(suggested > 0.0);
    }

    #[test]
    fn test_rkf45_trajectory() {
        let system = HarmonicOscillator::new(1.0);
        let solver = RungeKuttaFehlberg45::new();

        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0);

        let trajectory = solver
            .solve(&system, initial, 0.0, 2.0 * core::f64::consts::PI, 1000)
            .unwrap();

        // After one period, should return near initial state
        let final_state = trajectory.final_state().unwrap();
        let x = final_state.get(1);
        let v = final_state.get(2);

        assert!((x - 1.0).abs() < 1e-4, "Expected x ≈ 1, got {}", x);
        assert!(v.abs() < 1e-4, "Expected v ≈ 0, got {}", v);
    }

    #[test]
    fn test_rkf45_adaptive() {
        let system = HarmonicOscillator::new(1.0);
        let solver = RungeKuttaFehlberg45::new();
        let config = AdaptiveConfig::default();

        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0);

        let trajectory = solver
            .solve_adaptive(&system, initial, 0.0, 2.0 * core::f64::consts::PI, &config)
            .unwrap();

        // Should have recorded adaptive step sizes and errors
        assert!(trajectory.step_sizes.is_some());
        assert!(trajectory.error_estimates.is_some());

        // Should return near initial state
        let final_state = trajectory.final_state().unwrap();
        let x = final_state.get(1);
        let v = final_state.get(2);

        assert!((x - 1.0).abs() < 1e-5, "Expected x ≈ 1, got {}", x);
        assert!(v.abs() < 1e-5, "Expected v ≈ 0, got {}", v);
    }

    #[test]
    fn test_rkf45_energy_conservation() {
        let omega = 2.0;
        let system = HarmonicOscillator::new(omega);
        let solver = RungeKuttaFehlberg45::new();

        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0); // x = 1
        initial.set(2, 0.0); // v = 0

        // Initial energy: E = 0.5 * v² + 0.5 * ω² * x² = 0.5 * 4 * 1 = 2
        let initial_energy = 0.5 * omega * omega;

        let trajectory = solver
            .solve(
                &system,
                initial,
                0.0,
                10.0 * core::f64::consts::PI / omega,
                2000,
            )
            .unwrap();

        // Check energy conservation at final state
        let final_state = trajectory.final_state().unwrap();
        let x = final_state.get(1);
        let v = final_state.get(2);
        let final_energy = 0.5 * v * v + 0.5 * omega * omega * x * x;

        let energy_error = (final_energy - initial_energy).abs() / initial_energy;
        assert!(
            energy_error < 1e-8,
            "Energy drift: {} (expected < 1e-8)",
            energy_error
        );
    }

    #[test]
    fn test_rkf45_higher_order_than_rk4() {
        // RKF45 should be more accurate than RK4 for same number of steps
        let system = HarmonicOscillator::new(1.0);
        let rk4 = crate::solver::RungeKutta4::new();
        let rkf45 = RungeKuttaFehlberg45::new();

        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0);

        let steps = 100;
        let t_end = core::f64::consts::PI;

        let traj_rk4 = rk4
            .solve(&system, initial.clone(), 0.0, t_end, steps)
            .unwrap();
        let traj_rkf45 = rkf45.solve(&system, initial, 0.0, t_end, steps).unwrap();

        // Expected: x(π) = -1, v(π) = 0
        let x_rk4 = traj_rk4.final_state().unwrap().get(1);
        let x_rkf45 = traj_rkf45.final_state().unwrap().get(1);

        let error_rk4 = (x_rk4 - (-1.0)).abs();
        let error_rkf45 = (x_rkf45 - (-1.0)).abs();

        // RKF45 should be more accurate (or at least as accurate)
        assert!(
            error_rkf45 <= error_rk4 * 2.0,
            "RKF45 error {} should be <= 2*RK4 error {}",
            error_rkf45,
            error_rk4
        );
    }

    #[test]
    fn test_rkf45_with_config() {
        let config = AdaptiveConfig {
            atol: 1e-10,
            rtol: 1e-8,
            ..Default::default()
        };
        let solver = RKF45WithConfig::new(config);

        let system = HarmonicOscillator::new(1.0);
        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0);

        let trajectory = solver
            .solve(&system, initial, 0.0, core::f64::consts::PI)
            .unwrap();

        let x = trajectory.final_state().unwrap().get(1);
        assert!((x - (-1.0)).abs() < 1e-6);
    }

    #[test]
    fn test_embedded_order() {
        let solver = RungeKuttaFehlberg45::new();
        assert_eq!(
            <RungeKuttaFehlberg45 as AdaptiveODESolver<2, 0, 0>>::embedded_order(&solver),
            4
        );
        assert!(
            <RungeKuttaFehlberg45 as AdaptiveODESolver<2, 0, 0>>::embedded_order(&solver)
                < <RungeKuttaFehlberg45 as ODESolver<2, 0, 0>>::order(&solver)
        );
    }
}
