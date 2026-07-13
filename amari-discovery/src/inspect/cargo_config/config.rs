// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded, read-only reader and TOML parser for `.cargo/config.toml`.
//!
//! Reuses the Task 7 no-follow, bounded-read helpers. Only the project-root
//! `.cargo/config.toml` is authoritative in 0.24 — global/legacy config is
//! never inspected. All five [`InspectionLimits`] apply to this single input.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use sha2::{Digest, Sha256};
use syn::parse::Parser;
use syn::punctuated::Punctuated;
use syn::{Meta, Token};

use crate::error::{DiscoveryError, DiscoveryResult};
use crate::inspect::snapshot::{InspectionLimit, SnapshotState};
use crate::inspect::{bounded_read, BoundedOutcome, InspectionLimits};
#[cfg(not(unix))]
use crate::inspect::{nofollow_open_readonly, NofollowResult};

use super::types::{
    CargoBuildSettings, CargoPlatformWarning, CargoTargetKey, CargoTargetSettings, ConfigSetting,
    ConfigSettingIssue, ConfigSource, ConfiguredLinker, ConfiguredRunner, CustomTargetEvidence,
    RustflagCategory, RustflagCategoryCount, RustflagsEvidence,
};

/// The fixed relative path of the authoritative config file.
pub(super) const CONFIG_REL_PATH: &str = ".cargo/config.toml";

/// The directory containing the config file.
const CONFIG_DIR_REL: &str = ".cargo";

/// Path depth of the config file from the project root (`.cargo` + file).
const CONFIG_DEPTH: u64 = 2;

// ===========================================================================
// Race-free, no-follow config opener
// ===========================================================================

/// Outcome of [`open_config`].
#[derive(Debug)]
enum ConfigOpenOutcome {
    /// The config file was opened as a regular file, ready to read.
    Opened(std::fs::File),
    /// The `.cargo` directory or `config.toml` does not exist.
    Missing,
    /// The `.cargo` directory or `config.toml` is a symlink / loop / not a
    /// directory — never followed.
    Symlink,
    /// `config.toml` exists but is not a regular file (e.g. a directory,
    /// FIFO, or device node) — never read.
    Unsupported,
}

/// Open `.cargo/config.toml` for read-only access without ever following a
/// `.cargo` directory symlink or a `config.toml` symlink replacement race.
///
/// # Platform behaviour
///
/// - **Unix**: descends via `openat` holding directory file descriptors.
///   The root is opened as a directory, then `.cargo` is opened relative to
///   the root fd with `O_DIRECTORY | O_NOFOLLOW`, then `config.toml` is
///   opened relative to the `.cargo` fd with `O_NOFOLLOW | O_NONBLOCK` so a
///   FIFO/device entry cannot block before its file type is rejected. Because
///   each component is opened relative to a *held* parent directory descriptor,
///   a `.cargo` name swapped between a metadata check and the open can never
///   be followed. The opened descriptor is verified to be a regular file.
///   `ENOENT` maps to [`ConfigOpenOutcome::Missing`]; `ELOOP`/`ENOTDIR`
///   (symlink / not-a-directory) map to [`ConfigOpenOutcome::Symlink`].
/// - **Other targets** (including Windows): conservative static check using
///   reparse-point / symlink metadata on `.cargo` plus the existing
///   no-follow open for `config.toml`. The caller is responsible for having
///   already validated that `root` is a directory.
///
/// Errors from this function never expose absolute paths, the root value,
/// external symlink targets, or OS error strings.
fn open_config(root: &Path) -> std::io::Result<ConfigOpenOutcome> {
    #[cfg(unix)]
    {
        open_config_unix(root)
    }

    #[cfg(not(unix))]
    {
        open_config_conservative(root)
    }
}

/// Unix `openat` descent opener (race-free against `.cargo` symlink swaps).
#[cfg(unix)]
fn open_config_unix(root: &Path) -> std::io::Result<ConfigOpenOutcome> {
    use rustix::fs::{open, openat, Mode, OFlags};

    // Open the root as a directory. The caller has validated `root` is a
    // directory; a failure here is an unrecoverable I/O error.
    let root_fd = open(
        root,
        OFlags::DIRECTORY | OFlags::NONBLOCK | OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    // Open `.cargo` relative to the root fd, never following a symlink.
    let cargo_fd = match openat(
        &root_fd,
        CONFIG_DIR_REL,
        OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(ConfigOpenOutcome::Missing),
        Err(rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR) => {
            return Ok(ConfigOpenOutcome::Symlink);
        }
        Err(err) => return Err(err.into()),
    };

    // Open `config.toml` relative to the `.cargo` fd, never following a
    // symlink. The held `cargo_fd` prevents a `.cargo` name swap from
    // affecting resolution.
    let config_fd = match openat(
        &cargo_fd,
        "config.toml",
        OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(ConfigOpenOutcome::Missing),
        Err(rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR) => {
            return Ok(ConfigOpenOutcome::Symlink);
        }
        Err(err) => return Err(err.into()),
    };

    // Safe conversion: rustix OwnedFd → std File (stable since 1.63).
    let file: std::fs::File = config_fd.into();

    // Reject non-regular files (e.g. directories, FIFOs, device nodes) via
    // fstat on the fd. Reported truthfully as `Unsupported`, distinct from a
    // symlink.
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Ok(ConfigOpenOutcome::Unsupported);
    }

    Ok(ConfigOpenOutcome::Opened(file))
}

#[cfg(not(unix))]
fn open_config_conservative(root: &Path) -> std::io::Result<ConfigOpenOutcome> {
    let cargo_dir = root.join(CONFIG_DIR_REL);
    match std::fs::symlink_metadata(&cargo_dir) {
        Ok(m) => {
            if m.file_type().is_symlink() {
                return Ok(ConfigOpenOutcome::Symlink);
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ConfigOpenOutcome::Missing);
        }
        Err(e) => return Err(e),
    }
    let config_path = cargo_dir.join("config.toml");
    match nofollow_open_readonly(&config_path) {
        Ok(NofollowResult::Opened(f)) => {
            // Verify the opened entry is a regular file; a directory/FIFO/
            // device is reported as `Unsupported`, not a symlink.
            match f.metadata() {
                Ok(m) if m.file_type().is_file() => Ok(ConfigOpenOutcome::Opened(f)),
                Ok(_) => Ok(ConfigOpenOutcome::Unsupported),
                Err(_) => Ok(ConfigOpenOutcome::Unsupported),
            }
        }
        Ok(NofollowResult::SymlinkOrRace) => Ok(ConfigOpenOutcome::Symlink),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ConfigOpenOutcome::Missing),
        // Opening a directory can succeed on some platforms (e.g. read-only);
        // treat a directory-open ambiguity conservatively as Unsupported.
        Err(e)
            if e.kind() == std::io::ErrorKind::IsADirectory
                || e.raw_os_error() == Some(21) /* EISDIR */ =>
        {
            Ok(ConfigOpenOutcome::Unsupported)
        }
        Err(e) => Err(e),
    }
}

