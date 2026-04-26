# amari-gpu Public API Inventory — First Pass

Date: 2026-03-27
Current version: 0.19.1
Target release: 0.20.0
Scope: `amari-gpu` public API surface

This inventory supports the Amari 0.20.0 goal: make `amari-gpu` comprehensively expose practical Amari operations while keeping public claims honest, tested, and benchmarkable.

## Method

First pass inspected:

- `amari-gpu/src/lib.rs`
- `amari-gpu/Cargo.toml`
- public modules and crate-root re-exports
- visible `pub fn` / `pub async fn` APIs in source modules
- TODO / placeholder / fallback markers in `amari-gpu/src/*.rs`
- `amari-gpu/README.md`

This is not yet a final rustdoc-complete API map. It is a release-planning inventory designed to identify public API honesty risks and high-value coverage/testing priorities.

---

## Classification legend

| Label | Meaning |
|-------|---------|
| **GPU-backed** | Public operation has an actual GPU path/kernel. |
| **Adaptive** | Public operation chooses CPU/GPU based on availability or size. |
| **CPU fallback** | Public operation is useful but currently CPU-backed for all/some paths. |
| **Infrastructure** | Platform/benchmark/profiling/verification utility rather than domain math operation. |
| **Unvalidated** | GPU path exists but needs CPU-baseline and hardware-validation evidence. |
| **API honesty risk** | Public method/module exposes placeholder, redesign-pending, or broader-than-documented behavior. |

---

## Feature and dependency surface

| Feature | Dependency / domain | Current public shape | First-pass classification |
|---------|---------------------|----------------------|---------------------------|
| default | core, info-geom, network, relativistic, infra | broad crate-root + public modules | Mixed: GPU-backed + adaptive + infra; needs full baseline inventory |
| `calculus` | `amari-calculus` | `pub mod calculus`, `GpuCalculus` re-export | API honesty risk: several large-batch paths are placeholder/fallback |
| `measure` | `amari-measure` | `pub mod measure`, integration types re-exported | Mixed GPU-backed + CPU fallback; needs public docs distinction |
| `dual` | `amari-dual` | `pub mod dual` | GPU code exists; some APIs contain placeholder/unsupported paths |
| `fusion` | `amari-fusion` | `pub mod fusion` plus reduced crate-root re-exports | **Critical API honesty risk**: full module is public even though only reduced surface is documented as restored |
| `tropical` | `amari-tropical` | private module + narrow crate-root re-exports | Good v1 shape; real kernels restored and tested |
| `holographic` | `amari-holographic` | `pub mod holographic`, crate-root re-exports | Strong surface; some CPU-for-correctness/future placeholder notes remain |
| `probabilistic` | `amari-probabilistic` | `pub mod probabilistic`, crate-root re-exports | GPU-backed statistical kernels; needs hardware/crossover validation |
| `automata` | `amari-automata` | `pub mod automata` | GPU-backed; needs baseline inventory and public re-export review |
| `enumerative` | `amari-enumerative` | `pub mod enumerative` | Very broad public surface; likely needs validation/classification by operation |
| `functional` | `amari-functional` | `pub mod functional`, crate-root re-exports | Mixed: GPU kernels + CPU fallback for spectral/Jacobi pieces |
| `topology` | `amari-topology` | `pub mod topology`, crate-root re-exports | Mixed: GPU distance/Morse paths + CPU fallback for Betti/Rips pieces |
| `gf2` | `amari-core/gf2` | `pub mod gf2`, crate-root re-exports | Strong candidate for validation; needs hardware report |
| `webgpu` | `wgpu/webgpu` | backend feature | Backend capability only |
| `high-precision` | core/relativistic feature | feature-gated precision | Needs check: GPU kernels mostly f32/f64-oriented |

---

## Crate-root public re-exports

### Always available

| Area | Crate-root exports | Classification |
|------|--------------------|----------------|
| adaptive verification | `AdaptiveVerifier`, `AdaptiveVerification*`, `CpuFeatures`, `GpuBackend`, `Platform*`, `WasmEnvironment` | Infrastructure/adaptive; tests exist, hardware behavior needs validation |
| benchmarks | `AmariMultiGpuBenchmarks`, `BenchmarkConfig`, `BenchmarkResult`, `BenchmarkRunner`, `BenchmarkSuiteResults`, `BenchmarkSummary`, `ScalingAnalysis` | Infrastructure; benchmark engine currently includes simulated workload paths |
| core GA | `GpuCliffordAlgebra`, `AdaptiveCompute`, `GpuError`, `GpuDeviceInfo`, `GpuFisherMatrix` | GPU-backed/adaptive; needs current CPU-baseline matrix |
| info geometry | `GpuInfoGeometry` and related methods from `lib.rs` | GPU/fallback mixed; lacks explicit `info_geom` module boundary |
| multi-GPU | `DeviceId`, `GpuDevice`, `Workload*`, `IntelligentLoadBalancer`, `SynchronizationManager`, `MultiGpuBarrier`, stats types | Infrastructure; needs hardware validation on multi-device setups |
| network | `GpuGeometricNetwork`, `AdaptiveNetworkCompute`, `GpuNetworkError`, `GpuNetworkResult` | GPU-backed distances; some centrality/clustering reuse distance/CPU logic |
| performance | `GpuProfiler`, `AdaptiveDispatchPolicy`, `WorkgroupOptimizer`, profile/report types | Infrastructure; some profiling capabilities depend on adapter support |
| relativistic | `GpuRelativisticPhysics`, `GpuRelativisticParticle`, `GpuSpacetimeVector`, `GpuTrajectoryParams` | GPU-backed Minkowski/trajectory paths; needs baseline validation |
| shaders | `ShaderLibrary`, shader collections | Infrastructure/source exposure |
| timeline | timeline/performance monitor/report types | Infrastructure |
| unified | `GpuContext`, `SharedGpuContext`, `GpuDispatcher`, buffer-pool/result/param types | Infrastructure; central consolidation target |
| verification | `GpuBoundaryVerifier`, `StatisticalGpuVerifier`, `RelativisticVerifier`, `VerifiedMultivector`, config/strategy/error types | Validation infrastructure; important for 0.20.0 |

