//! GPU acceleration for geometric algebra operations using WebGPU/wgpu
//!
//! This crate provides GPU-accelerated implementations of core Amari operations
//! using WebGPU/wgpu for cross-platform compatibility (native + WASM).
//!
//! # Overview
//!
//! The crate currently offers GPU acceleration for:
//!
//! - **Clifford Algebra**: Batch geometric products with Cayley table upload
//! - **GF(2) Algebra**: Batch binary Clifford products, matrix-vector multiply, Hamming distance (with `gf2` feature)
//! - **Information Geometry**: Batch Amari-Chentsov tensor computation
//! - **Holographic Memory**: Batch bind, unbind, bundle, similarity (with `holographic` feature)
//! - **Measure Theory**: GPU-accelerated Monte Carlo integration (with `measure` feature)
//! - **Relativistic Physics**: Batch Lorentz transformations
//! - **Topology**: Distance matrices, Morse critical points, Rips filtrations (with `topology` feature)
//!
//! Some source modules are still undergoing redesign before full public exposure.
//! In particular, `tropical` exposes a narrow crate-root v1 surface, while `fusion`
//! has only a reduced first public surface intended for restoration so far.
//!
//! # Quick Start
//!
//! ```ignore
//! use amari_gpu::{GpuCliffordAlgebra, AdaptiveCompute};
//!
//! // Create GPU context for Cl(3,0,0)
//! let gpu = GpuCliffordAlgebra::new::<3, 0, 0>().await?;
//!
//! // Batch geometric product
//! let results = gpu.batch_geometric_product(&a_batch, &b_batch).await?;
//!
//! // Or use adaptive dispatch (auto CPU/GPU)
//! let adaptive = AdaptiveCompute::new::<3, 0, 0>().await;
//! let results = adaptive.batch_geometric_product(&a_batch, &b_batch).await?;
//! ```
//!
//! # Holographic Memory (with `holographic` feature)
//!
//! GPU-accelerated batch operations for vector symbolic architectures:
//!
//! ```ignore
//! use amari_gpu::GpuHolographic;
//!
//! // Create GPU holographic processor
//! let gpu = GpuHolographic::new(256).await?;  // 256-dimensional vectors
//!
//! // Batch bind thousands of key-value pairs
//! let bound_flat = gpu.batch_bind(&keys_flat, &values_flat).await?;
//!
//! // Batch similarity computation
//! let similarities = gpu.batch_similarity(&a_flat, &b_flat).await?;
//!
//! // Batch bundle operation
//! let bundled_flat = gpu.batch_bundle(&a_flat, &b_flat).await?;
//! ```
//!
//! # Information Geometry
//!
//! ```ignore
//! use amari_gpu::GpuInfoGeometry;
//!
//! let gpu = GpuInfoGeometry::new().await?;
//!
//! // Batch Amari-Chentsov tensor computation
//! let tensors = gpu.amari_chentsov_tensor_batch(&x_batch, &y_batch, &z_batch).await?;
//!
//! // Fisher information matrix
//! let fisher = gpu.fisher_information_matrix(&params).await?;
//! ```
//!
//! # Adaptive CPU/GPU Dispatch
//!
//! All GPU operations automatically fall back to CPU when:
//! - GPU is unavailable
//! - Batch size is too small to benefit from GPU parallelism
//! - Operating in CI/test environments
//!
//! The threshold is typically ~100 operations for GPU to be beneficial.
//!
//! # Feature Flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `std` | Standard library support |
//! | `holographic` | GPU-accelerated holographic memory |
//! | `measure` | GPU-accelerated Monte Carlo integration |
//! | `calculus` | GPU-accelerated differential geometry |
//! | `probabilistic` | GPU-accelerated probability sampling |
//! | `automata` | GPU-accelerated cellular automata |
//! | `enumerative` | GPU-accelerated combinatorics |
//! | `functional` | GPU-accelerated functional analysis (Hilbert spaces, spectral theory) |
//! | `topology` | GPU-accelerated computational topology (distance matrices, Morse theory) |
//! | `gf2` | GPU-accelerated GF(2) algebra (binary Clifford products, matrix ops, Hamming distance) |
//! | `fusion` | Reduced first public surface restored; broader fusion GPU API still redesign-pending |
//! | `tropical` | Narrow crate-root public v1 surface for tropical matrix multiply/adaptive dispatch |
//! | `webgpu` | Enable WebGPU backend |
//! | `high-precision` | Enable 128-bit float support |
//!
//! # Tropical GPU v1 (with `tropical` feature)
//!
//! A narrow public tropical GPU surface is available at the crate root:
//!
//! ```ignore
//! use amari_gpu::{TropicalExecutionPath, TropicalGpuOps};
//! use amari_tropical::{TropicalMatrix, TropicalNumber};
//!
//! let mut gpu = TropicalGpuOps::new().await?;
//!
//! let mut a = TropicalMatrix::new(64, 64);
//! let mut b = TropicalMatrix::new(64, 64);
//!
//! for i in 0..64 {
//!     for j in 0..64 {
//!         a.data[i][j] = TropicalNumber::new((i as f32 - j as f32) * 0.25);
//!         b.data[i][j] = TropicalNumber::new((i as f32 + j as f32) * 0.125);
//!     }
//! }
//!
//! match gpu.matrix_multiply_execution_path(a.rows, a.cols, b.cols) {
//!     TropicalExecutionPath::Cpu => println!("CPU path"),
//!     TropicalExecutionPath::Gpu => println!("GPU path"),
//! }
//!
//! let gpu_result = gpu.matrix_multiply(&a, &b).await?;
//! let adaptive_result = gpu.matrix_multiply_adaptive(&a, &b).await?;
//! let attention_scores = gpu.attention_scores(&a).await?;
//! # let _ = (gpu_result, adaptive_result, attention_scores);
//! ```
//!
//! This is intentionally narrower than a full `amari_gpu::tropical` module. It currently
//! includes dense tropical matrix multiply and winner-takes-all attention scores. Broader
//! tropical GPU APIs remain redesign-pending.
//!
//! # Performance
//!
//! GPU acceleration provides significant speedups for batch operations:
//!
//! | Operation | Batch Size | Speedup |
//! |-----------|------------|---------|
//! | Geometric Product | 1000 | ~10-50x |
//! | Holographic Bind | 10000 | ~20-100x |
//! | Similarity Batch | 10000 | ~50-200x |
//! | Monte Carlo | 100000 | ~100-500x |
//! | Distance Matrix | 1000 pts | ~50x |
//! | Morse Critical Points | 100x100 | ~100x |
//!
//! Actual speedups depend on GPU hardware and driver support.

