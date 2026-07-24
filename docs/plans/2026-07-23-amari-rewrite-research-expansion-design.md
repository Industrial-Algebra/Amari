# amari-rewrite 0.24 Research Expansion Design

- Date: 2026-07-23
- Status: Approved
- Supersedes: the unresolved choices in `2026-07-09-amari-rewrite-0.24-decisions.md`
- Preserves: the stable 0.23 core described by `2026-05-10-amari-rewrite-design.md`

## Goal

Expand `amari-rewrite` from a stable symbolic ARS/TRS foundation plus
experimental traits into a research-capable but bounded rewrite platform:
compile-time rewrite ergonomics, first-order confluence and termination
analysis, bounded Knuth–Bendix completion, concrete Candle training/inference,
in-process Z3 validation, geometric rewrite-search guidance, and deterministic
negative-example refinement. Every surface must also be truthfully discoverable
through `amari-discovery`.

“Research-heavy” does not mean unbounded. Every search, completion, solver,
training, graph, and synthesis entry point has explicit limits and a typed
complete, partial, unknown, rejected, or failed result. Existing 0.23 APIs
remain additive and stable.

## Release and compatibility posture

- The workspace MSRV rises from Rust 1.75 to **Rust 1.85**, required by current
  `z3` 0.20 and its Rust 2024 binding crate. This is a release-wide change and
  must update root metadata, badges, contributor docs, and CI validation.
- `amari-rewrite` keeps `default = ["std"]`. Candle, Z3, macros, completion,
  and network guidance remain opt-in features.
- Symbolic analysis that needs only `alloc` remains available without `std`
  where practical. Candle and Z3 features imply `std`.
- Current stable symbolic types (`Term`, `Rule`, `TermSystem`, ARS APIs,
  inverse search, anti-unification, `infer_rule`, and `infer_rules`) are not
  removed or semantically weakened.
- No 0.24 WASM binding is required. Discovery is the user-facing integration
  surface for these research capabilities.
- `0.24.0` is not accepted until this expansion merges and aggregate release
  Task 31 completes versioning, catalogs, packages, publication, and registry
  installation.

## Crate and feature architecture

A new workspace package, `amari-rewrite-macros`, owns all three proc macros.
It is published before `amari-rewrite`; `amari-rewrite` re-exports macros behind
its `macros` feature.

The intended feature map is:

```toml
[features]
default = ["std"]
std = []
serialize = ["dep:serde"]
macros = ["dep:amari-rewrite-macros"]
completion = []
neural = ["std", "dep:candle-core", "dep:candle-nn", "dep:sha2"]
smt = ["std", "dep:z3", "dep:sha2"]
network = ["std", "neural", "dep:amari-network"]
```

Pinned research dependencies:

- `candle-core = "=0.11.0"`, no accelerator features;
- `candle-nn = "=0.11.0"`, no accelerator features;
- `z3 = "=0.20.2"`, default features off, `vendored` enabled;
- optional workspace `sha2` for canonical neural/SMT evidence hashes;
- `syn`, `quote`, `proc-macro2`, and `proc-macro-crate` for macros;
- `trybuild` as a rewrite-crate dev dependency for UI contracts.

The exact Candle pins are intentional because Candle 0.11 does not declare an
MSRV. Cohort 1 must prove both exact releases under Rust 1.85 before the MSRV
change merges.

Vendored Z3 builds in process from reviewed crate source. The design rejects an
external `z3` executable and the `gh-release` build-time binary download path.
This makes all-feature builds heavier but avoids shell authority and hidden
runtime dependencies. CI must cache Cargo/build outputs and keep required check
names stable.

## Proc-macro ergonomics

`amari-rewrite-macros` exports:

- `#[derive(Rewritable)]` for recursive structs and enums;
- `term!(...)` for first-order term construction;
- `rule!(lhs => rhs)` for checked `Rule` construction.