// ===========================================================================
// ConfigRead — outcome of the bounded file read
// ===========================================================================

/// The bounded read outcome for `.cargo/config.toml`.
pub(super) struct ConfigRead {
    /// Accepted file bytes + source, when the file was read into provenance.
    pub accepted: Option<AcceptedConfig>,
    /// Framed input hash over accepted config bytes (SHA-256 of empty set if none).
    pub input_hash: String,
    /// Number of config files inspected (0 or 1).
    pub file_count: u64,
    /// Total bytes of accepted config content.
    pub total_bytes: u64,
    /// Overall state for the config read.
    pub state: SnapshotState,
    /// Config-specific warnings (missing/symlink/malformed/limit/invalid-utf8).
    pub warnings: Vec<CargoPlatformWarning>,
}

/// An accepted config file (read into provenance).
pub(super) struct AcceptedConfig {
    pub source: ConfigSource,
    pub bytes: Vec<u8>,
}

/// Read `.cargo/config.toml` safely under all five limits.
///
/// Never follows the `.cargo` directory or config file symlinks. The file is
/// opened via the race-free [`open_config`] descent (Unix `openat` with held
/// directory descriptors and `O_NOFOLLOW`), so a `.cargo` symlink swapped
/// between a metadata check and the open cannot be followed. A missing
/// `.cargo` directory or `config.toml` does not consume a considered slot.
/// Limit hits produce a [`SnapshotState::LimitExceeded`] state and do not
/// accept bytes. The per-file byte limit yields
/// [`InspectionLimit::PerFileBytes`] (when per-file is the tighter bound);
/// the aggregate byte limit yields [`InspectionLimit::TotalBytes`] (when
/// aggregate is tighter). Both report `observed = max + 1` bounded evidence.
pub(super) fn read_config(root: &Path, limits: &InspectionLimits) -> DiscoveryResult<ConfigRead> {
    let start = Instant::now();
    let mut warnings: Vec<CargoPlatformWarning> = Vec::new();

    // ---- Wall-clock check (before any read) ----
    let elapsed = start.elapsed().as_millis() as u64;
    if elapsed >= limits.max_inspection_wall_millis {
        return Ok(ConfigRead {
            accepted: None,
            input_hash: empty_input_hash(),
            file_count: 0,
            total_bytes: 0,
            state: SnapshotState::LimitExceeded {
                limit: InspectionLimit::WallClock {
                    max_millis: limits.max_inspection_wall_millis,
                    observed_millis: elapsed,
                },
            },
            warnings,
        });
    }

    // ---- Fixed-path depth must respect max_traversal_depth ----
    // Checked before the open (conservative pre-traversal gate): an
    // insufficient depth budget prevents descending to `.cargo/config.toml`
    // regardless of existence.
    if limits.max_traversal_depth < CONFIG_DEPTH {
        return Ok(ConfigRead {
            accepted: None,
            input_hash: empty_input_hash(),
            file_count: 0,
            total_bytes: 0,
            state: SnapshotState::LimitExceeded {
                limit: InspectionLimit::TraversalDepth {
                    max: limits.max_traversal_depth,
                },
            },
            warnings,
        });
    }

    // ---- Race-free, no-follow open of `.cargo/config.toml` ----
    // The opener descends via `openat` (Unix) holding directory fds, so a
    // `.cargo` symlink swapped between a metadata check and the open can
    // never be followed. No canonicalization or path containment check is
    // needed: `openat` relative to a held root fd is inherently contained.
    // Errors never embed absolute paths, the root value, or OS strings.
    let mut file = match open_config(root) {
        Ok(ConfigOpenOutcome::Opened(f)) => f,
        Ok(ConfigOpenOutcome::Missing) => {
            warnings.push(CargoPlatformWarning::MissingConfig {
                path: CONFIG_REL_PATH.to_string(),
            });
            return Ok(ConfigRead {
                accepted: None,
                input_hash: empty_input_hash(),
                file_count: 0,
                total_bytes: 0,
                state: SnapshotState::Complete,
                warnings,
            });
        }
        Ok(ConfigOpenOutcome::Symlink) => {
            warnings.push(CargoPlatformWarning::SymlinkedConfig {
                path: CONFIG_REL_PATH.to_string(),
            });
            return Ok(ConfigRead {
                accepted: None,
                input_hash: empty_input_hash(),
                file_count: 0,
                total_bytes: 0,
                state: SnapshotState::Complete,
                warnings,
            });
        }
        Ok(ConfigOpenOutcome::Unsupported) => {
            warnings.push(CargoPlatformWarning::UnsupportedConfigFile {
                path: CONFIG_REL_PATH.to_string(),
                reason: "config path is not a regular file".to_string(),
            });
            return Ok(ConfigRead {
                accepted: None,
                input_hash: empty_input_hash(),
                file_count: 0,
                total_bytes: 0,
                state: SnapshotState::Complete,
                warnings,
            });
        }
        Err(_) => {
            return Err(DiscoveryError::InspectionFailure(
                "cannot open project config".to_string(),
            ));
        }
    };

    // ---- max_inspection_files == 0 → typed partial after existence is
    // established, without reading content. The race-safe open is allowed (no
    // content read) so a MISSING config yields Complete + MissingConfig +
    // count 0 (it never consumes a file-count slot). An EXISTING regular
    // config consumes the single considered slot: observed = 1. ----
    if limits.max_inspection_files == 0 {
        return Ok(ConfigRead {
            accepted: None,
            input_hash: empty_input_hash(),
            file_count: 0,
            total_bytes: 0,
            state: SnapshotState::LimitExceeded {
                limit: InspectionLimit::FileCount {
                    max: limits.max_inspection_files,
                    observed: 1,
                },
            },
            warnings,
        });
    }

    // ---- bounded read against per-file + aggregate limits ----
    // `bounded_read` reads `min(per_file, aggregate) + 1` bytes. When the
    // per-file bound is the tighter constraint and is exceeded, the file is
    // reported as `PerFileBytes`. When the aggregate bound is tighter and is
    // exceeded, the bounded read returns `Accepted` with `aggregate + 1`
    // bytes, detected below as `TotalBytes`.
    let remaining_aggregate = limits.max_inspection_bytes;
    let outcome = bounded_read(&mut file, limits.max_per_file_bytes, remaining_aggregate)?;
    let bytes = match outcome {
        BoundedOutcome::Accepted(b) => b,
        BoundedOutcome::PerFileExceeded => {
            let limit = InspectionLimit::PerFileBytes {
                max: limits.max_per_file_bytes,
                observed: limits.max_per_file_bytes.saturating_add(1),
            };
            warnings.push(CargoPlatformWarning::LimitExceeded {
                limit: limit.clone(),
            });
            return Ok(ConfigRead {
                accepted: None,
                input_hash: empty_input_hash(),
                file_count: 0,
                total_bytes: 0,
                state: SnapshotState::LimitExceeded { limit },
                warnings,
            });
        }
    };

    let content_len = bytes.len() as u64;

    // ---- aggregate byte limit (actual content) ----
    if content_len > limits.max_inspection_bytes {
        let limit = InspectionLimit::TotalBytes {
            max: limits.max_inspection_bytes,
            // `observed` is bounded evidence (`max + 1`), never 0.
            observed: limits.max_inspection_bytes.saturating_add(1),
        };
        warnings.push(CargoPlatformWarning::LimitExceeded {
            limit: limit.clone(),
        });
        return Ok(ConfigRead {
            accepted: None,
            input_hash: empty_input_hash(),
            file_count: 0,
            total_bytes: 0,
            state: SnapshotState::LimitExceeded { limit },
            warnings,
        });
    }

    // ---- accept into provenance ----
    let content_hash = hex::encode(Sha256::digest(&bytes));
    let source = ConfigSource {
        path: CONFIG_REL_PATH.to_string(),
        line: None,
        content_hash: content_hash.clone(),
        byte_count: content_len,
    };
    let input_hash = framed_config_hash(&bytes);

    Ok(ConfigRead {
        accepted: Some(AcceptedConfig { source, bytes }),
        input_hash,
        file_count: 1,
        total_bytes: content_len,
        state: SnapshotState::Complete,
        warnings,
    })
}

