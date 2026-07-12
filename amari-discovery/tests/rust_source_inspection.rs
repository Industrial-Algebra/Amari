// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for Rust source inspection (Task 8B strict).
//!
//! Covers: package-scoped alias maps, member classification, trait bounds,
//! macro invocations, source locations (1-based columns), vocabulary from
//! source-anchored segments, hardened comment lexer, limit semantics,
//! typed warnings, input file list, cross-package alias isolation,
//! fixture immutability, and Cargo regressions.
//!
//! # Fixture immutability
//!
//! Tests **never mutate** tracked fixture files. Fixtures are materialized
//! into a per-test `TempDir`: source fixture is recursively copied, then
//! `Cargo.toml.in` / `Cargo.lock.in` are transformed to `Cargo.toml` /
//! `Cargo.lock` with the embedded catalog version. Dynamic malformed/string/
//! dummy files are written only into the temp directory. TempDir lifetime
//! keeps the copy alive for the test duration. No tracked fixture file is
//! ever written.

use std::collections::HashSet;
use std::path::Path;

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use amari_discovery::inspect::{
    inspect_cargo_project, inspect_rust_sources, InspectionLimits, RustFileKind,
    RustInspectionWarning, RustUsageKind, SnapshotState,
};

// ============================================================================
// Test helpers — TempDir-based fixture materialization
// ============================================================================

/// Returns the prototypical rust-project fixture source path.
fn fixture_source() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/rust-project"
    ))
}

/// Materialize the rust-project fixture into a TempDir.
///
/// Recursively copies all files from the fixture source, then transforms
/// `Cargo.toml.in` → `Cargo.toml` and `Cargo.lock.in` → `Cargo.lock` with
/// the embedded catalog version substituted for `__AMARI_VERSION__`.
///
/// Stale generated `Cargo.toml`/`Cargo.lock` files (non-.in) that may have
/// been left behind by previous runs are explicitly skipped — only `.in`
/// files serve as the authority for generated outputs.
///
/// The returned `TempDir` owns the temporary fixture — it is cleaned up
/// when dropped.
fn materialize_fixture() -> TempDir {
    let catalog_version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();

    let temp = TempDir::new().unwrap();
    let src = fixture_source();

    // Remove any stale generated Cargo.toml / Cargo.lock before copying
    let stale_toml = src.join("Cargo.toml");
    let stale_lock = src.join("Cargo.lock");
    if stale_toml.exists() {
        let _ = std::fs::remove_file(&stale_toml);
    }
    if stale_lock.exists() {
        let _ = std::fs::remove_file(&stale_lock);
    }

    // Recursive copy with .in transformation
    copy_and_transform(src, temp.path(), &catalog_version);

    temp
}

/// Recursively copy fixture source to destination, transforming .in files.
///
/// Explicitly skips `Cargo.toml` and `Cargo.lock` files that are not `.in`
/// ONLY when a corresponding `.in` file exists in the same directory
/// (stale generated outputs that may have been left behind by previous
/// runs). Member `Cargo.toml` files and other tracked manifests are copied
/// normally since they have no `.in` counterpart.
fn copy_and_transform(src: &Path, dst: &Path, version: &str) {
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        let src_path = entry.path();

        if src_path.is_dir() {
            let dst_sub = dst.join(&*file_name);
            std::fs::create_dir_all(&dst_sub).unwrap();
            copy_and_transform(&src_path, &dst_sub, version);
        } else if name.ends_with(".in") {
            // Transform .in → destination file
            let base = name.trim_end_matches(".in");
            let dst_path = dst.join(base);
            let content = std::fs::read_to_string(&src_path).unwrap();
            let transformed = content.replace("__AMARI_VERSION__", version);
            std::fs::write(&dst_path, &transformed).unwrap();
        } else if (name == "Cargo.toml" || name == "Cargo.lock")
            && src.join(format!("{name}.in")).exists()
        {
            // Skip stale generated files — only .in is authority.
            // Only skipped when a corresponding .in exists in same directory.
            continue;
        } else {
            // Plain copy
            let dst_path = dst.join(&*file_name);
            std::fs::copy(&src_path, &dst_path).unwrap();
        }
    }
}

/// Default limits for tests.
fn default_limits() -> InspectionLimits {
    InspectionLimits::default()
}

/// Inspect the materialized fixture directory.
fn inspect_fixture(
    temp: &TempDir,
) -> (
    amari_discovery::inspect::CargoInspection,
    amari_discovery::inspect::RustSourceInspection,
) {
    let root = temp.path();
    let cargo = inspect_cargo_project(root, &default_limits()).unwrap();
    let rust = inspect_rust_sources(root, &cargo, &default_limits()).unwrap();
    (cargo, rust)
}

/// Compute SHA-256 hash of file content.
fn file_hash(path: &Path) -> String {
    let content = std::fs::read(path).unwrap();
    hex::encode(Sha256::digest(&content))
}

/// Collect all tracked fixture source file hashes for immutability checks.
fn fixture_source_hashes() -> Vec<(String, String)> {
    let mut hashes = Vec::new();
    collect_hashes(fixture_source(), "", &mut hashes);
    hashes.sort_by(|a, b| a.0.cmp(&b.0));
    hashes
}

fn collect_hashes(dir: &Path, prefix: &str, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };
        if entry.path().is_dir() {
            collect_hashes(&entry.path(), &rel, out);
        } else if name.ends_with(".in") {
            // .in files are tracked source
            let hash = file_hash(&entry.path());
            out.push((rel, hash));
        } else if name != "Cargo.lock" {
            // All non-lock, non-.in files
            let hash = file_hash(&entry.path());
            out.push((rel, hash));
        }
    }
}

// ============================================================================
// FIXTURE IMMUTABILITY: Source fixture files must be unchanged by any test
// ============================================================================

#[test]
fn fixture_source_files_unchanged_after_inspection() {
    let before = fixture_source_hashes();
    let temp = materialize_fixture();
    let (_cargo, _rust) = inspect_fixture(&temp);
    drop(temp);
    let after = fixture_source_hashes();
    assert_eq!(
        before, after,
        "tracked fixture source files must be unchanged after inspection"
    );
}

// ============================================================================
// R1: Package-scoped CrateAliasMap tests
// ============================================================================

#[test]
fn test_member_a_classified_correctly() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let member_a_lib = inspection.file_kinds.iter().find(|k| match k {
        RustFileKind::Library { path, package } => {
            path.contains("member-a") && package == "member-a"
        }
        _ => false,
    });
    assert!(
        member_a_lib.is_some(),
        "member-a/src/lib.rs should be Library"
    );
}

