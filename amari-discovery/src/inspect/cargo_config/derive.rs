// SPDX-License-Identifier: MIT OR Apache-2.0

//! Evidence derivation for Cargo platform inspection.
//!
//! Derives benchmark, `no_std`, target cfg, WASM, and native/linker
//! requirements purely from already-computed
//! [`crate::inspect::CargoInspection`] / [`crate::inspect::RustSourceInspection`]
//! evidence and the parsed `.cargo/config.toml` settings. No manifest or
//! Rust source file is re-read, and nothing is executed.

use std::collections::BTreeMap;
use std::path::Path;

use crate::inspect::cargo::CargoInspection;
use crate::inspect::rust::types::RustFileKind;
use crate::inspect::snapshot::SourceLocation;
use crate::inspect::{DependencyKind, RustSourceInspection, SystemDependencyKind};

use super::types::{
    BenchmarkEvidence, BenchmarkStatus, CargoBuildSettings, CargoTargetKey, CargoTargetSettings,
    ConfigSource, NativeRequirement, NoStdEvidence, NoStdPackageEvidence, RustflagsEvidence,
    RustflagsScope as ScopeType, TargetCfgConstraint, TargetCfgSource, WasmTargetEvidence,
    WasmTargetOrigin,
};

// ===========================================================================
// WASM targets
// ===========================================================================

/// Returns `true` for a validated WASM target triple (`wasm32-*` or
/// `wasm64-*`). Build targets and target-table triples are already validated
/// by the parser; this only classifies the validated result.
fn is_wasm_target_triple(triple: &str) -> bool {
    triple.starts_with("wasm32-") || triple.starts_with("wasm64-")
}

/// Derive configured WASM targets from build settings and target tables.
///
/// Collects every validated `wasm32-*`/`wasm64-*` build target and
/// target-table triple key, deduplicating by target and preserving sorted,
/// deduplicated origins and direct [`ConfigSource`] provenance.
pub(super) fn derive_wasm_targets(
    build: &CargoBuildSettings,
    targets: &[CargoTargetSettings],
) -> Vec<WasmTargetEvidence> {
    // (target -> (origins, sources))
    let mut by_target: BTreeMap<String, (Vec<WasmTargetOrigin>, Vec<ConfigSource>)> =
        BTreeMap::new();

    for t in &build.target {
        if is_wasm_target_triple(t) {
            let (origins, sources) = by_target.entry(t.clone()).or_default();
            origins.push(WasmTargetOrigin::BuildTarget);
            if let Some(src) = &build.source {
                sources.push(src.clone());
            }
        }
    }
    for ts in targets {
        if let CargoTargetKey::Triple { triple } = &ts.key {
            if is_wasm_target_triple(triple) {
                let (origins, sources) = by_target.entry(triple.clone()).or_default();
                origins.push(WasmTargetOrigin::TargetTable);
                sources.push(ts.source.clone());
            }
        }
    }

    let mut out: Vec<WasmTargetEvidence> = by_target
        .into_iter()
        .map(|(target, (mut origins, mut sources))| {
            origins.sort();
            origins.dedup();
            sources.sort();
            sources.dedup();
            WasmTargetEvidence {
                target,
                origins,
                sources,
            }
        })
        .collect();
    out.sort_by(|a, b| a.target.cmp(&b.target));
    out
}

// ===========================================================================
// Native requirements
// ===========================================================================

