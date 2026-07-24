# amari-rewrite 0.25.0 — Decision Record

- Date opened: 2026-07-09
- Expanded inverse scope approved: 2026-07-24
- Status: **Approved — comprehensive bounded rewrite and inverse-rewrite research**
- Design: `2026-07-24-amari-rewrite-inverse-expansion-design.md`
- Implementation plan: `2026-07-24-amari-rewrite-inverse-expansion-implementation-plan.md`

## Context

`amari-rewrite` shipped in 0.23.0 with stable ARS, TRS, bounded predecessor
search, anti-unification, and positive-example inference, plus trait/summary
scaffolds for macros, neural, SMT, and network research. The first expansion
plan treated inverse rewriting as already complete because the stable
`BackwardSearch` API existed. That was too narrow: inverse rewriting is a
primary downstream interest, while the current iterator does not model
existential information, constraints, provenance, exact reversibility,
language preimages, or guided backward reasoning.

The expansion moves from 0.24.0 to 0.25.0 so the completed discovery and
holographic work can ship independently. The 0.25 cycle retains every
previously approved rewrite feature and adds a first-class inverse track.

## Decision 1: release sequence

- 0.24.0 ships `amari-discovery` and additive
  `BindingAlgebra::superpose`/`scale` only.
- 0.25.0 ships the comprehensive rewrite and inverse-rewrite expansion.
- 0.26.0 contains GPU/current-`wgpu`/Borsalino modernization.
- Explicitly sequenced later minor milestones shift one version later unless a
  later decision record changes them.

Rewrite research is no longer a 0.24 release gate. A 0.24 version bump still
is not a release without catalog, package, publication, registry-install, npm,
and tag evidence.

## Decision 2: additive stable core

Existing 0.23 public ARS/TRS/inverse/synthesis behavior remains additive. The
legacy `inverse::BackwardSearch` and `inverse::predecessors` APIs remain
available and retain their documented bounded iterator behavior. New inverse
semantics use explicit result types rather than silently changing legacy
outputs.

Default symbolic code remains `no_std + alloc` capable. Heavy research
backends are opt-in and imply `std`.

## Decision 3: constrained relational inverse foundation

The canonical inverse abstraction is a first-order constrained rewrite
relation, not a function from outputs to inputs.

A checked forward rule compiles to a `BackwardClause`. Backward application
freshens variables, unifies the rule RHS with a target, and produces a
`SymbolicPredecessor` containing:

- predecessor term pattern;
- existential variables;
- normalized equality/disequality constraints;
- substitution/unifier;
- rule/path provenance;
- resource observations and authority state.

Only declarative, serializable first-order data belongs in this core. No user
closure, shell, provider, project, filesystem, or network authority is added.

## Decision 4: information loss and exact residual replay

Information erased by a forward rule becomes explicit existential data. For
`erase(X) -> zero`, target `zero` has symbolic predecessor `erase(?x)`; it is
not presented as a recovered concrete input. Callers may provide a bounded
finite `GroundingDomain` to enumerate concrete instantiations.

For exact step reversal, forward execution may emit an automatic typed
`RewriteResidual` containing rule identity, position, and only bindings not
recoverable from the RHS. Residual replay must reconstruct the exact prior
term. Any target/source/rule/path/binding hash mismatch is a hard authority
error and returns no reconstruction. The engine checks separately:

1. relational soundness: every grounded predecessor rewrites to the target;
2. residual round trip: backward replay of a forward step reconstructs its
   source.

No opaque custom complement callbacks ship in 0.25.

## Decision 5: backward and bidirectional reasoning

Add bounded symbolic `BackwardExplorer` and `BidirectionalExplorer` APIs.
Bidirectional frontier meetings use unification plus constraint satisfiability,
not raw term equality. Successful outcomes contain replayable derivations with
rule/path/substitution/residual evidence.

Complete-within-limits mode never prunes a generated valid transition. It may
claim exhaustion only for a certified finite search space or exact language
exclusion. Otherwise limits produce typed partial outcomes with retained
frontiers.

An explicit approximate heuristic-pruning mode may use beam/top-k pruning. It
records dropped-state counts and scorer/configuration hashes and can never
claim exhaustive, unreachable, or exact authority.

## Decision 6: regular tree-language preimages

Add a canonical validated bottom-up finite tree automaton implementation over a
ranked alphabet. Also expose a fully supported regular tree grammar API with
checked, lossless conversion and round-trip parity; algorithms remain
automaton-canonical.

The public language surface includes validation, membership, emptiness,
witness extraction, union, intersection, determinization, canonicalization,
minimization where valid, and strict state/transition/rank/term/byte limits.

## Decision 7: exactness requires a closure-theorem gate

The first language-preimage cohort is a research spike. It must cite the exact
recognizability-preservation theorems, define executable syntactic classifiers,
and test each construction against bounded concrete relational semantics
before public exactness claims stabilize.

Ground systems are the conservative candidate baseline. Broader linear,
monadic, right-ground, or other classes are included only when the cited
theorem's preconditions exactly match the implemented classifier. If the gate
approves no exact class, exact tasks are replanned around lower/upper/partial
authority rather than bypassed. Unsupported systems are typed outcomes, not
silently approximated exact results.

