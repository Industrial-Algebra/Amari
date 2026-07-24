# amari-rewrite 0.25 Comprehensive Rewrite and Inverse Expansion Design

- Date: 2026-07-24
- Status: Approved
- Decision record: `2026-07-24-amari-rewrite-0.25-decisions.md`
- Preserves: stable 0.23 contracts in `2026-05-10-amari-rewrite-design.md`

## Purpose

Amari 0.25 turns `amari-rewrite` from a stable symbolic foundation with
research scaffolds into a comprehensive, bounded rewrite platform. The release
retains the previously approved macro, analysis/completion, synthesis, Candle,
SMT, network, and discovery work and makes inverse rewriting a first-class
architectural concern.

“Inverse” means a relation over possible predecessors, not an assumed
functional inverse. The design supports three primary outcomes:

1. recover and characterize possible source terms for a result;
2. perform goal-directed backward and bidirectional reasoning;
3. execute exact reversible steps when typed residual information is retained.

It additionally supports regular-language preimages for verification and
reachability, with exactness claims limited to proved closure fragments and
explicit lower/upper approximations elsewhere.

## Release boundary

This work is 0.25.0 scope. It does not block the 0.24 discovery/holographic
release. GPU/current-`wgpu`/Borsalino work moves to 0.26.0.

The 0.25 branch starts only after 0.24.0 is published and `develop` is synced
through the release backmerge. Aggregate 0.25 versioning occurs after all
rewrite cohorts merge; no feature PR changes the workspace version early.

## Compatibility and stability

The 0.23 APIs remain stable and additive:

- `Rewritable`, ARS and TRS types;
- `Term`, `Rule`, `TermSystem`, substitutions, matching, and normalization;
- `inverse::predecessors` and `inverse::BackwardSearch`;
- anti-unification, `infer_rule`, and `infer_rules`.

The legacy backward iterator keeps its current behavior. New APIs never change
what its `Iterator<Item = Term>` means.

Planned 0.25 stability tiers:

- **stable:** macro syntax; constrained relation data model; symbolic one-step
  predecessors; automatic residual replay; exact bounded backward/bidirectional
  outcome contracts; validated tree automaton/grammar core and conversions;
- **experimental:** recognizability classifiers and unbounded language
  preimages; lower/upper abstraction refinement; completion; learned/network/
  holographic guidance; concrete Z3 backend; negative/inverse synthesis.

Promotion requires documented public APIs, hard-limit tests, property oracles,
independent review, and discovery coverage.

## Crate and feature architecture

Add one publishable proc-macro crate and keep algorithms in `amari-rewrite`:

```text
amari-rewrite-macros/       proc macros only
amari-rewrite/
  src/
    relation/               constrained first-order relations
    inverse/                legacy API + symbolic/backward/bidirectional APIs
    reversible/             residual-backed exact replay
    language/
      automaton.rs           canonical bottom-up tree automata
      grammar.rs             regular tree grammar API
      preimage.rs            certified/approximated predecessor languages
      classify.rs            closure-fragment classifiers
    analysis/               unification, critical pairs, confluence, LPO
    completion/             bounded Knuth-Bendix
    synthesis/              positive/negative/inverse refinement
    neural/                 Candle encoding, ranking, training, checkpoints
    smt/                    vendored Z3 translation/certificates
    network/                geometric and hybrid frontier guidance
    holographic/            deterministic inverse-trace recall guidance
```

Feature intent:

```toml
default = ["std"]
std = []
serialize = ["dep:serde"]
macros = ["dep:amari-rewrite-macros"]
completion = []
neural = ["std", "dep:candle-core", "dep:candle-nn"]
smt = ["std", "dep:z3"]
network = ["std", "neural", "dep:amari-network"]
holographic-guidance = ["std", "dep:amari-holographic"]
```

The constrained relation, reversible, inverse search, and tree-language cores
use `alloc` and remain available without default features. Heavy backend
features imply `std`. `amari-holographic` and `amari-network` do not depend on
`amari-rewrite`, so the optional guidance edges create no cycle.

Pinned research dependencies begin with:

- `candle-core = "=0.11.0"` and `candle-nn = "=0.11.0"`, CPU only;
- `z3 = { version = "=0.20.2", default-features = false, features = ["vendored"] }`;
- workspace `sha2` with default features disabled as a small `no_std` core
  dependency for canonical relation, language, replay, and research-backend
  evidence identity;