// ===========================================================================
// Cooperative wall-clock helper
// ===========================================================================

/// Deterministic wall-clock observation: returns `(max_millis, observed_millis)`
/// when the nonzero `elapsed` time has reached or exceeded `budget_millis`.
///
/// Pure and deterministic — unit-testable with arbitrary [`Duration`]s without
/// any wall-clock sleep. The cooperative checks in
/// [`inspect_cargo_platform`](super::inspect_cargo_platform) call this with real
/// [`Instant::elapsed`] after each bounded phase (open/read, parse, derivation).
/// The entry **zero gate** (budget `== 0`) is handled separately before any
/// read, so this helper always requires a nonzero observed elapsed.
pub(super) fn wall_clock_exceeded(
    elapsed: std::time::Duration,
    budget_millis: u64,
) -> Option<(u64, u64)> {
    let observed = elapsed.as_millis() as u64;
    if observed > 0 && observed >= budget_millis {
        Some((budget_millis, observed))
    } else {
        None
    }
}

// ===========================================================================
// Framed input hash (mirrors library provenance framing)
// ===========================================================================

/// SHA-256 of the empty framed input set (no accepted files).
pub(super) fn empty_input_hash() -> String {
    hex::encode(Sha256::digest(b""))
}

/// Framed SHA-256 over a single accepted config file: u32 LE path len,
/// path bytes, u64 LE content len, content bytes. Root-independent.
pub(super) fn framed_config_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    let path = CONFIG_REL_PATH;
    hasher.update((path.len() as u32).to_le_bytes());
    hasher.update(path.as_bytes());
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

// ===========================================================================
// ParsedConfig — TOML parse result
// ===========================================================================

/// Parsed `[build]` and `[target.*]` settings, plus parse warnings.
pub(super) struct ParsedConfig {
    pub build_settings: CargoBuildSettings,
    pub target_settings: Vec<CargoTargetSettings>,
    pub warnings: Vec<CargoPlatformWarning>,
}

/// Parse accepted config bytes into typed settings.
///
/// Invalid UTF-8 → [`CargoPlatformWarning::InvalidUtf8Config`]. Malformed
/// TOML → [`CargoPlatformWarning::MalformedConfig`]. Both still yield empty
/// settings (the bytes were already accepted into provenance by the reader).
/// TOML error messages never leak source snippets.
pub(super) fn parse_config(bytes: &[u8], source: &ConfigSource) -> ParsedConfig {
    let utf8 = match std::str::from_utf8(bytes) {
        Ok(s) => s,
        Err(_) => {
            return ParsedConfig {
                build_settings: CargoBuildSettings::default(),
                target_settings: Vec::new(),
                warnings: vec![CargoPlatformWarning::InvalidUtf8Config {
                    path: source.path.clone(),
                    content_hash: source.content_hash.clone(),
                    byte_count: source.byte_count,
                }],
            };
        }
    };

    let value: toml::Value = match toml::from_str(utf8) {
        Ok(v) => v,
        Err(e) => {
            let (line, col) = toml_line_col(bytes, &e);
            return ParsedConfig {
                build_settings: CargoBuildSettings::default(),
                target_settings: Vec::new(),
                warnings: vec![CargoPlatformWarning::MalformedConfig {
                    path: source.path.clone(),
                    reason: malformed_reason(&e),
                    line,
                    column: col,
                    content_hash: source.content_hash.clone(),
                }],
            };
        }
    };

    let mut warnings: Vec<CargoPlatformWarning> = Vec::new();
    let build_settings = parse_build(&value, source, &mut warnings);
    let target_settings = parse_targets(&value, source, &mut warnings);

    ParsedConfig {
        build_settings,
        target_settings,
        warnings,
    }
}

// ===========================================================================
// [build] parsing
// ===========================================================================

fn parse_build(
    value: &toml::Value,
    source: &ConfigSource,
    warnings: &mut Vec<CargoPlatformWarning>,
) -> CargoBuildSettings {
    let table = match value.get("build").and_then(|v| v.as_table()) {
        Some(t) => t,
        None => {
            return CargoBuildSettings {
                source: Some(source.clone()),
                ..CargoBuildSettings::default()
            }
        }
    };

    let (target, custom_targets) = parse_build_target(table.get("target"), warnings);
    let target_dir_set = table.contains_key("target-dir");
    let incremental = table.get("incremental").and_then(|v| v.as_bool());
    let rustflags = parse_rustflags_setting(
        table.get("rustflags"),
        ConfigSetting::BuildRustflags,
        warnings,
    );
    let rustdocflags = parse_rustflags_setting(
        table.get("rustdocflags"),
        ConfigSetting::BuildRustdocflags,
        warnings,
    );

    CargoBuildSettings {
        target,
        custom_targets,
        target_dir_set,
        incremental,
        rustflags,
        rustdocflags,
        source: Some(source.clone()),
    }
}

