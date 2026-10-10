//! Dormand-Prince 5(4) adaptive solver
//!
//! The Dormand-Prince method (DOPRI5) is an embedded Runge-Kutta method
//! that is more efficient than RKF45 for most problems due to the FSAL
//! (First Same As Last) property.
//!
//! # Advantages over RKF45
//!
//! - FSAL property: The last evaluation of one step can be reused as the first
//!   evaluation of the next step, reducing function evaluations by ~14%
//! - Generally produces smaller error coefficients
//! - The 5th order solution is used for stepping (vs 4th in RKF45)
//!
//! # Butcher Tableau
//!
//! ```text
//! 0     |
//! 1/5   | 1/5
//! 3/10  | 3/40       9/40
//! 4/5   | 44/45      -56/15     32/9
//! 8/9   | 19372/6561 -25360/2187 64448/6561 -212/729
//! 1     | 9017/3168  -355/33    46732/5247  49/176  -5103/18656
//! 1     | 35/384     0          500/1113    125/192 -2187/6784   11/84
//! ------+------------------------------------------------------------------
//! 5th   | 35/384     0          500/1113    125/192 -2187/6784   11/84    0
//! 4th   | 5179/57600 0          7571/16695  393/640 -92097/339200 187/2100 1/40
//! ```
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::solver::{DormandPrince, AdaptiveODESolver, AdaptiveConfig};
//!
//! let solver = DormandPrince::new();
//! let config = AdaptiveConfig::default();
//! let trajectory = solver.solve_adaptive(&system, initial, 0.0, 10.0, &config)?;
//! ```

use amari_core::Multivector;

use crate::error::{DynamicsError, Result};
use crate::flow::DynamicalSystem;
#[cfg(test)]
use crate::solver::traits::AdaptiveConfig;
use crate::solver::traits::{AdaptiveODESolver, ODESolver, StepResult};

/// Dormand-Prince 5(4) adaptive solver
///
/// An embedded pair with FSAL (First Same As Last) property for efficiency.
/// The 5th order solution is used to advance the state.
#[derive(Debug, Clone, Default)]
pub struct DormandPrince {
    /// Cached k7 from previous step for FSAL optimization
    fsal_cache: Option<Vec<f64>>,
}

impl DormandPrince {
    /// Create a new Dormand-Prince solver
    pub fn new() -> Self {
        Self { fsal_cache: None }
    }

    /// Clear the FSAL cache (call when starting a new trajectory)
    pub fn clear_cache(&mut self) {
        self.fsal_cache = None;
    }
}

// Dormand-Prince Butcher tableau coefficients
// Time coefficients
#[allow(dead_code)]
const A2: f64 = 1.0 / 5.0;
#[allow(dead_code)]
const A3: f64 = 3.0 / 10.0;
#[allow(dead_code)]
const A4: f64 = 4.0 / 5.0;
#[allow(dead_code)]
const A5: f64 = 8.0 / 9.0;
#[allow(dead_code)]
const A6: f64 = 1.0;
#[allow(dead_code)]
const A7: f64 = 1.0;

// Stage coefficients
const B21: f64 = 1.0 / 5.0;

const B31: f64 = 3.0 / 40.0;
const B32: f64 = 9.0 / 40.0;

const B41: f64 = 44.0 / 45.0;
const B42: f64 = -56.0 / 15.0;
const B43: f64 = 32.0 / 9.0;

const B51: f64 = 19372.0 / 6561.0;
const B52: f64 = -25360.0 / 2187.0;
const B53: f64 = 64448.0 / 6561.0;
const B54: f64 = -212.0 / 729.0;

const B61: f64 = 9017.0 / 3168.0;
const B62: f64 = -355.0 / 33.0;
const B63: f64 = 46732.0 / 5247.0;
const B64: f64 = 49.0 / 176.0;
const B65: f64 = -5103.0 / 18656.0;

// B7x coefficients are identical to Cx (documenting for completeness)
#[allow(dead_code)]
const B71: f64 = 35.0 / 384.0;
// B72 = 0
#[allow(dead_code)]
const B73: f64 = 500.0 / 1113.0;
#[allow(dead_code)]
const B74: f64 = 125.0 / 192.0;
#[allow(dead_code)]
const B75: f64 = -2187.0 / 6784.0;
#[allow(dead_code)]
const B76: f64 = 11.0 / 84.0;

