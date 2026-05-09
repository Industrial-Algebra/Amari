# Amari-Surreal Design Document

## Overview

**`amari-surreal`** is a proposed Amari crate for **computable surreal numbers**, built on top of validated numeric games from `amari-cgt`. Its purpose is to extend Amari into a new algebraic and analytic direction while staying computationally grounded.

**Status:** Planned for `0.22.0`  
**Target Release:** `0.22.0`  
**Role in Amari:** Opt-in mathematical extension crate with high-completeness release goals  
**Primary Dependency:** `amari-cgt`  
**Primary Goal:** Exact computation with short surreal numbers and a disciplined path toward restricted symbolic surreal extensions  
**Roadmap:** `docs/roadmap/v0.22.0-cgt-surreal-roadmap.md`

The crate should begin with **short surreals** and only later expand toward carefully chosen symbolic subclasses. It should not begin by claiming support for the full class of all surreal numbers.

---

## Design Goals

1. **Build surreal support on top of numeric games, not parallel recursive machinery**
   - Reuse `amari-cgt`'s arena and game comparison machinery
   - Avoid duplicate recursion logic and inconsistent canonicalization

2. **Provide an exact computational model for short surreal numbers**
   - Dyadic rationals
   - Exact order
   - Exact arithmetic
   - Birthday tracking

3. **Make numeric-game validation explicit**
   - Distinguish arbitrary games from numeric games
   - Treat `Surreal` values as validated objects, not unchecked cuts

4. **Leave room for restricted symbolic surreal extensions later**
   - Infinite values
   - Infinitesimals
   - Chosen normal forms for supported subclasses

5. **Integrate honestly with Amari's computational orientation**
   - Exact arithmetic first
   - Explicitly bounded scope
   - No premature promises of universal surreal arithmetic

---

## Non-Goals for the First Serious Version

### Explicit non-goals

- Full implementation of the proper class of all surreal numbers
- Unrestricted transfinite recursion
- Hyperreal / nonstandard analysis machinery
- General-purpose replacement for all scalar arithmetic across Amari
- Automatic compatibility with `Float`-centric APIs in the existing workspace

The first serious version should be about **correct short surreal computation**, not maximal generality.

---

## Mathematical Foundation

### Surreal Numbers as Numeric Games

A surreal number is recursively given by a cut:

```text
x = { L | R }
```

where:
- every element of `L` and `R` is already a surreal number
- every `l ∈ L` is strictly less than every `r ∈ R`

This is exactly the numeric-game condition inside combinatorial game theory.

### Short Surreals

For **short games**, the surreal numbers obtained this way are exactly the **dyadic rationals**:

```text
m / 2^n
```

This is a major computational advantage: short surreal arithmetic can be implemented exactly and efficiently once numeric-game validation is in place.

### Simplest Number Principle

Given valid left and right sets with `L < R`, the surreal number `{L | R}` is the **simplest number** lying strictly between them.

This principle is central to:
- canonical construction
- birthday interpretation
- conversion from numeric games to exact dyadic form

---

## Strategic Scope

## Stage A — Short Surreals (Initial Stable Scope)

The first public version of `amari-surreal` should only promise:

- validated short numeric games
- exact dyadic arithmetic
- exact comparison
- birthdays
- simplest-number construction
- conversion to and from `amari-cgt` numeric games

## Stage B — Restricted Symbolic Surreal Layer

Only after Stage A is stable, the crate may add a deliberately restricted symbolic layer supporting selected infinite and infinitesimal constructions.

This should remain explicitly marked as experimental until its supported normal forms are stable.

---

## Representation Strategy

## Core Principle

`amari-surreal` should **not** start by implementing raw recursive cuts independently of `amari-cgt`.

Instead, it should layer on top of:
- a `GameArena`
- validated numeric-game `GameId`s
- canonical comparison and birthday machinery from `amari-cgt`

## Recommended Core Types

### `Dyadic`

Exact arithmetic backend for short surreals.

```rust
pub struct Dyadic {
    numer: BigInt,
    exponent: u32,
}
```

Interpreted as:

```text
numer / 2^exponent
```

### `ShortSurreal`

Validated short surreal number.

```rust
pub struct ShortSurreal {
    value: Dyadic,
    birthday: Birthday,
    provenance: Option<GameId>,
}
```

This structure keeps both:
- an efficient exact arithmetic representation
- optional provenance back into the originating game arena

### `NumericGame`

