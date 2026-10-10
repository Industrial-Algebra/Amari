//! Trajectory types with metadata
//!
//! This module provides enhanced trajectory types that include metadata
//! and analysis capabilities beyond the basic solver trajectory.
//!
//! # Overview
//!
//! While the solver module provides basic trajectory storage, this module
//! adds:
//!
//! - Trajectory classification (periodic, chaotic, convergent, etc.)
//! - Bundle operations for multiple trajectories
//! - Interpolation and resampling
//! - Statistical analysis across trajectory ensembles
//!
//! # Example
//!
//! ```ignore
//! use amari_dynamics::phase::{TrajectoryBundle, AnalyzedTrajectory};
//!
//! let bundle = TrajectoryBundle::from_trajectories(trajectories);
//! let stats = bundle.compute_statistics();
//! println!("Mean trajectory length: {}", stats.mean_length);
//! ```

use amari_core::Multivector;

use crate::solver::Trajectory;

/// Classification of trajectory behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrajectoryType {
    /// Trajectory converges to a fixed point
    ConvergentToFixed,

    /// Trajectory converges to a limit cycle
    ConvergentToPeriodicOrbit,

    /// Trajectory exhibits quasi-periodic motion (torus)
    QuasiPeriodic,

    /// Trajectory exhibits chaotic behavior
    Chaotic,

    /// Trajectory escapes to infinity
    Divergent,

    /// Trajectory remains bounded but behavior unclear
    Bounded,

    /// Not yet classified
    Unknown,
}

/// Metadata for an analyzed trajectory
#[derive(Debug, Clone)]
pub struct TrajectoryMetadata {
    /// Classification of trajectory behavior
    pub trajectory_type: TrajectoryType,

    /// Total arc length of the trajectory
    pub arc_length: f64,

    /// Minimum distance from origin
    pub min_distance: f64,

    /// Maximum distance from origin
    pub max_distance: f64,

    /// Mean distance from origin
    pub mean_distance: f64,

    /// Whether trajectory remained in a bounded region
    pub is_bounded: bool,

    /// Estimated period (if periodic)
    pub period: Option<f64>,

    /// Return time to initial region (if applicable)
    pub return_time: Option<f64>,
}

impl Default for TrajectoryMetadata {
    fn default() -> Self {
        Self {
            trajectory_type: TrajectoryType::Unknown,
            arc_length: 0.0,
            min_distance: 0.0,
            max_distance: 0.0,
            mean_distance: 0.0,
            is_bounded: true,
            period: None,
            return_time: None,
        }
    }
}

/// A trajectory with associated metadata and analysis
#[derive(Debug, Clone)]
pub struct AnalyzedTrajectory<const P: usize, const Q: usize, const R: usize> {
    /// The underlying trajectory data
    pub trajectory: Trajectory<P, Q, R>,

    /// Computed metadata
    pub metadata: TrajectoryMetadata,
}

impl<const P: usize, const Q: usize, const R: usize> AnalyzedTrajectory<P, Q, R> {
    /// Create an analyzed trajectory from a basic trajectory
    pub fn from_trajectory(trajectory: Trajectory<P, Q, R>) -> Self {
        let metadata = compute_trajectory_metadata(&trajectory);
        Self {
            trajectory,
            metadata,
        }
    }

    /// Get the initial state
    pub fn initial_state(&self) -> Option<&Multivector<P, Q, R>> {
        self.trajectory.states.first()
    }

    /// Get the final state
    pub fn final_state(&self) -> Option<&Multivector<P, Q, R>> {
        self.trajectory.states.last()
    }

    /// Get total duration
    pub fn duration(&self) -> f64 {
        if self.trajectory.times.len() >= 2 {
            self.trajectory.times.last().unwrap() - self.trajectory.times.first().unwrap()
        } else {
            0.0
        }
    }

    /// Number of points in trajectory
    pub fn len(&self) -> usize {
        self.trajectory.states.len()
    }

    /// Check if trajectory is empty
    pub fn is_empty(&self) -> bool {
        self.trajectory.states.is_empty()
    }
}

