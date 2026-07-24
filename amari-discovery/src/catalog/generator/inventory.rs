// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic Cargo workspace package inventory.

use std::{
    collections::{BTreeMap, HashSet},
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
    /// Resolved Rust edition, defaulting to `2015` when omitted.
    pub edition: String,
    /// Workspace-relative manifest path using `/` separators.
    pub manifest_path: String,
    /// Declared library output kinds, sorted deterministically.
    pub library_outputs: Vec<String>,
    /// Cargo feature edges sorted by feature name.
    pub features: Vec<FeatureInventoryRecord>,
    /// Normal, development, build, and target-specific dependencies.
    pub dependencies: Vec<DependencyInventoryRecord>,
    /// Explicit and conventional library, binary, and example targets.
    pub targets: Vec<TargetInventoryRecord>,
}

/// A Cargo feature and the feature/dependency edges it enables.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FeatureInventoryRecord {
    /// Feature name.
    pub name: String,
    /// Raw Cargo feature edges, sorted and deduplicated.
    pub enables: Vec<String>,
}

/// Cargo dependency table classification.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DependencyKind {
    /// A normal package dependency.
    Normal,
    /// A build-script dependency.
    Build,
    /// A development-only dependency.
    Development,
}

/// A resolved dependency declaration.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyInventoryRecord {
    /// Local dependency key used by Rust source.
    pub alias: String,
    /// Actual Cargo package name after `package = ...` renaming.
    pub package: String,
    /// Dependency table kind.
    pub kind: DependencyKind,
    /// Optional target selector from `[target.'...']`.
    pub target: Option<String>,
    /// Resolved version requirement, when declared.
    pub version: Option<String>,
    /// Resolved manifest path text, when declared.
    pub path: Option<String>,
    /// Whether the dependency is optional.
    pub optional: bool,
    /// Whether Cargo default features are enabled.
    pub default_features: bool,
    /// Explicit dependency features, sorted and deduplicated.
    pub features: Vec<String>,
}

/// Cargo target classification.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum TargetKind {
    /// Library target.
    Library,
    /// Binary target.
    Binary,
    /// Example target.
    Example,
}

/// A classified Cargo package target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TargetInventoryRecord {
    /// Cargo target name.
    pub name: String,
    /// Target kind.
    pub kind: TargetKind,
    /// Manifest-relative source path.
    pub path: String,
    /// Features required to build the target.
    pub required_features: Vec<String>,
    /// Library crate types, empty for binary and example targets.
    pub crate_types: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Manifest {
    package: Option<ManifestPackage>,
    workspace: Option<ManifestWorkspace>,
    lib: Option<ManifestLibrary>,
    #[serde(default)]
    features: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    dependencies: BTreeMap<String, DependencySpec>,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: BTreeMap<String, DependencySpec>,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: BTreeMap<String, DependencySpec>,
    #[serde(default)]
    target: BTreeMap<String, TargetDependencyTables>,
    #[serde(default, rename = "bin")]
    bins: Vec<ManifestTarget>,
    #[serde(default, rename = "example")]
    examples: Vec<ManifestTarget>,
}

#[derive(Debug, Deserialize)]
struct ManifestPackage {
    name: String,
    version: InheritedString,
    description: InheritedString,
    license: InheritedString,
    edition: Option<InheritedString>,
    autolib: Option<bool>,
    autobins: Option<bool>,
    autoexamples: Option<bool>,
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
    #[serde(default)]
    dependencies: BTreeMap<String, DependencySpec>,
}

