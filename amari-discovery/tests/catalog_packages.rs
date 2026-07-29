// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{
    fs,
    path::{Path, PathBuf},
};

use amari_discovery::catalog::generator::inventory_workspace;
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
fn fixture_inventory_resolves_inherited_metadata_and_library_targets() {
    let inventory = inventory_workspace(&fixture_root()).unwrap();
    let names: Vec<_> = inventory
        .packages
        .iter()
        .map(|package| package.name.as_str())
        .collect();

    assert_eq!(inventory.workspace_version, "9.8.7");
    assert_eq!(names, ["fixture-math", "fixture-root", "fixture-wasm"]);
    assert!(!names.contains(&"amari-discovery"));

    let math = inventory.package("fixture-math").unwrap();
    assert_eq!(math.version, "9.8.7");
    assert_eq!(math.license, "Apache-2.0");
    assert_eq!(math.edition, "2015");
    assert_eq!(math.description, "Inherited fixture description");
    assert_eq!(math.library_outputs, ["lib"]);
    assert_eq!(math.manifest_path, "crates/math/Cargo.toml");

    let root = inventory.package("fixture-root").unwrap();
    assert_eq!(root.library_outputs, ["rlib"]);
    assert_eq!(root.manifest_path, "Cargo.toml");

    let wasm = inventory.package("fixture-wasm").unwrap();
    assert_eq!(wasm.library_outputs, ["cdylib", "rlib"]);
}

#[test]
fn real_inventory_includes_root_and_wasm_but_excludes_discovery() {
    let inventory = inventory_workspace(workspace_root()).unwrap();

    assert!(inventory.package("amari").is_some());
    let wasm = inventory.package("amari-wasm").unwrap();
    assert_eq!(wasm.library_outputs, ["cdylib"]);
    assert!(inventory.package("amari-discovery").is_none());
    let names: Vec<_> = inventory
        .packages
        .iter()
        .map(|package| package.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "amari",
            "amari-automata",
            "amari-calculus",
            "amari-cgt",
            "amari-core",
            "amari-discovery-macros",
            "amari-dual",
            "amari-dynamics",
            "amari-enumerative",
            "amari-flynn",
            "amari-flynn-macros",
            "amari-functional",
            "amari-fusion",
            "amari-gpu",
            "amari-holographic",
            "amari-info-geom",
            "amari-measure",
            "amari-network",
            "amari-optimization",
            "amari-probabilistic",
            "amari-relativistic",
            "amari-rewrite",
            "amari-surcomplex",
            "amari-surreal",
            "amari-topology",
            "amari-tropical",
            "amari-wasm",
        ]
    );

    assert_eq!(
        inventory.package("amari").unwrap().description,
        "Advanced mathematical computing library with geometric algebra, tropical algebra, and automatic differentiation"
    );
    assert_eq!(
        inventory.package("amari-core").unwrap().description,
        "Core geometric algebra and mathematical structures"
    );
    assert_eq!(
        inventory.package("amari-flynn-macros").unwrap().description,
        "Procedural macros for amari-flynn probabilistic contracts"
    );
    assert_eq!(
        inventory
            .package("amari-discovery-macros")
            .unwrap()
            .description,
        "Bounded wire-contract derive macros for amari-discovery probe DTOs"
    );
    assert_eq!(
        inventory.package("amari-wasm").unwrap().description,
        "WebAssembly bindings for Amari mathematical computing library - geometric algebra, tropical algebra, automatic differentiation, measure theory, fusion systems, and information geometry"
    );
    assert_eq!(inventory.package("amari").unwrap().library_outputs, ["lib"]);
    assert_eq!(
        inventory
            .package("amari-flynn-macros")
            .unwrap()
            .library_outputs,
        ["proc-macro"]
    );
    for package in &inventory.packages {
        if !matches!(
            package.name.as_str(),
            "amari-wasm" | "amari-flynn-macros" | "amari-discovery-macros"
        ) {
            assert_eq!(package.library_outputs, ["lib"], "{} targets", package.name);
        }
    }

    for package in &inventory.packages {
        assert_eq!(
            package.version,
            env!("CARGO_PKG_VERSION"),
            "{} version",
            package.name
        );
        assert_eq!(package.edition, "2021", "{} edition", package.name);
        assert_eq!(
            package.license, "MIT OR Apache-2.0",
            "{} license",
            package.name
        );
        assert!(
            !package.description.is_empty(),
            "{} description",
            package.name
        );
        assert!(
            !package.library_outputs.is_empty(),
            "{} targets",
            package.name
        );
        assert!(package.manifest_path.ends_with("Cargo.toml"));
        assert!(!package.manifest_path.starts_with('/'));
    }
}

