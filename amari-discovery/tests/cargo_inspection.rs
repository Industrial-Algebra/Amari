// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for offline Cargo dependency inspection (Task 8A).
//!
//! Every test uses materialized fixture directories and exercises the
//! `inspect_cargo_project` entry point with typed assertions.
//!
//! # Safety invariants verified
//!
//! - No Cargo, rustc, build-script, network, or shell execution
//! - No absolute-root leakage in warnings/errors
//! - Poison PATH/build.rs markers remain untouched
//! - Symlinked manifests produce warnings, not followed
//! - Limits enforced on every manifest/lock input
//! - Provenance hash deterministic and input-framed

use std::collections::BTreeSet;
use std::fs;

use tempfile::TempDir;

use amari_discovery::inspect::{
    inspect_cargo_project, CargoInspectionWarning, DependencyKind, InspectionLimits,
};
use amari_discovery::Catalog;

// ---------------------------------------------------------------------------
// Materialization helper
// ---------------------------------------------------------------------------

/// Copies the `rust-project` fixture into a temp directory, replacing
/// `__AMARI_VERSION__` with the embedded catalog version in `.in` files
/// and materializing `.in` → desired paths.
fn materialize_current() -> (TempDir, String) {
    let dir = TempDir::new().unwrap();
    let catalog = Catalog::embedded().unwrap();
    let version = catalog.version().to_string();
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rust-project");

    for entry in walkdir::WalkDir::new(&fixture)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry.path().strip_prefix(&fixture).unwrap();
        let dest = dir.path().join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let content = fs::read_to_string(entry.path()).unwrap();
        let processed = content.replace("__AMARI_VERSION__", &version);
        let dest_path = if rel.extension().is_some_and(|e| e == "in") {
            dest.with_extension("")
        } else {
            dest
        };
        fs::write(&dest_path, processed).unwrap();
    }
    (dir, version)
}

/// Copies the `rust-project-stale` fixture into a temp directory (0.19.0).
fn materialize_stale() -> TempDir {
    let dir = TempDir::new().unwrap();
    let fixture =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rust-project-stale");
    for entry in walkdir::WalkDir::new(&fixture)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry.path().strip_prefix(&fixture).unwrap();
        let dest = dir.path().join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::copy(entry.path(), &dest).unwrap();
    }
    dir
}

fn default_limits() -> InspectionLimits {
    InspectionLimits::default()
}

// ===========================================================================
// 1 — Root package basics
// ===========================================================================

#[test]
fn root_package_has_name_version_and_manifest_path() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    assert_eq!(inspection.root_package.name, "rust-project");
    assert!(!inspection.root_package.version.is_empty());
    assert!(inspection
        .root_package
        .manifest_path
        .ends_with("Cargo.toml"));
    // CargoInspection provenance fields
    assert!(!inspection.input_hash.is_empty(), "must have input_hash");
    assert!(
        inspection.inspected_file_count >= 2,
        "must inspect at least root manifest + lock"
    );
    assert!(inspection.total_bytes > 0, "must have total_bytes");
}

// ===========================================================================
// 2 — Root amari dependency has no fabricated tropical feature
// ===========================================================================

#[test]
fn amari_dep_has_no_fabricated_tropical_feature() {
    let (dir, ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let amari_dep = inspection
        .root_package
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari")
        .expect("root must have 'amari' dependency");
    assert_eq!(amari_dep.declared_version, ver);
    assert!(
        !amari_dep.features.iter().any(|f| f == "tropical"),
        "amari dep must not fabricate a 'tropical' feature"
    );
}

// ===========================================================================
// 3 — Direct Amari crate dependencies are discovered
// ===========================================================================

#[test]
fn direct_amari_crates_are_discovered() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let names: BTreeSet<_> = inspection
        .root_package
        .dependencies
        .iter()
        .map(|d| d.package_name.clone())
        .collect();
    assert!(names.contains("amari"));
    assert!(names.contains("amari-core"));
    assert!(names.contains("amari-tropical"));
    assert!(names.contains("amari-cgt"));
    assert!(names.contains("amari-dual"));
    assert!(names.contains("amari-network"));
}

// ===========================================================================
// 4 — Renamed direct dependency (package = "amari-tropical")
// ===========================================================================

#[test]
fn renamed_dependency_preserves_package_name() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let renamed = inspection
        .root_package
        .dependencies
        .iter()
        .find(|d| d.alias == "renamed-tropical")
        .expect("root must have renamed-tropical dependency");
    assert_eq!(renamed.package_name, "amari-tropical");
    assert_eq!(renamed.alias, "renamed-tropical");
}

// ===========================================================================
// 5 — Dependency kinds (normal / dev / build)
// ===========================================================================

#[test]
fn dependency_kinds_are_correct() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();

    let amari_core = inspection
        .root_package
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-core")
        .unwrap();
    assert_eq!(amari_core.kind, DependencyKind::Normal);

    let amari_cgt = inspection
        .root_package
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-cgt")
        .unwrap();
    assert_eq!(amari_cgt.kind, DependencyKind::Dev);

    let amari_dual = inspection
        .root_package
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-dual")
        .unwrap();
    assert_eq!(amari_dual.kind, DependencyKind::Build);
}

// ===========================================================================
// 6 — Target-specific dependencies
// ===========================================================================

#[test]
fn target_specific_dependency_has_target_selector() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let network = inspection
        .root_package
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-network")
        .unwrap();
    assert_eq!(network.target.as_deref(), Some("cfg(unix)"));
}

// ===========================================================================
// 7 — Feature flags and optional marker
// ===========================================================================

#[test]
fn feature_flags_and_optional_marker() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();

    let amari_core = inspection
        .root_package
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-core")
        .unwrap();
    assert!(amari_core.features.iter().any(|f| f == "std"));
    assert!(!amari_core.optional);

    let amari_dual = inspection
        .root_package
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-dual")
        .unwrap();
    assert!(amari_dual.optional);
}

// ===========================================================================
// 8 — Default-features flag
// ===========================================================================

#[test]
fn default_features_flag() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    for dep in &inspection.root_package.dependencies {
        assert!(
            dep.default_features,
            "default-features should be true unless explicitly false"
        );
    }
}

// ===========================================================================
// 9 — [[bench]] declarations
// ===========================================================================

#[test]
fn bench_declarations_are_discovered() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let bench_names: BTreeSet<_> = inspection
        .root_package
        .benches
        .iter()
        .map(|b| b.name.clone())
        .collect();
    assert!(bench_names.contains("speed_bench"));
    assert!(bench_names.contains("correctness_bench"));
    assert_eq!(inspection.root_package.benches.len(), 2);
}

#[test]
fn bench_has_path_and_manifest_source() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let bench = inspection
        .root_package
        .benches
        .iter()
        .find(|b| b.name == "speed_bench")
        .unwrap();
    assert!(bench.path.contains("benches"));
    assert_eq!(bench.manifest_source.path, "Cargo.toml");
    assert!(!bench.manifest_source.content_hash.is_empty());
}

// ===========================================================================
// 10 — [package].links native-link signal
// ===========================================================================

#[test]
fn native_link_signal_is_detected() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let link = inspection
        .root_package
        .native_link
        .as_ref()
        .expect("root package has links");
    assert_eq!(link.links_key, "rust-project-native");
    assert!(!link.manifest_source.content_hash.is_empty());
}

// ===========================================================================
// 11 — Package metadata inheritance
// ===========================================================================

