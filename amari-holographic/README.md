# amari-holographic

Holographic Reduced Representations and Vector Symbolic Architectures for the Amari mathematical computing library.

## Overview

`amari-holographic` provides implementations of various binding algebras for holographic memory and vector symbolic architectures (VSA). These high-dimensional distributed representations enable storing and retrieving associations through algebraic operations.

## Key Concepts

### Vector Symbolic Architectures (VSA)

VSA systems use high-dimensional vectors (typically 1000+ dimensions) with three core operations:

| Operation | Symbol | Purpose | Property |
|-----------|--------|---------|----------|
| **Binding** | `⊛` | Create associations | Result dissimilar to inputs |
| **Bundling** | `⊕` | Superposition | Result similar to all inputs |
| **Similarity** | `sim(a,b)` | Compare representations | Cosine similarity |

### Holographic Reduced Representations (HRR)

HRR stores key-value associations in superposition:

```
memory = (key₁ ⊛ value₁) ⊕ (key₂ ⊛ value₂) ⊕ ... ⊕ (keyₙ ⊛ valueₙ)
```

Retrieval uses unbinding:
```
retrieved_value ≈ key⁻¹ ⊛ memory
```

## Supported Algebras

### ProductCliffordAlgebra (Recommended)

Product of K copies of Cl(3,0,0). Provides O(64K) compute complexity with dimension 8K.

```rust
use amari_holographic::{ProductCliffordAlgebra, BindingAlgebra};

type ProductCl3x32 = ProductCliffordAlgebra<32>; // 256-dimensional

let key = ProductCl3x32::random_versor(2);
let value = ProductCl3x32::random_versor(2);
let bound = key.bind(&value);

// Retrieve
let retrieved = key.unbind(&bound)?;
assert!(retrieved.similarity(&value) > 0.9);
```

**Advantages:**
- Linear scaling: O(K) for dimension 8K
- Exact inverse exists for versors
- High capacity: O(K / ln K) items

### CliffordAlgebra

General Clifford algebras Cl(p,q,r) wrapping `amari-core::Multivector`.

```rust
use amari_holographic::{CliffordAlgebra, BindingAlgebra};

type Cl8 = CliffordAlgebra<8, 0, 0>; // 256-dimensional

let a = Cl8::random_versor(2);
let b = Cl8::random_versor(2);
let product = a.bind(&b);
```

**Properties:**
- Dimension: 2^(p+q+r)
- Compute: O(4^n) for geometric product
- Full geometric algebra semantics

### Cl3 (Optimized)

Optimized 3D Clifford algebra with fully unrolled operations.

```rust
use amari_holographic::{Cl3, BindingAlgebra};

let a = Cl3::random_versor(2);
let b = Cl3::random_versor(2);
let product = a.bind(&b); // Unrolled, no loops
```

**Performance:**
- Fixed 8 coefficients: `[scalar, e1, e2, e3, e12, e13, e23, e123]`
- Fully unrolled geometric product
- Building block for ProductCliffordAlgebra

### FHRRAlgebra

Fourier Holographic Reduced Representation using frequency-domain operations.

```rust
use amari_holographic::{FHRRAlgebra, BindingAlgebra};

type FHRR256 = FHRRAlgebra<256>;

let key = FHRR256::random_unitary();
let value = FHRR256::random_unitary();
let bound = key.bind(&value); // Element-wise complex multiplication
```

**Properties:**
- Binding: Element-wise complex multiplication
- Simple inverse: Complex conjugate (for unitary elements)
- Efficient: O(D) operations

### MAPAlgebra

Multiply-Add-Permute bipolar algebra with self-inverse property.

```rust
use amari_holographic::{MAPAlgebra, BindingAlgebra};

type MAP256 = MAPAlgebra<256>;

let key = MAP256::random_bipolar();
let value = MAP256::random_bipolar();

// Self-inverse: key.bind(key) ≈ identity
let bound = key.bind(&value);
let retrieved = key.bind(&bound); // Same as unbind!
```

