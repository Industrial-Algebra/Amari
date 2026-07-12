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
}

/// Open `.cargo/config.toml` for read-only access without ever following a
/// `.cargo` directory symlink or a `config.toml` symlink replacement race.
///
/// # Platform behaviour
///
/// - **Unix**: descends via `openat` holding directory file descriptors.
///   The root is opened as a directory, then `.cargo` is opened relative to
///   the root fd with `O_DIRECTORY | O_NOFOLLOW`, then `config.toml` is
///   opened relative to the `.cargo` fd with `O_NOFOLLOW`. Because each
///   component is opened relative to a *held* parent directory descriptor,
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
        OFlags::DIRECTORY | OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;

    // Open `.cargo` relative to the root fd, never following a symlink.
    let cargo_fd = match openat(
        &root_fd,
        CONFIG_DIR_REL,
        OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::RDONLY | OFlags::CLOEXEC,
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
        OFlags::NOFOLLOW | OFlags::RDONLY | OFlags::CLOEXEC,
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

    // Reject non-regular files (e.g. device nodes) via fstat on the fd.
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Ok(ConfigOpenOutcome::Symlink);
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
        Ok(NofollowResult::Opened(f)) => Ok(ConfigOpenOutcome::Opened(f)),
        Ok(NofollowResult::SymlinkOrRace) => Ok(ConfigOpenOutcome::Symlink),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(ConfigOpenOutcome::Missing),
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

    // ---- max_inspection_files == 0 → typed partial without reading ----
    // `observed = 1` reflects considered-file semantics: the config is one
    // considered candidate, even though no content read occurs.
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

    // ---- Fixed-path depth must respect max_traversal_depth ----
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
        Err(_) => {
            return Err(DiscoveryError::InspectionFailure(
                "cannot open project config".to_string(),
            ));
        }
    };

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
    !s.is_empty()
        && s.len() <= 256
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && s.matches('-').count() >= 2
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
    // Dedup by key: distinct TOML headers may normalize to the same
    // `CargoTargetKey` (e.g. single- vs double-quoted cfg wrappers). Equal
    // keys imply identical predicates (identity hash matches); we keep the
    // first occurrence deterministically so no duplicate key survives.
    out.dedup_by(|a, b| a.key == b.key);
    out
}

/// Normalize a raw target table key into a validated [`CargoTargetKey`].
///
/// Strips surrounding quotes and whitespace. Accepts:
/// - `cfg(...)` expressions validated by a bounded tokenizer that accepts
///   legitimate Cargo selectors (including `target_feature = "+atomics"`,
///   nested `all`/`any`/`not`, and escaped string content) while rejecting
///   unbalanced/injection input. Quoted values are redacted in the display
///   form; a distinct SHA-256 identity preserves uniqueness.
/// - target triples matching `[A-Za-z0-9._-]+` containing at least one `-`.
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
    // Target triple: only safe identifier characters and at least one dash.
    if trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        && trimmed.contains('-')
    {
        return Some(CargoTargetKey::Triple {
            triple: trimmed.to_string(),
        });
    }
    None
}

/// Maximum cfg body length and nesting depth accepted by the sanitizer.
const CFG_MAX_LEN: usize = 1024;
const CFG_MAX_DEPTH: i32 = 32;

/// Validate a `cfg(...)` body with a bounded tokenizer and return the
/// redacted display form plus a distinct SHA-256 identity.
///
/// Accepts legitimate Cargo selectors: option names, `=`/`,`/`(`/`)`, `all`,
/// `any`, `not`, quoted string values (including `+atomics`-style values and
/// `\`-escaped content). Rejects unbalanced parens/quotes, excessive nesting
/// or length, disallowed characters outside strings, and control characters
/// inside strings. All quoted values are replaced with `<value>` in the
/// display form (never leaking secrets); the identity is computed over the
/// full original predicate so distinct selectors stay distinct.
fn validate_and_redact_cfg(inner: &str) -> Option<(String, String)> {
    if inner.is_empty() || inner.len() > CFG_MAX_LEN {
        return None;
    }
    let mut depth: i32 = 0;
    let mut in_str = false;
    let mut escape = false;
    let mut display = String::with_capacity(inner.len());
    for ch in inner.chars() {
        if in_str {
            if escape {
                escape = false;
                continue; // escaped char — redacted
            }
            match ch {
                '\\' => escape = true,
                '"' => in_str = false, // close (placeholder has closing quote)
                c if (c as u32) < 0x20 || c == '\u{7f}' => return None, // control char
                _ => {}                // content char — redacted
            }
            continue;
        }
        match ch {
            '(' => {
                depth += 1;
                if depth > CFG_MAX_DEPTH {
                    return None;
                }
                display.push('(');
            }
            ')' => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
                display.push(')');
            }
            '"' => {
                in_str = true;
                display.push_str("\"<value>\"");
            }
            c if c.is_ascii_alphanumeric()
                || matches!(c, '_' | '-' | '.' | '=' | ',' | ' ' | '+') =>
            {
                display.push(c);
            }
            _ => return None, // disallowed character outside a string
        }
    }
    if depth != 0 || in_str || escape {
        return None;
    }
    let full = format!("cfg({inner})");
    let display = format!("cfg({display})");
    Some((display, opaque_identity(&full)))
}

/// Parse a `[target.<key>].linker` value into a sanitized
/// [`ConfiguredLinker`], emitting a typed warning for wrong types.
fn parse_linker(
    value: Option<&toml::Value>,
    warnings: &mut Vec<CargoPlatformWarning>,
) -> Option<ConfiguredLinker> {
    let raw = value?;
    match raw.as_str() {
        Some(s) => Some(ConfiguredLinker {
            basename: sanitize_basename(s),
            identity: opaque_identity(s),
        }),
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
        executable_basename: sanitize_basename(&first_token),
        token_count,
        identity,
    })
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
pub(super) fn sanitize_basename(raw: &str) -> String {
    let first_token = raw.split_whitespace().next().unwrap_or("");
    let p = Path::new(first_token);
    p.file_name()
        .and_then(|n| n.to_str())
        .filter(|s| !s.is_empty())
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