/// Compute metadata for a trajectory
fn compute_trajectory_metadata<const P: usize, const Q: usize, const R: usize>(
    trajectory: &Trajectory<P, Q, R>,
) -> TrajectoryMetadata {
    if trajectory.states.is_empty() {
        return TrajectoryMetadata::default();
    }

    let states = &trajectory.states;

    // Compute arc length and distance statistics
    let mut arc_length = 0.0;
    let mut min_dist = f64::MAX;
    let mut max_dist = f64::MIN;
    let mut sum_dist = 0.0;

    for (i, state) in states.iter().enumerate() {
        let dist = state.coefficient_norm();
        min_dist = min_dist.min(dist);
        max_dist = max_dist.max(dist);
        sum_dist += dist;

        if i > 0 {
            let prev = &states[i - 1];
            let mut segment_len_sq = 0.0;
            // Compute distance between consecutive states
            for k in 0..(1 << (P + Q + R)) {
                let diff = state.get(k) - prev.get(k);
                segment_len_sq += diff * diff;
            }
            arc_length += segment_len_sq.sqrt();
        }
    }

    let mean_distance = sum_dist / states.len() as f64;
    let is_bounded = max_dist < 1e6; // Heuristic threshold

    // Try to detect periodicity
    let period = detect_period(trajectory);
    let trajectory_type = classify_trajectory(trajectory, is_bounded, period.is_some());

    TrajectoryMetadata {
        trajectory_type,
        arc_length,
        min_distance: min_dist,
        max_distance: max_dist,
        mean_distance,
        is_bounded,
        period,
        return_time: period, // For periodic orbits, return time equals period
    }
}

/// Detect period in trajectory (if periodic)
fn detect_period<const P: usize, const Q: usize, const R: usize>(
    trajectory: &Trajectory<P, Q, R>,
) -> Option<f64> {
    let states = &trajectory.states;
    let times = &trajectory.times;

    if states.len() < 10 {
        return None;
    }

    // Look for returns to initial region (last half of trajectory)
    let initial = &states[states.len() / 2];
    let initial_time = times[times.len() / 2];
    let tolerance = 0.1; // Relative tolerance

    let initial_norm = initial.coefficient_norm();
    if initial_norm < 1e-10 {
        return None;
    }

    for (i, state) in states.iter().enumerate().skip(states.len() / 2 + 10) {
        // Compute relative distance
        let mut dist_sq = 0.0;
        for k in 0..(1 << (P + Q + R)) {
            let diff = state.get(k) - initial.get(k);
            dist_sq += diff * diff;
        }
        let rel_dist = dist_sq.sqrt() / initial_norm;

        if rel_dist < tolerance {
            return Some(times[i] - initial_time);
        }
    }

    None
}

/// Classify trajectory behavior
fn classify_trajectory<const P: usize, const Q: usize, const R: usize>(
    trajectory: &Trajectory<P, Q, R>,
    is_bounded: bool,
    is_periodic: bool,
) -> TrajectoryType {
    if !is_bounded {
        return TrajectoryType::Divergent;
    }

    if is_periodic {
        return TrajectoryType::ConvergentToPeriodicOrbit;
    }

    // Check if converging to fixed point
    let states = &trajectory.states;
    if states.len() >= 10 {
        let last = &states[states.len() - 1];
        let prev = &states[states.len() - 10];

        let mut dist_sq = 0.0;
        for k in 0..(1 << (P + Q + R)) {
            let diff = last.get(k) - prev.get(k);
            dist_sq += diff * diff;
        }

        if dist_sq.sqrt() < 1e-6 {
            return TrajectoryType::ConvergentToFixed;
        }
    }

    TrajectoryType::Bounded
}

/// A collection of trajectories with ensemble analysis
#[derive(Debug, Clone)]
pub struct TrajectoryBundle<const P: usize, const Q: usize, const R: usize> {
    /// The trajectories in this bundle
    pub trajectories: Vec<Trajectory<P, Q, R>>,
}

impl<const P: usize, const Q: usize, const R: usize> TrajectoryBundle<P, Q, R> {
    /// Create a new empty bundle
    pub fn new() -> Self {
        Self {
            trajectories: Vec::new(),
        }
    }

    /// Create a bundle from a vector of trajectories
    pub fn from_trajectories(trajectories: Vec<Trajectory<P, Q, R>>) -> Self {
        Self { trajectories }
    }

    /// Add a trajectory to the bundle
    pub fn push(&mut self, trajectory: Trajectory<P, Q, R>) {
        self.trajectories.push(trajectory);
    }

    /// Get number of trajectories
    pub fn len(&self) -> usize {
        self.trajectories.len()
    }

    /// Check if bundle is empty
    pub fn is_empty(&self) -> bool {
        self.trajectories.is_empty()
    }

    /// Get iterator over trajectories
    pub fn iter(&self) -> impl Iterator<Item = &Trajectory<P, Q, R>> {
        self.trajectories.iter()
    }

    /// Analyze all trajectories
    pub fn analyze(&self) -> Vec<AnalyzedTrajectory<P, Q, R>> {
        self.trajectories
            .iter()
            .map(|t| AnalyzedTrajectory::from_trajectory(t.clone()))
            .collect()
    }