- `syn`, `quote`, `proc-macro2`, and `proc-macro-crate` for macros;
- `trybuild` for macro UI contracts.

Rust 1.85 is adopted only after exact dependency compilation succeeds in a
separate MSRV job.

## Constrained relation model

### Variables and freshening

`LogicVar` is distinct from display-level TRS variable names. Every backward
application allocates variables from a deterministic query-scoped namespace.
Canonical serialization renumbers by first structural occurrence, preventing
caller names or traversal accidents from changing hashes.

A `BackwardClause` contains a freshenable LHS/RHS relation compiled from a
checked `trs::Rule`. It cannot be constructed from a rule whose RHS contains a
variable absent from the LHS.

### Constraints

The core constraint theory is first-order syntactic equality/disequality:

```rust
pub enum TermConstraint {
    Equal(Term, Term),
    NotEqual(Term, Term),
}

pub struct ConstraintSet { /* canonical private representation */ }
```

Construction validates term limits, applies occurs-check unification, composes
substitutions, removes tautologies, rejects contradictions, and sorts/dedups
residual disequalities. Arithmetic or user-defined theory predicates are not
silently embedded. Future theories require typed extensions.

`ConstraintOutcome` distinguishes solved, satisfiable-with-residuals,
unsatisfiable, limit-reached, and unsupported-theory states. Optional Z3 may
validate the same first-order authority but cannot redefine core semantics.

### Symbolic predecessors

Backward application unifies a fresh rule RHS with the target subterm. A
successful transition returns:

```rust
pub struct SymbolicPredecessor {
    pub term: Term,
    pub existentials: Vec<LogicVar>,
    pub constraints: ConstraintSet,
    pub substitution: Substitution,
    pub provenance: BackwardProvenance,
    pub resources: RelationResources,
}
```

`BackwardProvenance` includes stable rule identity, target position, freshening
namespace, and authority hashes. It does not contain source text, paths, raw
backend diagnostics, or project data.

Every result must satisfy a forward-replay oracle after any grounding that
satisfies its constraints.

### Grounding

`GroundingDomain` is a caller-supplied finite ranked symbol domain with maximum
term depth/count. Grounding enumerates existential substitutions in canonical
size/lexicographic order and validates constraints before emitting terms.
Empty or oversized domains fail before allocation. Grounding is optional;
symbolic results are complete relational values.

## Reversible steps and typed residuals

A normal rewrite may erase information. Exact reversal therefore uses a
residual produced during forward execution:

```rust
pub struct RewriteResidual {
    pub rule_id: RuleId,
    pub position: TermPath,
    pub erased_bindings: Vec<(LogicVar, Term)>,
    pub source_hash: Sha256Digest,
    pub target_hash: Sha256Digest,
}

pub struct ReversibleStep {
    pub target: Term,
    pub residual: RewriteResidual,
    pub transition: ForwardTransition,
}
```

The necessary erased bindings are derived from LHS variables absent from the
RHS. Rule identity disambiguates rules with the same RHS; position identifies
the replaced context. For concrete forward execution, matching the RHS against
the target recovers all RHS variables. Residual replay reconstructs the LHS
subterm and replaces it at the recorded position.

Replay first validates digest syntax, rule identity, position, target hash,
binding schema, and constraint compatibility. It then reconstructs the source
and compares its canonical digest with `source_hash` before returning it. Any
mismatch is a hard typed authority error; replay never returns a degraded or
warning-only reconstruction and never reflects raw untrusted input.

`BidirectionalRule` and `BidirectionalSystem` wrap checked forward rules and
derived backward/residual contracts. They support:

- forward step with optional residual;
- symbolic predecessor relation;
- exact backward replay with residual;
- composition through typed derivations;
- structural reversibility/information-loss reports.

No arbitrary custom complement closure ships in 0.25.

## Backward search

`BackwardExplorer` searches `SymbolicState { term, constraints }` values. It
deduplicates canonical alpha-equivalent states, not just raw terms. Each edge
records rule/path/unifier/constraint deltas.

`InverseSearchConfig` caps depth, states, transitions, term nodes/depth,
constraint count, grounding count, operations, retained frontier bytes, and
trace bytes. Caller values above fixed ceilings are rejected.

Exact-ordering modes include canonical BFS and deterministic cost search over
symbolic costs. Outcomes are values:

