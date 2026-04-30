# Amari 0.20.0 / 0.21.0 Actionable Task List

> Planning update: the broader release sequence through 0.24.0 is now captured in `docs/roadmap/V0_20_0_TO_V0_24_0_RELEASE_SEQUENCE.md`. In that plan, `0.20.0` ships the `amari-gpu` stabilization baseline, `0.21.0` focuses on substantial `amari-tropical` and `amari-dual` extensions, `0.22.0` introduces `amari-cgt` and `amari-surreal`, `0.23.0` introduces `amari-surcomplex` and `amari-rewrite`, and `0.24.0` introduces `amari-cli` while revisiting GPU benchmark, dispatch, coverage, and `wgpu` migration issues.

Date: 2026-03-27
Current version: 0.19.1

This task list turns the gap analysis into a concrete execution plan.

## Change Policy for This Roadmap

For the Amari workspace broadly, the operating rule is:

- **extend, do not redefine**
- preserve existing crate identities and downstream expectations
- prefer additive APIs, new modules, feature-gated capabilities, and documentation expansion
- avoid semantic repurposing of established crates unless absolutely necessary

Note on `amari-fusion`: its LLM-oriented framing is **not** considered a problem by itself. The goal is to **preserve that general-purpose framing** while extending the crate for compiler-analysis, scheduling, and GPU-kernel optimization use cases.

### Special-case exception: `amari-gpu`

`amari-gpu` is the one crate in this roadmap with broad latitude for substantial redesign.
Because it has historically been maintained more for coverage than robustness, and because there are currently no important downstream dependencies constraining it, it may be:

- significantly reorganized
- expanded to expose many more Amari operations
- reworked for correctness, performance, and hardware validation
- reshaped to more closely mirror the breadth of `amari-wasm` where appropriate

In short:

- **most crates:** additive extension with strong backward-compatibility bias
- **`amari-gpu`:** robustness-first redesign is allowed

---

## Milestone Summary

### 0.20.0 — amari-gpu coverage, validation, and API honesty

Primary outcome:
- make `amari-gpu` comprehensively expose as many practical Amari operations as possible, with truthful API boundaries, CPU-baseline correctness tests, real-hardware validation, and benchmark/crossover documentation

Active focus note:
- `amari-wasm` audit/hardening and non-GPU algebra extension tracks remain documented, but are deferred while 0.20.0 development focuses on `amari-gpu`.

### 0.21.0 — Tropical / Dual extension release

Primary outcome:
- considerably extend `amari-tropical` and `amari-dual` for compiler design, scheduling, kernel optimization, performance modeling, and higher-order/batched AD use cases
- keep `amari-fusion` extension work additive and example-driven where it benefits from the new tropical/dual capabilities

### 0.22.0 — CGT / surreal foundations release

Primary outcome:
- introduce `amari-cgt` for combinatorial game theory
- introduce `amari-surreal` for surreal numbers

### 0.23.0 — Surcomplex / rewrite-systems release

Primary outcome:
- introduce `amari-surcomplex`
- introduce `amari-rewrite` for rewrite rules and term-rewriting-system workflows

### 0.24.0 — CLI / GPU revisit release

Primary outcome:
- introduce `amari-cli`
- revisit `amari-gpu` follow-up issues after the 0.21.0, 0.22.0, and 0.23.0 crate work is available
- defer benchmark-baseline completion, calibrated dispatch, high-upside kernel optimization, future crate GPU coverage, and `wgpu 29` migration planning from the 0.20.x patch lane to this cycle

---

# 0.20.0 TASK LIST

## Active 0.20.0 focus — amari-gpu

The implementation-facing source of truth for the active 0.20.0 focus is:

- `docs/roadmap/AMARI_GPU_0_20_0_RELEASE_PLAN.md`

The WASM and algebra-extension epics below are retained as deferred backlog, not the current 0.20.0 execution focus.

## Deferred Epic A — amari-wasm implementation audit

### A1. Inventory all exported bindings
- [ ] Enumerate all `#[wasm_bindgen]` types/functions by module
- [ ] Map each binding to its source domain-crate API
- [ ] Mark each binding as one of:
  - [ ] direct wrapper
  - [ ] manual compatibility shim
  - [ ] partial implementation
  - [ ] placeholder/stub
  - [ ] obsolete wrapper
- [ ] Produce module-by-module audit notes for:
  - [ ] `lib.rs`
  - [ ] `tropical.rs`
  - [ ] `dual.rs`
  - [ ] `fusion.rs`
  - [ ] `enumerative.rs`
  - [ ] `gf2.rs`
  - [ ] `probabilistic.rs`
  - [ ] `calculus.rs`
  - [ ] `optimization.rs`
  - [ ] `functional.rs`
  - [ ] `relativistic.rs`
  - [ ] `topology.rs`
  - [ ] `automata.rs`
  - [ ] `info_geom.rs`
  - [ ] `measure.rs`
  - [ ] `network.rs`
  - [ ] `optical.rs`
  - [ ] `flynn.rs`

