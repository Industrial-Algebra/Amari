// SPDX-License-Identifier: MIT OR Apache-2.0

//! Public types for Rust source inspection.
//!
//! All types in this module are re-exported through the parent `rust`
//! module and the inspect module root.

use serde::{Deserialize, Serialize};

use crate::inspect::snapshot::{InspectionLimit, SnapshotState, SourceLocation};

// ============================================================================
// RustSourceInspection
// ============================================================================

/// The result of inspecting Rust source files in a project for Amari API usage.
///
/// # State semantics
///
/// On `Ok`, the inspection always contains the collected evidence across all
/// accepted `.rs` and `README.md` files.
///
/// [`state`](Self::state) signals whether evidence is complete or truncated:
///
/// - [`SnapshotState::Complete`] — all files were parsed within resource limits.
/// - [`SnapshotState::LimitExceeded`] — one or more resource limits were hit.
///
/// # Empty input hash
///
/// When no Rust or README files are accepted,
/// [`input_hash`](Self::input_hash) is the SHA-256 of the empty framed
/// input set — i.e. `SHA-256("")`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RustSourceInspection {
    /// All detected Amari usages (imports, path references).
    pub usages: Vec<RustUsage>,
    /// File classification for every accepted `.rs` file.
    pub file_kinds: Vec<RustFileKind>,
    /// Crate-level attributes (e.g. `#![no_std]`, `#![forbid(...)]`).
    pub crate_attributes: Vec<RustCrateAttribute>,
    /// Syntactic `cfg` and `cfg_attr` evidence.
    pub cfg_evidence: Vec<RustCfgEvidence>,
    /// Curated domain/platform vocabulary from docs, comments, and README.
    pub vocabulary: Vec<VocabularyEvidence>,
    /// Non-fatal warnings accumulated during inspection.
    pub warnings: Vec<RustInspectionWarning>,
    /// Deterministic framed SHA-256 hash over accepted Rust/README bytes.
    pub input_hash: String,
    /// Overall inspection state.
    pub state: SnapshotState,
    /// Number of Rust/README files inspected (accepted).
    pub inspected_file_count: u64,
    /// Total bytes of Rust/README content inspected (accepted).
    pub total_bytes: u64,
    /// Source locations for every accepted input file.
    ///
    /// Every evidence item's `content_hash` and `path` resolve to one of
    /// these entries. Files with no evidence (no usages, vocabulary, etc.)
    /// are still listed.
    pub input_files: Vec<SourceLocation>,
}

// ============================================================================
// RustUsage
// ============================================================================

/// Evidence of Amari API usage in Rust source code.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RustUsage {
    /// The canonical Cargo package name (e.g. `amari`, `amari-core`).
    ///
    /// Resolved from the Cargo alias with `-` → `_` normalization and
    /// rename maps from [`crate::inspect::CargoInspection`].
    pub crate_name: String,
    /// The local alias used in source (e.g. `amari`, `amari_core`, `ama`).
    pub alias: String,
    /// Path segments after the crate root (e.g. `["tropical", "TropicalNumber"]`
    /// for `use amari::tropical::TropicalNumber`). Empty for bare crate imports.
    pub path_segments: Vec<String>,
    /// The kind of usage.
    pub kind: RustUsageKind,
    /// Source location of this usage.
    pub source: SourceLocation,
}

/// How the Amari API was referenced in source.
///
/// Every variant must be implementable — unused variants are compile errors
/// that must be resolved before review passes.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RustUsageKind {
    /// A `use` import — single item, renamed, grouped, or glob.
    Use,
    /// An `extern crate` declaration.
    ExternCrate,
    /// A path expression like `amari::tropical::TropicalNumber::new(...)`.
    PathExpression,
    /// A type path like `fn foo(x: amari::tropical::TropicalNumber) -> ...`.
    PathType,
    /// A trait bound path like `T: amari::trait_name::SomeTrait`.
    PathTrait,
    /// A macro invocation path like `amari::some_macro!()`.
    PathMacro,
}

// ============================================================================
// RustFileKind
// ============================================================================

