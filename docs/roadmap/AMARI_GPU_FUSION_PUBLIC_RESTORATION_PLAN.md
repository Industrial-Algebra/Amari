# amari-gpu Fusion Public Restoration Plan

Date: 2026-03-27
Current version: 0.19.1
Scope: `amari-gpu` only

This document defines the plan for restoring a **validated public fusion GPU surface** in `amari-gpu`.

It is intentionally separate from:

- `amari-wasm` audit/hardening
- additive extension work in `amari-fusion`
- additive extension work in `amari-tropical` and `amari-dual`

## Objective

Restore a small, coherent, hardware-validated public `fusion` module in `amari-gpu` without overcommitting to the entire existing `fusion.rs` source surface.

## Executive Summary

`amari-gpu/src/fusion.rs` looks like the best hidden module to restore first, but there is one major caveat:

### Critical blocker
The current file assumes GPU submodules/types that do **not** exist in the source crates:

- `amari_dual::gpu::{DualGpuOps, GpuDualNumber}`
- `amari_tropical::gpu::{GpuTropicalNumber, TropicalGpuOps}`

A search of `amari-dual` and `amari-tropical` found **no such `gpu` modules or types**.

That means the current `fusion.rs` is not merely hidden by `lib.rs`; it is also relying on an integration model that is currently unreal.

## Practical conclusion

The right plan is:

- **do not expose the current `fusion.rs` as-is**
- **restore a reduced, self-contained subset first**
- **remove dependency on nonexistent `amari_dual::gpu` / `amari_tropical::gpu` modules**

---

# 1. Restoration Strategy

## Phase A — Reduce scope before exposure

The full source file currently mixes several different concerns:

- general fusion context
- LLM evaluation
- geometric attention
- optimization gradients
- holographic GPU ops
- test scaffolding

The first public restoration should expose only the part most likely to be:

- internally self-contained
- benchmarkable
- verifiable against CPU baselines
- useful immediately

## Recommended first public subset

### Public subset v1
- `fusion::GpuHolographicTDC`
- `fusion::GpuResonatorOutput`
- `fusion::HolographicGpuOps`
  - `new()`
  - `batch_bind()`
  - `batch_similarity()`
  - `resonator_cleanup()`
  - `should_use_gpu()`

### Keep internal or disabled for now
- `FusionGpuOps`
- `FusionGpuContext` as a fully general fusion context
- `llm_evaluation()`
- `geometric_attention()`
- `batch_fusion_optimization()`
- gradient-related fusion optimization paths

Reason:
- the holographic subset is the most concrete, least speculative, and most directly tied to existing shader infrastructure in `amari-gpu`

---

# 2. Immediate blockers to fix

## Blocker 1 — Nonexistent cross-crate GPU imports

Current source references:

- `amari_dual::gpu::{DualGpuOps, GpuDualNumber}`
- `amari_tropical::gpu::{GpuTropicalNumber, TropicalGpuOps}`

These do not currently exist.

## Required action
For first restoration, `amari-gpu` must own the GPU-facing fusion data layout locally.

### Plan
- remove or isolate imports from nonexistent `amari_dual::gpu` / `amari_tropical::gpu`
- replace with local GPU POD structs inside `amari-gpu/src/fusion.rs`
- keep conversions from CPU-side `TropicalDualClifford<f32, 8>` local to `amari-gpu`

### Minimal local replacements
Likely local types:
- `GpuDualNumber { real: f32, dual: f32 }`
- `GpuTropicalScalar { value: f32 }`

This is consistent with `amari-gpu` being the integration layer.

---

## Blocker 2 — Overly broad first-surface ambition

The existing file tries to expose:

- LLM evaluation
- attention
- optimization
- holographic memory

Restoring all of that at once would create a large validation burden.

## Required action
- split the restoration into a **minimal validated first surface** and a later expansion phase

---

## Blocker 3 — Public API not wired into `lib.rs`

`amari-gpu/src/lib.rs` currently comments out:

```rust
// #[cfg(feature = "fusion")]
// pub mod fusion;
```

## Required action
Do not re-enable this until the reduced first surface is in place.

---

# 3. Concrete Restoration Plan

## Step 1 — Create a reduced fusion module boundary

### Tasks
- [x] identify the smallest section of `fusion.rs` needed for holographic ops
- [x] define a reduced public module around holographic operations only
- [ ] further isolate or split `FusionGpuOps` and broader fusion code in a later cleanup pass

### Preferred shape
The first public `fusion` module should be clearly documented as:

- GPU support for **fusion-derived holographic operations**
- not yet the full general fusion GPU platform

---

## Step 2 — Replace nonexistent cross-crate GPU types with local ones

