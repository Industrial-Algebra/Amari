// SPDX-License-Identifier: MIT OR Apache-2.0

//! Public types for Cargo dependency inspection.
//!
//! All types in this module are re-exported through the parent `cargo`
//! module and the crate root.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::inspect::snapshot::{InspectionLimit, SnapshotState};
use crate::protocol::Compatibility;

// ============================================================================
// CargoInspection
// ============================================================================

/// The result of inspecting a Cargo project for Amari dependencies.
///
/// # State semantics
///
/// On `Ok`, the root manifest is **always** fully parsed and present in
/// [`root_package`](Self::root_package). The root is never absent — if the
/// root manifest is missing, unreadable, or malformed, this function returns
/// `Err`, not an inspection with a partial root.
///
/// [`state`](Self::state) signals whether **optional** evidence is partial:
///
/// - [`SnapshotState::Complete`] — all requested manifests and the lockfile
///   (when present on disk) were parsed within resource limits.
/// - [`SnapshotState::LimitExceeded`] — one or more workspace members or
///   the lockfile could not be read because a resource limit was hit. The
///   root manifest and any already-parsed members are still present and
///   internally consistent.
///
/// # Empty input hash
///
/// When no files are accepted (e.g. an otherwise-empty project directory),
/// [`input_hash`](Self::input_hash) is the SHA-256 of the empty framed
/// input set — i.e. `SHA-256("")`. This is deterministic: two empty
/// inspections always produce the same hash. Framing uses relative paths
/// and content bytes only (no absolute paths, timestamps, or inodes), so
/// the hash is root-independent.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CargoInspection {
    /// The root package (where the root `Cargo.toml` lives).
    pub root_package: CargoPackage,
    /// Additional workspace member packages beyond the root.
    pub workspace_members: Vec<CargoPackage>,
    /// The parsed `Cargo.lock` contents alongside the root, if present.
    pub lock: Option<CargoLock>,
    /// Optional workspace-level metadata.
    pub workspace_meta: Option<WorkspaceMeta>,
    /// Non-fatal warnings accumulated during inspection.
    pub warnings: Vec<CargoInspectionWarning>,
    /// Deterministic framed SHA-256 hash over accepted manifest/lock
    /// bytes and their relative paths.
    pub input_hash: String,
    /// Overall inspection state.
    ///
    /// [`SnapshotState::Complete`] when all evidence was collected within
    /// limits; [`SnapshotState::LimitExceeded`] when optional lock or
    /// member evidence was truncated by a resource limit. The root manifest
    /// is always complete on `Ok`.
    pub state: SnapshotState,
    /// Number of Cargo files inspected (manifests + lock).
    pub inspected_file_count: u64,
    /// Total bytes of Cargo file content inspected.
    pub total_bytes: u64,
}

// ============================================================================
// CargoPackage
// ============================================================================

/// A single Cargo package (root or member) with its Amari dependency evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CargoPackage {
    /// Cargo package name from `[package].name`.
    pub name: String,
    /// Cargo package version from `[package].version`.
    pub version: String,
    /// Relative path to the package's `Cargo.toml` from the project root.
    pub manifest_path: String,
    /// Evidence for each direct Amari dependency (`amari` or `amari-*`).
    pub dependencies: Vec<AmariDependencyEvidence>,
    /// `[[bench]]` target declarations from the manifest.
    pub benches: Vec<CargoBench>,
    /// `[package].links` native-link signal, if present.
    pub native_link: Option<NativeLink>,
    /// Package metadata fields inherited from `[workspace.package]`.
    pub inherited_metadata: Vec<String>,
    /// System dependency signals detected in the manifest.
    pub system_dependencies: Vec<SystemDependencySignal>,
    /// Lightweight records of **every** dependency (Amari and non-Amari) across
    /// normal/dev/build tables, with optional target selectors. Used for
    /// platform-derivation of target cfg constraints.
    pub dependency_records: Vec<CargoDependencyRecord>,
    /// `[package].autobenches` value when present. When `Some(false)`,
    /// conventional `benches/*` discovery is suppressed (only explicit
    /// `[[bench]]` targets are considered).
    pub autobenches: Option<bool>,
}

