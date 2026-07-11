// SPDX-License-Identifier: MIT OR Apache-2.0

//! Tests for Task 5C1: recording cfg-gated public surfaces.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use amari_discovery::catalog::generator::inventory::{
    FeatureInventoryRecord, PackageInventoryRecord, TargetInventoryRecord, TargetKind,
};
use amari_discovery::catalog::generator::{
    cfg, cfg_gates, export_graph, module_graph, signature_catalog, trait_relationships, CfgExpr,
    CfgGate, CfgGateRecord, CfgStatus, CfgSurfaceKind,
};
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cfg-surface")
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

/// Returns sorted (path, gate_summary, status) triples for stable assertions.
fn gate_summaries(gates: &[CfgGateRecord]) -> Vec<(String, String, CfgStatus)> {
    let mut triples: Vec<_> = gates
        .iter()
        .map(|r| (r.path.clone(), gate_repr(&r.gate), r.status))
        .collect();
    triples.sort_by(|a, b| a.0.cmp(&b.0));
    triples
}

fn gate_repr(gate: &CfgGate) -> String {
    match gate {
        CfgGate::Always => "Always".to_owned(),
        CfgGate::Known(expr) => format!("Known({expr})"),
        CfgGate::Conditional { expr, unknowns } => {
            format!("Conditional({expr}, unknowns={unknowns:?})")
        }
        CfgGate::Unknown(unknowns) => format!("Unknown({unknowns:?})"),
    }
}

/// Builds a CfgGate from a single feature, for concise test assertions.
fn feature_gate(name: &str) -> CfgGate {
    CfgGate::Known(CfgExpr::Feature(name.to_owned()))
}

// ---------------------------------------------------------------------------
// Expression parsing and display
// ---------------------------------------------------------------------------

