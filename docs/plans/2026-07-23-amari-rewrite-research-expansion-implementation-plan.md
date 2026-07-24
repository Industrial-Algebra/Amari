# amari-rewrite 0.24 Research Expansion Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Deliver the approved research-heavy `amari-rewrite` expansion—macros, symbolic analysis/completion, negative-example refinement, Candle training, in-process Z3 validation, geometric/hybrid search, and truthful `amari-discovery` integration—without breaking the stable 0.23 core.

**Architecture:** Keep default `amari-rewrite` symbolic and lightweight. Add a proc-macro companion crate, bounded alloc-friendly analysis, and opt-in completion/Candle/Z3/network layers with typed partial outcomes and hard ceilings. Every public cohort updates the generated structural catalog plus curated discovery semantics; only safe bounded symbolic functions receive discovery probes.

**Tech Stack:** Rust 1.85+, `syn`/`quote`/`proc-macro-crate`, `candle-core` 0.11.0, `candle-nn` 0.11.0, vendored `z3` 0.20.2, `amari-network`, `serde`, `sha2`, `trybuild`, existing `amari-discovery` catalog/probe infrastructure.

---

## Execution rules

1. Work from `develop` in a fresh worktree per PR cohort.
2. Use strict RED → GREEN → refactor. Every canonical task below gets its own commit even when tasks share one PR.
3. New Rust/Python files start with `SPDX-License-Identifier: MIT OR Apache-2.0`.
4. Existing 0.23 public behavior is additive-only. Do not rename/remove stable APIs.
5. Validate configuration before allocation or recursion. Callers may tighten limits but never exceed fixed ceilings.
6. Preserve `no_std + alloc` for default symbolic code. `neural`, `smt`, and `network` imply `std`.
7. Do not use `unwrap`, `expect`, wildcard enum matches, raw solver diagnostics, external solver executables, or implicit network/file/project authority in library code.
8. Run default, no-default, individual-feature, and all-feature checks sequentially. Vendored Z3 builds are expensive; do not run duplicate all-feature matrices without a reason.
9. Update discovery in the same cohort as each public API. Generated structural records alone are insufficient: add semantic search/detail/graph tests.
10. Independent review is mandatory per grouped PR. Critical/Important findings block merge.
11. Whole-workspace hooks may still encounter unrelated `amari-core/src/generic.rs` warnings; use scoped warning-denied Clippy and documented `--no-verify` only when necessary.
12. Do not call feature completion a 0.24.0 release. Aggregate discovery Task 31 remains mandatory after all cohorts merge.

## Fixed authority ceilings

Implement constants, not caller-configurable maxima:

| Area | Hard ceilings |
| --- | --- |
| Analysis | 4,096 nodes/term, depth 64, 4,096 critical pairs, 65,536 joinability nodes, 1,000,000 operations |
| Completion | 256 rules, 4,096 pending pairs, 4,096 iterations, 1,000,000 operations |
| Neural | feature width 64, hidden width 256, 65,536 examples, 10,000 epochs, 16,777,216 tensor elements, 64 MiB checkpoint, 5 minute training deadline |
| SMT | 256 rules, 512 symbols, 512 variables, 65,536 term nodes, 4,096 assertions, 30 second timeout |
| Network | 4,096 terms, 65,536 edges, depth 64, 1,000,000 operations |
| Refinement | 4,096 examples, 256 partitions/candidates/rules, 1,000,000 operations |

Discovery probe limits must be substantially tighter than library maxima.

---

## Cohort 1 — Toolchain, dependency, and macro foundation

### Task 1: Raise and verify the research dependency baseline

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `README.md`
- Modify: `examples/README.md`
- Modify: `docs/claude-code/CONSOLIDATED_CONTEXT.md`
- Modify: `.github/workflows/ci.yml`
- Modify: `amari-rewrite/Cargo.toml`
- Create: `amari-rewrite/tests/research_dependencies.rs`

**Steps:**

1. RED: add feature-gated compile tests that construct a CPU `candle_core::Tensor`, a `candle_nn::VarMap`, and a vendored `z3::Solver`; run `cargo test -p amari-rewrite --all-features --test research_dependencies` and record missing crates/features.
2. Set workspace `rust-version = "1.85"`; update active badges/prerequisites; add exact workspace dependencies `candle-core = "=0.11.0"`, `candle-nn = "=0.11.0"`, `z3 = { version = "=0.20.2", default-features = false, features = ["vendored"] }`, and optional `sha2` wiring for neural/SMT evidence.
3. Wire optional rewrite features exactly as approved. `neural`, `smt`, and `network` imply `std`; default remains `std` only. Verify the exact Candle releases compile on Rust 1.85 because their package metadata declares no MSRV.
4. Add a separate `MSRV Check (1.85)` job running workspace default checks plus rewrite all-features; do not add 1.85 to the existing stable/nightly matrix. Add a separate 45-minute vendored-Z3 rewrite job that caches the complete Cargo `target` tree (including `z3-sys` CMake output) with compiler/lockfile keys. Preserve existing aggregate names and do not add Z3 to WASM targets.
5. GREEN: run the focused test, `cargo +1.85.0 check --workspace`, `cargo +1.85.0 check -p amari-rewrite --all-features`, default/no-default checks, and stable all-feature check. Record cold/warm Z3 build times and adjust the dedicated timeout only from evidence.
6. Commit `build: add rewrite research dependencies`.

