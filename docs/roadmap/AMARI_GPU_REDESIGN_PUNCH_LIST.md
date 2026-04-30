# amari-gpu Redesign Punch List

Date: 2026-03-27
Current version: 0.19.1
Derived from:

- `docs/roadmap/AMARI_GPU_REDESIGN_0_20_0_0_21_0_TASKS.md`
- `docs/roadmap/AMARI_GPU_COVERAGE_MATRIX_VS_WASM.md`

This is the prioritized, execution-oriented punch list for the first `amari-gpu` redesign pass.

## Core Principle

For `amari-gpu`, coherence and validation come before breadth.

The first pass should focus on:

1. removing drift between docs, features, and compiled public surface
2. validating existing public modules on real hardware
3. restoring or redesigning high-value hidden modules (`fusion`, `tropical`)
4. only then expanding toward broader platform parity

---

# Tier 0 — Immediate cleanup blockers

## 0.1 Fix README/code drift

### Tasks
- [ ] remove or correct `dynamics` support claims in `amari-gpu/README.md`
- [ ] correct `fusion` support claims to reflect actual public state
- [ ] correct `tropical` support claims to reflect actual public state
- [ ] verify all implementation tables in README against `lib.rs` and `Cargo.toml`

### Why first
This is the fastest way to stop further planning confusion.

---

## 0.2 Fix Cargo feature/dependency mismatches

### Tasks
- [ ] resolve `probabilistic` feature mismatch in `amari-gpu/Cargo.toml`
- [ ] decide whether `probabilistic` should:
  - [ ] remain self-contained via `rand` / `rand_distr`
  - [ ] explicitly depend on `amari-probabilistic`
- [ ] resolve `default-features is ignored` warnings
- [ ] confirm feature table matches actual intended public domains

### Why first
Until feature wiring is coherent, hardware validation results are harder to interpret.

---

# Tier 1 — Validate current public surface on real hardware

## 1.1 Build the validation matrix

### Public modules to validate first
- [ ] core geometric algebra (`GpuCliffordAlgebra`, adaptive/unified paths)
- [ ] `network`
- [ ] `relativistic`
- [ ] `measure`
- [ ] `calculus`
- [ ] `dual`
- [ ] `enumerative`
- [ ] `functional`
- [ ] `topology`
- [ ] `gf2`
- [ ] `holographic`
- [ ] `probabilistic`

### Tasks
- [ ] identify representative CPU baseline for each module
- [ ] run correctness comparisons on GB10
- [ ] run correctness comparisons on RTX 5080
- [ ] classify each module:
  - [ ] correct + performant
  - [ ] correct but not yet performant
  - [ ] inconsistent / needs rewrite
  - [ ] untestable in current shape

---

## 1.2 Establish GPU benchmark baseline

### Tasks
- [ ] benchmark latency vs CPU
- [ ] benchmark throughput vs CPU
- [ ] identify crossover points by batch size
- [ ] record adapter/backend-specific behavior
- [ ] identify modules where fallback should be preferred by default

### Deliverable
- [ ] first hardware-backed benchmark table per validated module

---

# Tier 2 — Restore high-value hidden domains

## 2.1 Fusion: restore or redesign as first major hidden module

### Current state
- source file exists
- README claims support
- module is commented out in `lib.rs`

### Tasks
- [ ] inspect why `fusion` was removed from public wiring
- [ ] determine whether current `fusion.rs` can be safely re-exposed
- [ ] if not, define minimal redesign scope for first restoration
- [ ] validate `FusionGpuOps` and `HolographicGpuOps` on hardware
- [ ] restore public module exposure if correctness is acceptable
- [ ] if restoration is not acceptable, replace with smaller validated surface first

### Suggested first public subset
- [ ] batch bind
- [ ] batch similarity
- [ ] resonator cleanup
- [ ] selected evaluation kernels

### Why before tropical
Fusion appears to already have substantial GPU code and is likely easier to turn into a coherent public surface than tropical.

---

## 2.2 Tropical: redesign as extension-trait / wrapper-based public module

### Current state
- source file exists
- module is commented out in `lib.rs`
- README explicitly says disabled because of orphan impl issues

### Tasks
- [ ] inspect `amari-gpu/src/tropical.rs`
- [ ] decide public API style that avoids orphan-impl problems:
  - [ ] wrapper types
  - [ ] extension traits
  - [ ] free-standing GPU ops structs
