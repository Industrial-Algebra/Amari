// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for Cargo platform configuration inspection (Task 8B2).
//!
//! These tests compose existing `CargoInspection` and `RustSourceInspection`
//! with bounded, read-only inspection of `.cargo/config.toml` via
//! [`inspect_cargo_platform`]. Every test materializes fixtures into an
//! isolated `TempDir`; no tracked source fixture is ever mutated.
//!
//! # Invariants verified
//!
//! - No Cargo/rustc/build-script/runner/linker/shell/network execution.
//! - Only `.cargo/config.toml` is authoritative (no global/legacy config).
//! - Symlinked `.cargo` dir and config file are never followed.
//! - All five `InspectionLimits` apply to the config input.
//! - Warnings never leak source excerpts, command values, absolute roots,
//!   external symlink targets, or secrets.
//! - Config-derived provenance is deterministic and root-independent.
//! - Every evidence source resolves to an accepted upstream Cargo/Rust
//!   or config input provenance.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use amari_discovery::inspect::{
    inspect_cargo_platform, inspect_cargo_platform_with_elapsed, inspect_cargo_project,
    inspect_rust_sources, CargoPlatformWarning, InspectionLimit, InspectionLimits, RustFileKind,
    SnapshotState,
};
use amari_discovery::{
    BenchmarkStatus, CargoPlatformInspection, CargoTargetKey, ConfigSetting, ConfigSettingIssue,
    NativeRequirement, RustflagCategory, TargetCfgSource, WasmTargetOrigin,
};

// ===========================================================================
// Fixture materialization helpers
// ===========================================================================

fn fixture_source() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/rust-project"
    ))
}

/// Recursively copy the fixture into a TempDir, transforming `.in` files
/// and substituting the embedded catalog version.
fn materialize_fixture() -> TempDir {
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    let temp = TempDir::new().unwrap();
    copy_and_transform(fixture_source(), temp.path(), &version);
    temp
}

fn copy_and_transform(src: &Path, dst: &Path, version: &str) {
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let file_name = entry.file_name();
        let name = file_name.to_string_lossy();
        let src_path = entry.path();
        if src_path.is_dir() {
            let dst_sub = dst.join(&*file_name);
            fs::create_dir_all(&dst_sub).unwrap();
            copy_and_transform(&src_path, &dst_sub, version);
        } else if name.ends_with(".in") {
            let base = name.trim_end_matches(".in");
            let content = fs::read_to_string(&src_path).unwrap();
            let transformed = content.replace("__AMARI_VERSION__", version);
            fs::write(dst.join(base), transformed).unwrap();
        } else if (name == "Cargo.toml" || name == "Cargo.lock")
            && src.join(format!("{name}.in")).exists()
        {
            continue;
        } else {
            fs::copy(&src_path, dst.join(&*file_name)).unwrap();
        }
    }
}

fn default_limits() -> InspectionLimits {
    InspectionLimits::default()
}

/// Inspect a materialized fixture fully (cargo + rust + platform).
fn inspect_all(
    dir: &Path,
) -> (
    amari_discovery::inspect::CargoInspection,
    amari_discovery::inspect::RustSourceInspection,
    CargoPlatformInspection,
) {
    let cargo = inspect_cargo_project(dir, &default_limits()).unwrap();
    let rust = inspect_rust_sources(dir, &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(dir, &cargo, &rust, &default_limits()).unwrap();
    (cargo, rust, platform)
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

// ===========================================================================
// 1 — Build settings: target array, rustflags, incremental, target-dir
// ===========================================================================

#[test]
fn build_settings_target_and_flags() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());

    // [build].target — both WASM and native, sorted/deduped
    let targets = &platform.build_settings.target;
    assert!(
        targets.contains(&"wasm32-unknown-unknown".to_string()),
        "build target must include wasm32-unknown-unknown: {targets:?}"
    );
    assert!(
        targets.contains(&"x86_64-unknown-linux-gnu".to_string()),
        "build target must include native triple: {targets:?}"
    );

    // incremental and target-dir presence
    assert_eq!(platform.build_settings.incremental, Some(true));
    assert!(
        platform.build_settings.target_dir_set,
        "target-dir presence must be recorded without exposing the value"
    );

    // rustflags evidence: 4 tokens, at least one native link-arg, no raw values
    let rf = &platform.build_settings.rustflags;
    assert_eq!(rf.flag_count, 4, "build rustflags token count");
    assert!(
        rf.has_native_linking,
        "build rustflags has a native link-arg"
    );
    let link_arg = rf
        .categories
        .iter()
        .find(|c| c.category == RustflagCategory::LinkArg);
    assert!(
        link_arg.is_some_and(|c| c.count == 1),
        "exactly one LinkArg category expected"
    );
    assert!(
        rf.categories
            .iter()
            .any(|c| c.category == RustflagCategory::TargetFeature),
        "target-feature category expected"
    );
    assert!(!rf.identity.is_empty(), "rustflags identity present");
}

#[test]
fn build_rustflags_rustdocflags_carry_categories() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());
    // rustdocflags exercised too
    assert!(
        platform.build_settings.rustdocflags.flag_count >= 2,
        "rustdocflags token count"
    );
    assert!(
        platform
            .build_settings
            .rustdocflags
            .categories
            .iter()
            .any(|c| c.category == RustflagCategory::Cfg),
        "rustdocflags --cfg categorized"
    );
}

// ===========================================================================
// 2 — Target settings: triple + cfg tables, linker/runner/rustflags
// ===========================================================================

#[test]
fn target_settings_capture_triple_and_cfg_tables() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());

    let wasm = platform
        .target_settings
        .iter()
        .find(|t| matches!(t.key, CargoTargetKey::Triple { ref triple } if triple == "wasm32-unknown-unknown"))
        .expect("wasm32 target table present");
    assert_eq!(
        wasm.linker.as_ref().unwrap().basename,
        "rust-lld",
        "linker basename sanitized"
    );
    assert!(
        wasm.rustflags.flag_count >= 2,
        "wasm target rustflags token count"
    );

    let unix = platform
        .target_settings
        .iter()
        .find(
            |t| matches!(t.key, CargoTargetKey::Cfg { ref display, .. } if display == "cfg(unix)"),
        )
        .expect("cfg(unix) target table present");
    let runner = unix.runner.as_ref().expect("runner configured");
    assert_eq!(
        runner.executable_basename, "valgrind",
        "runner executable basename sanitized"
    );
    assert_eq!(
        runner.token_count, 2,
        "runner token count (executable + arg)"
    );
    assert!(
        unix.rustflags.has_native_linking,
        "cfg(unix) rustflags carry a native link-arg"
    );

    let win = platform
        .target_settings
        .iter()
        .find(|t| matches!(t.key, CargoTargetKey::Triple { ref triple } if triple == "x86_64-pc-windows-msvc"))
        .expect("windows target table present");
    assert_eq!(win.linker.as_ref().unwrap().basename, "link.exe");
}

#[test]
fn target_settings_have_config_source() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());
    for ts in &platform.target_settings {
        assert_eq!(ts.source.path, ".cargo/config.toml");
        assert!(!ts.source.content_hash.is_empty());
    }
}

// ===========================================================================
// 3 — Configured WASM targets (build + target table, deduped)
// ===========================================================================

#[test]
fn wasm_targets_deduped_with_origins() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());

    let wasm = platform
        .wasm_targets
        .iter()
        .find(|w| w.target == "wasm32-unknown-unknown")
        .expect("wasm target evidence");
    let origins: HashSet<&WasmTargetOrigin> = wasm.origins.iter().collect();
    assert!(
        origins.contains(&WasmTargetOrigin::BuildTarget),
        "wasm from build target"
    );
    assert!(
        origins.contains(&WasmTargetOrigin::TargetTable),
        "wasm from target table"
    );
    // Deterministic dedup: only one entry per target
    assert_eq!(
        platform
            .wasm_targets
            .iter()
            .filter(|w| w.target == "wasm32-unknown-unknown")
            .count(),
        1,
        "single deduped wasm target entry"
    );
    // Sorted
    let mut sorted = platform.wasm_targets.clone();
    sorted.sort_by(|a, b| a.target.cmp(&b.target));
    assert_eq!(platform.wasm_targets, sorted);
}

#[test]
fn wasm64_target_evidence_included() {
    // wasm64-* targets must be included alongside wasm32-*.
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[build]\ntarget = [\"wasm64-unknown-unknown\"]\n[target.wasm64-unknown-unknown]\nlinker = \"rust-lld\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    let wasm64 = platform
        .wasm_targets
        .iter()
        .find(|w| w.target == "wasm64-unknown-unknown")
        .expect("wasm64 target evidence");
    let origins: HashSet<&WasmTargetOrigin> = wasm64.origins.iter().collect();
    assert!(origins.contains(&WasmTargetOrigin::BuildTarget));
    assert!(origins.contains(&WasmTargetOrigin::TargetTable));
}

#[test]
fn wasm_evidence_carries_resolving_config_source() {
    // Every WasmTargetEvidence must carry a direct ConfigSource that resolves
    // to the accepted config input (matching content hash/byte count).
    let temp = materialize_fixture();
    let cfg_bytes = fs::read(temp.path().join(".cargo").join("config.toml")).unwrap();
    let expected_hash = sha256_hex(&cfg_bytes);
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        !platform.wasm_targets.is_empty(),
        "fixture has wasm targets"
    );
    for w in &platform.wasm_targets {
        assert!(
            !w.sources.is_empty(),
            "wasm target {} must carry resolving source",
            w.target
        );
        for src in &w.sources {
            assert_eq!(
                src.content_hash, expected_hash,
                "wasm source resolves to real config hash"
            );
            assert_eq!(
                src.byte_count,
                cfg_bytes.len() as u64,
                "wasm source resolves to real config byte count"
            );
            assert_eq!(src.path, ".cargo/config.toml");
        }
    }
}

// ===========================================================================
// 4 — Native requirements: links, linker, native rustflags
// ===========================================================================

#[test]
fn native_requirements_from_cargo_and_config() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());

    // Cargo package.links (root + member-b)
    let links: Vec<_> = platform
        .native_requirements
        .iter()
        .filter_map(|n| match n {
            NativeRequirement::CargoLinks {
                links_key, package, ..
            } => Some((package.clone(), links_key.clone())),
            _ => None,
        })
        .collect();
    assert!(
        links
            .iter()
            .any(|(p, k)| p == "rust-project" && k == "rust-project-native"),
        "root links present: {links:?}"
    );
    assert!(
        links
            .iter()
            .any(|(p, k)| p == "member-b" && k == "member-b-native"),
        "member-b links present: {links:?}"
    );

    // Configured linkers (sanitized basenames)
    let linker_targets: Vec<_> = platform
        .native_requirements
        .iter()
        .filter_map(|n| match n {
            NativeRequirement::ConfiguredLinker {
                target_key,
                basename,
                ..
            } => Some((target_key.clone(), basename.clone())),
            _ => None,
        })
        .collect();
    assert!(
        linker_targets.iter().any(|(b, name)| b
            == &CargoTargetKey::Triple {
                triple: "wasm32-unknown-unknown".into()
            }
            && name == "rust-lld"),
        "wasm configured linker: {linker_targets:?}"
    );
    assert!(
        linker_targets.iter().any(|(b, name)| b
            == &CargoTargetKey::Triple {
                triple: "x86_64-pc-windows-msvc".into()
            }
            && name == "link.exe"),
        "windows configured linker: {linker_targets:?}"
    );

    // Native rustflags from build + target scopes
    let native_rf_count = platform
        .native_requirements
        .iter()
        .filter(|n| matches!(n, NativeRequirement::NativeRustflags { .. }))
        .count();
    assert!(
        native_rf_count >= 2,
        "at least build + cfg(unix) native rustflags: got {native_rf_count}"
    );
}

// ===========================================================================
// 5 — no_std evidence (only literal #![no_std], not cfg_attr)
// ===========================================================================

#[test]
fn no_std_evidence_only_from_literal_attribute() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());

    assert!(platform.no_std_evidence.has_no_std, "root has #![no_std]");
    let pkgs: Vec<&str> = platform
        .no_std_evidence
        .packages
        .iter()
        .map(|p| p.package.as_str())
        .collect();
    assert!(
        pkgs.contains(&"rust-project"),
        "root package is no_std: {pkgs:?}"
    );
    // member-a uses cfg_attr(target_arch = "wasm32", no_std) — NOT literal no_std
    assert!(
        !pkgs.contains(&"member-a"),
        "cfg_attr-gated no_std must not count as literal no_std: {pkgs:?}"
    );
    // sources resolve to accepted Rust input files
    for pkg in &platform.no_std_evidence.packages {
        for src in &pkg.sources {
            assert!(!src.content_hash.is_empty());
            assert!(src.path.ends_with("lib.rs"));
        }
    }
}

// ===========================================================================
// 6 — Target cfg constraints: Cargo cfg(unix) + Rust target_arch
// ===========================================================================