```rust
pub enum BackwardSearchOutcome {
    Witness(BackwardDerivation),
    Exhausted(CertifiedExhaustion),
    Partial(BackwardFrontier),
    Approximate(ApproximateSearchEvidence),
    Unsupported(UnsupportedRelation),
}
```

`Exhausted` requires a certified finite domain/frontier or exact regular-
language exclusion. Reaching a depth/node/operation ceiling is `Partial`, never
unreachable.

## Bidirectional search

`BidirectionalExplorer` expands forward from a source language/state and
backward from a goal language/state. Frontier entries meet when terms unify and
combined constraints are satisfiable. A meet is not accepted until the full
forward/backward derivation replays through the original `TermSystem`.

Results contain ordered forward transitions, ordered backward transitions,
meeting substitution/constraints, residual evidence when available, and all
resource observations. Search remains deterministic for fixed configuration
and guidance identity.

## Guidance and explicit approximation

The symbolic engine is the only transition generator. Guidance receives a
bounded list of already valid candidate transitions.

### Symbolic score

Every candidate has transparent minimization dimensions: depth, term growth,
constraint growth, unresolved existentials, residual cost, branch factor,
cycle/novelty state, and stable rule priority.

### Candle score

A structural encoder creates fixed-schema features for target, predecessor,
constraint summary, provenance, and source/goal context. A CPU MLP produces a
scalar rank score. Pairwise examples come from replay-validated successful
backward/bidirectional traces. Training uses bounded AdamW, fixed seeds,
finite-value checks, typed completed/partial outcomes, and safe-tensor
checkpoint validation.

Completed fixed-seed runs are deterministic. Wall-deadline truncation is
explicitly non-replayable and records no wall timestamp.

### Geometric-network score

A bounded inverse search graph backed by `GeometricNetwork<3,0,0>` stores
symbolic states as nodes and valid transitions as edges. Versioned
`InverseStateEmbeddingV1` maps a state to the eight Cl(3,0,0) blade
coefficients using fixed normalized descriptors: log node count, term depth,
variable ratio, symbol/arity diversity, constraint density, existential ratio,
search/provenance depth, and a signed SHA-256 content sketch. Normalizers use
hard ceilings, reject non-finite values, and are frozen by golden tests.
Colliding embeddings never merge symbolic states—the canonical state digest is
still node identity—and geometric distance is documented as a heuristic, not a
structure-preserving isometry or correctness claim. Distance, novelty,
branching, provenance, and target proximity remain separate explicit score
components.

### Holographic recall score

A frozen deterministic encoder maps inverse goals and replay-validated traces
to an additive holographic representation. `BindingAlgebra::superpose` combines
candidate evidence. Recall identity includes dimensions, seed, encoder version,
trace hashes, and catalog/tool identity. It never loads arbitrary project data
or external memories implicitly.

### Search modes

- `CompleteWithinLimits`: guidance changes ordering only; no candidate is
  dropped before a resource limit.
- `HeuristicPruning`: explicit beam/top-k pruning may drop candidates; outcomes
  are always `Approximate`, include dropped counts and scorer/config hashes,
  and cannot prove unreachable/exhaustive.

## Tree automaton core

The canonical language representation is an epsilon-free nondeterministic
bottom-up finite tree automaton (NFTA) over a validated ranked alphabet:

```rust
pub struct RankedSymbol { pub symbol: Symbol, pub arity: u16 }
pub struct TreeState(/* canonical ID */);
pub struct TreeTransition { pub symbol: RankedSymbol, pub children: Vec<TreeState>, pub parent: TreeState }
pub struct TreeAutomaton { /* private validated canonical data */ }
```

Constructors enforce symbol-rank consistency, known states, exact child arity,
nonempty final-state policy where required, deterministic canonical ordering,
and hard state/transition/rank/byte limits before allocation.

Stable operations:

- term membership with accepting run evidence;
- emptiness and smallest canonical witness;
- union and intersection;
- determinization under subset/state ceilings;
- completion/complement only for deterministic complete automata;
- reachable/co-reachable trimming;
- canonical renumbering and deterministic bytes;
- Myhill–Nerode context-equivalence minimization for trimmed deterministic
  complete automata, yielding the unique minimal equivalent automaton up to
  state renaming and then canonical renumbering;
- bounded accepted-term enumeration for testing/grounding.

Operations return typed partial/limit states rather than truncated automata
presented as complete.

## Regular tree grammar

