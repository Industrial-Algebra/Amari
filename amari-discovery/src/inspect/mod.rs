// SPDX-License-Identifier: MIT OR Apache-2.0

//! Read-only filesystem project inspection with deterministic hashing.
//!
//! The module provides a reusable entry point [`inspect_project`] and
//! the [`ProjectInspector`] trait for future task composition.
//!
//! ## Safety guarantees
//!
//! - **Read-only**: No file writes, no Cargo execution, no network access.
//! - **No secrets**: Environment directories (`*/.env*`) are pruned before
//!   descent and environment files (`.env*`) are excluded. Warnings never
//!   contain file content.
//! - **Symlink containment**: Symbolic links are never followed. Symlink
//!   entries produce safe, path-only warnings.
//! - **Deterministic hashing**: SHA-256 over sorted normalized relative
//!   paths + accepted file bytes with unambiguous framing. No absolute
//!   paths, mtimes, inodes, or directory ordering.
//! - **Resource bounded**: Enforces file-count, byte, depth, per-file,
//!   and wall-clock limits. Bounded reads prevent metadata/read races
//!   from exceeding byte limits.

pub mod cargo;
pub mod cargo_config;
mod limits;
pub mod rust;
mod snapshot;

use std::fs;
use std::io::Read;
use std::path::{Component, Path};
use std::time::Instant;

use sha2::{Digest, Sha256};

use crate::error::{DiscoveryError, DiscoveryResult};
use crate::protocol::{CatalogIdentity, Compatibility, Envelope, ReplayMetadata};
use crate::Catalog;

pub use cargo::{
    inspect_cargo_project, AmariDependencyEvidence, CargoBench, CargoDependencyRecord,
    CargoInspection, CargoInspectionWarning, CargoLock, CargoPackage, DependencyKind,
    LockedPackage, ManifestSource, NativeLink, SystemDependencyKind, SystemDependencySignal,
    WorkspaceDependencyBase, WorkspaceMeta,
};
/// Cooperative wall-clock-gated inspection with an injectable elapsed closure.
///
/// `#[doc(hidden)]`: internal testing surface for the post-phase wall-clock
/// checkpoints; not part of the stable public API.
#[doc(hidden)]
pub use cargo_config::inspect_cargo_platform_with_elapsed;
pub use cargo_config::{
    inspect_cargo_platform, BenchmarkEvidence, BenchmarkStatus, CargoBuildSettings,
    CargoPlatformInspection, CargoPlatformWarning, CargoTargetKey, CargoTargetSettings,
    ConfigInputProvenance, ConfigSetting, ConfigSettingIssue, ConfigSource, ConfiguredLinker,
    ConfiguredRunner, CustomTargetEvidence, NativeRequirement, NoStdEvidence, NoStdPackageEvidence,
    RustflagCategory, RustflagCategoryCount, RustflagsEvidence, RustflagsScope,
    TargetCfgConstraint, TargetCfgSource, WasmTargetEvidence, WasmTargetOrigin,
};
pub use limits::InspectionLimits;
pub use rust::{
    inspect_rust_sources, RustCfgEvidence, RustCrateAttribute, RustFileKind, RustInspectionWarning,
    RustSourceInspection, RustUsage, RustUsageKind, VocabularyEvidence,
};
pub use snapshot::{
    InspectionLimit, ProjectInspector, ProjectKind, ProjectSignal, ProjectSnapshot, SnapshotState,
    SourceLocation,
};

// ---------------------------------------------------------------------------
// Directory patterns pruned before descent (WalkDir filter_entry)
// ---------------------------------------------------------------------------

/// Returns `true` when a directory name should be pruned from traversal.
pub(super) fn is_skipped_dir_name(name: &str) -> bool {
    // Exact match on well-known build / cache / version-control directories.
    name == ".git" || name == "target" || name == "node_modules" || name == ".worktrees"
}

/// Returns `true` when a directory or file name matches an environment-secret
/// pattern (any name starting with `.env`).
pub(super) fn is_env_secret_name(name: &str) -> bool {
    name.starts_with(".env")
}

// ---------------------------------------------------------------------------
// No-follow read-only open — prevents symlink replacement races
// ---------------------------------------------------------------------------

/// Result of opening a file with [`nofollow_open_readonly`].
#[derive(Debug)]
pub(super) enum NofollowResult {
    /// The file was opened successfully and is a regular file.
    Opened(fs::File),
    /// The path was a symlink (or a replacement race was detected) —
    /// the caller should emit a safe path-only warning and skip.
    SymlinkOrRace,
}