### Tasks
- [x] replace `GpuDualNumber` dependency with a local POD type
- [x] replace `GpuTropicalNumber` dependency with a local POD type
- [x] remove `DualGpuOps` and `TropicalGpuOps` fields from first public path
- [x] preserve conversion from `TropicalDualClifford<f32, 8>` to GPU-local structs

### Expected result
The first restored public module becomes self-contained inside `amari-gpu`.

---

## Step 3 — Keep only validated shaders in the first public path

### Tasks
- [x] confirm which shader constants are already used by `HolographicGpuOps`
- [x] confirm CPU baseline equivalents for bind/similarity/resonator cleanup at the test level
- [~] test correctness of those operations on available hardware (focused tests now pass locally; broader hardware validation still pending)

### Current status note
The restored holographic subset now has WGSL validation fixes for:

- uniform buffer layout (`vec4<u32>` instead of invalid uniform arrays)
- reserved keyword usage
- invalid dynamic indexing in holographic bind/similarity/resonator shaders

Focused fusion holographic tests now pass under `--features fusion`.

### First benchmark targets
- [ ] batch bind
- [ ] pairwise similarity
- [ ] similarity matrix
- [ ] resonator cleanup

---

## Step 4 — Add first-pass public tests for restored fusion API

### Tests to keep/add
- [x] conversion tests for GPU fusion structs
- [x] shape validation tests
- [x] empty input handling
- [x] pairwise vs matrix similarity tests
- [x] resonator cleanup correctness vs CPU expectations (focused test coverage)
- [x] graceful no-adapter behavior

### Current status note
The heavier holographic GPU tests remain ignored in the full suite for now because they run reliably in focused validation but are too slow/flaky in the aggregated suite. They are still suitable for targeted hardware-validation runs.

### Important test rule
Tests should no longer be only “passes even if GPU is unavailable.”
For restoration, add:

- environment-independent shape/correctness tests where possible
- hardware-conditional correctness tests with explicit CPU comparison

---

## Step 5 — Re-enable public module exposure

Only after Steps 1–4:

- [x] re-enable `#[cfg(feature = "fusion")] pub mod fusion;` in `amari-gpu/src/lib.rs`
- [x] re-export only the reduced, validated first-surface types
- [x] update README with exact restored scope

---

# 4. What should stay deferred

These should remain internal, disabled, or explicitly redesign-pending until after the first restoration:

## Defer for later
- `FusionGpuOps`
- `llm_evaluation()`
- `geometric_attention()`
- `batch_fusion_optimization()`
- shader-generated gradient optimization paths

## Why defer
These features are:

- broader in scope
- harder to validate well
- more likely to need redesign after hardware testing
- not necessary for a credible first public restoration

---

# 5. Proposed Public API for First Restoration

Suggested first public re-exports:

```rust
#[cfg(feature = "fusion")]
pub mod fusion;

#[cfg(feature = "fusion")]
pub use fusion::{
    GpuHolographicTDC,
    GpuResonatorOutput,
    HolographicGpuOps,
    FusionGpuError,
    FusionGpuResult,
};
```

This is intentionally narrower than the current source file contents.

---

# 6. Validation Requirements Before Re-Exposure

The first restored public fusion surface should not ship without:

- [ ] compile success under `--features fusion`
- [ ] no dependency on nonexistent `amari_dual::gpu` / `amari_tropical::gpu`
- [ ] CPU baseline comparisons for bind/similarity/cleanup
- [ ] GB10 correctness validation
- [ ] RTX 5080 correctness validation
- [ ] at least one benchmark table showing crossover behavior

---

# 7. Recommended Immediate Code Targets

1. `amari-gpu/src/fusion.rs`
   - isolate holographic subset
   - remove nonexistent GPU-submodule assumptions

2. `amari-gpu/src/lib.rs`
   - re-enable `fusion` only after reduced surface is ready

3. `amari-gpu/README.md`
   - update fusion support section once re-exposed

---

# 8. Success Criteria

The first fusion restoration pass is successful when:

- [x] `amari-gpu --features fusion` compiles with a real public module
- [x] restored API is smaller but honest and validated
- [x] no fake cross-crate GPU dependencies remain
- [~] hardware-backed correctness and basic benchmarks exist (focused correctness tests pass; full GB10/RTX5080 validation and benchmark reporting still pending)

---

# Bottom Line

The right move is **not** to expose the current `fusion.rs` wholesale.

The right move is to:

- carve out the holographic subset
- make it self-contained within `amari-gpu`
- validate it on real hardware
- expose that smaller surface first

That gives `amari-gpu` a credible, useful fusion public API while keeping the larger fusion GPU ambitions for a later pass.
