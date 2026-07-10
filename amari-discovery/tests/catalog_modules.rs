// SPDX-License-Identifier: MIT OR Apache-2.0

use std::{
    fs,
    path::{Path, PathBuf},
};

use amari_discovery::catalog::generator::{
    module_graph, ModuleGraph, ModuleKind, ModuleRecord, ModuleVisibility,
};
use tempfile::TempDir;

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/module-graph")
}

fn by_path(graph: &ModuleGraph) -> std::collections::HashMap<&str, &ModuleRecord> {
    graph
        .modules
        .iter()
        .map(|record| (record.path.as_str(), record))
        .collect()
}

fn write_package(root: &Path, files: &[(&str, &str)]) {
    for (relative, source) in files {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, source).unwrap();
    }
}

#[test]
fn fixture_graph_maps_external_inline_path_and_visibility() {
    let graph = module_graph(&fixture_root(), "src/lib.rs").unwrap();
    let modules = by_path(&graph);

    assert_eq!(graph.modules.len(), 8);
    assert!(
        graph
            .modules
            .windows(2)
            .all(|pair| pair[0].path < pair[1].path),
        "modules must be sorted by canonical path"
    );

    let root = graph.root().unwrap();
    assert_eq!(root.path, "crate");
    assert_eq!(root.kind, ModuleKind::Crate);
    assert_eq!(root.visibility, ModuleVisibility::Public);
    assert_eq!(root.source_path.as_deref(), Some("src/lib.rs"));
    assert!(root.parent.is_none());

    let external = modules["crate::external_file"];
    assert_eq!(external.kind, ModuleKind::File);
    assert_eq!(external.visibility, ModuleVisibility::Public);
    assert_eq!(
        external.source_path.as_deref(),
        Some("src/external_file.rs")
    );
    assert_eq!(external.parent.as_deref(), Some("crate"));

    let nested = modules["crate::nested"];
    assert_eq!(nested.kind, ModuleKind::File);
    assert_eq!(nested.visibility, ModuleVisibility::Private);
    assert_eq!(nested.source_path.as_deref(), Some("src/nested/mod.rs"));
    assert_eq!(nested.parent.as_deref(), Some("crate"));

    let leaf = modules["crate::nested::leaf"];
    assert_eq!(leaf.kind, ModuleKind::File);
    assert_eq!(leaf.visibility, ModuleVisibility::Public);
    assert_eq!(leaf.source_path.as_deref(), Some("src/nested/leaf.rs"));
    assert_eq!(leaf.parent.as_deref(), Some("crate::nested"));

    let aliased = modules["crate::aliased"];
    assert_eq!(aliased.kind, ModuleKind::File);
    assert_eq!(aliased.visibility, ModuleVisibility::Private);
    assert_eq!(
        aliased.source_path.as_deref(),
        Some("src/custom/aliased.rs")
    );
    assert_eq!(aliased.parent.as_deref(), Some("crate"));

    let restricted = modules["crate::restricted"];
    assert_eq!(restricted.kind, ModuleKind::File);
    assert_eq!(restricted.visibility, ModuleVisibility::Restricted);
    assert_eq!(restricted.source_path.as_deref(), Some("src/restricted.rs"));

    let host = modules["crate::inline_host"];
    assert_eq!(host.kind, ModuleKind::Inline);
    assert_eq!(host.visibility, ModuleVisibility::Private);
    assert!(host.source_path.is_none());
    assert_eq!(host.parent.as_deref(), Some("crate"));

    let inner = modules["crate::inline_host::inner"];
    assert_eq!(inner.kind, ModuleKind::Inline);
    assert_eq!(inner.visibility, ModuleVisibility::Public);
    assert!(inner.source_path.is_none());
    assert_eq!(inner.parent.as_deref(), Some("crate::inline_host"));
}

#[test]
fn fixture_graph_children_edges_are_sorted_and_consistent() {
    let graph = module_graph(&fixture_root(), "src/lib.rs").unwrap();

    let root = graph.root().unwrap();
    assert_eq!(
        root.children,
        [
            "crate::aliased",
            "crate::external_file",
            "crate::inline_host",
            "crate::nested",
            "crate::restricted",
        ]
    );
    assert_eq!(
        graph.find("crate::nested").unwrap().children,
        ["crate::nested::leaf"]
    );
    assert_eq!(
        graph.find("crate::inline_host").unwrap().children,
        ["crate::inline_host::inner"]
    );
    assert!(graph
        .find("crate::external_file")
        .unwrap()
        .children
        .is_empty());
    assert!(graph
        .find("crate::inline_host::inner")
        .unwrap()
        .children
        .is_empty());
}

