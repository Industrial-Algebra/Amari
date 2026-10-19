//! Lightweight rule synthesis helpers.

mod anti_unify;
mod inference;
mod refinement;

pub use anti_unify::{anti_unify, anti_unify_all, anti_unify_iter, VarGen};
pub use inference::{infer_rule, infer_rules};
pub use refinement::{
    specialize_rules, CoverageReport, InconclusiveReason, RuleCoverage, SpecializationConfig,
    SpecializationOutcome,
};
