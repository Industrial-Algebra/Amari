//! Lightweight rule synthesis helpers.

mod anti_unify;

pub use anti_unify::{anti_unify, anti_unify_all, anti_unify_iter, VarGen};
