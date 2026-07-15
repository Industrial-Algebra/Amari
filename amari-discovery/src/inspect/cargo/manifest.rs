// SPDX-License-Identifier: MIT OR Apache-2.0

//! Offline Cargo manifest parsing.
//!
//! Parses `Cargo.toml` files purely with the `toml` crate — never shells
//! out to Cargo. Extracts Amari dependency evidence, workspace metadata,
//! bench targets, native-link signals, and system dependency signals.

use std::collections::{BTreeMap, BTreeSet};

use crate::error::{DiscoveryError, DiscoveryResult};

use super::toml_helpers::{
    parse_dep_value, toml_bool, toml_string, toml_strings_opt, toml_strings_sorted,
};
use super::types::{
    system_dep_kind, AmariDependencyEvidence, CargoBench, CargoDependencyRecord,
    CargoInspectionWarning, CargoPackage, DependencyKind, NativeLink, SystemDependencySignal,
    WorkspaceDependencyBase, WorkspaceMeta,
};
use super::ProvenanceAccumulator;

// ============================================================================
// Amari package detection
// ============================================================================

/// Returns true when `name` is an Amari package (`amari` or `amari-*`).
pub(super) fn is_amari_package(name: &str) -> bool {
    name == "amari" || name.starts_with("amari-")
}

// ============================================================================
// Resolved workspace dep (replaces 5-tuple return type)
// ============================================================================

/// Result of resolving a workspace-inherited dependency.
pub(super) struct ResolvedWorkspaceDep {
    /// The actual package name after alias/package resolution.
    pub package_name: String,
    /// The resolved version string.
    pub version: String,
    /// Merged features (base ∪ member overrides, sorted and deduplicated).
    pub features: Vec<String>,
    /// Whether the dependency is optional.
    pub optional: bool,
    /// Whether Cargo default features are enabled.
    pub default_features: bool,
}

// ============================================================================
// Workspace dependency inheritance (keyed by ALIAS)
// ============================================================================

/// Resolve a workspace-inherited dependency given its alias, the
/// workspace bases map, and the member's overrides.
fn resolve_workspace_dep(
    alias: &str,
    member_dep_table: &toml::value::Table,
    bases: &BTreeMap<String, WorkspaceDependencyBase>,
    package_name: &str,
    warnings: &mut Vec<CargoInspectionWarning>,
) -> ResolvedWorkspaceDep {
    // workspace = false is illegal
    if let Some(false) = toml_bool(member_dep_table, "workspace") {
        warnings.push(CargoInspectionWarning::WorkspaceFalseRejected {
            dep: alias.to_string(),
            package: package_name.to_string(),
        });
        return ResolvedWorkspaceDep {
            package_name: alias.to_string(),
            version: "unknown".to_string(),
            features: vec![],
            optional: false,
            default_features: true,
        };
    }

    // Base is keyed by alias, not package name
    let base = match bases.get(alias) {
        Some(b) => b,
        None => {
            warnings.push(CargoInspectionWarning::InheritedBaseMissing {
                member: package_name.to_string(),
                dep: alias.to_string(),
            });
            return ResolvedWorkspaceDep {
                package_name: alias.to_string(),
                version: "unknown".to_string(),
                features: vec![],
                optional: false,
                default_features: true,
            };
        }
    };

    // Reject illegal overrides on workspace deps
    for illegal_key in &["version", "path", "git", "package"] {
        if member_dep_table.contains_key(*illegal_key) {
            warnings.push(CargoInspectionWarning::WorkspaceOverrideRejected {
                dep: alias.to_string(),
                package: package_name.to_string(),
                key: illegal_key.to_string(),
            });
        }
    }

    // Resolve package name: base.package over the alias is the actual name,
    // else the alias itself.
    let resolved_pkg = base.package.clone().unwrap_or_else(|| alias.to_string());

    // Resolve version from base
    let version = base
        .version
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    // Merge features: base features ∪ member overrides, sorted/dedup
    let mut features: BTreeSet<String> = base.features.iter().cloned().collect();
    for f in toml_strings_opt(member_dep_table, "features").unwrap_or_default() {
        features.insert(f);
    }
    let merged_features: Vec<String> = features.into_iter().collect();

    // Resolve optional: member override > base
    let optional = toml_bool(member_dep_table, "optional").unwrap_or(base.optional);

    // Resolve default-features: member override > base
    let default_features = toml_bool(member_dep_table, "default-features")
        .or(base.default_features)
        .unwrap_or(true);

    ResolvedWorkspaceDep {
        package_name: resolved_pkg,
        version,
        features: merged_features,
        optional,
        default_features,
    }
}