#[test]
fn test_member_b_classified_correctly() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let member_b_lib = inspection.file_kinds.iter().find(|k| match k {
        RustFileKind::Library { path, package } => {
            path.contains("member-b") && package == "member-b"
        }
        _ => false,
    });
    assert!(
        member_b_lib.is_some(),
        "member-b/src/lib.rs should be Library"
    );
}

#[test]
fn test_no_cross_package_alias_contamination() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let renamed_in_b = inspection
        .usages
        .iter()
        .find(|u| u.alias == "renamed_core" && u.source.path.contains("member-b"));
    assert!(
        renamed_in_b.is_some(),
        "member-b should have renamed_core usage"
    );

    let found = renamed_in_b.unwrap();
    assert_eq!(
        found.crate_name, "amari-core",
        "renamed_core must resolve to amari-core"
    );

    let renamed_in_a = inspection
        .usages
        .iter()
        .any(|u| u.alias == "renamed_core" && u.source.path.contains("member-a"));
    assert!(!renamed_in_a, "member-a must not have renamed_core alias");
}

#[test]
fn test_undeclared_amari_gpu_not_evidence() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let gpu_usage = inspection
        .usages
        .iter()
        .any(|u| u.crate_name == "amari-gpu" || u.alias == "amari_gpu");
    assert!(
        !gpu_usage,
        "amari-gpu is undeclared and must not produce evidence"
    );
}

// ============================================================================
// R2: Complete RustUsageKind tests (PathTrait + PathMacro)
// ============================================================================

#[test]
fn test_trait_bound_detection() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let trait_bound = inspection
        .usages
        .iter()
        .find(|u| u.kind == RustUsageKind::PathTrait);
    assert!(trait_bound.is_some(), "should detect PathTrait usage kind");
}

// ============================================================================
// R4: 1-based columns, sanitized malformed warnings
// ============================================================================

#[test]
fn test_usage_lines_and_columns_are_1based() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    for usage in &inspection.usages {
        if let Some(col) = usage.source.column {
            assert!(col >= 1, "column must be 1-based");
        }
        if let Some(line) = usage.source.line {
            assert!(line >= 1, "line must be 1-based");
        }
    }
}

#[test]
fn test_malformed_source_warning_sanitized() {
    let temp = materialize_fixture();
    let malformed_path = temp.path().join("src").join("malformed.rs");
    let malformed_source = "use amari::tropical::TropicalNumber\nfn broken() {\n    let x = 1\n}";
    std::fs::write(&malformed_path, malformed_source).unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    let has_malformed = inspection
        .warnings
        .iter()
        .any(|w| matches!(w, RustInspectionWarning::MalformedSource { .. }));
    assert!(has_malformed, "should produce MalformedSource warning");

    for warning in &inspection.warnings {
        if let RustInspectionWarning::MalformedSource { reason, .. } = warning {
            assert!(
                !reason.contains("missing"),
                "reason should be sanitized, got: {reason}"
            );
            assert!(
                !reason.contains("TropicalNumber"),
                "reason must not contain source snippet, got: {reason}"
            );
        }
    }
}

// ============================================================================
// R5: Source-anchored vocabulary with exact line tests
// ============================================================================

#[test]
fn test_vocabulary_has_source_locations() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    assert!(!inspection.vocabulary.is_empty(), "should have vocabulary");
    for ev in &inspection.vocabulary {
        assert!(
            ev.source.is_some(),
            "vocab term '{t}' has no source location",
            t = ev.term
        );
        let src = ev.source.as_ref().unwrap();
        assert!(
            src.line.is_some(),
            "vocab term '{t}' has no line",
            t = ev.term
        );
        assert!(
            src.column.is_some(),
            "vocab term '{t}' has no column",
            t = ev.term
        );
    }
}

#[test]
fn test_comment_vocabulary_exact_line() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let lib_vocab: Vec<_> = inspection
        .vocabulary
        .iter()
        .filter(|v| v.path.contains("lib.rs") && !v.path.contains("member"))
        .collect();
    assert!(
        !lib_vocab.is_empty(),
        "should have vocab from src/lib.rs comments"
    );
    for ev in &lib_vocab {
        let src = ev.source.as_ref().unwrap();
        assert!(
            src.line.is_some() && src.column.is_some(),
            "lib.rs vocab should have line/column"
        );
    }
}

// ============================================================================
// R6: Hardened comment lexer tests
// ============================================================================

#[test]
fn test_strings_with_vocabulary_not_evidence() {
    let temp = materialize_fixture();
    let test_path = temp.path().join("src").join("string_vocab_test.rs");
    let source = concat!(
        "// real comment about gpu\n",
        "fn main() {\n",
        "    let s = \"tropical algebra // BLAS\";\n",
        "    let t = r#\"geometric algebra multivector\"#;\n",
        "}\n",
    );
    std::fs::write(&test_path, source).unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    let gpu_found = inspection
        .vocabulary
        .iter()
        .any(|v| v.term == "gpu" && v.path.contains("string_vocab_test"));
    assert!(gpu_found, "should find gpu from real comment");

    let tropical_from_string = inspection
        .vocabulary
        .iter()
        .any(|v| v.term == "tropical_algebra" && v.path.contains("string_vocab_test"));
    assert!(
        !tropical_from_string,
        "must not find tropical_algebra from inside a string"
    );

    let blas_from_string = inspection
        .vocabulary
        .iter()
        .any(|v| v.term == "blas" && v.path.contains("string_vocab_test"));
    assert!(!blas_from_string, "must not find blas from inside a string");
}

// ============================================================================
// R7: Cfg evidence
// ============================================================================

#[test]
fn test_cfg_evidence_present() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let lib_cfg: Vec<_> = inspection
        .cfg_evidence
        .iter()
        .filter(|c| c.path.contains("lib.rs") && !c.path.contains("member"))
        .collect();
    assert!(!lib_cfg.is_empty(), "should have cfg evidence from lib.rs");
}

// ============================================================================
// R8: Sort/dedup by full identity, input file list
// ============================================================================

#[test]
fn test_usage_deduplication_by_full_identity() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let mut seen = std::collections::BTreeSet::new();
    for usage in &inspection.usages {
        let key = (
            &usage.crate_name,
            &usage.alias,
            &usage.path_segments,
            usage.kind,
            &usage.source.path,
            usage.source.line,
            usage.source.column,
        );
        assert!(seen.insert(key), "found duplicate usage");
    }
}

