# amari-rewrite 0.25 Comprehensive Rewrite and Inverse Expansion Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Deliver the complete approved 0.25 rewrite expansion—especially constrained relational inverse rewriting, residual-backed reversibility, backward/bidirectional reasoning, and certified/approximated regular-language preimages—alongside macros, analysis/completion, synthesis, Candle, network/holographic guidance, Z3, and truthful discovery integration.

**Architecture:** Preserve the stable 0.23 core. Add an `alloc`-capable constrained relation and tree-language substrate, then layer bounded search, analysis, synthesis, and opt-in research backends over it. The symbolic engine alone creates transitions; every exactness, exhaustion, approximation, proof, and replay claim has a typed authority contract.

**Tech Stack:** Rust 1.85+, `syn`/`quote`/`proc-macro-crate`, `sha2`, exact Candle 0.11.0, exact vendored Z3 0.20.2, `amari-network`, `amari-holographic`, `serde`, `trybuild`, and existing process-isolated `amari-discovery` probes.

---

## Execution rules

1. Do not begin implementation until 0.24.0 is published/tagged and its main → develop backmerge is complete.
2. Use a fresh worktree per grouped cohort. Keep one canonical task per RED→GREEN commit even inside grouped PRs.
3. New Rust/Python files begin with `SPDX-License-Identifier: MIT OR Apache-2.0`.
4. Existing 0.23 APIs are additive-only; legacy `BackwardSearch` behavior cannot change silently.
5. Validate limits before allocation, recursion, native backend setup, file access, or worker launch. Callers may tighten but never raise hard ceilings.
6. Default symbolic relation/language code remains `no_std + alloc`; macros and research backends are optional; Candle/Z3/network/holographic features imply `std`.
7. Never use unchecked public construction, hidden `expect`/`unwrap`, wildcard matches on authority enums, raw backend diagnostics, or opaque user closures in serializable relations.
8. Every generated predecessor and witness must replay forward through the original `TermSystem`. Every exact reversible result must pass the residual round-trip law.
9. Exact/exhaustive/unreachable claims require certificates described in the design. Limits are partial outcomes. Heuristic pruning is always approximate.
10. The closure-theorem spike is a hard gate: no public exact regular-preimage class may stabilize before independent mathematical review approves its theorem/classifier matrix.
11. Update structural and semantic discovery in every public cohort. Add process probes only for bounded pure symbolic functions; never expose Candle training/checkpoints, Z3 solving, arbitrary holographic datasets, or project authority.
12. Assign every new flat `amari-discovery/tests/*.rs` target to exactly one shard and preserve required aggregate check names.
13. Run expensive feature combinations sequentially. A grouped PR receives one independent review and one full verification pass after focused task checkpoints.
14. Critical/Important findings block merge.
15. Feature completion is not a 0.25 release. Aggregate versioning, packaging, publication, registry installation, npm/WASM evidence, and tag gates remain mandatory.

## Fixed hard ceilings

| Area | Hard ceilings |
| --- | --- |
| Terms/constraints | 4,096 nodes/term, depth 64, 4,096 constraints, 1,000,000 operations |
| Backward/bidirectional search | 65,536 states, 262,144 transitions, depth 64, 64 MiB evidence |
| Grounding | 256 symbols, rank 16, depth 16, 65,536 terms |
| Tree automata | 4,096 states, 65,536 transitions, rank 16, 65,536 determinized subsets |
| Language preimage | horizon 64, 4,096 saturation iterations, 1,000,000 operations, 64 MiB evidence |
| Critical pairs/joinability | 4,096 pairs, 65,536 states, 1,000,000 operations |
| Completion | 256 rules, 4,096 pairs/iterations, 1,000,000 operations |
| Refinement | 4,096 examples, 256 candidates/partitions/rules, 1,000,000 operations |
| Neural | width 64, hidden 256, 65,536 examples, 10,000 epochs, 16,777,216 tensor elements, 64 MiB checkpoint, 5 minutes |
| SMT | 256 rules, 512 symbols/variables, 65,536 term nodes, 4,096 assertions, 30 seconds |
| Search graph | 4,096 nodes, 65,536 edges, depth 64, 1,000,000 operations |

Discovery descriptors use substantially lower limits.

---

## Cohort 1 — 0.24 baseline, toolchain, dependencies, and basic macros

### Task 1: Establish the post-0.24 implementation baseline

**Files:** Modify planning/status docs only if release evidence differs.

1. RED: verify `v0.24.0`, crates.io packages, npm package, main release commit, and main → develop backmerge. Stop if any release fact is absent.
2. Create the Cohort 1 worktree from updated `develop`; record exact base in the PR.
3. Run workspace tests, discovery sharding/binary/publish-order checks, and `version-sync verify 0.24.0`.
4. Commit only if factual status docs require correction; otherwise record a verification-only checkpoint.

### Task 2: Raise and prove the research dependency baseline

**Files:** Modify root `Cargo.toml`, `Cargo.lock`, `amari-rewrite/Cargo.toml`, active MSRV docs/badges, `.github/workflows/ci.yml`; create `amari-rewrite/tests/research_dependencies.rs`.

