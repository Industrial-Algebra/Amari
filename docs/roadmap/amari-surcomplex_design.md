# Amari-Surcomplex Design Document

## Overview

**`amari-surcomplex`** is a proposed Amari crate for **complex arithmetic over supported surreal coefficient domains**.

It should be introduced **as a separate crate after `amari-surreal` is mature**, not folded into the initial stable scope of `amari-surreal` itself.

**Status:** Implemented in `0.23.0`
**Target Release:** `0.23.0`
**Immediate prerequisite:** `RationalSurreal` in `amari-surreal`
**Primary dependency:** `amari-surreal`
**Transitive dependency:** `amari-cgt` via `amari-surreal`
**Role in Amari:** Opt-in extension crate for exact rational surcomplex computation

The crate begins with a **disciplined exact rational layer**:

- coefficients in `RationalSurreal`
- exact arithmetic for values of the form `a + b i`
- conjugation and norm-square operations
- checked division when the denominator is nonzero
- `1 / (1 + 1/2 i) = 4/5 - 2/5i` demonstrates why dyadic `ShortSurreal` coefficients are insufficient and rational scalars are needed for surcomplex closure

It should **not** begin by claiming support for the full complexification of the entire surreal proper class, nor a full analytic theory over all future symbolic surreal forms.

---

## Design Goals

1. **Keep surcomplex support separate from `amari-surreal`**
   - Preserve the clean scope of `amari-surreal` as the surreal-scalar crate
   - Avoid expanding the `0.22.0` surreal API into a broader algebra package

2. **Build on a mature surreal scalar layer**
   - Reuse `RationalSurreal` as the primary coefficient type, with `ShortSurreal`/`Dyadic` available through conversion and later symbolic surreal forms deferred
   - Avoid duplicating coefficient logic inside the complex layer

3. **Start with exact, computationally honest arithmetic**
   - Rational surcomplex numbers over exact rational surreal coefficients
   - No hidden float approximation in the stable core

4. **Leave room for later symbolic extensions**
   - Support a later path to coefficients involving explicitly supported infinite or infinitesimal surreal subclasses
   - Keep this experimental until `amari-surreal` itself has stable symbolic layers

5. **Stay mathematically honest about scope**
   - Distinguish “complex numbers over the currently supported surreal domain” from “the full surcomplex universe”
   - Avoid premature claims about transcendental completeness or universal branch-cut behavior

---

## Non-Goals for the First Serious Version

### Explicit non-goals

- Full implementation of a complexified proper-class surreal universe
- General analytic function support over arbitrary surreal coefficients
- Universal `exp`, `log`, trigonometric, or branch-sensitive functions
- Full symbolic infinitesimal/infinite coefficient calculus before `amari-surreal` stabilizes it
- A drop-in replacement for ordinary complex arithmetic across all Amari crates
- Any claim that `amari-surcomplex` is the algebraic closure of all supported future surreal constructions

The first serious version should be about **correct exact arithmetic for rational surcomplex values**, not maximal generality.

---

## Mathematical Foundation

## Surcomplex Values as Gaussian Extensions

At the computational level, a surcomplex value should initially be modeled as:

```text
z = a + b i
```

where:

- `a` and `b` belong to a supported surreal coefficient domain
- `i^2 = -1`

For the `0.23.0` release, the supported coefficient domain is:

```text
a, b ∈ RationalSurreal
```

Since `RationalSurreal` supports exact rational arithmetic (not limited to dyadics), the surcomplex layer is an exact **Gaussian rational** layer. The earlier `ShortSurreal` dyadic layer is accessible via conversion but is not the primary coefficient type — dyadic-only coefficients cannot represent all surcomplex division results (e.g., `1 / (1 + 1/2 i) = 4/5 - 2/5i`), which is why `RationalSurreal` was introduced as the scalar foundation.

## Important Ordering Boundary

Unlike `ShortSurreal`, a surcomplex type should **not** provide a total order.

The stable initial semantics should emphasize:

- equality
- exact arithmetic
- conjugation
- norm-square
- invertibility criteria

rather than ordered-field behavior.

## Division

Division should be defined by the usual exact formula when the denominator is nonzero:

```text
(a + b i) / (c + d i) = ((a + b i)(c - d i)) / (c^2 + d^2)
```

This fits naturally with `RationalSurreal` because:

- `c^2 + d^2` is an exact rational surreal scalar
- zero-testing is exact
- division is total for nonzero rational denominators

---

## Strategic Scope

## Stage A — Exact Rational Surcomplex Layer (shipped in v0.23.0)

The first public version of `amari-surcomplex` delivers:

- coefficients in `RationalSurreal`
- exact construction from integers, rationals, dyadics, or `RationalSurreal` values
- exact addition, subtraction, negation, multiplication
- conjugation
- exact norm-square
- checked division for nonzero denominators (producing exact rational coefficients)
- formatting and testable algebraic identities
- `1 / (1 + 1/2 i) = 4/5 - 2/5i` demonstrating rational-closure necessity