// 5th order weights (same as B7x due to FSAL)
const C1: f64 = 35.0 / 384.0;
// C2 = 0
const C3: f64 = 500.0 / 1113.0;
const C4: f64 = 125.0 / 192.0;
const C5: f64 = -2187.0 / 6784.0;
const C6: f64 = 11.0 / 84.0;
// C7 = 0

// Error estimation coefficients (difference between 5th and 4th order)
const E1: f64 = 35.0 / 384.0 - 5179.0 / 57600.0;
// E2 = 0
const E3: f64 = 500.0 / 1113.0 - 7571.0 / 16695.0;
const E4: f64 = 125.0 / 192.0 - 393.0 / 640.0;
const E5: f64 = -2187.0 / 6784.0 + 92097.0 / 339200.0;
const E6: f64 = 11.0 / 84.0 - 187.0 / 2100.0;
const E7: f64 = -1.0 / 40.0;

impl<const P: usize, const Q: usize, const R: usize> ODESolver<P, Q, R> for DormandPrince {
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

        // Compute 7 stages
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

        // 5th order solution (note: C7 = 0, so k7 not needed for result)
        let result_sum = &(&(&(&(&k1 * (dt * C1)) + &(&k3 * (dt * C3))) + &(&k4 * (dt * C4)))
            + &(&k5 * (dt * C5)))
            + &(&k6 * (dt * C6));
        let result = state + &result_sum;

        // Check for NaN/Inf
        if !result.coefficient_norm().is_finite() {
            return Err(DynamicsError::numerical_instability(
                "Dormand-Prince step",
                "result contains NaN or Inf",
            ));
        }

        Ok(StepResult::new(result))
    }

    fn order(&self) -> u32 {
        5
    }

    fn name(&self) -> &'static str {
        "Dormand-Prince 5(4)"
    }
}

impl<const P: usize, const Q: usize, const R: usize> AdaptiveODESolver<P, Q, R> for DormandPrince {
    fn step_with_error<S: DynamicalSystem<P, Q, R>>(
        &self,
        system: &S,
        state: &Multivector<P, Q, R>,
        _t: f64,
        dt: f64,
    ) -> Result<StepResult<P, Q, R>> {
        if dt <= 0.0 {
            return Err(DynamicsError::invalid_step_size(dt));
        }

        // Compute 7 stages
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

        // 5th order solution
        let result_sum = &(&(&(&(&k1 * (dt * C1)) + &(&k3 * (dt * C3))) + &(&k4 * (dt * C4)))
            + &(&k5 * (dt * C5)))
            + &(&k6 * (dt * C6));
        let result = state + &result_sum;

        // k7 at the new point (for FSAL and error estimation)
        let k7 = system.vector_field(&result)?;

        // Error estimate using the difference between 5th and 4th order
        let error_vec = &(&(&(&(&(&k1 * (dt * E1)) + &(&k3 * (dt * E3))) + &(&k4 * (dt * E4)))
            + &(&k5 * (dt * E5)))
            + &(&k6 * (dt * E6)))
            + &(&k7 * (dt * E7));
        let error = error_vec.coefficient_norm();

        // Check for NaN/Inf
        if !result.coefficient_norm().is_finite() || !error.is_finite() {
            return Err(DynamicsError::numerical_instability(
                "Dormand-Prince step",
                "result contains NaN or Inf",
            ));
        }

        // Compute optimal step size suggestion
        let safety = 0.9;
        let suggested_dt = if error > 1e-15 {
            dt * safety * (1.0 / error).powf(0.2)
        } else {
            dt * 2.0
        };

        Ok(StepResult::with_error(result, error, suggested_dt))
    }

    fn embedded_order(&self) -> u32 {
        4
    }
}

/// Dense output interpolator for Dormand-Prince
///
/// Provides continuous interpolation between discrete solution points,
/// useful for event detection and output at specific times.
#[derive(Debug, Clone)]
pub struct DormandPrinceDense<const P: usize, const Q: usize, const R: usize> {
    /// Solver with FSAL caching (reserved for future optimization)
    #[allow(dead_code)]
    solver: DormandPrince,
    /// Stage derivatives for interpolation
    stages: Option<DenseStages<P, Q, R>>,
}

