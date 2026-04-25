//! Computable short surreal numbers for Amari.
//!
//! The current implementation focuses on the short-surreal / dyadic layer:
//!
//! - exact dyadic arithmetic
//! - conversion from numeric short games in `amari-cgt`
//! - simplest-number construction for finite cuts

pub mod dyadic;
pub mod error;
pub mod numeric;
pub mod prelude;
pub mod short;

pub use dyadic::Dyadic;
pub use error::{Result, SurrealError};
pub use numeric::NumericGame;
pub use short::ShortSurreal;
