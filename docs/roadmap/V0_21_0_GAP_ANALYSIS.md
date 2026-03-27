# Amari Gap Analysis and 0.21.0 Delivery Plan

Date: 2026-03-27
Current workspace version: 0.19.1
Target horizon: 0.20.0 → 0.21.0

## Executive Summary

This repository is much closer to a 1.0-ready platform than its older context documents suggest, but there are still clear gaps between:

1. **what the READMEs and package metadata claim**
2. **what is currently tested and verified**
3. **what is production-ready on real hardware, especially GPU/WASM**

The highest-leverage pre-1.0 work for the next two releases is:

- **amari-wasm hardening**: implementation audit, API cleanup, browser/node test matrix, real-world performance/size benchmarking, WebGPU/WebAssembly integration testing
- **amari-gpu validation on actual hardware**: now possible with GB10 and RTX 5080 access
- **algebra-core extension work** in `amari-tropical`, `amari-dual`, and `amari-fusion` for compiler, IR, scheduling, and GPU-kernel use cases

Recommended release framing:

- **0.20.0** = WASM/GPU validation + API cleanup + benchmark infrastructure
- **0.21.0** = tropical/compiler/kernel extensions + dual/fusion expansion + end-to-end demos

---

## 1. Verified Current State

### Workspace shape

The workspace currently contains 22 packages:

- `amari`
- `amari-core`
- `amari-tropical`
- `amari-dual`
- `amari-network`
- `amari-fusion`
- `amari-info-geom`
- `amari-automata`
- `amari-enumerative`
- `amari-relativistic`
- `amari-gpu`
- `amari-optimization`
- `amari-flynn`
- `amari-flynn-macros`
- `amari-measure`
- `amari-calculus`
- `amari-holographic`
- `amari-probabilistic`
- `amari-functional`
- `amari-topology`
- `amari-dynamics`
- `amari-wasm`

### Testing snapshot

Observed locally:

- `cargo +stable test -p amari-tropical` ✅
- `cargo +stable test -p amari-dual` ✅
- `cargo +stable test -p amari-fusion` ✅
- `cargo +stable test -p amari-wasm -- --list` shows **99 tests** in the crate
- `cargo +stable test --workspace --all-features --quiet` ran successfully across many crates, but **timed out in GPU-heavy paths** in `amari-gpu`

### 1.0 audit status

`docs/1.0-audit.md` is the best correctness-status source right now.

Current audited-complete items:

- `amari-core`
- `amari-enumerative`

Most other crates remain pending in the formal audit sequence.

---

## 2. Gap Analysis: Claims vs Verified State

## A. Repository-level gaps

### Gap A1 — Project messaging is more current than some crate docs

The top-level repo is at `0.19.1`, but several crate READMEs still describe installation and API state in `0.12.x` language.

Examples:

- `amari-tropical/README.md` still tells users to depend on `amari-tropical = "0.12"`
- `amari-dual/README.md` still tells users to depend on `amari-dual = "0.12"`
- `amari-fusion/README.md` still tells users to depend on `amari-fusion = "0.12"`
- `amari-wasm` source comments repeatedly reference missing features from `v0.12.0`

**Impact:** Documentation trust erosion; high chance of misleading users and assistants.

**Priority:** High

### Gap A2 — Toolchain ergonomics are rough

The repo default toolchain is nightly, and ordinary commands can trigger rustup sync behavior even though stable is still a viable path for regular testing.

**Impact:** Friction for contributors and CI reproducibility.

**Priority:** Medium

### Gap A3 — Cargo feature warnings exist

Running tests shows warnings like:

- `default-features is ignored for amari-core`
- similar warnings for `amari-info-geom`, `amari-network`, `amari-relativistic`

These come from how some workspace dependencies are specified versus overridden in member crates.

**Impact:** Future Cargo compatibility risk.

**Priority:** Medium

### Gap A4 — Package metadata drift in JS ecosystem

Examples found:

- `examples-suite/package.json` depends on `@justinelliottcobb/amari-wasm: "latest"` instead of pinned workspace-aligned version
- `typescript/package.json` still has placeholder repository/homepage (`your-username`)

**Impact:** Release drift, reproducibility risk, misleading package metadata.

**Priority:** Medium

---

## B. amari-wasm gaps

This is the biggest near-term opportunity.

### Gap B1 — Large surface area, uneven validation depth

`amari-wasm/src` contains bindings for:

