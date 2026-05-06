# amari-dual

Forward-mode dual number automatic differentiation for optimization workloads.

## Overview

`amari-dual` implements dual numbers for forward-mode automatic differentiation. Dual numbers extend real numbers with an infinitesimal unit ε where ε² = 0, enabling exact derivative computation without numerical approximation or computational graphs. The crate is aimed at local sensitivity analysis, heuristic tuning, and small optimization loops rather than large reverse-mode graph systems.

## Features

- **DualNumber**: Single-variable automatic differentiation
- **MultiDualNumber**: Heap-backed multi-variable gradient computation
- **StaticMultiDual**: Fixed-size const-generic gradients for small hot loops
- **BranchPolicy**: Explicit tie behavior for `max` / `min` style branch points
- **DualMultivector**: Integration with geometric algebra
- **Mathematical Functions**: Differentiable sin, cos, exp, log, and more
- **High-Precision Support**: Optional extended precision arithmetic
- **no_std Support**: Usable in embedded and WASM environments

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
amari-dual = "0.21.0"
```

### Feature Flags

```toml
[dependencies]
# Default features
amari-dual = "0.21.0"

# With serialization
amari-dual = { version = "0.21.0", features = ["serialize"] }

# With GPU acceleration
amari-dual = { version = "0.21.0", features = ["gpu"] }

# High-precision arithmetic
amari-dual = { version = "0.21.0", features = ["high-precision"] }
```

## Quick Start

### Single-Variable Differentiation

```rust
use amari_dual::DualNumber;

// Create a dual number: x = 3 with derivative seed 1
let x = DualNumber::new(3.0, 1.0);

// Compute f(x) = x²
let f = x * x;

// Extract value and derivative
println!("f(3) = {}", f.value());       // 9.0
println!("f'(3) = {}", f.derivative()); // 6.0 (2x at x=3)
```

### Multi-Variable Gradients

```rust
use amari_dual::MultiDualNumber;

// Variables: x=2, y=3
let vars = MultiDualNumber::variables(&[2.0, 3.0]);
let x = vars[0].clone();
let y = vars[1].clone();

// Compute f(x,y) = x² + xy
let f = x.clone() * x.clone() + x * y;

// Get the gradient
let gradient = f.get_gradient(); // [2x + y, x] = [7, 2]
```

### Constants

```rust
use amari_dual::DualNumber;

// For constants (derivative = 0)
let c = DualNumber::constant(5.0);

// Multiply: f(x) = 5x
let x = DualNumber::new(2.0, 1.0);
let result = c * x;
// value = 10, derivative = 5
```

## Mathematical Background

### Dual Numbers

A dual number has the form:

```
a + εb   where ε² = 0
```

Arithmetic operations:
- Addition: (a + εb) + (c + εd) = (a + c) + ε(b + d)
- Multiplication: (a + εb) × (c + εd) = ac + ε(ad + bc)

### Automatic Differentiation

When we evaluate f(a + ε), we get:

```
f(a + ε) = f(a) + εf'(a)
```

This gives us both the value f(a) and derivative f'(a) in a single pass through the computation.

### Advantages

| Approach | Accuracy | Memory | Complexity |
|----------|----------|--------|------------|
| Finite Differences | O(h) error | O(1) | O(n) evaluations |
| Symbolic | Exact | O(expression) | Can explode |
| **Dual Numbers** | **Exact** | **O(1)** | **O(1) overhead** |

## Key Types

### DualNumber<T>

Single-variable dual number for computing f and f':

```rust
use amari_dual::DualNumber;

let x = DualNumber::new(value, derivative_seed);
let value = x.value();
let deriv = x.derivative();
```

### MultiDualNumber<T>

Heap-backed multi-variable dual number for computing gradients:

```rust
use amari_dual::MultiDualNumber;

let x = MultiDualNumber::new(value, gradient_seeds);
let value = x.get_value();
let gradient = x.get_gradient();
let n_vars = x.n_vars();
```

### StaticMultiDual<T, N>

Fixed-size multi-variable dual number for small optimization loops:

```rust
use amari_dual::StaticMultiDual;

let [alpha, beta] = StaticMultiDual::<f64, 2>::variables([0.5, 1.25]);
let score = alpha * alpha + StaticMultiDual::constant(3.0) * beta;

assert_eq!(score.get_gradient(), &[1.0, 3.0]);
```

### Branch-sensitive `max` / `min`

```rust
use amari_dual::{BranchPolicy, DualNumber};

let left = DualNumber::new(1.0, 2.0);
let right = DualNumber::new(1.0, 6.0);

// Backward-compatible default: left-biased on ties
assert_eq!(left.max(right).derivative(), 2.0);

// Explicit tie handling when equality cases matter
assert_eq!(left.max_by_policy(right, BranchPolicy::Average).derivative(), 4.0);
```

### DualMultivector

Dual numbers integrated with geometric algebra:

```rust
use amari_dual::DualMultivector;

// Differentiable geometric algebra operations
let mv = DualMultivector::new(/* ... */);
```

## Scope Boundaries

- `amari-dual` remains a forward-mode AD crate; it does not provide reverse-mode graph AD in `0.21.0`.
- `MultiDualNumber` stays the flexible heap-backed carrier, while `StaticMultiDual` is the fixed-size companion for small hot loops.
- Branch policies make tie behavior explicit for piecewise `max` / `min` style operators rather than pretending those cases are globally smooth.

## Modules

| Module | Description |
|--------|-------------|
| `types` | DualNumber, MultiDualNumber, StaticMultiDual, and branch policy types |
| `functions` | Differentiable mathematical functions (sin, cos, exp, log) |
| `multivector` | Integration with geometric algebra multivectors |
| `verified` | Phantom types for compile-time verification |
| `error` | Error types for dual operations |

## Differentiable Functions

```rust
use amari_dual::{DualNumber, functions};

let x = DualNumber::new(1.0, 1.0);

// Trigonometric
let sin_x = functions::sin(x);  // sin(1), cos(1)
let cos_x = functions::cos(x);  // cos(1), -sin(1)

// Exponential and logarithm
let exp_x = functions::exp(x);  // e¹, e¹
let ln_x = functions::ln(x);    // ln(1)=0, 1/1=1

// Power
let x_squared = x * x;          // 1, 2
```

## Historical Note: v0.12 API Changes

The API was updated in v0.12.0 for better encapsulation:

```rust
// Before (v0.11.x)
let x = DualNumber { real: 3.0, dual: 1.0 };
let value = x.real;
let deriv = x.dual;

// After (v0.12.0+)
let x = DualNumber::new(3.0, 1.0);
let value = x.value();
let deriv = x.derivative();
```

## Performance

- **Predictable overhead**: Dual arithmetic tracks value and derivative together without separate graph construction
- **Single pass**: Compute value and derivative together
- **Cache friendly**: Data locality for dual number pairs
- **SIMD potential**: Parallel computation of value and derivative

## Use Cases

- **Machine Learning**: Backpropagation and gradient descent
- **Scientific Computing**: Sensitivity analysis
- **Optimization**: Gradient-based methods and heuristic tuning
- **Compiler / Interpreter Workloads**: Small parameter loops and branch-sensitive scoring
- **Physics Simulation**: Computing forces from potentials

## License

Licensed under either of Apache License, Version 2.0 or MIT License at your option.

## Part of Amari

This crate is part of the [Amari](https://github.com/justinelliottcobb/Amari) mathematical computing library.