#[test]
fn target_cfg_constraints_cargo_and_rust() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());

    // Cargo selector cfg(unix) from [target.'cfg(unix)'.dependencies],
    // normalized to the canonical unwrapped form `unix` (matching Rust cfg
    // predicate representation) so equivalent Cargo/Rust constraints merge.
    let unix = platform
        .target_cfg_constraints
        .iter()
        .find(|c| c.predicate == "unix")
        .expect("cfg(unix) constraint from Cargo (unwrapped)");
    assert!(
        unix.sources
            .iter()
            .any(|s| matches!(s, TargetCfgSource::CargoDependencySelector { .. })),
        "cfg(unix) must trace to a Cargo dependency selector"
    );

    // Rust platform predicate target_arch = "wasm32" from member-a cfg_attr
    let arch = platform
        .target_cfg_constraints
        .iter()
        .find(|c| c.predicate.contains("target_arch") && c.predicate.contains("wasm32"))
        .expect("target_arch platform constraint from Rust");
    assert!(
        arch.sources
            .iter()
            .any(|s| matches!(s, TargetCfgSource::RustAttribute { .. })),
        "target_arch must trace to a Rust cfg/cfg_attr attribute"
    );
}

#[test]
fn cargo_and_rust_cfg_constraints_merge_by_canonical_predicate() {
    // A Cargo [target.'cfg(target_arch = "wasm32")'.dependencies] selector
    // and a Rust #[cfg_attr(target_arch = "wasm32", ...)] attribute must merge
    // into ONE constraint (canonical unwrapped predicate) with BOTH a Cargo
    // dependency-selector source and a Rust attribute source.
    let temp = materialize_fixture();
    let manifest_path = temp.path().join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    let with_dep = manifest.replace(
        "[features]",
        "[target.'cfg(target_arch = \"wasm32\")'.dependencies]\nserde_json = \"1.0\"\n\n[features]",
    );
    fs::write(&manifest_path, with_dep).unwrap();
    let (_, _, platform) = inspect_all(temp.path());

    // Canonical unwrapped predicate (no cfg(...) wrapper), matching Rust form.
    let merged = platform
        .target_cfg_constraints
        .iter()
        .find(|c| c.predicate == "target_arch = \"wasm32\"")
        .unwrap_or_else(|| {
            panic!(
                "merged unwrapped predicate missing; got predicates {:?}",
                platform
                    .target_cfg_constraints
                    .iter()
                    .map(|c| &c.predicate)
                    .collect::<Vec<_>>()
            )
        });
    let has_cargo = merged
        .sources
        .iter()
        .any(|s| matches!(s, TargetCfgSource::CargoDependencySelector { .. }));
    let has_rust = merged
        .sources
        .iter()
        .any(|s| matches!(s, TargetCfgSource::RustAttribute { .. }));
    assert!(
        has_cargo,
        "merged constraint must include a Cargo dependency selector source"
    );
    assert!(
        has_rust,
        "merged constraint must include a Rust attribute source"
    );
    // No separate wrapped cfg(...) duplicate remains.
    assert!(
        !platform
            .target_cfg_constraints
            .iter()
            .any(|c| c.predicate == "cfg(target_arch = \"wasm32\")"),
        "wrapped cfg(...) duplicate must not survive normalization"
    );
}

// ===========================================================================
// 7 — Feature-only cfg excluded from platform constraints
// ===========================================================================

#[test]
fn feature_only_cfg_excluded_from_constraints() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());
    for c in &platform.target_cfg_constraints {
        assert!(
            !c.predicate.contains("feature ="),
            "feature-only cfg must not be a platform constraint: {:?}",
            c.predicate
        );
    }
}

// ===========================================================================
// 8 — Benchmarks: declared+source, conventional, member
// ===========================================================================

#[test]
fn benchmarks_compose_declarations_and_sources() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());

    let find_bench = |package: &str, name: &str| {
        platform
            .benchmarks
            .iter()
            .find(|b| b.package == package && b.name == name)
    };

    // speed_bench: declared + has source
    let speed = find_bench("rust-project", "speed_bench").expect("speed_bench");
    assert!(
        matches!(speed.status, BenchmarkStatus::DeclaredWithSource),
        "speed_bench declared with source"
    );
    assert!(speed.source.is_some(), "source content hash present");
    assert!(speed.declaration_source.is_some());

    // correctness_bench: declared + has source
    let corr = find_bench("rust-project", "correctness_bench").expect("correctness_bench");
    assert!(matches!(corr.status, BenchmarkStatus::DeclaredWithSource));

    // bench.rs: conventional undeclared
    let conv = platform
        .benchmarks
        .iter()
        .find(|b| b.path == "benches/bench.rs")
        .expect("conventional bench");
    assert!(
        matches!(conv.status, BenchmarkStatus::ConventionalUndeclared),
        "benches/bench.rs is conventional undeclared"
    );
    assert!(conv.declaration_source.is_none());

    // member_bench: member declared + source
    let member = find_bench("member-b", "member_bench").expect("member_bench");
    assert!(matches!(member.status, BenchmarkStatus::DeclaredWithSource));
    assert!(member.path == "member-b/benches/member_bench.rs");
}

#[test]
fn benchmarks_source_resolves_to_input_file() {
    let temp = materialize_fixture();
    let (_, rust, platform) = inspect_all(temp.path());
    let input_hashes: HashSet<&str> = rust
        .input_files
        .iter()
        .map(|f| f.content_hash.as_str())
        .collect();
    for b in &platform.benchmarks {
        if let Some(src) = &b.source {
            assert!(
                input_hashes.contains(src.content_hash.as_str()),
                "bench source must resolve to accepted Rust input"
            );
        }
    }
}

// ===========================================================================
// 9 — Declared-but-missing source benchmark (dynamic case)
// ===========================================================================

#[test]
fn declared_bench_missing_source_status() {
    let temp = materialize_fixture();
    // Rewrite root Cargo.toml to add a declared bench with no source file.
    let manifest_path = temp.path().join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    let extra =
        "\n[[bench]]\nname = \"ghost_bench\"\npath = \"benches/ghost_bench.rs\"\nharness = false\n";
    let with_ghost = manifest.replace("[dependencies]", &format!("{extra}\n[dependencies]"));
    fs::write(&manifest_path, with_ghost).unwrap();

    let (_, _, platform) = inspect_all(temp.path());
    let ghost = platform
        .benchmarks
        .iter()
        .find(|b| b.name == "ghost_bench")
        .expect("ghost_bench declared");
    assert!(
        matches!(ghost.status, BenchmarkStatus::DeclaredMissingSource { .. }),
        "declared missing source bench"
    );
    assert!(
        matches!(ghost.status, BenchmarkStatus::DeclaredMissingSource { ref declared_path } if declared_path == "benches/ghost_bench.rs"),
        "ghost_bench carries declared path: {:?}",
        ghost.status
    );
    assert!(ghost.source.is_none(), "no source for missing bench");
}

// ===========================================================================
// 10 — Missing config: Complete + warning + empty provenance + derived ev
// ===========================================================================

#[test]
fn missing_config_is_complete_with_derived_evidence() {
    let temp = materialize_fixture();
    // Remove the config from the TempDir copy only.
    let cfg = temp.path().join(".cargo").join("config.toml");
    fs::remove_file(&cfg).unwrap();

    let (cargo, rust, platform) = inspect_all(temp.path());
    assert_eq!(platform.state, SnapshotState::Complete);
    assert!(platform
        .warnings
        .iter()
        .any(|w| matches!(w, CargoPlatformWarning::MissingConfig { .. })));
    // Empty config provenance
    assert_eq!(platform.config_input.file_count, 0);
    assert_eq!(platform.config_input.total_bytes, 0);
    assert_eq!(platform.config_input.source, None);
    // Empty config input hash is SHA-256 of empty framed input set
    assert_eq!(
        platform.config_input.input_hash,
        sha256_hex(b""),
        "empty config hash is SHA-256 of empty input"
    );
    // Build settings default empty; derived Cargo/Rust evidence present
    assert!(platform.build_settings.target.is_empty());
    assert!(!cargo.root_package.dependencies.is_empty());
    assert!(!rust.file_kinds.is_empty());
    // no_std + benchmarks still derived
    assert!(platform.no_std_evidence.has_no_std);
    assert!(!platform.benchmarks.is_empty());
}

// ===========================================================================
// 11 — Malformed config: warning, accepted into provenance, empty settings
// ===========================================================================

#[test]
fn malformed_config_warning_and_provenance() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    let bad = b"this is = = not valid toml [[[ \n unterminated";
    fs::write(&cfg, bad).unwrap();
    let expected_hash = sha256_hex(bad);

    let (_, _, platform) = inspect_all(temp.path());
    assert_eq!(platform.state, SnapshotState::Complete);
    let malformed = platform
        .warnings
        .iter()
        .find(|w| matches!(w, CargoPlatformWarning::MalformedConfig { .. }));
    let malformed = malformed.expect("MalformedConfig warning");
    // Accepted into provenance (count/bytes/hash contribute)
    assert_eq!(platform.config_input.file_count, 1);
    assert_eq!(platform.config_input.total_bytes, bad.len() as u64);
    assert_eq!(
        platform.config_input.input_hash,
        framed_hash(&[(".cargo/config.toml", bad)]),
        "malformed config still framed into input hash"
    );
    // No raw source text in the warning
    let json = serde_json::to_string(malformed).unwrap();
    assert!(!json.contains("unterminated"), "no source excerpt: {json}");
    assert!(
        json.contains(&expected_hash),
        "malformed warning carries content hash"
    );
    // Settings not derived from unparseable config
    assert!(platform.build_settings.target.is_empty());
}

// ===========================================================================
// 12 — Invalid UTF-8 config: warning, accepted into provenance
// ===========================================================================

#[test]
fn invalid_utf8_config_warning_and_provenance() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    let bad: &[u8] = b"[build]\ntarget = \"\xff\xfe bad utf8\"";
    fs::write(&cfg, bad).unwrap();

    let (_, _, platform) = inspect_all(temp.path());
    assert_eq!(platform.state, SnapshotState::Complete);
    assert!(platform
        .warnings
        .iter()
        .any(|w| matches!(w, CargoPlatformWarning::InvalidUtf8Config { .. })));
    assert_eq!(platform.config_input.file_count, 1);
    assert_eq!(platform.config_input.total_bytes, bad.len() as u64);
    assert_eq!(
        platform.config_input.input_hash,
        framed_hash(&[(".cargo/config.toml", bad)])
    );
    assert!(platform.build_settings.target.is_empty());
}

// ===========================================================================
// 13 — Symlinked config file is never followed
// ===========================================================================

#[cfg(unix)]
#[test]
fn symlinked_config_file_not_followed() {
    use std::os::unix::fs::symlink;
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    let outside = temp.path().join("outside.toml");
    fs::write(&outside, b"[build]\n").unwrap();
    fs::remove_file(&cfg).unwrap();
    symlink(&outside, &cfg).unwrap();

    let (_, _, platform) = inspect_all(temp.path());
    assert_eq!(platform.state, SnapshotState::Complete);
    assert!(platform
        .warnings
        .iter()
        .any(|w| matches!(w, CargoPlatformWarning::SymlinkedConfig { .. })));
    // Not accepted (symlink never followed)
    assert_eq!(platform.config_input.file_count, 0);
    assert_eq!(platform.config_input.total_bytes, 0);
}

// ===========================================================================
// 14 — Symlinked .cargo directory is never followed
// ===========================================================================

#[cfg(unix)]
#[test]
fn symlinked_cargo_dir_not_followed() {
    use std::os::unix::fs::symlink;
    let temp = materialize_fixture();
    let cargo_dir = temp.path().join(".cargo");
    // Move real config content to a sibling and replace .cargo with a symlink.
    let real = temp.path().join("real-cargo");
    fs::rename(&cargo_dir, &real).unwrap();
    symlink(&real, &cargo_dir).unwrap();

    let (_, _, platform) = inspect_all(temp.path());
    assert_eq!(platform.state, SnapshotState::Complete);
    assert!(platform
        .warnings
        .iter()
        .any(|w| matches!(w, CargoPlatformWarning::SymlinkedConfig { .. })));
    assert_eq!(platform.config_input.file_count, 0);
    assert_eq!(platform.config_input.total_bytes, 0);
}

// ===========================================================================
// 15 — Per-file byte limit on config
// ===========================================================================

#[test]
fn per_file_limit_on_config() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    let len = fs::read(&cfg).unwrap().len() as u64;

    let mut limits = default_limits();
    // One byte short → exceeded (consistent with Task 7/8B boundary).
    limits.max_per_file_bytes = len.saturating_sub(1);
    let (cargo, rust, platform) = {
        let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
        let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
        let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &limits).unwrap();
        (cargo, rust, platform)
    };
    assert!(matches!(
        platform.state,
        SnapshotState::LimitExceeded { .. }
    ));
    // Not accepted
    assert_eq!(platform.config_input.file_count, 0);
    // Derived Cargo/Rust evidence still present
    assert!(!cargo.root_package.dependencies.is_empty());
    assert!(!rust.file_kinds.is_empty());

    // Exactly at the limit → accepted
    let mut limits2 = default_limits();
    limits2.max_per_file_bytes = len;
    let cargo2 = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust2 = inspect_rust_sources(temp.path(), &cargo2, &default_limits()).unwrap();
    let platform2 = inspect_cargo_platform(temp.path(), &cargo2, &rust2, &limits2).unwrap();
    assert_eq!(platform2.state, SnapshotState::Complete);
    assert_eq!(platform2.config_input.file_count, 1);
}

