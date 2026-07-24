// SPDX-License-Identifier: MIT OR Apache-2.0

//! Safety and correctness tests for the filesystem inspector.
//!
//! Every test exercises the read-only `inspect_project` entry point with
//! temporary projects. Assertions cover deterministic hashing, ignored
//! directory pruning, symlink warnings, size/time/depth-limit behaviour,
//! snapshot privacy, filesystem non-mutation, resource-limits surface,
//! typed limit reporting, and capabilities inspector states.

use std::collections::HashSet;
use std::fs::{self};
#[cfg(unix)]
use std::os::unix::fs::{symlink, PermissionsExt};

use tempfile::TempDir;

use amari_discovery::capabilities::ResourceLimits;
use amari_discovery::inspect::{
    inspect_project, InspectionLimit, InspectionLimits, ProjectSignal, SnapshotState,
};

// ---------------------------------------------------------------------------
// Helper to create a minimal temp project with known files
// ---------------------------------------------------------------------------

fn make_temp_project(files: &[(&str, &[u8])]) -> TempDir {
    let dir = TempDir::new().unwrap();
    for (path, contents) in files {
        let full = dir.path().join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&full, contents).unwrap();
    }
    dir
}

// ===========================================================================
// TEST 1 — deterministic project hash is stable
// ===========================================================================

#[test]
fn deterministic_project_hash_is_stable() {
    let dir = make_temp_project(&[
        ("src/main.rs", b"fn main() {}"),
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
    ]);

    let limits = InspectionLimits::default();
    let snap_a = inspect_project(dir.path(), &limits).unwrap();
    let snap_b = inspect_project(dir.path(), &limits).unwrap();

    assert_eq!(snap_a.project_hash, snap_b.project_hash);
    assert!(!snap_a.project_hash.is_empty());
    assert!(
        matches!(snap_a.state, SnapshotState::Complete),
        "expected Complete state for trivial project, got {:?}",
        snap_a.state
    );
}

// ===========================================================================
// TEST 2 — .git, target, node_modules, .worktrees pruned before descent
// ===========================================================================

#[test]
fn ignores_git_target_node_modules_worktrees() {
    // Files inside ignored directories must not appear and not affect hash.

    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b""),
        (".git/HEAD", b"ref: refs/heads/main"),
        (".git/config", b"[core]\nrepositoryformatversion = 0"),
        ("target/debug/build/output", b"artifact bytes"),
        ("node_modules/pkg/index.js", b"module.exports = {};"),
        (".worktrees/feature-x/src/lib.rs", b"other worktree code"),
    ]);

    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    let paths: HashSet<&str> = snap.files.iter().map(|loc| loc.path.as_str()).collect();

    for p in paths.iter() {
        assert!(
            !p.starts_with(".git/"),
            "snapshot included path from .git: {}",
            p
        );
        assert!(
            !p.starts_with("target/"),
            "snapshot included path from target: {}",
            p
        );
        assert!(
            !p.starts_with("node_modules/"),
            "snapshot included path from node_modules: {}",
            p
        );
        assert!(
            !p.starts_with(".worktrees/"),
            "snapshot included path from .worktrees: {}",
            p
        );
    }

    // Must contain Cargo.toml and src/lib.rs
    assert!(paths.contains("Cargo.toml"), "Cargo.toml missing");
    assert!(paths.contains("src/lib.rs"), "src/lib.rs missing");
}

// ===========================================================================
// TEST 3 — escaping symlinks are not followed (Unix)
// ===========================================================================

#[cfg(unix)]
#[test]
fn escaping_symlinks_not_followed() {
    let dir = TempDir::new().unwrap();
    let outside_dir = TempDir::new().unwrap();
    let outside_file = outside_dir.path().join("secret.txt");
    fs::write(&outside_file, b"this should never be read").unwrap();

    let project_file = dir.path().join("src/lib.rs");
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(&project_file, b"fn lib() {}").unwrap();

    // Symlink that escapes
    let symlink_path = dir.path().join("escape_link");
    symlink(&outside_file, &symlink_path).unwrap();

    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    // The symlink must NOT appear in the file list.
    for loc in &snap.files {
        assert_ne!(
            loc.path, "escape_link",
            "symlink outside root was followed: {}",
            loc.path
        );
    }

    // Should have a symlink warning
    let has_sym_warning = snap
        .warnings
        .iter()
        .any(|w| w.contains("symlink") && w.to_lowercase().contains("escape"));
    assert!(
        has_sym_warning,
        "expected symlink warning, got warnings: {:?}",
        snap.warnings
    );
}

// ===========================================================================
// TEST 4 — oversized files are skipped with warnings
// ===========================================================================

#[test]
fn oversized_files_skipped_with_warnings() {
    let small_bytes = b"small file";
    let large_bytes = &[b'x'; 2048]; // 2 KiB

    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/small.rs", small_bytes),
        ("src/large.rs", large_bytes),
    ]);

    // Set max_per_file_bytes to 1024 so large.rs exceeds it
    let limits = InspectionLimits {
        max_per_file_bytes: 1024,
        ..InspectionLimits::default()
    };

    let snap = inspect_project(dir.path(), &limits).unwrap();

    // large.rs must NOT be in the file list
    let has_large = snap.files.iter().any(|loc| loc.path == "src/large.rs");
    assert!(!has_large, "oversized file src/large.rs should be skipped");

    // At least one warning about the oversized file
    let oversized_warnings: Vec<_> = snap
        .warnings
        .iter()
        .filter(|w| w.contains("large.rs") || w.contains("exceed") || w.contains("max_per_file"))
        .collect();
    assert!(
        !oversized_warnings.is_empty(),
        "expected warning about oversized file, got warnings: {:?}",
        snap.warnings
    );

    // Warnings must not contain source text (the file content)
    for w in &snap.warnings {
        let warning_str = w.to_lowercase();
        assert!(
            !warning_str.contains("xxxxx"),
            "warning leaked file content: {}",
            w
        );
    }
}