// ============================================================================
// CargoDependencyRecord — lightweight record of every dependency (Task 8C)
// ============================================================================

/// A lightweight record of a single Cargo dependency declaration, retaining
/// only its identity, table kind, optional target selector, and provenance
/// (no version or feature data).
///
/// Unlike [`AmariDependencyEvidence`], this records **every** dependency
/// (Amari and non-Amari) across normal/dev/build tables so that platform
/// derivation can see target-specific selectors for all dependencies.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CargoDependencyRecord {
    /// The local alias used in the dependency declaration.
    pub alias: String,
    /// The resolved Cargo package name (after `package = "..."` renaming).
    pub package: String,
    /// Which dependency table this came from: `normal`, `dev`, or `build`.
    pub kind: DependencyKind,
    /// Optional Cargo target selector (`cfg(...)` or triple) when declared
    /// under a `[target.<key>]` table.
    pub target: Option<String>,
    /// Location of this declaration in the manifest.
    pub source: ManifestSource,
}

// ============================================================================
// AmariDependencyEvidence
// ============================================================================

/// Typed evidence for one direct Amari dependency.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AmariDependencyEvidence {
    /// The local alias used in Rust source (the dependency key).
    pub alias: String,
    /// The actual Cargo package name after `package = "..."` renaming.
    pub package_name: String,
    /// Which dependency table this came from: `normal`, `dev`, or `build`.
    pub kind: DependencyKind,
    /// The declared version requirement string (e.g. `"0.23.0"`, `"^0.23"`).
    pub declared_version: String,
    /// The exact version resolved from `Cargo.lock`, when available.
    pub resolved_version: Option<String>,
    /// Compatibility of the resolved version against the embedded catalog.
    pub compatibility: Compatibility,
    /// Optional target selector from `[target.'cfg(...)'.dependencies]`.
    pub target: Option<String>,
    /// Explicit feature flags requested for this dependency.
    pub features: Vec<String>,
    /// Whether Cargo default features are enabled.
    pub default_features: bool,
    /// Whether the dependency is optional.
    pub optional: bool,
    /// Location of this dependency declaration in the manifest.
    pub manifest_source: ManifestSource,
    /// Location of the resolved entry in `Cargo.lock`, when available.
    pub lock_source: Option<ManifestSource>,
}

// ============================================================================
// DependencyKind
// ============================================================================

/// Which Cargo dependency table a dependency was declared in.
///
/// No public ordering is committed — sort order may change between releases.
/// Internal sorting uses a private rank rather than derived `Ord`.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    /// From `[dependencies]`.
    Normal,
    /// From `[dev-dependencies]`.
    Dev,
    /// From `[build-dependencies]`.
    Build,
}

impl DependencyKind {
    /// Private rank for deterministic sorting (Normal < Dev < Build).
    pub(super) fn rank(self) -> u8 {
        match self {
            DependencyKind::Normal => 0,
            DependencyKind::Dev => 1,
            DependencyKind::Build => 2,
        }
    }
}

// ============================================================================
// CargoBench
// ============================================================================

/// A `[[bench]]` target declaration in a manifest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CargoBench {
    /// The benchmark name.
    pub name: String,
    /// The benchmark source path relative to the package, normalized and
    /// validated (never absolute or parent-traversing).
    pub path: String,
    /// Whether the bench uses the default libtest harness (`harness`, default
    /// `true`). `false` indicates a custom `fn main`.
    pub harness: bool,
    /// Sorted, deduplicated `required-features` for the bench.
    pub required_features: Vec<String>,
    /// Location of this bench declaration in the manifest.
    pub manifest_source: ManifestSource,
}

// ============================================================================
// NativeLink
// ============================================================================

/// A `[package].links` native-library link signal.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NativeLink {
    /// The value of the `links` key.
    pub links_key: String,
    /// Location of this signal in the manifest.
    pub manifest_source: ManifestSource,
}

// ============================================================================
// SystemDependencyKind + SystemDependencySignal
// ============================================================================

/// A known declarative system dependency kind.
///
/// Captures the build-tool category of a system dependency detected in a
/// manifest without executing build scripts.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SystemDependencyKind {
    /// `pkg-config` or `pkg_config` crate.
    PkgConfig,
    /// `cc` crate.
    Cc,
    /// `bindgen` crate.
    Bindgen,
    /// `cmake` crate.
    Cmake,
    /// `vcpkg` crate.
    Vcpkg,
    /// `system-deps` crate.
    SystemDeps,
}

