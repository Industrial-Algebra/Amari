// SPDX-License-Identifier: MIT OR Apache-2.0

use std::fs;

use amari_discovery::{inspect_project, InspectionLimit, InspectionLimits, SnapshotState};
use tempfile::TempDir;

#[cfg(unix)]
use std::os::unix::fs::symlink;

fn project(files: &[(&str, &[u8])]) -> TempDir {
    let root = tempfile::tempdir().unwrap();
    for (path, contents) in files {
        let path = root.path().join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }
    root
}

#[test]
fn inspection_limits_reject_unknown_boundary_fields() {
    let value = serde_json::json!({
        "max_inspection_files": 10,
        "max_inspection_bytes": 1024,
        "max_traversal_depth": 4,
        "max_per_file_bytes": 512,
        "max_inspection_wall_millis": 1000,
        "follow_symlinks": true
    });
    let error = serde_json::from_value::<InspectionLimits>(value).unwrap_err();
    assert!(error.to_string().contains("unknown field"));
}

#[cfg(unix)]
#[test]
fn nested_symlink_cycles_and_parent_escapes_are_never_followed() {
    let root = project(&[("Cargo.toml", b"[package]\nname='safe'\n")]);
    let outside = tempfile::tempdir().unwrap();
    fs::write(
        outside.path().join("EXTERNAL_SECRET"),
        b"outside-poison-24a",
    )
    .unwrap();
    fs::create_dir_all(root.path().join("nested/a/b")).unwrap();
    symlink(
        root.path().join("nested/a"),
        root.path().join("nested/a/b/back"),
    )
    .unwrap();
    symlink(outside.path(), root.path().join("parent_escape")).unwrap();

    let snapshot = inspect_project(root.path(), &InspectionLimits::default()).unwrap();
    let encoded = serde_json::to_string(&snapshot).unwrap();
    assert!(!encoded.contains("EXTERNAL_SECRET"));
    assert!(!encoded.contains("outside-poison-24a"));
    assert!(!encoded.contains(outside.path().to_str().unwrap()));
    assert!(snapshot
        .warnings
        .iter()
        .any(|warning| warning.contains("nested/a/b/back")));
    assert!(snapshot
        .warnings
        .iter()
        .any(|warning| warning.contains("parent_escape")));
}

#[cfg(unix)]
#[test]
fn ignored_directories_cannot_be_used_as_escape_tunnels() {
    let root = project(&[("Cargo.toml", b"[package]\nname='safe'\n")]);
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("secret.rs"), b"ignored-escape-poison").unwrap();
    for ignored in [".git", "target", "node_modules", ".worktrees", ".env-cache"] {
        let directory = root.path().join(ignored);
        fs::create_dir_all(&directory).unwrap();
        symlink(outside.path(), directory.join("escape")).unwrap();
    }

    let snapshot = inspect_project(root.path(), &InspectionLimits::default()).unwrap();
    let encoded = serde_json::to_string(&snapshot).unwrap();
    assert!(!encoded.contains("secret.rs"));
    assert!(!encoded.contains("ignored-escape-poison"));
    assert!(!encoded.contains(outside.path().to_str().unwrap()));
    assert_eq!(snapshot.files.len(), 1);
}

#[test]
fn depth_ceiling_preserves_deterministic_root_evidence_as_partial() {
    let root = project(&[
        ("Cargo.toml", b"[package]\nname='bounded'\n"),
        ("src/deeper/lib.rs", b"pub fn hidden() {}"),
    ]);
    let limits = InspectionLimits {
        max_traversal_depth: 1,
        ..InspectionLimits::default()
    };

    let first = inspect_project(root.path(), &limits).unwrap();
    let second = inspect_project(root.path(), &limits).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.files.len(), 1);
    assert_eq!(first.files[0].path, "Cargo.toml");
    assert_eq!(
        first.state,
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::TraversalDepth { max: 1 }
        }
    );
}

#[test]
fn considered_file_ceiling_retains_only_accepted_prefix_as_partial() {
    let root = project(&[
        ("Cargo.toml", b"manifest"),
        ("a.rs", b"alpha"),
        ("b.rs", b"beta"),
    ]);
    let limits = InspectionLimits {
        max_inspection_files: 1,
        ..InspectionLimits::default()
    };

    let snapshot = inspect_project(root.path(), &limits).unwrap();
    assert_eq!(snapshot.file_count, 1);
    assert_eq!(snapshot.files[0].path, "Cargo.toml");
    assert_eq!(
        snapshot.state,
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::FileCount {
                max: 1,
                observed: 2,
            }
        }
    );
}

#[test]
fn aggregate_byte_ceiling_retains_exactly_fitting_evidence_as_partial() {
    let root = project(&[("a.rs", b"12345"), ("b.rs", b"67890")]);
    let limits = InspectionLimits {
        max_inspection_bytes: 5,
        max_per_file_bytes: 5,
        ..InspectionLimits::default()
    };

    let snapshot = inspect_project(root.path(), &limits).unwrap();
    assert_eq!(snapshot.total_bytes, 5);
    assert_eq!(snapshot.files.len(), 1);
    assert_eq!(snapshot.files[0].path, "a.rs");
    assert_eq!(
        snapshot.state,
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::TotalBytes {
                max: 5,
                observed: 5,
            }
        }
    );
}
