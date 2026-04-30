# amari-gpu Current-State Coverage Matrix vs amari-wasm

Date: 2026-03-27
Current workspace version: 0.19.1

This document compares the **current actual code surface** of `amari-gpu` against `amari-wasm`.
Its purpose is to identify:

- what `amari-gpu` already covers
- what `amari-wasm` covers that `amari-gpu` does not
- where README/package claims drift from actual compiled surface
- which areas are best candidates for redesign and expansion

## Key Takeaway

`amari-gpu` already has a substantial internal codebase, but its **actual compiled/exported surface is noticeably less coherent than `amari-wasm`**.

The biggest current gaps are not just missing modules; they are:

1. **coverage drift between README claims and compiled public surface**
2. **feature/dependency mismatches**
3. **disabled or commented-out high-value domains (`fusion`, `tropical`)**
4. **missing parity for several major `amari-wasm` domains**
5. **strong infrastructure investment, but incomplete productized domain exposure**

---

## Source of Truth Used Here

This matrix is based on:

- `amari-gpu/src/lib.rs`
- `amari-gpu/Cargo.toml`
- `amari-gpu/README.md`
- `amari-gpu/src/*`
- `amari-wasm/src/lib.rs`
- `amari-wasm/src/*`

Where these disagree, this document treats **compiled code and Cargo features** as the more authoritative current state.

---

## 1. Current Module Surface

## amari-wasm modules

`amari-wasm` currently exports these domain-facing modules:

- `automata`
- `calculus`
- `dual`
- `enumerative`
- `flynn`
- `functional`
- `fusion`
- `gf2`
- `info_geom`
- `measure`
- `network`
- `optical`
- `optimization`
- `probabilistic`
- `relativistic`
- `topology`
- `tropical`
- plus core functionality in `lib.rs`

## amari-gpu modules present in source tree

`amari-gpu/src/` contains:

- `adaptive`
- `automata`
- `benchmarks`
- `calculus`
- `dual`
- `enumerative`
- `functional`
- `fusion`
- `gf2`
- `holographic`
- `measure`
- `multi_gpu`
- `network`
- `performance`
- `probabilistic`
- `relativistic`
- `shaders`
- `timeline`
- `topology`
- `tropical`
- `unified`
- `verification`

## amari-gpu modules actually exported from lib.rs

Currently exported/publicly wired in `amari-gpu/src/lib.rs`:

- `adaptive`
- `automata` *(feature-gated)*
- `benchmarks`
- `calculus` *(feature-gated)*
- `dual` *(feature-gated)*
- `enumerative` *(feature-gated)*
- `functional` *(feature-gated)*
- `gf2` *(feature-gated)*
- `holographic` *(feature-gated)*
- `measure` *(feature-gated)*
- `multi_gpu`
- `network`
- `performance`
- `probabilistic` *(feature-gated)*
- `relativistic`
- `shaders`
- `timeline`
- `topology` *(feature-gated)*
- `unified`
- `verification`

**Important:**
- `fusion` source file exists, but `pub mod fusion;` is commented out in `lib.rs`
- `tropical` source file exists, but `pub mod tropical;` is commented out in `lib.rs`
- `info_geom` is not a separate module; info-geometry functionality is partially embedded in core/lib-facing exports
- there is no `optimization.rs`, `optical.rs`, or `flynn.rs`
- README references `dynamics`, but there is no `dynamics` module or Cargo feature in `amari-gpu/Cargo.toml`

---

## 2. Coverage Matrix: amari-gpu vs amari-wasm

Legend:
- **Yes** = present/exposed in a meaningful way
- **Partial** = some support exists, but not as a clean domain module or not fully aligned
- **No** = not currently exposed in a meaningful public way
- **Drift** = source/README exists but compiled/public state is inconsistent