**Properties:**
- Bipolar: All coefficients are ±1
- Self-inverse: Every element is its own inverse
- Hardware-friendly: XOR operations map to binding

## Holographic Memory

The `HolographicMemory<A>` type provides key-value storage in superposition:

```rust
use amari_holographic::{HolographicMemory, AlgebraConfig, ProductCliffordAlgebra, BindingAlgebra};

type ProductCl3x32 = ProductCliffordAlgebra<32>;

// Create memory
let mut memory = HolographicMemory::<ProductCl3x32>::new(AlgebraConfig::default());

// Store associations
let key1 = ProductCl3x32::random_versor(2);
let value1 = ProductCl3x32::random_versor(2);
memory.store(&key1, &value1);

let key2 = ProductCl3x32::random_versor(2);
let value2 = ProductCl3x32::random_versor(2);
memory.store(&key2, &value2);

// Retrieve
let result = memory.retrieve(&key1);
println!("Confidence: {}", result.confidence);
println!("Similarity to original: {}", result.value.similarity(&value1));

// Check capacity
let info = memory.capacity_info();
println!("Items: {} / {}", info.item_count, info.theoretical_capacity);
```

### Memory Features

- **Key tracking**: Enable with `with_key_tracking()` for attribution
- **Batch storage**: `store_batch(&pairs)` for efficient bulk insertion
- **Capacity monitoring**: SNR-based confidence and capacity warnings
- **Memory merging**: Combine multiple memories with `merge()`

## Resonator Networks

Resonator networks clean up noisy retrievals by iteratively projecting toward valid codebook items:

```rust
use amari_holographic::{Resonator, ResonatorConfig, ProductCliffordAlgebra, BindingAlgebra};

type ProductCl3x32 = ProductCliffordAlgebra<32>;

// Create codebook of valid states
let codebook: Vec<ProductCl3x32> = (0..10)
    .map(|_| ProductCl3x32::random_versor(2))
    .collect();

// Create resonator
let config = ResonatorConfig {
    max_iterations: 50,
    convergence_threshold: 0.999,
    initial_beta: 1.0,
    final_beta: 100.0,
};
let resonator = Resonator::new(codebook, config)?;

// Clean up noisy input
let noisy = ProductCl3x32::random_versor(2); // Some noisy retrieval
let result = resonator.cleanup(&noisy);

println!("Converged: {}", result.converged);
println!("Best match index: {}", result.best_match_index);
println!("Final similarity: {}", result.final_similarity);
```

### Factorization

Resonators can factorize bound pairs to recover original factors:

```rust
let result = resonator_a.factorize(&bound_pair, &resonator_b);
println!("Factor A similarity: {}", result.factor_a.similarity(&original_a));
println!("Factor B similarity: {}", result.factor_b.similarity(&original_b));
```

## Optical Field Operations

The `optical` module provides GA-native optical field operations for Lee hologram encoding, enabling seamless integration with DMD-based optical systems.

### Mathematical Background

Complex numbers are isomorphic to the even subalgebra of Cl(2,0):

```
Complex: z = a + bi           where i² = -1
GA:      R = a + b·e₁₂        where (e₁₂)² = -1

Isomorphism:
    i  ↔  e₁₂  (unit bivector)
    z* ↔  R†   (reverse/conjugate)
    |z|² ↔ R·R† (norm squared)
```

An optical field at position (x,y) is represented as a rotor field:
```
E(x,y) = A(x,y) · exp(φ(x,y) · e₁₂)
       = A(x,y) · (cos(φ) + sin(φ)·e₁₂)
```

### OpticalRotorField

Grade-separated rotor field for SIMD-optimized operations:

```rust
use amari_holographic::optical::{OpticalRotorField, OpticalFieldAlgebra};

// Create from phase array
let phases = vec![0.0, std::f32::consts::FRAC_PI_4, std::f32::consts::FRAC_PI_2];
let field = OpticalRotorField::from_phase(phases, (3, 1));

// Random field with deterministic seed
let random_field = OpticalRotorField::random((64, 64), 42);

// Access components
let phase = field.phase_at(1, 0);      // π/4
let amplitude = field.amplitude_at(1, 0); // 1.0
```