/// Opens a file for read-only access without ever following symlinks.
///
/// # Platform behaviour
///
/// - **Unix**: Opens with `O_NOFOLLOW | O_CLOEXEC | O_RDONLY` via rustix.
///   Symlink / dangling-symlink errors (`ELOOP`, `ENOTDIR`) are mapped to
///   [`NofollowResult::SymlinkOrRace`]. A pre-open metadata identity check
///   (dev + ino) catches replacement races between stat and open. The
///   opened descriptor is verified to be a regular file.
/// - **Windows**: Opens with `FILE_FLAG_OPEN_REPARSE_POINT`. If the opened
///   file has the `FILE_ATTRIBUTE_REPARSE_POINT` attribute, it is rejected
///   as a symlink / junction. Only regular files are accepted.
/// - **Other targets**: Conservative fallback using `std::fs::File::open`
///   with a post-open `is_file()` check. Symlinks may be followed on these
///   platforms; the regular-file check is a best-effort safeguard.
///
/// The caller is responsible for path-based containment checks before
/// calling this function. Errors from this function never expose external
/// symlink targets or absolute paths.
pub(super) fn nofollow_open_readonly(path: &Path) -> std::io::Result<NofollowResult> {
    #[cfg(unix)]
    {
        use rustix::fs::{open, Mode, OFlags};
        use std::os::unix::fs::MetadataExt as _;

        // Capture pre-open identity for replacement-race detection.
        let pre_identity = match fs::metadata(path) {
            Ok(m) => Some((m.dev(), m.ino())),
            Err(_) => None,
        };

        let fd = match open(
            path,
            OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::RDONLY,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR) => {
                return Ok(NofollowResult::SymlinkOrRace);
            }
            Err(err) => return Err(err.into()),
        };

        // Safe conversion: rustix OwnedFd → std File.
        // rustix re-exports std::os::unix::io::OwnedFd, so fd is already a
        // std OwnedFd. `From<OwnedFd> for File` (stable since Rust 1.63)
        // provides the safe conversion path.
        let file: fs::File = fd.into();

        let metadata = file.metadata()?;

        // Reject non-regular files (e.g. device nodes opened by accident).
        if !metadata.file_type().is_file() {
            return Ok(NofollowResult::SymlinkOrRace);
        }

        // Replacement-race detection: if dev / inode changed between the
        // pre-open stat and the post-open fstat, the entry was swapped.
        if let Some((pre_dev, pre_ino)) = pre_identity {
            if metadata.dev() != pre_dev || metadata.ino() != pre_ino {
                return Ok(NofollowResult::SymlinkOrRace);
            }
        }

        Ok(NofollowResult::Opened(file))
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::{MetadataExt as _, OpenOptionsExt};

        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0400;

        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .open(path)?;

        let metadata = file.metadata()?;
        let attrs = metadata.file_attributes();

        if (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
            return Ok(NofollowResult::SymlinkOrRace);
        }

        if !metadata.is_file() {
            return Ok(NofollowResult::SymlinkOrRace);
        }

        Ok(NofollowResult::Opened(file))
    }

    #[cfg(not(any(unix, windows)))]
    {
        // Conservative fallback with post-open regular-file check.
        // Symlinks may be followed on these platforms.
        let file = fs::File::open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() {
            return Ok(NofollowResult::SymlinkOrRace);
        }
        Ok(NofollowResult::Opened(file))
    }
}

fn to_usize(value: u64) -> DiscoveryResult<usize> {
    usize::try_from(value).map_err(|_| {
        DiscoveryError::InspectionFailure(format!(
            "value {value} exceeds platform addressable size"
        ))
    })
}

// ---------------------------------------------------------------------------
// Bounded read — prevents metadata/read races from exceeding limits
// ---------------------------------------------------------------------------

/// Outcome of [`bounded_read`].
pub(super) enum BoundedOutcome {
    /// File content fits within the per-file byte limit.
    Accepted(Vec<u8>),
    /// File content exceeds the per-file byte limit.
    /// The caller should issue a per-file-oversize warning and skip the file.
    PerFileExceeded,
}

/// Read at most `min(per_file_max, remaining_aggregate) + 1` bytes from
/// `reader`.
///
/// The extra byte allows the caller to distinguish a per-file oversize
/// from content that merely exhausts the remaining aggregate allowance.
///
/// - Returns [`BoundedOutcome::Accepted`] when the bytes read fit within
///   `per_file_max`. The caller must still check whether the returned
///   content fits within the remaining aggregate allowance.
/// - Returns [`BoundedOutcome::PerFileExceeded`] when the bytes read
///   exceed `per_file_max`. The file should be skipped with a warning
///   (no [`InspectionLimit`]-style partial state).
///
/// Never allocates or reads beyond the declared bounds. All arithmetic
/// uses saturating/checked operations to prevent overflow.
pub(super) fn bounded_read<R: Read>(
    reader: &mut R,
    per_file_max: u64,
    remaining_aggregate: u64,
) -> std::io::Result<BoundedOutcome> {
    let effective = per_file_max.min(remaining_aggregate);
    let max_read = effective.saturating_add(1);
    let mut content = Vec::new();
    let bytes_read = reader.take(max_read).read_to_end(&mut content)?;
    let len = bytes_read as u64;
    if len > per_file_max {
        Ok(BoundedOutcome::PerFileExceeded)
    } else {
        Ok(BoundedOutcome::Accepted(content))
    }
}

// ---------------------------------------------------------------------------
// Normalized relative path
// ---------------------------------------------------------------------------

/// Errors produced by [`normalized_relative_path`].
enum NormalizeError {
    /// The entry path cannot be stripped against the root.
    StripFailed,
    /// A path component contains non-UTF-8 bytes.
    NonUtf8,
}

/// Convert an absolute `entry_path` into a normalized `/`-separated
/// relative path with validated UTF-8 components.
///
/// Returns an error if the prefix cannot be stripped or any component
/// contains non-UTF-8 bytes — both cases produce warnings in the caller
/// instead of lossy fallbacks.
fn normalized_relative_path(entry_path: &Path, root: &Path) -> Result<String, NormalizeError> {
    let rel = entry_path
        .strip_prefix(root)
        .map_err(|_| NormalizeError::StripFailed)?;

    let mut parts: Vec<&str> = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(os_str) => match os_str.to_str() {
                Some(s) => parts.push(s),
                None => return Err(NormalizeError::NonUtf8),
            },
            _ => return Err(NormalizeError::NonUtf8),
        }
    }
    // An empty parts list means the entry is the root itself (a directory,
    // which should not reach this code path). Return "." as a safe default.
    if parts.is_empty() {
        Ok(".".into())
    } else {
        Ok(parts.join("/"))
    }
}

// ---------------------------------------------------------------------------
// Hashing helpers
// ---------------------------------------------------------------------------