#[test]
fn fixture_graph_is_deterministic() {
    let first = module_graph(&fixture_root(), "src/lib.rs").unwrap();
    let second = module_graph(&fixture_root(), "src/lib.rs").unwrap();
    assert_eq!(first, second);
}

#[test]
fn binary_target_root_resolves_like_library_root() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("src/main.rs", "mod util;\nfn main() {}\n"),
            ("src/util.rs", "pub const N: u8 = 1;\n"),
        ],
    );

    let graph = module_graph(temp.path(), "src/main.rs").unwrap();
    let root = graph.root().unwrap();
    assert_eq!(root.kind, ModuleKind::Crate);
    assert_eq!(root.source_path.as_deref(), Some("src/main.rs"));
    let util = graph.find("crate::util").unwrap();
    assert_eq!(util.kind, ModuleKind::File);
    assert_eq!(util.source_path.as_deref(), Some("src/util.rs"));
}

#[test]
fn inline_module_with_path_attribute_resolves_submodule_directory() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "#[path = \"group\"]\nmod host {\n    mod inner;\n}\n",
            ),
            ("src/group/inner.rs", "const INNER: u8 = 0;\n"),
        ],
    );

    let graph = module_graph(temp.path(), "src/lib.rs").unwrap();
    let host = graph.find("crate::host").unwrap();
    assert_eq!(host.kind, ModuleKind::Inline);
    let inner = graph.find("crate::host::inner").unwrap();
    assert_eq!(inner.kind, ModuleKind::File);
    assert_eq!(inner.source_path.as_deref(), Some("src/group/inner.rs"));
}

#[test]
fn missing_target_source_is_a_typed_error() {
    let temp = TempDir::new().unwrap();
    let error = module_graph(temp.path(), "src/lib.rs").unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("src/lib.rs"));
}

#[test]
fn missing_module_file_is_a_typed_error() {
    let temp = TempDir::new().unwrap();
    write_package(temp.path(), &[("src/lib.rs", "mod missing;\n")]);

    let error = module_graph(temp.path(), "src/lib.rs").unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("missing"));
    assert!(error.to_string().contains("src/lib.rs"));
}

#[test]
fn ambiguous_module_file_is_a_typed_error() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("src/lib.rs", "mod foo;\n"),
            ("src/foo.rs", ""),
            ("src/foo/mod.rs", ""),
        ],
    );

    let error = module_graph(temp.path(), "src/lib.rs").unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("foo"));
    assert!(error.to_string().contains("foo.rs"));
    assert!(error.to_string().contains("foo/mod.rs"));
}

#[test]
fn path_attribute_missing_file_is_a_typed_error() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[("src/lib.rs", "#[path = \"nope.rs\"]\nmod aliased;\n")],
    );

    let error = module_graph(temp.path(), "src/lib.rs").unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("aliased"));
}

#[test]
fn cyclic_module_path_is_a_typed_error() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[("src/lib.rs", "#[path = \"lib.rs\"]\nmod self_ref;\n")],
    );

    let error = module_graph(temp.path(), "src/lib.rs").unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().to_lowercase().contains("cycle"));
}

#[test]
fn escaping_path_attribute_is_contained() {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("outside.rs"), "").unwrap();
    let root = temp.path().join("pkg");
    write_package(
        &root,
        &[(
            "src/lib.rs",
            "#[path = \"../../outside.rs\"]\nmod escape;\n",
        )],
    );

    let error = module_graph(&root, "src/lib.rs").unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("escapes"));
}

/// Package fixture exercising valid cfg/path alternates that share one logical
/// module path: both must be parsed because cfg evaluation is deferred.
fn cfg_gated_alternate_package(temp: &tempfile::TempDir, with_children: bool) {
    let unix_decl = if with_children { "mod epoll;\n" } else { "" };
    let windows_decl = if with_children { "mod iocp;\n" } else { "" };
    let mut files: Vec<(&str, &str)> = vec![
        (
            "src/lib.rs",
            "#[cfg(unix)]\n#[path = \"sys/unix.rs\"]\nmod sys;\n\
             #[cfg(windows)]\n#[path = \"sys/windows.rs\"]\nmod sys;\n",
        ),
        ("src/sys/unix.rs", unix_decl),
        ("src/sys/windows.rs", windows_decl),
    ];
    if with_children {
        files.push(("src/sys/unix/epoll.rs", ""));
        files.push(("src/sys/windows/iocp.rs", ""));
    }
    write_package(temp.path(), &files);
}