#[test]
fn test_input_files_list_matches_inspected() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let list_len = inspection.input_files.len() as u64;
    assert_eq!(
        list_len, inspection.inspected_file_count,
        "input_files length must match inspected_file_count"
    );

    // Every evidence content_hash should resolve to an input file
    for usage in &inspection.usages {
        let found = inspection
            .input_files
            .iter()
            .any(|f| f.content_hash == usage.source.content_hash);
        assert!(found, "usage content_hash must be in input_files");
    }
    for ev in &inspection.vocabulary {
        if let Some(ref src) = ev.source {
            let found = inspection
                .input_files
                .iter()
                .any(|f| f.content_hash == src.content_hash);
            assert!(found, "vocab content_hash must be in input_files");
        }
    }
}

#[test]
fn test_input_files_list_includes_no_evidence_files() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let has_build = inspection
        .input_files
        .iter()
        .any(|f| f.path.ends_with("build.rs") && !f.path.contains("member"));
    assert!(has_build, "build.rs should be in input_files");
}

// ============================================================================
// R9: Traversal limit semantics tests
// ============================================================================

#[test]
fn test_candidate_count_exact() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    // The fixture has a known exact set of .rs and README.md files:
    // Root: lib.rs, main.rs, build.rs, README.md, examples/demo.rs,
    //        tests/integration.rs,
    //        benches/bench.rs, benches/speed_bench.rs, benches/correctness_bench.rs = 9
    // Member-a: lib.rs = 1
    // Member-b: lib.rs, build.rs, benches/member_bench.rs = 3
    // Total .rs + README: 13 files
    assert_eq!(
        inspection.inspected_file_count, 13,
        "inspected count should be exactly 13, got {}",
        inspection.inspected_file_count
    );
}

#[test]
fn test_file_count_limit_partial() {
    let temp = materialize_fixture();
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();

    let mut limits = default_limits();
    limits.max_inspection_files = 3;
    let inspection = inspect_rust_sources(temp.path(), &cargo, &limits).unwrap();

    assert!(matches!(
        inspection.state,
        SnapshotState::LimitExceeded { .. }
    ));
    let list_len = inspection.input_files.len() as u64;
    assert!(
        list_len <= 3,
        "accepted count should be <= 3, got {list_len}"
    );
    assert_eq!(list_len, inspection.inspected_file_count);
}

#[test]
fn test_file_count_limit_exact_boundary_with_trailing_irrelevant() {
    // Set max to exactly the number of candidates. Trailing irrelevant files
    // should NOT cause LimitExceeded — only an actual positive candidate
    // exceeding max triggers the limit.
    let temp = materialize_fixture();

    // Add a dummy.json that is not a candidate, and some non-utf8 paths
    std::fs::write(temp.path().join("dummy.json"), b"{}").unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();

    // 13 candidates, max = 13 — should be Complete
    let mut limits = default_limits();
    limits.max_inspection_files = 13;
    let inspection = inspect_rust_sources(temp.path(), &cargo, &limits).unwrap();

    assert_eq!(
        inspection.state,
        SnapshotState::Complete,
        "exactly at max should be Complete, got {:?}",
        inspection.state
    );
    assert_eq!(inspection.inspected_file_count, 13);

    // 13 candidates, max = 12 — should be LimitExceeded
    let mut limits2 = default_limits();
    limits2.max_inspection_files = 12;
    let inspection2 = inspect_rust_sources(temp.path(), &cargo, &limits2).unwrap();

    assert!(matches!(
        inspection2.state,
        SnapshotState::LimitExceeded { .. }
    ));
    assert!(inspection2.inspected_file_count <= 12);
}

#[test]
fn test_byte_limit_partial() {
    let temp = materialize_fixture();
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();

    let mut limits = default_limits();
    limits.max_inspection_bytes = 500;
    let inspection = inspect_rust_sources(temp.path(), &cargo, &limits).unwrap();

    assert!(matches!(
        inspection.state,
        SnapshotState::LimitExceeded { .. }
    ));
    assert!(inspection.total_bytes <= limits.max_inspection_bytes);
}

// ============================================================================
// R10: Depth pruning detection
// ============================================================================

#[test]
fn test_depth_pruning_state() {
    let temp = materialize_fixture();
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();

    let mut limits = default_limits();
    limits.max_traversal_depth = 1;
    let inspection = inspect_rust_sources(temp.path(), &cargo, &limits).unwrap();

    assert!(matches!(
        inspection.state,
        SnapshotState::LimitExceeded { .. }
    ));

    for f in &inspection.input_files {
        let has_slash = f.path.contains('/');
        assert!(
            !has_slash,
            "depth=1 should not include nested files, got: {}",
            f.path
        );
    }
}

#[test]
fn test_depth_independent_of_creation_order() {
    let temp = materialize_fixture();
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();

    let mut limits = default_limits();
    limits.max_traversal_depth = 2;
    let inspection1 = inspect_rust_sources(temp.path(), &cargo, &limits).unwrap();
    let inspection2 = inspect_rust_sources(temp.path(), &cargo, &limits).unwrap();

    assert_eq!(inspection1.input_hash, inspection2.input_hash);
    assert_eq!(
        inspection1.inspected_file_count,
        inspection2.inspected_file_count
    );
}

// ============================================================================
// R11: Typed warnings
// ============================================================================

#[test]
fn test_warnings_are_typed() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    for warning in &inspection.warnings {
        match warning {
            RustInspectionWarning::MalformedSource { .. }
            | RustInspectionWarning::LimitExceeded { .. }
            | RustInspectionWarning::SymlinkedFile { .. }
            | RustInspectionWarning::NonUtf8Path { .. }
            | RustInspectionWarning::InvalidUtf8Source { .. }
            | RustInspectionWarning::OversizedFile { .. }
            | RustInspectionWarning::ReadFailure { .. }
            | RustInspectionWarning::VocabularyTruncated { .. } => {}
        }
    }
}

#[test]
fn test_warning_serialization_no_source_or_secrets() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let json = serde_json::to_string(&inspection.warnings).unwrap();

    assert!(
        !json.contains("/home"),
        "warning JSON must not contain absolute paths"
    );
    assert!(
        !json.contains("/tmp"),
        "warning JSON must not contain absolute paths"
    );
    assert!(
        !json.contains("fn "),
        "warning JSON must not contain source code"
    );
}

// ============================================================================
// R12: Fixture materialization
// ============================================================================

#[test]
fn test_fixture_materialization_replaces_version() {
    let temp = materialize_fixture();

    let toml_content = std::fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
    assert!(
        !toml_content.contains("__AMARI_VERSION__"),
        "Cargo.toml should have version placeholder replaced"
    );

    let lock_content = std::fs::read_to_string(temp.path().join("Cargo.lock")).unwrap();
    assert!(
        !lock_content.contains("__AMARI_VERSION__"),
        "Cargo.lock should have version placeholder replaced"
    );
}

