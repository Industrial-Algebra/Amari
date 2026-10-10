//! GPU Verification Framework for Phase 4B
//!
//! This module implements boundary verification strategies for GPU-accelerated
//! geometric algebra operations, addressing the challenge that phantom types
//! cannot cross GPU memory boundaries while maintaining mathematical correctness.
//! Extended for Phase 4C with relativistic physics verification.

use crate::relativistic::{GpuRelativisticParticle, GpuSpacetimeVector};
use crate::{GpuCliffordAlgebra, GpuError};
use amari_core::Multivector;
use amari_info_geom::Parameter;
use amari_relativistic::constants::C;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GpuVerificationError {
    #[error("Verification failed: {0}")]
    VerificationFailed(String),

    #[error("Signature mismatch: expected {expected:?}, got {actual:?}")]
    SignatureMismatch {
        expected: (usize, usize, usize),
        actual: (usize, usize, usize),
    },

    #[error("Statistical verification failed: {failed}/{total} samples failed")]
    StatisticalMismatch { failed: usize, total: usize },

    #[error("Mathematical invariant violated: {invariant}")]
    InvariantViolation { invariant: String },

    #[error("GPU operation failed: {0}")]
    GpuOperation(#[from] GpuError),

    #[error("Performance budget exceeded: {actual:?} > {budget:?}")]
    PerformanceBudgetExceeded { actual: Duration, budget: Duration },
}

/// Verification strategy for GPU operations
#[derive(Debug, Clone)]
pub enum VerificationStrategy {
    /// Full verification of all elements (expensive)
    Strict,
    /// Statistical sampling verification (balanced)
    Statistical { sample_rate: f64 },
    /// Boundary verification only (fast)
    Boundary,
    /// Minimal verification (fastest)
    Minimal,
}

/// Platform-aware verification configuration
#[derive(Debug, Clone)]
pub struct VerificationConfig {
    pub strategy: VerificationStrategy,
    pub performance_budget: Duration,
    pub tolerance: f64,
    pub enable_invariant_checking: bool,
}

impl Default for VerificationConfig {
    fn default() -> Self {
        Self {
            strategy: VerificationStrategy::Statistical { sample_rate: 0.1 },
            performance_budget: Duration::from_millis(10),
            tolerance: 1e-12,
            enable_invariant_checking: true,
        }
    }
}

/// Verified multivector with signature information preserved
#[derive(Debug, Clone)]
pub struct VerifiedMultivector<const P: usize, const Q: usize, const R: usize> {
    pub inner: Multivector<P, Q, R>,
    verification_hash: u64,
    _phantom: PhantomData<(SignatureP<P>, SignatureQ<Q>, SignatureR<R>)>,
}

impl<const P: usize, const Q: usize, const R: usize> VerifiedMultivector<P, Q, R> {
    /// Create verified multivector with compile-time signature checking
    pub fn new(inner: Multivector<P, Q, R>) -> Self {
        let verification_hash = Self::compute_verification_hash(&inner);
        Self {
            inner,
            verification_hash,
            _phantom: PhantomData,
        }
    }

    /// Extract inner multivector for GPU operations (loses verification)
    pub fn into_inner(self) -> Multivector<P, Q, R> {
        self.inner
    }

    /// Get reference to inner multivector
    pub fn inner(&self) -> &Multivector<P, Q, R> {
        &self.inner
    }

    /// Verify mathematical invariants
    pub fn verify_invariants(&self) -> Result<(), GpuVerificationError> {
        // Check magnitude invariant
        let magnitude = self.inner.coefficient_norm();
        if !magnitude.is_finite() {
            return Err(GpuVerificationError::InvariantViolation {
                invariant: "Magnitude must be finite".to_string(),
            });
        }

        // Verify signature consistency
        if !self.verify_signature_constraints() {
            return Err(GpuVerificationError::InvariantViolation {
                invariant: "Signature constraints violated".to_string(),
            });
        }

        Ok(())
    }

    /// Compute verification hash for integrity checking
    fn compute_verification_hash(mv: &Multivector<P, Q, R>) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash signature
        (P, Q, R).hash(&mut hasher);

        // Hash coefficients (with tolerance for floating point)
        for i in 0..mv.dimension() {
            let coeff = mv.get(i);
            let normalized = (coeff * 1e12).round() as i64; // 12 decimal places
            normalized.hash(&mut hasher);
        }

        hasher.finish()
    }

    /// Verify signature constraints are satisfied
    fn verify_signature_constraints(&self) -> bool {
        // Check that P + Q + R matches dimension
        let expected_dim = 1 << (P + Q + R);
        self.inner.dimension() == expected_dim
    }

    /// Get signature tuple
    pub fn signature() -> (usize, usize, usize) {
        (P, Q, R)
    }
}