- automata
n- calculus
- dual
- enumerative
- flynn
- functional
- fusion
- gf2
- info_geom
- measure
- network
- optical
- optimization
- probabilistic
- relativistic
- topology
- tropical
- plus the core multivector/rotor layer in `lib.rs`

That is a very large exported surface for a single binding crate.

**Observed reality:**
- there are many tests in the crate
- but browser/node/wasm-target coverage is fragmented
- there is at least one disabled integration file: `amari-wasm/tests/wasm_edge_computing.rs.disabled`

**Impact:** High risk that native `cargo test` confidence exceeds actual WASM/runtime confidence.

**Priority:** Critical

### Gap B2 — Many wrappers still contain old compatibility notes and manual patches

Search results show numerous comments like:

- `Manual implementation as ... not in v0.12.0 API`
- `placeholder`
- `not yet available`
- `TODO: Re-enable when ... is added`

Examples:

- `amari-wasm/src/tropical.rs`
- `amari-wasm/src/dual.rs`
- `amari-wasm/src/fusion.rs`

**Interpretation:** parts of the binding layer are carrying historical compatibility shims that may no longer match current domain-crate APIs.

**Impact:** High chance of stale wrappers, suboptimal semantics, and missing direct bindings.

**Priority:** Critical

### Gap B3 — Incomplete runtime-specific testing strategy

`amari-wasm` contains both:

- standard Rust `#[test]` unit tests
- `wasm_bindgen_test` browser/WASM tests

But the runtime strategy is not unified:

- some modules test natively only
- some test with `wasm_bindgen_test`
- one browser integration file is disabled
- package scripts only run `wasm-pack test --node`

**Missing matrix:**

- native host correctness tests
- `wasm32-unknown-unknown` node tests
- browser tests
- JS/TS consumer smoke tests
- performance benchmarks for size + latency + throughput

**Priority:** Critical

### Gap B4 — No clear benchmark/perf baseline for WASM bundle quality

There is no obvious checked-in benchmark suite for:

- wasm binary size
- init latency
- hot-path latency
- browser throughput
- node throughput
- WebGPU interop overhead
- memory pressure / GC interaction

Given your new hardware access, this is the ideal time to establish real baselines.

**Priority:** Critical

### Gap B5 — Binding design is too monolithic for future optimization work

`amari-wasm` is now ~25k lines across many modules. Some modules are very large:

- `enumerative.rs` ~1898 lines
- `gf2.rs` ~1341 lines
- `fusion.rs` ~1281 lines
- `probabilistic.rs` ~1201 lines
- `lib.rs` ~1083 lines
- `calculus.rs` ~1074 lines

**Impact:** difficult to audit, benchmark, and evolve safely.

**Priority:** High

---

## C. amari-tropical gaps

### Gap C1 — Strong basic semiring support, weak compiler/kernel positioning

Current strengths:

- `TropicalNumber`
- `TropicalMatrix`
- `TropicalMultivector`
- Viterbi support
- tropical polytope support
- semiring verification helpers

Current missing pieces for compiler/GPU-kernel use:

- explicit **semiring traits** for generic algorithm reuse
- **min-plus** and configurable tropical convention as first-class API, not just verification-level ideas
- graph algorithms positioned for compiler use (shortest path, dominance, scheduling, dataflow)
- tropical linear-algebra kernels with batching/tiling APIs
- sparse matrix / sparse graph support
- IR-lowering or kernel-cost-model abstractions

**Priority:** High

### Gap C2 — README and product framing lag actual ambition

The crate is still framed mainly around HMMs, pathfinding, and generic optimization. It is not yet framed as:

- a semiring engine for compiler passes
- a scheduling algebra
- a kernel cost/profitability algebra
- a max-plus foundation for GPU launch/tile heuristics

**Priority:** Medium

### Gap C3 — GPU feature exists, but platform-level validation is unclear

`amari-tropical` has a `gpu` feature, but the repo-wide GPU validation story still centers on `amari-gpu`, and tropical GPU integration is explicitly disabled there.

**Priority:** High

---

## D. amari-dual gaps

### Gap D1 — Good forward-mode base, missing higher-order differentiation story

Current strengths:

- `DualNumber`
- `MultiDualNumber`
- differentiable functions
- dual multivectors
- gradient and Jacobian helpers in verified layers

Potential extension gaps for compiler/kernel use:

- Hessians / second-order forward mode
- directional derivatives as explicit API
- Jacobian-vector and vector-Jacobian products
- batched autodiff for kernel parameter sweeps
- dual-number support for cost models / occupancy models
- stronger no-alloc / stack-friendly APIs for hot kernels

**Priority:** High

