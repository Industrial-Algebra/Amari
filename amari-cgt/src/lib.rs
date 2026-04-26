//! Computational combinatorial game theory for Amari.
//!
//! The current implementation focuses on short normal-play games and provides:
//!
//! - arena-backed game storage
//! - birthdays
//! - negation, addition, subtraction, and canonicalization
//! - partial comparison and outcome classes
//! - impartiality checks and Grundy values
//! - small exhaustive generators for exact and bounded birthday/node-count layers
//! - layer analysis reports for canonical growth and classification trends

pub mod arena;
pub mod error;
pub mod game;
pub mod generation;
pub mod nimber;
pub mod prelude;

pub use arena::GameArena;
pub use error::{CgtError, Result};
pub use game::{Birthday, CanonicalGame, GameComparison, GameId, OutcomeClass};
pub use generation::{
    CanonicalCorpus, CorpusStats, LayerAnalysis, LayerAnalysisReport, OutcomeCounts,
    MAX_EXHAUSTIVE_OPTION_UNIVERSE,
};
pub use nimber::Nimber;
