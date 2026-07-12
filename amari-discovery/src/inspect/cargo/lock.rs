// SPDX-License-Identifier: MIT OR Apache-2.0

//! Offline `Cargo.lock` file parsing and version resolution.
//!
//! Parses `Cargo.lock` using the `toml` crate and resolves exact package
//! versions against declared dependency requirements.

use std::collections::{BTreeMap, BTreeSet};

use crate::protocol::Compatibility;

use super::toml_helpers::{toml_line_col_from_source, toml_malformed_reason, toml_string};
use super::types::{AmariDependencyEvidence, CargoInspectionWarning, LockedPackage};
use super::ProvenanceAccumulator;

// ============================================================================
// ParsedLock
// ============================================================================

/// Parsed Cargo.lock content.
pub(super) struct ParsedLock {
    /// Packages in declaration order.
    pub packages: Vec<LockedPackage>,
    /// Warnings accumulated during parsing.
    pub warnings: Vec<CargoInspectionWarning>,
}

// ============================================================================
// Lock file parsing
// ============================================================================

/// Parse a `Cargo.lock` file content entirely offline using TOML.
pub(super) fn parse_lock(content: &[u8], lock_path: &str) -> ParsedLock {
    let mut warnings = Vec::new();
    let mut packages = Vec::new();

    let raw_str = match std::str::from_utf8(content) {
        Ok(s) => s,
        Err(_) => {
            warnings.push(CargoInspectionWarning::MalformedLock {
                path: lock_path.to_string(),
                reason: "not valid UTF-8".to_string(),
                line: None,
                column: None,
            });
            return ParsedLock { packages, warnings };
        }
    };

    let lock: toml::Value = match toml::from_str(raw_str) {
        Ok(v) => v,
        Err(e) => {
            let (line, col) = toml_line_col_from_source(&e, content);
            let reason = toml_malformed_reason(&e);
            warnings.push(CargoInspectionWarning::MalformedLock {
                path: lock_path.to_string(),
                reason,
                line,
                column: col,
            });
            return ParsedLock { packages, warnings };
        }
    };

    // Cargo.lock format: [[package]] entries
    if let Some(pkgs) = lock.get("package").and_then(|v| v.as_array()) {
        for entry in pkgs {
            let table = match entry.as_table() {
                Some(t) => t,
                None => continue,
            };
            let name = match toml_string(table, "name") {
                Some(n) => n,
                None => continue,
            };
            let version = toml_string(table, "version").unwrap_or_else(|| "unknown".to_string());
            let checksum = toml_string(table, "checksum");
            let source = toml_string(table, "source");

            packages.push(LockedPackage {
                name,
                version,
                checksum,
                source,
            });
        }
    }

    // Check for ambiguous entries: group by package name, collect unique versions
    let mut seen: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for pkg in &packages {
        seen.entry(pkg.name.as_str())
            .or_default()
            .insert(pkg.version.as_str());
    }
    for (name, versions) in &seen {
        if versions.len() > 1 {
            let sorted_versions: Vec<String> = {
                let mut v: Vec<String> = versions.iter().map(|s| s.to_string()).collect();
                v.sort();
                v
            };
            warnings.push(CargoInspectionWarning::AmbiguousLockResolution {
                package: name.to_string(),
                versions: sorted_versions,
            });
        }
    }

    ParsedLock { packages, warnings }
}

// ============================================================================
// Compatibility resolution
// ============================================================================

/// Compute compatibility for each Amari dependency by matching against
/// the lockfile and comparing resolved versions with the catalog.
///
/// Resolution strategy:
/// - Single unique version in lock → resolved
/// - Multiple versions in lock → only resolve if a plain exact declared
///   version uniquely identifies one; otherwise unknown_version
/// - Duplicate same name+same version is not ambiguity
pub(super) fn resolve_compatibility(
    deps: &mut [AmariDependencyEvidence],
    lock: Option<&ParsedLock>,
    catalog_version: &str,
    lock_path: &str,
    lock_content: &[u8],
    provenance: &ProvenanceAccumulator,
) {
    // Build: package_name → set of unique versions
    let mut version_map: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    if let Some(l) = lock {
        for pkg in &l.packages {
            version_map
                .entry(pkg.name.as_str())
                .or_default()
                .insert(pkg.version.as_str());
        }
    }

    let lock_available = lock.is_some();

    let lock_source = if lock_available {
        Some(provenance.make_source(lock_path, lock_content, None))
    } else {
        None
    };

    for dep in deps.iter_mut() {
        if !lock_available {
            dep.compatibility = Compatibility {
                status: "unknown_version".to_string(),
                reasons: vec!["Cargo.lock not available for resolution".to_string()],
            };
            continue;
        }

        let versions = match version_map.get(dep.package_name.as_str()) {
            Some(v) => v,
            None => {
                dep.resolved_version = None;
                dep.compatibility = Compatibility {
                    status: "unknown_version".to_string(),
                    reasons: vec![format!(
                        "package {} not found in Cargo.lock",
                        dep.package_name
                    )],
                };
                continue;
            }
        };

        if versions.len() == 1 {
            // Unique version → resolved
            // SAFETY: versions.len() == 1 so first() always returns Some
            let ver = if let Some(v) = versions.first() {
                v.to_string()
            } else {
                // Unreachable: len == 1 guarantees first() is Some
                dep.resolved_version = None;
                dep.compatibility = Compatibility {
                    status: "unknown_version".to_string(),
                    reasons: vec!["internal error: empty version set".to_string()],
                };
                continue;
            };
            dep.resolved_version = Some(ver.clone());
            dep.lock_source = lock_source.clone();

            if ver == catalog_version {
                dep.compatibility = Compatibility {
                    status: "applicable".to_string(),
                    reasons: vec![format!(
                        "resolved version {} equals catalog version {}",
                        ver, catalog_version
                    )],
                };
            } else {
                dep.compatibility = Compatibility {
                    status: "unknown_version".to_string(),
                    reasons: vec![format!(
                        "resolved version {} differs from catalog version {}",
                        ver, catalog_version
                    )],
                };
            }
        } else {
            // Multiple versions: check if declared exact version uniquely identifies one
            let declared = &dep.declared_version;
            let matching: Vec<&&str> = versions.iter().filter(|v| v == &declared).collect();

            if matching.len() == 1 {
                let ver = matching[0].to_string();
                dep.resolved_version = Some(ver.clone());
                dep.lock_source = lock_source.clone();
                if ver == catalog_version {
                    dep.compatibility = Compatibility {
                        status: "applicable".to_string(),
                        reasons: vec![format!(
                            "declared version {} uniquely resolves to locked version {} which equals catalog version {}",
                            declared, ver, catalog_version
                        )],
                    };
                } else {
                    dep.compatibility = Compatibility {
                        status: "unknown_version".to_string(),
                        reasons: vec![format!(
                            "declared version {} uniquely resolves to locked version {} which differs from catalog version {}",
                            declared, ver, catalog_version
                        )],
                    };
                }
            } else {
                // Ambiguous — cannot resolve
                dep.resolved_version = None;
                dep.compatibility = Compatibility {
                    status: "unknown_version".to_string(),
                    reasons: vec![format!(
                        "package {} has ambiguous versions in Cargo.lock: {:?}",
                        dep.package_name,
                        versions.iter().collect::<Vec<_>>()
                    )],
                };
            }
        }

        // If declared version is "unknown", always mark as unknown_version
        if dep.declared_version == "unknown" {
            dep.compatibility = Compatibility {
                status: "unknown_version".to_string(),
                reasons: vec!["declared version could not be determined".to_string()],
            };
        }
    }
}
