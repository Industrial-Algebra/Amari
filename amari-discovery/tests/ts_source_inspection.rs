// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded TypeScript and generated declaration inspection (Task 9B).

use std::fs;
use std::path::Path;

use tempfile::TempDir;

use amari_discovery::{
    inspect_npm_project, inspect_typescript_sources, Catalog, InspectionLimit, InspectionLimits,
    SnapshotState, TypeScriptExportKind, TypeScriptFileRole, TypeScriptImportKind,
    TypeScriptInspectionWarning, TypeScriptRuntimeSignal,
};

const DECLARATION_PATH: &str = "node_modules/@justinelliottcobb/amari-wasm/amari_wasm.d.ts";

fn fixture_source() -> &'static Path {
    Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/ts-project"
    ))
}

fn materialize_fixture() -> TempDir {
    let version = Catalog::embedded().unwrap().version().to_string();
    let temp = TempDir::new().unwrap();
    copy_and_transform(fixture_source(), temp.path(), &version);
    temp
}

fn copy_and_transform(src: &Path, dst: &Path, version: &str) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let name = entry.file_name();
        let name_text = name.to_string_lossy();
        let source = entry.path();
        if source.is_dir() {
            copy_and_transform(&source, &dst.join(name), version);
        } else if name_text == "amari_wasm.d.ts.fixture" {
            let declaration = dst
                .join("node_modules")
                .join("@justinelliottcobb")
                .join("amari-wasm")
                .join("amari_wasm.d.ts");
            fs::create_dir_all(declaration.parent().unwrap()).unwrap();
            fs::copy(source, declaration).unwrap();
        } else if let Some(base) = name_text.strip_suffix(".in") {
            let content = fs::read_to_string(source).unwrap();
            fs::write(
                dst.join(base),
                content.replace("__AMARI_VERSION__", version),
            )
            .unwrap();
        } else {
            fs::copy(source, dst.join(name)).unwrap();
        }
    }
}

fn inspect(root: &Path, limits: &InspectionLimits) -> amari_discovery::TypeScriptInspection {
    let npm = inspect_npm_project(root, limits).unwrap();
    let catalog = Catalog::embedded().unwrap();
    inspect_typescript_sources(root, &npm, &catalog, limits).unwrap()
}

#[test]
fn imports_and_aliases_are_package_scoped() {
    let temp = materialize_fixture();
    let inspection = inspect(temp.path(), &InspectionLimits::default());

    assert!(inspection.imports.iter().any(|import| {
        import.imported_name.as_deref() == Some("WasmMultivector300")
            && import.local_name.as_deref() == Some("Multivector")
            && import.kind == TypeScriptImportKind::Named
            && import.source.path == "src/index.ts"
    }));
    assert!(inspection.imports.iter().any(|import| {
        import.local_name.as_deref() == Some("amari")
            && import.kind == TypeScriptImportKind::Namespace
    }));
    assert!(inspection
        .imports
        .iter()
        .any(|import| import.kind == TypeScriptImportKind::Dynamic));
    assert!(inspection.imports.iter().all(|import| {
        import.package_name == "@justinelliottcobb/amari-wasm"
            && !import.source.content_hash.is_empty()
    }));
}

#[test]
fn javascript_tests_examples_and_vocabulary_are_classified() {
    let temp = materialize_fixture();
    let inspection = inspect(temp.path(), &InspectionLimits::default());

    assert!(inspection.imports.iter().any(|import| {
        import.source.path == "examples/demo.js"
            && import.imported_name.as_deref() == Some("WasmMultivector300")
    }));
    assert!(inspection.file_contexts.iter().any(|context| {
        context.source.path == "examples/demo.js" && context.role == TypeScriptFileRole::Example
    }));
    assert!(inspection.file_contexts.iter().any(|context| {
        context.source.path == "tests/amari.test.ts" && context.role == TypeScriptFileRole::Test
    }));
    assert!(inspection
        .vocabulary
        .iter()
        .any(|evidence| evidence.term == "tropical algebra"));
    assert!(inspection
        .vocabulary
        .iter()
        .any(|evidence| evidence.term == "dual number"));
}

