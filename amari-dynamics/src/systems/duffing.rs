//! Duffing oscillator
//!
//! The Duffing oscillator is a nonlinear second-order differential equation
//! with a cubic stiffness term that models various physical phenomena including
//! mechanical oscillators with nonlinear restoring forces.
//!
//! # Equations
//!
//! The autonomous (unforced) Duffing equation:
//! ```text
//! dx/dt = y
//! dy/dt = -δy - αx - βx³
//! ```
//!
//! The forced (non-autonomous) Duffing equation:
//! ```text
//! dx/dt = y
//! dy/dt = -δy - αx - βx³ + γcos(ωt)
//! ```
//!
//! # Parameters
//!
//! - δ (delta): Damping coefficient
//! - α (alpha): Linear stiffness coefficient
//! - β (beta): Nonlinear (cubic) stiffness coefficient
//! - γ (gamma): Forcing amplitude (forced oscillator only)
//! - ω (omega): Forcing frequency (forced oscillator only)
//!
//! # Potential Types
//!
//! The potential energy is V(x) = αx²/2 + βx⁴/4
//!
//! - **Double-well** (α < 0, β > 0): Two stable equilibria at x = ±√(-α/β)
//! - **Hardening spring** (α > 0, β > 0): Single equilibrium, stiffness increases with amplitude
//! - **Softening spring** (α > 0, β < 0): Single equilibrium, stiffness decreases with amplitude
//!
//! # Classic Parameter Values
//!
//! - Double-well: α = -1, β = 1, δ = 0.3
//! - Chaotic (forced): α = -1, β = 1, δ = 0.25, γ = 0.3, ω = 1.0
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::systems::DuffingOscillator;
//! use amari_dynamics::solver::{RungeKutta4, ODESolver};
//!
//! let duffing = DuffingOscillator::double_well();
//! let solver = RungeKutta4::new();
//!
//! let initial = duffing.default_initial_condition();
//! let trajectory = solver.solve(&duffing, initial, 0.0, 100.0, 10000)?;
//! ```

use amari_core::Multivector;

use crate::error::Result;
use crate::flow::DynamicalSystem;

/// The autonomous Duffing oscillator
///
/// A 2D continuous dynamical system with cubic nonlinearity.
/// State is represented in Cl(2,0,0) using e1, e2 for x, y.
#[derive(Debug, Clone)]
pub struct DuffingOscillator {
    /// Damping coefficient (δ)
    pub delta: f64,
    /// Linear stiffness coefficient (α)
    pub alpha: f64,
    /// Nonlinear stiffness coefficient (β)
    pub beta: f64,
}

impl DuffingOscillator {
    /// Create a new Duffing oscillator with given parameters
    pub fn new(delta: f64, alpha: f64, beta: f64) -> Self {
        Self { delta, alpha, beta }
    }

    /// Create a double-well Duffing oscillator (α=-1, β=1, δ=0.3)
    ///
    /// This configuration has two stable equilibria and is bistable.
    pub fn double_well() -> Self {
        Self::new(0.3, -1.0, 1.0)
    }

    /// Create a hardening spring Duffing oscillator (α=1, β=1, δ=0.1)
    ///
    /// Single equilibrium at origin, stiffness increases with amplitude.
    pub fn hardening_spring() -> Self {
        Self::new(0.1, 1.0, 1.0)
    }

    /// Create a softening spring Duffing oscillator (α=1, β=-1, δ=0.1)
    ///
    /// Single equilibrium at origin, stiffness decreases with amplitude.
    pub fn softening_spring() -> Self {
        Self::new(0.1, 1.0, -1.0)
    }

    /// Create an undamped double-well oscillator (δ=0, α=-1, β=1)
    ///
    /// Conservative system for studying potential well dynamics.
    pub fn undamped_double_well() -> Self {
        Self::new(0.0, -1.0, 1.0)
    }

    /// Default initial condition
    pub fn default_initial_condition(&self) -> Multivector<2, 0, 0> {
        let mut state = Multivector::zero();
        state.set(1, 1.0); // x = 1
        state.set(2, 0.0); // y = 0
        state
    }

