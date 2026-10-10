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
//! constructions (ADR 0001); Task 25 adds the approved exact one-step
//! and finite-horizon preimage constructions ([`one_step_preimage`],
//! [`finite_horizon_preimage`]); Task 26 adds the exact unbounded
//! saturation preimage ([`saturation_preimage`]); Task 27 adds the
//! witnessed lower-bound approximation surface for approximation-only
//! cells ([`one_step_lower_bound`], [`finite_horizon_lower_bound`],
//! [`saturation_lower_bound`]), with `Partial` authority and an absent
//! upper bound.

mod alphabet;
mod approximate;
mod automaton;
mod certificate;
mod classify;
mod determinize;
mod grammar;
mod limits;
mod minimize;
mod operations;
mod preimage;
mod saturation;
mod witness;

pub use alphabet::RankedSymbol;
pub use approximate::{
    finite_horizon_lower_bound, one_step_lower_bound, saturation_lower_bound, ApproximationEvent,
    ApproximationOutcome,
};
pub use automaton::{AcceptingRun, TreeAutomaton, TreeState, TreeTransition};
pub use certificate::{language_digest, system_digest, CertificateAuthority, PreimageCertificate};
pub use classify::{
    classify_system, validate_alphabet, Classification, PreimageCapability, PreimageConstruction,
    PreimageOperation, TrsClass,
};
pub use determinize::MAX_DETERMINIZED_SUBSETS;
pub use grammar::{GrammarProduction, Nonterminal, RegularTreeGrammar, GRAMMAR_SYNTAX_HEADER};
pub use limits::TreeAutomatonLimits;
pub use preimage::{
    finite_horizon_preimage, identity_preimage, one_step_preimage, PreimageOutcome,
};
pub use saturation::saturation_preimage;