// ===========================================================================
// TEST 5 — file-count limit returns partial with typed LimitExceeded
// ===========================================================================

#[test]
fn file_count_limit_returns_partial_with_limit_exceeded() {
    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
        ("src/main.rs", b"fn main() {}"),
        ("src/util.rs", b"fn util() {}"),
    ]);

    let limits = InspectionLimits {
        max_inspection_files: 2,
        ..InspectionLimits::default()
    };

    let snap = inspect_project(dir.path(), &limits).unwrap();

    match &snap.state {
        SnapshotState::LimitExceeded { limit } => match limit {
            InspectionLimit::FileCount { max, observed } => {
                assert_eq!(*max, 2, "max should be 2");
                // observed is considered_files (includes the triggering file)
                assert_eq!(
                    *observed, 3,
                    "observed should be 3 (2 accepted + 1 considered)"
                );
            }
            other => panic!("expected FileCount limit, got {:?}", other),
        },
        other => panic!("expected LimitExceeded, got {:?}", other),
    }

    // At most max_inspection_files should be ACCEPTED in the file list
    assert!(
        snap.files.len() as u64 <= 2,
        "accepted file count {} exceeds limit of 2",
        snap.files.len()
    );

    // Invariant: file_count == files.len()
    assert_eq!(snap.file_count as usize, snap.files.len());

    // Determinism: calling again with same limits must produce same result
    let snap2 = inspect_project(dir.path(), &limits).unwrap();
    assert_eq!(snap.project_hash, snap2.project_hash);
    assert_eq!(snap.files.len(), snap2.files.len());
    assert_eq!(snap.file_count, snap2.file_count);
}

// ===========================================================================
// TEST 6 — wall-clock limit returns partial with typed WallClock limit
// ===========================================================================

#[test]
fn wall_time_zero_deadline_returns_partial_with_limit_exceeded() {
    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
    ]);

    let limits = InspectionLimits {
        max_inspection_wall_millis: 0,
        ..InspectionLimits::default()
    };

    let snap = inspect_project(dir.path(), &limits).unwrap();

    match &snap.state {
        SnapshotState::LimitExceeded { limit } => {
            assert!(
                matches!(limit, InspectionLimit::WallClock { .. }),
                "expected WallClock limit, got {:?}",
                limit
            );
        }
        SnapshotState::Complete => {
            // On extremely fast systems the zero-deadline may not trigger.
            // Snapshot must still be deterministic.
            let snap2 = inspect_project(dir.path(), &limits).unwrap();
            assert_eq!(snap.project_hash, snap2.project_hash);
        }
    }

    // Invariant: file_count == files.len()
    assert_eq!(snap.file_count as usize, snap.files.len());
}

// ===========================================================================
// TEST 7 — snapshot never contains full source text or secrets
// ===========================================================================

#[test]
fn snapshot_never_contains_full_source_text_or_secrets() {
    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() { let secret = \"abc123\"; }"),
        (".env", b"API_KEY=supersecretvalue"),
    ]);

    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    // No .env file should appear
    for loc in &snap.files {
        assert_ne!(loc.path, ".env", ".env file leaked into snapshot");
        assert!(
            !loc.path.starts_with(".env."),
            "env file leaked into snapshot: {}",
            loc.path
        );
    }

    // SourceLocation must have content_hash, not content
    let lib_loc = snap.files.iter().find(|loc| loc.path == "src/lib.rs");
    assert!(
        lib_loc.is_some(),
        "src/lib.rs should be in snapshot for a Cargo project"
    );
    if let Some(loc) = lib_loc {
        assert!(
            !loc.content_hash.is_empty(),
            "content_hash must be present for src/lib.rs"
        );
    }

    // No warnings should contain secret values
    for w in &snap.warnings {
        assert!(
            !w.contains("supersecretvalue"),
            "warning leaked secret: {}",
            w
        );
        assert!(!w.contains("API_KEY"), "warning leaked env var name: {}", w);
    }
}

// ===========================================================================
// TEST 8 — target content, permissions, size, mtime unchanged (Unix)
// ===========================================================================

#[cfg(unix)]
#[test]
fn target_content_permissions_size_mtime_unchanged() {
    let content = b"fn unchanged() -> u32 { 42 }";
    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", content),
    ]);

    let lib_path = dir.path().join("src/lib.rs");

    // Capture pre-inspection metadata
    let pre_meta = fs::metadata(&lib_path).unwrap();
    let pre_size = pre_meta.len();
    let pre_mode = pre_meta.permissions().mode();
    let pre_mtime = pre_meta.modified().unwrap();
    let pre_content = fs::read(&lib_path).unwrap();

    let limits = InspectionLimits::default();
    inspect_project(dir.path(), &limits).unwrap();

    // Verify post-inspection metadata
    let post_meta = fs::metadata(&lib_path).unwrap();
    assert_eq!(post_meta.len(), pre_size, "file size changed");
    assert_eq!(
        post_meta.permissions().mode(),
        pre_mode,
        "permissions changed"
    );
    assert_eq!(post_meta.modified().unwrap(), pre_mtime, "mtime changed");

    let post_content = fs::read(&lib_path).unwrap();
    assert_eq!(post_content, pre_content, "file content changed");
}

