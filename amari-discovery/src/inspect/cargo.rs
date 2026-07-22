// SPDX-License-Identifier: MIT OR Apache-2.0

//! Offline Cargo dependency inspection for Amari project compatibility.
//!
//! This module provides a deterministic, read-only TOML-only parser that
//! inspects Cargo manifests and lockfiles to discover Amari package
//! dependencies (`amari` or `amari-*`) and assess their version
//! compatibility against the embedded Amari catalog.
//!
//! # Module structure
//!
//! - [`types`] — All public domain types re-exported at this module's root.
//! - `manifest` — Manifest parsing internals (not public).
//! - `lock` — Lockfile parsing and compatibility resolution (not public).
//! - `toml` — TOML value extraction helpers (not public).
//!
//! # Safety
//!
//! - No Cargo, rustc, build-script, network, or shell execution
//! - No symlink following — uses Task7 nofollow cross-platform open
//! - No absolute-path leakage in warnings or errors
//! - Bounded reads using [`InspectionLimits`] with per-file and aggregate
//!   enforcement; no `read_to_string`; `unwrap_or_default` is only used
//!   on already-parsed TOML table values, never on I/O or parse results
//! - Never stores full manifest or lockfile content
//! - TOML parser errors never leak source snippets

mod lock;
mod manifest;
mod toml_helpers;
pub mod types;

pub use types::{
    AmariDependencyEvidence, CargoBench, CargoDependencyRecord, CargoInspection,
    CargoInspectionWarning, CargoLock, CargoPackage, DependencyKind, LockedPackage, ManifestSource,
    NativeLink, SystemDependencyKind, SystemDependencySignal, WorkspaceDependencyBase,
    WorkspaceMeta,
};

use std::collections::BTreeSet;
use std::path::{Component, Path};
use std::time::Instant;

use sha2::{Digest, Sha256};

use crate::error::{DiscoveryError, DiscoveryResult};
use crate::inspect::snapshot::{InspectionLimit, SnapshotState};
use crate::inspect::InspectionLimits;

use super::{bounded_read, nofollow_open_readonly, BoundedOutcome, NofollowResult};

// ============================================================================
// Provenance: deterministic input framing
// ============================================================================

/// Accumulator for accepted inspection inputs (manifests + lockfile).
#[derive(Default)]
pub(super) struct ProvenanceAccumulator {
    entries: Vec<(String, Vec<u8>)>,
    pub file_count: u64,
    pub total_bytes: u64,
}

impl ProvenanceAccumulator {
    /// Record an accepted file by relative path and byte content.
    pub fn accept(&mut self, path: &str, content: &[u8]) {
        self.file_count = self.file_count.saturating_add(1);
        self.total_bytes = self.total_bytes.saturating_add(content.len() as u64);
        self.entries.push((path.to_string(), content.to_vec()));
    }

    /// Compute deterministic framed SHA-256 hash over all accepted files
    /// sorted by relative path. Framing: u32 LE path len, raw path bytes,
    /// u64 LE content len, raw content bytes.
    pub fn compute_hash(&self) -> String {
        let mut sorted: Vec<&(String, Vec<u8>)> = self.entries.iter().collect();
        sorted.sort_by(|a, b| a.0.cmp(&b.0));

        let mut hasher = Sha256::new();
        for (path, content) in &sorted {
            hasher.update((path.len() as u32).to_le_bytes());
            hasher.update(path.as_bytes());
            hasher.update((content.len() as u64).to_le_bytes());
            hasher.update(content);
        }
        hex::encode(hasher.finalize())
    }

    /// Compute file content hash.
    pub fn content_hash(content: &[u8]) -> String {
        hex::encode(Sha256::digest(content))
    }

    /// Build a [`ManifestSource`] for a given relative path and content.
    pub fn make_source(&self, path: &str, content: &[u8], line: Option<usize>) -> ManifestSource {
        ManifestSource {
            path: path.to_string(),
            line,
            content_hash: Self::content_hash(content),
            byte_count: content.len() as u64,
        }
    }
}

// ============================================================================
// Safe file reading with limits
// ============================================================================

/// Outcome of a bounded manifest/lock read.
enum ReadOutcome {
    /// File read successfully within limits.
    Ok(Vec<u8>),
    /// Path was a symlink or replacement race.
    Symlink,
    /// Path doesn't exist.
    NotFound,
    /// Per-file limit exceeded.
    PerFileExceeded,
    /// Aggregate limit would be exceeded.
    AggregateExceeded,
    /// I/O error.
    IoErr(std::io::Error),
}

