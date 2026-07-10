# Amari 0.20.0 → 0.25.0 Release Sequence

Date: 2026-04-30, revised 2026-07-09
Current planning baseline: `0.23.0` shipped (`amari-surcomplex`, `amari-rewrite`, rational surreal, and epsilon). `0.24.0` is next.

## Release posture

The next releases should preserve the separation between stabilization, algebraic expansion, new crate introductions, rewrite-system foundations, tooling, and GPU follow-up work.

The `amari-gpu` follow-up issues raised after the 0.20.0 hardening pass are deferred to the `0.25.0` cycle, where Borsalino integration, the `wgpu` version bump, and accumulated new crate surface create the right context for GPU tooling, calibration, coverage review, and backend migration work.

Patch releases between these milestones should remain bug-fix only.

## 0.20.0 — `amari-gpu` stabilization baseline

Theme: correctness-first GPU stabilization.

Primary outcome:

- land the `amari-gpu` hardening PR as the known-good GPU baseline
- restore/narrow public GPU APIs where appropriate
- validate public GPU surfaces against CPU baselines
- document GB10 and RTX 5080 hardware validation
- document benchmark/crossover posture honestly
- distinguish GPU-backed, GPU-recommended, CPU-preferred, fallback, and infrastructure paths

Non-goals:

- do not require every GPU-backed path to beat CPU
- do not include the `wgpu 0.19 -> 29` migration
- do not add new broad GPU surfaces without CPU-baseline tests

Patch lane after 0.20.0:

- reserve `0.20.x` for packaging fixes, serious correctness bugs, or documentation corrections
- do not treat benchmark refinement or backend migration as mandatory `0.20.1` work

## 0.21.0 — `amari-tropical` and `amari-dual` extension release

Theme: additive algebraic expansion.

Primary outcome:

- considerably extend `amari-tropical`
- considerably extend `amari-dual`
- preserve existing crate identities and downstream compatibility
- focus on semiring abstractions, compiler/scheduling use cases, higher-order/batched AD, and practical examples

Secondary / optional:

- update `amari-fusion` examples only where needed to consume the new tropical/dual capabilities
- defer GPU integration of the new APIs unless it is small, obvious, and already covered by CPU-baseline tests

Non-goals:

- do not reopen the broad `amari-gpu` redesign during 0.21.0
- do not fold the `wgpu 29` migration into 0.21.0 unless it becomes unavoidable for compatibility

## 0.22.0 — `amari-cgt` and `amari-surreal`

Theme: combinatorial game theory and surreal-number foundations.

Primary outcome:

- introduce `amari-cgt` for combinatorial game theory
- introduce `amari-surreal` for surreal numbers
- define their public APIs, tests, documentation, and examples
- establish integration points with existing algebraic crates where appropriate

Non-goals:

- do not require immediate GPU acceleration for `amari-cgt` or `amari-surreal`
- do not block these crates on `amari-gpu` follow-up work

## 0.23.0 — `amari-surcomplex` and `amari-rewrite` (shipped)

Theme: surcomplex numbers, rational surreal arithmetic, and rewrite-system foundations.

