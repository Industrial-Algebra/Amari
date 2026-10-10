//! Attractor traits and types
//!
//! This module defines the core abstractions for attractors in dynamical systems.
//! An attractor is a set toward which a system evolves over time.
//!
//! # Types of Attractors
//!
//! - **Fixed Point**: A single stable equilibrium point
//! - **Limit Cycle**: A closed periodic orbit
//! - **Torus**: Quasiperiodic motion on a torus surface
//! - **Strange Attractor**: Fractal structure with sensitive dependence on initial conditions
//!
//! # Geometric Algebra Context
//!
//! In Clifford algebras, attractors can have rich geometric structure:
//! - Rotor attractors for orientation dynamics
//! - Bivector-valued limit cycles representing rotational motion
//! - Multivector basins with grade-specific attraction properties

use std::fmt;

use amari_core::Multivector;

use crate::solver::Trajectory;

/// Classification of attractor types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AttractorType {
    /// Fixed point (stable equilibrium)
    ///
    /// All nearby trajectories converge to this single point.
    FixedPoint,

    /// Limit cycle (periodic attractor)
    ///
    /// Trajectories converge to a closed orbit with period T.
    LimitCycle,

    /// Torus (quasiperiodic attractor)
    ///
    /// Motion on a torus surface with incommensurate frequencies.
    /// Trajectories densely fill the torus but never exactly repeat.
    Torus,

    /// Strange attractor (chaotic)
    ///
    /// Fractal structure with positive Lyapunov exponent.
    /// Nearby trajectories diverge exponentially while remaining bounded.
    Strange,

    /// Unknown or unclassified attractor type
    Unknown,
}