// ===========================================================================
// TEST 9 — ResourceLimits and InspectionLimits alignment
// ===========================================================================

#[test]
fn resource_limits_supports_equality() {
    let a = ResourceLimits::default();
    let b = ResourceLimits::default();
    assert_eq!(a, b);

    let custom = ResourceLimits {
        max_inspection_files: 42,
        ..ResourceLimits::default()
    };
    assert_ne!(a, custom);
}

#[test]
fn inspection_limits_derive_defaults_from_resource_limits() {
    let rl = ResourceLimits::default();
    let limits = InspectionLimits::default();

    // Every inspection field must match the corresponding ResourceLimits default.
    assert_eq!(limits.max_inspection_files, rl.max_inspection_files);
    assert_eq!(limits.max_inspection_bytes, rl.max_inspection_bytes);
    assert_eq!(limits.max_traversal_depth, rl.max_traversal_depth);
    assert_eq!(limits.max_per_file_bytes, rl.max_per_file_bytes);
    assert_eq!(
        limits.max_inspection_wall_millis,
        rl.max_inspection_wall_millis
    );

    // ResourceLimits probe fields are NOT part of InspectionLimits —
    // verify they still have defaults (ResourceLimits is the capability
    // authority; InspectionLimits only inherits inspection fields).
    assert!(rl.max_probe_input_bytes > 0);
    assert!(rl.max_probe_output_bytes > 0);
    assert!(rl.probe_timeout_millis > 0);

    // Custom InspectionLimits values with explicit overrides
    let custom = InspectionLimits {
        max_per_file_bytes: 512,
        max_inspection_wall_millis: 10_000,
        ..InspectionLimits::default()
    };
    assert_eq!(custom.max_per_file_bytes, 512);
    assert_eq!(custom.max_inspection_wall_millis, 10_000);
    // Non-overridden fields still match ResourceLimits defaults
    assert_eq!(custom.max_inspection_files, rl.max_inspection_files);
}

// ===========================================================================
// TEST 10 — capabilities reports accurate project inspector states
// ===========================================================================

#[test]
fn capabilities_reports_generic_traversal_inspector() {
    use assert_cmd::Command;
    use serde_json::Value;

    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(["capabilities", "--json"])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: Value = serde_json::from_slice(&output).unwrap();

    let inspectors = value["data"]["project_inspectors"].as_array().unwrap();

    // The generic filesystem traversal inspector must be present and known
    let traversal = inspectors.iter().find(|i| i["id"] == "generic-filesystem");
    assert!(
        traversal.is_some(),
        "capabilities must include the 'generic-filesystem' inspector"
    );
    let traversal = traversal.unwrap();
    assert_eq!(traversal["known"], true, "generic-filesystem must be known");

    // Both language-specific inspectors are executable after Task 9C.
    let rust = inspectors.iter().find(|i| i["id"] == "rust-cargo").unwrap();
    assert_eq!(rust["known"], true, "rust-cargo must be known");
    assert_eq!(
        rust["available"], true,
        "rust-cargo must report availability"
    );
    assert_eq!(
        rust["executable"], true,
        "rust-cargo must report executable implementation"
    );

    let npm = inspectors
        .iter()
        .find(|i| i["id"] == "npm-typescript")
        .unwrap();
    assert_eq!(npm["known"], true, "npm-typescript must be known");
    assert_eq!(
        npm["available"], true,
        "npm-typescript must report availability"
    );
    assert_eq!(
        npm["executable"], true,
        "npm-typescript must report executable implementation"
    );

    // Resource limits must include all five inspection fields
    let rl = &value["data"]["resource_limits"];
    assert!(
        rl["max_inspection_files"].as_u64().is_some(),
        "max_inspection_files must be in capabilities"
    );
    assert!(
        rl["max_inspection_bytes"].as_u64().is_some(),
        "max_inspection_bytes must be in capabilities"
    );
    assert!(
        rl["max_traversal_depth"].as_u64().is_some(),
        "max_traversal_depth must be in capabilities"
    );
    assert!(
        rl["max_per_file_bytes"].as_u64().is_some(),
        "max_per_file_bytes must be in capabilities"
    );
    assert!(
        rl["max_inspection_wall_millis"].as_u64().is_some(),
        "max_inspection_wall_millis must be in capabilities"
    );
}

// ===========================================================================
// TEST 11 — canonical root containment
// ===========================================================================

#[test]
fn canonical_root_prevents_directory_traversal_attacks() {
    let dir = TempDir::new().unwrap();
    let outside_dir = TempDir::new().unwrap();
    let outside_file = outside_dir.path().join("exfil.txt");
    fs::write(&outside_file, b"do not read").unwrap();

    // Create a legitimate file inside root
    let legit_file = dir.path().join("src/lib.rs");
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(&legit_file, b"fn lib() {}").unwrap();

    // Create a symlink inside root pointing outside
    let symlink_path = dir.path().join("tricky");
    #[cfg(unix)]
    symlink(&outside_file, &symlink_path).unwrap();

    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    // No file should have a path that resolves outside the root
    for loc in &snap.files {
        let full = dir.path().join(&loc.path);
        let canon = full.canonicalize();
        if let Ok(canon_path) = canon {
            let root_canon = dir.path().canonicalize().unwrap();
            assert!(
                canon_path.starts_with(&root_canon),
                "path {} escapes root: {}",
                loc.path,
                canon_path.display()
            );
        }
    }
}