## Decision 8: lower/upper approximation contract

Outside certified exact fragments, the engine may return:

- a replay-validated lower regular language;
- a sound upper regular language only when every true predecessor is proved to
  be included;
- retained frontier/evidence when no upper proof exists.

Membership is `Proven`, `Possible`, or `Excluded` when both bounds exist.
Refinement monotonically grows the lower language and shrinks the upper
language. Canonically equal independently certified bounds upgrade to exact.
Linearization, widening, or state merging may be used only with documented
soundness obligations; an absent upper bound is preferable to an unsound one.

## Decision 9: proc-macro architecture

Create publishable `amari-rewrite-macros`, re-exported by `amari-rewrite` behind
stable feature `macros`. It owns:

- `#[derive(Rewritable)]` with explicit child fields;
- checked `term!` and `rule!` syntax;
- checked relational/bidirectional syntax after relation types stabilize.

Expansions resolve renamed crates hygienically, use fully qualified TRS paths,
and never call unchecked constructors or hidden `expect`.

## Decision 10: analysis and completion

Implement first-order unification with occurs check, critical pairs, bounded
joinability, left-linearity-aware local-confluence reports, lexicographic path
ordering, and bounded Knuth–Bendix completion.

LPO success is a sound termination certificate; failure is unknown. Callers
must supply a strict total precedence over every used ranked symbol; partial or
cyclic precedence is rejected rather than silently extended. Ordinary
critical-pair local-confluence certification requires left-linear rules.
Completion reports `Complete` only with terminating orientation and sound
confluence preconditions; otherwise useful results remain partial/unknown.

Inverse analysis additionally reports information loss, ambiguity, residual
requirements, and structural reversibility. Completion preserves generated
equational theory, not automatically the original directed reachability
relation. Completed rules may generate candidates, but positive witnesses
replay with original rules and negative/exhaustion authority requires a
separate directed relation-equivalence certificate.

## Decision 11: synthesis and negative examples

Preserve `infer_rule`/`infer_rules`. Add deterministic bounded specialization
that detects negative coverage, chooses discriminating paths, partitions
positives, infers specialized rules, and validates all returned rules.

Extend examples to inverse/bidirectional traces. Proposed backward clauses or
bidirectional rules require forward replay and residual-law validation.
Optional SMT evidence may refute or support candidates; solver unknown never
becomes proof.

## Decision 12: Candle, network, and holographic guidance

Provide all three optional inverse frontier scorers:

- exact `candle-core`/`candle-nn` 0.11.0 CPU pairwise scorer and bounded AdamW
  training;
- `amari-network` geometric frontier scorer;
- deterministic `amari-holographic` recall scorer over inverse goals and
  replay-validated traces.

Guidance consumes only transitions created by the symbolic engine. In complete
mode it orders only. In approximate mode it may prune with explicit loss of
completeness authority. Completed fixed-seed training is deterministic;
deadline-truncated partial training is marked non-replayable.

No GPU training backend ships in 0.25.

## Decision 13: concrete SMT backend and MSRV

After explicit registry metadata/feature preflight, use exact vendored
in-process `z3 =0.20.2` behind experimental feature `smt`.
Do not use an external solver process or `gh-release` build-time binary
download. Results distinguish proved, refuted, and unknown, with bounded
sanitized evidence.

Use exact `candle-core =0.11.0` and `candle-nn =0.11.0`. Raise workspace MSRV
to Rust 1.85 only after the exact dependencies compile under a dedicated MSRV
check. Vendored Z3 gets a separately cached/time-bounded CI job.

## Decision 14: discovery integration

Every implementation cohort updates generated structural and curated semantic
discovery authority. Add rich registered process-isolated probes for bounded
pure symbolic operations:

- relation classification and symbolic predecessor steps;
- residual round trips;
- backward/bidirectional search;
- automaton/grammar conversion and membership;
- certified/bounded language preimages;
- analysis, completion, and refinement.

Do not expose Candle training/checkpoints, Z3 solving, arbitrary model loading,
or holographic external data as discovery probes. The installed command keeps
its existing generic `probe run` authority rather than adding a dedicated
`amari inverse` command.

## Decision 15: delivery and release acceptance

Deliver 0.25 as moderate grouped PR cohorts. Every canonical task retains a
RED→GREEN commit/checkpoint and focused tests; each cohort receives independent
review and discovery acceptance. Critical/Important findings block merge.

The 0.25 release is not complete when features merge. It still requires
workspace version sync, final catalog regeneration, source/package matrices,
publication in dependency order (including the new macro crate), verified
archive installation, crates.io installation, npm/WASM gates, and tag evidence.

## Rejected alternatives

- Treating the existing predecessor iterator as a complete inverse model.
- Requiring inverse rewriting to be functional.
- Silently treating existential holes as concrete recovered inputs.
- Letting learned systems invent transitions or make proof claims.
- Claiming unrestricted TRS preimages are always regular.
- Returning an upper approximation without a sound containment argument.
- Making regular tree grammars and automata separate algorithmic authorities.
- User-defined opaque lens callbacks in the initial release.
- GPU neural training in the rewrite cycle.
- Holding completed discovery/holographic work until rewrite research finishes.