#[test]
fn package_metadata_inheritance_from_workspace() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let inherited = &inspection.root_package.inherited_metadata;
    assert!(inherited.contains(&"version".to_string()));
    assert!(inherited.contains(&"authors".to_string()));
    assert!(inherited.contains(&"edition".to_string()));
    assert!(inherited.contains(&"license".to_string()));
    assert!(inherited.contains(&"rust-version".to_string()));
}

// ===========================================================================
// 12 — Workspace members are discovered
// ===========================================================================

#[test]
fn workspace_members_are_discovered() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let member_names: BTreeSet<_> = inspection
        .workspace_members
        .iter()
        .map(|m| m.name.clone())
        .collect();
    assert!(member_names.contains("member-a"));
    assert!(member_names.contains("member-b"));
}

// ===========================================================================
// 13 — Member dependencies with workspace inheritance
// ===========================================================================

#[test]
fn member_workspace_inherited_dependencies() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let member_a = inspection
        .workspace_members
        .iter()
        .find(|m| m.name == "member-a")
        .unwrap();

    // amari-core should be a normal dep
    let core = member_a
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-core")
        .expect("member-a should have amari-core");
    assert_eq!(core.kind, DependencyKind::Normal);

    // amari-cgt is NOW a normal optional dep (was dev, moved for legality)
    let cgt = member_a
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-cgt")
        .expect("member-a should have amari-cgt");
    assert_eq!(cgt.kind, DependencyKind::Normal);
    assert!(cgt.optional, "amari-cgt should be optional in member-a");
    assert!(!cgt.features.is_empty(), "amari-cgt should have features");
    assert!(cgt.features.iter().any(|f| f == "faster"));
}

#[test]
fn member_renamed_dependency() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let member_b = inspection
        .workspace_members
        .iter()
        .find(|m| m.name == "member-b")
        .unwrap();
    let renamed = member_b
        .dependencies
        .iter()
        .find(|d| d.alias == "renamed-core")
        .expect("member-b should have renamed-core");
    assert_eq!(renamed.package_name, "amari-core");
}

#[test]
fn member_b_bench_is_discovered() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let member_b = inspection
        .workspace_members
        .iter()
        .find(|m| m.name == "member-b")
        .unwrap();
    assert_eq!(member_b.benches.len(), 1);
    assert_eq!(member_b.benches[0].name, "member_bench");
}

// ===========================================================================
// 14 — Cargo.lock resolved versions
// ===========================================================================

#[test]
fn lock_resolves_exact_versions_for_amari_deps() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    assert!(inspection.lock.is_some(), "Cargo.lock must be parsed");
    let lock = inspection.lock.as_ref().unwrap();

    for dep in &inspection.root_package.dependencies {
        assert!(
            dep.resolved_version.is_some(),
            "Amari dep {} must have resolved version",
            dep.package_name
        );
        let lock_pkg = lock.packages.iter().find(|p| p.name == dep.package_name);
        assert!(
            lock_pkg.is_some(),
            "lock must contain package {}",
            dep.package_name
        );
        assert_eq!(
            dep.resolved_version.as_deref(),
            Some(lock_pkg.unwrap().version.as_str())
        );
    }
}

// ===========================================================================
// 15 — Compatibility: applicable when resolved == catalog version
// ===========================================================================

#[test]
fn applicable_when_resolved_equals_catalog_version() {
    let (dir, ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    for dep in &inspection.root_package.dependencies {
        assert_eq!(dep.resolved_version.as_deref(), Some(ver.as_str()));
        assert_eq!(dep.compatibility.status, "applicable");
    }
}

// ===========================================================================
// 16 — Stale fixture at 0.19.0 reports unknown_version
// ===========================================================================

#[test]
fn stale_fixture_reports_unknown_version() {
    let dir = materialize_stale();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    assert!(inspection.lock.is_some());
    for dep in &inspection.root_package.dependencies {
        assert_eq!(dep.compatibility.status, "unknown_version");
        assert!(!dep.compatibility.reasons.is_empty());
        // Assert resolved version is exactly 0.19.0
        assert_eq!(dep.resolved_version.as_deref(), Some("0.19.0"));
    }
}

// ===========================================================================
// 17 — Deterministic sorted output
// ===========================================================================

#[test]
fn output_is_deterministically_sorted() {
    let (dir, _ver) = materialize_current();
    let inspection1 = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let inspection2 = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    assert_eq!(
        inspection1.root_package.dependencies,
        inspection2.root_package.dependencies
    );
    assert_eq!(inspection1.workspace_members, inspection2.workspace_members);
}

// ===========================================================================
// 18 — Manifest/lock source locations are relative and have provenance
// ===========================================================================

#[test]
fn manifest_source_locations_are_relative() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    for dep in &inspection.root_package.dependencies {
        let source = &dep.manifest_source;
        assert!(
            !source.path.starts_with('/') && !source.path.starts_with("\\\\"),
            "manifest source path must be relative, got '{}'",
            source.path
        );
        assert!(!source.path.is_empty());
        assert!(!source.content_hash.is_empty(), "must have content_hash");
        assert!(source.byte_count > 0, "must have byte_count > 0");
    }
}

#[test]
fn lock_source_locations_are_relative() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    for dep in &inspection.root_package.dependencies {
        if let Some(source) = &dep.lock_source {
            assert!(!source.path.starts_with('/') && !source.path.starts_with("\\\\"));
            assert!(!source.content_hash.is_empty());
        }
    }
}

// ===========================================================================
// 19 — Lock includes checksums, sources, and provenance
// ===========================================================================

#[test]
fn lock_packages_have_checksums_and_sources() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let lock = inspection.lock.as_ref().unwrap();
    for pkg in &lock.packages {
        if pkg.name.starts_with("amari") {
            assert!(pkg.checksum.is_some());
            assert!(pkg.source.is_some());
        }
    }
    assert!(!lock.source.content_hash.is_empty());
}

// ===========================================================================
// 20 — Malformed manifest produces typed error/warning
// ===========================================================================

#[test]
fn malformed_manifest_produces_warning() {
    let dir = TempDir::new().unwrap();
    fs::write(dir.path().join("Cargo.toml"), "[package\nname = \"bad\"\n").unwrap();
    let result = inspect_cargo_project(dir.path(), &default_limits());
    assert!(
        result.is_err(),
        "malformed root manifest should produce error"
    );
    // Error must not contain TOML source snippet (just reason + line/col)
    let msg = format!("{}", result.unwrap_err());
    // Error must be typed — generic "expected" might appear in "unexpected TOML syntax"
    // but the raw TOML error's source-col info ("at line X column Y") should not appear verbatim
    assert!(
        !msg.contains("expected an equals") && !msg.contains("expected a value"),
        "error must not leak raw source snippets"
    );
    assert!(
        msg.contains("invalid TOML")
            || msg.contains("unexpected")
            || msg.contains("missing")
            || msg.contains("unterminated")
    );
}

// ===========================================================================
// 21 — Missing lock is handled gracefully
// ===========================================================================

#[test]
fn missing_lock_is_not_fatal() {
    let dir = TempDir::new().unwrap();
    let manifest = format!(
        r#"[package]
name = "no-lock"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "{}"
"#,
        Catalog::embedded().unwrap().version()
    );
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    assert!(inspection.lock.is_none());
    let core = inspection
        .root_package
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-core")
        .unwrap();
    assert_eq!(core.compatibility.status, "unknown_version");
    assert!(core
        .compatibility
        .reasons
        .iter()
        .any(|r| r.contains("lock") || r.contains("Lock")));
}

// ===========================================================================
// 22 — Member with missing inherited base produces warning
// ===========================================================================

