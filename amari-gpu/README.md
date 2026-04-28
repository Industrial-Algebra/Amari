# amari-gpu

GPU acceleration for Amari mathematical computations using WebGPU.

## Overview

`amari-gpu` is an integration crate that provides GPU-accelerated implementations of mathematical operations from Amari domain crates. It follows the **progressive enhancement** pattern: operations automatically fall back to CPU computation when GPU is unavailable or for small workloads, scaling to GPU acceleration for large batch operations in production.

## Architecture

As an **integration crate**, `amari-gpu` consumes APIs from domain crates and exposes them to GPU platforms:

```
Domain Crates (provide APIs):
  amari-core → amari-measure → amari-calculus
  amari-info-geom, amari-relativistic, amari-network

Integration Crates (consume APIs):
  amari-gpu → depends on domain crates
  amari-wasm → depends on domain crates
```

**Dependency Rule**: Integration crates depend on domain crates, never the reverse.

## Current Integrations (v0.19.1)

### Implemented GPU Acceleration

| Domain Crate | Module | Operations | Status |
|-------------|--------|------------|--------|
| **amari-core** | crate root | `GpuCliffordAlgebra` GPU batch geometric products; `AdaptiveCompute` Cl(3,0,0) CPU/GPU helper | ⚠️ GPU-backed core v1 with public baseline tests |
| **amari-info-geom** | crate root | `GpuInfoGeometry` CPU-baseline Amari-Chentsov, Fisher, and KL/Bregman operations after GPU context creation | ⚠️ GPU-ready CPU-baseline v1 |
| **amari-relativistic** | `relativistic` | Minkowski norm-squared and simplified geodesic propagation | ⚠️ GPU-backed narrow v1 |
| **amari-network** | `network` | Narrow GPU vector-distance path with mixed centrality/clustering | ⚠️ GPU-backed for vector-only `Cl(P,0,0)`, `P <= 3` |
| **amari-measure** | `measure` | 1D integration, Monte Carlo, Gaussian densities, tropical/multidim scaffolding | ⚠️ Mixed GPU-backed + documented CPU fallback (feature: `measure`) |
| **amari-calculus** | `calculus` | Field evaluation, gradients, divergence, curl | ⚠️ GPU-ready CPU-semantic fallback (feature: `calculus`) |
| **amari-dual** | `dual` | Narrow GPU-backed unary forward-AD v1 surface | ⚠️ Narrow v1 restored; broader gradients/training private/redesign-pending (feature: `dual`) |
| **amari-enumerative** | `enumerative` | High-use GPU kernels for WDVV, matroids, localization, CSM, operad, stability | ⚠️ Broad GPU-backed surface with representative public tests (feature: `enumerative`) |
| **amari-automata** | `automata` | GPU rule application/energy kernels, CPU neighborhood fallback | ⚠️ Mixed GPU-backed + documented CPU fallback (feature: `automata`) |
| **amari-fusion** | `fusion` | Reduced first public surface for holographic/fusion-derived GPU operations | ⚠️ Partially restored; broader fusion GPU API still under redesign |
| **amari-holographic** | `holographic` | Holographic memory, batch binding, similarity matrices, **optical field operations** | ✅ Implemented (feature: `holographic`) |
| **amari-probabilistic** | `probabilistic` | Gaussian sampling, batch statistics, Monte Carlo | ✅ Implemented (feature: `probabilistic`) |
| **amari-functional** | `functional` | GPU matrix batch ops, Hilbert batches, CPU spectral/fallback paths | ⚠️ Mixed GPU-backed + documented CPU fallback (feature: `functional`) |
| **amari-topology** | `topology` | GPU distance/Morse kernels, CPU Rips/Betti fallback paths | ⚠️ Mixed GPU-backed + documented CPU fallback (feature: `topology`) |

### Current Hardware Validation

Focused hardware validation has completed on both DGX Spark / NVIDIA GB10 and NVIDIA GeForce RTX 5080 Laptop GPU.
See:

- `docs/roadmap/AMARI_GPU_GB10_HARDWARE_VALIDATION.md`
- `docs/roadmap/AMARI_GPU_RTX5080_HARDWARE_VALIDATION.md`

Current status:

- Focused public API tests passed for default/core/info-geometry, network, relativistic, holographic, tropical, fusion, calculus, measure, functional, topology, dual, automata, probabilistic, GF(2), enumerative, and infra APIs.
- `cargo +stable test -p amari-gpu --all-features --quiet -- --test-threads=1` passed on GB10.
- `WGPU_BACKEND=vulkan cargo +stable test -p amari-gpu --all-features --quiet -- --test-threads=1` passed on RTX 5080.
- RTX 5080 validation should use `WGPU_BACKEND=vulkan` and serial aggregate execution on the current Ubuntu 25.10 laptop stack.
- Benchmark/crossover measurements are still pending and should be reported separately.

### Temporarily Disabled Modules

| Domain Crate | Module | Status | Reason |
|-------------|--------|--------|--------|
| amari-fusion | `fusion` | ⚠️ Partially restored | Reduced public surface is available; broader fusion GPU API remains redesign-pending |
| amari-tropical | `tropical` | ⚠️ Narrow v1 surface restored | Crate-root re-exports are available for `TropicalGpuOps`, `TropicalExecutionPath`, `TropicalGpuError`, `TropicalGpuResult`; broader module internals remain redesign-pending |

**Note**: `amari_gpu::tropical` is still not re-enabled as a full public module. The current public tropical restoration is a narrow crate-root surface intended for dense tropical matrix multiplication and adaptive CPU/GPU dispatch.

## Features

```toml
[features]
default = []
std = ["amari-core/std", "amari-relativistic/std", "amari-info-geom/std"]
webgpu = ["wgpu/webgpu"]
high-precision = ["amari-core/high-precision", "amari-relativistic/high-precision"]
measure = ["dep:amari-measure"]
calculus = ["dep:amari-calculus"]
dual = ["dep:amari-dual"]
enumerative = ["dep:amari-enumerative"]
automata = ["dep:amari-automata"]
fusion = ["dep:amari-fusion"]
holographic = ["dep:amari-holographic"]  # Holographic memory GPU acceleration
probabilistic = ["dep:amari-probabilistic", "dep:rand", "dep:rand_distr"]  # Probabilistic GPU acceleration
fusion = ["dep:amari-fusion"]  # Reduced first public surface available; broader API still redesign-pending
topology = ["dep:amari-topology"]  # Computational topology GPU acceleration
tropical = ["dep:amari-tropical"]  # Narrow crate-root public v1 surface available; broader internals remain redesign-pending
```

## Usage

### Basic Setup