/// Safely read a file for inspection: no symlink following, bounded reads
/// against per-file and aggregate limits. Returns the raw bytes on success.
fn safe_read_file(
    file_path: &Path,
    limits: &InspectionLimits,
    aggregate_so_far: u64,
) -> ReadOutcome {
    let file = match nofollow_open_readonly(file_path) {
        Ok(NofollowResult::Opened(f)) => f,
        Ok(NofollowResult::SymlinkOrRace) => return ReadOutcome::Symlink,
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                return ReadOutcome::NotFound;
            }
            return ReadOutcome::IoErr(e);
        }
    };

    let remaining = limits.max_inspection_bytes.saturating_sub(aggregate_so_far);
    let mut reader = std::io::BufReader::new(file);

    match bounded_read(&mut reader, limits.max_per_file_bytes, remaining) {
        Ok(BoundedOutcome::Accepted(bytes)) => {
            let len = bytes.len() as u64;
            // bounded_read already ensures len <= per_file_max
            if aggregate_so_far.saturating_add(len) > limits.max_inspection_bytes {
                ReadOutcome::AggregateExceeded
            } else {
                ReadOutcome::Ok(bytes)
            }
        }
        Ok(BoundedOutcome::PerFileExceeded) => ReadOutcome::PerFileExceeded,
        Err(e) => ReadOutcome::IoErr(e),
    }
}

// ============================================================================
// Workspace member path validation
// ============================================================================

/// Normalize a workspace member path. Rejects glob patterns, absolute paths,
/// parent/current components, non-UTF-8 components, or empty strings.
fn invalid_member_hint(member: &str) -> String {
    if member.is_empty() {
        "<empty>".to_owned()
    } else if member.contains('*') || member.contains('?') {
        "<glob>".to_owned()
    } else if Path::new(member).is_absolute() {
        "<absolute>".to_owned()
    } else if Path::new(member)
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        "<parent-or-current>".to_owned()
    } else {
        "<invalid>".to_owned()
    }
}