// ===========================================================================
// 16 — Aggregate byte limit on config (max_inspection_bytes)
// ===========================================================================

#[test]
fn aggregate_byte_limit_on_config() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    let len = fs::read(&cfg).unwrap().len() as u64;

    let mut limits = default_limits();
    limits.max_inspection_bytes = len.saturating_sub(1);
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &limits).unwrap();
    assert!(matches!(
        platform.state,
        SnapshotState::LimitExceeded { .. }
    ));
    assert_eq!(platform.config_input.file_count, 0);
}

// ===========================================================================
// 17 — File-count limit (max_inspection_files == 0) → partial without read
// ===========================================================================

#[test]
fn file_count_zero_returns_partial_without_read() {
    let temp = materialize_fixture();
    let mut limits = default_limits();
    limits.max_inspection_files = 0;
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &limits).unwrap();
    assert!(matches!(
        platform.state,
        SnapshotState::LimitExceeded { .. }
    ));
    assert_eq!(platform.config_input.file_count, 0, "config not read");
    assert!(
        platform.config_input.input_hash.is_empty()
            || platform.config_input.input_hash == sha256_hex(b"")
    );
    // Derived evidence still present
    assert!(!platform.benchmarks.is_empty());
}

// ===========================================================================
// 18 — Traversal depth limit (config is at depth 2)
// ===========================================================================

#[test]
fn depth_limit_prevents_config_read() {
    let temp = materialize_fixture();
    let mut limits = default_limits();
    // Config lives at .cargo/config.toml (depth 2). Depth 1 cannot reach it.
    limits.max_traversal_depth = 1;
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &limits).unwrap();
    assert!(matches!(
        platform.state,
        SnapshotState::LimitExceeded { .. }
    ));
    assert_eq!(platform.config_input.file_count, 0);
    // Depth 2 reaches it
    let mut limits2 = default_limits();
    limits2.max_traversal_depth = 2;
    let platform2 = inspect_cargo_platform(temp.path(), &cargo, &rust, &limits2).unwrap();
    assert_eq!(platform2.state, SnapshotState::Complete);
    assert_eq!(platform2.config_input.file_count, 1);
}

// ===========================================================================
// 19 — Wall-clock limit checked before reading config
// ===========================================================================

#[test]
fn wall_clock_zero_returns_partial() {
    let temp = materialize_fixture();
    let mut limits = default_limits();
    limits.max_inspection_wall_millis = 0;
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &limits).unwrap();
    assert!(matches!(
        platform.state,
        SnapshotState::LimitExceeded { .. }
    ));
    assert_eq!(platform.config_input.file_count, 0);
}

/// Injectable step-clock: returns successive millisecond durations, clamping
/// to the last value once exhausted. Lets the cooperative post-phase checks be
/// exercised deterministically (no flaky sleeps).
fn step_clock(steps: &[u64]) -> impl Fn() -> Duration {
    let idx = Arc::new(AtomicU64::new(0));
    let steps = steps.to_vec();
    move || {
        let i = idx.fetch_add(1, Ordering::SeqCst) as usize;
        Duration::from_millis(steps[i.min(steps.len().saturating_sub(1))])
    }
}

#[test]
fn wall_clock_post_read_trip_skips_parse() {
    // Clock trips at the first post-read checkpoint → parse is skipped.
    let temp = materialize_fixture();
    let mut limits = default_limits();
    limits.max_inspection_wall_millis = 100;
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let cfg_bytes = fs::read(temp.path().join(".cargo").join("config.toml")).unwrap();
    let platform = inspect_cargo_platform_with_elapsed(
        temp.path(),
        &cargo,
        &rust,
        &limits,
        step_clock(&[500]),
    )
    .unwrap();
    // WallClock state with actual observed and configured budget.
    match platform.state {
        SnapshotState::LimitExceeded {
            limit:
                InspectionLimit::WallClock {
                    max_millis,
                    observed_millis,
                },
        } => {
            assert_eq!(max_millis, 100);
            assert_eq!(observed_millis, 500);
        }
        other => panic!("expected WallClock limit, got {other:?}"),
    }
    // WallClock warning present with the same observed/budget (consistent).
    let wc = platform
        .warnings
        .iter()
        .find_map(|w| match w {
            CargoPlatformWarning::WallClock {
                max_millis,
                observed_millis,
            } => Some((*max_millis, *observed_millis)),
            _ => None,
        })
        .expect("WallClock warning present");
    assert_eq!(wc, (100, 500));
    // Provenance is internally consistent: the read happened.
    assert_eq!(platform.config_input.file_count, 1);
    assert_eq!(platform.config_input.total_bytes, cfg_bytes.len() as u64);
    // Parse was skipped → build target settings empty (no wasm32 build target).
    assert!(
        platform.build_settings.target.is_empty(),
        "parse skipped → no build targets"
    );
    // cargo/rust-derived evidence is still present.
    assert!(
        !platform.benchmarks.is_empty(),
        "benchmarks derived from cargo"
    );
}

#[test]
fn wall_clock_post_parse_trip_retains_settings() {
    // Clock passes post-read, trips post-parse → settings retained, all
    // derived evidence computed.
    let temp = materialize_fixture();
    let mut limits = default_limits();
    limits.max_inspection_wall_millis = 100;
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform_with_elapsed(
        temp.path(),
        &cargo,
        &rust,
        &limits,
        step_clock(&[0, 500]),
    )
    .unwrap();
    assert!(matches!(
        platform.state,
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::WallClock { .. }
        }
    ));
    // Parsed settings retained: the wasm32 build target is present.
    assert!(
        platform
            .build_settings
            .target
            .iter()
            .any(|t| t == "wasm32-unknown-unknown"),
        "post-parse trip retains parsed build targets"
    );
    // Derived wasm evidence computed from retained settings.
    assert!(
        platform
            .wasm_targets
            .iter()
            .any(|w| w.target == "wasm32-unknown-unknown"),
        "post-parse trip derives wasm evidence from retained settings"
    );
    assert!(platform
        .warnings
        .iter()
        .any(|w| matches!(w, CargoPlatformWarning::WallClock { .. })));
}

#[test]
fn wall_clock_post_derivation_trip() {
    // Clock passes post-read and post-parse, trips post-derivation → full
    // evidence present plus WallClock state/warning.
    let temp = materialize_fixture();
    let mut limits = default_limits();
    limits.max_inspection_wall_millis = 100;
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform_with_elapsed(
        temp.path(),
        &cargo,
        &rust,
        &limits,
        step_clock(&[0, 0, 500]),
    )
    .unwrap();
    assert!(matches!(
        platform.state,
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::WallClock { .. }
        }
    ));
    // Everything present.
    assert!(platform
        .build_settings
        .target
        .iter()
        .any(|t| t == "wasm32-unknown-unknown"));
    assert!(!platform.target_settings.is_empty());
    assert!(!platform.wasm_targets.is_empty());
    assert!(platform
        .warnings
        .iter()
        .any(|w| matches!(w, CargoPlatformWarning::WallClock { .. })));
}

#[test]
fn wall_clock_generous_budget_stays_complete() {
    // A generous budget with a zero elapsed clock never trips → Complete, no
    // WallClock warning (no false positive from the cooperative checks).
    let temp = materialize_fixture();
    let mut limits = default_limits();
    limits.max_inspection_wall_millis = 1_000_000;
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform =
        inspect_cargo_platform_with_elapsed(temp.path(), &cargo, &rust, &limits, step_clock(&[0]))
            .unwrap();
    assert_eq!(platform.state, SnapshotState::Complete);
    assert!(
        !platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::WallClock { .. })),
        "no false-positive WallClock warning"
    );
}

#[test]
fn config_hash_deterministic() {
    let temp = materialize_fixture();
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let p1 = inspect_cargo_platform(temp.path(), &cargo, &rust, &default_limits()).unwrap();
    let p2 = inspect_cargo_platform(temp.path(), &cargo, &rust, &default_limits()).unwrap();
    assert_eq!(p1.config_input.input_hash, p2.config_input.input_hash);
    assert!(!p1.config_input.input_hash.is_empty());
}

// ===========================================================================
// 21 — Config hash root-independent
// ===========================================================================

#[test]
fn config_hash_root_independent() {
    let temp = materialize_fixture();
    let canon = temp.path().canonicalize().unwrap();
    let cargo1 = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust1 = inspect_rust_sources(temp.path(), &cargo1, &default_limits()).unwrap();
    let p1 = inspect_cargo_platform(temp.path(), &cargo1, &rust1, &default_limits()).unwrap();
    let cargo2 = inspect_cargo_project(&canon, &default_limits()).unwrap();
    let rust2 = inspect_rust_sources(&canon, &cargo2, &default_limits()).unwrap();
    let p2 = inspect_cargo_platform(&canon, &cargo2, &rust2, &default_limits()).unwrap();
    assert_eq!(
        p1.config_input.input_hash, p2.config_input.input_hash,
        "config hash must be root-independent"
    );
}

// ===========================================================================
// 22 — No source/secret/absolute leakage in serialized output
// ===========================================================================

#[test]
fn no_secret_or_absolute_leakage() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());
    let json = serde_json::to_string(&platform).unwrap();

    // Secret flag values never persisted
    assert!(
        !json.contains("secret-native-flag"),
        "secret leaked: {json}"
    );
    assert!(
        !json.contains("--tool=memcheck"),
        "runner command leaked: {json}"
    );
    assert!(
        !json.contains("custom-target"),
        "target-dir value leaked: {json}"
    );
    // Absolute project root never appears
    let canon = temp.path().canonicalize().unwrap();
    assert!(
        !json.contains(canon.to_str().unwrap()),
        "absolute root leaked"
    );
    // External symlink targets absent
    assert!(!json.contains("outside.toml"));
    // Basenames ARE exposed (sanitized)
    assert!(
        json.contains("rust-lld"),
        "sanitized linker basename present"
    );
    assert!(
        json.contains("valgrind"),
        "sanitized runner basename present"
    );
}

#[test]
fn windows_runner_and_linker_paths_do_not_leak_directories() {
    let temp = materialize_fixture();
    write_config(
        &temp,
        r#"[target.x86_64-pc-windows-msvc]
linker = 'C:\Users\SECRET-LINKER-DIR\link.exe'
runner = ['C:\Users\SECRET-RUNNER-DIR\runner.exe', '--flag']
"#,
    );

    let (_, _, platform) = inspect_all(temp.path());
    let target = platform
        .target_settings
        .iter()
        .find(|target| {
            matches!(
                &target.key,
                CargoTargetKey::Triple { triple }
                    if triple == "x86_64-pc-windows-msvc"
            )
        })
        .expect("Windows target settings");

    assert_eq!(
        target
            .linker
            .as_ref()
            .map(|linker| linker.basename.as_str()),
        Some("link.exe")
    );
    assert_eq!(
        target
            .runner
            .as_ref()
            .map(|runner| runner.executable_basename.as_str()),
        Some("runner.exe")
    );

    let json = serde_json::to_string(&platform).unwrap();
    assert!(
        !json.contains("SECRET-LINKER-DIR"),
        "linker path leaked: {json}"
    );
    assert!(
        !json.contains("SECRET-RUNNER-DIR"),
        "runner path leaked: {json}"
    );
    assert!(
        !json.contains("C:\\Users"),
        "Windows directory leaked: {json}"
    );
}

// ===========================================================================
// 23 — Poison runner/linker/build markers remain untouched (no execution)
// ===========================================================================