```rust
use amari_gpu::unified::GpuContext;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize GPU context
    let context = GpuContext::new().await?;

    // Use GPU-accelerated operations
    // ...

    Ok(())
}
```

### Calculus GPU-Ready API *(CPU-semantic fallback in 0.20.0)*

```rust
use amari_gpu::calculus::GpuCalculus;
use amari_calculus::ScalarField;
use amari_core::Multivector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize calculus GPU context and pipeline scaffolding
    let gpu_calculus = GpuCalculus::new().await?;

    // Define a scalar field (e.g., f(x,y,z) = x² + y² + z²)
    let field = ScalarField::new(|pos: &[f64; 3]| -> f64 {
        pos[0] * pos[0] + pos[1] * pos[1] + pos[2] * pos[2]
    });

    // Batch evaluate through the amari-gpu API.
    // Current 0.20.0 behavior preserves CPU semantics while WGSL kernels are validated.
    let points: Vec<[f64; 3]> = generate_point_grid(100, 100); // 10,000 points
    let values = gpu_calculus.batch_eval_scalar_field(&field, &points).await?;

    // Batch gradient computation currently uses the CPU finite-difference baseline.
    let gradients = gpu_calculus.batch_gradient(&field, &points, 1e-6).await?;

    Ok(())
}
```

### Dual Number GPU Operations *(narrow v1 surface)*

The `dual` feature exposes a narrow crate-root public surface for element-wise unary forward-mode AD:

| Type / operation | Current 0.20.0 behavior |
|------------------|--------------------------|
| `DualGpuOps::batch_forward_ad()` | GPU-backed unary operation chains over `DualNumber<f32>` batches |
| `GpuDualNumber` | POD transfer representation for `DualNumber<f32>` |
| `DualOperation::{Sin,Cos,Exp,Log,ReLU,Sigmoid,Tanh,Square,Sqrt}` | supported unary operations |
| `DualOperation::{Add,Multiply}` | retained for API continuity but rejected until binary/broadcast semantics are designed |

The full historical `amari_gpu::dual` module is no longer public. Neural-network gradients,
vector-function gradients, optimization scaffolding, and generic multi-dual GPU traits are internal
redesign-pending implementation details.

```rust
use amari_dual::DualNumber;
use amari_gpu::{DualGpuOps, DualOperation};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut gpu = DualGpuOps::new().await?;
    let inputs = vec![DualNumber::new(2.0_f32, 1.0_f32)];

    // Computes exp(x²) and its forward derivative for every input.
    let outputs = gpu
        .batch_forward_ad(&inputs, &[DualOperation::Square, DualOperation::Exp])
        .await?;

    println!("value={}, derivative={}", outputs[0].real, outputs[0].dual);
    Ok(())
}
```

### Automata GPU Operations *(mixed GPU-backed + documented fallback)*

The `automata` feature exposes cellular-automata helpers through crate-root re-exports:

| Type / operation | Current 0.20.0 behavior |
|------------------|--------------------------|
| `AutomataGpuOps::batch_apply_rules()` | GPU-backed rule application using the first supplied rule configuration |
| `AutomataGpuOps::batch_evolve_ca()` | repeats the GPU rule-application path for `steps_per_batch` steps |
| `AutomataGpuOps::calculate_total_energy()` | GPU-backed sum of squared multivector components |
| `AutomataGpuOps::extract_neighborhoods()` | CPU Moore-neighborhood baseline with wrapping boundaries |
| CA evolution / neighborhood pipelines | validation-safe scaffolding pending richer neighborhood-aware GPU kernels |

```rust
use amari_gpu::{AutomataGpuOps, GpuCellData, GpuEvolutionParams, GpuRuleConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut gpu = AutomataGpuOps::new().await?;

    let cells = vec![GpuCellData { scalar: 2.0, e1: 1.0, ..GpuCellData::default() }];
    let rule = GpuRuleConfig { damping_factor: 0.1, threshold: 0.5, ..GpuRuleConfig::default() };

    let evolved = gpu.batch_apply_rules(&cells, &[rule]).await?;
    let energy = gpu.calculate_total_energy(&evolved).await?;

    let params = GpuEvolutionParams { steps_per_batch: 4.0, ..GpuEvolutionParams::default() };
    let after_four_steps = gpu.batch_evolve_ca(&cells, &[rule], &params).await?;

    Ok(())
}
```

### Measure GPU Operations *(mixed GPU-backed + documented fallback)*

The `measure` feature currently exposes a broad public module plus crate-root re-exports:

| Type / operation | Current 0.20.0 behavior |
|------------------|--------------------------|
| `GpuIntegrator::integrate_uniform()` | GPU built-in function evaluation with CPU readback reduction |
| `GpuIntegrator::integrate_values()` | CPU reduction fallback for precomputed values |
| `GpuMonteCarloIntegrator::{expectation_uniform, integrate}` | GPU sampling/evaluation for built-in functions with CPU readback reduction |
| `GpuParametricDensity::gaussian_batch()` | GPU-backed Gaussian density batch evaluation |
| `GpuTropicalMeasure::{supremum, infimum}` | CPU reduction fallback |
| `GpuMultidimIntegrator::monte_carlo_nd()` | exact hypercube volume for constant-one integrand; multidimensional GPU Monte Carlo pending |

```rust
use amari_gpu::{GpuIntegrator, GpuParametricDensity};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let integrator = GpuIntegrator::new().await?;

    // Built-in function IDs: 0=x, 1=x², 2=x³, 3=sin(x), 4=cos(x), 5=exp(x).
    // This evaluates x² on the GPU and reduces the result after readback.
    let integral = integrator.integrate_uniform(0.0, 2.0, 10_000, 1).await?;

    // Precomputed/custom values currently use a documented CPU fallback reduction.
    let custom_integral = integrator.integrate_values(&[1.0, 2.0, 3.0], 0.5).await?;

    let density = GpuParametricDensity::new().await?;
    let gaussian = density.gaussian_batch(&[0.0, 1.0], 0.0, 1.0).await?;

    Ok(())
}
```

### Functional Analysis GPU Operations *(mixed GPU-backed + documented fallback)*

The `functional` feature exposes matrix, Hilbert-space, spectral, and adaptive helpers:

