//! Pendulum systems
//!
//! This module provides implementations of simple and double pendulum systems.
//!
//! # Simple Pendulum
//!
//! The simple pendulum is a classic example of a nonlinear oscillator.
//! For small angles it approximates simple harmonic motion.
//!
//! ## Equations
//!
//! ```text
//! dθ/dt = ω
//! dω/dt = -(g/L)sin(θ) - γω
//! ```
//!
//! # Double Pendulum
//!
//! The double pendulum is a paradigmatic example of deterministic chaos.
//! Even with simple equations, it exhibits extreme sensitivity to initial
//! conditions.
//!
//! ## Equations
//!
//! Using angles θ₁, θ₂ measured from vertical and angular velocities ω₁, ω₂:
//!
//! The equations of motion are derived from the Lagrangian and are
//! coupled nonlinear ODEs (see implementation for full expressions).
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::systems::{SimplePendulum, DoublePendulum};
//! use amari_dynamics::solver::{RungeKutta4, ODESolver};
//!
//! let simple = SimplePendulum::new(1.0, 9.81, 0.0);
//! let double = DoublePendulum::symmetric();
//! ```

use amari_core::Multivector;

use crate::error::Result;
use crate::flow::DynamicalSystem;

/// Simple pendulum
///
/// A 2D continuous dynamical system modeling a mass on a rod.
/// State is represented in Cl(2,0,0) using e1, e2 for θ, ω.
#[derive(Debug, Clone)]
pub struct SimplePendulum {
    /// Length of the pendulum (L)
    pub length: f64,
    /// Gravitational acceleration (g)
    pub gravity: f64,
    /// Damping coefficient (γ)
    pub damping: f64,
}

impl SimplePendulum {
    /// Create a new simple pendulum
    pub fn new(length: f64, gravity: f64, damping: f64) -> Self {
        Self {
            length,
            gravity,
            damping,
        }
    }

    /// Create an undamped pendulum with standard parameters
    pub fn undamped() -> Self {
        Self::new(1.0, 9.81, 0.0)
    }

    /// Create a lightly damped pendulum
    pub fn lightly_damped() -> Self {
        Self::new(1.0, 9.81, 0.1)
    }

    /// Create a heavily damped pendulum
    pub fn heavily_damped() -> Self {
        Self::new(1.0, 9.81, 1.0)
    }

    /// Natural frequency ω₀ = √(g/L)
    pub fn natural_frequency(&self) -> f64 {
        (self.gravity / self.length).sqrt()
    }

    /// Period for small oscillations T = 2π√(L/g)
    pub fn small_angle_period(&self) -> f64 {
        2.0 * std::f64::consts::PI * (self.length / self.gravity).sqrt()
    }

    /// Default initial condition (small angle displacement)
    pub fn default_initial_condition(&self) -> Multivector<2, 0, 0> {
        let mut state = Multivector::zero();
        state.set(1, 0.2); // θ = 0.2 rad
        state.set(2, 0.0); // ω = 0
        state
    }

    /// Large initial condition for full rotations
    pub fn large_initial_condition(&self) -> Multivector<2, 0, 0> {
        let mut state = Multivector::zero();
        state.set(1, 3.0); // θ = 3.0 rad (nearly inverted)
        state.set(2, 0.0); // ω = 0
        state
    }

    /// Compute total energy E = (1/2)mL²ω² + mgL(1 - cos(θ))
    ///
    /// Normalized by mL²ω₀² where ω₀ = √(g/L)
    pub fn energy(&self, state: &Multivector<2, 0, 0>) -> f64 {
        let theta = state.get(1);
        let omega = state.get(2);
        let omega0_sq = self.gravity / self.length;

        // E = (1/2)ω² + ω₀²(1 - cos(θ))
        0.5 * omega * omega + omega0_sq * (1.0 - theta.cos())
    }

    /// Check if pendulum can rotate (E > 2ω₀²)
    pub fn can_rotate(&self, state: &Multivector<2, 0, 0>) -> bool {
        let e = self.energy(state);
        let e_critical = 2.0 * self.gravity / self.length;
        e > e_critical
    }