#[test]
fn poison_runner_and_linker_markers_untouched() {
    let temp = TempDir::new().unwrap();
    // Minimal cargo project with a poison config runner.
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    fs::write(
        temp.path().join("Cargo.toml"),
        format!(
            r#"[package]
name = "poison-plat"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "{version}"
"#
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Cargo.lock"),
        format!(
            r#"version = 3
[[package]]
name = "amari-core"
version = "{version}"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc"
[[package]]
name = "poison-plat"
version = "0.1.0"
"#
        ),
    )
    .unwrap();
    fs::create_dir(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src").join("lib.rs"), "").unwrap();
    fs::create_dir_all(temp.path().join(".cargo")).unwrap();
    // Poison runner that would create a marker file if executed.
    let poison_runner = "sh -c echo_poison_would_create_marker_file";
    let config = format!(
        "[target.'cfg(unix)']\nrunner = \"{poison_runner}\"\n[target.wasm32-unknown-unknown]\nlinker = \"echo poison_linker_executed\"\n"
    );
    fs::write(temp.path().join(".cargo").join("config.toml"), config).unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &default_limits()).unwrap();

    // The marker file was never created.
    assert!(
        !temp
            .path()
            .join("echo_poison_would_create_marker_file")
            .exists(),
        "runner was executed — marker created"
    );
    // Poison values not echoed into output (sanitized).
    let json = serde_json::to_string(&platform).unwrap();
    assert!(!json.contains("poison"), "poison token leaked: {json}");
    // Config still parsed (basename sanitized).
    assert!(json.contains("sh"), "runner basename sanitized");
}

// ===========================================================================
// 24 — Unrelated non-config file leaves whole inspection equal
// ===========================================================================

#[test]
fn unrelated_non_config_file_leaves_equal() {
    let temp = materialize_fixture();
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let p1 = inspect_cargo_platform(temp.path(), &cargo, &rust, &default_limits()).unwrap();

    // Add an unrelated file (not config, not affecting Cargo/Rust).
    fs::write(temp.path().join("NOTES.txt"), "unrelated content\n").unwrap();
    let p2 = inspect_cargo_platform(temp.path(), &cargo, &rust, &default_limits()).unwrap();

    assert_eq!(
        p1, p2,
        "unrelated non-config file must leave whole inspection equal"
    );
}

// ===========================================================================
// 25 — Duplicate evidence deterministically deduped
// ===========================================================================

#[test]
fn duplicate_evidence_deduped() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());

    // No duplicate target settings by key
    let mut keys: Vec<&CargoTargetKey> = platform.target_settings.iter().map(|t| &t.key).collect();
    let len = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), len, "target settings deduped");

    // No duplicate native requirements
    let json_set: HashSet<String> = platform
        .native_requirements
        .iter()
        .map(|n| serde_json::to_string(n).unwrap())
        .collect();
    assert_eq!(
        json_set.len(),
        platform.native_requirements.len(),
        "native requirements deduped"
    );

    // No duplicate target cfg predicates
    let mut preds: Vec<&str> = platform
        .target_cfg_constraints
        .iter()
        .map(|c| c.predicate.as_str())
        .collect();
    let len = preds.len();
    preds.sort();
    preds.dedup();
    assert_eq!(preds.len(), len, "cfg constraint predicates deduped");
}

// ===========================================================================
// 26 — Every evidence source resolves to accepted upstream/config source
// ===========================================================================
// 26 — Every evidence source resolves to accepted upstream/config source
// ===========================================================================

#[test]
fn every_evidence_source_resolves() {
    let temp = materialize_fixture();
    // Add an explicit arbitrary-path bench (outside benches/) with harness and
    // required-features so its provenance is exercised too.
    fs::create_dir_all(temp.path().join("perf")).unwrap();
    fs::write(temp.path().join("perf").join("custom.rs"), "fn main() {}").unwrap();
    let manifest_path = temp.path().join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    let with_bench = format!(
        "{manifest}\n[[bench]]\nname = \"custom\"\npath = \"perf/custom.rs\"\nharness = false\nrequired-features = [\"perf\"]\n"
    );
    fs::write(&manifest_path, with_bench).unwrap();

    let (cargo, rust, platform) = inspect_all(temp.path());

    // Exact upstream record sets.
    let config_source = platform
        .config_input
        .source
        .clone()
        .expect("accepted config source");
    // Manifest sources: path -> (content_hash, byte_count) from every accepted manifest.
    let mut manifest_records: std::collections::HashMap<&str, (&str, u64)> =
        std::collections::HashMap::new();
    for pkg in std::iter::once(&cargo.root_package).chain(cargo.workspace_members.iter()) {
        for d in &pkg.dependencies {
            manifest_records.insert(
                d.manifest_source.path.as_str(),
                (
                    d.manifest_source.content_hash.as_str(),
                    d.manifest_source.byte_count,
                ),
            );
        }
        for b in &pkg.benches {
            manifest_records.insert(
                b.manifest_source.path.as_str(),
                (
                    b.manifest_source.content_hash.as_str(),
                    b.manifest_source.byte_count,
                ),
            );
        }
        if let Some(n) = &pkg.native_link {
            manifest_records.insert(
                n.manifest_source.path.as_str(),
                (
                    n.manifest_source.content_hash.as_str(),
                    n.manifest_source.byte_count,
                ),
            );
        }
        for s in &pkg.system_dependencies {
            manifest_records.insert(
                s.manifest_source.path.as_str(),
                (
                    s.manifest_source.content_hash.as_str(),
                    s.manifest_source.byte_count,
                ),
            );
        }
    }
    // Rust input files: path -> content_hash.
    let rust_input: std::collections::HashMap<&str, &str> = rust
        .input_files
        .iter()
        .map(|f| (f.path.as_str(), f.content_hash.as_str()))
        .collect();

    let resolve_manifest = |ms: &amari_discovery::ManifestSource| {
        let (h, b) = manifest_records
            .get(ms.path.as_str())
            .unwrap_or_else(|| panic!("manifest source {} not in upstream records", ms.path));
        assert_eq!(
            ms.content_hash, *h,
            "manifest {} content_hash mismatch",
            ms.path
        );
        assert_eq!(
            ms.byte_count, *b,
            "manifest {} byte_count mismatch",
            ms.path
        );
    };
    let resolve_config = |cs: &amari_discovery::ConfigSource| {
        assert_eq!(cs.path, ".cargo/config.toml");
        assert_eq!(cs.content_hash, config_source.content_hash);
        assert_eq!(cs.byte_count, config_source.byte_count);
    };

    // --- Native requirements ---
    for nr in &platform.native_requirements {
        match nr {
            NativeRequirement::CargoLinks { source, .. }
            | NativeRequirement::SystemDependency { source, .. } => resolve_manifest(source),
            NativeRequirement::ConfiguredLinker { config, .. }
            | NativeRequirement::NativeRustflags { config, .. } => resolve_config(config),
        }
    }

    // --- Build + target settings resolve to the config source ---
    if let Some(bs) = &platform.build_settings.source {
        resolve_config(bs);
    }
    for ts in &platform.target_settings {
        resolve_config(&ts.source);
    }

    // --- WASM targets derive only from validated build triples / target triples ---
    let valid_triples: HashSet<&str> = platform
        .build_settings
        .target
        .iter()
        .map(|s| s.as_str())
        .collect();
    let target_table_triples: HashSet<&str> = platform
        .target_settings
        .iter()
        .filter_map(|t| match &t.key {
            amari_discovery::CargoTargetKey::Triple { triple } => Some(triple.as_str()),
            _ => None,
        })
        .collect();
    for w in &platform.wasm_targets {
        let is_build = valid_triples.contains(w.target.as_str());
        let is_table = target_table_triples.contains(w.target.as_str());
        assert!(
            is_build || is_table,
            "wasm target {} must derive from a validated triple",
            w.target
        );
        // Every direct ConfigSource on the wasm evidence resolves exactly to
        // the accepted config input.
        assert!(
            !w.sources.is_empty(),
            "wasm target {} must carry at least one resolving source",
            w.target
        );
        for src in &w.sources {
            resolve_config(src);
        }
    }

    // --- Benchmarks: source resolves to a Rust input, declaration to a manifest ---
    for b in &platform.benchmarks {
        if let Some(src) = &b.source {
            assert!(
                rust_input.contains_key(src.path.as_str()),
                "bench source {} must resolve to a Rust input file",
                src.path
            );
            assert_eq!(
                rust_input[src.path.as_str()],
                src.content_hash,
                "bench source content_hash mismatch"
            );
        }
        if let Some(decl) = &b.declaration_source {
            resolve_manifest(decl);
        }
    }

    // --- Explicit arbitrary-path bench (outside benches/) resolves directly ---
    let custom = platform
        .benchmarks
        .iter()
        .find(|b| b.name == "custom")
        .expect("arbitrary-path perf/custom.rs bench present");
    assert_eq!(custom.path, "perf/custom.rs");
    assert!(
        matches!(custom.status, BenchmarkStatus::DeclaredWithSource),
        "arbitrary bench matches accepted input file"
    );
    assert!(!custom.harness);
    assert_eq!(custom.required_features, vec!["perf".to_string()]);

    // --- no_std: sources resolve to Rust input files ---
    for pkg in &platform.no_std_evidence.packages {
        for src in &pkg.sources {
            assert!(
                rust_input.contains_key(src.path.as_str()),
                "no_std source {} must resolve to a Rust input",
                src.path
            );
            assert_eq!(
                rust_input[src.path.as_str()],
                src.content_hash,
                "no_std content_hash mismatch"
            );
        }
    }

    // --- cfg constraints: Rust attrs resolve to Rust input; Cargo selectors to manifest ---
    for c in &platform.target_cfg_constraints {
        for s in &c.sources {
            match s {
                TargetCfgSource::RustAttribute { source, .. } => {
                    assert!(
                        rust_input.contains_key(source.path.as_str()),
                        "Rust cfg source {} must resolve to input",
                        source.path
                    );
                    assert_eq!(rust_input[source.path.as_str()], source.content_hash);
                }
                TargetCfgSource::CargoDependencySelector { source, .. } => resolve_manifest(source),
            }
        }
    }
}

// ===========================================================================
// 27 — Config content hash matches the fixture bytes
// ===========================================================================

#[test]
fn config_source_hash_matches_bytes() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    let bytes = fs::read(&cfg).unwrap();
    let expected = sha256_hex(&bytes);
    let (_, _, platform) = inspect_all(temp.path());
    let src = platform
        .config_input
        .source
        .as_ref()
        .expect("config source");
    assert_eq!(src.content_hash, expected);
    assert_eq!(src.byte_count, bytes.len() as u64);
    assert_eq!(src.path, ".cargo/config.toml");
}

// ===========================================================================
// 28 — Only .cargo/config.toml authoritative; legacy config ignored
// ===========================================================================

#[test]
fn legacy_config_file_ignored() {
    let temp = materialize_fixture();
    // Remove config.toml and add legacy .cargo/config — it must be ignored.
    let cfg = temp.path().join(".cargo").join("config.toml");
    fs::remove_file(&cfg).unwrap();
    fs::write(
        temp.path().join(".cargo").join("config"),
        "[build]\ntarget = [\"wasm32-unknown-unknown\"]\n",
    )
    .unwrap();

    let (_, _, platform) = inspect_all(temp.path());
    // config.toml missing → MissingConfig; legacy config NOT consulted.
    assert!(platform
        .warnings
        .iter()
        .any(|w| matches!(w, CargoPlatformWarning::MissingConfig { .. })));
    assert!(platform.build_settings.target.is_empty());
    assert!(platform.wasm_targets.is_empty());
}

// ===========================================================================
// 29 — Regression: Task 8A (Cargo) and Task 8B (Rust) still pass
// ===========================================================================

#[test]
fn task8a_task8b_regression() {
    let temp = materialize_fixture();
    let (cargo, rust, _) = inspect_all(temp.path());

    // Task 8A: Cargo resolution
    assert_eq!(cargo.root_package.name, "rust-project");
    assert!(cargo
        .root_package
        .dependencies
        .iter()
        .any(|d| d.package_name == "amari-tropical"));
    assert!(cargo
        .root_package
        .dependencies
        .iter()
        .any(|d| d.target.as_deref() == Some("cfg(unix)")));
    assert_eq!(cargo.state, SnapshotState::Complete);

    // Task 8B: Rust usage + no_std crate attribute
    assert!(!rust.usages.is_empty());
    let no_std = rust
        .crate_attributes
        .iter()
        .any(|a| a.attribute == "no_std");
    assert!(no_std, "Rust #![no_std] detected");
    // Bench file classification still present
    assert!(rust
        .file_kinds
        .iter()
        .any(|f| matches!(f, RustFileKind::Bench { .. })));
}

// ===========================================================================
// B3 — build.target validated triples only + custom target JSON opaque
// ===========================================================================

fn write_config(temp: &TempDir, body: &str) {
    fs::write(
        temp.path().join(".cargo").join("config.toml"),
        body.as_bytes(),
    )
    .unwrap();
}