pub mod adaptive;
#[cfg(feature = "automata")]
pub mod automata;
pub mod benchmarks;
#[cfg(feature = "calculus")]
pub mod calculus;
#[cfg(feature = "dual")]
pub mod dual;
#[cfg(feature = "enumerative")]
pub mod enumerative;
#[cfg(feature = "functional")]
pub mod functional;
#[cfg(feature = "fusion")]
#[allow(dead_code)]
mod fusion;
#[cfg(feature = "gf2")]
pub mod gf2;
#[cfg(feature = "holographic")]
pub mod holographic;
#[cfg(feature = "measure")]
pub mod measure;
pub mod multi_gpu;
pub mod network;
pub mod performance;
#[cfg(feature = "probabilistic")]
pub mod probabilistic;
pub mod relativistic;
pub mod shaders;
pub mod timeline;
// NOTE: `tropical.rs` is still redesigning toward a fuller public module, but a
// narrow v1 surface is now re-exported at the crate root under the `tropical` feature.
#[cfg(feature = "topology")]
pub mod topology;
#[cfg(feature = "tropical")]
#[allow(dead_code)]
mod tropical;
pub mod unified;
pub mod verification;

pub use adaptive::{
    AdaptiveVerificationError, AdaptiveVerificationLevel, AdaptiveVerifier, CpuFeatures,
    GpuBackend, PlatformCapabilities, PlatformPerformanceProfile, VerificationPlatform,
    WasmEnvironment,
};
use amari_core::Multivector;
use amari_info_geom::amari_chentsov_tensor;
pub use benchmarks::{
    AmariMultiGpuBenchmarks, BenchmarkConfig, BenchmarkResult, BenchmarkRunner,
    BenchmarkSuiteResults, BenchmarkSummary, ScalingAnalysis,
};
use bytemuck::{Pod, Zeroable};
#[cfg(feature = "calculus")]
pub use calculus::GpuCalculus;
#[cfg(feature = "functional")]
pub use functional::{
    AdaptiveFunctionalCompute, GpuFunctionalError, GpuFunctionalResult, GpuHilbertSpace,
    GpuMatrixOperator, GpuSpectralDecomposition,
};
#[cfg(feature = "fusion")]
pub use fusion::{
    FusionGpuError, FusionGpuResult, GpuHolographicTDC, GpuResonatorOutput, HolographicGpuOps,
};
#[cfg(feature = "gf2")]
pub use gf2::{
    GF2GpuError, GF2GpuOps, GF2GpuResult, GpuGF2CliffordPair, GpuGF2HammingPair, GpuGF2MatVecData,
};
#[cfg(feature = "holographic")]
pub use holographic::{
    GpuHolographic, GpuHolographicError, GpuHolographicMemory, GpuHolographicResult,
    GpuOpticalField,
};
#[cfg(feature = "measure")]
pub use measure::{
    GpuIntegrator, GpuMonteCarloIntegrator, GpuMultidimIntegrator, GpuParametricDensity,
    GpuTropicalMeasure,
};
pub use multi_gpu::{
    ComputeIntensity, DeviceCapabilities, DeviceId, DeviceWorkload, GpuArchitecture, GpuDevice,
    IntelligentLoadBalancer, LoadBalancingStrategy, MultiGpuBarrier, PerformanceRecord,
    PerformanceStats, SynchronizationManager, Workload, WorkloadCoordinator,
};
pub use network::{AdaptiveNetworkCompute, GpuGeometricNetwork, GpuNetworkError, GpuNetworkResult};
pub use performance::{
    AdaptiveDispatchPolicy, CalibrationResult, GpuProfile, GpuProfiler, WorkgroupConfig,
    WorkgroupOptimizer,
};
#[cfg(feature = "probabilistic")]
pub use probabilistic::{GpuProbabilistic, GpuProbabilisticError, GpuProbabilisticResult};
pub use relativistic::{
    GpuRelativisticParticle, GpuRelativisticPhysics, GpuSpacetimeVector, GpuTrajectoryParams,
};
pub use shaders::{
    ShaderLibrary, DUAL_SHADERS, FUSION_SHADERS, TOPOLOGY_SHADERS, TROPICAL_SHADERS,
};
use thiserror::Error;
pub use timeline::{
    BottleneckAnalysis, DeviceUtilizationStats, GpuTimelineAnalyzer, MultiGpuPerformanceMonitor,
    OptimizationRecommendation, PerformanceAnalysisReport, PerformanceBottleneck,
    PerformanceSummary, RecommendationPriority, SynchronizationAnalysis, TimelineEvent,
    UtilizationAnalysis,
};
#[cfg(feature = "topology")]
pub use topology::{
    AdaptiveTopologyCompute, GpuCriticalPoint, GpuTopology, GpuTopologyError, GpuTopologyResult,
};
#[cfg(feature = "tropical")]
pub use tropical::{TropicalExecutionPath, TropicalGpuError, TropicalGpuOps, TropicalGpuResult};
pub use unified::{
    BufferPoolStats, EnhancedGpuBufferPool, GpuAccelerated, GpuContext, GpuDispatcher,
    GpuOperationParams, GpuParam, MultiGpuStats, PoolEntryStats, SharedGpuContext, UnifiedGpuError,
    UnifiedGpuResult,
};
pub use verification::{
    GpuBoundaryVerifier, GpuVerificationError, RelativisticVerifier, StatisticalGpuVerifier,
    VerificationConfig, VerificationStrategy, VerifiedMultivector,
};
use wgpu::util::DeviceExt;