impl fmt::Display for AttractorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FixedPoint => write!(f, "Fixed Point"),
            Self::LimitCycle => write!(f, "Limit Cycle"),
            Self::Torus => write!(f, "Torus"),
            Self::Strange => write!(f, "Strange Attractor"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

impl AttractorType {
    /// Check if this attractor type is regular (non-chaotic)
    pub fn is_regular(&self) -> bool {
        matches!(self, Self::FixedPoint | Self::LimitCycle | Self::Torus)
    }

    /// Check if this attractor type is chaotic
    pub fn is_chaotic(&self) -> bool {
        matches!(self, Self::Strange)
    }

    /// Get the topological dimension of the attractor
    ///
    /// - Fixed point: 0
    /// - Limit cycle: 1
    /// - Torus: 2
    /// - Strange: fractal (returns None)
    pub fn topological_dimension(&self) -> Option<usize> {
        match self {
            Self::FixedPoint => Some(0),
            Self::LimitCycle => Some(1),
            Self::Torus => Some(2),
            Self::Strange | Self::Unknown => None,
        }
    }
}

/// Trait for objects representing attractors
///
/// An attractor captures the long-term behavior of a dynamical system,
/// including its type, location, and basin of attraction.
pub trait Attractor<const P: usize, const Q: usize, const R: usize>: Clone {
    /// Get the type of this attractor
    fn attractor_type(&self) -> AttractorType;

    /// Get a representative point on the attractor
    ///
    /// For fixed points, this is the equilibrium.
    /// For limit cycles, this is a point on the orbit.
    fn representative_point(&self) -> &Multivector<P, Q, R>;

    /// Check if a state is on or near this attractor
    ///
    /// Returns true if the state is within tolerance of the attractor.
    fn contains(&self, state: &Multivector<P, Q, R>, tolerance: f64) -> bool;

    /// Compute the distance from a state to this attractor
    fn distance(&self, state: &Multivector<P, Q, R>) -> f64;

    /// Get the basin of attraction (if computed)
    fn basin(&self) -> Option<&Basin<P, Q, R>>;
}

/// A fixed point attractor
#[derive(Debug, Clone)]
pub struct FixedPointAttractor<const P: usize, const Q: usize, const R: usize> {
    /// The equilibrium point
    pub point: Multivector<P, Q, R>,

    /// Eigenvalues of the Jacobian at the fixed point
    pub eigenvalues: Vec<(f64, f64)>,

    /// Basin of attraction (if computed)
    pub basin: Option<Basin<P, Q, R>>,
}

impl<const P: usize, const Q: usize, const R: usize> FixedPointAttractor<P, Q, R> {
    /// Create a new fixed point attractor
    pub fn new(point: Multivector<P, Q, R>, eigenvalues: Vec<(f64, f64)>) -> Self {
        Self {
            point,
            eigenvalues,
            basin: None,
        }
    }

    /// Check if the fixed point is stable
    pub fn is_stable(&self) -> bool {
        self.eigenvalues.iter().all(|(re, _)| *re < 0.0)
    }

    /// Get the decay rate (largest real part of eigenvalues)
    pub fn decay_rate(&self) -> f64 {
        self.eigenvalues
            .iter()
            .map(|(re, _)| *re)
            .fold(f64::NEG_INFINITY, f64::max)
    }
}

impl<const P: usize, const Q: usize, const R: usize> Attractor<P, Q, R>
    for FixedPointAttractor<P, Q, R>
{
    fn attractor_type(&self) -> AttractorType {
        AttractorType::FixedPoint
    }

    fn representative_point(&self) -> &Multivector<P, Q, R> {
        &self.point
    }

    fn contains(&self, state: &Multivector<P, Q, R>, tolerance: f64) -> bool {
        self.distance(state) < tolerance
    }

    fn distance(&self, state: &Multivector<P, Q, R>) -> f64 {
        let diff = state - &self.point;
        diff.coefficient_norm()
    }

    fn basin(&self) -> Option<&Basin<P, Q, R>> {
        self.basin.as_ref()
    }
}

/// A limit cycle attractor
#[derive(Debug, Clone)]
pub struct LimitCycleAttractor<const P: usize, const Q: usize, const R: usize> {
    /// Points sampled along the limit cycle
    pub orbit_points: Vec<Multivector<P, Q, R>>,

    /// Period of the limit cycle
    pub period: f64,

    /// Floquet multipliers (stability of the cycle)
    pub floquet_multipliers: Vec<(f64, f64)>,

    /// Basin of attraction (if computed)
    pub basin: Option<Basin<P, Q, R>>,
}

impl<const P: usize, const Q: usize, const R: usize> LimitCycleAttractor<P, Q, R> {
    /// Create a new limit cycle attractor
    pub fn new(orbit_points: Vec<Multivector<P, Q, R>>, period: f64) -> Self {
        Self {
            orbit_points,
            period,
            floquet_multipliers: Vec::new(),
            basin: None,
        }
    }

    /// Create with Floquet multipliers
    pub fn with_floquet(
        orbit_points: Vec<Multivector<P, Q, R>>,
        period: f64,
        floquet_multipliers: Vec<(f64, f64)>,
    ) -> Self {
        Self {
            orbit_points,
            period,
            floquet_multipliers,
            basin: None,
        }
    }

    /// Check if the limit cycle is stable
    ///
    /// A limit cycle is stable if all Floquet multipliers have magnitude < 1.
    pub fn is_stable(&self) -> bool {
        self.floquet_multipliers.iter().all(|(re, im)| {
            let magnitude = (re * re + im * im).sqrt();
            magnitude < 1.0
        })
    }

    /// Get the approximate amplitude of the limit cycle
    pub fn amplitude(&self) -> f64 {
        if self.orbit_points.is_empty() {
            return 0.0;
        }

        // Compute center of mass
        let n = self.orbit_points.len() as f64;
        let dim = 1 << (P + Q + R);
        let mut center = Multivector::<P, Q, R>::zero();

        for point in &self.orbit_points {
            for i in 0..dim {
                center.set(i, center.get(i) + point.get(i) / n);
            }
        }

        // Compute average distance from center
        let mut avg_dist = 0.0;
        for point in &self.orbit_points {
            let diff = point - &center;
            avg_dist += diff.coefficient_norm();
        }
        avg_dist / n
    }

    /// Get the frequency of the limit cycle
    pub fn frequency(&self) -> f64 {
        if self.period > 0.0 {
            1.0 / self.period
        } else {
            0.0
        }
    }

    /// Get the angular frequency (2π/T)
    pub fn angular_frequency(&self) -> f64 {
        if self.period > 0.0 {
            2.0 * std::f64::consts::PI / self.period
        } else {
            0.0
        }
    }
}

impl<const P: usize, const Q: usize, const R: usize> Attractor<P, Q, R>
    for LimitCycleAttractor<P, Q, R>
{
    fn attractor_type(&self) -> AttractorType {
        AttractorType::LimitCycle
    }

    fn representative_point(&self) -> &Multivector<P, Q, R> {
        // Safety: LimitCycleAttractor should always have at least one point
        &self.orbit_points[0]
    }

    fn contains(&self, state: &Multivector<P, Q, R>, tolerance: f64) -> bool {
        self.distance(state) < tolerance
    }

    fn distance(&self, state: &Multivector<P, Q, R>) -> f64 {
        // Distance to nearest point on the sampled orbit
        self.orbit_points
            .iter()
            .map(|p| {
                let diff = state - p;
                diff.coefficient_norm()
            })
            .fold(f64::INFINITY, f64::min)
    }

    fn basin(&self) -> Option<&Basin<P, Q, R>> {
        self.basin.as_ref()
    }
}

/// A strange (chaotic) attractor
#[derive(Debug, Clone)]
pub struct StrangeAttractor<const P: usize, const Q: usize, const R: usize> {
    /// Sample trajectory on the attractor
    pub sample_trajectory: Vec<Multivector<P, Q, R>>,

    /// Lyapunov exponents characterizing the chaos
    pub lyapunov_exponents: Vec<f64>,

    /// Estimated fractal dimension (e.g., correlation dimension)
    pub fractal_dimension: Option<f64>,

    /// Basin of attraction (if computed)
    pub basin: Option<Basin<P, Q, R>>,
}

impl<const P: usize, const Q: usize, const R: usize> StrangeAttractor<P, Q, R> {
    /// Create a new strange attractor from a sample trajectory
    pub fn new(sample_trajectory: Vec<Multivector<P, Q, R>>, lyapunov_exponents: Vec<f64>) -> Self {
        Self {
            sample_trajectory,
            lyapunov_exponents,
            fractal_dimension: None,
            basin: None,
        }
    }

    /// Create with fractal dimension
    pub fn with_dimension(
        sample_trajectory: Vec<Multivector<P, Q, R>>,
        lyapunov_exponents: Vec<f64>,
        fractal_dimension: f64,
    ) -> Self {
        Self {
            sample_trajectory,
            lyapunov_exponents,
            fractal_dimension: Some(fractal_dimension),
            basin: None,
        }
    }

    /// Get the largest Lyapunov exponent
    pub fn largest_lyapunov_exponent(&self) -> Option<f64> {
        self.lyapunov_exponents.first().copied()
    }

    /// Check if this is truly chaotic (positive largest Lyapunov exponent)
    pub fn is_chaotic(&self) -> bool {
        self.largest_lyapunov_exponent()
            .map(|l| l > 0.0)
            .unwrap_or(false)
    }

    /// Get the Kaplan-Yorke dimension estimate
    ///
    /// D_KY = k + Σ_{i=1}^k λ_i / |λ_{k+1}|
    /// where k is the largest index such that Σ_{i=1}^k λ_i >= 0
    pub fn kaplan_yorke_dimension(&self) -> Option<f64> {
        if self.lyapunov_exponents.is_empty() {
            return None;
        }

        let mut sum = 0.0;
        let mut k = 0;

        for (i, &lambda) in self.lyapunov_exponents.iter().enumerate() {
            sum += lambda;
            if sum >= 0.0 {
                k = i + 1;
            } else {
                break;
            }
        }

        if k == 0 || k >= self.lyapunov_exponents.len() {
            return None;
        }

        let sum_k: f64 = self.lyapunov_exponents[..k].iter().sum();
        let lambda_k1 = self.lyapunov_exponents[k].abs();

        if lambda_k1 > 0.0 {
            Some(k as f64 + sum_k / lambda_k1)
        } else {
            None
        }
    }
}

impl<const P: usize, const Q: usize, const R: usize> Attractor<P, Q, R>
    for StrangeAttractor<P, Q, R>
{
    fn attractor_type(&self) -> AttractorType {
        AttractorType::Strange
    }

    fn representative_point(&self) -> &Multivector<P, Q, R> {
        // Safety: StrangeAttractor should always have at least one sample point
        &self.sample_trajectory[0]
    }

    fn contains(&self, state: &Multivector<P, Q, R>, tolerance: f64) -> bool {
        self.distance(state) < tolerance
    }

    fn distance(&self, state: &Multivector<P, Q, R>) -> f64 {
        // Distance to nearest point on the sampled trajectory
        self.sample_trajectory
            .iter()
            .map(|p| {
                let diff = state - p;
                diff.coefficient_norm()
            })
            .fold(f64::INFINITY, f64::min)
    }

    fn basin(&self) -> Option<&Basin<P, Q, R>> {
        self.basin.as_ref()
    }
}

/// Basin of attraction representation
///
/// The basin of attraction is the set of initial conditions that
/// converge to a particular attractor.
#[derive(Debug, Clone)]
pub struct Basin<const P: usize, const Q: usize, const R: usize> {
    /// Grid points that belong to this basin
    pub points: Vec<Multivector<P, Q, R>>,

    /// Bounding box (min, max for each dimension)
    pub bounds: Vec<(f64, f64)>,

    /// Estimated volume of the basin
    pub volume: f64,
}

impl<const P: usize, const Q: usize, const R: usize> Basin<P, Q, R> {
    /// Create an empty basin
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            bounds: Vec::new(),
            volume: 0.0,
        }
    }

    /// Create from a set of points
    pub fn from_points(points: Vec<Multivector<P, Q, R>>) -> Self {
        let dim = 1 << (P + Q + R);
        let mut bounds = vec![(f64::INFINITY, f64::NEG_INFINITY); dim];

        for point in &points {
            for (i, bound) in bounds.iter_mut().enumerate().take(dim) {
                let val = point.get(i);
                if val < bound.0 {
                    bound.0 = val;
                }
                if val > bound.1 {
                    bound.1 = val;
                }
            }
        }

        // Estimate volume from bounding box
        let volume = bounds
            .iter()
            .map(|(min, max)| (max - min).max(0.0))
            .product();

        Self {
            points,
            bounds,
            volume,
        }
    }

    /// Check if a point is in the basin
    pub fn contains(&self, state: &Multivector<P, Q, R>, tolerance: f64) -> bool {
        self.points.iter().any(|p| {
            let diff = state - p;
            diff.coefficient_norm() < tolerance
        })
    }

    /// Get the number of sample points in this basin
    pub fn num_points(&self) -> usize {
        self.points.len()
    }
}