### Task 2: Scaffold and publish-wire `amari-rewrite-macros`

**Files:**
- Create: `amari-rewrite-macros/Cargo.toml`
- Create: `amari-rewrite-macros/src/lib.rs`
- Modify: root `Cargo.toml`
- Modify: `amari-rewrite/Cargo.toml`
- Modify: `amari-rewrite/src/lib.rs`
- Modify: `.github/workflows/publish.yml`
- Test: `amari-rewrite/tests/macros_reexport.rs`

**Steps:**

1. RED: test that `amari_rewrite::{Rewritable, term, rule}` imports compile with `--features macros`; verify failure.
2. Add `proc-macro-crate = "3"` to workspace dependencies; create the proc-macro crate with workspace metadata and workspace `syn`, `quote`, `proc-macro2`, `proc-macro-crate`; add it before `amari-rewrite` in workspace/publish order.
3. Add placeholder proc macro entry points returning compile errors with stable “not implemented” diagnostics; wire optional dependency and re-exports.
4. GREEN only the re-export/feature wiring test; verify default/no-default builds do not compile the macro dependency.
5. Run workflow coverage, binary ownership, and publish-order scripts.
6. Commit `feat: scaffold rewrite macro crate`.

### Task 3: Implement `derive(Rewritable)`

**Files:**
- Create: `amari-rewrite-macros/src/derive_rewritable.rs`
- Modify: `amari-rewrite-macros/src/lib.rs`
- Create: `amari-rewrite/tests/ui/derive-pass.rs`
- Create: `amari-rewrite/tests/ui/derive-renamed-pass.rs`
- Create: `amari-rewrite/tests/ui/derive-union-fail.rs`
- Create: `amari-rewrite/tests/ui/derive-child-type-fail.rs`
- Create: `amari-rewrite/tests/ui/derive-duplicate-attribute-fail.rs`
- Create: `amari-rewrite/tests/ui/*.stderr`
- Create: `amari-rewrite/tests/macros_derive.rs`
- Modify: `amari-rewrite/Cargo.toml` (trybuild dev dependency)

**Public contract:**

```rust
#[derive(Clone, Debug, PartialEq, amari_rewrite::Rewritable)]
enum Expr {
    Constant(i64),
    Add(
        #[rewritable(child)] Box<Expr>,
        #[rewritable(child)] Box<Expr>,
    ),
}
```

**Steps:**

1. Add `trybuild = "1"` to `amari-rewrite` dev-dependencies. RED runtime tests for preorder children/positions, valid replacement, payload preservation, and invalid index; RED trybuild pass/fail fixtures including renamed crate resolution.
2. Parse explicit child attributes; validate enum/struct/field shapes before generation.
3. Generate exhaustive `child_count`, `child`, and `replace_child` matches with no wildcard variant arm and typed invalid-index errors.
4. Use `proc-macro-crate` for hygienic paths; reject unions and unsupported containers with span-local diagnostics.
5. GREEN runtime/UI suites; run macro crate Clippy/rustdoc and no-macro default rewrite tests.
6. Commit `feat: derive rewritable structures`.

### Task 4: Implement checked `term!` and `rule!`

**Files:**
- Create: `amari-rewrite-macros/src/term.rs`
- Create: `amari-rewrite-macros/src/rule.rs`
- Modify: `amari-rewrite-macros/src/lib.rs`
- Create: `amari-rewrite/tests/macros_terms.rs`
- Create: `amari-rewrite/tests/ui/term-invalid-fail.rs`
- Create: `amari-rewrite/tests/ui/rule-invalid-fail.rs`

**Public contract:**

```rust
let term = term!(add(zero, X));
let checked = rule!(add(zero, X) => X); // RewriteResult<Rule>
```

**Steps:**

1. RED exact construction tests for constants, variables, nested applications, `term!(f) == term!("f")`, checked RHS variables, renamed-crate hygiene, and prelude imports where ARS `Rule` and `TrsRule` coexist; RED malformed grammar UI tests.
2. Implement the approved grammar with canonical generated `Term` constructors; strip string literal quotes from symbol content.
3. Expand `rule!` through the hygienically resolved fully qualified `amari_rewrite::trs::Rule::new(term!(lhs), term!(rhs))`; never resolve the ARS `Rule`, emit `new_unchecked`, or call `expect`.
4. GREEN focused runtime/UI tests and rustdoc examples.
5. Commit `feat: add checked rewrite syntax macros`.