/// Classification of a single `[build].target` string entry.
enum BuildTargetClass {
    /// A validated target triple (kept verbatim in `target`).
    Triple,
    /// A custom target spec path/JSON (opaque evidence, no path retained).
    Custom,
    /// An invalid non-path, non-triple string.
    Invalid,
}

/// Returns `true` when `s` is a syntactically plausible target triple.
///
/// A triple is `arch-vendor-os[-env]`: ASCII identifier characters and at
/// least two dashes, no path separators, non-empty, bounded length.
fn is_valid_triple(s: &str) -> bool {
    if s.is_empty() || s.len() > 256 {
        return false;
    }
    let segments: Vec<&str> = s.split('-').collect();
    segments.len() >= 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.')
        })
}

/// Classify a single `[build].target` string entry.
fn classify_build_target_entry(s: &str) -> BuildTargetClass {
    // Path-like values (absolute/relative spec paths or `.json`) are custom
    // target specs — never retained as a raw string.
    if s.contains('/') || s.contains('\\') || s.ends_with(".json") {
        return BuildTargetClass::Custom;
    }
    if is_valid_triple(s) {
        return BuildTargetClass::Triple;
    }
    BuildTargetClass::Invalid
}

/// Parse `[build].target` (string or array) into validated triples plus
/// opaque custom-target-spec evidence, emitting typed warnings for wrong
/// types, mixed arrays, and invalid values. Raw paths/basenames/secrets are
/// never retained.
fn parse_build_target(
    value: Option<&toml::Value>,
    warnings: &mut Vec<CargoPlatformWarning>,
) -> (Vec<String>, CustomTargetEvidence) {
    let value = match value {
        Some(v) => v,
        None => return (Vec::new(), CustomTargetEvidence::default()),
    };
    let members: Vec<&toml::Value> = match value {
        toml::Value::String(_) => vec![value],
        toml::Value::Array(arr) => arr.iter().collect(),
        _ => {
            push_setting_warning(
                warnings,
                ConfigSetting::BuildTarget,
                ConfigSettingIssue::WrongType,
            );
            return (Vec::new(), CustomTargetEvidence::default());
        }
    };
    let mut triples: Vec<String> = Vec::new();
    let mut custom: Vec<String> = Vec::new();
    let mut invalid_count = 0usize;
    let mut mixed_count = 0usize;
    for m in members {
        match m.as_str() {
            Some(s) => match classify_build_target_entry(s) {
                BuildTargetClass::Triple => triples.push(s.to_string()),
                BuildTargetClass::Custom => custom.push(s.to_string()),
                BuildTargetClass::Invalid => invalid_count += 1,
            },
            None => mixed_count += 1,
        }
    }
    triples.sort();
    triples.dedup();
    let custom_ev = CustomTargetEvidence {
        count: custom.len(),
        identity: opaque_identity_tokens(&custom),
    };
    if mixed_count > 0 {
        push_setting_warning_count(
            warnings,
            ConfigSetting::BuildTarget,
            ConfigSettingIssue::MixedArray,
            mixed_count,
        );
    }
    if invalid_count > 0 {
        push_setting_warning_count(
            warnings,
            ConfigSetting::BuildTarget,
            ConfigSettingIssue::InvalidValue,
            invalid_count,
        );
    }
    (triples, custom_ev)
}

/// Push a single-count typed setting warning.
fn push_setting_warning(
    warnings: &mut Vec<CargoPlatformWarning>,
    setting: ConfigSetting,
    issue: ConfigSettingIssue,
) {
    push_setting_warning_count(warnings, setting, issue, 1);
}

/// Push a typed setting warning with an explicit count.
fn push_setting_warning_count(
    warnings: &mut Vec<CargoPlatformWarning>,
    setting: ConfigSetting,
    issue: ConfigSettingIssue,
    count: usize,
) {
    warnings.push(CargoPlatformWarning::InvalidConfigSetting {
        setting,
        issue,
        count,
    });
}

// ===========================================================================
// [target.*] parsing
// ===========================================================================

fn parse_targets(
    value: &toml::Value,
    source: &ConfigSource,
    warnings: &mut Vec<CargoPlatformWarning>,
) -> Vec<CargoTargetSettings> {
    let target_table = match value.get("target").and_then(|v| v.as_table()) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let mut out: Vec<CargoTargetSettings> = Vec::new();
    for (raw_key, entry) in target_table {
        let entry_table = match entry.as_table() {
            Some(t) => t,
            None => continue,
        };
        match normalize_target_key(raw_key) {
            Some(key) => {
                let rustflags = parse_rustflags_setting(
                    entry_table.get("rustflags"),
                    ConfigSetting::TargetTableRustflags,
                    warnings,
                );
                let rustdocflags = parse_rustflags_setting(
                    entry_table.get("rustdocflags"),
                    ConfigSetting::TargetTableRustdocflags,
                    warnings,
                );
                let linker = parse_linker(entry_table.get("linker"), warnings);
                let runner = parse_runner(
                    entry_table.get("runner"),
                    ConfigSetting::TargetTableRunner,
                    warnings,
                );
                out.push(CargoTargetSettings {
                    key,
                    rustflags,
                    rustdocflags,
                    linker,
                    runner,
                    source: source.clone(),
                });
            }
            None => warnings.push(CargoPlatformWarning::InvalidTargetIdentifier {
                path: source.path.clone(),
                reason: "invalid Cargo target triple or cfg selector".to_string(),
            }),
        }
    }

    out.sort();
    // Merge by key: keep the first occurrence deterministically. Two
    // normalized-equal keys with IDENTICAL settings are silently deduped; two
    // with CONFLICTING settings (rustflags/rustdocflags/linker/runner differ)
    // keep the first and emit a typed `DuplicateTargetSetting` warning so no
    // conflict is silently discarded. The warning carries the redacted key
    // display (never a secret).
    let mut merged: Vec<CargoTargetSettings> = Vec::new();
    let mut conflict_counts: BTreeMap<String, usize> = BTreeMap::new();
    for entry in out {
        if let Some(first) = merged.iter_mut().rev().find(|e| e.key == entry.key) {
            if first.rustflags != entry.rustflags
                || first.rustdocflags != entry.rustdocflags
                || first.linker != entry.linker
                || first.runner != entry.runner
            {
                *conflict_counts
                    .entry(target_key_display(&entry.key))
                    .or_insert(0) += 1;
            }
            // Identical → silent dedup; conflicting → already counted.
        } else {
            merged.push(entry);
        }
    }
    for (key_display, count) in conflict_counts {
        warnings.push(CargoPlatformWarning::DuplicateTargetSetting { key_display, count });
    }
    merged
}