/// Classification of an accepted `.rs` file within the project.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RustFileKind {
    /// A library crate root (`src/lib.rs` or declared lib path).
    Library {
        /// The package this file belongs to.
        package: String,
        /// Relative path from project root.
        path: String,
    },
    /// A binary crate root (`src/main.rs` or declared bin path).
    Binary {
        /// The package this file belongs to.
        package: String,
        /// Relative path from project root.
        path: String,
    },
    /// A test file (`tests/*.rs` or declared test path).
    Test {
        /// The package this file belongs to.
        package: String,
        /// Relative path from project root.
        path: String,
    },
    /// An example file (`examples/*.rs` or declared example path).
    Example {
        /// The package this file belongs to.
        package: String,
        /// Relative path from project root.
        path: String,
    },
    /// A benchmark file (`benches/*.rs` or declared bench path).
    Bench {
        /// The package this file belongs to.
        package: String,
        /// Relative path from project root.
        path: String,
    },
    /// A build script (`build.rs`).
    BuildScript {
        /// The package this file belongs to.
        package: String,
        /// Relative path from project root.
        path: String,
    },
    /// Any other `.rs` file that doesn't match the above categories.
    Other {
        /// The package this file belongs to.
        package: String,
        /// Relative path from project root.
        path: String,
    },
}

// ============================================================================
// RustCrateAttribute
// ============================================================================

/// A crate-level attribute detected in a Rust source file.
///
/// Captures `#![...]` inner attributes like `#![no_std]`, `#![forbid(...)]`,
/// `#![deny(...)]`, `#![allow(...)]`, `#![no_main]`, etc.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RustCrateAttribute {
    /// Relative path of the file containing this attribute.
    pub path: String,
    /// The normalized attribute string (e.g. `"no_std"`, `"forbid(unsafe_code)"`).
    pub attribute: String,
    /// Source location within the file.
    pub source: Option<SourceLocation>,
}

// ============================================================================
// RustCfgEvidence
// ============================================================================

/// Syntactic `cfg` or `cfg_attr` evidence captured from source.
///
/// Captured conservatively as normalized syntactic strings — never evaluated
/// for truth.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RustCfgEvidence {
    /// Relative path of the file containing this cfg evidence.
    pub path: String,
    /// The normalized cfg predicate (e.g. `"feature = \"gpu\""`).
    pub cfg_predicate: String,
    /// Whether this was a `#[cfg_attr(...)]` vs `#[cfg(...)]`.
    pub is_cfg_attr: bool,
    /// Source location within the file.
    pub source: Option<SourceLocation>,
}

// ============================================================================
// VocabularyEvidence
// ============================================================================

/// A curated domain/platform term extracted from doc comments, regular
/// comments, or README text.
///
/// Terms are matched against a fixed allowlist and normalized.
/// Full source text is never persisted.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct VocabularyEvidence {
    /// Relative path of the file containing this term.
    pub path: String,
    /// The normalized vocabulary term.
    pub term: String,
    /// Source location within the file.
    pub source: Option<SourceLocation>,
}

// ============================================================================
// RustInspectionWarning
// ============================================================================

/// Non-fatal warnings produced during Rust source inspection.
///
/// # Privacy guarantee
///
/// Warnings never contain source text, full comment text, absolute
/// root paths, or secret strings. Serialized forms are safe for
/// sharing across trust boundaries.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum RustInspectionWarning {
    /// A Rust source file could not be parsed by `syn`.
    MalformedSource {
        /// Relative path of the malformed file.
        path: String,
        /// Sanitized category of the parse error (never contains source excerpts).
        reason: String,
        /// Optional 1-based line number of the parse error.
        line: Option<u32>,
        /// Optional 1-based column number of the parse error.
        column: Option<u32>,
        /// SHA-256 content hash of the malformed file.
        content_hash: String,
    },
    /// A resource limit was exceeded during inspection.
    LimitExceeded {
        /// The typed limit that was exceeded.
        limit: InspectionLimit,
    },
    /// A file was a symlink and was not followed.
    SymlinkedFile {
        /// Relative path of the symlinked file.
        path: String,
    },
    /// A file path contains non-UTF-8 components.
    NonUtf8Path {
        /// Relative path context (best-effort, losslessly decoded where possible).
        path_hint: String,
    },
    /// A `.rs` or README candidate was not valid UTF-8.
    InvalidUtf8Source {
        /// Relative path of the file.
        path: String,
    },
    /// A `.rs` or README candidate exceeded the per-file byte limit.
    OversizedFile {
        /// Relative path of the oversized file.
        path: String,
        /// File size in bytes.
        size: u64,
        /// Configured per-file byte limit.
        limit: u64,
    },
    /// A `.rs` or README candidate could not be read (I/O error).
    ReadFailure {
        /// Relative path of the unreadable file.
        path: String,
        /// Sanitized reason (never contains source or full error).
        reason: String,
    },
    /// Vocabulary evidence was truncated at the global cap.
    VocabularyTruncated {
        /// Total vocabulary evidence count before truncation.
        total: usize,
        /// Maximum vocabulary capacity.
        cap: usize,
    },
}