| Type / operation | Current 0.20.0 behavior |
|------------------|--------------------------|
| `GpuMatrixOperator::apply_batch()` | GPU-backed batch matrix-vector products |
| `GpuMatrixOperator::multiply()` | CPU readback fallback for correctness across independently-created GPU operators |
| `GpuMatrixOperator::to_matrix_operator()` | GPU readback to CPU matrix |
| `GpuSpectralDecomposition::compute()` | CPU `amari-functional` spectral baseline after GPU matrix readback |
| `GpuSpectralDecomposition::apply_function_batch()` | CPU spectral functional-calculus batch helper |
| `GpuHilbertSpace::{inner_product_batch, norm_batch}` | GPU-backed batch inner products and norms |
| `AdaptiveFunctionalCompute` | CPU/GPU dispatch for matrix batches; spectral path currently preserves CPU baseline semantics |

```rust
use amari_core::Multivector;
use amari_functional::{LinearOperator, MatrixOperator};
use amari_gpu::{GpuMatrixOperator, GpuSpectralDecomposition};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matrix = MatrixOperator::<2, 0, 0>::diagonal(&[2.0, 3.0, 4.0, 5.0])?;
    let gpu_matrix = GpuMatrixOperator::from_matrix_operator(&matrix).await?;

    let vectors = vec![Multivector::<2, 0, 0>::from_coefficients(vec![1.0, 2.0, 3.0, 4.0])];
    let applied = gpu_matrix.apply_batch(&vectors).await?;

    // Spectral decomposition is currently the CPU spectral baseline after readback.
    let spectral = GpuSpectralDecomposition::compute(&gpu_matrix, 100, 1e-10).await?;
    assert_eq!(spectral.eigenvalues().len(), 4);

    Ok(())
}
```

### Core Geometric Algebra + Information Geometry *(default crate-root APIs)*

The default API exposes `GpuCliffordAlgebra`, `AdaptiveCompute`, `GpuInfoGeometry`,
`GpuDeviceInfo`, and `GpuFisherMatrix` at the crate root.

```rust
use amari_core::Multivector;
use amari_gpu::{AdaptiveCompute, GpuInfoGeometry};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Legacy adaptive Cl(3,0,0) helper: single products use CPU baseline;
    // flat batches use GPU only above the current crossover and when available.
    let adaptive = AdaptiveCompute::new::<3, 0, 0>().await;
    let e1 = Multivector::<3, 0, 0>::basis_vector(0);
    let e2 = Multivector::<3, 0, 0>::basis_vector(1);
    let product = adaptive.geometric_product(&e1, &e2).await;
    assert_eq!(product.to_vec(), e1.geometric_product(&e2).to_vec());

    // Info geometry currently uses validated CPU baselines after WebGPU context creation.
    let info = GpuInfoGeometry::new().await?;
    let fisher = info.fisher_information_matrix(&[0.25, 0.5]).await?;
    assert_eq!(fisher.matrix(), &[vec![4.0, 0.0], vec![0.0, 2.0]]);
    Ok(())
}
```

| Type / operation | Current 0.20.0 behavior |
|------------------|--------------------------|
| `GpuCliffordAlgebra::new::<P,Q,R>()` | GPU context with signature-specific basis count and Cayley table |
| `GpuCliffordAlgebra::batch_geometric_product()` | GPU-backed `f32` batch geometric product for complete finite flat coefficient batches |
| `AdaptiveCompute::geometric_product()` | CPU `amari-core` baseline for single multivector products |
| `AdaptiveCompute::batch_geometric_product()` | legacy Cl(3,0,0) flat-batch helper with CPU fallback and GPU above threshold when available |
| `GpuInfoGeometry::amari_chentsov_tensor{,_batch}` | CPU baseline using `amari-info-geom`; equal-length batch validation |
| `GpuInfoGeometry::amari_chentsov_tensor_from_typed_arrays()` | finite `[x,y,z]` vector-component flat input validation, then CPU baseline |
| `GpuInfoGeometry::fisher_information_matrix()` | CPU probability-simplex-style diagonal Fisher metric baseline; finite non-negative inputs |
| `GpuInfoGeometry::bregman_divergence_batch()` | CPU KL-style Bregman divergence with shape/finite/non-negative validation |
| `GpuInfoGeometry::memory_usage()` | returns `0`; portable `wgpu` does not expose allocator usage |

### Infrastructure / Adaptive / Performance / Timeline APIs *(orchestration layer)*

The default crate also exposes infrastructure APIs used by higher-level GPU domains.
These are orchestration, profiling, and dispatch helpers; they are not mathematical
kernel APIs by themselves.

| Area | Public types | Current 0.20.0 behavior |
|------|--------------|--------------------------|
| Adaptive verification | `AdaptiveVerifier`, `VerificationPlatform`, `AdaptiveVerificationLevel`, `PlatformCapabilities` | platform detection with CPU fallback; verified batch operations validate equal lengths |
| Unified dispatch/context | `GpuContext`, `SharedGpuContext`, `GpuDispatcher`, `GpuOperationParams`, `GpuParam`, buffer-pool stats | common WebGPU context/dispatcher infrastructure with CPU fallback when GPU operations fail or are unavailable |
| Performance tuning | `WorkgroupOptimizer`, `AdaptiveDispatchPolicy`, `WorkgroupConfig`, `CalibrationResult`, `GpuProfiler` | heuristic workgroup defaults, calibration history, crossover learning; non-finite benchmark values are sanitized |
| Timeline analysis | `TimelineEvent`, `GpuTimelineAnalyzer`, `MultiGpuPerformanceMonitor`, report/summary types | CPU-timeline-based event recording, utilization/bottleneck heuristics, safe zero-window and timestamp handling |
| Multi-GPU coordination | `DeviceId`, `GpuDevice`, `Workload`, `IntelligentLoadBalancer`, `WorkloadCoordinator`, synchronization types | workload distribution/coordinator scaffolding for available `wgpu` devices; hardware validation still pending |
| Benchmarks | `AmariMultiGpuBenchmarks`, `BenchmarkRunner`, result/config/summary types | benchmark orchestration/reporting; crossover numbers must be generated per hardware target |

### Tropical GPU Acceleration *(narrow v1 surface)*

```rust
use amari_gpu::{TropicalExecutionPath, TropicalGpuOps};
use amari_tropical::{TropicalMatrix, TropicalNumber};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut gpu = TropicalGpuOps::new().await?;

    let mut a = TropicalMatrix::new(64, 64);
    let mut b = TropicalMatrix::new(64, 64);

    for i in 0..64 {
        for j in 0..64 {
            a.data[i][j] = TropicalNumber::new((i as f32 - j as f32) * 0.25);
            b.data[i][j] = TropicalNumber::new((i as f32 + j as f32) * 0.125);
        }
    }

    match gpu.matrix_multiply_execution_path(a.rows, a.cols, b.cols) {
        TropicalExecutionPath::Cpu => println!("Using CPU path for this problem size"),
        TropicalExecutionPath::Gpu => println!("Using GPU path for this problem size"),
    }

    // Explicit GPU path
    let _gpu_result = gpu.matrix_multiply(&a, &b).await?;

    // Adaptive path using current crossover heuristic
    let _adaptive_result = gpu.matrix_multiply_adaptive(&a, &b).await?;

    // Tropical winner-takes-all attention scores for a logits matrix
    let _scores = gpu.attention_scores(&a).await?;

    Ok(())
}
```