#[test]
fn build_target_custom_json_secret_not_leaked() {
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[build]\ntarget = [\"wasm32-unknown-unknown\", \"/abs/SECRET-VALUE-XYZ/my-target.json\"]\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    // Only the validated triple survives in `target`.
    assert_eq!(
        platform.build_settings.target,
        vec!["wasm32-unknown-unknown".to_string()]
    );
    // The custom JSON path is opaque evidence (count, no path/basename).
    assert_eq!(
        platform.build_settings.custom_targets.count, 1,
        "custom target spec counted"
    );
    assert!(!platform.build_settings.custom_targets.identity.is_empty());
    // Secret and absolute path never leak into serialized output.
    let json = serde_json::to_string(&platform).unwrap();
    assert!(!json.contains("SECRET-VALUE-XYZ"), "secret leaked: {json}");
    assert!(!json.contains("/abs/"), "absolute path leaked: {json}");
    assert!(
        !json.contains("my-target.json"),
        "custom target basename leaked: {json}"
    );
}

#[test]
fn build_target_malformed_array_member_warns() {
    let temp = materialize_fixture();
    // Array with a non-string member (number) and a clean triple.
    write_config(
        &temp,
        "[build]\ntarget = [\"wasm32-unknown-unknown\", 12345]\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    // Clean triple still retained.
    assert_eq!(
        platform.build_settings.target,
        vec!["wasm32-unknown-unknown".to_string()]
    );
    // Typed warning for the mixed/invalid member (never the value).
    let has_warning = platform.warnings.iter().any(|w| {
        matches!(
            w,
            CargoPlatformWarning::InvalidConfigSetting {
                setting: amari_discovery::ConfigSetting::BuildTarget,
                issue: amari_discovery::ConfigSettingIssue::MixedArray,
                ..
            }
        )
    });
    assert!(has_warning, "expected MixedArray warning for build.target");
    let json = serde_json::to_string(&platform).unwrap();
    assert!(
        !json.contains("12345"),
        "malformed member value leaked: {json}"
    );
}

#[test]
fn build_target_invalid_non_path_non_triple_warned() {
    let temp = materialize_fixture();
    // A single garbage string: not a triple, not a path.
    write_config(&temp, "[build]\ntarget = \"%%not-a-triple%%\"\n");
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        platform.build_settings.target.is_empty(),
        "invalid non-path non-triple must not enter target"
    );
    let has_warning = platform.warnings.iter().any(|w| {
        matches!(
            w,
            CargoPlatformWarning::InvalidConfigSetting {
                setting: amari_discovery::ConfigSetting::BuildTarget,
                issue: amari_discovery::ConfigSettingIssue::InvalidValue,
                ..
            }
        )
    });
    assert!(
        has_warning,
        "expected InvalidValue warning for build.target"
    );
    let json = serde_json::to_string(&platform).unwrap();
    assert!(
        !json.contains("%%not-a-triple%%"),
        "invalid value leaked: {json}"
    );
}

#[test]
fn build_target_string_form_validated_triple() {
    let temp = materialize_fixture();
    write_config(&temp, "[build]\ntarget = \"wasm32-unknown-unknown\"\n");
    let (_, _, platform) = inspect_all(temp.path());
    assert_eq!(
        platform.build_settings.target,
        vec!["wasm32-unknown-unknown".to_string()]
    );
}

#[test]
fn build_target_wrong_table_type_warns() {
    let temp = materialize_fixture();
    // build.target as a table is the wrong type.
    write_config(&temp, "[build]\ntarget = { foo = \"bar\" }\n");
    let (_, _, platform) = inspect_all(temp.path());
    assert!(platform.build_settings.target.is_empty());
    let has_warning = platform.warnings.iter().any(|w| {
        matches!(
            w,
            CargoPlatformWarning::InvalidConfigSetting {
                setting: amari_discovery::ConfigSetting::BuildTarget,
                issue: amari_discovery::ConfigSettingIssue::WrongType,
                ..
            }
        )
    });
    assert!(has_warning, "expected WrongType warning for build.target");
}

#[test]
fn build_target_custom_relative_json_opaque() {
    let temp = materialize_fixture();
    // A relative custom target JSON (path-like, contains '/').
    write_config(
        &temp,
        "[build]\ntarget = [\"targets/my-wasm.json\", \"x86_64-unknown-linux-gnu\"]\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    assert_eq!(
        platform.build_settings.target,
        vec!["x86_64-unknown-linux-gnu".to_string()]
    );
    assert_eq!(platform.build_settings.custom_targets.count, 1);
    let json = serde_json::to_string(&platform).unwrap();
    assert!(
        !json.contains("targets/my-wasm.json"),
        "relative path leaked: {json}"
    );
    assert!(!json.contains("my-wasm"), "basename leaked: {json}");
}

// ===========================================================================
// B8 — Malformed semantic config values: typed warnings
// ===========================================================================

#[test]
fn build_rustflags_wrong_type_warns() {
    let temp = materialize_fixture();
    write_config(&temp, "[build]\nrustflags = { not = \"an array\" }\n");
    let (_, _, platform) = inspect_all(temp.path());
    let has = platform.warnings.iter().any(|w| {
        matches!(
            w,
            CargoPlatformWarning::InvalidConfigSetting {
                setting: amari_discovery::ConfigSetting::BuildRustflags,
                issue: amari_discovery::ConfigSettingIssue::WrongType,
                ..
            }
        )
    });
    assert!(has, "expected BuildRustflags WrongType warning");
}

#[test]
fn runner_empty_string_absent_with_warning() {
    let temp = materialize_fixture();
    write_config(&temp, "[target.'cfg(unix)']\nrunner = \"\"\n");
    let (_, _, platform) = inspect_all(temp.path());
    let unix = platform
        .target_settings
        .iter()
        .find(|t| matches!(t.key, amari_discovery::CargoTargetKey::Cfg { ref display, .. } if display == "cfg(unix)"))
        .expect("cfg(unix) present");
    assert!(
        unix.runner.is_none(),
        "empty runner must be absent, not Some(empty)"
    );
    let has = platform.warnings.iter().any(|w| {
        matches!(
            w,
            CargoPlatformWarning::InvalidConfigSetting {
                setting: amari_discovery::ConfigSetting::TargetTableRunner,
                issue: amari_discovery::ConfigSettingIssue::Empty,
                ..
            }
        )
    });
    assert!(has, "expected TargetTableRunner Empty warning");
}

#[test]
fn runner_wrong_type_warns() {
    let temp = materialize_fixture();
    write_config(&temp, "[target.'cfg(unix)']\nrunner = 12345\n");
    let (_, _, platform) = inspect_all(temp.path());
    let has = platform.warnings.iter().any(|w| {
        matches!(
            w,
            CargoPlatformWarning::InvalidConfigSetting {
                setting: amari_discovery::ConfigSetting::TargetTableRunner,
                issue: amari_discovery::ConfigSettingIssue::WrongType,
                ..
            }
        )
    });
    assert!(has, "expected TargetTableRunner WrongType warning");
    let json = serde_json::to_string(&platform).unwrap();
    assert!(!json.contains("12345"), "wrong-type value leaked: {json}");
}

#[test]
fn linker_wrong_type_warns() {
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.wasm32-unknown-unknown]\nlinker = [\"rust-lld\"]\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    let has = platform.warnings.iter().any(|w| {
        matches!(
            w,
            CargoPlatformWarning::InvalidConfigSetting {
                setting: amari_discovery::ConfigSetting::TargetTableLinker,
                issue: amari_discovery::ConfigSettingIssue::WrongType,
                ..
            }
        )
    });
    assert!(has, "expected TargetTableLinker WrongType warning");
}

#[test]
fn rustflags_mixed_array_warns_and_keeps_valid() {
    let temp = materialize_fixture();
    // Mixed array: one valid string flag + one non-string member.
    write_config(&temp, "[target.'cfg(unix)']\nrustflags = [\"-C\", 99999]\n");
    let (_, _, platform) = inspect_all(temp.path());
    let unix = platform
        .target_settings
        .iter()
        .find(|t| matches!(t.key, amari_discovery::CargoTargetKey::Cfg { ref display, .. } if display == "cfg(unix)"))
        .expect("cfg(unix) present");
    // The valid string flag is still counted.
    assert_eq!(unix.rustflags.flag_count, 1);
    let has = platform.warnings.iter().any(|w| {
        matches!(
            w,
            CargoPlatformWarning::InvalidConfigSetting {
                setting: amari_discovery::ConfigSetting::TargetTableRustflags,
                issue: amari_discovery::ConfigSettingIssue::MixedArray,
                ..
            }
        )
    });
    assert!(has, "expected TargetTableRustflags MixedArray warning");
    let json = serde_json::to_string(&platform).unwrap();
    assert!(!json.contains("99999"), "mixed member value leaked: {json}");
}

#[test]
fn target_table_rustdocflags_wrong_type_warns() {
    let temp = materialize_fixture();
    write_config(&temp, "[target.'cfg(unix)']\nrustdocflags = 42\n");
    let (_, _, platform) = inspect_all(temp.path());
    let has = platform.warnings.iter().any(|w| {
        matches!(
            w,
            CargoPlatformWarning::InvalidConfigSetting {
                setting: amari_discovery::ConfigSetting::TargetTableRustdocflags,
                issue: amari_discovery::ConfigSettingIssue::WrongType,
                ..
            }
        )
    });
    assert!(has, "expected TargetTableRustdocflags WrongType warning");
}

#[test]
fn config_warnings_deterministically_ordered() {
    let temp = materialize_fixture();
    // Multiple distinct (setting, issue) warnings; order must be deterministic
    // and stable across runs by typed sort key.
    write_config(
        &temp,
        "[build]\nrustflags = 1\nrustdocflags = 2\n[target.'cfg(unix)']\nrunner = 3\nlinker = 4\n",
    );
    let p1 = inspect_all(temp.path()).2;
    let p2 = inspect_all(temp.path()).2;
    assert_eq!(
        p1.warnings, p2.warnings,
        "warning order must be deterministic"
    );
}

#[test]
fn empty_linker_warns_and_omitted() {
    // An empty linker string must produce a typed Empty warning and NO
    // ConfiguredLinker evidence (never `Some` with an empty basename).
    let temp = materialize_fixture();
    write_config(&temp, "[target.wasm32-unknown-unknown]\nlinker = \"\"\n");
    let (_, _, platform) = inspect_all(temp.path());
    let ts = platform
        .target_settings
        .iter()
        .find(|t| matches!(&t.key, amari_discovery::CargoTargetKey::Triple { triple } if triple == "wasm32-unknown-unknown"))
        .expect("wasm32 target table");
    assert!(ts.linker.is_none(), "empty linker must yield None");
    assert!(
        platform.warnings.iter().any(|w| matches!(
            w,
            CargoPlatformWarning::InvalidConfigSetting {
                setting: amari_discovery::ConfigSetting::TargetTableLinker,
                issue: amari_discovery::ConfigSettingIssue::Empty,
                ..
            }
        )),
        "empty linker must produce Empty warning"
    );
    // No empty ConfiguredLinker in native_requirements either.
    assert!(
        !platform
            .native_requirements
            .iter()
            .any(|n| matches!(n, NativeRequirement::ConfiguredLinker { basename, .. } if basename.is_empty())),
        "no empty-basename ConfiguredLinker"
    );
}

#[test]
fn empty_runner_program_no_some_empty() {
    // A runner array whose program is empty must NOT yield `Some` with an empty
    // basename; it produces a typed Empty warning and None.
    let temp = materialize_fixture();
    write_config(&temp, "[target.'cfg(unix)']\nrunner = [\"\"]\n");
    let (_, _, platform) = inspect_all(temp.path());
    let ts = platform
        .target_settings
        .iter()
        .find(|t| matches!(&t.key, amari_discovery::CargoTargetKey::Cfg { .. }))
        .expect("cfg(unix) target table");
    assert!(ts.runner.is_none(), "empty-program runner must yield None");
    assert!(
        platform.warnings.iter().any(|w| matches!(
            w,
            CargoPlatformWarning::InvalidConfigSetting {
                setting: amari_discovery::ConfigSetting::TargetTableRunner,
                issue: amari_discovery::ConfigSettingIssue::Empty,
                ..
            }
        )),
        "empty runner program must produce Empty warning"
    );
}

#[test]
fn invalid_config_setting_aggregated_by_sum_across_tables() {
    // The same (setting, issue) across multiple target tables must be SUMMED
    // into a single warning count, not deduped (which underreports).
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.wasm32-unknown-unknown]\nlinker = 1\n[target.x86_64-pc-windows-msvc]\nlinker = 2\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    let linker_wrongtype_total = platform
        .warnings
        .iter()
        .filter_map(|w| match w {
            CargoPlatformWarning::InvalidConfigSetting {
                setting: amari_discovery::ConfigSetting::TargetTableLinker,
                issue: amari_discovery::ConfigSettingIssue::WrongType,
                count,
            } => Some(*count),
            _ => None,
        })
        .sum::<usize>();
    assert_eq!(
        linker_wrongtype_total, 2,
        "two wrong-type linkers across tables must sum to 2, got {linker_wrongtype_total}"
    );
    // Exactly ONE aggregated warning for this (setting, issue).
    let count_of_warnings = platform
        .warnings
        .iter()
        .filter(|w| {
            matches!(
                w,
                CargoPlatformWarning::InvalidConfigSetting {
                    setting: amari_discovery::ConfigSetting::TargetTableLinker,
                    issue: amari_discovery::ConfigSettingIssue::WrongType,
                    ..
                }
            )
        })
        .count();
    assert_eq!(
        count_of_warnings, 1,
        "(setting, issue) must aggregate to one warning, got {count_of_warnings}"
    );
}

// ===========================================================================
// B4-correction — target-table triple validation & non-regular config
// ===========================================================================

#[test]
fn target_triples_reject_empty_segments() {
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[build]\ntarget = [\"wasm32--unknown\"]\n[target.wasm32--unknown]\nlinker = \"rust-lld\"\n",
    );

    let (_, _, platform) = inspect_all(temp.path());
    assert!(platform.build_settings.target.is_empty());
    assert!(platform.target_settings.is_empty());
    assert!(platform.wasm_targets.is_empty());
    assert!(platform.warnings.iter().any(|warning| matches!(
        warning,
        CargoPlatformWarning::InvalidConfigSetting {
            setting: ConfigSetting::BuildTarget,
            issue: ConfigSettingIssue::InvalidValue,
            ..
        }
    )));
    assert!(platform.warnings.iter().any(|warning| matches!(
        warning,
        CargoPlatformWarning::InvalidTargetIdentifier { .. }
    )));
}