#[derive(Debug, Deserialize)]
struct WorkspacePackageDefaults {
    version: String,
    description: Option<String>,
    license: Option<String>,
    edition: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ManifestLibrary {
    name: Option<String>,
    path: Option<String>,
    #[serde(default, rename = "crate-type")]
    crate_types: Vec<String>,
    #[serde(default, rename = "proc-macro")]
    proc_macro: bool,
}

#[derive(Debug, Deserialize)]
struct ManifestTarget {
    name: String,
    path: Option<String>,
    #[serde(default, rename = "required-features")]
    required_features: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
enum DependencySpec {
    Version(String),
    Detail(DependencyDetail),
}

#[derive(Clone, Debug, Default, Deserialize)]
struct DependencyDetail {
    version: Option<String>,
    path: Option<String>,
    package: Option<String>,
    optional: Option<bool>,
    workspace: Option<bool>,
    #[serde(rename = "default-features")]
    default_features: Option<bool>,
    #[serde(default)]
    features: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct TargetDependencyTables {
    #[serde(default)]
    dependencies: BTreeMap<String, DependencySpec>,
    #[serde(default, rename = "dev-dependencies")]
    dev_dependencies: BTreeMap<String, DependencySpec>,
    #[serde(default, rename = "build-dependencies")]
    build_dependencies: BTreeMap<String, DependencySpec>,
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
/// inherited metadata and dependencies.
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
        let edition = match &package.edition {
            Some(edition) => resolve_field(
                edition,
                defaults.edition.as_ref(),
                "edition",
                &manifest_path,
            )?,
            None => "2015".into(),
        };
        if description.is_empty() || license.is_empty() || version.is_empty() || edition.is_empty()
        {
            return Err(DiscoveryError::CatalogCorruption(format!(
                "package metadata must be nonempty in {manifest_path}"
            )));
        }

        let package_dir = package_directory(&canonical_root, &manifest_path)?;
        let has_library = manifest.lib.is_some()
            || (package.autolib != Some(false) && package_dir.join("src/lib.rs").is_file());
        let library_outputs = if has_library {
            library_outputs(manifest.lib.as_ref())
        } else {
            Vec::new()
        };
        let dependencies = dependency_records(manifest, &workspace.dependencies, &manifest_path)?;
        packages.push(PackageInventoryRecord {
            name: package.name.clone(),
            version,
            description,
            license,
            edition: edition.clone(),
            manifest_path: normalize_path(&manifest_path),
            features: feature_records(&manifest.features, &dependencies),
            dependencies,
            targets: target_records(
                manifest,
                package,
                &package_dir,
                &library_outputs,
                has_library,
                &edition,
            )?,
            library_outputs,
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

fn package_directory(root: &Path, manifest_path: &str) -> DiscoveryResult<PathBuf> {
    let relative = Path::new(manifest_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let directory = fs::canonicalize(root.join(relative)).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "cannot resolve package directory for {manifest_path}: {error}"
        ))
    })?;
    if !directory.starts_with(root) {
        return Err(DiscoveryError::CatalogCorruption(format!(
            "package directory {} escapes workspace {}",
            directory.display(),
            root.display()
        )));
    }
    Ok(directory)
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

fn feature_records(
    features: &BTreeMap<String, Vec<String>>,
    dependencies: &[DependencyInventoryRecord],
) -> Vec<FeatureInventoryRecord> {
    let mut resolved = features.clone();
    let explicit_dependencies: HashSet<_> = features
        .values()
        .flatten()
        .filter_map(|edge| edge.strip_prefix("dep:"))
        .collect();
    for dependency in dependencies.iter().filter(|dependency| {
        dependency.kind != DependencyKind::Development
            && dependency.optional
            && !explicit_dependencies.contains(dependency.alias.as_str())
    }) {
        resolved
            .entry(dependency.alias.clone())
            .or_default()
            .push(format!("dep:{}", dependency.alias));
    }
    resolved
        .into_iter()
        .map(|(name, mut enables)| {
            enables.sort();
            enables.dedup();
            FeatureInventoryRecord { name, enables }
        })
        .collect()
}

fn dependency_records(
    manifest: &Manifest,
    workspace_dependencies: &BTreeMap<String, DependencySpec>,
    manifest_path: &str,
) -> DiscoveryResult<Vec<DependencyInventoryRecord>> {
    let mut records = Vec::new();
    append_dependencies(
        &mut records,
        &manifest.dependencies,
        workspace_dependencies,
        DependencyKind::Normal,
        None,
        manifest_path,
    )?;
    append_dependencies(
        &mut records,
        &manifest.build_dependencies,
        workspace_dependencies,
        DependencyKind::Build,
        None,
        manifest_path,
    )?;
    append_dependencies(
        &mut records,
        &manifest.dev_dependencies,
        workspace_dependencies,
        DependencyKind::Development,
        None,
        manifest_path,
    )?;
    for (target, tables) in &manifest.target {
        append_dependencies(
            &mut records,
            &tables.dependencies,
            workspace_dependencies,
            DependencyKind::Normal,
            Some(target),
            manifest_path,
        )?;
        append_dependencies(
            &mut records,
            &tables.build_dependencies,
            workspace_dependencies,
            DependencyKind::Build,
            Some(target),
            manifest_path,
        )?;
        append_dependencies(
            &mut records,
            &tables.dev_dependencies,
            workspace_dependencies,
            DependencyKind::Development,
            Some(target),
            manifest_path,
        )?;
    }
    records.sort_by(|left, right| {
        (&left.target, left.kind, &left.alias).cmp(&(&right.target, right.kind, &right.alias))
    });
    Ok(records)
}

fn append_dependencies(
    records: &mut Vec<DependencyInventoryRecord>,
    dependencies: &BTreeMap<String, DependencySpec>,
    workspace_dependencies: &BTreeMap<String, DependencySpec>,
    kind: DependencyKind,
    target: Option<&String>,
    manifest_path: &str,
) -> DiscoveryResult<()> {
    for (alias, specification) in dependencies {
        let detail =
            resolve_dependency(alias, specification, workspace_dependencies, manifest_path)?;
        if kind == DependencyKind::Development && detail.optional == Some(true) {
            return Err(DiscoveryError::CatalogCorruption(format!(
                "{manifest_path} declares optional development dependency {alias}"
            )));
        }
        let mut features = detail.features;
        features.sort();
        features.dedup();
        records.push(DependencyInventoryRecord {
            alias: alias.clone(),
            package: detail.package.unwrap_or_else(|| alias.clone()),
            kind,
            target: target.cloned(),
            version: detail.version,
            path: detail.path.map(|path| normalize_path(&path)),
            optional: detail.optional.unwrap_or(false),
            default_features: detail.default_features.unwrap_or(true),
            features,
        });
    }
    Ok(())
}

fn resolve_dependency(
    alias: &str,
    specification: &DependencySpec,
    workspace_dependencies: &BTreeMap<String, DependencySpec>,
    manifest_path: &str,
) -> DiscoveryResult<DependencyDetail> {
    let member = dependency_detail(specification);
    match member.workspace {
        Some(false) => {
            return Err(DiscoveryError::CatalogCorruption(format!(
                "{manifest_path} has invalid dependency {alias}.workspace = false"
            )));
        }
        None => return Ok(member),
        Some(true) => {}
    }
    let base_specification = workspace_dependencies.get(alias).ok_or_else(|| {
        DiscoveryError::CatalogCorruption(format!(
            "{manifest_path} inherits dependency {alias}, but [workspace.dependencies].{alias} is missing"
        ))
    })?;
    let mut base = dependency_detail(base_specification);
    if base.workspace.is_some() {
        return Err(DiscoveryError::CatalogCorruption(format!(
            "[workspace.dependencies].{alias} cannot set workspace"
        )));
    }
    if member.optional.is_some() {
        base.optional = member.optional;
    }
    if member.default_features.is_some() {
        base.default_features = member.default_features;
    }
    base.features.extend(member.features);
    Ok(base)
}

fn dependency_detail(specification: &DependencySpec) -> DependencyDetail {
    match specification {
        DependencySpec::Version(version) => DependencyDetail {
            version: Some(version.clone()),
            ..DependencyDetail::default()
        },
        DependencySpec::Detail(detail) => detail.clone(),
    }
}

fn target_records(
    manifest: &Manifest,
    package: &ManifestPackage,
    package_dir: &Path,
    library_outputs: &[String],
    has_library: bool,
    edition: &str,
) -> DiscoveryResult<Vec<TargetInventoryRecord>> {
    let mut targets = BTreeMap::new();
    let explicit_bins: HashSet<_> = manifest
        .bins
        .iter()
        .map(|target| target.name.as_str())
        .collect();
    let explicit_examples: HashSet<_> = manifest
        .examples
        .iter()
        .map(|target| target.name.as_str())
        .collect();
    if has_library {
        let library = manifest.lib.as_ref();
        let path = library
            .and_then(|record| record.path.as_deref())
            .unwrap_or("src/lib.rs");
        let record = TargetInventoryRecord {
            name: library
                .and_then(|record| record.name.clone())
                .unwrap_or_else(|| package.name.replace('-', "_")),
            kind: TargetKind::Library,
            path: validated_target_path(package_dir, path)?,
            required_features: Vec::new(),
            crate_types: library_outputs.to_vec(),
        };
        insert_unique_target(&mut targets, record)?;
    }

    let autobins = package
        .autobins
        .unwrap_or(edition != "2015" || manifest.bins.is_empty());
    if autobins {
        if package_dir.join("src/main.rs").is_file()
            && !explicit_bins.contains(package.name.as_str())
        {
            let record = TargetInventoryRecord {
                name: package.name.clone(),
                kind: TargetKind::Binary,
                path: validated_target_path(package_dir, "src/main.rs")?,
                required_features: Vec::new(),
                crate_types: Vec::new(),
            };
            insert_unique_target(&mut targets, record)?;
        }
        for record in discover_conventional_targets(package_dir, "src/bin", TargetKind::Binary)? {
            if !explicit_bins.contains(record.name.as_str()) {
                insert_unique_target(&mut targets, record)?;
            }
        }
    }
    let autoexamples = package
        .autoexamples
        .unwrap_or(edition != "2015" || manifest.examples.is_empty());
    if autoexamples {
        for record in discover_conventional_targets(package_dir, "examples", TargetKind::Example)? {
            if !explicit_examples.contains(record.name.as_str()) {
                insert_unique_target(&mut targets, record)?;
            }
        }
    }

    for target in &manifest.bins {
        let record = manifest_target(package_dir, target, TargetKind::Binary, &package.name)?;
        insert_unique_target(&mut targets, record)?;
    }
    for target in &manifest.examples {
        let record = manifest_target(package_dir, target, TargetKind::Example, &package.name)?;
        insert_unique_target(&mut targets, record)?;
    }
    Ok(targets.into_values().collect())
}

fn insert_unique_target(
    targets: &mut BTreeMap<(TargetKind, String), TargetInventoryRecord>,
    record: TargetInventoryRecord,
) -> DiscoveryResult<()> {
    let key = (record.kind, record.name.clone());
    if targets.insert(key, record).is_some() {
        return Err(DiscoveryError::CatalogCorruption(
            "duplicate Cargo target name and kind".into(),
        ));
    }
    Ok(())
}

fn discover_conventional_targets(
    package_dir: &Path,
    relative_directory: &str,
    kind: TargetKind,
) -> DiscoveryResult<Vec<TargetInventoryRecord>> {
    let directory = package_dir.join(relative_directory);
    if !directory.exists() {
        return Ok(Vec::new());
    }
    let resolved_directory = fs::canonicalize(&directory).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "cannot resolve target directory {}: {error}",
            directory.display()
        ))
    })?;
    if !resolved_directory.starts_with(package_dir) {
        return Err(DiscoveryError::CatalogCorruption(format!(
            "target directory {} escapes package {}",
            resolved_directory.display(),
            package_dir.display()
        )));
    }