#### Tropical v1 Surface

The currently supported public tropical GPU API is intentionally small:

- `TropicalGpuOps`
- `TropicalExecutionPath`
- `TropicalGpuError`
- `TropicalGpuResult`

Current real kernel support:

- dense max-plus matrix multiplication
- adaptive CPU/GPU dispatch for matrix multiplication
- winner-takes-all tropical attention scores

Still redesign-pending and intentionally not exposed as a full public module:

- Viterbi
- attention
- tropical solve
- multivector GPU ops
- the older trait-based placeholder surface

### Holographic Memory GPU Acceleration

```rust
use amari_gpu::GpuHolographic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize GPU holographic operations for 256-dimensional ProductCl3x32 vectors
    let gpu = GpuHolographic::new(256).await?;

    // Flat arrays: batch_size * dimension coefficients
    let keys = vec![0.0f64; 1000 * 256];
    let values = vec![0.0f64; 1000 * 256];

    // Batch bind 1000 key-value pairs on GPU
    let bound = gpu.batch_bind(&keys, &values).await?;
    println!("Produced {} coefficients", bound.len());

    // Batch similarity computation
    let similarities = gpu.batch_similarity(&keys, &values).await?;
    println!("Computed {} similarities", similarities.len());

    Ok(())
}
```

#### Holographic GPU Operations *(validated v1)*

`GpuHolographic` currently validates and supports the canonical 256-dimensional `ProductCl3x32` layout.
Flat coefficient arrays must be finite and have length `batch_size * 256`.

| Operation | Current 0.20.0 behavior | GPU Threshold |
|-----------|--------------------------|---------------|
| `GpuHolographic::new(256)` / `new_product_cl3x32()` | accepts only validated `ProductCl3x32` dimensionality | n/a |
| `batch_bind()` | GPU-backed Cl3 geometric-product binding with the same basis/sign convention as `amari-holographic` | ≥ 100 pairs |
| `batch_unbind()` | CPU correctness path using `amari-holographic` inverse/unbind semantics | CPU-backed |
| `batch_similarity()` | GPU-backed ProductCl3x32 cosine similarity | ≥ 100 pairs |
| `batch_bundle()` | CPU correctness path using `ProductCl3x32::bundle(..., beta = 1.0)` | CPU-backed |
| `find_most_similar()` | batch similarity plus CPU max reduction; empty codebooks rejected | inherits similarity path |
| `GpuHolographicMemory::store_batch()` | CPU memory path with equal-length validation | CPU-backed |

#### WGSL Shaders

The holographic module includes optimized WGSL compute shaders:

- **`holographic_batch_bind`**: Cayley table-based geometric product for binding
- **`holographic_batch_similarity`**: Inner product with reverse `<A B̃>₀` for similarity
- **`holographic_bundle_all`**: Parallel reduction for vector superposition
- **`holographic_resonator_step`**: Parallel max-finding for cleanup

### Optical Field GPU Acceleration *(v0.15.1)*

```rust
use amari_gpu::GpuOpticalField;
use amari_holographic::optical::{OpticalRotorField, LeeEncoderConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize GPU context for optical fields (256x256 dimensions)
    let gpu = GpuOpticalField::new((256, 256)).await?;

    // Create optical rotor fields
    let field_a = OpticalRotorField::random((256, 256), 42);
    let field_b = OpticalRotorField::random((256, 256), 123);

    // GPU-accelerated binding (rotor multiplication = phase addition)
    let bound = gpu.bind(&field_a, &field_b).await?;
    println!("Bound field total energy: {:.4}", bound.total_energy());

    // GPU-accelerated similarity computation
    let similarity = gpu.similarity(&field_a, &field_b).await?;
    println!("Field similarity: {:.4}", similarity);

    // GPU-accelerated Lee hologram encoding
    let config = LeeEncoderConfig::new((256, 256), 0.25);
    let hologram = gpu.encode_lee(&field_a, &config).await?;
    println!("Hologram fill factor: {:.4}", hologram.fill_factor());

    // Batch operations for multiple field pairs
    let fields_a = vec![field_a.clone(), field_b.clone()];
    let fields_b = vec![field_b.clone(), field_a.clone()];

    let batch_bound = gpu.batch_bind(&fields_a, &fields_b).await?;
    let batch_sim = gpu.batch_similarity(&fields_a, &fields_b).await?;

    println!("Processed {} field pairs", batch_bound.len());

    Ok(())
}
```

#### Optical Field GPU Operations

| Operation | Description | GPU Threshold |
|-----------|-------------|---------------|
| `bind()` | Rotor multiplication (phase addition) | ≥ 4096 pixels (64×64) |
| `similarity()` | Normalized inner product with reduction | ≥ 4096 pixels |
| `encode_lee()` | Binary hologram encoding with bit-packing | ≥ 4096 pixels |
| `batch_bind()` | Parallel binding of field pairs | Any batch size |
| `batch_similarity()` | Parallel similarity computation | Any batch size |

#### WGSL Shaders for Optical Operations

- **`OPTICAL_BIND_SHADER`**: Element-wise rotor product in Cl(2,0)
  - Computes: `s_out = a_s·b_s - a_b·b_b`, `b_out = a_s·b_b + a_b·b_s`
  - Uses a packed output buffer to remain within WebGPU's portable storage-buffer binding limit
  - 256-thread workgroups for per-pixel parallelism

- **`OPTICAL_SIMILARITY_SHADER`**: Inner product with workgroup reduction
  - Computes: `⟨R_a, R_b⟩ = Σ(a_s·b_s + a_b·b_b) × amplitude_a × amplitude_b`
  - 256-thread workgroups with shared memory reduction

- **`LEE_ENCODE_SHADER`**: Binary hologram encoding with bit-packing
  - Each thread handles 32 pixels, packing results into u32
  - 64-thread workgroups for word-level parallelism

### Topology GPU Operations *(mixed GPU-backed + documented fallback)*

The `topology` feature exposes distance, Morse, Rips, Betti, and adaptive helpers:

