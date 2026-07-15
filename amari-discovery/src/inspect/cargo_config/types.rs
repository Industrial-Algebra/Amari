// SPDX-License-Identifier: MIT OR Apache-2.0

//! Public types for Cargo platform configuration inspection (Task 8B2).
//!
//! Every public type is [`Serialize`]/[`Deserialize`]/[`Eq`] and documented.
//! Enums are exhaustive; collection types are deterministically sorted and
//! deduplicated by construction.
//!
//! # Safety guarantees encoded in these types
//!
//! - Raw runner commands, absolute paths, native search paths, and
//!   potentially secret flag values are **never** persisted. Executables
//!   are sanitized to basenames; opaque settings use counts/categories plus
//!   a SHA-256 identity.
//! - Target triples and cfg selectors are only retained after normalized
//!   validation; invalid identifiers produce a typed warning and are not
//!   stored.
//! - Every evidence item carries a source that resolves either to the
//!   accepted config input ([`ConfigSource`]) or to upstream Cargo/Rust
//!   provenance ([`crate::inspect::ManifestSource`] /
//!   [`crate::inspect::SourceLocation`]).

use serde::{Deserialize, Serialize};

use crate::inspect::snapshot::{InspectionLimit, SnapshotState, SourceLocation};
use crate::inspect::{DependencyKind, ManifestSource, SystemDependencyKind};

// ===========================================================================
// CargoPlatformInspection — top-level result
// ===========================================================================

/// The result of inspecting a Cargo project's platform configuration.
///
/// Composes existing [`crate::inspect::CargoInspection`] and
/// [`crate::inspect::RustSourceInspection`] evidence with bounded, read-only
/// inspection of the project-root `.cargo/config.toml`. The Cargo and Rust
/// inputs are never re-read; only the single config file is read.
///
/// # State semantics
///
/// [`state`](Self::state) is [`SnapshotState::Complete`] when the config was
/// read within resource limits (or was missing/malformed, which is not
/// fatal). It is [`crate::inspect::SnapshotState::LimitExceeded`] when a resource limit
/// prevented reading the config. In all cases the derived Cargo/Rust
/// evidence (benchmarks, `no_std`, target cfg constraints, native
/// requirements from manifests) is present and internally consistent.
///
/// # Empty config input hash
///
/// When the config is missing, unread, or a symlink,
/// [`config_input`](Self::config_input) has `file_count == 0`,
/// `total_bytes == 0`, and `input_hash` equal to the SHA-256 of the empty
/// framed input set.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CargoPlatformInspection {
    /// `[build]` table settings from `.cargo/config.toml`.
    pub build_settings: CargoBuildSettings,
    /// `[target.<key>]` table settings, deduplicated by key.
    pub target_settings: Vec<CargoTargetSettings>,
    /// Configured WASM targets (build target + target table keys beginning
    /// `wasm32-`), deduplicated by target.
    pub wasm_targets: Vec<WasmTargetEvidence>,
    /// Native/linker requirements from Cargo `links`/system deps and
    /// configured target linkers/native rustflags.
    pub native_requirements: Vec<NativeRequirement>,
    /// Benchmark evidence composed from Cargo `[[bench]]` declarations and
    /// Rust `benches/**/*.rs` classifications, package-scoped.
    pub benchmarks: Vec<BenchmarkEvidence>,
    /// `no_std` evidence derived only from literal `#![no_std]` crate
    /// attributes.
    pub no_std_evidence: NoStdEvidence,
    /// Target cfg constraints from Cargo target-specific dependency
    /// selectors and Rust platform cfg/cfg_attr predicates.
    pub target_cfg_constraints: Vec<TargetCfgConstraint>,
    /// Non-fatal warnings (never leak source/command/absolute/secret data).
    pub warnings: Vec<CargoPlatformWarning>,
    /// Provenance of the single config input.
    pub config_input: ConfigInputProvenance,
    /// Overall inspection state.
    pub state: SnapshotState,
}