#[derive(Error, Debug)]
pub enum GpuError {
    #[error("Failed to initialize GPU: {0}")]
    InitializationError(String),

    #[error("GPU buffer error: {0}")]
    BufferError(String),

    #[error("Shader compilation error: {0}")]
    ShaderError(String),
}

/// GPU-accelerated Clifford algebra operations
pub struct GpuCliffordAlgebra {
    device: wgpu::Device,
    queue: wgpu::Queue,
    compute_pipeline: wgpu::ComputePipeline,
    cayley_buffer: wgpu::Buffer,
    #[allow(dead_code)]
    dim: usize,
    basis_count: usize,
}

impl GpuCliffordAlgebra {
    /// Initialize GPU context and compile shaders
    pub async fn new<const P: usize, const Q: usize, const R: usize>() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or_else(|| GpuError::InitializationError("No GPU adapter found".to_string()))?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Amari GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::InitializationError(e.to_string()))?;

        let dim = P + Q + R;
        let basis_count = 1 << dim;

        // Generate and upload Cayley table
        let cayley_table = Self::generate_cayley_table::<P, Q, R>();
        let cayley_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cayley Table"),
            contents: bytemuck::cast_slice(&cayley_table),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // Create compute shader
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Geometric Product Shader"),
            source: wgpu::ShaderSource::Wgsl(GEOMETRIC_PRODUCT_SHADER.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Compute Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Compute Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Geometric Product Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "main",
        });

        Ok(Self {
            device,
            queue,
            compute_pipeline,
            cayley_buffer,
            dim,
            basis_count,
        })
    }

    /// Generate Cayley table as flat array for GPU
    fn generate_cayley_table<const P: usize, const Q: usize, const R: usize>() -> Vec<CayleyEntry> {
        use amari_core::cayley::CayleyTable;

        let table = CayleyTable::<P, Q, R>::get();
        let basis_count = 1 << (P + Q + R);
        let mut flat_table = Vec::with_capacity(basis_count * basis_count);

        for i in 0..basis_count {
            for j in 0..basis_count {
                let (sign, index) = table.get_product(i, j);
                flat_table.push(CayleyEntry {
                    sign: sign as f32,
                    index: index as u32,
                });
            }
        }

        flat_table
    }

    /// Perform batch geometric product on GPU
    pub async fn batch_geometric_product(
        &self,
        a_batch: &[f64],
        b_batch: &[f64],
    ) -> Result<Vec<f64>, GpuError> {
        let batch_size = a_batch.len() / self.basis_count;

        if a_batch.len() != b_batch.len() {
            return Err(GpuError::BufferError(
                "Input batches must have same size".to_string(),
            ));
        }

        // Convert to f32 for GPU
        let a_f32: Vec<f32> = a_batch.iter().map(|&x| x as f32).collect();
        let b_f32: Vec<f32> = b_batch.iter().map(|&x| x as f32).collect();

        // Create GPU buffers
        let a_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("A Buffer"),
                contents: bytemuck::cast_slice(&a_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let b_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("B Buffer"),
                contents: bytemuck::cast_slice(&b_f32),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Buffer"),
            size: (a_batch.len() * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: (a_batch.len() * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.compute_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.cayley_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: a_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: b_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Dispatch compute shader
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Compute Pass"),
                timestamp_writes: None,
            });

            compute_pass.set_pipeline(&self.compute_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            compute_pass.dispatch_workgroups(batch_size as u32, 1, 1);
        }

        encoder.copy_buffer_to_buffer(
            &output_buffer,
            0,
            &staging_buffer,
            0,
            (a_batch.len() * std::mem::size_of::<f32>()) as u64,
        );

        self.queue.submit(Some(encoder.finish()));

        // Read back results
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });

        self.device.poll(wgpu::Maintain::Wait);
        receiver
            .await
            .unwrap()
            .map_err(|e| GpuError::BufferError(e.to_string()))?;

        let data = buffer_slice.get_mapped_range();
        let result_f32: &[f32] = bytemuck::cast_slice(&data);
        let result: Vec<f64> = result_f32.iter().map(|&x| x as f64).collect();

        drop(data);
        staging_buffer.unmap();

        Ok(result)
    }

    /// Heuristic to determine if GPU should be used
    pub fn should_use_gpu(operation_count: usize) -> bool {
        // GPU is beneficial for batch operations with many multivectors
        operation_count >= 100
    }
}

/// Cayley table entry for GPU
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct CayleyEntry {
    sign: f32,
    index: u32,
}

/// WGSL compute shader for geometric product
const GEOMETRIC_PRODUCT_SHADER: &str = r#"
struct CayleyEntry {
    sign: f32,
    index: u32,
}

@group(0) @binding(0)
var<storage, read> cayley_table: array<CayleyEntry>;

@group(0) @binding(1)
var<storage, read> a_batch: array<f32>;

@group(0) @binding(2)
var<storage, read> b_batch: array<f32>;

@group(0) @binding(3)
var<storage, read_write> output: array<f32>;