A validated numeric-game wrapper around `GameId`.

```rust
pub struct NumericGame {
    game: GameId,
}
```

This acts as the bridge type from `amari-cgt` into `amari-surreal`.

### `SurrealExpr` (Later / Experimental)

A future symbolic extension type.

```rust
pub enum SurrealExpr {
    Short(ShortSurreal),
    Omega,
    NegOmega,
    Infinitesimal(String),
    Sum(Vec<SurrealExpr>),
    Product(Vec<SurrealExpr>),
}
```

This should not be part of the first stable scope.

---

## Why Dyadics Should Be First-Class

Once a short numeric game has been validated, its value belongs to the dyadic rationals. Representing short surreals as dyadics gives:

- exact arithmetic
- exact comparison
- compact canonical values
- easy testing against known results
- a clean boundary between the finite short theory and later symbolic extensions

This also keeps the crate computationally useful even before any infinite/infinitesimal symbolic layer exists.

---

## Proposed Module Structure

```text
amari-surreal/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── dyadic.rs         # Exact dyadic rational backend
│   ├── short.rs          # ShortSurreal type and core operations
│   ├── numeric.rs        # NumericGame validation and bridge from amari-cgt
│   ├── birthday.rs       # Birthday utilities and reconstruction helpers
│   ├── simplest.rs       # Simplest-number construction between bounds
│   ├── order.rs          # Exact ordering and comparisons
│   ├── arithmetic.rs     # +, -, *, reciprocal where defined/supported
│   ├── convert.rs        # GameId <-> ShortSurreal conversions
│   ├── prelude.rs
│   └── symbolic.rs       # Later experimental symbolic surreal layer
├── tests/
│   ├── dyadics.rs
│   ├── numeric_games.rs
│   ├── conversion.rs
│   ├── arithmetic.rs
│   └── simplest.rs
└── benches/
    ├── dyadic.rs
    └── conversion.rs
```

---

## Public API Sketch

```rust
use amari_cgt::GameArena;
use amari_surreal::{Dyadic, ShortSurreal};

let mut arena = GameArena::new();
let zero = arena.zero();
let one = arena.one();
let half_game = arena.from_options([zero], [one])?;

let half = ShortSurreal::from_game(&mut arena, half_game)?;
assert_eq!(half.to_dyadic(), Dyadic::new(1, 1));

let three_halves = ShortSurreal::from_integer(1) + half.clone();
assert_eq!(three_halves.to_dyadic(), Dyadic::new(3, 1));
```

### Suggested public API

```rust
impl Dyadic {
    pub fn new(numer: impl Into<BigInt>, exponent: u32) -> Self;
    pub fn from_integer(n: impl Into<BigInt>) -> Self;
    pub fn normalize(self) -> Self;
}

impl ShortSurreal {
    pub fn zero() -> Self;
    pub fn one() -> Self;
    pub fn from_integer(n: i64) -> Self;
    pub fn from_dyadic(value: Dyadic) -> Self;

    pub fn from_game(arena: &mut GameArena, game: GameId) -> Result<Self>;
    pub fn to_dyadic(&self) -> Dyadic;
    pub fn birthday(&self) -> Birthday;

    pub fn simplest_between(left: &[ShortSurreal], right: &[ShortSurreal]) -> Result<Self>;
}
```

---

## Relationship with `amari-cgt`

## Hard Dependency Direction

The intended relationship is:

```text
amari-cgt  --> foundational game engine
amari-surreal --> validated numeric layer over amari-cgt
```

not:

```text
amari-cgt and amari-surreal as parallel recursive universes
```

## Required Hooks from `amari-cgt`

To support `amari-surreal`, `amari-cgt` should expose or eventually support:

- `is_numeric(game: GameId) -> Result<bool>`
- `validate_numeric(game: GameId) -> Result<NumericGameWitness>`
- `birthday(game: GameId) -> Result<Birthday>`
- stable comparison against zero and between subgames
- canonicalization before numeric conversion

`amari-surreal` should consume these hooks rather than re-derive them independently.

---

## Numeric Validation Strategy

A game is numeric if:
- all options are numeric
- every left option is strictly less than every right option

The validation API should make this explicit.

### Suggested bridge flow

1. Build or obtain a `GameId` in `amari-cgt`
2. Canonicalize it
3. Validate that it is numeric
4. Convert it to `ShortSurreal`
5. Convert `ShortSurreal` to exact dyadic value