// ===========================================================================
// TEST 12 — hash is independent of filesystem metadata
// ===========================================================================

#[test]
fn hash_independent_of_filesystem_metadata() {
    // Two identical directory trees at different paths must produce
    // the same project hash, regardless of inodes, timestamps, etc.

    let dir_a = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
        ("src/main.rs", b"fn main() {}"),
    ]);

    let dir_b = make_temp_project(&[
        ("src/main.rs", b"fn main() {}"), // different creation order
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
    ]);

    let limits = InspectionLimits::default();
    let snap_a = inspect_project(dir_a.path(), &limits).unwrap();
    let snap_b = inspect_project(dir_b.path(), &limits).unwrap();

    // Same content, different temp dirs — hash must match
    assert_eq!(snap_a.project_hash, snap_b.project_hash);
}

// ===========================================================================
// TEST 13 — environment secret files and directories excluded
// ===========================================================================

#[test]
fn environment_secret_files_and_dirs_are_excluded() {
    // .env files and .env* directories must be excluded.

    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
        (".env", b"SECRET=shh"),
        (".env.production", b"SECRET=prod"),
    ]);

    // Also test that .env* directories are pruned before descent
    let dir2 = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/main.rs", b"fn main() {}"),
        (".env/secrets.txt", b"NESTED_SECRET=deep"),
        (".env.prod/nested.txt", b"nested secret"),
    ]);

    let limits = InspectionLimits::default();

    // dir: .env and .env.production are files, should be excluded
    let snap = inspect_project(dir.path(), &limits).unwrap();
    for loc in &snap.files {
        assert!(
            !loc.path.starts_with(".env"),
            "env file leaked into snapshot: {}",
            loc.path
        );
    }

    // dir2: .env/ and .env.prod/ are directories, should be pruned before descent
    let snap2 = inspect_project(dir2.path(), &limits).unwrap();
    for loc in &snap2.files {
        assert!(
            !loc.path.contains("/.env"),
            "nested env file leaked into snapshot: {}",
            loc.path
        );
        assert!(
            !loc.path.starts_with(".env/"),
            "env directory contents leaked into snapshot: {}",
            loc.path
        );
    }
}

// ===========================================================================
// TEST 14 — limits can be serialized round-trip
// ===========================================================================

#[test]
fn inspection_limits_serializes_and_deserializes() {
    let limits = InspectionLimits::default();
    let json = serde_json::to_string(&limits).unwrap();
    let parsed: InspectionLimits = serde_json::from_str(&json).unwrap();
    assert_eq!(limits, parsed);

    // Typed limit serialization
    let limit = InspectionLimit::FileCount {
        max: 10,
        observed: 5,
    };
    let json = serde_json::to_string(&limit).unwrap();
    let parsed: InspectionLimit = serde_json::from_str(&json).unwrap();
    assert_eq!(limit, parsed);
}

// ===========================================================================
// TEST 15 — traversal ordering is deterministic before limits
// ===========================================================================

#[test]
fn traversal_order_deterministic_before_limits() {
    let file_data: Vec<(String, Vec<u8>)> = (0..20)
        .map(|i| {
            let path = format!("src/file_{:03}.rs", i);
            let content = format!("fn file_{}() {{}}", i);
            (path, content.into_bytes())
        })
        .collect();
    let file_refs: Vec<(&str, &[u8])> = file_data
        .iter()
        .map(|(p, c)| (p.as_str(), c.as_slice()))
        .collect();
    let dir = make_temp_project(&file_refs);

    // Set a low file limit
    let limits = InspectionLimits {
        max_inspection_files: 5,
        ..InspectionLimits::default()
    };

    let snap = inspect_project(dir.path(), &limits).unwrap();
    assert!(matches!(snap.state, SnapshotState::LimitExceeded { .. }));
    assert_eq!(snap.files.len(), 5);

    // The included files must be the first in sorted order
    let included: Vec<&str> = snap.files.iter().map(|loc| loc.path.as_str()).collect();
    let mut expected = included.clone();
    expected.sort();
    assert_eq!(included, expected, "files must be in sorted order");

    // Determinism: a second run must produce the same subset
    let snap2 = inspect_project(dir.path(), &limits).unwrap();
    assert_eq!(snap.project_hash, snap2.project_hash);
    assert_eq!(snap.files.len(), snap2.files.len());
    for (a, b) in snap.files.iter().zip(snap2.files.iter()) {
        assert_eq!(a.path, b.path);
        assert_eq!(a.content_hash, b.content_hash);
    }
}

// ===========================================================================
// TEST 16 — unreadable ignored directory does not crash (Unix)
// ===========================================================================