// ===========================================================================
// CargoBuildSettings
// ===========================================================================

/// Sanitized settings parsed from the `[build]` table of `.cargo/config.toml`.
///
/// `target-dir` exposes only its presence (the value is an absolute or
/// project-relative path and is never persisted). `rustflags`/`rustdocflags`
/// expose only counts/categories and a SHA-256 identity. `target` holds only
/// validated target triples; custom target spec paths/JSON are represented as
/// opaque [`CustomTargetEvidence`] (count + identity, never the raw path).
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct CargoBuildSettings {
    /// `[build].target` as a sorted, deduplicated list of **validated** target
    /// triples (string or array form). Custom target spec paths/JSON are
    /// excluded and represented in [`custom_targets`](Self::custom_targets).
    pub target: Vec<String>,
    /// Opaque evidence for custom target spec entries (count + identity).
    pub custom_targets: CustomTargetEvidence,
    /// Whether `[build].target-dir` is set (value intentionally not exposed).
    pub target_dir_set: bool,
    /// `[build].incremental` value when present.
    pub incremental: Option<bool>,
    /// `[build].rustflags` sanitized evidence.
    pub rustflags: RustflagsEvidence,
    /// `[build].rustdocflags` sanitized evidence.
    pub rustdocflags: RustflagsEvidence,
    /// The accepted `.cargo/config.toml` source these settings were derived
    /// from. `None` when no config was accepted (missing/symlinked/limited);
    /// every derived evidence item resolves to this source when present.
    pub source: Option<ConfigSource>,
}

/// Opaque evidence for custom Cargo target spec entries (`.json` paths or
/// absolute/relative spec paths) found in `[build].target`.
///
/// Never persists the raw path, basename, or any content. `identity` is a
/// SHA-256 over length-framed entries (not reversible), distinguishing
/// distinct spec sets.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct CustomTargetEvidence {
    /// Number of custom target spec entries.
    pub count: usize,
    /// SHA-256 identity over the framed custom target spec entries (no raw path).
    pub identity: String,
}

impl Default for CustomTargetEvidence {
    fn default() -> Self {
        Self {
            count: 0,
            identity: super_empty_hash(),
        }
    }
}

/// Compute the SHA-256 of empty bytes (identity for an empty evidence set).
fn super_empty_hash() -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(b""))
}

impl Default for CargoBuildSettings {
    fn default() -> Self {
        Self {
            target: Vec::new(),
            custom_targets: CustomTargetEvidence::default(),
            target_dir_set: false,
            incremental: None,
            rustflags: RustflagsEvidence::empty(),
            rustdocflags: RustflagsEvidence::empty(),
            source: None,
        }
    }
}

// ===========================================================================
// CargoTargetSettings + CargoTargetKey
// ===========================================================================

/// Sanitized settings parsed from one `[target.<key>]` table.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct CargoTargetSettings {
    /// The normalized target table key.
    pub key: CargoTargetKey,
    /// `[target.<key>].rustflags` sanitized evidence.
    pub rustflags: RustflagsEvidence,
    /// `[target.<key>].rustdocflags` sanitized evidence.
    pub rustdocflags: RustflagsEvidence,
    /// `[target.<key>].linker` sanitized to basename + identity.
    pub linker: Option<ConfiguredLinker>,
    /// `[target.<key>].runner` sanitized to basename + count + identity.
    pub runner: Option<ConfiguredRunner>,
    /// Provenance within `.cargo/config.toml`.
    pub source: ConfigSource,
}