| Type / operation | Current 0.20.0 behavior |
|------------------|--------------------------|
| `GpuTopology::compute_distance_matrix()` | GPU-backed pairwise Euclidean distances; returns a flattened `n × n` matrix |
| `GpuTopology::find_critical_points_2d()` | GPU-backed discrete Morse critical point detection |
| `GpuTopology::build_rips_filtration()` | CPU filtration/clique construction from a supplied distance matrix |
| `GpuTopology::compute_betti_numbers()` | CPU `amari-topology` homology baseline |
| `AdaptiveTopologyCompute` | GPU for distance/Morse above thresholds when available; CPU fallback otherwise |

```rust
use amari_gpu::{AdaptiveTopologyCompute, GpuTopology};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpu_topology = GpuTopology::new().await?;

    let points = vec![(0.0, 0.0, 0.0), (1.0, 0.0, 0.0), (0.0, 1.0, 0.0)];
    let distances = gpu_topology.compute_distance_matrix(&points).await?;
    println!("Computed flattened {}x{} distance matrix", points.len(), points.len());

    // Rips construction currently uses CPU logic over the distance matrix.
    let filtration = gpu_topology
        .build_rips_filtration(&distances, points.len(), 1.5, 2)
        .await?;
    println!("Built filtration with {} simplices", filtration.len());

    let width = 128;
    let height = 128;
    let values: Vec<f64> = (0..width * height)
        .map(|i| {
            let x = (i % width) as f64 / width as f64;
            let y = (i / width) as f64 / height as f64;
            (x * std::f64::consts::TAU).sin() * (y * std::f64::consts::TAU).cos()
        })
        .collect();
    let critical_points = gpu_topology.find_critical_points_2d(&values, width, height).await?;
    println!("Found {} critical points", critical_points.len());

    // Adaptive dispatcher: GPU where validated/beneficial, CPU fallback otherwise.
    let adaptive = AdaptiveTopologyCompute::new().await;
    let adaptive_distances = adaptive.compute_distance_matrix(&points).await?;
    assert_eq!(adaptive_distances.len(), points.len() * points.len());

    Ok(())
}
```

#### Topology Operations

| Operation | Current path |
|-----------|--------------|
| `compute_distance_matrix()` | GPU direct path; adaptive CPU fallback for small/unavailable GPU |
| `find_critical_points_2d()` | GPU direct path; adaptive CPU fallback for small/unavailable GPU |
| `build_rips_filtration()` | CPU filtration/clique construction, optionally fed by GPU distance matrix |
| `compute_betti_numbers()` | CPU homology baseline; boundary/reduction GPU kernels pending validation |

#### WGSL Shaders for Topology Operations

- **Distance matrix shader**: parallel pairwise Euclidean distance computation.
- **Morse critical point shader**: compares each interior cell with its 8 neighbors.
- **Boundary/reduction shader scaffolding**: reserved for future persistent-homology kernels; public Betti behavior is currently CPU-backed.

### Dynamics GPU Acceleration *(v0.19.1)*

```rust
use amari_gpu::dynamics::{GpuDynamics, BatchTrajectoryConfig, GpuSystemType};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize GPU dynamics context
    let gpu = GpuDynamics::new().await?;

    // Batch trajectory integration (1000 initial conditions in parallel)
    let initial_conditions: Vec<[f64; 3]> = (0..1000)
        .map(|i| [1.0 + i as f64 * 0.001, 1.0, 1.0])
        .collect();

    let config = BatchTrajectoryConfig {
        dt: 0.01,
        steps: 5000,
        dim: 3,
        system_type: GpuSystemType::Lorenz { sigma: 10.0, rho: 28.0, beta: 8.0/3.0 },
    };

    let trajectories = gpu.batch_trajectories(&initial_conditions, &config).await?;
    println!("Computed {} trajectories on GPU", trajectories.len());

    // GPU bifurcation diagram (parameter sweep)
    let param_range = (2.5, 4.0);
    let num_params = 1000;
    let diagram = gpu.bifurcation_diagram(
        GpuSystemType::LogisticMap,
        param_range,
        num_params,
        500,  // transient
        100,  // samples
    ).await?;
    println!("Bifurcation diagram: {} parameter values", diagram.len());

    // GPU Lyapunov spectrum computation
    let lyapunov = gpu.lyapunov_spectrum(
        &[1.0, 1.0, 1.0],
        GpuSystemType::Lorenz { sigma: 10.0, rho: 28.0, beta: 8.0/3.0 },
        10000,  // steps
        0.01,   // dt
    ).await?;
    println!("Lyapunov exponents: {:?}", lyapunov);

    // GPU basin of attraction computation
    let grid_resolution = (100, 100);
    let basin = gpu.compute_basin(
        GpuSystemType::Duffing { alpha: 1.0, beta: -1.0, delta: 0.2, gamma: 0.3, omega: 1.2 },
        grid_resolution,
        (-2.0, 2.0),  // x range
        (-2.0, 2.0),  // y range
        1000,         // max iterations
    ).await?;
    println!("Basin computed: {} x {} grid", grid_resolution.0, grid_resolution.1);

    Ok(())
}
```

#### Dynamics GPU Operations

| Operation | Description | GPU Threshold |
|-----------|-------------|---------------|
| `batch_trajectories()` | Parallel ODE integration for many initial conditions | ≥ 100 trajectories |
| `bifurcation_diagram()` | Parameter sweep with attractor sampling | ≥ 100 parameter values |
| `lyapunov_spectrum()` | QR-based Lyapunov exponent computation | ≥ 1000 steps |
| `compute_basin()` | Basin of attraction grid computation | ≥ 10000 grid cells |

#### WGSL Shaders for Dynamics Operations

- **`DYNAMICS_RK4_STEP`**: Fourth-order Runge-Kutta integration step
  - 256-thread workgroups for parallel trajectory evolution
  - Supports Lorenz, Van der Pol, Duffing, Rossler, Henon systems

- **`DYNAMICS_LYAPUNOV_QR`**: QR decomposition for tangent space evolution
  - Computes orthonormalization for Lyapunov exponent estimation
  - Workgroup-shared memory for matrix operations

- **`DYNAMICS_BIFURCATION`**: Parameter-dependent attractor sampling
  - Parallel transient discard and attractor point collection
  - Outputs (parameter, attractor_value) pairs

- **`DYNAMICS_BASIN`**: Grid-based trajectory classification
  - Classifies each grid point by attractor convergence
  - 256-thread workgroups for spatial parallelism

### Enumerative Geometry GPU Operations *(high-use broad surface)*

The `enumerative` feature remains a broad public module for downstream compatibility, with
crate-root re-exports for the most-used data types and operations. Representative public tests now
cover the high-use kernels listed below.