### Task 5: Make macro capabilities discoverable

**Files:**
- Regenerate: `amari-discovery/catalog/generated.json`
- Modify: `amari-discovery/catalog/semantic/core.toml`
- Modify: `amari-discovery/tests/catalog_integrity.rs`
- Create: `amari-discovery/tests/rewrite_discovery_macros.rs`
- Modify: `scripts/run-discovery-test-shard.py`

**Steps:**

1. RED catalog tests for the new package, derive/function-like macro records, feature gate, semantic capability, and `amari discover search/detail/graph` resolution.
2. Add semantic capability `amari:amari-rewrite:macros:syntax` with macro symbol refs and relations to TRS normalization/inference.
3. Regenerate the catalog from the fixed workspace path; verify `amari-discovery` remains excluded and package count increases by one.
4. Assign the new flat integration target exactly once and run both sharding verifiers.
5. GREEN catalog/CLI tests, generator drift check, and binary ownership/publish order.
6. Independent review and full cohort verification.
7. Commit `feat: discover rewrite syntax macros`; open grouped PR 1.

---

## Cohort 2 — First-order analysis and completion

### Task 6: Add shared analysis limits and term metrics

**Files:**
- Create: `amari-rewrite/src/analysis/mod.rs`
- Create: `amari-rewrite/src/analysis/limits.rs`
- Create: `amari-rewrite/src/analysis/metrics.rs`
- Modify: `amari-rewrite/src/lib.rs`
- Modify: `amari-rewrite/src/error.rs`
- Create: `amari-rewrite/tests/analysis_limits.rs`

**Steps:**

1. RED zero/oversized configuration deserialization/construction, term node/depth accounting, and operation counter tests.
2. Implement `AnalysisLimits` with associated hard-ceiling constants and `validate`; implement iterative/pre-allocation metrics.
3. Add typed `InvalidConfiguration` and categorized `LimitExceeded` errors.
4. GREEN default and `--no-default-features` focused tests; document all public fields.
5. Commit `feat: bound rewrite analysis resources`.

### Task 7: Implement occurs-check first-order unification

**Files:**
- Create: `amari-rewrite/src/analysis/unify.rs`
- Modify: `amari-rewrite/src/analysis/mod.rs`
- Modify: `amari-rewrite/src/trs/substitution.rs`
- Create: `amari-rewrite/tests/unification.rs`

**Steps:**

1. RED tests for variable/symbol unification, repeated variables, arity mismatch, occurs check, substitution composition, symmetry, idempotent application, deterministic binding order, and limits.
2. Add checked substitution composition and variable-safe application helpers.
3. Implement a deterministic equation-worklist unifier returning `UnificationOutcome::{Unified, NotUnifiable}` plus resource observations.
4. Property-test that applying the MGU makes both inputs equal and does not exceed limits.
5. GREEN no-default/all-feature focused tests and Clippy.
6. Commit `feat: unify first-order rewrite terms`.

### Task 8: Generate deterministic critical pairs

**Files:**
- Create: `amari-rewrite/src/analysis/critical_pairs.rs`
- Modify: `amari-rewrite/src/analysis/mod.rs`
- Create: `amari-rewrite/tests/critical_pairs.rs`

**Steps:**

1. RED textbook overlaps, self-overlaps, renamed-apart variables, root/non-root provenance, variable-position exclusion, trivial pairs, sort order, and ceiling tests.
2. Implement collision-free rule-variable namespaces, a shared left-linearity/variable-position helper, and proper non-variable overlap enumeration.
3. Apply unification/substitution to construct `CriticalPair { left, right, first_rule, second_rule, position, trivial }`.
4. Deduplicate only identical pair+provenance records; preserve deterministic order.
5. GREEN focused/property tests and docs.
6. Commit `feat: compute rewrite critical pairs`.

### Task 9: Add bounded joinability and local-confluence reports

**Files:**
- Create: `amari-rewrite/src/analysis/confluence.rs`
- Modify: `amari-rewrite/src/analysis/mod.rs`
- Create: `amari-rewrite/tests/confluence.rs`

**Steps:**

1. RED joinable left-linear diamond, non-joinable bounded pair, non-left-linear unknown case, cyclic system, node/operation exhaustion, deterministic witness paths, and mixed report tests.
2. Implement canonical bidirectional bounded search over real `TermSystem::successors` plus explicit left-linearity validation.
3. Add `PairJoinability::{Joinable, NotJoinableWithinBounds, LimitReached}` and `ConfluenceReport` with exhaustive match semantics.
4. Certify local confluence only when all LHS patterns are left-linear, every generated pair is trivial/joinable, and pair generation/search completed; otherwise return typed unknown/partial evidence.
5. GREEN focused tests, no-default checks, rustdoc.
6. Commit `feat: analyze bounded rewrite confluence`.

