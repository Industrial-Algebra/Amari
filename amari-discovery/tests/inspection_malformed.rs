// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;

use amari_discovery::{
    inspect_cargo_project, inspect_npm_project, inspect_rust_sources, inspect_typescript_sources,
    CargoInspectionWarning, Catalog, DiscoveryError, InspectionLimits, NpmInspectionWarning,
    RustInspectionWarning, TypeScriptInspectionWarning,
};
use tempfile::TempDir;

fn write(root: &TempDir, path: &str, bytes: &[u8]) {
    let path = root.path().join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

#[test]
fn nested_workspace_roots_are_rejected_without_recursive_descent() {
    let root = tempfile::tempdir().unwrap();
    write(
        &root,
        "Cargo.toml",
        br#"[package]
name = "root"
version = "0.1.0"
edition = "2021"

[workspace]
members = ["member"]
"#,
    );
    write(
        &root,
        "member/Cargo.toml",
        br#"[package]
name = "nested-root"
version = "0.1.0"
edition = "2021"

[workspace]
members = ["child"]
"#,
    );
    write(
        &root,
        "member/child/Cargo.toml",
        br#"[package]
name = "must-not-be-inspected"
version = "0.1.0"
edition = "2021"
"#,
    );

    let inspection = inspect_cargo_project(root.path(), &InspectionLimits::default()).unwrap();
    assert!(inspection.workspace_members.is_empty());
    assert!(inspection.warnings.iter().any(|warning| matches!(
        warning,
        CargoInspectionWarning::NestedWorkspaceRoot { path }
            if path == "member/Cargo.toml"
    )));
    let encoded = serde_json::to_string(&inspection).unwrap();
    assert!(!encoded.contains("must-not-be-inspected"));
}

#[test]
fn normalized_duplicate_workspace_members_are_inspected_once() {
    let root = tempfile::tempdir().unwrap();
    write(
        &root,
        "Cargo.toml",
        br#"[package]
name = "root"
version = "0.1.0"
edition = "2021"

[workspace]
members = ["member", "member/"]
"#,
    );
    write(
        &root,
        "member/Cargo.toml",
        br#"[package]
name = "member"
version = "0.1.0"
edition = "2021"
"#,
    );

    let inspection = inspect_cargo_project(root.path(), &InspectionLimits::default()).unwrap();
    assert_eq!(inspection.workspace_members.len(), 1);
    assert_eq!(inspection.inspected_file_count, 2);
}

#[test]
fn mixed_encoding_inputs_produce_typed_sanitized_outcomes() {
    let root = tempfile::tempdir().unwrap();
    write(
        &root,
        "Cargo.toml",
        br#"[package]
name = "mixed"
version = "0.1.0"
edition = "2021"

[workspace]
members = ["bad-member"]
"#,
    );
    write(&root, "bad-member/Cargo.toml", b"[package]\nname='\xff'\n");
    write(&root, "src/lib.rs", b"pub fn valid() {}\n\xff");
    write(
        &root,
        "package.json",
        br#"{"name":"mixed","version":"0.1.0"}"#,
    );
    write(&root, "package-lock.json", b"{\xff}");
    write(&root, "src/index.ts", b"export const valid = 1;\n\xff");

    let limits = InspectionLimits::default();
    let cargo = inspect_cargo_project(root.path(), &limits).unwrap();
    assert!(cargo.warnings.iter().any(|warning| matches!(
        warning,
        CargoInspectionWarning::MalformedManifest { path, .. }
            if path == "bad-member/Cargo.toml"
    )));
    let rust = inspect_rust_sources(root.path(), &cargo, &limits).unwrap();
    assert!(rust.warnings.iter().any(|warning| matches!(
        warning,
        RustInspectionWarning::InvalidUtf8Source { path } if path == "src/lib.rs"
    )));
    let npm = inspect_npm_project(root.path(), &limits).unwrap();
    assert!(npm.warnings.iter().any(|warning| matches!(
        warning,
        NpmInspectionWarning::InvalidUtf8Lock { path, .. } if path == "package-lock.json"
    )));
    let typescript =
        inspect_typescript_sources(root.path(), &npm, &Catalog::embedded().unwrap(), &limits)
            .unwrap();
    assert!(typescript.warnings.iter().any(|warning| matches!(
        warning,
        TypeScriptInspectionWarning::InvalidUtf8Source { path, .. }
            if path == "src/index.ts"
    )));

    let encoded = serde_json::to_string(&(cargo, rust, npm, typescript)).unwrap();
    assert!(!encoded.contains('�'));
    assert!(!encoded.contains(root.path().to_str().unwrap()));
}

#[test]
fn malicious_root_manifests_return_sanitized_errors() {
    let cargo_root = tempfile::tempdir().unwrap();
    write(
        &cargo_root,
        "Cargo.toml",
        b"[package]\nname = \"SECRET-CARGO-SOURCE\n",
    );
    let cargo_error = inspect_cargo_project(cargo_root.path(), &InspectionLimits::default())
        .unwrap_err()
        .to_string();
    assert!(!cargo_error.contains("SECRET-CARGO-SOURCE"));
    assert!(!cargo_error.contains(cargo_root.path().to_str().unwrap()));

    let npm_root = tempfile::tempdir().unwrap();
    write(&npm_root, "package.json", br#"{"name":"SECRET-NPM-SOURCE""#);
    let npm_error = inspect_npm_project(npm_root.path(), &InspectionLimits::default())
        .unwrap_err()
        .to_string();
    assert!(!npm_error.contains("SECRET-NPM-SOURCE"));
    assert!(!npm_error.contains(npm_root.path().to_str().unwrap()));
}

#[test]
fn huge_source_token_streams_are_stopped_by_byte_guards_before_parsing() {
    let root = tempfile::tempdir().unwrap();
    write(
        &root,
        "Cargo.toml",
        br#"[package]
name = "bounded"
version = "0.1.0"
edition = "2021"
"#,
    );
    write(
        &root,
        "package.json",
        br#"{"name":"bounded","version":"0.1.0"}"#,
    );
    write(&root, "src/lib.rs", &vec![b';'; 4097]);
    write(&root, "src/index.ts", &vec![b';'; 4097]);
    let limits = InspectionLimits {
        max_per_file_bytes: 4096,
        ..InspectionLimits::default()
    };

    let cargo = inspect_cargo_project(root.path(), &limits).unwrap();
    let rust = inspect_rust_sources(root.path(), &cargo, &limits).unwrap();
    assert!(rust.warnings.iter().any(|warning| matches!(
        warning,
        RustInspectionWarning::OversizedFile { path, size, limit }
            if path == "src/lib.rs" && *size == 4097 && *limit == 4096
    )));
    assert!(rust.usages.is_empty());

    let npm = inspect_npm_project(root.path(), &limits).unwrap();
    let typescript =
        inspect_typescript_sources(root.path(), &npm, &Catalog::embedded().unwrap(), &limits)
            .unwrap();
    assert!(typescript.warnings.iter().any(|warning| matches!(
        warning,
        TypeScriptInspectionWarning::OversizedFile { path, limit, observed }
            if path == "src/index.ts" && *limit == 4096 && *observed == 4097
    )));
    assert!(typescript.imports.is_empty());
}

#[test]
fn huge_required_manifests_fail_with_typed_limits_not_parser_failures() {
    let root = tempfile::tempdir().unwrap();
    write(&root, "Cargo.toml", &vec![b'x'; 1025]);
    write(&root, "package.json", &vec![b'x'; 1025]);
    let limits = InspectionLimits {
        max_per_file_bytes: 1024,
        ..InspectionLimits::default()
    };

    assert!(matches!(
        inspect_cargo_project(root.path(), &limits).unwrap_err(),
        DiscoveryError::LimitExceeded(_)
    ));
    assert!(matches!(
        inspect_npm_project(root.path(), &limits).unwrap_err(),
        DiscoveryError::LimitExceeded(_)
    ));
}
