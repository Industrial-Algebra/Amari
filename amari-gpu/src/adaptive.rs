//! Adaptive Verification Framework for Cross-Platform GPU Operations
//!
//! This module implements platform detection and adaptive verification
//! strategies that automatically adjust verification approaches based on
//! the execution environment and performance constraints.

use crate::{verification::*, GpuCliffordAlgebra};
use std::time::{Duration, Instant};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdaptiveVerificationError {
    #[error("Platform detection failed: {0}")]
    PlatformDetection(String),

    #[error("GPU verification failed: {0}")]
    GpuVerification(#[from] GpuVerificationError),

    #[error("No suitable verification strategy available")]
    NoSuitableStrategy,

    #[error("Performance constraint violation: {constraint}")]
    PerformanceConstraint { constraint: String },
}

/// Platform-specific execution environment
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationPlatform {
    /// Native CPU with full phantom type support
    NativeCpu { features: CpuFeatures },
    /// GPU with boundary verification constraints
    Gpu {
        backend: GpuBackend,
        memory_mb: u64,
        compute_units: u32,
    },
    /// WebAssembly with runtime verification
    Wasm { env: WasmEnvironment },
}

#[derive(Debug, Clone, PartialEq)]
pub struct CpuFeatures {
    pub supports_simd: bool,
    pub core_count: usize,
    pub cache_size_kb: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GpuBackend {
    Vulkan,
    Metal,
    Dx12,
    OpenGL,
    WebGpu,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WasmEnvironment {
    Browser { engine: String },
    NodeJs { version: String },
    Standalone,
}

/// Verification level that adapts to platform constraints
#[derive(Debug, Clone, PartialEq)]
pub enum AdaptiveVerificationLevel {
    /// Maximum verification (CPU only)
    Maximum,
    /// High verification with performance awareness
    High,
    /// Balanced verification for production workloads
    Balanced,
    /// Minimal verification for performance-critical paths
    Minimal,
    /// Debug-only verification
    Debug,
}

/// Adaptive verifier that selects optimal strategy per platform
pub struct AdaptiveVerifier {
    platform: VerificationPlatform,
    verification_level: AdaptiveVerificationLevel,
    performance_budget: Duration,
    boundary_verifier: Option<GpuBoundaryVerifier>,
    gpu_instance: Option<GpuCliffordAlgebra>,
}

impl AdaptiveVerifier {
    /// Create adaptive verifier with automatic platform detection
    pub async fn new() -> Result<Self, AdaptiveVerificationError> {
        let platform = Self::detect_platform().await?;
        let verification_level = Self::determine_verification_level(&platform);
        let performance_budget = Self::determine_performance_budget(&platform);

        let (boundary_verifier, gpu_instance) = match &platform {
            VerificationPlatform::Gpu { .. } => {
                let config = Self::create_gpu_verification_config(&platform, &verification_level);
                let verifier = GpuBoundaryVerifier::new(config);
                let gpu = GpuCliffordAlgebra::new::<3, 0, 0>().await.ok();
                (Some(verifier), gpu)
            }
            _ => (None, None),
        };

        Ok(Self {
            platform,
            verification_level,
            performance_budget,
            boundary_verifier,
            gpu_instance,
        })
    }

    /// Create adaptive verifier with explicit configuration
    pub async fn with_config(
        level: AdaptiveVerificationLevel,
        budget: Duration,
    ) -> Result<Self, AdaptiveVerificationError> {
        let platform = Self::detect_platform().await?;

        let (boundary_verifier, gpu_instance) = match &platform {
            VerificationPlatform::Gpu { .. } => {
                let config = Self::create_gpu_verification_config(&platform, &level);
                let verifier = GpuBoundaryVerifier::new(config);
                let gpu = GpuCliffordAlgebra::new::<3, 0, 0>().await.ok();
                (Some(verifier), gpu)
            }
            _ => (None, None),
        };

        Ok(Self {
            platform,
            verification_level: level,
            performance_budget: budget,
            boundary_verifier,
            gpu_instance,
        })
    }

    /// Perform verified operation with platform-appropriate strategy
    pub async fn verified_geometric_product<const P: usize, const Q: usize, const R: usize>(
        &mut self,
        a: &VerifiedMultivector<P, Q, R>,
        b: &VerifiedMultivector<P, Q, R>,
    ) -> Result<VerifiedMultivector<P, Q, R>, AdaptiveVerificationError> {
        let start_time = Instant::now();

        let result = match &self.platform {
            VerificationPlatform::NativeCpu { .. } => {
                // Full phantom type verification available
                self.cpu_verification(a, b).await?
            }
            VerificationPlatform::Gpu { .. } => {
                // Single operations typically use CPU for efficiency
                self.cpu_verification(a, b).await?
            }
            VerificationPlatform::Wasm { .. } => {
                // Runtime contract verification
                self.wasm_runtime_verification(a, b).await?
            }
        };

        let elapsed = start_time.elapsed();
        if elapsed > self.performance_budget {
            return Err(AdaptiveVerificationError::PerformanceConstraint {
                constraint: format!(
                    "Operation exceeded budget: {:?} > {:?}",
                    elapsed, self.performance_budget
                ),
            });
        }

        Ok(result)
    }

    /// Perform verified batch operation with optimal GPU/CPU dispatch
    pub async fn verified_batch_geometric_product<
        const P: usize,
        const Q: usize,
        const R: usize,
    >(
        &mut self,
        a_batch: &[VerifiedMultivector<P, Q, R>],
        b_batch: &[VerifiedMultivector<P, Q, R>],
    ) -> Result<Vec<VerifiedMultivector<P, Q, R>>, AdaptiveVerificationError> {
        if a_batch.len() != b_batch.len() {
            return Err(AdaptiveVerificationError::NoSuitableStrategy);
        }
        if a_batch.is_empty() {
            return Ok(Vec::new());
        }

        match &self.platform {
            VerificationPlatform::NativeCpu { .. } => {
                // CPU batch processing with full verification
                self.cpu_batch_verification(a_batch, b_batch).await
            }
            VerificationPlatform::Gpu { .. } => {
                // GPU boundary verification strategy
                self.gpu_batch_verification(a_batch, b_batch).await
            }
            VerificationPlatform::Wasm { .. } => {
                // WASM runtime verification with progressive enhancement
                self.wasm_batch_verification(a_batch, b_batch).await
            }
        }
    }

    /// Get platform information
    pub fn platform(&self) -> &VerificationPlatform {
        &self.platform
    }

    /// Get current verification level
    pub fn verification_level(&self) -> &AdaptiveVerificationLevel {
        &self.verification_level
    }

    /// Get performance budget
    pub fn performance_budget(&self) -> Duration {
        self.performance_budget
    }

    /// Check if GPU acceleration should be used for given batch size
    pub fn should_use_gpu(&self, batch_size: usize) -> bool {
        match &self.platform {
            VerificationPlatform::Gpu {
                compute_units,
                memory_mb,
                ..
            } => {
                // Heuristic based on GPU capabilities and batch size
                let min_batch_size = match &self.verification_level {
                    AdaptiveVerificationLevel::Maximum => 500,
                    AdaptiveVerificationLevel::High => 200,
                    AdaptiveVerificationLevel::Balanced => 100,
                    AdaptiveVerificationLevel::Minimal => 50,
                    AdaptiveVerificationLevel::Debug => 1000, // Prefer CPU for debugging
                };

                // Scale threshold by GPU capabilities
                let capability_factor = (*compute_units as f64 / 16.0).clamp(0.5, 4.0);
                let memory_factor = (*memory_mb as f64 / 1024.0).clamp(0.5, 2.0);
                let adjusted_threshold =
                    (min_batch_size as f64 / (capability_factor * memory_factor)) as usize;

                batch_size >= adjusted_threshold
            }
            _ => false,
        }
    }

    /// Update verification level dynamically
    pub fn set_verification_level(&mut self, level: AdaptiveVerificationLevel) {
        // Update GPU verifier config if present
        if let Some(ref mut verifier) = self.boundary_verifier {
            let new_config = Self::create_gpu_verification_config(&self.platform, &level);
            *verifier = GpuBoundaryVerifier::new(new_config);
        }

        self.verification_level = level;
    }

    // Private implementation methods

    /// Detect current execution platform
    async fn detect_platform() -> Result<VerificationPlatform, AdaptiveVerificationError> {
        if std::env::var_os("AMARI_GPU_FORCE_CPU").is_some() {
            let features = Self::detect_cpu_features();
            return Ok(VerificationPlatform::NativeCpu { features });
        }

        // Try GPU detection with comprehensive error handling
        let gpu_platform = {
            // Use std::panic::catch_unwind to handle GPU driver panics
            let panic_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                // Use pollster to handle the async call safely
                pollster::block_on(async {
                    // Try full GPU initialization including capabilities detection
                    if GpuCliffordAlgebra::new::<3, 0, 0>().await.is_ok() {
                        let backend = Self::detect_gpu_backend();
                        let (memory_mb, compute_units) = Self::estimate_gpu_capabilities().await;
                        Some(VerificationPlatform::Gpu {
                            backend,
                            memory_mb,
                            compute_units,
                        })
                    } else {
                        None
                    }
                })
            }));

            // GPU initialization panicked or failed - gracefully fall back to CPU
            panic_result.unwrap_or(None)
        };

        if let Some(platform) = gpu_platform {
            return Ok(platform);
        }

        // Check for WASM environment
        if cfg!(target_arch = "wasm32") {
            let env = Self::detect_wasm_environment();
            return Ok(VerificationPlatform::Wasm { env });
        }

        // Default to native CPU
        let features = Self::detect_cpu_features();
        Ok(VerificationPlatform::NativeCpu { features })
    }

    /// Detect GPU backend type
    fn detect_gpu_backend() -> GpuBackend {
        // Platform-specific detection logic
        if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
            GpuBackend::Metal
        } else if cfg!(target_os = "windows") {
            GpuBackend::Dx12
        } else if cfg!(target_arch = "wasm32") {
            GpuBackend::WebGpu
        } else {
            GpuBackend::Vulkan
        }
    }

    /// Estimate GPU capabilities
    async fn estimate_gpu_capabilities() -> (u64, u32) {
        // Conservative estimates for broad compatibility
        // In production, these would query actual GPU capabilities
        (1024, 16) // 1GB memory, 16 compute units
    }

    /// Detect WASM execution environment
    fn detect_wasm_environment() -> WasmEnvironment {
        // Simplified detection - in practice would check JavaScript globals
        WasmEnvironment::Browser {
            engine: "Unknown".to_string(),
        }
    }

    /// Detect CPU features
    fn detect_cpu_features() -> CpuFeatures {
        CpuFeatures {
            supports_simd: true, // Assume SIMD support
            core_count: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            cache_size_kb: 8192, // 8MB L3 cache estimate
        }
    }

    /// Determine optimal verification level for platform
    fn determine_verification_level(platform: &VerificationPlatform) -> AdaptiveVerificationLevel {
        match platform {
            VerificationPlatform::NativeCpu { features } => {
                if features.core_count >= 8 {
                    AdaptiveVerificationLevel::High
                } else {
                    AdaptiveVerificationLevel::Balanced
                }
            }
            VerificationPlatform::Gpu { compute_units, .. } => {
                if *compute_units >= 32 {
                    AdaptiveVerificationLevel::Balanced
                } else {
                    AdaptiveVerificationLevel::Minimal
                }
            }
            VerificationPlatform::Wasm { .. } => {
                // WASM has limited debugging capabilities
                AdaptiveVerificationLevel::Minimal
            }
        }
    }

    /// Determine performance budget for platform
    fn determine_performance_budget(platform: &VerificationPlatform) -> Duration {
        match platform {
            VerificationPlatform::NativeCpu { .. } => Duration::from_millis(50),
            VerificationPlatform::Gpu { .. } => Duration::from_millis(20),
            VerificationPlatform::Wasm { .. } => Duration::from_millis(100),
        }
    }

    /// Create GPU verification configuration
    fn create_gpu_verification_config(
        platform: &VerificationPlatform,
        level: &AdaptiveVerificationLevel,
    ) -> VerificationConfig {
        let strategy = match level {
            AdaptiveVerificationLevel::Maximum => VerificationStrategy::Strict,
            AdaptiveVerificationLevel::High => {
                VerificationStrategy::Statistical { sample_rate: 0.2 }
            }
            AdaptiveVerificationLevel::Balanced => {
                VerificationStrategy::Statistical { sample_rate: 0.1 }
            }
            AdaptiveVerificationLevel::Minimal => VerificationStrategy::Boundary,
            AdaptiveVerificationLevel::Debug => VerificationStrategy::Strict,
        };

        let budget = Self::determine_performance_budget(platform);

        VerificationConfig {
            strategy,
            performance_budget: budget,
            tolerance: 1e-12,
            enable_invariant_checking: !matches!(level, AdaptiveVerificationLevel::Minimal),
        }
    }

    /// CPU verification implementation
    async fn cpu_verification<const P: usize, const Q: usize, const R: usize>(
        &self,
        a: &VerifiedMultivector<P, Q, R>,
        b: &VerifiedMultivector<P, Q, R>,
    ) -> Result<VerifiedMultivector<P, Q, R>, AdaptiveVerificationError> {
        // Full verification with phantom types
        let result = a.inner().geometric_product(b.inner());
        let verified_result = VerifiedMultivector::new(result);

        // Verify mathematical properties based on level
        match self.verification_level {
            AdaptiveVerificationLevel::Maximum | AdaptiveVerificationLevel::Debug => {
                verified_result.verify_invariants()?;
                // Additional checks for maximum verification
                self.verify_geometric_product_properties(a, b, &verified_result)?;
            }
            AdaptiveVerificationLevel::High => {
                verified_result.verify_invariants()?;
            }
            _ => {
                // Minimal verification
            }
        }

        Ok(verified_result)
    }

    /// CPU batch verification implementation
    async fn cpu_batch_verification<const P: usize, const Q: usize, const R: usize>(
        &self,
        a_batch: &[VerifiedMultivector<P, Q, R>],
        b_batch: &[VerifiedMultivector<P, Q, R>],
    ) -> Result<Vec<VerifiedMultivector<P, Q, R>>, AdaptiveVerificationError> {
        let mut results = Vec::with_capacity(a_batch.len());

        for (a, b) in a_batch.iter().zip(b_batch.iter()) {
            let result = self.cpu_verification(a, b).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// GPU batch verification implementation
    async fn gpu_batch_verification<const P: usize, const Q: usize, const R: usize>(
        &mut self,
        a_batch: &[VerifiedMultivector<P, Q, R>],
        b_batch: &[VerifiedMultivector<P, Q, R>],
    ) -> Result<Vec<VerifiedMultivector<P, Q, R>>, AdaptiveVerificationError> {
        if !self.should_use_gpu(a_batch.len()) {
            return self.cpu_batch_verification(a_batch, b_batch).await;
        }

        if let (Some(ref mut verifier), Some(ref gpu)) =
            (&mut self.boundary_verifier, &self.gpu_instance)
        {
            verifier
                .verified_batch_geometric_product(gpu, a_batch, b_batch)
                .await
                .map_err(AdaptiveVerificationError::GpuVerification)
        } else {
            // Fallback to CPU
            self.cpu_batch_verification(a_batch, b_batch).await
        }
    }

    /// WASM runtime verification implementation
    async fn wasm_runtime_verification<const P: usize, const Q: usize, const R: usize>(
        &self,
        a: &VerifiedMultivector<P, Q, R>,
        b: &VerifiedMultivector<P, Q, R>,
    ) -> Result<VerifiedMultivector<P, Q, R>, AdaptiveVerificationError> {
        // Runtime signature verification
        if VerifiedMultivector::<P, Q, R>::signature() != (P, Q, R) {
            return Err(AdaptiveVerificationError::GpuVerification(
                GpuVerificationError::SignatureMismatch {
                    expected: (P, Q, R),
                    actual: VerifiedMultivector::<P, Q, R>::signature(),
                },
            ));
        }

        // Perform operation with runtime checking
        let result = a.inner().geometric_product(b.inner());
        let verified_result = VerifiedMultivector::new(result);

        // Basic runtime validation
        if !verified_result.inner().magnitude().is_finite() {
            return Err(AdaptiveVerificationError::GpuVerification(
                GpuVerificationError::InvariantViolation {
                    invariant: "Result magnitude must be finite".to_string(),
                },
            ));
        }

        Ok(verified_result)
    }

    /// WASM batch verification implementation
    async fn wasm_batch_verification<const P: usize, const Q: usize, const R: usize>(
        &self,
        a_batch: &[VerifiedMultivector<P, Q, R>],
        b_batch: &[VerifiedMultivector<P, Q, R>],
    ) -> Result<Vec<VerifiedMultivector<P, Q, R>>, AdaptiveVerificationError> {
        let mut results = Vec::with_capacity(a_batch.len());

        for (a, b) in a_batch.iter().zip(b_batch.iter()) {
            let result = self.wasm_runtime_verification(a, b).await?;
            results.push(result);
        }

        Ok(results)
    }

    /// Verify geometric product mathematical properties
    fn verify_geometric_product_properties<const P: usize, const Q: usize, const R: usize>(
        &self,
        a: &VerifiedMultivector<P, Q, R>,
        b: &VerifiedMultivector<P, Q, R>,
        result: &VerifiedMultivector<P, Q, R>,
    ) -> Result<(), AdaptiveVerificationError> {
        // Verify magnitude inequality: |a * b| <= |a| * |b|
        let result_mag = result.inner().magnitude();
        let a_mag = a.inner().magnitude();
        let b_mag = b.inner().magnitude();

        if result_mag > a_mag * b_mag + 1e-12 {
            return Err(AdaptiveVerificationError::GpuVerification(
                GpuVerificationError::InvariantViolation {
                    invariant: format!(
                        "Magnitude inequality violated: {} > {} * {}",
                        result_mag, a_mag, b_mag
                    ),
                },
            ));
        }

        Ok(())
    }
}

/// Platform capabilities interface for adaptive optimization
pub trait PlatformCapabilities {
    /// Get maximum recommended batch size for the platform
    fn max_batch_size(&self) -> usize;

    /// Get optimal verification strategy for given workload
    fn optimal_strategy(&self, workload_size: usize) -> VerificationStrategy;

    /// Check if platform supports concurrent verification
    fn supports_concurrent_verification(&self) -> bool;

    /// Get platform-specific performance metrics
    fn performance_characteristics(&self) -> PlatformPerformanceProfile;
}

#[derive(Debug, Clone)]
pub struct PlatformPerformanceProfile {
    pub verification_overhead_percent: f64,
    pub memory_bandwidth_gbps: f64,
    pub compute_throughput_gflops: f64,
    pub latency_microseconds: f64,
}

impl PlatformCapabilities for VerificationPlatform {
    fn max_batch_size(&self) -> usize {
        match self {
            VerificationPlatform::NativeCpu { features } => features.core_count * 1000,
            VerificationPlatform::Gpu { memory_mb, .. } => {
                (*memory_mb as usize * 1024 * 1024) / (8 * 64) // Rough estimate
            }
            VerificationPlatform::Wasm { .. } => {
                10000 // Conservative for browser memory limits
            }
        }
    }

    fn optimal_strategy(&self, workload_size: usize) -> VerificationStrategy {
        match self {
            VerificationPlatform::NativeCpu { .. } => {
                if workload_size < 100 {
                    VerificationStrategy::Strict
                } else {
                    VerificationStrategy::Statistical { sample_rate: 0.1 }
                }
            }
            VerificationPlatform::Gpu { .. } => {
                if workload_size < 50 {
                    VerificationStrategy::Boundary
                } else {
                    VerificationStrategy::Statistical { sample_rate: 0.05 }
                }
            }
            VerificationPlatform::Wasm { .. } => {
                VerificationStrategy::Statistical { sample_rate: 0.02 }
            }
        }
    }

    fn supports_concurrent_verification(&self) -> bool {
        match self {
            VerificationPlatform::NativeCpu { features } => features.core_count > 1,
            VerificationPlatform::Gpu { .. } => true,
            VerificationPlatform::Wasm { .. } => false, // Limited by JS single-threading
        }
    }

    fn performance_characteristics(&self) -> PlatformPerformanceProfile {
        match self {
            VerificationPlatform::NativeCpu { features } => PlatformPerformanceProfile {
                verification_overhead_percent: 5.0,
                memory_bandwidth_gbps: 50.0,
                compute_throughput_gflops: features.core_count as f64 * 100.0,
                latency_microseconds: 1.0,
            },
            VerificationPlatform::Gpu { compute_units, .. } => PlatformPerformanceProfile {
                verification_overhead_percent: 15.0,
                memory_bandwidth_gbps: 200.0,
                compute_throughput_gflops: *compute_units as f64 * 50.0,
                latency_microseconds: 100.0,
            },
            VerificationPlatform::Wasm { .. } => PlatformPerformanceProfile {
                verification_overhead_percent: 25.0,
                memory_bandwidth_gbps: 10.0,
                compute_throughput_gflops: 10.0,
                latency_microseconds: 1000.0,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_features_detection() {
        let features = AdaptiveVerifier::detect_cpu_features();
        assert!(features.core_count > 0);
        assert!(features.cache_size_kb > 0);
    }

    #[test]
    fn test_verification_level_determination() {
        let cpu_platform = VerificationPlatform::NativeCpu {
            features: CpuFeatures {
                supports_simd: true,
                core_count: 8,
                cache_size_kb: 8192,
            },
        };

        let level = AdaptiveVerifier::determine_verification_level(&cpu_platform);
        assert_eq!(level, AdaptiveVerificationLevel::High);

        let gpu_platform = VerificationPlatform::Gpu {
            backend: GpuBackend::Vulkan,
            memory_mb: 2048,
            compute_units: 16,
        };

        let level = AdaptiveVerifier::determine_verification_level(&gpu_platform);
        assert_eq!(level, AdaptiveVerificationLevel::Minimal);
    }

    #[test]
    fn test_platform_capabilities() {
        let platform = VerificationPlatform::NativeCpu {
            features: CpuFeatures {
                supports_simd: true,
                core_count: 4,
                cache_size_kb: 8192,
            },
        };

        assert_eq!(platform.max_batch_size(), 4000);
        assert!(platform.supports_concurrent_verification());

        let profile = platform.performance_characteristics();
        assert_eq!(profile.verification_overhead_percent, 5.0);
        assert_eq!(profile.compute_throughput_gflops, 400.0);
    }

    #[tokio::test]
    #[ignore = "GPU hardware required, may fail in CI/CD environments"]
    async fn test_adaptive_verifier_creation() {
        // This test may fail in environments without GPU access
        match AdaptiveVerifier::new().await {
            Ok(verifier) => {
                assert!(verifier.performance_budget() > Duration::ZERO);
            }
            Err(AdaptiveVerificationError::PlatformDetection(_)) => {
                // Expected in limited environments
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_verification_with_config() {
        // Test adaptive behavior through the robust CPU fallback path; hardware GPU
        // probing is covered by ignored hardware tests because some headless EGL
        // stacks panic during adapter initialization.
        std::env::set_var("AMARI_GPU_FORCE_CPU", "1");
        match AdaptiveVerifier::with_config(
            AdaptiveVerificationLevel::Minimal,
            Duration::from_millis(5),
        )
        .await
        {
            Ok(verifier) => {
                // GPU or CPU succeeded - test functionality
                assert_eq!(
                    *verifier.verification_level(),
                    AdaptiveVerificationLevel::Minimal
                );
                assert_eq!(verifier.performance_budget(), Duration::from_millis(5));

                assert!(matches!(
                    verifier.platform(),
                    VerificationPlatform::NativeCpu { .. }
                ));
                println!("✅ CPU verification platform detected via forced fallback");
            }
            Err(e) => {
                std::env::remove_var("AMARI_GPU_FORCE_CPU");
                // Should not fail - adaptive design should always have a fallback
                panic!("Adaptive verifier should not fail, but got: {:?}", e);
            }
        }

        std::env::remove_var("AMARI_GPU_FORCE_CPU");
    }
}
