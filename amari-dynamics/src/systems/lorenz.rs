//! Lorenz attractor
//!
//! The Lorenz system is a simplified model of atmospheric convection that
//! exhibits chaotic behavior for certain parameter values.
//!
//! # Equations
//!
//! ```text
//! dx/dt = σ(y - x)
//! dy/dt = x(ρ - z) - y
//! dz/dt = xy - βz
//! ```
//!
//! # Parameters
//!
//! - σ (sigma): Prandtl number, typically 10
//! - ρ (rho): Rayleigh number, typically 28 for chaos
//! - β (beta): Geometric factor, typically 8/3
//!
//! # Classic Parameter Values
//!
//! The classic chaotic parameters are σ=10, ρ=28, β=8/3, which produce
//! the famous butterfly-shaped strange attractor with Lyapunov exponents
//! approximately (0.9, 0, -14.6).
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::systems::LorenzSystem;
//! use amari_dynamics::solver::{RungeKutta4, ODESolver};
//!
//! let lorenz = LorenzSystem::classic();
//! let solver = RungeKutta4::new();
//!
//! let initial = lorenz.default_initial_condition();
//! let trajectory = solver.solve(&lorenz, initial, 0.0, 100.0, 10000)?;
//! ```

use amari_core::Multivector;

use crate::error::Result;
use crate::flow::DynamicalSystem;

/// The Lorenz system
///
/// A 3D continuous dynamical system that exhibits chaotic behavior.
/// State is represented in Cl(3,0,0) using e1, e2, e4 for x, y, z.
#[derive(Debug, Clone)]
pub struct LorenzSystem {
    /// Prandtl number (σ)
    pub sigma: f64,
    /// Rayleigh number (ρ)
    pub rho: f64,
    /// Geometric factor (β)
    pub beta: f64,
}

impl LorenzSystem {
    /// Create a new Lorenz system with given parameters
    pub fn new(sigma: f64, rho: f64, beta: f64) -> Self {
        Self { sigma, rho, beta }
    }

    /// Create the classic chaotic Lorenz system (σ=10, ρ=28, β=8/3)
    pub fn classic() -> Self {
        Self::new(10.0, 28.0, 8.0 / 3.0)
    }

    /// Create a Lorenz system with custom Rayleigh number
    ///
    /// Uses σ=10, β=8/3, with custom ρ for bifurcation studies.
    pub fn with_rho(rho: f64) -> Self {
        Self::new(10.0, rho, 8.0 / 3.0)
    }

    /// Default initial condition near the attractor
    pub fn default_initial_condition(&self) -> Multivector<3, 0, 0> {
        let mut state = Multivector::zero();
        state.set(1, 1.0); // x = 1
        state.set(2, 1.0); // y = 1
        state.set(4, 1.0); // z = 1
        state
    }

    /// Get the fixed points of the system
    ///
    /// For ρ < 1: Only origin (0, 0, 0)
    /// For ρ > 1: Origin plus two symmetric points C± = (±√(β(ρ-1)), ±√(β(ρ-1)), ρ-1)
    pub fn fixed_points(&self) -> Vec<Multivector<3, 0, 0>> {
        let mut fps = Vec::new();

        // Origin is always a fixed point
        fps.push(Multivector::zero());

        // For ρ > 1, there are two additional fixed points
        if self.rho > 1.0 {
            let c = (self.beta * (self.rho - 1.0)).sqrt();
            let z = self.rho - 1.0;

            let mut c_plus = Multivector::zero();
            c_plus.set(1, c);
            c_plus.set(2, c);
            c_plus.set(4, z);
            fps.push(c_plus);

            let mut c_minus = Multivector::zero();
            c_minus.set(1, -c);
            c_minus.set(2, -c);
            c_minus.set(4, z);
            fps.push(c_minus);
        }

        fps
    }

    /// Check if parameters are in the chaotic regime
    ///
    /// Chaos occurs approximately for ρ > 24.74 (with σ=10, β=8/3)
    pub fn is_chaotic(&self) -> bool {
        // Approximate criterion for classic parameters
        if (self.sigma - 10.0).abs() < 0.1 && (self.beta - 8.0 / 3.0).abs() < 0.1 {
            self.rho > 24.74
        } else {
            // For other parameters, need numerical verification
            self.rho > 1.0 // Conservative estimate
        }
    }

    /// Get the critical Rayleigh number for Hopf bifurcation
    ///
    /// For the classic parameters, this is approximately 24.74
    pub fn hopf_bifurcation_rho(&self) -> f64 {
        self.sigma * (self.sigma + self.beta + 3.0) / (self.sigma - self.beta - 1.0)
    }

    /// Get x coordinate from state
    pub fn x(state: &Multivector<3, 0, 0>) -> f64 {
        state.get(1)
    }

    /// Get y coordinate from state
    pub fn y(state: &Multivector<3, 0, 0>) -> f64 {
        state.get(2)
    }

    /// Get z coordinate from state
    pub fn z(state: &Multivector<3, 0, 0>) -> f64 {
        state.get(4)
    }
}

impl Default for LorenzSystem {
    fn default() -> Self {
        Self::classic()
    }
}

impl DynamicalSystem<3, 0, 0> for LorenzSystem {
    const DIM: usize = 3;