1. Preflight exact availability/features with `cargo info candle-core@0.11.0`, `cargo info candle-nn@0.11.0`, `cargo info z3@0.20.2`, and `cargo info proc-macro-crate`; record output and stop/revise the decision if registry metadata differs. RED `cargo metadata` resolution plus feature-gated compile tests for Candle CPU tensor/VarMap, vendored Z3 solver, SHA-256 evidence, and optional holographic/network types.
2. Set workspace Rust version 1.85; add exact Candle `=0.11.0` and Z3 `=0.20.2` vendored; change root workspace SHA declaration to `sha2 = { version = "0.10", default-features = false }` and add `sha2.workspace = true` in `amari-rewrite`; add optional `amari-holographic` dependencies. Verify existing SHA consumers still pass under the workspace-level feature change.
3. Wire `neural`, `smt`, `network`, and `holographic-guidance` exactly as designed. Default/no-default must not compile Candle/Z3/network/holographic dependencies.
4. Add separate `MSRV Check (1.85)` and 45-minute vendored-Z3 jobs. Cache full target/CMake outputs with compiler+lockfile keys; preserve existing aggregate names and WASM isolation.
5. GREEN `cargo +1.85.0 check --workspace`, rewrite all-features, default/no-default, and focused tests. Record cold/warm Z3 times.
6. Commit `build: add rewrite research dependencies`.

### Task 3: Scaffold `amari-rewrite-macros`

**Files:** Create macro crate Cargo/lib; modify workspace, rewrite Cargo/lib, publish workflow; create `amari-rewrite/tests/macros_reexport.rs`.

1. RED import/re-export tests under `macros`; prove default/no-default do not compile the macro crate.
2. Add workspace `proc-macro-crate = "3"`; create publishable proc-macro crate with workspace metadata and syn/quote/proc-macro2.
3. Add placeholder entry points with stable compile diagnostics; publish macros before rewrite.
4. GREEN wiring tests and workflow/publish-order verifiers.
5. Commit `feat: scaffold rewrite macro crate`.

### Task 4: Implement `derive(Rewritable)`

**Files:** Create macro derive module; create trybuild pass/fail/renamed-crate fixtures and `macros_derive.rs`; add `trybuild = "1"` dev dependency.

1. RED preorder child/path/replacement tests and UI fixtures for structs/enums, explicit child attributes, unions, unsupported containers, duplicate attributes, and renamed crate.
2. Generate exhaustive variant matches and checked invalid-index handling through hygienic paths.
3. Reject unsupported syntax at precise spans; do not infer ambiguous recursive fields.
4. GREEN runtime/UI tests, macro Clippy/rustdoc, default rewrite tests.
5. Commit `feat: derive rewritable structures`.

### Task 5: Implement checked `term!`/`rule!` and discover macros

**Files:** Create macro term/rule modules and tests; modify semantic catalog; regenerate structural catalog; create `rewrite_discovery_macros.rs`; update discovery shard.

1. RED nested terms, variables, constants, `term!(f) == term!("f")`, renamed-crate hygiene, checked RHS variables, ARS/TRS Rule coexistence, and malformed grammar UI tests.
2. Expand `rule!` through fully qualified hygienic `amari_rewrite::trs::Rule::new`; never unchecked construction/expect.
3. Add macro package/symbol and semantic capability records with correct feature refs and CLI search/detail/graph tests.
4. Regenerate catalog, verify one macro package addition and no discovery self-indexing, assign test once.
5. GREEN macro/discovery suites, generator drift, sharding, publish order; independent Cohort 1 review.
6. Commit `feat: add discoverable rewrite syntax macros`; open PR 1.

---

## Cohort 2 — Constrained relations and exact reversible steps

### Task 6: Add relation limits, canonical digests, and logic variables

**Files:** Create `relation/{mod,limits,variable,digest}.rs`; modify lib/error; create `relation_contract.rs`.

1. RED zero/oversized configs, deterministic query namespaces, alpha-canonical renumbering, framed SHA-256 hashes, and term/depth/operation accounting.
2. Implement `LogicVar`, freshening namespace, `RelationLimits`, `RelationResources`, and validated `Sha256Digest`.
3. Keep implementation iterative/pre-allocation bounded and `alloc` compatible.
4. GREEN default/no-default/serialize tests and public docs.
5. Commit `feat: define constrained relation authority`.

### Task 7: Implement unification and normalized constraints

**Files:** Create `analysis/unify.rs`, `relation/constraints.rs`; modify substitution/error; create `unification.rs`, `constraints.rs`.

1. RED MGU cases, occurs check, repeated variables, arity mismatch, alpha-renaming, composition, equality/disequality tautology/contradiction/residuals, deterministic order, and limits.
2. Implement deterministic equation-worklist unification and checked idempotent substitution composition.
3. Normalize `ConstraintSet`; solve equalities, apply substitutions, retain canonical disequalities, and classify unsupported theory.
4. Property-test MGU equality, alpha invariance, and constraint-model soundness on generated small terms.
5. GREEN no-default/serialize/all-feature focused tests.
6. Commit `feat: solve first-order relation constraints`.

### Task 8: Compile rules into symbolic backward clauses

**Files:** Create `relation/clause.rs`, expand `inverse/mod.rs`; create `symbolic_predecessors.rs`.

1. RED direct/root/nested predecessors, fresh variables, erased variables, repeated RHS variables, impossible match, cyclic target, provenance order, and all limits.
2. Compile checked rules into freshenable `BackwardClause`; unify RHS with each target subterm and instantiate LHS.
3. Return `SymbolicPredecessor` with existentials, normalized constraints, substitution, provenance, and resources.
4. Add mandatory bounded-grounding forward-replay oracle tests.
5. Preserve legacy iterator bytes/behavior with regression tests.
6. Commit `feat: derive symbolic rewrite predecessors`.

