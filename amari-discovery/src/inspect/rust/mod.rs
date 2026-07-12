// SPDX-License-Identifier: MIT OR Apache-2.0

//! Rust source inspection — Amari API usage detection from `.rs` files.
//!
//! This module provides [`inspect_rust_sources`] which parses Rust source
//! files with `syn`, detects Amari crate usages (imports, path references),
//! classifies files, captures cfg/crate attributes, and extracts curated
//! domain vocabulary from docs, comments, and README files.
//!
//! # Public API
//!
//! - [`types`] — Public domain types re-exported at crate level.
//! - [`inspect_rust_sources`] — Main entry point for Rust source inspection.

pub mod types;

// Private modules
pub(super) mod inspect;
pub(super) mod parser;
pub(super) mod vocabulary;

// Re-export public types
pub use inspect::inspect_rust_sources;
pub use types::{
    RustCfgEvidence, RustCrateAttribute, RustFileKind, RustInspectionWarning, RustSourceInspection,
    RustUsage, RustUsageKind, VocabularyEvidence,
};