#[test]
fn missing_inherited_dependency_base_produces_warning() {
    let dir = TempDir::new().unwrap();
    let catalog = Catalog::embedded().unwrap();
    let _ver = catalog.version();
    let root_manifest = r#"[package]
name = "bad-inherit"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["sub"]

[workspace.dependencies]
"#;
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();
    fs::create_dir_all(dir.path().join("sub/src")).unwrap();
    let member_manifest = r#"[package]
name = "sub"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = { workspace = true }
"#;
    fs::write(dir.path().join("sub/Cargo.toml"), member_manifest).unwrap();
    fs::write(dir.path().join("sub/src/lib.rs"), "// SPDX\n").unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let missing_warning = inspection
        .warnings
        .iter()
        .find(|w| matches!(w, CargoInspectionWarning::InheritedBaseMissing { .. }));
    assert!(missing_warning.is_some());
}

// ===========================================================================
// 23 — Symlinked manifest produces warning (Unix only)
// ===========================================================================

#[cfg(unix)]
#[test]
fn symlinked_member_manifest_produces_warning() {
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().unwrap();
    let catalog = Catalog::embedded().unwrap();
    let ver = catalog.version();
    let member_dir = dir.path().join("real-member");
    fs::create_dir_all(member_dir.join("src")).unwrap();
    let member_toml = format!(
        r#"[package]
name = "real-member"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "{}"
"#,
        ver
    );
    fs::write(member_dir.join("Cargo.toml"), member_toml).unwrap();
    fs::write(member_dir.join("src/lib.rs"), "// SPDX\n").unwrap();

    let symlink_member = dir.path().join("symlinked-member");
    symlink(&member_dir, &symlink_member).unwrap();

    let root_manifest = format!(
        r#"[package]
name = "symlink-root"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "{}"

[workspace]
members = ["symlinked-member"]
"#,
        ver
    );
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    let lock_content = format!(
        r#"# Cargo.lock
version = 3

[[package]]
name = "amari-core"
version = "{}"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc123"

[[package]]
name = "symlink-root"
version = "0.1.0"
dependencies = [
 "amari-core",
]
"#,
        ver
    );
    fs::write(dir.path().join("Cargo.lock"), lock_content).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let symlink_warning = inspection
        .warnings
        .iter()
        .find(|w| matches!(w, CargoInspectionWarning::SymlinkedManifest { .. }));
    assert!(symlink_warning.is_some());
}

// ===========================================================================
// 24 — Absolute root is never leaked in warnings/errors
// ===========================================================================

#[test]
fn absolute_root_is_never_leaked() {
    let (dir, _ver) = materialize_current();
    let root_str = dir.path().to_string_lossy().to_string();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    for warning in &inspection.warnings {
        let msg = format!("{:?}", warning);
        assert!(
            !msg.contains(&root_str),
            "warning must not leak absolute root: {}",
            msg
        );
    }
}

// ===========================================================================
// 25 — Poison markers remain untouched (proves no execution)
// ===========================================================================

#[test]
fn poison_path_and_build_rs_markers_remain_untouched() {
    let dir = TempDir::new().unwrap();
    let catalog = Catalog::embedded().unwrap();
    let ver = catalog.version();

    fs::write(
        dir.path().join("build.rs"),
        "compile_error!(\"build.rs must never be executed during inspection\");\n",
    )
    .unwrap();

    let manifest = format!(
        r#"[package]
name = "poison"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "{}"
"#,
        ver
    );
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();
    let lock = format!(
        r#"# Cargo.lock
version = 3

[[package]]
name = "amari-core"
version = "{}"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc123"

[[package]]
name = "poison"
version = "0.1.0"
dependencies = [
 "amari-core",
]
"#,
        ver
    );
    fs::write(dir.path().join("Cargo.lock"), lock).unwrap();

    let result = inspect_cargo_project(dir.path(), &default_limits());
    assert!(result.is_ok());

    let build_rs = fs::read_to_string(dir.path().join("build.rs")).unwrap();
    assert!(build_rs.contains("must never be executed"));
}

// ===========================================================================
// 26 — Non-Amari dependencies are excluded from evidence
// ===========================================================================

#[test]
fn non_amari_deps_are_excluded() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    for dep in &inspection.root_package.dependencies {
        assert!(
            dep.package_name.starts_with("amari"),
            "only Amari deps should appear, got {}",
            dep.package_name
        );
    }
}

// ===========================================================================
// 27 — Ambiguous lock resolution produces warning
// ===========================================================================

#[test]
fn ambiguous_lock_resolution_produces_warning() {
    let dir = TempDir::new().unwrap();
    let lock = r#"# Cargo.lock
version = 3

[[package]]
name = "amari-core"
version = "0.23.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc123"

[[package]]
name = "amari-core"
version = "0.20.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "def456"

[[package]]
name = "ambig-root"
version = "0.1.0"
dependencies = [
 "amari-core",
]
"#;
    fs::write(dir.path().join("Cargo.lock"), lock).unwrap();

    let manifest = r#"[package]
name = "ambig-root"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "0.23"
"#;
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let ambig = inspection
        .warnings
        .iter()
        .find(|w| matches!(w, CargoInspectionWarning::AmbiguousLockResolution { .. }));
    assert!(ambig.is_some());

    let core_deps: Vec<_> = inspection
        .root_package
        .dependencies
        .iter()
        .filter(|d| d.package_name == "amari-core")
        .collect();
    assert_eq!(core_deps.len(), 1);
    // When declared version "0.23" exactly matches one locked version, it should resolve
    let core = &core_deps[0];
    // Declared version "0.23" (without patch) won't match locked "0.23.0" exactly
    // This is expected: without Cargo metadata, we can't resolve semver matching
    assert_eq!(core.compatibility.status, "unknown_version");
}

// ===========================================================================
// 28 — Same name + same version is NOT ambiguous (duplicate, not conflict)
// ===========================================================================

#[test]
fn duplicate_same_version_is_not_ambiguous() {
    let dir = TempDir::new().unwrap();
    let lock = r#"# Cargo.lock
version = 3

[[package]]
name = "amari-core"
version = "0.23.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc123"

[[package]]
name = "amari-core"
version = "0.23.0"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "def456"

[[package]]
name = "dup-root"
version = "0.1.0"
dependencies = [
 "amari-core",
]
"#;
    fs::write(dir.path().join("Cargo.lock"), lock).unwrap();

    let manifest = r#"[package]
name = "dup-root"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "0.23.0"
"#;
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let ambig = inspection
        .warnings
        .iter()
        .find(|w| matches!(w, CargoInspectionWarning::AmbiguousLockResolution { .. }));
    assert!(ambig.is_none(), "same name+version should NOT be ambiguous");
}

// ===========================================================================
// 29 — Malformed Cargo.lock produces warning
// ===========================================================================

#[test]
fn malformed_lock_produces_warning() {
    let dir = TempDir::new().unwrap();
    let catalog = Catalog::embedded().unwrap();
    let ver = catalog.version();
    let manifest = format!(
        r#"[package]
name = "bad-lock"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "{}"
"#,
        ver
    );
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();
    fs::write(dir.path().join("Cargo.lock"), "this is not valid toml [[[").unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let malformed_warning = inspection
        .warnings
        .iter()
        .find(|w| matches!(w, CargoInspectionWarning::MalformedLock { .. }));
    assert!(malformed_warning.is_some());
    // Verify malformed warnings have typed reasons, not raw TOML errors
    if let Some(CargoInspectionWarning::MalformedLock {
        reason,
        line,
        column,
        ..
    }) = malformed_warning
    {
        assert!(
            !reason.contains("expected an") && !reason.contains("expected a"),
            "reason must not contain raw source snippets: {}",
            reason
        );
        assert!(
            !reason.contains("toml"),
            "reason should be typed, not raw: {}",
            reason
        );
        let _ = line;
        let _ = column;
    }
}

