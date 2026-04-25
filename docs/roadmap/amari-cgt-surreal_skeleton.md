# Amari-CGT + Amari-Surreal Skeleton Proposal

## Purpose

This document proposes a concrete **crate skeleton**, **workspace integration plan**, and **API/module scaffold** for introducing:

- `amari-cgt`
- `amari-surreal`

into the Amari workspace.

It complements:

- `docs/roadmap/amari-cgt_design.md`
- `docs/roadmap/amari-surreal_design.md`

The goal here is not to finalize implementation details, but to make the next practical steps obvious when the project is ready to scaffold the crates.

**Target Release:** `0.22.0`  
**Roadmap:** `docs/roadmap/v0.22.0-cgt-surreal-roadmap.md`

---

## High-Level Dependency Plan

## Core direction

```text
amari-cgt        --> foundational short-game engine
amari-surreal    --> validated numeric/surreal layer over amari-cgt
```

## Initial dependency policy

### `amari-cgt`
Should start with minimal dependencies.

Recommended direct dependencies:
- `thiserror`
- `serde` (optional)

Recommended stance:
- do not make `amari-cgt` depend directly on `amari-enumerative` at first
- instead, add bridge modules or downstream integrations later

### `amari-surreal`
Recommended direct dependencies:
- `amari-cgt`
- `thiserror`
- `serde` (optional)
- `num-bigint`
- `num-traits`

This keeps the foundational layering clean.

---

## Workspace Additions

## Root `Cargo.toml`

### Workspace members

```toml
[workspace]
members = [
    # ... existing crates ...
    "amari-cgt",
    "amari-surreal",
]
```

### Workspace dependencies

```toml
[workspace.dependencies]
# ... existing crates ...
amari-cgt = { path = "amari-cgt", version = "0.22" }
amari-surreal = { path = "amari-surreal", version = "0.22" }
```

### Umbrella crate dependencies

```toml
[dependencies]
# ... existing dependencies ...
amari-cgt = { workspace = true, optional = true }
amari-surreal = { workspace = true, optional = true }
```

### Umbrella feature flags

```toml
[features]
# ... existing features ...
cgt = ["dep:amari-cgt"]
surreal = ["dep:amari-surreal", "cgt"]
```

### Umbrella exports

```rust
#[cfg(feature = "cgt")]
pub use amari_cgt as cgt;

#[cfg(feature = "surreal")]
pub use amari_surreal as surreal;
```

## Recommendation

Initially, **do not add these crates to `full`** unless `full` is explicitly redefined to include experimental crates. They are better introduced as opt-in features first.

---

## `amari-cgt` Skeleton

## Proposed `Cargo.toml`

```toml
[package]
name = "amari-cgt"
version.workspace = true
authors.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
description = "Computational combinatorial game theory for the Amari library"
repository = "https://github.com/justinelliottcobb/Amari"
homepage = "https://github.com/justinelliottcobb/Amari"
keywords = ["mathematics", "combinatorial-game-theory", "games", "nimber", "surreal"]
categories = ["mathematics", "science", "algorithms"]

[dependencies]
thiserror = { workspace = true }
serde = { workspace = true, optional = true }

[dev-dependencies]
criterion = "0.8"

[features]
default = ["std"]
std = []
serialize = ["dep:serde"]

[[bench]]
name = "comparison"
harness = false
```

## Proposed directory tree

```text
amari-cgt/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── game.rs
│   ├── arena.rs
│   ├── birthday.rs
│   ├── outcome.rs
│   ├── order.rs
│   ├── sum.rs
│   ├── negation.rs
│   ├── canonical.rs
│   ├── impartial.rs
│   ├── nimber.rs
│   ├── generation.rs
│   ├── examples.rs
│   └── prelude.rs
├── tests/
│   ├── basics.rs
│   ├── outcome_classes.rs
│   ├── order.rs
│   ├── canonical.rs
│   ├── nimbers.rs
│   └── generation.rs
└── benches/
    ├── comparison.rs
    ├── arena.rs
    └── canonicalization.rs
```

## Proposed `src/lib.rs`

```rust
//! amari-cgt: computational combinatorial game theory

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

pub mod arena;
pub mod birthday;
pub mod canonical;
pub mod error;
pub mod examples;
pub mod game;
pub mod generation;
pub mod impartial;
pub mod negation;
pub mod nimber;
pub mod order;
pub mod outcome;
pub mod prelude;
pub mod sum;

pub use arena::GameArena;
pub use birthday::Birthday;
pub use error::{CgtError, Result};
pub use game::{CanonicalGame, GameComparison, GameId, OutcomeClass};
pub use nimber::Nimber;
```

## Proposed key public items

```rust
pub struct GameArena;
pub struct GameId(u32);
pub struct Birthday(pub u32);
pub struct CanonicalGame(pub GameId);
pub struct Nimber(pub u32);

pub enum OutcomeClass {
    LeftWins,
    RightWins,
    NextPlayerWins,
    PreviousPlayerWins,
}

pub enum GameComparison {
    Less,
    Equal,
    Greater,
    Fuzzy,
}
```

