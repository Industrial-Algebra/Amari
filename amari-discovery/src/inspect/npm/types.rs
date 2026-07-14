// SPDX-License-Identifier: MIT OR Apache-2.0

//! Public domain types for offline npm package inspection.

use serde::{Deserialize, Serialize};

use crate::inspect::snapshot::{InspectionLimit, SnapshotState};
use crate::protocol::Compatibility;

/// Result of inspecting an npm package manifest and supported lockfile.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NpmInspection {
    /// Parsed root `package.json` evidence.
    pub package: NpmPackage,
    /// Parsed supported `package-lock.json` evidence, when available.
    pub lock: Option<NpmLock>,
    /// Embedded Amari catalog version used for compatibility checks.
    pub catalog_version: String,
    /// Non-fatal lockfile and resource warnings.
    pub warnings: Vec<NpmInspectionWarning>,
    /// Deterministic framed SHA-256 over accepted npm input files.
    pub input_hash: String,
    /// Whether optional lockfile evidence was complete or resource-limited.
    pub state: SnapshotState,
    /// Number of accepted npm files (`package.json` plus optional lockfile).
    pub inspected_file_count: u64,
    /// Total accepted npm input bytes.
    pub total_bytes: u64,
}

/// Parsed npm package metadata relevant to Amari discovery.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NpmPackage {
    /// Optional npm package name.
    pub name: Option<String>,
    /// Optional npm package version.
    pub version: Option<String>,
    /// Direct `@justinelliottcobb/amari-wasm` declarations.
    pub dependencies: Vec<NpmDependencyEvidence>,
    /// Provenance of `package.json`.
    pub source: NpmSource,
}

/// One direct Amari WASM dependency declaration.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NpmDependencyEvidence {
    /// Canonical npm package name.
    pub package_name: String,
    /// Manifest section containing the declaration.
    pub kind: NpmDependencyKind,
    /// Declared npm version requirement.
    pub declared_version: String,
    /// Exact version resolved by a supported package lock, when available.
    pub resolved_version: Option<String>,
    /// Compatibility with the embedded Amari catalog.
    pub compatibility: Compatibility,
    /// Provenance of the manifest declaration.
    pub manifest_source: NpmSource,
    /// Provenance of the resolved lock entry, when available.
    pub lock_source: Option<NpmSource>,
}

/// Supported npm dependency manifest sections.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NpmDependencyKind {
    /// `dependencies`.
    Production,
    /// `devDependencies`.
    Development,
    /// `optionalDependencies`.
    Optional,
    /// `peerDependencies`.
    Peer,
}

/// Parsed supported npm lockfile evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NpmLock {
    /// npm lock schema version (supported: 2 and 3).
    pub lockfile_version: u64,
    /// Relevant resolved package entries.
    pub packages: Vec<NpmLockedPackage>,
    /// Provenance of `package-lock.json`.
    pub source: NpmSource,
}

/// Exact package resolution from `package-lock.json`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct NpmLockedPackage {
    /// Canonical npm package name.
    pub package_name: String,
    /// Exact resolved version.
    pub version: String,
    /// Lockfile provenance.
    pub source: NpmSource,
}

/// Content-addressed npm input provenance.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct NpmSource {
    /// Fixed project-relative input path.
    pub path: String,
    /// Optional 1-based source line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u64>,
    /// SHA-256 of the complete accepted file bytes.
    pub content_hash: String,
    /// Accepted file byte count.
    pub byte_count: u64,
}

/// Non-fatal npm inspection warning.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum NpmInspectionWarning {
    /// `package-lock.json` was absent.
    MissingLock {
        /// Fixed relative path.
        path: String,
    },
    /// `package-lock.json` was malformed JSON.
    MalformedLock {
        /// Fixed relative path.
        path: String,
        /// Sanitized error category.
        reason: String,
        /// Optional 1-based parser line.
        line: Option<u64>,
        /// Optional 1-based parser column.
        column: Option<u64>,
        /// Content hash of the malformed lockfile.
        content_hash: String,
    },
    /// Lockfile bytes were not valid UTF-8/JSON text.
    InvalidUtf8Lock {
        /// Fixed relative path.
        path: String,
        /// Content hash of the malformed lockfile.
        content_hash: String,
    },
    /// The lockfile schema is outside the supported npm v2/v3 set.
    UnsupportedLockfileVersion {
        /// Unsupported numeric schema version.
        version: u64,
    },
    /// A lockfile symlink or replacement race was rejected.
    SymlinkedLock {
        /// Fixed relative path.
        path: String,
    },
    /// The lock path exists but is not a regular file.
    UnsupportedLockFile {
        /// Fixed relative path.
        path: String,
    },
    /// The optional lockfile could not be opened or read safely.
    ReadFailure {
        /// Fixed relative path.
        path: String,
    },
    /// Conflicting exact target versions were present in the lockfile.
    AmbiguousLockResolution {
        /// Canonical npm package name.
        package_name: String,
        /// Number of distinct exact versions found.
        version_count: usize,
    },
    /// An optional lockfile resource limit was reached.
    LimitExceeded {
        /// Typed bounded-resource reason.
        limit: InspectionLimit,
    },
}