Recursive children are explicit, using `#[rewritable(child)]` on fields of the
deriving type or `Box<DerivingType>`. Untagged fields are atomic payload. The
derive supports named, tuple, and unit variants; rejects unions, unsupported
child containers, duplicate attributes, and child fields whose type cannot
produce `&Self`. Generated `replace_child` code clones non-replaced payload and
returns `RewriteError::InvalidChildIndex` rather than panicking.

`term!` uses a deterministic grammar: uppercase identifiers or `?name` are
variables; lowercase identifiers and string literals are symbols; `f(a, X)` is
a symbol application. Identifier and string spellings are equivalent, so
`term!(f) == term!("f")`; quotes never become symbol content. `rule!` expands
through the hygienically resolved, fully qualified `amari_rewrite::trs::Rule::new`
and returns `RewriteResult<trs::Rule>`, never the distinct ARS `Rule`, an
unchecked rule, or hidden `expect`. `proc-macro-crate` resolves renamed
`amari-rewrite` dependencies hygienically. Trybuild pass/fail fixtures lock
diagnostics and prevent accidental acceptance of ambiguous syntax.

Macros are stable in 0.24 because they only construct or implement stable core
APIs. Their catalog records and semantic capability are generated and tested.

## First-order analysis substrate

A new `analysis` module supplies shared, deterministic foundations:

- variable renaming with collision-free namespaces;
- occurs-check first-order unification;
- substitutions composed in canonical variable order;
- proper non-variable overlap positions;
- critical-pair generation with rule/position provenance;
- bounded bidirectional joinability search;
- lexicographic path ordering (LPO) with explicit total symbol precedence.

Unification never treats a pattern match as symmetric unification. It returns a
typed substitution or typed failure and applies operation/node/depth limits
before recursive growth. Critical-pair output is sorted by rule indices,
position, and canonical term order. Trivial equal pairs are identified, not
silently discarded.

`ConfluenceAnalyzer` reports each pair as `Joinable`, `NotJoinableWithinBounds`,
or `LimitReached`. It may certify local confluence through the ordinary
critical-pair criterion only when every left-hand side is left-linear, pair
generation is exhaustive, and every pair is joinable. Non-left-linear systems
return `Unknown` with explicit evidence unless a later parallel-critical-pair
extension is implemented. It does not claim global confluence from bounded
evidence.

`LpoPrecedence` validates unique symbols and total order. `LpoAnalyzer` reports
whether each rule strictly decreases, fails to orient, or exceeds limits. A
successful LPO orientation is a sound termination certificate for the checked
rules. Failure to orient is `Unknown`, not proof of non-termination.

## Bounded Knuth–Bendix completion

Feature `completion` adds equation orientation and completion over the analysis
substrate. Public configuration has non-bypassable ceilings for input rules,
term nodes/depth, pending pairs, generated rules, iterations, and total
operations. Callers may tighten but not raise those ceilings.

The completion loop:

1. validates and simplifies equations;
2. orients them with LPO;
3. reduces both sides under current rules;
4. emits critical pairs in canonical order;
5. adds a new oriented rule only when it is nontrivial and checked;
6. re-simplifies affected rules;
7. stops on convergence, an unorientable equation, or a bound.

`CompletionOutcome` distinguishes `Complete`, `Partial`, and `Failed` and
contains a bounded proof trace: orientation, simplification, critical-pair,
rule-addition, and limit events. `Complete` additionally requires a terminating
LPO orientation and the sound local-confluence preconditions above; otherwise
the useful result remains `Partial`/`Unknown`. “Complete” means completion for
the supplied finite equations under the implemented ordering and checks, not
completeness of arbitrary equational reasoning.

## Concrete Candle neural rewriting

Feature `neural` retains `DifferentiableRule<State>` and adds concrete CPU-only
Candle components:

- `TermEncoder` and deterministic `StructuralTermEncoder`;
- a fixed-width feature schema with versioned ordering;
- `CandleRewriteRanker`, a small configurable MLP producing one candidate
  score;
- `RewriteTrainingExample` and bounded trace-to-pair generation;
- `TrainerConfig` and `TrainingReport`;
- pairwise margin-ranking training with AdamW;
- safe-tensor checkpoint save/load with model/feature schema metadata;
- `NeuralGuidedStrategy` selecting among real `TermSystem::successors`.