    fn vector_field(&self, state: &Multivector<3, 0, 0>) -> Result<Multivector<3, 0, 0>> {
        let x = state.get(1);
        let y = state.get(2);
        let z = state.get(4);

        let mut result = Multivector::zero();
        result.set(1, self.sigma * (y - x)); // dx/dt
        result.set(2, x * (self.rho - z) - y); // dy/dt
        result.set(4, x * y - self.beta * z); // dz/dt

        Ok(result)
    }

    fn jacobian(&self, state: &Multivector<3, 0, 0>) -> Result<Vec<f64>> {
        let x = state.get(1);
        let y = state.get(2);
        let z = state.get(4);

        // Jacobian matrix (row-major order):
        // [ -σ,      σ,    0 ]
        // [ ρ-z,    -1,   -x ]
        // [  y,      x,   -β ]
        Ok(vec![
            -self.sigma,
            self.sigma,
            0.0,
            self.rho - z,
            -1.0,
            -x,
            y,
            x,
            -self.beta,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lorenz_creation() {
        let lorenz = LorenzSystem::classic();
        assert_eq!(lorenz.sigma, 10.0);
        assert_eq!(lorenz.rho, 28.0);
        assert!((lorenz.beta - 8.0 / 3.0).abs() < 1e-10);
    }

    #[test]
    fn test_lorenz_default() {
        let lorenz = LorenzSystem::default();
        assert_eq!(lorenz.sigma, 10.0);
        assert_eq!(lorenz.rho, 28.0);
    }

    #[test]
    fn test_lorenz_with_rho() {
        let lorenz = LorenzSystem::with_rho(20.0);
        assert_eq!(lorenz.rho, 20.0);
        assert_eq!(lorenz.sigma, 10.0);
    }

    #[test]
    fn test_fixed_points_below_critical() {
        let lorenz = LorenzSystem::with_rho(0.5);
        let fps = lorenz.fixed_points();
        assert_eq!(fps.len(), 1); // Only origin
    }

    #[test]
    fn test_fixed_points_above_critical() {
        let lorenz = LorenzSystem::classic();
        let fps = lorenz.fixed_points();
        assert_eq!(fps.len(), 3); // Origin + two symmetric points

        // Check C+ point
        let c = (lorenz.beta * (lorenz.rho - 1.0)).sqrt();
        let c_plus = &fps[1];
        assert!((LorenzSystem::x(c_plus) - c).abs() < 1e-10);
        assert!((LorenzSystem::y(c_plus) - c).abs() < 1e-10);
        assert!((LorenzSystem::z(c_plus) - (lorenz.rho - 1.0)).abs() < 1e-10);
    }

    #[test]
    fn test_is_chaotic() {
        let chaotic = LorenzSystem::classic();
        assert!(chaotic.is_chaotic());

        let non_chaotic = LorenzSystem::with_rho(20.0);
        assert!(!non_chaotic.is_chaotic());
    }

    #[test]
    fn test_vector_field_at_origin() {
        let lorenz = LorenzSystem::classic();
        let origin = Multivector::zero();
        let vf = lorenz.vector_field(&origin).unwrap();

        // At origin, vector field should be zero
        assert!(vf.coefficient_norm() < 1e-10);
    }

    #[test]
    fn test_vector_field_nonzero() {
        let lorenz = LorenzSystem::classic();
        let mut state = Multivector::zero();
        state.set(1, 1.0);
        state.set(2, 2.0);
        state.set(4, 3.0);

        let vf = lorenz.vector_field(&state).unwrap();

        // dx/dt = σ(y - x) = 10(2 - 1) = 10
        assert!((vf.get(1) - 10.0).abs() < 1e-10);

        // dy/dt = x(ρ - z) - y = 1(28 - 3) - 2 = 23
        assert!((vf.get(2) - 23.0).abs() < 1e-10);

        // dz/dt = xy - βz = 1*2 - (8/3)*3 = 2 - 8 = -6
        assert!((vf.get(4) - (-6.0)).abs() < 1e-10);
    }

    #[test]
    fn test_jacobian_at_origin() {
        let lorenz = LorenzSystem::classic();
        let origin = Multivector::zero();
        let jac = lorenz.jacobian(&origin).unwrap();

        assert_eq!(jac.len(), 9);
        assert_eq!(jac[0], -10.0); // ∂(dx/dt)/∂x = -σ
        assert_eq!(jac[1], 10.0); // ∂(dx/dt)/∂y = σ
        assert_eq!(jac[3], 28.0); // ∂(dy/dt)/∂x = ρ - z = 28
        assert_eq!(jac[4], -1.0); // ∂(dy/dt)/∂y = -1
    }

    #[test]
    fn test_hopf_bifurcation() {
        let lorenz = LorenzSystem::classic();
        let rho_h = lorenz.hopf_bifurcation_rho();
        // For σ=10, β=8/3: ρ_H ≈ 24.74
        assert!((rho_h - 24.74).abs() < 0.1);
    }

    #[test]
    fn test_default_initial_condition() {
        let lorenz = LorenzSystem::classic();
        let ic = lorenz.default_initial_condition();
        assert_eq!(LorenzSystem::x(&ic), 1.0);
        assert_eq!(LorenzSystem::y(&ic), 1.0);
        assert_eq!(LorenzSystem::z(&ic), 1.0);
    }
}