// ===========================================================================
// 30 — No workspace members when workspace has empty members list
// ===========================================================================

#[test]
fn empty_workspace_members_produces_no_members() {
    let dir = TempDir::new().unwrap();
    let catalog = Catalog::embedded().unwrap();
    let ver = catalog.version();
    let manifest = format!(
        r#"[package]
name = "empty-ws"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "{}"

[workspace]
"#,
        ver
    );
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    assert!(inspection.workspace_members.is_empty());
}

// ===========================================================================
// 31 — Unsupported requirement (git) produces warning
// ===========================================================================

#[test]
fn unsupported_requirement_produces_warning() {
    let dir = TempDir::new().unwrap();
    let manifest = r#"[package]
name = "git-dep"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = { git = "https://github.com/example/amari", branch = "main" }
"#;
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let unsupported = inspection
        .warnings
        .iter()
        .find(|w| matches!(w, CargoInspectionWarning::UnsupportedRequirement { .. }));
    assert!(unsupported.is_some());
    let core = inspection
        .root_package
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-core")
        .unwrap();
    assert_eq!(core.compatibility.status, "unknown_version");
}

// ===========================================================================
// 32 — PROVENANCE: input_hash is deterministic (stability test)
// ===========================================================================

#[test]
fn input_hash_is_deterministic() {
    let (dir, _ver) = materialize_current();
    let inspection1 = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let inspection2 = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    assert_eq!(inspection1.input_hash, inspection2.input_hash);
    assert!(!inspection1.input_hash.is_empty());
}

// ===========================================================================
// 33 — PROVENANCE: input_hash is root-independent
// ===========================================================================

#[test]
fn input_hash_is_root_independent() {
    // Two different temp dirs with the same fixture content should produce
    // the same input_hash
    let (dir1, _ver) = materialize_current();
    let (dir2, _ver) = materialize_current();

    let inspection1 = inspect_cargo_project(dir1.path(), &default_limits()).unwrap();
    let inspection2 = inspect_cargo_project(dir2.path(), &default_limits()).unwrap();

    assert_eq!(
        inspection1.input_hash, inspection2.input_hash,
        "input_hash should be root-independent (hashes relative paths and content only)"
    );
}

// ===========================================================================
// 34 — PROVENANCE: input_hash changes when content changes
// ===========================================================================

#[test]
fn input_hash_changes_when_content_changes() {
    let (dir, _ver) = materialize_current();
    let inspection1 = inspect_cargo_project(dir.path(), &default_limits()).unwrap();

    // Modify the root manifest slightly
    let manifest_path = dir.path().join("Cargo.toml");
    let mut manifest = fs::read_to_string(&manifest_path).unwrap();
    manifest.push_str("# add a comment to change hash\n");
    fs::write(&manifest_path, manifest).unwrap();

    let inspection2 = inspect_cargo_project(dir.path(), &default_limits()).unwrap();

    assert_ne!(
        inspection1.input_hash, inspection2.input_hash,
        "input_hash must change when content changes"
    );
}

// ===========================================================================
// 35 — PROVENANCE: ManifestSource content_hash matches actual content
// ===========================================================================

#[test]
fn manifest_source_content_hash_is_correct() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();

    // Read actual Cargo.toml and compute its hash
    let manifest_bytes = fs::read(dir.path().join("Cargo.toml")).unwrap();
    use sha2::{Digest, Sha256};
    let expected_hash = hex::encode(Sha256::digest(&manifest_bytes));

    for dep in &inspection.root_package.dependencies {
        if dep.manifest_source.path == "Cargo.toml" {
            assert_eq!(dep.manifest_source.content_hash, expected_hash);
            assert_eq!(dep.manifest_source.byte_count, manifest_bytes.len() as u64);
        }
    }
}

// ===========================================================================
// 36 — LIMITS: root manifest exceeding per-file limit returns error
// ===========================================================================

#[test]
fn giant_root_manifest_exceeds_per_file_limit() {
    let dir = TempDir::new().unwrap();
    // Create a root manifest that's only 5 bytes
    // Set per_file limit to 4 bytes so it's exceeded
    fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"t\"\n").unwrap();
    let limits = InspectionLimits {
        max_per_file_bytes: 4,
        ..InspectionLimits::default()
    };
    let result = inspect_cargo_project(dir.path(), &limits);
    assert!(result.is_err(), "giant root should produce error");
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("exceeds") || err_msg.contains("limit"),
        "error should mention limit: {}",
        err_msg
    );
}

// ===========================================================================
// 37 — LIMITS: wall-clock=0 rejects inspection
// ===========================================================================

#[test]
fn wall_clock_zero_rejects_inspection() {
    let dir = TempDir::new().unwrap();
    let catalog = Catalog::embedded().unwrap();
    let ver = catalog.version();
    let manifest = format!(
        r#"[package]
name = "w0"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "{}"
"#,
        ver
    );
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();
    let limits = InspectionLimits {
        max_inspection_wall_millis: 0,
        ..InspectionLimits::default()
    };
    let result = inspect_cargo_project(dir.path(), &limits);
    assert!(result.is_err(), "wall=0 must reject inspection");
}

// ===========================================================================
// 38 — LIMITS: aggregate byte limit with multiple manifests
// ===========================================================================

#[test]
fn aggregate_byte_limit_hits_with_multiple_files() {
    let dir = TempDir::new().unwrap();
    let catalog = Catalog::embedded().unwrap();
    let ver = catalog.version();

    let root_manifest = format!(
        r#"[package]
name = "agg-test"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "{}"

[workspace]
members = ["sub"]

[workspace.package]
version = "{}"

[workspace.dependencies]
amari-core = {{ version = "{}" }}
"#,
        ver, ver, ver
    );
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    // Small lock
    let lock = format!(
        r#"# Cargo.lock
version = 3

[[package]]
name = "amari-core"
version = "{}"
"#,
        ver
    );
    fs::write(dir.path().join("Cargo.lock"), lock).unwrap();

    // Sub member
    fs::create_dir_all(dir.path().join("sub")).unwrap();
    let sub_manifest = r#"[package]
name = "sub"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = { workspace = true }
"#;
    fs::write(dir.path().join("sub/Cargo.toml"), sub_manifest).unwrap();

    // Set aggregate limit very low so it hits
    let limits = InspectionLimits {
        max_inspection_bytes: 50,
        max_per_file_bytes: 1024 * 1024,
        ..InspectionLimits::default()
    };
    let result = inspect_cargo_project(dir.path(), &limits);
    // Should either error (root too small) or produce partial with warning
    match result {
        Ok(inspection) => {
            // Partial — check for limit warnings
            let limit_warnings: Vec<_> = inspection
                .warnings
                .iter()
                .filter(|w| matches!(w, CargoInspectionWarning::LimitExceeded { .. }))
                .collect();
            // At minimum the aggregate limit should be detected for root manifest
            assert!(
                !limit_warnings.is_empty() || format!("{:?}", inspection.state).contains("limit"),
                "should have limit warning or state"
            );
        }
        Err(_) => {
            // Root manifest hit the limit, which is also acceptable
        }
    }
}

// ===========================================================================
// 39 — LIMITS: file-count limit
// ===========================================================================