Structural features include bounded node/depth/arity/variable statistics,
root-symbol hashing, rule index, rewrite position, and optional target-distance
features. Hashing is frozen and seed-explicit. Candidate ordering before tensor
construction and equal-score tie-breaking use canonical `Term` order, so CPU
inference is deterministic for fixed weights and inputs.

Training limits cap examples, candidates, epochs, batch size, tensor elements,
operations, checkpoint bytes, and wall-clock duration. Completed fixed-seed
runs are byte-deterministic; deadline-truncated partial runs are explicitly
non-replayable because the completed epoch can depend on machine speed. Reports
distinguish those states without recording wall timestamps. Non-finite tensors,
shape mismatches, unknown feature schemas, oversized checkpoints, and Candle
failures map to typed `RewriteError` variants without panic. The crate owns a
useful training loop, not data collection from arbitrary projects and not GPU
backend selection.

## In-process Z3 validation

Feature `smt` keeps the generic `RewriteSolver` trait and adds
`Z3RewriteSolver`. It translates a validated first-order signature into one Z3
uninterpreted term sort, constructor/function declarations by symbol arity,
free variables, and universally quantified rewrite equations. Symbol names are
content-addressed before entering Z3.

`Z3SolverConfig` caps rules, symbols, variables, term nodes/depth, generated
assertions, and solver timeout. The solver checks `lhs != rhs` under the rule
axioms:

- `unsat` produces a `ProvedEquivalent` certificate;
- `sat` produces `Refuted` with a bounded, sanitized model summary;
- `unknown` produces `Unknown` with a fixed reason category;
- translation/limit/backend errors are typed failures.

Certificates record normalized input hashes, rule-set hash, limits, Z3 version,
and status. Raw solver diagnostics, native paths, or unrestricted model text are
not public evidence. APIs include equivalence checking and checked rule
validation. Quantifier incompleteness is explicit: `Unknown` is never promoted
to proof.

An optional integration accepts completion/refinement candidates and rejects a
candidate only on a concrete refutation; unknown results preserve the symbolic
outcome with evidence.

## Geometric and learned search guidance

Feature `network` replaces the summary-only bridge with a bounded
`RewriteSearchGraph`:

- nodes are deduplicated `Term` values;
- edges retain source, target, rule index, and rewrite position;
- node embeddings use deterministic structural features in
  `GeometricNetwork<3,0,0>`;
- graph construction is bounded BFS over actual `TermSystem::successors`;
- partial graphs retain deterministic frontier evidence.

`NetworkGuidedStrategy` ranks frontier nodes using explicit weighted graph
features: depth, out-degree, geometric distance, shortest known target path,
and novelty. `HybridGuidedStrategy` combines normalized network features with
`CandleRewriteRanker` scores. Weights are typed and finite, and canonical term
order resolves ties. No implicit global model or mutable singleton exists.

A trace-data adapter produces bounded neural training examples from successful
paths. It never executes project code, obtains data from the network, or grants
additional authority. Search reports expose graph limits, explored nodes,
frontier, chosen transitions, score components, and complete/partial state.

## Negative-example specialization

The existing `infer_rule` and `infer_rules` APIs remain. A new bounded
`RuleRefiner` adds deterministic heuristic specialization:

1. infer a general positive rule;
2. identify negative examples the rule reproduces exactly;
3. find discriminating non-variable paths;
4. partition positives by canonical subterm shape;
5. infer and validate specialized rules per partition;
6. retain only rules that cover positives and no supplied negative;
7. report uncovered positives, rejected candidates, and exhausted limits.

`InferenceConfig` caps examples, term size/depth, partitions, candidates,
rules, and operations. `InferenceOutcome` is `Refined`, `Inconclusive`, or
`LimitReached`; heuristic failure is not reported as impossibility. With `smt`,
`refine_with_solver` can request bounded counterexample evidence. With
`completion`, refined rules can be analyzed, but neither feature is required
for deterministic base specialization.