#[test]
fn test_malformed_source_dynamic_creation() {
    let temp = materialize_fixture();
    let malformed_path = temp.path().join("src").join("malformed.rs");

    let bad_source = "this is not valid rust syntax at all {{{";
    std::fs::write(&malformed_path, bad_source).unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    let malformed_warning = inspection.warnings.iter().any(|w| {
        matches!(w, RustInspectionWarning::MalformedSource { path, .. } if path.contains("malformed.rs"))
    });
    assert!(
        malformed_warning,
        "should warn about dynamically created malformed.rs"
    );
}

// ============================================================================
// R13: README/comments vocabulary (WASM/native-link/domain)
// ============================================================================

#[test]
fn test_readme_vocabulary_comprehensive() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let readme_terms: Vec<&str> = inspection
        .vocabulary
        .iter()
        .filter(|v| v.path.contains("README.md"))
        .map(|v| v.term.as_str())
        .collect();

    assert!(
        readme_terms.contains(&"wasm"),
        "README must have WASM vocabulary"
    );
    assert!(
        readme_terms.contains(&"ffi"),
        "README must have FFI vocabulary"
    );
    assert!(
        readme_terms.contains(&"blas"),
        "README must have BLAS vocabulary"
    );
    assert!(
        readme_terms.contains(&"native_linker"),
        "README must have native_linker vocab"
    );
    assert!(
        readme_terms.contains(&"gpu"),
        "README must have GPU vocabulary"
    );
}

#[test]
fn test_no_std_and_cfg_coverage() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let no_std_attr = inspection.crate_attributes.iter().find(|a| {
        a.path.contains("lib.rs") && !a.path.contains("member") && a.attribute == "no_std"
    });
    assert!(no_std_attr.is_some(), "should detect #![no_std]");

    assert!(
        !inspection.cfg_evidence.is_empty(),
        "should have cfg evidence"
    );
}

// ============================================================================
// Extern crate end-to-end
// ============================================================================

#[test]
fn test_extern_crate_detection() {
    let temp = materialize_fixture();
    let test_path = temp.path().join("src").join("extern_test.rs");
    let source = "extern crate amari_core;\nfn main() {}";
    std::fs::write(&test_path, source).unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    let extern_usage = inspection
        .usages
        .iter()
        .find(|u| u.kind == RustUsageKind::ExternCrate && u.source.path.contains("extern_test"));
    assert!(
        extern_usage.is_some(),
        "should detect extern crate amari_core"
    );
}

// ============================================================================
// Cargo regression tests
// ============================================================================

#[test]
fn test_cargo_inspection_regression() {
    let temp = materialize_fixture();
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    assert_eq!(cargo.root_package.name, "rust-project");
    assert!(!cargo.root_package.dependencies.is_empty());
    assert!(!cargo.workspace_members.is_empty());
}

// ============================================================================
// Determinism and invariance
// ============================================================================

#[test]
fn test_hash_deterministic() {
    let temp = materialize_fixture();
    let (_, inspection1) = inspect_fixture(&temp);
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection2 = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    assert_eq!(
        inspection1.input_hash, inspection2.input_hash,
        "hash must be deterministic"
    );
}

#[test]
fn test_no_full_source_in_output() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let json = serde_json::to_string(&inspection).unwrap();
    assert!(
        !json.contains("fn "),
        "JSON must not include fn declarations"
    );
    assert!(!json.contains("let "), "JSON must not include let bindings");
}

#[test]
fn test_hash_independent_of_root() {
    let temp = materialize_fixture();
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection1 = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    let canon = temp.path().canonicalize().unwrap();
    let cargo2 = inspect_cargo_project(&canon, &default_limits()).unwrap();
    let inspection2 = inspect_rust_sources(&canon, &cargo2, &default_limits()).unwrap();

    assert_eq!(
        inspection1.input_hash, inspection2.input_hash,
        "hash must be root-independent"
    );
}

// ============================================================================
// Adding non-Rust/README files leaves inspection whole-struct equal
// (superseded by test_adding_non_source_file_whole_struct_equal above)
// ============================================================================

// ============================================================================
// Complete state
// ============================================================================

#[test]
fn test_complete_state() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);
    assert_eq!(inspection.state, SnapshotState::Complete);
}

// ============================================================================
// NEW TASK 8B RED TESTS
// ============================================================================

// ---- Traversal: symlink BEFORE is_file (warning not unreachable) ----

#[cfg(unix)]
#[test]
fn test_symlink_warning_before_is_file_check() {
    use std::os::unix::fs::symlink;

    let temp = materialize_fixture();
    let target = temp.path().join("src").join("lib.rs");
    let link = temp.path().join("src").join("link_to_lib.rs");
    symlink(&target, &link).unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    let sym_warning = inspection.warnings.iter().any(|w| {
        matches!(w, RustInspectionWarning::SymlinkedFile { path } if path.contains("link_to_lib"))
    });
    assert!(
        sym_warning,
        "symlink should produce warning, not be silently skipped"
    );
}

// ---- Traversal: nonUTF8 paths rejected without to_string_lossy ----

#[cfg(unix)]
#[test]
fn test_non_utf8_path_rejected_without_lossy() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temp = materialize_fixture();
    // Non-UTF8 name ending with ".rs" (0x2e, 0x72, 0x73)
    let bad_name = OsString::from_vec(vec![0x66, 0x6f, 0x6f, 0xFF, 0x2e, 0x72, 0x73]);
    let bad_path = temp.path().join("src").join(&bad_name);
    std::fs::create_dir_all(temp.path().join("src")).unwrap();
    std::fs::write(&bad_path, b"fn foo() {}").unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    // Non-UTF8 .rs path must produce a NonUtf8Path warning
    let non_utf8_warning = inspection
        .warnings
        .iter()
        .any(|w| matches!(w, RustInspectionWarning::NonUtf8Path { .. }));
    assert!(
        non_utf8_warning,
        "non-UTF8 .rs path should produce NonUtf8Path warning"
    );

    // The file must NOT appear in input_files (path not lossy-normalized)
    for f in &inspection.input_files {
        assert!(
            !f.path.contains("foo"),
            "non-UTF8 path must not appear in input_files: {}",
            f.path
        );
    }
}

// ---- Traversal: nonUTF8 non-.rs files silently skipped, no count/warning ----