### Task 9: Add finite grounding domains

**Files:** Create `relation/ground.rs`; modify relation mod; create `relation_grounding.rs`.

1. RED ranked-domain validation, canonical size/lexicographic enumeration, constraints, empty domains, rank/depth/term ceilings, and duplicate suppression.
2. Implement finite `GroundingDomain` and lazy bounded existential assignments.
3. Validate every emitted grounding by constraints and forward replay; expose typed complete/partial grounding outcomes.
4. GREEN property tests and no-default docs.
5. Commit `feat: ground symbolic predecessors safely`.

### Task 10: Implement automatic typed residuals

**Files:** Create `reversible/{mod,residual,step}.rs`; modify error/lib; create `residual_roundtrip.rs`.

1. RED lossless rule, erased variable, nested position, duplicate RHS variables, same-RHS rule ambiguity, tampered rule/path/binding/hash, oversized residual, deterministic bytes, and proof that any digest mismatch is a hard error with no returned reconstruction.
2. Derive erased bindings as LHS variables absent from RHS; emit rule/path/source/target authority during concrete forward step.
3. Validate residual/digest syntax, target hash, rule, path, binding schema, constraints, and limits; reconstruct privately, compare the resulting source hash, and return only on exact authority match.
4. Property-test `backward(forward(source)) == source` over generated checked systems.
5. GREEN no-default/serialize tests and docs.
6. Commit `feat: replay rewrites with typed residuals`.

### Task 11: Add bidirectional systems and inverse analysis

**Files:** Create `reversible/system.rs`, `analysis/inverse.rs`; create `bidirectional_system.rs`, `inverse_analysis.rs`.

1. RED forward/symbolic backward/residual replay composition; classify lossless, existential, ambiguous, finite-domain branching, and unsupported cases.
2. Implement declarative `BidirectionalRule/System` over checked rules, clauses, and residual schemas.
3. Implement `InverseAnalyzer` reports with stable evidence and no false functional-inverse claim.
4. GREEN property tests and exhaustive enum matches.
5. Commit `feat: analyze bidirectional rewrite systems`.

### Task 12: Discover and probe relation/reversibility APIs

**Files:** Modify semantic/probe catalogs, rewrite probe registry/adapter; regenerate catalog; create `probe_rewrite_relations.rs`, `rewrite_discovery_relations.rs`; update shard.

1. RED strict nested DTO and direct/process/CLI parity for one-step symbolic predecessor, inverse classification, and residual round trip.
2. Add semantic capabilities/relations for constrained inverse rewriting and reversible residuals.
3. Implement bounded process-isolated adapters with no caller-selected process/path/model/solver authority.
4. Regenerate catalog, lock descriptor/package counts, assign both tests exactly once.
5. Independent review and full Cohort 2 verification.
6. Commit `feat: discover constrained inverse rewriting`; open PR 2.

---

## Cohort 3 — Backward and bidirectional search

### Task 13: Define alpha-canonical symbolic search states

**Files:** Create `inverse/{state,config,outcome}.rs`; modify error; create `inverse_state.rs`.

1. RED alpha-equivalent state equality/hash, constraint-sensitive distinction, config ceilings, strict serialization, and retained-byte accounting.
2. Implement canonical `SymbolicState`, `InverseSearchConfig`, resource counters, frontier/witness/partial/unsupported outcome DTOs.
3. Prohibit `Exhausted` construction without certified finite/exact authority.
4. GREEN default/no-default/serialize tests.
5. Commit `feat: define inverse search states`.

### Task 14: Implement bounded `BackwardExplorer`

**Files:** Create `inverse/backward.rs`; modify inverse mod; create `backward_explorer.rs`.

1. RED direct/multi-step witness, existential states, cycle dedup, canonical BFS/cost order, finite-domain exhaustion, and every depth/state/transition/byte/operation ceiling.
2. Expand only valid `BackwardClause` transitions; retain provenance and partial frontier.
3. Replay each witness forward through original system before returning.
4. Return limit as `Partial`; certify exhaustion only for explicit finite grounding/search authority.
5. GREEN property replay/determinism tests.
6. Commit `feat: search symbolic predecessors backward`.

### Task 15: Implement bounded `BidirectionalExplorer`

**Files:** Create `inverse/bidirectional.rs`; create `bidirectional_explorer.rs`.

1. RED forward/backward meeting by unification, residual constraints, an adversarial combined contradiction that appears only after meeting substitution, no raw-term-equality meet, cyclic system, no meet, limits, and deterministic derivation.
2. Expand both frontiers with shared accounting; combine substitutions/constraints at candidate meets.
3. Replay full original-system path before witness acceptance.
4. Distinguish finite certified exhaustion from partial/unsupported.
5. GREEN generated small-system parity against one-direction searches.
6. Commit `feat: search rewrite relations bidirectionally`.

### Task 16: Add transparent guidance modes and replay hardening

**Files:** Create `inverse/guidance.rs`, `inverse/replay.rs`; create `inverse_guidance.rs`, `inverse_replay.rs`.