/// Redacted, stable display form of a [`CargoTargetKey`] for use in warnings
/// (never carries a secret — cfg values are already redacted in the stored
/// display).
pub(super) fn target_key_display(key: &CargoTargetKey) -> String {
    match key {
        CargoTargetKey::Triple { triple } => triple.clone(),
        CargoTargetKey::Cfg { display, .. } => display.clone(),
    }
}

/// Normalize a raw target table key into a validated [`CargoTargetKey`].
///
/// Strips surrounding quotes and whitespace. Accepts:
/// - `cfg(...)` expressions validated by a bounded recursive grammar parser
///   that accepts legitimate Cargo selectors (including
///   `target_feature = "+atomics"`, nested `all`/`any`/`not`, and escaped
///   string content) while rejecting unquoted values, malformed operators,
///   trailing junk, and injection input. Quoted values are redacted in the
///   display form; a distinct SHA-256 identity preserves uniqueness.
/// - target triples validated by the SAME `>= 2`-dash rule as `[build].target`
///   (`is_valid_triple`), so target-table triples and build targets accept an
///   identical identifier class.
fn normalize_target_key(raw: &str) -> Option<CargoTargetKey> {
    let trimmed = raw.trim_matches('\'').trim_matches('"').trim();
    if trimmed.is_empty() || trimmed.len() > 256 {
        return None;
    }
    if let Some(inner) = trimmed
        .strip_prefix("cfg(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let inner = inner.trim();
        if inner.is_empty() {
            return None;
        }
        return validate_and_redact_cfg(inner)
            .map(|(display, identity)| CargoTargetKey::Cfg { display, identity });
    }
    // Target triple: same >= 2-dash rule as build targets.
    if is_valid_triple(trimmed) {
        return Some(CargoTargetKey::Triple {
            triple: trimmed.to_string(),
        });
    }
    None
}

/// Maximum cfg body length and nesting depth accepted by the sanitizer.
const CFG_MAX_LEN: usize = 1024;
const CFG_MAX_DEPTH: i32 = 32;

/// Validate a `cfg(...)` body with a bounded, recursive grammar parser and
/// return the canonical redacted display form plus a distinct SHA-256
/// identity.
///
/// The grammar accepts exactly the Cargo/Rust cfg predicate subset:
/// - top level: a comma-separated conjunction (implicit `all`) of one or more
///   predicates;
/// - `Meta::Path` — a single-segment identifier (e.g. `unix`, `windows`);
/// - `Meta::NameValue` — `name = "<string literal>"` only (non-string
///   literals, barewords after `=`, and malformed operators are rejected);
/// - `Meta::List` — only `all`/`any` (≥1 element) or `not` (exactly 1
///   element), recursively.
///
/// Bounded by length and depth. Trailing junk, unbalanced delimiters, and
/// injection tokens (`;`, `/`, shell metacharacters) are rejected because the
/// recursive parser requires strict comma separation and valid Rust tokens.
///
/// Whitespace is **normalized**: the display and identity are rebuilt from the
/// parse tree with canonical spacing, so `cfg(a, b)` and `cfg(a,b)` (and any
/// other whitespace variant) produce identical keys and merge. Quoted values
/// are redacted to `<value>` in the display but **preserved verbatim** in the
/// canonical form used for the identity hash, so distinct secret values remain
/// distinct while no secret enters the display or serialized output.
fn validate_and_redact_cfg(inner: &str) -> Option<(String, String)> {
    if inner.is_empty() || inner.len() > CFG_MAX_LEN {
        return None;
    }
    let tokens = <proc_macro2::TokenStream as std::str::FromStr>::from_str(inner).ok()?;
    let list = Punctuated::<Meta, Token![,]>::parse_terminated
        .parse2(tokens)
        .ok()?;
    if list.is_empty() {
        return None;
    }
    let mut display = String::with_capacity(inner.len());
    let mut canonical = String::with_capacity(inner.len());
    let mut first = true;
    for m in &list {
        if !first {
            display.push_str(", ");
            canonical.push(',');
        }
        first = false;
        if !render_meta(m, 0, &mut display, &mut canonical) {
            return None;
        }
    }
    let display = format!("cfg({display})");
    let identity = opaque_identity(&format!("cfg({canonical})"));
    Some((display, identity))
}

/// Recursively render a parsed cfg predicate into a redacted display form and
/// a value-preserving canonical form. Returns `false` on any grammar
/// violation.
fn render_meta(meta: &Meta, depth: i32, display: &mut String, canonical: &mut String) -> bool {
    if depth > CFG_MAX_DEPTH {
        return false;
    }
    match meta {
        Meta::Path(path) => match single_ident(path) {
            Some(name) => {
                display.push_str(&name);
                canonical.push_str(&name);
                true
            }
            None => false,
        },
        Meta::NameValue(nv) => {
            let Some(name) = single_ident(&nv.path) else {
                return false;
            };
            // Value must be a STRING literal only.
            let lit = match &nv.value {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }) => s,
                _ => return false,
            };
            display.push_str(&name);
            display.push_str(" = \"<value>\"");
            canonical.push_str(&name);
            canonical.push_str(" = ");
            // Re-tokenized literal value (normalized) preserves distinctness
            // for the identity hash without leaking into the display.
            canonical.push_str(&lit.token().to_string());
            true
        }
        Meta::List(ml) => {
            let Some(name) = single_ident(&ml.path) else {
                return false;
            };
            match name.as_str() {
                "all" | "any" => {
                    let inner = Punctuated::<Meta, Token![,]>::parse_terminated
                        .parse2(ml.tokens.clone())
                        .ok();
                    let Some(inner) = inner else { return false };
                    if inner.is_empty() {
                        return false;
                    }
                    display.push_str(&name);
                    display.push('(');
                    canonical.push_str(&name);
                    canonical.push('(');
                    let mut first = true;
                    for m in inner {
                        if !first {
                            display.push_str(", ");
                            canonical.push(',');
                        }
                        first = false;
                        if !render_meta(&m, depth + 1, display, canonical) {
                            return false;
                        }
                    }
                    display.push(')');
                    canonical.push(')');
                    true
                }
                "not" => {
                    let inner = Punctuated::<Meta, Token![,]>::parse_terminated
                        .parse2(ml.tokens.clone())
                        .ok();
                    let Some(inner) = inner else { return false };
                    // `not` requires exactly one argument.
                    if inner.len() != 1 {
                        return false;
                    }
                    display.push_str("not(");
                    canonical.push_str("not(");
                    if !render_meta(&inner[0], depth + 1, display, canonical) {
                        return false;
                    }
                    display.push(')');
                    canonical.push(')');
                    true
                }
                _ => false,
            }
        }
    }
}