fn normalize_member_path(member: &str) -> Option<String> {
    if member.is_empty() || member.contains('*') || member.contains('?') {
        return None;
    }
    let path = Path::new(member);
    if path.is_absolute() {
        return None;
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_str()?),
            Component::ParentDir
            | Component::RootDir
            | Component::Prefix(_)
            | Component::CurDir => {
                return None;
            }
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

// ============================================================================
// Wall-clock check
// ============================================================================

/// Check wall-clock limit. Returns Err if exceeded.
fn check_wall_clock(start: Instant, limits: &InspectionLimits) -> DiscoveryResult<()> {
    let elapsed = start.elapsed().as_millis() as u64;
    if elapsed >= limits.max_inspection_wall_millis {
        Err(DiscoveryError::LimitExceeded(format!(
            "wall-clock limit {}ms exceeded after {}ms",
            limits.max_inspection_wall_millis, elapsed
        )))
    } else {
        Ok(())
    }
}

// ============================================================================
// Top-level inspection
// ============================================================================

/// Inspects a Cargo project root for Amari dependencies and compatibility
/// without invoking Cargo, rustc, build scripts, or the network.
///
/// This function:
/// - Parses the root `Cargo.toml` (and `Cargo.lock` if present) using the
///   `toml` crate — never shelling out to Cargo.
/// - Resolves workspace members, inheritance, and `[[bench]]`/`links` signals.
/// - Reports only Amari dependencies (`amari` or `amari-*`).
/// - Compares resolved exact versions from the lockfile against the embedded
///   catalog version to produce typed [`crate::protocol::Compatibility`] verdicts.
/// - Enforces all [`InspectionLimits`]: per-file bytes, aggregate bytes,
///   wall-clock time. No symlinks are followed.
///
/// # Safety
///
/// This function is read-only. It never writes files, executes project code,
/// accesses the network, or follows symlinks. Paths in warnings and errors
/// are always relative to the project root. TOML error messages never contain
/// source snippets.
///
/// # State semantics
///
/// On `Ok`, the root manifest is **always** fully parsed in
/// [`CargoInspection::root_package`]. If the root manifest is missing,
/// unreadable, or malformed, this function returns `Err`.
///
/// [`CargoInspection::state`] signals whether optional evidence is partial:
/// [`SnapshotState::LimitExceeded`] means one or more workspace members or
/// the lockfile hit a resource limit. The root and any already-parsed members
/// are still present and internally consistent.
///
/// # Errors
///
/// Returns [`DiscoveryError::LimitExceeded`] when the root manifest cannot be
/// read within limits, or [`DiscoveryError::InspectionFailure`] when the root
/// manifest is missing, unreadable, or contains malformed TOML that prevents
/// basic package identification.
pub fn inspect_cargo_project(
    root: &Path,
    limits: &InspectionLimits,
) -> DiscoveryResult<CargoInspection> {
    let catalog = crate::Catalog::embedded()?;
    let catalog_version = catalog.version().to_string();
    let mut warnings: Vec<CargoInspectionWarning> = Vec::new();
    let mut provenance = ProvenanceAccumulator::default();

    // Validate root is a directory
    if !root.is_dir() {
        return Err(DiscoveryError::InspectionFailure(
            "project root is not a directory".to_owned(),
        ));
    }

    // Canonicalize root for containment checks without exposing its path.
    let root_canon = root.canonicalize().map_err(|_| {
        DiscoveryError::InspectionFailure("cannot canonicalize project root".to_owned())
    })?;

    let start = Instant::now();

    // ---- Read root Cargo.toml with limits ----
    let root_manifest_path = root_canon.join("Cargo.toml");

    // Wall-clock check
    check_wall_clock(start, limits)?;

    let root_raw = match safe_read_file(&root_manifest_path, limits, 0) {
        ReadOutcome::Ok(bytes) => bytes,
        ReadOutcome::Symlink => {
            return Err(DiscoveryError::InspectionFailure(
                "root Cargo.toml is a symlink and cannot be inspected".into(),
            ));
        }
        ReadOutcome::NotFound => {
            return Err(DiscoveryError::InspectionFailure(
                "root Cargo.toml not found".into(),
            ));
        }
        ReadOutcome::PerFileExceeded | ReadOutcome::AggregateExceeded => {
            return Err(DiscoveryError::LimitExceeded(
                "root Cargo.toml exceeds byte limits".to_string(),
            ));
        }
        ReadOutcome::IoErr(e) => {
            return Err(DiscoveryError::InspectionFailure(format!(
                "cannot read root Cargo.toml: {}",
                e
            )));
        }
    };

    // Record root manifest for provenance
    provenance.accept("Cargo.toml", &root_raw);

    // ---- Single-pass root parsing: parse TOML once ----
    let root_str = std::str::from_utf8(&root_raw).map_err(|_| {
        DiscoveryError::InspectionFailure("root Cargo.toml is not valid UTF-8".into())
    })?;

    let root_manifest: toml::Value = toml::from_str(root_str).map_err(|e| {
        let (line, col) = toml_helpers::toml_line_col_from_source(&e, &root_raw);
        let reason = toml_helpers::toml_malformed_reason(&e);
        DiscoveryError::InspectionFailure(format!(
            "{} in root Cargo.toml at line {:?} col {:?}",
            reason, line, col
        ))
    })?;

    // Extract workspace metadata from the once-parsed root manifest
    let ws_bases = root_manifest
        .get("workspace")
        .and_then(|v| v.as_table())
        .map(manifest::parse_workspace_deps)
        .unwrap_or_default();

    let ws_package_fields = root_manifest
        .get("workspace")
        .and_then(|v| v.as_table())
        .map(manifest::parse_workspace_package_fields)
        .unwrap_or_default();

    // Parse root package from the already-parsed manifest value (no re-parse)
    let parsed_root = manifest::parse_manifest_from_value(
        &root_manifest,
        &root_raw,
        "Cargo.toml",
        true,
        &ws_bases,
        &ws_package_fields,
        &mut warnings,
        &provenance,
    )?;

    let mut root_pkg = parsed_root.package;
    let mut ws_meta = parsed_root.ws_meta;

    // ---- Read Cargo.lock with limits ----
    let lock_path = root_canon.join("Cargo.lock");
    let mut aggregate_bytes = root_raw.len() as u64;

    let (parsed_lock, lock_content_bytes) =
        match safe_read_file(&lock_path, limits, aggregate_bytes) {
            ReadOutcome::Ok(bytes) => {
                aggregate_bytes = aggregate_bytes.saturating_add(bytes.len() as u64);
                provenance.accept("Cargo.lock", &bytes);
                let parsed = lock::parse_lock(&bytes, "Cargo.lock");
                (Some(parsed), Some(bytes))
            }
            ReadOutcome::Symlink => {
                warnings.push(CargoInspectionWarning::SymlinkedManifest {
                    path: "Cargo.lock".to_string(),
                });
                (None, None)
            }
            ReadOutcome::NotFound => {
                warnings.push(CargoInspectionWarning::MissingLock {
                    path: "Cargo.lock".to_string(),
                });
                (None, None)
            }
            ReadOutcome::PerFileExceeded | ReadOutcome::AggregateExceeded => {
                warnings.push(CargoInspectionWarning::LimitExceeded {
                    limit: InspectionLimit::TotalBytes {
                        max: limits.max_inspection_bytes,
                        observed: aggregate_bytes,
                    },
                });
                (None, None)
            }
            ReadOutcome::IoErr(_) => {
                warnings.push(CargoInspectionWarning::MissingLock {
                    path: "Cargo.lock".to_string(),
                });
                (None, None)
            }
        };

    // Collect lock-specific warnings and build CargoLock
    let lock = parsed_lock.as_ref().map(|pl| {
        warnings.extend(pl.warnings.clone());
        CargoLock {
            path: "Cargo.lock".to_string(),
            packages: pl.packages.clone(),
            source: provenance.make_source(
                "Cargo.lock",
                lock_content_bytes.as_deref().unwrap_or(b""),
                None,
            ),
        }
    });

    // ---- Resolve compatibility for root deps ----
    lock::resolve_compatibility(
        &mut root_pkg.dependencies,
        parsed_lock.as_ref(),
        &catalog_version,
        "Cargo.lock",
        lock_content_bytes.as_deref().unwrap_or(b""),
        &provenance,
    );

    // ---- Process workspace members ----
    let mut members: Vec<CargoPackage> = Vec::new();
    let mut member_state: Option<SnapshotState> = None;

    if let Some(ref meta) = ws_meta {
        // Normalize, validate, and deduplicate member paths before any I/O.
        let mut valid_members = BTreeSet::new();
        for member in &meta.members {
            let Some(normalized) = normalize_member_path(member) else {
                warnings.push(CargoInspectionWarning::IllegalMemberPath {
                    member: invalid_member_hint(member),
                });
                continue;
            };
            valid_members.insert(normalized);
        }

        for member_dir_name in valid_members {
            // Wall-clock check before each member
            if let Err(_e) = check_wall_clock(start, limits) {
                member_state = Some(SnapshotState::LimitExceeded {
                    limit: InspectionLimit::WallClock {
                        max_millis: limits.max_inspection_wall_millis,
                        observed_millis: start.elapsed().as_millis() as u64,
                    },
                });
                warnings.push(CargoInspectionWarning::LimitExceeded {
                    limit: InspectionLimit::WallClock {
                        max_millis: limits.max_inspection_wall_millis,
                        observed_millis: start.elapsed().as_millis() as u64,
                    },
                });
                break;
            }

            let member_dir = root_canon.join(&member_dir_name);

            // Check if member directory itself is a symlink
            if std::fs::symlink_metadata(&member_dir)
                .map(|m| m.file_type().is_symlink())
                .unwrap_or(false)
            {
                warnings.push(CargoInspectionWarning::SymlinkedManifest {
                    path: format!("{}/Cargo.toml", member_dir_name),
                });
                continue;
            }

            let member_manifest = member_dir.join("Cargo.toml");

            // Check containment of member dir
            if match member_dir.canonicalize() {
                Ok(c) => !c.starts_with(&root_canon),
                Err(_) => true,
            } {
                warnings.push(CargoInspectionWarning::EscapingManifest {
                    path: format!("{}/Cargo.toml", member_dir_name),
                });
                continue;
            }

            // Check file-count limit
            if provenance.file_count >= limits.max_inspection_files {
                member_state = Some(SnapshotState::LimitExceeded {
                    limit: InspectionLimit::FileCount {
                        max: limits.max_inspection_files,
                        observed: provenance.file_count,
                    },
                });
                warnings.push(CargoInspectionWarning::LimitExceeded {
                    limit: InspectionLimit::FileCount {
                        max: limits.max_inspection_files,
                        observed: provenance.file_count,
                    },
                });
                break;
            }

            // Check aggregate limit before reading
            if aggregate_bytes >= limits.max_inspection_bytes {
                member_state = Some(SnapshotState::LimitExceeded {
                    limit: InspectionLimit::TotalBytes {
                        max: limits.max_inspection_bytes,
                        observed: aggregate_bytes,
                    },
                });
                warnings.push(CargoInspectionWarning::LimitExceeded {
                    limit: InspectionLimit::TotalBytes {
                        max: limits.max_inspection_bytes,
                        observed: aggregate_bytes,
                    },
                });
                break;
            }

            let member_rel_manifest = format!("{}/Cargo.toml", member_dir_name);

            match safe_read_file(&member_manifest, limits, aggregate_bytes) {
                ReadOutcome::Ok(bytes) => {
                    aggregate_bytes = aggregate_bytes.saturating_add(bytes.len() as u64);
                    provenance.accept(&member_rel_manifest, &bytes);

                    match manifest::parse_manifest(
                        &bytes,
                        &member_rel_manifest,
                        false,
                        &ws_bases,
                        &ws_package_fields,
                        &mut warnings,
                        &provenance,
                    ) {
                        Ok(parsed_member) => {
                            if parsed_member.declares_workspace {
                                warnings.push(CargoInspectionWarning::NestedWorkspaceRoot {
                                    path: member_rel_manifest,
                                });
                                continue;
                            }
                            let mut member_pkg = parsed_member.package;
                            lock::resolve_compatibility(
                                &mut member_pkg.dependencies,
                                parsed_lock.as_ref(),
                                &catalog_version,
                                "Cargo.lock",
                                lock_content_bytes.as_deref().unwrap_or(b""),
                                &provenance,
                            );
                            members.push(member_pkg);
                        }
                        Err(e) => {
                            // Extract sanitized reason from the parse error.
                            // parse_manifest already canonicalizes the error
                            // through toml_malformed_reason, so we strip the
                            // path suffix and rebuild a clean MalformedManifest.
                            let raw = e.to_string();
                            // Format: "<reason> at sub/Cargo.toml line Some(2) col Some(5)"
                            let reason = raw
                                .split(" at ")
                                .next()
                                .unwrap_or("failed to parse member manifest")
                                .to_string();
                            let (line, col) = toml_helpers::toml_line_col_from_manifest_path(&raw);
                            warnings.push(CargoInspectionWarning::MalformedManifest {
                                path: member_rel_manifest,
                                reason,
                                line,
                                column: col,
                            });
                        }
                    }
                }
                ReadOutcome::Symlink => {
                    warnings.push(CargoInspectionWarning::SymlinkedManifest {
                        path: member_rel_manifest,
                    });
                }
                ReadOutcome::NotFound => {
                    warnings.push(CargoInspectionWarning::MissingManifest {
                        path: member_rel_manifest,
                    });
                }
                ReadOutcome::PerFileExceeded => {
                    warnings.push(CargoInspectionWarning::LimitExceeded {
                        limit: InspectionLimit::TotalBytes {
                            max: limits.max_per_file_bytes,
                            observed: limits.max_per_file_bytes.saturating_add(1),
                        },
                    });
                }
                ReadOutcome::AggregateExceeded => {
                    member_state = Some(SnapshotState::LimitExceeded {
                        limit: InspectionLimit::TotalBytes {
                            max: limits.max_inspection_bytes,
                            observed: aggregate_bytes,
                        },
                    });
                    warnings.push(CargoInspectionWarning::LimitExceeded {
                        limit: InspectionLimit::TotalBytes {
                            max: limits.max_inspection_bytes,
                            observed: aggregate_bytes,
                        },
                    });
                    break;
                }
                ReadOutcome::IoErr(_) => {
                    warnings.push(CargoInspectionWarning::MissingManifest {
                        path: member_rel_manifest,
                    });
                }
            }
        }
    }

    // Keep only normalized, valid, deduplicated member paths in public evidence.
    if let Some(meta) = &mut ws_meta {
        meta.members = meta
            .members
            .iter()
            .filter_map(|member| normalize_member_path(member))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
    }

    // Sort members for determinism
    members.sort_by(|a, b| a.name.cmp(&b.name));

    // Compute final state
    let final_state = member_state.unwrap_or(SnapshotState::Complete);

    // Compute input hash
    let input_hash = provenance.compute_hash();

    Ok(CargoInspection {
        root_package: root_pkg,
        workspace_members: members,
        lock,
        workspace_meta: ws_meta,
        warnings,
        input_hash,
        state: final_state,
        inspected_file_count: provenance.file_count,
        total_bytes: provenance.total_bytes,
    })
}