// ============================================================================
// Single dependency table parsing
// ============================================================================

/// Parse a single dependency table section (e.g. `[dependencies]`) returning
/// Amari dependency evidence entries.
///
/// `target` is `Some(cfg)` for `[target.'cfg(...)'.dependencies]`, `None` for
/// regular dependency tables.
#[allow(clippy::too_many_arguments)]
pub(super) fn parse_dep_table(
    table: &toml::value::Table,
    kind: DependencyKind,
    target: Option<&str>,
    manifest_path: &str,
    manifest_content: &[u8],
    workspace_bases: &BTreeMap<String, WorkspaceDependencyBase>,
    package_name: &str,
    warnings: &mut Vec<CargoInspectionWarning>,
    provenance: &ProvenanceAccumulator,
) -> Vec<AmariDependencyEvidence> {
    let mut deps: Vec<AmariDependencyEvidence> = Vec::new();

    for (alias, raw) in table {
        let dep_table = match parse_dep_value(raw) {
            Some(t) => t,
            None => continue,
        };

        // Detect explicit `workspace = false` before checking is_workspace
        if let Some(false) = toml_bool(&dep_table, "workspace") {
            warnings.push(CargoInspectionWarning::WorkspaceFalseRejected {
                dep: alias.clone(),
                package: package_name.to_string(),
            });
            continue;
        }

        let is_workspace = toml_bool(&dep_table, "workspace").unwrap_or(false);
        let pkg = toml_string(&dep_table, "package");

        // Determine actual package name (after renaming)
        let package_name_val = if is_workspace {
            pkg.unwrap_or_else(|| alias.clone())
        } else {
            pkg.unwrap_or_else(|| alias.clone())
        };

        // Filter: only Amari deps
        if !is_workspace {
            if !is_amari_package(&package_name_val) && !is_amari_package(alias) {
                continue;
            }
        } else {
            let base_has_amari = workspace_bases
                .get(alias)
                .and_then(|base| base.package.as_deref())
                .is_some_and(is_amari_package);
            if !is_amari_package(alias) && !base_has_amari {
                continue;
            }
        }

        // Resolve features, optional, default-features
        let features: Vec<String> = toml_strings_sorted(&dep_table, "features");
        let optional = toml_bool(&dep_table, "optional").unwrap_or(false);
        let default_features = toml_bool(&dep_table, "default-features").unwrap_or(true);

        let declared_version: String;

        if is_workspace {
            let resolved =
                resolve_workspace_dep(alias, &dep_table, workspace_bases, package_name, warnings);
            declared_version = resolved.version;

            let final_pkg = if resolved.package_name != *alias {
                resolved.package_name.clone()
            } else {
                alias.clone()
            };

            if !is_amari_package(&final_pkg) {
                continue;
            }

            let final_features = if resolved.features.is_empty() {
                features
            } else {
                resolved.features
            };

            deps.push(AmariDependencyEvidence {
                alias: alias.clone(),
                package_name: final_pkg,
                kind,
                declared_version,
                resolved_version: None,
                compatibility: crate::protocol::Compatibility {
                    status: "unknown_version".to_string(),
                    reasons: vec!["not yet resolved against lock".to_string()],
                },
                target: target.map(String::from),
                features: final_features,
                default_features: resolved.default_features,
                optional: resolved.optional,
                manifest_source: provenance.make_source(manifest_path, manifest_content, None),
                lock_source: None,
            });
        } else {
            // Direct dependency
            let final_pkg = if let Some(ref p) = toml_string(&dep_table, "package") {
                p.clone()
            } else {
                alias.clone()
            };

            if !is_amari_package(&final_pkg) {
                continue;
            }

            if let Some(v) = toml_string(&dep_table, "version") {
                declared_version = v;
            } else if let Some(git) = toml_string(&dep_table, "git") {
                warnings.push(CargoInspectionWarning::UnsupportedRequirement {
                    package: package_name.to_string(),
                    dep: alias.clone(),
                    reason: format!("git dependency '{}' cannot be resolved offline", git),
                });
                declared_version = "unknown".to_string();
            } else if toml_string(&dep_table, "path").is_some() {
                warnings.push(CargoInspectionWarning::UnsupportedRequirement {
                    package: package_name.to_string(),
                    dep: alias.clone(),
                    reason: "path dependency cannot be resolved offline".to_string(),
                });
                declared_version = "unknown".to_string();
            } else {
                warnings.push(CargoInspectionWarning::UnsupportedRequirement {
                    package: package_name.to_string(),
                    dep: alias.clone(),
                    reason:
                        "dependency has no version, workspace marker, or offline-resolvable spec"
                            .to_string(),
                });
                declared_version = "unknown".to_string();
            }

            deps.push(AmariDependencyEvidence {
                alias: alias.clone(),
                package_name: final_pkg,
                kind,
                declared_version,
                resolved_version: None,
                compatibility: crate::protocol::Compatibility {
                    status: "unknown_version".to_string(),
                    reasons: vec!["not yet resolved against lock".to_string()],
                },
                target: target.map(String::from),
                features,
                default_features,
                optional,
                manifest_source: provenance.make_source(manifest_path, manifest_content, None),
                lock_source: None,
            });
        }
    }

    deps
}