#[cfg(unix)]
#[test]
fn unreadable_ignored_directory_does_not_crash() {
    // An ignored directory (.git) with 000 permissions must be pruned by
    // filter_entry before walkdir attempts to read its contents.

    let dir = TempDir::new().unwrap();

    // Create a regular file to assert the inspector still works
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/lib.rs"), b"fn lib() {}").unwrap();

    // Create .git with unreachable permissions
    let git_dir = dir.path().join(".git");
    fs::create_dir(&git_dir).unwrap();
    fs::write(git_dir.join("config"), b"[core]\n").unwrap();
    fs::set_permissions(&git_dir, fs::Permissions::from_mode(0o000)).unwrap();

    // This MUST NOT crash
    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    // Only src/lib.rs should be accepted
    let paths: Vec<&str> = snap.files.iter().map(|l| l.path.as_str()).collect();
    assert_eq!(paths, vec!["src/lib.rs"]);
}

// ===========================================================================
// TEST 17 — root must be a directory
// ===========================================================================

#[test]
fn root_must_be_a_directory() {
    let dir = make_temp_project(&[("Cargo.toml", b"[package]\nname = \"demo\"\n")]);

    // Point to a file, not a directory
    let file_path = dir.path().join("Cargo.toml");
    let limits = InspectionLimits::default();
    let result = inspect_project(&file_path, &limits);

    assert!(
        result.is_err(),
        "inspect_project should fail when root is a file"
    );
    let err = result.unwrap_err();
    let err_msg = format!("{}", err);
    assert!(
        err_msg.contains("not a directory"),
        "error should mention 'not a directory': {}",
        err_msg
    );
}

// ===========================================================================
// TEST 18 — depth limit produces typed TraversalDepth LimitExceeded
// ===========================================================================

#[test]
fn depth_limit_produces_typed_traversal_depth_exceeded() {
    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
        ("deep/a/b/c/deep.rs", b"fn deep() {}"),
    ]);

    // Set max depth to 2 — src/lib.rs (depth 1) is OK; deep/... (depth 3+) pruned
    let limits = InspectionLimits {
        max_traversal_depth: 2,
        ..InspectionLimits::default()
    };

    let snap = inspect_project(dir.path(), &limits).unwrap();

    // src/lib.rs should be present
    let has_lib = snap.files.iter().any(|loc| loc.path == "src/lib.rs");
    assert!(has_lib, "src/lib.rs should be at depth 1");

    // deep/a/b/c/deep.rs should NOT be present (depth > 2 pruned)
    let has_deep = snap.files.iter().any(|loc| loc.path.contains("deep.rs"));
    assert!(!has_deep, "deep file beyond depth limit should be pruned");

    match &snap.state {
        SnapshotState::LimitExceeded { limit } => {
            assert!(
                matches!(limit, InspectionLimit::TraversalDepth { max } if *max == 2),
                "expected TraversalDepth {{ max: 2 }}, got {:?}",
                limit
            );
        }
        other => panic!("expected LimitExceeded for depth pruning, got {:?}", other),
    }
}

// ===========================================================================
// TEST 19 — invariant: file_count == files.len()
// ===========================================================================

#[test]
fn invariant_file_count_equals_files_len() {
    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
        ("src/main.rs", b"fn main() {}"),
    ]);

    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    assert_eq!(snap.file_count as usize, snap.files.len());
    assert!(matches!(snap.state, SnapshotState::Complete));
}

#[test]
fn invariant_file_count_equals_files_len_on_partial() {
    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
        ("src/main.rs", b"fn main() {}"),
        ("src/util.rs", b"fn util() {}"),
    ]);

    let limits = InspectionLimits {
        max_inspection_files: 2,
        ..InspectionLimits::default()
    };

    let snap = inspect_project(dir.path(), &limits).unwrap();
    assert!(matches!(snap.state, SnapshotState::LimitExceeded { .. }));
    assert_eq!(snap.file_count as usize, snap.files.len());
    assert!(snap.file_count <= 2);
}

// ===========================================================================
// TEST 20 — invariant: total_bytes matches accepted content lengths
// ===========================================================================

#[test]
fn invariant_total_bytes_matches_accepted_content_lengths() {
    let content_a = b"hello";
    let content_b = b"world!";

    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("a.txt", content_a),
        ("b.txt", content_b),
    ]);

    let cargo_content = b"[package]\nname = \"demo\"\n";

    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    // Complete: all three files accepted (Cargo.toml + a.txt + b.txt)
    let expected_total =
        cargo_content.len() as u64 + content_a.len() as u64 + content_b.len() as u64;
    assert_eq!(snap.total_bytes, expected_total);

    // On partial: only Cargo.toml (first lexically) accepted
    let limits2 = InspectionLimits {
        max_inspection_files: 1,
        ..InspectionLimits::default()
    };
    let snap2 = inspect_project(dir.path(), &limits2).unwrap();
    let expected_partial = cargo_content.len() as u64;
    assert_eq!(snap2.total_bytes, expected_partial);
}

// ===========================================================================
// TEST 21 — wall clock partial has files-based invariant
// ===========================================================================

#[test]
fn invariant_wall_clock_partial_file_count_consistent() {
    // Even on zero-deadline, if any files were accepted, invariants hold.
    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
    ]);

    let limits = InspectionLimits {
        max_inspection_wall_millis: 0,
        ..InspectionLimits::default()
    };

    let snap = inspect_project(dir.path(), &limits).unwrap();

    // file_count == files.len() always
    assert_eq!(snap.file_count as usize, snap.files.len());
}

// ===========================================================================
// TEST 22 — Signals derived only from accepted files
// ===========================================================================