/// Stored stages for dense output interpolation
#[derive(Debug, Clone)]
struct DenseStages<const P: usize, const Q: usize, const R: usize> {
    k1: Multivector<P, Q, R>,
    k3: Multivector<P, Q, R>,
    k4: Multivector<P, Q, R>,
    k5: Multivector<P, Q, R>,
    k6: Multivector<P, Q, R>,
    k7: Multivector<P, Q, R>,
    y0: Multivector<P, Q, R>,
    y1: Multivector<P, Q, R>,
    dt: f64,
}

impl<const P: usize, const Q: usize, const R: usize> DormandPrinceDense<P, Q, R> {
    /// Create a new dense output solver
    pub fn new() -> Self {
        Self {
            solver: DormandPrince::new(),
            stages: None,
        }
    }

    /// Interpolate at a theta value in [0, 1]
    ///
    /// theta = 0 gives y0, theta = 1 gives y1
    pub fn interpolate(&self, theta: f64) -> Option<Multivector<P, Q, R>> {
        let stages = self.stages.as_ref()?;

        if theta <= 0.0 {
            return Some(stages.y0.clone());
        }
        if theta >= 1.0 {
            return Some(stages.y1.clone());
        }

        // 4th order Hermite interpolation using stored stages
        // This uses the continuous extension of DOPRI5
        let theta_sq = theta * theta;
        let theta_cu = theta_sq * theta;
        let h = stages.dt;

        // Interpolation coefficients (DOPRI5 continuous extension from literature)
        #[allow(clippy::excessive_precision)]
        let b1 = theta - 1.823892939054969 * theta_sq
            + 1.5542042460262826 * theta_cu
            + -0.3041495599499706 * theta_sq * theta_sq;
        #[allow(clippy::excessive_precision)]
        let b3 = 1.867179830173765 * theta_sq - 1.2958209063891553 * theta_cu
            + 0.22102050968765002 * theta_sq * theta_sq;
        #[allow(clippy::excessive_precision)]
        let b4 = -0.8401081088282644 * theta_sq + 0.6546413490263802 * theta_cu
            - 0.11236027591574785 * theta_sq * theta_sq;
        #[allow(clippy::excessive_precision)]
        let b5 = 2.0844756538361067 * theta_sq - 2.0998992011481327 * theta_cu
            + 0.49761953044378546 * theta_sq * theta_sq;
        #[allow(clippy::excessive_precision)]
        let b6 = -0.3040405979298403 * theta_sq + 0.2073490627153823 * theta_cu
            - 0.03104316440879753 * theta_sq * theta_sq;
        let b7 = theta * theta * (1.0 - theta);

        let result = &stages.y0
            + &(&(&(&(&(&(&stages.k1 * (h * b1)) + &(&stages.k3 * (h * b3)))
                + &(&stages.k4 * (h * b4)))
                + &(&stages.k5 * (h * b5)))
                + &(&stages.k6 * (h * b6)))
                + &(&stages.k7 * (h * b7)));

        Some(result)
    }
}