    /// Compute ensemble statistics
    pub fn compute_statistics(&self) -> BundleStatistics {
        if self.trajectories.is_empty() {
            return BundleStatistics::default();
        }

        let analyzed: Vec<_> = self.analyze();

        let lengths: Vec<f64> = analyzed.iter().map(|a| a.metadata.arc_length).collect();
        let max_dists: Vec<f64> = analyzed.iter().map(|a| a.metadata.max_distance).collect();
        let mean_dists: Vec<f64> = analyzed.iter().map(|a| a.metadata.mean_distance).collect();

        let mean_length = lengths.iter().sum::<f64>() / lengths.len() as f64;
        let mean_max_distance = max_dists.iter().sum::<f64>() / max_dists.len() as f64;
        let mean_mean_distance = mean_dists.iter().sum::<f64>() / mean_dists.len() as f64;

        let bounded_count = analyzed.iter().filter(|a| a.metadata.is_bounded).count();
        let bounded_fraction = bounded_count as f64 / analyzed.len() as f64;

        // Count trajectory types
        let mut type_counts = std::collections::HashMap::new();
        for a in &analyzed {
            *type_counts.entry(a.metadata.trajectory_type).or_insert(0) += 1;
        }

        BundleStatistics {
            num_trajectories: self.trajectories.len(),
            mean_length,
            min_length: lengths.iter().cloned().fold(f64::INFINITY, f64::min),
            max_length: lengths.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            mean_max_distance,
            mean_mean_distance,
            bounded_fraction,
            type_distribution: type_counts,
        }
    }

    /// Get all initial states
    pub fn initial_states(&self) -> Vec<Multivector<P, Q, R>> {
        self.trajectories
            .iter()
            .filter_map(|t| t.states.first().cloned())
            .collect()
    }

    /// Get all final states
    pub fn final_states(&self) -> Vec<Multivector<P, Q, R>> {
        self.trajectories
            .iter()
            .filter_map(|t| t.states.last().cloned())
            .collect()
    }
}

impl<const P: usize, const Q: usize, const R: usize> Default for TrajectoryBundle<P, Q, R> {
    fn default() -> Self {
        Self::new()
    }
}

/// Statistics for a bundle of trajectories
#[derive(Debug, Clone)]
pub struct BundleStatistics {
    /// Number of trajectories in bundle
    pub num_trajectories: usize,

    /// Mean arc length
    pub mean_length: f64,

    /// Minimum arc length
    pub min_length: f64,

    /// Maximum arc length
    pub max_length: f64,

    /// Mean of maximum distances from origin
    pub mean_max_distance: f64,

    /// Mean of mean distances from origin
    pub mean_mean_distance: f64,

    /// Fraction of trajectories that remained bounded
    pub bounded_fraction: f64,

    /// Distribution of trajectory types
    pub type_distribution: std::collections::HashMap<TrajectoryType, usize>,
}

impl Default for BundleStatistics {
    fn default() -> Self {
        Self {
            num_trajectories: 0,
            mean_length: 0.0,
            min_length: 0.0,
            max_length: 0.0,
            mean_max_distance: 0.0,
            mean_mean_distance: 0.0,
            bounded_fraction: 1.0,
            type_distribution: std::collections::HashMap::new(),
        }
    }
}

/// Interpolate a trajectory at a given time
pub fn interpolate_trajectory<const P: usize, const Q: usize, const R: usize>(
    trajectory: &Trajectory<P, Q, R>,
    t: f64,
) -> Option<Multivector<P, Q, R>> {
    let times = &trajectory.times;
    let states = &trajectory.states;

    if times.is_empty() || t < times[0] || t > *times.last()? {
        return None;
    }

    // Binary search for interval
    let idx = times.partition_point(|&time| time < t);

    if idx == 0 {
        return Some(states[0].clone());
    }

    if idx >= times.len() {
        return Some(states.last()?.clone());
    }

    // Linear interpolation
    let t0 = times[idx - 1];
    let t1 = times[idx];
    let alpha = (t - t0) / (t1 - t0);

    let s0 = &states[idx - 1];
    let s1 = &states[idx];

    let mut result = Multivector::<P, Q, R>::zero();
    for k in 0..(1 << (P + Q + R)) {
        let val = (1.0 - alpha) * s0.get(k) + alpha * s1.get(k);
        result.set(k, val);
    }

    Some(result)
}

