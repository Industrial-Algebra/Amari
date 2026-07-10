// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests for Task 5B2: public export and re-export reachability.

use std::{fs, path::Path};

use amari_discovery::catalog::generator::{
    export_graph, module_graph, ExportGraph, ExportItemKind, ExportSource, ExportWarningReason,
};
use tempfile::TempDir;

fn write_package(root: &Path, files: &[(&str, &str)]) {
    for (relative, source) in files {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, source).unwrap();
    }
}

/// Builds a module graph and export graph for a TempDir package.
fn exports_for(root: &Path, source_path: &str) -> ExportGraph {
    let graph = module_graph(root, source_path).unwrap();
    export_graph(&graph, root).unwrap()
}

/// Collects a deterministic `(path, source)` summary for stable assertions.
fn summary(graph: &ExportGraph) -> Vec<(String, String)> {
    graph
        .exports
        .iter()
        .map(|record| (record.path.clone(), source_summary(&record.source)))
        .collect()
}

fn source_summary(source: &ExportSource) -> String {
    match source {
        ExportSource::Local {
            module,
            ident,
            kind,
        } => format!("local:{module}::{ident}:{kind:?}"),
        ExportSource::Module { module } => format!("module:{module}"),
    }
}

fn paths(graph: &ExportGraph) -> Vec<String> {
    graph.exports.iter().map(|r| r.path.clone()).collect()
}

#[test]
fn ordinary_public_modules_export_their_public_items_only() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("src/lib.rs", "pub mod public_mod;\n"),
            (
                "src/public_mod.rs",
                "pub struct Direct;\n\
                 pub fn hello() {}\n\
                 struct Hidden;\n\
                 pub(crate) mod restricted_child {}\n",
            ),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let mut got = paths(&graph);
    got.sort();
    assert_eq!(
        got,
        vec![
            "crate::public_mod",
            "crate::public_mod::Direct",
            "crate::public_mod::hello",
        ]
    );
    // Private and restricted items never appear.
    assert!(got.iter().all(|path| !path.contains("Hidden")));
    assert!(got.iter().all(|path| !path.contains("restricted_child")));
}

#[test]
fn private_module_items_are_exported_only_through_pub_use() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod private_mod;\n\
                 pub use private_mod::ReExported;\n",
            ),
            (
                "src/private_mod.rs",
                "pub struct ReExported;\nstruct Hidden;\n",
            ),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let summary = summary(&graph);
    // The private module itself is not reachable.
    assert!(!summary.iter().any(|(p, _)| p == "crate::private_mod"));
    // But its public item is reachable through the re-export.
    assert!(summary.contains(&(
        "crate::ReExported".to_owned(),
        "local:crate::private_mod::ReExported:Struct".to_owned()
    )));
    // The private item trapped inside is not exported.
    assert!(summary.iter().all(|(p, _)| !p.contains("Hidden")));
}

#[test]
fn aliases_re_export_under_the_alias_name() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "pub mod defs;\n\
                 pub use defs::Real as Alias;\n",
            ),
            ("src/defs.rs", "pub struct Real;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let summary = summary(&graph);
    assert!(summary.contains(&(
        "crate::Alias".to_owned(),
        "local:crate::defs::Real:Struct".to_owned()
    )));
    // The original name is also still exported directly.
    assert!(summary.contains(&(
        "crate::defs::Real".to_owned(),
        "local:crate::defs::Real:Struct".to_owned()
    )));
}

#[test]
fn glob_re_export_exposes_public_items() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod prelude;\n\
                 pub use prelude::*;\n",
            ),
            (
                "src/prelude.rs",
                "pub struct Globbed;\n\
                 pub fn globbed_fn() {}\n\
                 struct Private;\n",
            ),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let summary = summary(&graph);
    assert!(summary.contains(&(
        "crate::Globbed".to_owned(),
        "local:crate::prelude::Globbed:Struct".to_owned()
    )));
    assert!(summary.contains(&(
        "crate::globbed_fn".to_owned(),
        "local:crate::prelude::globbed_fn:Function".to_owned()
    )));
    assert!(!summary.iter().any(|(p, _)| p.contains("Private")));
    assert!(!summary.iter().any(|(p, _)| p == "crate::prelude"));
}