### Gap D2 — WASM bindings still contain historical shims

`amari-wasm/src/dual.rs` includes manual compatibility logic and at least one documented stub-like workaround around `MultiDualNumber::sqrt()`.

**Priority:** High

### Gap D3 — Compiler/optimization positioning is still indirect

The crate is excellent for mathematical AD, but not yet productized for:

- cost model tuning
- autotuning gradients
- schedule search
- differentiable kernel parameterization
- differentiable compiler heuristics

**Priority:** Medium

---

## E. amari-fusion gaps

### Gap E1 — Interesting concept, but domain narrative is still LLM-centric

Current implementation includes:

- `TropicalDualClifford`
- attention modules
- evaluation metrics
- optimizer support
- holographic binding/unbinding/bundling

But for your stated direction, this crate could become a stronger foundation for:

- tropical-semiring guided compiler transformations
- differentiable scheduling
- geometric representations of kernel/resource state
- multi-objective optimization over kernel execution plans

**Priority:** High

### Gap E2 — Some WASM bindings are still explicitly stubbed / deferred

`amari-wasm/src/fusion.rs` includes historical notes and TODOs around:

- sensitivity analysis
- distribution support
- manual conversion helpers

That suggests the binding layer may lag the actual crate API.

**Priority:** High

### Gap E3 — Testing is respectable, but integration stories need clearer benchmarks

`amari-fusion` passes tests, but there is no clearly visible benchmark suite around:

- attention alternatives
- binding/unbinding throughput
- similarity search throughput
- compiler/kernel-inspired workloads

**Priority:** Medium

---

## 3. What To Do Next: Recommended Release Plan

## Release 0.20.0 — WASM/GPU hardening release

### Goal

Make `amari-wasm` and GPU-backed integration work trustworthy, measurable, and maintainable.

### 0.20.0 work items

#### 1. amari-wasm API audit

- remove stale `v0.12.0` comments and historical compatibility notes where no longer true
- replace manual wrappers with direct domain-crate API usage where possible
- identify all placeholder/stub/deferred exports
- either implement them properly or remove/hide them from public docs

#### 2. Establish WASM test matrix

Add CI/dev workflows for:

- `cargo test -p amari-wasm`
- `wasm-pack test --node`
- browser `wasm-bindgen-test`
- JS/TS smoke tests against generated package
- examples-suite basic integration smoke tests

#### 3. Re-enable and modernize disabled WASM integration tests

- inspect `amari-wasm/tests/wasm_edge_computing.rs.disabled`
- either restore it or replace it with a new runtime-focused suite

#### 4. Add benchmark harnesses

Create reproducible benchmarks for:

- wasm binary size
- cold init time
- hot-call throughput
- large-array marshalling overhead
- node vs browser performance
- WebGPU interop paths where relevant

#### 5. GPU validation on real hardware

On GB10 + RTX 5080:

- run `amari-gpu` workloads that previously timed out or were unverified
- compare CPU vs GPU numerical outputs
- capture throughput / latency / occupancy-like metrics
- identify kernel-specific regressions and adapter-specific issues

#### 6. Metadata/documentation cleanup

- pin examples-suite dependency versions to workspace release versions where appropriate
- fix `typescript/package.json` repository/homepage placeholders
- align README installation snippets to `0.19.x` → `0.20.0`
- document supported WASM runtimes clearly

### 0.20.0 exit criteria

- `amari-wasm` has explicit host/node/browser testing story
- no known placeholder bindings remain undocumented
- performance baseline published for at least core, tropical, dual, and fusion paths
- real GPU validation report exists for GB10 and RTX 5080

---

## Release 0.21.0 — Tropical/compiler/kernel expansion release

### Goal

Extend `amari-tropical`, `amari-dual`, and `amari-fusion` into a stronger algebraic foundation for compiler analysis and GPU kernel design.

### 0.21.0 work items

## A. amari-tropical extensions

### A1. Introduce semiring abstractions

Add traits such as:

- `Semiring`
- `IdempotentSemiring`
- `PathSemiring`
- `TropicalSemiring<Convention = MaxPlus | MinPlus>`

This allows reuse in:

- graph algorithms
- dataflow analysis
- cost propagation
- scheduling and dependence analysis

### A2. Add compiler-oriented graph algorithms

Candidate modules:

- shortest / longest path on CFG or DAG-like structures
- dataflow fixpoint acceleration via idempotent semiring ops
- dependence-distance accumulation
- schedule feasibility / profitability scoring
- instruction selection scoring in tropical form

