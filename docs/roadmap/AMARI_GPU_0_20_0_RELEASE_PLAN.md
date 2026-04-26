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

Breadth is important, but only when backed by tests and truthful API boundaries.

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

- [ ] core geometric algebra
- [ ] info geometry
- [ ] relativistic
- [ ] network
- [ ] holographic
- [ ] fusion restored subset
- [ ] tropical restored subset
- [ ] probabilistic
- [ ] topology
- [x] functional public import-path coverage for matrix/Hilbert/spectral/fallback paths
- [ ] dual
- [ ] enumerative
- [ ] automata
- [x] measure public import-path coverage for core paths/fallbacks
- [ ] calculus gradient/divergence/curl public baseline coverage
- [ ] GF(2)

## Tier 3 — Hardware validation

Target devices:

- [ ] GB10
- [ ] RTX 5080

For each domain:

- [ ] record adapter/backend
- [ ] run focused correctness tests
- [ ] run public integration tests
- [ ] run benchmark harnesses where available
- [ ] record pass/fail/skip/timeout
- [ ] record tolerances and backend-specific caveats

## Tier 4 — Benchmark and crossover reporting

- [ ] keep manual benchmark harnesses for restored kernels
- [ ] add benchmark harnesses for high-value public operations
- [ ] record CPU vs GPU crossover points
- [ ] identify kernels that should default to CPU for small sizes
- [ ] document when GPU acceleration is expected to help

Existing benchmark seed:

- tropical dense matrix multiply shows local crossover around `64x64x64` for the current naive kernel

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
- [ ] dual batched derivative kernels
- [ ] optimization-like GPU workflows if source crate/API surface supports them
- [ ] explicit info-geometry module boundary if public API clarity improves
- [ ] probabilistic/statistical kernels
- [ ] topology and graph/network kernels with baseline tests

---

## 0.20.0 exit criteria

`amari-gpu` is ready for 0.20.0 when:

- [ ] public API inventory is complete
- [ ] README and crate docs match actual public surface
- [ ] restored fusion/tropical APIs are honest and tested
- [ ] every public high-priority GPU operation has CPU-baseline correctness tests or is explicitly documented as infrastructure/fallback-only
- [ ] GB10 validation report exists
- [ ] RTX 5080 validation report exists
- [ ] benchmark/crossover notes exist for major kernels
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