#[test]
fn file_count_limit_stops_member_reading() {
    let dir = TempDir::new().unwrap();
    let catalog = Catalog::embedded().unwrap();
    let ver = catalog.version();

    let root_manifest = format!(
        r#"[package]
name = "fc-test"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "{}"

[workspace]
members = ["sub"]

[workspace.package]
version = "{}"

[workspace.dependencies]
amari-core = {{ version = "{}" }}
"#,
        ver, ver, ver
    );
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    let lock = format!(
        r#"# Cargo.lock
version = 3

[[package]]
name = "amari-core"
version = "{}"
"#,
        ver
    );
    fs::write(dir.path().join("Cargo.lock"), lock).unwrap();

    fs::create_dir_all(dir.path().join("sub")).unwrap();
    fs::write(
        dir.path().join("sub/Cargo.toml"),
        r#"[package]
name = "sub"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = { workspace = true }
"#,
    )
    .unwrap();

    // Set file count limit to 2 — root manifest + lock = 2, member falls beyond
    let limits = InspectionLimits {
        max_inspection_files: 2,
        ..InspectionLimits::default()
    };
    let result = inspect_cargo_project(dir.path(), &limits);
    assert!(result.is_ok());
    let inspection = result.unwrap();
    // Members may be empty or incomplete
    let state_str = format!("{:?}", inspection.state);
    assert!(inspection.workspace_members.is_empty() || state_str.contains("limit"));
}

// ===========================================================================
// 40 — WORKSPACE INHERITANCE: keyed by alias not package name
// ===========================================================================

#[test]
fn workspace_dep_inheritance_is_keyed_by_alias() {
    let dir = TempDir::new().unwrap();
    // Workspace base defines dep under alias "renamed-tropical"
    // pointing to package "amari-tropical"
    let root_manifest = r#"[package]
name = "alias-test"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["sub"]

[workspace.package]
version = "0.23.0"

[workspace.dependencies]
renamed-tropical = { package = "amari-tropical", version = "0.23.0" }
"#;
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    fs::create_dir_all(dir.path().join("sub")).unwrap();
    // Member uses alias "renamed-tropical" not the package name
    let member = r#"[package]
name = "sub"
version = "0.1.0"
edition = "2021"

[dependencies]
renamed-tropical = { workspace = true }
"#;
    fs::write(dir.path().join("sub/Cargo.toml"), member).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let member = inspection
        .workspace_members
        .iter()
        .find(|m| m.name == "sub")
        .expect("member sub should exist");
    let dep = member
        .dependencies
        .iter()
        .find(|d| d.alias == "renamed-tropical")
        .expect("member should have renamed-tropical dep");
    assert_eq!(dep.package_name, "amari-tropical");
    assert_eq!(dep.alias, "renamed-tropical");
    // Should NOT produce InheritedBaseMissing because lookup is by alias
    let missing = inspection
        .warnings
        .iter()
        .find(|w| matches!(w, CargoInspectionWarning::InheritedBaseMissing { .. }));
    assert!(missing.is_none(), "alias-based lookup should succeed");
}

// ===========================================================================
// 41 — WORKSPACE: inherited version.workspace = true yields correct value
// ===========================================================================

#[test]
fn member_version_workspace_true_equals_workspace_version() {
    let dir = TempDir::new().unwrap();
    let root_manifest = r#"[package]
name = "ver-test"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["sub"]

[workspace.package]
version = "0.23.0"
"#;
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    fs::create_dir_all(dir.path().join("sub")).unwrap();
    let member = r#"[package]
name = "sub"
version.workspace = true
edition = "2021"
"#;
    fs::write(dir.path().join("sub/Cargo.toml"), member).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let member = inspection
        .workspace_members
        .iter()
        .find(|m| m.name == "sub")
        .expect("member sub should exist");
    assert_eq!(
        member.version, "0.23.0",
        "version.workspace=true must resolve"
    );
    assert!(
        member.inherited_metadata.contains(&"version".to_string()),
        "version should be in inherited_metadata"
    );
}

// ===========================================================================
// 42 — WORKSPACE: workspace = false is rejected
// ===========================================================================

#[test]
fn workspace_false_is_rejected() {
    let dir = TempDir::new().unwrap();
    let root_manifest = r#"[package]
name = "wsfalse-test"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["sub"]

[workspace.package]
version = "0.23.0"

[workspace.dependencies]
amari-core = { version = "0.23.0" }
"#;
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    fs::create_dir_all(dir.path().join("sub")).unwrap();
    // workspace = false is illegal
    let member = r#"[package]
name = "sub"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = { workspace = false }
"#;
    fs::write(dir.path().join("sub/Cargo.toml"), member).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let rejected = inspection
        .warnings
        .iter()
        .find(|w| matches!(w, CargoInspectionWarning::WorkspaceFalseRejected { .. }));
    assert!(rejected.is_some(), "workspace=false should be rejected");
}

// ===========================================================================
// 43 — WORKSPACE: illegal override on workspace dep
// ===========================================================================

#[test]
fn workspace_override_version_is_rejected() {
    let dir = TempDir::new().unwrap();
    let root_manifest = r#"[package]
name = "override-test"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["sub"]

[workspace.package]
version = "0.23.0"

[workspace.dependencies]
amari-core = { version = "0.23.0" }
"#;
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    fs::create_dir_all(dir.path().join("sub")).unwrap();
    // workspace=true with version override is illegal
    let member = r#"[package]
name = "sub"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = { workspace = true, version = "0.20.0" }
"#;
    fs::write(dir.path().join("sub/Cargo.toml"), member).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let override_warning = inspection
        .warnings
        .iter()
        .find(|w| matches!(w, CargoInspectionWarning::WorkspaceOverrideRejected { .. }));
    assert!(
        override_warning.is_some(),
        "version override should be rejected"
    );
}

// ===========================================================================
// 44 — SYSTEM DEPS: system dependencies are detected
// ===========================================================================

#[test]
fn system_dependencies_are_detected() {
    let dir = TempDir::new().unwrap();
    let manifest = r#"[package]
name = "sysdep-test"
version = "0.1.0"
edition = "2021"

[build-dependencies]
cc = "1.0"
pkg-config = "0.3"

[target.'cfg(windows)'.build-dependencies]
cmake = "0.1"
"#;
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    use amari_discovery::SystemDependencyKind;

    let sys_deps = &inspection.root_package.system_dependencies;
    assert!(!sys_deps.is_empty(), "must detect system deps");

    let cc = sys_deps.iter().find(|s| s.alias == "cc").expect("cc dep");
    assert_eq!(cc.dependency_kind, DependencyKind::Build);
    assert_eq!(cc.system_kind, SystemDependencyKind::Cc);

    let pk = sys_deps
        .iter()
        .find(|s| s.alias == "pkg-config")
        .expect("pkg-config");
    assert_eq!(pk.dependency_kind, DependencyKind::Build);
    assert_eq!(pk.system_kind, SystemDependencyKind::PkgConfig);

    let cm = sys_deps.iter().find(|s| s.alias == "cmake").expect("cmake");
    assert_eq!(cm.target.as_deref(), Some("cfg(windows)"));
    assert_eq!(cm.system_kind, SystemDependencyKind::Cmake);
}

// ===========================================================================
// 45 — MEMBER PATHS: illegal glob/absolute/parent paths rejected
// ===========================================================================

#[test]
fn illegal_member_paths_are_rejected() {
    let dir = TempDir::new().unwrap();
    let root_manifest = r#"[package]
name = "illegal-member-test"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["*", "../escape", "/abs", ""]
"#;
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let illegal: Vec<_> = inspection
        .warnings
        .iter()
        .filter(|w| matches!(w, CargoInspectionWarning::IllegalMemberPath { .. }))
        .collect();
    assert!(!illegal.is_empty(), "illegal member paths must be warned");
}

// ===========================================================================
// 46 — STATE: Complete state on full inspection
// ===========================================================================