### OpticalFieldAlgebra

VSA operations on optical rotor fields:

```rust
use amari_holographic::optical::{OpticalFieldAlgebra, OpticalRotorField};

let algebra = OpticalFieldAlgebra::new((64, 64));

let a = OpticalRotorField::random((64, 64), 1);
let b = OpticalRotorField::random((64, 64), 2);

// Binding = phase addition (rotor product)
let bound = algebra.bind(&a, &b);

// Inverse = phase negation (rotor reverse)
let inv = algebra.inverse(&a);

// bind(a, inverse(a)) ≈ identity
let identity = algebra.bind(&a, &inv);

// Similarity (normalized inner product)
let sim = algebra.similarity(&a, &a); // 1.0

// Bundling (weighted superposition)
let bundled = algebra.bundle(&[a.clone(), b.clone()], &[0.5, 0.5]);
```

| VSA Operation | Complex Form | GA Form |
|---------------|--------------|---------|
| Binding | z₁ · z₂ | R₁ · R₂ (rotor product) |
| Unbinding | z₁ · z₂* | R₁ · R₂† (product with reverse) |
| Bundling | Σ wᵢzᵢ | Σ wᵢRᵢ (weighted sum) |
| Similarity | Re(z₁*z₂)/\|z₁\|\|z₂\| | ⟨R₁†R₂⟩₀ / (\|R₁\|\|R₂\|) |

### GeometricLeeEncoder

Lee hologram encoding for DMD display:

```rust
use amari_holographic::optical::{GeometricLeeEncoder, OpticalRotorField};

// Create encoder with carrier frequency
let encoder = GeometricLeeEncoder::with_frequency((256, 256), 0.25);

// Create optical field (use amplitude 0.5 for optimal encoding)
let mut field = OpticalRotorField::random((256, 256), 42);
for i in 0..field.len() {
    field.amplitudes_mut()[i] = 0.5;
}

// Encode to binary hologram
let hologram = encoder.encode(&field);

// Check encoding quality
println!("Fill factor: {}", hologram.fill_factor());
println!("Theoretical efficiency: {}", encoder.theoretical_efficiency(&field));
```

The Lee encoding process:
1. Multiply signal by carrier (rotor product = phase addition)
2. Threshold based on scalar part: `B(x,y) = 1 if ⟨Modulated⟩₀ > cos(π·A)`

### BinaryHologram

Bit-packed binary pattern for hardware interface:

```rust
use amari_holographic::optical::BinaryHologram;

// Create from boolean pattern
let pattern = vec![true, false, true, false, true, true, false, false];
let hologram = BinaryHologram::from_bools(&pattern, (8, 1));

// Access pixels
assert!(hologram.get(0, 0));
assert!(!hologram.get(1, 0));

// Statistics
println!("Fill factor: {}", hologram.fill_factor());
println!("Popcount: {}", hologram.popcount());

// Hamming distance between holograms
let other = BinaryHologram::from_bools(&vec![true; 8], (8, 1));
println!("Hamming distance: {}", hologram.hamming_distance(&other));
```

### OpticalCodebook

Seed-based symbol mapping for compact serialization:

```rust
use amari_holographic::optical::{OpticalCodebook, CodebookConfig, SymbolId};

let config = CodebookConfig::new((64, 64), 12345);
let mut codebook = OpticalCodebook::new(config);

// Register symbols (deterministic from seed)
codebook.register("AGENT".into());
codebook.register("ACTION".into());
codebook.register("OBJECT".into());

// Get field for symbol (generated on first access, cached)
let agent_field = codebook.get(&"AGENT".into()).unwrap();

// Export seeds for persistence (compact!)
let seeds = codebook.export_seeds();

// Restore from seeds
let mut restored = OpticalCodebook::new(CodebookConfig::new((64, 64), 12345));
restored.import_seeds(seeds);
```

### TropicalOpticalAlgebra

Tropical semiring operations for attractor dynamics:

