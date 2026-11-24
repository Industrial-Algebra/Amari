//! Abstract and term rewriting systems for Amari.
//!
//! `amari-rewrite` provides foundational rewriting tools: abstract rewriting
//! systems (ARS), first-order term rewriting systems (TRS), bounded inverse
//! rewriting, and lightweight rule synthesis via anti-unification.
//!
//! The analysis surface adds metered critical-pair enumeration, bounded
//! joinability and local-confluence certification, and lexicographic
//! path-order termination proofs ([`analysis`]). The non-default
//! `completion` feature adds bounded Knuth–Bendix completion with
//! convergence postchecks and completion-assisted inverse search
//! (`completion`): a convergent completion may propose predecessor
//! candidates, but every witness is replayed against the original system
//! and negative authority never transfers from the completed system.
//!
//! The non-default `holographic-guidance` feature adds deterministic
//! holographic trace recall (`holographic`): replay-validated inverse traces
//! are encoded into holographic vectors and ranked against a query by
//! similarity, returning advisory evidence with the decomposition exposed. A
//! default build exposes no holographic surface at all.

#![cfg_attr(not(feature = "std"), no_std)]
// Default builds expose no holographic surface: the module is behind
// `holographic-guidance`, and this doctest is compiled only when the feature is
// off, so it proves the module path does not resolve.
#![cfg_attr(
    not(feature = "holographic-guidance"),
    doc = "```compile_fail
use amari_rewrite::holographic;
```"
)]

extern crate alloc;

// Lets derive-generated `::amari_rewrite::...` paths resolve inside this
// crate's own targets (unit tests, examples) where proc-macro-crate
// reports `Itself`.
extern crate self as amari_rewrite;

pub mod analysis;
pub mod ars;
#[cfg(feature = "completion")]
pub mod completion;
pub mod error;
pub mod inverse;
pub mod language;
pub mod prelude;
pub mod relation;
pub mod reversible;
pub mod rewritable;
pub mod synthesis;
pub mod trs;

pub use error::{RewriteError, RewriteResult};
pub use rewritable::{Path, Rewritable};

#[cfg(feature = "neural")]
pub mod neural;

#[cfg(feature = "holographic-guidance")]
pub mod holographic;

#[cfg(feature = "smt")]
pub mod smt;

#[cfg(feature = "network")]
pub mod network;

#[cfg(feature = "macros")]
pub use amari_rewrite_macros::{relation, rule, term, Rewritable};
