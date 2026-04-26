# amari-enumerative

Enumerative geometry for counting geometric configurations.

## Overview

`amari-enumerative` provides tools for enumerative geometry, the mathematical discipline concerned with counting geometric objects satisfying given conditions. The crate implements intersection theory, Schubert calculus, Littlewood-Richardson coefficients, Gromov-Witten invariants, and tropical curve counting.

## Features

- **Intersection Theory**: Chow rings, intersection multiplicities, Bézout's theorem
- **Schubert Calculus**: Computations on Grassmannians and flag varieties
- **Littlewood-Richardson Coefficients**: Complete LR coefficient computation via Young tableaux
- **Gromov-Witten Theory**: Curve counting and quantum cohomology
- **Tropical Geometry**: Tropical curve counting via correspondence theorems
- **Tropical Schubert Calculus**: Fast intersection counting using tropical methods
- **Moduli Spaces**: Computations on moduli spaces of curves
- **Namespace/Capabilities**: Geometric access control via Schubert calculus
- **CGT Bridge**: Growth-sequence adapters for small `amari-cgt` layer reports
- **WDVV/Kontsevich Recursion**: Genus-0 rational curve counting via WDVV equations
- **Equivariant Localization**: Atiyah-Bott fixed point formula on Grassmannians
- **Matroid Theory**: Uniform/Schubert matroids, duality, deletion, contraction, Tutte polynomials
- **CSM Classes**: Chern-Schwartz-MacPherson classes and Euler characteristics
- **Operadic Composition**: Compose namespaces along input/output interfaces
- **Stability Conditions**: Bridgeland-type stability and wall-crossing phenomena
- **Phantom Types**: Compile-time verification of mathematical properties
- **Parallel Computation**: Rayon-based parallelization for batch operations

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
amari-enumerative = "0.19.1"
```

### Feature Flags

```toml
[dependencies]
# Default features
amari-enumerative = "0.19.1"

# With serialization
amari-enumerative = { version = "0.19.1", features = ["serde"] }

# With parallel computation (Rayon)
amari-enumerative = { version = "0.19.1", features = ["parallel"] }

# With tropical Schubert calculus
amari-enumerative = { version = "0.19.1", features = ["tropical-schubert"] }

# With the amari-cgt enumeration bridge
amari-enumerative = { version = "0.19.1", features = ["cgt-bridge"] }

# For WASM targets
amari-enumerative = { version = "0.19.1", features = ["wasm"] }

# All performance features
amari-enumerative = { version = "0.19.1", features = ["parallel", "tropical-schubert"] }
```

## Quick Start

```rust
use amari_enumerative::{ProjectiveSpace, ChowClass, IntersectionRing};

// Create projective 2-space (P²)
let p2 = ProjectiveSpace::new(2);

// Define two curves by degree
let cubic = ChowClass::hypersurface(3);   // Degree 3 curve
let quartic = ChowClass::hypersurface(4); // Degree 4 curve

// Compute intersection number using Bézout's theorem
let intersection = p2.intersect(&cubic, &quartic);
assert_eq!(intersection.multiplicity(), 12); // 3 × 4 = 12 points
```

## Key Concepts

### Schubert Calculus

Count linear subspaces satisfying incidence conditions:

```rust
use amari_enumerative::{SchubertCalculus, SchubertClass, IntersectionResult};

// How many lines meet 4 general lines in projective 3-space?
// This is computed in Gr(2, 4) with 4 copies of σ_1
let mut calc = SchubertCalculus::new((2, 4)); // Gr(2,4)
let sigma_1 = SchubertClass::new(vec![1], (2, 4)).unwrap();

let classes = vec![sigma_1.clone(), sigma_1.clone(), sigma_1.clone(), sigma_1.clone()];
let result = calc.multi_intersect(&classes);