#[test]
fn cfg_gated_path_variants_share_a_canonical_path_and_are_both_retained() {
    let temp = TempDir::new().unwrap();
    cfg_gated_alternate_package(&temp, false);

    let first = module_graph(temp.path(), "src/lib.rs").unwrap();
    let second = module_graph(temp.path(), "src/lib.rs").unwrap();
    assert_eq!(first, second, "the graph must be deterministic");

    let variants = first.find_all("crate::sys");
    assert_eq!(
        variants.len(),
        2,
        "both cfg/path variants must be retained, not collapsed"
    );
    let mut source_paths: Vec<&str> = variants
        .iter()
        .map(|record| record.source_path.as_deref().unwrap())
        .collect();
    source_paths.sort();
    assert_eq!(
        source_paths,
        ["src/sys/unix.rs", "src/sys/windows.rs"],
        "each variant keeps its own source path"
    );
}

#[test]
fn cfg_gated_path_variants_deduplicate_parent_children() {
    let temp = TempDir::new().unwrap();
    cfg_gated_alternate_package(&temp, true);

    let graph = module_graph(temp.path(), "src/lib.rs").unwrap();
    let root = graph.root().unwrap();
    let sys_edges = root
        .children
        .iter()
        .filter(|child| **child == "crate::sys")
        .count();
    assert_eq!(
        sys_edges, 1,
        "duplicate-path variants must collapse to a single logical child edge"
    );
    assert_eq!(root.children, ["crate::sys"]);
}

#[test]
fn cfg_gated_path_variants_each_receive_unioned_children() {
    let temp = TempDir::new().unwrap();
    cfg_gated_alternate_package(&temp, true);

    let graph = module_graph(temp.path(), "src/lib.rs").unwrap();
    let expected = ["crate::sys::epoll", "crate::sys::iocp"];
    for variant in graph.find_all("crate::sys") {
        assert_eq!(
            variant.children, expected,
            "every variant of a shared path must carry the unioned logical children \
             until Task 5C1 attaches gates"
        );
    }
}

#[test]
fn find_all_enumerates_every_declaration_variant_for_a_path() {
    let temp = TempDir::new().unwrap();
    cfg_gated_alternate_package(&temp, false);

    let graph = module_graph(temp.path(), "src/lib.rs").unwrap();
    let variants = graph.find_all("crate::sys");
    assert_eq!(variants.len(), 2);
    let mut source_paths: Vec<&str> = variants
        .iter()
        .map(|record| record.source_path.as_deref().unwrap())
        .collect();
    source_paths.sort();
    assert_eq!(source_paths, ["src/sys/unix.rs", "src/sys/windows.rs"]);
    // `find` returns the first deterministic variant in source-declaration order.
    assert_eq!(
        graph.find("crate::sys").unwrap().source_path.as_deref(),
        Some("src/sys/unix.rs")
    );
    // A path with a single declaration also resolves through `find_all`.
    assert_eq!(graph.find_all("crate").len(), 1);
    assert!(graph.find_all("crate::nonexistent").is_empty());
}

#[test]
fn path_attribute_pointing_at_a_directory_resolves_mod_rs() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("src/lib.rs", "#[path = \"group\"]\nmod group;\n"),
            ("src/group/mod.rs", "mod inner;\n"),
            ("src/group/inner.rs", "const INNER: u8 = 0;\n"),
        ],
    );

    let graph = module_graph(temp.path(), "src/lib.rs").unwrap();
    let group = graph.find("crate::group").unwrap();
    assert_eq!(group.kind, ModuleKind::File);
    assert_eq!(group.source_path.as_deref(), Some("src/group/mod.rs"));
    assert_eq!(group.children, ["crate::group::inner"]);
    let inner = graph.find("crate::group::inner").unwrap();
    assert_eq!(inner.source_path.as_deref(), Some("src/group/inner.rs"));
}

#[cfg(unix)]
#[test]
fn escaping_symlinked_module_source_is_contained() {
    use std::os::unix::fs::symlink;

    let temp = TempDir::new().unwrap();
    let root = temp.path().join("pkg");
    let outside = temp.path().join("outside.rs");
    fs::write(&outside, "").unwrap();
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "mod escape;\n").unwrap();
    symlink(&outside, root.join("src/escape.rs")).unwrap();

    let error = module_graph(&root, "src/lib.rs").unwrap_err();
    assert_eq!(error.kind(), "catalog_corruption");
    assert!(error.to_string().contains("escapes"));
}