### Task 10: Add LPO termination certificates

**Files:**
- Create: `amari-rewrite/src/analysis/lpo.rs`
- Modify: `amari-rewrite/src/analysis/mod.rs`
- Create: `amari-rewrite/tests/lpo.rs`

**Steps:**

1. RED precedence validation, subterm property, precedence case, lexicographic case, variable condition, orientable Peano rules, unorientable/cyclic rules, and limits.
2. Implement `LpoPrecedence`, memoized bounded comparison, `RuleOrientation`, and `TerminationReport`.
3. Require a total unique precedence over every symbol used by checked rules.
4. Return `ProvedTerminating` only when every rule strictly decreases; otherwise `Unknown` with typed reasons.
5. GREEN property tests (irreflexivity/transitivity on generated small terms), no-default checks, docs.
6. Commit `feat: certify termination with lpo`.

### Task 11: Implement bounded Knuth–Bendix completion

**Files:**
- Create: `amari-rewrite/src/completion/mod.rs`
- Create: `amari-rewrite/src/completion/config.rs`
- Create: `amari-rewrite/src/completion/trace.rs`
- Modify: `amari-rewrite/src/lib.rs`
- Modify: `amari-rewrite/Cargo.toml`
- Create: `amari-rewrite/tests/completion.rs`

**Steps:**

1. RED equation orientation, simplification, critical-pair addition, duplicate suppression, convergent finite example, unorientable equation, and each hard ceiling.
2. Add `completion` feature and strict `CompletionConfig` below fixed maxima.
3. Implement canonical completion loop using Tasks 8–10 and checked `Rule::new`.
4. Return `CompletionOutcome::{Complete, Partial, Failed}` with bounded exhaustive trace events and resource counts.
5. Re-run the resulting system through confluence/LPO analyzers before `Complete`; require terminating orientation, left-linearity, and sound local-confluence certification, otherwise retain `Partial`/`Unknown`.
6. GREEN focused/all-feature/no-default-with-completion checks and docs.
7. Commit `feat: complete bounded rewrite systems`.

### Task 12: Expose safe analysis and completion probes

**Files:**
- Modify: `amari-discovery/catalog/probes.toml`
- Modify: `amari-discovery/catalog/semantic/core.toml`
- Modify: `amari-discovery/src/probes/rewrite.rs`
- Modify: `amari-discovery/src/probes/registry.rs`
- Regenerate: `amari-discovery/catalog/generated.json`
- Create: `amari-discovery/tests/probe_rewrite_analysis.rs`
- Create: `amari-discovery/tests/rewrite_discovery_analysis.rs`
- Modify: `scripts/run-discovery-test-shard.py`

**Steps:**

1. RED typed probe DTOs/parity for critical pairs+joinability, LPO, and completion; require `#[serde(deny_unknown_fields)]` on every nested DTO and reject malformed terms and every tightened limit.
2. Add semantic capabilities for unification, critical pairs, LPO, and completion with exact symbol/feature refs and relations.
3. Implement process-isolated adapters using shared bounded term/rule transport; set descriptor limits far below library maxima.
4. RED/GREEN CLI search/detail/graph and Rust recommendation tests for all capabilities.
5. Regenerate catalog, update exact descriptor/package counts, and assign both tests exactly once.
6. Independent review and full cohort verification.
7. Commit `feat: discover rewrite analysis`; open grouped PR 2.

---

## Cohort 3 — Negative-example specialization

### Task 13: Define bounded refinement contracts

**Files:**
- Create: `amari-rewrite/src/synthesis/refinement.rs`
- Modify: `amari-rewrite/src/synthesis/mod.rs`
- Modify: `amari-rewrite/src/error.rs`
- Create: `amari-rewrite/tests/rule_refinement.rs`

**Steps:**

1. RED strict `InferenceConfig` zero/maximum checks and typed `InferenceOutcome::{Refined, Inconclusive, LimitReached}` serialization (when `serialize`).
2. Add example/candidate/partition/rule/operation ceilings and resource observations.
3. Preserve existing `infer_rule`/`infer_rules` signatures and behavior.
4. GREEN contract tests under default/no-default/serialize.
5. Commit `feat: define bounded rule refinement`.

### Task 14: Implement deterministic specialization and partitioning

**Files:**
- Modify: `amari-rewrite/src/synthesis/refinement.rs`
- Create: `amari-rewrite/tests/rule_specialization.rs`

**Steps:**

