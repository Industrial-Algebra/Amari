# amari-tropical M1 Architecture Pass

## Purpose

This document records the concrete architecture decisions from **M1** of the `0.21.0` cycle.

Its job is to answer five questions:

1. what the current `amari-tropical` public surface actually looks like
2. what compatibility constraints exist downstream
3. what the **smallest practical semiring abstraction** is
4. what should remain float-specific in `0.21.0`
5. what the public ordinal API should look like in M2

---

## Current Public Surface Audit

### Public modules

Current `amari-tropical` exposes:

- `types`
- `error`
- `polytope`
- `viterbi`
- `verified` behind `phantom-types`
- `verified_contracts` behind `contracts`

### Current core public types

The main runtime surface is centered on:

- `TropicalNumber<T: Float>`
- `TropicalMatrix<T: Float>`
- `TropicalMultivector<T: Float, const P: usize, const Q: usize, const R: usize>`

The important architectural fact is that all three are currently built around the same assumption:

- the carrier is floating-point
- tropical multiplication is implemented as ordinary addition on that floating-point carrier

### Float-specific modules today

The current non-core modules are also mostly float-bound:

- `viterbi` depends on log-probability input and direct numeric comparisons
- `polytope` depends on subtraction, epsilon checks, and approximate geometric heuristics

### Verified layer today

The `verified` layer already contains phantom semiring distinctions:

- `MaxPlus`
- `MinPlus`
- `TropicalSemiring`

That is useful context, but it is not yet the same thing as a **runtime semiring abstraction** for the main crate surface.

---

## Downstream Compatibility Constraints

A workspace-wide usage scan shows that the current runtime types are already used across multiple crates, including:

- `amari-optimization`
- `amari-fusion`
- `amari-network`
- `amari-measure`
- `amari-wasm`
- `amari-gpu`
- `amari-automata`

The most important compatibility observations are:

### 1. `TropicalNumber` is the primary shared runtime currency

Many downstream users depend directly on:

- `TropicalNumber::new(...)`
- `TropicalNumber::zero()` / `tropical_zero()`
- `tropical_add(...)`
- `tropical_mul(...)`
- `value()`

So `TropicalNumber<T>` should be preserved as the main float max-plus runtime carrier.

### 2. `TropicalMatrix` currently exposes public storage fields

Downstream code reads or writes:

- `rows`
- `cols`
- `data`

directly.

So any deeper refactor of matrix internals would have a large compatibility cost.

### 3. Some current tropical functionality is intrinsically numeric rather than semiring-generic

Examples:

- log-probability constructors
- epsilon-based geometry heuristics
- raw floating-point conversions

These should not be forced into a generic semiring API just to satisfy the ordinal use case.

---

## M1 Decision: Keep the Existing Float Layer, Add a Minimal Shared Trait

The chosen M1 architecture is:

- keep the existing float max-plus types intact
- add a **small runtime semiring trait** for shared algebraic structure
- add the ordinal substrate as a separate module rather than forcing it into `TropicalNumber<T: Float>`

### Chosen trait shape

The minimum practical trait is:

```rust
pub trait Semiring: Clone + PartialEq {
    fn zero() -> Self;
    fn one() -> Self;
    fn oplus(&self, other: &Self) -> Self;
    fn otimes(&self, other: &Self) -> Self;
}
```

This was chosen because it is:

- small enough to be useful immediately
- expressive enough for both float max-plus and ordinal-weight carriers
- not so ambitious that it forces a giant generic rewrite

### Why this is the right minimum

This abstraction captures exactly what both target carriers share:

- additive identity
- multiplicative identity
- additive combination
- multiplicative composition

It does **not** try to encode everything else, such as:

- total ordering
- valuation
- formatting policy
- provenance
- matrix layouts
- HMM-specific behavior

Those belong in higher layers or carrier-specific APIs.

---

## What Stays Float-Specific in `0.21.0`

The following should remain concretely float-oriented unless there is a very strong reason to generalize them later in the cycle.

### `TropicalNumber<T>`

Stays as the canonical float max-plus runtime type.

It should also implement the new `Semiring` trait.

### `TropicalMatrix<T>`

Stays public and compatible as the float max-plus matrix type.

For `0.21.0`, we should **not** force a full public generic-matrix migration unless ordinal-driven utilities prove that it is genuinely needed.

### `TropicalMultivector<T, P, Q, R>`

Stays float-specific for now.