#[test]
fn inventory_is_deterministic_and_sorted() {
    let first = inventory_workspace(workspace_root()).unwrap();
    let second = inventory_workspace(workspace_root()).unwrap();

    assert_eq!(first, second);
    assert!(first
        .packages
        .windows(2)
        .all(|pair| pair[0].name < pair[1].name));
    for package in &first.packages {
        assert!(package
            .library_outputs
            .windows(2)
            .all(|pair| pair[0] < pair[1]));
    }
}

fn write_file(path: &Path, source: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, source).unwrap();
}

fn virtual_workspace(root: &Path, members: &str, include_description: bool) {
    let description = if include_description {
        "description = \"Inherited description\"\n"
    } else {
        ""
    };
    write_file(
        &root.join("Cargo.toml"),
        &format!(
            "[workspace]\nmembers = [{members}]\n\n[workspace.package]\nversion = \"1.2.3\"\n{description}license = \"Apache-2.0\"\n"
        ),
    );
}

fn inherited_package(name: &str) -> String {
    format!(
        "[package]\nname = \"{name}\"\nversion.workspace = true\ndescription.workspace = true\nlicense.workspace = true\n"
    )
}

#[test]
fn invalid_member_paths_and_missing_manifests_are_typed_errors() {
    for member in ["../escape", "/absolute/escape", "crates/*"] {
        let temp = TempDir::new().unwrap();
        virtual_workspace(temp.path(), &format!("\"{member}\""), true);
        let error = inventory_workspace(temp.path()).unwrap_err();
        assert_eq!(error.kind(), "catalog_corruption");
        assert!(error.to_string().contains(member));
    }

    let temp = TempDir::new().unwrap();
    virtual_workspace(temp.path(), "\"missing\"", true);
    let error = inventory_workspace(temp.path()).unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("missing/Cargo.toml"));
}

#[test]
fn malformed_duplicate_and_invalid_inheritance_are_typed_errors() {
    let malformed = TempDir::new().unwrap();
    virtual_workspace(malformed.path(), "\"member\"", true);
    write_file(&malformed.path().join("member/Cargo.toml"), "not = [");
    let error = inventory_workspace(malformed.path()).unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("member/Cargo.toml"));

    let duplicate = TempDir::new().unwrap();
    virtual_workspace(duplicate.path(), "\"a\", \"b\"", true);
    write_file(
        &duplicate.path().join("a/Cargo.toml"),
        &inherited_package("same"),
    );
    write_file(
        &duplicate.path().join("b/Cargo.toml"),
        &inherited_package("same"),
    );
    let error = inventory_workspace(duplicate.path()).unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error
        .to_string()
        .contains("duplicate workspace package name same"));

    let missing_default = TempDir::new().unwrap();
    virtual_workspace(missing_default.path(), "\"member\"", false);
    write_file(
        &missing_default.path().join("member/Cargo.toml"),
        &inherited_package("missing-default"),
    );
    let error = inventory_workspace(missing_default.path()).unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("inherits description"));

    let missing_license = TempDir::new().unwrap();
    write_file(
        &missing_license.path().join("Cargo.toml"),
        "[workspace]\nmembers = [\"member\"]\n\n[workspace.package]\nversion = \"1.2.3\"\ndescription = \"Inherited description\"\n",
    );
    write_file(
        &missing_license.path().join("member/Cargo.toml"),
        &inherited_package("missing-license"),
    );
    let error = inventory_workspace(missing_license.path()).unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("inherits license"));

    let invalid_false = TempDir::new().unwrap();
    virtual_workspace(invalid_false.path(), "\"member\"", true);
    write_file(
        &invalid_false.path().join("member/Cargo.toml"),
        "[package]\nname = \"invalid-false\"\nversion.workspace = false\ndescription.workspace = true\nlicense.workspace = true\n",
    );
    let error = inventory_workspace(invalid_false.path()).unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("version.workspace = false"));
}

#[cfg(unix)]
#[test]
fn symlinked_member_cannot_escape_the_workspace() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let root = temp.path().join("root");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&root).unwrap();
    write_file(&outside.join("Cargo.toml"), &inherited_package("outside"));
    virtual_workspace(&root, "\"member\"", true);
    symlink(&outside, root.join("member")).unwrap();

    let error = inventory_workspace(&root).unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("escapes the workspace"));
}