    /// Get the fixed points of the system
    ///
    /// For α > 0, β > 0: Only origin (0, 0)
    /// For α < 0, β > 0: Origin plus two symmetric points at (±√(-α/β), 0)
    pub fn fixed_points(&self) -> Vec<Multivector<2, 0, 0>> {
        let mut fps = Vec::new();

        // Origin is always a fixed point
        fps.push(Multivector::zero());

        // For double-well potential (α < 0, β > 0), there are two additional fixed points
        if self.alpha < 0.0 && self.beta > 0.0 {
            let x_eq = (-self.alpha / self.beta).sqrt();

            let mut fp_plus = Multivector::zero();
            fp_plus.set(1, x_eq);
            fps.push(fp_plus);

            let mut fp_minus = Multivector::zero();
            fp_minus.set(1, -x_eq);
            fps.push(fp_minus);
        }

        fps
    }

    /// Compute the potential energy V(x) = αx²/2 + βx⁴/4
    pub fn potential_energy(state: &Multivector<2, 0, 0>, alpha: f64, beta: f64) -> f64 {
        let x = state.get(1);
        0.5 * alpha * x * x + 0.25 * beta * x * x * x * x
    }

    /// Compute the kinetic energy K = y²/2
    pub fn kinetic_energy(state: &Multivector<2, 0, 0>) -> f64 {
        let y = state.get(2);
        0.5 * y * y
    }

    /// Compute total mechanical energy E = K + V
    ///
    /// For undamped system (δ=0), this is conserved.
    pub fn total_energy(&self, state: &Multivector<2, 0, 0>) -> f64 {
        Self::kinetic_energy(state) + Self::potential_energy(state, self.alpha, self.beta)
    }

    /// Check if the system is in a double-well configuration
    pub fn is_double_well(&self) -> bool {
        self.alpha < 0.0 && self.beta > 0.0
    }

    /// Barrier height for double-well potential
    ///
    /// The barrier height at x=0 relative to the wells at x=±√(-α/β)
    /// Returns None if not in double-well configuration.
    pub fn barrier_height(&self) -> Option<f64> {
        if self.is_double_well() {
            // V(0) - V(x_eq) where x_eq = √(-α/β)
            // V(0) = 0
            // V(x_eq) = α*(-α/β)/2 + β*(-α/β)²/4 = -α²/(2β) + α²/(4β) = -α²/(4β)
            Some(self.alpha * self.alpha / (4.0 * self.beta))
        } else {
            None
        }
    }

    /// Get x coordinate from state
    pub fn x(state: &Multivector<2, 0, 0>) -> f64 {
        state.get(1)
    }

    /// Get y coordinate (velocity) from state
    pub fn y(state: &Multivector<2, 0, 0>) -> f64 {
        state.get(2)
    }
}

impl Default for DuffingOscillator {
    fn default() -> Self {
        Self::double_well()
    }
}

impl DynamicalSystem<2, 0, 0> for DuffingOscillator {
    const DIM: usize = 2;

    fn vector_field(&self, state: &Multivector<2, 0, 0>) -> Result<Multivector<2, 0, 0>> {
        let x = state.get(1);
        let y = state.get(2);

        let mut result = Multivector::zero();
        result.set(1, y); // dx/dt = y
        result.set(2, -self.delta * y - self.alpha * x - self.beta * x * x * x); // dy/dt

        Ok(result)
    }

    fn jacobian(&self, state: &Multivector<2, 0, 0>) -> Result<Vec<f64>> {
        let x = state.get(1);

        // Jacobian matrix (row-major order):
        // [     0,              1     ]
        // [ -α - 3βx²,        -δ     ]
        Ok(vec![
            0.0,
            1.0,
            -self.alpha - 3.0 * self.beta * x * x,
            -self.delta,
        ])
    }
}

/// Forced Duffing oscillator
///
/// Adds external periodic forcing to the Duffing equations, which can
/// produce chaotic behavior for certain parameter combinations.
///
/// # Equations
///
/// ```text
/// dx/dt = y
/// dy/dt = -δy - αx - βx³ + γcos(ωt)
/// ```
#[derive(Debug, Clone)]
pub struct ForcedDuffing {
    /// Base Duffing oscillator
    pub base: DuffingOscillator,
    /// Forcing amplitude (γ)
    pub gamma: f64,
    /// Forcing frequency (ω)
    pub omega: f64,
}

