# Amari v0.22.0

**Comprehensive Mathematical Computing Platform with Geometric Algebra, Differential Calculus, Measure Theory, Probability Theory, Functional Analysis, Algebraic Topology, Dynamical Systems, and Vector Symbolic Architectures**

A unified mathematical computing library featuring geometric algebra, differential calculus, measure theory, probability theory on geometric spaces, functional analysis (Hilbert spaces, operators, spectral theory), algebraic topology (homology, persistent homology, Morse theory), dynamical systems analysis (ODE solvers, stability, bifurcations, chaos, Lyapunov exponents), relativistic physics, tropical algebra, automatic differentiation, holographic associative memory (Vector Symbolic Architectures), optical field operations for holographic displays, and information geometry. The library provides multi-GPU infrastructure with intelligent workload distribution and complete WebAssembly support for browser deployment. Version 0.22.0 releases the `amari-cgt` and `amari-surreal` exact game-theoretic crates while preserving the 0.21.0 tropical/dual extension surface and the 0.20.0 correctness-first GPU stabilization baseline.

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![WebAssembly](https://img.shields.io/badge/WebAssembly-Ready-blue.svg)](https://webassembly.org/)
[![TypeScript](https://img.shields.io/badge/TypeScript-5.0+-blue.svg)](https://www.typescriptlang.org/)
[![License](https://img.shields.io/badge/License-MIT%20OR%20Apache--2.0-green.svg)](LICENSE)

**[Live Examples Suite](https://amari-math.netlify.app)** | **[API Documentation](https://docs.rs/amari)** | **[MCP Server](https://github.com/justinelliottcobb/Amari-mcp)**


## v0.22.0 Highlights

- **`amari-cgt`** is now a first-class short combinatorial game theory crate with arena-backed games, canonicalization, outcome/comparison helpers, nimbers, bounded/exact layer generation, and growth-reporting utilities.
- **`amari-surreal`** is now a first-class short-surreal crate with exact dyadic arithmetic, numeric-game validation over `amari-cgt`, simplest-number construction, and game/value round trips for supported finite cuts.
- **`amari-enumerative`** includes an opt-in `cgt-bridge` feature for adapting CGT layer-analysis reports into enumerative growth summaries.
- **`amari-wasm`** exposes release-focused CGT/surreal bindings to TypeScript: `WasmCgtArena`, `WasmGameInspection`, `WasmDyadic`, and `WasmShortSurreal`.
- **Scope remains explicit**: the release is about short normal-play games and short surreal numbers; loopy games, misère play, thermographs, hyperreal analysis, symbolic infinite surreal universes, and surcomplex arithmetic remain deferred.

## v0.21.0 Highlights

- **`amari-tropical`** now has a clearer optimization-facing split between float max-plus carriers and an arena-backed ordinal substrate below `ε₀` via `OrdinalArena`, `OrdinalId`, `CnfTerm`, and `OrdinalWeight`.
- **`amari-dual`** now includes explicit `BranchPolicy` handling, `MultiDualNumber::variables(...)` seed helpers, and `StaticMultiDual<T, const N: usize>` for small hot loops.
- **`amari-wasm`** exposes the new tropical/dual extension surface to TypeScript: semiring folds, ordinal arenas/weights, branch-policy min/max, multi-dual seeding, and fixed-size `WasmStaticMultiDual2/3/4` wrappers.
- **Examples and API docs** now include release-focused `0.21.0` tropical/dual workflows across Rust and WebAssembly.
- **Scope remains explicit**: the ordinal layer does not claim broader transfinite arithmetic, `viterbi` and `polytope` remain float-specific, and `amari-dual` remains a forward-mode AD crate.

## Features

### Core Mathematical Systems

- **Geometric Algebra (Clifford Algebra)**: Multivectors, rotors, and geometric products for 3D rotations and spatial transformations
- **Differential Calculus**: Unified geometric calculus with scalar/vector fields, gradients, divergence, curl, and Lie derivatives
- **Measure Theory**: Sigma-algebras, measurable functions, integration on geometric spaces, and probability measures
- **Probability Theory**: Distributions on multivector spaces, stochastic processes, MCMC sampling, and Bayesian inference
- **Functional Analysis**: Hilbert spaces, bounded operators, spectral decomposition, eigenvalue algorithms, and Sobolev spaces
- **Algebraic Topology**: Simplicial complexes, homology computation, persistent homology (TDA), Morse theory, and fiber bundles
- **Dynamical Systems**: ODE solvers (RK4, RKF45, Dormand-Prince), stability analysis, bifurcation diagrams, Lyapunov exponents, phase portraits, and attractor characterization
- **Vector Symbolic Architectures**: Holographic Reduced Representations (HRR), binding algebras, and associative memory
- **Optical Field Operations**: GA-native Lee hologram encoding for DMD displays, rotor field algebra, and VSA on optical wavefronts
- **Relativistic Physics**: Complete spacetime algebra (Cl(1,3)) with Minkowski signature for relativistic calculations
- **Tropical Algebra** *(v0.21.0 extension focus)*: Max-plus semiring operations plus ordinal-weighted optimization carriers below `ε₀`, semiring folds, and ordinal-weight best/compose helpers
- **Automatic Differentiation** *(v0.21.0 extension focus)*: Forward-mode AD with dual numbers, branch-policy min/max semantics, heap-backed multi-dual seeding, and fixed-size static gradients
- **Fusion Systems**: Tropical-dual-Clifford fusion combining three algebraic systems
- **Information Geometry**: Statistical manifolds, KL/JS divergences, and Fisher information
- **Optimization**: Gradient-based optimization with geometric constraints
- **Network Analysis**: Geometric network analysis and graph neural networks
- **Cellular Automata**: Geometric automata with configurable rules
- **Enumerative Geometry**: Intersection theory, Schubert calculus, LR coefficients, WDVV curve counting, matroids, CSM classes, equivariant localization, operadic composition, and stability conditions
- **GF(2) Algebra**: Finite field GF(2) linear algebra, binary Clifford algebra Cl(N,R; F₂), coding theory (Hamming, Reed-Muller, Golay codes), Grassmannian combinatorics, matroid representability, and Kazhdan-Lusztig polynomials
- **Probabilistic Contracts**: SMT-LIB2 proof obligation generation, Monte Carlo statistical verification, probabilistic value tracking, and rare event classification
- **Combinatorial Game Theory** *(v0.22.0 extension focus)*: Short normal-play games, canonical forms, outcome classes, nimbers, and small exact-layer generation via `amari-cgt`
- **Short Surreal Numbers** *(v0.22.0 extension focus)*: Exact dyadic arithmetic, numeric-game validation, simplest-number construction, and game/value round trips via `amari-surreal`
- **Rewrite Systems** *(v0.23.0 development focus)*: Abstract rewriting systems, term rewriting systems, bounded inverse search, and anti-unification/rule inference via `amari-rewrite`
- **GPU Backend Stabilization** *(v0.20.0 baseline)*: Hardware-validated WebGPU acceleration with public API tests, CPU-baseline correctness checks, benchmark/crossover documentation, and explicit GPU-backed vs GPU-recommended guidance

### GPU Acceleration & Multi-GPU Infrastructure

- **Correctness-First GPU Backend**: Hardware-validated WebGPU kernels and documented CPU-baseline fallback paths retained from the 0.20.0 release line
- **Multi-GPU Architecture**: Infrastructure supporting up to 8 GPUs with intelligent workload distribution
- **Advanced Load Balancing**: Five strategies including Balanced, CapabilityAware, MemoryAware, LatencyOptimized, and Adaptive
- **Performance Profiling**: Timeline analysis with microsecond precision and automatic bottleneck detection
- **Benchmark-Informed Dispatch**: Manual crossover reports distinguish GPU-backed paths from GPU-recommended paths
- **Graceful Degradation**: Automatic fallback to single GPU or CPU when multi-GPU unavailable or CPU is preferred

### Platform Support

- **Native Rust**: High-performance execution with rug (GMP/MPFR) backend for high-precision arithmetic
- **WebAssembly**: Full-featured WASM bindings with dashu backend for browser compatibility
- **GPU Acceleration**: WebGPU support for large-scale parallel computations
- **TypeScript Support**: Complete TypeScript definitions included
- **Cross-Platform**: Linux, macOS, Windows, browsers, Node.js, and edge computing environments

## v0.22.0 Exact Game-Theoretic Extensions

The workspace now includes two opt-in exact game-theoretic crates released for the `0.22.0` cycle:

- **`amari-cgt`**: computational combinatorial game theory for short normal-play games, including canonicalization, nimbers, and small exhaustive layer generation
- **`amari-surreal`**: computable short surreal numbers built on validated numeric games from `amari-cgt`, with exact dyadic arithmetic

At the umbrella-crate level these are exposed through optional features:

```toml
[dependencies]
amari = { version = "0.22.0", features = ["cgt", "surreal"] }
```

`surreal` implies `cgt`, while `cgt` also enables the lightweight `amari-enumerative` bridge for CGT layer-growth summaries.

## Installation

### Rust Crates

Add to your `Cargo.toml`:

```toml
[dependencies]
# Complete library with all features
amari = "0.22.0"

# Or individual crates:

# Core geometric algebra and mathematical foundations
amari-core = "0.22.0"

# Differential calculus with geometric algebra
amari-calculus = "0.22.0"

# Measure theory and integration
amari-measure = "0.22.0"

# Probability theory on geometric algebra spaces
amari-probabilistic = "0.22.0"

# Functional analysis: Hilbert spaces, operators, spectral theory
amari-functional = "0.22.0"

# Algebraic topology: homology, persistent homology, Morse theory
amari-topology = "0.22.0"

# Dynamical systems: ODE solvers, stability, bifurcations, Lyapunov exponents
amari-dynamics = "0.22.0"

# Vector Symbolic Architectures, holographic memory, and optical fields
amari-holographic = "0.22.0"

# High-precision relativistic physics
amari-relativistic = { version = "0.22.0", features = ["high-precision"] }

# GPU acceleration (includes optical field GPU operations)
amari-gpu = "0.22.0"

# Optimization algorithms
amari-optimization = "0.22.0"

# Additional mathematical systems
amari-tropical = "0.22.0"    # max-plus semiring + ordinal weights below ε₀
amari-dual = "0.22.0"        # forward-mode AD + branch policies + static gradients
amari-info-geom = "0.22.0"
amari-automata = "0.22.0"
amari-fusion = "0.22.0"
amari-network = "0.22.0"
amari-enumerative = "0.22.0"
amari-cgt = "0.22.0"         # short combinatorial games + nimbers
amari-surreal = "0.22.0"    # exact short surreal numbers over numeric games
amari-rewrite = "0.22.0"    # ARS/TRS rewriting, inverse search, and rule inference

# Probabilistic verification contracts (SMT-LIB2, Monte Carlo)
amari-flynn = "0.22.0"

# Optional Rust-side WebAssembly crate
amari-wasm = "0.22.0"
```

### JavaScript/TypeScript (WebAssembly)

```bash
npm install @justinelliottcobb/amari-wasm@0.22.0
```

Or with yarn:

```bash
yarn add @justinelliottcobb/amari-wasm@0.22.0
```

## Quick Start

### Rust: Geometric Algebra

```rust
use amari_core::{Multivector, basis::Basis, rotor::Rotor};

// 3D Euclidean Clifford algebra Cl(3,0,0)
type Cl3 = Multivector<3, 0, 0>;

// Create basis vectors
let e1: Cl3 = Basis::e1();
let e2: Cl3 = Basis::e2();

// Geometric product: e1 * e2 = e1 ∧ e2 (bivector)
let e12 = e1.geometric_product(&e2);

// Create rotor for 90° rotation in xy-plane
let rotor = Rotor::from_bivector(&e12, std::f64::consts::PI / 2.0);

// Apply rotation: e1 → e2
let rotated = rotor.apply(&e1);
```

### Rust: Differential Calculus

```rust
use amari_calculus::{ScalarField, VectorField, VectorDerivative};
use amari_core::Multivector;

// Define a scalar field f(x,y,z) = x² + y² + z²
let field = ScalarField::<3, 0, 0>::new(|pos| {
    pos[0] * pos[0] + pos[1] * pos[1] + pos[2] * pos[2]
});

// Compute gradient at a point
let point = [1.0, 2.0, 3.0];
let gradient = field.gradient(&point, 1e-6);

// Define a vector field
let vector_field = VectorField::<3, 0, 0>::new(|pos| {
    [pos[1] * pos[2], pos[0] * pos[2], pos[0] * pos[1]]
});

// Compute divergence and curl
let derivative = VectorDerivative::<3, 0, 0>::new(1e-6);
let div = derivative.divergence(&vector_field, &point);
let curl = derivative.curl(&vector_field, &point);
```

### Rust: Tropical Algebra

```rust
use amari_tropical::TropicalNumber;

// Create tropical numbers using the constructor
let a = TropicalNumber::new(3.0);
let b = TropicalNumber::new(5.0);

// Tropical operations
let sum = a.tropical_add(&b);     // max(3, 5) = 5
let product = a.tropical_mul(&b); // 3 + 5 = 8

// Access values
println!("Tropical sum: {}", sum.value());
println!("Tropical product: {}", product.value());

// Tropical identities
let zero = TropicalNumber::<f64>::tropical_zero(); // -∞ (additive identity)
let one = TropicalNumber::<f64>::tropical_one();   // 0 (multiplicative identity)
```

### Rust: Tropical Ordinal Weights (v0.21.0)

```rust
use amari_tropical::{CnfTerm, OrdinalArena, OrdinalWeight};

let mut arena = OrdinalArena::new();
let one = arena.one();
let omega = arena.omega();
let omega_plus_one = arena.add(omega, one).unwrap();
let omega_squared = arena.intern_cnf(vec![CnfTerm::new(omega, 1)]).unwrap();

let weights = [
    OrdinalWeight::bottom(),
    OrdinalWeight::from_ordinal(omega_plus_one),
    OrdinalWeight::from_ordinal(omega_squared),
];

let best = arena.best_weight(&weights).unwrap();
println!("Best ordinal weight: {}", arena.format_weight(best).unwrap()); // ω^ω
```

### Rust: Automatic Differentiation

```rust
use amari_dual::DualNumber;

// Create dual number for differentiation
// f(x) = x² at x = 3, with seed derivative 1
let x = DualNumber::new(3.0, 1.0);

// Compute f(x) = x²
let result = x * x;

// Extract value and derivative
println!("f(3) = {}", result.value());       // 9.0
println!("f'(3) = {}", result.derivative()); // 6.0

// For constants (derivative = 0)
let c = DualNumber::constant(2.0);
let scaled = x * c; // 2x, derivative = 2
```

### Rust: Branch Policies and Static Gradients (v0.21.0)

```rust
use amari_dual::{BranchPolicy, DualNumber, MultiDualNumber, StaticMultiDual};

// Explicit derivative behavior at non-smooth branch ties.
let left = DualNumber::new(2.0, 1.0);
let right = DualNumber::new(2.0, 5.0);
let averaged = left.max_by_policy(right, BranchPolicy::Average);
assert_eq!(averaged.derivative(), 3.0);

// Heap-backed basis seeding for gradients.
let vars = MultiDualNumber::variables(&[2.0, 3.0]);
let x = vars[0].clone();
let y = vars[1].clone();
let score = x.clone() * x + y;
println!("gradient = {:?}", score.get_gradient());

// Fixed-size carrier for small hot loops.
let [sx, sy] = StaticMultiDual::<f64, 2>::variables([2.0, 3.0]);
let static_score = sx * sx + sy;
println!("static gradient = {:?}", static_score.get_gradient());
```

### Rust: Tropical-Dual-Clifford Fusion

```rust
use amari_fusion::TropicalDualClifford;

// Create from logits (common in ML applications)
let logits = vec![1.5, 2.0, 0.8, 1.2];
let tdc = TropicalDualClifford::<f64, 4>::from_logits(&logits);

// Evaluate using all three algebras simultaneously
let other = TropicalDualClifford::from_logits(&[2.0, 1.5, 1.0, 0.9]);
let evaluation = tdc.evaluate(&other);

// Extract features from each algebra
let tropical_features = tdc.extract_tropical_features(); // Fast path-finding
let dual_features = tdc.extract_dual_features();         // Automatic gradients

// Perform sensitivity analysis
let sensitivity = tdc.sensitivity_analysis();
let most_sensitive = sensitivity.most_sensitive(2);

println!("Combined score: {}", evaluation.combined_score);
println!("Most sensitive components: {:?}", most_sensitive);
```

### Rust: Vector Symbolic Architectures (Holographic Memory)

```rust
use amari_holographic::{HolographicMemory, ProductCl3x32, BindingAlgebra};

// Create holographic memory with 256-dimensional vectors
let mut memory: HolographicMemory<ProductCl3x32> = HolographicMemory::new();

// Generate random keys and values
let key1 = ProductCl3x32::random();
let value1 = ProductCl3x32::random();

// Store key-value associations via superposition
memory.store(&key1, &value1);

let key2 = ProductCl3x32::random();
let value2 = ProductCl3x32::random();
memory.store(&key2, &value2);

// Retrieve with a query key
let retrieved = memory.retrieve(&key1);
println!("Similarity to original: {:.3}", retrieved.similarity(&value1));

// Binding operations for role-filler structures
let bound = key1.bind(&value1);        // key ⊛ value
let recovered = bound.unbind(&key1);    // Approximately recovers value1
println!("Recovery similarity: {:.3}", recovered.similarity(&value1));

// Resonator network for cleanup/factorization
use amari_holographic::Resonator;
let codebook = vec![value1.clone(), value2.clone()];
let resonator = Resonator::new(codebook.clone());
let noisy = retrieved.clone(); // Noisy retrieval
let cleaned = resonator.cleanup(&noisy, 10); // 10 iterations
```

### Rust: Optical Field Operations (Lee Hologram Encoding)

```rust
use amari_holographic::optical::{
    OpticalRotorField, GeometricLeeEncoder, LeeEncoderConfig,
    OpticalFieldAlgebra, OpticalCodebook, TropicalOpticalAlgebra,
};

// Create optical rotor fields (phase + amplitude on a 2D grid)
let field_a = OpticalRotorField::random((256, 256), 42);
let field_b = OpticalRotorField::random((256, 256), 123);
let uniform = OpticalRotorField::uniform(0.0, 0.5, (256, 256));

println!("Field dimensions: {:?}", field_a.dimensions());
println!("Total energy: {:.4}", field_a.total_energy());

// Lee hologram encoding for DMD display
let config = LeeEncoderConfig::new((256, 256), 0.25); // carrier frequency = 0.25
let encoder = GeometricLeeEncoder::new(config);
let hologram = encoder.encode(&uniform);

println!("Hologram fill factor: {:.4}", hologram.fill_factor());

// Get binary data for hardware interface (bit-packed)
let binary_data = hologram.as_bytes();
println!("Packed binary size: {} bytes", binary_data.len());

// VSA operations on optical fields
let algebra = OpticalFieldAlgebra::new((256, 256));

// Bind two fields (rotor multiplication = phase addition)
let bound = algebra.bind(&field_a, &field_b);

// Compute similarity between fields
let similarity = algebra.similarity(&field_a, &field_a); // Self-similarity ≈ 1.0
println!("Self-similarity: {:.4}", similarity);

// Unbind to retrieve original field
let retrieved = algebra.unbind(&field_a, &bound);
let retrieval_sim = algebra.similarity(&retrieved, &field_b);
println!("Retrieval similarity: {:.4}", retrieval_sim);

// Seed-based symbol codebook for reproducible VSA
let mut codebook = OpticalCodebook::new((64, 64), 42);
codebook.register("AGENT");
codebook.register("ACTION");
codebook.register("TARGET");

let agent_field = codebook.get("AGENT");
let action_field = codebook.get("ACTION");
println!("Registered symbols: {:?}", codebook.symbols());

// Tropical operations for attractor dynamics
let tropical = TropicalOpticalAlgebra::new((64, 64));
let tropical_sum = tropical.tropical_add(&field_a, &field_b); // Pointwise min phase
let phase_distance = tropical.phase_distance(&field_a, &field_b);
println!("Phase distance: {:.4}", phase_distance);
```

### Rust: Functional Analysis (Hilbert Spaces & Spectral Theory)

```rust
use amari_functional::space::{MultivectorHilbertSpace, HilbertSpace, InnerProductSpace};
use amari_functional::operator::MatrixOperator;
use amari_functional::spectral::spectral_decompose;

// Create the Hilbert space Cl(2,0,0) ≅ ℝ⁴
let space: MultivectorHilbertSpace<2, 0, 0> = MultivectorHilbertSpace::new();

// Create orthogonal elements
let x = space.from_coefficients(&[1.0, 2.0, 0.0, 0.0]).unwrap();
let y = space.from_coefficients(&[0.0, 0.0, 3.0, 4.0]).unwrap();

// Inner product and norm
let ip = space.inner_product(&x, &y);  // 0.0 (orthogonal)
let norm_x = space.norm(&x);           // √5

// Create a symmetric matrix operator (Hamiltonian)
let hamiltonian = MatrixOperator::<2, 0, 0>::diagonal(&[0.0, 1.0, 2.0, 3.0]).unwrap();

// Spectral decomposition: H = Σᵢ λᵢ Pᵢ
let spectral = spectral_decompose(&hamiltonian, 100, 1e-10).unwrap();
let eigenvalues = spectral.eigenvalues();
println!("Energy levels: {:?}", eigenvalues.iter().map(|e| e.value).collect::<Vec<_>>());

// Functional calculus: compute e^{-βH} for thermal states
let beta = 0.5;
let thermal = spectral.apply_function(|E| (-beta * E).exp(), &x);
```

### Rust: Algebraic Topology (Homology & Persistent Homology)

```rust
use amari_topology::{SimplicialComplex, Simplex, Filtration, PersistentHomology};

// Build a simplicial complex (hollow tetrahedron = 2-sphere)
let mut sphere = SimplicialComplex::new();
sphere.add_simplex(Simplex::new(vec![0, 1, 2]));
sphere.add_simplex(Simplex::new(vec![0, 1, 3]));
sphere.add_simplex(Simplex::new(vec![0, 2, 3]));
sphere.add_simplex(Simplex::new(vec![1, 2, 3]));

// Compute Betti numbers
let betti = sphere.betti_numbers();
assert_eq!(betti[0], 1); // 1 connected component
assert_eq!(betti[1], 0); // No 1D holes
assert_eq!(betti[2], 1); // 1 void (sphere encloses space)

// Euler characteristic: χ = V - E + F = 4 - 6 + 4 = 2
assert_eq!(sphere.euler_characteristic(), 2);

// Persistent homology for topological data analysis
let mut filt = Filtration::new();
filt.add(0.0, Simplex::new(vec![0])); // Points born at t=0
filt.add(0.0, Simplex::new(vec![1]));
filt.add(0.0, Simplex::new(vec![2]));
filt.add(1.0, Simplex::new(vec![0, 1])); // Edge connects 0-1 at t=1
filt.add(2.0, Simplex::new(vec![1, 2])); // Edge connects 1-2 at t=2
filt.add(3.0, Simplex::new(vec![0, 2])); // Closes loop at t=3

let ph = PersistentHomology::compute(&mut filt).unwrap();
let betti_t3 = ph.diagram().betti_at(3.5);
// At t=3.5: 1 component (β₀=1), 1 loop (β₁=1)
```

### Rust: Probability on Geometric Algebra

```rust
use amari_probabilistic::distribution::{GaussianMultivector, Distribution, MultivectorDistribution};
use amari_probabilistic::stochastic::{GeometricBrownianMotion, StochasticProcess};

// Gaussian distribution on Cl(3,0,0) - 8-dimensional multivector space
let gaussian = GaussianMultivector::<3, 0, 0>::standard();

// Draw samples
let mut rng = rand::thread_rng();
let sample = gaussian.sample(&mut rng);
println!("Sample: {:?}", sample.to_vec());

// Evaluate log-probability
let log_p = gaussian.log_prob(&sample).unwrap();
println!("Log probability: {:.3}", log_p);

// Grade-concentrated distribution (e.g., only on bivectors)
let bivector_dist = GaussianMultivector::<3, 0, 0>::grade_concentrated(2, 1.0).unwrap();

// Geometric Brownian Motion on multivector space
let gbm = GeometricBrownianMotion::<3, 0, 0>::new(0.1, 0.2); // drift=0.1, volatility=0.2
let path = gbm.sample_path(0.0, 1.0, 100, &mut rng).unwrap();
println!("Path has {} points", path.len());

// MCMC sampling with Metropolis-Hastings
use amari_probabilistic::sampling::MetropolisHastings;
let sampler = MetropolisHastings::new(gaussian.clone(), 0.5); // step_size=0.5
let samples = sampler.sample_n(&mut rng, 1000);
println!("Drew {} MCMC samples", samples.len());
```

### Rust: Dynamical Systems (Chaos & Bifurcations)

```rust
use amari_dynamics::{
    LorenzSystem, VanDerPolOscillator, RungeKutta4, ODESolver,
    stability::{find_fixed_point, analyze_stability},
    lyapunov::compute_lyapunov_spectrum,
    bifurcation::{BifurcationDiagram, ContinuationConfig},
};
use amari_core::Multivector;

// Create classic Lorenz system (sigma=10, rho=28, beta=8/3)
let lorenz = LorenzSystem::classic();

// Initial condition as multivector
let mut initial: Multivector<3, 0, 0> = Multivector::zero();
initial.set(1, 1.0); // x
initial.set(2, 1.0); // y
initial.set(4, 1.0); // z

// Integrate trajectory with RK4
let solver = RungeKutta4::new();
let trajectory = solver.solve(&lorenz, initial.clone(), 0.0, 50.0, 5000).unwrap();

println!("Trajectory has {} points", trajectory.len());
let (x, y, z) = (
    trajectory.final_state().unwrap().get(1),
    trajectory.final_state().unwrap().get(2),
    trajectory.final_state().unwrap().get(4),
);
println!("Final state: ({:.3}, {:.3}, {:.3})", x, y, z);

// Compute Lyapunov exponents for chaos detection
let spectrum = compute_lyapunov_spectrum(&lorenz, &initial, 10000, 0.01).unwrap();
println!("Lyapunov exponents: {:?}", spectrum.exponents);
println!("Sum: {:.4} (negative = dissipative)", spectrum.sum());

if spectrum.exponents[0] > 0.0 {
    println!("System is chaotic!");
}

// Van der Pol stability analysis
let vdp = VanDerPolOscillator::new(1.0);
let mut guess: Multivector<2, 0, 0> = Multivector::zero();
guess.set(1, 0.1);
guess.set(2, 0.1);

let fp = find_fixed_point(&vdp, &guess, 1e-10).unwrap();
let stability = analyze_stability(&vdp, &fp.point).unwrap();
println!("Fixed point stability: {:?}", stability.stability_type);

// Bifurcation diagram for logistic map
let config = ContinuationConfig {
    parameter_range: (2.5, 4.0),
    num_points: 1000,
    transient: 500,
    samples: 100,
    ..Default::default()
};
let diagram = BifurcationDiagram::compute(|r| LogisticMap::new(r), &config).unwrap();
println!("Bifurcation diagram: {} parameter values", diagram.branches().len());
```

### JavaScript/TypeScript: Mathematical Computing

```typescript
import init, {
  TropicalBatch,
  WasmBranchPolicy,
  WasmDualNumber,
  WasmMultivector,
  WasmOrdinalArena,
  WasmStaticMultiDual2,
  WasmTropicalNumber,
} from '@justinelliottcobb/amari-wasm';

async function main() {
  // Initialize the WASM module
  await init();

  // Geometric Algebra: Create and rotate vectors
  const e1 = WasmMultivector.basisVector(0);
  const e2 = WasmMultivector.basisVector(1);
  const bivector = e1.geometricProduct(e2);
  console.log('Geometric product:', bivector.toString());

  // Tropical Algebra: max-plus arithmetic and v0.21.0 semiring folds
  const trop1 = new WasmTropicalNumber(3.0);
  const trop2 = new WasmTropicalNumber(5.0);
  const sum = trop1.tropicalAdd(trop2);       // max(3, 5) = 5
  const product = trop1.tropicalMul(trop2);   // 3 + 5 = 8
  const best = TropicalBatch.foldOplus(new Float64Array([3, 5, 2]));
  const composed = TropicalBatch.foldOtimes(new Float64Array([3, 5, 2]));
  console.log('Tropical operations:', sum.getValue(), product.getValue(), best, composed);

  // v0.21.0 ordinal-weighted tropical carriers below ε₀
  const arena = new WasmOrdinalArena();
  const omega = arena.omega();
  const one = arena.one();
  const omegaPlusOne = arena.add(omega, one);
  const ordinalWeight = arena.weightFromOrdinal(omegaPlusOne);
  console.log('Ordinal weight:', arena.formatWeight(ordinalWeight));

  // Automatic Differentiation: Compute derivatives and branch ties
  const x = new WasmDualNumber(2.0, 1.0);
  const xSquared = x.mul(x); // f(x) = x², f'(x) = 2x
  const left = new WasmDualNumber(2.0, 1.0);
  const right = new WasmDualNumber(2.0, 5.0);
  const averageTie = left.maxByPolicy(right, WasmBranchPolicy.Average);
  console.log('f(2) =', xSquared.getReal(), "f'(2) =", xSquared.getDual());
  console.log('average tie derivative =', averageTie.getDual());

  // v0.21.0 fixed-size multi-dual wrapper for small hot loops
  const sx = WasmStaticMultiDual2.variable(2.0, 0);
  const sy = WasmStaticMultiDual2.variable(3.0, 1);
  const staticSquared = sx.mul(sx);
  const score = staticSquared.add(sy);
  console.log('static gradient =', score.getGradient());

  // Clean up WASM memory
  e1.free(); e2.free(); bivector.free();
  trop1.free(); trop2.free(); sum.free(); product.free();
  ordinalWeight.free(); omegaPlusOne.free(); one.free(); omega.free(); arena.free();
  x.free(); xSquared.free(); left.free(); right.free(); averageTie.free();
  sx.free(); sy.free(); staticSquared.free(); score.free();
}

main();
```

## Architecture

### Crate Hierarchy

**Domain Crates** (provide mathematical APIs):
- `amari-core`: Core Clifford algebra types and CPU implementations
- `amari-measure`: Measure theory, sigma-algebras, and integration
- `amari-calculus`: Differential calculus with geometric algebra
- `amari-probabilistic`: Probability distributions on multivector spaces, stochastic processes, MCMC
- `amari-functional`: Hilbert spaces, bounded operators, spectral decomposition, Sobolev spaces
- `amari-topology`: Algebraic topology, simplicial complexes, persistent homology, Morse theory
- `amari-dynamics`: Dynamical systems, ODE solvers, stability analysis, bifurcations, Lyapunov exponents, phase portraits
- `amari-holographic`: Vector Symbolic Architectures (VSA), binding algebras, holographic memory, optical field operations
- `amari-tropical`: Tropical (max-plus) algebra, semiring folds, and ordinal-weighted optimization below `ε₀`
- `amari-dual`: Forward-mode dual numbers, explicit branch policies, heap-backed multi-dual seeding, and fixed-size gradient carriers
- `amari-fusion`: Unified Tropical-Dual-Clifford system
- `amari-info-geom`: Information geometry and statistical manifolds
- `amari-automata`: Cellular automata with geometric algebra
- `amari-network`: Graph neural networks and network analysis
- `amari-relativistic`: Spacetime algebra and relativistic physics
- `amari-enumerative`: Enumerative geometry, Schubert calculus, WDVV curve counting, matroids, CSM classes, equivariant localization, and stability
- `amari-optimization`: Gradient-based optimization algorithms
- `amari-flynn`: Probabilistic verification contracts
- `amari-rewrite`: Abstract and term rewriting systems, inverse search, and rule inference

**Integration Crates** (consume domain APIs):
- `amari-gpu`: Multi-GPU acceleration with WebGPU
- `amari-wasm`: WebAssembly bindings for TypeScript/JavaScript, including the `0.21.0` tropical/dual extension surface
- `amari`: Umbrella crate re-exporting all features

### Key Types

```rust
// Multivector in Clifford algebra Cl(P,Q,R)
// P: positive signature, Q: negative signature, R: zero signature
Multivector<const P: usize, const Q: usize, const R: usize>

// Tropical numbers (max-plus semiring)
TropicalNumber<T: Float>  // Use TropicalNumber::new(value)

// Arena-backed ordinals below ε₀ and ordinal tropical weights
OrdinalArena              // Owns OrdinalId handles
OrdinalWeight             // Bottom-extended optimization carrier

// Dual numbers for automatic differentiation
DualNumber<T: Float>      // Use DualNumber::new(value, derivative)
BranchPolicy              // Explicit tie behavior for min/max branch points

// Multi-variable dual numbers
MultiDualNumber<T: Float>         // Heap-backed gradients; use MultiDualNumber::variables(...)
StaticMultiDual<T, const N: usize> // Inline fixed-size gradients for small hot loops

// Tropical-Dual-Clifford unified system
TropicalDualClifford<T: Float, const DIM: usize>

// Common algebras
type Cl3 = Multivector<3, 0, 0>;      // 3D Euclidean
type Spacetime = Multivector<1, 3, 0>; // Minkowski spacetime
type PGA3D = Multivector<3, 0, 1>;    // Projective Geometric Algebra
```

## Mathematical Foundation

### Clifford Algebra Cl(P,Q,R)

- **P**: Basis vectors with e²ᵢ = +1 (positive signature)
- **Q**: Basis vectors with e²ᵢ = -1 (negative signature)
- **R**: Basis vectors with e²ᵢ = 0 (degenerate)

```
ab = a·b + a∧b      // Geometric product = inner + outer product
```

### Tropical Algebra (Max-Plus)

```
a ⊕ b = max(a, b)    // Tropical addition
a ⊙ b = a + b        // Tropical multiplication
```

`0.21.0` also adds a bounded ordinal substrate for optimization-oriented ranking:

- `OrdinalArena` interns canonical ordinals below `ε₀`
- `OrdinalWeight` extends those ordinals with a bottom element for impossible paths/states
- `best_weight` and `compose_weights` provide semiring-style selection and composition

Applications: Path optimization, sequence decoding, dynamic programming, ordinal ranking, and valuation workflows

### Dual Numbers

```
a + εb where ε² = 0
(a + εb) + (c + εd) = (a + c) + ε(b + d)
(a + εb) × (c + εd) = ac + ε(ad + bc)
```

`0.21.0` makes branch-sensitive behavior explicit with `BranchPolicy::{Left, Right, Average}` and adds fixed-size `StaticMultiDual<T, N>` carriers for small optimization loops.

Applications: Automatic differentiation, gradient computation, local sensitivity analysis, and browser-friendly optimization loops

### Information Geometry

- **Fisher Information Metric**: Riemannian metric on statistical manifolds
- **α-Connections**: Generalized connections parameterized by α ∈ [-1,1]
- **Dually Flat Manifolds**: Manifolds with e-connection (α=+1) and m-connection (α=-1)
- **Bregman Divergences**: Information-geometric divergences

## v0.12.0 Breaking Changes

Version 0.12.0 introduced significant API improvements for better encapsulation:

### TropicalNumber

```rust
// Before (v0.11.x)
let a = TropicalNumber(3.0);
let value = a.0;
let sum = a.tropical_add(b);

// After (v0.12.0+)
let a = TropicalNumber::new(3.0);
let value = a.value();
let sum = a.tropical_add(&b);  // Now takes reference
```

### DualNumber

```rust
// Before (v0.11.x)
let x = DualNumber { real: 3.0, dual: 1.0 };
let value = x.real;

// After (v0.12.0+)
let x = DualNumber::new(3.0, 1.0);
let value = x.value();
let deriv = x.derivative();
```

See [docs/archive/MIGRATION_v0.12.0.md](docs/archive/MIGRATION_v0.12.0.md) for complete migration guide.

## Interactive Examples Suite

The **[Amari Examples Suite](https://amari-math.netlify.app)** provides comprehensive interactive documentation:

- **Live Visualizations**: 19 interactive visualizations demonstrating mathematical concepts
  - Multivector coefficient manipulation in Cl(3,0,0)
  - Tropical algebra operations with convergence animation
  - Dual numbers with real-time derivative curves
  - Rotor-based 3D rotations
  - Fisher information on probability simplex
  - MCMC sampling visualization
  - Interactive geometric networks
  - Lorenz, Van der Pol, Duffing, and Rossler attractors
  - Bifurcation diagram explorer
  - Nullcline and phase portrait analysis
  - Poincaré section visualization
  - Lyapunov exponent heatmaps
  - Ergodic measure evolution
  - Grade decomposition (Clifford algebra)
  - Möbius transformations (conformal geometry)
  - Tropical shortest path (Bellman-Ford)
  - Eigenvalue trajectory tracking
  - Geodesic distance heatmaps
  - Holographic memory exploration

- **Comprehensive API Reference**: 77+ classes with 300+ methods fully documented
  - Geometric Algebra (Multivector, Rotor, Bivector)
  - Tropical Algebra (TropicalNumber, TropicalMatrix, `TropicalBatch`, `WasmOrdinalArena`, `WasmOrdinalWeight`)
  - Automatic Differentiation (`WasmDualNumber`, `WasmBranchPolicy`, `WasmMultiDualNumber`, `WasmStaticMultiDual2/3/4`)
  - CGT & Short Surreals (`WasmCgtArena`, `WasmGameInspection`, `WasmDyadic`, `WasmShortSurreal`)
  - Probability (GaussianMultivector, MCMC samplers)
  - And 12 more categories

- **Interactive Playground**: Write and run JavaScript code with live WASM execution

## Examples & Documentation (v0.22.0)

The examples suite is versioned for `0.22.0` and now includes release-focused CGT/surreal WebAssembly workflows alongside the broader mathematical catalog:

- **CGT 0.22.0 WASM examples**: `WasmCgtArena`, short-game cuts, outcomes, comparison, nimbers, and `WasmGameInspection`
- **Surreal 0.22.0 WASM examples**: `WasmDyadic`, `WasmShortSurreal`, exact dyadic arithmetic, checked division, and numeric-game conversion
- **Tropical 0.21.0 WASM examples**: `TropicalBatch.foldOplus`, `TropicalBatch.foldOtimes`, `WasmOrdinalArena`, and `WasmOrdinalWeight`
- **Dual 0.21.0 WASM examples**: `WasmBranchPolicy`, `WasmMultiDualNumber.variables(...)`, and `WasmStaticMultiDual2` hot-loop gradients
- **API reference coverage**: the new CGT/surreal and tropical/dual extension classes and methods are listed in the browser reference

The suite also keeps comprehensive coverage of the broader crate family:

### Rust Examples (`examples/rust/`)

- **dynamical-systems/** (7 examples): Lorenz attractor, Van der Pol oscillator, bifurcation analysis, Lyapunov exponents, stability analysis, phase portraits, stochastic dynamics
- **topology/** (4 examples): Simplicial complexes, persistent homology, Morse theory, topological data analysis
- **functional-analysis/** (5 examples): Hilbert spaces, operators, spectral theory, Banach spaces, distributions
- **physics-simulation/**: Rigid body dynamics, electromagnetic fields, fluid dynamics, quantum mechanics
- **computer-graphics/**: 3D transformations, camera projection, mesh operations, ray tracing
- **machine-learning/**: Automatic differentiation, neural networks, optimization, verified learning

### Language Bindings

- **TypeScript** (`examples/typescript/`): Node.js examples with `@justinelliottcobb/amari-wasm`
- **PureScript** (`examples/purescript/`): Type-safe FFI bindings with Effect tracking
- **Web Demos** (`examples/web/`): Interactive browser visualizations

See [`examples/README.md`](examples/README.md) for complete documentation and [`examples/LEARNING_PATHS.md`](examples/LEARNING_PATHS.md) for structured learning curricula.

## GPU Module Status (v0.21.0)

| Module | Status | Feature Flag |
|--------|--------|--------------|
| Core GA | ✅ Enabled | default |
| Info Geometry | ✅ Enabled | default |
| Relativistic | ✅ Enabled | default |
| Network | ✅ Enabled | default |
| Measure | ✅ Enabled | `measure` |
| Calculus | ✅ Enabled | `calculus` |
| Dual | ✅ Enabled | `dual` |
| Enumerative | ✅ Enabled | `enumerative` |
| Automata | ✅ Enabled | `automata` |
| Fusion | ✅ Enabled | `fusion` |
| Holographic | ✅ Enabled | `holographic` |
| Optical Fields | ✅ Enabled | `holographic` |
| Probabilistic | ✅ Enabled | `probabilistic` |
| Functional | ✅ Enabled | `functional` |
| Topology | ✅ Enabled | `topology` |
| Dynamics | ✅ Enabled | `dynamics` |
| GF(2) | ✅ Enabled | `gf2` |
| Enumerative (GF(2)) | ✅ Enabled | `enumerative` |
| Tropical | ❌ Disabled | - |

Note: Tropical GPU module temporarily disabled due to Rust orphan impl rules. Use CPU implementations from domain crates.

### v0.21.0 Extension Additions

**Tropical / Dual Optimization Extensions** — Release-focused additions across `amari-tropical`, `amari-dual`, `amari-wasm`, and the examples suite:

- **amari-tropical**: `Semiring`, `fold_oplus`, `fold_otimes`, `OrdinalArena`, `OrdinalId`, `CnfTerm`, `OrdinalWeight`, ordinal inspection, and ordinal-weight best/compose helpers
- **amari-dual**: `BranchPolicy`, explicit branch-sensitive min/max semantics, `MultiDualNumber::variables(...)`, and `StaticMultiDual<T, const N: usize>`
- **amari-wasm**: `TropicalBatch.foldOplus/foldOtimes`, `WasmOrdinalArena`, `WasmOrdinalWeight`, `WasmBranchPolicy`, `WasmMultiDualNumber.variables(...)`, and `WasmStaticMultiDual2/3/4`
- **examples-suite**: browser examples and API reference entries for the new tropical/dual `0.21.0` WASM surface

### Earlier Extension Additions

**GF(2) Algebra & Coding Theory** — Mathematical domain across amari-core, amari-enumerative, amari-gpu, and amari-wasm:

- **amari-core**: GF(2) vectors, matrices, Gaussian elimination, binary Clifford algebra Cl(N,R; F₂)
- **amari-enumerative**: Binary linear codes (Hamming, Reed-Muller, Golay), weight enumerators, Grassmannian combinatorics, matroid representability, Kazhdan-Lusztig polynomials
- **amari-gpu**: GPU-accelerated GF(2) matrix operations, coding theory, and representability checking
- **amari-wasm**: Complete WASM bindings for all GF(2) operations (8 new types/utilities)

**Probabilistic Contracts (amari-flynn)** — Formal verification framework:

- SMT-LIB2 proof obligation generation for external solvers (Z3, CVC5)
- Monte Carlo statistical verification in WASM
- Probabilistic value tracking and rare event classification
- WASM bindings for browser-based verification workflows

**MCP Server** — [Amari-MCP](https://github.com/justinelliottcobb/Amari-mcp) provides Model Context Protocol integration, enabling AI assistants to use Amari's mathematical operations as tools.

### v0.19.1 GPU Additions (Dynamics)

The `dynamics` feature provides GPU-accelerated dynamical systems operations:

- **GpuDynamics**: GPU context for dynamical systems operations with adaptive dispatch
- **DYNAMICS_RK4_STEP**: Parallel RK4 integration (256-thread workgroups)
- **DYNAMICS_LYAPUNOV_QR**: QR-based Lyapunov exponent computation
- **DYNAMICS_BIFURCATION**: Parameter sweep with attractor sampling
- **DYNAMICS_BASIN**: Grid-based basin of attraction computation
- Automatic CPU fallback for < 100 trajectories or < 10,000 grid cells

### v0.16.0 GPU Additions

The `topology` feature provides GPU-accelerated computational topology operations:

- **GpuTopology**: GPU context for topology operations with adaptive dispatch
- **TOPOLOGY_DISTANCE_MATRIX**: Parallel pairwise Euclidean distance computation (256-thread workgroups)
- **TOPOLOGY_MORSE_CRITICAL**: Discrete Morse theory critical point detection (8-neighbor comparison)
- **TOPOLOGY_BOUNDARY_MATRIX**: Sparse boundary operator construction for persistent homology
- **TOPOLOGY_MATRIX_REDUCTION**: Column reduction algorithm for persistence computation
- Automatic CPU fallback for < 100 points (distance matrix) or < 10,000 cells (Morse theory)

### v0.15.1 GPU Additions

The `holographic` feature includes GPU-accelerated optical field operations:

- **GpuOpticalField**: GPU context for optical rotor field operations
- **OPTICAL_BIND_SHADER**: Parallel rotor multiplication (256-thread workgroups)
- **OPTICAL_SIMILARITY_SHADER**: Inner product with workgroup reduction
- **LEE_ENCODE_SHADER**: Binary hologram encoding with bit-packing
- Automatic CPU fallback for fields < 4096 pixels (64×64)

## Building

### Prerequisites

- Rust 1.75+ with `cargo`
- Node.js 16+ (for TypeScript bindings)
- `wasm-pack` (for WASM builds)

### Build and Test

```bash
# Run all tests
cargo test --workspace

# Run with all features
cargo test --workspace --all-features

# Build documentation
cargo doc --workspace --open
```

### WebAssembly Build

```bash
cd amari-wasm
wasm-pack build --target web
```

## Performance

The library is optimized for high-performance applications:

- **SIMD**: Vectorized operations where supported
- **Cache Alignment**: 64-byte aligned data structures
- **Const Generics**: Zero-cost abstractions for dimensions
- **GPU Fallback**: Automatic CPU/GPU dispatch based on workload size
- **Batch Operations**: Efficient batch processing for large datasets

## Documentation

- **[Examples Suite](examples/README.md)**: Comprehensive examples for all crates
- **[Learning Paths](examples/LEARNING_PATHS.md)**: Structured learning curricula
- **[API Reference](examples/DOCUMENTATION.md)**: Detailed API documentation
- **[Migration Guide](docs/archive/MIGRATION_v0.12.0.md)**: Migrating from v0.11.x to v0.12.0+
- **[Changelog](CHANGELOG.md)**: Version history and changes
- **[docs.rs](https://docs.rs/amari)**: Complete API reference

## Contributing

Contributions are welcome. For development setup:

```bash
git clone https://github.com/justinelliottcobb/Amari.git
cd Amari
cargo test --workspace
```

## License

This project is licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT License ([LICENSE-MIT](LICENSE-MIT))

at your option.

## Acknowledgments

- Inspired by the geometric algebra community and research in Information Geometry
- Built with modern Rust performance idioms and WebAssembly best practices
- Named after Shun-ichi Amari's contributions to Information Geometry

## Support

- Issues: [GitHub Issues](https://github.com/justinelliottcobb/Amari/issues)
- Discussions: [GitHub Discussions](https://github.com/justinelliottcobb/Amari/discussions)
- Documentation: [API Docs](https://docs.rs/amari)

---

*"Geometry is the art of correct reasoning from incorrectly drawn figures."* - Henri Poincaré