`RegularTreeGrammar` is a full authored API:

```rust
pub struct Nonterminal(/* canonical name/ID */);
pub struct GrammarProduction { pub lhs: Nonterminal, pub symbol: RankedSymbol, pub children: Vec<Nonterminal> }
pub struct RegularTreeGrammar { /* validated productions/start symbols */ }
```

It supports checked construction, parsing/rendering of a fixed grammar syntax,
membership/witness convenience methods through checked conversion, canonical
serialization, and lossless conversion to/from `TreeAutomaton`.

Round trips preserve language and canonical bytes after documented
normalization. The grammar does not maintain a second implementation of union,
intersection, determinization, or preimage algorithms.

## Closure-theorem research gate

Before language-preimage APIs stabilize, a dedicated research cohort produces:

1. cited recognizability-preservation theorems from primary literature;
2. a matrix of exact one-step, finite-horizon, and reflexive-transitive
   preimage guarantees by TRS syntactic class;
3. executable classifiers whose checks exactly match theorem preconditions;
4. proof sketches connecting Amari term/rule semantics to each theorem;
5. bounded concrete differential oracles and adversarial counterexamples;
6. an ADR approving the exact matrix.

Ground TRSs are the conservative candidate baseline, not a pre-approved claim.
Left-linear, monadic, right-ground, or other classes are not claimed until the
gate verifies the specific construction. If the gate approves no exact class,
the exact-class tasks are removed/replanned and the public API ships only
soundly justified lower/upper/partial results; no schedule pressure may bypass
the gate. The implementation may ship fewer exact classes than the research
investigates.

## Language preimages

`LanguagePreimageEngine` receives a checked `TermSystem`, target automaton, and
strict config. It computes one-step, bounded-horizon, or supported unbounded
predecessor languages.

```rust
pub enum PreimageLanguageResult {
    Exact { language: TreeAutomaton, certificate: ExactnessCertificate },
    Bounds { lower: TreeAutomaton, upper: TreeAutomaton, certificate: BoundsCertificate },
    Partial { lower: TreeAutomaton, upper: Option<TreeAutomaton>, frontier: PreimageFrontier, reason: PartialReason },
    UnsupportedClass { classification: TrsClassification },
}
```

Exactness certificates bind the original system/language hashes, classifier,
theorem/construction ID, horizon, limits, and resulting language hash. A
certificate is evidence produced by a reviewed algorithm, not a machine-
checked theorem proof.

### Lower bound

The lower language contains only replay-validated predecessors. It may be built
from finite witnessed terms/derivations and monotonically enlarged. Every
witness extracted from it must replay to the target language.

### Upper bound

An upper language is emitted only by an abstraction with a documented proof
that it contains all concrete predecessors for the claimed horizon/class. Any
linearization, state merging, or widening records exactly which abstraction
steps occurred. If sound containment cannot be established, `upper` is absent.

### Membership authority

For bounded query term `t`:

- `Proven`: `t` is accepted by lower and has/reconstructs a valid witness;
- `Possible`: `t` is in upper but not yet lower;
- `Excluded`: `t` is outside a sound upper;
- `Unknown`: no sound upper or a limit prevents classification.

Refinement may add concrete witnesses to lower or split/refine upper states.
It must never shrink lower or grow upper. Canonically equal independently
certified bounds become `Exact`.

## Analysis and completion

### Unification

Implement deterministic first-order unification with occurs check, canonical
binding order, checked substitution composition, and strict term/operation
limits. This unifier is shared by critical pairs, inverse frontier meeting,
constraints, and synthesis.

### Critical pairs and confluence

Rules are renamed apart before non-variable overlap enumeration. Generated
pairs include complete provenance. Joinability uses bounded bidirectional
search over real successors.

Ordinary critical-pair certification may report local confluence only for
left-linear LHS patterns after exhaustive pair generation and successful
joinability. Non-left-linear systems remain unknown unless a separately proved
parallel-critical-pair extension is added.

### LPO

The caller supplies one strict total ordering list covering every ranked symbol
used by the checked rules. Validation rejects missing/duplicate symbols and
any relation-form precedence that is cyclic or non-total; the engine does not
silently extend a partial order. That precedence drives bounded lexicographic
path ordering. Every rule must strictly decrease before `ProvedTerminating`;
otherwise the result is unknown with reasons.

### Completion