impl<const P: usize, const Q: usize, const R: usize> Default for Basin<P, Q, R> {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of attractor detection
#[derive(Debug, Clone)]
pub struct AttractorDetectionResult<const P: usize, const Q: usize, const R: usize> {
    /// The type of attractor detected
    pub attractor_type: AttractorType,

    /// Representative state on the attractor
    pub representative_state: Multivector<P, Q, R>,

    /// The trajectory used for detection
    pub trajectory: Trajectory<P, Q, R>,

    /// Confidence in the classification (0 to 1)
    pub confidence: f64,

    /// Additional metrics depending on attractor type
    pub metrics: AttractorMetrics,
}

/// Metrics for characterizing attractors
#[derive(Debug, Clone, Default)]
pub struct AttractorMetrics {
    /// Period (for limit cycles)
    pub period: Option<f64>,

    /// Amplitude (for limit cycles)
    pub amplitude: Option<f64>,

    /// Largest Lyapunov exponent
    pub largest_lyapunov: Option<f64>,

    /// Fractal dimension estimate
    pub dimension: Option<f64>,

    /// Average return time to Poincaré section
    pub return_time: Option<f64>,
}

/// Configuration for attractor detection
#[derive(Debug, Clone)]
pub struct AttractorConfig {
    /// Time to integrate for transient decay
    pub transient_time: f64,