    /// Get angle θ from state
    pub fn theta(state: &Multivector<2, 0, 0>) -> f64 {
        state.get(1)
    }

    /// Get angular velocity ω from state
    pub fn omega(state: &Multivector<2, 0, 0>) -> f64 {
        state.get(2)
    }
}

impl Default for SimplePendulum {
    fn default() -> Self {
        Self::undamped()
    }
}

impl DynamicalSystem<2, 0, 0> for SimplePendulum {
    const DIM: usize = 2;

    fn vector_field(&self, state: &Multivector<2, 0, 0>) -> Result<Multivector<2, 0, 0>> {
        let theta = state.get(1);
        let omega = state.get(2);

        let mut result = Multivector::zero();
        result.set(1, omega); // dθ/dt = ω
        result.set(
            2,
            -(self.gravity / self.length) * theta.sin() - self.damping * omega,
        ); // dω/dt = -(g/L)sin(θ) - γω

        Ok(result)
    }

    fn jacobian(&self, state: &Multivector<2, 0, 0>) -> Result<Vec<f64>> {
        let theta = state.get(1);

        // Jacobian matrix (row-major order):
        // [         0,              1     ]
        // [ -(g/L)cos(θ),         -γ     ]
        Ok(vec![
            0.0,
            1.0,
            -(self.gravity / self.length) * theta.cos(),
            -self.damping,
        ])
    }
}

/// Double pendulum
///
/// A 4D chaotic dynamical system consisting of two coupled pendulums.
/// State is represented in Cl(4,0,0) using e1, e2, e4, e8 for θ₁, θ₂, ω₁, ω₂.
///
/// The system uses normalized units where g = 1 for simplicity.
#[derive(Debug, Clone)]
pub struct DoublePendulum {
    /// Mass of first pendulum (m₁)
    pub m1: f64,
    /// Mass of second pendulum (m₂)
    pub m2: f64,
    /// Length of first pendulum (L₁)
    pub l1: f64,
    /// Length of second pendulum (L₂)
    pub l2: f64,
    /// Gravitational acceleration (g)
    pub gravity: f64,
}

impl DoublePendulum {
    /// Create a new double pendulum
    pub fn new(m1: f64, m2: f64, l1: f64, l2: f64, gravity: f64) -> Self {
        Self {
            m1,
            m2,
            l1,
            l2,
            gravity,
        }
    }

    /// Create a symmetric double pendulum (equal masses and lengths)
    pub fn symmetric() -> Self {
        Self::new(1.0, 1.0, 1.0, 1.0, 9.81)
    }

    /// Create a double pendulum with normalized gravity (g=1)
    pub fn normalized() -> Self {
        Self::new(1.0, 1.0, 1.0, 1.0, 1.0)
    }

    /// Create a heavy-light double pendulum
    pub fn heavy_light() -> Self {
        Self::new(2.0, 1.0, 1.0, 1.0, 9.81)
    }

    /// Default initial condition (both pendulums displaced)
    pub fn default_initial_condition(&self) -> Multivector<4, 0, 0> {
        let mut state = Multivector::zero();
        state.set(1, 2.0); // θ₁ = 2.0 rad
        state.set(2, 2.0); // θ₂ = 2.0 rad
        state.set(4, 0.0); // ω₁ = 0
        state.set(8, 0.0); // ω₂ = 0
        state
    }

    /// Initial condition for small oscillations
    pub fn small_oscillation_ic(&self) -> Multivector<4, 0, 0> {
        let mut state = Multivector::zero();
        state.set(1, 0.2); // θ₁ = 0.2 rad
        state.set(2, 0.3); // θ₂ = 0.3 rad
        state.set(4, 0.0); // ω₁ = 0
        state.set(8, 0.0); // ω₂ = 0
        state
    }

