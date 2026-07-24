# Amari 0.20.0 Release Plan — amari-gpu Coverage, Validation, and API Honesty

Date: 2026-03-27
Current version: 0.19.1
Target release: 0.20.0
Scope: `amari-gpu`

## Active release decision

For Amari 0.20.0, active development focus is now **`amari-gpu`**.

The goal is not merely to fix one module. The goal is to make `amari-gpu` a broad, honest, thoroughly-tested GPU integration layer that exposes as many Amari operations as is practical while avoiding unvalidated or placeholder public claims.

The following tracks are explicitly deferred for now:

- `amari-wasm` audit/hardening
- additive extension work in non-GPU crates
- broader `amari-tropical` / `amari-dual` / `amari-fusion` domain-crate redesigns

Those tracks remain documented elsewhere, but they are not the active 0.20.0 implementation focus.

---

## 0.20.0 release thesis

`amari-gpu` 0.20.0 should be a **coverage + validation release**.

It should prioritize:

1. coherent public API exposure
2. real GPU kernels where practical
3. CPU-baseline correctness tests
4. hardware validation on available devices
5. benchmark/crossover reporting
6. honest documentation of what is GPU-backed, fallback-only, or redesign-pending

Breadth is important, but only when backed by tests and truthful API boundaries. The release posture is correctness-first and hardware-validated, with conservative adaptive dispatch informed by GB10 and RTX 5080 benchmark data. The goal is not to make every restored path faster than CPU in 0.20.0; it is to put `amari-gpu` in a solid, truthful place to improve later as new crates and extensions arrive.

Benchmark terminology for public docs:

- **GPU-backed** means a real GPU path exists and is validated.
- **GPU-recommended** means benchmark/crossover data justifies using GPU by default above a documented threshold.
- **CPU-preferred** means the GPU path is retained for validation, future optimization, and manual use, but adaptive/default dispatch should remain on CPU for current tested sizes.

---

## Current restored high-value surfaces

### Fusion

Current restored public surface under `feature = "fusion"`:

- `FusionGpuError`
- `FusionGpuResult`
- `GpuHolographicTDC`
- `GpuResonatorOutput`
- `HolographicGpuOps`

Status:

- reduced public holographic/fusion-derived surface restored
- legacy nonexistent `amari_dual::gpu` / `amari_tropical::gpu` dependencies removed
- focused holographic WGSL validation improved
- broader fusion GPU API remains redesign-pending

### Tropical

Current restored public surface under `feature = "tropical"` via crate-root re-exports:

- `TropicalGpuError`
- `TropicalGpuResult`
- `TropicalExecutionPath`
- `TropicalGpuOps`

Current real public kernels:

- `TropicalGpuOps::matrix_multiply(...)`
- `TropicalGpuOps::matrix_multiply_adaptive(...)`
- `TropicalGpuOps::matrix_multiply_execution_path(...)`
- `TropicalGpuOps::should_use_gpu_for_matrix_multiply(...)`
- `TropicalGpuOps::attention_scores(...)`

Status:

- narrow public v1 surface restored
- dense tropical matrix multiply implemented and tested against CPU
- winner-takes-all tropical attention scores implemented and tested against CPU
- manual CPU-vs-GPU benchmark harness exists
- broader placeholder/prototype APIs isolated or kept non-public

---

## Priority order for the rest of 0.20.0

## Tier 1 — Public API honesty and surface inventory