```rust
use std::collections::BTreeSet;
use amari_enumerative::Matroid;
use amari_gpu::{EnumerativeGpuOps, GpuMatroidRankData, GpuWDVVData};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut gpu_ops = EnumerativeGpuOps::new().await?;

    // Batch WDVV/Kontsevich curve counts for P², degrees 1..=6.
    let wdvv_data: Vec<GpuWDVVData> = (1..=6).map(GpuWDVVData::from_degree).collect();
    let counts = gpu_ops.batch_wdvv_curve_counts(&wdvv_data).await?;
    assert_eq!(counts, vec![1, 1, 12, 620, 87304, 26312976]);

    // Batch matroid rank computation via bitmask-encoded bases.
    let matroid = Matroid::uniform(2, 4);
    let subset: BTreeSet<usize> = [0, 1, 2].into_iter().collect();
    let rank_data = vec![GpuMatroidRankData::from_matroid_subset(&matroid, &subset)];
    let ranks = gpu_ops.batch_matroid_ranks(&rank_data).await?;

    Ok(())
}
```

#### Enumerative GPU Operations

| Operation | Current path | Mathematical basis / caveat |
|-----------|--------------|-----------------------------|
| `batch_intersection_numbers()` | GPU-backed compact formula | degree/codimension compatibility with multiplicity/genus correction |
| `batch_wdvv_curve_counts()` | GPU-backed lookup | Kontsevich numbers `N_1..N_6` for `P²`; higher degrees return `0` |
| `batch_localization_euler_classes()` | GPU-backed product formula | tangent Euler class at fixed points, weights limited by compact GPU data layout |
| `batch_matroid_ranks()` | GPU-backed bitmask computation | max `|A ∩ B|` over up to 32 encoded bases |
| `batch_csm_euler_characteristics()` | GPU-backed cell contribution | Schubert-cell contribution currently returns `1` per cell |
| `batch_operad_multiplicities()` | GPU-backed codimension check | matching single-interface codimensions give multiplicity `1` within dimension bounds |
| `batch_stability_phases()` | GPU-backed phase formula | normalized `atan2(trust * dim, -codim) / π` |
| `batch_stability_checks()` | GPU-backed phase interval test | stable iff normalized phase is strictly in `(0, 1)` |
| broader Schubert/GW/LR/namespace/tropical/GF(2) helpers | GPU-backed, representative tests exist | deeper mathematical parity work remains a post-0.20.0 task |

See `docs/roadmap/AMARI_GPU_ENUMERATIVE_CLASSIFICATION.md` for the method-by-method classification table.

### GF(2) GPU Operations *(fixed-layout GPU-backed surface)*

The `gf2` feature exposes batch binary Clifford products, GF(2) matrix-vector multiplication,
and Hamming distance kernels. The public API validates fixed-layout bounds before dispatch,
and representative tests compare these kernels against `amari-core::gf2` CPU baselines and
GF(2) algebraic properties.

| Operation | Current 0.20.0 behavior |
|-----------|--------------------------|
| `batch_gf2_geometric_product()` | GPU-backed `Cl(N,R;F₂)` product, up to 128 blades (`num_generators <= 7`) |
| `batch_gf2_matvec()` | GPU-backed GF(2) matrix-vector multiplication, up to 16 rows × 32 columns |
| `batch_gf2_hamming_distance()` | GPU-backed Hamming distance, up to 128 bits, masks unused final-word bits by `dim` |

```rust
use amari_core::gf2::{GF2Matrix, GF2Vector};
use amari_gpu::{GF2GpuOps, GpuGF2CliffordPair, GpuGF2MatVecData};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut gpu = GF2GpuOps::new().await?;

    // e1 * e2 = e12 in Cl(3,0;F₂).
    let products = gpu.batch_gf2_geometric_product(&[
        GpuGF2CliffordPair::from_bits(&[0, 1], &[0, 0, 1], 3, 0),
    ]).await?;
    assert_eq!(products[0][0] & (1 << 3), 1 << 3);

    let matrix = GF2Matrix::identity(3);
    let vector = GF2Vector::from_bits(&[1, 0, 1]);
    let matvec = GpuGF2MatVecData::from_matrix_and_vector(&matrix, &vector);
    assert_eq!(gpu.batch_gf2_matvec(&[matvec]).await?, vec![0b101]);

    Ok(())
}
```

### Relativistic GPU Operations *(Minkowski products + simplified geodesic propagation)*

The default relativistic API exposes crate-root `GpuRelativisticPhysics`, `GpuSpacetimeVector`,
`GpuRelativisticParticle`, and `GpuTrajectoryParams`. `GpuSpacetimeVector` stores coordinates as
`(ct, x, y, z)` and `compute_minkowski_products()` returns `ct² - x² - y² - z²`. Particle
propagation uses a simplified Schwarzschild-style GPU geodesic step and validates trajectory and
particle fields before dispatch.

```rust
use amari_gpu::{GpuRelativisticPhysics, GpuSpacetimeVector};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let gpu = GpuRelativisticPhysics::new().await?;
    let norms = gpu.compute_minkowski_products(&[
        GpuSpacetimeVector::new(2.0, 1.0, 0.5, 0.25),
    ]).await?;
    assert!((norms[0] - (4.0 - 1.0 - 0.25 - 0.0625)).abs() < 1e-5);
    Ok(())
}
```

| Operation | Current 0.20.0 behavior |
|-----------|--------------------------|
| `GpuSpacetimeVector::{from,to}_spacetime_vector()` | preserves `[ct, x, y, z]` CPU coordinates |
| `compute_minkowski_products()` | GPU-backed Minkowski norm-squared for finite spacetime vectors; empty input returns empty |
| `propagate_particles()` | simplified GPU geodesic step; zero steps return input unchanged; validates finite/non-negative fields and trajectory parameters |

### Network GPU Operations *(narrow GPU distances + adaptive CPU fallback)*

The default network API exposes crate-root `GpuGeometricNetwork` and `AdaptiveNetworkCompute`.
The current GPU kernel computes pairwise Euclidean distances for vector-only `Cl(P,0,0)` embeddings
with `P <= 3`. Geometric centrality and clustering reuse those GPU distances but perform their
reductions/medoid updates on the CPU. Adaptive dispatch falls back to `amari-network` CPU geometric
baselines for small networks, unsupported signatures, or non-vector multivectors.

```rust
use amari_core::Vector;
use amari_gpu::AdaptiveNetworkCompute;
use amari_network::GeometricNetwork;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut network = GeometricNetwork::<3, 0, 0>::new();
    let a = network.add_node(Vector::from_components(0.0, 0.0, 0.0).mv);
    let b = network.add_node(Vector::from_components(3.0, 4.0, 0.0).mv);
    network.add_undirected_edge(a, b, 1.0)?;

    let adaptive = AdaptiveNetworkCompute::new().await;
    let distances = adaptive.compute_all_pairwise_distances(&network).await?;
    assert_eq!(distances[a][b], 5.0); // geometric distance, not edge shortest path

    let centrality = adaptive.compute_geometric_centrality(&network).await?;
    assert_eq!(centrality.len(), network.num_nodes());

    Ok(())
}
```

