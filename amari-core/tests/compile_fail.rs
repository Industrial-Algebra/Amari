// SPDX-License-Identifier: MIT OR Apache-2.0
#![cfg(feature = "phantom-types")]

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

/// R6 P2: owner-private RAII scratch. Mode 0o700 (not the default
/// 0755), removed by Drop — which also runs during unwinding, so
/// induced panics cannot leak the directory.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!(
            "c2-cf-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        // R7 P2: atomic owner-private creation — DirBuilderExt::mode
        // applies 0o700 IN the mkdir syscall; create-then-chmod leaves a
        // 755 exposure window.
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .mode(0o700)
                .create(&p)
                .unwrap_or_else(|e| panic!("create {tag} scratch (0700): {e}"));
        }
        #[cfg(not(unix))]
        {
            std::fs::create_dir(&p).unwrap_or_else(|e| panic!("create {tag} scratch: {e}"));
        }
        ScratchDir(p)
    }
    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn host_deps_dir(deps: &std::path::Path) -> PathBuf {
    // R2 P2-1: proc-macros and build scripts are HOST artifacts even in
    // explicit-target builds. R3: the host path must derive from the
    // SAME target root as the running test binary (<root>/debug/deps),
    // NOT the default worktree target — stale libamari_core rmeta/rlib
    // artifacts from differently-featured builds there poison extern
    // resolution (observed E0463 despite a valid --extern path).
    // Layout: <root>/<triple>/debug/deps (explicit target) or
    // <root>/debug/deps (plain — already the host dir).
    let debug_dir = deps.parent().expect("parent of deps is <profile>");
    let profile = debug_dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if profile == "debug" || profile == "release" {
        let maybe_triple = debug_dir
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());
        match maybe_triple {
            Some(t) if t.contains('-') => {
                // explicit-target layout: <root>/<triple>/<profile>/deps
                let root = debug_dir
                    .parent()
                    .expect("triple dir")
                    .parent()
                    .expect("root");
                root.join("debug").join("deps")
            }
            _ => deps.to_path_buf(), // plain layout: deps IS the host deps dir
        }
    } else {
        deps.to_path_buf()
    }
}

fn cargo_target_deps() -> PathBuf {
    // R3 P2-1: anchor to the RUNNING TEST BINARY's own directory — it
    // sits next to the exact rlib this test was linked against, in
    // whichever layout cargo used (<dir>/debug/deps plain, or
    // <dir>/<triple>/debug/deps under explicit --target; release
    // likewise). Layout probing selected STALE artifacts from earlier
    // differently-featured builds sharing the target dir (E0432).
    let deps = std::env::current_exe()
        .expect("current_exe")
        .parent()
        .expect("deps dir of the test binary")
        .to_path_buf();
    let has_rlib = std::fs::read_dir(&deps).is_ok_and(|rd| {
        rd.filter_map(|e| e.ok()).any(|e| {
            e.file_name()
                .to_str()
                .is_some_and(|n| n.starts_with("libamari_core-") && n.ends_with(".rlib"))
        })
    });
    assert!(
        has_rlib,
        "no libamari_core rlib beside the test binary at {deps:?} — build first"
    );
    deps
}

fn amari_core_rlib(deps: &std::path::Path, host_deps: &std::path::Path) -> PathBuf {
    // R4 P2: select by CAPABILITY, not mtimes (cached alternating builds
    // leave stale-newest rlibs). The rlib we need is exactly the one
    // that provides the feature-complete surface this suite exercises
    // (amari_core::verified requires phantom-types): probe-compile a
    // minimal user with each candidate; the first that compiles wins.
    // Falls back loudly if none does.
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(deps)
        .expect("read deps")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("libamari_core-") && n.ends_with(".rlib"))
        })
        .collect();
    candidates.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
    candidates.reverse(); // newest first as a tiebreak heuristic only
    let scratch = ScratchDir::new("probe");
    let probe = scratch.path().join("probe.rs");
    let probe_meta = scratch.path().join("probe.meta");
    std::fs::write(
        &probe,
        "// capability probe: verified requires phantom-types\nfn main() {\n    let _ = amari_core::verified::VerifiedMultivector::<f64, 3, 0, 0>::scalar;\n}\n",
    )
    .expect("write probe");
    for cand in &candidates {
        let ok = Command::new(&rustc)
            .arg("--edition=2024")
            .arg("--crate-type=bin")
            .arg("--emit=metadata")
            .arg("-o")
            .arg(&probe_meta)
            .arg("--extern")
            .arg(format!("amari_core={}", cand.display()))
            .arg("-L")
            .arg(format!("dependency={}", deps.display()))
            .arg("-L")
            .arg(format!("dependency={}", host_deps.display()))
            .arg(&probe)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return cand.clone(); // scratch removed by Drop
        }
    }
    panic!(
        "no candidate rlib in {deps:?} provides the phantom-types surface ({} candidates probed)",
        candidates.len()
    );
}

#[test]
fn static_rejection_cases() {
    let deps = cargo_target_deps();
    let host_deps = host_deps_dir(&deps);
    let rlib = amari_core_rlib(&deps, &host_deps);
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let cases_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/compile-fail");
    let out_dir = ScratchDir::new("out");
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
            .arg(out_dir.path().join("reject-bin"))
            .arg("--extern")
            .arg(format!("amari_core={}", rlib.display()))
            .arg("-L")
            .arg(format!("dependency={}", deps.display()))
            .arg("-L")
            .arg(format!("dependency={}", host_deps.display()))
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