    /// Time to integrate for attractor sampling
    pub sample_time: f64,

    /// Integration time step
    pub dt: f64,

    /// Tolerance for detecting periodicity
    pub period_tolerance: f64,

    /// Tolerance for fixed point detection
    pub fixed_point_tolerance: f64,

    /// Whether to compute Lyapunov exponents
    pub compute_lyapunov: bool,

    /// Whether to compute basin of attraction
    pub compute_basin: bool,
}

impl Default for AttractorConfig {
    fn default() -> Self {
        Self {
            transient_time: 100.0,
            sample_time: 100.0,
            dt: 0.01,
            period_tolerance: 1e-4,
            fixed_point_tolerance: 1e-8,
            compute_lyapunov: true,
            compute_basin: false,
        }
    }
}

impl AttractorConfig {
    /// Create configuration for fast detection
    pub fn fast() -> Self {
        Self {
            transient_time: 50.0,
            sample_time: 50.0,
            dt: 0.02,
            period_tolerance: 1e-3,
            fixed_point_tolerance: 1e-6,
            compute_lyapunov: false,
            compute_basin: false,
        }
    }

    /// Create configuration for thorough analysis
    pub fn thorough() -> Self {
        Self {
            transient_time: 500.0,
            sample_time: 500.0,
            dt: 0.001,
            period_tolerance: 1e-6,
            fixed_point_tolerance: 1e-10,
            compute_lyapunov: true,
            compute_basin: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_attractor_type_display() {
        assert_eq!(format!("{}", AttractorType::FixedPoint), "Fixed Point");
        assert_eq!(format!("{}", AttractorType::LimitCycle), "Limit Cycle");
        assert_eq!(format!("{}", AttractorType::Strange), "Strange Attractor");
    }

    #[test]
    fn test_attractor_type_properties() {
        assert!(AttractorType::FixedPoint.is_regular());
        assert!(AttractorType::LimitCycle.is_regular());
        assert!(!AttractorType::Strange.is_regular());

        assert!(AttractorType::Strange.is_chaotic());
        assert!(!AttractorType::FixedPoint.is_chaotic());
    }

    #[test]
    fn test_topological_dimension() {
        assert_eq!(AttractorType::FixedPoint.topological_dimension(), Some(0));
        assert_eq!(AttractorType::LimitCycle.topological_dimension(), Some(1));
        assert_eq!(AttractorType::Torus.topological_dimension(), Some(2));
        assert_eq!(AttractorType::Strange.topological_dimension(), None);
    }

    #[test]
    fn test_fixed_point_attractor() {
        let point = Multivector::<2, 0, 0>::zero();
        let eigenvalues = vec![(-1.0, 0.0), (-2.0, 0.0)];

        let attractor = FixedPointAttractor::new(point, eigenvalues);

        assert!(attractor.is_stable());
        assert_eq!(attractor.attractor_type(), AttractorType::FixedPoint);
        assert!(attractor.decay_rate() < 0.0);
    }

    #[test]
    fn test_limit_cycle_attractor() {
        // Create a simple circular orbit
        let mut orbit_points = Vec::new();
        for i in 0..10 {
            let theta = 2.0 * std::f64::consts::PI * (i as f64) / 10.0;
            let mut point = Multivector::<2, 0, 0>::zero();
            point.set(1, theta.cos());
            point.set(2, theta.sin());
            orbit_points.push(point);
        }

        let attractor = LimitCycleAttractor::new(orbit_points, 2.0 * std::f64::consts::PI);

        assert_eq!(attractor.attractor_type(), AttractorType::LimitCycle);
        assert!((attractor.amplitude() - 1.0).abs() < 0.1);
        assert!((attractor.frequency() - 1.0 / (2.0 * std::f64::consts::PI)).abs() < 1e-10);
    }

    #[test]
    fn test_strange_attractor() {
        let trajectory = vec![Multivector::<3, 0, 0>::zero()];
        let lyapunov = vec![0.9, 0.0, -14.5]; // Lorenz-like

        let attractor = StrangeAttractor::new(trajectory, lyapunov);

        assert!(attractor.is_chaotic());
        assert_eq!(attractor.largest_lyapunov_exponent(), Some(0.9));

        // Kaplan-Yorke dimension: 2 + (0.9 + 0.0) / 14.5 ≈ 2.062
        let d_ky = attractor.kaplan_yorke_dimension().unwrap();
        assert!((d_ky - 2.062).abs() < 0.01);
    }

    #[test]
    fn test_basin_from_points() {
        let mut points = Vec::new();
        for i in 0..3 {
            for j in 0..3 {
                let mut p = Multivector::<2, 0, 0>::zero();
                p.set(0, i as f64);
                p.set(1, j as f64);
                points.push(p);
            }
        }

        let basin = Basin::from_points(points);
        assert_eq!(basin.num_points(), 9);
        // Volume may be 0 if not all dimensions are used
        assert!(basin.volume >= 0.0);
    }

    #[test]
    fn test_attractor_config() {
        let default = AttractorConfig::default();
        let fast = AttractorConfig::fast();
        let thorough = AttractorConfig::thorough();

        assert!(fast.transient_time < default.transient_time);
        assert!(thorough.transient_time > default.transient_time);
        assert!(thorough.compute_basin);
    }
}