/// A declarative system dependency signal detected in the manifest.
///
/// Captures known native tool deps (pkg-config, cc, bindgen, cmake, etc.)
/// without executing build scripts or inferring resolution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct SystemDependencySignal {
    /// The local alias used in the dependency declaration.
    pub alias: String,
    /// The resolved Cargo package name (after `package = "..."`).
    pub package: String,
    /// Which Cargo dependency table this came from.
    pub dependency_kind: DependencyKind,
    /// The classified system dependency category.
    pub system_kind: SystemDependencyKind,
    /// Optional target selector.
    pub target: Option<String>,
    /// Location of this signal in the manifest.
    pub manifest_source: ManifestSource,
}

/// Known system dependency package names and their kinds.
pub(super) const SYSTEM_DEP_KINDS: &[(&str, SystemDependencyKind)] = &[
    ("pkg-config", SystemDependencyKind::PkgConfig),
    ("pkg_config", SystemDependencyKind::PkgConfig),
    ("cc", SystemDependencyKind::Cc),
    ("cmake", SystemDependencyKind::Cmake),
    ("bindgen", SystemDependencyKind::Bindgen),
    ("vcpkg", SystemDependencyKind::Vcpkg),
    ("system-deps", SystemDependencyKind::SystemDeps),
];

/// Returns `Some(kind)` if `name` is a known system dependency package.
pub(super) fn system_dep_kind(name: &str) -> Option<SystemDependencyKind> {
    SYSTEM_DEP_KINDS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, k)| *k)
}

// ============================================================================
// WorkspaceMeta + WorkspaceDependencyBase
// ============================================================================

/// Optional workspace-level metadata parsed from the root manifest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceMeta {
    /// Explicit `[workspace].members` entries.
    pub members: Vec<String>,
    /// Base dependency declarations from `[workspace.dependencies]`.
    pub dependency_bases: BTreeMap<String, WorkspaceDependencyBase>,
    /// Fields from `[workspace.package]`.
    pub package_fields: BTreeMap<String, String>,
}

/// A base dependency declaration in `[workspace.dependencies]`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WorkspaceDependencyBase {
    /// The actual package name, when `package = "..."` is set.
    pub package: Option<String>,
    /// Base version requirement, if declared.
    pub version: Option<String>,
    /// Base path, if declared.
    pub path: Option<String>,
    /// Base git repository, if declared.
    pub git: Option<String>,
    /// Base features, if declared.
    pub features: Vec<String>,
    /// Base `default-features` override, if declared.
    pub default_features: Option<bool>,
    /// Whether the workspace base is optional.
    pub optional: bool,
}

// ============================================================================
// CargoLock + LockedPackage
// ============================================================================

/// The parsed contents of a `Cargo.lock` file.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CargoLock {
    /// Relative path to the lockfile.
    pub path: String,
    /// Locked packages in declaration order.
    pub packages: Vec<LockedPackage>,
    /// Source location of the lockfile.
    pub source: ManifestSource,
}

/// A single `[[package]]` entry in `Cargo.lock`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LockedPackage {
    /// Package name.
    pub name: String,
    /// Exact locked version.
    pub version: String,
    /// SHA-256 checksum, when the package has a registry source.
    pub checksum: Option<String>,
    /// Package source (e.g. `registry+https://...`).
    pub source: Option<String>,
}

// ============================================================================
// ManifestSource
// ============================================================================

/// A source location within a manifest or lockfile, with content provenance.
///
/// # Line numbers
///
/// `line` is `None` for syntax-tree records produced from successfully-parsed
/// TOML entries (dependency declarations, bench targets, native-link signals,
/// system dependency signals, lockfile entries). These records are extracted
/// from the already-parsed TOML value tree, which discards span information.
///
/// Line numbers are only populated when a TOML parse error provides a known
/// error position via [`toml::de::Error::span`]. Extracting line numbers for
/// successful syntax-tree entries requires adopting a span-preserving TOML
/// parser, which is not yet integrated.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ManifestSource {
    /// Relative path to the file.
    pub path: String,
    /// Optional line number (1-based) of the relevant entry.
    pub line: Option<usize>,
    /// SHA-256 hash of the file content.
    pub content_hash: String,
    /// Byte count of the file content.
    pub byte_count: u64,
}