1. RED symbolic score dimensions, ordering-only candidate-set parity, beam/top-k dropped counts, approximate authority, tampered traces/config/hashes, and non-bypassable ceilings.
2. Implement `CompleteWithinLimits` and explicit `HeuristicPruning`; initial scorer is transparent symbolic cost only.
3. Bind derivation replay to system/query/config/guidance hashes and exact resource authority.
4. Prevent approximate outcomes from constructing exhaustion/unreachable certificates.
5. GREEN cross-process deterministic replay tests.
6. Commit `feat: harden guided inverse search`.

### Task 17: Expose backward/bidirectional probes

**Files:** Modify probe catalogs/adapters/registry; regenerate catalog; create `probe_rewrite_inverse_search.rs`, `rewrite_discovery_inverse_search.rs`; update shard.

1. RED direct/process/human/JSON/NDJSON parity for witnesses, partials, unsupported, exact ordering, and explicit approximate pruning.
2. Use strict typed requests with terms/rules/limits only; no project context.
3. Add semantic graph relations from legacy predecessor search to symbolic backward/bidirectional APIs.
4. Regenerate/verify/assign tests; independent Cohort 3 review.
5. Commit `feat: probe bidirectional rewrite search`; open PR 3.

---

## Cohort 4 — Tree automata and regular tree grammars

### Task 18: Implement ranked alphabets and validated NFTAs

**Files:** Create `language/{mod,alphabet,automaton,limits}.rs`; modify lib/error; create `tree_automaton.rs`.

1. RED rank conflicts, unknown states, arity mismatch, duplicate transitions, invalid finals, canonical ordering, term membership accepting runs, and ceilings.
2. Implement epsilon-free bottom-up NFTA with private validated canonical storage.
3. Validate before allocation and return typed malformed/limit errors.
4. Property-test membership against a direct recursive oracle.
5. GREEN no-default/serialize docs/tests.
6. Commit `feat: add finite tree automata`.

### Task 19: Add language operations and witnesses

**Files:** Create `language/operations.rs`, `language/witness.rs`; create `tree_automaton_operations.rs`.

1. RED emptiness/nonemptiness, smallest witness, union/intersection membership parity, trimming, disjoint state renaming, empty languages, and limits.
2. Implement reachable/co-reachable trimming and deterministic witness extraction.
3. Property-test Boolean operation membership on bounded generated terms.
4. GREEN focused no-default tests.
5. Commit `feat: operate on regular tree languages`.

### Task 20: Add determinization, completion, complement, and minimization

**Files:** Create `language/determinize.rs`, `language/minimize.rs`; create `tree_automaton_determinize.rs`.

1. RED subset construction parity, subset ceiling partial, deterministic completeness checks, complement laws, minimization language parity/idempotence, a nontrivial automaton with redundant context-equivalent states whose known minimal state count is smaller, and canonical bytes.
2. Implement bounded determinization; never return truncated automata as complete.
3. Trim reachable/co-reachable states first, then minimize validated complete deterministic automata by standard Myhill–Nerode context-equivalence partition refinement; canonicalize the unique minimal result up to renaming.
4. Property-test bounded De Morgan/membership laws, distinguishability/minimality against brute-force small contexts, and repeated canonicalization.
5. GREEN focused tests and docs.
6. Commit `feat: canonicalize tree automata`.

### Task 21: Implement fully supported regular tree grammars

**Files:** Create `language/grammar.rs`; create `regular_tree_grammar.rs`, grammar fixtures.

1. RED checked production/start/rank validation, fixed parser/render syntax, malformed/oversized input, grammar membership/witness, and automaton↔grammar round trips.
2. Implement canonical grammar storage and lossless checked conversions.
3. Route operations through automaton conversion rather than duplicate algorithms.
4. Property-test language parity and canonical round-trip bytes.
5. GREEN no-default/serialize/rustdoc tests.
6. Commit `feat: add regular tree grammars`.

### Task 22: Discover and probe regular-language foundations

**Files:** Modify semantic/probe catalogs and adapters; regenerate; create `probe_rewrite_languages.rs`, `rewrite_discovery_languages.rs`; update shard.

1. RED bounded conversion, membership, emptiness, witness, union/intersection parity and malformed/limit DTOs.
2. Add structural/semantic capabilities for NFTA and grammar APIs.
3. Implement process-isolated pure adapters; output certificates/hashes, not source text.
4. Regenerate/verify/assign; independent Cohort 4 review.
5. Commit `feat: discover regular tree languages`; open PR 4.

---

## Cohort 5 — Closure theorems and language preimages

### Task 23: Complete the recognizability-preservation research gate

**Files:** Create `docs/research/rewrite-preimage-closure-matrix.md`, `docs/adr/NNNN-regular-preimage-exactness.md`, bounded oracle prototype/tests under `amari-rewrite/tests/closure_oracle.rs`.

1. RED: collect counterexamples showing unrestricted exact-regular claims fail; write bounded concrete oracle tests before selecting classes.
2. Cite primary literature with theorem numbers/preconditions for ground and any broader candidate classes; map Amari semantics explicitly.
3. Specify exact one-step/finite-horizon/unbounded construction matrix and sound approximation obligations.
4. Have an independent mathematical reviewer approve or request changes. Do not begin Task 24 until no Critical/Important finding remains.
5. If no exact class is approved—including if the ground candidate needs unmet extra conditions—replace/replan Tasks 24–26 around classifiers plus lower/upper/partial authority only; do not implement or stabilize an exact API by schedule assumption.
6. Commit `docs: certify regular preimage scope`.