| Domain / Capability | amari-wasm | amari-gpu actual public surface | Notes |
|---|---:|---:|---|
| Core geometric algebra | Yes | Yes | `GpuCliffordAlgebra`, adaptive/unified infrastructure exist |
| Information geometry | Yes | Partial | present in README and some lib-level ops, but not as a dedicated `info_geom` module |
| Network | Yes | Yes | public module/export exists |
| Relativistic | Yes | Yes | public module/export exists |
| Measure | Yes | Yes | feature-gated in GPU |
| Calculus | Yes | Yes | feature-gated in GPU |
| Dual | Yes | Yes | feature-gated in GPU |
| Enumerative | Yes | Yes | feature-gated in GPU |
| Automata | Yes | Yes | feature-gated in GPU |
| Functional | Yes | Yes | feature-gated in GPU |
| Topology | Yes | Yes | feature-gated in GPU |
| GF(2) | Yes | Yes | feature-gated in GPU |
| Probabilistic | Yes | Partial | GPU module exists, but Cargo feature wiring does **not** depend on `amari-probabilistic` |
| Fusion | Yes | Drift | source file exists and README claims support, but module is commented out in `lib.rs` |
| Tropical | Yes | Drift / No | source file exists but module is commented out; README says disabled |
| Holographic memory | Indirect via `fusion`/`optical` | Yes | GPU has explicit `holographic` module |
| Optical field operations | Yes | Partial | likely under `holographic`, but no separate `optical` GPU module |
| Optimization | Yes | No | no `amari-gpu` optimization module |
| Flynn / probabilistic contracts | Yes | No | no `amari-gpu` flynn module |
| Dynamics | No separate wasm module yet | Drift | README claims support, but no Cargo feature/module present |
| Unified runtime context | No direct analog | Yes | `unified`, `adaptive`, `verification`, `multi_gpu` are GPU-only infra strengths |
| Multi-GPU support | No | Yes | major GPU-specific strength |
| Profiling/timeline infra | No | Yes | strong GPU-only infra surface |
| Verification/adaptive dispatch | No direct analog | Yes | GPU-specific infra strength |

---

## 3. Major Drift and Inconsistency Findings

## A. Fusion drift

### What exists
- `amari-gpu/src/fusion.rs` exists and is substantial
- README claims `amari-fusion` GPU acceleration is implemented
- source includes `FusionGpuOps`, `HolographicGpuOps`, etc.

### What is actually public
In `amari-gpu/src/lib.rs`, the fusion module is commented out:

- `// #[cfg(feature = "fusion")]`
- `// pub mod fusion;`

### Meaning
This is one of the clearest examples of **code existing without coherent public integration**.

**Priority:** Very high

---

## B. Tropical drift

### What exists
- `amari-gpu/src/tropical.rs` exists
- Cargo has a `tropical` feature
- README says tropical module is disabled

### What is actually public
In `amari-gpu/src/lib.rs`, tropical is commented out:

- `// #[cfg(feature = "tropical")]`
- `// pub mod tropical;`

### Meaning
This is a prime redesign candidate. It also aligns directly with your 0.21.0 tropical extension goals.

**Priority:** Very high

---

## C. Probabilistic feature mismatch

### README claim
README says:
- `amari-probabilistic` GPU support is implemented

### Cargo reality
`amari-gpu/Cargo.toml` has:

```toml
probabilistic = ["dep:rand", "dep:rand_distr"]
```

There is **no `amari-probabilistic` dependency** in the manifest.

### Meaning
This is not just a coverage gap; it is a **feature/dependency design mismatch**.
Either:
- the module is intentionally self-contained and should be documented that way, or
- the crate wiring needs to be corrected to match the domain-crate model

**Priority:** High

---

## D. Dynamics README drift

### README claim
README lists:
- `amari-dynamics` as implemented and new in `v0.19.1`

### Cargo / source reality
- no `amari-dynamics` dependency in `amari-gpu/Cargo.toml`
- no `dynamics.rs` in `amari-gpu/src`
- no `dynamics` feature in `Cargo.toml`
- no `pub mod dynamics;` in `lib.rs`

### Meaning
This is a hard documentation mismatch.

**Priority:** High

---

## E. Information geometry is present but not cleanly modularized