#[test]
fn multi_hop_local_re_export_chain_resolves_to_origin() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod hop_a;\n\
                 mod hop_b;\n\
                 pub use hop_a::Forwarded;\n",
            ),
            ("src/hop_a.rs", "pub use crate::hop_b::Forwarded;\n"),
            ("src/hop_b.rs", "pub struct Forwarded;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let summary = summary(&graph);
    // The chain resolves all the way back to hop_b::Forwarded.
    assert!(summary.contains(&(
        "crate::Forwarded".to_owned(),
        "local:crate::hop_b::Forwarded:Struct".to_owned()
    )));
}

#[test]
fn glob_chains_resolve_through_a_private_re_export() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod layer;\n\
                 mod origin;\n\
                 pub use layer::*;\n",
            ),
            ("src/layer.rs", "pub use crate::origin::*;\n"),
            ("src/origin.rs", "pub struct Chained;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let summary = summary(&graph);
    assert!(summary.contains(&(
        "crate::Chained".to_owned(),
        "local:crate::origin::Chained:Struct".to_owned()
    )));
}

#[test]
fn glob_re_export_includes_named_pub_use() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod layer;\n\
                 mod origin;\n\
                 pub use layer::*;\n",
            ),
            ("src/layer.rs", "pub use crate::origin::Named;\n"),
            ("src/origin.rs", "pub struct Named;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let summary = summary(&graph);
    // A `pub use layer::*` must export the named `pub use` declared in `layer`,
    // resolving its source back to the origin declaration.
    assert!(summary.contains(&(
        "crate::Named".to_owned(),
        "local:crate::origin::Named:Struct".to_owned()
    )));
}

#[test]
fn glob_re_export_includes_aliased_pub_use_under_alias() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod layer;\n\
                 mod origin;\n\
                 pub use layer::*;\n",
            ),
            ("src/layer.rs", "pub use crate::origin::Named as Alias;\n"),
            ("src/origin.rs", "pub struct Named;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let summary = summary(&graph);
    // The aliased `pub use` inside the glob target is exported under its alias
    // name, still pointing back at the original declaration.
    assert!(summary.contains(&(
        "crate::Alias".to_owned(),
        "local:crate::origin::Named:Struct".to_owned()
    )));
    assert!(!summary.iter().any(|(p, _)| p == "crate::Named"));
}

#[test]
fn glob_re_export_flattens_mixed_glob_and_named_local_chain() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod layer;\n\
                 mod staging;\n\
                 mod alias;\n\
                 mod base;\n\
                 pub use layer::*;\n",
            ),
            // Glob chain: `layer` pulls everything from `staging`.
            ("src/layer.rs", "pub use crate::staging::*;\n"),
            // Named re-export inside a glob target whose source itself resolves
            // through a local alias (`alias::B`).
            ("src/staging.rs", "pub use crate::alias::B as Staged;\n"),
            // The aliased source: `B` is itself an alias of `base::Base`.
            ("src/alias.rs", "pub use crate::base::Base as B;\n"),
            ("src/base.rs", "pub struct Base;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let summary = summary(&graph);
    // `pub use layer::*` flattens the glob chain into `staging`, finds the
    // named alias `Staged`, and resolves its source through `alias::B` all the
    // way back to `base::Base`.
    assert!(summary.contains(&(
        "crate::Staged".to_owned(),
        "local:crate::base::Base:Struct".to_owned()
    )));
}

#[test]
fn glob_re_export_external_named_re_export_becomes_warning() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("src/lib.rs", "mod layer;\npub use layer::*;\n"),
            ("src/layer.rs", "pub use std::io::Read;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    // No local export is fabricated for the external name seen through the glob.
    assert!(graph.exports.iter().all(|r| !r.path.contains("Read")));
    // The external named re-export is reported as a contextual warning,
    // attributed to the module that declared the `pub use`.
    assert!(graph.warnings.iter().any(|w| {
        w.declared_in == "crate::layer"
            && matches!(
                &w.reason,
                ExportWarningReason::ExternalReexport { target } if target == "std::io::Read"
            )
    }));
}

#[test]
fn crate_self_and_super_forms_in_pub_use_resolve() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "pub mod outer;\n\
                 mod base { pub struct Root; }\n\
                 pub use crate::base::Root;\n",
            ),
            (
                "src/outer.rs",
                "pub mod inner_mod;\n\
                 pub use self::inner_mod::ViaSelf;\n\
                 pub use super::base::Root as ViaSuper;\n",
            ),
            ("src/outer/inner_mod.rs", "pub struct ViaSelf;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let summary = summary(&graph);
    assert!(summary.contains(&(
        "crate::Root".to_owned(),
        "local:crate::base::Root:Struct".to_owned()
    )));
    assert!(summary.contains(&(
        "crate::outer::ViaSelf".to_owned(),
        "local:crate::outer::inner_mod::ViaSelf:Struct".to_owned()
    )));
    assert!(summary.contains(&(
        "crate::outer::ViaSuper".to_owned(),
        "local:crate::base::Root:Struct".to_owned()
    )));
}