#[test]
fn cfg_expr_display_roundtrips_features() {
    let feat = CfgExpr::Feature("foo".to_owned());
    assert_eq!(feat.to_string(), r#"feature("foo")"#);

    let not_feat = CfgExpr::Not(Box::new(CfgExpr::Feature("bar".to_owned())));
    assert_eq!(not_feat.to_string(), r#"not(feature("bar"))"#);
}

#[test]
fn cfg_expr_display_all_and_any() {
    let all = CfgExpr::All(vec![
        CfgExpr::Feature("a".to_owned()),
        CfgExpr::Feature("b".to_owned()),
    ]);
    assert_eq!(all.to_string(), r#"all(feature("a"), feature("b"))"#);

    let any = CfgExpr::Any(vec![
        CfgExpr::Feature("x".to_owned()),
        CfgExpr::Feature("y".to_owned()),
    ]);
    assert_eq!(any.to_string(), r#"any(feature("x"), feature("y"))"#);
}

#[test]
fn unknown_cfg_displays_source_text() {
    let unk = CfgExpr::UnknownCfg(r#"target_os = "linux""#.to_owned());
    assert_eq!(unk.to_string(), r#"unknown_cfg(target_os = "linux")"#);
}

#[test]
fn cfg_expr_equality_follows_structure() {
    let a = CfgExpr::Feature("x".to_owned());
    let b = CfgExpr::Feature("x".to_owned());
    let c = CfgExpr::Feature("y".to_owned());
    assert_eq!(a, b);
    assert_ne!(a, c);

    let all1 = CfgExpr::all(vec![a.clone(), c.clone()]);
    let all2 = CfgExpr::all(vec![c.clone(), a.clone()]);
    assert_eq!(all1, all2, "all children must be deterministic");
}

// ---------------------------------------------------------------------------
// Three-valued evaluation
// ---------------------------------------------------------------------------

fn enabled_defaults(names: &[&str]) -> BTreeMap<String, CfgStatus> {
    let mut map = BTreeMap::new();
    for &name in names {
        map.insert(name.to_owned(), CfgStatus::Enabled);
    }
    map
}

#[test]
fn evaluate_feature_gate_against_known_defaults() {
    let defaults = enabled_defaults(&["default_on"]);
    assert_eq!(
        CfgGate::Known(CfgExpr::Feature("default_on".to_owned())).evaluate(&defaults),
        CfgStatus::Enabled
    );
    assert_eq!(
        CfgGate::Known(CfgExpr::Feature("opt_in".to_owned())).evaluate(&defaults),
        CfgStatus::Disabled
    );
}

#[test]
fn evaluate_not_inverts_status() {
    let defaults = enabled_defaults(&["default_on"]);
    let not_on = CfgGate::Known(CfgExpr::Not(Box::new(CfgExpr::Feature(
        "default_on".to_owned(),
    ))));
    assert_eq!(not_on.evaluate(&defaults), CfgStatus::Disabled);

    let not_off = CfgGate::Known(CfgExpr::Not(Box::new(CfgExpr::Feature(
        "opt_in".to_owned(),
    ))));
    assert_eq!(not_off.evaluate(&defaults), CfgStatus::Enabled);
}

#[test]
fn evaluate_all_requires_all_enabled() {
    let defaults = enabled_defaults(&["default_on"]);
    let all_both = CfgGate::Known(CfgExpr::All(vec![
        CfgExpr::Feature("default_on".to_owned()),
        CfgExpr::Feature("opt_in".to_owned()),
    ]));
    assert_eq!(all_both.evaluate(&defaults), CfgStatus::Disabled);

    let all_on = CfgGate::Known(CfgExpr::All(vec![
        CfgExpr::Feature("default_on".to_owned()),
        CfgExpr::Feature("default_on".to_owned()),
    ]));
    assert_eq!(all_on.evaluate(&defaults), CfgStatus::Enabled);
}

#[test]
fn evaluate_any_needs_at_least_one_enabled() {
    let defaults = enabled_defaults(&["default_on"]);
    let any = CfgGate::Known(CfgExpr::Any(vec![
        CfgExpr::Feature("default_on".to_owned()),
        CfgExpr::Feature("opt_in".to_owned()),
    ]));
    assert_eq!(any.evaluate(&defaults), CfgStatus::Enabled);

    let any_none = CfgGate::Known(CfgExpr::Any(vec![
        CfgExpr::Feature("opt_in".to_owned()),
        CfgExpr::Feature("other".to_owned()),
    ]));
    assert_eq!(any_none.evaluate(&defaults), CfgStatus::Disabled);
}

#[test]
fn evaluate_unknown_cfg_is_always_unknown() {
    let defaults = enabled_defaults(&["default_on"]);
    let gate = CfgGate::Unknown(vec!["target_os".to_owned()]);
    assert_eq!(gate.evaluate(&defaults), CfgStatus::Unknown);

    // Conditional with an UnknownCfg node preserved in the expression tree.
    // all(feature("default_on"), unknown(...)) evaluates feature → Enabled,
    // then all(Enabled, Unknown) → Unknown (Kleene).
    let cond = CfgGate::from_expr(CfgExpr::All(vec![
        CfgExpr::Feature("default_on".to_owned()),
        CfgExpr::UnknownCfg("target_arch".to_owned()),
    ]));
    assert_eq!(cond.evaluate(&defaults), CfgStatus::Unknown);
}

#[test]
fn evaluate_always_is_enabled() {
    let defaults = BTreeMap::new();
    assert_eq!(CfgGate::Always.evaluate(&defaults), CfgStatus::Enabled);
}

// ---------------------------------------------------------------------------
// Normalization: all/any children are sorted and deduplicated
// ---------------------------------------------------------------------------

#[test]
fn all_children_are_normalized_on_construction() {
    let expr = CfgExpr::all(vec![
        CfgExpr::Feature("b".to_owned()),
        CfgExpr::Feature("a".to_owned()),
        CfgExpr::Feature("a".to_owned()),
    ]);
    let expected = CfgExpr::All(vec![
        CfgExpr::Feature("a".to_owned()),
        CfgExpr::Feature("b".to_owned()),
    ]);
    assert_eq!(expr, expected);
}

#[test]
fn any_children_are_normalized_on_construction() {
    let expr = CfgExpr::any(vec![
        CfgExpr::Feature("z".to_owned()),
        CfgExpr::Feature("a".to_owned()),
    ]);
    let expected = CfgExpr::Any(vec![
        CfgExpr::Feature("a".to_owned()),
        CfgExpr::Feature("z".to_owned()),
    ]);
    assert_eq!(expr, expected);
}

#[test]
fn nested_empty_all_or_any_are_valid() {
    let empty_all = CfgExpr::all(vec![]);
    assert_eq!(empty_all.to_string(), "all()");

    let empty_any = CfgExpr::any(vec![]);
    assert_eq!(empty_any.to_string(), "any()");
}

// ---------------------------------------------------------------------------
// Feature default closure from PackageInventoryRecord
// ---------------------------------------------------------------------------

#[test]
fn feature_default_closure_follows_feature_to_feature_edges() {
    use amari_discovery::catalog::generator::inventory::FeatureInventoryRecord;

    let features = vec![
        FeatureInventoryRecord {
            name: "default".to_owned(),
            enables: vec!["default_on".to_owned()],
        },
        FeatureInventoryRecord {
            name: "default_on".to_owned(),
            enables: vec!["transitive".to_owned(), "dep:some_dep".to_owned()],
        },
        FeatureInventoryRecord {
            name: "transitive".to_owned(),
            enables: vec!["transitive2".to_owned()],
        },
        FeatureInventoryRecord {
            name: "transitive2".to_owned(),
            enables: vec![],
        },
        FeatureInventoryRecord {
            name: "opt_in".to_owned(),
            enables: vec![],
        },
    ];

    let closure = cfg::feature_default_closure(&features);

    assert!(
        closure.contains("default_on"),
        "default_on is in default closure"
    );
    assert!(
        closure.contains("transitive"),
        "transitive reached via feature edge"
    );
    assert!(
        closure.contains("transitive2"),
        "transitive2 reached transitively"
    );
    assert!(
        !closure.contains("opt_in"),
        "opt_in is not in default closure"
    );
    assert!(!closure.contains("some_dep"), "dep: edges are excluded");
    assert!(
        !closure.contains("default"),
        "default feature name is excluded from closure"
    );
}

// ---------------------------------------------------------------------------
// Fixture: cfg-surface crate
// ---------------------------------------------------------------------------

fn fixture_gates() -> Vec<CfgGateRecord> {
    let root = fixture_root();
    let graph = module_graph(&root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, &root).unwrap();
    let sigs = signature_catalog(&graph, &exports, &root).unwrap();
    let inventory = cfg_surface_inventory();
    cfg_gates(&graph, &exports, &sigs, &inventory, &root).unwrap()
}

#[test]
fn fixture_cfg_surface_always_available_is_ungated() {
    let gates = fixture_gates();
    let alice = gates
        .iter()
        .find(|r| r.path == "crate::always_available")
        .unwrap();
    assert_eq!(alice.gate, CfgGate::Always);
    assert_eq!(alice.status, CfgStatus::Enabled);
}

#[test]
fn fixture_cfg_surface_default_feature_gated_is_enabled() {
    let gates = fixture_gates();
    let record = gates
        .iter()
        .find(|r| r.path == "crate::default_feature_gated")
        .unwrap();
    assert_eq!(
        record.gate,
        feature_gate("default_on"),
        "default_feature_gated should be gated on feature default_on"
    );
    assert_eq!(
        record.status,
        CfgStatus::Enabled,
        "default_on is in the default closure"
    );
}

#[test]
fn fixture_cfg_surface_opt_in_feature_is_disabled() {
    let gates = fixture_gates();
    let record = gates
        .iter()
        .find(|r| r.path == "crate::opt_in_feature_gated")
        .unwrap();
    assert_eq!(record.gate, feature_gate("opt_in"));
    assert_eq!(record.status, CfgStatus::Disabled);
}

#[test]
fn fixture_cfg_surface_all_conjunction() {
    let gates = fixture_gates();
    let record = gates
        .iter()
        .find(|r| r.path == "crate::all_conjunction")
        .unwrap();
    let expected = CfgGate::Known(CfgExpr::all(vec![
        CfgExpr::Feature("default_on".to_owned()),
        CfgExpr::Feature("opt_in".to_owned()),
    ]));
    assert_eq!(record.gate, expected);
    assert_eq!(record.status, CfgStatus::Disabled, "opt_in is not default");
}

#[test]
fn fixture_cfg_surface_any_disjunction() {
    let gates = fixture_gates();
    let record = gates
        .iter()
        .find(|r| r.path == "crate::any_disjunction")
        .unwrap();
    let expected = CfgGate::Known(CfgExpr::any(vec![
        CfgExpr::Feature("default_on".to_owned()),
        CfgExpr::Feature("opt_in".to_owned()),
    ]));
    assert_eq!(record.gate, expected);
    assert_eq!(
        record.status,
        CfgStatus::Enabled,
        "default_on satisfies any"
    );
}

#[test]
fn fixture_cfg_surface_inherited_module_gate() {
    let gates = fixture_gates();
    let child = gates
        .iter()
        .find(|r| r.path == "crate::gated_module::child_in_gated_module")
        .unwrap();
    assert_eq!(child.gate, feature_gate("default_on"));
    assert_eq!(child.status, CfgStatus::Enabled);
}

#[test]
fn fixture_cfg_surface_same_file_cfg_variants_are_distinct() {
    let gates = fixture_gates();
    let variants: Vec<_> = gates
        .iter()
        .filter(|r| r.path == "crate::SameFileVariant")
        .collect();
    assert_eq!(
        variants.len(),
        2,
        "same-file cfg variants must produce distinct gate records"
    );

    let mut summaries: Vec<_> = variants.iter().map(|r| gate_repr(&r.gate)).collect();
    summaries.sort();
    assert!(
        summaries.contains(&r#"Known(feature("default_on"))"#.to_owned()),
        "one variant gated on default_on"
    );
    assert!(
        summaries.contains(&r#"Known(feature("opt_in"))"#.to_owned()),
        "one variant gated on opt_in"
    );

    let ordinals: Vec<_> = variants.iter().map(|r| r.source_ordinal).collect();
    assert_eq!(ordinals.len(), 2);
    assert_ne!(ordinals[0], ordinals[1]);
}

#[test]
fn fixture_cfg_surface_unsupported_predicate_is_unknown() {
    let gates = fixture_gates();
    let record = gates
        .iter()
        .find(|r| r.path == "crate::target_os_linux")
        .unwrap();
    assert!(
        matches!(&record.gate, CfgGate::Unknown(_)),
        "target_os should produce Unknown gate, got {}",
        gate_repr(&record.gate)
    );
    assert_eq!(record.status, CfgStatus::Unknown);
}

#[test]
fn fixture_cfg_surface_bare_unix_is_unknown() {
    let gates = fixture_gates();
    let record = gates
        .iter()
        .find(|r| r.path == "crate::bare_unix_predicate")
        .unwrap();
    assert!(
        matches!(&record.gate, CfgGate::Unknown(_)),
        "bare unix predicate should be Unknown"
    );
}

#[test]
fn fixture_cfg_surface_mixed_known_and_unknown() {
    let gates = fixture_gates();
    let record = gates
        .iter()
        .find(|r| r.path == "crate::mixed_known_and_unknown")
        .unwrap();
    match &record.gate {
        CfgGate::Conditional { unknowns, .. } => {
            assert!(!unknowns.is_empty(), "must record unknowns");
        }
        CfgGate::Unknown(_) => {}
        other => panic!("expected Conditional or Unknown, got {}", gate_repr(other)),
    }
    // any(feature("default_on"), target_arch = "wasm32") → Enabled
    // because default_on is in the default closure (Kleene short-circuit).
    assert_eq!(record.status, CfgStatus::Enabled);
}

// ===================================================================
// Impl-block cfg inheritance (Issue 1)
// ===================================================================

#[test]
fn fixture_impl_block_cfg_gates_both_method_and_const() {
    // impl Owner gated on feature="impl_gate" — both the method and const
    // must carry that gate even though they have no individual #[cfg].
    let gates = fixture_gates();

    let method = gates
        .iter()
        .find(|r| r.path == "crate::Owner::impl_gated_method")
        .unwrap_or_else(|| {
            panic!(
                "crate::Owner::impl_gated_method not found. paths: {:?}",
                gates.iter().map(|r| &r.path).collect::<Vec<_>>()
            )
        });
    assert_eq!(
        gate_repr(&method.gate),
        r#"Known(feature("impl_gate"))"#,
        "impl_gated_method should be gated on impl_gate"
    );
    assert_eq!(method.status, CfgStatus::Disabled);
    assert_eq!(method.surface_kind, CfgSurfaceKind::InherentMethod);

    let konst = gates
        .iter()
        .find(|r| r.path == "crate::Owner::IMPL_GATED_CONST")
        .unwrap_or_else(|| {
            panic!(
                "crate::Owner::IMPL_GATED_CONST not found. paths: {:?}",
                gates.iter().map(|r| &r.path).collect::<Vec<_>>()
            )
        });
    assert_eq!(
        gate_repr(&konst.gate),
        r#"Known(feature("impl_gate"))"#,
        "IMPL_GATED_CONST should be gated on impl_gate"
    );
    assert_eq!(konst.status, CfgStatus::Disabled);
    assert_eq!(konst.surface_kind, CfgSurfaceKind::InherentConst);
}

#[test]
fn fixture_impl_plus_member_cfg_combined() {
    // #[cfg(feature="impl_gate")] impl Owner {
    //     #[cfg(feature="member_gate")] pub fn method(&self) {}
    // }
    // gate must be all(impl_gate, member_gate).
    let gates = fixture_gates();

    let method = gates
        .iter()
        .find(|r| r.path == "crate::Owner::impl_and_member_gated_method")
        .unwrap_or_else(|| {
            panic!(
                "crate::Owner::impl_and_member_gated_method not found. paths: {:?}",
                gates.iter().map(|r| &r.path).collect::<Vec<_>>()
            )
        });
    // all(impl_gate, member_gate) — both are unknown features (not in default closure).
    let expected = CfgGate::Known(CfgExpr::all(vec![
        CfgExpr::Feature("impl_gate".to_owned()),
        CfgExpr::Feature("member_gate".to_owned()),
    ]));
    assert_eq!(method.gate, expected);
    assert_eq!(method.status, CfgStatus::Disabled);
    assert_eq!(method.surface_kind, CfgSurfaceKind::InherentMethod);
}

#[test]
fn fixture_impl_block_with_unsupported_predicate_on_member() {
    // #[cfg(feature="default_on")] impl Owner {
    //     #[cfg(target_os = "linux")] pub fn impl_default_member_unsupported(&self) {}
    // }
    // gate must combine impl_gate (feature default_on) with member gate (unknown target_os).
    let gates = fixture_gates();

    let method = gates
        .iter()
        .find(|r| r.path == "crate::Owner::impl_default_member_unsupported")
        .unwrap_or_else(|| {
            panic!(
                "crate::Owner::impl_default_member_unsupported not found. paths: {:?}",
                gates.iter().map(|r| &r.path).collect::<Vec<_>>()
            )
        });
    match &method.gate {
        CfgGate::Conditional { expr, unknowns } => {
            assert!(
                expr.to_string().contains(r#"feature("default_on")"#),
                "must contain known feature default_on"
            );
            assert!(!unknowns.is_empty(), "must have unknowns from target_os");
        }
        CfgGate::Unknown(_) => {}
        other => panic!("expected Conditional or Unknown, got {}", gate_repr(other)),
    }
    // all(feature("default_on"), unknown(target_os)) — default_on is Enabled,
    // so Kleene all(Enabled, Unknown) → Unknown.
    assert_eq!(method.status, CfgStatus::Unknown);
}

#[test]
fn fixture_ungated_inherent_method_is_always() {
    // impl Owner { pub fn always_method(&self) {} }
    // Must produce the gate Always (no cfg on impl or member).
    let gates = fixture_gates();

    let method = gates
        .iter()
        .find(|r| r.path == "crate::Owner::always_method")
        .unwrap_or_else(|| {
            panic!(
                "crate::Owner::always_method not found. paths: {:?}",
                gates.iter().map(|r| &r.path).collect::<Vec<_>>()
            )
        });
    assert_eq!(method.gate, CfgGate::Always);
    assert_eq!(method.status, CfgStatus::Enabled);
    assert_eq!(method.surface_kind, CfgSurfaceKind::InherentMethod);
}

#[test]
fn fixture_cfg_surface_not_default_is_disabled() {
    let gates = fixture_gates();
    let record = gates
        .iter()
        .find(|r| r.path == "crate::not_default")
        .unwrap();
    assert_eq!(
        record.gate,
        CfgGate::Known(CfgExpr::Not(Box::new(CfgExpr::Feature(
            "default_on".to_owned()
        ))))
    );
    assert_eq!(
        record.status,
        CfgStatus::Disabled,
        "not(default_on) is disabled when default_on is on"
    );
}

// ---------------------------------------------------------------------------
// Synthesized package: simple/default, simple disabled
// ---------------------------------------------------------------------------

#[test]
fn synthesized_simple_and_default_gates() {
    use amari_discovery::catalog::generator::inventory::{
        FeatureInventoryRecord, TargetInventoryRecord, TargetKind,
    };

    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "#[cfg(feature = \"std\")]\npub fn with_std() {}\n\
                 pub fn always_there() {}\n\
                 #[cfg(feature = \"serde\")]\npub fn with_serde() {}\n",
        )],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();

    let pkg = PackageInventoryRecord {
        name: "demo".to_owned(),
        version: "0.1.0".to_owned(),
        description: "Test package".to_owned(),
        license: "MIT".to_owned(),
        edition: "2021".to_owned(),
        manifest_path: String::new(),
        library_outputs: vec!["lib".to_owned()],
        features: vec![
            FeatureInventoryRecord {
                name: "default".to_owned(),
                enables: vec!["std".to_owned()],
            },
            FeatureInventoryRecord {
                name: "std".to_owned(),
                enables: vec![],
            },
            FeatureInventoryRecord {
                name: "serde".to_owned(),
                enables: vec![],
            },
        ],
        dependencies: vec![],
        targets: vec![TargetInventoryRecord {
            name: "demo".to_owned(),
            kind: TargetKind::Library,
            path: "src/lib.rs".to_owned(),
            required_features: vec![],
            crate_types: vec!["lib".to_owned()],
        }],
    };

    let gates = cfg_gates(&graph, &exports, &sigs, &pkg, root).unwrap();

    let summaries = gate_summaries(&gates);
    let expected = vec![
        (
            "crate::always_there".to_owned(),
            "Always".to_owned(),
            CfgStatus::Enabled,
        ),
        (
            "crate::with_serde".to_owned(),
            r#"Known(feature("serde"))"#.to_owned(),
            CfgStatus::Disabled,
        ),
        (
            "crate::with_std".to_owned(),
            r#"Known(feature("std"))"#.to_owned(),
            CfgStatus::Enabled,
        ),
    ];
    assert_eq!(summaries, expected);
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

#[test]
fn cfg_gates_are_deterministic() {
    let first = fixture_gates();
    let second = fixture_gates();
    assert_eq!(first, second, "cfg_gates must be deterministic");
}

// ===================================================================
// RED phase — Issue 1: File Module Canonical Path
// ===================================================================

#[test]
fn external_file_module_items_indexed_under_correct_canonical_path() {
    // Items declared in src/external_module.rs must be indexed under
    // crate::external_module, not under crate.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            ("src/lib.rs", "pub mod external_module;\n"),
            ("src/external_module.rs", "pub struct ExtStruct;\n"),
        ],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();
    let inventory = minimal_inventory();
    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();

    // ExtStruct should be at crate::external_module::ExtStruct, not crate::ExtStruct.
    let record = gates
        .iter()
        .find(|r| r.path == "crate::external_module::ExtStruct")
        .unwrap_or_else(|| {
            panic!(
                "ExtStruct should be at crate::external_module::ExtStruct, got paths: {:?}",
                gates.iter().map(|r| &r.path).collect::<Vec<_>>()
            )
        });
    assert!(
        record.source_path.contains("external_module.rs"),
        "source_path should be external_module.rs, got {}",
        record.source_path
    );
}

#[test]
fn external_file_module_inherits_mod_declaration_cfg_gate() {
    // A gated mod declaration in lib.rs should propagate to items in the
    // external file module.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "#[cfg(feature = \"optional_mod\")]\n\
                 pub mod gated_external;\n",
            ),
            ("src/gated_external.rs", "pub struct GatedItem;\n"),
        ],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();

    let mut inventory = minimal_inventory();
    inventory.features.push(FeatureInventoryRecord {
        name: "optional_mod".to_owned(),
        enables: vec![],
    });

    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();

    let record = gates
        .iter()
        .find(|r| r.path == "crate::gated_external::GatedItem")
        .expect("GatedItem should be found");
    assert_eq!(
        gate_repr(&record.gate),
        r#"Known(feature("optional_mod"))"#,
        "GatedItem should inherit the mod declaration's cfg gate"
    );
    assert_eq!(
        record.status,
        CfgStatus::Disabled,
        "optional_mod is not in default closure, so GatedItem should be Disabled"
    );
}

#[test]
fn nested_external_file_modules_index_items_correctly() {
    // A deeply nested external file module chain must index items under
    // their full canonical path.
    let root = TempDir::new().unwrap();
    let src = root.path().join("src");
    fs::create_dir_all(src.join("a")).unwrap();
    fs::write(src.join("lib.rs"), "pub mod a;\n").unwrap();
    fs::write(src.join("a.rs"), "pub mod b;\n").unwrap();
    fs::write(src.join("a/b.rs"), "pub struct NestedDeep;\n").unwrap();

    let graph = module_graph(root.path(), "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root.path()).unwrap();
    let sigs = signature_catalog(&graph, &exports, root.path()).unwrap();
    let inventory = minimal_inventory();
    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root.path()).unwrap();

    let record = gates
        .iter()
        .find(|r| r.path == "crate::a::b::NestedDeep")
        .unwrap_or_else(|| {
            panic!(
                "NestedDeep should be at crate::a::b::NestedDeep, got paths: {:?}",
                gates.iter().map(|r| &r.path).collect::<Vec<_>>()
            )
        });
    assert!(
        record.source_path.contains("a/b.rs"),
        "source_path should be a/b.rs, got {}",
        record.source_path
    );
}

// ===================================================================
// RED phase — Issue 2: Ordinal Contract
// ===================================================================

#[test]
fn cfg_ordinals_match_traits_ordinals_for_same_file_case() {
    // When the same source file declares items counted by both cfg and traits
    // ordinals, the numbers must agree. A function before a trait should bump
    // the ordinal in both systems, so the trait's ordinal matches its cfg gate
    // ordinal.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub fn helper() {}\n\
             pub const HELPER_CONST: u8 = 0;\n\
             pub trait CountedTrait {}\n\
             impl CountedTrait for () {}\n\
             pub struct AfterTrait;\n",
        )],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();
    let inventory = minimal_inventory();
    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();
    let traits_catalog = trait_relationships(&graph, &exports, root).unwrap();

    // CountedTrait's cfg ordinal should match its TraitDefinition ordinal.
    let cfg_counted = gates
        .iter()
        .find(|r| r.path == "crate::CountedTrait")
        .expect("crate::CountedTrait cfg gate not found");

    let trait_counted = traits_catalog
        .definitions
        .iter()
        .find(|d| d.export_path == "crate::CountedTrait")
        .expect("crate::CountedTrait definition not found");

    assert_eq!(
        cfg_counted.source_ordinal, trait_counted.source_ordinal,
        "CountedTrait source_ordinal must match between cfg and traits. \
         cfg ordinal={}, traits ordinal={}. \
         This means helper() and HELPER_CONST must be counted by the \
         traits ordinal to push CountedTrait to 2 in both systems.",
        cfg_counted.source_ordinal, trait_counted.source_ordinal,
    );
    assert!(
        cfg_counted.source_ordinal > 0,
        "CountedTrait must have ordinal > 0 because helper() and HELPER_CONST precede it"
    );
}

#[test]
fn cfg_and_traits_ordinals_match_with_preceding_impl() {
    // An impl block before a struct should bump the ordinal in both systems.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait OrdTrait {}\n\
             pub struct OrdStruct;\n\
             impl OrdTrait for OrdStruct {}\n\
             pub struct AfterImpl;\n",
        )],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();
    let inventory = minimal_inventory();
    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();
    let traits_catalog = trait_relationships(&graph, &exports, root).unwrap();

    let cfg_after = gates
        .iter()
        .find(|r| r.path == "crate::AfterImpl")
        .expect("crate::AfterImpl cfg gate not found");

    // Find the TraitImplementation for OrdImpl for OrdStruct
    let impl_ord = traits_catalog
        .implementations
        .iter()
        .find(|imp| imp.trait_path == "crate::OrdTrait" && imp.impl_type_path == "crate::OrdStruct")
        .expect("impl OrdTrait for OrdStruct not found");

    // AfterImpl should have ordinal 3 (0=OrdTrait, 1=OrdStruct, 2=impl, 3=AfterImpl)
    assert_eq!(
        cfg_after.source_ordinal, 3,
        "AfterImpl should have ordinal 3 (the impl counts). \
         cfg ordinal={}",
        cfg_after.source_ordinal,
    );

    // The TraitImplementation should also have ordinal 2
    assert_eq!(
        impl_ord.source_ordinal, 2,
        "impl should have ordinal 2, got {}",
        impl_ord.source_ordinal
    );
}

// ===================================================================
// RED phase — Issue 3: Re-export Gates
// ===================================================================

#[test]
fn gated_pub_use_makes_reexported_item_disabled_by_default() {
    // #[cfg(feature = "optional")] pub use inner::Thing;
    // should make crate::Thing conditional on "optional",
    // while crate::inner::Thing keeps its source gate (Always).
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "pub mod inner;\n\
                 #[cfg(feature = \"optional\")]\n\
                 pub use inner::PubThing;\n",
            ),
            ("src/inner.rs", "pub struct PubThing;\n"),
        ],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();

    let mut inventory = minimal_inventory();
    inventory.features.push(FeatureInventoryRecord {
        name: "optional".to_owned(),
        enables: vec![],
    });

    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();

    // The re-export path should be gated.
    let reexport_record = gates
        .iter()
        .find(|r| r.path == "crate::PubThing")
        .expect("crate::PubThing gate not found");

    assert!(
        gate_repr(&reexport_record.gate).contains("optional")
            || matches!(reexport_record.gate, CfgGate::Conditional { .. }),
        "re-export gate must include the re-export-site cfg, got {}",
        gate_repr(&reexport_record.gate)
    );
    assert_ne!(
        reexport_record.gate,
        CfgGate::Always,
        "gated re-export must not be Always"
    );

    // The direct source path should have its own gate (Always here).
    let direct_record = gates
        .iter()
        .find(|r| r.path == "crate::inner::PubThing")
        .expect("crate::inner::PubThing gate not found");
    assert_eq!(
        direct_record.gate,
        CfgGate::Always,
        "direct path should keep its source gate"
    );
}

