// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded, read-only Cargo platform configuration inspection (Task 8B2).
//!
//! This module composes existing [`CargoInspection`] and
//! [`RustSourceInspection`] evidence with bounded inspection of the
//! project-root `.cargo/config.toml`, producing a deterministic
//! [`CargoPlatformInspection`].
//!
//! # Safety
//!
//! - Pure / read-only: never invokes Cargo, rustc, build scripts, runners,
//!   linkers, shell, network, or providers.
//! - Only `.cargo/config.toml` is authoritative in 0.24; user/global Cargo
//!   config and legacy `.cargo/config` are never inspected.
//! - Symlinked `.cargo` directory or config file is never followed. The
//!   reader opens the root as a directory and descends via `openat` (Unix),
//!   opening `.cargo` then `config.toml` relative to held directory file
//!   descriptors with `O_NOFOLLOW`. Because each component is opened
//!   relative to a *held* parent descriptor, a `.cargo` name swapped between
//!   a metadata check and the open can never be followed, and no canonical
//!   root containment check is required (the openat descent is inherently
//!   contained). Other targets use a conservative static reparse/symlink
//!   check. Root, canonicalization, and open errors are sanitized and never
//!   embed absolute project paths, the root value, or OS strings.
//! - All five [`InspectionLimits`] apply to the single config input.
//! - No `read_to_string`, arbitrary `read_to_end`, lossy paths, or
//!   `unwrap`/`expect`/`panic` in library code.
//! - Warnings never contain TOML/source excerpts, command values, absolute
//!   project roots, external symlink targets, or secrets. Executables are
//!   sanitized to basenames; opaque settings use counts/categories plus a
//!   SHA-256 identity.
//!
//! # Module structure
//!
//! - [`types`] — all public domain types re-exported at this module's root.
//! - `config` — bounded reader + offline TOML parser (not public).
//! - `derive` — evidence derivation from Cargo/Rust/config (not public).
//!
//! [`CargoInspection`]: crate::inspect::CargoInspection
//! [`RustSourceInspection`]: crate::inspect::RustSourceInspection

pub mod types;

mod config;
mod derive;

pub use types::{
    BenchmarkEvidence, BenchmarkStatus, CargoBuildSettings, CargoPlatformInspection,
    CargoPlatformWarning, CargoTargetKey, CargoTargetSettings, ConfigInputProvenance,
    ConfigSetting, ConfigSettingIssue, ConfigSource, ConfiguredLinker, ConfiguredRunner,
    CustomTargetEvidence, NativeRequirement, NoStdEvidence, NoStdPackageEvidence, RustflagCategory,
    RustflagCategoryCount, RustflagsEvidence, RustflagsScope, TargetCfgConstraint, TargetCfgSource,
    WasmTargetEvidence, WasmTargetOrigin,
};

use std::path::Path;

use crate::error::DiscoveryResult;
use crate::inspect::{CargoInspection, InspectionLimits, RustSourceInspection};

use self::config::{read_config, AcceptedConfig};
use self::derive::{
    derive_benchmarks, derive_native_requirements, derive_no_std, derive_target_cfg,
    derive_wasm_targets,
};