/// Return the identifier text when `path` is a single unqualified segment with
/// no path arguments (no leading `::`, no `a::b`, no turbofish). Otherwise
/// `None`.
fn single_ident(path: &syn::Path) -> Option<String> {
    if path.leading_colon.is_some() {
        return None;
    }
    if path.segments.len() != 1 {
        return None;
    }
    let seg = path.segments.first()?;
    if !matches!(seg.arguments, syn::PathArguments::None) {
        return None;
    }
    Some(seg.ident.to_string())
}

/// Parse a `[target.<key>].linker` value into a sanitized
/// [`ConfiguredLinker`], emitting typed warnings for wrong types and empty
/// values. An empty linker string yields a typed `Empty` warning and `None`
/// (never `Some` with an empty basename).
fn parse_linker(
    value: Option<&toml::Value>,
    warnings: &mut Vec<CargoPlatformWarning>,
) -> Option<ConfiguredLinker> {
    let raw = value?;
    match raw.as_str() {
        Some(s) => {
            let basename = sanitize_basename(s);
            if basename.is_empty() {
                push_setting_warning(
                    warnings,
                    ConfigSetting::TargetTableLinker,
                    ConfigSettingIssue::Empty,
                );
                return None;
            }
            Some(ConfiguredLinker {
                basename,
                identity: opaque_identity(s),
            })
        }
        None => {
            push_setting_warning(
                warnings,
                ConfigSetting::TargetTableLinker,
                ConfigSettingIssue::WrongType,
            );
            None
        }
    }
}

/// Parse a runner value (string or array) into a sanitized [`ConfiguredRunner`],
/// emitting typed warnings for wrong types, empty values, and mixed arrays.
fn parse_runner(
    value: Option<&toml::Value>,
    setting: ConfigSetting,
    warnings: &mut Vec<CargoPlatformWarning>,
) -> Option<ConfiguredRunner> {
    let raw = value?;
    let (first_token, token_count, identity) = match raw {
        toml::Value::String(s) => {
            let tokens: Vec<&str> = s.split_whitespace().collect();
            if tokens.is_empty() {
                push_setting_warning(warnings, setting, ConfigSettingIssue::Empty);
                return None;
            }
            let first = tokens.first().copied().unwrap_or("").to_string();
            let owned: Vec<String> = tokens.iter().map(|t| (*t).to_string()).collect();
            (first, tokens.len(), opaque_identity_tokens(&owned))
        }
        toml::Value::Array(arr) => {
            let mut strs: Vec<String> = Vec::new();
            let mut mixed_count = 0usize;
            for v in arr {
                match v.as_str() {
                    Some(s) => strs.push(s.to_string()),
                    None => mixed_count += 1,
                }
            }
            if strs.is_empty() {
                push_setting_warning(warnings, setting, ConfigSettingIssue::Empty);
                return None;
            }
            if mixed_count > 0 {
                push_setting_warning_count(
                    warnings,
                    setting,
                    ConfigSettingIssue::MixedArray,
                    mixed_count,
                );
            }
            let first = strs[0].clone();
            let count = strs.len();
            (first, count, opaque_identity_tokens(&strs))
        }
        _ => {
            push_setting_warning(warnings, setting, ConfigSettingIssue::WrongType);
            return None;
        }
    };
    Some(ConfiguredRunner {
        executable_basename: sanitize_basename_checked(&first_token, warnings, setting)?,
        token_count,
        identity,
    })
}

/// Sanitize the runner program token, returning `None` (with a typed `Empty`
/// warning) when the resulting basename is empty — so no `Some` with an empty
/// basename is ever produced for a mixed/empty runner array or empty program.
fn sanitize_basename_checked(
    raw: &str,
    warnings: &mut Vec<CargoPlatformWarning>,
    setting: ConfigSetting,
) -> Option<String> {
    let basename = sanitize_basename(raw);
    if basename.is_empty() {
        push_setting_warning(warnings, setting, ConfigSettingIssue::Empty);
        return None;
    }
    Some(basename)
}

// ===========================================================================
// rustflags evidence
// ===========================================================================

/// Build [`RustflagsEvidence`] from a `rustflags`/`rustdocflags` value,
/// emitting typed warnings for wrong types and mixed arrays.
pub(super) fn parse_rustflags_setting(
    value: Option<&toml::Value>,
    setting: ConfigSetting,
    warnings: &mut Vec<CargoPlatformWarning>,
) -> RustflagsEvidence {
    let value = match value {
        Some(v) => v,
        None => return RustflagsEvidence::empty(),
    };
    let tokens: Vec<String> = match value {
        toml::Value::String(s) => s.split_whitespace().map(String::from).collect(),
        toml::Value::Array(arr) => {
            let mut toks: Vec<String> = Vec::new();
            let mut mixed_count = 0usize;
            for v in arr {
                match v.as_str() {
                    Some(s) => toks.push(s.to_string()),
                    None => mixed_count += 1,
                }
            }
            if mixed_count > 0 {
                push_setting_warning_count(
                    warnings,
                    setting,
                    ConfigSettingIssue::MixedArray,
                    mixed_count,
                );
            }
            toks
        }
        _ => {
            push_setting_warning(warnings, setting, ConfigSettingIssue::WrongType);
            return RustflagsEvidence::empty();
        }
    };
    rustflags_evidence(&tokens)
}

/// Build deterministic, sanitized evidence from flag tokens.
pub(super) fn rustflags_evidence(tokens: &[String]) -> RustflagsEvidence {
    let mut counts: BTreeMap<RustflagCategory, usize> = BTreeMap::new();
    let mut has_native_linking = false;
    let mut native_hasher = Sha256::new();
    let mut native_flag_count = 0usize;
    let mut hasher = Sha256::new();
    for tok in tokens {
        hasher.update((tok.len() as u32).to_le_bytes());
        hasher.update(tok.as_bytes());
        let cat = classify_rustflag(tok);
        *counts.entry(cat).or_insert(0) += 1;
        if cat.is_native_linking() {
            has_native_linking = true;
            native_flag_count += 1;
            native_hasher.update((tok.len() as u32).to_le_bytes());
            native_hasher.update(tok.as_bytes());
        }
    }
    let identity = hex::encode(hasher.finalize());
    let native_identity = hex::encode(native_hasher.finalize());
    let categories: Vec<RustflagCategoryCount> = counts
        .into_iter()
        .map(|(category, count)| RustflagCategoryCount { category, count })
        .collect();
    RustflagsEvidence {
        flag_count: tokens.len(),
        categories,
        has_native_linking,
        native_flag_count,
        identity,
        native_identity,
    }
}