### A2. Remove stale historical compatibility language
- [ ] Remove/replace references to missing `v0.12.0` APIs where no longer accurate
- [ ] Replace comments like “manual implementation as ... not in v0.12.0 API” with current rationale or remove them
- [ ] Replace “placeholder” comments with tracked issues/tasks or full implementations
- [ ] Ensure public docs describe current crate behavior, not historical migration state

### A3. Fix incomplete wrappers
- [ ] Review `amari-wasm/src/tropical.rs` for compatibility shims and placeholder behavior
- [ ] Review `amari-wasm/src/dual.rs` for manual math wrappers and any stub-like gradient behavior
- [ ] Review `amari-wasm/src/fusion.rs` for deferred features like sensitivity analysis / distribution support
- [ ] Decide per incomplete API:
  - [ ] implement properly now
  - [ ] gate behind feature
  - [ ] remove from public API
  - [ ] document as intentionally approximate behavior

### A4. Align binding semantics with Rust crates
- [ ] Ensure naming consistency between Rust and JS/TS APIs
- [ ] Ensure error mapping to `JsValue` is consistent and descriptive
- [ ] Ensure dimension/shape validation is done uniformly
- [ ] Ensure all array-heavy APIs clearly specify layout conventions
- [ ] Check memory ownership/free patterns in docs and examples

---

## Epic B — amari-wasm testing overhaul

### B1. Restore disabled integration coverage
- [ ] Inspect `amari-wasm/tests/wasm_edge_computing.rs.disabled`
- [ ] Decide whether to:
  - [ ] re-enable directly
  - [ ] split into smaller focused suites
  - [ ] replace with a new runtime integration suite
- [ ] Add it back to test workflows

### B2. Create explicit runtime test matrix
- [ ] Define test categories:
  - [ ] native host correctness tests
  - [ ] `wasm-bindgen-test` node tests
  - [ ] browser tests
  - [ ] JS package smoke tests
  - [ ] examples-suite integration smoke tests
- [ ] Document exact commands in `docs/wasm/TEST_MATRIX.md`
- [ ] Add per-module coverage status table

### B3. Increase module-level runtime coverage
- [ ] Add/verify WASM runtime tests for:
  - [ ] tropical
  - [ ] dual
  - [ ] fusion
  - [ ] gf2
  - [ ] probabilistic
  - [ ] calculus
  - [ ] optimization
  - [ ] topology
  - [ ] functional
  - [ ] relativistic
- [ ] Identify modules still only tested natively and add runtime tests where useful

### B4. Add JS/TS consumer tests
- [ ] Add Node-based import/init smoke test for generated package
- [ ] Add browser import/init smoke test
- [ ] Add TS typecheck smoke tests for representative APIs
- [ ] Add examples-suite CI smoke route(s) using built package instead of `latest`

### B5. Add failure-mode tests
- [ ] invalid dimension/shape inputs
- [ ] domain errors (`ln`, `sqrt`, division by zero)
- [ ] out-of-bounds accessors
- [ ] malformed batch buffers
- [ ] invalid graph/matrix dimensions
- [ ] unsupported runtime capability handling

---

## Epic C — amari-wasm benchmarking and profiling

### C1. Define benchmark categories
- [ ] binary size
- [ ] initialization latency
- [ ] hot-call throughput
- [ ] batch throughput
- [ ] large-array marshal/unmarshal overhead
- [ ] memory growth behavior
- [ ] browser vs node comparison
- [ ] WebGPU interop overhead where applicable

### C2. Build benchmark harnesses
- [ ] Node benchmark harness
- [ ] browser benchmark harness
- [ ] examples-suite benchmark/demo integration where useful
- [ ] reproducible benchmark scripts checked into repo

### C3. Establish baseline workloads
- [ ] core multivector ops
- [ ] tropical batch ops / Viterbi
- [ ] dual scalar and multivariable differentiation
- [ ] fusion evaluation / similarity / attention-like ops
- [ ] gf2 matrix operations
- [ ] selected enumerative workloads

### C4. Publish baseline results
- [ ] Add `docs/wasm/BENCHMARKS.md`
- [ ] Record machine specs
- [ ] Record runtime versions (browser, node, wasm-pack)
- [ ] Record binary sizes and representative latency/throughput numbers

---

## Epic D — amari-gpu redesign + real GPU validation on GB10 and RTX 5080

