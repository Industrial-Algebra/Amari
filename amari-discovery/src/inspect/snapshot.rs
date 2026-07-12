// SPDX-License-Identifier: MIT OR Apache-2.0

//! Domain types for project snapshots produced by the filesystem inspector.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::DiscoveryResult;

use super::limits::InspectionLimits;

// ---------------------------------------------------------------------------
// InspectionLimit — typed limit exceeded reason
// ---------------------------------------------------------------------------

/// A specific resource limit that halted inspection, with the configured
/// maximum and optionally the observed value at the time of the halt.
///
/// Agents can branch on the limit variant to decide whether to retry with
/// relaxed bounds, synthesize a partial plan, or report a permanent blocker.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum InspectionLimit {
    /// The considered-file-count limit was reached.
    FileCount {
        /// Configured maximum number of regular non-secret files considered.
        max: u64,
        /// Number of files considered (including oversized/unreadable) before
        /// the limit was hit.
        observed: u64,
    },
    /// The aggregate byte limit was reached.
    TotalBytes {
        /// Configured maximum inspection bytes.
        max: u64,
        /// Bytes accounted before the limit was hit.
        observed: u64,
    },
    /// The traversal depth limit truncated potential descendants.
    TraversalDepth {
        /// Configured maximum traversal depth.
        max: u64,
    },
    /// The wall-clock time limit was reached.
    WallClock {
        /// Configured maximum wall-clock milliseconds.
        max_millis: u64,
        /// Milliseconds elapsed when the limit was hit.
        observed_millis: u64,
    },
}

// ---------------------------------------------------------------------------
// SnapshotState
// ---------------------------------------------------------------------------

/// The completion state of a project snapshot.
///
/// When a resource limit is hit, the snapshot is still internally
/// consistent — it represents deterministic partial evidence — but
/// the state is [`SnapshotState::LimitExceeded`] rather than
/// [`SnapshotState::Complete`].
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum SnapshotState {
    /// Inspection completed within all resource limits.
    Complete,
    /// One or more resource limits were exceeded during traversal.
    /// Partial snapshot data is internally consistent.
    LimitExceeded {
        /// The typed limit that was exceeded.
        limit: InspectionLimit,
    },
}

// ---------------------------------------------------------------------------
// ProjectKind
// ---------------------------------------------------------------------------

/// The detected project kind based on manifest signals in accepted files.
///
/// Detailed language extraction is deferred to later task phases.
/// This minimal detection covers the common Rust and npm/TypeScript
/// project layouts.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectKind {
    /// A Cargo.toml file was accepted during inspection.
    RustCargo,
    /// A package.json file was accepted during inspection.
    NpmTypeScript,
    /// Both Cargo.toml and package.json were accepted.
    Mixed,
    /// Neither a Cargo.toml nor a package.json was detected in accepted files.
    Unknown,
}

// ---------------------------------------------------------------------------
// ProjectSignal
// ---------------------------------------------------------------------------

/// A typed signal extracted from accepted files during inspection.
///
/// Signals are structural evidence (manifest presence, source file types)
/// rather than source code or secrets. They inform capability planning
/// without exposing project internals.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum ProjectSignal {
    /// A Cargo.toml file was accepted during inspection.
    CargoManifest,
    /// A package.json file was accepted during inspection.
    NpmManifest,
    /// One or more Rust source files (.rs) were accepted.
    RustSource {
        /// Number of Rust source files detected.
        count: u64,
    },
    /// One or more TypeScript source files (.ts, .tsx) were accepted.
    TypeScriptSource {
        /// Number of TypeScript source files detected.
        count: u64,
    },
}

// ---------------------------------------------------------------------------
// SourceLocation
// ---------------------------------------------------------------------------

/// The inspected location of a source file within a project.
///
/// `SourceLocation` stores a normalized relative path, an optional
/// line/column range, and a content hash — never the raw file bytes.
/// This keeps snapshots compact and prevents accidental exposure of
/// source text or environment secrets.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SourceLocation {
    /// Normalized relative path from the project root, using `/` separators.
    pub path: String,
    /// Optional starting line number (1-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Optional starting column number (1-based).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// Hex-encoded SHA-256 hash of the file content.
    pub content_hash: String,
}

// ---------------------------------------------------------------------------
// ProjectSnapshot
// ---------------------------------------------------------------------------

/// A read-only snapshot of a project's filesystem structure.
///
/// The snapshot records file locations and content hashes (never raw
/// source text), detected project kind, structural signals, and
/// resource-consumption counters. It is produced by the
/// [`ProjectInspector::inspect`] method.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProjectSnapshot {
    /// Deterministic SHA-256 hash over all inspected files.
    pub project_hash: String,
    /// The detected project kind from accepted manifest files.
    pub project_kind: ProjectKind,
    /// Typed structural signals found during traversal.
    pub signals: Vec<ProjectSignal>,
    /// Number of files **accepted** (i.e. fully read and within all limits).
    /// This is always equal to `files.len()`.
    pub file_count: u64,
    /// Total bytes read from inspected files (sum of accepted content lengths).
    pub total_bytes: u64,
    /// Snapshot completion state.
    pub state: SnapshotState,
    /// Non-fatal warnings accumulated during inspection.
    ///
    /// Warnings must never contain source text or environment secrets.
    pub warnings: Vec<String>,
    /// File locations with content hashes, sorted by normalized path.
    pub files: Vec<SourceLocation>,
}

// ---------------------------------------------------------------------------
// ProjectInspector trait
// ---------------------------------------------------------------------------

/// A read-only project inspector.
///
/// Implementations traverse a project root, detect structural signals
/// from accepted files, and produce a [`ProjectSnapshot`] with
/// deterministic hashing. Inspectors must be read-only — they must
/// never modify files, execute project code, or capture environment
/// secrets.
pub trait ProjectInspector {
    /// Inspects a project at the given root path within the supplied limits.
    ///
    /// Resource limits may cause the traversal to halt early. When this
    /// happens, the returned snapshot is still internally consistent
    /// (deterministic hash from the files read so far) but carries a
    /// [`SnapshotState::LimitExceeded`] state instead of
    /// [`SnapshotState::Complete`].
    ///
    /// # Errors
    ///
    /// Returns [`crate::DiscoveryError::InspectionFailure`] when the root
    /// cannot be read, canonicalized, or is not a directory.
    fn inspect(&self, root: &Path, limits: &InspectionLimits) -> DiscoveryResult<ProjectSnapshot>;
}
