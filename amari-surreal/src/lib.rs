//! Computable short surreal numbers for Amari.
//!
//! The current implementation focuses on the short-surreal / dyadic layer:
//!
//! - exact dyadic arithmetic
//! - conversion from numeric short games in `amari-cgt`
//! - reconstruction back into numeric short games
//! - simplest-number construction for finite cuts
//!
//! # Example
//!
//! ```rust
//! use amari_cgt::GameArena;
//! use amari_surreal::{Dyadic, ShortSurreal};
//!
//! let mut arena = GameArena::new();
//! let zero = arena.zero();
//! let one = arena.one()?;
//! let half_game = arena.from_options([zero], [one])?;
//! let half = ShortSurreal::from_game(&mut arena, half_game)?;
//!
//! assert_eq!(half.to_dyadic(), Dyadic::new(1, 1));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod dyadic;
pub mod error;
pub mod numeric;
pub mod prelude;
pub mod rational;
pub mod short;

pub use dyadic::Dyadic;
pub use error::{Result, SurrealError};
pub use numeric::NumericGame;
pub use rational::RationalSurreal;
pub use short::ShortSurreal;
