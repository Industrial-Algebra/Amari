//! Van der Pol oscillator
//!
//! The Van der Pol oscillator is a non-conservative oscillator with nonlinear
//! damping that exhibits self-sustained oscillations (limit cycles).
//!
//! # Equations
//!
//! ```text
//! dx/dt = y
//! dy/dt = μ(1 - x²)y - x
//! ```
//!
//! Or in second-order form:
//! ```text
//! d²x/dt² - μ(1 - x²)dx/dt + x = 0
//! ```
//!
//! # Parameters
//!
//! - μ (mu): Nonlinearity and damping parameter
//!   - μ = 0: Simple harmonic oscillator
//!   - μ small: Near-sinusoidal limit cycle
//!   - μ large: Relaxation oscillations
//!
//! # Properties
//!
//! - Has a unique unstable fixed point at the origin
//! - Has a stable limit cycle for all μ > 0
//! - Period increases with μ (approximately T ≈ (3 - 2ln2)μ for large μ)
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::systems::VanDerPolOscillator;
//! use amari_dynamics::solver::{RungeKutta4, ODESolver};
//!
//! let vdp = VanDerPolOscillator::new(1.0);
//! let solver = RungeKutta4::new();
//!
//! let initial = vdp.default_initial_condition();
//! let trajectory = solver.solve(&vdp, initial, 0.0, 50.0, 5000)?;
//! ```

use amari_core::Multivector;

use crate::error::Result;
use crate::flow::DynamicalSystem;

/// The Van der Pol oscillator
///
/// A 2D continuous dynamical system with a stable limit cycle.
/// State is represented in Cl(2,0,0) using e1, e2 for x, y.
#[derive(Debug, Clone)]
pub struct VanDerPolOscillator {
    /// Nonlinearity parameter (μ)
    pub mu: f64,
}

impl VanDerPolOscillator {
    /// Create a new Van der Pol oscillator with given μ
    pub fn new(mu: f64) -> Self {
        Self { mu }
    }

    /// Create a weakly nonlinear oscillator (μ = 0.1)
    pub fn weak() -> Self {
        Self::new(0.1)
    }

    /// Create a moderately nonlinear oscillator (μ = 1.0)
    pub fn moderate() -> Self {
        Self::new(1.0)
    }

    /// Create a strongly nonlinear (relaxation) oscillator (μ = 5.0)
    pub fn relaxation() -> Self {
        Self::new(5.0)
    }

    /// Default initial condition outside the limit cycle
    pub fn default_initial_condition(&self) -> Multivector<2, 0, 0> {
        let mut state = Multivector::zero();
        state.set(1, 2.0); // x = 2
        state.set(2, 0.0); // y = 0
        state
    }

    /// Approximate period of the limit cycle
    ///
    /// For small μ: T ≈ 2π
    /// For large μ: T ≈ (3 - 2ln2)μ ≈ 1.614μ
    pub fn approximate_period(&self) -> f64 {
        if self.mu < 0.5 {
            2.0 * std::f64::consts::PI * (1.0 + self.mu * self.mu / 16.0)
        } else {
            (3.0 - 2.0 * 2.0_f64.ln()) * self.mu + 2.0 * std::f64::consts::PI / self.mu
        }
    }

    /// Approximate amplitude of the limit cycle
    ///
    /// For all μ > 0, the limit cycle has amplitude approximately 2.
    pub fn approximate_amplitude(&self) -> f64 {
        2.0
    }

    /// Get x coordinate from state
    pub fn x(state: &Multivector<2, 0, 0>) -> f64 {
        state.get(1)
    }

    /// Get y coordinate (velocity) from state
    pub fn y(state: &Multivector<2, 0, 0>) -> f64 {
        state.get(2)
    }

    /// Compute total energy (not conserved)
    ///
    /// E = (x² + y²)/2
    pub fn energy(state: &Multivector<2, 0, 0>) -> f64 {
        let x = Self::x(state);
        let y = Self::y(state);
        0.5 * (x * x + y * y)
    }
}

impl Default for VanDerPolOscillator {
    fn default() -> Self {
        Self::moderate()
    }
}

impl DynamicalSystem<2, 0, 0> for VanDerPolOscillator {
    const DIM: usize = 2;

    fn vector_field(&self, state: &Multivector<2, 0, 0>) -> Result<Multivector<2, 0, 0>> {
        let x = state.get(1);
        let y = state.get(2);

        let mut result = Multivector::zero();
        result.set(1, y); // dx/dt = y
        result.set(2, self.mu * (1.0 - x * x) * y - x); // dy/dt = μ(1-x²)y - x

        Ok(result)
    }

    fn jacobian(&self, state: &Multivector<2, 0, 0>) -> Result<Vec<f64>> {
        let x = state.get(1);
        let y = state.get(2);

        // Jacobian matrix (row-major order):
        // [       0,           1      ]
        // [ -2μxy - 1,    μ(1 - x²)   ]
        Ok(vec![
            0.0,
            1.0,
            -2.0 * self.mu * x * y - 1.0,
            self.mu * (1.0 - x * x),
        ])
    }
}