/// Classify a single rustflag token by its prefix (values never inspected).
fn classify_rustflag(token: &str) -> RustflagCategory {
    let t = token.trim();
    if t.is_empty() {
        return RustflagCategory::Unknown;
    }
    // -L (uppercase) = native link search path.
    if t.starts_with("-L") {
        return RustflagCategory::LinkSearch;
    }
    // -l (lowercase L) = link a native library. ("-L" already handled above.)
    if t.starts_with("-l") {
        return RustflagCategory::LibraryLink;
    }
    let lower = t.to_ascii_lowercase();
    let no_val = lower.split('=').next().unwrap_or(&lower);
    let p = no_val.trim();
    if p == "-c" {
        return RustflagCategory::Codegen;
    }
    if p == "-c link-arg"
        || p.starts_with("-clink-arg")
        || p == "link-arg"
        || p.starts_with("link-arg")
        || p == "-c link-args"
        || p.starts_with("-clink-args")
        || p == "link-args"
        || p.starts_with("link-args")
    {
        return RustflagCategory::LinkArg;
    }
    if p == "-c linker" || p.starts_with("-clinker") || p == "linker" || p.starts_with("linker=") {
        return RustflagCategory::Linker;
    }
    if p == "-c link-search-path"
        || p.starts_with("-clink-search-path")
        || p == "link-search-path"
        || p.starts_with("link-search-path")
    {
        return RustflagCategory::LinkSearch;
    }
    if p == "-c target-feature"
        || p.starts_with("-ctarget-feature")
        || p == "target-feature"
        || p == "-c target-cpu"
        || p.starts_with("-ctarget-cpu")
        || p == "target-cpu"
    {
        return RustflagCategory::TargetFeature;
    }
    if p.starts_with("-c") || p.starts_with("-c=") {
        return RustflagCategory::Codegen;
    }
    if p.starts_with("-z") {
        return RustflagCategory::Unstable;
    }
    if p == "--cfg" || p.starts_with("--cfg") {
        return RustflagCategory::Cfg;
    }
    if p.starts_with("-w") || p == "--warn" || p.starts_with("--warn") {
        return RustflagCategory::Warning;
    }
    if p == "--remap-path-prefix" || p.starts_with("--remap-path-prefix") {
        return RustflagCategory::RemapPath;
    }
    RustflagCategory::Unknown
}

// ===========================================================================
// Sanitization helpers
// ===========================================================================

/// Sanitize an executable string to its basename (no path, no args).
///
/// Both `/` and `\\` are treated as separators regardless of the host OS so
/// inspecting a Windows project on Unix (or vice versa) cannot persist the
/// configured executable's directory path.
pub(super) fn sanitize_basename(raw: &str) -> String {
    let first_token = raw.split_whitespace().next().unwrap_or("");
    first_token
        .rsplit(['/', '\\'])
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("")
        .to_string()
}

/// SHA-256 identity over a raw configured value (framed, not reversible).
pub(super) fn opaque_identity(raw: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update((raw.len() as u64).to_le_bytes());
    hasher.update(raw.as_bytes());
    hex::encode(hasher.finalize())
}

/// SHA-256 identity over a token sequence using explicit per-token length
/// framing (never a separator join). Each token contributes a `u64` LE
/// length prefix followed by its bytes, so two distinct sequences never
/// collide even if a token contains control or separator characters.
pub(super) fn opaque_identity_tokens(tokens: &[String]) -> String {
    let mut hasher = Sha256::new();
    for tok in tokens {
        hasher.update((tok.len() as u64).to_le_bytes());
        hasher.update(tok.as_bytes());
    }
    hex::encode(hasher.finalize())
}

// ===========================================================================
// TOML error helpers (stable, no source snippets)
// ===========================================================================

fn toml_line_col(source: &[u8], err: &toml::de::Error) -> (Option<usize>, Option<usize>) {
    if let Some(span) = err.span() {
        let pos = span.start.min(source.len());
        let line = source[..pos].iter().filter(|&&b| b == b'\n').count() + 1;
        let col = match source[..pos].iter().rposition(|&b| b == b'\n') {
            Some(last_nl) => pos - last_nl,
            None => pos + 1,
        };
        return (Some(line), Some(col));
    }
    let msg = err.to_string();
    let line = msg
        .split(" at line ")
        .nth(1)
        .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|s| s.parse::<usize>().ok());
    let col = msg
        .split(" column ")
        .nth(1)
        .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|s| s.parse::<usize>().ok());
    (line, col)
}

