// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{
    fs,
    path::{Path, PathBuf},
};

use amari_discovery::catalog::generator::{inventory_workspace, DependencyKind, TargetKind};
use tempfile::TempDir;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/catalog-workspace")
}

fn workspace_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("amari-discovery is inside the workspace")
}

#[test]
fn fixture_inventory_resolves_features_dependencies_and_targets() {
    let inventory = inventory_workspace(&fixture_root()).unwrap();
    let root = inventory.package("fixture-root").unwrap();

    let feature_names: Vec<_> = root
        .features
        .iter()
        .map(|feature| feature.name.as_str())
        .collect();
    assert_eq!(
        feature_names,
        [
            "build-helper",
            "default",
            "std",
            "target-build",
            "tooling",
            "wasm-only",
        ]
    );
    let feature = |name: &str| {
        root.features
            .iter()
            .find(|feature| feature.name == name)
            .unwrap()
    };
    assert_eq!(feature("build-helper").enables, ["dep:build-helper"]);
    assert_eq!(feature("default").enables, ["std"]);
    assert!(feature("std").enables.is_empty());
    assert_eq!(
        feature("tooling").enables,
        ["dep:renamed-math", "renamed-math/fast"]
    );
    assert_eq!(feature("target-build").enables, ["dep:target-build"]);
    assert_eq!(feature("wasm-only").enables, ["dep:wasm-only"]);

    let renamed = root
        .dependencies
        .iter()
        .find(|dependency| dependency.alias == "renamed-math")
        .unwrap();
    assert_eq!(renamed.package, "fixture-math");
    assert_eq!(renamed.kind, DependencyKind::Normal);
    assert_eq!(renamed.target, None);
    assert_eq!(renamed.path.as_deref(), Some("crates/math"));
    assert_eq!(renamed.version, None);
    assert!(renamed.optional);
    assert!(renamed.default_features);

    let shared = root
        .dependencies
        .iter()
        .find(|dependency| dependency.alias == "shared")
        .unwrap();
    assert_eq!(shared.package, "fixture-shared");
    assert_eq!(shared.version.as_deref(), Some("2"));
    assert_eq!(shared.features, ["base"]);
    assert!(!shared.optional);
    assert!(!shared.default_features);
    assert_eq!(shared.path.as_deref(), Some("vendor/shared"));

    let target = root
        .dependencies
        .iter()
        .find(|dependency| dependency.alias == "wasm-only")
        .unwrap();
    assert_eq!(target.package, "fixture-wasm-support");
    assert_eq!(
        target.target.as_deref(),
        Some("cfg(target_arch = \"wasm32\")")
    );
    assert!(target.optional);

    for (alias, kind, target_selector) in [
        ("build-helper", DependencyKind::Build, None),
        ("dev-helper", DependencyKind::Development, None),
        (
            "target-build",
            DependencyKind::Build,
            Some("cfg(target_arch = \"wasm32\")"),
        ),
        (
            "target-dev",
            DependencyKind::Development,
            Some("cfg(target_arch = \"wasm32\")"),
        ),
    ] {
        let dependency = root
            .dependencies
            .iter()
            .find(|dependency| dependency.alias == alias)
            .unwrap();
        assert_eq!(dependency.kind, kind);
        assert_eq!(dependency.target.as_deref(), target_selector);
    }
    assert!(
        !root
            .dependencies
            .iter()
            .find(|dependency| dependency.alias == "build-helper")
            .unwrap()
            .default_features
    );

    assert_eq!(root.targets.len(), 5);
    assert_eq!(root.targets[0].kind, TargetKind::Library);
    assert_eq!(root.targets[0].name, "fixture_root");
    assert_eq!(root.targets[0].path, "src/lib.rs");
    assert_eq!(root.targets[0].crate_types, ["rlib"]);
    let directory_bin = root
        .targets
        .iter()
        .find(|target| target.name == "directory-tool")
        .unwrap();
    assert_eq!(directory_bin.kind, TargetKind::Binary);
    assert_eq!(directory_bin.path, "src/bin/directory-tool/main.rs");
    let explicit_bin = root
        .targets
        .iter()
        .find(|target| target.name == "fixture-tool")
        .unwrap();
    assert_eq!(explicit_bin.kind, TargetKind::Binary);
    assert_eq!(explicit_bin.required_features, ["tooling"]);
    let directory_example = root
        .targets
        .iter()
        .find(|target| target.name == "directory_demo")
        .unwrap();
    assert_eq!(directory_example.kind, TargetKind::Example);
    assert_eq!(directory_example.path, "examples/directory_demo/main.rs");
    let explicit_example = root
        .targets
        .iter()
        .find(|target| target.name == "fixture_demo")
        .unwrap();
    assert_eq!(explicit_example.kind, TargetKind::Example);
    assert_eq!(explicit_example.required_features, ["std"]);
    assert!(root
        .targets
        .iter()
        .all(|target| !matches!(target.name.as_str(), "fixture-root" | "auto_ignored")));

    let math = inventory.package("fixture-math").unwrap();
    assert!(math.targets.iter().any(|target| {
        target.kind == TargetKind::Example
            && target.name == "implicit"
            && target.path == "examples/implicit.rs"
    }));
    assert!(math.targets.iter().any(|target| {
        target.kind == TargetKind::Binary
            && target.name == "fixture-math"
            && target.path == "src/main.rs"
    }));
    let wasm = inventory.package("fixture-wasm").unwrap();
    assert!(wasm.targets.iter().all(|target| target.name != "ignored"));
}