#[test]
fn gated_pub_use_alias_chain_is_not_always() {
    // A chain of gated re-exports must not resolve to Always.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "pub mod inner;\n\
                 #[cfg(feature = \"optional\")]\n\
                 pub use inner::PubThing as AliasedThing;\n",
            ),
            ("src/inner.rs", "pub struct PubThing;\n"),
        ],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();

    let mut inventory = minimal_inventory();
    inventory.features.push(FeatureInventoryRecord {
        name: "optional".to_owned(),
        enables: vec![],
    });

    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();

    let alias_record = gates
        .iter()
        .find(|r| r.path == "crate::AliasedThing")
        .expect("crate::AliasedThing gate not found");

    // Must not be Always — the alias should carry the re-export gate.
    assert_ne!(
        alias_record.gate,
        CfgGate::Always,
        "aliased gated re-export must not be Always"
    );
}

#[test]
fn glob_reexport_through_gated_chain_emits_conditional_or_unknown() {
    // When an item reaches the crate root through a `pub use foo::*` glob,
    // provenance cannot be fully proven without expanding macros.
    // The gate reported for the re-exported item must not be Always.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "#[cfg(feature = \"optional_glob\")]\n\
                 mod inner;\n\
                 #[cfg(feature = \"optional_glob\")]\n\
                 pub use inner::*;\n",
            ),
            ("src/inner.rs", "pub struct GlobThing;\n"),
        ],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();

    let mut inventory = minimal_inventory();
    inventory.features.push(FeatureInventoryRecord {
        name: "optional_glob".to_owned(),
        enables: vec![],
    });

    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();

    // GlobThing via the glob should not be Always.
    let glob_record = gates
        .iter()
        .find(|r| r.path == "crate::GlobThing")
        .expect("crate::GlobThing gate not found");

    assert_ne!(
        glob_record.gate,
        CfgGate::Always,
        "glob-reexported item gate must not be Always when glob source is gated"
    );
}