// ============================================================================
// All-dependency records (Task 8C — non-Amari target deps)
// ============================================================================

/// Collect a lightweight [`CargoDependencyRecord`] for **every** dependency in
/// a table (Amari and non-Amari), preserving the table kind and optional
/// target selector. No version or feature data is retained.
///
/// # Workspace inheritance
///
/// A member declaration `foo = { workspace = true }` is resolved through the
/// `[workspace.dependencies]` base keyed by the **alias** `foo`. The canonical
/// Cargo package name comes from the base `package = "..."` rename when set,
/// otherwise the alias. An illegal member `package = "..."` override on a
/// `workspace = true` dependency is **never** honored (Cargo rejects it at
/// build time). A `workspace = true` declaration whose base is missing falls
/// back conservatively to the alias (the record is retained so platform
/// constraints still surface; no warning is emitted here to avoid duplicating
/// the `InheritedBaseMissing` warning produced by the Amari-aware path).
pub(super) fn collect_dependency_records(
    table: &toml::value::Table,
    kind: DependencyKind,
    target: Option<&str>,
    manifest_path: &str,
    manifest_content: &[u8],
    workspace_bases: &BTreeMap<String, WorkspaceDependencyBase>,
    provenance: &ProvenanceAccumulator,
) -> Vec<CargoDependencyRecord> {
    let mut out = Vec::new();
    for (alias, raw) in table {
        let dep_table = match parse_dep_value(raw) {
            Some(t) => t,
            None => continue,
        };
        let is_workspace = toml_bool(&dep_table, "workspace").unwrap_or(false);
        let pkg = if is_workspace {
            // Resolve the canonical name through the workspace base keyed by
            // the alias. Never honor an illegal member `package` override.
            workspace_bases
                .get(alias)
                .and_then(|base| base.package.clone())
                .unwrap_or_else(|| alias.clone())
        } else {
            toml_string(&dep_table, "package").unwrap_or_else(|| alias.clone())
        };
        out.push(CargoDependencyRecord {
            alias: alias.clone(),
            package: pkg,
            kind,
            target: target.map(String::from),
            source: provenance.make_source(manifest_path, manifest_content, None),
        });
    }
    out
}

