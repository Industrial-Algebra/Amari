//! Tropical algebra and ordinal-weighted optimization carriers.
//!
//! `amari-tropical` currently supports two complementary layers:
//!
//! - float-oriented max-plus tropical arithmetic via [`TropicalNumber`], [`TropicalMatrix`],
//!   and related utilities
//! - arena-backed ordinals below `ε₀` via [`OrdinalArena`] and [`OrdinalWeight`]
//!
//! Tropical max-plus arithmetic replaces traditional `(+ , ×)` with `(max, +)`,
//! which is useful for path/ranking computations, dynamic programming, and
//! optimization-oriented workloads. The ordinal layer is intentionally bounded to
//! ordinals below `ε₀`, and float-oriented modules such as `viterbi` and `polytope`
//! remain separate from that arena-backed substrate.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

// Import precision types from amari-core
#[cfg(feature = "high-precision")]
pub use amari_core::HighPrecisionFloat;
pub use amari_core::{ExtendedFloat, PrecisionFloat, StandardFloat};

// Core tropical algebra types
pub mod ordinal;
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
pub use ordinal::{
    CnfTerm, OrdinalArena, OrdinalId, OrdinalInspection, OrdinalKind, OrdinalWeight,
    OrdinalWeightInspection,
};
pub use semiring::{fold_oplus, fold_otimes, Semiring};
pub use types::{StandardTropical, TropicalMatrix, TropicalMultivector, TropicalNumber};

#[cfg(feature = "high-precision")]
pub use types::ExtendedTropical;

// Re-export GPU functionality when available
