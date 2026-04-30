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

Additional harnesses use the same ignored-test pattern:

```bash
cargo +stable test -p amari-gpu --features tropical \
  --test tropical_attention_benchmark_crossover \
  -- --ignored --nocapture --test-threads=1
cargo +stable test -p amari-gpu --features holographic \
  --test holographic_benchmark_crossover \
  -- --ignored --nocapture --test-threads=1
cargo +stable test -p amari-gpu --features gf2 \
  --test gf2_benchmark_crossover \
  -- --ignored --nocapture --test-threads=1
cargo +stable test -p amari-gpu --features probabilistic \
  --test probabilistic_benchmark_crossover \
  -- --ignored --nocapture --test-threads=1
cargo +stable test -p amari-gpu --features topology \
  --test topology_benchmark_crossover \
  -- --ignored --nocapture --test-threads=1
cargo +stable test -p amari-gpu --features automata \
  --test automata_benchmark_crossover \
  -- --ignored --nocapture --test-threads=1
cargo +stable test -p amari-gpu --features measure \
  --test measure_benchmark_crossover \
  -- --ignored --nocapture --test-threads=1
cargo +stable test -p amari-gpu --features functional \
  --test functional_benchmark_crossover \
  -- --ignored --nocapture --test-threads=1
cargo +stable test -p amari-gpu \
  --test network_benchmark_crossover \
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


### Tropical: attention scores

Benchmark harness: `amari-gpu/tests/tropical_attention_benchmark_crossover.rs`

| Rows×cols | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|------------|------------|---------|---------|
| 16×64 | 0.050 | 2.068 | 0.02× | yes |
| 64×64 | 0.198 | 2.125 | 0.09× | yes |
| 128×128 | 0.770 | 2.998 | 0.26× | yes |
| 256×256 | 3.038 | 6.331 | 0.48× | yes |

**Observed crossover:** none through `256×256` for the current kernel/test-profile harness.

**Release guidance:** treat GPU attention scores as correctness-restored but not yet a default performance win at these sizes.

### Holographic: ProductCl3x32 bind/similarity

Benchmark harness: `amari-gpu/tests/holographic_benchmark_crossover.rs`

| Operation | Batch | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|-------|------------|------------|---------|---------|
| bind | 16 | 0.103 | 0.151 | 0.68× | yes |
| bind | 100 | 0.640 | 0.957 | 0.67× | yes |
| bind | 512 | 3.368 | 4.238 | 0.79× | yes |
| bind | 2048 | 13.530 | 15.507 | 0.87× | yes |
| similarity | 16 | 0.126 | 0.160 | 0.79× | yes |
| similarity | 100 | 0.787 | 0.768 | 1.02× | yes |
| similarity | 512 | 4.036 | 3.493 | 1.16× | yes |
| similarity | 2048 | 16.139 | 12.176 | 1.33× | yes |

**Observed crossover:** similarity crosses over near batch `100`; bind did not cross over through batch `2048` in this debug/test-profile sweep.

### Optical holographic GPU timings

The optical harness currently records GPU-path timings and correctness invariants only; CPU baseline timings are still pending.

| Operation | Field size | GPU avg ms | Correct |
|-----------|------------|------------|---------|
| optical bind | 256 | 0.028 | yes |
| optical similarity | 256 | 0.015 | yes |
| Lee encode | 256 | 0.023 | yes |
| optical bind | 4096 | 0.634 | yes |
| optical similarity | 4096 | 0.398 | yes |
| Lee encode | 4096 | 0.807 | yes |
| optical bind | 16384 | 1.791 | yes |
| optical similarity | 16384 | 1.121 | yes |
| Lee encode | 16384 | 2.727 | yes |

### GF(2): Clifford, matvec, and Hamming kernels

Benchmark harness: `amari-gpu/tests/gf2_benchmark_crossover.rs`

| Operation | Batch | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|-------|------------|------------|---------|---------|
| Clifford one-hot | 16 | 0.000 | 3.598 | 0.00× | yes |
| Clifford one-hot | 4096 | 0.067 | 3.818 | 0.02× | yes |
| matvec 16×16 | 16 | 0.002 | 4.235 | 0.00× | yes |
| matvec 16×16 | 4096 | 0.501 | 4.367 | 0.11× | yes |
| Hamming 64-bit | 16 | 0.001 | 3.112 | 0.00× | yes |
| Hamming 64-bit | 4096 | 0.204 | 3.084 | 0.07× | yes |

**Observed crossover:** none through batch `4096` for these small fixed-layout inputs; CPU bit operations are extremely cheap.

### Probabilistic: sampling/statistics

Benchmark harness: `amari-gpu/tests/probabilistic_benchmark_crossover.rs`

| Operation | Samples×dim | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|-------------|------------|------------|---------|---------|
| mean | 16×8 | 0.002 | 0.002 | 0.97× | yes |
| mean | 8192×8 | 1.103 | 1.931 | 0.57× | yes |
| variance | 16×8 | 0.002 | 0.003 | 0.79× | yes |
| variance | 8192×8 | 1.022 | 1.426 | 0.72× | yes |
| deterministic Gaussian | 16×8 | 0.001 | 0.037 | 0.02× | yes |
| deterministic Gaussian | 8192×8 | 0.311 | 19.609 | 0.02× | yes |

**Observed crossover:** none through `8192×8`; mean/variance are close enough to revisit with release-mode reductions, but current sampling path is not a performance win for zero-std deterministic sampling.

### Topology: distance matrix and critical points

Benchmark harness: `amari-gpu/tests/topology_benchmark_crossover.rs`

| Operation | Size | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|------|------------|------------|---------|---------|
| distance matrix | 16 points | 0.006 | 0.282 | 0.02× | yes |
| distance matrix | 64 points | 0.073 | 0.319 | 0.23× | yes |
| distance matrix | 256 points | 1.311 | 0.904 | 1.45× | yes |
| distance matrix | 512 points | 5.093 | 2.253 | 2.26× | yes |
| critical points 2D | 256 cells | — | 0.302 | — | yes |
| critical points 2D | 4096 cells | — | 0.330 | — | yes |
| critical points 2D | 16384 cells | — | 0.376 | — | yes |

**Observed crossover:** distance-matrix crossover between `64` and `256` points; critical-point CPU baseline timings still need to be added.

### Automata: rule application, energy, and evolution

Benchmark harness: `amari-gpu/tests/automata_benchmark_crossover.rs`

| Operation | Cells | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|-------|------------|------------|---------|---------|
| apply rules | 256 | 0.007 | 0.245 | 0.03× | yes |
| apply rules | 4096 | 0.097 | 0.252 | 0.39× | yes |
| apply rules | 16384 | 0.382 | 0.635 | 0.60× | yes |
| energy | 256 | 0.002 | 0.274 | 0.01× | yes |
| energy | 4096 | 0.035 | 0.883 | 0.04× | yes |
| energy | 16384 | 0.140 | 2.781 | 0.05× | yes |
| evolve CA | 256 | — | 0.310 | — | yes |
| evolve CA | 4096 | — | 0.546 | — | yes |
| evolve CA | 16384 | — | 0.821 | — | yes |

**Observed crossover:** none through `16384` cells for rule/energy in debug/test profile; rule application approaches parity at the largest tested size.

### Measure: built-ins, densities, tropical reductions

Benchmark harness: `amari-gpu/tests/measure_benchmark_crossover.rs`

| Operation | Size | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|------|------------|------------|---------|---------|
| integrate x² | 1000 | 0.014 | 0.246 | 0.06× | yes |
| integrate x² | 10000 | 0.137 | 0.305 | 0.45× | yes |
| integrate x² | 100000 | 1.372 | 0.724 | 1.90× | yes |
| Gaussian density | 256 | 0.003 | 0.313 | 0.01× | yes |
| Gaussian density | 4096 | 0.039 | 0.314 | 0.12× | yes |
| Gaussian density | 65536 | 0.645 | 0.343 | 1.88× | yes |
| tropical supremum | 65536 | 0.554 | 0.554 | 1.00× | yes |
| tropical infimum | 65536 | 0.555 | 0.555 | 1.00× | yes |

**Observed crossover:** integration and Gaussian density cross over between `10000` and `100000` / between `4096` and `65536`; tropical reductions are CPU fallbacks and track CPU timing.

### Functional: matrix apply and Hilbert inner products

Benchmark harness: `amari-gpu/tests/functional_benchmark_crossover.rs`

| Operation | Batch | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|-------|------------|------------|---------|---------|
| matrix apply | 16 | 0.009 | 0.252 | 0.04× | yes |
| matrix apply | 64 | 0.036 | 0.293 | 0.12× | yes |
| matrix apply | 256 | 0.143 | 0.856 | 0.17× | yes |
| matrix apply | 1024 | 0.571 | 0.889 | 0.64× | yes |
| matrix apply | 4096 | 2.202 | 2.183 | 1.01× | yes |
| Hilbert inner | 4096 | 0.976 | 2.496 | 0.39× | yes |

**Observed crossover:** matrix apply reaches parity around batch `4096`; Hilbert inner products did not cross over through `4096`.

### Network: distances, centrality, clustering

Benchmark harness: `amari-gpu/tests/network_benchmark_crossover.rs`

| Operation | Nodes | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|-------|------------|------------|---------|---------|
| distances | 16 | 0.004 | 0.174 | 0.02× | yes |
| distances | 64 | 0.040 | 0.230 | 0.17× | yes |
| distances | 128 | 0.144 | 0.547 | 0.26× | yes |
| distances | 256 | 0.558 | 1.296 | 0.43× | yes |
| centrality | 256 | 0.287 | 1.659 | 0.17× | yes |
| clustering | 256 | — | 2.335 | — | yes |

**Observed crossover:** none through `256` nodes for this public API path; larger node-count and release-mode sweeps are needed before claiming acceleration.


## RTX 5080 results

Hardware: NVIDIA GeForce RTX 5080 Laptop GPU (`rindler`)
Backend: `WGPU_BACKEND=vulkan`
Driver/CUDA: NVIDIA `580.126.09`, CUDA `13.0`
Execution: serial ignored benchmark tests with `-- --ignored --nocapture --test-threads=1`.

### Core GA: `GpuCliffordAlgebra::batch_geometric_product::<Cl(3,0,0)>`

| Batch size | CPU avg ms | GPU avg ms | Speedup | Correct |
|------------|------------|------------|---------|---------|
| 16 | 0.066 | 0.333 | 0.20× | yes |
| 64 | 0.270 | 0.349 | 0.77× | yes |
| 256 | 1.127 | 0.469 | 2.40× | yes |
| 1024 | 4.440 | 0.679 | 6.54× | yes |
| 4096 | 13.630 | 1.443 | 9.44× | yes |

**Observed crossover:** between batch `64` and `256` on RTX 5080. This is later than GB10's ~`64` crossover because the RTX 5080 CPU baseline is faster in this test-profile run.

### Tropical: dense max-plus matrix multiply

| Dimensions `(m×k×n)` | CPU avg ms | GPU avg ms | Speedup | Correct |
|----------------------|------------|------------|---------|---------|
| 16×16×16 | 0.058 | 3.633 | 0.02× | yes |
| 32×32×32 | 0.438 | 3.776 | 0.12× | yes |
| 64×64×64 | 3.406 | 3.826 | 0.89× | yes |
| 128×128×128 | 27.885 | 4.552 | 6.13× | yes |

**Observed crossover:** between `64³` and `128³` on RTX 5080. This is later than GB10, where `64³` already wins.

### Tropical: attention scores

| Rows×cols | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|------------|------------|---------|---------|
| 16×64 | 0.031 | 3.980 | 0.01× | yes |
| 64×64 | 0.116 | 3.210 | 0.04× | yes |
| 128×128 | 0.460 | 4.076 | 0.11× | yes |
| 256×256 | 1.837 | 6.448 | 0.28× | yes |

**Observed crossover:** none through `256×256`.

### Holographic: ProductCl3x32 bind/similarity

| Operation | Batch | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|-------|------------|------------|---------|---------|
| bind | 16 | 0.047 | 0.077 | 0.61× | yes |
| bind | 100 | 0.302 | 1.881 | 0.16× | yes |
| bind | 512 | 1.614 | 3.499 | 0.46× | yes |
| bind | 2048 | 6.826 | 11.119 | 0.61× | yes |
| similarity | 16 | 0.089 | 0.109 | 0.82× | yes |
| similarity | 100 | 0.571 | 1.341 | 0.43× | yes |
| similarity | 512 | 2.879 | 2.647 | 1.09× | yes |
| similarity | 2048 | 11.127 | 8.697 | 1.28× | yes |

**Observed crossover:** similarity crosses over around batch `512`; bind did not cross over through batch `2048`.

### Optical holographic GPU timings

| Operation | Field size | GPU avg ms | Correct |
|-----------|------------|------------|---------|
| optical bind | 256 | 0.017 | yes |
| optical similarity | 256 | 0.009 | yes |
| Lee encode | 256 | 0.015 | yes |
| optical bind | 4096 | 0.826 | yes |
| optical similarity | 4096 | 0.463 | yes |
| Lee encode | 4096 | 1.354 | yes |
| optical bind | 16384 | 1.597 | yes |
| optical similarity | 16384 | 1.674 | yes |
| Lee encode | 16384 | 1.658 | yes |

CPU baseline timings are still pending for optical operations.

### GF(2): Clifford, matvec, and Hamming kernels

| Operation | Batch | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|-------|------------|------------|---------|---------|
| Clifford one-hot | 16 | 0.000 | 4.240 | 0.00× | yes |
| Clifford one-hot | 4096 | 0.061 | 3.753 | 0.02× | yes |
| matvec 16×16 | 16 | 0.001 | 3.968 | 0.00× | yes |
| matvec 16×16 | 4096 | 0.306 | 4.571 | 0.07× | yes |
| Hamming 64-bit | 16 | 0.000 | 4.020 | 0.00× | yes |
| Hamming 64-bit | 4096 | 0.102 | 4.045 | 0.03× | yes |

**Observed crossover:** none through batch `4096`.

### Probabilistic: sampling/statistics

| Operation | Samples×dim | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|-------------|------------|------------|---------|---------|
| mean | 16×8 | 0.001 | 0.001 | 1.02× | yes |
| mean | 8192×8 | 0.703 | 0.916 | 0.77× | yes |
| variance | 16×8 | 0.001 | 0.001 | 0.78× | yes |
| variance | 8192×8 | 0.505 | 1.050 | 0.48× | yes |
| deterministic Gaussian | 16×8 | 0.000 | 0.035 | 0.01× | yes |
| deterministic Gaussian | 8192×8 | 0.201 | 18.520 | 0.01× | yes |

**Observed crossover:** none through `8192×8`; mean approaches parity at the largest tested size.

### Topology: distance matrix and critical points

| Operation | Size | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|------|------------|------------|---------|---------|
| distance matrix | 16 points | 0.002 | 0.300 | 0.01× | yes |
| distance matrix | 64 points | 0.026 | 0.302 | 0.09× | yes |
| distance matrix | 256 points | 0.409 | 0.853 | 0.48× | yes |
| distance matrix | 512 points | 1.725 | 2.007 | 0.86× | yes |
| critical points 2D | 256 cells | — | 0.543 | — | yes |
| critical points 2D | 4096 cells | — | 0.432 | — | yes |
| critical points 2D | 16384 cells | — | 0.515 | — | yes |

**Observed crossover:** none through `512` points on RTX 5080, though distance matrix approaches parity at `512`.

### Automata: rule application, energy, and evolution

| Operation | Cells | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|-------|------------|------------|---------|---------|
| apply rules | 256 | 0.006 | 0.745 | 0.01× | yes |
| apply rules | 4096 | 0.090 | 0.381 | 0.24× | yes |
| apply rules | 16384 | 0.433 | 0.896 | 0.48× | yes |
| energy | 256 | 0.002 | 0.379 | 0.00× | yes |
| energy | 4096 | 0.031 | 0.562 | 0.05× | yes |
| energy | 16384 | 0.105 | 1.601 | 0.07× | yes |
| evolve CA | 256 | — | 0.292 | — | yes |
| evolve CA | 4096 | — | 0.440 | — | yes |
| evolve CA | 16384 | — | 1.008 | — | yes |

**Observed crossover:** none through `16384` cells.

### Measure: built-ins, densities, tropical reductions

| Operation | Size | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|------|------------|------------|---------|---------|
| integrate x² | 1000 | 0.007 | 0.462 | 0.02× | yes |
| integrate x² | 10000 | 0.072 | 0.440 | 0.16× | yes |
| integrate x² | 100000 | 0.739 | 0.818 | 0.90× | yes |
| Gaussian density | 256 | 0.002 | 0.661 | 0.00× | yes |
| Gaussian density | 4096 | 0.030 | 0.438 | 0.07× | yes |
| Gaussian density | 65536 | 0.487 | 0.570 | 0.85× | yes |
| tropical supremum | 65536 | 0.454 | 0.359 | 1.27× | yes |
| tropical infimum | 65536 | 0.454 | 0.361 | 1.25× | yes |

**Observed crossover:** integration and Gaussian density approach parity but do not cross over at the largest tested sizes; tropical reductions are CPU fallbacks and track CPU timing.

### Functional: matrix apply and Hilbert inner products

| Operation | Batch | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|-------|------------|------------|---------|---------|
| matrix apply | 16 | 0.005 | 1.313 | 0.00× | yes |
| matrix apply | 64 | 0.020 | 1.519 | 0.01× | yes |
| matrix apply | 256 | 0.076 | 0.878 | 0.09× | yes |
| matrix apply | 1024 | 0.290 | 0.867 | 0.33× | yes |
| matrix apply | 4096 | 1.395 | 1.818 | 0.77× | yes |
| Hilbert inner | 4096 | 0.497 | 2.123 | 0.23× | yes |

**Observed crossover:** none through batch `4096`; matrix apply approaches parity.

### Network: distances, centrality, clustering

| Operation | Nodes | CPU avg ms | GPU avg ms | Speedup | Correct |
|-----------|-------|------------|------------|---------|---------|
| distances | 16 | 0.002 | 0.410 | 0.01× | yes |
| distances | 64 | 0.027 | 0.445 | 0.06× | yes |
| distances | 128 | 0.089 | 0.536 | 0.17× | yes |
| distances | 256 | 0.352 | 0.915 | 0.39× | yes |
| centrality | 256 | 0.219 | 1.745 | 0.13× | yes |
| clustering | 256 | — | 1.729 | — | yes |

**Observed crossover:** none through `256` nodes.

## Current crossover guidance summary

| Operation | Current public path | GB10 snapshot | RTX 5080 snapshot | Guidance |
|-----------|---------------------|---------------|-------------------|----------|
| Core GA batch geometric product, Cl(3,0,0) | GPU-backed batch kernel | ~64 complete products | between 64 and 256 | CPU for tiny batches; GPU for medium/large flat batches |
| Tropical dense max-plus matmul | GPU-backed kernel + adaptive dispatch | between 32³ and 64³ | between 64³ and 128³ | CPU below 64³; GPU is clearly useful by 128³ on both hardware targets |
| Tropical attention scores | GPU-backed winner-takes-all scores | none through 256×256 | none through 256×256 | CPU default until larger/release-mode sweeps justify GPU |
| Holographic ProductCl3x32 bind | GPU-backed bind | none through batch 2048 | none through batch 2048 | CPU may remain better for basis-like sparse batches |
| Holographic ProductCl3x32 similarity | GPU-backed similarity | ~100 vectors | ~512 vectors | GPU starts helping for larger similarity batches |
| Optical bind/similarity/Lee encoding | GPU-backed above field-size heuristic | GPU timings only | GPU timings only | Add CPU baseline before final crossover guidance |
| GF(2) kernels | GPU-backed fixed-layout kernels | none through batch 4096 | none through batch 4096 | CPU bit ops remain preferred for small fixed-layout workloads |
| Probabilistic sampling/statistics | GPU-backed after context; small stats CPU fallback | none through 8192×8 | none through 8192×8 | Revisit with release-mode reductions; CPU preferred for current harness sizes |
| Topology distance matrix | GPU-backed distance matrix | between 64 and 256 points | none through 512 points | GPU useful on GB10 for medium point clouds; RTX 5080 needs larger/release sweeps |
| Topology critical points | GPU-backed discrete Morse path | GPU timings only | GPU timings only | Add CPU critical-point timing baseline |
| Automata rule/energy | GPU-backed rule/energy, CPU neighborhood fallback | none through 16384 cells | none through 16384 cells | Rule path approaches parity at largest tested size; CPU preferred for now |
| Measure built-ins/densities | mixed GPU-backed integrators/densities | crosses only at large sample/value counts | approaches parity but no crossover at tested sizes | GPU useful only for large batches and hardware-dependent; keep conservative thresholds |
| Functional matrix batches | GPU-backed batch apply/Hilbert, CPU spectral fallback | matrix apply ~4096; Hilbert none through 4096 | matrix apply approaches parity; Hilbert none through 4096 | GPU only for large matrix-apply batches after hardware-specific thresholding |
| Network distances/centrality/clustering | GPU distance + CPU reductions | none through 256 nodes | none through 256 nodes | Need larger/release sweeps before GPU default claims |

## Manual benchmark harness inventory

Implemented manual/ignored harnesses:

- `amari-gpu/tests/core_ga_benchmark_crossover.rs`
- `amari-gpu/tests/tropical_attention_benchmark_crossover.rs`
- `amari-gpu/tests/holographic_benchmark_crossover.rs`
- `amari-gpu/tests/gf2_benchmark_crossover.rs`
- `amari-gpu/tests/probabilistic_benchmark_crossover.rs`
- `amari-gpu/tests/topology_benchmark_crossover.rs`
- `amari-gpu/tests/automata_benchmark_crossover.rs`
- `amari-gpu/tests/measure_benchmark_crossover.rs`
- `amari-gpu/tests/functional_benchmark_crossover.rs`
- `amari-gpu/tests/network_benchmark_crossover.rs`

Already present:

- ignored tropical matrix multiply benchmark in `amari-gpu/src/tropical.rs`

Remaining benchmark work:

- add CPU baseline timings for optical holographic operations, topology critical points, automata CA evolution, and network clustering.
- add any missing RTX 5080 rows if additional larger-size/release-mode sweeps are run.
- repeat selected sweeps in release-mode or Criterion-style harnesses before making external performance claims.

## Caveats

- These timings are not comparable across debug/release modes. Current numbers are from test-profile execution, matching the manual harness invocation.
- Criterion-style benchmarks or release-mode examples would be better for final public performance claims.
- For 0.20.0, the goal is honest crossover guidance, not absolute peak performance marketing.
- GPU setup/readback dominates small inputs; adaptive thresholds should remain conservative until per-domain release-mode benchmarks are available.
