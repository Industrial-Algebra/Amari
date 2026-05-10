//! Term rewriting systems.

mod matching;
mod rule;
mod substitution;
mod system;
mod term;

pub use matching::match_pattern;
pub use rule::Rule;
pub use substitution::Substitution;
pub use system::TermSystem;
pub use term::{Symbol, Term, Variable};
