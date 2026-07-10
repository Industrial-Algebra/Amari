// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic Cargo workspace package inventory.

use std::{
    collections::HashSet,
    fs,
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;

use crate::{DiscoveryError, DiscoveryResult};

/// Metadata inventory for an Amari workspace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceInventory {
    /// Version inherited from `[workspace.package]`.
    pub workspace_version: String,
    /// Included packages sorted by package name.
    pub packages: Vec<PackageInventoryRecord>,
}

impl WorkspaceInventory {
    /// Finds a package by its Cargo package name.
    pub fn package(&self, name: &str) -> Option<&PackageInventoryRecord> {
        self.packages.iter().find(|package| package.name == name)
    }
}

/// Selected Cargo package metadata needed by structural catalog generation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackageInventoryRecord {
    /// Cargo package name.
    pub name: String,
    /// Resolved package version.
    pub version: String,
    /// Resolved package description.
    pub description: String,
    /// Resolved SPDX license expression.
    pub license: String,
    /// Workspace-relative manifest path using `/` separators.
    pub manifest_path: String,
    /// Declared library output kinds, sorted deterministically.
    pub targets: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    package: Option<ManifestPackage>,
    workspace: Option<ManifestWorkspace>,
    lib: Option<ManifestLibrary>,
}

#[derive(Debug, Deserialize)]
struct ManifestPackage {
    name: String,
    version: InheritedString,
    description: InheritedString,
    license: InheritedString,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum InheritedString {
    Value(String),
    Workspace { workspace: bool },
}

#[derive(Debug, Deserialize)]
struct ManifestWorkspace {
    members: Vec<String>,
    package: WorkspacePackageDefaults,
}

#[derive(Debug, Deserialize)]
struct WorkspacePackageDefaults {
    version: String,
    description: Option<String>,
    license: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ManifestLibrary {
    #[serde(default, rename = "crate-type")]
    crate_types: Vec<String>,
    #[serde(default, rename = "proc-macro")]
    proc_macro: bool,
}

/// Inventories selected package metadata from a Cargo workspace without
/// invoking Cargo, build scripts, dependencies, or the network.
///
/// The root package is included when present. The `amari-discovery` package is
/// excluded to prevent the generated catalog from indexing itself.
///
/// # Errors
///
/// Returns a catalog-corruption error when manifests are missing, malformed,
/// escape the workspace, contain unsupported member patterns, or omit required
/// inherited metadata.
pub fn inventory_workspace(root: &Path) -> DiscoveryResult<WorkspaceInventory> {
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "cannot resolve workspace root {}: {error}",
            root.display()
        ))
    })?;
    let root_manifest_path = contained_manifest(&canonical_root, Path::new("Cargo.toml"))?;
    let root_manifest = read_manifest(&root_manifest_path)?;
    let workspace = root_manifest.workspace.as_ref().ok_or_else(|| {
        DiscoveryError::CatalogCorruption("root Cargo.toml has no [workspace] table".into())
    })?;

    let mut manifests = Vec::with_capacity(workspace.members.len() + 1);
    if root_manifest.package.is_some() {
        manifests.push(("Cargo.toml".to_owned(), &root_manifest));
    }

    let mut member_manifests = Vec::with_capacity(workspace.members.len());
    for member in &workspace.members {
        validate_member_path(member)?;
        let relative_manifest = format!("{member}/Cargo.toml");
        let manifest_path = contained_manifest(&canonical_root, Path::new(&relative_manifest))?;
        let manifest = read_manifest(&manifest_path)?;
        member_manifests.push((relative_manifest, manifest));
    }
    manifests.extend(
        member_manifests
            .iter()
            .map(|(path, manifest)| (path.clone(), manifest)),
    );

    let defaults = &workspace.package;
    let mut names = HashSet::new();
    let mut packages = Vec::with_capacity(manifests.len());
    for (manifest_path, manifest) in manifests {
        let package = manifest.package.as_ref().ok_or_else(|| {
            DiscoveryError::CatalogCorruption(format!(
                "workspace member {manifest_path} has no [package] table"
            ))
        })?;
        if package.name == "amari-discovery" {
            continue;
        }
        if !names.insert(package.name.as_str()) {
            return Err(DiscoveryError::CatalogCorruption(format!(
                "duplicate workspace package name {}",
                package.name
            )));
        }

        let version = resolve_field(
            &package.version,
            Some(&defaults.version),
            "version",
            &manifest_path,
        )?;
        let description = resolve_field(
            &package.description,
            defaults.description.as_ref(),
            "description",
            &manifest_path,
        )?;
        let license = resolve_field(
            &package.license,
            defaults.license.as_ref(),
            "license",
            &manifest_path,
        )?;
        if description.is_empty() || license.is_empty() || version.is_empty() {
            return Err(DiscoveryError::CatalogCorruption(format!(
                "package metadata must be nonempty in {manifest_path}"
            )));
        }

        packages.push(PackageInventoryRecord {
            name: package.name.clone(),
            version,
            description,
            license,
            manifest_path: manifest_path.replace(std::path::MAIN_SEPARATOR, "/"),
            targets: library_targets(manifest.lib.as_ref()),
        });
    }

    packages.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(WorkspaceInventory {
        workspace_version: defaults.version.clone(),
        packages,
    })
}

fn contained_manifest(root: &Path, relative: &Path) -> DiscoveryResult<PathBuf> {
    let candidate = root.join(relative);
    let resolved = fs::canonicalize(&candidate).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "cannot resolve {}: {error}",
            candidate.display()
        ))
    })?;
    if !resolved.starts_with(root) {
        return Err(DiscoveryError::CatalogCorruption(format!(
            "workspace manifest {} escapes the workspace root {}",
            resolved.display(),
            root.display()
        )));
    }
    Ok(resolved)
}

fn read_manifest(path: &Path) -> DiscoveryResult<Manifest> {
    let source = fs::read_to_string(path).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!("cannot read {}: {error}", path.display()))
    })?;
    toml::from_str(&source).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!("invalid {}: {error}", path.display()))
    })
}

fn resolve_field(
    field: &InheritedString,
    workspace_default: Option<&String>,
    name: &str,
    manifest_path: &str,
) -> DiscoveryResult<String> {
    match field {
        InheritedString::Value(value) => Ok(value.clone()),
        InheritedString::Workspace { workspace: true } => {
            workspace_default.cloned().ok_or_else(|| {
                DiscoveryError::CatalogCorruption(format!(
                    "{manifest_path} inherits {name}, but [workspace.package].{name} is missing"
                ))
            })
        }
        InheritedString::Workspace { workspace: false } => Err(DiscoveryError::CatalogCorruption(
            format!("{manifest_path} has invalid {name}.workspace = false"),
        )),
    }
}

fn validate_member_path(member: &str) -> DiscoveryResult<()> {
    let path = Path::new(member);
    let valid = !member.is_empty()
        && !member.contains(['*', '?', '[', ']'])
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if valid {
        Ok(())
    } else {
        Err(DiscoveryError::CatalogCorruption(format!(
            "workspace member path is unsupported or escapes the root: {member}"
        )))
    }
}

fn library_targets(library: Option<&ManifestLibrary>) -> Vec<String> {
    let mut targets = match library {
        Some(library) if library.proc_macro => vec!["proc-macro".into()],
        Some(library) if !library.crate_types.is_empty() => library.crate_types.clone(),
        Some(_) | None => vec!["lib".into()],
    };
    targets.sort();
    targets.dedup();
    targets
}
