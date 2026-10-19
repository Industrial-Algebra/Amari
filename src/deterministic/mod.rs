//! Deterministic computation module for networked physics applications
//!
//! Provides bit-exact reproducibility across platforms (x86-64, ARM64,
//! WASM32) and compiler optimization levels for networked physics
//! simulations — **on the finite domain**: computations whose inputs,
//! intermediates, and final results are finite. NaN bit patterns carry NO
//! promise (Rust permits NaN sign/payload variation even within one
//! binary). See the [`Deterministic`] trait for the precise contract.
//!
//! # Overview
//!
//! Standard IEEE 754 floating-point has platform-specific behaviors that
//! break determinism in networked applications. This module provides
//! deterministic alternatives through:
//!
//! - Fixed-iteration algorithms (no early termination, no libm calls)
//! - Pure-arithmetic polynomial approximations for transcendentals
//!   (accuracy-limited; see `approx` — no accuracy claim is made)
//!
//! # Example
//!
//! ```
//! # #[cfg(feature = "deterministic")]
//! # {
//! use amari::deterministic::*;
//! use amari::deterministic::ga2d::*;
//!
//! // Create deterministic ship orientation
//! let mut rotation = DetRotor2::IDENTITY;
//!
//! // Rotate by fixed amount (bit-exact on all platforms — finite domain)
//! let delta = DetRotor2::from_angle(DetF32::from_f32(0.1));
//! rotation = rotation * delta;
//!
//! // Transform vectors
//! let thrust_dir = rotation.transform(DetVector2::X_AXIS);
//! # }
//! ```

mod approx; // Extends scalar with trig functions
pub mod ga2d;
mod scalar;

// Re-export primary types
pub use ga2d::{DetRotor2, DetVector2};
pub use scalar::{DetF32, Deterministic};