### Feature-gated crate-root re-exports

| Feature | Re-exports | Classification |
|---------|------------|----------------|
| `calculus` | `GpuCalculus` | API honesty risk until fallback/placeholder behavior is documented/tested |
| `functional` | `AdaptiveFunctionalCompute`, `GpuHilbertSpace`, `GpuMatrixOperator`, `GpuSpectralDecomposition`, error/result types | Mixed GPU + CPU fallback |
| `gf2` | `GF2GpuOps`, GF2 data/error/result types | GPU-backed; validate |
| `fusion` | `FusionGpuError`, `FusionGpuResult`, `GpuHolographicTDC`, `GpuResonatorOutput`, `HolographicGpuOps` | Intended reduced v1 surface, but `pub mod fusion` exposes much more |
| `holographic` | `GpuHolographic`, `GpuHolographicMemory`, `GpuOpticalField`, error/result types | GPU-backed/adaptive; validate broader CPU correctness |
| `measure` | `GpuIntegrator`, `GpuMonteCarloIntegrator`, `GpuMultidimIntegrator`, `GpuParametricDensity`, `GpuTropicalMeasure` | Mixed GPU + CPU fallback |
| `probabilistic` | `GpuProbabilistic`, error/result types | GPU-backed; validate |
| `tropical` | `TropicalExecutionPath`, `TropicalGpuError`, `TropicalGpuOps`, `TropicalGpuResult` | Narrow v1 public surface; good API honesty state |
| `topology` | `AdaptiveTopologyCompute`, `GpuCriticalPoint`, `GpuTopology`, error/result types | Mixed GPU + CPU fallback |

---

## Public module exposure risk

Rust `pub mod` exposes every `pub` item inside the module, even if crate-root re-exports are narrow.

### Current public modules that expose entire module contents

- `adaptive`
- `automata` with feature
- `benchmarks`
- `calculus` with feature
- `dual` with feature
- `enumerative` with feature
- `functional` with feature
- `gf2` with feature
- `fusion` with feature
- `holographic` with feature
- `measure` with feature
- `multi_gpu`
- `network`
- `performance`
- `probabilistic` with feature
- `relativistic`
- `shaders`
- `timeline`
- `topology` with feature
- `unified`
- `verification`

### Private module with narrow re-exports

- `tropical` with feature

This is currently the cleanest model for a restored but not fully mature domain surface.

---

## Critical findings

## 1. Fusion reduced-surface intent is not actually enforced

The roadmap and README describe fusion as a reduced first public surface centered on `HolographicGpuOps`, but `lib.rs` currently has:

```rust
#[cfg(feature = "fusion")]
pub mod fusion;
```

That means broader public items are exposed, including but not limited to:

- `FusionGpuOps`
- `FusionGpuContext`
- `LlmEvaluationConfig`
- `GeometricAttentionConfig`
- `FusionOptimizationConfig`
- `LlmEvaluationResult`
- `FusionObjective`
- broader methods such as `llm_evaluation`, `geometric_attention`, and `batch_fusion_optimization`

First-pass classification: **API honesty risk**.

Recommended 0.20.0 fix:

- change `pub mod fusion` to private `mod fusion`
- keep only the intended crate-root re-exports public
- or explicitly classify/test/document the broader fusion module before leaving it public

## 2. Tropical v1 is currently the cleanest restored domain pattern

`tropical` is private and exposes only selected crate-root items:

- `TropicalGpuOps`
- `TropicalExecutionPath`
- `TropicalGpuError`
- `TropicalGpuResult`

Public methods currently backed by real logic:

- `matrix_multiply`
- `matrix_multiply_adaptive`
- `matrix_multiply_execution_path`
- `should_use_gpu_for_matrix_multiply`
- `attention_scores`

First-pass classification: **good v1 API honesty state**, pending hardware validation and broader benchmarks.