#[test]
fn target_table_single_dash_triple_rejected() {
    // A target-table key with only ONE dash is not a valid triple
    // (arch-vendor-os needs >= 2 dashes, matching the build-target rule).
    let temp = materialize_fixture();
    write_config(&temp, "[target.foo-bar]\nlinker = \"clang\"\n");
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::InvalidTargetIdentifier { .. })),
        "single-dash target key must be rejected"
    );
}

#[cfg(unix)]
#[test]
fn non_regular_config_file_unsupported_warning() {
    // A directory at `.cargo/config.toml` must produce a truthful
    // UnsupportedConfigFile warning (not SymlinkedConfig) and Complete with
    // count 0.
    use amari_discovery::CargoPlatformWarning;
    let temp = materialize_fixture();
    // Replace the config file with a directory.
    let cfg_path = temp.path().join(".cargo").join("config.toml");
    fs::remove_file(&cfg_path).unwrap();
    fs::create_dir(&cfg_path).unwrap();
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::UnsupportedConfigFile { .. })),
        "non-regular config must produce UnsupportedConfigFile warning"
    );
    assert!(
        !platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::SymlinkedConfig { .. })),
        "non-regular config must NOT be reported as SymlinkedConfig"
    );
    assert_eq!(platform.config_input.file_count, 0);
}

#[test]
fn target_key_target_feature_atomics_accepted() {
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.'cfg(target_feature = \"+atomics\")']\nlinker = \"rust-lld\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    let present = platform.target_settings.iter().any(|t| {
        matches!(
            t.key,
            amari_discovery::CargoTargetKey::Cfg { ref display, .. }
                if display.contains("target_feature")
        )
    });
    assert!(
        present,
        "cfg(target_feature = \"+atomics\") must be accepted"
    );
    // The value (+atomics) is redacted in display and absent from output.
    let json = serde_json::to_string(&platform).unwrap();
    assert!(!json.contains("+atomics"), "atomics value leaked: {json}");
}

#[test]
fn target_key_nested_all_any_not_accepted() {
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.'cfg(all(target_arch = \"x86_64\", any(target_os = \"linux\", target_os = \"macos\"), not(target_arch = \"wasm32\")))']\nlinker = \"clang\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    let present = platform.target_settings.iter().any(|t| {
        matches!(
            t.key,
            amari_discovery::CargoTargetKey::Cfg { ref display, .. }
                if display.contains("all") && display.contains("any") && display.contains("not")
        )
    });
    assert!(present, "nested cfg(all(any(not(...)))) must be accepted");
}

#[test]
fn target_key_secret_value_redacted_and_distinct() {
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.'cfg(feature = \"SECRET-CFG-TOKEN-XYZ\")']\nlinker = \"rust-lld\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    let key = platform
        .target_settings
        .iter()
        .find(|t| {
            matches!(
                t.key,
                amari_discovery::CargoTargetKey::Cfg { ref display, .. }
                    if display.contains("feature")
            )
        })
        .map(|t| t.key.clone())
        .expect("feature cfg accepted");
    // Secret never appears in display or serialized output.
    if let amari_discovery::CargoTargetKey::Cfg { display, identity } = key {
        assert!(
            !display.contains("SECRET-CFG-TOKEN-XYZ"),
            "display leaked secret"
        );
        assert!(display.contains("<value>"), "value redacted to placeholder");
        assert!(!identity.is_empty(), "distinct identity present");
    }
    let json = serde_json::to_string(&platform).unwrap();
    assert!(
        !json.contains("SECRET-CFG-TOKEN-XYZ"),
        "secret leaked in json: {json}"
    );
}

#[test]
fn target_key_malformed_unbalanced_rejected() {
    let temp = materialize_fixture();
    // Unbalanced parens — must be rejected, not retained.
    write_config(
        &temp,
        "[target.'cfg(target_arch = \"x86_64\"']\nlinker = \"clang\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::InvalidTargetIdentifier { .. })),
        "unbalanced cfg must produce InvalidTargetIdentifier warning"
    );
}

#[test]
fn target_key_injection_rejected() {
    let temp = materialize_fixture();
    // Shell-injection-style content with disallowed chars (`;`, `/`) outside strings.
    write_config(
        &temp,
        "[target.'cfg(target_arch = \"x86_64\"; rm -rf /)']\nlinker = \"clang\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::InvalidTargetIdentifier { .. })),
        "injection-style cfg must be rejected"
    );
    let json = serde_json::to_string(&platform).unwrap();
    assert!(!json.contains("rm -rf"), "injection content leaked: {json}");
}

#[test]
fn target_key_escaped_string_accepted() {
    let temp = materialize_fixture();
    // Escaped quote inside a string value.
    write_config(
        &temp,
        "[target.'cfg(target_env = \"gnu\\\"msvc\")']\nlinker = \"clang\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    let present = platform.target_settings.iter().any(|t| {
        matches!(
            t.key,
            amari_discovery::CargoTargetKey::Cfg { ref display, .. }
                if display.contains("target_env")
        )
    });
    assert!(present, "cfg with escaped string content must be accepted");
}

// ===========================================================================
// B14 — Duplicate target keys normalize deterministically (deduped)
// ===========================================================================

#[test]
fn duplicate_target_keys_deduped() {
    let temp = materialize_fixture();
    // Two distinct TOML table headers (different raw segments) that normalize
    // to the same cfg key: `cfg(unix)` vs `cfg( unix )` (whitespace variant).
    write_config(
        &temp,
        "[target.'cfg(unix)']\nlinker = \"clang\"\n[target.'cfg( unix )']\nlinker = \"gcc\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    let unix_count = platform
        .target_settings
        .iter()
        .filter(|t| {
            matches!(
                t.key,
                amari_discovery::CargoTargetKey::Cfg { ref display, .. } if display == "cfg(unix)"
            )
        })
        .count();
    assert_eq!(
        unix_count, 1,
        "duplicate-normalizing target keys must never leave a duplicate: got {unix_count}"
    );
}

// ===========================================================================
// B2-correction — cfg grammar validation via bounded recursive parser
// ===========================================================================

#[test]
fn target_key_grammar_unquoted_value_rejected() {
    // `cfg(target_arch = x86_64)` (bareword after `=`) is invalid Cargo cfg;
    // values must be string literals. Must be rejected.
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.'cfg(target_arch = x86_64)']\nlinker = \"clang\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::InvalidTargetIdentifier { .. })),
        "unquoted cfg value must be rejected"
    );
}

#[test]
fn target_key_grammar_double_equals_rejected() {
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.'cfg(target_arch == \"x86_64\")']\nlinker = \"clang\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::InvalidTargetIdentifier { .. })),
        "malformed `==` operator must be rejected"
    );
}

#[test]
fn target_key_grammar_not_arity_enforced() {
    // `cfg(not(unix, windows))` has arity > 1 for `not` — invalid.
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.'cfg(not(unix, windows))']\nlinker = \"clang\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::InvalidTargetIdentifier { .. })),
        "`not` with arity != 1 must be rejected"
    );
}

#[test]
fn target_key_grammar_numeric_value_rejected() {
    // Non-string literal value (`64`) is invalid Cargo cfg.
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.'cfg(target_pointer_width = 64)']\nlinker = \"clang\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::InvalidTargetIdentifier { .. })),
        "non-string literal value must be rejected"
    );
}

#[test]
fn target_key_grammar_trailing_junk_rejected() {
    // Missing comma / junk tokens inside cfg → reject.
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.'cfg(unix extra junk)']\nlinker = \"clang\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::InvalidTargetIdentifier { .. })),
        "trailing junk in cfg must be rejected"
    );
}

#[test]
fn target_key_grammar_single_segment_required() {
    // Multi-segment path `cfg(a::b)` must be rejected (only single-segment ids).
    let temp = materialize_fixture();
    write_config(&temp, "[target.'cfg(a::b)']\nlinker = \"clang\"\n");
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::InvalidTargetIdentifier { .. })),
        "multi-segment path must be rejected"
    );
}

#[test]
fn target_key_internal_whitespace_normalized_merges() {
    // `cfg(target_arch = "x86_64")` and `cfg(target_arch="x86_64")` differ
    // only in internal whitespace; they must merge into ONE target table.
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.'cfg(target_arch = \"x86_64\")']\nlinker = \"clang\"\n[target.'cfg(target_arch=\"x86_64\")']\nlinker = \"clang\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    let arch_count = platform
        .target_settings
        .iter()
        .filter(|t| {
            matches!(
                t.key,
                amari_discovery::CargoTargetKey::Cfg { ref display, .. }
                    if display.contains("target_arch")
            )
        })
        .count();
    assert_eq!(
        arch_count, 1,
        "internal-whitespace variants must merge by canonical normalization: got {arch_count}"
    );
}

#[test]
fn duplicate_target_conflicting_settings_warns() {
    // Two raw target selectors that NORMALIZE to the same key but have
    // CONFLICTING settings must emit a typed DuplicateTargetSetting warning
    // (not silently discard). Distinct raw keys avoid a TOML duplicate error.
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.'cfg(unix)']\nlinker = \"clang\"\n[target.'cfg( unix )']\nlinker = \"gcc\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::DuplicateTargetSetting { .. })),
        "conflicting duplicate target settings must produce DuplicateTargetSetting warning"
    );
    // The warning must not leak the linker values or any secret.
    let json = serde_json::to_string(&platform).unwrap();
    assert!(!json.contains("SECRET"), "warning leaked secret: {json}");
}

#[test]
fn duplicate_target_identical_settings_no_conflict_warning() {
    // Two normalized-equal target selectors with IDENTICAL settings must NOT
    // emit a DuplicateTargetSetting warning (silent dedup).
    let temp = materialize_fixture();
    write_config(
        &temp,
        "[target.'cfg(unix)']\nlinker = \"clang\"\n[target.'cfg( unix )']\nlinker = \"clang\"\n",
    );
    let (_, _, platform) = inspect_all(temp.path());
    assert!(
        !platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::DuplicateTargetSetting { .. })),
        "identical duplicate target settings must not warn"
    );
}

// ===========================================================================
// B5 — Native rustflags/config evidence resolves to real ConfigSource
// ===========================================================================

#[test]
fn native_rustflags_build_resolves_to_real_config_source() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    let bytes = fs::read(&cfg).unwrap();
    let expected_hash = sha256_hex(&bytes);
    let (_, _, platform) = inspect_all(temp.path());

    // The build-scope native rustflags requirement must carry the REAL config
    // source (non-empty hash, real byte count), not a hollow placeholder.
    let build_native = platform
        .native_requirements
        .iter()
        .find_map(|n| match n {
            NativeRequirement::NativeRustflags {
                scope: amari_discovery::RustflagsScope::Build,
                config,
                ..
            } => Some(config.clone()),
            _ => None,
        })
        .expect("build-scope native rustflags present");
    assert_eq!(
        build_native.content_hash, expected_hash,
        "build native rustflags must resolve to real config content hash"
    );
    assert_eq!(
        build_native.byte_count,
        bytes.len() as u64,
        "build native rustflags must resolve to real config byte count"
    );
}

// ===========================================================================
// B11 — Native SystemDependency retains target selector context
// ===========================================================================

#[test]
fn system_dependency_retains_target_selector() {
    let temp = TempDir::new().unwrap();
    let version = amari_discovery::Catalog::embedded()
        .unwrap()
        .version()
        .to_string();
    fs::write(
        temp.path().join("Cargo.toml"),
        format!(
            r#"[package]
name = "sysdep-tgt"
version = "0.1.0"
edition = "2021"

[dependencies]
amari-core = "{version}"

[target.'cfg(unix)'.build-dependencies]
cc = "1.0"

[target.'cfg(windows)'.build-dependencies]
pkg-config = "0.3"
"#
        ),
    )
    .unwrap();
    fs::write(
        temp.path().join("Cargo.lock"),
        format!(
            r#"version = 3
[[package]]
name = "amari-core"
version = "{version}"
source = "registry+https://github.com/rust-lang/crates.io-index"
checksum = "abc"
[[package]]
name = "sysdep-tgt"
version = "0.1.0"
"#
        ),
    )
    .unwrap();
    fs::create_dir(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src").join("lib.rs"), "").unwrap();

    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &default_limits()).unwrap();

    let sysdeps: Vec<_> = platform
        .native_requirements
        .iter()
        .filter_map(|n| match n {
            NativeRequirement::SystemDependency {
                package,
                target,
                system_kind,
                ..
            } => Some((package.clone(), target.clone(), *system_kind)),
            _ => None,
        })
        .collect();
    // cc under cfg(unix) retains its target selector.
    let cc = sysdeps
        .iter()
        .find(|(p, _, _)| p == "cc")
        .expect("cc system dependency detected");
    assert_eq!(
        cc.2,
        amari_discovery::SystemDependencyKind::Cc,
        "cc classified"
    );
    assert_eq!(
        cc.1.as_deref(),
        Some("cfg(unix)"),
        "cc target selector context retained"
    );
    // pkg-config under cfg(windows) retains its selector too.
    let pc = sysdeps
        .iter()
        .find(|(p, _, _)| p == "pkg-config")
        .expect("pkg-config system dependency detected");
    assert_eq!(
        pc.1.as_deref(),
        Some("cfg(windows)"),
        "pkg-config target selector context retained"
    );
}

