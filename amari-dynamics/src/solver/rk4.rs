//! Classic fourth-order Runge-Kutta method
//!
//! Implements the standard RK4 method for solving ordinary differential equations:
//!
//! ```text
//! k1 = f(t_n, y_n)
//! k2 = f(t_n + h/2, y_n + h*k1/2)
//! k3 = f(t_n + h/2, y_n + h*k2/2)
//! k4 = f(t_n + h, y_n + h*k3)
//! y_{n+1} = y_n + h/6 * (k1 + 2*k2 + 2*k3 + k4)
//! ```
//!
//! # Properties
//!
//! - Fourth-order accurate (local error O(h^5), global error O(h^4))
//! - Four function evaluations per step
//! - Explicit method (suitable for non-stiff problems)
//! - No adaptive step size control (use RKF45 for adaptive stepping)
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::solver::{RungeKutta4, ODESolver};
//!
//! let solver = RungeKutta4::new();
//! let trajectory = solver.solve(&system, initial_state, 0.0, 10.0, 1000)?;
//! ```

use amari_core::Multivector;

use crate::error::{DynamicsError, Result};
use crate::flow::DynamicalSystem;
use crate::solver::{ODESolver, StepResult};

/// Classic fourth-order Runge-Kutta solver
///
/// The most widely used fixed-step explicit method for non-stiff ODEs.
/// Provides a good balance between accuracy and computational cost.
///
/// # Contracts (when `contracts` feature is enabled)
///
/// - Requires positive step size
/// - Requires state within system domain
#[derive(Debug, Clone, Copy, Default)]
pub struct RungeKutta4 {
    /// Whether to check for NaN/Inf after each step
    pub check_finite: bool,
}

impl RungeKutta4 {
    /// Create a new RK4 solver with default settings
    pub const fn new() -> Self {
        Self { check_finite: true }
    }

    /// Create an RK4 solver without finite checks (faster but less safe)
    pub const fn unchecked() -> Self {
        Self {
            check_finite: false,
        }
    }

    /// Set whether to check for finite values after each step
    pub const fn with_finite_check(mut self, check: bool) -> Self {
        self.check_finite = check;
        self
    }

    /// Validate that all components of a multivector are finite
    fn check_state_finite<const P: usize, const Q: usize, const R: usize>(
        &self,
        state: &Multivector<P, Q, R>,
    ) -> Result<()> {
        if !self.check_finite {
            return Ok(());
        }

        // Check all coefficients are finite
        for i in 0..(1 << (P + Q + R)) {
            let val = state.get(i);
            if !val.is_finite() {
                return Err(DynamicsError::numerical_instability(
                    "RK4 step",
                    format!("non-finite value at component {}: {}", i, val),
                ));
            }
        }

        Ok(())
    }
}

impl<const P: usize, const Q: usize, const R: usize> ODESolver<P, Q, R> for RungeKutta4 {
    fn step<S: DynamicalSystem<P, Q, R>>(
        &self,
        system: &S,
        state: &Multivector<P, Q, R>,
        _t: f64,
        dt: f64,
    ) -> Result<StepResult<P, Q, R>> {
        // Validate step size
        if dt <= 0.0 {
            return Err(DynamicsError::invalid_step_size(dt));
        }

        // k1 = f(y_n)
        let k1 = system.vector_field(state)?;
        self.check_state_finite(&k1)?;

        // k2 = f(y_n + h/2 * k1)
        let y2 = state + &(&k1 * (dt / 2.0));
        if !system.in_domain(&y2) {
            return Err(DynamicsError::OutOfDomain);
        }
        let k2 = system.vector_field(&y2)?;
        self.check_state_finite(&k2)?;

        // k3 = f(y_n + h/2 * k2)
        let y3 = state + &(&k2 * (dt / 2.0));
        if !system.in_domain(&y3) {
            return Err(DynamicsError::OutOfDomain);
        }
        let k3 = system.vector_field(&y3)?;
        self.check_state_finite(&k3)?;

        // k4 = f(y_n + h * k3)
        let y4 = state + &(&k3 * dt);
        if !system.in_domain(&y4) {
            return Err(DynamicsError::OutOfDomain);
        }
        let k4 = system.vector_field(&y4)?;
        self.check_state_finite(&k4)?;

        // y_{n+1} = y_n + h/6 * (k1 + 2*k2 + 2*k3 + k4)
        let k2_2 = &k2 * 2.0;
        let k3_2 = &k3 * 2.0;
        let sum = &(&(&k1 + &k2_2) + &k3_2) + &k4;
        let increment = &sum * (dt / 6.0);
        let new_state = state + &increment;

        self.check_state_finite(&new_state)?;

        if !system.in_domain(&new_state) {
            return Err(DynamicsError::OutOfDomain);
        }

        Ok(StepResult::new(new_state))
    }

    fn order(&self) -> u32 {
        4
    }

    fn name(&self) -> &'static str {
        "Runge-Kutta 4"
    }
}