/// Phantom types for compile-time signature verification
struct SignatureP<const P: usize>;
struct SignatureQ<const Q: usize>;
struct SignatureR<const R: usize>;

/// GPU boundary verification system
pub struct GpuBoundaryVerifier {
    config: VerificationConfig,
    performance_stats: PerformanceStats,
}

impl GpuBoundaryVerifier {
    /// Create new boundary verifier with configuration
    pub fn new(config: VerificationConfig) -> Self {
        Self {
            config,
            performance_stats: PerformanceStats::new(),
        }
    }

    /// Verify batch geometric product with boundary checking
    pub async fn verified_batch_geometric_product<
        const P: usize,
        const Q: usize,
        const R: usize,
    >(
        &mut self,
        gpu: &GpuCliffordAlgebra,
        a_batch: &[VerifiedMultivector<P, Q, R>],
        b_batch: &[VerifiedMultivector<P, Q, R>],
    ) -> Result<Vec<VerifiedMultivector<P, Q, R>>, GpuVerificationError> {
        let start_time = Instant::now();

        // 1. Pre-GPU verification phase
        self.verify_input_batch_invariants(a_batch, b_batch)?;

        // 2. Extract raw data for GPU (loses phantom types temporarily)
        let raw_a = self.extract_raw_coefficients(a_batch);
        let raw_b = self.extract_raw_coefficients(b_batch);

        // 3. GPU computation (unverified internally)
        let raw_result = gpu
            .batch_geometric_product(&raw_a, &raw_b)
            .await
            .map_err(GpuVerificationError::GpuOperation)?;

        // 4. Post-GPU verification and phantom type restoration
        let verified_result =
            self.verify_and_restore_types::<P, Q, R>(&raw_result, a_batch, b_batch)?;

        // 5. Performance tracking
        let elapsed = start_time.elapsed();
        self.performance_stats
            .record_operation(elapsed, a_batch.len());

        if elapsed > self.config.performance_budget {
            return Err(GpuVerificationError::PerformanceBudgetExceeded {
                actual: elapsed,
                budget: self.config.performance_budget,
            });
        }

        Ok(verified_result)
    }

    /// Verify input batch mathematical invariants
    fn verify_input_batch_invariants<const P: usize, const Q: usize, const R: usize>(
        &self,
        a_batch: &[VerifiedMultivector<P, Q, R>],
        b_batch: &[VerifiedMultivector<P, Q, R>],
    ) -> Result<(), GpuVerificationError> {
        if a_batch.len() != b_batch.len() {
            return Err(GpuVerificationError::VerificationFailed(
                "Batch sizes must match".to_string(),
            ));
        }

        if !self.config.enable_invariant_checking {
            return Ok(());
        }

        // Verify invariants based on strategy
        match &self.config.strategy {
            VerificationStrategy::Strict => {
                // Verify all elements
                for (i, (a, b)) in a_batch.iter().zip(b_batch.iter()).enumerate() {
                    a.verify_invariants().map_err(|e| {
                        GpuVerificationError::VerificationFailed(format!("Input A[{}]: {}", i, e))
                    })?;
                    b.verify_invariants().map_err(|e| {
                        GpuVerificationError::VerificationFailed(format!("Input B[{}]: {}", i, e))
                    })?;
                }
            }
            VerificationStrategy::Statistical { sample_rate } => {
                // Verify random sample
                let sample_size = ((a_batch.len() as f64) * sample_rate).ceil() as usize;
                let indices = self.select_random_indices(a_batch.len(), sample_size);

                for &idx in &indices {
                    a_batch[idx].verify_invariants()?;
                    b_batch[idx].verify_invariants()?;
                }
            }
            VerificationStrategy::Boundary | VerificationStrategy::Minimal => {
                // Only verify first and last elements
                if !a_batch.is_empty() {
                    a_batch[0].verify_invariants()?;
                    b_batch[0].verify_invariants()?;

                    if a_batch.len() > 1 {
                        let last = a_batch.len() - 1;
                        a_batch[last].verify_invariants()?;
                        b_batch[last].verify_invariants()?;
                    }
                }
            }
        }

        Ok(())
    }

