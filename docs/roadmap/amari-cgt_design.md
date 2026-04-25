# Amari-CGT Design Document

## Overview

**`amari-cgt`** is a proposed Amari crate for **computational combinatorial game theory**, centered on short partizan and impartial games. The crate is intended to extend Amari into a new mathematical domain while staying aligned with the project's existing strengths: exact algebraic modeling, enumeration, canonical forms, and computational experimentation.

**Status:** Proposal  
**Role in Amari:** Experimental extension, initially outside the stable 1.0 nucleus  
**Primary Goal:** A correct, memoized engine for short games and their canonical forms  
**Primary Downstream Consumer:** `amari-surreal`

This crate should be designed as the **foundational recursive game engine** on which future combinatorial-game and surreal-number work is built.

---

## Design Goals

1. **Represent short combinatorial games canonically**
   - Finite, well-founded game trees
   - Hash-consed DAG representation
   - Stable `GameId`-based identity inside an arena

2. **Support computational CGT workflows**
   - Disjunctive sum, negation, subtraction
   - Outcome-class computation
   - Partial comparison and equivalence
   - Canonicalization and reduction

3. **Support both partizan and impartial layers**
   - General short games first
   - Sprague-Grundy / nimbers as a first-class impartial subsystem

4. **Serve as the substrate for surreal numbers**
   - Numeric-game validation hooks
   - Birthday computation
   - Canonical recursive structure reusable by `amari-surreal`

5. **Integrate naturally with the Amari ecosystem without forcing false numeric generality**
   - Strong conceptual affinity with `amari-enumerative`
   - Optional future bridges to `amari-automata`, `amari-network`, and `amari-wasm`

---

## Non-Goals for the First Serious Version

The initial implementation should **not** attempt to cover all of CGT.

### Explicit non-goals

- Loopy games
- Misère theory
- Scoring play
- Thermographs / temperature theory
- Infinite or transfinite game universes
- Full theorem-proving or proof assistant style formalization

These can all be future extensions, but they should not shape the first stable architecture.

---

## Mathematical Foundation

### Short Partizan Games

A short partizan game is recursively given by:

```text
G = { G^L | G^R }
```

where:
- `G^L` is the finite set of Left options
- `G^R` is the finite set of Right options
- all options are themselves short games

The zero game is:

```text
0 = { | }
```

### Core Operations

- **Negation:**
  ```text
  -{L | R} = { -R | -L }
  ```
- **Disjunctive Sum:**
  ```text
  G + H = { G^L + H, G + H^L | G^R + H, G + H^R }
  ```
- **Subtraction:**
  ```text
  G - H = G + (-H)
  ```

### Outcome Classes

For normal play, short games fall into four outcome classes:

- `L`: Left wins regardless of who starts
- `R`: Right wins regardless of who starts
- `N`: Next player wins
- `P`: Previous player wins

### Comparison

Short partizan games are only **partially ordered**. The comparison relation should therefore support:

- `Less`
- `Equal`
- `Greater`
- `Fuzzy` (incomparable / first-player-sensitive)

### Impartial Games and Nimbers

For impartial normal-play games:
- Left and Right option sets coincide
- positions admit Sprague-Grundy values
- disjunctive sums reduce to nimber XOR at the impartial layer

This makes impartial games a valuable computational subsystem even before the full partizan engine is heavily optimized.

---

## Representation Strategy

## Arena + Hash-Consed DAG

The core representation should use an arena of interned nodes:

```rust
pub struct GameArena {
    nodes: Vec<GameNode>,
    intern: HashMap<NodeKey, GameId>,
    caches: GameCaches,
}

pub struct GameNode {
    left: Vec<GameId>,
    right: Vec<GameId>,
    birthday: Birthday,
    flags: GameFlags,
}

pub struct GameId(u32);
```

### Why this is the right fit

This gives:
- structural sharing
- cheap equality by arena identity after canonicalization/interning
- straightforward memoization
- stable recursion over finite DAGs
- a reusable substrate for surreal-number validation

### Canonical Node Key

Interning should use a normalized node key:
- sorted option lists
- deduplicated options
- optional reduced/canonical options after normalization

That makes the arena naturally resistant to combinatorial duplication.

---

## Core Types

### `GameId`

Opaque handle into the arena.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameId(u32);
```

### `Birthday`

Represents the recursive construction depth of a game.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Birthday(pub u32);
```

### `OutcomeClass`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutcomeClass {
    LeftWins,
    RightWins,
    NextPlayerWins,
    PreviousPlayerWins,
}
```

### `GameComparison`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameComparison {
    Less,
    Equal,
    Greater,
    Fuzzy,
}
```

### `CanonicalGame`

Optional wrapper for a reduced game guaranteed to be in canonical form.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CanonicalGame(pub GameId);
```

### `Nimber`

First-pass impartial-game value type.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Nimber(pub u32);
```

---

## Proposed Module Structure

```text
amari-cgt/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs
│   ├── error.rs
│   ├── game.rs          # GameId, GameNode, public game-facing API types
│   ├── arena.rs         # Arena storage, interning, caches
│   ├── birthday.rs      # Recursive birthday computation
│   ├── outcome.rs       # L/R/N/P classification
│   ├── order.rs         # G <= H, equivalence, fuzzy relation
│   ├── sum.rs           # Disjunctive sum, subtraction
│   ├── negation.rs      # Game negation
│   ├── canonical.rs     # Dominated/reversible reductions, normalization
│   ├── impartial.rs     # Impartial-game recognition and helpers
│   ├── nimber.rs        # Sprague-Grundy / nimber support
│   ├── generation.rs    # Optional generation of short games up to a bound
│   ├── prelude.rs       # Common imports
│   └── examples.rs      # Tiny named games (0, *, 1, -1, ↑, ↓)
├── tests/
│   ├── basics.rs
│   ├── outcome_classes.rs
│   ├── order.rs
│   ├── canonical.rs
│   ├── nimbers.rs
│   └── generation.rs
└── benches/
    ├── arena.rs
    ├── comparison.rs
    └── canonicalization.rs
```

