//! Term rewriting systems.

mod substitution;
mod term;

pub use substitution::Substitution;
pub use term::{Symbol, Term, Variable};