    /// Initial condition for large chaotic motion
    pub fn chaotic_ic(&self) -> Multivector<4, 0, 0> {
        let mut state = Multivector::zero();
        state.set(1, std::f64::consts::PI - 0.1); // θ₁ nearly inverted
        state.set(2, std::f64::consts::PI - 0.1); // θ₂ nearly inverted
        state.set(4, 0.0);
        state.set(8, 0.0);
        state
    }

    /// Total energy of the double pendulum
    ///
    /// E = T + V where T is kinetic and V is potential energy
    pub fn energy(&self, state: &Multivector<4, 0, 0>) -> f64 {
        let theta1 = state.get(1);
        let theta2 = state.get(2);
        let omega1 = state.get(4);
        let omega2 = state.get(8);

        let cos_diff = (theta1 - theta2).cos();

        // Kinetic energy
        let t = 0.5 * (self.m1 + self.m2) * self.l1 * self.l1 * omega1 * omega1
            + 0.5 * self.m2 * self.l2 * self.l2 * omega2 * omega2
            + self.m2 * self.l1 * self.l2 * omega1 * omega2 * cos_diff;

        // Potential energy (reference: both hanging down)
        let v = -(self.m1 + self.m2) * self.gravity * self.l1 * theta1.cos()
            - self.m2 * self.gravity * self.l2 * theta2.cos();

        t + v
    }

    /// Get θ₁ from state
    pub fn theta1(state: &Multivector<4, 0, 0>) -> f64 {
        state.get(1)
    }

    /// Get θ₂ from state
    pub fn theta2(state: &Multivector<4, 0, 0>) -> f64 {
        state.get(2)
    }

    /// Get ω₁ from state
    pub fn omega1(state: &Multivector<4, 0, 0>) -> f64 {
        state.get(4)
    }

    /// Get ω₂ from state
    pub fn omega2(state: &Multivector<4, 0, 0>) -> f64 {
        state.get(8)
    }

    /// Position of first mass (x₁, y₁)
    pub fn position1(&self, state: &Multivector<4, 0, 0>) -> (f64, f64) {
        let theta1 = state.get(1);
        let x1 = self.l1 * theta1.sin();
        let y1 = -self.l1 * theta1.cos();
        (x1, y1)
    }

    /// Position of second mass (x₂, y₂)
    pub fn position2(&self, state: &Multivector<4, 0, 0>) -> (f64, f64) {
        let theta1 = state.get(1);
        let theta2 = state.get(2);
        let x2 = self.l1 * theta1.sin() + self.l2 * theta2.sin();
        let y2 = -self.l1 * theta1.cos() - self.l2 * theta2.cos();
        (x2, y2)
    }
}

impl Default for DoublePendulum {
    fn default() -> Self {
        Self::symmetric()
    }
}

impl DynamicalSystem<4, 0, 0> for DoublePendulum {
    const DIM: usize = 4;

    fn vector_field(&self, state: &Multivector<4, 0, 0>) -> Result<Multivector<4, 0, 0>> {
        let theta1 = state.get(1);
        let theta2 = state.get(2);
        let omega1 = state.get(4);
        let omega2 = state.get(8);

        let m_total = self.m1 + self.m2;
        let delta = theta1 - theta2;
        let sin_delta = delta.sin();
        let cos_delta = delta.cos();

        // Common denominator for ω̇₁ and ω̇₂
        let denom = self.l1 * (m_total - self.m2 * cos_delta * cos_delta);

        // dω₁/dt
        let omega1_dot = (self.m2 * self.l1 * omega1 * omega1 * sin_delta * cos_delta
            + self.m2 * self.gravity * theta2.sin() * cos_delta
            + self.m2 * self.l2 * omega2 * omega2 * sin_delta
            - m_total * self.gravity * theta1.sin())
            / denom;

        // dω₂/dt
        let denom2 = self.l2 / self.l1 * denom;
        let omega2_dot = (-self.m2 * self.l2 * omega2 * omega2 * sin_delta * cos_delta
            - m_total * self.l1 * omega1 * omega1 * sin_delta
            - m_total * self.gravity * theta1.sin() * cos_delta
            + m_total * self.gravity * theta2.sin())
            / denom2;

        let mut result = Multivector::zero();
        result.set(1, omega1); // dθ₁/dt = ω₁
        result.set(2, omega2); // dθ₂/dt = ω₂
        result.set(4, omega1_dot); // dω₁/dt
        result.set(8, omega2_dot); // dω₂/dt

        Ok(result)
    }