/// Derive native/linker requirements from Cargo manifests and config.
pub(super) fn derive_native_requirements(
    cargo: &CargoInspection,
    targets: &[CargoTargetSettings],
    build: &CargoBuildSettings,
) -> Vec<NativeRequirement> {
    let mut out: Vec<NativeRequirement> = Vec::new();

    // Cargo package.links + system dependency signals (root + members).
    for pkg in std::iter::once(&cargo.root_package).chain(cargo.workspace_members.iter()) {
        if let Some(link) = &pkg.native_link {
            out.push(NativeRequirement::CargoLinks {
                links_key: link.links_key.clone(),
                package: pkg.name.clone(),
                manifest_path: pkg.manifest_path.clone(),
                source: link.manifest_source.clone(),
            });
        }
        for sys in &pkg.system_dependencies {
            out.push(NativeRequirement::SystemDependency {
                alias: sys.alias.clone(),
                package: sys.package.clone(),
                system_kind: sys.system_kind,
                dependency_kind: sys.dependency_kind,
                target: sys.target.clone(),
                source: sys.manifest_source.clone(),
            });
        }
    }

    // Configured target linkers.
    for ts in targets {
        if let Some(linker) = &ts.linker {
            out.push(NativeRequirement::ConfiguredLinker {
                target_key: ts.key.clone(),
                basename: linker.basename.clone(),
                config: ts.source.clone(),
            });
        }
    }

    // Native/link-affecting rustflags (build + each target scope).
    push_native_rustflags(
        &mut out,
        &build.rustflags,
        ScopeType::Build,
        build.source.as_ref(),
    );
    for ts in targets {
        push_native_rustflags(
            &mut out,
            &ts.rustflags,
            ScopeType::Target {
                key: ts.key.clone(),
            },
            Some(&ts.source),
        );
    }

    sort_dedup_native(&mut out);
    out
}

fn push_native_rustflags(
    out: &mut Vec<NativeRequirement>,
    rf: &RustflagsEvidence,
    scope: ScopeType,
    config: Option<&ConfigSource>,
) {
    if !rf.has_native_linking || rf.native_flag_count == 0 {
        return;
    }
    let Some(config) = config else {
        // No accepted config source → no native rustflag requirement can be
        // attributed. (Build settings with no accepted config have no flags.)
        return;
    };
    out.push(NativeRequirement::NativeRustflags {
        scope,
        flag_count: rf.native_flag_count,
        identity: rf.native_identity.clone(),
        config: config.clone(),
    });
}

fn sort_dedup_native(out: &mut Vec<NativeRequirement>) {
    out.sort_by(native_cmp);
    out.dedup();
}

/// Typed comparison for [`NativeRequirement`], never relying on `Debug`
/// formatting. Variants are ordered by a fixed discriminant; within a variant
/// by typed fields (package/alias/basename, enum ranks via stable matches,
/// target-key/scope typed representations).
fn native_cmp(a: &NativeRequirement, b: &NativeRequirement) -> std::cmp::Ordering {
    let ka = native_sort_key(a);
    let kb = native_sort_key(b);
    ka.cmp(&kb)
}

/// Typed sort key tuple for a native requirement (no `Debug` strings).
fn native_sort_key(nr: &NativeRequirement) -> (u8, String, String, u8, u8, String, u64) {
    match nr {
        NativeRequirement::CargoLinks {
            links_key,
            package,
            manifest_path,
            ..
        } => (
            0,
            package.clone(),
            links_key.clone(),
            0,
            0,
            manifest_path.clone(),
            0,
        ),
        NativeRequirement::SystemDependency {
            alias,
            package,
            system_kind,
            dependency_kind,
            source,
            ..
        } => (
            1,
            package.clone(),
            alias.clone(),
            system_dep_kind_rank(*system_kind),
            dep_kind_rank(*dependency_kind),
            source.path.clone(),
            0,
        ),
        NativeRequirement::ConfiguredLinker {
            target_key,
            basename,
            config,
            ..
        } => {
            let (kr, ks) = target_key_sort_repr(target_key);
            (2, ks, basename.clone(), kr, 0, config.path.clone(), 0)
        }
        NativeRequirement::NativeRustflags {
            scope,
            flag_count,
            config,
            ..
        } => {
            let (sr, ss) = scope_sort_repr(scope);
            (
                3,
                ss,
                String::new(),
                sr,
                0,
                config.path.clone(),
                *flag_count as u64,
            )
        }
    }
}

/// Stable rank for [`SystemDependencyKind`] (definition order).
fn system_dep_kind_rank(k: SystemDependencyKind) -> u8 {
    match k {
        SystemDependencyKind::PkgConfig => 0,
        SystemDependencyKind::Cc => 1,
        SystemDependencyKind::Bindgen => 2,
        SystemDependencyKind::Cmake => 3,
        SystemDependencyKind::Vcpkg => 4,
        SystemDependencyKind::SystemDeps => 5,
    }
}