    let entries = fs::read_dir(&directory).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "cannot read target directory {}: {error}",
            directory.display()
        ))
    })?;
    let mut records = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| {
            DiscoveryError::CatalogCorruption(format!(
                "cannot read entry in {}: {error}",
                directory.display()
            ))
        })?;
        let path = entry.path();
        let (name, relative_path) = if path.is_file()
            && path.extension().and_then(|extension| extension.to_str()) == Some("rs")
        {
            let Some(name) = path.file_stem().and_then(|name| name.to_str()) else {
                return Err(DiscoveryError::CatalogCorruption(format!(
                    "target filename is not UTF-8: {}",
                    path.display()
                )));
            };
            (name.to_owned(), format!("{relative_directory}/{name}.rs"))
        } else if path.is_dir() && path.join("main.rs").is_file() {
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return Err(DiscoveryError::CatalogCorruption(format!(
                    "target directory name is not UTF-8: {}",
                    path.display()
                )));
            };
            (
                name.to_owned(),
                format!("{relative_directory}/{name}/main.rs"),
            )
        } else {
            continue;
        };
        records.push(TargetInventoryRecord {
            name,
            kind,
            path: validated_target_path(package_dir, &relative_path)?,
            required_features: Vec::new(),
            crate_types: Vec::new(),
        });
    }
    records.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(records)
}