#[test]
fn signals_derived_only_from_accepted_files() {
    // Project with Cargo.toml + 3 .rs files, but limit to 1 file
    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
        ("src/main.rs", b"fn main() {}"),
        ("src/util.rs", b"fn util() {}"),
    ]);

    let limits = InspectionLimits {
        max_inspection_files: 1,
        ..InspectionLimits::default()
    };

    let snap = inspect_project(dir.path(), &limits).unwrap();

    // Cargo.toml should be accepted (first in lexical order), so CargoManifest
    // and 0 RustSource files should be present
    let has_cargo = snap
        .signals
        .iter()
        .any(|s| matches!(s, ProjectSignal::CargoManifest));
    assert!(has_cargo, "Cargo.toml was accepted, CargoManifest expected");

    let rust_signal = snap
        .signals
        .iter()
        .find_map(|s| {
            if let ProjectSignal::RustSource { count } = s {
                Some(*count)
            } else {
                None
            }
        })
        .unwrap_or(0);
    assert_eq!(
        rust_signal, 0,
        "no .rs files accepted, RustSource count should be 0"
    );
}

// ===========================================================================
// TEST 23 — hash independent of absolute root and ignored content
// ===========================================================================

#[test]
fn hash_independent_of_absolute_root_and_ignored_content() {
    // Same project content, different absolute roots, different .git blobs
    let dir_a = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
        (".git/HEAD", b"ref: refs/heads/a"),
    ]);

    let dir_b = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
        (".git/HEAD", b"ref: refs/heads/completely_different_branch"),
    ]);

    let limits = InspectionLimits::default();
    let snap_a = inspect_project(dir_a.path(), &limits).unwrap();
    let snap_b = inspect_project(dir_b.path(), &limits).unwrap();

    // Hash must match — .git content is ignored
    assert_eq!(snap_a.project_hash, snap_b.project_hash);
    assert!(!snap_a.project_hash.is_empty());
}

// ===========================================================================
// TEST 24 — considered_files bounds regular non-secret files examined,
//            not just accepted files. Oversized candidates consume
//            considered slots preventing count-bypass attacks.
// ===========================================================================

#[test]
fn considered_files_bounds_examined_not_just_accepted() {
    // Cargo.toml (accepted), src/big.rs (oversized 2 KiB), src/lib.rs (accepted)
    // max_inspection_files = 2
    // With old accepted-count semantics: 2 accepted, skip oversized
    // With new considered semantics: oversized file consumes a slot
    //   → Cargo.toml accepted (considered=1)
    //   → src/big.rs considered but skipped (considered=2)
    //   → src/lib.rs considered (considered=3 > 2) → stop, only 1 accepted

    let small = b"small";
    let big = &[b'x'; 2048];

    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/big.rs", big),
        ("src/lib.rs", small),
    ]);

    let limits = InspectionLimits {
        max_inspection_files: 2,
        max_per_file_bytes: 1024, // big.rs at 2 KiB is oversized
        ..InspectionLimits::default()
    };

    let snap = inspect_project(dir.path(), &limits).unwrap();

    // The oversized file consumed a considered slot, so only 1 file accepted
    assert_eq!(
        snap.files.len(),
        1,
        "oversized file should consume considered slot, only 1 accepted"
    );
    assert_eq!(snap.file_count as usize, snap.files.len());

    // The one accepted file should be Cargo.toml (lexically first)
    assert_eq!(snap.files[0].path, "Cargo.toml");

    // State must be LimitExceeded with FileCount
    match &snap.state {
        SnapshotState::LimitExceeded { limit } => {
            assert!(
                matches!(limit, InspectionLimit::FileCount { observed, .. } if *observed == 3),
                "expected FileCount with observed=3 (all three considered), got {:?}",
                limit
            );
        }
        other => panic!("expected LimitExceeded, got {:?}", other),
    }

    // Determinism: a second run with the same setup must produce
    // identical output (same partial snapshot).
    let snap2 = inspect_project(dir.path(), &limits).unwrap();
    assert_eq!(snap.project_hash, snap2.project_hash);
    assert_eq!(snap.files.len(), snap2.files.len());
}

// ===========================================================================
// TEST 25 — reject non-UTF8 relative path components (Unix)
// ===========================================================================

#[cfg(unix)]
#[test]
fn non_utf8_path_components_rejected_no_collision() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let dir = TempDir::new().unwrap();

    // Two distinct invalid UTF-8 byte sequences that to_string_lossy
    // would both collapse to the same replacement-character string,
    // potentially causing a hash collision. Our handler must reject
    // both with a path-unavailable warning, never accept either.
    let invalid_bytes1: Vec<u8> = vec![0x66, 0x6f, 0x6f, 0xFF]; // "foo" + 0xFF
    let invalid_bytes2: Vec<u8> = vec![0x62, 0x61, 0x72, 0xFE]; // "bar" + 0xFE

    let name1 = OsString::from_vec(invalid_bytes1);
    let name2 = OsString::from_vec(invalid_bytes2);

    // Verify the names are distinct on the OS level
    assert_ne!(name1, name2, "invalid names must be distinct OS strings");
    // Verify to_string_lossy produces distinct output (not a collision)
    // but both contain replacement chars.
    let lossy1 = name1.to_string_lossy().into_owned();
    let lossy2 = name2.to_string_lossy().into_owned();
    assert!(
        lossy1.contains('\u{FFFD}'),
        "lossy1 should contain replacement char"
    );
    assert!(
        lossy2.contains('\u{FFFD}'),
        "lossy2 should contain replacement char"
    );
    assert_ne!(
        lossy1, lossy2,
        "lossy strings must differ (different valid prefixes)"
    );

    let file1 = dir.path().join(&name1);
    let file2 = dir.path().join(&name2);
    fs::write(&file1, b"content1").unwrap();
    fs::write(&file2, b"content2").unwrap();

    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    // Neither path should appear in the file list
    for loc in &snap.files {
        assert!(
            !loc.path.contains("foo"),
            "non-UTF8 path should not appear in files: {}",
            loc.path
        );
        assert!(
            !loc.path.contains("bar"),
            "non-UTF8 path should not appear in files: {}",
            loc.path
        );
    }
    assert!(
        snap.files.is_empty(),
        "no files should be accepted from non-UTF8 project"
    );

    // Warnings should mention non-UTF-8 path issues (not the raw bytes)
    let has_path_warning = snap
        .warnings
        .iter()
        .any(|w| w.contains("non-UTF-8") || w.contains("non-UTF8"));
    assert!(
        has_path_warning,
        "expected path-unavailable warning for non-UTF-8 files, got: {:?}",
        snap.warnings
    );

    // Determinism: hash should be consistent (empty project hash)
    let snap2 = inspect_project(dir.path(), &limits).unwrap();
    assert_eq!(snap.project_hash, snap2.project_hash);
}