/// Stable rank for [`DependencyKind`] (Normal < Dev < Build).
fn dep_kind_rank(k: DependencyKind) -> u8 {
    match k {
        DependencyKind::Normal => 0,
        DependencyKind::Dev => 1,
        DependencyKind::Build => 2,
    }
}

/// Typed `(rank, key-string)` representation of a [`CargoTargetKey`] for
/// deterministic sorting (no `Debug`).
fn target_key_sort_repr(key: &CargoTargetKey) -> (u8, String) {
    match key {
        CargoTargetKey::Triple { triple } => (0, triple.clone()),
        CargoTargetKey::Cfg { display, identity } => {
            // Unit separator delimits display from identity; both fields are
            // bounded/redacted so this is a typed construction, not Debug.
            (1, format!("{display}\u{1f}{identity}"))
        }
    }
}

/// Typed `(rank, repr)` representation of a [`RustflagsScope`] for
/// deterministic sorting (no `Debug`).
fn scope_sort_repr(scope: &ScopeType) -> (u8, String) {
    match scope {
        ScopeType::Build => (0, String::new()),
        ScopeType::Target { key } => {
            let (kr, ks) = target_key_sort_repr(key);
            (1, format!("{kr}\u{1f}{ks}"))
        }
    }
}

// ===========================================================================
// Benchmarks — compose Cargo declarations and Rust source classifications
// ===========================================================================

/// Derive package-scoped benchmark evidence by joining Cargo `[[bench]]`
/// declarations with Rust `benches/**/*.rs` classifications.
pub(super) fn derive_benchmarks(
    cargo: &CargoInspection,
    rust: &RustSourceInspection,
) -> Vec<BenchmarkEvidence> {
    // Index accepted input files by project-relative path → content hash.
    let mut input_by_path: BTreeMap<&str, &SourceLocation> = BTreeMap::new();
    for f in &rust.input_files {
        input_by_path.insert(f.path.as_str(), f);
    }

    let mut out: Vec<BenchmarkEvidence> = Vec::new();

    for pkg in std::iter::once(&cargo.root_package).chain(cargo.workspace_members.iter()) {
        let pkg_dir = package_dir(&pkg.manifest_path);
        // `autobenches = false` suppresses conventional discovery; only
        // explicit `[[bench]]` targets are considered for this package.
        let conventional_enabled = pkg.autobenches != Some(false);

        // Conventional bench source files for this package (RustFileKind::Bench).
        let mut source_bench_paths: Vec<&str> = Vec::new();
        if conventional_enabled {
            for fk in &rust.file_kinds {
                if let RustFileKind::Bench { package, path } = fk {
                    if package == &pkg.name {
                        source_bench_paths.push(path.as_str());
                    }
                }
            }
            source_bench_paths.sort();
            source_bench_paths.dedup();
        }

        // Declared benches → join. An explicit [[bench]] path matches ANY
        // exact accepted Rust input file (even outside `benches/`), not just
        // RustFileKind::Bench classifications.
        let mut declared_project_paths: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        for bench in &pkg.benches {
            let project_path = join_pkg_path(pkg_dir, &bench.path);
            declared_project_paths.insert(project_path.clone());
            if input_by_path.contains_key(project_path.as_str()) {
                let src = input_by_path
                    .get(project_path.as_str())
                    .map(|loc| (*loc).clone());
                out.push(BenchmarkEvidence {
                    package: pkg.name.clone(),
                    name: bench.name.clone(),
                    path: project_path,
                    status: BenchmarkStatus::DeclaredWithSource,
                    harness: bench.harness,
                    required_features: bench.required_features.clone(),
                    source: src,
                    declaration_source: Some(bench.manifest_source.clone()),
                });
            } else {
                out.push(BenchmarkEvidence {
                    package: pkg.name.clone(),
                    name: bench.name.clone(),
                    path: project_path.clone(),
                    status: BenchmarkStatus::DeclaredMissingSource {
                        declared_path: bench.path.clone(),
                    },
                    harness: bench.harness,
                    required_features: bench.required_features.clone(),
                    source: None,
                    declaration_source: Some(bench.manifest_source.clone()),
                });
            }
        }

        // Conventional bench sources with no declaration.
        for &sp in &source_bench_paths {
            if !declared_project_paths.contains(sp) {
                let name = conventional_bench_name(sp).unwrap_or_else(|| {
                    Path::new(sp)
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("bench")
                        .to_string()
                });
                let src = input_by_path.get(sp).map(|loc| (*loc).clone());
                out.push(BenchmarkEvidence {
                    package: pkg.name.clone(),
                    name,
                    path: sp.to_string(),
                    status: BenchmarkStatus::ConventionalUndeclared,
                    harness: true,
                    required_features: Vec::new(),
                    source: src,
                    declaration_source: None,
                });
            }
        }
    }

    out.sort();
    out.dedup();
    out
}

