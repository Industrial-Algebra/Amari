//! Computational combinatorial game theory for Amari.
//!
//! The current implementation focuses on short normal-play games and provides:
//!
//! - arena-backed game storage
//! - arena-independent structural game forms for import/export
//! - birthdays
//! - explicit numeric-validation witnesses for downstream surreal conversion
//! - negation, addition, subtraction, and canonicalization
//! - partial comparison and outcome classes
//! - impartiality checks and Grundy values
//! - small exhaustive generators for exact and bounded birthday/node-count layers
//! - layer analysis reports for canonical growth and classification trends
//!
//! # Example
//!
//! ```rust
//! use amari_cgt::{GameArena, GameComparison, OutcomeClass};
//!
//! let mut arena = GameArena::new();
//! let zero = arena.zero();
//! let star = arena.star()?;
//! let one = arena.one()?;
//!
//! assert_eq!(arena.compare(one, zero)?, GameComparison::Greater);
//! assert_eq!(arena.outcome(star)?, OutcomeClass::NextPlayerWins);
//! # Ok::<(), amari_cgt::CgtError>(())
//! ```

pub mod arena;
pub mod error;
pub mod form;
pub mod game;
pub mod generation;
pub mod nimber;
pub mod prelude;

pub use arena::GameArena;
pub use error::{CgtError, Result};
pub use form::GameForm;
pub use game::{Birthday, CanonicalGame, GameComparison, GameId, NumericGameWitness, OutcomeClass};
pub use generation::{
    CanonicalCorpus, CorpusStats, LayerAnalysis, LayerAnalysisReport, OutcomeCounts,
    MAX_EXHAUSTIVE_OPTION_UNIVERSE,
};
pub use nimber::Nimber;
