# amari-gpu Redesign and Hardware Validation Task List

Date: 2026-03-27
Current version: 0.19.1
Target horizon: 0.20.0 → 0.21.0

This document is a **standalone task list** for `amari-gpu`.

It is intentionally separate from:

- `amari-wasm` audit and hardening
- algebra extension work in `amari-tropical`, `amari-dual`, and `amari-fusion`

## Special Change Policy

Unlike the other `amari-*` crates, `amari-gpu` has broad redesign latitude in this roadmap.
Because there are currently no important downstream users constraining its shape, this crate may be:

- significantly reorganized
- expanded in coverage
- redesigned for correctness and robustness first
- reworked to better expose Amari operations across the workspace

## Goal

Turn `amari-gpu` from a coverage-oriented integration crate into a robust, hardware-validated GPU platform layer for Amari.

The long-term direction is for `amari-gpu` to expose as many Amari operations as are technically justified, ideally approaching the breadth of `amari-wasm`, while maintaining strong CPU-baseline validation and graceful fallback behavior.

---

# 0.20.0 Focus

Active release-plan note: the 0.20.0 implementation focus is now captured in `docs/roadmap/AMARI_GPU_0_20_0_RELEASE_PLAN.md`. The guiding goal is comprehensive, practical `amari-gpu` operation exposure with CPU-baseline tests, hardware validation, benchmark/crossover data, and honest public API boundaries.

## 1. Current-state audit

- [ ] Inventory all current `amari-gpu` modules and exposed operations
- [ ] Map each operation to its source domain crate
- [ ] Classify each current path as:
  - [ ] production-ready
  - [ ] unvalidated on hardware
  - [ ] fallback-only
  - [ ] fragile / workaround-heavy
  - [ ] obsolete or redesign candidate
- [ ] Identify all GPU tests currently timing out, skipping, or depending on unavailable adapters
- [ ] Review current WGSL kernels for validation issues and workaround complexity

### Deliverable
- [ ] `amari-gpu` current-state audit notes

---

## 2. Coverage map vs workspace

- [ ] Build a coverage matrix comparing:
  - [ ] workspace crates
  - [ ] `amari-wasm` surface area
  - [ ] current `amari-gpu` surface area
- [ ] Identify high-value missing GPU candidates by domain:
  - [ ] tropical
  - [ ] dual
  - [ ] fusion
  - [ ] calculus
  - [ ] probabilistic
  - [ ] topology
  - [ ] functional
  - [ ] optimization
  - [ ] dynamics
  - [ ] holographic / optical
  - [ ] GF(2) where justified
- [ ] Decide which domains should be first-class priorities for redesign

### Deliverable
- [ ] GPU coverage parity plan

---

## 3. Hardware validation on GB10 and RTX 5080

- [ ] Record exact hardware/software environment for GB10
- [ ] Record exact hardware/software environment for RTX 5080
- [ ] Verify adapter enumeration and backend selection
- [ ] Run current GPU test suites on real hardware
- [ ] Record pass/fail, timeout, skip, and adapter-specific issues
- [ ] Compare numerical outputs to CPU baselines for representative workloads

### Per-domain correctness validation
- [ ] core geometric algebra
- [ ] info geometry
- [ ] relativistic
- [ ] network
- [ ] measure
- [ ] calculus
- [ ] dual
- [ ] enumerative
- [ ] automata
- [ ] fusion
- [ ] holographic / optical
- [ ] probabilistic
- [ ] functional
- [ ] topology
- [ ] dynamics

### Deliverable
- [ ] hardware validation report for GB10 and RTX 5080

---

## 4. Performance baseline and crossover analysis

- [ ] Measure latency vs CPU
- [ ] Measure throughput vs CPU
- [ ] Identify batch-size crossover points
- [ ] Record adapter-specific differences
- [ ] Record precision/tolerance behavior
- [ ] Identify kernels with negative ROI on GPU
- [ ] Identify kernels that should remain CPU-first with fallback only

### Deliverable
- [ ] GPU baseline benchmark report

---

## 5. Architecture redesign plan