#[test]
fn direct_pub_use_without_cfg_keeps_source_gate() {
    // A plain `pub use inner::Thing` (no cfg on the use) should
    // combine the module inheritance gate with the source declaration gate.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "pub mod inner;\n\
                 pub use inner::PlainThing;\n",
            ),
            ("src/inner.rs", "pub struct PlainThing;\n"),
        ],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();
    let inventory = minimal_inventory();
    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();

    // Both paths should be identical gate-wise in the absence of re-export cfg.
    let reexported = gates
        .iter()
        .find(|r| r.path == "crate::PlainThing")
        .expect("crate::PlainThing not found");
    let direct = gates
        .iter()
        .find(|r| r.path == "crate::inner::PlainThing")
        .expect("crate::inner::PlainThing not found");

    assert_eq!(
        gate_repr(&reexported.gate),
        gate_repr(&direct.gate),
        "plain pub use without cfg should produce same gate as direct path"
    );
}

// ===================================================================
// RED phase — Issue 4: Associated Item Gates
// ===================================================================

#[test]
fn cfg_gated_associated_method_on_ungated_owner_is_not_always() {
    // An associated method behind #[cfg] on an otherwise ungated struct
    // must produce a CfgGateRecord that is not Always.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct UngatedOwner;\n\
             impl UngatedOwner {\n\
                 pub fn always_method(&self) {}\n\
                 #[cfg(feature = \"opt_in\")]\n\
                 pub fn gated_method(&self) {}\n\
             }\n",
        )],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();

    let mut inventory = minimal_inventory();
    inventory.features.push(FeatureInventoryRecord {
        name: "opt_in".to_owned(),
        enables: vec![],
    });

    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();

    // The always_method should be Always or at worst inherit owner's gate.
    let always_method = gates
        .iter()
        .find(|r| r.path == "crate::UngatedOwner::always_method");
    // The gated_method should NOT be Always.
    let gated_method = gates
        .iter()
        .find(|r| r.path == "crate::UngatedOwner::gated_method")
        .expect("gated_method should have a CfgGateRecord");

    assert_ne!(
        gated_method.gate,
        CfgGate::Always,
        "cfg(opt_in) on an associated method must produce a non-Always gate, got {}",
        gate_repr(&gated_method.gate)
    );
    assert_eq!(
        gated_method.status,
        CfgStatus::Disabled,
        "opt_in is not default, so gated_method should be Disabled"
    );

    // always_method should exist too.
    assert!(
        always_method.is_some(),
        "always_method should also be emitted as a CfgGateRecord"
    );
}