This is the cleanest separation between the game-theoretic substrate and the numeric layer.

---

## Arithmetic Model

## Initial Arithmetic Scope

For `ShortSurreal`, implement:

- addition
- negation
- subtraction
- multiplication
- total order

Because short surreals are dyadics, this arithmetic can be delegated to `Dyadic` once validation and conversion are complete.

## Reciprocal / Division

Division is well-defined except at zero. Recommended approach:

- implement reciprocal for nonzero dyadics
- implement division in terms of reciprocal
- keep symbolic reciprocal for non-dyadic future extensions out of stable scope until needed

---

## Simplest-Number Construction

The `simplest_between` operation should be a central public feature.

### Purpose

Given valid left and right finite sets of short surreals satisfying:

```text
max(L) < min(R)
```

construct the simplest dyadic strictly between them.

### Why this matters

This gives the crate a direct computational expression of the surreal construction principle rather than reducing everything to mere rational arithmetic.

---

## Error Model

Use a crate-local `SurrealError`.

### Initial error cases

- invalid dyadic normalization
- numeric-game validation failure
- empty/invalid cut construction
- incompatible bounds (`left >= right`)
- division by zero
- unsupported symbolic operation in experimental layers

---

## Testing Strategy

## Short Surreal Canonical Values

Build a test corpus for:

- `0`
- `1`
- `-1`
- `1/2 = {0 | 1}`
- `1/4 = {0 | 1/2}`
- `3/2 = {1 | 2}`
- negative dyadics and mixed sums

## Conversion Tests

- numeric games convert successfully
- non-numeric games fail validation
- canonicalized numeric games map to stable dyadic values

## Arithmetic Tests

- exact dyadic addition and multiplication
- order laws
- simplification/normalization of dyadics
- birthday preservation or documented birthday transformation behavior

## Property Tests

- normalized dyadic equality is canonical
- arithmetic agrees with dyadic backend
- conversion round-trips preserve value for supported cases

---

## Benchmarks

Initial benchmark targets:

- dyadic normalization
- arithmetic throughput
- conversion from numeric `GameId` to `ShortSurreal`
- `simplest_between` performance over small finite cuts

---

## Integration with Existing Amari Crates

## `amari-cgt`

Primary direct dependency.

## `amari-enumerative`

Natural future connection for:
- counting short numeric games by birthday
- studying dyadic distribution across bounded game universes
- generating finite cut families for experiments

## `amari-wasm`

Excellent later target for visual and educational tooling:
- birthday visualization
- simplest-number construction
- dyadic lattice exploration
- game-to-number conversion demos

---

## Release Strategy

Like `amari-cgt`, `amari-surreal` should initially be introduced as:

- a workspace crate
- an **opt-in umbrella feature**
- not part of the default stable nucleus at first

Recommended umbrella features:

```toml
cgt = ["dep:amari-cgt"]
surreal = ["dep:amari-surreal", "cgt"]
```

This makes the dependency relationship explicit.

---

## Proposed Implementation Phases

## Phase 1 — Exact Dyadic Backend

Deliver:
- `Dyadic`
- normalization
- exact arithmetic
- exact comparison

## Phase 2 — Numeric-Game Conversion

Deliver:
- numeric validation bridge from `amari-cgt`
- `ShortSurreal::from_game`
- provenance and birthday capture

## Phase 3 — Short Surreal Arithmetic and Simplest Numbers

Deliver:
- exact arithmetic via dyadics
- simplest-between construction
- test corpus for small short surreals

## Phase 4 — Restricted Symbolic Extensions

Deliver:
- explicitly limited symbolic types
- selected infinite/infinitesimal constructors
- clearly documented supported operations

This phase should remain experimental until the supported class is mathematically and computationally disciplined.

---

## Future Directions

Possible later extensions include:

- symbolic infinities and infinitesimals
- restricted normal forms for selected surreal subclasses
- ordinal-adjacent constructions
- asymptotic comparisons and growth classes
- bridges to tropicalization or valuation-style abstractions later on

These should come only after the short-surreal core is unquestionably solid.

---

## Summary

`amari-surreal` should begin as a **precise computational crate for short surreal numbers**, built on:

- `amari-cgt`'s validated numeric games
- exact dyadic arithmetic
- clear birthday-aware construction
- a carefully staged extension path toward symbolic surreal work

This design keeps the crate honest, computational, and highly aligned with the direction you want Amari to take.