/// Strip `/Cargo.toml` from a manifest path to get the package directory
/// (empty for the root package).
fn package_dir(manifest_path: &str) -> &str {
    if let Some(dir) = manifest_path.strip_suffix("/Cargo.toml") {
        dir
    } else if let Some(dir) = manifest_path.strip_suffix("\\Cargo.toml") {
        dir
    } else if manifest_path == "Cargo.toml" || manifest_path == "\\Cargo.toml" {
        ""
    } else {
        manifest_path
    }
}

/// Join a package directory with a package-relative bench path.
fn join_pkg_path(pkg_dir: &str, rel: &str) -> String {
    if pkg_dir.is_empty() {
        rel.to_string()
    } else {
        format!("{pkg_dir}/{rel}")
    }
}

/// Derive the conventional Cargo bench name from a project-relative bench
/// ROOT path. `benches/<name>.rs` -> `<name>`; `benches/<name>/main.rs` ->
/// `<name>`. Returns `None` for non-root paths (helpers, deeper nesting).
fn conventional_bench_name(project_path: &str) -> Option<String> {
    let parts: Vec<&str> = project_path.split('/').collect();
    let idx = parts.iter().position(|p| *p == "benches")?;
    let after: &[&str] = &parts[idx + 1..];
    match after {
        [single] => single.strip_suffix(".rs").map(|s| s.to_string()),
        [dir, file] if *file == "main.rs" && !dir.is_empty() => Some((*dir).to_string()),
        _ => None,
    }
}

// ===========================================================================
// no_std evidence
// ===========================================================================

/// Derive `no_std` evidence from literal `#![no_std]` crate attributes.
///
/// Package attribution uses the **exact** path→package identity recorded in
/// [`RustFileKind`] (the authoritative assignment from the Rust inspector),
/// never a re-derived prefix match or a fallback to the root package. A
/// `#![no_std]` attribute whose file has no `RustFileKind` (an orphan) is
/// omitted conservatively rather than misattributed.
pub(super) fn derive_no_std(
    _cargo: &CargoInspection,
    rust: &RustSourceInspection,
) -> NoStdEvidence {
    // Authoritative path -> package from RustFileKind (exact identity).
    let mut pkg_by_path: BTreeMap<&str, &str> = BTreeMap::new();
    for fk in &rust.file_kinds {
        let (package, path) = match fk {
            RustFileKind::Library { package, path }
            | RustFileKind::Binary { package, path }
            | RustFileKind::Test { package, path }
            | RustFileKind::Example { package, path }
            | RustFileKind::Bench { package, path }
            | RustFileKind::BuildScript { package, path }
            | RustFileKind::Other { package, path } => (package, path),
        };
        pkg_by_path.insert(path.as_str(), package.as_str());
    }

    let mut by_pkg: BTreeMap<String, Vec<SourceLocation>> = BTreeMap::new();

    for attr in &rust.crate_attributes {
        if attr.attribute != "no_std" {
            continue;
        }
        // Exact identity: attribute must trace to a classified RustFileKind.
        let Some(&pkg_name) = pkg_by_path.get(attr.path.as_str()) else {
            // Orphan: not attributable to a known package — omit conservatively.
            continue;
        };
        let sources = by_pkg.entry(pkg_name.to_string()).or_default();
        if let Some(src) = &attr.source {
            sources.push(src.clone());
        } else if let Some(loc) = rust.input_files.iter().find(|f| f.path == attr.path) {
            sources.push(SourceLocation {
                path: loc.path.clone(),
                line: None,
                column: None,
                content_hash: loc.content_hash.clone(),
            });
        }
    }

    let mut packages: Vec<NoStdPackageEvidence> = by_pkg
        .into_iter()
        .map(|(package, mut sources)| {
            sources.sort();
            sources.dedup();
            NoStdPackageEvidence { package, sources }
        })
        .collect();
    packages.sort_by(|a, b| a.package.cmp(&b.package));

    let has_no_std = !packages.is_empty();
    NoStdEvidence {
        has_no_std,
        packages,
    }
}

