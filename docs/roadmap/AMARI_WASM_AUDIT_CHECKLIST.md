# amari-wasm Audit Checklist

Date: 2026-03-27
Target release: 0.20.0
Current crate version: 0.19.1

This document is the implementation-facing audit checklist for `amari-wasm`.
Its purpose is to turn the broad 0.20.0 roadmap into a module-by-module execution plan.

## Audit Goals

1. verify that exported WASM bindings match current Rust crate behavior
2. remove historical compatibility shims that no longer reflect reality
3. identify placeholders, stubs, deferred APIs, and disabled tests
4. define a clean testing matrix across native, node, and browser runtimes
5. prepare the crate for benchmarking and hardware-backed validation

## Current High-Level Findings

### Observed scale

`amari-wasm/src` currently contains 18 modules:

- `lib.rs` — 1083 lines
- `automata.rs` — 724 lines
- `calculus.rs` — 1074 lines
- `dual.rs` — 893 lines
- `enumerative.rs` — 1898 lines
- `flynn.rs` — 494 lines
- `functional.rs` — 707 lines
- `fusion.rs` — 1281 lines
- `gf2.rs` — 1341 lines
- `info_geom.rs` — 378 lines
- `measure.rs` — 789 lines
- `network.rs` — 661 lines
- `optical.rs` — 949 lines
- `optimization.rs` — 443 lines
- `probabilistic.rs` — 1201 lines
- `relativistic.rs` — 517 lines
- `topology.rs` — 551 lines
- `tropical.rs` — 466 lines

### Observed risk signals

The following files contain explicit historical-compatibility or placeholder markers:

- `amari-wasm/src/tropical.rs`
- `amari-wasm/src/dual.rs`
- `amari-wasm/src/fusion.rs`
- `amari-wasm/src/optimization.rs`

Examples found:

- `Manual implementation as ... not in v0.12.0 API`
- `Return 0 as a placeholder`
- `Providing stub ...`
- `TODO: Re-enable when ...`
- commented-out deferred APIs

### Testing snapshot

- `cargo +stable test -p amari-wasm -- --list` shows **99 tests**
- there are mixed native `#[test]` and `wasm_bindgen_test` tests
- one integration file is disabled:
  - `amari-wasm/tests/wasm_edge_computing.rs.disabled`
- the disabled file is clearly TDD-placeholder content and should not simply be re-enabled as-is

---

## Audit Status Legend

- **Green**: looks straightforward, mostly validation/doc cleanup
- **Yellow**: substantial review needed, but no obvious stub/placeholder risk yet
- **Red**: contains known placeholder/shim/deferred behavior and must be audited first

---

# Phase 1: Priority Audit Order

## Priority 1 — Red modules

1. `tropical.rs`
2. `dual.rs`
3. `fusion.rs`
4. `optimization.rs`
5. disabled integration tests in `tests/wasm_edge_computing.rs.disabled`

## Priority 2 — Large and high-surface modules

6. `lib.rs`
7. `enumerative.rs`
8. `gf2.rs`
9. `probabilistic.rs`
10. `calculus.rs`

## Priority 3 — Broad validation pass

11. `optical.rs`
12. `measure.rs`
13. `automata.rs`
14. `functional.rs`
15. `network.rs`
16. `topology.rs`
17. `relativistic.rs`
18. `flynn.rs`
19. `info_geom.rs`

---

# Module-by-Module Checklist

## 1. `amari-wasm/src/lib.rs` — Yellow

### Scope
Core WASM crate entry point plus geometric algebra / rotor bindings and module exports.

### Known observations
- very large file
- centralizes module exports and core `WasmMultivector` behavior
- has many native tests
- likely the foundation for public npm-facing examples

### Audit tasks
- [ ] verify all exported core types are still the right public surface
- [ ] confirm `WasmMultivector` dimension assumptions are clearly documented
- [ ] verify error messages and shape checks are consistent
- [ ] verify ownership/free patterns in examples remain correct
- [ ] split file if internal organization is impeding maintainability
- [ ] add runtime tests for the most important public examples