// ===========================================================================
// B13 — no_std attribution uses exact RustFileKind path->package identity
// ===========================================================================

#[test]
fn no_std_attribution_matches_rustfilekind_identity() {
    let temp = materialize_fixture();
    let (_, rust, platform) = inspect_all(temp.path());
    // Build the authoritative path -> package map from RustFileKind.
    let pkg_by_path: std::collections::HashMap<&str, &str> = rust
        .file_kinds
        .iter()
        .map(|fk| {
            let (package, path) = match fk {
                amari_discovery::RustFileKind::Library { package, path }
                | amari_discovery::RustFileKind::Binary { package, path }
                | amari_discovery::RustFileKind::Test { package, path }
                | amari_discovery::RustFileKind::Example { package, path }
                | amari_discovery::RustFileKind::Bench { package, path }
                | amari_discovery::RustFileKind::BuildScript { package, path }
                | amari_discovery::RustFileKind::Other { package, path } => (package, path),
            };
            (path.as_str(), package.as_str())
        })
        .collect();
    // Every no_std source must be attributed to the exact RustFileKind package.
    for pkg in &platform.no_std_evidence.packages {
        for src in &pkg.sources {
            let fk_pkg = pkg_by_path.get(src.path.as_str()).unwrap_or_else(|| {
                panic!(
                    "no_std source {} has no RustFileKind (orphan must be omitted)",
                    src.path
                )
            });
            assert_eq!(
                pkg.package, *fk_pkg,
                "no_std package '{}' must equal RustFileKind package '{}' for {}",
                pkg.package, fk_pkg, src.path
            );
        }
    }
}

// ===========================================================================
// B12 — Benchmark composition follows Cargo targets
// ===========================================================================

fn minimal_pkg_project() -> TempDir {
    let temp = TempDir::new().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        r#"[package]
name = "benchproj"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("Cargo.lock"),
        r#"version = 3
[[package]]
name = "benchproj"
version = "0.1.0"
"#,
    )
    .unwrap();
    fs::create_dir(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src").join("lib.rs"), "").unwrap();
    temp
}

#[test]
fn benchmark_dir_main_rs_conventional() {
    let temp = minimal_pkg_project();
    fs::create_dir_all(temp.path().join("benches").join("mybench")).unwrap();
    fs::write(
        temp.path().join("benches").join("mybench").join("main.rs"),
        "fn main() {}",
    )
    .unwrap();
    let (_, _, platform) = inspect_all(temp.path());
    let bench = platform
        .benchmarks
        .iter()
        .find(|b| b.name == "mybench")
        .expect("dir-style conventional bench mybench");
    assert_eq!(bench.path, "benches/mybench/main.rs");
    assert!(
        matches!(bench.status, BenchmarkStatus::ConventionalUndeclared),
        "dir-style bench is conventional undeclared"
    );
}

#[test]
fn benchmark_helper_not_fabricated() {
    let temp = minimal_pkg_project();
    fs::create_dir_all(temp.path().join("benches").join("mybench")).unwrap();
    fs::write(
        temp.path().join("benches").join("mybench").join("main.rs"),
        "fn main() {}",
    )
    .unwrap();
    // A nested helper module — must NOT be fabricated as a separate benchmark.
    fs::write(
        temp.path()
            .join("benches")
            .join("mybench")
            .join("helper.rs"),
        "pub fn help() {}",
    )
    .unwrap();
    let (_, _, platform) = inspect_all(temp.path());
    let helper_paths: Vec<&str> = platform
        .benchmarks
        .iter()
        .map(|b| b.path.as_str())
        .collect();
    assert!(
        !helper_paths.iter().any(|p| p.contains("helper.rs")),
        "helper file must not be fabricated as a benchmark: {helper_paths:?}"
    );
    assert!(
        !platform.benchmarks.iter().any(|b| b.name == "helper"),
        "helper must not produce a benchmark named 'helper'"
    );
}

#[test]
fn autobenches_false_skips_conventional() {
    let temp = TempDir::new().unwrap();
    fs::write(
        temp.path().join("Cargo.toml"),
        r#"[package]
name = "nobench"
version = "0.1.0"
edition = "2021"
autobenches = false

[[bench]]
name = "declared_bench"
path = "benches/declared_bench.rs"
harness = false
"#,
    )
    .unwrap();
    fs::write(
        temp.path().join("Cargo.lock"),
        r#"version = 3
[[package]]
name = "nobench"
version = "0.1.0"
"#,
    )
    .unwrap();
    fs::create_dir(temp.path().join("src")).unwrap();
    fs::write(temp.path().join("src").join("lib.rs"), "").unwrap();
    fs::create_dir(temp.path().join("benches")).unwrap();
    // A conventional undeclared bench that autobenches=false must suppress.
    fs::write(temp.path().join("benches").join("bench.rs"), "fn main() {}").unwrap();
    fs::write(
        temp.path().join("benches").join("declared_bench.rs"),
        "fn main() {}",
    )
    .unwrap();

    let (_, _, platform) = inspect_all(temp.path());
    // The declared bench is present.
    assert!(
        platform
            .benchmarks
            .iter()
            .any(|b| b.name == "declared_bench"),
        "declared bench present"
    );
    // The conventional undeclared bench.rs is suppressed by autobenches=false.
    assert!(
        !platform
            .benchmarks
            .iter()
            .any(|b| b.path == "benches/bench.rs" && b.declaration_source.is_none()),
        "conventional bench must be suppressed when autobenches=false"
    );
}

#[test]
fn benchmark_retains_harness_and_required_features() {
    // An explicit [[bench]] outside benches/ with harness=false and
    // required-features must be retained with those fields and match the
    // accepted Rust input file at that exact path.
    let temp = minimal_pkg_project();
    fs::create_dir_all(temp.path().join("perf")).unwrap();
    fs::write(temp.path().join("perf").join("custom.rs"), "fn main() {}").unwrap();
    let manifest = fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
    let with_bench = format!(
        "{manifest}\n[[bench]]\nname = \"custom\"\npath = \"perf/custom.rs\"\nharness = false\nrequired-features = [\"zeta\", \"alpha\", \"alpha\"]\n"
    );
    fs::write(temp.path().join("Cargo.toml"), with_bench).unwrap();
    let (_, _, platform) = inspect_all(temp.path());
    let bench = platform
        .benchmarks
        .iter()
        .find(|b| b.name == "custom")
        .expect("custom bench present");
    assert_eq!(bench.path, "perf/custom.rs");
    assert!(
        matches!(bench.status, BenchmarkStatus::DeclaredWithSource),
        "explicit bench outside benches/ matches accepted input file"
    );
    assert!(!bench.harness, "harness=false retained");
    assert_eq!(
        bench.required_features,
        vec!["alpha".to_string(), "zeta".to_string()],
        "required-features sorted and deduped"
    );
}

#[test]
fn benchmark_harness_defaults_true() {
    // A [[bench]] without harness defaults to true.
    let temp = minimal_pkg_project();
    fs::create_dir(temp.path().join("benches")).unwrap();
    fs::write(temp.path().join("benches").join("d.rs"), "fn main() {}").unwrap();
    let manifest = fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
    let with_bench = format!("{manifest}\n[[bench]]\nname = \"d\"\npath = \"benches/d.rs\"\n");
    fs::write(temp.path().join("Cargo.toml"), with_bench).unwrap();
    let (_, _, platform) = inspect_all(temp.path());
    let bench = platform
        .benchmarks
        .iter()
        .find(|b| b.name == "d")
        .expect("d bench present");
    assert!(bench.harness, "harness defaults to true");
    assert!(bench.required_features.is_empty());
}

#[test]
fn benchmark_invalid_path_omitted_and_warned() {
    // A declared bench with an escaping/absolute path must NOT be serialized
    // (no secret/absolute leak) and must emit a typed sanitized warning.
    use amari_discovery::CargoInspectionWarning;
    let temp = minimal_pkg_project();
    let manifest = fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
    let with_bench = format!(
        "{manifest}\n[[bench]]\nname = \"escape\"\npath = \"../../../../etc/SECRET-BENCH-PATH\"\n"
    );
    fs::write(temp.path().join("Cargo.toml"), with_bench).unwrap();
    let (cargo, _, platform) = inspect_all(temp.path());
    // The escaping bench must not appear in evidence.
    assert!(
        !platform.benchmarks.iter().any(|b| b.name == "escape"),
        "escaping bench path must be omitted"
    );
    // A typed warning is emitted (never carrying the raw path).
    assert!(
        cargo
            .warnings
            .iter()
            .any(|w| matches!(w, CargoInspectionWarning::InvalidBenchPath { .. })),
        "escaping bench path must produce InvalidBenchPath warning"
    );
    // No secret/absolute leak in serialized cargo output.
    let json = serde_json::to_string(&cargo).unwrap();
    assert!(!json.contains("SECRET-BENCH-PATH"), "leaked: {json}");
}

#[test]
fn benchmark_absolute_path_omitted() {
    use amari_discovery::CargoInspectionWarning;
    let temp = minimal_pkg_project();
    let manifest = fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
    let with_bench =
        format!("{manifest}\n[[bench]]\nname = \"abs\"\npath = \"/absolute/SECRET-ABS-BENCH\"\n");
    fs::write(temp.path().join("Cargo.toml"), with_bench).unwrap();
    let (cargo, _, platform) = inspect_all(temp.path());
    assert!(!platform.benchmarks.iter().any(|b| b.name == "abs"));
    assert!(cargo
        .warnings
        .iter()
        .any(|w| matches!(w, CargoInspectionWarning::InvalidBenchPath { .. })));
    let json = serde_json::to_string(&cargo).unwrap();
    assert!(!json.contains("SECRET-ABS-BENCH"), "leaked: {json}");
    assert!(
        !json.contains("/absolute/"),
        "absolute prefix leaked: {json}"
    );
}

#[test]
fn benchmark_paths_use_cross_platform_separator_rules() {
    use amari_discovery::CargoInspectionWarning;

    let temp = minimal_pkg_project();
    fs::create_dir(temp.path().join("benches")).unwrap();
    fs::write(
        temp.path().join("benches").join("windows.rs"),
        "fn main() {}",
    )
    .unwrap();
    let manifest = fs::read_to_string(temp.path().join("Cargo.toml")).unwrap();
    let with_benches = format!(
        "{manifest}\n[[bench]]\nname = \"escape-windows\"\npath = '..\\SECRET-WINDOWS-BENCH\\bench.rs'\n\n[[bench]]\nname = \"windows\"\npath = 'benches\\windows.rs'\n"
    );
    fs::write(temp.path().join("Cargo.toml"), with_benches).unwrap();

    let (cargo, _, platform) = inspect_all(temp.path());
    assert!(
        !platform
            .benchmarks
            .iter()
            .any(|bench| bench.name == "escape-windows"),
        "Windows parent traversal must be omitted"
    );
    assert!(cargo
        .warnings
        .iter()
        .any(|warning| matches!(warning, CargoInspectionWarning::InvalidBenchPath { .. })));

    let windows = platform
        .benchmarks
        .iter()
        .find(|bench| bench.name == "windows")
        .expect("valid Windows-separated bench");
    assert_eq!(windows.path, "benches/windows.rs");
    assert!(matches!(
        windows.status,
        BenchmarkStatus::DeclaredWithSource
    ));

    let json = serde_json::to_string(&cargo).unwrap();
    assert!(
        !json.contains("SECRET-WINDOWS-BENCH"),
        "bench path leaked: {json}"
    );
}

// ===========================================================================
// B10 — Platform constraints see ALL target deps (incl. non-Amari)
// ===========================================================================

