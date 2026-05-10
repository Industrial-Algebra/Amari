//! Term rewriting systems.

mod matching;
mod rule;
mod substitution;
mod term;

pub use matching::match_pattern;
pub use rule::Rule;
pub use substitution::Substitution;
pub use term::{Symbol, Term, Variable};