    fn jacobian(&self, state: &Multivector<4, 0, 0>) -> Result<Vec<f64>> {
        // For the double pendulum, the Jacobian is complex.
        // We use numerical differentiation for robustness.
        let eps = 1e-8;
        let mut jac = vec![0.0; 16];

        let f0 = self.vector_field(state)?;

        // Indices for Cl(4,0,0): e1=1, e2=2, e4=4, e8=8
        let indices = [1, 2, 4, 8];

        for (j, &idx) in indices.iter().enumerate() {
            let mut state_plus = state.clone();
            let val = state_plus.get(idx);
            state_plus.set(idx, val + eps);

            let f_plus = self.vector_field(&state_plus)?;

            for (i, &out_idx) in indices.iter().enumerate() {
                jac[i * 4 + j] = (f_plus.get(out_idx) - f0.get(out_idx)) / eps;
            }
        }

        Ok(jac)
    }
}

/// Driven simple pendulum
///
/// Adds a periodic torque to the simple pendulum, which can produce
/// chaotic behavior for certain parameters.
///
/// # Equations
///
/// ```text
/// dθ/dt = ω
/// dω/dt = -(g/L)sin(θ) - γω + A*cos(Ωt)
/// ```
#[derive(Debug, Clone)]
pub struct DrivenPendulum {
    /// Base simple pendulum
    pub base: SimplePendulum,
    /// Driving amplitude (A)
    pub amplitude: f64,
    /// Driving frequency (Ω)
    pub drive_frequency: f64,
}

impl DrivenPendulum {
    /// Create a new driven pendulum
    pub fn new(
        length: f64,
        gravity: f64,
        damping: f64,
        amplitude: f64,
        drive_frequency: f64,
    ) -> Self {
        Self {
            base: SimplePendulum::new(length, gravity, damping),
            amplitude,
            drive_frequency,
        }
    }

    /// Create a classic chaotic driven pendulum
    pub fn chaotic() -> Self {
        // Parameters known to produce chaos
        Self::new(1.0, 9.81, 0.5, 1.2, 2.0 / 3.0 * (9.81_f64).sqrt())
    }

    /// Evaluate the driving torque at time t
    pub fn driving_torque(&self, t: f64) -> f64 {
        self.amplitude * (self.drive_frequency * t).cos()
    }

    /// Evaluate vector field at a given time (non-autonomous)
    pub fn vector_field_at(
        &self,
        state: &Multivector<2, 0, 0>,
        t: f64,
    ) -> Result<Multivector<2, 0, 0>> {
        let theta = state.get(1);
        let omega = state.get(2);

        let mut result = Multivector::zero();
        result.set(1, omega);
        result.set(
            2,
            -(self.base.gravity / self.base.length) * theta.sin() - self.base.damping * omega
                + self.driving_torque(t),
        );

        Ok(result)
    }

    /// Period of the driving force
    pub fn driving_period(&self) -> f64 {
        2.0 * std::f64::consts::PI / self.drive_frequency
    }