#[test]
fn nested_public_modules_are_reachable() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("src/lib.rs", "pub mod outer;\n"),
            ("src/outer.rs", "pub mod inner;\n"),
            ("src/outer/inner.rs", "pub struct Deep;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let summary = summary(&graph);
    assert!(summary.contains(&(
        "crate::outer::inner::Deep".to_owned(),
        "local:crate::outer::inner::Deep:Struct".to_owned()
    )));
    assert!(summary
        .iter()
        .any(|(p, s)| p == "crate::outer::inner" && s == "module:crate::outer::inner"));
}

#[test]
fn duplicate_same_source_exports_dedupe() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "pub mod defs;\n\
                 pub use defs::Same;\n\
                 pub use defs::Same;\n",
            ),
            ("src/defs.rs", "pub struct Same;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let count = graph
        .exports
        .iter()
        .filter(|r| r.path == "crate::Same")
        .count();
    assert_eq!(count, 1, "identical (path, source) exports must dedupe");
}

#[test]
fn cfg_deferred_conflicting_sources_for_one_path_are_both_retained() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod unix_src;\n\
                 mod windows_src;\n\
                 #[cfg(unix)]\n\
                 pub use unix_src::Handle as Handle;\n\
                 #[cfg(windows)]\n\
                 pub use windows_src::Handle as Handle;\n",
            ),
            ("src/unix_src.rs", "pub struct Handle;\n"),
            ("src/windows_src.rs", "pub struct Handle;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let handle_sources: Vec<String> = graph
        .exports
        .iter()
        .filter(|r| r.path == "crate::Handle")
        .map(|r| source_summary(&r.source))
        .collect();
    assert_eq!(
        handle_sources.len(),
        2,
        "cfg-deferred conflicting sources must both be retained, not collapsed: {handle_sources:?}"
    );
    assert!(handle_sources.contains(&"local:crate::unix_src::Handle:Struct".to_owned()));
    assert!(handle_sources.contains(&"local:crate::windows_src::Handle:Struct".to_owned()));
}

#[test]
fn unresolved_external_exports_become_sorted_contextual_warnings() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub use std::collections::HashMap;\n\
                 pub use serde::Serialize;\n\
                 pub use external_crate::nested::Thing;\n",
        )],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    // No local export path is emitted for external items.
    assert!(graph.exports.iter().all(|r| !r.path.contains("HashMap")));
    assert!(graph.exports.iter().all(|r| !r.path.contains("Serialize")));
    assert!(graph.exports.iter().all(|r| !r.path.contains("Thing")));

    let targets: Vec<String> = graph
        .warnings
        .iter()
        .filter_map(|w| match &w.reason {
            ExportWarningReason::ExternalReexport { target } => Some(target.clone()),
            _ => None,
        })
        .collect();
    assert!(targets.contains(&"std::collections::HashMap".to_owned()));
    assert!(targets.contains(&"serde::Serialize".to_owned()));
    assert!(targets.contains(&"external_crate::nested::Thing".to_owned()));
    // Warnings carry the declaring module as context.
    assert!(graph.warnings.iter().all(|w| w.declared_in == "crate"));
    // Warnings are deduplicated and sorted.
    let mut sorted = graph.warnings.clone();
    sorted.sort();
    assert_eq!(graph.warnings, sorted, "warnings must be sorted");
    let raw_count = graph.warnings.len();
    let deduped: std::collections::BTreeSet<_> = graph.warnings.iter().collect();
    assert_eq!(raw_count, deduped.len(), "warnings must be deduplicated");
}

#[test]
fn private_use_then_pub_use_external_resolves_to_warning() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "use std::collections::HashMap;\n\
                 pub use HashMap;\n",
        )],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    assert!(graph.exports.iter().all(|r| !r.path.contains("HashMap")));
    assert!(graph.warnings.iter().any(|w| matches!(
        &w.reason,
        ExportWarningReason::ExternalReexport { target } if target == "std::collections::HashMap"
    )));
}

