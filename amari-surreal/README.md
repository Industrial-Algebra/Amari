# amari-surreal

Computable surreal numbers for the Amari library.

## Current scope

The current implementation focuses on **short surreal numbers**:

- exact dyadic arithmetic
- conversion from numeric short games in `amari-cgt`
- reconstruction back into numeric short games
- simplest-number construction for finite cuts
- birthday-aware short surreal values

## Important scope boundary

This crate does **not** currently claim to implement the full proper class of surreal numbers. The implemented scope is the computationally useful short-surreal / dyadic layer.

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
