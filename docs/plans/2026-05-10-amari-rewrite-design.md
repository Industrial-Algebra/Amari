# amari-rewrite 0.23.0 Design

Date: 2026-05-10
Status: implemented for the stable 0.23 core
0.24 continuation: `2026-07-23-amari-rewrite-research-expansion-design.md` (additive; the stable contracts below remain authoritative)
Source context: `/home/lucien/working/industrial-algebra/IA-documents/Amari/rewrite/rewrite-ideation-session.md`

## Goal

`amari-rewrite` is a foundational rewrite-systems crate for Amari. It should be a toolkit for building and exploring rewrite systems over Amari-owned or user-owned data structures, not a wrapper around an external e-graph engine and not a rewrite engine that takes over existing data.

The 0.23.0 target is a broad experimental toolkit with a stable symbolic core and explicitly feature-gated research surfaces.

## Stability tiers

Stable/default 0.23.0 surface:

- `rewritable`: path/subterm traversal for user-owned data
- `ars`: abstract rewrite systems over arbitrary `T: Rewritable`
- `trs`: first-order terms, variables, substitutions, matching, and rewrite rules
- `inverse`: bounded predecessor/backward search
- `synthesis`: anti-unification and basic rule inference from examples

Experimental feature-gated surfaces:

- `macros`: future `derive(Rewritable)` and term/rule helper macros
- `smt`: placeholder interfaces for solver-backed validation and synthesis
- `neural`: trait-level scaffolding for differentiable/neural rewrite rules
- `network`: optional `amari-network` bridge for graph/geometric rewrite guidance

The default crate should remain lightweight and inspectable. It should not depend on `egg`, `egglog`, neural tensor frameworks, or SMT solvers in the default path.

## Crate layout

```rust
pub mod rewritable;
pub mod ars;
pub mod trs;
pub mod inverse;
pub mod synthesis;

#[cfg(feature = "macros")]
pub mod macros;

#[cfg(feature = "smt")]
pub mod smt;

#[cfg(feature = "neural")]
pub mod neural;

#[cfg(feature = "network")]
pub mod network;

pub mod prelude;
```

The prelude should include only everyday user-facing items: `Rewritable`, `Path`, `Strategy`, `System`, `Term`, `Rule`, and `Substitution`.

## Core abstraction

`Rewritable` keeps user-owned data at the center. `trs::Term` is useful and concrete, but it must not be mandatory for all rewriting.

```rust
pub trait Rewritable: Clone + PartialEq + core::fmt::Debug {
    fn children(&self) -> Children<'_, Self>;
    fn replace_at(&self, path: &Path, replacement: Self) -> RewriteResult<Self>;

    fn subterm(&self, path: &Path) -> Option<&Self> { /* provided */ }
    fn positions(&self) -> Vec<Path> { /* provided */ }
}
```

Avoid `Box<dyn Iterator>` in the public trait if a simple `Children<'a, T>` helper or slice-like adapter can keep implementations straightforward without heap allocation.

## ARS layer

The ARS layer models rewriting as abstract state transitions over any `T: Rewritable`.

Primary types:

```rust
ars::Rule<T>
ars::System<T>
ars::Strategy
ars::RewriteStep<T>
```

Initial strategy set:

```rust
Strategy::OuterFirst
Strategy::InnerFirst
Strategy::FirstRule
Strategy::All
```

Behavioral API:

- `apply_once(term, strategy)`
- `rewrite_steps(term, limit)`
- `normalize(term)`
- `normalize_with_limit(term, max_steps)`
- `normal_forms(term, max_depth)` for bounded exploration

`Strategy::All` should return possible one-step successors, not choose one.

## TRS layer

The TRS layer provides a concrete first-order term rewriting system on top of ARS.

Primary types:

```rust
trs::Term
trs::Variable
trs::Symbol
trs::Substitution
trs::Rule
trs::TermSystem
```

`trs::Rule { lhs, rhs }` should validate the usual TRS condition: variables appearing in `rhs` must also appear in `lhs`. Unchecked or experimental construction should be explicit.

Matching must be deterministic and substitution-consistent:

```text
match_pattern(add(0, X), add(0, s(0))) => { X ↦ s(0) }
match_pattern(f(X, X), f(a, b)) => None
```

## Inverse rewriting

Inverse rewriting is not a true functional inverse. It is bounded predecessor generation.

Proposed API:

```rust
inverse::predecessors(system, target)
inverse::BackwardSearch::new(system, target)
    .max_depth(...)
    .max_nodes(...)
    .strategy(...)
```

Rules can be explored backward by matching the original RHS and instantiating the original LHS. Search must be explicitly bounded and should deduplicate visited terms.