Feature `completion` adds bounded Knuth–Bendix orientation, simplification,
critical-pair processing, duplicate suppression, and traces. `Complete`
requires terminating orientation, left-linearity, and sound local-confluence
certification. The trace establishes preservation of the generated equational
theory, not equality of the original and completed *directed reachability*
relations. Completion-derived rules may guide candidate generation, but every
positive inverse witness must replay with original directed rules and no
negative/exhaustion claim transfers without a separate directed
`RelationEquivalenceCertificate`. Optional SMT evidence may support that
certificate but is not substituted for the required derivations. Limit,
unorientable, or non-left-linear outcomes remain partial or failed without
false global claims.

### Inverse analysis

`InverseAnalyzer` reports per-rule/system:

- erased variables and required residual schema;
- RHS overlap/ambiguity classes;
- structurally lossless vs existential backward behavior;
- finite-domain branching estimates;
- known exact language-preimage classifier;
- unsupported/unknown reasons.

## Synthesis and refinement

Preserve positive-example APIs. Add strict configs and typed outcomes for:

- negative-example specialization via discriminating paths and partitions;
- inverse examples `(possible_source, target)`;
- residual examples `(source, target, residual)`;
- candidate backward clauses and bidirectional systems.

Every returned forward rule is checked with `Rule::new`. Every inverse
candidate must forward-replay supplied positives, reject supplied negatives,
and satisfy residual laws when claimed reversible. SMT integration is opt-in;
unknown evidence does not promote a candidate.

## Macros

Create `amari-rewrite-macros` and re-export behind `macros`:

```rust
#[derive(amari_rewrite::Rewritable)]
enum Expr { /* explicit #[rewritable(child)] fields */ }

let t = term!(add(zero, X));
let r = rule!(add(zero, X) => X);            // RewriteResult<trs::Rule>
let rel = relation!(add(zero, X) <=> X);     // checked relational value
```

Identifier and string symbol spellings are equivalent. `rule!` resolves the
fully qualified TRS rule, not ARS `Rule`. Relational syntax expands only to
checked declarative constructors. Trybuild fixtures lock diagnostics, hygiene,
renamed-crate support, and unsupported syntax.

## SMT

Feature `smt` preserves `RewriteSolver` and adds a bounded vendored
`Z3RewriteSolver`. It uses one uninterpreted term sort, function declarations
per ranked symbol, and universally quantified checked rule equations.

Applications include:

- term equivalence under rewrite axioms;
- residual equality/disequality satisfiability;
- selected round-trip obligations;
- completion/refinement candidate evidence.

`unsat` inequality means proved equivalent under supplied axioms; `sat` means a
bounded sanitized refutation/model summary; timeout/quantifier unknown remains
unknown. Solver/tactic/parameter selection is fixed, versioned, and included
with query/rule/signature/config hashes and Z3 version in canonical
certificates. Raw model text, diagnostics, paths, and native build details are
not public evidence.

## Resource authority

Fixed ceilings are associated constants and cannot be raised by callers:

| Area | Hard ceilings |
| --- | --- |
| Terms/constraints | 4,096 nodes/term, depth 64, 4,096 constraints, 1,000,000 operations |
| Backward/bidirectional search | 65,536 states, 262,144 transitions, depth 64, 64 MiB retained evidence |
| Grounding | 256 symbols, rank 16, depth 16, 65,536 emitted terms |
| Tree automata | 4,096 states, 65,536 transitions, rank 16, determinized subsets 65,536 |
| Language preimage | horizon 64, 4,096 saturation iterations, 1,000,000 operations, 64 MiB evidence |
| Critical pairs/joinability | 4,096 pairs, 65,536 joinability states, 1,000,000 operations |
| Completion | 256 rules, 4,096 pairs/iterations, 1,000,000 operations |
| Refinement | 4,096 examples, 256 candidates/partitions/rules, 1,000,000 operations |
| Neural | width 64, hidden 256, 65,536 examples, 10,000 epochs, 16,777,216 tensor elements, 64 MiB checkpoint, 5 minutes |
| SMT | 256 rules, 512 symbols/variables, 65,536 term nodes, 4,096 assertions, 30 seconds |
| Search graph | 4,096 graph nodes, 65,536 graph edges, depth 64, 1,000,000 operations |

Limits are validated before allocation, recursion, native solver/model setup,
or worker execution. Callers and discovery descriptors may only tighten them.

## Errors and outcomes