### Deliverable
- [ ] core API audit notes
- [ ] decision on whether to keep monolithic or split into submodules

---

## 2. `amari-wasm/src/tropical.rs` — Red

### Known issues found
- historical `v0.12.0` compatibility comments
- manual implementations for `from_log_prob`, `to_prob`, `is_infinity`, negation semantics
- use of comment `to_attention_scores() not available in v0.12.0 API`
- explicit placeholder:
  - `coefficients_count()` returns `0`

### Audit questions
- is every manual implementation still required against `amari-tropical 0.19.1`?
- should `coefficients_count()` be implemented, removed, or documented differently?
- are JS-visible semantics for tropical zero/one/infinity clearly correct?
- should a richer tropical API be exposed for compiler/scheduling work later?

### Audit tasks
- [ ] map every exported wrapper to current `amari-tropical` API
- [ ] remove outdated migration commentary
- [ ] replace placeholder `coefficients_count()` behavior
- [ ] verify Viterbi and polynomial wrappers against current crate behavior
- [ ] add node/browser tests for all public classes and batch ops
- [ ] document exact layout/shape expectations for matrices and polynomials

### Deliverable
- [ ] tropical wrapper parity report
- [ ] list of APIs to keep/replace/remove

---

## 3. `amari-wasm/src/dual.rs` — Red

### Known issues found
- historical `v0.12.0` compatibility comments
- manual `relu()` and `softplus()` wrappers
- `WasmMultiDualNumber::sqrt()` contains explicit stub-like commentary:
  - `Providing stub that preserves value but computes correct gradient`

### Audit questions
- are current manual wrappers acceptable and mathematically documented?
- should `MultiDualNumber::sqrt()` be implemented crate-side in `amari-dual` instead of wrapper-side?
- do JS users get a correct and unsurprising API for multi-variable differentiation?

### Audit tasks
- [ ] compare each wrapper to current `amari-dual` APIs
- [ ] identify wrapper-only math that belongs in `amari-dual`
- [ ] resolve `MultiDualNumber::sqrt()` behavior properly
- [ ] verify all domain checks/errors (`ln`, `sqrt`, division)
- [ ] add explicit runtime tests for edge-case derivatives and gradients
- [ ] add JS/TS examples for multivariable AD and matrix utilities

### Deliverable
- [ ] dual wrapper correctness report
- [ ] recommendation list: move logic into crate vs keep in WASM layer

---

## 4. `amari-wasm/src/fusion.rs` — Red

### Known issues found
- historical `v0.12.0` comments
- `sensitivity_analysis()` is explicitly a stub-like fallback
- commented-out deferred `WasmTropicalDualDistribution`
- manual conversion helpers with historical notes

### Clarifying principle
`amari-fusion` remains general-purpose and can keep its LLM/attention framing. The audit goal is to ensure the WASM layer is accurate and extensible, not to narrow the crate.

### Audit questions
- does the current wrapper match current `amari-fusion` capabilities?
- should `sensitivity_analysis()` now be implemented in `amari-fusion` itself?
- should deferred distribution types be restored, removed, or left intentionally absent?
- are attention/holographic bindings runtime-tested enough?

### Audit tasks
- [ ] map all exported fusion APIs to current crate functionality
- [ ] decide fate of `WasmTropicalDualDistribution`
- [ ] replace fallback sensitivity logic with native support or document it as approximate
- [ ] verify bind/unbind/bundle/similarity semantics in node and browser
- [ ] add benchmark candidates for evaluation, similarity, and memory ops
- [ ] document intended extension points for later compiler/kernel APIs

### Deliverable
- [ ] fusion binding parity report
- [ ] explicit decision on all deferred/commented-out APIs

---

## 5. `amari-wasm/src/optimization.rs` — Red

### Known issues found
- batch GPU optimization path inserts `NaN` / `INFINITY` placeholder-style result rows on error

### Audit questions
- is placeholder-style error encoding acceptable for JS users?
- should batch APIs instead return structured result objects or result/error arrays?
- is the async shape appropriate for browser and node use?

