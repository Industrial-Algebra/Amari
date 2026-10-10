//! Attractor analysis for dynamical systems
//!
//! This module provides tools for detecting, classifying, and analyzing
//! attractors in dynamical systems on geometric algebra spaces.
//!
//! # Overview
//!
//! An attractor is a set toward which a dynamical system evolves over time.
//! This module supports several types of attractors:
//!
//! - **Fixed Points**: Stable equilibria (dimension 0)
//! - **Limit Cycles**: Periodic orbits (dimension 1)
//! - **Tori**: Quasiperiodic motion (dimension 2)
//! - **Strange Attractors**: Chaotic, fractal sets (fractal dimension)
//!
//! # Poincaré Sections
//!
//! The primary tool for analyzing periodic and chaotic motion is the
//! Poincaré section - a lower-dimensional slice through phase space
//! where trajectory crossings are recorded.
//!
//! ```ignore
//! use amari_dynamics::attractor::{PoincareSection, collect_crossings};
//!
//! let section = PoincareSection::origin(0); // x₀ = 0 plane
//! let result = collect_crossings(&trajectory, &section);
//!
//! if result.is_periodic(0.01) {
//!     println!("Period: {:.4}", result.estimated_period().unwrap());
//! }
//! ```
//!
//! # Basin of Attraction
//!
//! For systems with multiple attractors, the basin of attraction
//! determines which initial conditions lead to which attractor.
//!
//! ```ignore
//! use amari_dynamics::attractor::{compute_basins, BasinConfig};
//!
//! let config = BasinConfig::two_dimensional((-2.0, 2.0), (-2.0, 2.0), 100, 100);
//! let basins = compute_basins(&system, &attractors, &config)?;
//!
//! for (idx, fraction) in basins.fractions() {
//!     println!("Attractor {}: {:.1}% of phase space", idx, fraction * 100.0);
//! }
//! ```
//!
//! # Geometric Algebra Context
//!
//! In Clifford algebra spaces, attractors can have rich structure:
//!
//! - Rotor attractors for stable orientations
//! - Bivector-valued limit cycles for rotational motion
//! - Multivector basins with grade-specific properties

mod basin;
mod limit_cycle;
mod traits;

// Re-export main types
#[cfg(feature = "parallel")]
pub use basin::compute_basins_parallel;
pub use basin::{
    compute_basins, generate_grid_points, refine_boundary, BasinConfig, BasinResult,
    MultiBasinResult,
};

pub use limit_cycle::{
    collect_crossings, compute_floquet_multipliers, detect_limit_cycle, detect_period,
    PoincareCrossing, PoincareResult, PoincareSection,
};

pub use traits::{
    Attractor, AttractorConfig, AttractorDetectionResult, AttractorMetrics, AttractorType, Basin,
    FixedPointAttractor, LimitCycleAttractor, StrangeAttractor,
};

/// Detect the type of attractor from a trajectory
///
/// Analyzes the asymptotic behavior to classify the attractor.
pub fn detect_attractor_type<const P: usize, const Q: usize, const R: usize>(
    trajectory: &crate::solver::Trajectory<P, Q, R>,
    config: &AttractorConfig,
) -> AttractorType {
    let states = &trajectory.states;

    if states.len() < 10 {
        return AttractorType::Unknown;
    }

    // Check for fixed point (velocity near zero)
    let last_states: Vec<_> = states.iter().rev().take(10).collect();
    let mut is_stationary = true;

    for window in last_states.windows(2) {
        let diff = window[0] - window[1];
        if diff.coefficient_norm() > config.fixed_point_tolerance {
            is_stationary = false;
            break;
        }
    }

    if is_stationary {
        return AttractorType::FixedPoint;
    }

    // Check for periodicity using autocorrelation
    let section = PoincareSection::origin(0);
    let poincare = collect_crossings(trajectory, &section);

    if poincare.is_periodic(config.period_tolerance) {
        return AttractorType::LimitCycle;
    }

    // If not periodic and not fixed, might be chaotic or quasiperiodic
    // Would need Lyapunov exponents for definitive classification
    AttractorType::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use amari_core::Multivector;

    #[test]
    fn test_attractor_type_classification() {
        assert!(AttractorType::FixedPoint.is_regular());
        assert!(!AttractorType::Strange.is_regular());
        assert!(AttractorType::Strange.is_chaotic());
    }

    #[test]
    fn test_fixed_point_attractor_creation() {
        let point = Multivector::<2, 0, 0>::zero();
        let eigenvalues = vec![(-1.0, 0.0), (-0.5, 0.5)];

        let attractor = FixedPointAttractor::new(point, eigenvalues);

        assert!(attractor.is_stable());
        assert_eq!(attractor.attractor_type(), AttractorType::FixedPoint);
    }

    #[test]
    fn test_limit_cycle_attractor() {
        let mut points = Vec::new();
        for i in 0..20 {
            let theta = 2.0 * std::f64::consts::PI * (i as f64) / 20.0;
            let mut p = Multivector::<2, 0, 0>::zero();
            p.set(0, theta.cos());
            p.set(1, theta.sin());
            points.push(p);
        }

        let cycle = LimitCycleAttractor::new(points, 2.0 * std::f64::consts::PI);

        assert_eq!(cycle.attractor_type(), AttractorType::LimitCycle);
        assert!((cycle.amplitude() - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_strange_attractor_lyapunov() {
        let trajectory = vec![Multivector::<3, 0, 0>::zero()];
        let lyapunov = vec![0.9, 0.0, -14.5];

        let attractor = StrangeAttractor::new(trajectory, lyapunov);

        assert!(attractor.is_chaotic());
        assert_eq!(attractor.largest_lyapunov_exponent(), Some(0.9));

        // Kaplan-Yorke dimension
        let d_ky = attractor.kaplan_yorke_dimension().unwrap();
        assert!(d_ky > 2.0 && d_ky < 2.1);
    }

    #[test]
    fn test_poincare_section() {
        let section = PoincareSection::hyperplane(0, 0.0, true);

        let mut before = Multivector::<2, 0, 0>::zero();
        before.set(0, -1.0);

        let mut after = Multivector::<2, 0, 0>::zero();
        after.set(0, 1.0);

        assert!(section.check_crossing(&before, &after));
    }

    #[test]
    fn test_basin_config() {
        let config = BasinConfig::two_dimensional((-1.0, 1.0), (-1.0, 1.0), 10, 10);
        assert_eq!(config.num_grid_points(), 100);
    }
}