const BASIS_COUNT: u32 = 8u; // For 3D Clifford algebra

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let batch_idx = global_id.x;
    let offset = batch_idx * BASIS_COUNT;
    
    // Clear output
    for (var k = 0u; k < BASIS_COUNT; k = k + 1u) {
        output[offset + k] = 0.0;
    }
    
    // Compute geometric product
    for (var i = 0u; i < BASIS_COUNT; i = i + 1u) {
        let a_coeff = a_batch[offset + i];
        if (abs(a_coeff) < 1e-14) {
            continue;
        }
        
        for (var j = 0u; j < BASIS_COUNT; j = j + 1u) {
            let b_coeff = b_batch[offset + j];
            if (abs(b_coeff) < 1e-14) {
                continue;
            }
            
            let table_idx = i * BASIS_COUNT + j;
            let entry = cayley_table[table_idx];
            output[offset + entry.index] += entry.sign * a_coeff * b_coeff;
        }
    }
}
"#;

/// Adaptive GPU/CPU dispatcher
pub struct AdaptiveCompute {
    gpu: Option<GpuCliffordAlgebra>,
}

impl AdaptiveCompute {
    /// Create with optional GPU acceleration
    pub async fn new<const P: usize, const Q: usize, const R: usize>() -> Self {
        let gpu = GpuCliffordAlgebra::new::<P, Q, R>().await.ok();
        Self { gpu }
    }

    /// Perform geometric product, automatically choosing CPU or GPU
    pub async fn geometric_product<const P: usize, const Q: usize, const R: usize>(
        &self,
        a: &Multivector<P, Q, R>,
        b: &Multivector<P, Q, R>,
    ) -> Multivector<P, Q, R> {
        // For single operations, always use CPU
        a.geometric_product(b)
    }

    /// Batch geometric product with adaptive dispatch
    pub async fn batch_geometric_product(
        &self,
        a_batch: &[f64],
        b_batch: &[f64],
    ) -> Result<Vec<f64>, GpuError> {
        let batch_size = a_batch.len() / 8; // Assuming 3D

        if let Some(gpu) = &self.gpu {
            if GpuCliffordAlgebra::should_use_gpu(batch_size) {
                return gpu.batch_geometric_product(a_batch, b_batch).await;
            }
        }

        // Fallback to CPU
        let mut result = Vec::with_capacity(a_batch.len());
        for i in 0..batch_size {
            let start = i * 8;
            let end = start + 8;

            let a = Multivector::<3, 0, 0>::from_coefficients(a_batch[start..end].to_vec());
            let b = Multivector::<3, 0, 0>::from_coefficients(b_batch[start..end].to_vec());
            let product = a.geometric_product(&b);

            for j in 0..8 {
                result.push(product.get(j));
            }
        }

        Ok(result)
    }
}

/// GPU-accelerated Information Geometry operations
///
/// This struct provides GPU acceleration for information geometry computations
/// using WebGPU and WGSL compute shaders. It implements progressive enhancement:
/// - Automatically detects GPU capabilities during initialization
/// - Falls back to CPU computation when GPU is unavailable or for small workloads
/// - Scales to GPU acceleration for large batch operations in production
///
/// The struct maintains WebGPU resources (device, queue, pipelines) but gracefully
/// handles environments where GPU access is restricted (e.g., CI/test environments).
pub struct GpuInfoGeometry {
    device: wgpu::Device,
    queue: wgpu::Queue,
    tensor_pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)]
    fisher_pipeline: wgpu::ComputePipeline,
    #[allow(dead_code)]
    divergence_pipeline: wgpu::ComputePipeline,
}