// ===========================================================================
// Target cfg constraints
// ===========================================================================

/// Derive target cfg constraints from Cargo target selectors and Rust
/// platform cfg/cfg_attr predicates.
pub(super) fn derive_target_cfg(
    cargo: &CargoInspection,
    rust: &RustSourceInspection,
) -> Vec<TargetCfgConstraint> {
    let mut by_pred: BTreeMap<String, Vec<TargetCfgSource>> = BTreeMap::new();

    // Cargo target-specific dependency selectors (cfg(...) only) — across ALL
    // dependencies (Amari and non-Amari) via dependency_records, so platform
    // constraints see non-Amari target deps too. No manifest re-read. The
    // `cfg(...)` selector is normalized to the same canonical UNWRAPPED form
    // as Rust cfg predicates (inner body, whitespace-collapsed) so equivalent
    // Cargo/Rust constraints (e.g. `target_arch = "wasm32"`) merge sources.
    for pkg in std::iter::once(&cargo.root_package).chain(cargo.workspace_members.iter()) {
        for dep in &pkg.dependency_records {
            if let Some(target) = &dep.target {
                if target.starts_with("cfg(") {
                    let predicate = normalize_cargo_cfg_selector(target);
                    by_pred.entry(predicate).or_default().push(
                        TargetCfgSource::CargoDependencySelector {
                            manifest_path: pkg.manifest_path.clone(),
                            alias: dep.alias.clone(),
                            package_name: dep.package.clone(),
                            source: dep.source.clone(),
                        },
                    );
                }
            }
        }
    }

    // Rust cfg/cfg_attr platform predicates.
    for cfg in &rust.cfg_evidence {
        if is_platform_cfg(&cfg.cfg_predicate) {
            if let Some(src) = &cfg.source {
                by_pred.entry(cfg.cfg_predicate.clone()).or_default().push(
                    TargetCfgSource::RustAttribute {
                        path: cfg.path.clone(),
                        is_cfg_attr: cfg.is_cfg_attr,
                        source: src.clone(),
                    },
                );
            }
        }
    }

    let mut out: Vec<TargetCfgConstraint> = by_pred
        .into_iter()
        .map(|(predicate, mut sources)| {
            sources.sort();
            sources.dedup();
            TargetCfgConstraint { predicate, sources }
        })
        .collect();
    out.sort_by(|a, b| a.predicate.cmp(&b.predicate));
    out
}

/// Normalize a Cargo `[target.'cfg(...)']` dependency selector to the same
/// canonical UNWRAPPED representation used for Rust cfg predicates: strip the
/// `cfg(...)` wrapper and collapse internal whitespace (matching Rust's
/// `split_whitespace().join(" ")`). Non-cfg selectors (target triples) are
/// returned unchanged. Values are preserved per the existing extracted-signal
/// contract (cfg constraint values like `"wasm32"` are useful signals, not
/// secrets).
///
/// A malformed/unbalanced `cfg(` selector that cannot be unwrapped is returned
/// unchanged so it never silently merges with a distinct predicate.
fn normalize_cargo_cfg_selector(target: &str) -> String {
    if let Some(inner) = target
        .strip_prefix("cfg(")
        .and_then(|s| s.strip_suffix(')'))
    {
        inner.split_whitespace().collect::<Vec<_>>().join(" ")
    } else {
        target.to_string()
    }
}