fn manifest_target(
    package_dir: &Path,
    target: &ManifestTarget,
    kind: TargetKind,
    package_name: &str,
) -> DiscoveryResult<TargetInventoryRecord> {
    let mut required_features = target.required_features.clone();
    required_features.sort();
    required_features.dedup();
    let path = match &target.path {
        Some(path) => path.clone(),
        None => default_manifest_target_path(package_dir, kind, &target.name, package_name)?,
    };
    Ok(TargetInventoryRecord {
        name: target.name.clone(),
        kind,
        path: validated_target_path(package_dir, &path)?,
        required_features,
        crate_types: Vec::new(),
    })
}

fn default_manifest_target_path(
    package_dir: &Path,
    kind: TargetKind,
    name: &str,
    package_name: &str,
) -> DiscoveryResult<String> {
    let mut candidates = Vec::new();
    match kind {
        TargetKind::Binary => {
            if name == package_name && package_dir.join("src/main.rs").is_file() {
                return Ok("src/main.rs".into());
            }
            candidates.push(format!("src/bin/{name}.rs"));
            candidates.push(format!("src/bin/{name}/main.rs"));
        }
        TargetKind::Example => {
            candidates.push(format!("examples/{name}.rs"));
            candidates.push(format!("examples/{name}/main.rs"));
        }
        TargetKind::Library => {
            return Err(DiscoveryError::CatalogCorruption(
                "library target path resolution used the wrong target kind".into(),
            ));
        }
    }
    let existing: Vec<_> = candidates
        .into_iter()
        .filter(|candidate| package_dir.join(candidate).is_file())
        .collect();
    match existing.as_slice() {
        [path] => Ok(path.clone()),
        [] => Err(DiscoveryError::CatalogCorruption(format!(
            "cannot find default source for {kind:?} target {name} in {}",
            package_dir.display()
        ))),
        _ => Err(DiscoveryError::CatalogCorruption(format!(
            "multiple default sources exist for {kind:?} target {name} in {}",
            package_dir.display()
        ))),
    }
}

fn validated_target_path(package_dir: &Path, relative: &str) -> DiscoveryResult<String> {
    let relative_path = Path::new(relative);
    if relative_path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(DiscoveryError::CatalogCorruption(format!(
            "target path is absolute or escapes its package: {relative}"
        )));
    }
    let candidate = package_dir.join(relative_path);
    let resolved = fs::canonicalize(&candidate).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "cannot resolve target source {}: {error}",
            candidate.display()
        ))
    })?;
    if !resolved.starts_with(package_dir) || !resolved.is_file() {
        return Err(DiscoveryError::CatalogCorruption(format!(
            "target source {} escapes its package or is not a file",
            resolved.display()
        )));
    }
    Ok(normalize_path(relative))
}

fn library_outputs(library: Option<&ManifestLibrary>) -> Vec<String> {
    let mut outputs = match library {
        Some(library) if library.proc_macro => vec!["proc-macro".into()],
        Some(library) if !library.crate_types.is_empty() => library.crate_types.clone(),
        Some(_) | None => vec!["lib".into()],
    };
    outputs.sort();
    outputs.dedup();
    outputs
}

fn normalize_path(path: &str) -> String {
    path.replace(std::path::MAIN_SEPARATOR, "/")
}