Recommended pattern:

- use the tropical model for future partial restorations where full module contents are not yet release-ready

## 3. Calculus has public large-batch paths that appear placeholder/fallback-heavy

`GpuCalculus` public methods advertise GPU acceleration, but TODO markers show large-batch/internal GPU paths still include placeholders or CPU fallback behavior.

Public methods needing inspection:

- `batch_eval_scalar_field`
- `batch_eval_vector_field`
- `batch_gradient`
- `batch_divergence`
- `batch_curl`

First-pass classification: **API honesty risk / CPU fallback mixed**.

Recommended 0.20.0 fix:

- either implement real kernels, or explicitly document as adaptive/CPU-fallback until GPU kernels are complete
- add CPU-baseline tests for public behavior

## 4. Benchmarks module includes simulated operation paths

`benchmarks.rs` includes comments indicating simulated operations for benchmark purposes.

First-pass classification: **infrastructure with simulation caveat**.

Recommended 0.20.0 fix:

- ensure benchmark docs distinguish simulated framework validation from real kernel benchmarks
- add real per-domain benchmark harnesses for restored kernels

## 5. Several modules expose useful CPU fallback under GPU names

Examples found by marker scan:

- `measure::GpuIntegrator::integrate_values` uses CPU summation
- `measure::GpuTropicalMeasure::{supremum, infimum}` use CPU reduction
- `measure::GpuMultidimIntegrator::monte_carlo_nd` uses CPU implementation
- `functional::GpuSpectralDecomposition::compute` falls back to CPU Jacobi algorithm
- `functional::GpuSpectralDecomposition::apply_function_batch` uses CPU implementation
- `topology::build_rips_filtration` uses CPU implementation with GPU distance matrix
- `topology::compute_betti_numbers` falls back to CPU
- `network` centrality/clustering reuses GPU distance or CPU logic
- `holographic::batch_unbind` uses CPU for correctness

First-pass classification: **mixed GPU/fallback**.

This is not necessarily bad, but it must be documented and tested honestly.

---

## First-pass domain classification

| Domain / module | Current public state | First-pass release classification | 0.20.0 action |
|-----------------|----------------------|-----------------------------------|---------------|
| core GA in `lib.rs` | public default | GPU-backed/adaptive | add/verify CPU baseline tests and hardware validation |
| info geometry in `lib.rs` | public default | GPU-backed/fallback mixed | consider explicit module boundary; validate tensor/fisher/divergence |
| `network` | public default | GPU-backed distances, mixed higher ops | validate and document centrality/clustering fallback behavior |
| `relativistic` | public default | GPU-backed | validate Minkowski/trajectory CPU parity |
| `holographic` | feature public module | GPU-backed/adaptive + CPU correctness path | validate all public ops; document CPU unbind path if retained |
| `fusion` | feature public module | intended reduced surface, actually broad | **fix module exposure or validate/document broad API** |
| `tropical` | feature narrow crate-root API | real restored v1 kernels | hardware validate and benchmark further |
| `calculus` | feature public module | API honesty risk | inspect/fix placeholder GPU paths |
| `measure` | feature public module | mixed GPU/fallback | document/test fallback methods |
| `functional` | feature public module | mixed GPU/fallback | document/test CPU spectral fallback |
| `topology` | feature public module | mixed GPU/fallback | validate distance/Morse; document CPU Rips/Betti paths |
| `dual` | feature public module | GPU-backed plus unsupported paths | inspect shader correctness and placeholder gradient paths |
| `enumerative` | feature very broad public module | broad/unvalidated | prioritize representative CPU-baseline tests |
| `automata` | feature public module | GPU-backed | validate against CPU automata baselines |
| `probabilistic` | feature public module | GPU-backed | validate statistical correctness/tolerances |
| `gf2` | feature public module | GPU-backed | validate exact CPU parity |
| infra modules | public default | infrastructure | keep, document simulation/profiling caveats |

---

## Immediate 0.20.0 follow-up tasks

1. **Fix fusion public exposure**
   - [ ] decide whether to make `fusion` private with narrow crate-root re-exports
   - [ ] or validate/document the broad `fusion` module as public

2. **Audit calculus public methods**
   - [ ] identify which methods are real GPU-backed vs CPU fallback vs placeholder
   - [ ] update docs and tests accordingly

3. **Create per-domain validation checklist**
   - [ ] one CPU baseline test per high-priority public operation
   - [ ] one public import-path integration test per feature domain

4. **Document fallback semantics**
   - [ ] README table should distinguish GPU-backed, adaptive, CPU fallback, and infrastructure

5. **Hardware validation template**
   - [ ] GB10 result table
   - [ ] RTX 5080 result table

---

## Recommended next code change

The highest-impact immediate code cleanup is:

> Make `fusion` follow the `tropical` pattern: private module plus narrow crate-root re-exports, unless the broader `FusionGpuOps` API is fully validated and intended for 0.20.0.

This would align implementation with the documented reduced fusion restoration plan and prevent accidental public exposure of unvalidated broader APIs.