/// Compute a framed SHA-256 hash over sorted file paths and their bytes.
///
/// Framing: `u32` LE path byte length, raw path bytes, `u64` LE content
/// byte length, raw content bytes. Only files that passed all limit checks
/// are included.
///
/// # Empty project hash
///
/// When the inspected project contains no accepted files (all entries are
/// skipped due to secrets, symlinks, size limits, or non-UTF-8 names), the
/// returned hash is the SHA-256 of the empty framed input set — i.e.
/// `SHA-256("")`. This is consistent and deterministic: two empty projects
/// always produce the same hash, and an empty project never collides with a
/// non-empty one (the empty input is a distinct message).
///
/// # Sorting
///
/// Entries are explicitly sorted by normalized path before hashing. This
/// guarantees determinism regardless of the filesystem walk order. The
/// filesystem walker (`WalkDir::sort_by_file_name`) provides a separate,
/// earlier sort that stabilises the order entries are *encountered* — this
/// is critical for limit-bound traversals where only the first N accepted
/// files matter and must be consistent across runs.
fn compute_project_hash(entries: &[(String, Vec<u8>)]) -> String {
    let mut sorted: Vec<&(String, Vec<u8>)> = entries.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (path, content) in &sorted {
        let path_len = path.len() as u32;
        let content_len = content.len() as u64;
        hasher.update(path_len.to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update(content_len.to_le_bytes());
        hasher.update(content);
    }
    hex::encode(hasher.finalize())
}

/// Compute the hex-encoded SHA-256 hash of file content.
fn file_content_hash(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

// ---------------------------------------------------------------------------
// Per-file signal classification
// ---------------------------------------------------------------------------

/// Inspects a relative path to classify file-type signals.
///
/// Returns an optional pair of signal kind and count-increment descriptor
/// so callers can accumulate counters without duplicating the match logic.
fn classify_file_signal(path: &str) -> Option<FileSignalKind> {
    if path.ends_with(".rs") {
        Some(FileSignalKind::Rust)
    } else if path.ends_with(".ts") || path.ends_with(".tsx") {
        Some(FileSignalKind::TypeScript)
    } else {
        None
    }
}

enum FileSignalKind {
    Rust,
    TypeScript,
}

// ---------------------------------------------------------------------------
// Traversal stop reason (single exit point)
// ---------------------------------------------------------------------------

/// Why the traversal loop stopped early.
enum StopReason {
    /// Wall-clock limit exceeded.
    WallClock { elapsed_millis: u64 },
    /// Considered-file-count limit exceeded.
    FileCount { considered: u64 },
    /// Aggregate byte limit would be exceeded by the next accepted file.
    TotalBytes { observed: u64 },
}

// ---------------------------------------------------------------------------
// Traversal state builder
// ---------------------------------------------------------------------------

/// Accumulator for the single-pass traversal.
struct TraversalState {
    /// Paths and content of accepted files (relative, normalized `/`).
    accepted_files: Vec<(String, Vec<u8>)>,
    /// Source locations produced.
    source_locations: Vec<SourceLocation>,
    /// Non-fatal warnings.
    warnings: Vec<String>,
    /// Count of accepted Rust source files.
    seen_rust_source: u64,
    /// Count of accepted TypeScript source files.
    seen_ts_source: u64,
    /// Total bytes of accepted content (not metadata).
    total_bytes: u64,
    /// Whether a directory at max depth was pruned.
    saw_depth_prune: bool,
    /// Number of regular non-secret files **considered** (including
    /// oversized and unreadable candidates). Bounded by
    /// `max_inspection_files` before acceptance checks.
    considered_files: u64,
}

impl TraversalState {
    fn new() -> Self {
        Self {
            accepted_files: Vec::new(),
            source_locations: Vec::new(),
            warnings: Vec::new(),
            seen_rust_source: 0,
            seen_ts_source: 0,
            total_bytes: 0,
            saw_depth_prune: false,
            considered_files: 0,
        }
    }

    fn file_count(&self) -> u64 {
        self.accepted_files.len() as u64
    }

    /// Finalize signals from accumulated state.
    fn finalize_signals(&self) -> Vec<ProjectSignal> {
        let mut signals: Vec<ProjectSignal> = Vec::new();

        // Derive manifest signals only from actually accepted files
        for (path, _) in &self.accepted_files {
            match path.as_str() {
                "Cargo.toml" => signals.push(ProjectSignal::CargoManifest),
                "package.json" => signals.push(ProjectSignal::NpmManifest),
                _ => {}
            }
        }

        // Always finalize source-count signals for the accepted subset
        if self.seen_rust_source > 0 {
            signals.push(ProjectSignal::RustSource {
                count: self.seen_rust_source,
            });
        }
        if self.seen_ts_source > 0 {
            signals.push(ProjectSignal::TypeScriptSource {
                count: self.seen_ts_source,
            });
        }

        signals
    }

    /// Classify the project from finalized signals.
    fn classify_project(signals: &[ProjectSignal]) -> ProjectKind {
        let has_cargo = signals
            .iter()
            .any(|s| matches!(s, ProjectSignal::CargoManifest));
        let has_npm = signals
            .iter()
            .any(|s| matches!(s, ProjectSignal::NpmManifest));

        match (has_cargo, has_npm) {
            (true, true) => ProjectKind::Mixed,
            (true, false) => ProjectKind::RustCargo,
            (false, true) => ProjectKind::NpmTypeScript,
            (false, false) => ProjectKind::Unknown,
        }
    }

    /// Build the final snapshot, consuming self.
    ///
    /// Files and accepted entries are explicitly sorted by normalized
    /// path so that signals and hash are deterministic regardless of
    /// filesystem walk order.
    fn into_snapshot(mut self, snapshot_state: SnapshotState) -> ProjectSnapshot {
        // Sort for determinism — not reliant on walker behaviour.
        self.source_locations.sort_by(|a, b| a.path.cmp(&b.path));
        self.accepted_files.sort_by(|a, b| a.0.cmp(&b.0));

        let signals = self.finalize_signals();
        let project_kind = Self::classify_project(&signals);
        let file_count = self.file_count();
        let project_hash = compute_project_hash(&self.accepted_files);

        ProjectSnapshot {
            project_hash,
            project_kind,
            signals,
            cargo: None,
            rust: None,
            platform: None,
            file_count,
            total_bytes: self.total_bytes,
            state: snapshot_state,
            warnings: self.warnings,
            files: self.source_locations,
        }
    }
}

// ---------------------------------------------------------------------------
// Default filesystem inspector
// ---------------------------------------------------------------------------

/// A default filesystem inspector that traverses a project root,
/// detects structural signals from accepted files, and produces a
/// [`ProjectSnapshot`].
struct DefaultInspector;

impl ProjectInspector for DefaultInspector {
    fn inspect(&self, root: &Path, limits: &InspectionLimits) -> DiscoveryResult<ProjectSnapshot> {
        // Validate root is a directory (before canonicalization)
        if !root.is_dir() {
            return Err(DiscoveryError::InspectionFailure(format!(
                "project root is not a directory: {}",
                root.display()
            )));
        }

        // Canonicalize root once
        let root_canon = root.canonicalize().map_err(|err| {
            DiscoveryError::InspectionFailure(format!(
                "cannot canonicalize project root {}: {}",
                root.display(),
                err
            ))
        })?;

        let start = Instant::now();

        // Convert max depth to usize safely
        let max_depth_usize = to_usize(limits.max_traversal_depth)?;

        // Build the walker with filter_entry pruning of ignored directories
        let walker = walkdir::WalkDir::new(&root_canon)
            .follow_links(false)
            .sort_by_file_name()
            .max_depth(max_depth_usize)
            .into_iter()
            .filter_entry(|e| {
                // Never skip the root itself
                if e.depth() == 0 {
                    return true;
                }
                // Prune ignored and env-secret directories before descent.
                // Non-UTF8 directories are pruned; non-UTF8 non-directory
                // entries (files, symlinks) proceed to the traversal loop
                // for safe warning handling.
                if let Some(name) = e.file_name().to_str() {
                    !is_skipped_dir_name(name) && !is_env_secret_name(name)
                } else {
                    // Non-UTF8 name: prune directories, allow files/symlinks through
                    !e.file_type().is_dir()
                }
            });

        let mut state = TraversalState::new();
        let mut stop_reason: Option<StopReason> = None;

        // Single-pass traversal
        for entry in walker {
            // --- wall-clock check before each entry ---
            let elapsed_millis = start.elapsed().as_millis() as u64;
            if elapsed_millis >= limits.max_inspection_wall_millis {
                stop_reason = Some(StopReason::WallClock { elapsed_millis });
                break;
            }

            // Entry iteration errors are sanitized — the WalkDir error
            // potentially contains an absolute path, so we emit a generic
            // message rather than embedding it as-is.
            let entry = entry.map_err(|_| {
                DiscoveryError::InspectionFailure(
                    "cannot read directory entry (root not shown for privacy)".into(),
                )
            })?;

            // --- depth pruning detection ---
            // WalkDir's max_depth yields entries at depth up to max_depth
            // but does not descend into directories at that depth.
            if entry.depth() >= max_depth_usize && entry.file_type().is_dir() {
                state.saw_depth_prune = true;
                continue;
            }

            // --- only regular files; symlinks produce path-only warnings ---
            let ft = entry.file_type();
            if ft.is_symlink() {
                match normalized_relative_path(entry.path(), &root_canon) {
                    Ok(rel_path) => {
                        state
                            .warnings
                            .push(format!("symlink not followed: {rel_path}"));
                    }
                    Err(NormalizeError::NonUtf8) => {
                        state
                            .warnings
                            .push("symlink not followed: path unavailable (non-UTF-8)".into());
                    }
                    Err(NormalizeError::StripFailed) => {
                        state
                            .warnings
                            .push("symlink not followed: path unavailable".into());
                    }
                }
                continue;
            }
            if !ft.is_file() {
                continue;
            }

            // --- env-secret file exclusion ---
            // (directories are pruned by filter_entry; files need explicit check)
            if let Some(name) = entry.file_name().to_str() {
                if is_env_secret_name(name) {
                    continue;
                }
            }

            // --- increment considered count (bounds regular non-secret files
            //     examined, not just accepted ones). Counted BEFORE UTF-8
            //     normalization so non-UTF8 candidates cannot bypass the
            //     considered-file bound. ---
            state.considered_files = state.considered_files.saturating_add(1);

            // --- file-count limit (based on considered, not accepted) ---
            if state.considered_files > limits.max_inspection_files {
                stop_reason = Some(StopReason::FileCount {
                    considered: state.considered_files,
                });
                break;
            }

            // --- canonical containment ---
            let entry_path = entry.path();
            let rel_path = match normalized_relative_path(entry_path, &root_canon) {
                Ok(p) => p,
                Err(NormalizeError::NonUtf8) => {
                    // Safe generic warning — never exposes raw non-UTF8 bytes.
                    state
                        .warnings
                        .push("path contains non-UTF-8 component, skipping".into());
                    continue;
                }
                Err(NormalizeError::StripFailed) => {
                    // Do not embed the absolute entry path in the warning.
                    state
                        .warnings
                        .push("cannot compute relative path, skipping".into());
                    continue;
                }
            };

            let entry_canon = match entry_path.canonicalize() {
                Ok(c) => c,
                Err(_) => {
                    state.warnings.push(format!(
                        "cannot resolve {rel_path}, skipping (filesystem error)"
                    ));
                    continue;
                }
            };
            if !entry_canon.starts_with(&root_canon) {
                state
                    .warnings
                    .push(format!("path escapes root, skipping: {rel_path}"));
                continue;
            }

            // --- metadata (fast pre-check for clearly oversized files) ---
            let metadata = entry.metadata().map_err(|err| {
                DiscoveryError::InspectionFailure(format!("cannot stat {rel_path}: {err}"))
            })?;
            let file_size = metadata.len();

            // --- per-file limit (metadata pre-check) ---
            if file_size > limits.max_per_file_bytes {
                state.warnings.push(format!(
                    "file {rel_path} exceeds max-per-file limit ({file_size} > {} bytes), skipped",
                    limits.max_per_file_bytes
                ));
                continue;
            }

            // --- bounded read (authoritative — catches metadata/read race) ---
            // Use no-follow open to prevent symlink replacement attacks between
            // walk entry inspection and file open.
            let mut file = match nofollow_open_readonly(entry_path) {
                Ok(NofollowResult::Opened(f)) => f,
                Ok(NofollowResult::SymlinkOrRace) => {
                    // Symlink or replacement race detected at open time.
                    // Emit a safe path-only warning — never leak symlink target.
                    state
                        .warnings
                        .push(format!("symlink or replaced entry, skipping: {rel_path}"));
                    continue;
                }
                Err(err) => {
                    return Err(DiscoveryError::InspectionFailure(format!(
                        "cannot open {rel_path}: {err}"
                    )));
                }
            };

            let remaining_aggregate = limits
                .max_inspection_bytes
                .saturating_sub(state.total_bytes);

            let content =
                match bounded_read(&mut file, limits.max_per_file_bytes, remaining_aggregate) {
                    Ok(BoundedOutcome::Accepted(bytes)) => bytes,
                    Ok(BoundedOutcome::PerFileExceeded) => {
                        state.warnings.push(format!(
                            "file {rel_path} grew beyond per-file limit during read, skipped"
                        ));
                        continue;
                    }
                    Err(err) => {
                        return Err(DiscoveryError::InspectionFailure(format!(
                            "cannot read {rel_path}: {err}"
                        )));
                    }
                };

            // --- aggregate byte limit (checked against actual content, not metadata) ---
            let content_len = content.len() as u64;
            let new_total = state
                .total_bytes
                .checked_add(content_len)
                .ok_or_else(|| DiscoveryError::InspectionFailure("byte total overflow".into()))?;
            if new_total > limits.max_inspection_bytes {
                stop_reason = Some(StopReason::TotalBytes {
                    observed: state.total_bytes,
                });
                break;
            }

            // --- accumulate ---
            let content_hash = file_content_hash(&content);

            state.source_locations.push(SourceLocation {
                path: rel_path.clone(),
                line: None,
                column: None,
                content_hash,
            });

            // Classify file signal
            if let Some(kind) = classify_file_signal(&rel_path) {
                match kind {
                    FileSignalKind::Rust => state.seen_rust_source += 1,
                    FileSignalKind::TypeScript => state.seen_ts_source += 1,
                }
            }

            state.total_bytes = new_total;
            state.accepted_files.push((rel_path, content));
        }

        // --- finalize ---
        let final_state = match stop_reason {
            Some(StopReason::WallClock { elapsed_millis }) => SnapshotState::LimitExceeded {
                limit: InspectionLimit::WallClock {
                    max_millis: limits.max_inspection_wall_millis,
                    observed_millis: elapsed_millis,
                },
            },
            Some(StopReason::FileCount { considered }) => SnapshotState::LimitExceeded {
                limit: InspectionLimit::FileCount {
                    max: limits.max_inspection_files,
                    observed: considered,
                },
            },
            Some(StopReason::TotalBytes { observed }) => SnapshotState::LimitExceeded {
                limit: InspectionLimit::TotalBytes {
                    max: limits.max_inspection_bytes,
                    observed,
                },
            },
            None => {
                if state.saw_depth_prune {
                    SnapshotState::LimitExceeded {
                        limit: InspectionLimit::TraversalDepth {
                            max: limits.max_traversal_depth,
                        },
                    }
                } else {
                    SnapshotState::Complete
                }
            }
        };

        Ok(state.into_snapshot(final_state))
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Inspects a project root directory without modifying it.
///
/// Traverses the filesystem, detects structural signals from accepted
/// files, and produces a deterministic [`ProjectSnapshot`] with SHA-256
/// hashing. All resource limits from [`InspectionLimits`] are enforced.
///
/// Resource limits may halt traversal early. When this happens the
/// snapshot is still internally consistent — it carries partial
/// evidence with a [`SnapshotState::LimitExceeded`] state rather than
/// returning an error.
///
/// # Safety
///
/// This function is read-only. It does not write to any file, execute
/// any project code, access the network, or capture environment variables.
///
/// # Errors
///
/// Returns [`DiscoveryError::InspectionFailure`] when the root is not a
/// directory, cannot be read, or cannot be canonicalized.
///
/// ```no_run
/// use amari_discovery::inspect::{inspect_project, InspectionLimits};
/// use std::path::Path;
///
/// let root = Path::new(env!("CARGO_MANIFEST_DIR"));
/// let limits = InspectionLimits::default();
/// let snapshot = inspect_project(root, &limits).unwrap();
/// assert!(!snapshot.project_hash.is_empty());
/// ```
pub fn inspect_project(root: &Path, limits: &InspectionLimits) -> DiscoveryResult<ProjectSnapshot> {
    let inspector = DefaultInspector;
    inspector.inspect(root, limits)
}

/// Composes filesystem, Cargo, Rust-source, and platform evidence for a project.
///
/// Rust/Cargo projects are enriched with typed manifest, API-usage, benchmark,
/// and platform evidence. Other project kinds retain the bounded generic
/// filesystem snapshot; npm/TypeScript enrichment is added in Task 9C.
///
/// # Safety
///
/// This function is read-only and offline. It never invokes Cargo, rustc,
/// runners, linkers, build scripts, project code, a shell, or the network.
///
/// # Errors
///
/// Returns [`DiscoveryError::InspectionFailure`] when a required project input
/// cannot be inspected safely.
pub fn inspect_rust_project(
    root: &Path,
    limits: &InspectionLimits,
) -> DiscoveryResult<ProjectSnapshot> {
    let mut snapshot = inspect_project(root, limits)?;
    if matches!(
        snapshot.project_kind,
        ProjectKind::RustCargo | ProjectKind::Mixed
    ) {
        let cargo = inspect_cargo_project(root, limits)?;
        let rust = inspect_rust_sources(root, &cargo, limits)?;
        let platform = inspect_cargo_platform(root, &cargo, &rust, limits)?;
        if matches!(snapshot.state, SnapshotState::Complete) {
            snapshot.state = [&cargo.state, &rust.state, &platform.state]
                .into_iter()
                .find_map(|state| match state {
                    SnapshotState::Complete => None,
                    SnapshotState::LimitExceeded { .. } => Some(state.clone()),
                })
                .unwrap_or(SnapshotState::Complete);
        }
        snapshot.cargo = Some(cargo);
        snapshot.rust = Some(rust);
        snapshot.platform = Some(platform);
    }
    Ok(snapshot)
}

/// Produces the shared versioned envelope for `amari inspect`.
///
/// The envelope binds the embedded catalog identity to the deterministic
/// project hash and reports aggregate compatibility across resolved Amari
/// dependencies. Matching hashes make the read-only snapshot replayable.
///
/// # Errors
///
/// Returns an inspection error for unsafe/unreadable project inputs or catalog
/// corruption when the embedded catalog cannot be validated.
pub fn inspect_project_envelope(
    root: &Path,
    limits: &InspectionLimits,
) -> DiscoveryResult<Envelope<ProjectSnapshot>> {
    let snapshot = inspect_rust_project(root, limits)?;
    let catalog = Catalog::embedded()?;
    let compatibility = snapshot_compatibility(&snapshot);
    let project_hash = snapshot.project_hash.clone();

    let mut required_hashes = vec!["project_hash".to_string()];
    let mut warnings = snapshot.warnings.clone();
    if let Some(cargo) = &snapshot.cargo {
        required_hashes.push("cargo.input_hash".to_string());
        if !cargo.warnings.is_empty() {
            warnings.push(format!(
                "Cargo inspection reported {} warning(s)",
                cargo.warnings.len()
            ));
        }
    }
    if let Some(rust) = &snapshot.rust {
        required_hashes.push("rust.input_hash".to_string());
        if !rust.warnings.is_empty() {
            warnings.push(format!(
                "Rust source inspection reported {} warning(s)",
                rust.warnings.len()
            ));
        }
    }
    if let Some(platform) = &snapshot.platform {
        required_hashes.push("platform.config_input.input_hash".to_string());
        if !platform.warnings.is_empty() {
            warnings.push(format!(
                "Cargo platform inspection reported {} warning(s)",
                platform.warnings.len()
            ));
        }
    }

    let mut envelope = Envelope::new(
        snapshot,
        CatalogIdentity {
            version: catalog.version().to_string(),
            hash: catalog.content_hash().to_string(),
        },
        compatibility,
        ReplayMetadata {
            replayable: true,
            required_hashes,
            reasons: Vec::new(),
        },
    );
    envelope.provenance.project_hash = Some(project_hash);
    envelope.warnings = warnings;
    Ok(envelope)
}

/// Aggregate dependency compatibility into one project-level verdict.
fn snapshot_compatibility(snapshot: &ProjectSnapshot) -> Compatibility {
    let Some(cargo) = &snapshot.cargo else {
        return Compatibility {
            status: "compatible".to_string(),
            reasons: vec!["no Rust/Cargo dependency compatibility was required".to_string()],
        };
    };

    let dependencies = std::iter::once(&cargo.root_package)
        .chain(cargo.workspace_members.iter())
        .flat_map(|package| package.dependencies.iter());
    let mut saw_dependency = false;
    let mut saw_unknown_version = false;
    let mut reasons = Vec::new();
    for dependency in dependencies {
        saw_dependency = true;
        if dependency.compatibility.status != "applicable" {
            saw_unknown_version = true;
            reasons.extend(dependency.compatibility.reasons.iter().cloned());
        }
    }
    reasons.sort();
    reasons.dedup();

    if saw_unknown_version {
        if reasons.is_empty() {
            reasons.push("one or more Amari dependency versions are unknown".to_string());
        }
        Compatibility {
            status: "unknown_version".to_string(),
            reasons,
        }
    } else if saw_dependency {
        Compatibility {
            status: "applicable".to_string(),
            reasons: vec!["resolved Amari dependencies match the embedded catalog".to_string()],
        }
    } else {
        Compatibility {
            status: "compatible".to_string(),
            reasons: vec!["no direct Amari dependencies were detected".to_string()],
        }
    }
}

// ---------------------------------------------------------------------------
// Unit tests for bounded_read (testable with Cursor)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    // -- bounded_read within limits --

    #[test]
    fn bounded_read_within_both_limits() {
        let data = b"hello";
        let mut cursor = Cursor::new(data.as_ref());
        let result = bounded_read(&mut cursor, 1024, 1024).unwrap();
        assert!(
            matches!(result, BoundedOutcome::Accepted(ref v) if v == b"hello"),
            "small file should be Accepted"
        );
    }

    // -- bounded_read per-file exceeded (per_file_max is the tighter bound) --

    #[test]
    fn bounded_read_per_file_exceeded_tighter() {
        let data = [0u8; 100];
        let mut cursor = Cursor::new(data.as_ref());
        // per_file_max=50, remaining_aggregate=1024 → min=50, read 51 bytes
        let result = bounded_read(&mut cursor, 50, 1024).unwrap();
        assert!(
            matches!(result, BoundedOutcome::PerFileExceeded),
            "51 bytes read > per_file_max of 50"
        );
    }

    // -- bounded_read per-file exceeded with larger remaining_aggregate --

    #[test]
    fn bounded_read_per_file_tighter_than_aggregate() {
        let data = [0u8; 10];
        let mut cursor = Cursor::new(data.as_ref());
        // per_file_max=5 < remaining_aggregate=1024 → min=5, read 6 bytes
        let result = bounded_read(&mut cursor, 5, 1024).unwrap();
        assert!(
            matches!(result, BoundedOutcome::PerFileExceeded),
            "6 bytes read > per_file_max of 5"
        );
    }

    // -- bounded_read aggregate tighter than per-file (returns Accepted
    //    with partial bytes; caller must detect aggregate exhaustion) --

    #[test]
    fn bounded_read_aggregate_tighter_than_per_file() {
        let data = [0u8; 100];
        let mut cursor = Cursor::new(data.as_ref());
        // remaining_aggregate=10 < per_file_max=1024 → min=10, read 11 bytes
        let result = bounded_read(&mut cursor, 1024, 10).unwrap();
        match result {
            BoundedOutcome::Accepted(v) => {
                // 11 bytes read, all within per_file_max=1024
                assert_eq!(v.len(), 11);
                // Caller would detect 11 > 10 (remaining_aggregate) and
                // return LimitExceeded::TotalBytes.
            }
            BoundedOutcome::PerFileExceeded => {
                panic!("11 bytes should not exceed per_file_max of 1024");
            }
        }
    }

    // -- bounded_read stale-small metadata equivalent ---
    // A stream whose actual content is 200 bytes but per_file_max says 1024.
    // remaining_aggregate=100 means we read at most 101 bytes.
    // bounded_read returns Accepted with 101 bytes; caller sees 101 > 100
    // and returns TotalBytes LimitExceeded with the partial snapshot.

    #[test]
    fn bounded_read_stale_small_metadata_aggregate_exhausted() {
        let data = [0u8; 200];
        let mut cursor = Cursor::new(data.as_ref());
        let result = bounded_read(&mut cursor, 1024, 100).unwrap();
        match result {
            BoundedOutcome::Accepted(v) => {
                // min(1024, 100) + 1 = 101 bytes read
                assert_eq!(v.len(), 101);
                // This is the "stale metadata" scenario: the file is
                // actually 200 bytes but we only read 101. The caller
                // checks 101 > 100 remaining_aggregate → aggregate exhaustion.
            }
            _ => panic!("expected Accepted with partial content (stale metadata)"),
        }
    }

    // -- bounded_read with per_file_max=0 (degenerate) --

    #[test]
    fn bounded_read_zero_per_file_max() {
        let data = [0u8; 1];
        let mut cursor = Cursor::new(data.as_ref());
        // per_file_max=0, remaining_aggregate=1024 → min=0, read 1 byte
        let result = bounded_read(&mut cursor, 0, 1024).unwrap();
        assert!(
            matches!(result, BoundedOutcome::PerFileExceeded),
            "1 byte > 0 per_file_max"
        );
    }

    // -- bounded_read empty file --

    #[test]
    fn bounded_read_empty_file() {
        let data: [u8; 0] = [];
        let mut cursor = Cursor::new(data.as_ref());
        let result = bounded_read(&mut cursor, 1024, 1024).unwrap();
        assert!(
            matches!(result, BoundedOutcome::Accepted(ref v) if v.is_empty()),
            "empty file should be Accepted with zero bytes"
        );
    }

    // -- bounded_read exact per_file_max boundary --

    #[test]
    fn bounded_read_exact_per_file_max() {
        let data = [b'x'; 10];
        let mut cursor = Cursor::new(data.as_ref());
        // per_file_max=10 → min=10, read 11 bytes. File only has 10.
        let result = bounded_read(&mut cursor, 10, 1024).unwrap();
        assert!(
            matches!(result, BoundedOutcome::Accepted(ref v) if v.len() == 10),
            "exactly at per_file_max should be Accepted"
        );
    }

    // -- bounded_read file one byte over per_file_max --

    #[test]
    fn bounded_read_one_byte_over_per_file_max() {
        let data = [b'x'; 11];
        let mut cursor = Cursor::new(data.as_ref());
        let result = bounded_read(&mut cursor, 10, 1024).unwrap();
        assert!(
            matches!(result, BoundedOutcome::PerFileExceeded),
            "11 bytes > 10 per_file_max"
        );
    }

    // -- bounded_read both limits are zero --

    #[test]
    fn bounded_read_both_zero_limits() {
        let data = [0u8; 1];
        let mut cursor = Cursor::new(data.as_ref());
        let result = bounded_read(&mut cursor, 0, 0).unwrap();
        assert!(
            matches!(result, BoundedOutcome::PerFileExceeded),
            "1 byte > 0 per_file_max when both are zero"
        );
    }

    // -- bounded_read with remaining_aggregate=0, per_file_max large --

    #[test]
    fn bounded_read_zero_remaining_aggregate_large_per_file() {
        let data = [0u8; 2];
        let mut cursor = Cursor::new(data.as_ref());
        // remaining_aggregate=0 < per_file_max=1024 → min=0, read 1 byte
        let result = bounded_read(&mut cursor, 1024, 0).unwrap();
        match result {
            BoundedOutcome::Accepted(v) => {
                assert_eq!(v.len(), 1); // only 1 byte read
            }
            BoundedOutcome::PerFileExceeded => {
                panic!("1 byte should not exceed per_file_max of 1024");
            }
        }
    }

    // -- empty project hash is SHA-256 of empty input --

    #[test]
    fn empty_project_hash_is_sha256_of_empty_input() {
        let empty_hash = compute_project_hash(&[]);
        let expected = hex::encode(Sha256::digest(b""));
        assert_eq!(empty_hash, expected);
        assert!(!empty_hash.is_empty());

        // Determinism: two calls always match
        let empty_hash2 = compute_project_hash(&[]);
        assert_eq!(empty_hash, empty_hash2);
    }

    // -- SnapshotState JSON roundtrip shape --

    #[test]
    fn snapshot_state_complete_json_roundtrip() {
        let state = SnapshotState::Complete;
        let json = serde_json::to_string(&state).unwrap();
        // Unit variants omit the content field entirely
        assert_eq!(json, r#"{"kind":"complete"}"#);
        let parsed: SnapshotState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, state);
        // Also accepts explicit null detail
        let parsed2: SnapshotState =
            serde_json::from_str(r#"{"kind":"complete","detail":null}"#).unwrap();
        assert_eq!(parsed2, state);
    }

    #[test]
    fn snapshot_state_limit_exceeded_json_roundtrip() {
        let state = SnapshotState::LimitExceeded {
            limit: InspectionLimit::FileCount {
                max: 100,
                observed: 42,
            },
        };
        let json = serde_json::to_string(&state).unwrap();
        let expected = r#"{"kind":"limit_exceeded","detail":{"limit":{"kind":"file_count","detail":{"max":100,"observed":42}}}}"#;
        assert_eq!(json, expected);
        let parsed: SnapshotState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, state);
    }

    #[test]
    fn snapshot_state_all_limit_variants_json_roundtrip() {
        use crate::inspect::InspectionLimit;

        let cases = vec![
            (SnapshotState::Complete, r#"{"kind":"complete"}"#),
            (
                SnapshotState::LimitExceeded {
                    limit: InspectionLimit::TotalBytes {
                        max: 1_000_000,
                        observed: 500_000,
                    },
                },
                r#"{"kind":"limit_exceeded","detail":{"limit":{"kind":"total_bytes","detail":{"max":1000000,"observed":500000}}}}"#,
            ),
            (
                SnapshotState::LimitExceeded {
                    limit: InspectionLimit::PerFileBytes {
                        max: 4096,
                        observed: 4097,
                    },
                },
                r#"{"kind":"limit_exceeded","detail":{"limit":{"kind":"per_file_bytes","detail":{"max":4096,"observed":4097}}}}"#,
            ),
            (
                SnapshotState::LimitExceeded {
                    limit: InspectionLimit::TraversalDepth { max: 8 },
                },
                r#"{"kind":"limit_exceeded","detail":{"limit":{"kind":"traversal_depth","detail":{"max":8}}}}"#,
            ),
            (
                SnapshotState::LimitExceeded {
                    limit: InspectionLimit::WallClock {
                        max_millis: 60_000,
                        observed_millis: 60_001,
                    },
                },
                r#"{"kind":"limit_exceeded","detail":{"limit":{"kind":"wall_clock","detail":{"max_millis":60000,"observed_millis":60001}}}}"#,
            ),
        ];

        for (state, expected_json) in cases {
            let json = serde_json::to_string(&state).unwrap();
            assert_eq!(json, expected_json, "roundtrip failed for {:?}", state);
            let parsed: SnapshotState = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, state, "parsed value mismatch for {:?}", state);
        }
    }

    // -- nofollow helper unit tests (Unix only) --

    #[cfg(unix)]
    #[test]
    fn nofollow_open_rejects_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::TempDir::new().unwrap();
        let target = dir.path().join("real.txt");
        let link = dir.path().join("link_to_real.txt");
        std::fs::write(&target, b"real content").unwrap();
        symlink(&target, &link).unwrap();

        let result = nofollow_open_readonly(&link).unwrap();
        assert!(
            matches!(result, NofollowResult::SymlinkOrRace),
            "nofollow open must reject symlinks, got: {:?}",
            result
        );

        let result = nofollow_open_readonly(&target).unwrap();
        assert!(
            matches!(result, NofollowResult::Opened(_)),
            "nofollow open must allow regular files"
        );
    }

    #[cfg(unix)]
    #[test]
    fn nofollow_open_reads_regular_file() {
        use std::fs;
        use std::io::Read;

        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("data.txt");
        fs::write(&path, b"expected content").unwrap();

        let result = nofollow_open_readonly(&path).unwrap();
        match result {
            NofollowResult::Opened(mut file) => {
                let mut buf = String::new();
                file.read_to_string(&mut buf).unwrap();
                assert_eq!(buf, "expected content");
            }
            NofollowResult::SymlinkOrRace => panic!("regular file should open"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn nofollow_open_rejects_dangling_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempfile::TempDir::new().unwrap();
        let nonexistent = dir.path().join("absent.txt");
        let link = dir.path().join("dangling_link.txt");
        symlink(&nonexistent, &link).unwrap();

        let result = nofollow_open_readonly(&link).unwrap();
        assert!(
            matches!(result, NofollowResult::SymlinkOrRace),
            "nofollow open must reject dangling symlinks"
        );
    }
}