```rust
use amari_holographic::optical::{TropicalOpticalAlgebra, OpticalRotorField};

let tropical = TropicalOpticalAlgebra::new((64, 64));

let a = OpticalRotorField::random((64, 64), 1);
let b = OpticalRotorField::random((64, 64), 2);

// Tropical add: point-wise minimum phase magnitude
let min_phase = tropical.tropical_add(&a, &b);

// Soft tropical add with temperature
let soft = tropical.soft_tropical_add(&a, &b, 10.0);

// Attractor convergence
let attractors = vec![
    OpticalRotorField::uniform(0.0, 1.0, (64, 64)),
    OpticalRotorField::uniform(std::f32::consts::FRAC_PI_4, 1.0, (64, 64)),
];
let initial = OpticalRotorField::random((64, 64), 42);
let (final_state, iterations) = tropical.attractor_converge(&initial, &attractors, 100, 1e-6);
```

## API Reference

### Core Trait: `BindingAlgebra`

```rust
pub trait BindingAlgebra: Clone + Send + Sync {
    // Required methods
    fn zero() -> Self;
    fn identity() -> Self;
    fn bind(&self, other: &Self) -> Self;
    fn inverse(&self) -> AlgebraResult<Self>;
    fn unbind(&self, other: &Self) -> AlgebraResult<Self>;
    fn bundle(&self, other: &Self, beta: f64) -> AlgebraResult<Self>;
    fn similarity(&self, other: &Self) -> f64;
    fn norm(&self) -> f64;
    fn normalize(&self) -> AlgebraResult<Self>;
    fn permute(&self, shift: usize) -> Self;
    fn dimension(&self) -> usize;
    fn from_coefficients(coeffs: &[f64]) -> AlgebraResult<Self>;
    fn to_coefficients(&self) -> Vec<f64>;
    fn random_vector() -> Self;
    fn random_versor(num_factors: usize) -> Self;

    // Provided methods
    fn superpose(&self, other: &Self) -> AlgebraResult<Self>;
    fn scale(&self, factor: f64) -> AlgebraResult<Self>;
    fn bundle_all(items: &[Self], beta: f64) -> AlgebraResult<Self>;
    fn estimate_snr(&self, num_items: usize) -> f64;
    fn theoretical_capacity(&self) -> usize;
}
```

Use `superpose` for additive accumulation such as a holographic memory trace
`T = Σ keyᵢ ⊛ valueᵢ`; its magnitude grows as evidence is added. Use `bundle`
for attention or cleanup, where softmax weighting keeps the result bounded.
Weighted accumulation composes `scale` and `superpose`:

```rust
let weighted = BindingAlgebra::scale(&binding, weight)?;
trace = BindingAlgebra::superpose(&trace, &weighted)?;
```

### AlgebraConfig

```rust
pub struct AlgebraConfig {
    pub bundle_beta: f64,        // Temperature for bundling (default: 1.0)
    pub retrieval_beta: f64,     // Temperature for retrieval (default: f64::INFINITY)
    pub similarity_threshold: f64, // Threshold for "probably contains" (default: 0.5)
}
```

### HolographicMemory<A>

```rust
impl<A: BindingAlgebra> HolographicMemory<A> {
    pub fn new(config: AlgebraConfig) -> Self;
    pub fn with_key_tracking(config: AlgebraConfig) -> Self;

    pub fn store(&mut self, key: &A, value: &A);
    pub fn store_batch(&mut self, pairs: &[(A, A)]);

    pub fn retrieve(&self, key: &A) -> RetrievalResult<A>;
    pub fn retrieve_at_temperature(&self, key: &A, beta: f64) -> RetrievalResult<A>;

    pub fn probably_contains(&self, key: &A) -> bool;
    pub fn capacity_info(&self) -> CapacityInfo;

    pub fn clear(&mut self);
    pub fn merge(&mut self, other: &Self);

    pub fn item_count(&self) -> usize;
    pub fn trace(&self) -> &A;
}
```

### RetrievalResult<A>

