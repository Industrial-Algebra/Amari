# amari-flynn

Probabilistic contracts and verification - named after Kevin Flynn's acceptance of spontaneous perfection.

## Overview

`amari-flynn` implements probabilistic verification, distinguishing between impossible, rare, and emergent events. Named after Kevin Flynn from Tron: Legacy, who discovered that spontaneous, imperfect emergence (the ISOs) represented a form of perfection beyond rigid determinism.

This library embodies that philosophy: formal verification should prove what's impossible while allowing rare, emergent possibilities.

## Features

- **Probabilistic Contracts**: Verify statistical properties of code
- **Event Classification**: Distinguish impossible (P=0), rare (0<P<<1), and emergent (P>0) events
- **Monte Carlo Verification**: Statistical verification of probability bounds
- **Distribution Types**: Uniform, Normal, Bernoulli, Exponential distributions
- **Procedural Macros**: `#[prob_requires]`, `#[prob_ensures]`, `#[ensures_expected]` with full multi-parameter support
- **SMT-LIB2 Backend**: Generate formal proof obligations for Z3, CVC5, and other solvers
- **Rare Event Tracking**: Monitor and bound low-probability events

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
amari-flynn = "0.19"
```

### Feature Flags

```toml
[dependencies]
# Default features (includes std)
amari-flynn = "0.19"

# Minimal, no-std compatible (disables file export)
amari-flynn = { version = "0.22", default-features = false }
```

## Quick Start

```rust
use amari_flynn::prelude::*;

// Create probabilistic values
let coin_flip = Prob::with_probability(0.5, true);

// Sample from distributions
let die_roll = Uniform::new(1, 6).sample();

// Track rare events
let miracle_shot = RareEvent::<()>::new(0.001, "critical_hit");
```

## The Flynn Philosophy

### Three Types of Events

Flynn teaches us to distinguish between three categories:

1. **Impossible** (P=0): Formally proven to never occur
   - Violates mathematical axioms
   - Proved unreachable via formal verification
   - System invariants guarantee exclusion

2. **Rare** (0 < P << 1): Bounded probability, tracked and verified
   - Low but non-zero probability
   - Statistical bounds verified via Monte Carlo
   - Tracked as legitimate possibilities

3. **Emergent** (P > 0): Possible but not prescribed
   - Not predicted or designed
   - Arise spontaneously from system rules
   - The "ISOs" of your system

## Probabilistic Contracts

### Using Macros

```rust
use amari_flynn::prelude::*;
use amari_flynn::{prob_requires, prob_ensures, ensures_expected};

// Precondition: x > 0 holds with P >= 0.95
#[prob_requires(x > 0.0, 0.95)]
fn compute(x: f64) -> f64 {
    x.sqrt()
}

// Postcondition: result is non-negative with P >= 0.99
#[prob_ensures(result >= 0.0, 0.99)]
fn safe_compute(x: f64) -> f64 {
    x.abs()
}

// Multi-parameter functions are fully supported
#[prob_requires(x > 0.0 && y > 0.0, 0.9)]
fn product_positive(x: f64, y: f64) -> f64 {
    x * y
}

// Expected value: result should average to 0.5 +/- 0.15
#[ensures_expected(result, 0.5, 0.15)]
fn biased_coin() -> f64 {
    if rand::random::<bool>() { 1.0 } else { 0.0 }
}
```

### Monte Carlo Verification

```rust
use amari_flynn::prelude::*;

// Create verifier with 10,000 samples
let verifier = MonteCarloVerifier::new(10_000);

// Verify a probability bound
let result = verifier.verify_probability_bound(
    || rand::random::<f64>() > 0.9, // Event: random > 0.9
    0.15, // Should occur with P <= 0.15
);

match result {
    VerificationResult::Verified => println!("Bound verified!"),
    VerificationResult::Violated => println!("Bound violated!"),
    VerificationResult::Inconclusive => println!("Need more samples"),
}
```

## SMT-LIB2 Formal Verification

Generate proof obligations in SMT-LIB2 format for external solvers like Z3 and CVC5. The SMT backend uses `QF_NRA` (quantifier-free nonlinear real arithmetic) logic.

### Generating Proof Obligations

```rust
use amari_flynn::prelude::*;

// Hoeffding bound: verify that P(|X_bar - mu| > epsilon) <= delta
let obligation = SmtProofObligation::hoeffding_obligation(
    "sample_mean_bound", 1000, 0.1, 0.05,
);