// ============================================================================
// System dependency detection
// ============================================================================

/// Scan a dependency table for known system dependency packages and
/// emit typed signals.
pub(super) fn scan_system_deps(
    table: &toml::value::Table,
    kind: DependencyKind,
    target: Option<&str>,
    manifest_path: &str,
    manifest_content: &[u8],
    provenance: &ProvenanceAccumulator,
) -> Vec<SystemDependencySignal> {
    let mut signals = Vec::new();
    for (alias, raw) in table {
        let dep_table = match parse_dep_value(raw) {
            Some(t) => t,
            None => continue,
        };
        let pkg = toml_string(&dep_table, "package").unwrap_or_else(|| alias.clone());
        if let Some(sys_kind) = system_dep_kind(&pkg) {
            signals.push(SystemDependencySignal {
                alias: alias.clone(),
                package: pkg,
                dependency_kind: kind,
                system_kind: sys_kind,
                target: target.map(String::from),
                manifest_source: provenance.make_source(manifest_path, manifest_content, None),
            });
        } else if let Some(sys_kind) = system_dep_kind(alias) {
            signals.push(SystemDependencySignal {
                alias: alias.clone(),
                package: alias.clone(),
                dependency_kind: kind,
                system_kind: sys_kind,
                target: target.map(String::from),
                manifest_source: provenance.make_source(manifest_path, manifest_content, None),
            });
        }
    }
    signals
}

// ============================================================================
// Bench / links / metadata parsing
// ============================================================================

/// Parse `[[bench]]` declarations from the root of a TOML manifest.
///
/// Each declared `path` is validated as a normalized package-relative path
/// (rejecting absolute paths and parent-traversal `..`); invalid paths emit
/// a sanitized [`CargoInspectionWarning::InvalidBenchPath`] and the bench is
/// omitted conservatively (the raw path is never serialized). `harness`
/// defaults to `true`; `required-features` is sorted and deduplicated.
pub(super) fn parse_benches(
    manifest: &toml::Value,
    manifest_path: &str,
    manifest_content: &[u8],
    pkg_name: &str,
    warnings: &mut Vec<CargoInspectionWarning>,
    provenance: &ProvenanceAccumulator,
) -> Vec<CargoBench> {
    let mut benches = Vec::new();
    if let Some(arr) = manifest.get("bench").and_then(|v| v.as_array()) {
        for entry in arr {
            let table = match entry.as_table() {
                Some(t) => t,
                None => continue,
            };
            let name = match toml_string(table, "name") {
                Some(n) => n,
                None => continue,
            };
            let path = toml_string(table, "path")
                .unwrap_or_else(|| "benches/".to_string() + &name + ".rs");
            // Validate the declared/default path. Invalid paths are omitted
            // conservatively (never serialized) with a sanitized warning that
            // carries only the safe bench name, never the raw path.
            if !is_normal_relative_path(&path) {
                warnings.push(CargoInspectionWarning::InvalidBenchPath {
                    bench_name: name,
                    package: pkg_name.to_string(),
                });
                continue;
            }
            // Normalize both platform separator forms so inspection output and
            // source joins are host-independent.
            let path = path.replace('\\', "/");
            let harness = toml_bool(table, "harness").unwrap_or(true);
            let required_features = toml_strings_sorted(table, "required-features");
            benches.push(CargoBench {
                name,
                path,
                harness,
                required_features,
                manifest_source: provenance.make_source(manifest_path, manifest_content, None),
            });
        }
    }
    benches
}