assert_eq!(result, IntersectionResult::Finite(2)); // Answer: 2 lines!
```

### Littlewood-Richardson Coefficients

Compute structure constants for Schubert calculus:

```rust
use amari_enumerative::{lr_coefficient, schubert_product, Partition};

// Compute c^ν_{λμ} - the LR coefficient
let lambda = Partition::new(vec![2, 1]);
let mu = Partition::new(vec![1, 1]);
let nu = Partition::new(vec![3, 2]);

let coeff = lr_coefficient(&lambda, &mu, &nu);

// Expand a Schubert product: σ_λ · σ_μ = Σ c^ν_{λμ} σ_ν
let products = schubert_product(&lambda, &mu, (3, 6));
for (partition, coefficient) in products {
    println!("σ_{:?} with coefficient {}", partition.parts, coefficient);
}
```

### Namespace and Capabilities (Geometric Access Control)

Use enumerative geometry for access control:

```rust
use amari_enumerative::{
    Namespace, Capability, CapabilityId, IntersectionResult,
    namespace_intersection, capability_accessible,
};

// Create a namespace in Gr(2, 4)
let mut ns = Namespace::full("agent", 2, 4).unwrap();

// Grant capabilities (each is a Schubert condition)
let read = Capability::new("read", "Read Access", vec![1], (2, 4)).unwrap();
let write = Capability::new("write", "Write Access", vec![1], (2, 4))
    .unwrap()
    .requires(CapabilityId::new("read")); // Dependency

ns.grant(read).unwrap();
ns.grant(write).unwrap();

// Count valid configurations
let count = ns.count_configurations();
// With 2 capabilities of codimension 1 each, we have dimension 2
```

### Phantom Types for Compile-Time Verification

Zero-cost type markers for mathematical properties:

```rust
use amari_enumerative::{
    ValidPartition, UnvalidatedPartition,
    Semistandard, LatticeWord,
    Transverse, Excess,
    FitsInBox,
};

// These types encode mathematical properties at the type level:
// - ValidPartition: Partition has been validated (weakly decreasing, positive parts)
// - Semistandard: Tableau satisfies semistandard conditions
// - LatticeWord: Tableau satisfies lattice word (Yamanouchi) condition
// - Transverse: Intersection is transverse (codimensions sum to dimension)
// - FitsInBox: Partition fits in k × (n-k) Grassmannian box
```

### Parallel Batch Operations

When the `parallel` feature is enabled:

```rust
use amari_enumerative::{
    lr_coefficients_batch, multi_intersect_batch,
    count_configurations_batch, Partition, SchubertClass,
};

// Compute many LR coefficients in parallel
let triples = vec![
    (Partition::new(vec![2, 1]), Partition::new(vec![1]), Partition::new(vec![3, 1])),
    (Partition::new(vec![1]), Partition::new(vec![1]), Partition::new(vec![2])),
    // ... many more
];
let coefficients = lr_coefficients_batch(&triples);

// Compute multiple Schubert intersections in parallel
let sigma_1 = SchubertClass::new(vec![1], (2, 4)).unwrap();
let batches = vec![
    (vec![sigma_1.clone(); 4], (2, 4)), // σ_1^4 in Gr(2,4)
    (vec![sigma_1.clone(); 6], (2, 5)), // σ_1^6 in Gr(2,5)
];
let results = multi_intersect_batch(&batches);
```

### Tropical Schubert Calculus

Fast intersection counting using tropical methods (requires `tropical-schubert` feature):

```rust
use amari_enumerative::{
    tropical_intersection_count, tropical_convexity_check,
    TropicalSchubertClass, TropicalResult, SchubertClass,
};

let sigma_1 = SchubertClass::new(vec![1], (2, 4)).unwrap();
let classes = vec![sigma_1.clone(); 4];

// Tropical methods give exact answers for many practical cases
let result = tropical_intersection_count(&classes, (2, 4));
assert_eq!(result, TropicalResult::Finite(2));