fn malformed_reason(err: &toml::de::Error) -> String {
    let msg = err.to_string();
    if msg.contains("missing field") {
        "missing required field".to_string()
    } else if msg.contains("invalid type") {
        "invalid type for field".to_string()
    } else if msg.contains("duplicate") {
        "duplicate key".to_string()
    } else if msg.contains("expected") {
        "unexpected TOML syntax".to_string()
    } else if msg.contains("newline") || msg.contains("EOF") {
        "unterminated string or table".to_string()
    } else {
        "invalid TOML".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn wall_clock_exceeded_helper_is_deterministic() {
        // Zero observed never trips (even at a zero budget) — the entry zero
        // gate handles budget == 0 separately before any cooperative check.
        assert_eq!(wall_clock_exceeded(Duration::ZERO, 0), None);
        assert_eq!(wall_clock_exceeded(Duration::ZERO, 100), None);
        // Nonzero observed at/above budget trips with actual observed value.
        assert_eq!(
            wall_clock_exceeded(Duration::from_millis(100), 50),
            Some((50, 100))
        );
        // Boundary: observed exactly at budget (nonzero) still trips.
        assert_eq!(
            wall_clock_exceeded(Duration::from_millis(50), 50),
            Some((50, 50))
        );
        // Below budget: no trip.
        assert_eq!(wall_clock_exceeded(Duration::from_millis(10), 50), None);
        // Sub-millisecond observed (rounds to 0) never trips via the helper.
        assert_eq!(wall_clock_exceeded(Duration::from_micros(999), 0), None);
    }

    #[test]
    fn wall_clock_exceeded_micros_round_to_zero() {
        // 999 microseconds rounds down to 0 ms → no trip (no flaky sub-ms).
        assert_eq!(wall_clock_exceeded(Duration::from_micros(999), 1), None);
    }

    #[test]
    fn empty_input_hash_is_sha256_of_empty() {
        assert_eq!(empty_input_hash(), hex::encode(Sha256::digest(b"")));
    }

    #[test]
    fn framed_hash_is_root_independent() {
        let bytes = b"[build]\ntarget = [\"wasm32-unknown-unknown\"]\n";
        let h1 = framed_config_hash(bytes);
        let h2 = framed_config_hash(bytes);
        assert_eq!(h1, h2);
        assert_ne!(h1, empty_input_hash());
    }

    #[test]
    fn normalize_target_key_cfg_and_triple() {
        assert!(matches!(
            normalize_target_key("'cfg(unix)'"),
            Some(CargoTargetKey::Cfg { ref display, .. }) if display == "cfg(unix)"
        ));
        assert!(matches!(
            normalize_target_key("wasm32-unknown-unknown"),
            Some(CargoTargetKey::Triple { triple }) if triple == "wasm32-unknown-unknown"
        ));
        assert!(normalize_target_key("").is_none());
        assert!(normalize_target_key("not a triple").is_none());
    }

    #[test]
    fn classify_rustflag_categories() {
        assert_eq!(
            classify_rustflag("-Lnative=foo"),
            RustflagCategory::LinkSearch
        );
        assert_eq!(classify_rustflag("-lm"), RustflagCategory::LibraryLink);
        assert_eq!(
            classify_rustflag("link-arg=--secret"),
            RustflagCategory::LinkArg
        );
        assert_eq!(classify_rustflag("-C"), RustflagCategory::Codegen);
        assert_eq!(classify_rustflag("--cfg"), RustflagCategory::Cfg);
        assert_eq!(classify_rustflag("-Zunstable"), RustflagCategory::Unstable);
        assert_eq!(
            classify_rustflag("target-feature=+atomics"),
            RustflagCategory::TargetFeature
        );
        assert_eq!(classify_rustflag("web_sys"), RustflagCategory::Unknown);
    }

    #[test]
    fn sanitize_basename_strips_path_and_args() {
        assert_eq!(sanitize_basename("/usr/bin/rust-lld"), "rust-lld");
        assert_eq!(sanitize_basename("valgrind --tool=memcheck"), "valgrind");
        assert_eq!(sanitize_basename("link.exe"), "link.exe");
    }

    // -- B1: open_config race-free descent (private opener harness) --

    #[cfg(unix)]
    #[test]
    fn open_config_missing_cargo_dir() {
        let dir = tempfile::TempDir::new().unwrap();
        // No .cargo at all.
        match open_config(dir.path()).unwrap() {
            ConfigOpenOutcome::Missing => {}
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn open_config_missing_config_file() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".cargo")).unwrap();
        match open_config(dir.path()).unwrap() {
            ConfigOpenOutcome::Missing => {}
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn open_config_opens_regular_file() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".cargo")).unwrap();
        std::fs::write(dir.path().join(".cargo").join("config.toml"), b"[build]\n").unwrap();
        match open_config(dir.path()).unwrap() {
            ConfigOpenOutcome::Opened(_) => {}
            other => panic!("expected Opened, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn open_config_rejects_cargo_dir_symlink() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::TempDir::new().unwrap();
        // Real dir with a config, elsewhere under the temp root.
        let real = dir.path().join("real-cargo");
        std::fs::create_dir(&real).unwrap();
        std::fs::write(real.join("config.toml"), b"[build]\n").unwrap();
        // .cargo is a symlink to that real dir — must never be followed.
        symlink(&real, dir.path().join(".cargo")).unwrap();
        match open_config(dir.path()).unwrap() {
            ConfigOpenOutcome::Symlink => {}
            other => panic!("expected Symlink for .cargo symlink, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn open_config_rejects_config_file_symlink() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(dir.path().join(".cargo")).unwrap();
        let outside = dir.path().join("outside.toml");
        std::fs::write(&outside, b"[build]\n").unwrap();
        symlink(&outside, dir.path().join(".cargo").join("config.toml")).unwrap();
        match open_config(dir.path()).unwrap() {
            ConfigOpenOutcome::Symlink => {}
            other => panic!("expected Symlink for config.toml symlink, got {other:?}"),
        }
    }

    /// Child-process entry point used by [`open_config_fifo_never_blocks`].
    #[cfg(unix)]
    #[test]
    fn open_config_fifo_child() {
        let Ok(root) = std::env::var("AMARI_DISCOVERY_FIFO_TEST_ROOT") else {
            return;
        };
        match open_config(std::path::Path::new(&root)).unwrap() {
            ConfigOpenOutcome::Unsupported => {}
            other => panic!("expected Unsupported for FIFO, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn open_config_fifo_never_blocks() {
        use rustix::fs::{mkfifoat, open, Mode, OFlags};
        use std::process::Command;
        use std::thread;
        use std::time::{Duration, Instant};

        let dir = tempfile::TempDir::new().unwrap();
        let cargo_path = dir.path().join(".cargo");
        std::fs::create_dir(&cargo_path).unwrap();
        let cargo_fd = open(
            &cargo_path,
            OFlags::DIRECTORY | OFlags::RDONLY | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .unwrap();
        mkfifoat(&cargo_fd, "config.toml", Mode::RUSR | Mode::WUSR).unwrap();

        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("open_config_fifo_child")
            .arg("--nocapture")
            .env("AMARI_DISCOVERY_FIFO_TEST_ROOT", dir.path())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "FIFO child failed: {status}");
                break;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("opening a FIFO config blocked beyond the bounded deadline");
            }
            thread::sleep(Duration::from_millis(20));
        }
    }

    #[test]
    fn runner_array_identity_no_separator_collision() {
        // Two distinct runner arrays must never share an identity. With a
        // separator join, ["a", "b"] and ["a\u{1f}b"] both serialize to the
        // same string "a\u{1f}b" — a collision. Explicit per-token length
        // framing distinguishes them.
        let mut warnings = Vec::new();
        let two_tokens = parse_runner(
            Some(&toml::Value::Array(vec![
                toml::Value::String("a".into()),
                toml::Value::String("b".into()),
            ])),
            ConfigSetting::TargetTableRunner,
            &mut warnings,
        )
        .expect("two-token runner");
        let one_token_with_sep = parse_runner(
            Some(&toml::Value::Array(vec![toml::Value::String(
                "a\u{1f}b".into(),
            )])),
            ConfigSetting::TargetTableRunner,
            &mut warnings,
        )
        .expect("one-token runner containing separator");
        assert_ne!(
            two_tokens.identity, one_token_with_sep.identity,
            "separator-join identity collision between distinct runner arrays"
        );
        assert_eq!(two_tokens.token_count, 2);
        assert_eq!(one_token_with_sep.token_count, 1);
    }
}