/// Returns `true` when `path` is a normalized package-relative path: not
/// absolute, with no parent-traversal (`..`), curdir (`.`), or empty
/// components. Forward and back slashes are both treated as separators.
pub(super) fn is_normal_relative_path(path: &str) -> bool {
    if path.is_empty() || path.starts_with(['/', '\\']) {
        return false;
    }

    // Reject Windows drive prefixes on every host. `std::path::Path` follows
    // host semantics, so it would otherwise treat `C:\\...` as a normal Unix
    // filename when inspection runs on Unix.
    let bytes = path.as_bytes();
    if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
        return false;
    }

    path.split(['/', '\\']).all(|component| {
        !component.is_empty() && component != "." && component != ".." && !component.contains('\0')
    })
}

/// Parse `[package].links` from the manifest.
pub(super) fn parse_native_link(
    package_table: &toml::value::Table,
    manifest_path: &str,
    manifest_content: &[u8],
    provenance: &ProvenanceAccumulator,
) -> Option<NativeLink> {
    toml_string(package_table, "links").map(|key| NativeLink {
        links_key: key,
        manifest_source: provenance.make_source(manifest_path, manifest_content, None),
    })
}

/// Parse `[workspace.package]` fields into a map.
pub(super) fn parse_workspace_package_fields(
    ws_table: &toml::value::Table,
) -> BTreeMap<String, String> {
    match ws_table.get("package").and_then(|v| v.as_table()) {
        Some(pkg) => pkg
            .iter()
            .filter_map(|(k, v)| {
                if let Some(s) = v.as_str() {
                    Some((k.clone(), s.to_string()))
                } else if v.is_array() || v.is_table() {
                    Some((k.clone(), v.to_string()))
                } else {
                    None
                }
            })
            .collect(),
        None => BTreeMap::new(),
    }
}

/// Resolve the package version.
pub(super) fn resolve_package_version(
    package_table: &toml::value::Table,
    ws_package_fields: &BTreeMap<String, String>,
) -> String {
    // Direct version string
    if let Some(v) = toml_string(package_table, "version") {
        return v;
    }
    // `version.workspace = true` or `version = { workspace = true }`
    if let Some(ver_val) = package_table.get("version") {
        if let Some(ver_table) = ver_val.as_table() {
            if toml_bool(ver_table, "workspace").unwrap_or(false) {
                return ws_package_fields
                    .get("version")
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string());
            }
        }
    }
    "unknown".to_string()
}

/// Parse `[workspace.package]` and determine which fields the package
/// inherits.
pub(super) fn parse_inherited_metadata(
    package_table: &toml::value::Table,
    ws_package_fields: &BTreeMap<String, String>,
    pkg_name: &str,
    warnings: &mut Vec<CargoInspectionWarning>,
) -> Vec<String> {
    let inheritable_keys = [
        "version",
        "authors",
        "edition",
        "license",
        "rust-version",
        "description",
        "homepage",
        "repository",
        "documentation",
        "readme",
        "keywords",
        "categories",
        "exclude",
        "include",
    ];
    let mut inherited: Vec<String> = Vec::new();
    for key in &inheritable_keys {
        if let Some(sub) = package_table.get(*key) {
            if let Some(t) = sub.as_table() {
                if toml_bool(t, "workspace").unwrap_or(false) {
                    if ws_package_fields.contains_key(*key) {
                        inherited.push(key.to_string());
                    } else {
                        warnings.push(CargoInspectionWarning::WorkspaceFieldNotFound {
                            field: key.to_string(),
                            package: pkg_name.to_string(),
                        });
                    }
                    continue;
                }
            }
        }
    }
    inherited.sort();
    inherited
}

/// Build the workspace dependency bases map from `[workspace.dependencies]`.
pub(super) fn parse_workspace_deps(
    ws_table: &toml::value::Table,
) -> BTreeMap<String, WorkspaceDependencyBase> {
    let deps_table = match ws_table.get("dependencies").and_then(|v| v.as_table()) {
        Some(t) => t,
        None => return BTreeMap::new(),
    };
    let mut bases = BTreeMap::new();
    for (name, raw) in deps_table {
        let dep = match parse_dep_value(raw) {
            Some(t) => t,
            None => continue,
        };
        bases.insert(
            name.clone(),
            WorkspaceDependencyBase {
                package: toml_string(&dep, "package"),
                version: toml_string(&dep, "version"),
                path: toml_string(&dep, "path"),
                git: toml_string(&dep, "git"),
                features: toml_strings_sorted(&dep, "features"),
                default_features: toml_bool(&dep, "default-features"),
                optional: toml_bool(&dep, "optional").unwrap_or(false),
            },
        );
    }
    bases
}