/// A normalized Cargo target table key.
///
/// Only safe identifiers are retained: a target triple (`arch-vendor-os` or
/// `arch-vendor-os-env`) or a `cfg(...)` expression. Invalid identifiers are
/// dropped with a typed warning.
///
/// For `cfg(...)` keys, quoted values are **redacted** in the `display` field
/// (never leaking secrets), while `identity` is a SHA-256 over the full
/// original predicate so distinct cfgs remain distinct and deterministic even
/// when their displayed form is identical.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum CargoTargetKey {
    /// A target triple such as `wasm32-unknown-unknown`.
    Triple {
        /// The validated triple.
        triple: String,
    },
    /// A `cfg(...)` expression such as `cfg(unix)`.
    Cfg {
        /// The cfg predicate with all quoted values redacted (e.g.
        /// `cfg(target_arch = <value>)`). Never leaks secret values.
        display: String,
        /// SHA-256 identity over the full original cfg predicate (framed, not
        /// reversible). Distinct cfgs with different secret values remain
        /// distinct even when `display` is identical.
        identity: String,
    },
}

// ===========================================================================
// RustflagsEvidence + RustflagCategory
// ===========================================================================

/// Sanitized evidence for a set of Cargo rustflags/rustdocflags.
///
/// Never persists raw flag values, absolute paths, native search paths, or
/// potentially secret values. Establishes deterministic identity via a
/// SHA-256 over the normalized token sequence.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct RustflagsEvidence {
    /// Number of flag tokens.
    pub flag_count: usize,
    /// Per-category flag counts, sorted by category.
    pub categories: Vec<RustflagCategoryCount>,
    /// Whether any flag is native/link-affecting.
    pub has_native_linking: bool,
    /// Number of native/link-affecting flag tokens.
    pub native_flag_count: usize,
    /// SHA-256 identity over the framed token sequence (not reversible).
    pub identity: String,
    /// SHA-256 identity over the native/link-affecting tokens only.
    pub native_identity: String,
}

impl RustflagsEvidence {
    /// Empty evidence (zero flags). Identity is the SHA-256 of empty input.
    pub fn empty() -> Self {
        use sha2::{Digest, Sha256};
        let empty = hex::encode(Sha256::digest(b""));
        Self {
            flag_count: 0,
            categories: Vec::new(),
            has_native_linking: false,
            native_flag_count: 0,
            identity: empty.clone(),
            native_identity: empty,
        }
    }
}

/// A category count for one rustflag category.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct RustflagCategoryCount {
    /// The flag category.
    pub category: RustflagCategory,
    /// Number of tokens in this category.
    pub count: usize,
}

/// A typed category for a single rustflag token.
///
/// Categorization is by flag prefix only — flag values are never inspected
/// or persisted. The set is exhaustive; every token maps to exactly one
/// category.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RustflagCategory {
    /// `-C link-arg`, `-Clink-arg`, `link-arg`, `link-args`.
    LinkArg,
    /// `-C linker`, `-Clinker`, `linker`.
    Linker,
    /// `-L`, `-C link-search-path`, `-Clink-search-path`, `link-search-path`.
    LinkSearch,
    /// `-l<lib>` (link a native library).
    LibraryLink,
    /// `-C target-feature`, `-Ctarget-feature`, `-C target-cpu`.
    TargetFeature,
    /// `--cfg`.
    Cfg,
    /// `-Z` (nightly/unstable).
    Unstable,
    /// `-C <other>` codegen option.
    Codegen,
    /// `-W` / `--warn` warning option.
    Warning,
    /// `--remap-path-prefix`.
    RemapPath,
    /// Any unrecognized token.
    Unknown,
}

impl RustflagCategory {
    /// Returns `true` when this category affects native linking.
    pub fn is_native_linking(self) -> bool {
        matches!(
            self,
            RustflagCategory::LinkArg
                | RustflagCategory::Linker
                | RustflagCategory::LinkSearch
                | RustflagCategory::LibraryLink
        )
    }
}

// ===========================================================================
// ConfiguredLinker / ConfiguredRunner
// ===========================================================================

/// A configured linker executable, sanitized to its basename.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ConfiguredLinker {
    /// Linker executable basename (no path, no args).
    pub basename: String,
    /// SHA-256 identity over the raw configured value (framed, not reversible).
    pub identity: String,
}

