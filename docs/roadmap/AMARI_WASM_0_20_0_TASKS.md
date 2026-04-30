# Amari WASM 0.20.0 Task List

Date: 2026-03-27
Current version: 0.19.1

This document is a **standalone task list** for `amari-wasm`.
It is intentionally separate from:

- algebra extension work in `amari-tropical`, `amari-dual`, and `amari-fusion`
- broader `amari-gpu` redesign and hardware validation

## Goal

Make `amari-wasm` accurate, well-tested, benchmarked, and ready for 0.20.0.

---

## 1. Implementation audit

- [ ] Audit all exported `#[wasm_bindgen]` APIs module-by-module
- [ ] Map each wrapper to the current underlying Rust crate API
- [ ] Identify:
  - [ ] direct wrappers
  - [ ] compatibility shims
  - [ ] partial implementations
  - [ ] placeholders/stubs
  - [ ] obsolete wrappers
- [ ] Remove stale `v0.12.0` migration-era comments where no longer accurate
- [ ] Replace placeholder behavior with real implementations or explicit API changes

### Priority modules
- [ ] `amari-wasm/src/tropical.rs`
- [ ] `amari-wasm/src/dual.rs`
- [ ] `amari-wasm/src/fusion.rs`
- [ ] `amari-wasm/src/optimization.rs`
- [ ] `amari-wasm/src/lib.rs`

Reference audit doc:
- `docs/roadmap/AMARI_WASM_AUDIT_CHECKLIST.md`

---

## 2. Testing overhaul

- [ ] Define explicit test matrix for:
  - [ ] native host tests
  - [ ] `wasm-bindgen-test` node tests
  - [ ] browser tests
  - [ ] JS package smoke tests
  - [ ] TS typecheck smoke tests
- [ ] Replace disabled placeholder integration tests in:
  - [ ] `amari-wasm/tests/wasm_edge_computing.rs.disabled`
- [ ] Add real runtime integration tests for:
  - [ ] TypedArray interop
  - [ ] async initialization
  - [ ] browser execution
  - [ ] node execution
  - [ ] failure-mode handling

---

## 3. Benchmarking and profiling

- [ ] Add benchmark harnesses for node and browser
- [ ] Measure:
  - [ ] wasm binary size
  - [ ] initialization latency
  - [ ] hot-path throughput
  - [ ] batch throughput
  - [ ] JS/WASM marshalling overhead
- [ ] Benchmark representative modules:
  - [ ] core multivector ops
  - [ ] tropical batch ops
  - [ ] dual differentiation
  - [ ] fusion evaluation / similarity
  - [ ] GF(2) operations

---

## 4. Packaging and release hygiene

- [ ] Ensure npm-facing docs reflect current version line
- [ ] Add/verify JS and TS consumer examples
- [ ] Make sure examples-suite uses a workspace-aligned package flow for testing
- [ ] Document supported runtime matrix clearly

---

## 0.20.0 Exit Criteria

- [ ] WASM API audit complete
- [ ] placeholder or shim behavior resolved or explicitly justified
- [ ] disabled integration tests replaced with real runtime tests
- [ ] node/browser/native test matrix documented and runnable
- [ ] benchmark baseline published