1. RED cases where the general rule covers a negative, a discriminating `succ(_)` specialization succeeds, multiple positive shapes split deterministically, no unconditional rule can separate examples, duplicates, and limits.
2. Implement exact negative coverage detection, canonical discriminating-path selection, positive partitioning, per-partition inference, and validation against all supplied examples.
3. Sort/deduplicate candidates and rules by canonical terms; record rejected candidates without echoing unsafe payload text.
4. Property-test every returned rule covers at least one positive and no supplied negative; every covered positive produces its expected RHS.
5. GREEN focused tests and docs.
6. Commit `feat: specialize rules from negative examples`.

### Task 15: Make refinement discoverable and probeable

**Files:**
- Modify: `amari-discovery/catalog/probes.toml`
- Modify: `amari-discovery/catalog/semantic/core.toml`
- Modify: `amari-discovery/src/probes/rewrite.rs`
- Modify: `amari-discovery/src/probes/registry.rs`
- Regenerate: `amari-discovery/catalog/generated.json`
- Create: `amari-discovery/tests/probe_rewrite_refinement.rs`
- Create: `amari-discovery/tests/rewrite_discovery_refinement.rs`
- Modify: `scripts/run-discovery-test-shard.py`

**Steps:**

1. RED process-isolated probe parity, result-state, deterministic ordering, `#[serde(deny_unknown_fields)]` across nested DTOs, malformed examples, and tightened ceilings.
2. Add `amari:amari-rewrite:synthesis:negative-refinement` and relation from basic inference and to analysis/completion.
3. Implement the bounded adapter; do not expose solver authority.
4. Regenerate/verify catalog and CLI search/detail/graph/recommendation.
5. Independent review and full cohort verification.
6. Commit `feat: discover negative rule refinement`; open grouped PR 3.

---

## Cohort 4 — Candle model, training, and neural strategy

### Task 16: Encode terms and score candidates with Candle

**Files:**
- Replace/expand: `amari-rewrite/src/neural/mod.rs`
- Create: `amari-rewrite/src/neural/encode.rs`
- Create: `amari-rewrite/src/neural/model.rs`
- Create: `amari-rewrite/src/neural/config.rs`
- Modify: `amari-rewrite/src/error.rs`
- Create: `amari-rewrite/tests/neural_model.rs`

**Steps:**

1. RED feature-schema order/hash, deterministic term features, exact tensor shapes, deterministic CPU scores, equal-score ties, non-finite values, and width/element limits.
2. Keep `DifferentiableRule`; add `TermEncoder`, `StructuralTermEncoder`, `FeatureSchema`, and `CandleRewriteRanker` with configurable MLP widths below hard ceilings.
3. Freeze root-symbol hashing and feature normalization; canonicalize candidate order before batching.
4. Map Candle errors and shape/non-finite failures to typed rewrite errors.
5. GREEN neural-only/all-feature tests, Clippy, rustdoc; verify default/no-default remains Candle-free.
6. Commit `feat: score rewrites with candle`.

### Task 17: Generate bounded pairwise training data

**Files:**
- Create: `amari-rewrite/src/neural/data.rs`
- Modify: `amari-rewrite/src/neural/mod.rs`
- Create: `amari-rewrite/tests/neural_training_data.rs`

**Steps:**

1. RED direct trace, branching successor, target-distance labels, duplicate candidates, deterministic negatives, empty/no-positive trace, and ceilings.
2. Add `RewriteTrainingExample` and trace/system adapters producing positive-vs-negative candidate pairs from actual successors.
3. Use explicit seed only for bounded sampling; otherwise canonical deterministic selection.
4. GREEN focused/property tests and docs.
5. Commit `feat: derive rewrite ranking data`.

### Task 18: Add bounded AdamW training

**Files:**
- Create: `amari-rewrite/src/neural/train.rs`
- Modify: `amari-rewrite/src/neural/mod.rs`
- Create: `amari-rewrite/tests/neural_training.rs`

**Steps:**

1. RED pairwise margin loss, loss reduction on a tiny fixed corpus, fixed-seed completed-run determinism, explicitly non-replayable deadline partials, non-finite gradients, and every training limit.
2. Implement `TrainerConfig`, pairwise batches, margin loss, backward pass, gradient checks, AdamW step, and `TrainingReport`.
3. Check wall deadline at deterministic epoch boundaries; return completed/partial typed outcomes with bounded metrics, and mark deadline-truncated reports non-replayable without wall timestamps.
4. Do not add project data loading, GPU selection, or background threads.
5. GREEN focused tests (use coarse deadlines), neural/all-feature Clippy/docs.
6. Commit `feat: train neural rewrite rankers`.

### Task 19: Add safe-tensor checkpoint contracts

