// SPDX-License-Identifier: MIT OR Apache-2.0

//! Rust project using Amari APIs for tropical algebra and geometric algebra.
//!
//! This crate demonstrates usage of amari for shortest path and optimization use cases.
//! It covers WASM/WebAssembly targets and no_std environments.
//!
//! # Features
//! - Tropical algebra (max-plus semiring) for shortest path computation
//! - Geometric algebra for spatial transforms
//! - Autodiff via dual numbers

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![cfg_attr(feature = "wasm", no_main)]

// Direct crate import
use amari_core::Multivector;

// Umbrella crate path import — amari::tropical must map to umbrella package `amari`
use amari::tropical::TropicalNumber;

// Grouped imports from umbrella
use amari::{
    tropical::{TropicalMatrix, TropicalMultivector},
};

// Renamed import (alias)
use amari::dual::DualNumber as Dual;

// Glob import
use amari::tropical::*;

// cfg-gated import
#[cfg(feature = "gpu")]
use amari::gpu::GpuContext;

// cfg_attr on an item
#[cfg_attr(feature = "nightly", doc = "Nightly-only tropical utilities")]
pub mod tropical_util {
    //! Tropical algebra utility functions for shortest path optimization.
    //!
    //! Also usable for WASM targets and native FFI via BLAS linking.
    /// A tropical number wrapper for max-plus max-path computation.
    pub struct TropicalWrapper;
}

// Item with doc attribute containing vocabulary: geometric algebra, autodiff
/// Performs geometric product using multivectors with dual number autodiff
/// for WASM-compatible network optimization.
pub fn geometric_dual(a: &Multivector<3, 0, 0>, b: &Multivector<3, 0, 0>) -> Multivector<3, 0, 0> {
    a.geometric_product(b)
}

// Type alias using umbrella path
pub type TropicNum = amari::tropical::TropicalNumber<f64>;

/// Uses fully-qualified Amari path in expression context.
pub fn make_tropical() -> amari::tropical::TropicalNumber<f64> {
    amari::tropical::TropicalNumber::new(1.0)
}