    /// Default initial condition
    pub fn default_initial_condition(&self) -> Multivector<2, 0, 0> {
        self.base.default_initial_condition()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Simple pendulum tests

    #[test]
    fn test_simple_pendulum_creation() {
        let p = SimplePendulum::new(2.0, 9.81, 0.1);
        assert_eq!(p.length, 2.0);
        assert_eq!(p.gravity, 9.81);
        assert_eq!(p.damping, 0.1);
    }

    #[test]
    fn test_simple_pendulum_presets() {
        let undamped = SimplePendulum::undamped();
        assert_eq!(undamped.damping, 0.0);

        let light = SimplePendulum::lightly_damped();
        assert_eq!(light.damping, 0.1);

        let heavy = SimplePendulum::heavily_damped();
        assert_eq!(heavy.damping, 1.0);
    }

    #[test]
    fn test_simple_pendulum_default() {
        let p = SimplePendulum::default();
        assert_eq!(p.damping, 0.0);
    }

    #[test]
    fn test_natural_frequency() {
        let p = SimplePendulum::new(1.0, 9.81, 0.0);
        let omega0 = p.natural_frequency();
        assert!((omega0 - 3.1321).abs() < 0.01);
    }

    #[test]
    fn test_small_angle_period() {
        let p = SimplePendulum::new(1.0, 9.81, 0.0);
        let t = p.small_angle_period();
        // T = 2π√(1/9.81) ≈ 2.006
        assert!((t - 2.006).abs() < 0.01);
    }

    #[test]
    fn test_simple_pendulum_vector_field_at_equilibrium() {
        let p = SimplePendulum::undamped();
        let origin = Multivector::zero();
        let vf = p.vector_field(&origin).unwrap();

        // At equilibrium, vector field should be zero
        assert!(vf.coefficient_norm() < 1e-10);
    }

    #[test]
    fn test_simple_pendulum_vector_field() {
        let p = SimplePendulum::new(1.0, 10.0, 0.5);
        let mut state = Multivector::zero();
        state.set(1, std::f64::consts::PI / 6.0); // θ = 30°
        state.set(2, 1.0); // ω = 1

        let vf = p.vector_field(&state).unwrap();

        // dθ/dt = ω = 1
        assert!((vf.get(1) - 1.0).abs() < 1e-10);

        // dω/dt = -10*sin(π/6) - 0.5*1 = -10*0.5 - 0.5 = -5.5
        assert!((vf.get(2) - (-5.5)).abs() < 1e-10);
    }

    #[test]
    fn test_simple_pendulum_jacobian_at_origin() {
        let p = SimplePendulum::new(1.0, 10.0, 0.5);
        let origin = Multivector::zero();
        let jac = p.jacobian(&origin).unwrap();

        assert_eq!(jac.len(), 4);
        assert_eq!(jac[0], 0.0); // ∂(dθ/dt)/∂θ = 0
        assert_eq!(jac[1], 1.0); // ∂(dθ/dt)/∂ω = 1
        assert_eq!(jac[2], -10.0); // ∂(dω/dt)/∂θ = -(g/L)cos(0) = -10
        assert_eq!(jac[3], -0.5); // ∂(dω/dt)/∂ω = -γ = -0.5
    }

    #[test]
    fn test_simple_pendulum_energy() {
        let p = SimplePendulum::new(1.0, 10.0, 0.0);
        let mut state = Multivector::zero();
        state.set(1, 0.0);
        state.set(2, 0.0);

        let e = p.energy(&state);
        // At equilibrium with zero velocity, energy = 0
        assert!(e.abs() < 1e-10);
    }

    #[test]
    fn test_can_rotate() {
        let p = SimplePendulum::new(1.0, 10.0, 0.0);

        // Low energy state cannot rotate
        let mut low_e = Multivector::zero();
        low_e.set(1, 0.5);
        low_e.set(2, 0.0);
        assert!(!p.can_rotate(&low_e));

        // High energy state can rotate
        let mut high_e = Multivector::zero();
        high_e.set(1, 0.0);
        high_e.set(2, 10.0); // High velocity
        assert!(p.can_rotate(&high_e));
    }

    // Double pendulum tests

    #[test]
    fn test_double_pendulum_creation() {
        let dp = DoublePendulum::new(1.0, 2.0, 1.5, 1.0, 9.81);
        assert_eq!(dp.m1, 1.0);
        assert_eq!(dp.m2, 2.0);
        assert_eq!(dp.l1, 1.5);
        assert_eq!(dp.l2, 1.0);
        assert_eq!(dp.gravity, 9.81);
    }

    #[test]
    fn test_double_pendulum_presets() {
        let sym = DoublePendulum::symmetric();
        assert_eq!(sym.m1, sym.m2);
        assert_eq!(sym.l1, sym.l2);

        let norm = DoublePendulum::normalized();
        assert_eq!(norm.gravity, 1.0);

        let hl = DoublePendulum::heavy_light();
        assert!(hl.m1 > hl.m2);
    }

    #[test]
    fn test_double_pendulum_default() {
        let dp = DoublePendulum::default();
        assert_eq!(dp.m1, 1.0);
        assert_eq!(dp.m2, 1.0);
    }

    #[test]
    fn test_double_pendulum_vector_field_at_equilibrium() {
        let dp = DoublePendulum::symmetric();
        let origin = Multivector::zero();
        let vf = dp.vector_field(&origin).unwrap();

        // At equilibrium (both hanging down), vector field should be zero
        assert!(vf.coefficient_norm() < 1e-10);
    }

    #[test]
    fn test_double_pendulum_positions() {
        let dp = DoublePendulum::new(1.0, 1.0, 1.0, 1.0, 9.81);
        let mut state = Multivector::zero();
        state.set(1, std::f64::consts::PI / 2.0); // θ₁ = 90°
        state.set(2, 0.0); // θ₂ = 0°

        let (x1, y1) = dp.position1(&state);
        assert!((x1 - 1.0).abs() < 1e-10); // sin(90°) = 1
        assert!(y1.abs() < 1e-10); // cos(90°) = 0

        let (x2, y2) = dp.position2(&state);
        assert!((x2 - 1.0).abs() < 1e-10); // 1 + sin(0) = 1
        assert!((y2 - (-1.0)).abs() < 1e-10); // 0 - cos(0) = -1
    }

    #[test]
    fn test_double_pendulum_energy() {
        let dp = DoublePendulum::normalized();
        let origin = Multivector::zero();
        let e = dp.energy(&origin);

        // At equilibrium, E = -(m1+m2)*g*l1 - m2*g*l2 = -2 - 1 = -3 (for normalized)
        assert!((e - (-3.0)).abs() < 1e-10);
    }

    #[test]
    fn test_double_pendulum_jacobian_shape() {
        let dp = DoublePendulum::symmetric();
        let origin = Multivector::zero();
        let jac = dp.jacobian(&origin).unwrap();

        assert_eq!(jac.len(), 16); // 4x4 matrix
    }

    // Driven pendulum tests

    #[test]
    fn test_driven_pendulum_creation() {
        let dp = DrivenPendulum::new(1.0, 9.81, 0.5, 1.0, 2.0);
        assert_eq!(dp.base.length, 1.0);
        assert_eq!(dp.amplitude, 1.0);
        assert_eq!(dp.drive_frequency, 2.0);
    }

    #[test]
    fn test_driven_pendulum_chaotic() {
        let dp = DrivenPendulum::chaotic();
        assert!(dp.amplitude > 0.0);
        assert!(dp.drive_frequency > 0.0);
    }

    #[test]
    fn test_driving_torque() {
        let dp = DrivenPendulum::new(1.0, 9.81, 0.0, 1.0, 1.0);

        // At t=0, driving = A*cos(0) = A = 1
        assert!((dp.driving_torque(0.0) - 1.0).abs() < 1e-10);

        // At t=π/2, driving = A*cos(π/2) = 0
        assert!(dp.driving_torque(std::f64::consts::PI / 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_driven_pendulum_vector_field() {
        let dp = DrivenPendulum::new(1.0, 10.0, 0.5, 2.0, 1.0);
        let origin = Multivector::zero();
        let vf = dp.vector_field_at(&origin, 0.0).unwrap();

        // At origin with t=0: dω/dt = -10*sin(0) - 0.5*0 + 2*cos(0) = 2
        assert!((vf.get(2) - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_driving_period() {
        let dp = DrivenPendulum::new(1.0, 9.81, 0.0, 1.0, 2.0);
        let period = dp.driving_period();
        assert!((period - std::f64::consts::PI).abs() < 1e-10);
    }
}
