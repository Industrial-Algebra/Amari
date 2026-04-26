//! Common imports for `amari-cgt`.

pub use crate::arena::GameArena;
pub use crate::error::{CgtError, Result};
pub use crate::form::GameForm;
pub use crate::game::{
    Birthday, CanonicalGame, GameComparison, GameId, GameInspection, NumericGameWitness,
    OutcomeClass,
};
pub use crate::generation::{
    CanonicalCorpus, CorpusStats, LayerAnalysis, LayerAnalysisReport, LayerGrowthCounts,
    LayerGrowthSummary, OutcomeCounts, MAX_EXHAUSTIVE_OPTION_UNIVERSE,
};
pub use crate::nimber::Nimber;