### D0. Redesign target for amari-gpu
- [ ] Audit current `amari-gpu` module coverage against workspace crates
- [ ] Define target surface area relative to `amari-wasm`
- [ ] Decide which operations belong in:
  - [ ] `amari-gpu` directly
  - [ ] domain crates with GPU feature support
  - [ ] shared abstractions used by both `amari-gpu` and `amari-wasm`
- [ ] Identify obsolete or weakly justified kernels for rewrite/removal
- [ ] Define a correctness-first architecture before optimization passes

### D1. Hardware validation setup
- [ ] Record exact GB10 environment details
- [ ] Record exact RTX 5080 environment details
- [ ] Verify WebGPU/wgpu adapter detection and backend selection
- [ ] Create repeatable benchmark environment notes

### D2. Re-run GPU-heavy tests on real hardware
- [ ] `amari-gpu` default tests
- [ ] `amari-gpu` all-feature tests
- [ ] GPU-sensitive `amari-wasm` workloads where relevant
- [ ] collect pass/fail by module

### D3. Validate numerical correctness against CPU baselines
- [ ] core geometric algebra kernels
- [ ] dual kernels
- [ ] calculus kernels
- [ ] enumerative kernels
- [ ] topology kernels
- [ ] probabilistic kernels
- [ ] dynamics kernels
- [ ] fusion/holographic kernels

### D4. Measure real performance
- [ ] throughput vs CPU
- [ ] latency vs CPU
- [ ] batch-size crossover points
- [ ] adapter-specific regressions
- [ ] precision deviations and tolerance windows

### D5. Expand amari-gpu coverage toward platform parity
- [ ] Prioritize high-value operation families to expose through `amari-gpu`
- [ ] Bring GPU coverage closer to `amari-wasm` where technically justified
- [ ] Add missing GPU integration points for tropical/dual/fusion/compiler-facing workloads
- [ ] Ensure every exposed GPU path has CPU baseline validation
- [ ] Define per-module fallback behavior and capability detection

### D6. Publish validation/redesign report
- [ ] Add `docs/gpu/HARDWARE_VALIDATION_GB10_RTX5080.md`
- [ ] Note kernels that are production-ready
- [ ] Note kernels needing redesign/tuning
- [ ] Note hardware/backend-specific caveats

---

## Epic E — release hygiene and metadata cleanup

### E1. Fix version/documentation drift
- [ ] Update `amari-tropical/README.md` from `0.12` to current/release-target versioning
- [ ] Update `amari-dual/README.md` from `0.12`
- [ ] Update `amari-fusion/README.md` from `0.12`
- [ ] Update any `v0.12.0` migration-era wording that is now misleading

### E2. Fix JS package metadata drift
- [ ] Pin `examples-suite` dependency strategy appropriately
- [ ] Fix `typescript/package.json` repository URL
- [ ] Fix `typescript/package.json` homepage URL
- [ ] Review npm package metadata for consistency with workspace release

### E3. Fix Cargo warnings
- [ ] Resolve `default-features is ignored` warnings in member crates
- [ ] Re-run workspace tests and ensure warnings are gone or intentionally documented

### E4. Tighten release workflow
- [ ] Define 0.20.0 release checklist
- [ ] Ensure version-sync scripts cover all JS/WASM package metadata
- [ ] Ensure benchmark/validation docs are part of release notes

---

## 0.20.0 Exit Criteria
- [ ] `amari-wasm` audit complete
- [ ] disabled WASM integration tests restored or replaced
- [ ] documented host/node/browser test matrix exists
- [ ] benchmark baselines published
- [ ] GB10 and RTX 5080 validation/redesign report published
- [ ] `amari-gpu` redesign direction documented and underway, with expanded coverage plan
- [ ] crate READMEs and package metadata aligned with current version line
- [ ] Cargo feature warnings addressed

---

# 0.21.0 TASK LIST

## Epic F — amari-tropical extensions for compiler/kernel work

### F1. Introduce reusable semiring abstractions
- [ ] Design `Semiring` trait
- [ ] Design `IdempotentSemiring` trait
- [ ] Design tropical convention abstraction (`MaxPlus`, `MinPlus`)
- [ ] Refactor core algorithms to use semiring abstractions where practical

### F2. Extend matrix/graph infrastructure
- [ ] Add sparse tropical matrix support
- [ ] Add graph-oriented APIs suitable for compiler analysis
- [ ] Add shortest/longest path helpers for DAG/CFG-like workloads
- [ ] Add fixpoint/dataflow-oriented utilities

### F3. Add compiler-oriented algorithms
- [ ] schedule cost propagation
- [ ] dependence distance / latency accumulation
- [ ] profitability scoring for fusion/fission
- [ ] path scoring for lowering / instruction selection experiments
- [ ] tropical dynamic programming patterns useful for compiler passes

### F4. Add GPU-kernel oriented APIs
- [ ] launch configuration scoring
- [ ] tile-size / block-size score propagation
- [ ] memory hierarchy penalty modeling
- [ ] occupancy-inspired score models