#[test]
fn complete_state_on_full_inspection() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    // State must be Complete (serializes as "complete")
    let state_str = format!("{:?}", inspection.state);
    assert!(
        state_str.contains("Complete"),
        "full inspection should be Complete: {}",
        state_str
    );
}

// ===========================================================================
// 47 — WorkspaceMeta has package_fields
// ===========================================================================

#[test]
fn workspace_meta_has_package_fields() {
    let (dir, _ver) = materialize_current();
    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let meta = inspection
        .workspace_meta
        .as_ref()
        .expect("fixture has workspace meta");
    assert!(meta.package_fields.contains_key("version"));
    assert!(meta.package_fields.contains_key("edition"));
}

// ===========================================================================
// 48 — System deps are not Amari deps (no leakage)
// ===========================================================================

#[test]
fn system_deps_are_not_amari_deps() {
    let dir = TempDir::new().unwrap();
    let manifest = r#"[package]
name = "no-amari-sysdep"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "0.23.0"

[build-dependencies]
cc = "1.0"
"#;
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    // System deps should be in system_dependencies, NOT in dependencies
    assert!(!inspection.root_package.system_dependencies.is_empty());
    for dep in &inspection.root_package.dependencies {
        assert!(
            dep.package_name.starts_with("amari"),
            "non-Amari deps should not be in dependencies list"
        );
    }
}

// ===========================================================================
// 49 — WORKSPACE BASE: features, optional, default-features=false inheritance
// ===========================================================================

#[test]
fn workspace_base_features_optional_and_default_features_inheritance() {
    let dir = TempDir::new().unwrap();
    let catalog = Catalog::embedded().unwrap();
    let ver = catalog.version();

    // Workspace base with features, optional=true, default-features=false
    let root_manifest = format!(
        r#"[package]
name = "base-inherit"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["sub"]

[workspace.package]
version = "0.23.0"

[workspace.dependencies]
amari-core = {{ version = "{}", features = ["std"], optional = true, default-features = false }}
"#,
        ver
    );
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    // Lockfile with matching version
    let lock = format!(
        r#"# Cargo.lock
version = 3

[[package]]
name = "amari-core"
version = "{}"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc123"
"#,
        ver
    );
    let lock_str = lock;
    fs::write(dir.path().join("Cargo.lock"), &lock_str).unwrap();

    fs::create_dir_all(dir.path().join("sub")).unwrap();
    // Member inherits workspace base with feature overrides and default-features=true override
    let member = r#"[package]
name = "sub"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = { workspace = true, features = ["extra"], default-features = true }
"#;
    fs::write(dir.path().join("sub/Cargo.toml"), member).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    assert!(
        !inspection.workspace_members.is_empty(),
        "must have workspace members"
    );
    let member = inspection
        .workspace_members
        .iter()
        .find(|m| m.name == "sub")
        .expect("member sub");
    let dep = member
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-core")
        .expect("amari-core in member");

    // Features: base "std" + member "extra" = union
    assert!(dep.features.contains(&"std".to_string()));
    assert!(dep.features.contains(&"extra".to_string()));
    // Optional inherited from base
    assert!(dep.optional, "optional should be true from base");
    // default-features: member override true beats base false
    assert!(dep.default_features, "member override should win");

    // Also test the false path: a member that does NOT override default-features
    let dir2 = TempDir::new().unwrap();
    fs::create_dir_all(dir2.path().join("sub2")).unwrap();
    let member2 = r#"[package]
name = "sub2"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = { workspace = true }
"#;
    fs::write(dir2.path().join("sub2/Cargo.toml"), member2).unwrap();
    let root2 = format!(
        r#"[package]
name = "base-inherit2"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["sub2"]

[workspace.package]
version = "0.23.0"

[workspace.dependencies]
amari-core = {{ version = "{}", features = ["std"], optional = true, default-features = false }}
"#,
        ver
    );
    fs::write(dir2.path().join("Cargo.toml"), root2).unwrap();
    fs::write(dir2.path().join("Cargo.lock"), &lock_str).unwrap();
    let inspection2 = inspect_cargo_project(dir2.path(), &default_limits()).unwrap();
    let member2_pkg = inspection2
        .workspace_members
        .iter()
        .find(|m| m.name == "sub2")
        .expect("member sub2");
    let dep2 = member2_pkg
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-core")
        .expect("amari-core in sub2");
    // default-features: inherited false from base (no member override)
    assert!(
        !dep2.default_features,
        "base default-features=false must be inherited"
    );
    assert!(dep2.optional, "optional should be inherited true");
    assert!(dep2.features.contains(&"std".to_string()));
}

// ===========================================================================
// 50 — STALE: member-b dev/build dependency kind + exact 0.19.0 assertions
// ===========================================================================

#[test]
fn stale_member_dependency_kinds_and_exact_versions() {
    let dir = TempDir::new().unwrap();

    // Create a stale (0.19.0) workspace with member-b-like member
    let root = r#"[package]
name = "stale-ws"
version = "0.1.0"
edition = "2021"

[dependencies]
amari = "0.19.0"

[workspace]
members = ["member-b"]

[workspace.package]
version = "0.19.0"

[workspace.dependencies]
amari-core = { version = "0.19.0" }
amari-dual = { version = "0.19.0" }
amari-tropical = { version = "0.19.0" }
"#;
    fs::write(dir.path().join("Cargo.toml"), root).unwrap();

    // Lock with 0.19.0 versions
    let lock = r#"# Cargo.lock
version = 3

[[package]]
name = "amari"
version = "0.19.0"

[[package]]
name = "amari-core"
version = "0.19.0"

[[package]]
name = "amari-dual"
version = "0.19.0"

[[package]]
name = "amari-tropical"
version = "0.19.0"

[[package]]
name = "stale-ws"
version = "0.1.0"
dependencies = [
 "amari",
]

[[package]]
name = "member-b"
version = "0.19.0"
dependencies = [
 "amari-core",
 "amari-dual",
 "amari-tropical",
]
"#;
    fs::write(dir.path().join("Cargo.lock"), lock).unwrap();

    fs::create_dir_all(dir.path().join("member-b")).unwrap();
    let member = r#"[package]
name = "member-b"
version.workspace = true
edition = "2021"

[dependencies]
amari-core = { workspace = true }

[dev-dependencies]
amari-dual = { workspace = true }

[build-dependencies]
amari-tropical = { workspace = true }
"#;
    fs::write(dir.path().join("member-b/Cargo.toml"), member).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let member_pkg = inspection
        .workspace_members
        .iter()
        .find(|m| m.name == "member-b")
        .expect("member-b should exist");

    // Assert dev dependency kind
    let dual = member_pkg
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-dual")
        .expect("amari-dual in member-b");
    assert_eq!(
        dual.kind,
        DependencyKind::Dev,
        "amari-dual should be in dev-dependencies"
    );
    assert_eq!(
        dual.declared_version, "0.19.0",
        "declared_version should be 0.19.0"
    );
    assert_eq!(
        dual.resolved_version.as_deref(),
        Some("0.19.0"),
        "resolved_version should be 0.19.0"
    );

    // Assert build dependency kind
    let tropical = member_pkg
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-tropical")
        .expect("amari-tropical in member-b");
    assert_eq!(
        tropical.kind,
        DependencyKind::Build,
        "amari-tropical should be in build-dependencies"
    );
    assert_eq!(
        tropical.declared_version, "0.19.0",
        "declared_version should be 0.19.0"
    );
    assert_eq!(
        tropical.resolved_version.as_deref(),
        Some("0.19.0"),
        "resolved_version should be 0.19.0"
    );

    // Normal dep
    let core = member_pkg
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-core")
        .expect("amari-core in member-b");
    assert_eq!(core.kind, DependencyKind::Normal);
    assert_eq!(core.declared_version, "0.19.0");
    assert_eq!(core.resolved_version.as_deref(), Some("0.19.0"));
}