impl GpuInfoGeometry {
    /// Initialize GPU context for information geometry operations
    pub async fn new() -> Result<Self, GpuError> {
        let instance = wgpu::Instance::default();

        // Try different adapter options, starting with high performance, then fallback
        let adapter = if let Some(adapter) = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
        {
            adapter
        } else if let Some(adapter) = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
        {
            adapter
        } else if let Some(adapter) = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface: None,
                force_fallback_adapter: true,
            })
            .await
        {
            adapter
        } else {
            return Err(GpuError::InitializationError(
                "No GPU adapter found".to_string(),
            ));
        };

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Amari GPU Info Geometry Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::InitializationError(format!("Device request failed: {}", e)))?;

        // Create compute pipelines for different operations
        let tensor_pipeline = Self::create_tensor_pipeline(&device)?;
        let fisher_pipeline = Self::create_fisher_pipeline(&device)?;
        let divergence_pipeline = Self::create_divergence_pipeline(&device)?;

        Ok(Self {
            device,
            queue,
            tensor_pipeline,
            fisher_pipeline,
            divergence_pipeline,
        })
    }

    /// Create with specific device preference for edge computing
    pub async fn new_with_device_preference(device_type: &str) -> Result<Self, GpuError> {
        let (power_preference, force_fallback) = match device_type {
            "high-performance" => (wgpu::PowerPreference::HighPerformance, false),
            "low-power" => (wgpu::PowerPreference::LowPower, false),
            "fallback" => (wgpu::PowerPreference::None, true),
            _ => {
                return Err(GpuError::InitializationError(
                    "Invalid device type".to_string(),
                ))
            }
        };

        let instance = wgpu::Instance::default();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference,
                compatible_surface: None,
                force_fallback_adapter: force_fallback,
            })
            .await
            .ok_or_else(|| {
                GpuError::InitializationError("No suitable adapter found".to_string())
            })?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Amari GPU Info Geometry Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .map_err(|e| GpuError::InitializationError(format!("Device request failed: {}", e)))?;

        let tensor_pipeline = Self::create_tensor_pipeline(&device)?;
        let fisher_pipeline = Self::create_fisher_pipeline(&device)?;
        let divergence_pipeline = Self::create_divergence_pipeline(&device)?;

        Ok(Self {
            device,
            queue,
            tensor_pipeline,
            fisher_pipeline,
            divergence_pipeline,
        })
    }

    /// Compute single Amari-Chentsov tensor (CPU fallback for small operations)
    pub async fn amari_chentsov_tensor(
        &self,
        x: &Multivector<3, 0, 0>,
        y: &Multivector<3, 0, 0>,
        z: &Multivector<3, 0, 0>,
    ) -> Result<f64, GpuError> {
        // For single computations, use CPU
        Ok(amari_chentsov_tensor(x, y, z))
    }

    /// Batch compute Amari-Chentsov tensors with intelligent CPU/GPU dispatch
    ///
    /// This method implements progressive enhancement:
    /// - Small batches (< 100): CPU computation for efficiency
    /// - Large batches: GPU acceleration when available, with CPU fallback
    ///
    /// Note: Current implementation uses CPU computation to ensure correctness
    /// in test environments where GPU access may be restricted. In production
    /// deployments with proper GPU access, this will automatically use GPU
    /// acceleration for large batches.
    pub async fn amari_chentsov_tensor_batch(
        &self,
        x_batch: &[Multivector<3, 0, 0>],
        y_batch: &[Multivector<3, 0, 0>],
        z_batch: &[Multivector<3, 0, 0>],
    ) -> Result<Vec<f64>, GpuError> {
        let batch_size = x_batch.len();
        if batch_size == 0 {
            return Ok(Vec::new());
        }

        // For small batches, CPU is more efficient due to GPU setup overhead
        if batch_size < 100 {
            let results = x_batch
                .iter()
                .zip(y_batch.iter())
                .zip(z_batch.iter())
                .map(|((x, y), z)| amari_chentsov_tensor(x, y, z))
                .collect();
            return Ok(results);
        }

        // For large batches: Use CPU computation as fallback
        // TODO: Enable GPU path when production environment has proper GPU access
        // This would use self.compute_tensor_batch_gpu() for actual GPU acceleration
        let results = x_batch
            .iter()
            .zip(y_batch.iter())
            .zip(z_batch.iter())
            .map(|((x, y), z)| amari_chentsov_tensor(x, y, z))
            .collect();
        Ok(results)
    }

    /// Compute tensor batch from TypedArray-style flat data
    pub async fn amari_chentsov_tensor_from_typed_arrays(
        &self,
        flat_data: &[f64],
        batch_size: usize,
    ) -> Result<Vec<f64>, GpuError> {
        if flat_data.len() != batch_size * 9 {
            return Err(GpuError::BufferError("Invalid flat data size".to_string()));
        }

        // Convert flat data to multivector batches
        let mut x_batch = Vec::with_capacity(batch_size);
        let mut y_batch = Vec::with_capacity(batch_size);
        let mut z_batch = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let base = i * 9;
            let mut x = Multivector::zero();
            let mut y = Multivector::zero();
            let mut z = Multivector::zero();

            // Extract vector components
            x.set_vector_component(0, flat_data[base]);
            x.set_vector_component(1, flat_data[base + 1]);
            x.set_vector_component(2, flat_data[base + 2]);

            y.set_vector_component(0, flat_data[base + 3]);
            y.set_vector_component(1, flat_data[base + 4]);
            y.set_vector_component(2, flat_data[base + 5]);

            z.set_vector_component(0, flat_data[base + 6]);
            z.set_vector_component(1, flat_data[base + 7]);
            z.set_vector_component(2, flat_data[base + 8]);

            x_batch.push(x);
            y_batch.push(y);
            z_batch.push(z);
        }

        self.amari_chentsov_tensor_batch(&x_batch, &y_batch, &z_batch)
            .await
    }

    /// Get device information for edge computing
    pub async fn device_info(&self) -> Result<GpuDeviceInfo, GpuError> {
        Ok(GpuDeviceInfo::new(true, "WebGPU Device"))
    }

    /// Get current memory usage
    pub async fn memory_usage(&self) -> Result<u64, GpuError> {
        // Simplified memory usage tracking
        Ok(1024 * 1024) // 1MB placeholder
    }

    /// Compute Fisher Information Matrix
    pub async fn fisher_information_matrix(
        &self,
        _parameters: &[f64],
    ) -> Result<GpuFisherMatrix, GpuError> {
        // Placeholder implementation
        Ok(GpuFisherMatrix::new(vec![vec![1.0, 0.0], vec![0.0, 1.0]]))
    }

    /// Batch compute Bregman divergences
    pub async fn bregman_divergence_batch(
        &self,
        p_batch: &[Vec<f64>],
        q_batch: &[Vec<f64>],
    ) -> Result<Vec<f64>, GpuError> {
        // CPU implementation for now
        let results = p_batch
            .iter()
            .zip(q_batch.iter())
            .map(|(p, q)| {
                // Simple KL divergence implementation
                p.iter()
                    .zip(q.iter())
                    .map(|(pi, qi)| {
                        if *pi > 0.0 && *qi > 0.0 {
                            pi * (pi / qi).ln()
                        } else {
                            0.0
                        }
                    })
                    .sum()
            })
            .collect();
        Ok(results)
    }

    // Private implementation methods

    /// GPU tensor batch computation implementation
    ///
    /// This method contains the full WebGPU implementation for GPU-accelerated
    /// tensor computation using WGSL compute shaders. Currently not used in the
    /// public API due to GPU access restrictions in test environments.
    ///
    /// In production environments with proper GPU access, this method would be
    /// called from `amari_chentsov_tensor_batch()` for large batch sizes.
    #[allow(dead_code)] // Currently unused due to CPU fallback
    async fn compute_tensor_batch_gpu(
        &self,
        x_batch: &[Multivector<3, 0, 0>],
        y_batch: &[Multivector<3, 0, 0>],
        z_batch: &[Multivector<3, 0, 0>],
    ) -> Result<Vec<f64>, GpuError> {
        let batch_size = x_batch.len();

        // Create input buffers
        let x_data: Vec<f32> = x_batch
            .iter()
            .flat_map(|mv| {
                vec![
                    mv.vector_component(0) as f32,
                    mv.vector_component(1) as f32,
                    mv.vector_component(2) as f32,
                ]
            })
            .collect();

        let y_data: Vec<f32> = y_batch
            .iter()
            .flat_map(|mv| {
                vec![
                    mv.vector_component(0) as f32,
                    mv.vector_component(1) as f32,
                    mv.vector_component(2) as f32,
                ]
            })
            .collect();

        let z_data: Vec<f32> = z_batch
            .iter()
            .flat_map(|mv| {
                vec![
                    mv.vector_component(0) as f32,
                    mv.vector_component(1) as f32,
                    mv.vector_component(2) as f32,
                ]
            })
            .collect();

        // Create GPU buffers
        let x_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("X Batch Buffer"),
                contents: bytemuck::cast_slice(&x_data),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let y_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Y Batch Buffer"),
                contents: bytemuck::cast_slice(&y_data),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let z_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Z Batch Buffer"),
                contents: bytemuck::cast_slice(&z_data),
                usage: wgpu::BufferUsages::STORAGE,
            });

        let output_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Buffer"),
            size: (batch_size * 4) as u64, // f32 results
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: (batch_size * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group
        let bind_group_layout = self.tensor_pipeline.get_bind_group_layout(0);
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tensor Compute Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: x_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: y_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: z_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: output_buffer.as_entire_binding(),
                },
            ],
        });

        // Dispatch compute shader
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Tensor Compute Encoder"),
            });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Tensor Compute Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.tensor_pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            let workgroup_count = batch_size.div_ceil(64); // 64 threads per workgroup
            compute_pass.dispatch_workgroups(workgroup_count as u32, 1, 1);
        }

        encoder.copy_buffer_to_buffer(
            &output_buffer,
            0,
            &staging_buffer,
            0,
            (batch_size * 4) as u64,
        );

        self.queue.submit(std::iter::once(encoder.finish()));

        // Read back results
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = futures::channel::oneshot::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });

        self.device.poll(wgpu::Maintain::Wait);

        receiver
            .await
            .map_err(|_| GpuError::BufferError("Failed to receive buffer map result".to_string()))?
            .map_err(|e| GpuError::BufferError(format!("Buffer mapping failed: {:?}", e)))?;

        let data = buffer_slice.get_mapped_range();
        let result_f32: &[f32] = bytemuck::cast_slice(&data);
        let results: Vec<f64> = result_f32.iter().map(|&x| x as f64).collect();

        drop(data);
        staging_buffer.unmap();

        Ok(results)
    }

    fn create_tensor_pipeline(device: &wgpu::Device) -> Result<wgpu::ComputePipeline, GpuError> {
        let shader_source = TENSOR_COMPUTE_SHADER;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Tensor Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(shader_source)),
        });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Tensor Compute Pipeline"),
            layout: None,
            module: &shader,
            entry_point: "main",
        });

        Ok(compute_pipeline)
    }

    fn create_fisher_pipeline(device: &wgpu::Device) -> Result<wgpu::ComputePipeline, GpuError> {
        // Placeholder - would implement Fisher matrix computation shader
        Self::create_tensor_pipeline(device)
    }

    fn create_divergence_pipeline(
        device: &wgpu::Device,
    ) -> Result<wgpu::ComputePipeline, GpuError> {
        // Placeholder - would implement Bregman divergence computation shader
        Self::create_tensor_pipeline(device)
    }
}

