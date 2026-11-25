//! Holographic Reduced Representations and Vector Symbolic Architectures
//!
//! This crate provides implementations of various binding algebras for
//! holographic memory and vector symbolic architectures (VSA).
//!
//! # Overview
//!
//! Holographic reduced representations (HRR) and vector symbolic architectures
//! enable storing and retrieving associations in high-dimensional distributed
//! representations through algebraic operations. These systems use vectors with
//! thousands of dimensions to encode symbolic information in a distributed manner.
//!
//! ## Core Operations
//!
//! | Operation | Symbol | Purpose | Property |
//! |-----------|--------|---------|----------|
//! | **Binding** | `⊛` | Create associations | Result dissimilar to inputs |
//! | **Bundling** | `⊕` | Superposition | Result similar to all inputs |
//! | **Unbinding** | `⊛⁻¹` | Retrieve associations | Inverse of binding |
//! | **Similarity** | `sim(a,b)` | Compare representations | Cosine similarity |
//!
//! ## How HRR Works
//!
//! HRR stores key-value associations in superposition:
//!
//! ```text
//! memory = (key₁ ⊛ value₁) ⊕ (key₂ ⊛ value₂) ⊕ ... ⊕ (keyₙ ⊛ valueₙ)
//! ```
//!
//! Retrieval uses unbinding with a query key:
//!
//! ```text
//! retrieved_value ≈ key⁻¹ ⊛ memory
//! ```
//!
//! The retrieved value is a noisy approximation of the original value, with
//! signal-to-noise ratio decreasing as more items are stored.
//!
//! # Supported Algebras
//!
//! The [`algebra`] module provides multiple algebra implementations optimized
//! for different use cases:
//!
//! | Algebra | Dimension | Compute | Use Case |
//! |---------|-----------|---------|----------|
//! | [`ProductCliffordAlgebra`] | 8K | O(64K) | **Recommended**: High-capacity with linear scaling |
//! | [`Cl3`] | 8 | O(64) | Optimized building block for ProductClifford |
//! | [`CliffordAlgebra`] | 2^n | O(4^n) | General Clifford algebras Cl(p,q,r) |
//! | [`FHRRAlgebra`] | D | O(D) | Frequency domain, simple inverse |
//! | [`MAPAlgebra`] | D | O(D) | Bipolar, self-inverse, hardware-friendly |
//!
//! ## ProductCliffordAlgebra (Recommended)
//!
//! The product Clifford algebra uses K copies of Cl(3,0,0) operating independently,
//! providing O(64K) compute complexity for dimension 8K. This gives linear scaling
//! instead of the exponential scaling of general Clifford algebras.
//!
//! ```ignore
//! use amari_holographic::{ProductCliffordAlgebra, BindingAlgebra};
//!
//! type ProductCl3x32 = ProductCliffordAlgebra<32>; // 256-dimensional
//!
//! let key = ProductCl3x32::random_versor(2);   // Product of 2 random vectors
//! let value = ProductCl3x32::random_versor(2);
//! let bound = key.bind(&value);                 // Geometric product
//!
//! // Retrieve using unbinding
//! let retrieved = key.unbind(&bound).unwrap();
//! assert!(retrieved.similarity(&value) > 0.9);
//! ```
//!
//! ## FHRRAlgebra
//!
//! Fourier Holographic Reduced Representation uses frequency-domain operations.
//! Binding is element-wise complex multiplication, making it very efficient.
//!
//! ```ignore
//! use amari_holographic::{FHRRAlgebra, BindingAlgebra};
//!
//! type FHRR256 = FHRRAlgebra<256>;
//!
//! let key = FHRR256::random_unitary();
//! let value = FHRR256::random_unitary();
//! let bound = key.bind(&value);  // Element-wise complex multiply
//! ```
//!
//! ## MAPAlgebra
//!
//! Multiply-Add-Permute algebra uses bipolar vectors (±1 values) with XOR-like
//! binding. Every element is its own inverse, simplifying retrieval.
//!
//! ```ignore
//! use amari_holographic::{MAPAlgebra, BindingAlgebra};
//!
//! type MAP256 = MAPAlgebra<256>;
//!
//! let key = MAP256::random_bipolar();
//! let value = MAP256::random_bipolar();
//!
//! // Self-inverse property: key.bind(key) ≈ identity
//! let bound = key.bind(&value);
//! let retrieved = key.bind(&bound);  // Same as unbind!
//! ```
//!
//! # Holographic Memory
//!
//! The [`memory`] module provides [`HolographicMemory`], a key-value store
//! that uses holographic superposition. It's generic over any [`BindingAlgebra`].
//!
//! ```
//! use amari_holographic::{BindingAlgebra, HolographicMemory, MAPAlgebra};
//! type M = MAPAlgebra<4>;
//! let mut memory = HolographicMemory::with_key_tracking(Default::default());
//! let key = M::new([1., -1., 1., -1.]);
//! let value = M::new([2., 0., 3., 0.]); // Values need not be keys.
//! memory.store(&key, &value)?;
//! memory.store(&key, &value)?;
//! assert_eq!(memory.retrieve(&key)?.raw_value.components(), &[4., 0., 6., 0.]);
//! # Ok::<(), amari_holographic::HolographicError>(())
//! ```
//!
//! # Resonator Networks
//!
//! [`Resonator`] offers separate exhaustive nearest-key selection and iterative
//! soft mixing. A candidate is accepted only against the original observation,
//! coefficient by coefficient. A stationary iterate alone is not evidence of
//! recovery; factorization compares the returned **ordered product** to the bound.
//!
//! # Capacity and Performance
//!
//! ## Theoretical Capacity
//!
//! The nominal load heuristic is of order D / ln D where D is dimension.
//! It neither estimates query membership nor certifies reconstruction:
//!
//! | Configuration | Dimension | Capacity (~items) |
//! |---------------|-----------|-------------------|
//! | ProductCl3x32 | 256 | ~46 |
//! | ProductCl3x64 | 512 | ~85 |
//! | ProductCl3x128 | 1024 | ~147 |
//! | FHRR1024 | 1024 | ~147 |
//! | MAP2048 | 2048 | ~280 |
//!
//! ## Performance Guidelines
//!
//! - **Stay below 50% capacity** for reliable retrieval (SNR > 3dB)
//! - **Use versors** (products of vectors) for better invertibility
//! - **Monitor SNR**: Confidence drops as items are added
//! - **Use resonators** to clean up noisy retrievals
//! - **Batch operations** when storing many items
//!
//! # Features
//!
//! ```toml
//! [dependencies]
//! amari-holographic = { version = "0.12", features = ["parallel"] }
//! ```
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `std` | Standard library support (default) |
//! | `parallel` | Parallel operations via rayon |
//! | `serialize` | Serde serialization support |
//!
//! # Integration
//!
//! ## With amari-fusion
//!
//! The `TropicalDualClifford` type has built-in binding operations:
//!
//! ```ignore
//! use amari_fusion::TropicalDualClifford;
//!
//! let tdc1 = TropicalDualClifford::<f64, 8>::random_with_scale(1.0);
//! let tdc2 = TropicalDualClifford::<f64, 8>::random_with_scale(1.0);
//!
//! let bound = tdc1.bind(&tdc2);
//! let similarity = tdc1.similarity(&tdc2);
//! ```
//!
//! ## With amari-gpu
//!
//! GPU-accelerated batch operations are available:
//!
//! ```ignore
//! use amari_gpu::GpuHolographic;
//!
//! let gpu = GpuHolographic::new(256).await?;
//!
//! // Batch bind thousands of pairs in parallel
//! let results = gpu.batch_bind(&keys_flat, &values_flat).await?;
//! let similarities = gpu.batch_similarity(&a_flat, &b_flat).await?;
//! ```
//!
//! # Example: Semantic Memory
//!
//! ```ignore
//! use amari_holographic::{HolographicMemory, ProductCliffordAlgebra, BindingAlgebra, AlgebraConfig};
//!
//! type ProductCl3x32 = ProductCliffordAlgebra<32>;
//!
//! let mut memory = HolographicMemory::<ProductCl3x32>::new(AlgebraConfig::default());
//!
//! // Create semantic symbols
//! let dog = ProductCl3x32::random_versor(2);
//! let cat = ProductCl3x32::random_versor(2);
//! let animal = ProductCl3x32::random_versor(2);
//! let bark = ProductCl3x32::random_versor(2);
//! let meow = ProductCl3x32::random_versor(2);
//!
//! // Roles
//! let is_a = ProductCl3x32::random_versor(2);
//! let can = ProductCl3x32::random_versor(2);
//!
//! // Store relationships: dog IS-A animal, dog CAN bark
//! memory.store(&dog.bind(&is_a), &animal)?;
//! memory.store(&dog.bind(&can), &bark)?;
//!
//! // Store: cat IS-A animal, cat CAN meow
//! memory.store(&cat.bind(&is_a), &animal)?;
//! memory.store(&cat.bind(&can), &meow)?;
//!
//! // Query: what can dog do?
//! let result = memory.retrieve(&dog.bind(&can))?;
//! println!("Dog can: similarity to bark = {}", result.raw_value.similarity(&bark));
//!
//! // Query: what is cat?
//! let result = memory.retrieve(&cat.bind(&is_a))?;
//! println!("Cat is-a: similarity to animal = {}", result.raw_value.similarity(&animal));
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
// Allow index loops for clarity in algebra implementations
#![allow(clippy::needless_range_loop)]
#![allow(clippy::manual_range_contains)]

extern crate alloc;

// Core algebra module
pub mod algebra;

// Holographic memory module
pub mod memory;

// GA-native optical field operations for Lee hologram encoding
pub mod optical;

// Re-export core types from algebra
pub use algebra::{AlgebraConfig, AlgebraError, AlgebraResult, BindingAlgebra, GeometricAlgebra};

// Re-export specific algebras
pub use algebra::{Cl3, CliffordAlgebra, FHRRAlgebra, MAPAlgebra, ProductCliffordAlgebra};

// Re-export memory types
pub use memory::{
    Bindable, CapacityInfo, CleanupResult, FactorizationResult, HolographicError,
    HolographicMemory, HolographicResult, MembershipEvidence, NearestKey, Resonator,
    ResonatorConfig, RetrievalResult, WeightedResult, WeightedStatus,
};