// ===========================================================================
// TEST 26 — snapshot warnings must not leak absolute root paths
// ===========================================================================

#[test]
fn snapshot_warnings_never_contain_absolute_root_path() {
    // Create a project with elements that trigger various warning paths
    // (symlink, oversized file). Assert that NO warning string contains
    // the canonical absolute path of the temp directory root.

    let dir = TempDir::new().unwrap();
    fs::write(
        dir.path().join("Cargo.toml"),
        b"[package]\nname = \"demo\"\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/lib.rs"), b"fn lib() {}").unwrap();

    // Oversized file (triggers per-file-exceeded warning with rel_path)
    let big_content = vec![b'x'; 2048];
    fs::write(dir.path().join("src/big.rs"), &big_content).unwrap();

    // Symlink (triggers symlink-not-followed warning with normalized path)
    let outside_target = TempDir::new().unwrap();
    let outside_file = outside_target.path().join("outside.txt");
    fs::write(&outside_file, b"outside").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let symlink_path = dir.path().join("link_to_outside");
        symlink(&outside_file, &symlink_path).unwrap();
    }

    let root_canon = dir.path().canonicalize().unwrap();
    let root_str = root_canon.to_string_lossy();

    let limits = InspectionLimits {
        max_per_file_bytes: 1024,
        ..InspectionLimits::default()
    };

    let snap = inspect_project(dir.path(), &limits).unwrap();

    // Serialize to JSON and check both structured warnings and the
    // serialized form for the absolute root path.
    let json = serde_json::to_string_pretty(&snap).unwrap();
    assert!(
        !json.contains(root_str.as_ref()),
        "serialized snapshot must not contain absolute root path.\nRoot: {root_str}\nJSON: {json}"
    );

    // Also check each individual warning
    for w in &snap.warnings {
        assert!(
            !w.contains(root_str.as_ref()),
            "warning must not contain absolute root path.\nWarning: {w}\nRoot: {root_str}"
        );
    }

    // There should be at least one warning (the oversized file)
    assert!(
        !snap.warnings.is_empty(),
        "test requires at least one warning to be meaningful"
    );
}

// ===========================================================================
// TEST 27 — ProjectSnapshot.files is explicitly sorted by normalized
//            path in finalization (not reliant on walker behaviour).
//            Signals remain deterministic.
// ===========================================================================

#[test]
fn files_sorted_by_normalized_path_in_finalization() {
    // Create files in lexically reverse order and assert output is sorted.
    let dir = make_temp_project(&[
        ("z.rs", b"fn z() {}"),
        ("m.rs", b"fn m() {}"),
        ("a.rs", b"fn a() {}"),
        ("src/x.rs", b"fn x() {}"),
        ("src/b.rs", b"fn b() {}"),
    ]);

    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    assert!(matches!(snap.state, SnapshotState::Complete));
    assert_eq!(snap.files.len(), 5);

    // Files must be sorted by normalized path
    let paths: Vec<&str> = snap.files.iter().map(|loc| loc.path.as_str()).collect();
    let mut sorted = paths.clone();
    sorted.sort();
    assert_eq!(
        paths, sorted,
        "files must be sorted by normalized path, got: {:?}",
        paths
    );

    // Determinism: stable hash across multiple runs
    let snap2 = inspect_project(dir.path(), &limits).unwrap();
    assert_eq!(snap.project_hash, snap2.project_hash);
    assert_eq!(snap.files, snap2.files);
    assert_eq!(snap.file_count, snap2.file_count);

    // Signals must be deterministic too
    assert_eq!(snap.signals, snap2.signals);
}

// ===========================================================================
// TASK 7 — Change 1: environment secret pattern uses starts_with(".env")
// ===========================================================================

#[test]
fn envrc_and_env_backup_files_are_excluded() {
    // .envrc and .env_backup files (not only .env / .env.production)
    // must be excluded from snapshots.

    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/lib.rs", b"fn lib() {}"),
        (".envrc", b"DIRENV_CONFIG=secret"),
        (".env_backup", b"OLD_SECRET=archived"),
    ]);

    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    for loc in &snap.files {
        assert!(
            !loc.path.starts_with(".env"),
            "env variant file leaked into snapshot: {}",
            loc.path
        );
    }

    // Only Cargo.toml and src/lib.rs should be present
    assert_eq!(snap.files.len(), 2);
}

