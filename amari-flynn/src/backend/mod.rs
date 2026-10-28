//! Verification backends
//!
//! - [`monte_carlo`]: Statistical sampling-based verification using Hoeffding bounds
//! - [`smt`]: SMT-LIB2 formal proof obligation generation for external solvers
//! - [`why3`]: Legacy Why3 placeholder (deprecated in favor of `smt`)

pub mod monte_carlo;
#[cfg(feature = "std")]
mod rational;
#[cfg(feature = "std")]
pub mod smt;