## Stage B — Ergonomic Extensions

After the exact core is stable, the crate may add:

- scalar multiplication helpers
- convenience constructors from `Dyadic`
- small polynomial evaluation utilities
- matrix/vector integration hooks where useful

## Stage C — Experimental Symbolic Coefficient Extensions

Only after `amari-surreal` supports stable symbolic subclasses should `amari-surcomplex` consider:

- coefficients involving supported infinities or infinitesimals
- experimentally supported symbolic normal forms
- carefully delimited analytic operations on supported subclasses

This stage should remain explicitly experimental until the coefficient story is mature.

---

## Representation Strategy

## Core Principle

`amari-surcomplex` should **not** own surreal scalar logic.

Instead, it should depend on:

- `amari-surreal::RationalSurreal` for the stable initial coefficient layer
- later stable symbolic surreal types if and when they exist

## Recommended Core Type

### `RationalSurcomplex` (shipped in v0.23.0)

```rust
pub struct RationalSurcomplex {
    real: RationalSurreal,
    imag: RationalSurreal,
}
```

This is the stable first-class type for the `0.23.0` release. It uses `RationalSurreal` coefficients rather than `ShortSurreal` to support exact rational division results.

### Why `RationalSurreal` rather than `ShortSurreal`?

Division in surcomplex arithmetic produces rational (not necessarily dyadic) coefficients: `1 / (1 + 1/2 i) = 4/5 - 2/5i`. The dyadic `ShortSurreal` layer cannot represent `4/5`, so `RationalSurreal` was introduced as the scalar foundation for surcomplex arithmetic. A generic coefficient type is deferred until the need for it becomes clear.

### Later / Experimental Types

If `amari-surreal` grows a symbolic layer, `amari-surcomplex` may later introduce something like:

```rust
pub enum SurcomplexExpr {
    Rational(RationalSurcomplex),
    Symbolic {
        real: SurrealExpr,
        imag: SurrealExpr,
    },
}
```

This should remain out of the first stable scope.

---

## Proposed Module Structure

```text
amari-surcomplex/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── rational.rs       # RationalSurcomplex core type
│   ├── arithmetic.rs     # +, -, *, checked_div, conjugation, norm_sq
│   ├── convert.rs        # integer/dyadic/short-surreal conversions
│   ├── prelude.rs
│   └── symbolic.rs       # later experimental symbolic coefficient layer
├── tests/
│   ├── basics.rs
│   ├── arithmetic.rs
│   ├── division.rs
│   └── identities.rs
└── benches/
    ├── arithmetic.rs
    └── polynomial.rs
```

The actual module layout can be simplified at first, but the main architectural point is:

- exact rational layer first
- symbolic layer later and explicitly separated

---

## Public API Sketch

```rust
use amari_surcomplex::RationalSurcomplex;
use amari_surreal::RationalSurreal;

let one = RationalSurcomplex::one();
let i = RationalSurcomplex::i();
let z = one.clone() + i.clone();
let w = z.clone() * z.clone();

assert_eq!(w.real(), &RationalSurreal::zero());
assert_eq!(w.imag(), &RationalSurreal::from_integer(2));
assert_eq!(
    z.conjugate(),
    RationalSurcomplex::from_parts(RationalSurreal::one(), -RationalSurreal::one())
);
# Ok::<(), Box<dyn std::error::Error>>(())
```

### Actual public API (v0.23.0)

```rust
impl RationalSurcomplex {
    pub fn zero() -> Self;
    pub fn one() -> Self;
    pub fn i() -> Self;

    pub fn from_integer(n: i64) -> Self;
    pub fn from_parts(real: RationalSurreal, imag: RationalSurreal) -> Self;

    pub fn real(&self) -> &RationalSurreal;
    pub fn imag(&self) -> &RationalSurreal;
    pub fn conjugate(&self) -> Self;
    pub fn norm_sq(&self) -> RationalSurreal;

    pub fn checked_div(&self, rhs: &Self) -> Result<Self, SurcomplexError>;
}
```

Traits to implement in the initial stable scope:

- `Clone`
- `Eq`
- `Hash`
- `Display`
- `Add`
- `Sub`
- `Mul`
- `Neg`

Division should remain a checked method initially rather than immediately implementing `/` if that would obscure the explicit failure path.

---

## Relationship with `amari-surreal` and `amari-cgt`

## Hard Dependency Direction

The intended relationship is:

```text
amari-cgt         --> foundational short-game engine
amari-surreal     --> validated short-surreal scalar layer
amari-surcomplex  --> complex arithmetic over supported surreal scalars
```

not:

```text
amari-surreal and amari-surcomplex as merged crates
```

and not:

```text
amari-surcomplex depending directly on amari-cgt for its scalar semantics
```

## Why the separation matters

Keeping `amari-surcomplex` separate:

- preserves a clean scalar-layer story in `amari-surreal`
- makes later coefficient generalization easier
- allows release timing to follow surreal maturity rather than forcing it into `0.22.0`
- prevents complex-specific APIs from cluttering the short-surreal core

