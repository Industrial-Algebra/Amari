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
| **amari-core** | `core` | Geometric algebra operations (G2, G3, G4), multivector products | ✅ Implemented |
| **amari-info-geom** | `info_geom` | Fisher metric, divergence computations, statistical manifolds | ✅ Implemented |
| **amari-relativistic** | `relativistic` | Minkowski space operations, Lorentz transformations | ✅ Implemented |
| **amari-network** | `network` | Graph operations, spectral methods | ✅ Implemented |
| **amari-measure** | `measure` | Measure theory computations, sigma-algebras | ✅ Implemented (feature: `measure`) |
| **amari-calculus** | `calculus` | Field evaluation, gradients, divergence, curl | ✅ Implemented (feature: `calculus`) |
| **amari-dual** | `dual` | Automatic differentiation GPU operations | ✅ Implemented (feature: `dual`) |
| **amari-enumerative** | `enumerative` | Intersection theory, WDVV curve counting, matroid ranks, CSM classes, localization, operad, stability | ✅ Implemented (feature: `enumerative`) |
| **amari-automata** | `automata` | Cellular automata GPU evolution | ✅ Implemented (feature: `automata`) |
| **amari-fusion** | `fusion` | Reduced first public surface for holographic/fusion-derived GPU operations | ⚠️ Partially restored; broader fusion GPU API still under redesign |
| **amari-holographic** | `holographic` | Holographic memory, batch binding, similarity matrices, **optical field operations** | ✅ Implemented (feature: `holographic`) |
| **amari-probabilistic** | `probabilistic` | Gaussian sampling, batch statistics, Monte Carlo | ✅ Implemented (feature: `probabilistic`) |
| **amari-functional** | `functional` | Matrix operators, spectral decomposition, Hilbert spaces | ✅ Implemented (feature: `functional`) |
| **amari-topology** | `topology` | Distance matrices, Morse critical points, Rips filtrations | ✅ Implemented (feature: `topology`) |

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

### Calculus GPU Acceleration