- [x] Restore and enforce reduced fusion public surface
- [x] Add public import-path integration test for reduced fusion surface
- [x] Fix calculus large-batch vector-field placeholder behavior and document CPU-semantic fallback
- [x] Audit `measure` public surface, fix/document fallback behavior, and add public import-path tests
- [x] Audit `functional` public surface, fix/document fallback behavior, and add public import-path tests
- [x] Audit `topology` public surface, fix/document fallback behavior, and add public import-path tests
- [x] Restore narrow `dual` public v1 surface, hide redesign-pending scaffolding, and add public import-path tests
- [x] Audit `automata` public surface, document CPU neighborhood fallback, and add public import-path tests
- [x] Start special-care `enumerative` audit: fix representative shader validation issues, stabilize local GPU test execution, add high-use public import-path tests, and create method-by-method classification table
- [x] Start `gf2` audit: document fixed-layout bounds, add validation, add crate-root context re-export, add public import-path tests, and add CPU parity/property tests
- [x] Start `probabilistic` audit: document sampling/statistics semantics, add input validation, guard trailing GPU sample lanes, and add public import-path/parity tests
- [x] Start `network` audit: document narrow GPU-distance semantics, fix GPU dispatch tiling, correct adaptive CPU fallback semantics, add validation, and add public import-path/baseline tests
- [x] Start `relativistic` audit: document `(ct,x,y,z)` semantics, fix CPU conversion/layout issues, add validation, and add public import-path/baseline tests
- [x] Start broader `holographic` audit: document ProductCl3x32 v1 semantics, fix Cl3 binding shader parity, pack optical bind outputs under portable WebGPU limits, add validation, and add public import-path/baseline tests
- [x] Start default/core GA + info-geometry audit: document crate-root v1 semantics, fix signature-specific GA shader basis counts, replace info-geometry placeholders with CPU baselines, add validation, and add public import-path/baseline tests
- [x] Start broader infra/adaptive/performance/timeline audit: document orchestration semantics, add adaptive batch validation, harden non-finite calibration/timestamp edge cases, and add public import-path/baseline tests
- [x] Restore narrow tropical public v1 surface
- [x] Keep full `amari_gpu::tropical` module private while re-exporting only v1 API
- [x] Quarantine redesign-pending tropical trait scaffolding internally
- [x] Make redesign-pending tropical placeholder methods non-public
- [x] Create first-pass public `amari-gpu` API inventory by domain and feature
- [ ] Complete rustdoc-level inventory of every public type/function
- [ ] Mark each public operation as one of:
  - [ ] real GPU-backed
  - [ ] adaptive CPU/GPU
  - [ ] CPU fallback only
  - [ ] hardware-unvalidated
  - [ ] redesign-pending / should not be public
- [ ] Ensure README, crate docs, Cargo features, and `lib.rs` agree

## Tier 2 — CPU-baseline correctness coverage

For each practical public GPU operation:

- [ ] add deterministic CPU baseline tests
- [ ] test shape/dimension errors
- [ ] test empty or minimal inputs where meaningful
- [ ] test representative nontrivial inputs
- [ ] test feature-gated public import paths

Priority domains:

- [x] core geometric algebra representative public import-path/baseline coverage for adaptive CPU fallback, direct GPU batch products, validation, and signature-specific shader basis counts
- [x] info geometry representative public import-path/baseline coverage for Amari-Chentsov, typed-array input, Fisher matrices, KL-style divergence, memory usage, and device info
- [x] relativistic representative public import-path/baseline coverage for conversion, Minkowski products, validation, zero-step identity, and one-step propagation
- [x] network representative public import-path/baseline coverage for adaptive geometric distances, GPU distances/centrality/clustering when available, and unsupported embeddings
- [x] holographic representative public import-path/baseline coverage for ProductCl3x32 binding/similarity/bundling and optical bind/similarity/Lee encoding
- [x] infra/adaptive/performance/timeline representative public import-path coverage for CPU fallback, platform traits, optimizer calibration, dispatch policy learning, timeline accounting, and unified dispatcher fallback
- [ ] fusion restored subset
- [ ] tropical restored subset
- [x] probabilistic representative public import-path/parity coverage for validation, CPU fallback stats, GPU mean/variance, and deterministic zero-variance sampling
- [x] topology public import-path coverage for distance/Morse/Rips/Betti/fallback paths
- [x] functional public import-path coverage for matrix/Hilbert/spectral/fallback paths
- [x] dual public import-path coverage for unary forward-AD v1 surface
- [x] enumerative representative public import-path coverage for WDVV, intersection, localization, matroid, CSM, operad, and stability paths
- [x] gf2 representative public import-path coverage for Clifford product, matvec, Hamming distance, empty batches, and invalid fixed-layout inputs
- [x] gf2 CPU parity/property coverage for `amari-core::gf2` Clifford products, degenerate-generator behavior, associativity, distributivity, matvec parity/linearity, and Hamming final-word masking
- [x] automata public import-path coverage for rule application, energy, evolution, and neighborhood fallback
- [x] measure public import-path coverage for core paths/fallbacks
- [ ] calculus gradient/divergence/curl public baseline coverage
- [ ] GF(2)

## Tier 3 — Hardware validation

Target devices:

- [x] GB10 — DGX Spark / NVIDIA GB10 validation passed; see `docs/roadmap/AMARI_GPU_GB10_HARDWARE_VALIDATION.md`
- [x] RTX 5080 — NVIDIA GeForce RTX 5080 Laptop GPU validation passed with `WGPU_BACKEND=vulkan`; see `docs/roadmap/AMARI_GPU_RTX5080_HARDWARE_VALIDATION.md`

For each domain:

- [x] record adapter/backend for GB10 and RTX 5080
- [x] run focused correctness tests on GB10 and RTX 5080
- [x] run public integration tests on GB10 and RTX 5080
- [ ] run benchmark harnesses where available
- [x] record pass/fail/skip/timeout for GB10 and RTX 5080
- [x] record tolerances and backend-specific caveats for GB10 and RTX 5080