#[test]
fn envrc_and_env_backup_directories_are_pruned() {
    // Directories named .envrc/ and .env_backup/ must be pruned before
    // descent, preventing any nested content from leaking.

    let dir = make_temp_project(&[
        ("Cargo.toml", b"[package]\nname = \"demo\"\n"),
        ("src/main.rs", b"fn main() {}"),
        (".envrc/secret.txt", b"DIRENV_SECRET=hush"),
        (".env_backup/nested/key.txt", b"ARCHIVED=hush"),
    ]);

    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    for loc in &snap.files {
        assert!(
            !loc.path.contains("/.env"),
            "nested env directory file leaked into snapshot: {}",
            loc.path
        );
        assert!(
            !loc.path.starts_with(".env"),
            "env directory content leaked into snapshot: {}",
            loc.path
        );
    }

    assert_eq!(snap.files.len(), 2);
}

// ===========================================================================
// TASK 7 — Change 3: non-UTF8 directories pruned + counting before
//            UTF-8 normalization + low-file-limit RED test
// ===========================================================================

#[cfg(unix)]
#[test]
fn non_utf8_files_consume_considered_slots_with_low_limit() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let dir = TempDir::new().unwrap();

    // One valid accepted file
    fs::write(
        dir.path().join("Cargo.toml"),
        b"[package]\nname = \"demo\"\n",
    )
    .unwrap();
    fs::create_dir_all(dir.path().join("src")).unwrap();
    fs::write(dir.path().join("src/lib.rs"), b"fn lib() {}").unwrap();

    // Several non-UTF8 regular candidate files — these are regular,
    // non-secret files, so they MUST consume considered slots even
    // though they will be skipped at UTF-8 normalization.
    // Avoid NUL bytes (0x00) which are invalid in filenames.
    for i in 1u8..6u8 {
        let name = OsString::from_vec(vec![0x66, 0x6f, 0x6f, 0xFF, i]);
        let p = dir.path().join(&name);
        fs::write(&p, b"payload").unwrap();
    }

    // max_inspection_files = 3. If non-UTF8 files consume considered
    // slots, only Cargo.toml gets accepted before the limit.
    let limits = InspectionLimits {
        max_inspection_files: 3,
        ..InspectionLimits::default()
    };

    let snap = inspect_project(dir.path(), &limits).unwrap();

    // Must be partial due to FileCount limit
    match &snap.state {
        SnapshotState::LimitExceeded { limit } => {
            assert!(
                matches!(limit, InspectionLimit::FileCount { .. }),
                "expected FileCount limit, got {:?}",
                limit
            );
        }
        other => panic!("expected LimitExceeded, got {:?}", other),
    }

    // At most 3 files accepted (but many fewer because non-UTF8 consumed slots)
    assert!(snap.files.len() <= 3);
}

#[cfg(unix)]
#[test]
fn non_utf8_directories_are_pruned_before_descent() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let dir = TempDir::new().unwrap();

    // Create a non-UTF8 directory with files inside
    let dir_name = OsString::from_vec(vec![0x64, 0x69, 0x72, 0xFF]);
    let bad_dir = dir.path().join(&dir_name);
    fs::create_dir(&bad_dir).unwrap();
    fs::write(bad_dir.join("nested.txt"), b"secret").unwrap();

    // Also a valid file at root
    fs::write(
        dir.path().join("Cargo.toml"),
        b"[package]\nname = \"demo\"\n",
    )
    .unwrap();

    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    // The nested file must not appear
    for loc in &snap.files {
        assert!(
            !loc.path.contains("nested"),
            "file inside non-UTF8 directory leaked: {}",
            loc.path
        );
    }

    // Only Cargo.toml should be present
    assert_eq!(snap.files.len(), 1);
}

// ===========================================================================
// TASK 7 — Change 5: nofollow errors must not leak external target paths
// ===========================================================================

#[cfg(unix)]
#[test]
fn nofollow_errors_dont_leak_external_target_paths() {
    let dir = TempDir::new().unwrap();
    let outside_dir = TempDir::new().unwrap();
    let outside_path = outside_dir.path().join("exfiltrate_me.txt");
    fs::write(&outside_path, b"sensitive external content").unwrap();
    let outside_str = outside_path.to_string_lossy().to_string();

    // Create a symlink inside root pointing outside
    let link_path = dir.path().join("friendly_link");
    symlink(&outside_path, &link_path).unwrap();

    // Also add a valid file
    fs::write(
        dir.path().join("Cargo.toml"),
        b"[package]\nname = \"demo\"\n",
    )
    .unwrap();

    let limits = InspectionLimits::default();
    let snap = inspect_project(dir.path(), &limits).unwrap();

    // Serialize to JSON and assert no external path appears
    let json = serde_json::to_string_pretty(&snap).unwrap();
    assert!(
        !json.contains(&outside_str),
        "snapshot JSON must not leak external symlink target path.\nExternal: {outside_str}\nJSON: {json}"
    );

    // Each warning must not contain the external path
    for w in &snap.warnings {
        assert!(
            !w.contains(&outside_str),
            "warning must not leak external symlink target.\nWarning: {w}\nExternal: {outside_str}"
        );
    }
}