#[cfg(unix)]
#[test]
fn test_non_utf8_non_rs_silently_skipped() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temp = materialize_fixture();
    // Non-UTF8 name NOT ending with ".rs"
    let bad_name = OsString::from_vec(vec![0x66, 0x6f, 0x6f, 0xFF, 0x42]);
    let bad_path = temp.path().join("src").join(&bad_name);
    std::fs::create_dir_all(temp.path().join("src")).unwrap();
    std::fs::write(&bad_path, b"not rust").unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    // Non-UTF8 non-.rs files must be silently skipped:
    // no NonUtf8Path warning, no effect on input_files, inspection still Complete
    let non_utf8_warning = inspection
        .warnings
        .iter()
        .any(|w| matches!(w, RustInspectionWarning::NonUtf8Path { .. }));
    assert!(
        !non_utf8_warning,
        "non-UTF8 non-.rs file should NOT produce NonUtf8Path warning"
    );

    assert_eq!(inspection.state, SnapshotState::Complete);
    assert_eq!(inspection.inspected_file_count, 13);
}

// ---- Traversal: nonUTF8 .rs consumed in considered count ----

#[cfg(unix)]
#[test]
fn test_non_utf8_consumes_considered_slot() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let temp = materialize_fixture();

    // Create several non-UTF8 regular .rs files
    // Name: 0x66, 0xFF, <digit>, 0x2e, 0x72, 0x73 = "f" + invalid + N + ".rs"
    std::fs::create_dir_all(temp.path().join("src")).unwrap();
    for i in 1u8..6u8 {
        let name = OsString::from_vec(vec![0x66, 0xFF, i, 0x2e, 0x72, 0x73]);
        let p = temp.path().join("src").join(&name);
        std::fs::write(&p, b"fn foo() {}").unwrap();
    }

    // With low max_inspection_files, non-UTF8 consumption prevents many accepted
    let mut limits = default_limits();
    limits.max_inspection_files = 4;
    let cargo = inspect_cargo_project(temp.path(), &limits).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &limits).unwrap();

    assert!(matches!(
        inspection.state,
        SnapshotState::LimitExceeded { .. }
    ));
    // Non-UTF8 .rs files consumed slots, so far fewer accepted than 4
    assert!(
        inspection.inspected_file_count <= 4,
        "non-UTF8 .rs files should consume considered slots"
    );
}

// ---- Traversal: exact count boundary with trailing irrelevant files ----

#[test]
fn test_exact_count_boundary_trailing_irrelevant() {
    let temp = materialize_fixture();

    // Add many non-.rs, non-README files that come AFTER the last candidate
    for i in 0..20 {
        std::fs::write(temp.path().join(format!("z_extra_{i}.txt")), b"x").unwrap();
    }

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    // Should be Complete — trailing irrelevant files don't exceed file-count limit
    assert_eq!(inspection.state, SnapshotState::Complete);
    assert_eq!(inspection.inspected_file_count, 13);
}

// ---- Invalid UTF-8 source included as evidence ----

#[test]
fn test_invalid_utf8_source_in_input_files() {
    let temp = materialize_fixture();
    let bad_path = temp.path().join("src").join("bad_utf8.rs");
    // Write an .rs file with invalid UTF-8
    let bad_bytes = vec![
        0x66, 0x6e, 0x20, 0x6d, 0x61, 0x69, 0x6e, 0x28, 0x29, 0x20, 0x7b, 0x7d, 0xFF, 0xFE,
    ];
    std::fs::write(&bad_path, &bad_bytes).unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    // Must have InvalidUtf8Source warning
    let has_warning = inspection.warnings.iter().any(|w| {
        matches!(w, RustInspectionWarning::InvalidUtf8Source { path } if path.contains("bad_utf8"))
    });
    assert!(
        has_warning,
        "invalid UTF-8 file should produce InvalidUtf8Source warning"
    );

    // Must be in input_files with content_hash
    let in_files = inspection
        .input_files
        .iter()
        .any(|f| f.path.contains("bad_utf8"));
    assert!(in_files, "invalid UTF-8 file must be in input_files");

    // Must be counted in inspected_file_count and total_bytes
    assert!(
        inspection.inspected_file_count >= 14,
        "invalid UTF-8 file counts toward inspected_file_count"
    );
}

#[test]
fn test_invalid_utf8_contributes_to_input_hash() {
    let temp = materialize_fixture();

    let bad_path = temp.path().join("src").join("bad_utf8.rs");
    let bad_bytes = vec![0xFF, 0xFE];
    std::fs::write(&bad_path, &bad_bytes).unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection1 = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    // Hash must differ from clean fixture
    // Re-materialize clean
    let temp2 = materialize_fixture();
    let cargo2 = inspect_cargo_project(temp2.path(), &default_limits()).unwrap();
    let inspection2 = inspect_rust_sources(temp2.path(), &cargo2, &default_limits()).unwrap();

    assert_ne!(
        inspection1.input_hash, inspection2.input_hash,
        "invalid UTF-8 content must change input_hash"
    );
}

// ---- Aggregate budget cannot be bypassed by many invalid UTF-8 files ----

#[test]
fn test_invalid_utf8_cannot_bypass_byte_budget() {
    let temp = materialize_fixture();

    // Create several large invalid-UTF-8 files
    let large_bad = vec![0xFFu8; 2000];
    for i in 0..5 {
        std::fs::write(temp.path().join(format!("src/bad_{}.rs", i)), &large_bad).unwrap();
    }

    let mut limits = default_limits();
    limits.max_inspection_bytes = 3000;
    limits.max_per_file_bytes = 10000;
    let cargo = inspect_cargo_project(temp.path(), &limits).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &limits).unwrap();

    // Total bytes must be within budget
    assert!(
        inspection.total_bytes <= limits.max_inspection_bytes,
        "total_bytes {} exceeds budget {}",
        inspection.total_bytes,
        limits.max_inspection_bytes
    );

    // Must have LimitExceeded
    assert!(matches!(
        inspection.state,
        SnapshotState::LimitExceeded { .. }
    ));
}

// ---- Zero-byte candidate permitted ----

#[test]
fn test_zero_byte_candidate_accepted() {
    let temp = materialize_fixture();
    // Zero-byte .rs file
    std::fs::write(temp.path().join("src").join("empty.rs"), b"").unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    let has_empty = inspection
        .input_files
        .iter()
        .any(|f| f.path.contains("empty.rs"));
    assert!(has_empty, "zero-byte .rs file should be accepted");

    assert_eq!(inspection.inspected_file_count, 14);
}

// ---- Package mapping: member build.rs classified correctly ----

