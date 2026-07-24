# amari-rewrite 0.24.0 — Decision Record

- Date opened: 2026-07-09
- Date resolved: 2026-07-23
- Status: **Approved — research-heavy, bounded expansion**
- Authoritative design: `2026-07-23-amari-rewrite-research-expansion-design.md`
- Implementation plan: `2026-07-23-amari-rewrite-research-expansion-implementation-plan.md`

## Context

`amari-rewrite` shipped in 0.23.0 with stable ARS, TRS, inverse search,
anti-unification, and positive-example inference, plus experimental trait-only
neural/SMT and summary-only network scaffolds. The original 0.24 decision list
left dependency, API, algorithm, and stability choices unresolved.

The approved profile intentionally chooses the deeper research implementation,
but every operation remains bounded and every pre-existing stable API remains
additive.

## Decision 1: proc-macro architecture

Create a separate publishable `amari-rewrite-macros` proc-macro crate. It owns:

- `#[derive(Rewritable)]` with explicit `#[rewritable(child)]` fields;
- `term!(...)` first-order term syntax;
- `rule!(lhs => rhs)` checked rule syntax.

`amari-rewrite` re-exports all three behind `macros`. The feature is stable in
0.24. Macro expansion resolves renamed crates hygienically and receives
trybuild pass/fail coverage. The macro crate publishes before `amari-rewrite`.

## Decision 2: concrete SMT backend

Use exact `z3` `=0.20.2` inside `amari-rewrite` behind experimental feature
`smt`. Enable vendored source builds, not an external executable or build-time
GitHub-release binary download. Keep `RewriteSolver`; add a bounded
`Z3RewriteSolver` for first-order equivalence under quantified rewrite axioms.

Results distinguish proved-equivalent, refuted, and unknown. Quantifier
unknown/timeout is never promoted to proof. Raw Z3 diagnostics, paths, and
unbounded model text are not public evidence.

Current Z3 requires Rust 1.85, so the workspace MSRV rises to 1.85 as an
explicit release-wide change.

## Decision 3: Candle model and training

Use exact `candle-core` and `candle-nn` `=0.11.0` behind experimental feature
`neural`. Implement:

- deterministic structural term encoding;
- a concrete CPU MLP rewrite ranker;
- pairwise training-data derivation from real rewrite traces;
- bounded AdamW training and typed partial reports;
- safe-tensor checkpoint validation;
- neural-guided successor selection.

Training infrastructure belongs in `amari-rewrite`; arbitrary project data
loading and GPU/CUDA/Metal backends do not. Existing `DifferentiableRule`
remains available.

## Decision 4: geometric and hybrid network guidance

Feature `network` continues to depend on `amari-network` and `neural`. Implement
all three previously considered layers:

1. a bounded rewrite search graph using `GeometricNetwork<3,0,0>`;
2. deterministic network-guided frontier ranking;
3. hybrid network/Candle scoring and trace-derived training examples.

Nodes are terms, edges are actual rule applications with provenance, and
partial graphs preserve bounded frontier evidence. No implicit model or global
mutable graph exists.

## Decision 5: confluence, termination, and completion

Implement first-order unification with occurs check, critical-pair generation,
bounded joinability/local-confluence reporting, and lexicographic path ordering
(LPO). Add bounded Knuth–Bendix completion behind feature `completion`.

LPO success is a sound termination certificate. Failure to orient is unknown,
not proof of non-termination. Ordinary critical-pair local-confluence
certification requires left-linear rules; non-left-linear systems remain
unknown unless parallel critical pairs are added. Bounded unresolved pairs are
partial/unknown, not proof of non-confluence. Completion has fixed
rule/pair/iteration/operation ceilings and typed complete/partial/failed
outcomes.

## Decision 6: negative-example specialization

Keep existing `infer_rule` and `infer_rules`. Add a bounded deterministic
`RuleRefiner` that detects exact negative coverage, selects discriminating
paths, partitions positives, infers specialized rules, and validates all
returned rules against supplied examples.

The result is refined, inconclusive, or limit-reached. It is heuristic, not a
complete learner. With `smt`, optional counterexample evidence can refine or
reject candidates; unknown solver outcomes preserve symbolic evidence.

## Decision 7: stability, discovery, and release scope

- Existing 0.23 symbolic APIs remain stable.
- Macros are stable in 0.24.
- Completion, neural, SMT, network, and negative-example refinement are
  explicitly experimental.
- Default builds stay lightweight and do not activate macros, Candle, Z3, or
  `amari-network`.
- No new WASM bindings are required in 0.24.
- Every public API receives generated structural and curated semantic discovery
  records in its implementation cohort.
- Process-isolated discovery probes are limited to bounded pure symbolic
  analysis/completion/refinement. Candle training and Z3 solving are
  discoverable but not executable through discovery.
- The work ships as multiple moderate PRs with per-task RED→GREEN commits,
  independent review, and explicit default/no-default/feature/all-feature
  matrices.
- Aggregate 0.24.0 acceptance remains blocked until all expansion cohorts merge
  and discovery Task 31 completes versioning, catalog, packaging, publication,
  and registry-install gates.

## Rejected alternatives

- Keeping Candle and SMT trait-only: inconsistent with the selected
  research-heavy profile.
- Deferring concrete neural/SMT work to 0.25: rejected for the same reason.
- Latest Z3 via `gh-release`: rejected because it downloads a native binary at
  build time.
- Runtime external `z3` process: rejected because it adds undeclared executable
  and shell/process authority.
- GPU neural training in 0.24: rejected to preserve the separate 0.25 GPU,
  Borsalino, and `wgpu` modernization track.
- Unbounded completion, global confluence claims, or “failed LPO means
  non-terminating”: rejected as mathematically unsound.
