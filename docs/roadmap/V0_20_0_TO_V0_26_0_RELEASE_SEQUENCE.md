# Amari 0.20.0 → 0.26.0 Release Sequence

- Original date: 2026-04-30
- Revised: 2026-07-24
- Current baseline: 0.23.0 shipped; 0.24.0 is the next release

## Release posture

Each minor release has one coherent center of gravity. Patch releases remain
bug-fix only. A later feature must not hold completed, independently releasable
work indefinitely.

The sequence is now:

1. 0.24.0 — ship completed discovery and holographic superposition;
2. 0.25.0 — comprehensive rewrite research, including first-class inverse
   rewriting;
3. 0.26.0 — GPU/current-`wgpu`/Borsalino modernization.

Explicitly sequenced later minor milestones move one version later unless a
future decision record says otherwise.

## 0.20.0 — `amari-gpu` stabilization baseline (shipped)

Theme: correctness-first GPU stabilization.

Primary outcomes:

- known-good conservative GPU baseline;
- public GPU/CPU parity and hardware validation;
- honest benchmark/crossover documentation;
- explicit GPU-backed, GPU-recommended, CPU-preferred, fallback, and
  infrastructure classifications.

The current-`wgpu` migration and broad new GPU surfaces were intentionally
excluded.

## 0.21.0 — tropical and dual extension (shipped)

Theme: additive algebraic expansion.

Primary outcomes:

- substantial `amari-tropical` and `amari-dual` extensions;
- semiring, compiler/scheduling, higher-order/batched AD, and practical example
  coverage;
- no broad GPU redesign.

## 0.22.0 — CGT and surreal foundations (shipped)

Theme: combinatorial game theory and surreal-number foundations.

Primary outcomes:

- `amari-cgt`;
- `amari-surreal`;
- tested public APIs, examples, and integration points;
- no GPU release blocker.

## 0.23.0 — surcomplex and rewrite foundations (shipped)

Theme: exact rational/surcomplex arithmetic and stable rewrite foundations.

What shipped in PR #155:

- `RationalSurreal` exact rational scalar layer;
- experimental epsilon rational functions;
- `RationalSurcomplex` in new `amari-surcomplex`;
- stable `amari-rewrite` ARS, TRS, bounded predecessor search,
  anti-unification, and positive-example inference;
- experimental rewrite neural/SMT/network scaffolds;
- WASM bindings and examples-suite coverage.

## 0.24.0 — agentic discovery and holographic superposition

Theme: ship the completed discovery runtime and the additive holographic
operation it requires.

Primary outcomes:

- publish `amari-discovery` as sole owner of the installed `amari` command;
- generated structural catalog plus curated semantic capabilities;
- bounded read-only Rust/Cargo and npm TypeScript inspection;
- deterministic recall, graph expansion, Pareto ranking, replayable plans, and
  process-isolated registered probes;
- shared human/JSON/NDJSON/shell contracts and provider-neutral AI validation
  boundary;
- canonical `BindingAlgebra::superpose` and `scale`, including the FHRR
  correctness override.

Verified implementation status:

- superposition/scaling merged in PR #189;
- discovery implementation and hardening merged through PR #214;
- release-readiness documentation, functional budgets, and packaging evidence
  merged in PR #215;
- `amari-discovery` solely owns `amari` and publication-order verification
  places it after all direct Amari dependencies.

Remaining 0.24 gates are aggregate release mechanics only:

- synchronize workspace/internal constraints to 0.24.0;
- regenerate Rust and WASM catalogs at 0.24.0;
- run complete source verification;
- package/publish dependencies in verified order;
- install the verified archive and repeat installation from crates.io;
- complete npm/WASM publication evidence and tag the release.

Non-goals:

- no research-heavy rewrite expansion;
- no inverse-rewrite expansion;
- no GPU/current-`wgpu`/Borsalino work.