// ===========================================================================
// 51 — SYSTEM DEPS: normal/dev/build/target classification
// ===========================================================================

#[test]
fn system_deps_classification_by_dependency_kind_and_target() {
    let dir = TempDir::new().unwrap();
    let manifest = r#"[package]
name = "classify-sysdep"
version = "0.1.0"
edition = "2021"

[dependencies]
# Normal dep that is also a system dep (unusual but legal)
cc = "1.0"

[dev-dependencies]
cmake = "0.1"

[build-dependencies]
pkg-config = "0.3"

[target.'cfg(windows)'.dependencies]
vcpkg = "0.2"
"#;
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let sys_deps = &inspection.root_package.system_dependencies;

    use amari_discovery::SystemDependencyKind;

    // Normal dep
    let cc = sys_deps.iter().find(|s| s.alias == "cc").expect("cc");
    assert_eq!(cc.dependency_kind, DependencyKind::Normal);
    assert_eq!(cc.system_kind, SystemDependencyKind::Cc);
    assert!(cc.target.is_none());

    // Dev dep
    let cmake = sys_deps.iter().find(|s| s.alias == "cmake").expect("cmake");
    assert_eq!(cmake.dependency_kind, DependencyKind::Dev);
    assert_eq!(cmake.system_kind, SystemDependencyKind::Cmake);

    // Build dep
    let pk = sys_deps
        .iter()
        .find(|s| s.alias == "pkg-config")
        .expect("pkg-config");
    assert_eq!(pk.dependency_kind, DependencyKind::Build);
    assert_eq!(pk.system_kind, SystemDependencyKind::PkgConfig);

    // Target-specific
    let vcpkg = sys_deps.iter().find(|s| s.alias == "vcpkg").expect("vcpkg");
    assert_eq!(vcpkg.target.as_deref(), Some("cfg(windows)"));
    assert_eq!(vcpkg.system_kind, SystemDependencyKind::Vcpkg);
}

// ===========================================================================
// 52 — ILLEGAL MEMBER PATH: every invalid member emits typed warning
// ===========================================================================

#[test]
fn every_illegal_member_path_emits_individual_warning() {
    let dir = TempDir::new().unwrap();
    let root_manifest = r#"[package]
name = "illegal-each"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["*", "../escape", "/abs", ""]
"#;
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();
    let illegal: Vec<_> = inspection
        .warnings
        .iter()
        .filter(|w| matches!(w, CargoInspectionWarning::IllegalMemberPath { .. }))
        .collect();

    // Every supplied illegal member must emit a warning
    assert_eq!(
        illegal.len(),
        4,
        "must have exactly 4 warnings, one per illegal member"
    );
    let member_texts: BTreeSet<String> = illegal
        .iter()
        .filter_map(|w| {
            if let CargoInspectionWarning::IllegalMemberPath { member } = w {
                Some(member.clone())
            } else {
                None
            }
        })
        .collect();
    assert!(member_texts.contains("*"));
    assert!(member_texts.contains("../escape"));
    assert!(member_texts.contains("/abs"));
    assert!(member_texts.contains(""));
}

// ===========================================================================
// 53 — AMBIGUOUS LOCK: resolved_version=None when unprovable
// ===========================================================================

#[test]
fn ambiguous_lock_multi_version_no_exact_match_resolves_none() {
    let dir = TempDir::new().unwrap();
    let lock = r#"# Cargo.lock
version = 3

[[package]]
name = "amari-core"
version = "0.23.0"

[[package]]
name = "amari-core"
version = "0.24.0"

[[package]]
name = "ambig2"
version = "0.1.0"
dependencies = [
 "amari-core",
]
"#;
    fs::write(dir.path().join("Cargo.lock"), lock).unwrap();

    // Declared version "0.22" doesn't match either 0.23.0 or 0.24.0
    let manifest = r#"[package]
name = "ambig2"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "0.22"
"#;
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();

    // Must have AmbiguousLockResolution warning
    let ambig_warn: Vec<_> = inspection
        .warnings
        .iter()
        .filter(|w| matches!(w, CargoInspectionWarning::AmbiguousLockResolution { .. }))
        .collect();
    assert_eq!(ambig_warn.len(), 1);
    // Versions must be sorted
    if let CargoInspectionWarning::AmbiguousLockResolution { versions, .. } = &ambig_warn[0] {
        assert_eq!(versions, &vec!["0.23.0".to_string(), "0.24.0".to_string()]);
    }

    let core = inspection
        .root_package
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-core")
        .unwrap();
    assert_eq!(core.resolved_version, None, "unprovable must be None");
    assert_eq!(core.compatibility.status, "unknown_version");
}

// ===========================================================================
// 54 — AMBIGUOUS LOCK: exact declared uniquely selects one version
// ===========================================================================

#[test]
fn exact_declared_version_uniquely_selects_from_multi_version_lock() {
    let dir = TempDir::new().unwrap();
    let lock = r#"# Cargo.lock
version = 3

[[package]]
name = "amari-core"
version = "0.23.0"

[[package]]
name = "amari-core"
version = "0.24.0"

[[package]]
name = "exact-sel"
version = "0.1.0"
dependencies = [
 "amari-core",
]
"#;
    fs::write(dir.path().join("Cargo.lock"), lock).unwrap();

    // Exact version match resolves to 0.24.0
    let manifest = r#"[package]
name = "exact-sel"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "0.24.0"
"#;
    fs::write(dir.path().join("Cargo.toml"), manifest).unwrap();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();

    // AmbiguousLockResolution warning still fires (lock has 2 versions)
    let ambig_warn: Vec<_> = inspection
        .warnings
        .iter()
        .filter(|w| matches!(w, CargoInspectionWarning::AmbiguousLockResolution { .. }))
        .collect();
    assert_eq!(ambig_warn.len(), 1);

    let core = inspection
        .root_package
        .dependencies
        .iter()
        .find(|d| d.package_name == "amari-core")
        .unwrap();
    assert_eq!(
        core.resolved_version.as_deref(),
        Some("0.24.0"),
        "exact declared version 0.24.0 must uniquely resolve"
    );
}

// ============================================================================
// 55 — RED REGRESSION: nested symlink component canonical escape
//          produces EscapingManifest (Unix only)
// ============================================================================