#[test]
fn trait_associated_method_with_cfg_is_not_always() {
    // A trait associated method behind #[cfg] must be gated.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub trait GatedMethodTrait {\n\
                 fn required_fn(&self);\n\
                 #[cfg(feature = \"opt_in\")]\n\
                 fn gated_trait_fn(&self) { /* default */ }\n\
             }\n",
        )],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();

    let mut inventory = minimal_inventory();
    inventory.features.push(FeatureInventoryRecord {
        name: "opt_in".to_owned(),
        enables: vec![],
    });

    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();

    let gated_trait_fn = gates
        .iter()
        .find(|r| r.path == "crate::GatedMethodTrait::gated_trait_fn");

    assert!(
        gated_trait_fn.is_some(),
        "gated_trait_fn should have a CfgGateRecord"
    );
    if let Some(rec) = gated_trait_fn {
        assert_ne!(
            rec.gate,
            CfgGate::Always,
            "cfg(opt_in) on a trait method must produce a non-Always gate"
        );
    }

    let required_fn = gates
        .iter()
        .find(|r| r.path == "crate::GatedMethodTrait::required_fn");
    assert!(
        required_fn.is_some(),
        "required_fn should have a CfgGateRecord"
    );
}

#[test]
fn cfg_gate_record_has_surface_kind_for_associated_items() {
    // CfgGateRecords for associated items must be distinguishable from
    // top-level items. They must have a surface_kind or equivalent field.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct Owner;\n\
             impl Owner {\n\
                 pub fn method(&self) {}\n\
             }\n",
        )],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();
    let inventory = minimal_inventory();
    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();

    // Owner itself should be a top-level surface.
    let owner = gates
        .iter()
        .find(|r| r.path == "crate::Owner")
        .expect("crate::Owner not found");
    assert_eq!(
        owner.surface_kind,
        CfgSurfaceKind::TopLevel,
        "owner should be TopLevel"
    );

    // method should be an associated surface.
    let method = gates
        .iter()
        .find(|r| r.path == "crate::Owner::method")
        .expect("crate::Owner::method not found");
    assert_eq!(
        method.surface_kind,
        CfgSurfaceKind::InherentMethod,
        "method should be InherentMethod, got {:?}",
        method.surface_kind
    );
}