/// Resample a trajectory at uniform time intervals
pub fn resample_trajectory<const P: usize, const Q: usize, const R: usize>(
    trajectory: &Trajectory<P, Q, R>,
    num_samples: usize,
) -> Trajectory<P, Q, R> {
    if trajectory.times.is_empty() || num_samples == 0 {
        return Trajectory {
            states: Vec::new(),
            times: Vec::new(),
            step_sizes: None,
            error_estimates: None,
        };
    }

    let t_start = trajectory.times[0];
    let t_end = *trajectory.times.last().unwrap();
    let dt = (t_end - t_start) / (num_samples - 1).max(1) as f64;

    let mut states = Vec::with_capacity(num_samples);
    let mut times = Vec::with_capacity(num_samples);

    for i in 0..num_samples {
        let t = t_start + i as f64 * dt;
        times.push(t);

        if let Some(state) = interpolate_trajectory(trajectory, t) {
            states.push(state);
        }
    }

    Trajectory {
        states,
        times,
        step_sizes: None,
        error_estimates: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_simple_trajectory() -> Trajectory<2, 0, 0> {
        let mut states = Vec::new();
        let mut times = Vec::new();

        for i in 0..100 {
            let t = i as f64 * 0.1;
            let mut state = Multivector::<2, 0, 0>::zero();
            state.set(0, t.cos());
            state.set(1, t.sin());
            states.push(state);
            times.push(t);
        }

        Trajectory {
            states,
            times,
            step_sizes: None,
            error_estimates: None,
        }
    }

    #[test]
    fn test_trajectory_type_enum() {
        assert_ne!(TrajectoryType::Chaotic, TrajectoryType::ConvergentToFixed);
        assert_eq!(TrajectoryType::Unknown, TrajectoryType::Unknown);
    }

    #[test]
    fn test_trajectory_metadata_default() {
        let meta = TrajectoryMetadata::default();
        assert!(matches!(meta.trajectory_type, TrajectoryType::Unknown));
        assert_eq!(meta.arc_length, 0.0);
    }

    #[test]
    fn test_analyzed_trajectory() {
        let traj = make_simple_trajectory();
        let analyzed = AnalyzedTrajectory::from_trajectory(traj);

        assert!(!analyzed.is_empty());
        assert_eq!(analyzed.len(), 100);
        assert!(analyzed.initial_state().is_some());
        assert!(analyzed.final_state().is_some());
        assert!(analyzed.duration() > 0.0);
        assert!(analyzed.metadata.arc_length > 0.0);
    }

    #[test]
    fn test_trajectory_bundle() {
        let traj1 = make_simple_trajectory();
        let traj2 = make_simple_trajectory();

        let mut bundle = TrajectoryBundle::new();
        assert!(bundle.is_empty());

        bundle.push(traj1);
        bundle.push(traj2);

        assert_eq!(bundle.len(), 2);
        assert!(!bundle.is_empty());

        let analyzed = bundle.analyze();
        assert_eq!(analyzed.len(), 2);

        let stats = bundle.compute_statistics();
        assert_eq!(stats.num_trajectories, 2);
        assert!(stats.mean_length > 0.0);
    }

    #[test]
    fn test_bundle_from_trajectories() {
        let trajectories = vec![make_simple_trajectory(), make_simple_trajectory()];
        let bundle = TrajectoryBundle::from_trajectories(trajectories);
        assert_eq!(bundle.len(), 2);
    }

    #[test]
    fn test_interpolate_trajectory() {
        let traj = make_simple_trajectory();

        // Interpolate at start
        let start = interpolate_trajectory(&traj, 0.0);
        assert!(start.is_some());

        // Interpolate in middle
        let mid = interpolate_trajectory(&traj, 5.0);
        assert!(mid.is_some());
        if let Some(m) = mid {
            assert!((m.get(0) - 5.0_f64.cos()).abs() < 0.1);
        }

        // Out of range
        let before = interpolate_trajectory(&traj, -1.0);
        assert!(before.is_none());

        let after = interpolate_trajectory(&traj, 100.0);
        assert!(after.is_none());
    }

    #[test]
    fn test_resample_trajectory() {
        let traj = make_simple_trajectory();
        let resampled = resample_trajectory(&traj, 50);

        assert_eq!(resampled.states.len(), 50);
        assert_eq!(resampled.times.len(), 50);
    }

    #[test]
    fn test_bundle_initial_final_states() {
        let bundle = TrajectoryBundle::from_trajectories(vec![
            make_simple_trajectory(),
            make_simple_trajectory(),
        ]);

        let initials = bundle.initial_states();
        assert_eq!(initials.len(), 2);

        let finals = bundle.final_states();
        assert_eq!(finals.len(), 2);
    }

    #[test]
    fn test_bundle_statistics_default() {
        let stats = BundleStatistics::default();
        assert_eq!(stats.num_trajectories, 0);
        assert_eq!(stats.bounded_fraction, 1.0);
    }
}