/// A cfg predicate is a platform constraint if it mentions any `target_*`
/// option or a bare `unix`/`windows` option name (outside quoted strings).
///
/// Quoted-string contents are always stripped before matching, so a feature
/// whose value happens to contain a `target_*` key (e.g.
/// `feature = "target_arch"`) is never mistaken for a platform constraint.
fn is_platform_cfg(predicate: &str) -> bool {
    let lower = predicate.to_ascii_lowercase();
    // Strip quoted-string contents first so values can never masquerade as
    // platform option names.
    let stripped = strip_quoted(&lower);
    const TARGET_KEYS: &[&str] = &[
        "target_arch",
        "target_os",
        "target_family",
        "target_env",
        "target_vendor",
        "target_pointer_width",
        "target_endian",
        "target_has_atomic",
    ];
    stripped
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .any(|word| TARGET_KEYS.contains(&word) || word == "unix" || word == "windows")
}

/// Strip quoted-string contents from `s`, replacing each character inside a
/// double-quoted string (and the quotes themselves) with spaces.
///
/// Escape-aware: a backslash inside a string escapes the next character, so
/// an escaped quote (`\"`) does NOT close the string. This prevents a quoted
/// value containing `\"` followed by a target key from leaking the key as a
/// bareword platform option.
fn strip_quoted(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_str = false;
    while let Some(ch) = chars.next() {
        if in_str {
            if ch == '\\' {
                // Escaped next char: consume it, redact both backslash and
                // the escaped character so neither can close the string.
                chars.next();
                out.push(' ');
                out.push(' ');
                continue;
            }
            if ch == '"' {
                in_str = false;
                out.push(' ');
                continue;
            }
            out.push(' ');
        } else if ch == '"' {
            in_str = true;
            out.push(' ');
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::is_platform_cfg;

    #[test]
    fn quoted_target_key_value_is_not_platform() {
        // A feature whose value is literally "target_arch" must NOT count as
        // a platform constraint — the key appears only inside a quoted value.
        assert!(
            !is_platform_cfg(r#"cfg(feature = "target_arch")"#),
            "quoted target_arch value must not be a platform constraint"
        );
        assert!(
            !is_platform_cfg(r#"feature = "target_os""#),
            "quoted target_os value must not be a platform constraint"
        );
    }

    #[test]
    fn platform_option_names_require_exact_tokens() {
        assert!(
            !is_platform_cfg(r#"cfg(not_target_arch = "secret")"#),
            "a custom cfg key containing target_arch is not a platform option"
        );
        assert!(
            !is_platform_cfg(r#"target_architecture = "custom""#),
            "target_architecture must not match target_arch"
        );
    }

    #[test]
    fn nested_platform_predicates_positive() {
        assert!(
            is_platform_cfg(r#"cfg(all(target_arch = "x86_64", target_os = "linux"))"#),
            "nested all() with target keys is a platform constraint"
        );
        assert!(
            is_platform_cfg(r#"cfg(not(target_arch = "wasm32"))"#),
            "not() target predicate is a platform constraint"
        );
        assert!(
            is_platform_cfg(r#"cfg(any(target_os = "macos", target_os = "linux"))"#),
            "any() target predicate is a platform constraint"
        );
    }

    #[test]
    fn bare_unix_windows_positive() {
        assert!(is_platform_cfg("cfg(unix)"));
        assert!(is_platform_cfg("cfg(windows)"));
    }

    #[test]
    fn escaped_quote_in_value_is_not_platform() {
        // A quoted value containing an escaped quote followed by a target key
        // must NOT leak the target key outside the string. Without
        // escape-aware stripping, `\"` would close the string early and the
        // `target_os` token would appear as a bareword platform key.
        assert!(
            !is_platform_cfg(r#"cfg(feature = "a\"target_os\"")"#),
            "escaped-quote target_os value must not be a platform constraint"
        );
        assert!(
            !is_platform_cfg(r#"feature = \"target_arch\""#),
            "escaped leading-quote target_arch must not be a platform constraint"
        );
    }

    #[test]
    fn strip_quoted_handles_escaped_quotes() {
        // Direct check of the helper: an escaped quote inside a value does not
        // close the string, so a target key following it stays redacted.
        use super::strip_quoted;
        let stripped = strip_quoted(r#"feature = "a\"target_os""#);
        assert!(
            !stripped.contains("target_os"),
            "escaped-quote value leaked target_os: {stripped}"
        );
    }
}