// Check if conditions are satisfiable
let tropical_classes: Vec<_> = classes.iter()
    .map(TropicalSchubertClass::from_classical)
    .collect();
assert!(tropical_convexity_check(&tropical_classes, 2, 4));
```

### `amari-cgt` Bridge

When the `cgt-bridge` feature is enabled, `amari-enumerative` can adapt
small `amari-cgt` layer reports into enumerative growth sequences:

```rust
use amari_enumerative::cgt_birthday_growth_summary;

let summary = cgt_birthday_growth_summary(1).unwrap();
assert_eq!(summary.raw_sequence(), vec![(0, 1), (1, 3)]);
assert_eq!(summary.numeric_sequence(), vec![(0, 1), (1, 2)]);
```

This is intended as a lightweight bridge for studying:

- raw vs canonical class growth
- numeric vs non-numeric short-game counts
- impartial vs partizan layer counts
- exact small-universe growth sequences by birthday or node count

### Gromov-Witten Invariants

Count curves in algebraic varieties:

```rust
use amari_enumerative::{GromovWittenInvariant, QuantumCohomology};

// Count rational curves of degree d in P²
let gw = GromovWittenInvariant::new(variety, degree);
let count = gw.compute_with_insertions(&insertions)?;
```

### WDVV/Kontsevich Curve Counting

Count rational curves via Kontsevich's recursion:

```rust
use amari_enumerative::WDVVEngine;

let mut engine = WDVVEngine::new();

// N_d = number of rational degree-d curves in P² through 3d-1 general points
assert_eq!(engine.rational_curve_count(1), 1);   // 1 line through 2 points
assert_eq!(engine.rational_curve_count(2), 1);   // 1 conic through 5 points
assert_eq!(engine.rational_curve_count(3), 12);  // 12 cubics through 8 points
assert_eq!(engine.rational_curve_count(4), 620); // 620 quartics through 11 points

// Required points for dimension constraint: 3d + g - 1
let points = WDVVEngine::required_point_count(5, 0); // 14 for degree 5 genus 0

// Table of all computed values
let table = engine.table(); // [(1,1), (2,1), (3,12), (4,620), ...]
```

### Equivariant Localization

Compute intersection numbers via the Atiyah-Bott fixed point formula:

```rust
use amari_enumerative::{EquivariantLocalizer, TorusWeights};

// Create localizer for Gr(2,4) with standard torus weights
let localizer = EquivariantLocalizer::new(2, 4);

// Number of T-fixed points = C(n,k)
assert_eq!(localizer.fixed_point_count(), 6); // C(4,2) = 6

// Verify σ_1^4 = 2 via localization
let classes = vec![vec![1], vec![1], vec![1], vec![1]];
let result = localizer.localized_intersection(&classes);
assert!((result - 2.0).abs() < 1e-10);
```

### Matroid Theory

Work with uniform matroids, duality, and rank functions:

```rust
use amari_enumerative::Matroid;

// Create uniform matroid U_{2,4}: every 2-element subset is a basis
let m = Matroid::uniform(2, 4);
assert_eq!(m.rank(), 2);
assert_eq!(m.ground_set_size(), 4);
assert_eq!(m.num_bases(), 6); // C(4,2)

// Matroid duality: rank of dual = n - k
let dual = m.dual();
assert_eq!(dual.rank(), 2); // 4 - 2

// Deletion and contraction
let deleted = m.delete(0);
let contracted = m.contract(0);
```

### Stability Conditions & Wall-Crossing

Bridgeland-type stability and wall-crossing phenomena:

```rust
use amari_enumerative::{StabilityCondition, WallCrossingEngine};

// Create stability condition on Gr(2,4) with trust level
let condition = StabilityCondition::new(2, 4, 0.8);

// Wall-crossing engine
let engine = WallCrossingEngine::new(2, 4);
let walls = engine.compute_walls(&namespace);