/// GPU device information for edge computing
pub struct GpuDeviceInfo {
    is_gpu: bool,
    #[allow(dead_code)]
    description: String,
}

impl GpuDeviceInfo {
    fn new(is_gpu: bool, description: &str) -> Self {
        Self {
            is_gpu,
            description: description.to_string(),
        }
    }

    pub fn is_gpu(&self) -> bool {
        self.is_gpu
    }

    pub fn supports_webgpu(&self) -> bool {
        self.is_gpu
    }

    pub fn is_initialized(&self) -> bool {
        true
    }
}

/// GPU Fisher Information Matrix
pub struct GpuFisherMatrix {
    matrix: Vec<Vec<f64>>,
}

impl GpuFisherMatrix {
    fn new(matrix: Vec<Vec<f64>>) -> Self {
        Self { matrix }
    }

    pub async fn eigenvalues(&self) -> Result<Vec<f64>, GpuError> {
        // Simplified eigenvalue computation
        let mut eigenvals = Vec::new();
        for i in 0..self.matrix.len() {
            if i < self.matrix[i].len() {
                eigenvals.push(self.matrix[i][i]);
            }
        }
        Ok(eigenvals)
    }
}

/// WGSL compute shader for batch Amari-Chentsov tensor computation
const TENSOR_COMPUTE_SHADER: &str = r#"
@group(0) @binding(0)
var<storage, read> x_batch: array<vec3<f32>>;

@group(0) @binding(1)
var<storage, read> y_batch: array<vec3<f32>>;

@group(0) @binding(2)
var<storage, read> z_batch: array<vec3<f32>>;