### Task 24: Implement TRS classifiers and evidence certificates

**Files:** Create `language/{classify,certificate}.rs`; create `trs_classification.rs`.

1. RED exact boundary/adversarial fixtures for every approved theorem precondition, unknown/unsupported cases, deterministic hashes, and strict certificate serialization.
2. Implement only classifiers approved in Task 23; exhaustive enum reasons, no heuristic promotion.
3. Bind certificates to system/language/classifier/construction/horizon/limit/result hashes.
4. GREEN no-default/serialize tests and source citations in rustdoc.
5. Commit `feat: classify regular preimage systems`.

### Task 25: Implement exact one-step and finite-horizon preimages

**Files:** Create `language/preimage.rs`; create `preimage_exact_bounded.rs`.

1. RED textbook and adversarial systems for every approved class, horizon 0/1/N, context positions, variables/ground rules as approved, and all automaton/resource ceilings.
2. Implement theorem-approved construction only; canonicalize after each horizon without changing language.
3. Differential-test membership against bounded concrete relational predecessor enumeration in both directions.
4. Return exact certificate only after all construction and canonicalization completes.
5. GREEN property tests and docs.
6. Commit `feat: compute certified bounded preimages`.

### Task 26: Implement supported unbounded saturation

**Files:** Create `language/saturate.rs`; create `preimage_saturation.rs`.

1. RED exact fixpoint examples, nonterminating growth, saturation iteration/state/transition limits, idempotent rerun, and unsupported class.
2. Implement only approved unbounded closure classes; expose partial frontier on growth/limit.
3. Validate fixpoint by an additional exact predecessor step and canonical equality before certificate.
4. GREEN differential tests and deterministic bytes.
5. Commit `feat: saturate certified predecessor languages`.

### Task 27: Implement lower/upper approximation bounds

**Files:** Create `language/approximate.rs`; create `preimage_bounds.rs`.

1. RED witnessed lower inclusion, sound upper containment against bounded oracle, abstraction trace, absent upper when no proof applies, monotonic iterations, and every widening/state limit.
2. Implement only Task 23-approved sound abstractions; record linearization/merge/widen events.
3. Lower contains replayed witnesses only; upper construction cannot reuse unproved heuristics.
4. Property-test bounded oracle ⊆ upper and lower witnesses ⊆ concrete preimage.
5. GREEN focused tests; independent mathematical spot review.
6. Commit `feat: bound regular predecessor languages`.

### Task 28: Add three-valued membership and refinement

**Files:** Create `language/refine.rs`; create `preimage_refinement.rs`.

1. RED `Proven/Possible/Excluded/Unknown`, witness replay, spurious possible term, lower growth, upper shrink, equality upgrade, and refinement limits.
2. Implement canonical query/refinement evidence and monotonicity checks.
3. Reject any refinement that shrinks lower, grows upper, or loses authority provenance.
4. GREEN property tests and serialization docs.
5. Commit `feat: refine predecessor language bounds`.

### Task 29: Discover and probe language preimages

**Files:** Modify semantic/probe catalogs/adapters/registry; regenerate; create `probe_rewrite_preimages.rs`, `rewrite_discovery_preimages.rs`; update shard.

1. RED process/direct parity for exact, bounds, partial, unsupported, membership statuses, certificates, strict nested DTOs, and tightened limits.
2. Probe only bounded approved constructions under much smaller state/horizon ceilings.
3. Add semantic relations among inverse search, automata, grammar, exact preimage, approximation, and verification.
4. Regenerate/verify/assign tests; independent full Cohort 5 mathematical/architecture review.
5. Commit `feat: discover regular language preimages`; open PR 5.

---

## Cohort 6 — Critical pairs, confluence, termination, and completion

### Task 30: Generate deterministic critical pairs

**Files:** Create `analysis/critical_pairs.rs`; create `critical_pairs.rs`.

1. RED root/non-root/self overlaps, renamed-apart variables, variable-position exclusion, non-left-linear detection, trivial pairs, provenance, order, and limits.
2. Share variable-position/left-linearity helpers with inverse classifier code where semantics coincide.
3. Use Task 7 unification and checked substitutions; dedup exact pair+provenance only.
4. GREEN textbook/property tests.
5. Commit `feat: compute rewrite critical pairs`.

### Task 31: Add bounded joinability and local-confluence reports

**Files:** Create `analysis/confluence.rs`; create `confluence.rs`.

1. RED joinable left-linear diamond, nonjoinable/limit/cycle, non-left-linear unknown, deterministic witnesses, and mixed reports.
2. Implement bounded bidirectional successor search with shared search accounting.
3. Certify local confluence only for exhaustive left-linear critical-pair coverage.
4. GREEN no-default/property tests and docs.
5. Commit `feat: analyze bounded rewrite confluence`.

### Task 32: Add LPO termination certificates

**Files:** Create `analysis/lpo.rs`; create `lpo.rs`.

1. RED caller-supplied ordering validation (missing, duplicate, cyclic/non-total relation forms), subterm/precedence/lexicographic cases, variable condition, orientable/unorientable systems, and limits.
2. Require one strict total ordering over every used ranked symbol; reject rather than extend partial orders, then implement memoized bounded comparison.
3. Return `ProvedTerminating` only when every rule strictly decreases.
4. Property-test irreflexivity/transitivity on generated small terms.
5. Commit `feat: certify termination with lpo`.