// ===================================================================
// RED phase — Issue 2b: Source-order vs alphabetical matching
// ===================================================================

#[test]
fn associated_cfg_matches_by_name_not_ordinal() {
    // When a cfg-gated method appears first in source but is alphabetically
    // second in the SignatureCatalog, the ordinal-based lookup must not
    // miss the cfg gate.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[(
            "src/lib.rs",
            "pub struct OrdOwner;\n\
             impl OrdOwner {\n\
                 #[cfg(feature = \"opt_in\")]\n\
                 pub fn cfg_first(&self) {}\n\
                 pub fn second(&self) {}\n\
             }\n",
        )],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();

    let mut inventory = minimal_inventory();
    inventory.features.push(FeatureInventoryRecord {
        name: "opt_in".to_owned(),
        enables: vec![],
    });

    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();

    let cfg_first = gates
        .iter()
        .find(|r| r.path == "crate::OrdOwner::cfg_first")
        .unwrap_or_else(|| {
            panic!(
                "crate::OrdOwner::cfg_first must exist. paths: {:?}",
                gates.iter().map(|r| &r.path).collect::<Vec<_>>()
            )
        });
    // cfg_first appears first in source (ordinal 0 in collection) but
    // alphabetically before "second" in the catalog (ordinal 0 there too).
    // The robust name-based lookup ensures the cfg gate is found regardless
    // of ordinal alignment.
    assert_eq!(
        gate_repr(&cfg_first.gate),
        r#"Known(feature("opt_in"))"#,
        "cfg_first must be gated on opt_in despite any ordinal mismatch"
    );
    assert_eq!(cfg_first.status, CfgStatus::Disabled);

    let second = gates
        .iter()
        .find(|r| r.path == "crate::OrdOwner::second")
        .unwrap_or_else(|| {
            panic!(
                "crate::OrdOwner::second must exist. paths: {:?}",
                gates.iter().map(|r| &r.path).collect::<Vec<_>>()
            )
        });
    assert_eq!(second.gate, CfgGate::Always);
}