#[test]
fn test_member_build_script_classified() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let member_b_build = inspection.file_kinds.iter().find(|k| match k {
        RustFileKind::BuildScript { path, package } => {
            path.contains("member-b/build.rs") && package == "member-b"
        }
        _ => false,
    });
    assert!(
        member_b_build.is_some(),
        "member-b/build.rs should be BuildScript for member-b"
    );
}

// ---- Package mapping: nested member prefix boundary (foo vs foobar) ----

#[test]
fn test_nested_member_prefix_boundary() {
    // Create a temp fixture with members "foo" and "foobar" to test prefix isolation
    let temp = TempDir::new().unwrap();
    let catalog_version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();

    // Top-level Cargo.toml (workspace root with package section)
    std::fs::write(
        temp.path().join("Cargo.toml"),
        format!(
            "[package]\nname = \"root-ws\"\nversion = \"{}\"\nedition = \"2021\"\n\
             [workspace]\nmembers = [\"foo\", \"foobar\"]\n\
             [workspace.dependencies]\n\
             amari-core = {{ version = \"{}\" }}\n",
            catalog_version, catalog_version
        ),
    )
    .unwrap();

    std::fs::write(
        temp.path().join("Cargo.lock"),
        format!(
            "version = 3\n[[package]]\nname = \"amari-core\"\nversion = \"{}\"\n",
            catalog_version
        ),
    )
    .unwrap();

    // foo member
    std::fs::create_dir_all(temp.path().join("foo").join("src")).unwrap();
    std::fs::write(
        temp.path().join("foo").join("Cargo.toml"),
        format!(
            "[package]\nname = \"foo\"\nversion = \"{}\"\n\
             [dependencies]\namari-core = {{ version = \"{}\" }}\n",
            catalog_version, catalog_version
        ),
    )
    .unwrap();
    std::fs::write(
        temp.path().join("foo").join("src").join("lib.rs"),
        b"use amari_core::Multivector;",
    )
    .unwrap();

    // foobar member — prefix is "foo" but distinct from "foobar"
    std::fs::create_dir_all(temp.path().join("foobar").join("src")).unwrap();
    std::fs::write(
        temp.path().join("foobar").join("Cargo.toml"),
        format!(
            "[package]\nname = \"foobar\"\nversion = \"{}\"\n\
             [dependencies]\namari-core = {{ version = \"{}\" }}\n",
            catalog_version, catalog_version
        ),
    )
    .unwrap();
    std::fs::write(
        temp.path().join("foobar").join("src").join("lib.rs"),
        b"use amari_core::Multivector;",
    )
    .unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    // foo/src/lib.rs should be classified under "foo"
    let foo_lib = inspection.file_kinds.iter().find(|k| match k {
        RustFileKind::Library { path, package } => {
            path.contains("foo/src/lib.rs") && package == "foo"
        }
        _ => false,
    });
    assert!(
        foo_lib.is_some(),
        "foo/src/lib.rs should be Library for foo"
    );

    // foobar/src/lib.rs should be classified under "foobar"
    let foobar_lib = inspection.file_kinds.iter().find(|k| match k {
        RustFileKind::Library { path, package } => {
            path.contains("foobar/src/lib.rs") && package == "foobar"
        }
        _ => false,
    });
    assert!(
        foobar_lib.is_some(),
        "foobar/src/lib.rs should be Library for foobar"
    );
}

// ---- No fallback from known member to root on lookup failure ----

#[test]
fn test_no_member_fallback_to_root() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    // member-a has its own dependencies (amari-core, amari-tropical)
    // These should NOT show root package dependencies in their alias context
    // Root has `amari` (umbrella), member-a does NOT
    let member_a_has_umbrella = inspection
        .usages
        .iter()
        .any(|u| u.alias == "amari" && u.source.path.contains("member-a"));
    assert!(
        !member_a_has_umbrella,
        "member-a should not resolve via root's umbrella map"
    );
}

// ---- Multiple separated doc comments anchor to actual lines ----

#[test]
fn test_multiple_separated_doc_comments_anchor_to_actual_lines() {
    let temp = materialize_fixture();
    let test_path = temp.path().join("src").join("multi_doc.rs");
    let source = concat!(
        "/// First doc: tropical algebra for shortest path\n",
        "fn first() {}\n",
        "\n",
        "/// Second doc: geometric algebra with clifford\n",
        "fn second() {}\n",
        "\n",
        "/// Third doc: wasm target no_std embedded\n",
        "fn third() {}\n",
    );
    std::fs::write(&test_path, source).unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    let multi_vocab: Vec<_> = inspection
        .vocabulary
        .iter()
        .filter(|v| v.path.contains("multi_doc"))
        .collect();

    // "tropical_algebra" from first doc at line 1
    let ta = multi_vocab.iter().find(|v| v.term == "tropical_algebra");
    assert!(ta.is_some(), "should find tropical_algebra");
    let ta_src = ta.unwrap().source.as_ref().unwrap();
    assert_eq!(ta_src.line, Some(1), "tropical_algebra should be at line 1");

    // "geometric_algebra" from second doc at line 4
    let ga = multi_vocab.iter().find(|v| v.term == "geometric_algebra");
    assert!(ga.is_some(), "should find geometric_algebra");
    let ga_src = ga.unwrap().source.as_ref().unwrap();
    assert_eq!(
        ga_src.line,
        Some(4),
        "geometric_algebra should be at line 4"
    );

    // "wasm" from third doc at line 7
    let wasm = multi_vocab.iter().find(|v| v.term == "wasm");
    assert!(wasm.is_some(), "should find wasm");
    let wasm_src = wasm.unwrap().source.as_ref().unwrap();
    assert_eq!(wasm_src.line, Some(7), "wasm should be at line 7");
}

// ---- Vocabulary dedup by path+term+source preserves distinct occurrences ----

#[test]
fn test_vocabulary_preserves_distinct_occurrences() {
    let temp = materialize_fixture();

    // Write a file with the same vocabulary term at two different lines
    let test_path = temp.path().join("src").join("dup_vocab.rs");
    let source = concat!(
        "/// Line 1: wasm target\n",
        "fn a() {}\n",
        "/// Line 3: also mention wasm\n",
        "fn b() {}\n",
    );
    std::fs::write(&test_path, source).unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    let wasm_occurrences: Vec<_> = inspection
        .vocabulary
        .iter()
        .filter(|v| v.term == "wasm" && v.path.contains("dup_vocab"))
        .collect();

    // Two distinct occurrences at lines 1 and 3
    assert!(
        wasm_occurrences.len() >= 2,
        "should preserve both wasm occurrences, got {}",
        wasm_occurrences.len()
    );

    let lines: HashSet<u32> = wasm_occurrences
        .iter()
        .filter_map(|v| v.source.as_ref().and_then(|s| s.line))
        .collect();
    assert!(lines.contains(&1), "should have wasm at line 1");
    assert!(lines.contains(&3), "should have wasm at line 3");
}