```rust
use amari_gpu::calculus::GpuCalculus;
use amari_calculus::ScalarField;
use amari_core::Multivector;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize GPU calculus
    let gpu_calculus = GpuCalculus::new().await?;

    // Define a scalar field (e.g., f(x,y,z) = x² + y² + z²)
    let field = ScalarField::new(|pos: &[f64; 3]| -> f64 {
        pos[0] * pos[0] + pos[1] * pos[1] + pos[2] * pos[2]
    });

    // Batch evaluate at 10,000 points (uses GPU)
    let points: Vec<[f64; 3]> = generate_point_grid(100, 100); // 10,000 points
    let values = gpu_calculus.batch_eval_scalar_field(&field, &points).await?;

    // Batch gradient computation (uses GPU for large batches)
    let gradients = gpu_calculus.batch_gradient(&field, &points, 1e-6).await?;

    Ok(())
}
```

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
    let keys = vec![0.0f32; 1000 * 256];
    let values = vec![0.0f32; 1000 * 256];

    // Batch bind 1000 key-value pairs on GPU
    let bound = gpu.batch_bind(&keys, &values).await?;
    println!("Produced {} coefficients", bound.len());

    // Batch similarity computation
    let similarities = gpu.batch_similarity(&keys, &values).await?;
    println!("Computed {} similarities", similarities.len());

    Ok(())
}
```

#### Holographic GPU Operations

| Operation | Description | GPU Threshold |
|-----------|-------------|---------------|
| `batch_bind()` | Parallel geometric product binding | ≥ 100 pairs |
| `batch_similarity()` | Pairwise or matrix similarity computation | ≥ 100 vectors |
| `resonator_cleanup()` | Parallel codebook search for best match | ≥ 100 codebook entries |

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
  - 256-thread workgroups for per-pixel parallelism

- **`OPTICAL_SIMILARITY_SHADER`**: Inner product with workgroup reduction
  - Computes: `⟨R_a, R_b⟩ = Σ(a_s·b_s + a_b·b_b) × amplitude_a × amplitude_b`
  - 256-thread workgroups with shared memory reduction

- **`LEE_ENCODE_SHADER`**: Binary hologram encoding with bit-packing
  - Each thread handles 32 pixels, packing results into u32
  - 64-thread workgroups for word-level parallelism

### Topology GPU Acceleration *(v0.16.0)*

```rust
use amari_gpu::topology::{GpuTopology, AdaptiveTopologyCompute};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize GPU topology operations
    let gpu_topology = GpuTopology::new().await?;

    // Compute distance matrix for Rips filtration (uses GPU for > 100 points)
    let points = vec![
        vec![0.0, 0.0], vec![1.0, 0.0], vec![0.5, 0.866],
        vec![2.0, 0.0], vec![2.5, 0.866], vec![3.0, 0.0],
        // ... more points ...
    ];
    let distances = gpu_topology.compute_distance_matrix(&points).await?;
    println!("Computed {}x{} distance matrix", distances.len(), distances[0].len());

    // Find Morse critical points in 2D scalar field (uses GPU for > 10000 cells)
    let grid_size = (128, 128);
    let values: Vec<f64> = (0..grid_size.0 * grid_size.1)
        .map(|i| {
            let x = (i % grid_size.0) as f64 / grid_size.0 as f64;
            let y = (i / grid_size.0) as f64 / grid_size.1 as f64;
            (x * 6.28).sin() * (y * 6.28).cos()
        })
        .collect();

    let critical_points = gpu_topology.find_critical_points_2d(&values, grid_size).await?;
    println!("Found {} critical points", critical_points.len());

    // Build Rips filtration from distance matrix
    let max_radius = 2.0;
    let max_dimension = 2;
    let filtration = gpu_topology.build_rips_filtration(&distances, max_radius, max_dimension).await?;
    println!("Built filtration with {} simplices", filtration.simplices().len());

    // Use adaptive dispatcher (automatic CPU/GPU selection)
    let adaptive = AdaptiveTopologyCompute::new().await;
    let betti = adaptive.compute_betti_numbers(&distances, max_radius, max_dimension).await?;
    println!("Betti numbers: β₀={}, β₁={}, β₂={}", betti[0], betti[1], betti[2]);

    Ok(())
}
```

#### Topology GPU Operations

| Operation | Description | GPU Threshold |
|-----------|-------------|---------------|
| `compute_distance_matrix()` | Pairwise Euclidean distances | ≥ 100 points |
| `find_critical_points_2d()` | Morse critical point detection | ≥ 10000 grid cells |
| `build_rips_filtration()` | Vietoris-Rips complex construction | Uses distance matrix |
| `compute_betti_numbers()` | Persistent homology computation | Adaptive |

#### WGSL Shaders for Topology Operations

- **`TOPOLOGY_DISTANCE_MATRIX`**: Parallel pairwise distance computation
  - 256-thread workgroups computing `√Σ(xᵢ - yⱼ)²`
  - Outputs upper triangular matrix to minimize memory

- **`TOPOLOGY_MORSE_CRITICAL`**: Discrete Morse theory critical point detection
  - Compares each cell with 8 neighbors (2D grid)
  - Outputs: index (0=regular, 1=min, 2=saddle, 3=max)

- **`TOPOLOGY_BOUNDARY_MATRIX`**: Boundary operator matrix construction
  - Builds sparse representation for simplicial complex
  - Used in persistent homology computation

- **`TOPOLOGY_MATRIX_REDUCTION`**: Column reduction for persistence
  - Implements standard algorithm for reduced boundary matrix
  - Extracts persistence pairs from reduced matrix

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

### Enumerative Geometry GPU Acceleration *(v0.19.1)*

```rust
use amari_gpu::enumerative::{EnumerativeGpuOps, GpuWDVVData, GpuMatroidRankData, GpuStabilityData};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut gpu_ops = EnumerativeGpuOps::new().await?;

    // Batch WDVV curve counts (degrees 1-6)
    let wdvv_data: Vec<GpuWDVVData> = (1..=6).map(GpuWDVVData::from_degree).collect();
    let counts = gpu_ops.batch_wdvv_curve_counts(&wdvv_data).await?;
    // counts = [1, 1, 12, 620, 87304, 26312976]

    // Batch matroid rank computation
    let matroid_data = vec![
        GpuMatroidRankData::from_matroid_subset(&matroid, &[0, 1]),
        GpuMatroidRankData::from_matroid_subset(&matroid, &[0, 2, 3]),
    ];
    let ranks = gpu_ops.batch_matroid_ranks(&matroid_data).await?;

    // Batch stability phase computation
    let stability_data = vec![
        GpuStabilityData::from_class_and_trust(&class, 0.5),
        GpuStabilityData::from_class_and_trust(&class, 1.0),
    ];
    let phases = gpu_ops.batch_stability_phases(&stability_data).await?;

    Ok(())
}
```

#### Enumerative GPU Operations

| Operation | Description | Shader |
|-----------|-------------|--------|
| `batch_wdvv_curve_counts()` | WDVV rational curve counts N_d | Lookup table (N_1..N_6) |
| `batch_localization_euler_classes()` | Tangent Euler classes at fixed points | Product formula |
| `batch_matroid_ranks()` | Matroid rank via bitmask popcount | Bitmask intersection |
| `batch_csm_euler_characteristics()` | CSM Euler characteristics | Cell decomposition |
| `batch_operad_multiplicities()` | Operadic composition multiplicities | Codimension matching |
| `batch_stability_phases()` | Stability phases | Normalized atan2 |
| `batch_stability_checks()` | Stability checks (phase in (0,1)) | Phase interval test |

### Probabilistic GPU Acceleration

```rust
use amari_gpu::probabilistic::GpuProbabilistic;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize GPU probabilistic operations
    let gpu_prob = GpuProbabilistic::new().await?;

    // Batch sample 10,000 Gaussians on GPU
    let samples = gpu_prob.batch_sample_gaussian(10000, 0.0, 1.0).await?;
    println!("Generated {} samples", samples.len());

    // Compute batch statistics
    let mean = gpu_prob.batch_mean(&samples).await?;
    let variance = gpu_prob.batch_variance(&samples).await?;
    println!("Sample mean: {:.4}, variance: {:.4}", mean, variance);

    Ok(())
}
```

#### Probabilistic GPU Operations

| Operation | Description | GPU Threshold |
|-----------|-------------|---------------|
| `batch_sample_gaussian()` | Parallel Box-Muller Gaussian sampling | ≥ 1000 samples |
| `batch_mean()` | Parallel reduction for mean | ≥ 1000 elements |
| `batch_variance()` | Two-pass parallel variance | ≥ 1000 elements |

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
| Scalar field evaluation | < 1000 points | ≥ 1000 points |
| Vector field evaluation | < 500 points | ≥ 500 points |
| Gradient computation | < 500 points | ≥ 500 points |
| Divergence/Curl | < 500 points | ≥ 500 points |
| Holographic binding | < 100 pairs | ≥ 100 pairs |
| Holographic similarity | < 100 vectors | ≥ 100 vectors |
| Resonator cleanup | < 100 codebook | ≥ 100 codebook |
| Optical field bind | < 4096 pixels | ≥ 4096 pixels (64×64) |
| Optical similarity | < 4096 pixels | ≥ 4096 pixels |
| Lee hologram encoding | < 4096 pixels | ≥ 4096 pixels |
| Gaussian sampling | < 1000 samples | ≥ 1000 samples |
| Batch mean/variance | < 1000 elements | ≥ 1000 elements |
| Distance matrix | < 100 points | ≥ 100 points |
| Morse critical points | < 10000 cells | ≥ 10000 cells |
| Rips filtration | N/A | Uses GPU distance matrix |
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

### Probabilistic Module (v0.13.0)

**GPU Implementations** (✅ Complete):
- Batch Gaussian sampling on multivector spaces
- Parallel mean and variance computation
- Monte Carlo integration acceleration
- GPU-based random number generation with Box-Muller transform

**Types**:
- `GpuHolographicTDC`: GPU-compatible TropicalDualClifford representation
- `GpuResonatorOutput`: Cleanup result with best match info
- `HolographicGpuOps`: Main GPU operations struct

**Shaders**:
- `HOLOGRAPHIC_BATCH_BIND`: 64-thread workgroups for binding
- `HOLOGRAPHIC_BATCH_SIMILARITY`: 256-thread workgroups for similarity
- `HOLOGRAPHIC_BUNDLE_ALL`: Workgroup-shared memory reduction
- `HOLOGRAPHIC_RESONATOR_STEP`: 256-thread parallel max-finding

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

**GPU Implementations** (✅ Complete):
- Distance matrix computation with parallel pairwise Euclidean distance
- Morse critical point detection for 2D scalar fields
- Boundary matrix construction for simplicial complexes
- Column reduction for persistent homology

**Types**:
- `GpuTopology`: GPU context for topology operations
- `GpuCriticalPoint`: Critical point with position, value, type, and index
- `AdaptiveTopologyCompute`: Automatic CPU/GPU dispatch based on workload size
- `GpuTopologyError` / `GpuTopologyResult`: Error handling types

**Shaders**:
- `TOPOLOGY_DISTANCE_MATRIX`: 256-thread workgroups for O(n²) distance computation
- `TOPOLOGY_MORSE_CRITICAL`: 8-neighbor comparison for critical point classification
- `TOPOLOGY_BOUNDARY_MATRIX`: Sparse boundary operator construction
- `TOPOLOGY_MATRIX_REDUCTION`: Standard column reduction algorithm

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
