//! Holographic memory using generalized binding algebras.
//!
//! This module provides Vector Symbolic Architecture (VSA) operations
//! using the [`crate::algebra::BindingAlgebra`] trait, enabling:
//!
//! - Compositional key-value storage in superposition
//! - Content-addressable retrieval with automatic cleanup
//! - Temperature-controlled soft↔hard retrieval
//! - Support for multiple algebra backends (Clifford, FHRR, MAP, etc.)
//!
//! # Holographic Reduced Representations
//!
//! A holographic memory stores associations `(key, value)` by:
//! 1. **Binding**: `bound = key ⊛ value` (creates association, result dissimilar to both inputs)
//! 2. **Bundling**: `memory = bound₁ ⊕ bound₂ ⊕ ... ⊕ boundₙ` (superposition in single vector)
//! 3. **Retrieval**: `value ≈ key⁻¹ ⊛ memory` (unbind with inverse, get target + noise)
//!
//! The noise term is the sum/max of cross-talk from other items. Capacity is O(dim / log dim) items.
//!
//! # Temperature-Parameterized Operations
//!
//! The Maslov dequantization connects soft (standard) and hard (tropical) operations:
//!
//! ```text
//! β → 0:   soft operations (sum, product)
//! β → ∞:   hard operations (max, +)
//! ```
//!
//! Encoding should be soft (preserves superposition), retrieval can be hard (automatic cleanup).
//!
//! # Example
//!
//! ```ignore
//! use amari_holographic::{HolographicMemory, AlgebraConfig};
//! use amari_holographic::algebra::ProductCl3x32;
//!
//! let mut memory = HolographicMemory::<ProductCl3x32>::new(AlgebraConfig::default());
//!
//! let key = ProductCl3x32::random_versor(2);
//! let value = ProductCl3x32::random_versor(2);
//!
//! memory.store(&key, &value);
//!
//! let retrieved = memory.retrieve(&key);
//! assert!(retrieved.confidence > 0.9);
//! ```

mod error;
mod holographic_memory;
mod resonator;

pub use error::{HolographicError, HolographicResult};
pub use holographic_memory::{CapacityInfo, HolographicMemory, RetrievalResult};
pub use resonator::{CleanupResult, FactorizationResult, Resonator, ResonatorConfig};

// Re-export the Bindable trait (now an alias for BindingAlgebra compatibility)
pub use crate::algebra::BindingAlgebra as Bindable;