`RewriteError` gains categorized variants for invalid configuration, authority
limit, inconsistent constraints, unsupported relation/class, malformed
automaton/grammar, residual drift, backend failure, non-finite tensor,
checkpoint incompatibility, and replay drift.

Expected domain states are values, not errors: not-unifiable, no witness,
partial frontier, unsupported exact class, unknown SMT result, approximate
language/search, and non-reversible rule classification.

Serialized authority-bearing DTOs reject unknown fields recursively.
Untrusted symbols/terms are represented by bounded canonical hashes/categories
in errors; raw source text, absolute paths, solver diagnostics, model files,
and worker stderr are never reflected.

## Discovery integration

Every cohort updates structural catalog generation and curated semantic records.
Planned capability families include:

- rewrite macros and checked relation syntax;
- unification, critical pairs, LPO, confluence, completion;
- constrained predecessors and inverse analysis;
- residual-backed reversible steps;
- backward and bidirectional reasoning;
- tree automata and regular tree grammars;
- exact/bounded/approximated language preimages;
- negative/inverse synthesis;
- neural, network, and holographic guidance;
- solver-backed equivalence and round-trip validation.

Safe bounded symbolic functions gain registered process-isolated probes for:

- one-step symbolic predecessor relation;
- relation/reversibility classification;
- residual forward/backward round trip;
- bounded backward/bidirectional witness search;
- automaton/grammar conversion, membership, emptiness, and witness;
- certified/bounded language preimage;
- critical pairs/joinability/LPO/completion/refinement.

Probe DTOs reject unknown fields at every level, use limits far below library
maxima, and contain no path, executable, argument, shell, provider, model,
checkpoint, project, environment, or network authority. Candle training,
checkpoint loading, Z3 solving, and external holographic datasets remain
catalog-only, non-probe capabilities.

## Testing strategy

Strict RED→GREEN applies per canonical task. Required oracle families:

1. **Relational soundness:** each grounded symbolic predecessor performs the
   recorded forward transition to the target.
2. **Residual law:** forward-with-residual then backward-replay reconstructs
   exact source bytes/term.
3. **Search replay:** every witness derivation replays through the original
   system; approximate/exhaustive authority is correct.
4. **Automaton/grammar parity:** checked round trips preserve membership,
   emptiness, witnesses, and canonical normalization.
5. **Preimage containment:** bounded concrete enumeration is contained in every
   claimed upper and contains every lower witness; exact results agree both
   ways.
6. **Analysis laws:** MGU application equality, critical-pair provenance, LPO
   irreflexivity/transitivity on generated terms, completion postchecks.
7. **Guidance authority:** scorers never create transitions; complete mode
   preserves candidate sets; pruning always downgrades authority.
8. **Discovery parity:** process-isolated results equal direct API results and
   obey framing/output/provenance limits.

Property/fuzz suites cover alpha-renaming, occurs checks, disequality
normalization, erased variables, nested residual drift, cyclic systems,
automaton rank errors, determinization blow-up, malformed grammars,
linearization/widening soundness fixtures, hostile serialized DTOs, and exact
limit boundaries.

## CI, packaging, and publication

- separate Rust 1.85 MSRV job; existing aggregate names remain unchanged;
- dedicated 45-minute vendored-Z3 job with complete target/CMake cache keyed by
  compiler and lockfile;
- default, no-default, serialize, macros, completion, neural, smt, network,
  holographic-guidance, combined-guidance, and all-feature matrices;
- no Z3/Candle/holographic research backend in WASM targets;
- sequential expensive feature runs and measured cold/warm evidence;
- `amari-rewrite-macros` publishes before `amari-rewrite`;
- `amari-holographic`, `amari-network`, and other direct dependencies publish
  before `amari-rewrite`; `amari-discovery` remains after rewrite and all its
  direct dependencies;
- verified package/archive/registry installations are mandatory before 0.25 is
  called shipped.

## Non-goals

0.25 does not provide:

- unrestricted exact regular preimages for arbitrary TRSs;
- a proof assistant or machine-checked theorem certificates;
- arbitrary arithmetic/theory constraints in the core relation;
- opaque user closures as serialized inverse relations;
- learned transition generation or proof authority;
- GPU/CUDA/Metal training;
- external solver processes or downloaded native solver binaries;
- arbitrary project/provider/shell/network execution;
- new WASM rewrite bindings;
- GPU/current-`wgpu`/Borsalino modernization.
