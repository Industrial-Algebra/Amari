# amari-gpu Fusion vs Tropical Viability Check

Date: 2026-03-27
Purpose: decide which hidden domain is in better shape for first public restoration in `amari-gpu`

This is a focused GPU-only analysis.
It is **not** the same task as:

- the standalone `amari-wasm` audit/hardening work
- the standalone additive extension work for `amari-tropical`, `amari-dual`, and `amari-fusion`

Those tracks remain separate.

---

## Executive Conclusion

**`amari-gpu/src/fusion.rs` is in significantly better shape than `amari-gpu/src/tropical.rs` for first restoration work.**

Reason:

- `fusion.rs` is currently hidden mainly because of **integration/coherence/public wiring problems**
- `tropical.rs` is currently hidden because it has **multiple explicit placeholder implementations** and a still-unresolved **public API shape issue**

So the next GPU restoration order should be:

1. **fusion first**
2. **tropical second**

---

## 1. Buildability check

### `--features fusion`
Result:
- `cargo +stable test -p amari-gpu --features fusion -- --list` succeeded
- no fusion-specific test targets appeared because `fusion` is not currently wired into `lib.rs`
- the crate still compiles with the feature enabled

### `--features tropical`
Result:
- `cargo +stable test -p amari-gpu --features tropical -- --list` succeeded
- again, no tropical-specific test targets appeared because `tropical` is not currently wired into `lib.rs`

### Interpretation
Both source files are still syntactically/feature-wise compilable enough to coexist in the crate.
That alone does **not** mean they are equally ready for public exposure.

---

## 2. Fusion viability

## What fusion has going for it

### A. Substantial implementation exists
`amari-gpu/src/fusion.rs` includes substantial code for:

- `FusionGpuContext`
- `FusionGpuOps`
- `GpuTropicalDualClifford`
- shader-driven compute paths
- `HolographicGpuOps`
- batch bind / similarity / resonator cleanup
- evaluation and attention-oriented workflows

### B. It is closer to productized GPU functionality
The file contains meaningful operational structure rather than broad placeholder fallbacks.
The design is large, but it reads like a real implementation waiting for reintegration.

### C. Fewer obvious placeholder markers
In the focused scan, fusion showed:

- no obvious `placeholder` comments
- no obvious `return self as placeholder` style stubs
- no obvious `TODO` clusters indicating core operations are fake

What it does show is mostly:

- graceful fallback prints in tests
- integration assumptions about GPU-capable submodules in constituent crates

### D. Strategic importance
Fusion is a high-value GPU domain because it connects to:

- holographic operations already exposed elsewhere in `amari-gpu`
- future optimization/program-state workloads
- your broader roadmap without requiring redefining the math crates themselves

## Main blocker for fusion
The biggest issue seems to be **public integration design**, not obviously fake kernels.
From `lib.rs` comments, the historical concern was that it relied on GPU submodules/types in `amari-dual` and `amari-tropical`.

That suggests the first fusion task is:

- determine whether `fusion.rs` can be made self-contained enough for public exposure inside `amari-gpu`, or
- replace those assumptions with local wrapper/integration types

## Fusion verdict
**Viability: High**

Recommended next action:
- perform a reintegration audit and define a minimal public subset for restoration

---

## 3. Tropical viability

## What tropical has going for it

### A. Basic infrastructure exists
`amari-gpu/src/tropical.rs` includes:

- `TropicalGpuContext`
- `GpuTropicalNumber`
- error/result types
- some trait-based conversion ideas
- basic context/buffer helpers

### B. It already points toward a usable redesign direction
The file structure suggests a likely future based on:

- GPU wrapper types
- trait-based conversion
- free-standing ops/context objects

This is aligned with the idea that the public redesign should avoid orphan-impl issues.

## What holds tropical back
The focused scan found multiple explicit placeholders in core operations:

- matrix multiplication: placeholder
- GPU Viterbi: TODO
- attention scores: TODO
- geometric product: placeholder
- tropical addition: placeholder
- tropical scaling: placeholder
- full attention mechanism: placeholder
- linear system solve: placeholder

Examples found in comments:

- `TODO: Implement actual GPU matrix multiplication using tropical shaders`
- `For now, return self as placeholder`
- `For now, return query as placeholder`
- `For now, return b as placeholder`

### Interpretation
This means tropical is not merely hidden because of public API shape.
It also has a **significant amount of not-yet-real core behavior**.

## Tropical verdict
**Viability: Low-to-medium for immediate public restoration**

Recommended next action:
- do not re-expose tropical publicly yet
- first redesign around a small, real, benchmarkable kernel subset

---

## 4. Comparative judgment

| Criterion | Fusion | Tropical |
|---|---:|---:|
| Source file exists | Yes | Yes |
| Compiles when feature enabled | Yes | Yes |
| Public API currently wired | No | No |
| Placeholder density | Low | High |
| Core operations look real | Mostly | Mixed / many stubs |
| Public restoration difficulty | Moderate | High |
| Strategic roadmap value | High | High |
| Immediate restoration readiness | Better | Worse |

---

## 5. Recommended next GPU order

## Step 1 — fusion reintegration audit

Goal:
- determine the smallest validated public fusion surface that can be restored safely

Suggested initial scope:
- batch bind
- batch similarity
- resonator cleanup
- selected evaluation workflow if hardware validation succeeds

## Step 2 — tropical redesign plan

Goal:
- carve out a small, real tropical GPU subset rather than restoring the current placeholder-heavy module wholesale

Suggested initial scope:
- one real tropical matrix kernel
- one real path/scoring kernel
- CPU-baseline tests and benchmark crossover points

---

## Bottom Line

Your original task separation still holds:

- `amari-wasm` audit is one track
- algebra extension work is another track
- `amari-gpu` redesign is its own track

Within the **GPU track**, however, fusion is currently the more useful domain to inspect first because it appears to suffer more from **public-integration drift** than from fake implementation.

Tropical, by contrast, looks like it will need more substantial kernel and API redesign before it deserves public re-exposure.