- [ ] define minimal validated tropical GPU surface
- [ ] validate it against CPU baselines on hardware
- [ ] restore public module exposure only once API shape is coherent

### Suggested first public subset
- [ ] tropical matrix multiply
- [ ] path / score batch operations
- [ ] Viterbi-style batch kernels if justified
- [ ] tropical operations most relevant to future compiler/kernel work

### Why high priority
This directly supports the 0.21.0 tropical extension roadmap.

---

# Tier 3 — Strengthen API coherence

## 3.1 Normalize public API shape across modules

### Tasks
- [ ] standardize naming conventions across GPU modules
- [ ] standardize constructor patterns (`new`, `new_with_config`, adaptive variants)
- [ ] standardize fallback behavior and capability reporting
- [ ] standardize error types/results where practical
- [ ] standardize CPU-baseline comparison utilities in tests

---

## 3.2 Clarify module boundaries

### Tasks
- [ ] decide whether info-geometry should become its own explicit public module
- [ ] decide whether optical should remain under `holographic` or become its own GPU module
- [ ] decide whether certain modules are infra-only vs user-facing

### Recommended near-term decisions
- [ ] info-geometry: likely deserves explicit public module boundary
- [ ] optical: likely keep under holographic until validated, then split if needed

---

# Tier 4 — Infrastructure consolidation

## 4.1 Preserve and strengthen unique amari-gpu assets

These are strengths and should be retained through redesign:

- [ ] `adaptive`
- [ ] `unified`
- [ ] `verification`
- [ ] `multi_gpu`
- [ ] `performance`
- [ ] `timeline`
- [ ] `benchmarks`

### Tasks
- [ ] ensure domain modules consistently plug into this infrastructure
- [ ] remove one-off patterns where modules bypass common infra unnecessarily
- [ ] define a shared pattern for:
  - [ ] adapter selection
  - [ ] dispatch thresholds
  - [ ] CPU fallback
  - [ ] correctness verification
  - [ ] benchmarking hooks

---

# Tier 5 — Expansion toward parity with amari-wasm

Only after Tier 0–4 are in good shape.

## 5.1 Missing domains to triage

### Possible additions
- [ ] optimization
- [ ] stronger info-geometry surface
- [ ] explicit optical module
- [ ] future dynamics support if actually implemented and justified
- [ ] flynn only if a GPU use case becomes compelling

### Rule
No new domain should be added until:
- [ ] CPU baseline exists
- [ ] realistic GPU workload exists
- [ ] benchmark justification exists
- [ ] fallback policy is defined

---

# Suggested Execution Order

## Pass 1 — stop drift
- [ ] README correction
- [ ] Cargo feature cleanup
- [ ] classify actual public surface

## Pass 2 — validate what is already public
- [ ] run current public modules on GB10
- [ ] run current public modules on RTX 5080
- [ ] build correctness/performance matrix

## Pass 3 — restore hidden but valuable modules
- [ ] fusion first
- [ ] tropical second

## Pass 4 — unify API/infrastructure
- [ ] standardize patterns across modules
- [ ] clean module boundaries

## Pass 5 — selective expansion
- [ ] add new high-value domains only after validation discipline is established

---

# Concrete First Code Targets

If starting implementation immediately, the first targets should be:

1. `amari-gpu/README.md`
   - remove/correct inaccurate support tables

2. `amari-gpu/Cargo.toml`
   - fix feature mismatches and warnings

3. `amari-gpu/src/lib.rs`
   - make public surface reflect deliberate design, not accumulated drift

4. `amari-gpu/src/fusion.rs`
   - determine restore vs redesign path

5. `amari-gpu/src/tropical.rs`
   - determine viable public API shape

---

# Success Criteria for First Redesign Pass

- [ ] README, Cargo, and `lib.rs` agree on what `amari-gpu` actually exposes
- [ ] current public modules have first-pass hardware validation on GB10 and RTX 5080
- [ ] fusion has a clear restore/redesign decision
- [ ] tropical has a clear restore/redesign decision
- [ ] shared infrastructure is preserved as a foundation for future coverage expansion

---

# Bottom Line

The first `amari-gpu` redesign pass should **not** start by adding more modules.
It should start by making the crate honest, validated, and architecturally coherent.

Once that is done, `fusion` and `tropical` are the two most important next modules to bring into a well-formed public GPU surface.