impl ForcedDuffing {
    /// Create a new forced Duffing oscillator
    pub fn new(delta: f64, alpha: f64, beta: f64, gamma: f64, omega: f64) -> Self {
        Self {
            base: DuffingOscillator::new(delta, alpha, beta),
            gamma,
            omega,
        }
    }

    /// Create the classic chaotic Duffing oscillator
    ///
    /// Parameters: δ=0.25, α=-1, β=1, γ=0.3, ω=1.0
    pub fn chaotic() -> Self {
        Self::new(0.25, -1.0, 1.0, 0.3, 1.0)
    }

    /// Create a strongly driven double-well
    ///
    /// Parameters: δ=0.3, α=-1, β=1, γ=0.5, ω=1.0
    pub fn strongly_driven() -> Self {
        Self::new(0.3, -1.0, 1.0, 0.5, 1.0)
    }

    /// Evaluate the forcing at time t
    pub fn forcing(&self, t: f64) -> f64 {
        self.gamma * (self.omega * t).cos()
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
        result.set(
            2,
            -self.base.delta * y - self.base.alpha * x - self.base.beta * x * x * x
                + self.forcing(t),
        );

        Ok(result)
    }

    /// Period of the forcing
    pub fn forcing_period(&self) -> f64 {
        2.0 * std::f64::consts::PI / self.omega
    }

