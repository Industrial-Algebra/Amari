// SPDX-License-Identifier: MIT OR Apache-2.0

//! Offline npm `package.json` / `package-lock.json` inspection (Task 9A).

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use amari_discovery::{
    inspect_npm_project, InspectionLimit, InspectionLimits, NpmDependencyKind,
    NpmInspectionWarning, SnapshotState,
};

const PACKAGE: &str = "@justinelliottcobb/amari-wasm";

fn fixture(name: &str) -> &'static Path {
    match name {
        "current" => Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/npm-project"
        )),
        "stale" => Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/npm-project-stale"
        )),
        _ => unreachable!(),
    }
}

fn materialize_current() -> TempDir {
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    materialize(fixture("current"), Some(&version))
}

fn materialize_stale() -> TempDir {
    materialize(fixture("stale"), None)
}

fn materialize(src: &Path, version: Option<&str>) -> TempDir {
    let temp = TempDir::new().unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        let content = fs::read_to_string(entry.path()).unwrap();
        if let Some(base) = name_text.strip_suffix(".in") {
            fs::write(
                temp.path().join(base),
                content.replace("__AMARI_VERSION__", version.unwrap()),
            )
            .unwrap();
        } else {
            fs::write(temp.path().join(name), content).unwrap();
        }
    }
    temp
}

fn write_minimal_package(root: &Path, requirement: &str) {
    fs::write(
        root.join("package.json"),
        format!(
            r#"{{
  "name": "minimal-npm-project",
  "version": "1.0.0",
  "dependencies": {{
    "{PACKAGE}": "{requirement}"
  }}
}}
"#
        ),
    )
    .unwrap();
}

fn write_lock(root: &Path, lockfile_version: u64, resolved_version: &str) {
    fs::write(
        root.join("package-lock.json"),
        format!(
            r#"{{
  "name": "minimal-npm-project",
  "version": "1.0.0",
  "lockfileVersion": {lockfile_version},
  "packages": {{
    "": {{
      "dependencies": {{ "{PACKAGE}": "{resolved_version}" }}
    }},
    "node_modules/{PACKAGE}": {{
      "version": "{resolved_version}"
    }}
  }},
  "dependencies": {{
    "{PACKAGE}": {{ "version": "{resolved_version}" }}
  }}
}}
"#
        ),
    )
    .unwrap();
}

#[test]
fn current_manifest_and_v3_lock_resolve_applicable_wasm_package() {
    let temp = materialize_current();
    let inspection = inspect_npm_project(temp.path(), &InspectionLimits::default()).unwrap();

    assert_eq!(
        inspection.package.name.as_deref(),
        Some("amari-npm-project")
    );
    assert_eq!(inspection.state, SnapshotState::Complete);
    assert_eq!(inspection.inspected_file_count, 2);
    assert!(inspection.total_bytes > 0);
    assert!(!inspection.input_hash.is_empty());

    let dependency = inspection
        .package
        .dependencies
        .iter()
        .find(|dependency| dependency.package_name == PACKAGE)
        .unwrap();
    assert_eq!(dependency.kind, NpmDependencyKind::Production);
    assert_eq!(
        dependency.resolved_version.as_deref(),
        Some(inspection.catalog_version.as_str())
    );
    assert_eq!(dependency.compatibility.status, "applicable");
    assert_eq!(dependency.manifest_source.path, "package.json");
    assert_eq!(
        dependency
            .lock_source
            .as_ref()
            .map(|source| source.path.as_str()),
        Some("package-lock.json")
    );

    let lock = inspection.lock.as_ref().unwrap();
    assert_eq!(lock.lockfile_version, 3);
    assert!(lock
        .packages
        .iter()
        .any(|package| package.package_name == PACKAGE));
}

#[test]
fn stale_lock_version_reports_unknown_version() {
    let temp = materialize_stale();
    let inspection = inspect_npm_project(temp.path(), &InspectionLimits::default()).unwrap();
    let dependency = &inspection.package.dependencies[0];

    assert_eq!(dependency.declared_version, "0.19.0");
    assert_eq!(dependency.resolved_version.as_deref(), Some("0.19.0"));
    assert_eq!(dependency.compatibility.status, "unknown_version");
}