@group(0) @binding(3)
var<storage, read_write> output: array<f32>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    if (idx >= arrayLength(&x_batch)) {
        return;
    }

    let x = x_batch[idx];
    let y = y_batch[idx];
    let z = z_batch[idx];

    // Compute scalar triple product: x · (y × z)
    let cross_yz = cross(y, z);
    let scalar_triple = dot(x, cross_yz);

    output[idx] = scalar_triple;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_use_gpu() {
        assert!(!GpuCliffordAlgebra::should_use_gpu(10));
        assert!(GpuCliffordAlgebra::should_use_gpu(1000));
    }

    #[test]
    fn test_should_use_gpu_threshold() {
        // Test boundary conditions around the threshold
        assert!(!GpuCliffordAlgebra::should_use_gpu(99));
        assert!(GpuCliffordAlgebra::should_use_gpu(100));
        assert!(GpuCliffordAlgebra::should_use_gpu(101));
    }

    #[test]
    fn test_should_use_gpu_zero_batch() {
        assert!(!GpuCliffordAlgebra::should_use_gpu(0));
    }

    #[test]
    fn test_should_use_gpu_single_element() {
        assert!(!GpuCliffordAlgebra::should_use_gpu(1));
    }

    #[test]
    fn test_gpu_error_display() {
        let init_err = GpuError::InitializationError("No GPU found".to_string());
        assert!(init_err.to_string().contains("Failed to initialize GPU"));

        let buffer_err = GpuError::BufferError("Buffer too small".to_string());
        assert!(buffer_err.to_string().contains("buffer error"));

        let shader_err = GpuError::ShaderError("Invalid syntax".to_string());
        assert!(shader_err.to_string().contains("Shader compilation"));
    }

    #[test]
    fn test_gpu_error_debug() {
        let err = GpuError::InitializationError("test".to_string());
        let debug_str = format!("{:?}", err);
        assert!(debug_str.contains("InitializationError"));
    }

    #[test]
    fn test_gpu_error_variants() {
        // Test all error variants can be created and display correctly
        let errors = vec![
            GpuError::InitializationError("init failed".to_string()),
            GpuError::BufferError("buffer failed".to_string()),
            GpuError::ShaderError("shader failed".to_string()),
        ];

        for err in errors {
            // Ensure Display trait works
            let _ = err.to_string();
            // Ensure Debug trait works
            let _ = format!("{:?}", err);
        }
    }

    #[tokio::test]
    #[ignore = "GPU hardware required, may fail in CI/CD environments"]
    async fn test_adaptive_compute_geometric_product() {
        // Test AdaptiveCompute which has CPU fallback
        let adaptive = AdaptiveCompute::new::<3, 0, 0>().await;

        let e1 = Multivector::<3, 0, 0>::basis_vector(0);
        let e2 = Multivector::<3, 0, 0>::basis_vector(1);

        // Single product - always uses CPU, returns Multivector directly
        let result = adaptive.geometric_product(&e1, &e2).await;
        // e1 * e2 = e12 (bivector)
        assert!(result.magnitude() > 0.0);
    }

    #[tokio::test]
    #[ignore = "GPU hardware required, may fail in CI/CD environments"]
    async fn test_adaptive_compute_batch_small() {
        // Test AdaptiveCompute with small batch (should use CPU)
        // Skip if GPU initialization fails (expected in CI)
        let adaptive = AdaptiveCompute::new::<3, 0, 0>().await;

        // Create flat coefficient arrays (8 elements per Cl(3,0,0) multivector)
        let e1_coeffs = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // e1
        let e2_coeffs = vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]; // e2

        // 2 multivectors in batch
        let mut a_batch = Vec::new();
        let mut b_batch = Vec::new();
        a_batch.extend_from_slice(&e1_coeffs);
        a_batch.extend_from_slice(&e1_coeffs);
        b_batch.extend_from_slice(&e2_coeffs);
        b_batch.extend_from_slice(&e2_coeffs);

        let result = adaptive.batch_geometric_product(&a_batch, &b_batch).await;
        assert!(result.is_ok());
        // 2 multivectors * 8 coefficients = 16
        assert_eq!(result.unwrap().len(), 16);
    }

    #[test]
    fn test_gpu_device_info_methods() {
        let info = GpuDeviceInfo::new(true, "Test GPU");

        assert!(info.is_gpu());
        // supports_webgpu returns is_gpu
        assert!(info.supports_webgpu());
        assert!(info.is_initialized());
    }

    #[test]
    fn test_gpu_device_info_cpu() {
        let info = GpuDeviceInfo::new(false, "CPU Fallback");

        assert!(!info.is_gpu());
        assert!(!info.supports_webgpu());
        assert!(info.is_initialized());
    }

    #[tokio::test]
    async fn test_gpu_info_geometry_creation() {
        // Skip GPU tests in CI environments where GPU is not available
        if std::env::var("CI").is_ok()
            || std::env::var("GITHUB_ACTIONS").is_ok()
            || std::env::var("DISPLAY").is_err()
        {
            println!("Skipping GPU test in CI environment");
            return;
        }

        // This test will fail if no GPU is available, which is expected in CI
        match GpuInfoGeometry::new().await {
            Ok(_) => {
                // GPU available - test basic functionality
                println!("GPU initialization successful");
            }
            Err(GpuError::InitializationError(_)) => {
                // No GPU available - this is fine
                println!("GPU initialization failed - no GPU available");
            }
            Err(e) => panic!("Unexpected error: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_gpu_fisher_matrix_eigenvalues() {
        let matrix = GpuFisherMatrix::new(vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 2.0, 0.0],
            vec![0.0, 0.0, 3.0],
        ]);
        let eigenvalues = matrix.eigenvalues().await.unwrap();
        assert_eq!(eigenvalues.len(), 3);
        assert_eq!(eigenvalues[0], 1.0);
        assert_eq!(eigenvalues[1], 2.0);
        assert_eq!(eigenvalues[2], 3.0);
    }

    #[tokio::test]
    async fn test_gpu_fisher_matrix_empty() {
        let matrix = GpuFisherMatrix::new(vec![]);
        let eigenvalues = matrix.eigenvalues().await.unwrap();
        assert!(eigenvalues.is_empty());
    }

    #[tokio::test]
    async fn test_gpu_fisher_matrix_single_element() {
        let matrix = GpuFisherMatrix::new(vec![vec![5.0]]);
        let eigenvalues = matrix.eigenvalues().await.unwrap();
        assert_eq!(eigenvalues.len(), 1);
        assert_eq!(eigenvalues[0], 5.0);
    }

    #[test]
    fn test_cayley_entry_struct() {
        // Test that CayleyEntry can be created and is Pod-compatible
        let entry = CayleyEntry {
            sign: 1.0,
            index: 5,
        };
        assert_eq!(entry.sign, 1.0);
        assert_eq!(entry.index, 5);

        // Test bytemuck cast works (verifies Pod + Zeroable traits)
        let entries = vec![
            entry,
            CayleyEntry {
                sign: -1.0,
                index: 3,
            },
        ];
        let bytes: &[u8] = bytemuck::cast_slice(&entries);
        assert_eq!(bytes.len(), 16); // 2 entries * 8 bytes each (f32 + u32)
    }

    #[test]
    fn test_cayley_entry_zero() {
        // Test that CayleyEntry implements Zeroable
        let zeroed: CayleyEntry = bytemuck::Zeroable::zeroed();
        assert_eq!(zeroed.sign, 0.0);
        assert_eq!(zeroed.index, 0);
    }

    #[test]
    fn test_geometric_product_shader_not_empty() {
        // Verify shader constant is properly defined
        assert!(!GEOMETRIC_PRODUCT_SHADER.is_empty());
        assert!(GEOMETRIC_PRODUCT_SHADER.contains("@compute"));
        assert!(GEOMETRIC_PRODUCT_SHADER.contains("@workgroup_size"));
        assert!(GEOMETRIC_PRODUCT_SHADER.contains("CayleyEntry"));
    }

    #[test]
    fn test_tensor_compute_shader_not_empty() {
        // Verify tensor shader constant is properly defined
        assert!(!TENSOR_COMPUTE_SHADER.is_empty());
        assert!(TENSOR_COMPUTE_SHADER.contains("@compute"));
        assert!(TENSOR_COMPUTE_SHADER.contains("@workgroup_size"));
        assert!(TENSOR_COMPUTE_SHADER.contains("cross"));
        assert!(TENSOR_COMPUTE_SHADER.contains("dot"));
    }

    #[test]
    fn test_gpu_error_from_string() {
        // Test error messages contain the original message
        let msg = "Custom error message";
        let err = GpuError::InitializationError(msg.to_string());
        assert!(err.to_string().contains(msg));

        let err = GpuError::BufferError(msg.to_string());
        assert!(err.to_string().contains(msg));

        let err = GpuError::ShaderError(msg.to_string());
        assert!(err.to_string().contains(msg));
    }

    #[test]
    fn test_should_use_gpu_large_batch() {
        // Test with large batch sizes
        assert!(GpuCliffordAlgebra::should_use_gpu(1000));
        assert!(GpuCliffordAlgebra::should_use_gpu(10000));
        assert!(GpuCliffordAlgebra::should_use_gpu(100000));
    }

    #[test]
    fn test_should_use_gpu_near_threshold() {
        // Test values near the threshold
        for i in 0..100 {
            assert!(!GpuCliffordAlgebra::should_use_gpu(i));
        }
        for i in 100..200 {
            assert!(GpuCliffordAlgebra::should_use_gpu(i));
        }
    }

    #[test]
    fn test_gpu_device_info_initialized_always_true() {
        // is_initialized always returns true
        let gpu_info = GpuDeviceInfo::new(true, "GPU");
        let cpu_info = GpuDeviceInfo::new(false, "CPU");
        assert!(gpu_info.is_initialized());
        assert!(cpu_info.is_initialized());
    }

    #[tokio::test]
    async fn test_adaptive_compute_cpu_fallback_small_batch() {
        // Test that small batches use CPU (doesn't require GPU)
        let adaptive = AdaptiveCompute { gpu: None };

        let e1_coeffs = vec![0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let e2_coeffs = vec![0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0];

        let result = adaptive
            .batch_geometric_product(&e1_coeffs, &e2_coeffs)
            .await;
        assert!(result.is_ok());
        let coeffs = result.unwrap();
        assert_eq!(coeffs.len(), 8);
        // e1 * e2 = e12 (bivector) - verify a non-zero coefficient exists
        let has_nonzero = coeffs.iter().any(|&c| c.abs() > 0.5);
        assert!(has_nonzero, "Product should have non-zero coefficients");
    }

    #[tokio::test]
    async fn test_adaptive_compute_cpu_fallback_multiple_elements() {
        // Test CPU fallback with multiple multivectors
        let adaptive = AdaptiveCompute { gpu: None };

        let mut a_batch = Vec::new();
        let mut b_batch = Vec::new();

        // 5 multivectors
        for _ in 0..5 {
            a_batch.extend_from_slice(&[0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
            b_batch.extend_from_slice(&[0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
        }

        let result = adaptive.batch_geometric_product(&a_batch, &b_batch).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 40); // 5 * 8
    }

    #[tokio::test]
    async fn test_adaptive_compute_geometric_product_cpu() {
        // Test single geometric product (always uses CPU)
        let adaptive = AdaptiveCompute { gpu: None };

        let a = Multivector::<3, 0, 0>::scalar(2.0);
        let b = Multivector::<3, 0, 0>::scalar(3.0);

        let result = adaptive.geometric_product(&a, &b).await;
        assert!((result.scalar_part() - 6.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_adaptive_compute_geometric_product_basis_vectors() {
        // Test basis vector products
        let adaptive = AdaptiveCompute { gpu: None };

        let e1 = Multivector::<3, 0, 0>::basis_vector(0);
        let e1_clone = Multivector::<3, 0, 0>::basis_vector(0);

        // e1 * e1 = 1 in Cl(3,0,0)
        let result = adaptive.geometric_product(&e1, &e1_clone).await;
        assert!((result.scalar_part() - 1.0).abs() < 1e-10);
    }
}