/// Forward Euler method (first-order)
///
/// The simplest explicit method: y_{n+1} = y_n + h * f(y_n)
///
/// Primarily useful for comparison and educational purposes.
/// Use RK4 or adaptive methods for actual computations.
#[derive(Debug, Clone, Copy, Default)]
pub struct ForwardEuler {
    /// Whether to check for NaN/Inf after each step
    pub check_finite: bool,
}

impl ForwardEuler {
    /// Create a new Forward Euler solver
    pub const fn new() -> Self {
        Self { check_finite: true }
    }
}

impl<const P: usize, const Q: usize, const R: usize> ODESolver<P, Q, R> for ForwardEuler {
    fn step<S: DynamicalSystem<P, Q, R>>(
        &self,
        system: &S,
        state: &Multivector<P, Q, R>,
        _t: f64,
        dt: f64,
    ) -> Result<StepResult<P, Q, R>> {
        if dt <= 0.0 {
            return Err(DynamicsError::invalid_step_size(dt));
        }

        let k = system.vector_field(state)?;
        let new_state = state + &(&k * dt);

        if self.check_finite {
            for i in 0..(1 << (P + Q + R)) {
                let val = new_state.get(i);
                if !val.is_finite() {
                    return Err(DynamicsError::numerical_instability(
                        "Forward Euler step",
                        format!("non-finite value at component {}", i),
                    ));
                }
            }
        }

        if !system.in_domain(&new_state) {
            return Err(DynamicsError::OutOfDomain);
        }

        Ok(StepResult::new(new_state))
    }

    fn order(&self) -> u32 {
        1
    }

    fn name(&self) -> &'static str {
        "Forward Euler"
    }
}

/// Midpoint method (second-order)
///
/// Also known as explicit midpoint or RK2:
/// ```text
/// k1 = f(y_n)
/// k2 = f(y_n + h/2 * k1)
/// y_{n+1} = y_n + h * k2
/// ```
#[derive(Debug, Clone, Copy, Default)]
pub struct MidpointMethod {
    /// Whether to check for NaN/Inf after each step
    pub check_finite: bool,
}

impl MidpointMethod {
    /// Create a new Midpoint solver
    pub const fn new() -> Self {
        Self { check_finite: true }
    }
}

impl<const P: usize, const Q: usize, const R: usize> ODESolver<P, Q, R> for MidpointMethod {
    fn step<S: DynamicalSystem<P, Q, R>>(
        &self,
        system: &S,
        state: &Multivector<P, Q, R>,
        _t: f64,
        dt: f64,
    ) -> Result<StepResult<P, Q, R>> {
        if dt <= 0.0 {
            return Err(DynamicsError::invalid_step_size(dt));
        }

        // k1 = f(y_n)
        let k1 = system.vector_field(state)?;

        // k2 = f(y_n + h/2 * k1)
        let y_mid = state + &(&k1 * (dt / 2.0));
        if !system.in_domain(&y_mid) {
            return Err(DynamicsError::OutOfDomain);
        }
        let k2 = system.vector_field(&y_mid)?;

        // y_{n+1} = y_n + h * k2
        let new_state = state + &(&k2 * dt);

        if self.check_finite {
            for i in 0..(1 << (P + Q + R)) {
                let val = new_state.get(i);
                if !val.is_finite() {
                    return Err(DynamicsError::numerical_instability(
                        "Midpoint step",
                        format!("non-finite value at component {}", i),
                    ));
                }
            }
        }

        if !system.in_domain(&new_state) {
            return Err(DynamicsError::OutOfDomain);
        }

        Ok(StepResult::new(new_state))
    }

    fn order(&self) -> u32 {
        2
    }

    fn name(&self) -> &'static str {
        "Midpoint (RK2)"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::HarmonicOscillator;

    #[test]
    fn test_rk4_creation() {
        let solver = RungeKutta4::new();
        // Use concrete type to call trait method
        assert_eq!(<RungeKutta4 as ODESolver<2, 0, 0>>::order(&solver), 4);
        assert_eq!(
            <RungeKutta4 as ODESolver<2, 0, 0>>::name(&solver),
            "Runge-Kutta 4"
        );
        assert!(solver.check_finite);

        let unchecked = RungeKutta4::unchecked();
        assert!(!unchecked.check_finite);
    }

    #[test]
    fn test_forward_euler_creation() {
        let solver = ForwardEuler::new();
        assert_eq!(<ForwardEuler as ODESolver<2, 0, 0>>::order(&solver), 1);
        assert_eq!(
            <ForwardEuler as ODESolver<2, 0, 0>>::name(&solver),
            "Forward Euler"
        );
    }

    #[test]
    fn test_midpoint_creation() {
        let solver = MidpointMethod::new();
        assert_eq!(<MidpointMethod as ODESolver<2, 0, 0>>::order(&solver), 2);
        assert_eq!(
            <MidpointMethod as ODESolver<2, 0, 0>>::name(&solver),
            "Midpoint (RK2)"
        );
    }

