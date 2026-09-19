// SPDX-License-Identifier: MIT OR Apache-2.0

//! Regular tree languages: ranked alphabets and validated tree
//! automata.
//!
//! Task 18 of the 0.25 inverse-rewrite expansion: the canonical
//! language representation is an epsilon-free nondeterministic
//! bottom-up finite tree automaton ([`TreeAutomaton`]) over a
//! validated ranked alphabet ([`RankedSymbol`]). Constructors
//! validate before allocation and return typed malformed or limit
//! errors; storage is canonical, so equality and ordering never
//! depend on the caller's input order.

mod alphabet;
mod automaton;
mod determinize;
mod grammar;
mod limits;
mod minimize;
mod operations;
mod witness;

pub use alphabet::RankedSymbol;
pub use automaton::{AcceptingRun, TreeAutomaton, TreeState, TreeTransition};
pub use determinize::MAX_DETERMINIZED_SUBSETS;
pub use grammar::{GrammarProduction, Nonterminal, RegularTreeGrammar, GRAMMAR_SYNTAX_HEADER};
pub use limits::TreeAutomatonLimits;