### Audit tasks
- [ ] review all optimization result encoding strategies
- [ ] replace implicit placeholder rows with explicit structured error handling where feasible
- [ ] verify async GPU fallback behavior
- [ ] add tests for partial-failure batch scenarios
- [ ] document output layout for all batch methods

### Deliverable
- [ ] optimization API shape recommendation

---

## 6. `amari-wasm/src/enumerative.rs` — Yellow

### Known observations
- largest module in crate
- very large number of exported bindings
- likely high maintenance burden
- many `wasm_bindgen_test` tests exist

### Audit tasks
- [ ] classify exported APIs into stable core vs advanced/experimental groups
- [ ] verify shape/layout conventions for batch interfaces
- [ ] review whether file should be split by subdomain
- [ ] ensure docs/examples cover the most important entry points only
- [ ] verify performance-sensitive batch operations have benchmark coverage

### Deliverable
- [ ] enumerative API tiering plan

---

## 7. `amari-wasm/src/gf2.rs` — Yellow

### Known observations
- large module with strong recent feature relevance
- many native tests
- likely important for 0.19.1 story and downstream correctness

### Audit tasks
- [ ] verify all exposed GF(2) operations align with `amari-core` / `amari-enumerative`
- [ ] add runtime WASM tests for representative matrix/code/matroid operations
- [ ] confirm JS array layout conventions are documented
- [ ] verify binary/matrix APIs do not over-copy unnecessarily

### Deliverable
- [ ] GF(2) runtime validation checklist

---

## 8. `amari-wasm/src/probabilistic.rs` — Yellow

### Known observations
- very large module
- many exported bindings
- some `wasm_bindgen_test` coverage exists

### Audit tasks
- [ ] verify distribution constructors and covariance semantics
- [ ] verify shape validation for batch/sample APIs
- [ ] add reproducibility policy notes if random behavior is exposed
- [ ] benchmark sample generation and batch statistics paths
- [ ] verify browser/node runtime behavior for random sources

### Deliverable
- [ ] probabilistic runtime/seed policy note

---

## 9. `amari-wasm/src/calculus.rs` — Yellow

### Known observations
- large module
- browser `wasm_bindgen_test` coverage exists

### Audit tasks
- [ ] verify field evaluation APIs and dimensional assumptions
- [ ] verify browser tests cover gradient/divergence/curl correctness meaningfully
- [ ] add benchmark cases for batch geometric calculus operations
- [ ] ensure examples map to current `amari-calculus` API

### Deliverable
- [ ] calculus benchmark candidate set

---

## 10. `amari-wasm/src/optical.rs` — Yellow

### Known observations
- large and specialized
- has `wasm_bindgen_test` coverage
- likely sensitive to browser/runtime characteristics

### Audit tasks
- [ ] verify all optical/VSA operations are documented with correct expectations
- [ ] verify browser-focused tests exercise meaningful use cases
- [ ] identify performance-critical paths for benchmark harness

### Deliverable
- [ ] optical browser benchmark candidate set

---

## 11. `amari-wasm/src/measure.rs` — Yellow

### Audit tasks
- [ ] verify measure/integration APIs against current crate behavior
- [ ] test JS shape validation for integrals and Monte Carlo-style routines
- [ ] add runtime smoke tests for representative operations

---

## 12. `amari-wasm/src/automata.rs` — Yellow

### Audit tasks
- [ ] verify grid/state layout conventions are documented
- [ ] verify browser tests cover evolution and rule setup adequately
- [ ] identify candidate demos for examples-suite integration

---

## 13. `amari-wasm/src/functional.rs` — Yellow

### Audit tasks
- [ ] verify operator/matrix layout expectations
- [ ] verify spectral decomposition API is documented clearly for JS users
- [ ] add runtime tests for matrix/operator failures and success paths

---

## 14. `amari-wasm/src/network.rs` — Yellow

### Audit tasks
- [ ] verify graph/network input encoding conventions
- [ ] verify geometric network operations align with crate APIs
- [ ] add runtime smoke tests for at least one end-to-end scenario