// ============================================================================
// Manifest parsing
// ============================================================================

/// Result of parsing a single manifest.
pub(super) struct ParsedManifest {
    /// The package extracted from the manifest.
    pub package: CargoPackage,
    /// Workspace metadata, only populated for the root manifest.
    pub ws_meta: Option<WorkspaceMeta>,
}

/// Parse a single `Cargo.toml` manifest and extract the package info,
/// Amari dependencies, benches, native link, inherited metadata, and
/// system dependency signals.
///
/// When `root_manifest` is provided (for the root), the raw bytes are
/// also accepted but parsing is done from the pre-parsed `toml::Value`
/// to avoid double-parsing.
#[allow(clippy::too_many_arguments)]
pub(super) fn parse_manifest_from_value(
    manifest: &toml::Value,
    content: &[u8],
    manifest_path: &str,
    is_root: bool,
    workspace_bases: &BTreeMap<String, WorkspaceDependencyBase>,
    ws_package_fields: &BTreeMap<String, String>,
    warnings: &mut Vec<CargoInspectionWarning>,
    provenance: &ProvenanceAccumulator,
) -> DiscoveryResult<ParsedManifest> {
    // -- [package]
    let package_table = match manifest.get("package").and_then(|v| v.as_table()) {
        Some(t) => t.clone(),
        None => {
            return Err(DiscoveryError::InspectionFailure(format!(
                "missing [package] section in {}",
                manifest_path
            )));
        }
    };

    let name = toml_string(&package_table, "name").unwrap_or_else(|| "unknown".to_string());
    let pkg_name = name.clone();

    let version = resolve_package_version(&package_table, ws_package_fields);
    let inherited_metadata =
        parse_inherited_metadata(&package_table, ws_package_fields, &pkg_name, warnings);
    let native_link = parse_native_link(&package_table, manifest_path, content, provenance);
    let benches = parse_benches(
        manifest,
        manifest_path,
        content,
        &pkg_name,
        warnings,
        provenance,
    );
    let autobenches = toml_bool(&package_table, "autobenches");

    // -- Parse dependencies from various tables
    let mut deps: Vec<AmariDependencyEvidence> = Vec::new();
    let mut sys_deps: Vec<SystemDependencySignal> = Vec::new();
    let mut dep_records: Vec<CargoDependencyRecord> = Vec::new();

    let dep_sections: &[(_, DependencyKind)] = &[
        ("dependencies", DependencyKind::Normal),
        ("dev-dependencies", DependencyKind::Dev),
        ("build-dependencies", DependencyKind::Build),
    ];

    for (section, kind) in dep_sections {
        if let Some(table) = manifest.get(*section).and_then(|v| v.as_table()) {
            deps.extend(parse_dep_table(
                table,
                *kind,
                None,
                manifest_path,
                content,
                workspace_bases,
                &pkg_name,
                warnings,
                provenance,
            ));
            sys_deps.extend(scan_system_deps(
                table,
                *kind,
                None,
                manifest_path,
                content,
                provenance,
            ));
            dep_records.extend(collect_dependency_records(
                table,
                *kind,
                None,
                manifest_path,
                content,
                workspace_bases,
                provenance,
            ));
        }
    }

    // Target-specific dependencies
    if let Some(target_table) = manifest.get("target").and_then(|v| v.as_table()) {
        for (cfg_key, cfg_value) in target_table {
            let cfg_pred = cfg_key.trim_matches('\'');
            let cfg_table = match cfg_value.as_table() {
                Some(t) => t,
                None => continue,
            };
            for (section, kind) in dep_sections {
                if let Some(deps_table) = cfg_table.get(*section).and_then(|v| v.as_table()) {
                    deps.extend(parse_dep_table(
                        deps_table,
                        *kind,
                        Some(cfg_pred),
                        manifest_path,
                        content,
                        workspace_bases,
                        &pkg_name,
                        warnings,
                        provenance,
                    ));
                    sys_deps.extend(scan_system_deps(
                        deps_table,
                        *kind,
                        Some(cfg_pred),
                        manifest_path,
                        content,
                        provenance,
                    ));
                    dep_records.extend(collect_dependency_records(
                        deps_table,
                        *kind,
                        Some(cfg_pred),
                        manifest_path,
                        content,
                        workspace_bases,
                        provenance,
                    ));
                }
            }
        }
    }

    // Sort deps for determinism
    deps.sort_by(|a, b| {
        a.package_name
            .cmp(&b.package_name)
            .then(a.alias.cmp(&b.alias))
            .then(a.kind.rank().cmp(&b.kind.rank()))
    });

    // Sort system deps deterministically
    sys_deps.sort_by(|a, b| {
        a.package
            .cmp(&b.package)
            .then(a.alias.cmp(&b.alias))
            .then(a.dependency_kind.rank().cmp(&b.dependency_kind.rank()))
    });

    // Sort all-dependency records deterministically (by package, alias, kind,
    // then target selector) — target selectors retained for platform derivation.
    dep_records.sort_by(|a, b| {
        a.package
            .cmp(&b.package)
            .then(a.alias.cmp(&b.alias))
            .then(a.kind.rank().cmp(&b.kind.rank()))
            .then(a.target.cmp(&b.target))
    });

    // -- Workspace metadata (only from root manifest)
    let ws_meta = if is_root {
        manifest
            .get("workspace")
            .and_then(|v| v.as_table())
            .map(|ws_table| {
                let members = toml_strings_opt(ws_table, "members").unwrap_or_default();
                let dep_bases = parse_workspace_deps(ws_table);
                let pkg_fields = parse_workspace_package_fields(ws_table);
                WorkspaceMeta {
                    members,
                    dependency_bases: dep_bases,
                    package_fields: pkg_fields,
                }
            })
    } else {
        None
    };

    let pkg = CargoPackage {
        name,
        version,
        manifest_path: manifest_path.to_string(),
        dependencies: deps,
        benches,
        native_link,
        inherited_metadata,
        system_dependencies: sys_deps,
        dependency_records: dep_records,
        autobenches,
    };

    Ok(ParsedManifest {
        package: pkg,
        ws_meta,
    })
}