    /// Extract raw coefficients for GPU computation
    fn extract_raw_coefficients<const P: usize, const Q: usize, const R: usize>(
        &self,
        batch: &[VerifiedMultivector<P, Q, R>],
    ) -> Vec<f64> {
        let basis_count = 1 << (P + Q + R);
        let mut raw_data = Vec::with_capacity(batch.len() * basis_count);

        for mv in batch {
            for i in 0..basis_count {
                raw_data.push(mv.inner.get(i));
            }
        }

        raw_data
    }

    /// Verify GPU results and restore phantom types
    fn verify_and_restore_types<const P: usize, const Q: usize, const R: usize>(
        &self,
        raw_result: &[f64],
        a_batch: &[VerifiedMultivector<P, Q, R>],
        b_batch: &[VerifiedMultivector<P, Q, R>],
    ) -> Result<Vec<VerifiedMultivector<P, Q, R>>, GpuVerificationError> {
        let basis_count = 1 << (P + Q + R);
        let batch_size = raw_result.len() / basis_count;

        if batch_size != a_batch.len() {
            return Err(GpuVerificationError::VerificationFailed(
                "Result batch size mismatch".to_string(),
            ));
        }

        let mut verified_results = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let start_idx = i * basis_count;
            let end_idx = start_idx + basis_count;

            let coefficients = raw_result[start_idx..end_idx].to_vec();
            let result_mv = Multivector::<P, Q, R>::from_coefficients(coefficients);

            // Verify result based on strategy
            match &self.config.strategy {
                VerificationStrategy::Strict => {
                    // Full verification: check against CPU computation
                    let expected = a_batch[i].inner.geometric_product(&b_batch[i].inner);
                    self.verify_approximately_equal(&result_mv, &expected, i)?;
                }
                VerificationStrategy::Statistical { sample_rate } => {
                    // Statistical verification: check random samples
                    if self.should_verify_sample(i, *sample_rate) {
                        let expected = a_batch[i].inner.geometric_product(&b_batch[i].inner);
                        self.verify_approximately_equal(&result_mv, &expected, i)?;
                    }
                }
                VerificationStrategy::Boundary => {
                    // Boundary verification: check first and last
                    if i == 0 || i == batch_size - 1 {
                        let expected = a_batch[i].inner.geometric_product(&b_batch[i].inner);
                        self.verify_approximately_equal(&result_mv, &expected, i)?;
                    }
                }
                VerificationStrategy::Minimal => {
                    // Minimal verification: basic sanity checks only
                    if !result_mv.coefficient_norm().is_finite() {
                        return Err(GpuVerificationError::InvariantViolation {
                            invariant: format!("Result[{}] magnitude is not finite", i),
                        });
                    }
                }
            }

            verified_results.push(VerifiedMultivector::new(result_mv));
        }

