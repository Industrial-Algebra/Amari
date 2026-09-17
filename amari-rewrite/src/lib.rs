//! Abstract and term rewriting systems for Amari.
//!
//! `amari-rewrite` provides foundational rewriting tools: abstract rewriting
//! systems (ARS), first-order term rewriting systems (TRS), bounded inverse
//! rewriting, and lightweight rule synthesis via anti-unification.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

// Lets derive-generated `::amari_rewrite::...` paths resolve inside this
// crate's own targets (unit tests, examples) where proc-macro-crate
// reports `Itself`.
extern crate self as amari_rewrite;

pub mod analysis;
pub mod ars;
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

#[cfg(feature = "smt")]
pub mod smt;

#[cfg(feature = "network")]
pub mod network;

#[cfg(feature = "macros")]
pub use amari_rewrite_macros::{relation, rule, term, Rewritable};
