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
use crate::inspect::snapshot::{InspectionLimit, SnapshotState};
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
/// # Wall-clock enforcement
///
/// The wall-clock budget is enforced **cooperatively** at four points — the
/// entry zero gate (budget `== 0` returns before any read), and after each of
/// the open/read, parse, and derivation phases — using an internal
/// deterministic checkpoint helper. Enforcement never interrupts an in-flight
/// operation: each phase runs to completion and the next phase is skipped when
/// the elapsed time has reached the budget. The returned state and
/// [`CargoPlatformWarning::WallClock`] carry the actual observed elapsed and
/// the configured budget; accepted provenance and completed-phase evidence
/// remain present and internally consistent.
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
    let start = std::time::Instant::now();
    inspect_cargo_platform_with_elapsed(root, cargo, rust, limits, move || start.elapsed())
}

/// Cooperative wall-clock-gated implementation of [`inspect_cargo_platform`],
/// accepting an injectable `elapsed` closure so the post-phase checkpoints can
/// be unit-tested with artificial [`std::time::Duration`]s (no flaky sleeps).
///
/// The closure is consulted after open/read, after parse, and after
/// derivation; the entry zero gate (budget `== 0`) is enforced inside the
/// bounded config reader using a real [`std::time::Instant`].
pub fn inspect_cargo_platform_with_elapsed(
    root: &Path,
    cargo: &CargoInspection,
    rust: &RustSourceInspection,
    limits: &InspectionLimits,
    elapsed: impl Fn() -> std::time::Duration,
) -> DiscoveryResult<CargoPlatformInspection> {
    if !root.is_dir() {
        return Err(crate::error::DiscoveryError::InspectionFailure(
            "project root is not a directory".to_string(),
        ));
    }

    let budget = limits.max_inspection_wall_millis;

    // ---- Bounded read of .cargo/config.toml (open + read + accept) ----
    // `read_config` enforces the entry **zero gate** (budget == 0) before any
    // read, returning a `LimitExceeded(WallClock)` state with observed 0.
    let config_read = read_config(root, limits)?;

    // Cooperative wall-clock check AFTER open/read. A nonzero elapsed at or
    // over budget skips the parse phase: accepted provenance still reflects
    // the bytes actually read (file_count/total_bytes/input_hash), build
    // settings are empty (parse did not run), and only the cargo/rust-derived
    // evidence (benchmarks, no_std, target cfg) plus empty config-derived
    // evidence is present.
    if let Some((max, observed)) = config::wall_clock_exceeded(elapsed(), budget) {
        let empty_build = types::CargoBuildSettings::default();
        let empty_targets: Vec<types::CargoTargetSettings> = Vec::new();
        let wc = WallClockObs {
            max_millis: max,
            observed_millis: observed,
        };
        return Ok(assemble(
            cargo,
            rust,
            &config_read,
            empty_build,
            empty_targets,
            derive_benchmarks(cargo, rust),
            &wc,
        ));
    }

    let (build_settings, target_settings, parse_warnings) = match &config_read.accepted {
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

    // Cooperative wall-clock check AFTER parse. Parsed settings are retained
    // and all derived evidence is computed.
    if let Some((max, observed)) = config::wall_clock_exceeded(elapsed(), budget) {
        let wc = WallClockObs {
            max_millis: max,
            observed_millis: observed,
        };
        return Ok(assemble_with_wall_clock_after_parse(
            cargo,
            rust,
            &config_read,
            build_settings,
            target_settings,
            parse_warnings,
            &wc,
        ));
    }

    // Merge config-read warnings (missing/symlink/limit) with parse warnings.
    let mut warnings = parse_warnings;
    warnings.extend(config_read.warnings.clone());
    sort_dedup_warnings(&mut warnings);

    // ---- Derive composed evidence ----
    let wasm_targets = derive_wasm_targets(&build_settings, &target_settings);
    let native_requirements = derive_native_requirements(cargo, &target_settings, &build_settings);
    let benchmarks = derive_benchmarks(cargo, rust);
    let no_std_evidence = derive_no_std(cargo, rust);
    let target_cfg_constraints = derive_target_cfg(cargo, rust);

    // Cooperative wall-clock check AFTER derivation. All evidence is present;
    // the state records the cooperative stop and a WallClock warning is added.
    if let Some((max, observed)) = config::wall_clock_exceeded(elapsed(), budget) {
        warnings.push(CargoPlatformWarning::WallClock {
            max_millis: max,
            observed_millis: observed,
        });
        sort_dedup_warnings(&mut warnings);
        let config_input = build_config_input(&config_read);
        return Ok(CargoPlatformInspection {
            build_settings,
            target_settings,
            wasm_targets,
            native_requirements,
            benchmarks,
            no_std_evidence,
            target_cfg_constraints,
            warnings,
            config_input,
            state: SnapshotState::LimitExceeded {
                limit: InspectionLimit::WallClock {
                    max_millis: max,
                    observed_millis: observed,
                },
            },
        });
    }

    let config_input = build_config_input(&config_read);
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

/// Build [`ConfigInputProvenance`] from a completed config read.
fn build_config_input(config_read: &config::ConfigRead) -> ConfigInputProvenance {
    ConfigInputProvenance {
        source: config_read.accepted.as_ref().map(|a| a.source.clone()),
        input_hash: config_read.input_hash.clone(),
        file_count: config_read.file_count,
        total_bytes: config_read.total_bytes,
    }
}

/// A cooperative wall-clock observation pair (budget + observed elapsed).
struct WallClockObs {
    max_millis: u64,
    observed_millis: u64,
}

/// Assemble a [`CargoPlatformInspection`] for the post-read wall-clock trip:
/// settings are empty (parse skipped) but accepted provenance and all
/// cargo/rust-derived evidence are present and internally consistent.
fn assemble(
    cargo: &CargoInspection,
    rust: &RustSourceInspection,
    config_read: &config::ConfigRead,
    build_settings: types::CargoBuildSettings,
    target_settings: Vec<types::CargoTargetSettings>,
    benchmarks: Vec<BenchmarkEvidence>,
    wc: &WallClockObs,
) -> CargoPlatformInspection {
    let mut warnings = config_read.warnings.clone();
    warnings.push(CargoPlatformWarning::WallClock {
        max_millis: wc.max_millis,
        observed_millis: wc.observed_millis,
    });
    sort_dedup_warnings(&mut warnings);
    let wasm_targets = derive_wasm_targets(&build_settings, &target_settings);
    let native_requirements = derive_native_requirements(cargo, &target_settings, &build_settings);
    let no_std_evidence = derive_no_std(cargo, rust);
    let target_cfg_constraints = derive_target_cfg(cargo, rust);
    CargoPlatformInspection {
        build_settings,
        target_settings,
        wasm_targets,
        native_requirements,
        benchmarks,
        no_std_evidence,
        target_cfg_constraints,
        warnings,
        config_input: build_config_input(config_read),
        state: SnapshotState::LimitExceeded {
            limit: InspectionLimit::WallClock {
                max_millis: wc.max_millis,
                observed_millis: wc.observed_millis,
            },
        },
    }
}

/// Assemble a [`CargoPlatformInspection`] for the post-parse wall-clock trip:
/// parsed settings are retained and all derived evidence is computed.
fn assemble_with_wall_clock_after_parse(
    cargo: &CargoInspection,
    rust: &RustSourceInspection,
    config_read: &config::ConfigRead,
    build_settings: types::CargoBuildSettings,
    target_settings: Vec<types::CargoTargetSettings>,
    parse_warnings: Vec<CargoPlatformWarning>,
    wc: &WallClockObs,
) -> CargoPlatformInspection {
    let mut warnings = parse_warnings;
    warnings.extend(config_read.warnings.clone());
    warnings.push(CargoPlatformWarning::WallClock {
        max_millis: wc.max_millis,
        observed_millis: wc.observed_millis,
    });
    sort_dedup_warnings(&mut warnings);
    let wasm_targets = derive_wasm_targets(&build_settings, &target_settings);
    let native_requirements = derive_native_requirements(cargo, &target_settings, &build_settings);
    let benchmarks = derive_benchmarks(cargo, rust);
    let no_std_evidence = derive_no_std(cargo, rust);
    let target_cfg_constraints = derive_target_cfg(cargo, rust);
    CargoPlatformInspection {
        build_settings,
        target_settings,
        wasm_targets,
        native_requirements,
        benchmarks,
        no_std_evidence,
        target_cfg_constraints,
        warnings,
        config_input: build_config_input(config_read),
        state: SnapshotState::LimitExceeded {
            limit: InspectionLimit::WallClock {
                max_millis: wc.max_millis,
                observed_millis: wc.observed_millis,
            },
        },
    }
}

/// Deterministically sort and deduplicate warnings by a total typed key.
///
/// [`CargoPlatformWarning`] cannot derive `Ord` (it embeds
/// [`crate::inspect::InspectionLimit`]); this helper preserves a stable,
/// deterministic order by a typed sort key, not by insertion order or
/// `Debug` formatting. Multiple distinct `(setting, issue)` issues are never
/// collapsed. `InvalidConfigSetting` warnings with the same `(setting, issue)`
/// are aggregated by **summing** their counts into a single warning (so the
/// same issue across multiple target tables is fully reported, never
/// underreported by dedup).
fn sort_dedup_warnings(warnings: &mut Vec<CargoPlatformWarning>) {
    aggregate_invalid_config_settings(warnings);
    warnings.sort_by_key(warning_sort_key);
    warnings.dedup();
}

/// Merge all `InvalidConfigSetting` warnings sharing a `(setting, issue)` key
/// into a single warning whose `count` is the SUM of the merged counts.
/// Distinct `(setting, issue)` pairs remain separate. Deterministic order by
/// typed `(setting, issue)` rank.
fn aggregate_invalid_config_settings(warnings: &mut Vec<CargoPlatformWarning>) {
    use std::collections::BTreeMap;
    let mut totals: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    let mut kept: Vec<CargoPlatformWarning> = Vec::with_capacity(warnings.len());
    for w in warnings.drain(..) {
        if let CargoPlatformWarning::InvalidConfigSetting {
            setting,
            issue,
            count,
        } = w
        {
            *totals.entry((setting as u8, issue as u8)).or_insert(0) += count;
        } else {
            kept.push(w);
        }
    }
    warnings.extend(kept);
    for ((setting_rank, issue_rank), count) in totals {
        // Reconstruct the typed enums from their stable discriminant ranks.
        let setting = config_setting_from_rank(setting_rank);
        let issue = config_setting_issue_from_rank(issue_rank);
        warnings.push(CargoPlatformWarning::InvalidConfigSetting {
            setting,
            issue,
            count,
        });
    }
}

/// Reconstruct a [`ConfigSetting`] from its stable definition-order rank.
fn config_setting_from_rank(rank: u8) -> ConfigSetting {
    match rank {
        0 => ConfigSetting::BuildTarget,
        1 => ConfigSetting::BuildRustflags,
        2 => ConfigSetting::BuildRustdocflags,
        3 => ConfigSetting::TargetTableKey,
        4 => ConfigSetting::TargetTableRustflags,
        5 => ConfigSetting::TargetTableRustdocflags,
        6 => ConfigSetting::TargetTableLinker,
        _ => ConfigSetting::TargetTableRunner,
    }
}

/// Reconstruct a [`ConfigSettingIssue`] from its stable definition-order rank.
fn config_setting_issue_from_rank(rank: u8) -> ConfigSettingIssue {
    match rank {
        0 => ConfigSettingIssue::WrongType,
        1 => ConfigSettingIssue::InvalidValue,
        2 => ConfigSettingIssue::MixedArray,
        _ => ConfigSettingIssue::Empty,
    }
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
        CargoPlatformWarning::DuplicateTargetSetting { key_display, count } => {
            (5, 0, 0, *count, key_display.clone())
        }
        CargoPlatformWarning::InvalidConfigSetting {
            setting,
            issue,
            count,
        } => (6, *setting as u8, *issue as u8, *count, String::new()),
        CargoPlatformWarning::LimitExceeded { .. } => (7, 0, 0, 0, String::new()),
        CargoPlatformWarning::UnsupportedConfigFile { path, .. } => (8, 0, 0, 0, path.clone()),
        CargoPlatformWarning::WallClock { .. } => (9, 0, 0, 0, String::new()),
    }
}