---

## `amari-surreal` Skeleton

## Proposed `Cargo.toml`

```toml
[package]
name = "amari-surreal"
version.workspace = true
authors.workspace = true
edition.workspace = true
license.workspace = true
rust-version.workspace = true
description = "Computable surreal numbers for the Amari library"
repository = "https://github.com/justinelliottcobb/Amari"
homepage = "https://github.com/justinelliottcobb/Amari"
keywords = ["mathematics", "surreal-numbers", "dyadic", "combinatorial-game-theory"]
categories = ["mathematics", "science", "algorithms"]

[dependencies]
amari-cgt = { workspace = true }
thiserror = { workspace = true }
serde = { workspace = true, optional = true }
num-bigint = "0.4"
num-traits = { workspace = true }

[dev-dependencies]
criterion = "0.8"

[features]
default = ["std"]
std = []
serialize = ["dep:serde"]
symbolic = []

[[bench]]
name = "dyadic"
harness = false
```

## Proposed directory tree

```text
amari-surreal/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── dyadic.rs
│   ├── short.rs
│   ├── numeric.rs
│   ├── birthday.rs
│   ├── simplest.rs
│   ├── order.rs
│   ├── arithmetic.rs
│   ├── convert.rs
│   ├── symbolic.rs
│   └── prelude.rs
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

## Proposed `src/lib.rs`

```rust
//! amari-surreal: computable surreal numbers

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(missing_docs)]

pub mod arithmetic;
pub mod birthday;
pub mod convert;
pub mod dyadic;
pub mod error;
pub mod numeric;
pub mod order;
pub mod prelude;
pub mod short;
pub mod simplest;

#[cfg(feature = "symbolic")]
pub mod symbolic;

pub use dyadic::Dyadic;
pub use error::{Result, SurrealError};
pub use numeric::NumericGame;
pub use short::ShortSurreal;
```

## Proposed key public items

```rust
pub struct Dyadic {
    numer: BigInt,
    exponent: u32,
}

pub struct NumericGame {
    game: GameId,
}

pub struct ShortSurreal {
    value: Dyadic,
    birthday: Birthday,
    provenance: Option<GameId>,
}
```

---

## Module-by-Module Skeleton Notes

## `amari-cgt`

### `game.rs`
Should define the public-facing core types:
- `GameId`
- `GameComparison`
- `OutcomeClass`
- `CanonicalGame`

### `arena.rs`
Should own:
- game-node storage
- interning
- caches
- constructors for `0`, `*`, `1`, `-1`
- public recursive operations

### `canonical.rs`
Should focus on:
- option sorting and deduplication
- dominated-option elimination
- reversible-option reduction
- stable canonical interning

### `impartial.rs` / `nimber.rs`
Should support:
- impartiality checks
- mex computation
- Grundy values
- nim heap constructors

## `amari-surreal`

### `dyadic.rs`
Should provide:
- normalized exact dyadic arithmetic
- conversion from integers
- total order
- display formatting

### `numeric.rs`
Should provide:
- validation from `GameId`
- a minimal bridge object proving numericity

### `short.rs`
Should provide:
- validated short surreal wrapper
- arithmetic via dyadics
- birthday/provenance accessors

### `simplest.rs`
Should provide:
- simplest dyadic strictly between finite left/right bounds

---

## Suggested Initial API Skeleton

## `amari-cgt`

```rust
impl GameArena {
    pub fn new() -> Self;

    pub fn zero(&mut self) -> GameId;
    pub fn star(&mut self) -> GameId;
    pub fn one(&mut self) -> GameId;
    pub fn minus_one(&mut self) -> GameId;

    pub fn from_options<L, R>(&mut self, left: L, right: R) -> Result<GameId>
    where
        L: IntoIterator<Item = GameId>,
        R: IntoIterator<Item = GameId>;

    pub fn birthday(&mut self, game: GameId) -> Result<Birthday>;
    pub fn outcome(&mut self, game: GameId) -> Result<OutcomeClass>;
    pub fn compare(&mut self, lhs: GameId, rhs: GameId) -> Result<GameComparison>;
    pub fn equivalent(&mut self, lhs: GameId, rhs: GameId) -> Result<bool>;

    pub fn neg(&mut self, game: GameId) -> Result<GameId>;
    pub fn add(&mut self, lhs: GameId, rhs: GameId) -> Result<GameId>;
    pub fn sub(&mut self, lhs: GameId, rhs: GameId) -> Result<GameId>;

    pub fn canonicalize(&mut self, game: GameId) -> Result<CanonicalGame>;

    pub fn is_impartial(&mut self, game: GameId) -> Result<bool>;
    pub fn grundy(&mut self, game: GameId) -> Result<Nimber>;
    pub fn nim_heap(&mut self, size: u32) -> Result<GameId>;

