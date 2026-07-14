// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic, bounded, offline npm package inspection.
//!
//! Task 9A intentionally supports only project-root `package.json` and npm
//! `package-lock.json` schema versions 2 and 3. Yarn and pnpm lockfiles are
//! outside the v0.24 contract and are never opened.
//!
//! # Safety
//!
//! The inspector is read-only and never invokes npm, Node.js, lifecycle
//! scripts, project code, a shell, providers, or the network. Fixed input paths
//! are opened without following symlinks; Unix opens are descriptor-relative
//! and nonblocking so FIFOs and device nodes cannot stall inspection.

pub mod types;

pub use types::{
    NpmDependencyEvidence, NpmDependencyKind, NpmInspection, NpmInspectionWarning, NpmLock,
    NpmLockedPackage, NpmPackage, NpmSource,
};

use std::collections::BTreeSet;
use std::path::Path;
use std::time::Instant;

use sha2::{Digest, Sha256};

use crate::error::{DiscoveryError, DiscoveryResult};
use crate::inspect::snapshot::{InspectionLimit, SnapshotState};
use crate::inspect::{bounded_read, BoundedOutcome, InspectionLimits};
use crate::protocol::Compatibility;

#[cfg(not(unix))]
use crate::inspect::{nofollow_open_readonly, NofollowResult};

const PACKAGE_PATH: &str = "package.json";
const LOCK_PATH: &str = "package-lock.json";
const AMARI_WASM_PACKAGE: &str = "@justinelliottcobb/amari-wasm";

#[derive(Default)]
struct Provenance {
    entries: Vec<(String, Vec<u8>)>,
    file_count: u64,
    total_bytes: u64,
}

impl Provenance {
    fn accept(&mut self, path: &str, bytes: &[u8]) {
        self.entries.push((path.to_string(), bytes.to_vec()));
        self.file_count = self.file_count.saturating_add(1);
        self.total_bytes = self.total_bytes.saturating_add(bytes.len() as u64);
    }