#[test]
fn pub_crate_and_pub_super_are_not_externally_public() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("src/lib.rs", "pub mod outer;\n"),
            (
                "src/outer.rs",
                "pub(crate) struct CrateOnly;\n\
                 pub(super) struct SuperOnly;\n\
                 pub(in crate::outer) struct InOnly;\n\
                 pub struct Public;\n",
            ),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let paths = paths(&graph);
    assert!(paths.contains(&"crate::outer::Public".to_owned()));
    assert!(paths.iter().all(|p| !p.contains("CrateOnly")));
    assert!(paths.iter().all(|p| !p.contains("SuperOnly")));
    assert!(paths.iter().all(|p| !p.contains("InOnly")));
}

#[test]
fn public_items_trapped_behind_a_private_module_are_not_exported() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("src/lib.rs", "mod hidden;\n"),
            ("src/hidden.rs", "pub struct Trapped;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    assert!(
        graph.exports.is_empty(),
        "no exports should escape a private module"
    );
    assert!(graph.warnings.is_empty());
}

#[test]
fn crate_root_public_items_are_exported_directly() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct Root;\n\
                 pub const ROOT_N: u8 = 0;\n\
                 struct PrivateRoot;\n",
        )],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let summary = summary(&graph);
    assert!(summary.contains(&(
        "crate::Root".to_owned(),
        "local:crate::Root:Struct".to_owned()
    )));
    assert!(summary.contains(&(
        "crate::ROOT_N".to_owned(),
        "local:crate::ROOT_N:Constant".to_owned()
    )));
    assert!(!summary.iter().any(|(p, _)| p.contains("PrivateRoot")));
}

#[test]
fn exported_item_kinds_are_classified() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct S;\n\
                 pub enum E {}\n\
                 pub union U { f: u8 }\n\
                 pub fn f() {}\n\
                 pub const C: u8 = 0;\n\
                 pub static G: u8 = 0;\n\
                 pub trait T {}\n\
                 pub type A = u8;\n",
        )],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let kind_of = |name: &str| -> ExportItemKind {
        graph
            .exports
            .iter()
            .find(|r| r.path == format!("crate::{name}"))
            .and_then(|r| match &r.source {
                ExportSource::Local { kind, .. } => Some(*kind),
                _ => None,
            })
            .unwrap()
    };
    assert_eq!(kind_of("S"), ExportItemKind::Struct);
    assert_eq!(kind_of("E"), ExportItemKind::Enum);
    assert_eq!(kind_of("U"), ExportItemKind::Union);
    assert_eq!(kind_of("f"), ExportItemKind::Function);
    assert_eq!(kind_of("C"), ExportItemKind::Constant);
    assert_eq!(kind_of("G"), ExportItemKind::Static);
    assert_eq!(kind_of("T"), ExportItemKind::Trait);
    assert_eq!(kind_of("A"), ExportItemKind::TypeAlias);
}

#[test]
fn glob_re_export_exposes_a_module_and_its_items() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "mod bundle;\n\
                 pub use bundle::*;\n",
            ),
            ("src/bundle.rs", "pub mod sub;\n"),
            ("src/bundle/sub.rs", "pub struct FromGlobbedModule;\n"),
        ],
    );

    let graph = exports_for(temp.path(), "src/lib.rs");
    let summary = summary(&graph);
    // The module re-exported by the glob is reachable under the glob site.
    assert!(summary
        .iter()
        .any(|(p, s)| p == "crate::sub" && s == "module:crate::bundle::sub"));
    // And its public item is reachable through that path.
    assert!(summary.contains(&(
        "crate::sub::FromGlobbedModule".to_owned(),
        "local:crate::bundle::sub::FromGlobbedModule:Struct".to_owned()
    )));
}

#[test]
fn export_graph_is_deterministic() {
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "pub mod defs;\n\
                 mod priv_mod;\n\
                 pub use priv_mod::Hidden as Shown;\n\
                 pub use defs::X;\n\
                 pub use std::io::Read;\n",
            ),
            ("src/defs.rs", "pub struct X;\n"),
            ("src/priv_mod.rs", "pub struct Hidden;\n"),
        ],
    );

    let first = exports_for(temp.path(), "src/lib.rs");
    let second = exports_for(temp.path(), "src/lib.rs");
    assert_eq!(first, second);
}

#[test]
fn missing_source_file_for_a_file_module_is_a_typed_error() {
    let temp = TempDir::new().unwrap();
    write_package(temp.path(), &[("src/lib.rs", "pub mod missing;\n")]);
    let graph = module_graph(temp.path(), "src/lib.rs"); // graph build itself errors
    assert!(graph.is_err());
}
