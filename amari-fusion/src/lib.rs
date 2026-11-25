//! Tropical-Dual-Clifford fusion system for optimal LLM evaluation
//!
//! This crate combines three exotic number systems:
//! - **Tropical algebra**: Converts expensive softmax operations to max operations
//! - **Dual numbers**: Provides automatic differentiation without computational graphs
//! - **Clifford algebra**: Handles geometric relationships and rotations
//!
//! Together, these systems create a powerful framework for efficient neural network
//! evaluation and optimization.
//!
//! # Key Types
//!
//! The main type is [`TropicalDualClifford`], which fuses all three algebras:
//!
//! ```ignore
//! use amari_fusion::TropicalDualClifford;
//!
//! // Create TDC values
//! let tdc1 = TropicalDualClifford::<f64, 8>::random_with_scale(1.0);
//! let tdc2 = TropicalDualClifford::<f64, 8>::random_with_scale(1.0);
//!
//! // Geometric operations
//! let product = &tdc1 * &tdc2;  // Tropical-modified geometric product
//! let rotated = tdc1.rotate_by(&tdc2);  // Rotor-based rotation
//!
//! // Access components
//! let tropical_val = tdc1.tropical_value();
//! let gradient = tdc1.gradient();
//! ```
//!
//! # Vector Symbolic Architectures
//!
//! [`TropicalDualClifford`] has built-in binding operations for holographic memory:
//!
//! ```ignore
//! use amari_fusion::TropicalDualClifford;
//!
//! let key = TropicalDualClifford::<f64, 8>::random_with_scale(1.0);
//! let value = TropicalDualClifford::<f64, 8>::random_with_scale(1.0);
//!
//! // Binding operations (inherent methods)
//! let bound = key.bind(&value);              // Create association
//! let retrieved = key.unbind(&bound);        // Retrieve value
//! let superposed = key.bundle(&value, 1.0);  // Superposition
//! let sim = key.similarity(&value);          // Cosine similarity
//! ```
//!
//! # Holographic Memory (v0.12.3+)
//!
//! For dedicated Vector Symbolic Architectures (VSA) and holographic reduced
//! representations with multiple algebra options, use the standalone
//! [`amari-holographic`](https://docs.rs/amari-holographic) crate:
//!
//! ```ignore
//! use amari_holographic::{HolographicMemory, ProductCliffordAlgebra, BindingAlgebra, AlgebraConfig};
//!
//! type ProductCl3x32 = ProductCliffordAlgebra<32>;
//!
//! // Create memory with 256-dimensional algebra
//! let mut memory = HolographicMemory::<ProductCl3x32>::new(AlgebraConfig::default());
//!
//! // Store and retrieve
//! let key = ProductCl3x32::random_versor(2);
//! let value = ProductCl3x32::random_versor(2);
//! memory.store(&key, &value)?;
//! let raw = memory.retrieve(&key)?.raw_value;
//! # Ok::<(), amari_holographic::HolographicError>(())
//! ```
//!
//! When the `holographic` feature is enabled, types are re-exported here for
//! backward compatibility.
//!
//! # Feature Flags
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `std` | Standard library support (default) |
//! | `high-precision` | Enable 128-bit and arbitrary precision floats |
//! | `holographic` | Re-export `amari-holographic` types |
//! | `parallel` | Enable parallel operations via rayon |

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

// Re-export precision types from all constituent crates
pub use amari_core::{ExtendedFloat, PrecisionFloat, StandardFloat};
pub use amari_dual::{ExtendedDual, ExtendedMultiDual, StandardDual, StandardMultiDual};
pub use amari_tropical::{ExtendedTropical, StandardTropical};

#[cfg(feature = "high-precision")]
pub use amari_core::HighPrecisionFloat;

// Core fusion types
pub mod types;

// Domain modules
pub mod attention;
pub mod evaluation;
pub mod optimizer;

// Verification modules
pub mod verified;
pub mod verified_contracts;

// Re-export core types
pub use types::{EvaluationError, EvaluationResult, TropicalDualClifford};

// Re-export holographic types when feature is enabled (backward compatibility)
#[cfg(feature = "holographic")]
pub use amari_holographic::{
    // Algebra types
    AlgebraConfig,
    AlgebraError,
    AlgebraResult,
    // Memory types
    Bindable,
    BindingAlgebra,
    CapacityInfo,
    Cl3,
    // Resonator types
    CleanupResult,
    CliffordAlgebra,
    FHRRAlgebra,
    FactorizationResult,
    GeometricAlgebra,
    // Error types
    HolographicError,
    HolographicMemory,
    HolographicResult,
    MAPAlgebra,
    MembershipEvidence,
    NearestKey,
    ProductCliffordAlgebra,
    Resonator,
    ResonatorConfig,
    RetrievalResult,
    WeightedResult,
    WeightedStatus,
};

// GPU acceleration exports