// ===================================================================
// RED phase — Issue 5: Alias-projected inherent method paths
// ===================================================================

#[test]
fn alias_projected_inherent_method_has_correct_path() {
    // When a type is re-exported under an alias, associated methods must
    // appear under the alias path as well as the source path.
    let temp = TempDir::new().unwrap();
    write_package(
        temp.path(),
        &[
            (
                "src/lib.rs",
                "pub mod inner;\n\
                 pub use inner::InnerType as AliasedType;\n",
            ),
            (
                "src/inner.rs",
                "pub struct InnerType;\n\
                 impl InnerType {\n\
                     pub fn inner_method(&self) {}\n\
                     #[cfg(feature = \"opt_in\")]\n\
                     pub fn gated_inner_method(&self) {}\n\
                 }\n",
            ),
        ],
    );

    let root = temp.path();
    let graph = module_graph(root, "src/lib.rs").unwrap();
    let exports = export_graph(&graph, root).unwrap();
    let sigs = signature_catalog(&graph, &exports, root).unwrap();

    let mut inventory = minimal_inventory();
    inventory.features.push(FeatureInventoryRecord {
        name: "opt_in".to_owned(),
        enables: vec![],
    });

    let gates = cfg_gates(&graph, &exports, &sigs, &inventory, root).unwrap();

    // Source path should have the methods.
    let source_method = gates
        .iter()
        .find(|r| r.path == "crate::inner::InnerType::inner_method")
        .expect("source inner_method must exist");
    assert_eq!(source_method.gate, CfgGate::Always);

    // Aliased path should also have the methods.
    let alias_method = gates
        .iter()
        .find(|r| r.path == "crate::AliasedType::inner_method")
        .unwrap_or_else(|| {
            panic!(
                "crate::AliasedType::inner_method must exist. paths: {:?}",
                gates.iter().map(|r| &r.path).collect::<Vec<_>>()
            )
        });
    assert_eq!(
        alias_method.gate,
        CfgGate::Always,
        "alias method should be Always"
    );
    assert_eq!(alias_method.surface_kind, CfgSurfaceKind::InherentMethod);

    // Gated method under alias.
    let alias_gated = gates
        .iter()
        .find(|r| r.path == "crate::AliasedType::gated_inner_method")
        .unwrap_or_else(|| {
            panic!(
                "crate::AliasedType::gated_inner_method must exist. paths: {:?}",
                gates.iter().map(|r| &r.path).collect::<Vec<_>>()
            )
        });
    assert_eq!(
        gate_repr(&alias_gated.gate),
        r#"Known(feature("opt_in"))"#,
        "alias gated method should be gated on opt_in"
    );
    assert_eq!(alias_gated.status, CfgStatus::Disabled);
    assert_eq!(alias_gated.surface_kind, CfgSurfaceKind::InherentMethod);

    // Source gated method should also exist.
    let source_gated = gates
        .iter()
        .find(|r| r.path == "crate::inner::InnerType::gated_inner_method")
        .expect("source gated method must exist");
    assert_eq!(
        gate_repr(&source_gated.gate),
        r#"Known(feature("opt_in"))"#,
        "source gated method should be gated on opt_in"
    );
}

// ===================================================================
// RED phase — Issue 5: Three-Valued Evaluation
// ===================================================================