**Files:**
- Create: `amari-rewrite/src/neural/checkpoint.rs`
- Modify: `amari-rewrite/src/neural/mod.rs`
- Create: `amari-rewrite/tests/neural_checkpoint.rs`

**Steps:**

1. RED round-trip scores, feature/model metadata, wrong schema/shape, truncated/oversized file, symlink rejection, non-finite tensor, and deterministic parameter map ordering.
2. Add explicit path-based save/load APIs under `std`; validate regular files, byte cap, metadata, and all tensors before model use.
3. Never include absolute paths or raw safetensor diagnostics in public errors.
4. GREEN tempdir tests and checkpoint documentation.
5. Commit `feat: checkpoint neural rewrite models`.

### Task 20: Select real successors with neural guidance

**Files:**
- Create: `amari-rewrite/src/neural/strategy.rs`
- Modify: `amari-rewrite/src/neural/mod.rs`
- Create: `amari-rewrite/tests/neural_strategy.rs`

**Steps:**

1. RED preferred successor, canonical ties, no successors, cyclic bounded normalization, score/resource trace, model error, and operation/node limits.
2. Implement `NeuralGuidedStrategy::choose` and bounded normalization over `TermSystem::successors`.
3. Return score components and selected transition without changing the existing symbolic `Strategy` enum.
4. GREEN parity/determinism tests and docs.
5. Commit `feat: guide rewrites with neural scores`.

### Task 21: Catalog neural training and guidance

**Files:**
- Modify: `amari-discovery/catalog/semantic/core.toml`
- Regenerate: `amari-discovery/catalog/generated.json`
- Create: `amari-discovery/tests/rewrite_discovery_neural.rs`
- Modify: `scripts/run-discovery-test-shard.py`

**Steps:**

1. RED structural cfg/feature records and semantic search/detail/graph tests for encoder, ranker, trainer, checkpoint, and strategy.
2. Add semantic capabilities with `amari-rewrite:neural` feature refs and explicit experimental stability.
3. Do not add an executable discovery probe or enable Candle in `amari-discovery`.
4. Regenerate catalog; assert capability runtime guidance truthfully names required compile feature.
5. Independent review, measure all-feature build time/size, and full cohort verification.
6. Commit `feat: discover neural rewrite training`; open grouped PR 4.

---

## Cohort 5 — Vendored Z3 equivalence and integration

### Task 22: Validate and translate first-order signatures to Z3

**Files:**
- Replace/expand: `amari-rewrite/src/smt/mod.rs`
- Create: `amari-rewrite/src/smt/config.rs`
- Create: `amari-rewrite/src/smt/signature.rs`
- Create: `amari-rewrite/src/smt/translate.rs`
- Modify: `amari-rewrite/src/error.rs`
- Create: `amari-rewrite/tests/smt_translation.rs`

**Steps:**

1. RED conflicting arity, bounded symbol/variable/node counts, deterministic content-addressed names, free variables, nested terms, and sanitized translation failures.
2. Keep `RewriteSolver`; add strict `Z3SolverConfig`, validated `FirstOrderSignature`, and one uninterpreted term sort with function declarations per symbol/arity.
3. Translate terms and substitutions without raw user symbols in backend identifiers.
4. GREEN SMT-only/all-feature tests; verify no external process and vendored Z3 linkage.
5. Commit `feat: translate rewrite terms to z3`.

### Task 23: Prove/refute equivalence under rewrite axioms

**Files:**
- Create: `amari-rewrite/src/smt/solver.rs`
- Create: `amari-rewrite/src/smt/certificate.rs`
- Modify: `amari-rewrite/src/smt/mod.rs`
- Create: `amari-rewrite/tests/smt_equivalence.rs`

**Steps:**

1. RED reflexive proof, proof using a quantified rewrite axiom, satisfiable refutation, quantifier unknown/timeout, malformed rule signature, deterministic hashes, and all ceilings.
2. Encode checked rules as universally quantified equations; assert inequality for the query and set Z3 timeout.
3. Map unsat/sat/unknown exhaustively to `SmtOutcome::{ProvedEquivalent, Refuted, Unknown}`; sanitize bounded model summaries/reason categories.
4. Record canonical query/rule hashes, limits, and Z3 version in certificates.
5. GREEN focused tests repeated in fresh processes for deterministic public evidence.
6. Commit `feat: validate rewrites with z3`.

### Task 24: Integrate solver evidence with rules, completion, and refinement

**Files:**
- Create: `amari-rewrite/src/smt/integration.rs`
- Modify: `amari-rewrite/src/smt/mod.rs`
- Modify: `amari-rewrite/src/completion/mod.rs`
- Modify: `amari-rewrite/src/synthesis/refinement.rs`
- Create: `amari-rewrite/tests/smt_integration.rs`

**Steps:**