#[test]
fn lockfile_versions_two_and_three_are_supported() {
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    for schema in [2, 3] {
        let temp = TempDir::new().unwrap();
        write_minimal_package(temp.path(), &version);
        write_lock(temp.path(), schema, &version);
        let inspection = inspect_npm_project(temp.path(), &InspectionLimits::default()).unwrap();

        assert_eq!(inspection.lock.as_ref().unwrap().lockfile_version, schema);
        assert_eq!(
            inspection.package.dependencies[0]
                .resolved_version
                .as_deref(),
            Some(version.as_str())
        );
        assert!(!inspection.warnings.iter().any(|warning| matches!(
            warning,
            NpmInspectionWarning::UnsupportedLockfileVersion { .. }
        )));
    }
}

#[test]
fn unsupported_v1_lock_is_typed_and_not_used_for_resolution() {
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    let temp = TempDir::new().unwrap();
    write_minimal_package(temp.path(), &version);
    write_lock(temp.path(), 1, &version);

    let inspection = inspect_npm_project(temp.path(), &InspectionLimits::default()).unwrap();
    assert!(inspection.lock.is_none());
    assert!(inspection.warnings.iter().any(|warning| matches!(
        warning,
        NpmInspectionWarning::UnsupportedLockfileVersion { version: 1 }
    )));
    assert!(inspection.package.dependencies[0]
        .resolved_version
        .is_none());
    assert_eq!(
        inspection.package.dependencies[0].compatibility.status,
        "unknown_version"
    );
}

#[test]
fn missing_lock_is_nonfatal_and_typed() {
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    let temp = TempDir::new().unwrap();
    write_minimal_package(temp.path(), &version);

    let inspection = inspect_npm_project(temp.path(), &InspectionLimits::default()).unwrap();
    assert_eq!(inspection.state, SnapshotState::Complete);
    assert!(inspection.lock.is_none());
    assert!(inspection.warnings.iter().any(|warning| matches!(
        warning,
        NpmInspectionWarning::MissingLock { path } if path == "package-lock.json"
    )));
    assert_eq!(inspection.inspected_file_count, 1);
}

#[test]
fn malformed_lock_is_sanitized_and_retained_in_provenance() {
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    let temp = TempDir::new().unwrap();
    write_minimal_package(temp.path(), &version);
    fs::write(
        temp.path().join("package-lock.json"),
        b"{ \"SECRET-LOCK-SOURCE\": [ }",
    )
    .unwrap();

    let inspection = inspect_npm_project(temp.path(), &InspectionLimits::default()).unwrap();
    assert!(inspection.lock.is_none());
    assert_eq!(inspection.inspected_file_count, 2);
    assert!(inspection.warnings.iter().any(|warning| matches!(
        warning,
        NpmInspectionWarning::MalformedLock { path, .. } if path == "package-lock.json"
    )));
    let json = serde_json::to_string(&inspection).unwrap();
    assert!(!json.contains("SECRET-LOCK-SOURCE"));
    assert!(!json.contains(temp.path().to_str().unwrap()));
}

#[test]
fn invalid_utf8_lock_is_typed_and_content_addressed() {
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    let temp = TempDir::new().unwrap();
    write_minimal_package(temp.path(), &version);
    fs::write(temp.path().join("package-lock.json"), [0xff, 0xfe, 0xfd]).unwrap();

    let inspection = inspect_npm_project(temp.path(), &InspectionLimits::default()).unwrap();
    assert_eq!(inspection.inspected_file_count, 2);
    assert!(inspection.warnings.iter().any(|warning| matches!(
        warning,
        NpmInspectionWarning::InvalidUtf8Lock { path, content_hash }
            if path == "package-lock.json" && !content_hash.is_empty()
    )));
}

#[test]
fn missing_or_malformed_package_manifest_is_a_sanitized_error() {
    let missing = TempDir::new().unwrap();
    let missing_error = inspect_npm_project(missing.path(), &InspectionLimits::default())
        .unwrap_err()
        .to_string();
    assert!(missing_error.contains("package.json"));
    assert!(!missing_error.contains(missing.path().to_str().unwrap()));

    let malformed = TempDir::new().unwrap();
    fs::write(
        malformed.path().join("package.json"),
        b"{ \"SECRET-PACKAGE-SOURCE\": [ }",
    )
    .unwrap();
    let malformed_error = inspect_npm_project(malformed.path(), &InspectionLimits::default())
        .unwrap_err()
        .to_string();
    assert!(!malformed_error.contains("SECRET-PACKAGE-SOURCE"));
    assert!(!malformed_error.contains(malformed.path().to_str().unwrap()));
}