        Ok(verified_results)
    }

    /// Verify two multivectors are approximately equal
    fn verify_approximately_equal<const P: usize, const Q: usize, const R: usize>(
        &self,
        actual: &Multivector<P, Q, R>,
        expected: &Multivector<P, Q, R>,
        index: usize,
    ) -> Result<(), GpuVerificationError> {
        let basis_count = 1 << (P + Q + R);

        for i in 0..basis_count {
            let diff = (actual.get(i) - expected.get(i)).abs();
            let rel_error = if expected.get(i).abs() > self.config.tolerance {
                diff / expected.get(i).abs()
            } else {
                diff
            };

            if rel_error > self.config.tolerance {
                return Err(GpuVerificationError::VerificationFailed(
                    format!(
                        "Verification failed at result[{}], component[{}]: expected {}, got {}, error {}",
                        index, i, expected.get(i), actual.get(i), rel_error
                    )
                ));
            }
        }

        Ok(())
    }

    /// Select random indices for statistical sampling
    fn select_random_indices(&self, total: usize, sample_size: usize) -> Vec<usize> {
        use std::collections::HashSet;

        let mut indices = HashSet::new();
        let sample_size = sample_size.min(total);

        // Simple deterministic "random" selection for reproducibility
        let step = total / sample_size.max(1);
        for i in 0..sample_size {
            indices.insert((i * step) % total);
        }

        // Ensure we always include first and last
        if total > 0 {
            indices.insert(0);
            if total > 1 {
                indices.insert(total - 1);
            }
        }

        indices.into_iter().collect()
    }

    /// Determine if a sample should be verified
    fn should_verify_sample(&self, index: usize, sample_rate: f64) -> bool {
        // Simple deterministic sampling based on index
        let hash = index.wrapping_mul(2654435761); // Large prime
        let normalized = (hash as f64) / (u32::MAX as f64);
        normalized < sample_rate
    }

    /// Get performance statistics
    pub fn performance_stats(&self) -> &PerformanceStats {
        &self.performance_stats
    }
}

/// Performance tracking for verification operations
#[derive(Debug, Clone)]
pub struct PerformanceStats {
    operation_count: usize,
    total_duration: Duration,
    total_elements: usize,
    max_duration: Duration,
}

impl PerformanceStats {
    fn new() -> Self {
        Self {
            operation_count: 0,
            total_duration: Duration::ZERO,
            total_elements: 0,
            max_duration: Duration::ZERO,
        }
    }

    fn record_operation(&mut self, duration: Duration, element_count: usize) {
        self.operation_count += 1;
        self.total_duration += duration;
        self.total_elements += element_count;
        if duration > self.max_duration {
            self.max_duration = duration;
        }
    }

    /// Get average operation duration
    pub fn average_duration(&self) -> Duration {
        if self.operation_count > 0 {
            self.total_duration / (self.operation_count as u32)
        } else {
            Duration::ZERO
        }
    }