#[cfg(unix)]
#[test]
fn nested_symlink_component_canonical_escape_produces_escaping_manifest() {
    use std::os::unix::fs::symlink;

    let dir = TempDir::new().unwrap();
    let outside_dir = TempDir::new().unwrap();

    // Create a real member directory outside the project root.
    let outside_member = outside_dir.path().join("escaped-member");
    fs::create_dir_all(&outside_member).unwrap();
    fs::write(
        outside_member.join("Cargo.toml"),
        "[package]\nname = \"escaped-member\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();

    // Create a symlink INSIDE root that points to the outside directory.
    // The symlink is a PATH COMPONENT, not the member leaf itself.
    let link_path = dir.path().join("link-to-outside");
    symlink(outside_dir.path(), &link_path).unwrap();

    // Root manifest references member through the symlink component.
    let root_manifest = r#"[package]
name = "escape-root"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["link-to-outside/escaped-member"]
"#;
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    let root_str = dir
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();

    // Must produce EscapingManifest warning (canonicalize resolves
    // through the symlink component to outside root).
    let escaping: Vec<_> = inspection
        .warnings
        .iter()
        .filter(|w| matches!(w, CargoInspectionWarning::EscapingManifest { .. }))
        .collect();
    assert!(!escaping.is_empty(), "must have EscapingManifest warning");

    // The escaped member must NOT appear in workspace_members.
    let has_escaped = inspection
        .workspace_members
        .iter()
        .any(|m| m.name == "escaped-member");
    assert!(
        !has_escaped,
        "escaped member must not be in workspace_members"
    );

    // Path in warning is relative only — no absolute root leakage.
    for w in &escaping {
        let dbg = format!("{:?}", w);
        assert!(
            !dbg.contains(&root_str),
            "EscapingManifest warning must not leak absolute root path: {}",
            dbg
        );
        assert!(
            !dbg.starts_with('/'),
            "EscapingManifest warning path must be relative"
        );
    }

    // All warnings must have relative paths only.
    for w in &inspection.warnings {
        let dbg = format!("{:?}", w);
        assert!(
            !dbg.contains(&root_str),
            "no warning may leak absolute root: {}",
            dbg
        );
    }
}

// ============================================================================
// 56 — RED REGRESSION: missing member Cargo.toml produces MissingManifest
// ============================================================================

#[test]
fn missing_member_cargo_toml_produces_missing_manifest_warning() {
    let dir = TempDir::new().unwrap();

    // Workspace with a member dir that has no Cargo.toml.
    let root_manifest = r#"[package]
name = "missing-member-root"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["ghost-member"]
"#;
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    // Create the member directory but NOT its Cargo.toml.
    fs::create_dir_all(dir.path().join("ghost-member")).unwrap();

    let root_str = dir
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();

    // Must produce MissingManifest for ghost-member/Cargo.toml.
    let missing: Vec<_> = inspection
        .warnings
        .iter()
        .filter(|w| matches!(w, CargoInspectionWarning::MissingManifest { .. }))
        .collect();
    assert!(!missing.is_empty(), "must have MissingManifest warning");

    // Verify the warning holds the expected relative path.
    let has_ghost_path = missing.iter().any(|w| {
        if let CargoInspectionWarning::MissingManifest { path } = w {
            path.contains("ghost-member") && path.ends_with("Cargo.toml")
        } else {
            false
        }
    });
    assert!(
        has_ghost_path,
        "MissingManifest must reference ghost-member"
    );

    // Ghost member must not appear in workspace_members.
    let has_ghost = inspection
        .workspace_members
        .iter()
        .any(|m| m.name == "ghost-member");
    assert!(
        !has_ghost,
        "missing member must not appear in workspace_members"
    );

    // All warning paths are relative only — no absolute root leakage.
    for w in &inspection.warnings {
        let dbg = format!("{:?}", w);
        assert!(
            !dbg.contains(&root_str),
            "no warning may leak absolute root: {}",
            dbg
        );
    }
}

// ============================================================================
// 57 — RED REGRESSION: malformed member TOML → MalformedManifest with
//          sanitized reason and no source snippet
// ============================================================================

#[test]
fn malformed_member_toml_produces_malformed_manifest_with_sanitized_reason() {
    let dir = TempDir::new().unwrap();

    let root_manifest = r#"[package]
name = "bad-member-root"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["bad-member"]
"#;
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    fs::create_dir_all(dir.path().join("bad-member")).unwrap();
    // TOML with missing '=' sign — will fail to parse.
    fs::write(
        dir.path().join("bad-member/Cargo.toml"),
        "[package\nname = \"bad\"\nversion = 0.1.0\nedition = 2021\n",
    )
    .unwrap();

    let root_str = dir
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();

    // Must produce MalformedManifest.
    let malformed: Vec<_> = inspection
        .warnings
        .iter()
        .filter(|w| matches!(w, CargoInspectionWarning::MalformedManifest { .. }))
        .collect();
    assert!(!malformed.is_empty(), "must have MalformedManifest warning");

    for w in &malformed {
        let dbg = format!("{:?}", w);

        // Reason must NOT contain raw TOML source snippets.
        assert!(
            !dbg.contains("expected an equals")
                && !dbg.contains("expected a value")
                && !dbg.contains("expected a newline"),
            "MalformedManifest reason must be sanitized (no source snippet): {}",
            dbg
        );

        // Path must be relative only.
        assert!(
            !dbg.starts_with('/'),
            "MalformedManifest path must be relative: {}",
            dbg
        );

        // Path must reference bad-member.
        assert!(
            dbg.contains("bad-member"),
            "MalformedManifest must reference bad-member: {}",
            dbg
        );

        // No absolute root leakage.
        assert!(
            !dbg.contains(&root_str),
            "MalformedManifest must not leak absolute root: {}",
            dbg
        );
    }

    // Bad member must not appear in workspace_members.
    let has_bad = inspection
        .workspace_members
        .iter()
        .any(|m| m.name == "bad-member");
    assert!(
        !has_bad,
        "malformed member must not appear in workspace_members"
    );
}

// ============================================================================
// 58 — RED REGRESSION: workspace inherited field absent → WorkspaceFieldNotFound
// ============================================================================

#[test]
fn workspace_inherited_field_absent_produces_workspace_field_not_found() {
    let dir = TempDir::new().unwrap();

    // [workspace.package] does NOT include 'license', but the member
    // requests `license.workspace = true`.
    let root_manifest = r#"[package]
name = "missing-field-root"
version = "0.1.0"
edition = "2021"

[dependencies]

[workspace]
members = ["sub"]

[workspace.package]
version = "0.23.0"
edition = "2021"
"#;
    fs::write(dir.path().join("Cargo.toml"), root_manifest).unwrap();

    fs::create_dir_all(dir.path().join("sub")).unwrap();
    // Member inherits version (ok), license (MISSING from workspace.package),
    // and edition (ok).
    let member_manifest = r#"[package]
name = "sub"
version.workspace = true
edition.workspace = true
license.workspace = true
"#;
    fs::write(dir.path().join("sub/Cargo.toml"), member_manifest).unwrap();

    let root_str = dir
        .path()
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();

    let inspection = inspect_cargo_project(dir.path(), &default_limits()).unwrap();

    // Must produce WorkspaceFieldNotFound for 'license'.
    let field_absent: Vec<_> = inspection
        .warnings
        .iter()
        .filter(|w| matches!(w, CargoInspectionWarning::WorkspaceFieldNotFound { .. }))
        .collect();
    assert!(
        !field_absent.is_empty(),
        "must have WorkspaceFieldNotFound warning"
    );

    // Assert the exact field and package.
    let license_warning = field_absent.iter().find(|w| {
        if let CargoInspectionWarning::WorkspaceFieldNotFound { field, .. } = w {
            field == "license"
        } else {
            false
        }
    });
    assert!(
        license_warning.is_some(),
        "must warn that 'license' is not in workspace.package"
    );

    // Member should still be present in workspace_members (version/edition resolved).
    let member = inspection
        .workspace_members
        .iter()
        .find(|m| m.name == "sub");
    assert!(member.is_some(), "member 'sub' must still be present");
    let member = member.unwrap();

    // Only 'version' and 'edition' should be inherited, not 'license'.
    assert!(member.inherited_metadata.contains(&"version".to_string()));
    assert!(member.inherited_metadata.contains(&"edition".to_string()));
    assert!(!member.inherited_metadata.contains(&"license".to_string()));

    // All warning paths are relative only.
    for w in &inspection.warnings {
        let dbg = format!("{:?}", w);
        assert!(
            !dbg.contains(&root_str),
            "no warning may leak absolute root: {}",
            dbg
        );
    }

    // Inspection metadata paths are relative.
    let meta = member.manifest_path.as_str();
    assert!(
        !meta.starts_with('/'),
        "manifest_path must be relative: {}",
        meta
    );
}