| Operation | Current 0.20.0 behavior |
|-----------|--------------------------|
| `GpuGeometricNetwork::compute_all_pairwise_distances()` | GPU Euclidean distances for vector-only `Cl(P,0,0)`, `P <= 3`; rejects unsupported embeddings |
| `GpuGeometricNetwork::compute_geometric_centrality()` | GPU distance matrix plus CPU centrality reduction |
| `GpuGeometricNetwork::geometric_clustering()` | GPU distance matrix plus CPU k-medoid assignment/update and distance-derived cohesion |
| `AdaptiveNetworkCompute::*` | Uses GPU only for supported large networks; otherwise CPU geometric-distance baselines |

### Probabilistic GPU Operations *(GPU-backed sampling/statistics + small-batch CPU fallback)*

The `probabilistic` feature exposes crate-root `GpuProbabilistic` APIs for vector-valued Gaussian
sampling and batch statistics. A GPU context is still required; after initialization, small batches
currently use CPU fallback while larger batches dispatch WGSL kernels.

```rust
use amari_gpu::GpuProbabilistic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Dimension of each sample vector / multivector coefficient array.
    let gpu_prob = GpuProbabilistic::new(3).await?;

    // Batch sample 10,000 Gaussian vectors on GPU.
    let samples = gpu_prob
        .batch_sample_gaussian(10_000, &[0.0, 1.0, 2.0], &[1.0, 0.5, 2.0])
        .await?;
    assert_eq!(samples.len(), 10_000 * 3);

    // Compute coefficient-wise statistics.
    let mean = gpu_prob.batch_mean(&samples).await?;
    let variance = gpu_prob.batch_variance(&samples, &mean).await?;

    Ok(())
}
```

#### Probabilistic GPU Operations

| Operation | Current 0.20.0 behavior | Validation / fallback |
|-----------|--------------------------|-----------------------|
| `GpuProbabilistic::new(dimension)` | creates GPU pipelines for fixed sample dimension | rejects `dimension == 0` |
| `batch_sample_gaussian()` | GPU Box-Muller sampling for `num_samples >= 100`; CPU fallback below that | validates mean/std-dev lengths, finite means, finite non-negative std-devs |
| `batch_mean()` | GPU coefficient-wise sum/readback for `num_samples >= 100`; CPU fallback below that | rejects empty, non-finite, or mis-shaped sample buffers |
| `batch_variance()` | GPU coefficient-wise squared-difference/readback with Bessel correction for `num_samples >= 100`; CPU fallback below that | rejects empty, one-sample, non-finite, mis-shaped, or bad mean buffers |

### Adaptive CPU/GPU Dispatch

The library automatically selects the optimal execution path:

```rust
// Small batch: Automatically uses CPU (< 1000 points for scalar fields)
let small_points = vec![[0.0, 0.0, 0.0], [1.0, 1.0, 1.0]];
let values = gpu_calculus.batch_eval_scalar_field(&field, &small_points).await?;
// ↑ Executed on CPU (overhead of GPU transfer exceeds benefit)

// Large batch: Automatically uses GPU (≥ 1000 points)
let large_points = generate_point_grid(100, 100); // 10,000 points
let values = gpu_calculus.batch_eval_scalar_field(&field, &large_points).await?;
// ↑ Executed on GPU (parallel processing advantage)
```

### Batch Size Thresholds

| Operation | CPU Threshold | GPU Threshold |
|-----------|--------------|---------------|
| Scalar field evaluation | current 0.20.0 path | CPU-semantic fallback; WGSL kernel pending |
| Vector field evaluation | current 0.20.0 path | CPU-semantic fallback; WGSL kernel pending |
| Gradient computation | current 0.20.0 path | CPU finite-difference fallback; WGSL kernel pending |
| Divergence/Curl | current 0.20.0 path | CPU finite-difference fallback; WGSL kernel pending |
| Holographic binding | < 100 pairs | ≥ 100 pairs |
| Holographic similarity | < 100 vectors | ≥ 100 vectors |
| Resonator cleanup | < 100 codebook | ≥ 100 codebook |
| Optical field bind | < 4096 pixels | ≥ 4096 pixels (64×64) |
| Optical similarity | < 4096 pixels | ≥ 4096 pixels |
| Lee hologram encoding | < 4096 pixels | ≥ 4096 pixels |
| Gaussian sampling | < 100 samples | ≥ 100 samples |
| Batch mean/variance | < 100 samples | ≥ 100 samples |
| Measure built-in 1D integration | GPU evaluation | CPU readback reduction |
| Measure precomputed values | CPU reduction fallback | GPU reduction pending |
| Measure Gaussian density | N/A | GPU batch evaluation |
| Tropical measure extrema | CPU reduction fallback | GPU reduction pending |
| Distance matrix | < 100 points | ≥ 100 points |
| Morse critical points | < 10000 cells | ≥ 10000 cells |
| Rips filtration | CPU filtration construction | may use GPU distance matrix |
| Batch trajectories | < 100 trajectories | ≥ 100 trajectories |
| Bifurcation diagram | < 100 params | ≥ 100 parameter values |
| Lyapunov spectrum | < 1000 steps | ≥ 1000 steps |
| Basin of attraction | < 10000 cells | ≥ 10000 grid cells |

## Implementation Status

### Holographic Module (v0.13.0)

**GPU Implementations** (✅ Complete):
- Batch binding with Cayley table geometric product
- Batch similarity using proper inner product `<A B̃>₀`
- Parallel reduction for vector bundling
- Resonator cleanup with parallel codebook search

### Optical Field Module (v0.15.1)

**GPU Implementations** (✅ Complete):
- Rotor field binding via `OPTICAL_BIND_SHADER`
- Similarity with workgroup reduction via `OPTICAL_SIMILARITY_SHADER`
- Lee hologram encoding with bit-packing via `LEE_ENCODE_SHADER`
- Automatic CPU fallback for small fields (< 4096 pixels)

**Types**:
- `GpuOpticalField`: GPU context for optical rotor field operations
- Uses `OpticalRotorField` from amari-holographic (SoA layout: scalar, bivector, amplitude)
- Uses `BinaryHologram` for bit-packed hologram output
- Uses `LeeEncoderConfig` for carrier wave parameters

### Probabilistic Module (v0.20.0)