#[test]
fn lock_file_count_limit_returns_consistent_partial_evidence() {
    let temp = materialize_current();
    let limits = InspectionLimits {
        max_inspection_files: 1,
        ..InspectionLimits::default()
    };
    let inspection = inspect_npm_project(temp.path(), &limits).unwrap();

    assert!(matches!(
        inspection.state,
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::FileCount {
                max: 1,
                observed: 2
            }
        }
    ));
    assert_eq!(inspection.inspected_file_count, 1);
    assert!(inspection.lock.is_none());
    assert!(inspection.package.dependencies[0]
        .resolved_version
        .is_none());
}

#[test]
fn oversized_lock_returns_typed_partial_without_persisting_content() {
    let temp = materialize_current();
    let limits = InspectionLimits {
        max_per_file_bytes: 300,
        ..InspectionLimits::default()
    };
    let inspection = inspect_npm_project(temp.path(), &limits).unwrap();

    assert!(matches!(
        inspection.state,
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::PerFileBytes { max: 300, .. }
        }
    ));
    assert_eq!(inspection.inspected_file_count, 1);
    assert!(inspection.lock.is_none());
}

#[test]
fn input_hash_is_deterministic_root_independent_and_ignores_other_lock_managers() {
    let first = materialize_current();
    let second = materialize_current();
    fs::write(first.path().join("yarn.lock"), "SECRET-YARN-CONTENT").unwrap();
    fs::write(first.path().join("pnpm-lock.yaml"), "SECRET-PNPM-CONTENT").unwrap();

    let a = inspect_npm_project(first.path(), &InspectionLimits::default()).unwrap();
    let b = inspect_npm_project(second.path(), &InspectionLimits::default()).unwrap();
    assert_eq!(a, b);
    let json = serde_json::to_string(&a).unwrap();
    assert!(!json.contains("SECRET-YARN-CONTENT"));
    assert!(!json.contains("SECRET-PNPM-CONTENT"));
}

#[test]
fn package_scripts_are_never_executed_or_persisted() {
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    let temp = TempDir::new().unwrap();
    let marker = temp.path().join("npm-script-executed");
    fs::write(
        temp.path().join("package.json"),
        format!(
            r#"{{
  "name": "poison-npm-project",
  "version": "1.0.0",
  "scripts": {{ "preinstall": "touch npm-script-executed SECRET-NPM-SCRIPT" }},
  "dependencies": {{ "{PACKAGE}": "{version}" }}
}}
"#
        ),
    )
    .unwrap();

    let inspection = inspect_npm_project(temp.path(), &InspectionLimits::default()).unwrap();
    assert!(!marker.exists());
    assert!(!serde_json::to_string(&inspection)
        .unwrap()
        .contains("SECRET-NPM-SCRIPT"));
}

#[cfg(unix)]
#[test]
fn symlinked_package_manifest_is_never_followed() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    fs::write(
        outside.path().join("outside.json"),
        r#"{ "name": "SECRET-EXTERNAL-PACKAGE" }"#,
    )
    .unwrap();
    symlink(
        outside.path().join("outside.json"),
        temp.path().join("package.json"),
    )
    .unwrap();

    let error = inspect_npm_project(temp.path(), &InspectionLimits::default())
        .unwrap_err()
        .to_string();
    assert!(!error.contains("SECRET-EXTERNAL-PACKAGE"));
    assert!(!error.contains(outside.path().to_str().unwrap()));
}

#[test]
fn capabilities_scope_names_only_the_npm_typescript_inspector() {
    let capabilities = amari_discovery::Capabilities::current().unwrap();
    let ids: Vec<&str> = capabilities
        .project_inspectors
        .iter()
        .map(|inspector| inspector.id.as_str())
        .collect();
    assert!(ids.contains(&"npm-typescript"));
    assert!(!ids
        .iter()
        .any(|id| id.contains("yarn") || id.contains("pnpm")));
}