### Task 33: Implement bounded Knuth–Bendix completion

**Files:** Create `completion/{mod,config,trace}.rs`; modify Cargo/lib/error; create `completion.rs`.

1. RED orientation, simplification, pair addition, duplicate suppression, finite example, unorientable/non-left-linear/limit outcomes.
2. Implement canonical bounded loop using Tasks 30–32 and checked Rule construction.
3. Return complete/partial/failed with bounded trace; require LPO+left-linear confluence postchecks for `Complete`.
4. GREEN completion/all-feature/no-default-with-completion tests.
5. Commit `feat: complete bounded rewrite systems`.

### Task 34: Integrate completion with inverse authority

**Files:** Create `completion/inverse.rs`; modify inverse analyzer; create `completion_inverse.rs`.

1. RED equationally equivalent but directed-reachability-different systems, completed-system candidate witnesses, relation-changing rejection, residual schema differences, partial completion, and replay against original directed system.
2. Treat completion traces as equational-theory evidence only. Permit completed rules to generate candidates, but require original-system replay for every positive witness; transfer no negative/exhaustion authority without a separate directed `RelationEquivalenceCertificate` containing both-direction derivations (optionally supported, never replaced, by SMT evidence).
3. Never replace original inverse semantics or certificates solely from completed rules or `CompletionOutcome::Complete`.
4. GREEN differential tests.
5. Commit `feat: validate completion-assisted inverse search`.

### Task 35: Discover and probe analysis/completion

**Files:** Modify semantic/probe catalogs/adapters; regenerate; create `probe_rewrite_analysis.rs`, `rewrite_discovery_analysis.rs`; update shard.

1. RED strict bounded critical-pair/joinability/LPO/completion direct/process parity and inverse-authority fields.
2. Add semantic capabilities and graph relationships to inverse/preimage systems.
3. Regenerate/verify/assign; independent Cohort 6 review.
4. Commit `feat: discover rewrite analysis`; open PR 6.

---

## Cohort 7 — Negative/inverse synthesis and relational macros

### Task 36: Implement deterministic negative-example specialization

**Files:** Create/expand `synthesis/refinement.rs`; create `rule_specialization.rs`.

1. RED negative coverage, discriminating path, deterministic partitions, inseparable examples, duplicates, and every config ceiling.
2. Implement exact coverage detection, canonical candidate order, per-partition inference, and validation against all examples.
3. Return refined/inconclusive/limit states; preserve old inference APIs.
4. Property-test every returned rule covers a positive and no supplied negative.
5. Commit `feat: specialize rules from negative examples`.

### Task 37: Infer inverse clauses and bidirectional systems

**Files:** Create `synthesis/inverse.rs`; create `inverse_synthesis.rs`.

1. RED source→target examples with erased variables, negative pairs, residual examples, ambiguity, underdetermination, and limits.
2. Infer checked forward rules then derive clauses/residual schemas; do not synthesize unchecked reverse rules with extra RHS variables.
3. Require forward replay, negative rejection, and residual round-trip validation for promoted candidates.
4. Return typed candidate/refutation/unknown evidence.
5. Commit `feat: infer bidirectional rewrite relations`.

### Task 38: Add checked relational macros

**Files:** Create macro relation module; modify exports; add runtime/UI tests.

1. RED checked `relation!`/bidirectional syntax, existential markers, malformed constraints, renamed crate, and ARS/TRS ambiguity.
2. Expand only to fully qualified checked declarative constructors; no closure or unchecked rule generation.
3. Lock exact output/result types and diagnostics with trybuild.
4. GREEN macros/default/no-default tests and docs.
5. Commit `feat: add checked relational rewrite syntax`.

### Task 39: Discover and probe inverse synthesis

**Files:** Modify semantic/probe catalogs/adapters; regenerate; create `probe_rewrite_inverse_synthesis.rs`, `rewrite_discovery_synthesis.rs`; update shard.

1. RED bounded refinement/inverse inference parity, malformed examples, strict unknown fields, and all tightened limits.
2. Add syntax/refinement/inverse-synthesis capabilities and relations.
3. Regenerate/verify/assign; independent Cohort 7 review.
4. Commit `feat: discover inverse rewrite synthesis`; open PR 7.

---

## Cohort 8 — Candle inverse ranking and training

### Task 40: Encode and score inverse candidates with Candle

**Files:** Expand `neural/{mod,config,encode,model}.rs`; create `neural_inverse_model.rs`.

1. RED frozen feature schema/hash, term+constraint+provenance encoding, tensor shapes, deterministic CPU scores, ties, nonfinite values, and limits.
2. Preserve `DifferentiableRule`; add structural encoder and bounded MLP ranker.
3. Canonicalize candidates before batching; map Candle errors to sanitized typed errors.
4. GREEN neural/all-feature tests; default/no-default remains Candle-free.
5. Commit `feat: score inverse rewrites with candle`.

### Task 41: Derive training data from replayed traces

**Files:** Create `neural/data.rs`; create `neural_inverse_data.rs`.

1. RED backward/bidirectional positives, valid unchosen negatives, target distance, duplicate candidates, explicit seed sampling, empty/partial/approximate trace rejection, and ceilings.
2. Generate pairwise examples only from replay-validated exact traces by default; approximate traces require explicit labeling and cannot be proof data.
3. GREEN deterministic/property tests.
4. Commit `feat: derive inverse ranking data`.