---

## Arithmetic Model

## Stable Initial Operations

For `RationalSurcomplex`, the stable initial release implements:

- addition
- subtraction
- negation
- multiplication
- conjugation
- exact equality
- norm-square
- checked division

These operations are expressed in terms of exact `RationalSurreal` arithmetic.

## Scalar Embedding

A rational surreal value embeds naturally as a purely real surcomplex value:

```text
a ↦ a + 0i
```

This is important for ergonomic use and for future interoperability with scalar-accepting APIs.

## Multiplication Laws

The crate should validate and test:

```text
i^2 = -1
(a + bi)(c + di) = (ac - bd) + (ad + bc)i
```

with exact coefficient arithmetic.

## Division and Invertibility

A value is invertible exactly when:

```text
norm_sq(z) ≠ 0
```

For the initial rational/exact layer, this is an exact and efficiently testable condition.

---

## Error Model

Use a crate-local `SurcomplexError`.

### Initial error cases

- division by zero / noninvertible denominator
- error propagated from `amari-surreal`
- unsupported symbolic operation in experimental layers

Suggested shape:

```rust
pub enum SurcomplexError {
    Surreal(#[from] amari_surreal::SurrealError),
    DivisionByZero,
    UnsupportedOperation(&'static str),
}
```

The initial exact layer may only need the first two cases.

---

## Testing Strategy

## Basic construction tests

Build a test corpus for:

- `0`
- `1`
- `i`
- `1 + i`
- `1/2 + 3/2 i`
- negative and mixed-sign examples

## Arithmetic tests

- exact addition/subtraction/multiplication
- `i * i = -1`
- conjugation involution
- distributivity and associativity on small exact examples
- division round-trips for nonzero values

## Identity tests

- `z * conjugate(z) = norm_sq(z)` as a purely real value
- `norm_sq(z * w) = norm_sq(z) * norm_sq(w)` in the rational exact layer
- purely real embeddings behave like underlying `RationalSurreal` arithmetic

## Failure tests

- checked division fails on zero denominator
- symbolic-only operations fail cleanly when not supported

---

## Benchmarks

Initial benchmark targets:

- rational surcomplex addition
- multiplication
- division
- polynomial evaluation on small exact inputs
- repeated conjugation/norm computations

Benchmarks should stay focused on the exact rational layer before any symbolic complexity is introduced.

---

## Integration with Existing Amari Crates

## `amari-surreal`

Primary direct dependency and scalar foundation.

## `amari-cgt`

Indirect foundational dependency through `amari-surreal`.

## `amari-wasm`

Release target covered in v0.23.0 via `WasmRationalSurcomplex`, with later room for:

- visualizing exact Gaussian-rational lattices
- interactive multiplication/conjugation demos
- educational views of exact rational-surcomplex arithmetic

## `amari-enumerative`

Possible later connection for:

- counting bounded exact coefficient families
- studying distribution of norm values in small rational-surcomplex corpora

These integrations should come only after the core surcomplex arithmetic is stable.

---

## Release Strategy

`amari-surcomplex` was introduced in `0.23.0`:

- as a separate workspace crate
- as an **opt-in** extension
- after `RationalSurreal` provided the exact rational scalar dependency
- with the umbrella feature:

```toml
surcomplex = ["dep:amari-surcomplex", "surreal"]
```

`surcomplex` implies `surreal`, which implies `cgt`.

---

## Proposed Implementation Phases

## Phase 1 — Exact Rational Surcomplex Core

Deliver:

- `RationalSurcomplex`
- exact constructors
- exact arithmetic
- conjugation
- norm-square
- checked division
- tests and at least basic benchmarks

## Phase 2 — API Polish and Integration

Deliver:

- formatting/display polish
- scalar conversion helpers
- README/examples
- umbrella crate integration when the release is scheduled

## Phase 3 — Experimental Symbolic Coefficient Layer

Deliver only after `amari-surreal` is ready:

- symbolic coefficient support for explicitly supported subclasses
- carefully delimited analytic operations
- clearly documented branch and scope restrictions

This phase should remain experimental until the underlying surreal symbolic layer is itself disciplined and stable.

---

## Future Directions

Possible later extensions include:

- symbolic infinite and infinitesimal coefficients once supported upstream
- polynomial and matrix utilities over surcomplex scalars
- branch-aware experimental `exp` / `log` work for explicitly supported subclasses
- visualization and educational tooling via WASM
- connections to asymptotic or valuation-style abstractions later on

All of these should come only after the exact rational layer is unquestionably solid.

---

## Summary

`amari-surcomplex` is a **separate crate** that depends on the disciplined scalar layers in `amari-surreal`.

Its first stable scope should be:

- exact rational surcomplex arithmetic
- concrete `RationalSurcomplex` values
- clean dependence on `RationalSurreal`
- mathematically honest boundaries around what is and is not implemented

This keeps `amari-surreal` focused, gives Amari a clean future extension path, and lets future surcomplex work grow only when the surreal scalar layer is ready to support it well.