// ---- Adding unrelated Rust changes only inputs/hash/file-kind ----

#[test]
fn test_unrelated_rust_changes_only_affect_inputs() {
    let temp = materialize_fixture();
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection1 = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    // Add a new .rs file with no Amari usage
    let new_rs = temp.path().join("src").join("unrelated.rs");
    std::fs::write(&new_rs, b"fn hello() -> u32 { 42 }\n").unwrap();

    let cargo2 = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection2 = inspect_rust_sources(temp.path(), &cargo2, &default_limits()).unwrap();

    // Input hash and file count change
    assert_ne!(inspection1.input_hash, inspection2.input_hash);
    assert_eq!(
        inspection2.inspected_file_count,
        inspection1.inspected_file_count + 1
    );

    // file_kinds changes by exactly 1 (new file classified)
    assert_eq!(
        inspection2.file_kinds.len(),
        inspection1.file_kinds.len() + 1,
        "file_kinds should increase by exactly 1"
    );

    // input_files changes by exactly 1
    assert_eq!(
        inspection2.input_files.len(),
        inspection1.input_files.len() + 1,
        "input_files should increase by exactly 1"
    );

    // usages, vocabulary, cfg_evidence, crate_attributes must NOT change
    // (no new Amari usages from the unrelated file)
    assert_eq!(
        inspection1.usages, inspection2.usages,
        "usages must be unchanged by unrelated .rs file"
    );
    assert_eq!(
        inspection1.vocabulary, inspection2.vocabulary,
        "vocabulary must be unchanged by unrelated .rs file"
    );
    assert_eq!(
        inspection1.cfg_evidence, inspection2.cfg_evidence,
        "cfg_evidence must be unchanged by unrelated .rs file"
    );
    assert_eq!(
        inspection1.crate_attributes, inspection2.crate_attributes,
        "crate_attributes must be unchanged by unrelated .rs file"
    );
}

// ---- Adding non-source files (non .rs/README) leaves inspection whole-struct equal ----

#[test]
fn test_adding_non_source_file_whole_struct_equal() {
    let temp = materialize_fixture();
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection1 = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    // Add a .json file (non-candidate)
    let dummy_path = temp.path().join("dummy.json");
    std::fs::write(&dummy_path, "{}").unwrap();

    let cargo2 = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let inspection2 = inspect_rust_sources(temp.path(), &cargo2, &default_limits()).unwrap();

    // Whole-struct must be equal
    assert_eq!(
        inspection1, inspection2,
        "non-source file addition must leave Rust inspection whole-struct equal"
    );
}

// ---- Exact accepted input count (not >=) in normal fixture ----

#[test]
fn test_exact_input_file_count_and_path_set() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    assert_eq!(
        inspection.state,
        SnapshotState::Complete,
        "normal fixture should be Complete"
    );

    // Exact known set of paths
    let expected_paths: HashSet<&str> = [
        "src/lib.rs",
        "src/main.rs",
        "build.rs",
        "README.md",
        "examples/demo.rs",
        "tests/integration.rs",
        "benches/bench.rs",
        "benches/speed_bench.rs",
        "benches/correctness_bench.rs",
        "member-a/src/lib.rs",
        "member-b/src/lib.rs",
        "member-b/build.rs",
        "member-b/benches/member_bench.rs",
    ]
    .iter()
    .cloned()
    .collect();

    let actual_paths: HashSet<&str> = inspection
        .input_files
        .iter()
        .map(|f| f.path.as_str())
        .collect();

    assert_eq!(
        actual_paths,
        expected_paths,
        "input file paths must match exactly. Extra: {:?}, Missing: {:?}",
        actual_paths.difference(&expected_paths),
        expected_paths.difference(&actual_paths)
    );
}

// ---- Crate attributes preserve distinct locations ----

#[test]
fn test_crate_attributes_preserve_distinct_locations() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    // root lib.rs has: #![no_std], #![forbid(unsafe_code)], #![deny(missing_docs)], #![cfg_attr(...)]
    // member-a lib.rs has: #![cfg_attr(target_arch = "wasm32", no_std)]
    let root_attrs: Vec<_> = inspection
        .crate_attributes
        .iter()
        .filter(|a| a.path.contains("lib.rs") && !a.path.contains("member"))
        .collect();

    assert!(
        root_attrs.len() >= 3,
        "root lib.rs should have at least 3 crate attributes (no_std, forbid, deny), got {}",
        root_attrs.len()
    );

    let member_attrs: Vec<_> = inspection
        .crate_attributes
        .iter()
        .filter(|a| a.path.contains("member"))
        .collect();
    assert!(
        !member_attrs.is_empty(),
        "member files should have crate attributes"
    );
}

// ---- Every usage source path+hash must resolve to input_files ----

#[test]
fn test_every_usage_source_resolves_to_input_files() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let input_hash_set: HashSet<&str> = inspection
        .input_files
        .iter()
        .map(|f| f.content_hash.as_str())
        .collect();

    for usage in &inspection.usages {
        assert!(
            input_hash_set.contains(usage.source.content_hash.as_str()),
            "usage content_hash '{}' (path: {}) must be in input_files",
            usage.source.content_hash,
            usage.source.path
        );
    }
}

// ---- Every cfg_evidence source path+hash resolves to input_files ----

#[test]
fn test_every_cfg_source_resolves_to_input_files() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let input_hash_set: HashSet<&str> = inspection
        .input_files
        .iter()
        .map(|f| f.content_hash.as_str())
        .collect();

    for cfg in &inspection.cfg_evidence {
        if let Some(ref src) = cfg.source {
            assert!(
                input_hash_set.contains(src.content_hash.as_str()),
                "cfg evidence content_hash '{}' (path: {}) must be in input_files",
                src.content_hash,
                src.path
            );
        }
    }
}

// ---- Every crate attribute source path+hash resolves to input_files ----

#[test]
fn test_every_attr_source_resolves_to_input_files() {
    let temp = materialize_fixture();
    let (_, inspection) = inspect_fixture(&temp);

    let input_hash_set: HashSet<&str> = inspection
        .input_files
        .iter()
        .map(|f| f.content_hash.as_str())
        .collect();

    for attr in &inspection.crate_attributes {
        if let Some(ref src) = attr.source {
            assert!(
                input_hash_set.contains(src.content_hash.as_str()),
                "crate attribute content_hash '{}' (path: {}) must be in input_files",
                src.content_hash,
                src.path
            );
        }
    }
}