### Task 42: Add bounded AdamW training

**Files:** Create `neural/train.rs`; create `neural_training.rs`.

1. RED pairwise margin loss, fixed-corpus reduction, fixed-seed completed determinism, deadline partial non-replayability, nonfinite gradients, and all ceilings.
2. Implement bounded batches/backprop/gradient checks/AdamW and typed reports.
3. Check deadline at deterministic epoch boundaries; no GPU/backend selection or project loading.
4. GREEN neural/all-feature tests and docs.
5. Commit `feat: train inverse rewrite rankers`.

### Task 43: Add safe-tensor checkpoints

**Files:** Create `neural/checkpoint.rs`; create `neural_checkpoint.rs`.

1. RED score round trip, schema/model metadata, wrong shape/schema, truncated/oversized/nonfinite file, symlink rejection, and canonical parameter ordering.
2. Validate regular file/byte cap/metadata/tensors before model construction; sanitize path/backend errors.
3. GREEN tempdir tests and docs.
4. Commit `feat: checkpoint rewrite rankers`.

### Task 44: Integrate Candle ordering and pruning

**Files:** Create `neural/strategy.rs`; modify inverse guidance; create `neural_inverse_strategy.rs`.

1. RED ordering-only candidate-set parity, preferred valid predecessor, ties, model failure, beam/top-k drops, approximate authority, and replay hashes.
2. Implement rank adapter over symbolic candidates only.
3. Ensure complete mode never drops; heuristic mode records dropped counts and cannot construct exhaustion.
4. GREEN differential tests.
5. Commit `feat: guide inverse search with neural scores`.

### Task 45: Catalog neural inverse guidance

**Files:** Modify semantic catalog; regenerate structural catalog; create `rewrite_discovery_neural.rs`; update shard.

1. RED feature-gated structural and search/detail/graph/recommendation tests for encoder/ranker/trainer/checkpoint/strategy.
2. Add experimental capabilities; do not add probes or enable Candle in discovery.
3. Regenerate/verify/assign; independent Cohort 8 review and build-size/time report.
4. Commit `feat: discover neural inverse guidance`; open PR 8.

---

## Cohort 9 — Geometric-network and holographic inverse guidance

### Task 46: Build geometric inverse search graphs and scorer

**Files:** Expand `network/{mod,config,graph,strategy}.rs`; create `network_inverse.rs`.

1. RED golden `InverseStateEmbeddingV1` coefficients for log node count, depth, variable ratio, symbol/arity diversity, constraint density, existential ratio, provenance/search depth, and signed SHA content sketch; finite normalization boundaries, collision-with-distinct-state identity, state/transition parity, distance/novelty/branching components, partial graph, cycles, and limits.
2. Implement the frozen eight-blade descriptor with hard-ceiling normalizers and explicit version/hash; document it as a heuristic non-isometric embedding. Build bounded `GeometricNetwork<3,0,0>` from valid inverse transitions only and key node identity by canonical state digest, never embedding equality.
3. Rank retained candidates transparently; no graph-generated transition or geometric correctness claim.
4. GREEN network+neural/all-feature tests.
5. Commit `feat: guide inverse search geometrically`.

### Task 47: Add deterministic holographic trace recall

**Files:** Create `holographic/{mod,encode,recall}.rs`; modify Cargo/features; create `holographic_inverse.rs`.

1. RED frozen encoder vectors, additive superposition, exact repeated recall order, seed/dimension/trace hash identity, empty/malformed/oversized corpus, and limits.
2. Encode only caller-provided replay-validated goals/traces; use canonical `BindingAlgebra::superpose`, not bundle.
3. Return transparent similarity evidence; no external/project memory access.
4. GREEN holographic-guidance/all-feature tests and default isolation.
5. Commit `feat: recall analogous inverse traces`.

### Task 48: Combine symbolic, Candle, network, and recall scores

**Files:** Create `inverse/hybrid.rs`; create `hybrid_inverse_guidance.rs`.

1. RED normalized score components, zero/one weight extremes, ties, missing backend, failures, ordering parity, pruning authority, and deterministic hashes.
2. Implement typed finite weights and exhaustive component evidence; no mutable global model/memory/graph.
3. Generate training/recall traces only after original-system replay.
4. GREEN combined-feature/all-feature tests.
5. Commit `feat: combine inverse search guidance`.

### Task 49: Catalog geometric/holographic/hybrid guidance

**Files:** Modify semantic catalog; regenerate; create `rewrite_discovery_guidance.rs`; update shard.

1. RED exact feature refs and search/detail/graph/recommendation tests.
2. Add catalog-only experimental capabilities; no model/recall data probes and no heavy discovery features.
3. Regenerate/verify/assign; independent Cohort 9 review and cost report.
4. Commit `feat: discover hybrid inverse guidance`; open PR 9.

---

## Cohort 10 — Vendored Z3 validation

### Task 50: Translate first-order signatures and prove equivalence

**Files:** Expand `smt/{mod,config,signature,translate,solver,certificate}.rs`; create `smt_equivalence.rs`.

1. RED rank conflicts, deterministic content-addressed names, free variables, nested terms, rule axioms, reflexive/axiom proof, satisfiable refutation, unknown/timeout, hashes, and ceilings.
2. Use one uninterpreted term sort and function per ranked symbol; encode checked rules as universal equations and query inequality. Freeze/version the Z3 tactic and all timeout/seed/MBQI/e-matching parameters used by each supported fragment.
3. Map unsat/sat/unknown exhaustively; sanitize model/reason evidence and bind solver/tactic/parameter identity into certificates.
4. GREEN SMT/all-feature tests; verify no external process/downloaded binary.
5. Commit `feat: validate rewrite equivalence with z3`.