Use cases:

- backward reachability
- debugging rewrite traces
- finding candidate predecessors for synthesis
- exploring inverse TRS behavior

## Synthesis and anti-unification

Initial synthesis should be lightweight, symbolic, and honest about limitations.

Public functions:

```rust
anti_unify(a, b)
anti_unify_all(terms)
infer_rule(positive_examples)
infer_rules(positive_examples, negative_examples, config)
```

Anti-unification should compute a most-specific generalization for first-order terms. Rule inference should use positive examples first, then apply simple negative-example filtering or heuristic specialization. The 0.23.0 docs must state that negative-example specialization is heuristic, not complete.

Important tests:

- constants anti-unify to themselves
- different constants produce fresh variables
- nested examples such as `add(0, succ(0))` and `add(0, succ(succ(0)))` produce `add(0, succ(X))`
- the generated generalization can instantiate back to all positive examples
- non-linear patterns are handled consistently

## Experimental neural module

The `neural` feature should initially provide trait scaffolding only. It should not pick `burn`, `candle`, `ndarray`, or another tensor dependency in 0.23.0.

Example trait shape:

```rust
pub trait DifferentiableRule<State> {
    type Parameters;
    type Gradient;
    type Error;

    fn forward(&self, state: &State) -> Result<State, Self::Error>;
    fn loss(&self, predicted: &State, target: &State) -> Result<f64, Self::Error>;
}
```

The near-term purpose is to keep the architecture open for learned rewrite rules, neural inverse rewriting, and neural-guided strategies without coupling the stable symbolic crate to a research stack.

## Optional amari-network integration

`amari-network` should contribute to the experimental neural direction as an optional bridge, not as a default dependency.

Feature shape:

```toml
[features]
default = ["std"]
neural = []
network = ["dep:amari-network", "neural"]
```

Integration role:

- model rewrite search spaces as directed graphs where terms are nodes and rewrite steps are edges
- rank forward/backward search frontiers using graph/geometric heuristics
- represent rule embeddings, state embeddings, and rewrite trajectories for learned strategy selection
- support neural-guided inverse rewriting by prioritizing likely predecessors

Boundary: `amari-rewrite` core must not depend on `amari-network`. The optional `network` module is a strategy/guidance layer over the symbolic core.

## Experimental SMT module

The `smt` feature should define interfaces before integrating a concrete solver.

Example trait shape:

```rust
pub trait RewriteSolver {
    type Term;
    type Certificate;
    type Error;

    fn prove_equivalent(
        &self,
        lhs: &Self::Term,
        rhs: &Self::Term,
    ) -> Result<Self::Certificate, Self::Error>;
}
```

Use cases are solver-backed rule validation, equivalence checks, and eventual rule synthesis. No solver dependency is required for the first crate scaffold.

## Macros

The `macros` feature is reserved for ergonomics. A future release may add:

- `derive(Rewritable)`
- `term!` helper macro
- `rule!` helper macro

For 0.23.0, prioritize the explicit core API. Proc macros should be deferred unless they are trivial and do not destabilize the release.

## Tests

Required first-wave tests:

- path traversal and replacement on a sample recursive `Expr`
- ARS normalization and bounded-step behavior
- ARS strategies: outer-first, inner-first, all successors
- TRS substitution and matching consistency
- non-linear pattern matching: `f(X, X)`
- RHS variable validation
- inverse predecessor generation on Peano arithmetic
- anti-unification examples and generalization property checks
- rule inference from positive examples
- negative examples filtered or marked inconclusive
- feature-gated neural/network/smt trait compilation tests

## Examples

Ship examples such as:

- `symbolic_simplification.rs`
- `peano_trs.rs`
- `inverse_search.rs`
- `infer_rule_from_examples.rs`
- `network_guided_search.rs` behind the `network` feature
- `neural_rule_trait.rs` behind the `neural` feature

## Documentation

Crate docs should explain:

- ARS vs TRS
- rewrite strategies
- normal forms and termination limits
- inverse rewriting as bounded predecessor search
- anti-unification and rule inference limits
- why neural/SMT/network modules are feature-gated and experimental

The README should include a minimal symbolic simplification example and a small TRS example.

## 0.23.0 success criteria

- `amari-rewrite` is a workspace crate with stable ARS/TRS/inverse/synthesis APIs.
- Default build uses no heavy external rewrite, neural, or SMT dependencies.
- Feature-gated experimental modules compile and document their boundaries.
- The crate has enough tests/examples to demonstrate symbolic simplification, Peano TRS, inverse search, and positive-example rule inference.
- The design remains compatible with later `amari-surcomplex` and future Amari DSLs.
