//! Tropical (max-plus) algebra for efficient LLM operations
//!
//! Tropical algebra replaces traditional (+, ×) with (max, +), which converts
//! expensive softmax operations into simple max operations. This is particularly
//! useful for finding most likely sequences and optimization in neural networks.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

// Import precision types from amari-core
#[cfg(feature = "high-precision")]
pub use amari_core::HighPrecisionFloat;
pub use amari_core::{ExtendedFloat, PrecisionFloat, StandardFloat};

// Core tropical algebra types
pub mod semiring;
pub mod types;

// Domain modules
pub mod error;
pub mod polytope;
pub mod viterbi;

#[cfg(feature = "phantom-types")]
pub mod verified;

#[cfg(feature = "contracts")]
pub mod verified_contracts;

// Re-export error types
pub use error::{TropicalError, TropicalResult};

// Re-export core traits and types
pub use semiring::Semiring;
pub use types::{StandardTropical, TropicalMatrix, TropicalMultivector, TropicalNumber};

#[cfg(feature = "high-precision")]
pub use types::ExtendedTropical;

// Re-export GPU functionality when available