#[test]
fn workspace_dependency_inheritance_merges_member_overrides() {
    let inventory = inventory_workspace(&fixture_root()).unwrap();
    let math = inventory.package("fixture-math").unwrap();
    let shared = math
        .dependencies
        .iter()
        .find(|dependency| dependency.alias == "shared")
        .unwrap();

    assert_eq!(shared.package, "fixture-shared");
    assert_eq!(shared.version.as_deref(), Some("2"));
    assert_eq!(shared.features, ["base", "extra"]);
    assert!(shared.optional);
    assert!(shared.default_features);
    assert_eq!(shared.path.as_deref(), Some("vendor/shared"));
}

#[test]
fn real_workspace_links_match_cargo_manifests() {
    let inventory = inventory_workspace(workspace_root()).unwrap();
    let tropical = inventory.package("amari-tropical").unwrap();

    let high_precision = tropical
        .features
        .iter()
        .find(|feature| feature.name == "high-precision")
        .unwrap();
    assert_eq!(high_precision.enables, ["amari-core/high-precision"]);
    for optional in ["bytemuck", "futures", "pollster", "serde", "wgpu"] {
        let feature = tropical
            .features
            .iter()
            .find(|feature| feature.name == optional)
            .unwrap();
        assert_eq!(feature.enables, [format!("dep:{optional}")]);
    }
    assert_eq!(
        tropical
            .targets
            .iter()
            .filter(|target| target.kind == TargetKind::Example)
            .count(),
        3
    );

    let core = tropical
        .dependencies
        .iter()
        .find(|dependency| dependency.alias == "amari-core")
        .unwrap();
    assert_eq!(core.package, "amari-core");
    assert_eq!(core.version.as_deref(), Some(env!("CARGO_PKG_VERSION")));
    assert_eq!(core.path.as_deref(), Some("amari-core"));
    assert_eq!(core.kind, DependencyKind::Normal);
    assert!(!core.optional);

    let serde = tropical
        .dependencies
        .iter()
        .find(|dependency| dependency.alias == "serde")
        .unwrap();
    assert!(serde.optional);
    assert_eq!(serde.version.as_deref(), Some("1.0"));

    let root = inventory.package("amari").unwrap();
    assert!(root.targets.iter().any(|target| {
        target.kind == TargetKind::Example
            && target.name == "networked_physics"
            && target.required_features == ["deterministic"]
    }));
    let network = inventory.package("amari-network").unwrap();
    assert_eq!(
        network
            .targets
            .iter()
            .filter(|target| target.kind == TargetKind::Example)
            .count(),
        5
    );
}

#[test]
fn workspace_dependency_errors_are_typed_and_contextual() {
    let temp = TempDir::new().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n\n[workspace.package]\nversion = \"1\"\ndescription = \"test\"\nlicense = \"Apache-2.0\"\n",
    )
    .unwrap();
    fs::create_dir_all(temp.path().join("member")).unwrap();
    let package_prefix = "[package]\nname = \"member\"\nversion.workspace = true\ndescription.workspace = true\nlicense.workspace = true\n\n[dependencies]\n";

    fs::write(
        temp.path().join("member/Cargo.toml"),
        format!("{package_prefix}missing = {{ workspace = true }}\n"),
    )
    .unwrap();
    let error = inventory_workspace(temp.path()).unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error
        .to_string()
        .contains("[workspace.dependencies].missing is missing"));

    fs::write(
        temp.path().join("member/Cargo.toml"),
        format!("{package_prefix}invalid = {{ workspace = false, version = \"1\" }}\n"),
    )
    .unwrap();
    let error = inventory_workspace(temp.path()).unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error
        .to_string()
        .contains("dependency invalid.workspace = false"));

    fs::write(
        temp.path().join("member/Cargo.toml"),
        format!(
            "{package_prefix}[dev-dependencies]\ninvalid-dev = {{ version = \"1\", optional = true }}\n"
        ),
    )
    .unwrap();
    let error = inventory_workspace(temp.path()).unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error
        .to_string()
        .contains("optional development dependency invalid-dev"));
}

#[test]
fn duplicate_conventional_target_names_are_rejected() {
    let temp = TempDir::new().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n\n[workspace.package]\nversion = \"1\"\ndescription = \"test\"\nlicense = \"Apache-2.0\"\n",
    )
    .unwrap();
    fs::create_dir_all(temp.path().join("member/src")).unwrap();
    fs::create_dir_all(temp.path().join("member/examples/duplicate")).unwrap();
    fs::write(
        temp.path().join("member/Cargo.toml"),
        "[package]\nname = \"member\"\nversion.workspace = true\ndescription.workspace = true\nlicense.workspace = true\n",
    )
    .unwrap();
    fs::write(temp.path().join("member/src/lib.rs"), "").unwrap();
    fs::write(temp.path().join("member/examples/duplicate.rs"), "").unwrap();
    fs::write(temp.path().join("member/examples/duplicate/main.rs"), "").unwrap();

    let error = inventory_workspace(temp.path()).unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("duplicate Cargo target"));
}

#[test]
fn feature_dependency_and_target_records_are_deterministic() {
    let first = inventory_workspace(workspace_root()).unwrap();
    let second = inventory_workspace(workspace_root()).unwrap();
    assert_eq!(first, second);

    for package in &first.packages {
        assert!(package
            .features
            .windows(2)
            .all(|pair| pair[0].name < pair[1].name));
        assert!(package.dependencies.windows(2).all(|pair| {
            (&pair[0].target, pair[0].kind, &pair[0].alias)
                < (&pair[1].target, pair[1].kind, &pair[1].alias)
        }));
        assert!(package
            .targets
            .windows(2)
            .all(|pair| { (pair[0].kind, &pair[0].name) < (pair[1].kind, &pair[1].name) }));
    }
}
