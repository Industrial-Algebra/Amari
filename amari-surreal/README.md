# amari-surreal

Computable surreal numbers for the Amari library.

## Current scope

The crate provides two layers:

### `ShortSurreal` — finite short/dyadic layer

- exact dyadic arithmetic
- conversion from numeric short games in `amari-cgt`
- reconstruction back into numeric short games
- simplest-number construction for finite cuts
- birthday-aware short surreal values

### `RationalSurreal` — exact rational scalar field

- exact rational arithmetic backed by `BigRational`, not limited to dyadics
- bridges the gap between the dyadic `ShortSurreal` layer and full surreal generality
- exact construction from integer ratios (`from_ratio`), exact comparison, addition, subtraction, multiplication, and checked division
- `from_short` / `to_short_if_dyadic` conversion between `RationalSurreal` and `ShortSurreal` (partial: non-dyadic rational values return `None`)
- sign, ordering, and arithmetic utilities (`is_zero`, `is_positive`, `is_negative`, `abs`, `checked_reciprocal`, `checked_div`)

### Experimental epsilon rational functions

Behind the `experimental-epsilon` feature gate:

- polynomials and rational functions in a formal positive infinitesimal `ε`
- ordered by asymptotic behaviour as `ε → 0⁺`
- coefficient field is `RationalSurreal`
- **not** a nilpotent-dual-number system: `ε²` is a smaller positive infinitesimal, not zero
- the exponent type (`EpsilonExponent`) is a newtype wrapper so that future Puiseux / Hahn series extensions can replace the exponent without changing the public API shape

## Important scope boundary

This crate does **not** currently claim to implement the full proper class of surreal numbers. The implemented scope is:

- `ShortSurreal` — the computationally useful short-surreal / dyadic layer, not arbitrary rationals
- `RationalSurreal` — the exact rational scalar field, not arbitrary surreal numbers
- `experimental-epsilon` — formal epsilon rational functions, not Puiseux series, Hahn series, or generalized ordered-series fields (all deferred to future extensions)

## Example

```rust
use amari_cgt::GameArena;
use amari_surreal::{Dyadic, ShortSurreal};

let mut arena = GameArena::new();
let zero = arena.zero();
let one = arena.one()?;
let half_game = arena.from_options([zero], [one])?;
let half = ShortSurreal::from_game(&mut arena, half_game)?;

assert_eq!(half.to_dyadic(), Dyadic::new(1, 1));
# Ok::<(), Box<dyn std::error::Error>>(())
```

### RationalSurreal example

```rust
use amari_surreal::RationalSurreal;

let a = RationalSurreal::from_ratio(1, 3).unwrap();
let b = RationalSurreal::from_ratio(2, 5).unwrap();
let sum = a + b; // 11/15
assert_eq!(sum.to_string(), "11/15");
```