#[test]
fn generated_declaration_exports_are_typed_and_content_addressed() {
    let temp = materialize_fixture();
    let inspection = inspect(temp.path(), &InspectionLimits::default());

    for (name, kind) in [
        ("WasmMultivector300", TypeScriptExportKind::Class),
        ("initSync", TypeScriptExportKind::Function),
        ("TropicalMode", TypeScriptExportKind::Enum),
        ("InitOutput", TypeScriptExportKind::Interface),
        ("InitInput", TypeScriptExportKind::TypeAlias),
    ] {
        assert!(inspection
            .declaration_exports
            .iter()
            .any(|export| export.name == name && export.kind == kind));
    }
    assert!(inspection
        .declaration_exports
        .iter()
        .all(|export| export.source.path == DECLARATION_PATH));
}

#[test]
fn bundler_and_runtime_signals_are_typed() {
    let temp = materialize_fixture();
    let inspection = inspect(temp.path(), &InspectionLimits::default());
    let signals: Vec<_> = inspection
        .runtime_signals
        .iter()
        .map(|evidence| evidence.signal)
        .collect();

    assert!(signals.contains(&TypeScriptRuntimeSignal::Vite));
    assert!(signals.contains(&TypeScriptRuntimeSignal::Browser));
    assert!(signals.contains(&TypeScriptRuntimeSignal::Node));
    assert!(signals.contains(&TypeScriptRuntimeSignal::WebAssembly));
}

#[test]
fn declarations_map_to_shared_semantic_capability_ids() {
    let temp = materialize_fixture();
    let inspection = inspect(temp.path(), &InspectionLimits::default());

    let geometric = inspection
        .capabilities
        .iter()
        .find(|evidence| evidence.wasm_path == "WasmMultivector300.geometricProduct")
        .expect("geometric-product mapping");
    assert_eq!(
        geometric.capability_id.to_string(),
        "amari:amari-core:product:geometric-product"
    );
    assert_eq!(geometric.source.path, DECLARATION_PATH);

    assert!(inspection
        .capabilities
        .iter()
        .any(|evidence| evidence.wasm_path == "WasmRotor300.apply"));
}

#[test]
fn missing_declaration_file_is_typed_and_capabilities_are_not_fabricated() {
    let temp = materialize_fixture();
    fs::remove_file(temp.path().join(DECLARATION_PATH)).unwrap();
    let inspection = inspect(temp.path(), &InspectionLimits::default());

    assert!(inspection.declaration_exports.is_empty());
    assert!(inspection.capabilities.is_empty());
    assert!(inspection.warnings.iter().any(|warning| matches!(
        warning,
        TypeScriptInspectionWarning::MissingDeclarations { path }
            if path == DECLARATION_PATH
    )));
}

#[test]
fn malformed_declarations_are_sanitized() {
    let temp = materialize_fixture();
    fs::write(
        temp.path().join(DECLARATION_PATH),
        "SECRET-DTS-SOURCE not a declaration",
    )
    .unwrap();
    let inspection = inspect(temp.path(), &InspectionLimits::default());

    assert!(inspection.warnings.iter().any(|warning| matches!(
        warning,
        TypeScriptInspectionWarning::MalformedDeclarations { path, .. }
            if path == DECLARATION_PATH
    )));
    let json = serde_json::to_string(&inspection).unwrap();
    assert!(!json.contains("SECRET-DTS-SOURCE"));
}

#[test]
fn file_count_limit_returns_deterministic_partial_evidence() {
    let temp = materialize_fixture();
    let limits = InspectionLimits {
        max_inspection_files: 1,
        ..InspectionLimits::default()
    };
    let inspection = inspect(temp.path(), &limits);

    assert_eq!(inspection.inspected_file_count, 1);
    assert!(matches!(
        inspection.state,
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::FileCount {
                max: 1,
                observed: 2
            }
        }
    ));
}

#[test]
fn oversized_source_is_typed_and_does_not_leak_content() {
    let temp = materialize_fixture();
    fs::write(
        temp.path().join("src").join("large.ts"),
        "SECRET-LARGE-TS".repeat(100),
    )
    .unwrap();
    let limits = InspectionLimits {
        max_per_file_bytes: 512,
        ..InspectionLimits::default()
    };
    let inspection = inspect(temp.path(), &limits);

    assert!(inspection.warnings.iter().any(|warning| matches!(
        warning,
        TypeScriptInspectionWarning::OversizedFile { path, .. }
            if path == "src/large.ts"
    )));
    assert!(!serde_json::to_string(&inspection)
        .unwrap()
        .contains("SECRET-LARGE-TS"));
}