    /// Get average throughput (elements per second)
    pub fn average_throughput(&self) -> f64 {
        if self.total_duration.as_secs_f64() > 0.0 {
            self.total_elements as f64 / self.total_duration.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Get verification overhead as percentage
    pub fn verification_overhead_percent(&self, baseline_duration: Duration) -> f64 {
        if baseline_duration.as_secs_f64() > 0.0 {
            let overhead = self.average_duration().as_secs_f64() / baseline_duration.as_secs_f64();
            (overhead - 1.0) * 100.0
        } else {
            0.0
        }
    }

    /// Get operation count
    pub fn operation_count(&self) -> usize {
        self.operation_count
    }

    /// Get total elements processed
    pub fn total_elements(&self) -> usize {
        self.total_elements
    }

    /// Get maximum operation duration
    pub fn max_duration(&self) -> Duration {
        self.max_duration
    }
}

/// Statistical verification for large GPU batches
pub struct StatisticalGpuVerifier<const P: usize, const Q: usize, const R: usize> {
    sample_rate: f64,
    tolerance: f64,
    verification_cache: HashMap<u64, bool>,
}

impl<const P: usize, const Q: usize, const R: usize> StatisticalGpuVerifier<P, Q, R> {
    /// Create new statistical verifier
    pub fn new(sample_rate: f64, tolerance: f64) -> Self {
        Self {
            sample_rate,
            tolerance,
            verification_cache: HashMap::new(),
        }
    }

    /// Verify batch result through statistical sampling
    pub async fn verify_batch_statistical(
        &mut self,
        _gpu: &GpuCliffordAlgebra,
        inputs: &[(VerifiedMultivector<P, Q, R>, VerifiedMultivector<P, Q, R>)],
        gpu_results: &[Multivector<P, Q, R>],
    ) -> Result<Vec<VerifiedMultivector<P, Q, R>>, GpuVerificationError> {
        if inputs.len() != gpu_results.len() {
            return Err(GpuVerificationError::VerificationFailed(
                "Input and result batch sizes must match".to_string(),
            ));
        }

        let sample_size = (inputs.len() as f64 * self.sample_rate).ceil() as usize;
        let indices = self.select_random_indices(inputs.len(), sample_size);

        let mut failed_samples = 0;

        for &idx in &indices {
            let (a, b) = &inputs[idx];
            let expected = a.inner.geometric_product(&b.inner);
            let actual = &gpu_results[idx];

            if !self.approximately_equal(&expected, actual) {
                failed_samples += 1;

                // Cache failed verification
                let hash = self.compute_input_hash(a, b);
                self.verification_cache.insert(hash, false);
            }
        }

        // Allow small number of failures for statistical verification
        let failure_rate = failed_samples as f64 / indices.len() as f64;
        let max_failure_rate = 0.01; // 1% maximum failure rate

        if failure_rate > max_failure_rate {
            return Err(GpuVerificationError::StatisticalMismatch {
                failed: failed_samples,
                total: indices.len(),
            });
        }

        // If samples pass, assume entire batch is correct with verification restoration
        let verified_results = gpu_results
            .iter()
            .map(|mv| VerifiedMultivector::new(mv.clone()))
            .collect();

        Ok(verified_results)
    }

    /// Check if two multivectors are approximately equal
    fn approximately_equal(&self, a: &Multivector<P, Q, R>, b: &Multivector<P, Q, R>) -> bool {
        let basis_count = 1 << (P + Q + R);

        for i in 0..basis_count {
            let diff = (a.get(i) - b.get(i)).abs();
            let rel_error = if b.get(i).abs() > self.tolerance {
                diff / b.get(i).abs()
            } else {
                diff
            };

            if rel_error > self.tolerance {
                return false;
            }
        }

        true
    }

    /// Select random indices for sampling
    fn select_random_indices(&self, total: usize, sample_size: usize) -> Vec<usize> {
        let mut indices = Vec::new();
        let sample_size = sample_size.min(total);

        if total == 0 {
            return indices;
        }

        // Always include first and last
        indices.push(0);
        if total > 1 {
            indices.push(total - 1);
        }

        // Add random intermediate indices
        let step = if sample_size > 2 {
            total / (sample_size - 2).max(1)
        } else {
            total
        };

        for i in 1..sample_size.saturating_sub(1) {
            let idx = (i * step) % total;
            if !indices.contains(&idx) {
                indices.push(idx);
            }
        }

        indices.sort_unstable();
        indices
    }

    /// Compute hash for input pair caching
    fn compute_input_hash(
        &self,
        a: &VerifiedMultivector<P, Q, R>,
        b: &VerifiedMultivector<P, Q, R>,
    ) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        a.verification_hash.hash(&mut hasher);
        b.verification_hash.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verified_multivector_creation() {
        let mv = Multivector::<3, 0, 0>::zero();
        let verified = VerifiedMultivector::new(mv);

        assert_eq!(VerifiedMultivector::<3, 0, 0>::signature(), (3, 0, 0));
        assert!(verified.verify_invariants().is_ok());
    }

    #[test]
    fn test_verification_config_default() {
        let config = VerificationConfig::default();

        match config.strategy {
            VerificationStrategy::Statistical { sample_rate } => {
                assert!((sample_rate - 0.1).abs() < 1e-10);
            }
            _ => panic!("Expected statistical strategy"),
        }

        assert_eq!(config.performance_budget, Duration::from_millis(10));
        assert!((config.tolerance - 1e-12).abs() < 1e-15);
        assert!(config.enable_invariant_checking);
    }

    #[test]
    fn test_performance_stats() {
        let mut stats = PerformanceStats::new();

        stats.record_operation(Duration::from_millis(5), 100);
        stats.record_operation(Duration::from_millis(10), 200);

        assert_eq!(stats.operation_count, 2);
        assert_eq!(stats.total_elements, 300);
        assert_eq!(
            stats.average_duration(),
            Duration::from_millis(7) + Duration::from_micros(500)
        );
        assert_eq!(stats.max_duration, Duration::from_millis(10));

        let throughput = stats.average_throughput();
        assert!(throughput > 0.0);
    }

    #[test]
    fn test_statistical_verifier_sampling() {
        let verifier = StatisticalGpuVerifier::<3, 0, 0>::new(0.1, 1e-12);

        let indices = verifier.select_random_indices(100, 10);
        assert!(indices.len() <= 10);
        assert!(indices.contains(&0)); // First element
        assert!(indices.contains(&99)); // Last element

        // Test empty case
        let empty_indices = verifier.select_random_indices(0, 5);
        assert!(empty_indices.is_empty());
    }

    #[tokio::test]
    async fn test_boundary_verifier_creation() {
        let config = VerificationConfig::default();
        let verifier = GpuBoundaryVerifier::new(config);

        assert_eq!(verifier.performance_stats().operation_count, 0);
        assert_eq!(
            verifier.performance_stats().average_duration(),
            Duration::ZERO
        );
    }
}

/// Relativistic physics verification functions for GPU operations
pub struct RelativisticVerifier {
    tolerance: f64,
    #[allow(dead_code)]
    config: VerificationConfig,
}

impl RelativisticVerifier {
    /// Create new relativistic verifier
    pub fn new(tolerance: f64) -> Self {
        Self {
            tolerance,
            config: VerificationConfig::default(),
        }
    }

    /// Verify four-velocity normalization: u·u = c²
    pub fn verify_four_velocity_normalization(
        &self,
        velocities: &[GpuSpacetimeVector],
    ) -> Result<(), GpuVerificationError> {
        let c_squared = (C * C) as f32;

        for (i, velocity) in velocities.iter().enumerate() {
            let norm_squared = self.minkowski_norm_squared(velocity);
            let deviation = (norm_squared - c_squared).abs();
            let relative_error = deviation / c_squared;

            if relative_error > self.tolerance as f32 {
                return Err(GpuVerificationError::InvariantViolation {
                    invariant: format!(
                        "Four-velocity normalization violation at index {}: |u|² = {:.6e}, expected c² = {:.6e}, deviation = {:.6e}",
                        i, norm_squared, c_squared, deviation
                    ),
                });
            }
        }

        Ok(())
    }

    /// Verify energy-momentum relation: E² = (pc)² + (mc²)²
    pub fn verify_energy_momentum_relation(
        &self,
        particles: &[GpuRelativisticParticle],
    ) -> Result<(), GpuVerificationError> {
        for (i, particle) in particles.iter().enumerate() {
            let gamma = self.lorentz_factor(&particle.velocity);
            let c = C as f32;

            // Total energy: E = γmc²
            let energy = gamma * particle.mass * c * c;

            // Spatial momentum magnitude: |p| = γm|v|
            let spatial_vel_mag = (particle.velocity.x * particle.velocity.x
                + particle.velocity.y * particle.velocity.y
                + particle.velocity.z * particle.velocity.z)
                .sqrt();
            let momentum_mag = gamma * particle.mass * spatial_vel_mag;

            // Energy-momentum relation: E² = (pc)² + (mc²)²
            let lhs = energy * energy;
            let rhs = (momentum_mag * c) * (momentum_mag * c)
                + (particle.mass * c * c) * (particle.mass * c * c);

            let deviation = (lhs - rhs).abs();
            let relative_error = deviation / lhs.max(rhs);

            if relative_error > self.tolerance as f32 {
                return Err(GpuVerificationError::InvariantViolation {
                    invariant: format!(
                        "Energy-momentum relation violation at particle {}: E² = {:.6e}, (pc)² + (mc²)² = {:.6e}, relative error = {:.6e}",
                        i, lhs, rhs, relative_error
                    ),
                });
            }
        }

        Ok(())
    }

    /// Verify Minkowski signature preservation
    pub fn verify_minkowski_signature(
        &self,
        vectors: &[GpuSpacetimeVector],
        expected_signs: &[i8], // +1 for timelike, -1 for spacelike, 0 for null
    ) -> Result<(), GpuVerificationError> {
        if vectors.len() != expected_signs.len() {
            return Err(GpuVerificationError::VerificationFailed(
                "Vector and expected sign arrays must have same length".to_string(),
            ));
        }

        for (i, (vector, &expected_sign)) in vectors.iter().zip(expected_signs.iter()).enumerate() {
            let norm_squared = self.minkowski_norm_squared(vector);

            let actual_sign = if norm_squared > self.tolerance as f32 {
                1 // Timelike
            } else if norm_squared < -(self.tolerance as f32) {
                -1 // Spacelike
            } else {
                0 // Null
            };

            if actual_sign != expected_sign {
                return Err(GpuVerificationError::InvariantViolation {
                    invariant: format!(
                        "Minkowski signature violation at index {}: expected {}, got {} (norm² = {:.6e})",
                        i, expected_sign, actual_sign, norm_squared
                    ),
                });
            }
        }

        Ok(())
    }

    /// Verify causality constraints
    pub fn verify_causality_constraints(
        &self,
        velocities: &[GpuSpacetimeVector],
    ) -> Result<(), GpuVerificationError> {
        let c = C as f32;

        for (i, velocity) in velocities.iter().enumerate() {
            let spatial_speed_squared =
                velocity.x * velocity.x + velocity.y * velocity.y + velocity.z * velocity.z;
            let spatial_speed = spatial_speed_squared.sqrt();

            // Check that spatial velocity magnitude is less than c
            if spatial_speed >= c * (1.0 - self.tolerance as f32) {
                return Err(GpuVerificationError::InvariantViolation {
                    invariant: format!(
                        "Causality violation at index {}: spatial speed {:.6e} >= c ({:.6e})",
                        i, spatial_speed, c
                    ),
                });
            }
        }

        Ok(())
    }

    /// Compute Minkowski norm squared: t² - x² - y² - z²
    fn minkowski_norm_squared(&self, vector: &GpuSpacetimeVector) -> f32 {
        vector.t * vector.t - vector.x * vector.x - vector.y * vector.y - vector.z * vector.z
    }

    /// Compute Lorentz factor γ = 1/√(1 - v²/c²)
    fn lorentz_factor(&self, four_velocity: &GpuSpacetimeVector) -> f32 {
        let c = C as f32;
        four_velocity.t / c
    }

    /// Verify a batch of relativistic particles with comprehensive checks
    pub fn verify_particle_batch(
        &self,
        particles: &[GpuRelativisticParticle],
    ) -> Result<(), GpuVerificationError> {
        // Extract velocities for verification
        let velocities: Vec<GpuSpacetimeVector> = particles.iter().map(|p| p.velocity).collect();

        // Verify four-velocity normalization
        self.verify_four_velocity_normalization(&velocities)?;

        // Verify energy-momentum relation
        self.verify_energy_momentum_relation(particles)?;

        // Verify causality constraints
        self.verify_causality_constraints(&velocities)?;

        Ok(())
    }

    /// Statistical verification of relativistic invariants
    pub fn statistical_verify_particles(
        &self,
        particles: &[GpuRelativisticParticle],
        sample_rate: f64,
    ) -> Result<(), GpuVerificationError> {
        if particles.is_empty() {
            return Ok(());
        }

        let sample_count = ((particles.len() as f64) * sample_rate).ceil() as usize;
        let sample_count = sample_count.min(particles.len()).max(1);

        let mut sampled_particles = Vec::with_capacity(sample_count);
        let step = particles.len() / sample_count;

        for i in 0..sample_count {
            let index = i * step;
            if index < particles.len() {
                sampled_particles.push(particles[index]);
            }
        }

        // Always include first and last if not already included
        if !sampled_particles.is_empty() {
            sampled_particles[0] = particles[0];
            if sampled_particles.len() > 1 && sample_count < particles.len() {
                let last_index = sampled_particles.len() - 1;
                sampled_particles[last_index] = particles[particles.len() - 1];
            }
        }

        self.verify_particle_batch(&sampled_particles)
    }
}

#[cfg(test)]
mod relativistic_tests {
    use super::*;

    #[test]
    fn test_four_velocity_normalization_verification() {
        // Use relative tolerance appropriate for c² scale values
        let verifier = RelativisticVerifier::new(1e-5);
        let c = C as f32;

        // Test with properly normalized four-velocity directly

        // Actually create properly normalized four-velocity
        let beta = 0.6_f32;
        let gamma = 1.0_f32 / (1.0_f32 - beta * beta).sqrt();
        let normalized_velocity = GpuSpacetimeVector::new(gamma * c, gamma * beta * c, 0.0, 0.0);

        // Debug: Check the actual norm
        let actual_norm_sq = normalized_velocity.t * normalized_velocity.t
            - normalized_velocity.x * normalized_velocity.x
            - normalized_velocity.y * normalized_velocity.y
            - normalized_velocity.z * normalized_velocity.z;
        let expected_c_sq = c * c;
        let deviation = (actual_norm_sq - expected_c_sq).abs();

        // Check relative error instead of absolute error for large numbers like c²
        let relative_error = deviation / expected_c_sq;
        if relative_error > 1e-6 {
            panic!("Four-velocity not properly normalized: actual = {:.8e}, expected = {:.8e}, relative error = {:.8e}",
                actual_norm_sq, expected_c_sq, relative_error);
        }

        let velocities = vec![normalized_velocity];
        assert!(verifier
            .verify_four_velocity_normalization(&velocities)
            .is_ok());

        // Invalid four-velocity (not normalized)
        let invalid_velocity = GpuSpacetimeVector::new(1.0, 2.0, 3.0, 4.0);
        let invalid_velocities = vec![invalid_velocity];
        assert!(verifier
            .verify_four_velocity_normalization(&invalid_velocities)
            .is_err());
    }

    #[test]
    fn test_causality_verification() {
        let verifier = RelativisticVerifier::new(1e-6);
        let c = C as f32;

        // Valid subluminal velocity
        let valid_velocity = GpuSpacetimeVector::new(c, 0.5 * c, 0.0, 0.0);
        let valid_velocities = vec![valid_velocity];
        assert!(verifier
            .verify_causality_constraints(&valid_velocities)
            .is_ok());

        // Invalid superluminal velocity
        let invalid_velocity = GpuSpacetimeVector::new(c, 1.1 * c, 0.0, 0.0);
        let invalid_velocities = vec![invalid_velocity];
        assert!(verifier
            .verify_causality_constraints(&invalid_velocities)
            .is_err());
    }

    #[test]
    fn test_minkowski_signature_verification() {
        let verifier = RelativisticVerifier::new(1e-6);

        // Timelike vector (t² > x² + y² + z²)
        let timelike = GpuSpacetimeVector::new(2.0, 1.0, 0.0, 0.0);
        // Spacelike vector (t² < x² + y² + z²)
        let spacelike = GpuSpacetimeVector::new(1.0, 2.0, 0.0, 0.0);
        // Null vector (t² = x² + y² + z²)
        let null = GpuSpacetimeVector::new(1.0, 1.0, 0.0, 0.0);

        let vectors = vec![timelike, spacelike, null];
        let expected_signs = vec![1, -1, 0];

        assert!(verifier
            .verify_minkowski_signature(&vectors, &expected_signs)
            .is_ok());

        // Wrong expectations should fail
        let wrong_signs = vec![-1, 1, 0];
        assert!(verifier
            .verify_minkowski_signature(&vectors, &wrong_signs)
            .is_err());
    }
}
