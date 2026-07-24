# Algebra Extension 0.21.0 Task List

Date: 2026-03-27
Current version: 0.19.1

This document is a **standalone task list** for the 0.21.0 algebra extension release.

Primary 0.21.0 focus:

- considerably extending `amari-tropical`
- considerably extending `amari-dual`

Secondary/additive 0.21.0 work:

- `amari-fusion` examples and integrations where they naturally consume the new tropical/dual capabilities

This work is intentionally separate from:

- `amari-wasm` audit/hardening
- `amari-gpu` redesign and hardware validation, now deferred to the 0.26.0 GPU/Borsalino modernization cycle after the 0.24.0 discovery/holographic release and 0.25.0 rewrite/inverse expansion

## Guiding Rule

These crates should be **extended, not redefined**.
Existing downstream users and current crate identities should be preserved.

## Goal

Deliver additive, backward-compatible extensions that broaden these crates toward:

- compiler analysis
- scheduling
- optimization
- GPU-kernel design
- performance modeling

without reducing their existing mathematical and application scope.

---

# 1. amari-tropical

## Main direction
Strengthen `amari-tropical` as a reusable semiring and optimization foundation.

## Tasks
- [ ] Design additive semiring trait layer
  - [ ] `Semiring`
  - [ ] `IdempotentSemiring`
  - [ ] tropical convention abstraction (`MaxPlus`, `MinPlus`)
- [ ] Extend graph/matrix infrastructure
  - [ ] sparse tropical matrix support
  - [ ] graph/path APIs
  - [ ] fixed-point/dataflow helpers
- [ ] Add compiler/scheduling-oriented utilities
  - [ ] path scoring
  - [ ] schedule cost propagation
  - [ ] dependence-distance accumulation
  - [ ] profitability scoring for transformations
- [ ] Add GPU-kernel-oriented utilities
  - [ ] launch/tile/block score models
  - [ ] memory penalty models
  - [ ] occupancy-inspired scoring helpers
- [ ] Add examples and benchmarks for these new uses

---

# 2. amari-dual

## Main direction
Extend `amari-dual` from strong forward-mode AD into richer optimization and analysis workflows.

## Tasks
- [ ] Add higher-order differentiation support
  - [ ] nested dual evaluation or explicit second-order dual types
  - [ ] Hessian or Hessian-vector APIs
- [ ] Add structured differentiation helpers
  - [ ] directional derivatives
  - [ ] Jacobian-vector products
  - [ ] vector-Jacobian products
  - [ ] batched Jacobian helpers
- [ ] Improve low-allocation / hot-path ergonomics
  - [ ] const-generic small-gradient paths where useful
  - [ ] reduced allocation in common operations
- [ ] Add examples tied to optimization and tuning
  - [ ] differentiable cost model demo
  - [ ] autotuning or parameter-sensitivity demo

---

# 3. amari-fusion

## Main direction
Keep `amari-fusion` general-purpose, preserving its current LLM/attention/holographic strengths while extending it for broader optimization workflows.

## Tasks
- [ ] Preserve current public framing and use cases
- [ ] Add compiler/kernel/scheduling-oriented workflows alongside existing ones
- [ ] Extend APIs where useful for:
  - [ ] plan/schedule representations
  - [ ] richer similarity/evaluation of optimization states
  - [ ] sensitivity analysis
  - [ ] multi-objective scoring helpers
- [ ] Add examples
  - [ ] kernel plan comparison
  - [ ] schedule search/interpolation
  - [ ] symbolic retrieval of known-good optimization patterns
- [ ] Add benchmarks
  - [ ] evaluation throughput
  - [ ] similarity search
  - [ ] holographic bind/unbind
  - [ ] optimization-search workloads

---

# 4. Cross-crate integration

- [ ] Ensure new APIs remain additive and backward compatible
- [ ] Add integration examples across tropical/dual/fusion where useful
- [ ] Decide which new APIs should later be surfaced through `amari-wasm`
- [ ] Decide which new APIs should later be accelerated through `amari-gpu`

---

## 0.21.0 Exit Criteria

- [ ] `amari-tropical` has additive semiring/compiler-oriented extensions
- [ ] `amari-dual` has additive higher-order or structured differentiation extensions
- [ ] `amari-fusion` has additive broader optimization workflows without losing existing framing
- [ ] new examples and benchmarks demonstrate practical value