- [ ] Decide target architecture for `amari-gpu`
- [ ] Separate concerns cleanly between:
  - [ ] adapter/context management
  - [ ] kernel dispatch
  - [ ] CPU fallback logic
  - [ ] data marshaling/layout
  - [ ] per-domain GPU wrappers
  - [ ] benchmark and validation utilities
- [ ] Decide what shared abstractions should exist for future growth
- [ ] Decide what should be removed, consolidated, or rewritten entirely

### Deliverable
- [ ] redesign architecture proposal

---

## 6. Immediate redesign tasks for 0.20.0

- [ ] Fix or replace fragile WGSL kernels blocking reliable validation
- [ ] Standardize CPU-baseline comparison utilities
- [ ] Standardize fallback behavior and capability detection
- [ ] Add structured benchmark harnesses
- [ ] Add adapter-aware test helpers
- [ ] Document known hardware/backend caveats

---

## 0.20.0 Exit Criteria

- [ ] current `amari-gpu` surface audited
- [ ] GB10 and RTX 5080 validation completed for current major domains
- [ ] baseline benchmark/crossover data recorded
- [ ] redesign architecture documented
- [ ] highest-risk current kernels either fixed, isolated, or marked for replacement

---

# 0.21.0 Focus

## 7. Coverage expansion toward platform parity

- [ ] Expand `amari-gpu` toward broader workspace coverage where technically justified
- [ ] Prioritize high-value additions with strong CPU baselines
- [ ] Add missing GPU paths for:
  - [ ] tropical operations
  - [ ] dual differentiation kernels
  - [ ] fusion workflows
  - [ ] optimization-heavy workflows
  - [ ] selected compiler/kernel-facing workloads from new algebra extensions

### Deliverable
- [ ] expanded coverage matrix

---

## 8. Tropical / Dual / Fusion GPU integration

This track depends on the additive 0.21.0 algebra extension work, but remains a separate GPU implementation effort.

### Tropical
- [ ] identify tropical kernels worth direct GPU support
- [ ] validate semiring/matrix/path kernels on real hardware
- [ ] benchmark sparse vs dense approaches where relevant

### Dual
- [ ] identify batched forward-mode kernels worth GPU support
- [ ] validate derivative correctness against CPU
- [ ] benchmark parameter-sweep / batched differentiation workloads

### Fusion
- [ ] identify evaluation/similarity/binding kernels worth GPU support
- [ ] validate numerical and semantic parity with CPU
- [ ] benchmark optimization-state workloads

---

## 9. Robustness-first API design

- [ ] expose GPU capabilities clearly and predictably
- [ ] document fallback semantics per domain
- [ ] avoid silently claiming acceleration where kernels are unvalidated
- [ ] prefer explicit capability checks in public APIs where appropriate
- [ ] ensure test and benchmark evidence backs release claims

---

## 10. Examples and documentation

- [ ] add examples showing validated GPU workflows
- [ ] document hardware support and backend caveats
- [ ] document crossover guidance: when GPU actually helps
- [ ] document unsupported or intentionally CPU-only paths

---

## 0.21.0 Exit Criteria

- [ ] `amari-gpu` has a redesigned, robustness-first architecture
- [ ] current and newly added domains have CPU-baseline validation on real hardware
- [ ] tropical/dual/fusion-related GPU paths are implemented where justified
- [ ] documentation reflects validated capabilities rather than aspirational coverage
- [ ] release claims are benchmark-backed

---

# Recommended Order

## First phase
- [ ] audit current `amari-gpu`
- [ ] build coverage matrix vs workspace and `amari-wasm`
- [ ] run hardware validation on GB10 and RTX 5080
- [ ] collect benchmark and correctness baselines

## Second phase
- [ ] define redesign architecture
- [ ] fix highest-risk kernels
- [ ] standardize fallback/capability/benchmark infrastructure

## Third phase
- [ ] expand coverage toward platform parity
- [ ] implement new tropical/dual/fusion GPU paths where justified
- [ ] add documentation and examples

---

# Suggested Immediate Next Deliverable

- [ ] `amari-gpu` current-state coverage matrix vs `amari-wasm`

That artifact should make it much easier to decide what to redesign first and what to defer.