// Stable count changes at wall-crossing values
let count_before = engine.stable_count_at(&namespace, 0.49);
let count_after = engine.stable_count_at(&namespace, 0.51);
```

### Tropical Curves

Use tropical geometry for curve counting:

```rust
use amari_enumerative::tropical_curves::TropicalCurve;

let curve = TropicalCurve::new(degree, genus);
let count = curve.tropical_count()?;
```

## Modules

| Module | Description |
|--------|-------------|
| `intersection` | Chow rings, intersection products, Bézout |
| `schubert` | Schubert calculus on Grassmannians |
| `littlewood_richardson` | LR coefficients, partitions, Young tableaux |
| `namespace` | Namespace/Capability types for geometric access control |
| `phantom` | Compile-time verification phantom types |
| `gromov_witten` | Curve counting, quantum cohomology |
| `tropical_curves` | Tropical geometry methods |
| `tropical_schubert` | Tropical Schubert calculus (feature-gated) |
| `moduli_space` | Moduli spaces of curves |
| `higher_genus` | Higher genus curve counting, DT/PT invariants |
| `geometric_algebra` | Integration with geometric algebra |
| `wdvv` | WDVV/Kontsevich recursion for rational curve counts |
| `localization` | Equivariant localization on Grassmannians |
| `matroid` | Matroid theory: uniform, Schubert, duality, Tutte |
| `csm` | Chern-Schwartz-MacPherson classes |
| `operad` | Operadic composition of namespaces |
| `stability` | Bridgeland stability conditions and wall-crossing |

## Mathematical Background

### Littlewood-Richardson Rule

The LR coefficient c^ν_{λμ} counts semistandard Young tableaux of skew shape ν/λ with content μ satisfying the lattice word condition:

```
σ_λ · σ_μ = Σ_ν c^ν_{λμ} σ_ν
```

### Chow Rings

The Chow ring A*(X) captures the intersection theory of a variety X:

```
A*(P^n) = Z[H] / (H^(n+1))
```

where H is the hyperplane class.

### Schubert Cells

The Grassmannian Gr(k,n) has a cell decomposition by Schubert cells:

```
Gr(k,n) = ⊔ Ω_λ
```

indexed by partitions λ ⊂ (n-k)^k.

### Gromov-Witten Theory

GW invariants count curves via:

```
⟨τ_a1(γ1),...,τ_an(γn)⟩_{g,β} = ∫_{[M̄_{g,n}(X,β)]^{vir}} ψ_1^{a1} ev_1*(γ1) ∧ ...
```

## Classic Enumerative Problems

| Problem | Answer |
|---------|--------|
| Lines through 2 points | 1 |
| Conics through 5 points | 1 |
| Lines meeting 4 general lines in P³ | 2 |
| Rational cubics through 8 points in P² | 12 |
| Lines on a smooth cubic surface | 27 |
| Rational quartics through 11 points in P² | 620 |
| Rational quintics through 14 points in P² | 87,304 |

## Performance

- **Parallel Computation**: Rayon-based parallelization for batch operations
- **Tropical Acceleration**: Fast counting via tropical correspondence
- **Batch Processing**: Process multiple computations simultaneously
- **Caching**: LR coefficients and intersection numbers are cached

## Contracts and Verification

The crate uses Creusot-style contracts documented in function signatures:

```rust
/// Compute LR coefficient c^ν_{λμ}
///
/// # Contract
/// ```text
/// requires: lambda, mu, nu are valid partitions
/// ensures: result >= 0
/// ensures: |nu| != |lambda| + |mu| => result == 0
/// ensures: !nu.contains(lambda) => result == 0
/// ```
pub fn lr_coefficient(lambda: &Partition, mu: &Partition, nu: &Partition) -> u64
```

## License

Licensed under either of Apache License, Version 2.0 or MIT License at your option.

## Part of Amari

This crate is part of the [Amari](https://github.com/justinelliottcobb/Amari) mathematical computing library.
