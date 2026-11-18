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

fn artifact_dirs() -> Vec<PathBuf> {
    // Layout-independent discovery (cargo 1.100 scatters rlibs into
    // per-package `build/<pkg>/<hash>/out/` dirs; ≤1.98 kept them in
    // `<profile>/deps/`). Search dirs, newest semantics preserved:
    //   (a) the running test binary's own directory when it sits beside
    //       rlibs (old layout, incl. explicit --target variants);
    //   (b) `<profile>/deps` derived by walking up from THIS binary
    //       (never the default worktree target — R3 stale-artifact rule);
    //   (c) every `<profile>/build/*/*/out` directory holding lib*.rlib
    //       (cargo 1.100+).
    let exe = std::env::current_exe().expect("current_exe");
    let bin_dir = exe.parent().expect("dir of the test binary").to_path_buf();
    // Accept rlibs AND proc-macro dylibs: compiling against amari_core
    // needs its dependency rlibs and any proc-macros they used
    // (thiserror_impl is a .so).
    let has_rlib = |dir: &std::path::Path| {
        std::fs::read_dir(dir).is_ok_and(|rd| {
            rd.filter_map(|e| e.ok()).any(|e| {
                e.file_name().to_str().is_some_and(|n| {
                    n.starts_with("lib") && (n.ends_with(".rlib") || n.ends_with(".so"))
                })
            })
        })
    };
    let mut dirs: Vec<PathBuf> = vec![];
    if has_rlib(&bin_dir) {
        dirs.push(bin_dir.clone());
    }
    if let Some(profile) = profile_dir_of(&bin_dir) {
        let deps = profile.join("deps");
        if has_rlib(&deps) && !dirs.contains(&deps) {
            dirs.push(deps);
        }
        if let Ok(build) = std::fs::read_dir(profile.join("build")) {
            for pkg in build.filter_map(|e| e.ok()) {
                if let Ok(hashes) = std::fs::read_dir(pkg.path()) {
                    for hash in hashes.filter_map(|e| e.ok()) {
                        let out = hash.path().join("out");
                        if has_rlib(&out) && !dirs.contains(&out) {
                            dirs.push(out);
                        }
                    }
                }
            }
        }
    }
    assert!(
        !dirs.is_empty(),
        "no artifact directories found (bin at {bin_dir:?}) — build first"
    );
    dirs
}

/// `<profile>` (debug/release) owning the given binary directory, for
/// the out → <hash> → <pkg> → build → <profile> shape (cargo 1.100+)
/// or the plain deps shape (≤1.98 returns the target root's profile
/// via its parent).
fn profile_dir_of(bin_dir: &std::path::Path) -> Option<PathBuf> {
    if bin_dir.ends_with("out") {
        // out -> <hash> -> <pkg> -> build -> <profile>
        bin_dir
            .parent()
            .and_then(|h| h.parent())
            .and_then(|p| p.parent())
            .and_then(|b| b.parent())
            .map(PathBuf::from)
    } else {
        // <profile>/deps (possibly under <triple>/): deps -> <profile>
        bin_dir.parent().map(PathBuf::from)
    }
}

fn amari_core_rlib(dirs: &[PathBuf]) -> PathBuf {
    // R4 P2: select by CAPABILITY, not mtimes (cached alternating builds
    // leave stale rlibs). The rlib we need is exactly the one that
    // provides the feature-complete surface this suite exercises
    // (amari_core::verified requires phantom-types): probe-compile a
    // minimal user with each candidate; the first that compiles wins.
    // Falls back loudly if none does.
    let rustc = std::env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let mut candidates: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|d| {
            std::fs::read_dir(d)
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok())
        })
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("libamari_core-") && n.ends_with(".rlib"))
        })
        .collect();
    candidates.sort();
    candidates.dedup();
    let scratch = ScratchDir::new("probe");
    let probe = scratch.path().join("probe.rs");
    let probe_meta = scratch.path().join("probe.meta");
    std::fs::write(
        &probe,
        "// capability probe: verified requires phantom-types\nfn main() {\n    let _ = amari_core::verified::VerifiedMultivector::<f64, 3, 0, 0>::scalar;\n}\n",
    )
    .expect("write probe");
    for cand in &candidates {
        let mut cmd = Command::new(&rustc);
        cmd.arg("--edition=2024")
            .arg("--crate-type=bin")
            .arg("--emit=metadata")
            .arg("-o")
            .arg(&probe_meta)
            .arg("--extern")
            .arg(format!("amari_core={}", cand.display()));
        for dir in dirs {
            cmd.arg("-L").arg(format!("dependency={}", dir.display()));
        }
        let ok = cmd
            .arg(&probe)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return cand.clone(); // scratch removed by Drop
        }
    }
    panic!(
        "no candidate rlib in {dirs:?} provides the phantom-types surface ({} candidates probed)",
        candidates.len()
    );
}

#[test]
fn static_rejection_cases() {
    let dirs = artifact_dirs();
    let rlib = amari_core_rlib(&dirs);
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

        let mut cmd = Command::new(&rustc);
        cmd.arg("--edition=2024")
            .arg("--crate-type=bin")
            .arg("-o")
            .arg(out_dir.path().join("reject-bin"))
            .arg("--extern")
            .arg(format!("amari_core={}", rlib.display()));
        for dir in &dirs {
            cmd.arg("-L").arg(format!("dependency={}", dir.display()));
        }
        let out = cmd.arg(case).output().expect("run rustc");

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