/// A configured runner command, sanitized.
///
/// The full runner command is never persisted. Only the executable basename,
/// total token count, and a SHA-256 identity are retained.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ConfiguredRunner {
    /// Runner executable basename (first token).
    pub executable_basename: String,
    /// Number of tokens in the runner command (string or array form).
    pub token_count: usize,
    /// SHA-256 identity over the raw configured value (framed, not reversible).
    pub identity: String,
}

// ===========================================================================
// WasmTargetEvidence + WasmTargetOrigin
// ===========================================================================

/// Evidence for one configured WASM target, with all origins.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WasmTargetEvidence {
    /// The WASM target triple (e.g. `wasm32-unknown-unknown`,
    /// `wasm64-unknown-unknown`).
    pub target: String,
    /// Sorted, deduplicated origins of this target.
    pub origins: Vec<WasmTargetOrigin>,
    /// Sorted, deduplicated direct [`ConfigSource`] provenance so every
    /// evidence item resolves directly to an accepted config input.
    pub sources: Vec<ConfigSource>,
}

/// The origin of a WASM target evidence item.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WasmTargetOrigin {
    /// From `[build].target`.
    BuildTarget,
    /// From a `[target.wasm32-...]` table key.
    TargetTable,
}

// ===========================================================================
// NativeRequirement + RustflagsScope
// ===========================================================================

/// A native/linker requirement derived from Cargo and config evidence.
///
/// Never claims execution or resolution of any native dependency; only
/// records declarative evidence.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum NativeRequirement {
    /// A `[package].links` native-library signal from a Cargo manifest.
    CargoLinks {
        /// The `links` key value.
        links_key: String,
        /// The package declaring `links`.
        package: String,
        /// Relative manifest path.
        manifest_path: String,
        /// Manifest provenance.
        source: ManifestSource,
    },
    /// A Cargo system dependency signal (pkg-config, cc, etc.).
    SystemDependency {
        /// Local alias used in the declaration.
        alias: String,
        /// Resolved package name.
        package: String,
        /// Classified system dependency category.
        system_kind: SystemDependencyKind,
        /// Cargo dependency table kind.
        dependency_kind: DependencyKind,
        /// Optional Cargo target selector context (`cfg(...)` or triple), if
        /// the dependency was declared under a `[target.<key>]` table.
        target: Option<String>,
        /// Manifest provenance.
        source: ManifestSource,
    },
    /// A configured target linker from `.cargo/config.toml`.
    ConfiguredLinker {
        /// The target table key.
        target_key: CargoTargetKey,
        /// Sanitized linker basename.
        basename: String,
        /// Config provenance.
        config: ConfigSource,
    },
    /// Native/link-affecting rustflags configured in `.cargo/config.toml`.
    NativeRustflags {
        /// The scope of the rustflags.
        scope: RustflagsScope,
        /// Number of native/link-affecting tokens.
        flag_count: usize,
        /// SHA-256 identity over the native tokens.
        identity: String,
        /// Config provenance.
        config: ConfigSource,
    },
}

/// The scope of a rustflags setting in `.cargo/config.toml`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum RustflagsScope {
    /// `[build].rustflags`.
    Build,
    /// `[target.<key>].rustflags`.
    Target {
        /// The target table key.
        key: CargoTargetKey,
    },
}

// ===========================================================================
// BenchmarkEvidence + BenchmarkStatus
// ===========================================================================

/// Composed benchmark evidence, package-scoped.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct BenchmarkEvidence {
    /// The package this benchmark belongs to.
    pub package: String,
    /// The benchmark name (from declaration, or filename stem for conventional).
    pub name: String,
    /// Project-relative path of the benchmark source.
    pub path: String,
    /// The composed status.
    pub status: BenchmarkStatus,
    /// Whether the bench uses the default libtest harness (`harness`, default
    /// `true`).
    pub harness: bool,
    /// Sorted, deduplicated `required-features` for the bench.
    pub required_features: Vec<String>,
    /// Source location of the bench file (when accepted), with content hash.
    pub source: Option<SourceLocation>,
    /// Manifest provenance of the `[[bench]]` declaration, when declared.
    pub declaration_source: Option<ManifestSource>,
}

