// SPDX-License-Identifier: MIT OR Apache-2.0

//! Regular tree languages: ranked alphabets, validated tree automata,
//! and the ADR 0001 preimage classification/certificate surface.
//!
//! Tasks 18-24 of the 0.25 inverse-rewrite expansion: the canonical
//! language representation is an epsilon-free nondeterministic
//! bottom-up finite tree automaton ([`TreeAutomaton`]) over a
//! validated ranked alphabet ([`RankedSymbol`]). Constructors
//! validate before allocation and return typed malformed or limit
//! errors; storage is canonical, so equality and ordering never
//! depend on the caller's input order. Task 24 adds the exhaustive
//! TRS taxonomy ([`TrsClass`]) and evidence certificates
//! ([`PreimageCertificate`]) that gate the approved exact preimage
//! constructions (ADR 0001); no preimage automaton construction
//! exists yet (Tasks 25-26).

mod alphabet;
mod automaton;
mod certificate;
mod classify;
mod determinize;
mod grammar;
mod limits;
mod minimize;
mod operations;
mod witness;

pub use alphabet::RankedSymbol;
pub use automaton::{AcceptingRun, TreeAutomaton, TreeState, TreeTransition};
pub use certificate::{language_digest, system_digest, CertificateAuthority, PreimageCertificate};
pub use classify::{
    classify_system, validate_alphabet, Classification, PreimageCapability, PreimageConstruction,
    PreimageOperation, TrsClass,
};
pub use determinize::MAX_DETERMINIZED_SUBSETS;
pub use grammar::{GrammarProduction, Nonterminal, RegularTreeGrammar, GRAMMAR_SYNTAX_HEADER};
pub use limits::TreeAutomatonLimits;