### A3. Add sparse structures

- `SparseTropicalMatrix`
- graph adjacency abstractions
- CSR/COO-based kernels

### A4. Add tropical kernel-cost APIs

Possible abstractions:

- kernel launch cost models
- tile-size score propagation
- memory hierarchy penalty models
- fusion/fission profitability in tropical form

## B. amari-dual extensions

### B1. Add higher-order differentiation support

Options:

- nested dual numbers
- dedicated second-order dual types
- Hessian / Hessian-vector APIs

### B2. Add optimization-oriented differentiation utilities

- batched Jacobians
- Jacobian-vector products
- vector-Jacobian products
- directional derivatives
- sensitivity maps for kernel parameters

### B3. Add low-allocation / fixed-size APIs

For hot compiler/kernel loops, emphasize:

- const-generic gradients where possible
- stack-backed small-gradient representations
- reusable workspace buffers

## C. amari-fusion extensions

### C1. Reframe fusion around optimization/program spaces

Extend beyond LLM framing with concepts like:

- schedule embeddings
- kernel plan embeddings
- dependence geometry
- multi-objective evaluation for latency / occupancy / locality / numerics

### C2. Add fusion-based multi-objective optimizer examples

Example directions:

- tile-size search
- launch-config search
- memory-layout search
- compiler pass ordering heuristics

### C3. Formalize sensitivity analysis

Implement crate-native APIs so WASM no longer has to fake or stub them.

### C4. Add benchmark workloads tied to real use cases

- tropical attention vs standard approximations
- kernel-plan similarity search
- binding/unbinding throughput for symbolic schedule retrieval
- differentiable optimization of schedule parameters

### 0.21.0 exit criteria

- tropical crate has explicit semiring/generic-algorithm story
- dual crate has at least one robust higher-order or batched differentiation expansion
- fusion crate has at least one compiler/kernel-oriented end-to-end workflow
- wasm bindings expose these additions cleanly
- benchmark docs show practical advantages on your available hardware

---

## 4. Recommended Work Sequencing

### Phase 1 — immediate

1. audit `amari-wasm` wrappers for stale/manual compatibility code
2. restore/replace disabled WASM integration tests
3. create benchmark harness for WASM + GPU comparison
4. clean README/package metadata drift

### Phase 2 — 0.20.0 stabilization

1. validate `amari-gpu` on GB10 and RTX 5080
2. publish baseline benchmark/report artifacts
3. fix adapter/runtime-specific GPU and WASM issues
4. update docs to match verified behavior

### Phase 3 — 0.21.0 algebra expansion

1. add semiring abstractions to `amari-tropical`
2. add compiler/dataflow/kernel use-case modules
3. add higher-order/batched utilities to `amari-dual`
4. extend `amari-fusion` toward kernel-plan / schedule optimization
5. bind the new APIs in `amari-wasm`

---

## 5. Concrete Shortlist of High-Priority Issues

### Must-fix before 0.20.0

- outdated `0.12` README install/version text in tropical/dual/fusion
- stale/manual compatibility comments in `amari-wasm`
- disabled WASM integration test file
- missing explicit WASM runtime test matrix
- JS metadata drift (`latest`, placeholder repository URLs)
- Cargo `default-features` warnings

### Must-fix before 0.21.0

- tropical crate lacks compiler-oriented semiring abstraction layer
- dual crate lacks stronger higher-order/batched AD story
- fusion crate remains overly LLM-narrated relative to broader optimization goals
- GPU validation still needs hard data on real hardware

---

## 6. Suggested Deliverables

### Deliverables for 0.20.0

- `docs/wasm/TEST_MATRIX.md`
- `docs/wasm/BENCHMARKS.md`
- `docs/gpu/HARDWARE_VALIDATION_GB10_RTX5080.md`
- updated READMEs and package metadata
- restored WASM integration tests

### Deliverables for 0.21.0

- compiler-oriented tropical module(s)
- dual higher-order/batched AD APIs
- fusion kernel/schedule optimization examples
- examples-suite demos showing browser-facing value
- benchmark comparison docs across CPU / WASM / GPU

---

## Bottom Line

The repo is already broad and sophisticated enough for a serious 1.0 trajectory.
The biggest remaining issue is not lack of scope; it is **closing the gap between ambitious surface area and hardware/runtime-validated implementation quality**.

With your current hardware access, the strongest path is:

- **0.20.0:** make WASM/GPU claims fully trustworthy
- **0.21.0:** extend tropical/dual/fusion into compiler and GPU-kernel design territory