/// The composed status of a benchmark.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum BenchmarkStatus {
    /// Declared in `[[bench]]` with a matching source file.
    DeclaredWithSource,
    /// Declared in `[[bench]]` but the source file is missing.
    DeclaredMissingSource {
        /// The declared (package-relative) path.
        declared_path: String,
    },
    /// A conventional `benches/*.rs` source with no `[[bench]]` declaration.
    ConventionalUndeclared,
}

// ===========================================================================
// NoStdEvidence
// ===========================================================================

/// `no_std` evidence derived only from literal `#![no_std]` crate attributes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NoStdEvidence {
    /// Whether any literal `#![no_std]` was found.
    pub has_no_std: bool,
    /// Packages with `#![no_std]` evidence.
    pub packages: Vec<NoStdPackageEvidence>,
}

/// `no_std` evidence for one package.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct NoStdPackageEvidence {
    /// The package name.
    pub package: String,
    /// Source locations of `#![no_std]` attributes (sorted, deduplicated).
    pub sources: Vec<SourceLocation>,
}

// ===========================================================================
// TargetCfgConstraint + TargetCfgSource
// ===========================================================================

/// A target/platform cfg constraint, with all sources.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TargetCfgConstraint {
    /// The normalized cfg predicate (e.g. `cfg(unix)`, `target_arch = "wasm32"`).
    pub predicate: String,
    /// Sorted, deduplicated sources of this constraint.
    pub sources: Vec<TargetCfgSource>,
}

/// The source of a target cfg constraint.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum TargetCfgSource {
    /// From a Cargo `[target.'cfg(...)']` dependency selector.
    CargoDependencySelector {
        /// Relative manifest path.
        manifest_path: String,
        /// The dependency alias.
        alias: String,
        /// The resolved package name.
        package_name: String,
        /// Manifest provenance.
        source: ManifestSource,
    },
    /// From a Rust `#[cfg(...)]` or `#[cfg_attr(...)]` attribute.
    RustAttribute {
        /// Relative path of the file.
        path: String,
        /// Whether this was a `cfg_attr`.
        is_cfg_attr: bool,
        /// Source location within the file.
        source: SourceLocation,
    },
}

// ===========================================================================
// ConfigInputProvenance + ConfigSource
// ===========================================================================

/// Provenance of the single `.cargo/config.toml` input.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ConfigInputProvenance {
    /// The config source, when the file was accepted (read into provenance).
    pub source: Option<ConfigSource>,
    /// Deterministic framed SHA-256 hash over accepted config bytes and path.
    pub input_hash: String,
    /// Number of config files inspected (always 0 or 1).
    pub file_count: u64,
    /// Total bytes of config content inspected.
    pub total_bytes: u64,
}

/// A source location within `.cargo/config.toml`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct ConfigSource {
    /// Relative path (always `.cargo/config.toml`).
    pub path: String,
    /// Optional 1-based line number (currently `None` for parsed entries).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub line: Option<usize>,
    /// SHA-256 hash of the accepted config content.
    pub content_hash: String,
    /// Byte count of the accepted config content.
    pub byte_count: u64,
}

// ===========================================================================
// CargoPlatformWarning
// ===========================================================================

/// Which Cargo config setting category a typed issue refers to.
///
/// Used by [`CargoPlatformWarning::InvalidConfigSetting`] to identify the
/// affected setting without ever exposing its raw value.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSetting {
    /// `[build].target`.
    BuildTarget,
    /// `[build].rustflags`.
    BuildRustflags,
    /// `[build].rustdocflags`.
    BuildRustdocflags,
    /// A `[target.<key>]` table key.
    TargetTableKey,
    /// `[target.<key>].rustflags`.
    TargetTableRustflags,
    /// `[target.<key>].rustdocflags`.
    TargetTableRustdocflags,
    /// `[target.<key>].linker`.
    TargetTableLinker,
    /// `[target.<key>].runner`.
    TargetTableRunner,
}