### Task 51: Validate constraints and residual round trips

**Files:** Create `smt/relation.rs`; create `smt_inverse.rs`.

1. RED satisfiable/unsatisfiable residual constraints, existential variables, proved round trip, counterexample, unknown, malformed signature, and limits.
2. Translate only supported core equality/disequality theory and selected quantified laws.
3. Bind certificates to relation/residual/system/config hashes; unknown never proves.
4. GREEN `smt` + relation/reversible tests.
5. Commit `feat: validate inverse relations with z3`.

### Task 52: Integrate solver evidence with completion and synthesis

**Files:** Create `smt/integration.rs`; modify completion/synthesis; create `smt_integration.rs`.

1. RED candidate acceptance/refutation/unknown, completion relation preservation, inverse candidate counterexample, and symbolic parity without SMT.
2. Add opt-in integration functions; reject only concrete refutations and preserve unknown evidence.
3. Never trust raw solver text or change base symbolic defaults.
4. GREEN `smt+completion`, synthesis, and all-feature tests.
5. Commit `feat: attach solver evidence to inverse rewriting`.

### Task 53: Catalog solver-backed rewrite validation

**Files:** Modify semantic catalog; regenerate; create `rewrite_discovery_smt.rs`; update shard.

1. RED structural feature and semantic search/detail/graph/recommendation tests for equivalence, constraints, residual laws, and integrations.
2. Assert no solver probe exists and discovery does not enable rewrite `smt`.
3. Regenerate/verify/assign; independent Cohort 10 review and Z3 cold/warm/package evidence.
4. Commit `feat: discover solver-backed inverse validation`; open PR 10.

---

## Cohort 11 — Documentation, packaging, and feature-branch acceptance

### Task 54: Add comprehensive examples and public documentation

**Files:** Add examples for macros, symbolic predecessors, residual round trips, backward/bidirectional search, automata/grammar, exact/bounded preimages, completion, neural, Z3, and hybrid guidance; modify rewrite/root READMEs, CHANGELOG, discovery guide, roadmap.

1. RED compile/run every example under exact required features; add doc tests for stable contracts.
2. Document relational non-functionality, existential holes, residual laws, exact/exhaustive/partial/approximate authority, closure matrix, three-valued membership, feature/MSRV/build costs, and security boundaries.
3. Add executable `amari discover` and bounded probe examples for all safe inverse capabilities.
4. Run every runnable shell example and doc test.
5. Commit `docs: document comprehensive inverse rewriting`.

### Task 55: Audit publication, packaging, and CI matrices

**Files:** Modify publish/workflow/verifier scripts only as required; create `docs/releases/v0.25.0-rewrite-feature-gates.md`.

1. Derive dependencies from Cargo metadata; verify macro → rewrite and holographic/network → rewrite → discovery/root order.
2. Prove default package graph excludes Candle/Z3/network/holographic backends; inspect every individual feature and all-features archive.
3. Verify vendored Z3 uses dependency-managed source, no external executable or `gh-release`, and record cold/warm/build/archive costs.
4. Lock explicit default/no-default/serialize/macros/completion/neural/smt/network/holographic/combined/all-feature CI coverage without duplicate expensive jobs.
5. Run workflow/binary/publish-order/sharding verifiers.
6. Commit `docs: add rewrite 0.25 release gates`.

### Task 56: Mandatory feature-branch verification

**Files:** Create `scripts/verify-rewrite-features.sh`; modify implementation/tests only for measured regressions.

1. RED verifier against an omitted feature, then implement exact sequential matrix.
2. Run:

```bash
cargo fmt --all -- --check
git diff --check
cargo +1.85.0 check --workspace
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
./scripts/version-sync.sh verify 0.24.0
./scripts/verify-workflow-crates.sh
python3 scripts/verify-publish-order.py
python3 scripts/verify-amari-binary-owner.py
python3 scripts/verify-discovery-ci-sharding.py
```

3. Run property/fuzz suites for relation constraints, residual drift, inverse search replay, automata/grammar, preimage containment, worker DTOs, and hard boundaries.
4. Measure all-feature time, Z3 cold/warm time, package archives, and discovery release binary.
5. Independent final review; Critical/Important findings block.
6. Commit `test: verify comprehensive rewrite expansion`; open PR 11.

---

## Post-merge aggregate 0.25 acceptance

After all eleven cohorts merge:

1. create the aggregate release branch from synchronized `develop`;
2. set every workspace/internal Rust/npm constraint to 0.25.0;
3. regenerate Rust and authoritative WASM catalogs after the final API merge;
4. run the complete MSRV/default/no-default/feature/all-feature/source/docs/
   property/package matrix;
5. package and publish dependencies in verified order, waiting for indexing;
6. package/install `amari-discovery` from a verified extracted archive without
   `--no-verify` and repeat from crates.io;
7. complete npm/WASM publication and smoke tests;
8. merge release branch, tag `v0.25.0`, and backmerge main to develop.

No planning PR, feature cohort, path install, unverified archive, version bump,
or CI-green aggregate branch is a release claim before publication and tag
evidence exists.
