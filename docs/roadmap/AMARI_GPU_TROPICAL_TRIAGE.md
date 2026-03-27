# amari-gpu Tropical GPU Triage

Date: 2026-03-27
Current version: 0.19.1
Scope: `amari-gpu` only

This document is the GPU-only triage for `amari-gpu/src/tropical.rs`.
It is intentionally separate from:

- the standalone `amari-wasm` audit
- the standalone additive extension work in `amari-tropical`
- the broader `amari-gpu` redesign roadmap

## Executive Summary

`amari-gpu/src/tropical.rs` is **not ready for public restoration**.

Unlike fusion, which was mostly blocked by legacy integration drift, tropical is blocked by a combination of:

1. **placeholder implementations in core public-facing operations**
2. **an API shape problem** around how GPU operations should attach to tropical domain types
3. **an unclear minimal kernel subset** for first restoration

## Bottom-line recommendation

Do **not** re-enable public `amari-gpu::tropical` yet.

Instead:

- keep the module hidden
- redesign around a small, real subset
- restore only after replacing placeholder operations with actual GPU-backed implementations

---

## 1. Current viability assessment

## What currently exists

`amari-gpu/src/tropical.rs` includes:

- `TropicalGpuContext`
- `GpuTropicalNumber`
- error/result types
- `TropicalGpuAccelerated` trait
- conversions for `TropicalNumber`, `TropicalMatrix`, `TropicalMultivector`
- `TropicalGpuOps`
- WGSL shader constants for at least matrix multiplication and attention
- some tests

## What is the key problem
The file contains many methods that return placeholders instead of real GPU results.

Examples found:

- matrix multiplication returns `self.clone()` as placeholder
- Viterbi returns `self.clone()`
- attention scores return `self.clone()`
- geometric product returns `self.clone()`
- tropical addition returns `self.clone()`
- tropical scaling returns `self.clone()`
- neural attention returns `query.clone()`
- batch Viterbi returns empty results
- tropical solve returns `b.clone()`

This means the file is not just hidden because of API wiring.
It is hidden because large parts of the operational surface are not implemented honestly yet.

---

## 2. Triage classification

### Category A — keep
These look like useful foundations worth preserving:

- `TropicalGpuContext`
- `GpuTropicalNumber`
- error/result types
- buffer helpers
- the general idea of a dedicated `TropicalGpuOps`
- shader constants for matrix multiplication / attention

### Category B — redesign
These need redesign before public exposure:

- `TropicalGpuAccelerated` trait as currently used on domain types
- direct inherent-style GPU methods on `TropicalMatrix` and `TropicalMultivector`
- placeholder-heavy high-level APIs

### Category C — remove or defer from first public restoration
These should not be in the first restored public surface:

- placeholder Viterbi APIs
- placeholder attention APIs
- placeholder solve APIs
- placeholder multivector operations

---

## 3. Why tropical is different from fusion

Fusion restoration was able to proceed because:
- the holographic subset was mostly real
- the main problem was legacy nonexistent GPU imports

Tropical is different because:
- many of its visible methods are explicitly fake/placeholder
- restoring it publicly would create a misleading API surface

That means tropical requires a **true first-pass redesign**, not just reintegration.

---

## 4. Recommended redesign direction

## Principle
Start with one or two real kernels and build upward from there.

## Recommended first public subset

### Candidate minimal v1 surface
- `TropicalGpuContext`
- `GpuTropicalNumber`
- `TropicalGpuOps`
  - `new()`
  - **one real dense tropical matrix multiply kernel**
  - optional: one small batch/vector max-plus kernel if justified

### Do not include in v1
- Viterbi
- attention
- multivector geometric product
- tropical solve
- placeholder trait-based generic operations

## Why this subset
Because tropical matrix multiply is:
- the clearest semiring kernel
- easy to compare against CPU baselines
- aligned with future compiler/scheduling work
- small enough to benchmark and validate cleanly

---

## 5. API design recommendation

## Avoid current placeholder-heavy trait surface
The current `TropicalGpuAccelerated` trait attaches GPU behavior directly to domain types and encourages many pseudo-implemented methods.

## Preferred redesign shape
Use explicit GPU ops structs and free-standing methods instead of pretending every domain type already has a complete GPU implementation.

### Preferred direction
Something like:

```rust
pub struct TropicalGpuOps {
    context: TropicalGpuContext,
}

impl TropicalGpuOps {
    pub async fn new() -> TropicalGpuResult<Self> { ... }

    pub async fn matrix_multiply<T>(
        &mut self,
        a: &TropicalMatrix<T>,
        b: &TropicalMatrix<T>,
    ) -> TropicalGpuResult<TropicalMatrix<T>>
    where
        T: Float + bytemuck::Pod + Into<f32> + From<f32>;
}
```

This is much more honest and easier to validate.

---

## 6. Immediate code recommendations

## Keep for now
- context/buffer code
- basic conversion utilities
- shader constants that can be validated

## Defer/remove from first public path
- `gpu_viterbi`
- `gpu_attention_scores`
- `gpu_geometric_product`
- `gpu_tropical_add`
- `gpu_tropical_scale`
- `neural_attention`
- `batch_viterbi`
- `tropical_solve`

## First real implementation target
- replace placeholder `gpu_matrix_multiply` / matrix multiply path with a real shader-backed path

---

## 7. Public restoration preconditions

Before `amari-gpu::tropical` should be re-enabled publicly, all of the following should be true:

- [ ] no placeholder return paths remain in the first public surface
- [ ] one real matrix kernel is implemented and validated
- [ ] CPU baseline comparisons exist
- [ ] GB10 validation exists
- [ ] RTX 5080 validation exists
- [ ] benchmark crossover data exists
- [ ] API shape is explicit and non-misleading

---

## 8. Immediate next tropical task

The right next implementation task for tropical GPU is:

### Tropical GPU v1 kernel task
- implement one real dense tropical matrix multiplication path in `amari-gpu`
- benchmark it against CPU
- use that as the basis for deciding whether and how to restore the public module

---

## Bottom Line

`amari-gpu/src/tropical.rs` is currently a **prototype / scaffold**, not a public-ready GPU module.

The correct path is:

- do not expose it yet
- redesign around a single real kernel first
- restore the module only after that kernel and its API surface are honest, validated, and benchmarked