What shipped (PR #155):

- stable `RationalSurreal` exact rational scalar layer in `amari-surreal`
- experimental epsilon rational functions behind feature `experimental-epsilon`
- new `amari-surcomplex` crate with `RationalSurcomplex` over `RationalSurreal`
- new `amari-rewrite` crate: stable core (ARS, TRS, inverse search, synthesis/anti-unification) plus experimental `neural`, `smt`, and `network` scaffolding
- workspace version bump to 0.23.0
- WASM bindings (`WasmRationalSurreal`, `WasmRationalSurcomplex`, `WasmExperimentalEpsilonRational`)
- examples-suite `/surcomplex` page and API reference
- v0.23 roadmap/checklist docs

## 0.24.0 — `amari-rewrite` expansion, `amari-discovery`, and `BindingAlgebra::superpose`

Theme: rewrite-system completion, agentic mathematical discovery, and holographic trait fix.

Primary outcome:

- **`amari-rewrite` deferred features** (shipped in 0.23.0 with stable core; these were left experimental or unimplemented):
  - `macros` feature: `derive(Rewritable)`, `term!`, `rule!` proc-macro helpers
  - `neural` feature: flesh out `DifferentiableRule<State>` with `candle` as the tensor dependency
  - `smt` feature: integrate a concrete solver behind the `RewriteSolver` trait
  - `network` feature: expand beyond `RewriteGraphSummary` — geometric/learned strategy selection via `amari-network`
  - confluence / termination analysis scaffolding
  - `infer_rules` with negative-example filtering and heuristic specialization (currently only `infer_rule` with positive examples)
- **`amari-discovery`**: publish an agent-first discovery runtime with the installed `amari` command. It combines a generated API index and semantic capability catalog, read-only Rust/TypeScript project inspection, an Amari-native recommendation/planning engine, and bounded real probes. Human-readable output and a versioned JSON/NDJSON protocol share one typed core. See `docs/plans/2026-07-09-amari-discovery-design.md`
- **`BindingAlgebra::superpose` + `scale`** (PR #176): additive superposition trait on `amari-holographic`, default implementations via existing trait methods, non-breaking. Unblocks Minuet `DenseTrace` recall-decay bug fix and supports correct holographic candidate accumulation in `amari-discovery`

Non-goals:

- do not include GPU follow-up work in 0.24.0
- do not include `wgpu` migration or Borsalino integration

## 0.25.0 — `amari-gpu` follow-up and Borsalino integration

Theme: GPU revisit, Borsalino integration, and wgpu modernization.

Primary outcome:

- integrate Borsalino (Borsalino) for GPU acceleration
- bump `wgpu` from 0.19 to current
- revisit `amari-gpu` in light of the 0.21.0, 0.22.0, 0.23.0, and 0.24.0 crates
- decide which new operations are practical GPU candidates

GPU follow-up issues planned for this cycle:

- #137 — Add missing CPU baseline timings to `amari-gpu` benchmark harnesses
- #138 — Add release-mode or Criterion benchmarks for `amari-gpu`
- #139 — Implement hardware-aware calibrated dispatch for `amari-gpu`
- #140 — Optimize high-upside `amari-gpu` kernels identified by crossover data
- #141 — Revisit `amari-gpu` coverage for upcoming crates and extensions
- #142 — Plan dedicated migration from `wgpu 0.19` to `wgpu 29`

`wgpu` migration note:

- track it during 0.25.0 as a dedicated migration effort
- do not combine it with unrelated release work
- rerun GB10 and RTX 5080 validation before claiming the migration complete

## Summary table

| Version | Theme | Primary crates/work | GPU posture |
|---------|-------|---------------------|-------------|
| 0.20.0 | GPU stabilization | `amari-gpu` hardening, validation, benchmark docs | Establish known-good conservative baseline |
| 0.21.0 | Algebra extension | `amari-tropical`, `amari-dual` | Defer broad GPU follow-up |
| 0.22.0 | New mathematical foundations | `amari-cgt`, `amari-surreal` | No GPU blocker |
| 0.23.0 | Surcomplex + rewrite systems | `amari-surcomplex`, `amari-rewrite`, `RationalSurreal`, experimental epsilon | No GPU blocker |
| 0.24.0 | Rewrite completion + agentic discovery + holographic fix | `amari-rewrite` expansion (macros, candle, SMT, network, confluence, negative-example inference), `amari-discovery` (`amari` command), `BindingAlgebra::superpose` (PR #176) | No GPU blocker |
| 0.25.0 | GPU revisit + Borsalino | `amari-gpu` follow-up, Borsalino integration, `wgpu` bump | Full GPU modernization cycle |