// ============================================================================
// CargoInspectionWarning
// ============================================================================

/// Non-fatal warnings produced during Cargo project inspection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum CargoInspectionWarning {
    /// A manifest was expected but not found on disk.
    MissingManifest {
        /// Relative path that was expected.
        path: String,
    },
    /// No `Cargo.lock` was found alongside the root manifest.
    MissingLock {
        /// Relative path that was expected.
        path: String,
    },
    /// A manifest file exists but is not valid TOML.
    MalformedManifest {
        /// Relative path of the malformed file.
        path: String,
        /// Stable typed reason (never contains source snippets).
        reason: String,
        /// Optional 1-based line number of the TOML error.
        line: Option<usize>,
        /// Optional 1-based column number of the TOML error.
        column: Option<usize>,
    },
    /// A lockfile exists but is not valid TOML or has an unsupported format.
    MalformedLock {
        /// Relative path of the malformed file.
        path: String,
        /// Stable typed reason (never contains source snippets).
        reason: String,
        /// Optional 1-based line number of the TOML error.
        line: Option<usize>,
        /// Optional 1-based column number of the TOML error.
        column: Option<usize>,
    },
    /// A manifest path is a symlink (never followed).
    SymlinkedManifest {
        /// Relative path of the symlinked manifest.
        path: String,
    },
    /// A manifest resolved to a path outside the project root.
    EscapingManifest {
        /// Relative path of the escaped manifest.
        path: String,
    },
    /// A Cargo.lock contains multiple entries with the same package name
    /// at different versions, making exact resolution ambiguous.
    AmbiguousLockResolution {
        /// The package name with ambiguous versions.
        package: String,
        /// The conflicting versions found (sorted, deduplicated).
        versions: Vec<String>,
    },
    /// A member dependency uses `workspace = true` but the referenced
    /// dependency alias is not defined in `[workspace.dependencies]`.
    InheritedBaseMissing {
        /// The member package name.
        member: String,
        /// The dependency alias that is missing a base.
        dep: String,
    },
    /// A dependency requirement cannot be resolved offline (e.g. git
    /// or path dependency that isn't a workspace base).
    UnsupportedRequirement {
        /// The package that declares the dependency.
        package: String,
        /// The dependency alias with the unsupported requirement.
        dep: String,
        /// Human-readable reason (never contains source snippets).
        reason: String,
    },
    /// A resource limit was exceeded during inspection.
    LimitExceeded {
        /// The typed limit that was exceeded.
        limit: InspectionLimit,
    },
    /// A workspace member path uses illegal patterns (glob, abs, parent
    /// components, or is empty).
    IllegalMemberPath {
        /// Fixed diagnostic category; never the raw manifest value.
        member: String,
    },
    /// A listed member declares another workspace root and is omitted.
    NestedWorkspaceRoot {
        /// Normalized relative member manifest path.
        path: String,
    },
    /// An inherited workspace field (`workspace = true`) references a
    /// field that does not exist or is not `true` in `[workspace.package]`.
    WorkspaceFieldNotFound {
        /// The field name that could not be resolved.
        field: String,
        /// The package that requested inheritance.
        package: String,
    },
    /// A `workspace = false` value was encountered (illegal).
    WorkspaceFalseRejected {
        /// The dependency alias.
        dep: String,
        /// The package that declared it.
        package: String,
    },
    /// An illegal override on a workspace-inherited dependency (e.g.
    /// supplying a local version alongside `workspace = true`).
    WorkspaceOverrideRejected {
        /// The dependency alias.
        dep: String,
        /// The package that declared it.
        package: String,
        /// The overridden key.
        key: String,
    },
    /// A `[[bench]]` declaration has an invalid source path (absolute, parent
    /// traversal, or non-normalized). The bench is omitted conservatively and
    /// the raw path is never serialized.
    InvalidBenchPath {
        /// The bench name (safe identifier, not the path).
        bench_name: String,
        /// The package that declared it.
        package: String,
    },
}