**GPU-backed implementations**:
- Batch Gaussian sampling on coefficient vectors using Box-Muller transform
- Coefficient-wise batch mean computation
- Coefficient-wise batch variance computation with Bessel correction

**CPU fallback paths**:
- Sampling/statistics batches with fewer than 100 samples use CPU fallback after GPU context creation.

**Types**:
- `GpuProbabilistic`: GPU context for probabilistic sampling/statistics
- `GpuProbabilisticError` / `GpuProbabilisticResult`: error/result types

**Current validation**:
- rejects zero dimensions
- validates sample-buffer shape and finite values
- validates finite means and non-negative finite standard deviations
- rejects variance requests with fewer than two samples

### Calculus Module (v0.13.0)

**CPU Implementations** (✅ Complete):
- Central finite differences for numerical derivatives
- Field evaluation at multiple points
- Gradient, divergence, and curl computation
- Step size: h = 1e-6 for numerical stability

**GPU Implementations** (⏸️ Future Work):
- WGSL compute shaders for parallel field evaluation
- Parallel finite difference computation
- Optimized memory layout for GPU transfer

**Current Behavior**:
- Infrastructure and pipelines are in place
- All operations currently use CPU implementations
- Shaders can be added incrementally without API changes

### Topology Module (v0.16.0)

**GPU-backed Implementations** (✅ Complete):
- Distance matrix computation with parallel pairwise Euclidean distance
- Morse critical point detection for 2D scalar fields

**CPU fallback / scaffolding paths**:
- Rips filtration construction from a distance matrix currently uses CPU clique construction
- Betti number computation currently uses the `amari-topology` CPU homology baseline
- Boundary/reduction shader scaffolding is reserved for future persistent-homology kernels

**Types**:
- `GpuTopology`: GPU context for topology operations
- `GpuCriticalPoint`: Critical point with position, value, type, and index
- `AdaptiveTopologyCompute`: Automatic CPU/GPU dispatch based on workload size
- `GpuTopologyError` / `GpuTopologyResult`: Error handling types

**Shaders**:
- Distance matrix shader: 8×8 workgroups for O(n²) distance computation
- Morse critical point shader: 8-neighbor comparison for critical point classification
- Boundary/reduction scaffolding: validation-safe placeholders until persistent-homology kernels are restored

**Adaptive Thresholds**:
- Distance matrix: GPU for ≥ 100 points (n² = 10,000 operations)
- Morse critical points: GPU for ≥ 10,000 grid cells (100×100)
- Falls back to CPU for smaller workloads to avoid transfer overhead

### Dynamics Module (v0.19.1)

**GPU Implementations** (✅ Complete):
- Batch trajectory integration with RK4 solver
- Bifurcation diagram computation with parallel parameter sweeps
- Lyapunov spectrum via QR-based tangent space evolution
- Basin of attraction grid computation

**Types**:
- `GpuDynamics`: GPU context for dynamical systems operations
- `BatchTrajectoryConfig`: Configuration for parallel trajectory integration
- `GpuSystemType`: Enum for built-in systems (Lorenz, VanDerPol, Duffing, Rossler, Henon, LogisticMap)
- `GpuDynamicsError` / `GpuDynamicsResult`: Error handling types

**Shaders**:
- `DYNAMICS_RK4_STEP`: 256-thread workgroups for RK4 integration
- `DYNAMICS_LYAPUNOV_QR`: QR decomposition for Lyapunov exponents
- `DYNAMICS_BIFURCATION`: Parameter sweep attractor sampling
- `DYNAMICS_BASIN`: Grid-based trajectory classification

**Adaptive Thresholds**:
- Batch trajectories: GPU for ≥ 100 initial conditions
- Bifurcation diagram: GPU for ≥ 100 parameter values
- Lyapunov spectrum: GPU for ≥ 1000 integration steps
- Basin computation: GPU for ≥ 10,000 grid cells

## Examples

See the `examples/` directory for complete examples:

```bash
# Run geometric algebra example
cargo run --example ga_operations

# Run information geometry example
cargo run --example fisher_metric

# Run calculus example (requires 'calculus' feature)
cargo run --features calculus --example field_ops
```

## Development

### Running Tests

```bash
# Run all tests
cargo test

# Run with specific features
cargo test --features calculus
cargo test --features measure

# Run GPU tests (requires GPU access)
cargo test --test gpu_integration
```

### Building Documentation

```bash
cargo doc --all-features --no-deps --open
```

## Future Work

### Short-term (v0.13.x)
1. Implement WGSL shaders for calculus operations
2. Add GPU benchmarks comparing CPU vs GPU performance
3. Optimize memory transfer patterns
4. Add more comprehensive examples
5. **Restore tropical GPU module** using extension traits (orphan impl fix)

### Medium-term (v0.14.x - v0.15.x)
1. Implement tropical algebra GPU operations
2. Multi-GPU support for large holographic memories
3. Performance optimization across all GPU modules
4. Unified GPU context sharing across all modules

### Long-term (v1.0.0+)
1. WebGPU backend for browser deployment
2. Multi-GPU support for distributed computation
3. Kernel fusion optimization
4. Custom WGSL shader compilation pipeline

## Performance Considerations

- **GPU Initialization**: ~100-200ms startup cost for context creation
- **Data Transfer**: Significant overhead for small batches (< 500 elements)
- **Optimal Use Cases**: Large batch operations (> 1000 elements)
- **Memory**: GPU buffers are sized for batch operations (dynamically allocated)

## Platform Support

| Platform | Backend | Status |
|----------|---------|--------|
| Linux | Vulkan | ✅ Tested |
| macOS | Metal | ✅ Supported (not regularly tested) |
| Windows | DirectX 12 / Vulkan | ✅ Supported (not regularly tested) |
| WebAssembly | WebGPU | ⏸️ Requires `webgpu` feature |

## Dependencies

- `wgpu` (v0.19): WebGPU implementation
- `bytemuck`: Zero-cost GPU buffer conversions
- `nalgebra`: Linear algebra operations
- `tokio`: Async runtime for GPU operations
- `futures`, `pollster`: Async utilities

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT License ([LICENSE-MIT](../LICENSE-MIT))

at your option.

## Contributing

Contributions are welcome! Areas of particular interest:

1. WGSL shader implementations for calculus operations
2. Performance benchmarks and optimization
3. Platform-specific testing and bug reports
4. Documentation improvements and examples

## References

- [WebGPU Specification](https://www.w3.org/TR/webgpu/)
- [wgpu Documentation](https://docs.rs/wgpu/)
- [Geometric Algebra GPU Acceleration](https://arxiv.org/abs/2103.00123) (example reference)