/// Parse a member manifest from raw bytes (re-parse TOML since we don't
/// have a pre-parsed value).
pub(super) fn parse_manifest(
    content: &[u8],
    manifest_path: &str,
    is_root: bool,
    workspace_bases: &BTreeMap<String, WorkspaceDependencyBase>,
    ws_package_fields: &BTreeMap<String, String>,
    warnings: &mut Vec<CargoInspectionWarning>,
    provenance: &ProvenanceAccumulator,
) -> DiscoveryResult<ParsedManifest> {
    let raw_str = std::str::from_utf8(content).map_err(|_| {
        DiscoveryError::InspectionFailure(format!("manifest {} is not valid UTF-8", manifest_path))
    })?;

    let manifest: toml::Value = toml::from_str(raw_str).map_err(|e| {
        let (line, col) = super::toml_helpers::toml_line_col_from_source(&e, content);
        let reason = super::toml_helpers::toml_malformed_reason(&e);
        DiscoveryError::InspectionFailure(format!(
            "{} at {} line {:?} col {:?}",
            reason, manifest_path, line, col
        ))
    })?;

    parse_manifest_from_value(
        &manifest,
        content,
        manifest_path,
        is_root,
        workspace_bases,
        ws_package_fields,
        warnings,
        provenance,
    )
}
