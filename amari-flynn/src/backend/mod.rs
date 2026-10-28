//! Verification backends
//!
//! - [`monte_carlo`]: Statistical sampling-based verification using Hoeffding bounds
//! - `smt`: SMT-LIB2 formal proof obligation generation for external solvers (std builds)

pub mod monte_carlo;
#[cfg(feature = "std")]
mod rational;
#[cfg(feature = "std")]
pub mod smt;
