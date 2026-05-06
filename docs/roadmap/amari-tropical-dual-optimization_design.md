# amari-tropical / amari-dual Optimization Design Notes

## Purpose

This design note explains how `amari-tropical` and `amari-dual` should be positioned for `0.21.0` as **compiler/interpreter optimization-oriented extensions**.

The goal is not to turn Amari into a full compiler framework. The goal is to make these two crates useful as reusable mathematical layers inside optimization-heavy systems.

---

## Target Workloads

The intended workload class includes things like:

- path/ranking computations
- dynamic programming over candidate traces
- precedence and rewrite scoring
- abstract-interpretation joins with weighted structure
- heuristic tuning
- local cost sensitivity analysis
- fast ranking or pruning over structured search spaces

These workloads tend to value:

- deterministic semantics
- cheap composition
- explicit algebraic identities
- inspectable intermediate results
- low allocation in hot loops
- predictable tie behavior

---

## Role Split Between the Crates

### `amari-tropical`

`amari-tropical` should own:

- idempotent semiring structure
- ranking / aggregation semantics
- max-based competition between alternatives
- compositional path weights
- valuation-style projections
- ordinal-weighted optimization layers

### `amari-dual`

`amari-dual` should own:

- local sensitivity information
- differentiable scoring
- heuristic calibration
- small-dimensional gradient/Jacobian propagation
- forward-mode derivative computation for optimization loops

### What they should not do

- `amari-tropical` should not become a generic compiler IR crate
- `amari-dual` should not become a giant reverse-mode graph system in `0.21.0`
- neither crate should be forced into a single giant fused abstraction just because they can both be used in optimization contexts

---

## Design Principles

### 1. Optimize for explicit algebra, not hidden magic

Users of these crates should be able to see:

- what the carrier is
- what the identities are
- what the composition rule is
- how ties and branch points behave

This is especially important in compiler/interpreter use, where explainability matters.

### 2. Keep hot-path allocations visible and controllable

Optimization workloads often live in tight loops. APIs that allocate on every operation should be avoided or clearly separated from lower-level alternatives.

This matters most for:

- `MultiDualNumber`
- path / trace utilities built on top of tropical semirings
- formatting/reporting helpers that should not leak into core hot-path logic

### 3. Prefer small concrete useful abstractions over large generic towers

A tiny semiring trait that supports real use cases is better than a huge abstraction surface that delays the release.

### 4. Make tie behavior explicit

Operations like:

- `max`
- `min`
- branch selection
- winner-take-all reductions

are not innocent in optimization code. Ties should either:

- be documented as deterministic left/right preference
- be policy-driven
- or be represented by a witness/provenance layer when needed

### 5. Favor inspectability

Debugging optimization systems is easier when intermediate objects are easy to print and inspect. Both crates should have a strong story for:

- formatting
- lightweight summaries
- example-driven teachability

---

## `amari-tropical` Design Direction

### From “float max-plus crate” to “optimization semiring crate”

The current float max-plus layer remains useful and should stay supported.

But `0.21.0` should make it clear that `amari-tropical` is really about:

- semiring-guided optimization structure
- not only `f64` wrappers

### High-value features for optimization use

- lightweight semiring abstraction
- max-plus carrier for standard log/ranking workflows
- ordinal-weighted carrier for layered or transfinite ranking schemes
- formatting and inspection of small weights
- possible witness-preserving max helpers later

### Practical API bias

The APIs should favor:

- explicit identities
- explicit composition
- easy conversion from ordinary numeric/log-score data
- easy extraction of rankings or valuations

---

## `amari-dual` Design Direction

### From “calculus demo crate” to “optimization AD crate”

`amari-dual` already computes exact forward derivatives well. For `0.21.0`, the main shift is one of emphasis.

The crate should feel useful for:

- tuning a small set of heuristic parameters
- differentiating local cost models
- evaluating sensitivities in optimization passes
- cheap Jacobian/gradient propagation in interpreters or runtime systems

### High-value features for optimization use

- fixed-size or lower-allocation multi-variable representations
- seed helpers
- better piecewise/branch documentation
- benchmarks around small-dimensional AD workloads
- examples that look like optimization pipelines rather than textbook calculus only

### Important semantic issue: branch points

For optimization code, `max` and `min` are common. But derivatives at ties are not classically smooth.

`amari-dual` should therefore either:

- document deterministic winner selection
- expose tie policies where worthwhile
- or clearly state that these operations represent one-sided / selected-branch differentiation

That is better than pretending the issue does not exist.

---

## Cross-Crate Relationship

These crates should be **complementary**, not collapsed into one abstraction.

A good mental model is:

- tropical selects and composes candidates
- dual measures sensitivity inside candidate scoring

Possible future integrations may exist, but `0.21.0` should not require a fused tropical-dual algebraic system.

---

## Suggested `0.21.0` API Priorities

### `amari-tropical`

1. preserve the existing float max-plus layer cleanly
2. add a minimal semiring abstraction only where it helps
3. add the ordinal substrate as a dedicated module
4. add optimization-oriented examples and formatting helpers

### `amari-dual`

1. improve small-dimensional multi-variable ergonomics
2. clarify branch-sensitive operations
3. add optimization-oriented examples and benchmarks
4. keep the core forward-mode layer simple and dependable

---

## Non-Goals

For `0.21.0`, this design note explicitly does **not** recommend:

- turning Amari into a compiler framework
- adding reverse-mode AD to `amari-dual`
- making `amari-tropical` a fully generic algebra laboratory
- coupling this work tightly to `amari-surreal`
- pursuing abstraction so aggressively that practical utility gets delayed

---

## Summary

The `0.21.0` cycle should make:

- `amari-tropical` a better crate for ranking, composition, and ordinal-weighted optimization
- `amari-dual` a better crate for local optimization sensitivity and differentiable heuristics

The key to success is not maximal theory coverage. It is a clean, practical, inspectable algebraic surface that matches real optimization workloads.