    #[test]
    fn test_rk4_invalid_step_size() {
        let solver = RungeKutta4::new();
        let system = HarmonicOscillator::new(1.0);
        let mut state = Multivector::<2, 0, 0>::zero();
        state.set(1, 1.0); // x = 1
        state.set(2, 0.0); // v = 0

        let result = solver.step(&system, &state, 0.0, 0.0);
        assert!(matches!(result, Err(DynamicsError::InvalidStepSize { .. })));

        let result = solver.step(&system, &state, 0.0, -0.1);
        assert!(matches!(result, Err(DynamicsError::InvalidStepSize { .. })));
    }

    #[test]
    fn test_rk4_single_step() {
        let solver = RungeKutta4::new();
        let system = HarmonicOscillator::new(1.0);

        // Initial condition: x = 1, v = 0
        let mut state = Multivector::<2, 0, 0>::zero();
        state.set(1, 1.0);
        state.set(2, 0.0);

        let result = solver.step(&system, &state, 0.0, 0.01);
        assert!(result.is_ok());

        let new_state = result.unwrap().state;
        // Position should decrease slightly (harmonic oscillator)
        assert!(new_state.get(1) < 1.0);
        // Velocity should become negative
        assert!(new_state.get(2) < 0.0);
    }

    #[test]
    fn test_rk4_harmonic_oscillator_trajectory() {
        let solver = RungeKutta4::new();
        let system = HarmonicOscillator::new(1.0);

        // Initial condition: x = 1, v = 0
        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0);
        initial.set(2, 0.0);

        // Integrate for one period (2π for ω=1)
        let period = 2.0 * std::f64::consts::PI;
        let trajectory = solver.solve(&system, initial, 0.0, period, 10000);
        assert!(trajectory.is_ok());

        let traj = trajectory.unwrap();
        assert_eq!(traj.len(), 10001);

        // After one period, should return close to initial state
        let final_state = traj.final_state().unwrap();
        let x_final = final_state.get(1);
        let v_final = final_state.get(2);

        // Should be close to (1, 0) after one period
        assert!((x_final - 1.0).abs() < 1e-4, "x_final = {}", x_final);
        assert!(v_final.abs() < 1e-4, "v_final = {}", v_final);
    }

    #[test]
    fn test_rk4_energy_conservation() {
        let solver = RungeKutta4::new();
        let omega = 2.0;
        let system = HarmonicOscillator::new(omega);

        // Initial condition: x = 1, v = 0
        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0);
        initial.set(2, 0.0);

        // Energy = 0.5 * v^2 + 0.5 * ω^2 * x^2
        let compute_energy = |state: &Multivector<2, 0, 0>| {
            let x = state.get(1);
            let v = state.get(2);
            0.5 * v * v + 0.5 * omega * omega * x * x
        };

        let initial_energy = compute_energy(&initial);

        let trajectory = solver.solve(&system, initial, 0.0, 10.0, 10000).unwrap();

        // Check energy at several points
        for (_, state) in trajectory.iter() {
            let energy = compute_energy(state);
            let relative_error = (energy - initial_energy).abs() / initial_energy;
            assert!(
                relative_error < 1e-6,
                "Energy drift: {} vs {}",
                energy,
                initial_energy
            );
        }
    }

    #[test]
    fn test_solver_order_comparison() {
        // Compare convergence of different order methods
        let system = HarmonicOscillator::new(1.0);

        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0);
        initial.set(2, 0.0);

        let t_final = 1.0;

        // Compute "exact" solution with very fine RK4
        let rk4 = RungeKutta4::new();
        let exact = rk4
            .solve(&system, initial.clone(), 0.0, t_final, 100000)
            .unwrap();
        let exact_final = exact.final_state().unwrap().clone();

        // Test convergence at different step counts
        let step_counts = [100, 200, 400, 800];
        let mut euler_errors = Vec::new();
        let mut rk4_errors = Vec::new();

        let euler = ForwardEuler::new();

        for &steps in &step_counts {
            let euler_traj = euler
                .solve(&system, initial.clone(), 0.0, t_final, steps)
                .unwrap();
            let rk4_traj = rk4
                .solve(&system, initial.clone(), 0.0, t_final, steps)
                .unwrap();

            let euler_error = (euler_traj.final_state().unwrap() - &exact_final).coefficient_norm();
            let rk4_error = (rk4_traj.final_state().unwrap() - &exact_final).coefficient_norm();

            euler_errors.push(euler_error);
            rk4_errors.push(rk4_error);
        }

        // Check that RK4 converges faster than Euler
        // When steps double, Euler error should roughly halve (O(h))
        // RK4 error should decrease by factor of 16 (O(h^4))
        for i in 1..euler_errors.len() {
            // RK4 should have much smaller error than Euler at same step count
            assert!(rk4_errors[i] < euler_errors[i] * 0.01);
        }
    }
}