/// Inspect a Cargo project's platform configuration without invoking Cargo,
/// rustc, build scripts, runners, linkers, or the network.
///
/// This function composes the already-computed `cargo` and `rust` evidence
/// with a bounded, read-only read of the project-root `.cargo/config.toml`.
/// Manifests and Rust source files are never re-read.
///
/// # Missing / malformed / symlinked config
///
/// A missing `.cargo/config.toml` is a successful [`crate::inspect::SnapshotState::Complete`]
/// result with a [`CargoPlatformWarning::MissingConfig`] warning and empty
/// config provenance, while still returning the derived Cargo/Rust platform
/// evidence. Malformed or invalid-UTF-8 config is accepted into provenance
/// (contributing count/bytes/hash) and reported as a sanitized typed warning.
/// A symlinked `.cargo` directory or config file is never followed.
///
/// # Limits
///
/// All five [`InspectionLimits`] apply to the config input:
/// `max_inspection_files == 0`, insufficient `max_traversal_depth`,
/// per-file/aggregate byte limits, and the wall-clock limit all produce a
/// [`crate::inspect::SnapshotState::LimitExceeded`] state without reading (or fully reading)
/// the config. Derived Cargo/Rust evidence is still present in every case.
///
/// # Errors
///
/// Returns [`crate::DiscoveryError::InspectionFailure`] only when the project
/// root is not a directory or cannot be canonicalized, or on an unrecoverable
/// I/O error opening the config.
///
/// ```no_run
/// use amari_discovery::inspect::{
///     inspect_cargo_platform, inspect_cargo_project, inspect_rust_sources, InspectionLimits,
/// };
/// use std::path::Path;
///
/// let root = Path::new(env!("CARGO_MANIFEST_DIR"));
/// let limits = InspectionLimits::default();
/// let cargo = inspect_cargo_project(root, &limits).unwrap();
/// let rust = inspect_rust_sources(root, &cargo, &limits).unwrap();
/// let platform = inspect_cargo_platform(root, &cargo, &rust, &limits).unwrap();
/// assert!(!platform.config_input.input_hash.is_empty());
/// ```
pub fn inspect_cargo_platform(
    root: &Path,
    cargo: &CargoInspection,
    rust: &RustSourceInspection,
    limits: &InspectionLimits,
) -> DiscoveryResult<CargoPlatformInspection> {
    if !root.is_dir() {
        return Err(crate::error::DiscoveryError::InspectionFailure(
            "project root is not a directory".to_string(),
        ));
    }

    // ---- Bounded read of .cargo/config.toml ----
    let config_read = read_config(root, limits)?;

    let (build_settings, target_settings, mut warnings) = match &config_read.accepted {
        Some(AcceptedConfig { source, bytes }) => {
            let parsed = config::parse_config(bytes, source);
            (
                parsed.build_settings,
                parsed.target_settings,
                parsed.warnings,
            )
        }
        None => (types::CargoBuildSettings::default(), Vec::new(), Vec::new()),
    };

    // Merge config-read warnings (missing/symlink/limit) with parse warnings.
    warnings.extend(config_read.warnings);
    sort_dedup_warnings(&mut warnings);

    // ---- Derive composed evidence ----
    let wasm_targets = derive_wasm_targets(&build_settings, &target_settings);
    let native_requirements = derive_native_requirements(cargo, &target_settings, &build_settings);
    let benchmarks = derive_benchmarks(cargo, rust);
    let no_std_evidence = derive_no_std(cargo, rust);
    let target_cfg_constraints = derive_target_cfg(cargo, rust);

    // ---- Build provenance ----
    let config_input = ConfigInputProvenance {
        source: config_read.accepted.as_ref().map(|a| a.source.clone()),
        input_hash: config_read.input_hash,
        file_count: config_read.file_count,
        total_bytes: config_read.total_bytes,
    };

    Ok(CargoPlatformInspection {
        build_settings,
        target_settings,
        wasm_targets,
        native_requirements,
        benchmarks,
        no_std_evidence,
        target_cfg_constraints,
        warnings,
        config_input,
        state: config_read.state,
    })
}

/// Deterministically sort and deduplicate warnings by a total typed key.
///
/// [`CargoPlatformWarning`] cannot derive `Ord` (it embeds
/// [`crate::inspect::InspectionLimit`]); this helper preserves a stable,
/// deterministic order by a typed sort key, not by insertion order or
/// `Debug` formatting. Multiple distinct `(setting, issue)` issues are never
/// collapsed.
fn sort_dedup_warnings(warnings: &mut Vec<CargoPlatformWarning>) {
    warnings.sort_by_key(warning_sort_key);
    warnings.dedup();
}

/// A fully deterministic comparable sort key for a warning.
///
/// The first element is the variant index (stable enum definition order).
/// Subsequent elements disambiguate within a variant using typed fields
/// (never `Debug` strings).
fn warning_sort_key(w: &CargoPlatformWarning) -> (u8, u8, u8, usize, String) {
    match w {
        CargoPlatformWarning::MissingConfig { path } => (0, 0, 0, 0, path.clone()),
        CargoPlatformWarning::MalformedConfig { path, .. } => (1, 0, 0, 0, path.clone()),
        CargoPlatformWarning::InvalidUtf8Config { path, .. } => (2, 0, 0, 0, path.clone()),
        CargoPlatformWarning::SymlinkedConfig { path } => (3, 0, 0, 0, path.clone()),
        CargoPlatformWarning::InvalidTargetIdentifier { path, .. } => (4, 0, 0, 0, path.clone()),
        CargoPlatformWarning::InvalidConfigSetting {
            setting,
            issue,
            count,
        } => (5, *setting as u8, *issue as u8, *count, String::new()),
        CargoPlatformWarning::LimitExceeded { .. } => (6, 0, 0, 0, String::new()),
    }
}