1. RED rule validation acceptance/refutation/unknown, completion candidate rejection, refinement counterexample evidence, and no-SMT base parity.
2. Add opt-in integration functions rather than changing existing symbolic defaults.
3. Reject only concrete refutations; preserve unknown evidence and symbolic partial outcomes.
4. Thread shared limits and canonical hashes; never expose raw Z3 diagnostics.
5. GREEN `smt`, `smt+completion`, and all-feature tests.
6. Commit `feat: attach solver evidence to rewriting`.

### Task 25: Make SMT validation discoverable without executable authority

**Files:**
- Modify: `amari-discovery/catalog/semantic/core.toml`
- Regenerate: `amari-discovery/catalog/generated.json`
- Create: `amari-discovery/tests/rewrite_discovery_smt.rs`
- Modify: `scripts/run-discovery-test-shard.py`

**Steps:**

1. RED generated symbol/feature and semantic search/detail/graph/recommendation tests.
2. Add experimental capability `amari:amari-rewrite:smt:equivalence` and relations to rule inference/completion.
3. Assert no probe descriptor/adapter is created and `amari-discovery` does not enable rewrite `smt`.
4. Regenerate/verify catalogs, sharding, publish order, and feature map.
5. Independent review; record vendored Z3 cold/warm CI time and package impact.
6. Commit `feat: discover solver-backed rewriting`; open grouped PR 5.

---

## Cohort 6 — Geometric and hybrid search

### Task 26: Build bounded rewrite search graphs

**Files:**
- Replace/expand: `amari-rewrite/src/network/mod.rs`
- Create: `amari-rewrite/src/network/graph.rs`
- Create: `amari-rewrite/src/network/embed.rs`
- Create: `amari-rewrite/src/network/config.rs`
- Modify: `amari-rewrite/src/error.rs`
- Create: `amari-rewrite/tests/network_graph.rs`

**Steps:**

1. RED direct successor/edge parity, deduplication, rule/position provenance, deterministic structural Cl(3,0,0) embeddings, partial depth/node/edge limits, cycles, and empty systems.
2. Implement canonical BFS `RewriteSearchGraph` backed by `GeometricNetwork<3,0,0>` plus typed term/transition tables.
3. Validate embeddings are finite and graph edge weights nonnegative before insertion.
4. Preserve deterministic frontier/path evidence for partial graphs.
5. GREEN network/all-feature tests and docs; default remains network-free.
6. Commit `feat: model rewrite search geometrically`.

### Task 27: Rank frontiers with network guidance

**Files:**
- Create: `amari-rewrite/src/network/strategy.rs`
- Modify: `amari-rewrite/src/network/mod.rs`
- Create: `amari-rewrite/tests/network_strategy.rs`

**Steps:**

1. RED graph-depth/out-degree/distance/novelty components, shortest known target path, finite weight validation, ties, unreachable target, and partial graph behavior.
2. Implement typed `NetworkScoreWeights`, all-minimization score components, and `NetworkGuidedStrategy`.
3. Select only retained actual successors and expose complete component evidence.
4. GREEN parity/determinism/limit tests.
5. Commit `feat: guide rewrite frontiers with networks`.

### Task 28: Combine network and Candle guidance and derive training traces

**Files:**
- Create: `amari-rewrite/src/network/hybrid.rs`
- Create: `amari-rewrite/src/network/training.rs`
- Modify: `amari-rewrite/src/network/mod.rs`
- Create: `amari-rewrite/tests/hybrid_strategy.rs`

**Steps:**

1. RED normalized network/neural score combination, zero/one weight extremes, canonical ties, model failures, graph partials, successful-path training pairs, and data ceilings.
2. Implement `HybridGuidedStrategy` with explicit finite weights and no mutable global model.
3. Add bounded successful-path → `RewriteTrainingExample` adapter using actual graph edges.
4. GREEN network+neural/all-feature tests; verify no project/file/network authority.
5. Commit `feat: combine geometric and neural rewrite guidance`.

### Task 29: Make network and hybrid guidance discoverable

**Files:**
- Modify: `amari-discovery/catalog/semantic/core.toml`
- Regenerate: `amari-discovery/catalog/generated.json`
- Create: `amari-discovery/tests/rewrite_discovery_network.rs`
- Modify: `scripts/run-discovery-test-shard.py`

**Steps:**

1. RED cfg structural records and semantic search/detail/graph/recommendation for geometric graph, network strategy, hybrid strategy, and training adapter.
2. Add experimental capabilities with exact `network`/`neural` feature refs and relations to normalization/neural guidance.
3. Keep discovery probe-free and do not enable heavy rewrite features in the installed command.
4. Regenerate/verify catalog, package count, semantic references, and sharding.
5. Independent review, all-feature build/performance measurement, and full cohort verification.
6. Commit `feat: discover guided rewrite search`; open grouped PR 6.