```rust
pub struct RetrievalResult<A: BindingAlgebra> {
    pub value: A,              // Retrieved value (after cleanup)
    pub raw_value: A,          // Raw retrieved value
    pub confidence: f64,       // Estimated confidence [0, 1]
    pub attribution: Vec<(usize, f64)>, // Which keys contributed
    pub query_similarity: f64, // Similarity to query key
}
```

### Resonator<A>

```rust
impl<A: BindingAlgebra> Resonator<A> {
    pub fn new(codebook: Vec<A>, config: ResonatorConfig) -> HolographicResult<Self>;

    pub fn cleanup(&self, noisy: &A) -> CleanupResult<A>;
    pub fn factorize(&self, bound: &A, other: &Resonator<A>) -> FactorizationResult<A>;

    pub fn codebook_size(&self) -> usize;
    pub fn get_codebook_item(&self, index: usize) -> Option<&A>;
}
```

## Capacity and Performance

### Theoretical Capacity

All algebras provide theoretical capacity of O(D / ln D) where D is dimension:

| Algebra | Dimension | Capacity (~items) |
|---------|-----------|-------------------|
| ProductCl3x32 | 256 | ~46 |
| ProductCl3x128 | 1024 | ~147 |
| FHRR1024 | 1024 | ~147 |
| MAP2048 | 2048 | ~280 |

### Performance Guidelines

- **Stay below 50% capacity** for reliable retrieval
- **Use versors** (products of vectors) for better invertibility
- **Monitor SNR**: Confidence drops as items are added
- **Use resonators** for noisy retrievals

## Features

```toml
[dependencies]
amari-holographic = { version = "0.15", features = ["parallel"] }
```

| Feature | Description |
|---------|-------------|
| `std` | Standard library (default) |
| `parallel` | Parallel operations via rayon |
| `serialize` | Serde serialization support |

The `optical` module is always available and uses `rand` and `rand_chacha` for deterministic random generation.

## Integration

### With amari-fusion

```rust
// TropicalDualClifford has built-in binding operations
use amari_fusion::TropicalDualClifford;

let tdc1 = TropicalDualClifford::<f64, 8>::random_with_scale(1.0);
let tdc2 = TropicalDualClifford::<f64, 8>::random_with_scale(1.0);

let bound = tdc1.bind(&tdc2);
let similarity = tdc1.similarity(&tdc2);
```

### With amari-gpu

```rust
use amari_gpu::{GpuHolographic, GpuHolographicMemory};

// GPU-accelerated batch operations
let gpu = GpuHolographic::new_product_cl3x32().await?;

let results = gpu.batch_bind(&keys_flat, &values_flat).await?;
let similarities = gpu.batch_similarity(&a_flat, &b_flat).await?;
```

## Examples

### Semantic Memory

```rust
// Store word associations
let mut memory = HolographicMemory::<ProductCl3x32>::new(AlgebraConfig::default());

let dog = ProductCl3x32::random_versor(2);
let animal = ProductCl3x32::random_versor(2);
let bark = ProductCl3x32::random_versor(2);

// dog IS-A animal
memory.store(&dog, &animal);

// dog CAN bark
let can = ProductCl3x32::random_versor(2);
memory.store(&dog.bind(&can), &bark);

// Query: what can dog do?
let query = dog.bind(&can);
let result = memory.retrieve(&query);
println!("Dog can: similarity to bark = {}", result.value.similarity(&bark));
```

### Role-Filler Binding

```rust
// Represent structured knowledge: "John loves Mary"
let john = ProductCl3x32::random_versor(2);
let mary = ProductCl3x32::random_versor(2);
let agent_role = ProductCl3x32::random_versor(2);
let patient_role = ProductCl3x32::random_versor(2);
let loves_frame = ProductCl3x32::random_versor(2);

// Create frame
let sentence = loves_frame
    .bind(&agent_role.bind(&john))
    .bind(&patient_role.bind(&mary));

// Query: who is the agent?
let agent_query = loves_frame.bind(&agent_role);
let agent = agent_query.unbind(&sentence)?;
println!("Agent similarity to John: {}", agent.similarity(&john));
```

## License

MIT OR Apache-2.0