## Tier 4 — Benchmark and crossover reporting

- [x] keep manual benchmark harnesses for restored kernels
- [x] add initial benchmark harnesses for high-value public operation groups
- [x] record initial CPU vs GPU crossover points for core GA, tropical matmul, tropical attention, holographic, GF(2), probabilistic, topology, automata, measure, functional, and network paths
- [x] identify initial kernels that should default to CPU for small sizes
- [x] document initial guidance for when GPU acceleration is expected to help

Current benchmark report:

- `docs/roadmap/AMARI_GPU_BENCHMARK_CROSSOVER_REPORT.md`

Initial GB10/RTX 5080 crossover snapshots:

- core GA Cl(3,0,0) batch geometric product crosses over around batch size `64` on GB10 and between `64` and `256` on RTX 5080
- tropical dense matrix multiply crosses over between `32x32x32` and `64x64x64` on GB10 and between `64x64x64` and `128x128x128` on RTX 5080
- topology distance matrix crosses over between `64` and `256` points on GB10, but did not cross over through `512` points on RTX 5080
- measure integration/density kernels cross over only at large sample/value counts on GB10 and approached parity without crossing over in the RTX 5080 test-profile sweep
- several restored kernels are correctness-valid but should remain CPU-preferred until larger/release-mode benchmark sweeps show a GPU win

## Tier 5 — Coverage expansion

Only add or expose more operations when they satisfy:

- [ ] practical GPU value
- [ ] clear API boundary
- [ ] CPU baseline available
- [ ] tests added
- [ ] docs updated

Candidate expansion areas:

- [ ] tropical batched matrix multiply / additional semiring kernels
- [ ] fusion broader public surface after WGSL cleanup
- [x] dual unary batched forward-AD kernel v1
- [ ] dual binary/broadcast operations and broader gradient/training kernels
- [ ] optimization-like GPU workflows if source crate/API surface supports them
- [ ] explicit info-geometry module boundary if public API clarity improves (deferred; crate-root v1 documented/tested)
- [x] probabilistic/statistical kernels representative parity; broader statistical distribution/hardware validation pending
- [x] topology and graph/network representative baseline tests; broader graph/network expansion and hardware validation pending

---

## Post-0.20.0 planning note

The 0.20.0 goal is the validated `amari-gpu` stabilization baseline. Follow-up GPU issues are deferred to the 0.26.0 GPU/Borsalino modernization cycle rather than a planned 0.20.1 fast-follow, after the 0.25.0 rewrite/inverse research cycle:

- #137 benchmark CPU baseline completion
- #138 release-mode/Criterion benchmark pass
- #139 hardware-aware calibrated dispatch
- #140 focused high-upside kernel optimization
- #141 coverage revisit for upcoming crates/extensions
- #142 dedicated `wgpu 29` migration planning

This keeps 0.21.0 focused on `amari-tropical`/`amari-dual` extension work, 0.22.0 focused on `amari-cgt`/`amari-surreal` introductions, and 0.23.0 focused on `amari-surcomplex`/`amari-rewrite`. Patch releases in the 0.20.x line should remain bug-fix only unless a release-blocking issue appears.

## 0.20.0 exit criteria

`amari-gpu` is ready for 0.20.0 when:

- [ ] public API inventory is complete
- [ ] README and crate docs match actual public surface
- [ ] restored fusion/tropical APIs are honest and tested
- [ ] every public high-priority GPU operation has CPU-baseline correctness tests or is explicitly documented as infrastructure/fallback-only
- [x] GB10 validation report exists
- [x] RTX 5080 validation report exists
- [x] GB10/RTX 5080 benchmark/crossover notes exist for core GA, tropical, holographic, GF(2), probabilistic, topology, automata, measure, functional, and network paths
- [ ] placeholder or redesign-pending APIs are not accidentally public
- [ ] `cargo +stable test -p amari-gpu --quiet` passes
- [ ] feature-focused checks/tests pass for restored surfaces

---

## Immediate next implementation item

Start with a public API inventory for `amari-gpu`, then fix any public methods that are placeholder-like, misleading, or insufficiently tested.

The first concrete cleanups completed under this plan:

- tropical placeholder methods on public `TropicalGpuOps` were made non-public:
  - `neural_attention(...)`
  - `batch_viterbi(...)`
  - `tropical_solve(...)`
- first-pass public API inventory created:
  - `docs/roadmap/AMARI_GPU_PUBLIC_API_INVENTORY.md`

The highest-impact cleanup identified by the inventory was completed: `fusion` now uses a private module plus narrow crate-root re-exports so implementation matches the documented reduced public surface.