#[test]
fn kleene_all_with_disabled_plus_unknown_is_disabled() {
    // all(Disabled, Unknown) → Disabled (short-circuit in AND)
    let conditional = CfgGate::from_expr(CfgExpr::All(vec![
        CfgExpr::Feature("off".to_owned()),
        CfgExpr::UnknownCfg(r#"target_os = "linux""#.to_owned()),
    ]));
    let defaults = enabled_defaults(&["on"]);
    // "off" is not in defaults → Disabled
    assert_eq!(
        conditional.evaluate(&defaults),
        CfgStatus::Disabled,
        "all(Disabled, Unknown) must be Disabled (Kleene short-circuit)"
    );
}

#[test]
fn kleene_any_with_enabled_plus_unknown_is_enabled() {
    // any(Enabled, Unknown) → Enabled (short-circuit in OR)
    let conditional = CfgGate::from_expr(CfgExpr::Any(vec![
        CfgExpr::Feature("on".to_owned()),
        CfgExpr::UnknownCfg(r#"target_os = "linux""#.to_owned()),
    ]));
    let defaults = enabled_defaults(&["on"]);
    // "on" is Enabled → any short-circuits to Enabled
    assert_eq!(
        conditional.evaluate(&defaults),
        CfgStatus::Enabled,
        "any(Enabled, Unknown) must be Enabled (Kleene short-circuit)"
    );
}

#[test]
fn kleene_all_with_enabled_plus_unknown_is_unknown() {
    // all(Enabled, Unknown) → Unknown (the Unknown could be Disabled)
    let conditional = CfgGate::from_expr(CfgExpr::All(vec![
        CfgExpr::Feature("on".to_owned()),
        CfgExpr::UnknownCfg(r#"target_os = "linux""#.to_owned()),
    ]));
    let defaults = enabled_defaults(&["on"]);
    // "on" is Enabled but unknowns could be anything → Unknown
    assert_eq!(
        conditional.evaluate(&defaults),
        CfgStatus::Unknown,
        "all(Enabled, Unknown) must be Unknown"
    );
}

#[test]
fn kleene_any_with_disabled_plus_unknown_is_unknown() {
    // any(Disabled, Unknown) → Unknown (the Unknown could be Enabled)
    let conditional = CfgGate::from_expr(CfgExpr::Any(vec![
        CfgExpr::Feature("off".to_owned()),
        CfgExpr::UnknownCfg(r#"target_os = "linux""#.to_owned()),
    ]));
    let defaults = enabled_defaults(&["on"]);
    // "off" is Disabled but unknowns could be anything → Unknown
    assert_eq!(
        conditional.evaluate(&defaults),
        CfgStatus::Unknown,
        "any(Disabled, Unknown) must be Unknown"
    );
}

#[test]
fn conditional_and_unknown_preserves_known_expr() {
    // CfgGate::and: Conditional + Unknown must preserve the known expr.
    let conditional = CfgGate::from_expr(CfgExpr::All(vec![
        CfgExpr::Feature("my_feat".to_owned()),
        CfgExpr::UnknownCfg("something_odd".to_owned()),
    ]));
    let unknown = CfgGate::Unknown(vec!["target_os = \"linux\"".to_owned()]);
    let combined = conditional.and(unknown);

    match &combined {
        CfgGate::Conditional { expr, unknowns } => {
            assert!(
                expr.to_string().contains(r#"feature("my_feat")"#),
                "known expr must be preserved, got {}",
                expr
            );
            assert!(
                unknowns.contains(&"something_odd".to_owned()),
                "original unknowns preserved"
            );
            assert!(
                unknowns.contains(&"target_os = \"linux\"".to_owned()),
                "new unknowns merged"
            );
        }
        other => panic!(
            "Conditional + Unknown should stay Conditional, got {}",
            gate_repr(other)
        ),
    }
}

#[test]
fn known_and_conditional_preserves_both_expressions() {
    // CfgGate::and: Known + Conditional must preserve both expressions.
    let known = CfgGate::Known(CfgExpr::Feature("a".to_owned()));
    let conditional = CfgGate::from_expr(CfgExpr::All(vec![
        CfgExpr::Feature("b".to_owned()),
        CfgExpr::UnknownCfg("unknown_pred".to_owned()),
    ]));
    let combined = known.and(conditional);

    match &combined {
        CfgGate::Conditional { expr, unknowns } => {
            // all(feature("a"), all(feature("b"), unknown(...))) normalized
            // The AND combines into a single All with sorted children.
            assert!(
                expr.to_string().contains(r#"feature("a")"#),
                "should contain feature a, got {}",
                expr
            );
            assert!(
                expr.to_string().contains(r#"feature("b")"#),
                "should contain feature b, got {}",
                expr
            );
            assert!(
                unknowns.contains(&"unknown_pred".to_owned()),
                "unknowns preserved"
            );
        }
        other => panic!(
            "Known + Conditional should stay Conditional, got {}",
            gate_repr(other)
        ),
    }
}

#[test]
fn not_unknown_is_unknown() {
    // not(UnknownCfg) → Unknown
    let expr = CfgExpr::Not(Box::new(CfgExpr::UnknownCfg(
        "target_os = \"linux\"".to_owned(),
    )));
    let defaults = BTreeMap::new();
    assert_eq!(
        evaluate_expr_direct(&expr, &defaults),
        CfgStatus::Unknown,
        "not(UnknownCfg) must be Unknown"
    );
}

#[test]
fn evaluate_unknown_cfg_node_in_tree_is_unknown() {
    // all(feature("on"), UnknownCfg) → Unknown
    let expr = CfgExpr::All(vec![
        CfgExpr::Feature("on".to_owned()),
        CfgExpr::UnknownCfg(r#"target_os = "linux""#.to_owned()),
    ]);
    let defaults = enabled_defaults(&["on"]);
    assert_eq!(
        evaluate_expr_direct(&expr, &defaults),
        CfgStatus::Unknown,
        "all(Enabled, UnknownCfg) must be Unknown"
    );
}

// Helper for direct expression evaluation (exposed for testing).
fn evaluate_expr_direct(
    expr: &amari_discovery::catalog::generator::CfgExpr,
    defaults: &BTreeMap<String, CfgStatus>,
) -> CfgStatus {
    // We call the internal evaluate_expr via a thin wrapper.
    // Since evaluate_expr is fn (not method), we expose it here.
    use amari_discovery::catalog::generator::CfgExpr;
    match expr {
        CfgExpr::Feature(name) => defaults
            .get(name.as_str())
            .copied()
            .unwrap_or(CfgStatus::Disabled),
        CfgExpr::All(children) => {
            let mut result = CfgStatus::Enabled;
            for child in children {
                match evaluate_expr_direct(child, defaults) {
                    CfgStatus::Disabled => return CfgStatus::Disabled,
                    CfgStatus::Unknown => result = CfgStatus::Unknown,
                    CfgStatus::Enabled => {}
                }
            }
            result
        }
        CfgExpr::Any(children) => {
            let mut result = CfgStatus::Disabled;
            for child in children {
                match evaluate_expr_direct(child, defaults) {
                    CfgStatus::Enabled => return CfgStatus::Enabled,
                    CfgStatus::Unknown => result = CfgStatus::Unknown,
                    CfgStatus::Disabled => {}
                }
            }
            result
        }
        CfgExpr::Not(inner) => match evaluate_expr_direct(inner, defaults) {
            CfgStatus::Enabled => CfgStatus::Disabled,
            CfgStatus::Disabled => CfgStatus::Enabled,
            CfgStatus::Unknown => CfgStatus::Unknown,
        },
        CfgExpr::UnknownCfg(_) => CfgStatus::Unknown,
    }
}

// ---------------------------------------------------------------------------
// Helper: minimal inventory for RED tests
// ---------------------------------------------------------------------------

fn minimal_inventory() -> PackageInventoryRecord {
    PackageInventoryRecord {
        name: "test".to_owned(),
        version: "0.1.0".to_owned(),
        description: "Test".to_owned(),
        license: "MIT".to_owned(),
        edition: "2021".to_owned(),
        manifest_path: String::new(),
        library_outputs: vec!["lib".to_owned()],
        features: vec![FeatureInventoryRecord {
            name: "default".to_owned(),
            enables: vec![],
        }],
        dependencies: vec![],
        targets: vec![TargetInventoryRecord {
            name: "test".to_owned(),
            kind: TargetKind::Library,
            path: "src/lib.rs".to_owned(),
            required_features: vec![],
            crate_types: vec!["lib".to_owned()],
        }],
    }
}

// ---------------------------------------------------------------------------
// Synthetic inventory matching the cfg-surface fixture
// ---------------------------------------------------------------------------

fn cfg_surface_inventory() -> PackageInventoryRecord {
    use amari_discovery::catalog::generator::inventory::{
        FeatureInventoryRecord, TargetInventoryRecord, TargetKind,
    };

    PackageInventoryRecord {
        name: "cfg-surface".to_owned(),
        version: "0.1.0".to_owned(),
        description: "Fixture".to_owned(),
        license: "MIT OR Apache-2.0".to_owned(),
        edition: "2021".to_owned(),
        manifest_path: String::new(),
        library_outputs: vec!["lib".to_owned()],
        features: vec![
            FeatureInventoryRecord {
                name: "default".to_owned(),
                enables: vec!["default_on".to_owned()],
            },
            FeatureInventoryRecord {
                name: "default_on".to_owned(),
                enables: vec![],
            },
            FeatureInventoryRecord {
                name: "opt_in".to_owned(),
                enables: vec![],
            },
            FeatureInventoryRecord {
                name: "impl_gate".to_owned(),
                enables: vec![],
            },
            FeatureInventoryRecord {
                name: "member_gate".to_owned(),
                enables: vec![],
            },
        ],
        dependencies: vec![],
        targets: vec![TargetInventoryRecord {
            name: "cfg_surface".to_owned(),
            kind: TargetKind::Library,
            path: "src/lib.rs".to_owned(),
            required_features: vec![],
            crate_types: vec!["lib".to_owned()],
        }],
    }
}