---

## Public API Sketch

```rust
use amari_cgt::{GameArena, GameComparison, OutcomeClass, Nimber};

let mut arena = GameArena::new();

let zero = arena.zero();
let star = arena.from_options([zero], [zero])?;
let one = arena.from_options([zero], [])?;
let neg_one = arena.neg(one)?;
let sum = arena.add(one, star)?;

assert_eq!(arena.outcome(star)?, OutcomeClass::NextPlayerWins);
assert_eq!(arena.compare(one, zero)?, GameComparison::Greater);
assert_eq!(arena.compare(star, zero)?, GameComparison::Fuzzy);

let g = arena.nim_heap(5)?;
assert_eq!(arena.grundy(g)?, Nimber(5));
```

### Suggested arena API

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
}
```

---

## Canonicalization Strategy

Canonicalization should be treated as a major feature, not a cleanup detail.

### Initial canonicalization scope

- option sorting and deduplication
- removal of duplicate suboptions
- dominated-option elimination
- reversible-option reduction
- stable canonical interning

### Why this matters

Without canonicalization, enumeration and comparison explode in complexity and duplicate many equivalent positions. A good canonical layer is what makes the crate computationally useful rather than merely representational.

---

## Memoization and Caching

The arena should include explicit caches for:

- birthdays
- outcome classes
- partial comparison
- addition
- negation
- canonicalization
- Grundy values

Suggested internal shape:

```rust
struct GameCaches {
    birthdays: HashMap<GameId, Birthday>,
    outcomes: HashMap<GameId, OutcomeClass>,
    comparisons: HashMap<(GameId, GameId), GameComparison>,
    sums: HashMap<(GameId, GameId), GameId>,
    negations: HashMap<GameId, GameId>,
    canonicals: HashMap<GameId, CanonicalGame>,
    grundy: HashMap<GameId, Nimber>,
}
```

---

## Error Model

Use a crate-local `CgtError`.

### Initial error cases

- invalid arena ID
- malformed game construction
- cycle detected (should be unreachable in normal constructors)
- impartiality required but violated
- canonicalization failure / invariant violation
- numeric-game validation failure hooks for downstream crates

---

## Testing Strategy

## Unit Tests

Cover named small games and core operations:

- `0 = { | }`
- `* = { 0 | 0 }`
- `1 = { 0 | }`
- `-1 = { | 0 }`
- `↑ = { 0 | * }`
- tiny sums and negations

## Property Tests

- `G + 0 == G`
- `G + (-G)` outcome behaves as expected for test families
- `-(G + H) == (-G) + (-H)`
- birthday monotonicity over option inclusion
- canonicalization idempotence

## Impartial Tests

- nim heap Grundy values
- mex correctness
- XOR behavior under disjunctive sum

## Regression Corpus

Maintain a library of small canonical games and expected relations.

---

## Benchmarks

Initial benchmark targets:

- arena interning throughput
- repeated comparison on canonical games
- addition with memoization
- canonicalization cost by birthday / node count
- Grundy computation over generated impartial families

This will be especially useful if the crate later becomes a dependency for enumeration-heavy experiments.

---

## Integration Points with Existing Amari Crates

## `amari-enumerative`

Strongest long-term connection.

Potential uses:
- generate short games by birthday bound
- count canonical classes
- count numeric games vs non-numeric games
- study growth sequences

## `amari-automata`

Potential future bridge for rule-generated finite games and move systems.

## `amari-network`

Potential future use for game graphs, option digraphs, and component analysis.

## `amari-wasm`

Excellent later target for interactive visualizations:
- game trees
- canonical reductions
- nimber decomposition
- birthday growth

---

## Release Strategy

`amari-cgt` should initially be introduced as:

- a new workspace crate
- an **opt-in umbrella feature**
- **not** part of the umbrella `full` feature at first unless `full` explicitly includes experimental crates

Recommended umbrella feature shape:

```toml
cgt = ["dep:amari-cgt"]
```

---

## Proposed Implementation Phases

## Phase 1 — Short-Game Core

Deliver:
- arena
- `GameId`
- birthdays
- negation
- disjunctive sum
- outcome classes
- comparison

## Phase 2 — Canonicalization

Deliver:
- normalized option sets
- dominated/reversible reduction
- stable canonical forms
- better caching

## Phase 3 — Impartial/Nimber Layer

Deliver:
- impartial recognition
- Grundy computation
- nim heaps
- nim-sum behavior

## Phase 4 — Enumeration Hooks

Deliver:
- bounded generation of short games
- canonical-class counting hooks
- bridges to `amari-enumerative`

---

## Future Extensions

After the short-game kernel is stable, later layers may include:

- loopy games
- misère play
- scoring play
- thermal theory
- named families (Hackenbush, Domineering, Kayles variants)
- symbolic game-family generators

These should be **additive extensions**, not forces that destabilize the kernel design.

---

## Summary

`amari-cgt` should be built as a **memoized, canonical short-game engine** with:

- strong recursive invariants
- a compact arena-based representation
- first-class comparison and canonicalization
- a usable impartial/nimber subsystem
- clean downstream support for `amari-surreal`

If implemented this way, it can become one of the more distinctive and computationally rich crates in the Amari ecosystem.
