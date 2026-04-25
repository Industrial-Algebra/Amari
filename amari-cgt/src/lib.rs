//! Computational combinatorial game theory for Amari.
//!
//! The current implementation focuses on short normal-play games and provides:
//!
//! - arena-backed game storage
//! - birthdays
//! - negation, addition, and subtraction
//! - partial comparison and outcome classes
//! - impartiality checks and Grundy values

pub mod arena;
pub mod error;
pub mod game;
pub mod nimber;
pub mod prelude;

pub use arena::GameArena;
pub use error::{CgtError, Result};
pub use game::{Birthday, CanonicalGame, GameComparison, GameId, OutcomeClass};
pub use nimber::Nimber;
