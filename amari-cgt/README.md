# amari-cgt

Computational combinatorial game theory for the Amari library.

## Current scope

The initial implementation focuses on:

- short partizan games
- short impartial games
- birthdays
- arena-independent structural game forms for import/export
- formatting/display of small named games and recursive cuts
- explicit numeric-validation witnesses for downstream surreal conversion
- canonical / numeric / impartial inspection helpers
- normal-play outcome classes
- partial comparison
- canonicalization via dominated and reversible option reduction
- disjunctive sum / negation / subtraction
- nim heaps and Sprague-Grundy values
- small exhaustive generation by exact or bounded birthday / reachable-node layers
- canonical corpus metadata, layer maps, bucketing, and counting hooks
- layer analysis reports for growth, canonical reduction, and classification trends

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