#[test]
fn non_amari_target_dep_appears_in_constraints() {
    let temp = materialize_fixture();
    // Add a non-Amari dependency under a target table.
    let manifest_path = temp.path().join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    let with_dep = manifest.replace(
        "[features]",
        "[target.'cfg(target_arch = \"wasm32\")'.dependencies]\nserde_json = \"1.0\"\ncc = \"1.0\"\n\n[features]",
    );
    fs::write(&manifest_path, with_dep).unwrap();

    let (_, _, platform) = inspect_all(temp.path());
    // The cfg(target_arch = "wasm32") constraint must exist and trace to a
    // Cargo dependency selector — including the non-Amari serde_json/cc deps.
    let constraint = platform
        .target_cfg_constraints
        .iter()
        .find(|c| c.predicate.contains("target_arch") && c.predicate.contains("wasm32"))
        .expect("cfg(target_arch = wasm32) constraint from non-Amari target dep");
    let packages: Vec<&str> = constraint
        .sources
        .iter()
        .filter_map(|s| match s {
            TargetCfgSource::CargoDependencySelector { package_name, .. } => {
                Some(package_name.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        packages.contains(&"serde_json"),
        "non-Amari serde_json target dep must appear in constraints: {packages:?}"
    );
    assert!(
        packages.contains(&"cc"),
        "non-Amari cc target dep must appear in constraints: {packages:?}"
    );
}

// ===========================================================================
// B1 — Workspace-renamed all-dependency records (Task 8B2 correction)
// ===========================================================================

#[test]
fn workspace_renamed_target_dep_resolves_base_package() {
    // A workspace base `foo = { package = "serde_json" }` plus a member
    // target-specific `[target.'cfg(...)'.dependencies] foo = { workspace = true }`
    // must resolve the canonical Cargo package name `serde_json` (never the
    // alias `foo`, never an illegal member `package` override).
    let temp = materialize_fixture();
    let manifest_path = temp.path().join("Cargo.toml");
    let manifest = fs::read_to_string(&manifest_path).unwrap();
    // Inject a renamed serde_json workspace base + an orphan alias with no
    // package rename. Anchor on the stable `serde` base line (the version
    // placeholder is already substituted by materialize_fixture).
    let with_base = manifest.replace(
        "serde = { version = \"1.0\", features = [\"derive\"] }",
        "serde = { version = \"1.0\", features = [\"derive\"] }\nfoo = { package = \"serde_json\", version = \"1.0\" }\norphan-alias = { version = \"1.0\" }",
    );
    assert!(
        with_base.contains("foo = { package = \"serde_json\""),
        "workspace base substitution failed"
    );
    // Inject a target-specific workspace=true dependency using alias `foo`,
    // plus an illegal member `package` override that must NOT be honored, and
    // a `bar` reference whose base is missing (conservative fallback).
    let with_dep = with_base.replace(
        "[features]",
        "[target.'cfg(target_arch = \"wasm32\")'.dependencies]\nfoo = { workspace = true }\nillegal = { workspace = true, package = \"should-be-ignored\" }\nbar = { workspace = true }\norphan-alias = { workspace = true }\n\n[features]",
    );
    assert!(
        with_dep.contains("foo = { workspace = true }"),
        "target dep substitution failed"
    );
    fs::write(&manifest_path, with_dep).unwrap();

    let (cargo, _, platform) = inspect_all(temp.path());

    // (a) CargoDependencyRecord resolves the canonical package name.
    let foo_rec = cargo
        .root_package
        .dependency_records
        .iter()
        .find(|r| r.alias == "foo")
        .unwrap_or_else(|| {
            panic!(
                "foo dependency record missing: {:?}",
                cargo.root_package.dependency_records
            )
        });
    assert_eq!(
        foo_rec.package, "serde_json",
        "workspace-renamed dep must resolve base package serde_json, not alias foo"
    );
    assert_eq!(
        foo_rec.target.as_deref(),
        Some("cfg(target_arch = \"wasm32\")"),
        "target selector preserved"
    );

    // Illegal member `package` override is NOT honored: it resolves through
    // the base (which has no package, since `illegal` is not a base) → alias.
    let illegal_rec = cargo
        .root_package
        .dependency_records
        .iter()
        .find(|r| r.alias == "illegal");
    if let Some(rec) = illegal_rec {
        assert_ne!(
            rec.package, "should-be-ignored",
            "illegal member package override must not be honored"
        );
    }

    // Missing base (`bar`) falls back conservatively to the alias.
    let bar_rec = cargo
        .root_package
        .dependency_records
        .iter()
        .find(|r| r.alias == "bar")
        .unwrap_or_else(|| panic!("bar dependency record missing"));
    assert_eq!(
        bar_rec.package, "bar",
        "missing workspace base resolves conservatively to alias"
    );

    // (b) The TargetCfgSource for the cfg(target_arch = wasm32) constraint
    // resolves package_name to serde_json (canonical), merged across sources.
    let constraint = platform
        .target_cfg_constraints
        .iter()
        .find(|c| c.predicate.contains("target_arch") && c.predicate.contains("wasm32"))
        .unwrap_or_else(|| panic!("cfg(target_arch = wasm32) constraint missing"));
    let serde_packages: Vec<&str> = constraint
        .sources
        .iter()
        .filter_map(|s| match s {
            TargetCfgSource::CargoDependencySelector { package_name, .. } => {
                Some(package_name.as_str())
            }
            _ => None,
        })
        .collect();
    assert!(
        serde_packages.contains(&"serde_json"),
        "TargetCfgSource must resolve canonical serde_json: {serde_packages:?}"
    );
    assert!(
        !serde_packages.contains(&"foo"),
        "alias foo must never appear as a canonical package name: {serde_packages:?}"
    );
}

// ===========================================================================
// B6 — Sanitized errors: non-dir root never leaks absolute path
// ===========================================================================

#[test]
fn non_dir_platform_root_error_sanitized() {
    // Drive cargo+rust from a valid fixture, then pass a regular file as the
    // platform root. The non-dir error must never embed the absolute path or
    // a sensitive name embedded in that path.
    let temp = materialize_fixture();
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();

    let secret = temp.path().join("SECRET-PROJECT-NAME-XYZ");
    fs::write(&secret, b"not a dir").unwrap();

    let result = inspect_cargo_platform(&secret, &cargo, &rust, &default_limits());
    let err = result.expect_err("non-dir root must error");
    let msg = format!("{err:?}");
    assert!(
        !msg.contains("SECRET-PROJECT-NAME-XYZ"),
        "error leaked sensitive root name: {msg}"
    );
    assert!(
        !msg.contains(temp.path().to_str().unwrap()),
        "error leaked absolute temp path: {msg}"
    );
}

#[test]
fn non_dir_platform_root_error_sanitized_external_symlink() {
    // A malicious external root value must not leak into any warning or the
    // serialized result either.
    let temp = materialize_fixture();
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let secret = temp.path().join("evil-root-value-AAA");
    fs::write(&secret, b"x").unwrap();
    let result = inspect_cargo_platform(&secret, &cargo, &rust, &default_limits());
    if let Err(e) = result {
        let msg = format!("{e:?}");
        assert!(!msg.contains("evil-root-value-AAA"), "leaked: {msg}");
    }
}

/// Mirror of the library's framed input hash for a single config file:
/// u32 LE path len, path bytes, u64 LE content len, content bytes.
fn framed_hash(entries: &[(&str, &[u8])]) -> String {
    let mut sorted: Vec<(&str, &[u8])> = entries.to_vec();
    sorted.sort_by(|a, b| a.0.cmp(b.0));
    let mut hasher = Sha256::new();
    for (path, content) in &sorted {
        hasher.update((path.len() as u32).to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update((content.len() as u64).to_le_bytes());
        hasher.update(content);
    }
    hex::encode(hasher.finalize())
}

#[test]
fn missing_config_with_max_files_zero_is_complete() {
    // A missing config must NOT consume a file-count slot: with max_files == 0
    // the race-safe open is allowed (no content read) to establish existence;
    // a missing config yields Complete + MissingConfig + count 0, NOT a
    // FileCount limit.
    let temp = materialize_fixture();
    fs::remove_file(temp.path().join(".cargo").join("config.toml")).unwrap();
    let mut limits = default_limits();
    limits.max_inspection_files = 0;
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &limits).unwrap();
    assert_eq!(
        platform.state,
        SnapshotState::Complete,
        "missing config + max_files=0 must be Complete (does not consume a slot)"
    );
    assert!(
        platform
            .warnings
            .iter()
            .any(|w| matches!(w, CargoPlatformWarning::MissingConfig { .. })),
        "missing config must produce MissingConfig warning"
    );
    assert_eq!(platform.config_input.file_count, 0);
}

// ===========================================================================
// B2 — Correct per-file vs aggregate limit variants
// ===========================================================================

#[test]
fn per_file_tighter_reports_per_file_bytes_variant() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    let len = fs::read(&cfg).unwrap().len() as u64;

    let mut limits = default_limits();
    // Per-file is the tighter bound (aggregate left generous).
    limits.max_per_file_bytes = len.saturating_sub(1);
    limits.max_inspection_bytes = len * 10;
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &limits).unwrap();
    match platform.state {
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::PerFileBytes { max, observed },
        } => {
            assert_eq!(max, len.saturating_sub(1));
            assert_eq!(
                observed, len,
                "observed must be bounded evidence (max+1), never 0"
            );
        }
        other => panic!("expected PerFileBytes limit, got {other:?}"),
    }
}

#[test]
fn aggregate_tighter_reports_total_bytes_variant() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    let len = fs::read(&cfg).unwrap().len() as u64;

    let mut limits = default_limits();
    // Aggregate is the tighter bound (per-file left generous).
    limits.max_per_file_bytes = len * 10;
    limits.max_inspection_bytes = len.saturating_sub(1);
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &limits).unwrap();
    match platform.state {
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::TotalBytes { max, observed },
        } => {
            assert_eq!(max, len.saturating_sub(1));
            assert_ne!(observed, 0, "observed must never be 0");
            assert_eq!(observed, len, "observed is bounded evidence (max+1)");
        }
        other => panic!("expected TotalBytes limit, got {other:?}"),
    }
}

#[test]
fn zero_byte_config_accepted() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    fs::write(&cfg, b"").unwrap();
    let (_, _, platform) = inspect_all(temp.path());
    assert_eq!(platform.state, SnapshotState::Complete);
    assert_eq!(platform.config_input.file_count, 1);
    assert_eq!(platform.config_input.total_bytes, 0);
    assert_eq!(
        platform.config_input.input_hash,
        framed_hash(&[(".cargo/config.toml", b"")]),
        "zero-byte config framed hash"
    );
}

#[test]
fn exactly_at_per_file_boundary_accepted() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    let len = fs::read(&cfg).unwrap().len() as u64;
    let mut limits = default_limits();
    limits.max_per_file_bytes = len;
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &limits).unwrap();
    assert_eq!(platform.state, SnapshotState::Complete);
    assert_eq!(platform.config_input.file_count, 1);
}

#[test]
fn exactly_at_aggregate_boundary_accepted() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    let len = fs::read(&cfg).unwrap().len() as u64;
    let mut limits = default_limits();
    limits.max_inspection_bytes = len;
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &limits).unwrap();
    assert_eq!(platform.state, SnapshotState::Complete);
    assert_eq!(platform.config_input.file_count, 1);
}

#[test]
fn file_count_zero_observed_is_one() {
    let temp = materialize_fixture();
    let mut limits = default_limits();
    limits.max_inspection_files = 0;
    let cargo = inspect_cargo_project(temp.path(), &default_limits()).unwrap();
    let rust = inspect_rust_sources(temp.path(), &cargo, &default_limits()).unwrap();
    let platform = inspect_cargo_platform(temp.path(), &cargo, &rust, &limits).unwrap();
    match platform.state {
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::FileCount { max, observed },
        } => {
            assert_eq!(max, 0);
            assert_eq!(
                observed, 1,
                "max_files=0 reports observed=1 (considered-file semantics)"
            );
        }
        other => panic!("expected FileCount limit, got {other:?}"),
    }
}

// ===========================================================================
// B15 — Empty input hash exact SHA-256 (no empty-string alternative)
// ===========================================================================

#[test]
fn missing_config_empty_hash_is_exact_sha256() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    fs::remove_file(&cfg).unwrap();
    let (_, _, platform) = inspect_all(temp.path());
    assert!(!platform.config_input.input_hash.is_empty());
    assert_eq!(
        platform.config_input.input_hash,
        sha256_hex(b""),
        "empty config hash is exactly SHA-256 of empty bytes, no alternative"
    );
}

// ===========================================================================
// B15 — Malformed TOML exact 1-based line/column + ConfigSource None skip
// ===========================================================================

#[test]
fn malformed_toml_exact_line_column() {
    let temp = materialize_fixture();
    let cfg = temp.path().join(".cargo").join("config.toml");
    // Syntax error on line 3: `foo =` with no value. toml's span points at
    // the newline where the value was expected (column 6 of line 3).
    let bad = b"[build]\ntarget = \"ok\"\nfoo =\n";
    fs::write(&cfg, bad).unwrap();

    let (_, _, platform) = inspect_all(temp.path());
    let malformed = platform
        .warnings
        .iter()
        .find_map(|w| match w {
            CargoPlatformWarning::MalformedConfig {
                line,
                column,
                reason,
                ..
            } => Some((*line, *column, reason.clone())),
            _ => None,
        })
        .expect("MalformedConfig warning");
    let (line, column, _reason) = malformed;
    assert_eq!(
        line,
        Some(3),
        "malformed TOML line must be exact 1-based (3), got {line:?}"
    );
    assert_eq!(
        column,
        Some(6),
        "malformed TOML column must be exact 1-based (newline after `foo =`), got {column:?}"
    );
}

#[test]
fn config_source_none_line_not_serialized() {
    let temp = materialize_fixture();
    let (_, _, platform) = inspect_all(temp.path());
    // ConfigSource.line is always None for parsed entries; it must be omitted
    // from serialized output (consistent skip-None protocol style), never
    // serialized as null.
    for ts in &platform.target_settings {
        let json = serde_json::to_string(&ts.source).unwrap();
        assert!(
            !json.contains("\"line"),
            "None line must be skipped, not serialized as null: {json}"
        );
    }
}