---

## 15. `amari-wasm/src/topology.rs` — Yellow

### Known observations
- mixed native and `wasm_bindgen_test` attributes appear together in some tests

### Audit tasks
- [ ] verify test strategy is intentional and not duplicated/confusing
- [ ] verify simplicial/homology APIs are documented with shape expectations
- [ ] add representative browser/node runtime tests

### Deliverable
- [ ] topology test-style cleanup note

---

## 16. `amari-wasm/src/relativistic.rs` — Yellow

### Audit tasks
- [ ] verify physical units/assumptions are documented clearly
- [ ] verify trajectory output layout for JS consumers
- [ ] add browser/node smoke tests for a representative orbital or spacetime scenario

---

## 17. `amari-wasm/src/flynn.rs` — Green

### Audit tasks
- [ ] verify SMT-LIB2 strings and Monte Carlo outputs are stable and documented
- [ ] add JS/TS smoke examples if not already present

---

## 18. `amari-wasm/src/info_geom.rs` — Green

### Audit tasks
- [ ] verify current tests still reflect meaningful public API behavior
- [ ] add minimal node/browser smoke coverage if missing

---

# Runtime Test Strategy Checklist

## Native host tests
- [ ] keep fast correctness tests for wrapper logic and shape validation
- [ ] ensure these do not create false confidence for WASM runtime behavior

## Node WASM tests
- [ ] core multivector/rotor
- [ ] tropical
- [ ] dual
- [ ] fusion
- [ ] gf2
- [ ] probabilistic
- [ ] selected topology/calculus/functional paths

## Browser WASM tests
- [ ] calculus
- [ ] automata
- [ ] optical
- [ ] probabilistic
- [ ] fusion
- [ ] selected enumerative workflows

## JS/TS package tests
- [ ] package import/init
- [ ] TypeScript compile checks for key exported APIs
- [ ] smoke tests against built package, not `latest`

---

# Disabled Test File Resolution

## `amari-wasm/tests/wasm_edge_computing.rs.disabled`

### Current assessment
This file is not merely disabled coverage; it is placeholder/TDD scaffolding with invented stand-in types.
It should **not** be restored verbatim.

### Resolution checklist
- [ ] extract any still-valuable intended scenarios from the file
- [ ] delete or archive placeholder-only content once replacement exists
- [ ] replace with real tests for:
  - [ ] zero-copy / TypedArray interop
  - [ ] device capability detection
  - [ ] async initialization behavior
  - [ ] browser-side batch workloads

### Deliverable
- [ ] new `wasm_runtime_integration.rs` or equivalent real test suite

---

# Benchmark Preparation Checklist

## Core benchmark targets
- [ ] multivector operations
- [ ] tropical batch ops and Viterbi
- [ ] dual differentiation
- [ ] fusion evaluation / similarity / holographic ops
- [ ] GF(2) matrix operations

## For each benchmark
- [ ] define input sizes
- [ ] define runtime(s): native, node, browser
- [ ] record cold and warm runs
- [ ] record binary size impact if module-specific

---

# Immediate Next Actions

## Week 1 target
- [ ] audit `tropical.rs`
- [ ] audit `dual.rs`
- [ ] audit `fusion.rs`
- [ ] inspect and replace disabled integration test file
- [ ] create `docs/wasm/TEST_MATRIX.md`

## Week 2 target
- [ ] audit `optimization.rs`
- [ ] audit `lib.rs`
- [ ] tier/split plan for `enumerative.rs` and `gf2.rs`
- [ ] add first node/browser benchmark harnesses

---

# Completion Criteria

This audit is complete when:

- [ ] every module has a disposition: validated / refactored / split / deprecated / documented
- [ ] all placeholder or stub behavior is either removed or explicitly justified
- [ ] disabled placeholder integration tests are replaced with real runtime tests
- [ ] node/browser/native testing strategy is documented and runnable
- [ ] benchmark harnesses exist for the core public performance paths