    /// Default initial condition
    pub fn default_initial_condition(&self) -> Multivector<2, 0, 0> {
        self.base.default_initial_condition()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_duffing_creation() {
        let duffing = DuffingOscillator::new(0.1, -1.0, 1.0);
        assert_eq!(duffing.delta, 0.1);
        assert_eq!(duffing.alpha, -1.0);
        assert_eq!(duffing.beta, 1.0);
    }

    #[test]
    fn test_duffing_double_well() {
        let duffing = DuffingOscillator::double_well();
        assert_eq!(duffing.alpha, -1.0);
        assert_eq!(duffing.beta, 1.0);
        assert!(duffing.is_double_well());
    }

    #[test]
    fn test_duffing_hardening_spring() {
        let duffing = DuffingOscillator::hardening_spring();
        assert_eq!(duffing.alpha, 1.0);
        assert_eq!(duffing.beta, 1.0);
        assert!(!duffing.is_double_well());
    }

    #[test]
    fn test_duffing_softening_spring() {
        let duffing = DuffingOscillator::softening_spring();
        assert_eq!(duffing.alpha, 1.0);
        assert_eq!(duffing.beta, -1.0);
        assert!(!duffing.is_double_well());
    }

    #[test]
    fn test_duffing_default() {
        let duffing = DuffingOscillator::default();
        assert!(duffing.is_double_well());
    }

    #[test]
    fn test_fixed_points_single_well() {
        let duffing = DuffingOscillator::hardening_spring();
        let fps = duffing.fixed_points();
        assert_eq!(fps.len(), 1); // Only origin
    }

    #[test]
    fn test_fixed_points_double_well() {
        let duffing = DuffingOscillator::double_well();
        let fps = duffing.fixed_points();
        assert_eq!(fps.len(), 3); // Origin + two wells

        // Check well positions for α=-1, β=1: x = ±1
        let x_eq = 1.0;
        assert!((DuffingOscillator::x(&fps[1]) - x_eq).abs() < 1e-10);
        assert!((DuffingOscillator::x(&fps[2]) + x_eq).abs() < 1e-10);
    }

    #[test]
    fn test_vector_field_at_origin() {
        let duffing = DuffingOscillator::double_well();
        let origin = Multivector::zero();
        let vf = duffing.vector_field(&origin).unwrap();

        // At origin, vector field should be zero
        assert!(vf.coefficient_norm() < 1e-10);
    }

    #[test]
    fn test_vector_field_nonzero() {
        let duffing = DuffingOscillator::new(0.1, 1.0, 1.0);
        let mut state = Multivector::zero();
        state.set(1, 1.0); // x = 1
        state.set(2, 2.0); // y = 2

        let vf = duffing.vector_field(&state).unwrap();

        // dx/dt = y = 2
        assert!((vf.get(1) - 2.0).abs() < 1e-10);

        // dy/dt = -δy - αx - βx³ = -0.1*2 - 1*1 - 1*1 = -0.2 - 1 - 1 = -2.2
        assert!((vf.get(2) - (-2.2)).abs() < 1e-10);
    }

    #[test]
    fn test_jacobian_at_origin() {
        let duffing = DuffingOscillator::new(0.5, 2.0, 1.0);
        let origin = Multivector::zero();
        let jac = duffing.jacobian(&origin).unwrap();

        assert_eq!(jac.len(), 4);
        assert_eq!(jac[0], 0.0); // ∂(dx/dt)/∂x = 0
        assert_eq!(jac[1], 1.0); // ∂(dx/dt)/∂y = 1
        assert_eq!(jac[2], -2.0); // ∂(dy/dt)/∂x = -α = -2
        assert_eq!(jac[3], -0.5); // ∂(dy/dt)/∂y = -δ = -0.5
    }

    #[test]
    fn test_jacobian_nonzero() {
        let duffing = DuffingOscillator::new(0.5, 1.0, 2.0);
        let mut state = Multivector::zero();
        state.set(1, 1.0);

        let jac = duffing.jacobian(&state).unwrap();

        // ∂(dy/dt)/∂x = -α - 3βx² = -1 - 3*2*1 = -7
        assert_eq!(jac[2], -7.0);
    }

    #[test]
    fn test_energy_conservation_undamped() {
        let duffing = DuffingOscillator::undamped_double_well();
        let mut state = Multivector::zero();
        state.set(1, 0.5);
        state.set(2, 0.3);

        let e0 = duffing.total_energy(&state);

        // For undamped system, energy should be computed correctly
        let k = 0.5 * 0.3 * 0.3; // 0.045
        let v = -0.5 * 0.5 * 0.5 + 0.25 * 1.0 * 0.5 * 0.5 * 0.5 * 0.5; // -0.125 + 0.015625
        assert!((e0 - (k + v)).abs() < 1e-10);
    }

    #[test]
    fn test_barrier_height() {
        let duffing = DuffingOscillator::new(0.0, -1.0, 1.0);
        let barrier = duffing.barrier_height().unwrap();

        // For α=-1, β=1: barrier = α²/(4β) = 1/4 = 0.25
        assert!((barrier - 0.25).abs() < 1e-10);
    }

    #[test]
    fn test_barrier_height_not_double_well() {
        let duffing = DuffingOscillator::hardening_spring();
        assert!(duffing.barrier_height().is_none());
    }

    #[test]
    fn test_default_initial_condition() {
        let duffing = DuffingOscillator::double_well();
        let ic = duffing.default_initial_condition();
        assert_eq!(DuffingOscillator::x(&ic), 1.0);
        assert_eq!(DuffingOscillator::y(&ic), 0.0);
    }

    #[test]
    fn test_forced_duffing() {
        let forced = ForcedDuffing::chaotic();
        assert_eq!(forced.base.delta, 0.25);
        assert_eq!(forced.base.alpha, -1.0);
        assert_eq!(forced.base.beta, 1.0);
        assert_eq!(forced.gamma, 0.3);
        assert_eq!(forced.omega, 1.0);

        // Check forcing at t=0
        assert!((forced.forcing(0.0) - 0.3).abs() < 1e-10);

        // Check forcing period
        let expected_period = 2.0 * std::f64::consts::PI;
        assert!((forced.forcing_period() - expected_period).abs() < 1e-10);
    }

    #[test]
    fn test_forced_duffing_vector_field() {
        let forced = ForcedDuffing::new(0.1, 1.0, 1.0, 0.5, 2.0);
        let mut state = Multivector::zero();
        state.set(1, 1.0);
        state.set(2, 0.0);

        let vf = forced.vector_field_at(&state, 0.0).unwrap();

        // dx/dt = y = 0
        assert!(vf.get(1).abs() < 1e-10);

        // dy/dt = -δy - αx - βx³ + γcos(ωt) = -0 - 1 - 1 + 0.5 = -1.5
        assert!((vf.get(2) - (-1.5)).abs() < 1e-10);
    }
}
