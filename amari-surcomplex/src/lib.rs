//! Exact rational surcomplex numbers for Amari.
//!
//! This crate provides [`RationalSurcomplex`], an exact complex number
//! backed by [`RationalSurreal`] from `amari-surreal`.  It supports
//! addition, subtraction, multiplication, division, conjugation, and
//! norm — all exact without floating-point rounding.
//!
//! # Example
//!
//! ```rust
//! use amari_surcomplex::RationalSurcomplex;
//! use amari_surreal::RationalSurreal;
//!
//! // 1 / (1 + 1/2 i) = 4/5 - 2/5 i
//! let one = RationalSurreal::one();
//! let half = RationalSurreal::from_ratio(1, 2).unwrap();
//! let z = RationalSurcomplex::from_parts(one, half);
//! let q = RationalSurcomplex::one().checked_div(&z).unwrap();
//! assert_eq!(q.real().to_string(), "4/5");
//! assert_eq!(q.imag().to_string(), "-2/5");
//! ```

pub mod error;
pub mod prelude;
pub mod rational;

pub use error::{Result, SurcomplexError};
pub use rational::RationalSurcomplex;
