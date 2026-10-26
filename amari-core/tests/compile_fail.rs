// SPDX-License-Identifier: MIT OR Apache-2.0
//! C2 static-rejection harness (rustc-direct — operator decision D1,
//! contract #350). Compiles each `tests/compile-fail/*.rs` case with the
//! same pinned rustc that built this test, asserting:
//!   * the compilation FAILS (nonzero exit),
//!   * every pipe-separated TOKEN in the case header appears in stderr
//!     (fail-closed: missing token = test failure),
//!   * the positive control compiles (exit 0).
//!
//! No trybuild, no snapshots — the boundary itself is the contract.

use std::path::PathBuf;
use std::process::Command;

fn cargo_target_deps() -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("target")
        });
    // Cargo's layout depends on invocation (R1 P2): an explicit
    // --target places artifacts under target/<triple>/<profile>/deps;
    // without it, target/<profile>/deps. Probe both layouts and both
    // common profiles; the first containing an amari_core rlib wins.
    let candidates = [
        target.join("debug").join("deps"),
        target.join("release").join("deps"),
    ];
    let mut tried: Vec<PathBuf> = candidates.to_vec();
    if let Ok(rd) = std::fs::read_dir(&target) {
        let subs: Vec<PathBuf> = rd
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.contains('-') && n != "debug" && n != "release")
            })
            .flat_map(|p| [p.join("debug").join("deps"), p.join("release").join("deps")])
            .collect();
        tried.extend(subs);
    }
    for deps in &tried {
        let has_rlib = std::fs::read_dir(deps).is_ok_and(|rd| {
            rd.filter_map(|e| e.ok()).any(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("libamari_core-") && n.ends_with(".rlib"))
            })
        });
        if has_rlib {
            return deps.clone();
        }
    }
    panic!(
        "no libamari_core rlib under any probed layout of {target:?} (tried {tried:?}) — build first"
    );
}

fn amari_core_rlib(deps: &PathBuf) -> PathBuf {
    // Newest libamari_core-*.rlib (single build of this workspace crate).
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in std::fs::read_dir(deps).expect("read deps") {
        let p = entry.expect("entry").path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with("libamari_core-") && name.ends_with(".rlib") {
            let mtime = std::fs::metadata(&p).and_then(|m| m.modified()).ok();
            if best
                .as_ref()
                .is_none_or(|(t, _)| mtime.is_some_and(|m| m > *t))
            {
                if let Some(m) = mtime {
                    best = Some((m, p));
                }
            }
        }
    }
    best.map(|(_, p)| p)
        .unwrap_or_else(|| panic!("libamari_core-*.rlib not found in {deps:?}"))
}

#[test]
fn static_rejection_cases() {
    let deps = cargo_target_deps();
    let rlib = amari_core_rlib(&deps);
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let cases_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/compile-fail");
    let mut cases = vec![];
    for entry in std::fs::read_dir(&cases_dir).expect("compile-fail dir") {
        let p = entry.expect("entry").path();
        if p.extension().and_then(|e| e.to_str()) == Some("rs") {
            cases.push(p);
        }
    }
    cases.sort();
    assert!(
        cases.len() >= 7,
        "expected >= 7 case files, found {}",
        cases.len()
    );

    for case in &cases {
        let src = std::fs::read_to_string(case).unwrap();
        let is_positive = case
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("positive");
        let tokens: Vec<String> = src
            .lines()
            .find_map(|l| l.strip_prefix("// TOKEN: "))
            .map(|t| t.split('|').map(|s| s.trim().to_string()).collect())
            .unwrap_or_default();

        let out = Command::new(&rustc)
            .arg("--edition=2024")
            .arg("--crate-type=bin")
            .arg("-o")
            .arg(std::env::temp_dir().join("c2-reject-bin"))
            .arg("--extern")
            .arg(format!("amari_core={}", rlib.display()))
            .arg("-L")
            .arg(format!("dependency={}", deps.display()))
            .arg(case)
            .output()
            .expect("run rustc");

        let stderr = String::from_utf8_lossy(&out.stderr);
        if is_positive {
            assert!(
                out.status.success(),
                "POSITIVE CONTROL failed to compile ({}):\n{stderr}",
                case.display()
            );
            continue;
        }
        assert!(
            !out.status.success(),
            "case COMPILED but must be rejected ({}):\n{stderr}",
            case.display()
        );
        for token in &tokens {
            assert!(
                stderr.contains(token.as_str()),
                "case {} failed (exit {:?}) but boundary token {token:?} absent from stderr:\n{stderr}",
                case.display(),
                out.status.code(),
            );
        }
        eprintln!(
            "REJECTED {} [{}] with {} token(s)",
            case.file_name().unwrap().to_string_lossy(),
            out.status.code().unwrap_or(-1),
            tokens.len()
        );
    }
}