## Discovery integration

Discoverability is part of each implementation cohort, not post-release docs.
Generated structural records must include the macro crate, cfg-gated research
APIs, feature requirements, examples, and proc/function-like macro records.
Curated semantic capabilities cover:

- rewrite macros;
- unification and critical-pair analysis;
- LPO termination evidence;
- bounded completion;
- negative-example refinement;
- Candle training and neural guidance;
- Z3 equivalence validation;
- geometric and hybrid search guidance.

Relationships connect inference → analysis → completion → normalization and
network/neural/SMT alternatives. `amari discover search`, `detail`, and `graph`
must resolve every capability. Rust project inspection and recommendation must
recognize corresponding crate features and symbols.

Process-isolated discovery probes are added only for bounded pure-symbolic
operations: critical pairs/joinability, LPO orientation, completion, and
negative-example refinement. Candle training and Z3 solving are catalogued but
not executable through discovery in 0.24. This preserves the discovery rule
that probes do not gain arbitrary native solver, file, provider, shell, or
network authority. Probe DTOs reject unknown fields and reuse the existing
bounded term/rule transport.

After every public rewrite cohort, regenerate `catalog/generated.json`, update
semantic/probe manifests, verify catalog identity, and test real CLI search and
recommendation. `amari-discovery` remains excluded from its own generated Rust
records.

## Errors, limits, and determinism

`RewriteError` gains structured variants for invalid configurations,
limit-exhaustion category, unification/ordering/completion failures, tensor
shape/non-finite/checkpoint errors, solver translation/backend/unknown states,
and network construction. Public result enums carry domain-level
`Unknown`/`Partial`/`Inconclusive` values where useful evidence exists; errors
are reserved for invalid input, failed authority, or unusable execution.

Every configuration rejects zero values and values above fixed ceilings.
Encoded input and output byte limits accompany node/operation limits. Hashes
use canonical serialization and SHA-256. No public API depends on hash-map
iteration order, process randomness, current directory, absolute path, or wall
clock for semantic ordering.

## Testing and verification

Each canonical task follows RED → GREEN → refactor and commits separately.
Required coverage includes:

- unit/property tests for unification, substitution composition, critical pairs,
  LPO, and completion invariants;
- trybuild macro pass/fail/UI diagnostics and renamed-crate hygiene;
- Candle tensor shape, deterministic inference, loss decrease, checkpoint
  round-trip, non-finite and limit tests;
- Z3 proof/refutation/unknown/timeout, rule axioms, sanitization, and limit
  tests under vendored builds;
- network graph parity with direct successors, deterministic partials, hybrid
  ranking, and training-data bounds;
- specialization soundness against all supplied examples and deterministic
  inconclusive outcomes;
- discovery catalog, CLI search/detail/graph, recommendation, probe parity,
  process isolation, and sharding tests;
- default, no-default, individual-feature, all-feature, rustdoc, Clippy, format,
  package, and MSRV checks.

The new macro crate is inserted before `amari-rewrite` in publication order.
`amari-discovery` remains after all direct dependencies. A separate MSRV job
checks Rust 1.85 without altering existing matrix/aggregate names. A dedicated
45-minute vendored-Z3 all-feature job caches the complete Cargo `target`
directory (including `z3-sys` CMake outputs) with compiler and lockfile keys;
cold and warm times are measured before finalizing that timeout. Vendored Z3
build time and Candle/research feature binary impact are documented rather
than hidden.

## Explicit non-goals

Even under the research-heavy profile, 0.24 does not include:

- GPU/CUDA/Metal Candle backends;
- automatic execution of project code, build scripts, solver executables, or
  network providers;
- proof that bounded joinability implies global confluence;
- proof of non-termination when LPO cannot orient a system;
- unrestricted Knuth–Bendix completion;
- higher-order, associative-commutative, conditional, or dependent rewriting;
- end-to-end differentiable symbolic execution through arbitrary Rust types;
- WASM bindings for Z3, Candle, or the new analysis modules.