    fn input_hash(&self) -> String {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut hasher = Sha256::new();
        for (path, bytes) in entries {
            hasher.update((path.len() as u32).to_le_bytes());
            hasher.update(path.as_bytes());
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
        hex::encode(hasher.finalize())
    }
}

#[derive(Debug)]
enum OpenOutcome {
    Opened(std::fs::File),
    Missing,
    Symlink,
    Unsupported,
}

#[cfg(unix)]
fn open_input(root: &Path, name: &str) -> std::io::Result<OpenOutcome> {
    use rustix::fs::{open, openat, Mode, OFlags};

    let root_fd = open(
        root,
        OFlags::DIRECTORY | OFlags::NONBLOCK | OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    let fd = match openat(
        &root_fd,
        name,
        OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(OpenOutcome::Missing),
        Err(rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR) => {
            return Ok(OpenOutcome::Symlink);
        }
        Err(error) => return Err(error.into()),
    };
    let file: std::fs::File = fd.into();
    if !file.metadata()?.file_type().is_file() {
        return Ok(OpenOutcome::Unsupported);
    }
    Ok(OpenOutcome::Opened(file))
}

#[cfg(not(unix))]
fn open_input(root: &Path, name: &str) -> std::io::Result<OpenOutcome> {
    match nofollow_open_readonly(&root.join(name)) {
        Ok(NofollowResult::Opened(file)) => Ok(OpenOutcome::Opened(file)),
        Ok(NofollowResult::SymlinkOrRace) => Ok(OpenOutcome::Symlink),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(OpenOutcome::Missing),
        Err(error) => Err(error),
    }
}

enum ReadOutcome {
    Accepted(Vec<u8>),
    PerFileExceeded,
    AggregateExceeded,
    Failed,
}

fn read_bounded(
    mut file: std::fs::File,
    limits: &InspectionLimits,
    aggregate_so_far: u64,
) -> ReadOutcome {
    let remaining = limits.max_inspection_bytes.saturating_sub(aggregate_so_far);
    match bounded_read(&mut file, limits.max_per_file_bytes, remaining) {
        Ok(BoundedOutcome::Accepted(bytes)) => {
            if aggregate_so_far.saturating_add(bytes.len() as u64) > limits.max_inspection_bytes {
                ReadOutcome::AggregateExceeded
            } else {
                ReadOutcome::Accepted(bytes)
            }
        }
        Ok(BoundedOutcome::PerFileExceeded) => ReadOutcome::PerFileExceeded,
        Err(_) => ReadOutcome::Failed,
    }
}

fn source(path: &str, bytes: &[u8]) -> NpmSource {
    NpmSource {
        path: path.to_string(),
        line: None,
        content_hash: hex::encode(Sha256::digest(bytes)),
        byte_count: bytes.len() as u64,
    }
}

fn wall_clock_limit(start: Instant, limits: &InspectionLimits) -> Option<InspectionLimit> {
    let observed = start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;
    (observed >= limits.max_inspection_wall_millis).then_some(InspectionLimit::WallClock {
        max_millis: limits.max_inspection_wall_millis,
        observed_millis: observed,
    })
}

fn parse_reason(error: &serde_json::Error) -> &'static str {
    match error.classify() {
        serde_json::error::Category::Io => "JSON input could not be read",
        serde_json::error::Category::Syntax => "invalid JSON syntax",
        serde_json::error::Category::Data => "invalid JSON value",
        serde_json::error::Category::Eof => "unexpected end of JSON input",
    }
}

fn parse_package(bytes: &[u8], catalog_version: &str) -> DiscoveryResult<NpmPackage> {
    let value: serde_json::Value = serde_json::from_slice(bytes).map_err(|error| {
        DiscoveryError::InspectionFailure(format!(
            "{} in package.json at line {} column {}",
            parse_reason(&error),
            error.line(),
            error.column()
        ))
    })?;
    let object = value.as_object().ok_or_else(|| {
        DiscoveryError::InspectionFailure("package.json root is not an object".to_string())
    })?;
    let package_source = source(PACKAGE_PATH, bytes);
    let mut dependencies = Vec::new();
    for (section, kind) in [
        ("dependencies", NpmDependencyKind::Production),
        ("devDependencies", NpmDependencyKind::Development),
        ("optionalDependencies", NpmDependencyKind::Optional),
        ("peerDependencies", NpmDependencyKind::Peer),
    ] {
        let Some(requirement) = object
            .get(section)
            .and_then(serde_json::Value::as_object)
            .and_then(|table| table.get(AMARI_WASM_PACKAGE))
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        dependencies.push(NpmDependencyEvidence {
            package_name: AMARI_WASM_PACKAGE.to_string(),
            kind,
            declared_version: requirement.to_string(),
            resolved_version: None,
            compatibility: unresolved_compatibility(requirement, catalog_version),
            manifest_source: package_source.clone(),
            lock_source: None,
        });
    }
    dependencies.sort_by_key(|dependency| dependency.kind);
    Ok(NpmPackage {
        name: object
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        version: object
            .get("version")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        dependencies,
        source: package_source,
    })
}

fn unresolved_compatibility(requirement: &str, catalog_version: &str) -> Compatibility {
    Compatibility {
        status: "unknown_version".to_string(),
        reasons: vec![format!(
            "declared requirement {requirement} has no supported exact lock resolution for catalog {catalog_version}"
        )],
    }
}

fn parse_lock(bytes: &[u8]) -> Result<NpmLock, NpmInspectionWarning> {
    let lock_source = source(LOCK_PATH, bytes);
    if std::str::from_utf8(bytes).is_err() {
        return Err(NpmInspectionWarning::InvalidUtf8Lock {
            path: LOCK_PATH.to_string(),
            content_hash: lock_source.content_hash,
        });
    }
    let value: serde_json::Value =
        serde_json::from_slice(bytes).map_err(|error| NpmInspectionWarning::MalformedLock {
            path: LOCK_PATH.to_string(),
            reason: parse_reason(&error).to_string(),
            line: u64::try_from(error.line()).ok(),
            column: u64::try_from(error.column()).ok(),
            content_hash: lock_source.content_hash.clone(),
        })?;
    let version = value
        .get("lockfileVersion")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if !matches!(version, 2 | 3) {
        return Err(NpmInspectionWarning::UnsupportedLockfileVersion { version });
    }

    let mut versions = BTreeSet::new();
    if let Some(resolved) = value
        .get("packages")
        .and_then(serde_json::Value::as_object)
        .and_then(|packages| packages.get(&format!("node_modules/{AMARI_WASM_PACKAGE}")))
        .and_then(serde_json::Value::as_object)
        .and_then(|package| package.get("version"))
        .and_then(serde_json::Value::as_str)
    {
        versions.insert(resolved.to_string());
    }
    if let Some(resolved) = value
        .get("dependencies")
        .and_then(serde_json::Value::as_object)
        .and_then(|dependencies| dependencies.get(AMARI_WASM_PACKAGE))
        .and_then(serde_json::Value::as_object)
        .and_then(|package| package.get("version"))
        .and_then(serde_json::Value::as_str)
    {
        versions.insert(resolved.to_string());
    }

    let packages = versions
        .into_iter()
        .map(|version| NpmLockedPackage {
            package_name: AMARI_WASM_PACKAGE.to_string(),
            version,
            source: lock_source.clone(),
        })
        .collect();
    Ok(NpmLock {
        lockfile_version: version,
        packages,
        source: lock_source,
    })
}

fn resolve_dependencies(
    package: &mut NpmPackage,
    lock: &NpmLock,
    catalog_version: &str,
    warnings: &mut Vec<NpmInspectionWarning>,
) {
    let versions: BTreeSet<&str> = lock
        .packages
        .iter()
        .filter(|package| package.package_name == AMARI_WASM_PACKAGE)
        .map(|package| package.version.as_str())
        .collect();
    if versions.len() > 1 {
        warnings.push(NpmInspectionWarning::AmbiguousLockResolution {
            package_name: AMARI_WASM_PACKAGE.to_string(),
            version_count: versions.len(),
        });
        return;
    }
    let Some(version) = versions.first().copied() else {
        return;
    };
    for dependency in &mut package.dependencies {
        dependency.resolved_version = Some(version.to_string());
        dependency.lock_source = Some(lock.source.clone());
        dependency.compatibility = if version == catalog_version {
            Compatibility {
                status: "applicable".to_string(),
                reasons: vec![format!(
                    "resolved npm version {version} matches the embedded catalog"
                )],
            }
        } else {
            Compatibility {
                status: "unknown_version".to_string(),
                reasons: vec![format!(
                    "resolved npm version {version} does not match catalog {catalog_version}"
                )],
            }
        };
    }
}

fn finish(
    package: NpmPackage,
    lock: Option<NpmLock>,
    catalog_version: String,
    warnings: Vec<NpmInspectionWarning>,
    provenance: Provenance,
    state: SnapshotState,
) -> NpmInspection {
    NpmInspection {
        package,
        lock,
        catalog_version,
        warnings,
        input_hash: provenance.input_hash(),
        state,
        inspected_file_count: provenance.file_count,
        total_bytes: provenance.total_bytes,
    }
}

/// Inspects project-root npm manifest and supported lockfile evidence offline.
///
/// `package.json` is required. `package-lock.json` is optional; only npm lock
/// schema versions 2 and 3 are used for exact resolution. Missing, malformed,
/// unsupported, symlinked, or resource-limited lockfiles produce typed partial
/// evidence rather than invoking npm or consulting the network.
///
/// # Errors
///
/// Returns an inspection error when the root is not a directory, the required
/// package manifest is missing/malformed/unsafe, or the manifest cannot be
/// accepted within the configured limits.
pub fn inspect_npm_project(
    root: &Path,
    limits: &InspectionLimits,
) -> DiscoveryResult<NpmInspection> {
    if !root.is_dir() {
        return Err(DiscoveryError::InspectionFailure(
            "project root is not a directory".to_string(),
        ));
    }
    let root = root.canonicalize().map_err(|_| {
        DiscoveryError::InspectionFailure("cannot canonicalize project root".to_string())
    })?;
    if limits.max_traversal_depth < 1 {
        return Err(DiscoveryError::LimitExceeded(
            "package.json exceeds traversal depth limit".to_string(),
        ));
    }
    if limits.max_inspection_files == 0 {
        return Err(DiscoveryError::LimitExceeded(
            "package.json exceeds file-count limit".to_string(),
        ));
    }
    let start = Instant::now();
    if wall_clock_limit(start, limits).is_some() {
        return Err(DiscoveryError::LimitExceeded(
            "package.json exceeds wall-clock limit".to_string(),
        ));
    }

    let package_file = match open_input(&root, PACKAGE_PATH) {
        Ok(OpenOutcome::Opened(file)) => file,
        Ok(OpenOutcome::Missing) => {
            return Err(DiscoveryError::InspectionFailure(
                "package.json not found".to_string(),
            ));
        }
        Ok(OpenOutcome::Symlink) => {
            return Err(DiscoveryError::InspectionFailure(
                "package.json is a symlink and cannot be inspected".to_string(),
            ));
        }
        Ok(OpenOutcome::Unsupported) => {
            return Err(DiscoveryError::InspectionFailure(
                "package.json is not a regular file".to_string(),
            ));
        }
        Err(_) => {
            return Err(DiscoveryError::InspectionFailure(
                "cannot open package.json".to_string(),
            ));
        }
    };
    let package_bytes = match read_bounded(package_file, limits, 0) {
        ReadOutcome::Accepted(bytes) => bytes,
        ReadOutcome::PerFileExceeded => {
            return Err(DiscoveryError::LimitExceeded(
                "package.json exceeds per-file byte limit".to_string(),
            ));
        }
        ReadOutcome::AggregateExceeded => {
            return Err(DiscoveryError::LimitExceeded(
                "package.json exceeds aggregate byte limit".to_string(),
            ));
        }
        ReadOutcome::Failed => {
            return Err(DiscoveryError::InspectionFailure(
                "cannot read package.json".to_string(),
            ));
        }
    };

    let catalog_version = crate::Catalog::embedded()?.version().to_string();
    let mut package = parse_package(&package_bytes, &catalog_version)?;
    let mut provenance = Provenance::default();
    provenance.accept(PACKAGE_PATH, &package_bytes);
    let mut warnings = Vec::new();

    if let Some(limit) = wall_clock_limit(start, limits) {
        warnings.push(NpmInspectionWarning::LimitExceeded {
            limit: limit.clone(),
        });
        return Ok(finish(
            package,
            None,
            catalog_version,
            warnings,
            provenance,
            SnapshotState::LimitExceeded { limit },
        ));
    }

    let lock_file = match open_input(&root, LOCK_PATH) {
        Ok(OpenOutcome::Opened(file)) => file,
        Ok(OpenOutcome::Missing) => {
            warnings.push(NpmInspectionWarning::MissingLock {
                path: LOCK_PATH.to_string(),
            });
            return Ok(finish(
                package,
                None,
                catalog_version,
                warnings,
                provenance,
                SnapshotState::Complete,
            ));
        }
        Ok(OpenOutcome::Symlink) => {
            warnings.push(NpmInspectionWarning::SymlinkedLock {
                path: LOCK_PATH.to_string(),
            });
            return Ok(finish(
                package,
                None,
                catalog_version,
                warnings,
                provenance,
                SnapshotState::Complete,
            ));
        }
        Ok(OpenOutcome::Unsupported) => {
            warnings.push(NpmInspectionWarning::UnsupportedLockFile {
                path: LOCK_PATH.to_string(),
            });
            return Ok(finish(
                package,
                None,
                catalog_version,
                warnings,
                provenance,
                SnapshotState::Complete,
            ));
        }
        Err(_) => {
            warnings.push(NpmInspectionWarning::ReadFailure {
                path: LOCK_PATH.to_string(),
            });
            return Ok(finish(
                package,
                None,
                catalog_version,
                warnings,
                provenance,
                SnapshotState::Complete,
            ));
        }
    };

    if limits.max_inspection_files < 2 {
        let limit = InspectionLimit::FileCount {
            max: limits.max_inspection_files,
            observed: 2,
        };
        warnings.push(NpmInspectionWarning::LimitExceeded {
            limit: limit.clone(),
        });
        return Ok(finish(
            package,
            None,
            catalog_version,
            warnings,
            provenance,
            SnapshotState::LimitExceeded { limit },
        ));
    }

    let lock_bytes = match read_bounded(lock_file, limits, provenance.total_bytes) {
        ReadOutcome::Accepted(bytes) => bytes,
        ReadOutcome::PerFileExceeded => {
            let limit = InspectionLimit::PerFileBytes {
                max: limits.max_per_file_bytes,
                observed: limits.max_per_file_bytes.saturating_add(1),
            };
            warnings.push(NpmInspectionWarning::LimitExceeded {
                limit: limit.clone(),
            });
            return Ok(finish(
                package,
                None,
                catalog_version,
                warnings,
                provenance,
                SnapshotState::LimitExceeded { limit },
            ));
        }
        ReadOutcome::AggregateExceeded => {
            let limit = InspectionLimit::TotalBytes {
                max: limits.max_inspection_bytes,
                observed: limits.max_inspection_bytes.saturating_add(1),
            };
            warnings.push(NpmInspectionWarning::LimitExceeded {
                limit: limit.clone(),
            });
            return Ok(finish(
                package,
                None,
                catalog_version,
                warnings,
                provenance,
                SnapshotState::LimitExceeded { limit },
            ));
        }
        ReadOutcome::Failed => {
            warnings.push(NpmInspectionWarning::ReadFailure {
                path: LOCK_PATH.to_string(),
            });
            return Ok(finish(
                package,
                None,
                catalog_version,
                warnings,
                provenance,
                SnapshotState::Complete,
            ));
        }
    };
    provenance.accept(LOCK_PATH, &lock_bytes);

    let lock = match parse_lock(&lock_bytes) {
        Ok(lock) => {
            resolve_dependencies(&mut package, &lock, &catalog_version, &mut warnings);
            Some(lock)
        }
        Err(warning) => {
            warnings.push(warning);
            None
        }
    };

    let state = if let Some(limit) = wall_clock_limit(start, limits) {
        warnings.push(NpmInspectionWarning::LimitExceeded {
            limit: limit.clone(),
        });
        SnapshotState::LimitExceeded { limit }
    } else {
        SnapshotState::Complete
    };
    Ok(finish(
        package,
        lock,
        catalog_version,
        warnings,
        provenance,
        state,
    ))
}