## 0.25.0 — comprehensive rewrite and inverse-rewrite research

Theme: turn the stable 0.23 rewrite foundation into a bounded research platform,
with inverse rewriting as a first-class concern.

The full approved scope includes:

- stable proc macros: `derive(Rewritable)`, checked `term!`, checked relational
  and rule syntax;
- first-order unification, critical pairs, bounded joinability,
  left-linearity-aware confluence evidence, LPO termination certificates, and
  bounded Knuth–Bendix completion;
- constrained relational inverse semantics with existential holes,
  substitutions, normalized constraints, and provenance;
- automatic typed residuals for exact single-step round trips;
- bounded backward and bidirectional reasoning over symbolic states;
- bottom-up finite tree automata plus fully supported regular tree grammars;
- exact certified language preimages and explicit lower/upper approximations
  outside certified closure fragments;
- negative-example specialization and inverse-rule synthesis;
- exact Candle 0.11 CPU training/inference, geometric-network guidance, and
  deterministic holographic recall guidance;
- explicit approximate heuristic-pruning mode that never claims exhaustive
  authority;
- vendored in-process Z3 equivalence, constraint, and selected round-trip
  validation;
- generated/semantic discovery for every public surface and rich bounded
  process-isolated symbolic inverse probes.

Release gates include a literature-backed closure-theorem spike before exact
regular-preimage APIs stabilize. Candle/Z3/holographic guidance never creates
rewrite transitions; the symbolic relation remains authoritative.

Non-goals:

- no GPU training backend;
- no external solver executable or build-time downloaded solver binary;
- no arbitrary project/provider/shell/network execution;
- no GPU/current-`wgpu`/Borsalino modernization.

See:

- `docs/plans/2026-07-24-amari-rewrite-0.25-decisions.md`;
- `docs/plans/2026-07-24-amari-rewrite-inverse-expansion-design.md`;
- `docs/plans/2026-07-24-amari-rewrite-inverse-expansion-implementation-plan.md`.

## 0.26.0 — GPU, current `wgpu`, and Borsalino

Theme: GPU revisit after discovery and comprehensive rewrite APIs stabilize.

Primary outcomes:

- integrate Borsalino where it provides measured GPU value;
- migrate `wgpu` 0.19 to the current supported release as a dedicated effort;
- revisit GPU coverage for the 0.21–0.25 mathematical/rewrite surfaces;
- add missing CPU baselines and release-mode/criterion benchmarks;
- implement hardware-aware calibrated dispatch;
- optimize only kernels with measured crossover upside;
- rerun GB10 and RTX 5080 validation before claiming completion.

Tracked issues move to this cycle:

- #137 — CPU baseline timings;
- #138 — release-mode/Criterion benchmarks;
- #139 — hardware-aware calibrated dispatch;
- #140 — measured high-upside kernel optimization;
- #141 — coverage for new crates/extensions;
- #142 — current-`wgpu` migration plan and execution.

Do not combine the backend migration with unrelated minor-release work.

## Summary

| Version | Theme | Primary work | GPU posture |
| --- | --- | --- | --- |
| 0.20.0 | GPU stabilization | conservative `amari-gpu` baseline | Known-good baseline |
| 0.21.0 | Algebra extension | `amari-tropical`, `amari-dual` | Broad follow-up deferred |
| 0.22.0 | Mathematical foundations | `amari-cgt`, `amari-surreal` | No GPU blocker |
| 0.23.0 | Exact scalars + rewrite foundation | `amari-surcomplex`, stable rewrite core | No GPU blocker |
| 0.24.0 | Discovery + holographic operation | `amari-discovery`, `superpose`, `scale` | No GPU blocker |
| 0.25.0 | Rewrite/inverse research | symbolic, language, learned, SMT, discovery | CPU research features only |
| 0.26.0 | GPU modernization | Borsalino, current `wgpu`, dispatch/benchmarks | Full GPU cycle |