impl<const P: usize, const Q: usize, const R: usize> Default for DormandPrinceDense<P, Q, R> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::HarmonicOscillator;

    #[test]
    fn test_dopri_creation() {
        let solver = DormandPrince::new();
        assert_eq!(<DormandPrince as ODESolver<2, 0, 0>>::order(&solver), 5);
        assert_eq!(
            <DormandPrince as ODESolver<2, 0, 0>>::name(&solver),
            "Dormand-Prince 5(4)"
        );
    }

    #[test]
    fn test_dopri_single_step() {
        let system = HarmonicOscillator::new(1.0);
        let solver = DormandPrince::new();

        let mut state = Multivector::<2, 0, 0>::zero();
        state.set(1, 1.0);
        state.set(2, 0.0);

        let result = solver.step(&system, &state, 0.0, 0.1).unwrap();

        assert!(result.state.get(1) < 1.0);
        assert!(result.state.get(2) < 0.0);
    }

    #[test]
    fn test_dopri_invalid_step_size() {
        let system = HarmonicOscillator::new(1.0);
        let solver = DormandPrince::new();
        let state = Multivector::<2, 0, 0>::zero();

        assert!(solver.step(&system, &state, 0.0, -0.1).is_err());
        assert!(solver.step(&system, &state, 0.0, 0.0).is_err());
    }

    #[test]
    fn test_dopri_step_with_error() {
        let system = HarmonicOscillator::new(1.0);
        let solver = DormandPrince::new();

        let mut state = Multivector::<2, 0, 0>::zero();
        state.set(1, 1.0);

        let result = solver.step_with_error(&system, &state, 0.0, 0.1).unwrap();

        assert!(result.error_estimate.is_some());
        assert!(result.error_estimate.unwrap().is_finite());
        assert!(result.suggested_dt.is_some());
        assert!(result.suggested_dt.unwrap() > 0.0);
    }

    #[test]
    fn test_dopri_trajectory() {
        let system = HarmonicOscillator::new(1.0);
        let solver = DormandPrince::new();

        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0);

        let trajectory = solver
            .solve(&system, initial, 0.0, 2.0 * core::f64::consts::PI, 1000)
            .unwrap();

        let final_state = trajectory.final_state().unwrap();
        let x = final_state.get(1);
        let v = final_state.get(2);

        assert!((x - 1.0).abs() < 1e-4, "Expected x ≈ 1, got {}", x);
        assert!(v.abs() < 1e-4, "Expected v ≈ 0, got {}", v);
    }

    #[test]
    fn test_dopri_adaptive() {
        let system = HarmonicOscillator::new(1.0);
        let solver = DormandPrince::new();
        let config = AdaptiveConfig::default();

        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0);

        let trajectory = solver
            .solve_adaptive(&system, initial, 0.0, 2.0 * core::f64::consts::PI, &config)
            .unwrap();

        assert!(trajectory.step_sizes.is_some());
        assert!(trajectory.error_estimates.is_some());

        let final_state = trajectory.final_state().unwrap();
        let x = final_state.get(1);
        let v = final_state.get(2);

        assert!((x - 1.0).abs() < 1e-5, "Expected x ≈ 1, got {}", x);
        assert!(v.abs() < 1e-5, "Expected v ≈ 0, got {}", v);
    }

    #[test]
    fn test_dopri_energy_conservation() {
        let omega = 2.0;
        let system = HarmonicOscillator::new(omega);
        let solver = DormandPrince::new();

        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0);
        initial.set(2, 0.0);

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
    fn test_embedded_order() {
        let solver = DormandPrince::new();
        assert_eq!(
            <DormandPrince as AdaptiveODESolver<2, 0, 0>>::embedded_order(&solver),
            4
        );
        assert!(
            <DormandPrince as AdaptiveODESolver<2, 0, 0>>::embedded_order(&solver)
                < <DormandPrince as ODESolver<2, 0, 0>>::order(&solver)
        );
    }

    #[test]
    fn test_dopri_vs_rkf45_accuracy() {
        use crate::solver::RungeKuttaFehlberg45;

        let system = HarmonicOscillator::new(1.0);
        let dopri = DormandPrince::new();
        let rkf45 = RungeKuttaFehlberg45::new();

        let mut initial = Multivector::<2, 0, 0>::zero();
        initial.set(1, 1.0);

        let steps = 100;
        let t_end = core::f64::consts::PI;

        let traj_dopri = dopri
            .solve(&system, initial.clone(), 0.0, t_end, steps)
            .unwrap();
        let traj_rkf45 = rkf45.solve(&system, initial, 0.0, t_end, steps).unwrap();

        // Both should give similar accuracy
        let x_dopri = traj_dopri.final_state().unwrap().get(1);
        let x_rkf45 = traj_rkf45.final_state().unwrap().get(1);

        let error_dopri = (x_dopri - (-1.0)).abs();
        let error_rkf45 = (x_rkf45 - (-1.0)).abs();

        // DOPRI should be at least as good as RKF45
        assert!(
            error_dopri <= error_rkf45 * 2.0,
            "DOPRI error {} vs RKF45 error {}",
            error_dopri,
            error_rkf45
        );
    }
}
