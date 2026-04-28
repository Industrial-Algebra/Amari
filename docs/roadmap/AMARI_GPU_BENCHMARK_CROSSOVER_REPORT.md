# amari-gpu Benchmark and Crossover Report

Date: 2026-04-28
Initial hardware: DGX Spark / NVIDIA GB10 (`great-attractor`)
Driver/CUDA: NVIDIA `580.142`, CUDA `13.0`
Scope: initial manual CPU-vs-GPU crossover data for restored high-priority kernels.

This document is deliberately separate from correctness validation. Hardware validation reports prove that public API tests pass on GB10 and RTX 5080; this file records when GPU acceleration starts to pay off.

## Methodology

- Commands are manual/ignored benchmark harnesses, not CI tests.
- Timings are wall-clock elapsed times from Rust `Instant` around complete public API calls.
- GPU timings include command submission and readback overhead, so small cases are expected to favor CPU.
- Values are representative snapshots, not statistically rigorous criterion results.
- Use serial execution for benchmark runs to avoid GPU context contention.

## Commands

### Core GA batch geometric product

```bash
cargo +stable test -p amari-gpu \
  --test core_ga_benchmark_crossover \
  -- --ignored --nocapture --test-threads=1
```

### Tropical matrix multiply

```bash
cargo +stable test -p amari-gpu --features tropical \
  tropical::tests::benchmark_tropical_matrix_multiply_cpu_vs_gpu \
  -- --ignored --nocapture --test-threads=1
```

For RTX 5080 laptop validation, use the same commands with explicit Vulkan backend:

```bash
WGPU_BACKEND=vulkan cargo +stable test ... -- --ignored --nocapture --test-threads=1
```

## GB10 results

### Core GA: `GpuCliffordAlgebra::batch_geometric_product::<Cl(3,0,0)>`

Benchmark harness: `amari-gpu/tests/core_ga_benchmark_crossover.rs`

| Batch size | CPU avg ms | GPU avg ms | Speedup | Correct |
|------------|------------|------------|---------|---------|
| 16 | 0.093 | 0.326 | 0.29× | yes |
| 64 | 0.369 | 0.325 | 1.14× | yes |
| 256 | 1.480 | 0.421 | 3.51× | yes |
| 1024 | 5.890 | 0.989 | 5.95× | yes |
| 4096 | 23.649 | 1.957 | 12.08× | yes |

**Observed crossover:** around batch size `64` for Cl(3,0,0) flat batches on GB10.

**Release guidance:** keep CPU for very small batches; GPU is useful by roughly `>= 64` complete Cl(3,0,0) products on this hardware, with strong gains by `>= 256`.

### Tropical: dense max-plus matrix multiply

Benchmark harness: ignored `tropical::tests::benchmark_tropical_matrix_multiply_cpu_vs_gpu`

| Dimensions `(m×k×n)` | CPU avg ms | GPU avg ms | Speedup | Correct |
|----------------------|------------|------------|---------|---------|
| 16×16×16 | 0.101 | 2.515 | 0.04× | yes |
| 32×32×32 | 0.741 | 2.501 | 0.30× | yes |
| 64×64×64 | 5.772 | 2.691 | 2.14× | yes |
| 128×128×128 | 45.954 | 3.848 | 11.94× | yes |

**Observed crossover:** between `32³` and `64³`; current adaptive heuristic choosing GPU for `64×64×64` is appropriate on GB10.

**Release guidance:** keep CPU for small tropical matrices; GPU is useful by `64×64×64` and very beneficial by `128×128×128` for the current kernel.

## Current crossover guidance summary

| Operation | Current public path | GB10 crossover snapshot | Guidance |
|-----------|---------------------|--------------------------|----------|
| Core GA batch geometric product, Cl(3,0,0) | GPU-backed batch kernel | ~64 complete products | CPU for tiny batches; GPU for medium/large flat batches |
| Tropical dense max-plus matmul | GPU-backed kernel + adaptive dispatch | between 32³ and 64³ | CPU below 64³; GPU at/above 64³ on GB10 |
| Tropical attention scores | GPU-backed winner-takes-all scores | pending | Need dedicated harness |
| Holographic ProductCl3x32 bind/similarity | GPU-backed for batches >= 100 | pending | Expected GPU only for larger batches due setup/readback overhead |
| Optical bind/similarity/Lee encoding | GPU-backed above field-size heuristic | pending | Need field-size sweep |
| GF(2) kernels | GPU-backed fixed-layout kernels | pending | Need batch-size sweep for Clifford/matvec/Hamming |
| Probabilistic sampling/statistics | GPU-backed after context; small stats CPU fallback | pending | Need dimension/sample-count sweep |
| Topology distance/Morse | mixed GPU-backed + CPU fallback | pending | Need point-count/grid-size sweep |
| Automata rule/energy | GPU-backed rule/energy, CPU neighborhood fallback | pending | Need grid-size/step-count sweep |
| Measure built-ins | mixed GPU-backed integrators/densities | pending | Need sample-count sweep |
| Functional matrix batches | GPU-backed batch apply/Hilbert, CPU spectral fallback | pending | Need matrix-size/batch sweep |
| Network distances/centrality/clustering | GPU distance + CPU reductions | pending | Need node-count/embedding-size sweep |

## Manual benchmark harness inventory

Implemented in this pass:

- `amari-gpu/tests/core_ga_benchmark_crossover.rs`
  - ignored manual benchmark for Cl(3,0,0) batch geometric products
  - checks GPU output against CPU baseline before printing timing rows

Already present:

- ignored tropical matrix multiply benchmark in `amari-gpu/src/tropical.rs`
  - checks GPU output against CPU baseline before printing timing rows

Still needed:

- dedicated harnesses for holographic, optical, GF(2), probabilistic, topology, automata, measure, functional, and network public kernels.
- RTX 5080 benchmark run using `WGPU_BACKEND=vulkan`.

## Caveats

- These timings are not comparable across debug/release modes. Current numbers are from test-profile execution, matching the manual harness invocation.
- Criterion-style benchmarks or release-mode examples would be better for final public performance claims.
- For 0.20.0, the goal is honest crossover guidance, not absolute peak performance marketing.
- GPU setup/readback dominates small inputs; adaptive thresholds should remain conservative until per-domain release-mode benchmarks are available.