Its current story is tied to the broader geometric algebra ecosystem and should not be destabilized during the ordinal substrate pass.

### `viterbi`

Stays float-specific in `0.21.0`.

Reasons:

- log-probability constructors
- direct numeric score comparisons
- HMM-oriented semantics

If generic path algorithms are added later, they should likely live beside `viterbi`, not replace it.

### `polytope`

Stays float-specific in `0.21.0`.

Reasons:

- subtraction-based heuristics
- epsilon-style numeric decisions
- visualization-oriented geometry rather than semiring-core algebra

---

## What Becomes Shared / Semiring-Oriented

### `semiring` module

Add a small public module:

- `amari_tropical::semiring`

with public re-export:

- `amari_tropical::Semiring`

### `TropicalNumber<T>` implements `Semiring`

This gives the existing float layer a shared algebraic surface without changing its role.

### Future ordinal carrier follows the same semiring vocabulary

Because the ordinal layer uses arena-backed identifiers, its composition operations need arena context for interning.

So the planned ordinal runtime weight type should expose the same `zero` / `one` / `oplus` / `otimes` vocabulary, but via arena-aware methods rather than a direct implementation of the context-free `Semiring` trait.

---

## Ordinal Layer Naming Decision

The naming decision is now fixed:

- module: `ordinal`
- arena: `OrdinalArena`
- identifier: `OrdinalId`
- CNF term: `CnfTerm`
- optimization-facing semiring wrapper: `OrdinalWeight`

`OrdinalId` is preferred over a generic `NodeId` because it is semantically clear at call sites and matches the public concept being modeled.

---

## Planned M2 Public API Surface

The intended M2 ordinal surface should look approximately like this:

```rust
pub mod ordinal {
    pub struct OrdinalArena { /* ... */ }
    pub struct OrdinalId(/* ... */);

    pub struct CnfTerm {
        pub exponent: OrdinalId,
        pub coefficient: u64,
    }

    pub enum OrdinalWeight {
        Bottom,
        Ordinal(OrdinalId),
    }
}
```

with core arena operations such as:

```rust
impl OrdinalArena {
    pub fn zero(&self) -> OrdinalId;
    pub fn finite(&mut self, n: u64) -> OrdinalId;
    pub fn omega(&mut self) -> OrdinalId;
    pub fn intern_cnf(&mut self, terms: Vec<CnfTerm>) -> Result<OrdinalId, TropicalError>;

    pub fn compare(
        &self,
        left: OrdinalId,
        right: OrdinalId,
    ) -> Result<core::cmp::Ordering, TropicalError>;
    pub fn add(
        &mut self,
        left: OrdinalId,
        right: OrdinalId,
    ) -> Result<OrdinalId, TropicalError>;

    pub fn leading_exponent(&self, ordinal: OrdinalId) -> Result<Option<OrdinalId>, TropicalError>;
    pub fn leading_term(
        &self,
        ordinal: OrdinalId,
    ) -> Result<Option<(OrdinalId, u64)>, TropicalError>;
}

impl OrdinalWeight {
    pub fn bottom() -> Self;
    pub fn one() -> Self;
    pub fn oplus(self, other: Self, arena: &OrdinalArena) -> Result<Self, TropicalError>;
    pub fn otimes(self, other: Self, arena: &mut OrdinalArena) -> Result<Self, TropicalError>;
    pub fn valuation(self, arena: &OrdinalArena) -> Result<Option<OrdinalId>, TropicalError>;
}
```

---

## Deferred by Design

M1 explicitly does **not** commit us to:

- a giant generic matrix rewrite
- immediate genericization of `viterbi`
- immediate genericization of `polytope`
- a full runtime min-plus carrier redesign
- making every tropical utility generic over every semiring-like object in the crate

Those may be revisited later, but they are not required to make `0.21.0` successful.

---

## Concrete M1 Outcome

M1 is considered successful if the crate now has:

- a documented compatibility-preserving architecture
- a minimal `Semiring` trait
- a clear boundary between float-specific and semiring-generic layers
- a locked public naming plan for the ordinal substrate

That is enough to begin M2 implementation without architectural ambiguity.

---

## Next Implementation Steps

1. add the small `semiring` module and implement it for `TropicalNumber<T>`
2. add the `ordinal` module skeleton
3. implement `OrdinalArena`, `OrdinalId`, `CnfTerm`, and `OrdinalWeight`
4. add below-`ε₀` CNF normalization, comparison, and ordinal addition
5. add leading-exponent valuation and formatting helpers