---

## Cohort 7 — Documentation, packaging, and feature-branch acceptance

### Task 30: Add research examples and public documentation

**Files:**
- Create: `amari-rewrite/examples/macros.rs`
- Create: `amari-rewrite/examples/analyze_and_complete.rs`
- Create: `amari-rewrite/examples/neural_training.rs`
- Create: `amari-rewrite/examples/z3_validation.rs`
- Create: `amari-rewrite/examples/hybrid_guidance.rs`
- Modify: `amari-rewrite/Cargo.toml` (required features per example)
- Modify: `amari-rewrite/README.md`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/guide/amari-discovery.md`
- Modify: `docs/roadmap/V0_20_0_TO_V0_25_0_RELEASE_SEQUENCE.md`

**Steps:**

1. RED compile/run each example under its exact feature set and add doc tests for stable macro/analysis APIs.
2. Document stability, MSRV, feature/dependency costs, bounds, unknown/partial semantics, solver quantifier limits, training determinism, and no GPU/WASM claims.
3. Add tested `amari discover search/detail/graph` examples for every rewrite capability.
4. Run all doc tests and every runnable shell example.
5. Commit `docs: document rewrite research workflows`.

### Task 31: Audit publication order and feature packaging

**Files:**
- Modify: `.github/workflows/publish.yml` only if prior wiring is incomplete
- Modify: `scripts/verify-workflow-crates.sh` only if needed
- Modify: `scripts/verify-publish-order.py` only if needed
- Create: `docs/releases/v0.24.0-rewrite-feature-gates.md`
- Modify: package include/exclude metadata only if archive inspection requires it

**Steps:**

1. Derive direct dependencies for both rewrite crates from Cargo metadata and verify macro crate → rewrite → discovery/root order.
2. Inspect default, macros, completion, neural, smt, network, and all-feature graphs; prove default packaging does not activate Candle/Z3/network.
3. Run unverified feature-branch package inspection with explicit caveats for unpublished 0.24 dependencies; record archive sizes and research dependency impact.
4. Verify vendored Z3 source is dependency-managed, no external solver executable is required, and package scripts do not fetch `gh-release` binaries.
5. Run workflow/binary/publish-order scripts and update aggregate release gate links.
6. Commit `docs: add rewrite feature release gates`.

### Task 32: Mandatory rewrite feature-branch verification

**Files:**
- Create: `scripts/verify-rewrite-features.sh`
- Create/modify: CI only for unique coverage and stable aggregate names
- Modify: implementation only for measured regressions

**Steps:**

1. RED the verifier against a deliberately omitted feature combination, then implement an explicit matrix: default, no-default, serialize, macros, completion, neural, smt, network, and all-features.
2. Run sequentially:

```bash
cargo fmt --all -- --check
git diff --check
cargo +1.85.0 check -p amari-rewrite --all-features
cargo test -p amari-rewrite
cargo test -p amari-rewrite --no-default-features
./scripts/verify-rewrite-features.sh
cargo test -p amari-rewrite-macros
cargo test -p amari-discovery --all-features
cargo test -p amari-discovery --no-default-features
cargo test --workspace --quiet
cargo clippy -p amari-rewrite -p amari-rewrite-macros -p amari-discovery \
  --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc \
  -p amari-rewrite -p amari-rewrite-macros -p amari-discovery \
  --all-features --no-deps
cargo run -p amari-discovery --example generate_catalog -- .
git diff --exit-code -- amari-discovery/catalog/generated.json
./scripts/verify-workflow-crates.sh
python3 scripts/verify-publish-order.py
python3 scripts/verify-amari-binary-owner.py
python3 scripts/verify-discovery-ci-sharding.py
./scripts/version-sync.sh verify 0.23.0
```

3. Measure cold/warm vendored Z3 build time, all-feature test time, rewrite package archives, and discovery release binary to expose research cost; compare evidence to the dedicated job's 45-minute timeout and full-target cache behavior, then adjust only if measured data requires it.
4. Independent final review; Critical/Important findings block.
5. Commit `test: verify rewrite research expansion`; open grouped PR 7.

---

## Post-merge aggregate acceptance

After all seven cohorts merge, execute Task 31 from
`docs/plans/2026-07-09-amari-discovery-implementation-plan.md` on the aggregate
release branch:

1. sync every package and active internal requirement to 0.24.0;
2. regenerate authoritative WASM and Rust catalogs after the final rewrite API;
3. run complete workspace/MSRV/feature/package/publish-dry-run matrices;
4. publish dependencies in verified order and wait for indexing;
5. package/install `amari-discovery` without `--no-verify` and repeat from the registry;
6. publish/tag only after all acceptance evidence exists.

No rewrite cohort, merged PR, passing path install, or unverified archive is a
0.24.0 release claim.