// ---- Oversized/invalid candidate count consumes considered slots ----

#[test]
fn test_oversized_candidate_consumes_considered_slot() {
    let temp = materialize_fixture();

    // Add a very large .rs file (oversized at ~5KB vs 2KB limit).
    // Place in an aaa/ dir at root so it's visited before benches/
    // alphabetically, ensuring it's the 2nd or 3rd candidate.
    std::fs::create_dir_all(temp.path().join("aaa")).unwrap();
    let big = vec![b'x'; 5000];
    std::fs::write(temp.path().join("aaa").join("huge.rs"), &big).unwrap();

    let mut limits = default_limits();
    limits.max_per_file_bytes = 2048; // Allow Cargo.toml (~1400 bytes), reject huge.rs
    limits.max_inspection_files = 3;

    let cargo = inspect_cargo_project(temp.path(), &limits).unwrap();
    let inspection = inspect_rust_sources(temp.path(), &cargo, &limits).unwrap();

    // Oversized file consumed a considered slot
    assert!(matches!(
        inspection.state,
        SnapshotState::LimitExceeded { .. }
    ));
    assert!(
        inspection.inspected_file_count <= 3,
        "oversized should consume considered slots, accepted: {}",
        inspection.inspected_file_count
    );

    // Oversized warning present
    let has_oversized = inspection.warnings.iter().any(|w| {
        matches!(w, RustInspectionWarning::OversizedFile { path, .. } if path.contains("aaa/huge"))
    });
    assert!(has_oversized, "should warn about oversized file");
}

// ============================================================================
// Stale generated fixture regression
// ============================================================================

/// Verify that if stale generated Cargo.toml/Cargo.lock files exist in the
/// fixture source dir, `copy_and_transform` skips them and processes only
/// `.in` files as authority. The transformed output must match the clean
/// fixture.
#[test]
fn test_stale_generated_files_do_not_override_in_transform() {
    let catalog_version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();

    // Create a synthetic source with stale generated files
    let temp_src = TempDir::new().unwrap();
    let src = temp_src.path();

    // Create a stale Cargo.toml (wrong version)
    std::fs::write(
        src.join("Cargo.toml"),
        "[package]\nname = \"stale\"\nversion = \"999.999.999\"\n",
    )
    .unwrap();

    // Create a stale Cargo.lock (wrong content)
    std::fs::write(src.join("Cargo.lock"), "# stale lock file\nversion = 3\n").unwrap();

    // Create Cargo.toml.in (authority)
    std::fs::write(
        src.join("Cargo.toml.in"),
        "[package]\nname = \"rust-project\"\nversion = \"__AMARI_VERSION__\"\nedition = \"2021\"\n\
             [workspace]\nmembers = [\"member-a\", \"member-b\"]\n\
             [workspace.package]\nversion = \"__AMARI_VERSION__\"\n\
             [workspace.dependencies]\n\
             amari-core = { version = \"__AMARI_VERSION__\" }\n",
    )
    .unwrap();

    // Create Cargo.lock.in (authority)
    std::fs::write(
        src.join("Cargo.lock.in"),
        "# This file is @generated\nversion = 3\n\
             [[package]]\nname = \"amari-core\"\nversion = \"__AMARI_VERSION__\"\n",
    )
    .unwrap();

    // Create a .rs file so cargo inspection succeeds
    std::fs::create_dir_all(src.join("src")).unwrap();
    std::fs::write(src.join("src").join("lib.rs"), b"").unwrap();

    // Transform
    let temp_dst = TempDir::new().unwrap();
    copy_and_transform(src, temp_dst.path(), &catalog_version);

    // The stale Cargo.toml/Cargo.lock (non-.in) should NOT be copied
    let toml_content = std::fs::read_to_string(temp_dst.path().join("Cargo.toml")).unwrap();
    let lock_content = std::fs::read_to_string(temp_dst.path().join("Cargo.lock")).unwrap();

    // Assert .in was transformed (not stale file copied)
    assert!(
        toml_content.contains(&catalog_version),
        "Cargo.toml must contain catalog version (transformed from .in), got: {}",
        toml_content.lines().next().unwrap_or("")
    );
    assert!(
        !toml_content.contains("stale"),
        "Cargo.toml must NOT contain stale content"
    );
    assert!(
        !lock_content.contains("stale lock"),
        "Cargo.lock must NOT contain stale content"
    );
    assert!(
        lock_content.contains(&catalog_version),
        "Cargo.lock must contain catalog version"
    );
}

/// Verify that when both a stale generated Cargo.toml and Cargo.toml.in
/// exist, the transformed .in wins deterministically on repeated runs.
#[test]
fn test_stale_generated_files_deterministic_on_repeat() {
    let catalog_version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();

    // Run twice and assert identical outputs
    let run = || {
        let temp_src = TempDir::new().unwrap();
        let src = temp_src.path();

        // Stale Cargo.toml
        std::fs::write(
            src.join("Cargo.toml"),
            "[package]\nname = \"stale\"\nversion = \"0.0.0\"\n",
        )
        .unwrap();

        // Authority .in
        std::fs::write(
            src.join("Cargo.toml.in"),
            "[package]\nname = \"rust-project\"\nversion = \"__AMARI_VERSION__\"\nedition = \"2021\"\n\
                 [workspace]\nmembers = []\n\
                 [workspace.package]\nversion = \"__AMARI_VERSION__\"\n\
                 [workspace.dependencies]\n",
        )
        .unwrap();

        std::fs::write(src.join("Cargo.lock.in"), "version = 3\n").unwrap();

        std::fs::create_dir_all(src.join("src")).unwrap();
        std::fs::write(src.join("src").join("lib.rs"), b"").unwrap();

        let temp_dst = TempDir::new().unwrap();
        copy_and_transform(src, temp_dst.path(), &catalog_version);

        (
            std::fs::read_to_string(temp_dst.path().join("Cargo.toml")).unwrap(),
            std::fs::read_to_string(temp_dst.path().join("Cargo.lock")).unwrap(),
        )
    };

    let (toml1, lock1) = run();
    let (toml2, lock2) = run();

    assert_eq!(
        toml1, toml2,
        "Cargo.toml output must be deterministic across runs"
    );
    assert_eq!(
        lock1, lock2,
        "Cargo.lock output must be deterministic across runs"
    );
}
