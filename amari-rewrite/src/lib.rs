//! Abstract and term rewriting systems for Amari.
//!
//! `amari-rewrite` provides foundational rewriting tools: abstract rewriting
//! systems (ARS), first-order term rewriting systems (TRS), bounded inverse
//! rewriting, and lightweight rule synthesis via anti-unification.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

pub mod ars;
pub mod error;
pub mod inverse;
pub mod prelude;
pub mod rewritable;
pub mod synthesis;
pub mod trs;

pub use error::{RewriteError, RewriteResult};
pub use rewritable::{Path, Rewritable};
