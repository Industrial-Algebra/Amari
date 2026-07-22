// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{collections::BTreeMap, fs, path::Path};

use amari_discovery::{
    inspect_cargo_project, inspect_npm_project, inspect_project, inspect_project_envelope,
    inspect_rust_sources, InspectionLimits,
};
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const CARGO_SECRET: &str = "CARGO-TOKEN-24B2-DO-NOT-LEAK";
const NPM_SECRET: &str = "NPM-TOKEN-24B2-DO-NOT-LEAK";
const SOURCE_SECRET: &str = "SOURCE-TOKEN-24B2-DO-NOT-LEAK";
const ENV_SECRET: &str = "ENV-TOKEN-24B2-DO-NOT-LEAK";

fn write(root: &TempDir, path: &str, bytes: &[u8]) {
    let path = root.path().join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn mixed_project() -> TempDir {
    let root = tempfile::tempdir().unwrap();
    write(
        &root,
        "Cargo.toml",
        format!(
            r#"[package]
name = "privacy-fixture"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = {{ git = "https://user:{CARGO_SECRET}@example.invalid/private.git" }}
"#
        )
        .as_bytes(),
    );
    write(
        &root,
        "Cargo.lock",
        format!(
            r#"version = 3

[[package]]
name = "privacy-fixture"
version = "0.1.0"

[[package]]
name = "amari-core"
version = "0.23.0"
source = "git+https://user:{CARGO_SECRET}@example.invalid/private.git"
checksum = "abc123"
"#
        )
        .as_bytes(),
    );
    write(
        &root,
        "package.json",
        format!(
            r#"{{"name":"privacy-fixture","version":"0.1.0","dependencies":{{"@justinelliottcobb/amari-wasm":"https://user:{NPM_SECRET}@example.invalid/pkg.tgz"}},"scripts":{{"secret":"{SOURCE_SECRET}"}}}}"#
        )
        .as_bytes(),
    );
    write(
        &root,
        "package-lock.json",
        format!(
            r#"{{"name":"privacy-fixture","version":"0.1.0","lockfileVersion":3,"packages":{{"node_modules/@justinelliottcobb/amari-wasm":{{"version":"{NPM_SECRET}"}}}}}}"#
        )
        .as_bytes(),
    );
    write(
        &root,
        "src/lib.rs",
        format!("use amari_core::Multivector;\npub const PRIVATE: &str = \"{SOURCE_SECRET}\";\n")
            .as_bytes(),
    );
    write(
        &root,
        "src/index.ts",
        format!(
            "import {{ GenericMultivector }} from '@justinelliottcobb/amari-wasm';\nconst private = '{SOURCE_SECRET}';\n"
        )
        .as_bytes(),
    );
    write(
        &root,
        ".env.production",
        format!("API_KEY={ENV_SECRET}\n").as_bytes(),
    );
    root
}

fn assert_no_secrets(value: &str, root: &Path) {
    for secret in [CARGO_SECRET, NPM_SECRET, SOURCE_SECRET, ENV_SECRET] {
        assert!(!value.contains(secret), "leaked secret marker {secret}");
    }
    assert!(
        !value.contains(root.to_str().unwrap()),
        "leaked absolute root"
    );
}

#[test]
fn composed_snapshot_and_errors_redact_secret_shaped_content() {
    let root = mixed_project();
    let envelope = inspect_project_envelope(root.path(), &InspectionLimits::default()).unwrap();
    let encoded = serde_json::to_string(&envelope).unwrap();
    assert_no_secrets(&encoded, root.path());
    assert!(encoded.contains("git dependency cannot be resolved offline"));
    assert!(encoded.contains("unsupported"));
}

#[test]
fn root_validation_errors_never_include_absolute_input_paths() {
    let root = mixed_project();
    let secret_named_file = root.path().join("ABSOLUTE-ROOT-SECRET-24B2");
    fs::write(&secret_named_file, b"not a directory").unwrap();
    let limits = InspectionLimits::default();

    let generic = inspect_project(&secret_named_file, &limits)
        .unwrap_err()
        .to_string();
    let cargo = inspect_cargo_project(&secret_named_file, &limits)
        .unwrap_err()
        .to_string();
    let cargo_context = inspect_cargo_project(root.path(), &limits).unwrap();
    let rust = inspect_rust_sources(&secret_named_file, &cargo_context, &limits)
        .unwrap_err()
        .to_string();
    let npm = inspect_npm_project(&secret_named_file, &limits)
        .unwrap_err()
        .to_string();

    for error in [generic, cargo, rust, npm] {
        assert!(!error.contains("ABSOLUTE-ROOT-SECRET-24B2"), "{error}");
        assert!(!error.contains(root.path().to_str().unwrap()), "{error}");
        assert!(error.contains("not a directory"), "{error}");
    }
}

#[cfg(unix)]
#[derive(Debug, Eq, PartialEq)]
struct FileIdentity {
    bytes: Vec<u8>,
    length: u64,
    mode: u32,
    modified: std::time::SystemTime,
}

#[cfg(unix)]
fn identities(root: &Path, paths: &[&str]) -> BTreeMap<String, FileIdentity> {
    paths
        .iter()
        .map(|relative| {
            let path = root.join(relative);
            let metadata = fs::metadata(&path).unwrap();
            (
                (*relative).to_owned(),
                FileIdentity {
                    bytes: fs::read(path).unwrap(),
                    length: metadata.len(),
                    mode: metadata.permissions().mode(),
                    modified: metadata.modified().unwrap(),
                },
            )
        })
        .collect()
}

#[cfg(unix)]
#[test]
fn composed_inspection_preserves_content_permissions_size_and_mtime() {
    let root = mixed_project();
    let paths = [
        "Cargo.toml",
        "Cargo.lock",
        "package.json",
        "package-lock.json",
        "src/lib.rs",
        "src/index.ts",
        ".env.production",
    ];
    fs::set_permissions(
        root.path().join("src/lib.rs"),
        fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    let before = identities(root.path(), &paths);

    inspect_project_envelope(root.path(), &InspectionLimits::default()).unwrap();

    let after = identities(root.path(), &paths);
    assert_eq!(after, before);
}

#[test]
fn source_locations_are_hash_only_and_secret_files_are_absent() {
    let root = mixed_project();
    let envelope = inspect_project_envelope(root.path(), &InspectionLimits::default()).unwrap();
    assert!(envelope
        .data
        .files
        .iter()
        .all(|source| source.content_hash.len() == 64));
    assert!(envelope
        .data
        .files
        .iter()
        .all(|source| !source.path.starts_with(".env")));
    let encoded = serde_json::to_string(&envelope.data.files).unwrap();
    assert!(!encoded.contains("PRIVATE"));
    assert!(!encoded.contains(SOURCE_SECRET));
}