#[test]
fn unrelated_non_source_file_leaves_inspection_equal() {
    let temp = materialize_fixture();
    let before = inspect(temp.path(), &InspectionLimits::default());
    fs::write(temp.path().join("notes.txt"), "SECRET-UNRELATED-NOTE").unwrap();
    let after = inspect(temp.path(), &InspectionLimits::default());

    assert_eq!(before, after);
}

#[test]
fn input_hash_is_root_independent_and_output_has_no_source_text() {
    let first = materialize_fixture();
    let second = materialize_fixture();
    let a = inspect(first.path(), &InspectionLimits::default());
    let b = inspect(second.path(), &InspectionLimits::default());

    assert_eq!(a, b);
    let json = serde_json::to_string(&a).unwrap();
    assert!(!json.contains(first.path().to_str().unwrap()));
    assert!(!json.contains("Browser-side Amari WASM integration"));
    assert!(!json.contains("AMARI_RUNTIME"));
}

#[cfg(unix)]
#[test]
fn symlinked_typescript_source_is_not_followed() {
    use std::os::unix::fs::symlink;

    let temp = materialize_fixture();
    let outside = TempDir::new().unwrap();
    fs::write(
        outside.path().join("secret.ts"),
        "import { SECRET_EXTERNAL } from '@justinelliottcobb/amari-wasm';",
    )
    .unwrap();
    symlink(
        outside.path().join("secret.ts"),
        temp.path().join("src").join("external.ts"),
    )
    .unwrap();

    let inspection = inspect(temp.path(), &InspectionLimits::default());
    assert!(inspection.warnings.iter().any(|warning| matches!(
        warning,
        TypeScriptInspectionWarning::SymlinkedFile { path }
            if path == "src/external.ts"
    )));
    let json = serde_json::to_string(&inspection).unwrap();
    assert!(!json.contains("SECRET_EXTERNAL"));
    assert!(!json.contains(outside.path().to_str().unwrap()));
}

#[test]
fn aggregate_byte_limit_returns_typed_partial_state() {
    let temp = materialize_fixture();
    let limits = InspectionLimits {
        max_inspection_bytes: 600,
        ..InspectionLimits::default()
    };
    let inspection = inspect(temp.path(), &limits);
    assert!(matches!(
        inspection.state,
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::TotalBytes {
                max: 600,
                observed: 601
            }
        }
    ));
    assert!(inspection.total_bytes <= 600);
}

#[test]
fn invalid_utf8_source_is_content_addressed_without_leaking_bytes() {
    let temp = materialize_fixture();
    fs::write(
        temp.path().join("src").join("invalid.ts"),
        [0xff, 0xfe, 0x41],
    )
    .unwrap();
    let inspection = inspect(temp.path(), &InspectionLimits::default());
    assert!(inspection.warnings.iter().any(|warning| matches!(warning,
        TypeScriptInspectionWarning::InvalidUtf8Source { path, content_hash }
            if path == "src/invalid.ts" && !content_hash.is_empty()
    )));
}

#[cfg(unix)]
#[test]
fn symlinked_declaration_package_directory_is_not_followed() {
    use std::os::unix::fs::symlink;

    let temp = materialize_fixture();
    let package_dir = temp
        .path()
        .join("node_modules/@justinelliottcobb/amari-wasm");
    fs::remove_dir_all(&package_dir).unwrap();
    let outside = TempDir::new().unwrap();
    fs::write(
        outside.path().join("amari_wasm.d.ts"),
        "export class SECRET_EXTERNAL_DECLARATION {}",
    )
    .unwrap();
    symlink(outside.path(), &package_dir).unwrap();

    let inspection = inspect(temp.path(), &InspectionLimits::default());
    assert!(inspection.warnings.iter().any(|warning| matches!(warning,
        TypeScriptInspectionWarning::SymlinkedFile { path } if path == DECLARATION_PATH
    )));
    let json = serde_json::to_string(&inspection).unwrap();
    assert!(!json.contains("SECRET_EXTERNAL_DECLARATION"));
    assert!(!json.contains(outside.path().to_str().unwrap()));
}
