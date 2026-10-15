# amari-core

Core geometric algebra (Clifford algebra) implementation for the Amari mathematical computing library.

## Overview

`amari-core` provides the foundational types and operations for working with Clifford algebras of arbitrary signature Cl(P,Q,R). It implements multivectors, geometric products, rotors, and related operations with high performance through SIMD optimizations and cache-aligned memory layouts.

## Features

- **Arbitrary Signatures**: Support for Cl(P,Q,R) with any combination of positive, negative, and zero-squaring basis vectors
- **Geometric Product**: Full implementation of the geometric product with optimized Cayley tables
- **Rotors**: Rotation operations using the sandwich product
- **SIMD Optimization**: AVX2-accelerated operations on x86/x86_64 platforms
- **High-Precision Arithmetic**: Optional extended precision for scientific computing
- **no_std Support**: Can be used in embedded and WASM environments
- **Phantom Types**: Compile-time enforcement of dimension and signature invariants (algebraic-law contracts are expressed via annotations; see the formal-verification caveats in the docs)

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
amari-core = "0.12"
```

### Feature Flags

```toml
[dependencies]
# Default: std + phantom-types + high-precision
amari-core = "0.12"

# Minimal (no_std compatible)
amari-core = { version = "0.12", default-features = false }

# With serialization
amari-core = { version = "0.12", features = ["serialize"] }

# With parallel operations
amari-core = { version = "0.12", features = ["parallel"] }

# Native high-precision (uses rug/GMP, maximum performance)
amari-core = { version = "0.12", features = ["native-precision"] }
```

## Quick Start

```rust
use amari_core::{Multivector, basis::Basis, rotor::Rotor};

// Define a 3D Euclidean Clifford algebra Cl(3,0,0)
type Cl3 = Multivector<3, 0, 0>;

// Create basis vectors
let e1: Cl3 = Basis::e1();
let e2: Cl3 = Basis::e2();
let e3: Cl3 = Basis::e3();

// Geometric product: e1 * e2 creates a bivector
let e12 = e1.geometric_product(&e2);

// Create a rotor for 90° rotation in the xy-plane
let angle = std::f64::consts::PI / 2.0;
let rotor = Rotor::from_bivector(&e12, angle);

// Apply rotation to e1 (should give e2)
let rotated = rotor.apply(&e1);
```

## Key Types

### Multivector<P, Q, R>

The primary type representing elements of a Clifford algebra:

- `P`: Number of basis vectors squaring to +1
- `Q`: Number of basis vectors squaring to -1
- `R`: Number of basis vectors squaring to 0

```rust
// Common algebras
type Cl2 = Multivector<2, 0, 0>;      // 2D Euclidean (complex numbers)
type Cl3 = Multivector<3, 0, 0>;      // 3D Euclidean (quaternions embedded)
type Spacetime = Multivector<1, 3, 0>; // Minkowski spacetime
type PGA3D = Multivector<3, 0, 1>;    // Projective Geometric Algebra
```

### Basis Blade Indexing

Basis blades are indexed using binary representation:

| Index | Binary | Blade |
|-------|--------|-------|
| 0 | 0b000 | 1 (scalar) |
| 1 | 0b001 | e₁ |
| 2 | 0b010 | e₂ |
| 3 | 0b011 | e₁∧e₂ |
| 4 | 0b100 | e₃ |
| 5 | 0b101 | e₁∧e₃ |
| 6 | 0b110 | e₂∧e₃ |
| 7 | 0b111 | e₁∧e₂∧e₃ |

## Modules

| Module | Description |
|--------|-------------|
| `basis` | Basis vector constructors and operations |
| `rotor` | Rotation operations using the sandwich product |
| `cayley` | Cayley table generation for geometric products |
| `precision` | High-precision floating-point types |
| `simd` | SIMD-accelerated operations (x86/x86_64) |
| `verified` | Phantom types for compile-time verification |
| `unicode_ops` | Unicode operator support (∧, ∨, etc.) |

## Mathematical Background

### Geometric Product

The geometric product combines the inner (dot) and outer (wedge) products:

```
ab = a·b + a∧b
```

For orthonormal basis vectors:
- `eᵢ·eᵢ = +1` (positive signature)
- `eᵢ·eᵢ = -1` (negative signature)
- `eᵢ·eᵢ = 0` (degenerate)
- `eᵢ·eⱼ = 0` for i ≠ j

### Rotors

Rotors encode rotations as even-grade multivectors. A rotation by angle θ in the plane defined by bivector B:

```
R = cos(θ/2) + sin(θ/2) * B̂
```

Apply rotation using the sandwich product:
```
v' = R v R†
```

## Performance

- **Memory Layout**: 32-byte aligned for AVX2 SIMD operations
- **Cayley Tables**: Precomputed for O(1) sign lookups
- **Cache Optimization**: Coefficients stored contiguously for cache efficiency
- **Zero-Cost Abstractions**: Const generics enable compile-time dimension checking

## License

Licensed under either of Apache License, Version 2.0 or MIT License at your option.

## Part of Amari

This crate is part of the [Amari](https://github.com/justinelliottcobb/Amari) mathematical computing library.