    pub fn is_numeric(&mut self, game: GameId) -> Result<bool>;
}
```

## `amari-surreal`

```rust
impl Dyadic {
    pub fn new(numer: impl Into<BigInt>, exponent: u32) -> Self;
    pub fn from_integer(n: impl Into<BigInt>) -> Self;
    pub fn normalize(self) -> Self;
}

impl NumericGame {
    pub fn from_game(arena: &mut GameArena, game: GameId) -> Result<Self>;
    pub fn game_id(&self) -> GameId;
}

impl ShortSurreal {
    pub fn zero() -> Self;
    pub fn one() -> Self;
    pub fn from_integer(n: i64) -> Self;
    pub fn from_dyadic(value: Dyadic) -> Self;

    pub fn from_game(arena: &mut GameArena, game: GameId) -> Result<Self>;
    pub fn to_dyadic(&self) -> Dyadic;
    pub fn birthday(&self) -> Birthday;
    pub fn provenance(&self) -> Option<GameId>;

    pub fn simplest_between(left: &[ShortSurreal], right: &[ShortSurreal]) -> Result<Self>;
}
```

---

## Suggested Error Types

## `amari-cgt`

```rust
#[derive(thiserror::Error, Debug)]
pub enum CgtError {
    #[error("invalid game id: {0:?}")]
    InvalidGameId(GameId),

    #[error("cyclic game construction detected")]
    CycleDetected,

    #[error("game is not impartial")]
    NotImpartial,

    #[error("game is not numeric")]
    NotNumeric,

    #[error("canonicalization invariant violated: {0}")]
    Canonicalization(String),
}
```

## `amari-surreal`

```rust
#[derive(thiserror::Error, Debug)]
pub enum SurrealError {
    #[error("numeric game validation failed")]
    NotNumeric,

    #[error("invalid dyadic representation")]
    InvalidDyadic,

    #[error("invalid cut: left bound is not strictly less than right bound")]
    InvalidCut,

    #[error("division by zero")]
    DivisionByZero,

    #[error("symbolic surreal feature required")]
    SymbolicFeatureRequired,
}
```

---

## Suggested README Scope

## `amari-cgt/README.md`
Should include:
- short-game overview
- named tiny examples (`0`, `*`, `1`)
- outcome classes
- impartial/Nim example
- explicit note that the initial scope is short games only

## `amari-surreal/README.md`
Should include:
- surreals as numeric games
- short-surreal = dyadic scope
- example conversion from game to dyadic
- simplest-number example
- explicit note that unrestricted symbolic surreals are not the first stable target

---

## Suggested Testing Layout

## `amari-cgt`

### `tests/basics.rs`
- zero, star, one
- birthdays
- simple sums and negations

### `tests/outcome_classes.rs`
- `0`, `*`, `1`, `-1`, small sums

### `tests/order.rs`
- `1 > 0`
- `* || 0` via `Fuzzy`
- equality/equivalence checks

### `tests/canonical.rs`
- deduplication
- reduction
- canonicalization idempotence

### `tests/nimbers.rs`
- nim heaps
- XOR behavior
- mex correctness

## `amari-surreal`

### `tests/dyadics.rs`
- normalization
- exact arithmetic
- comparison

### `tests/numeric_games.rs`
- numeric-game validation
- rejection of non-numeric games

### `tests/conversion.rs`
- `{0|1} -> 1/2`
- `{1|2} -> 3/2`

### `tests/simplest.rs`
- simplest between `0` and `1` is `1/2`
- simplest between `1` and `2` is `3/2`

---

## Suggested Benchmark Layout

## `amari-cgt`
- arena interning
- comparison
- canonicalization
- Grundy computation

## `amari-surreal`
- dyadic normalization
- dyadic multiplication
- `GameId -> ShortSurreal` conversion
- `simplest_between`

---

## CI / Workflow Checklist

When these crates are actually added, follow `docs/development/ADDING_NEW_CRATES.md`.

That means updating at least:
- root `Cargo.toml`
- `.github/workflows/publish.yml`
- `.github/workflows/parallel-verification.yml`
- `.github/workflows/test-status.yml`
- `README.md`
- `CHANGELOG.md`

Also run:

```bash
./scripts/verify-workflow-crates.sh
```

---

## Recommended Introduction Order

## Step 1
Scaffold `amari-cgt` only.

Deliver:
- crate skeleton
- arena core
- named small games
- comparison/outcome tests

## Step 2
Stabilize `amari-cgt`'s canonicalization and impartial layer.

## Step 3
Scaffold `amari-surreal`.

Deliver:
- `Dyadic`
- numeric-game validation bridge
- `ShortSurreal`
- short-surreal arithmetic tests

This staging keeps the dependency chain clean and reduces premature abstraction.

---

## Summary

The proposed crate skeleton intentionally reflects the design priorities already identified:

- `amari-cgt` is the foundational recursive engine
- `amari-surreal` is the validated numeric layer on top
- both begin as opt-in experimental crates
- both are structured for computational correctness first

This should make it straightforward to move from planning to actual scaffolding when you are ready.