### F5. Add benchmarks and examples
- [ ] compiler/dataflow example
- [ ] kernel scheduling example
- [ ] sparse graph/pathfinding benchmark
- [ ] tropical-vs-conventional scoring benchmark

---

## Epic G — amari-dual extensions for optimization and analysis

### G1. Add higher-order differentiation support
- [ ] evaluate nested-dual design vs explicit second-order types
- [ ] implement Hessian or Hessian-vector support
- [ ] add tests for second-order correctness

### G2. Add batched/structured differentiation utilities
- [ ] Jacobian-vector products
- [ ] vector-Jacobian products
- [ ] directional derivatives
- [ ] batched Jacobian helpers
- [ ] parameter-sweep helpers for optimization workloads

### G3. Improve low-allocation performance paths
- [ ] const-generic small-gradient APIs where useful
- [ ] reduce heap allocations in hot paths
- [ ] add reusable workspace buffers where practical

### G4. Add compiler/kernel examples
- [ ] differentiable cost model demo
- [ ] autotuning gradient demo
- [ ] sensitivity analysis over kernel parameters

---

## Epic H — amari-fusion extensions for broader optimization workloads

### H1. Preserve general-purpose fusion framing
- [ ] keep LLM/attention use cases in docs
- [ ] add compiler/kernel/scheduling use cases alongside them
- [ ] update README and examples to reflect broader applicability

### H2. Extend fusion APIs for optimization/program representations
- [ ] schedule or plan embedding utilities
- [ ] richer similarity/evaluation APIs for optimization states
- [ ] better sensitivity analysis support as crate-native API
- [ ] optional multi-objective scoring helpers

### H3. Add compiler/kernel-oriented workflows
- [ ] kernel plan comparison example
- [ ] schedule interpolation / search example
- [ ] resource-state or dependence-geometry experiment
- [ ] symbolic retrieval of known-good optimization patterns

### H4. Improve fusion benchmarks
- [ ] evaluation throughput benchmark
- [ ] similarity search benchmark
- [ ] holographic bind/unbind benchmark
- [ ] optimization-search benchmark

### H5. Align wasm bindings with new fusion capabilities
- [ ] expose native sensitivity analysis once formalized
- [ ] remove deferred/stubbed fusion WASM behavior
- [ ] add JS/TS examples for new optimization workflows

---

## Epic I — cross-crate integration work for 0.21.0

### I1. amari-wasm bindings for new algebra APIs
- [ ] bind new tropical semiring/compiler APIs
- [ ] bind new dual higher-order/batched APIs
- [ ] bind new fusion optimization APIs

### I2. examples-suite additions
- [ ] tropical scheduling / path-cost demo
- [ ] differentiable optimization demo
- [ ] fusion-based plan comparison demo
- [ ] benchmark visualization page if feasible

### I3. documentation updates
- [ ] add compiler/kernel-oriented docs for tropical
- [ ] add optimization-oriented docs for dual
- [ ] broaden fusion docs with non-LLM examples
- [ ] update root README release highlights

---

## 0.21.0 Exit Criteria
- [ ] tropical has semiring abstractions and at least one compiler-oriented module
- [ ] dual has at least one substantial higher-order or batched AD expansion
- [ ] fusion has explicit compiler/kernel optimization examples without losing LLM/general-purpose framing
- [ ] new APIs are exposed via wasm where appropriate
- [ ] examples and benchmarks demonstrate practical value

---

# Recommended Order of Execution

## First 2 weeks
- [ ] Start `amari-wasm` audit
- [ ] fix stale docs/metadata drift
- [ ] restore/replace disabled wasm integration tests
- [ ] define benchmark and hardware validation templates

## Next phase
- [ ] run GPU validation on GB10 and RTX 5080
- [ ] stabilize wasm test matrix
- [ ] publish 0.20.0 benchmark + validation docs

## After 0.20.0 branch point
- [ ] begin tropical semiring/compiler API work for 0.21.0
- [ ] add dual higher-order/batched extensions for 0.21.0
- [ ] extend fusion examples only where they naturally consume new tropical/dual capabilities
- [ ] prepare 0.22.0 crate plans for `amari-cgt` and `amari-surreal`
- [ ] prepare 0.23.0 crate plans for `amari-surcomplex` and `amari-rewrite`
- [ ] defer broad `amari-gpu` follow-up work to 0.24.0 alongside `amari-cli`
- [ ] update wasm bindings and examples-suite where appropriate for each release train

---

# Suggested Immediate Next Deliverable

The highest-value immediate artifact is:

- [ ] `amari-wasm` module-by-module audit checklist with concrete findings and fix recommendations

That should be the next working document before implementation begins.