/// The kind of typed issue affecting a Cargo config setting.
///
/// Never carries the offending value — only a fixed issue category.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfigSettingIssue {
    /// The value has the wrong TOML type (e.g. a table where a string/array
    /// was expected).
    WrongType,
    /// The value is a syntactically invalid non-path, non-triple string.
    InvalidValue,
    /// An array mixes valid and invalid member types.
    MixedArray,
    /// The value is an empty string or empty array where a non-empty value
    /// is required.
    Empty,
}

/// Non-fatal warnings produced during Cargo platform inspection.
///
/// Warnings never contain TOML/source excerpts, command values, absolute
/// project roots, external symlink targets, or secrets.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum CargoPlatformWarning {
    /// `.cargo/config.toml` is missing (inspection is still `Complete`).
    MissingConfig {
        /// Expected relative path.
        path: String,
    },
    /// `.cargo/config.toml` exists but is not valid TOML.
    MalformedConfig {
        /// Relative path of the malformed file.
        path: String,
        /// Stable typed reason (never contains source snippets).
        reason: String,
        /// Optional 1-based line number.
        line: Option<usize>,
        /// Optional 1-based column number.
        column: Option<usize>,
        /// SHA-256 content hash of the malformed file.
        content_hash: String,
    },
    /// `.cargo/config.toml` is not valid UTF-8.
    InvalidUtf8Config {
        /// Relative path.
        path: String,
        /// SHA-256 content hash.
        content_hash: String,
        /// Byte count.
        byte_count: u64,
    },
    /// `.cargo` directory or `config.toml` is a symlink (never followed).
    SymlinkedConfig {
        /// Expected relative path.
        path: String,
    },
    /// A config target identifier failed normalized validation.
    InvalidTargetIdentifier {
        /// Relative path of the config.
        path: String,
        /// Stable typed reason (never contains the raw identifier).
        reason: String,
    },
    /// Two `[target.<key>]` tables normalized to the same target key but with
    /// conflicting settings (rustflags/linker/runner differ). The first
    /// occurrence wins deterministically; subsequent conflicting occurrences
    /// are dropped and reported here. `key_display` is the redacted display
    /// form (never a secret); `count` is the number of dropped conflicting
    /// occurrences.
    DuplicateTargetSetting {
        /// Redacted display form of the conflicting target key.
        key_display: String,
        /// Number of additional occurrences with conflicting settings.
        count: usize,
    },
    /// A Cargo config setting has a wrong type, mixed array, empty value, or
    /// invalid value. Identifies only a fixed setting/category and issue
    /// category — never the offending value.
    InvalidConfigSetting {
        /// Which Cargo config setting category is affected.
        setting: ConfigSetting,
        /// The kind of issue (never the value).
        issue: ConfigSettingIssue,
        /// Number of offending entries with this (setting, issue).
        count: usize,
    },
    /// A resource limit was exceeded during config inspection.
    LimitExceeded {
        /// The typed limit that was exceeded.
        limit: InspectionLimit,
    },
    /// `.cargo/config.toml` exists but is not a regular file (e.g. a
    /// directory, FIFO, or device node). Never followed; never exposes its
    /// contents. The path is the fixed `.cargo/config.toml` relative path.
    UnsupportedConfigFile {
        /// Relative path.
        path: String,
        /// Stable typed reason (never the raw type).
        reason: String,
    },
    /// Cooperative wall-clock enforcement observed elapsed time at or above
    /// the configured budget after a bounded phase (open/read, parse, or
    /// derivation). The inspection cooperatively stops further phases rather
    /// than interrupting mid-operation; phases already completed are
    /// reflected in the returned state.
    WallClock {
        /// The wall-clock budget in milliseconds.
        max_millis: u64,
        /// Observed elapsed milliseconds at the cooperative checkpoint.
        observed_millis: u64,
    },
}