`amari-wasm` has an explicit `info_geom` module.
`amari-gpu` has info-geometry functionality in README/lib-level exports and tests, but not as a clearly dedicated `info_geom.rs` public module.

### Meaning
Not necessarily wrong, but it indicates a weaker public API boundary than in `amari-wasm`.

**Priority:** Medium

---

## 4. Where amari-gpu Is Stronger Than amari-wasm

Despite the domain-coverage gaps, `amari-gpu` has major strengths that `amari-wasm` does not try to provide.

## Infrastructure strengths

- `adaptive`
- `unified`
- `verification`
- `multi_gpu`
- `performance`
- `timeline`
- `benchmarks`

These are meaningful assets for the redesign.

## Interpretation
The redesign should not just chase parity with `amari-wasm` at the domain level.
It should preserve and improve these GPU-platform strengths while broadening domain coverage.

---

## 5. Coverage Assessment by Strategic Category

## Category A — Already solid foundations

These areas appear to have meaningful code and should be validated first rather than conceptually reinvented:

- core geometric algebra
- network
- relativistic
- unified/adaptive context management
- multi-GPU infrastructure
- verification and profiling infrastructure

## Category B — Present but needs public/product cleanup

These likely need redesign or proper public reintegration rather than greenfield work:

- fusion
- tropical
- probabilistic
- info geometry module boundary
- optical exposure through holographic layer

## Category C — Missing relative to amari-wasm

These are the clearest parity gaps:

- optimization
- flynn / probabilistic contracts
- dedicated optical module
- explicit dynamics support

Not all of these need immediate GPU support, but they should be explicitly triaged.

---

## 6. Recommended Redesign Priorities

## Priority 1 — Fix drift before adding new coverage

1. fusion public wiring
2. tropical public wiring/design
3. probabilistic feature/dependency mismatch
4. dynamics README mismatch
5. Cargo feature warning cleanup

## Priority 2 — Validate what already exists on real hardware

On GB10 and RTX 5080, validate:

- core
- network
- relativistic
- functional
- topology
- calculus
- dual
- enumerative
- holographic

## Priority 3 — Decide parity targets vs amari-wasm

Recommended first parity targets:

1. tropical
2. fusion
3. optical/holographic cleanup
4. optimization-oriented GPU paths
5. stronger info-geometry public module boundary

## Priority 4 — Explicitly defer or scope-limit low-value parity work

Potentially defer until justified:

- flynn GPU support
- full dynamics GPU support if not actually implemented yet
- domains whose workload sizes rarely justify GPU acceleration

---

## 7. Suggested Redesign Target State

A cleaner long-term shape for `amari-gpu` would be:

### Platform layer
- `unified`
- `adaptive`
- `verification`
- `multi_gpu`
- `performance`
- `timeline`
- `benchmarks`

### Domain GPU modules
- `core`
- `info_geom`
- `network`
- `relativistic`
- `measure`
- `calculus`
- `dual`
- `tropical`
- `fusion`
- `holographic`
- `probabilistic`
- `functional`
- `topology`
- `enumerative`
- `automata`
- optional future: `optimization`, `dynamics`

This would move `amari-gpu` closer to the breadth of `amari-wasm`, while still keeping its own GPU-specific identity.

---

## 8. Immediate Action Items

- [ ] verify and document actual public coverage vs README claims
- [ ] decide whether `fusion.rs` should be restored or rewritten first
- [ ] decide whether `tropical.rs` should be restored via extension traits / new wrappers
- [ ] fix probabilistic feature wiring mismatch
- [ ] remove or implement README dynamics claims
- [ ] produce hardware-backed validation results for currently public modules

---

## Bottom Line

`amari-gpu` is **not empty** and **not merely aspirational**. It already has substantial domain code plus unusually strong platform infrastructure.

But relative to `amari-wasm`, its current problem is coherence:

- some domains exist only in source, not in the public API
- some docs overclaim what is actually wired up
- some features do not reflect the dependency model implied by the rest of the workspace

That makes the next best step clear:

**first redesign `amari-gpu` for coherent, validated public coverage; then expand it toward high-value parity with `amari-wasm`.**
