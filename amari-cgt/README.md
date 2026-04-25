# amari-cgt

Computational combinatorial game theory for the Amari library.

## Current scope

The initial implementation focuses on:

- short partizan games
- short impartial games
- birthdays
- normal-play outcome classes
- partial comparison
- canonicalization via dominated and reversible option reduction
- disjunctive sum / negation / subtraction
- nim heaps and Sprague-Grundy values
- small exhaustive generation by birthday or reachable node count
- canonical corpus metadata, bucketing, and counting hooks

## Non-goals for the current implementation

- loopy games
- misère play
- scoring play
- thermographs / temperatures
- transfinite game universes
- large exhaustive universe generation beyond intentionally small bounds

## Example

```rust
use amari_cgt::{GameArena, GameComparison, OutcomeClass};

let mut arena = GameArena::new();
let zero = arena.zero();
let star = arena.star()?;
let one = arena.one()?;

assert_eq!(arena.compare(one, zero)?, GameComparison::Greater);
assert_eq!(arena.outcome(star)?, OutcomeClass::NextPlayerWins);
# Ok::<(), amari_cgt::CgtError>(())
```