// Export as SMT-LIB2 string (for Z3, CVC5, etc.)
let smt_output = obligation.to_smtlib2();
println!("{}", smt_output);

// Or write directly to a .smt2 file
obligation.write_to_file("proof.smt2").unwrap();
```

### Convenience Constructors

```rust
use amari_flynn::prelude::*;

// Precondition bound
let pre = precondition_obligation("input_valid", "x > 0", 0.95);

// Postcondition bound
let post = postcondition_obligation("output_safe", "result >= 0", 0.99);

// Expected value verification
let ev = expected_value_obligation("fair_coin", 0.5, 0.05, 10000);

// Each can be verified statistically as a bridge to formal methods
let result = pre.verify_with_monte_carlo(10_000);
```

### Custom Obligations

```rust
use amari_flynn::prelude::*;

let mut ob = SmtProofObligation::new(
    "custom_bound",
    "Verify tail probability is small",
    ObligationKind::ConcentrationBound { samples: 500, epsilon: 0.05 },
);
ob.add_variable("alpha", SmtSort::Real);
ob.add_assertion("(> alpha 0.0)", Some("alpha is positive".to_string()));

let smt = ob.to_smtlib2();
// Feed to Z3: z3 -smt2 output.smt2
// If result is "unsat", the property holds
```

## Key Types

### Prob<T>

Probabilistic values with associated probability:

```rust
use amari_flynn::prelude::*;

// Value with 70% probability
let likely = Prob::with_probability(0.7, "success");

// Map preserves probability
let doubled = Prob::with_probability(0.5, 10).map(|v| v * 2);
assert_eq!(doubled.into_inner(), 20);
assert_eq!(doubled.probability(), 0.5);

// Monadic bind multiplies probabilities (independence assumed)
let combined = Prob::with_probability(0.5, 10)
    .and_then(|x| Prob::with_probability(0.4, x + 5));
assert_eq!(combined.probability(), 0.2); // 0.5 * 0.4
```

### Distributions

```rust
use amari_flynn::prelude::*;

// Uniform distribution over [1, 6]
let die = Uniform::new(1, 6);

// Normal distribution N(0, 1)
let standard_normal = Normal::new(0.0, 1.0);

// Bernoulli with P(true) = 0.3
let coin = Bernoulli::new(0.3);

// Exponential with rate lambda = 2.0
let waiting_time = Exponential::new(2.0);
```

### RareEvent<T>

Track events that should occur rarely:

```rust
use amari_flynn::prelude::*;

// Critical hit with 1% chance
let crit = RareEvent::<()>::new(0.01, "critical_hit");
assert!(crit.is_rare(0.05)); // rare relative to 5% threshold

// Classify events
use amari_flynn::contracts::EventVerification;
let class = EventVerification::classify(0.001, 0.01);
// Returns EventVerification::Rare
```

## Modules

| Module | Description |
|--------|-------------|
| `prob` | Core Prob<T> type and probability operations |
| `distributions` | Statistical distributions (Uniform, Normal, Bernoulli, Exponential) |
| `contracts` | ProbabilisticContract, RareEvent, VerificationResult, EventVerification |
| `backend::monte_carlo` | Monte Carlo statistical verification |
| `backend::smt` | SMT-LIB2 proof obligation generation and export |
| `backend::why3` | *(deprecated)* Use `backend::smt` instead |
| `statistical` | Statistical bounds (Hoeffding, Chernoff) and estimators |

## Use Cases

- **Game Mechanics**: Verify critical hit rates, loot drops stay within bounds
- **Reliability Engineering**: Bound failure probabilities in distributed systems
- **Financial Modeling**: Verify risk bounds in Monte Carlo simulations
- **Quality Assurance**: Statistical testing of randomized algorithms
- **Formal Methods**: Generate SMT-LIB2 proof obligations for automated theorem provers

## The ISO Philosophy

> "The ISOs, they were a miracle. They weren't meant to be - they just were."
>
> - Kevin Flynn

Like the ISOs in Tron: Legacy, the most valuable behaviors in a system are often those that emerge spontaneously, unpredicted by design. Flynn enables you to prove safety boundaries while preserving space for emergence.

## Roadmap

- **External Solver Integration**: Automatic invocation of installed Z3/CVC5 solvers
- **Creusot Support**: Rust-native formal verification
- **Geometric Integration**: Probabilistic contracts over geometric algebra types

## License

Licensed under either of Apache License, Version 2.0 or MIT License at your option.

## Part of Amari

This crate is part of the [Amari](https://github.com/justinelliottcobb/Amari) mathematical computing library.
