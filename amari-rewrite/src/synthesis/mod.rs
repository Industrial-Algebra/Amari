//! Lightweight rule synthesis helpers.

mod anti_unify;
mod inference;

pub use anti_unify::{anti_unify, anti_unify_all, anti_unify_iter, VarGen};
pub use inference::{infer_rule, infer_rules};
