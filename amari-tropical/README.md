# amari-tropical

Tropical max-plus algebra and ordinal-weighted optimization carriers.

## Overview

`amari-tropical` now exposes two complementary layers:

- a float-oriented max-plus layer built around `TropicalNumber`, `TropicalMatrix`, and `TropicalMultivector`
- an arena-backed ordinal layer below `ε₀` built around `OrdinalArena` and `OrdinalWeight`

Together these layers target ranking, path aggregation, Viterbi-style decoding, dynamic programming, and ordinal-weighted optimization heuristics.

## Features

- **TropicalNumber**: Core scalar type with max-plus operations
- **TropicalMatrix**: Matrix operations in tropical algebra
- **TropicalMultivector**: Integration with geometric algebra
- **OrdinalArena / OrdinalWeight**: Arena-backed ordinals below `ε₀` for ordinal-weighted optimization
- **Viterbi Decoder**: Efficient sequence decoding using tropical operations
- **Tropical Polytopes**: Geometric structures in tropical space
- **High-Precision Support**: Optional extended precision arithmetic

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
amari-tropical = "0.21.0"
```

### Feature Flags

```toml
[dependencies]
# Default features
amari-tropical = "0.21.0"

# With serialization
amari-tropical = { version = "0.21.0", features = ["serialize"] }

# With GPU acceleration
amari-tropical = { version = "0.21.0", features = ["gpu"] }

# High-precision arithmetic
amari-tropical = { version = "0.21.0", features = ["high-precision"] }
```

## Quick Start

```rust
use amari_tropical::{fold_oplus, fold_otimes, TropicalNumber};

// Create tropical numbers
let a = TropicalNumber::new(3.0);
let b = TropicalNumber::new(5.0);

// Tropical addition: max(3, 5) = 5
let sum = a.tropical_add(&b);
assert_eq!(sum.value(), 5.0);

// Tropical multiplication: 3 + 5 = 8
let product = a.tropical_mul(&b);
assert_eq!(product.value(), 8.0);

// Tropical identities
let zero = TropicalNumber::<f64>::tropical_zero(); // -∞ (additive identity)
let one = TropicalNumber::<f64>::tropical_one();   // 0 (multiplicative identity)

// Fold alternative scores with tropical max and path scores with tropical composition
let best = fold_oplus([a, b]);
let path = fold_otimes([a, b]);
assert_eq!(best.value(), 5.0);
assert_eq!(path.value(), 8.0);
```

## Mathematical Background

### Tropical Semiring

In tropical algebra, we define:

| Standard | Tropical |
|----------|----------|
| a + b | max(a, b) |
| a × b | a + b |
| 0 (additive identity) | -∞ |
| 1 (multiplicative identity) | 0 |

### Why "Tropical"?

The name honors Brazilian mathematician Imre Simon, who pioneered this field. The algebra is particularly powerful because:

1. **Exponentiation becomes linear**: e^(a+b) → max(a, b) in log-space
2. **Products become sums**: Reduces computational complexity
3. **Softmax approximation**: max approximates log-sum-exp

### Applications

- **Viterbi Algorithm**: Find most likely state sequences in HMMs
- **Shortest Paths**: Tropical matrix multiplication solves all-pairs shortest paths
- **Neural Networks**: Approximate attention mechanisms efficiently
- **Optimization**: Linear programming in tropical geometry

## Key Types

### TropicalNumber<T>

```rust
use amari_tropical::TropicalNumber;

let x = TropicalNumber::new(2.5);

// Access the underlying value
let val = x.value();

// Tropical operations (take references)
let y = TropicalNumber::new(3.0);
let sum = x.tropical_add(&y);  // max(2.5, 3.0) = 3.0
let prod = x.tropical_mul(&y); // 2.5 + 3.0 = 5.5
```

### TropicalMatrix

```rust
use amari_tropical::TropicalMatrix;

// Create matrices for shortest path computation
let distances = TropicalMatrix::new(/* ... */);

// Tropical matrix multiplication gives shortest paths
let paths = distances.tropical_matmul(&distances);
```

### OrdinalArena / OrdinalWeight

```rust
use amari_tropical::{CnfTerm, OrdinalArena, OrdinalWeight};

let mut arena = OrdinalArena::new();
let one = arena.one();
let omega = arena.omega();
let omega_plus_one = arena.add(omega, one)?;
let omega_squared = arena.intern_cnf(vec![CnfTerm::new(omega, 1)])?;

assert_eq!(arena.format_ordinal(omega), Ok("ω".to_string()));
assert_eq!(arena.format_ordinal(omega_plus_one), Ok("ω + 1".to_string()));
assert_eq!(arena.format_ordinal(omega_squared), Ok("ω^ω".to_string()));

let weight = OrdinalWeight::from_ordinal(omega_plus_one);
assert_eq!(arena.format_weight(weight), Ok("ω + 1".to_string()));
assert_eq!(arena.best_weight(&[OrdinalWeight::bottom(), weight]), Ok(weight));
# Ok::<(), amari_tropical::TropicalError>(())
```

### Viterbi Decoder

```rust
use amari_tropical::viterbi::TropicalViterbi;

let transitions = vec![vec![-1.0, -2.0], vec![-2.0, -1.0]];
let emissions = vec![vec![-1.0, -3.0], vec![-3.0, -1.0]];
let observations = vec![0, 1, 0];

// Efficient sequence decoding using tropical algebra
let decoder = TropicalViterbi::new(transitions, emissions);
let best_path = decoder.decode(&observations);
assert_eq!(best_path.0.len(), observations.len());
```

## Scope Boundaries

- The ordinal substrate is intentionally restricted to ordinals below `ε₀` in canonical Cantor normal form.
- `OrdinalWeight` is a bottom-extended optimization carrier, not a full transfinite arithmetic tower.
- `viterbi` and `polytope` remain float-specific in the `0.21.0` release cycle.

## Modules

| Module | Description |
|--------|-------------|
| `types` | Core tropical number, matrix, and multivector types |
| `ordinal` | Arena-backed ordinals below `ε₀`, ordinal inspection, and bottom-extended ordinal weights |
| `semiring` | Minimal semiring trait plus fold helpers for context-free runtime carriers |
| `viterbi` | Viterbi algorithm for sequence decoding |
| `polytope` | Tropical polytopes and geometric structures |
| `verified` | Phantom types for compile-time verification |
| `error` | Error types for tropical operations |

## Performance

Tropical algebra offers significant performance benefits:

- **O(n) vs O(n log n)**: Max operation vs softmax
- **No overflow**: Log-space operations avoid numerical issues
- **Parallelizable**: Element-wise max operations are embarrassingly parallel
- **GPU-friendly**: Simple operations map well to GPU architectures

## Historical Note: v0.12 API Changes

The API was updated in v0.12.0 for better encapsulation:

```rust
// Before (v0.11.x)
let a = TropicalNumber(3.0);
let sum = a.tropical_add(b);
let val = a.0;

// After (v0.12.0+)
let a = TropicalNumber::new(3.0);
let sum = a.tropical_add(&b);  // Now takes reference
let val = a.value();
```

## License

Licensed under either of Apache License, Version 2.0 or MIT License at your option.

## Part of Amari

This crate is part of the [Amari](https://github.com/justinelliottcobb/Amari) mathematical computing library.