/// Forced Van der Pol oscillator
///
/// Adds external periodic forcing to the Van der Pol equations.
///
/// # Equations
///
/// ```text
/// dx/dt = y
/// dy/dt = μ(1 - x²)y - x + A*cos(ωt)
/// ```
#[derive(Debug, Clone)]
pub struct ForcedVanDerPol {
    /// Base Van der Pol oscillator
    pub base: VanDerPolOscillator,
    /// Forcing amplitude
    pub amplitude: f64,
    /// Forcing frequency
    pub omega: f64,
}

impl ForcedVanDerPol {
    /// Create a new forced Van der Pol oscillator
    pub fn new(mu: f64, amplitude: f64, omega: f64) -> Self {
        Self {
            base: VanDerPolOscillator::new(mu),
            amplitude,
            omega,
        }
    }

    /// Evaluate the forcing at time t
    pub fn forcing(&self, t: f64) -> f64 {
        self.amplitude * (self.omega * t).cos()
    }

    /// Evaluate vector field at a given time (non-autonomous)
    pub fn vector_field_at(
        &self,
        state: &Multivector<2, 0, 0>,
        t: f64,
    ) -> Result<Multivector<2, 0, 0>> {
        let x = state.get(1);
        let y = state.get(2);

        let mut result = Multivector::zero();
        result.set(1, y);
        result.set(2, self.base.mu * (1.0 - x * x) * y - x + self.forcing(t));

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vanderpol_creation() {
        let vdp = VanDerPolOscillator::new(2.0);
        assert_eq!(vdp.mu, 2.0);
    }

    #[test]
    fn test_vanderpol_presets() {
        let weak = VanDerPolOscillator::weak();
        assert_eq!(weak.mu, 0.1);

        let moderate = VanDerPolOscillator::moderate();
        assert_eq!(moderate.mu, 1.0);

        let relaxation = VanDerPolOscillator::relaxation();
        assert_eq!(relaxation.mu, 5.0);
    }

    #[test]
    fn test_vanderpol_default() {
        let vdp = VanDerPolOscillator::default();
        assert_eq!(vdp.mu, 1.0);
    }

    #[test]
    fn test_vector_field_at_origin() {
        let vdp = VanDerPolOscillator::new(1.0);
        let origin = Multivector::zero();
        let vf = vdp.vector_field(&origin).unwrap();

        // At origin, vector field should be zero
        assert!(vf.coefficient_norm() < 1e-10);
    }

    #[test]
    fn test_vector_field_nonzero() {
        let vdp = VanDerPolOscillator::new(1.0);
        let mut state = Multivector::zero();
        state.set(1, 1.0); // x = 1
        state.set(2, 1.0); // y = 1

        let vf = vdp.vector_field(&state).unwrap();

        // dx/dt = y = 1
        assert!((vf.get(1) - 1.0).abs() < 1e-10);

        // dy/dt = μ(1 - x²)y - x = 1*(1-1)*1 - 1 = -1
        assert!((vf.get(2) - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_jacobian_at_origin() {
        let vdp = VanDerPolOscillator::new(2.0);
        let origin = Multivector::zero();
        let jac = vdp.jacobian(&origin).unwrap();

        assert_eq!(jac.len(), 4);
        assert_eq!(jac[0], 0.0); // ∂(dx/dt)/∂x = 0
        assert_eq!(jac[1], 1.0); // ∂(dx/dt)/∂y = 1
        assert_eq!(jac[2], -1.0); // ∂(dy/dt)/∂x = -2μ*0*0 - 1 = -1
        assert_eq!(jac[3], 2.0); // ∂(dy/dt)/∂y = μ(1 - 0) = μ = 2
    }

    #[test]
    fn test_approximate_period() {
        let weak = VanDerPolOscillator::weak();
        // For small μ, period ≈ 2π
        assert!((weak.approximate_period() - 2.0 * std::f64::consts::PI).abs() < 0.1);

        let relaxation = VanDerPolOscillator::relaxation();
        // For large μ, period ≈ (3 - 2ln2)μ
        let expected = (3.0 - 2.0 * 2.0_f64.ln()) * 5.0;
        assert!((relaxation.approximate_period() - expected).abs() < 2.0);
    }

    #[test]
    fn test_energy() {
        let mut state = Multivector::<2, 0, 0>::zero();
        state.set(1, 3.0);
        state.set(2, 4.0);

        let energy = VanDerPolOscillator::energy(&state);
        assert!((energy - 12.5).abs() < 1e-10); // (9 + 16) / 2 = 12.5
    }

    #[test]
    fn test_default_initial_condition() {
        let vdp = VanDerPolOscillator::new(1.0);
        let ic = vdp.default_initial_condition();
        assert_eq!(VanDerPolOscillator::x(&ic), 2.0);
        assert_eq!(VanDerPolOscillator::y(&ic), 0.0);
    }

    #[test]
    fn test_forced_vanderpol() {
        let forced = ForcedVanDerPol::new(1.0, 0.5, 2.0);
        assert_eq!(forced.base.mu, 1.0);
        assert_eq!(forced.amplitude, 0.5);
        assert_eq!(forced.omega, 2.0);

        // Check forcing at t=0
        assert!((forced.forcing(0.0) - 0.5).abs() < 1e-10);

        // Check forcing at t=π/4 (quarter period of forcing)
        let t = std::f64::consts::PI / 4.0;
        let expected = 0.5 * (2.0 * t).cos();
        assert!((forced.forcing(t) - expected).abs() < 1e-10);
    }
}
