// SPDX-License-Identifier: MIT OR Apache-2.0

//! Discovery surfacing for the geometric, holographic, and hybrid inverse
//! guidance backends (0.25 Cohort 9, Task 49): the state embedding, search
//! graph, geometric strategy, trace encoding, trace recall, and hybrid
//! strategy capabilities are curated, marked experimental, searchable,
//! detailed, graph-connected, and recommended — as metadata only, with no
//! probes and no guidance dependency added to `amari-discovery`.
//!
//! Gating note: the backends are `amari-rewrite`'s non-default `network` and
//! `holographic-guidance` features, and the hybrid bridge is compiled under
//! `any(feature = "neural", feature = "holographic-guidance")`. Discovery
//! never enables any of them. The hybrid OR-gate and optional-lane availability
//! are DESCRIPTIVE metadata, not unconditional Cargo feature steps. The test
//! target is gated exactly like the other `rewrite_discovery_*` targets
//! (`standard-probes`, the default).

#![cfg(feature = "standard-probes")]

use std::collections::BTreeSet;

use amari_discovery::{
    CandidateRetriever, CapabilityRecord, Catalog, GoalSpec, GraphPath, PlanGenerator, PlanStep,
    ProjectKind, ProjectSnapshot, RankingComponents, SnapshotState, StabilityTier,
};
use assert_cmd::Command;
use serde_json::Value;

const STATE_EMBEDDING: &str = "amari:amari-rewrite:geometric:state-embedding";
const SEARCH_GRAPH: &str = "amari:amari-rewrite:geometric:search-graph";
const GEOMETRIC_STRATEGY: &str = "amari:amari-rewrite:geometric:strategy";
const TRACE_ENCODING: &str = "amari:amari-rewrite:holographic:trace-encoding";
const TRACE_RECALL: &str = "amari:amari-rewrite:holographic:trace-recall";
const HYBRID_STRATEGY: &str = "amari:amari-rewrite:hybrid:strategy";

const NETWORK_FEATURE: &str = "amari-rewrite:network";
const HOLOGRAPHIC_FEATURE: &str = "amari-rewrite:holographic-guidance";
const INVERSE_PREDECESSORS: &str = "amari:amari-rewrite:inverse:predecessors";
const BACKWARD_SEARCH: &str = "amari:amari-rewrite:inverse:backward-search";
const NEURAL_STRATEGY: &str = "amari:amari-rewrite:neural:strategy";

const CAPABILITIES: [&str; 6] = [
    STATE_EMBEDDING,
    SEARCH_GRAPH,
    GEOMETRIC_STRATEGY,
    TRACE_ENCODING,
    TRACE_RECALL,
    HYBRID_STRATEGY,
];

/// The exact catalog probe DESCRIPTOR set at the Task 49 base commit
/// (`576399e`), derived from `catalog/probes.toml` before any catalog edit.
/// The separate `engine_derives_executable_state_from_the_private_registry`
/// test in `probe_engine.rs` pins the 24 executable handlers (one descriptor
/// is intentionally unregistered).
const BASE_PROBE_IDS: [&str; 25] = [
    "amari-probe:cgt:nim-sum:v1",
    "amari-probe:core:geometric-product:v1",
    "amari-probe:dual:polynomial-derivative:v1",
    "amari-probe:holographic:recall:v2",
    "amari-probe:holographic:superposition:v1",
    "amari-probe:network:shortest-path:v1",
    "amari-probe:optimization:pareto-front:v1",
    "amari-probe:rewrite:analysis:v1",
    "amari-probe:rewrite:backward-search:v1",
    "amari-probe:rewrite:bidirectional-search:v1",
    "amari-probe:rewrite:completion:v1",
    "amari-probe:rewrite:infer-rule:v1",
    "amari-probe:rewrite:inverse-analysis:v1",
    "amari-probe:rewrite:languages:v1",
    "amari-probe:rewrite:normalize:v1",
    "amari-probe:rewrite:predecessors:v1",
    "amari-probe:rewrite:preimages:v1",
    "amari-probe:rewrite:residual-replay:v1",
    "amari-probe:rewrite:specialize:v1",
    "amari-probe:rewrite:symbolic-predecessors:v1",
    "amari-probe:rewrite:synthesize-inverse:v1",
    "amari-probe:surcomplex:rational-division:v1",
    "amari-probe:surreal:rational-arithmetic:v1",
    "amari-probe:tropical:shortest-path:v1",
    "amari-probe:tropical:viterbi:v1",
];

fn command_json(arguments: &[&str]) -> Value {
    let output = Command::cargo_bin("amari")
        .unwrap()
        .args(arguments)
        .arg("--json")
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    serde_json::from_slice(&output).unwrap()
}

fn capability(id: &str) -> CapabilityRecord {
    Catalog::embedded()
        .unwrap()
        .capabilities()
        .iter()
        .find(|capability| capability.id.to_string() == id)
        .unwrap_or_else(|| panic!("missing capability {id}"))
        .clone()
}

fn result_ids(value: &Value) -> Vec<String> {
    value["data"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap().to_string())
        .collect()
}

fn graph_relations(value: &Value) -> Vec<(String, String, String)> {
    value["data"]["relations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|relation| {
            (
                relation["from"].as_str().unwrap().to_string(),
                relation["to"].as_str().unwrap().to_string(),
                relation["kind"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

fn empty_snapshot(hash: &str) -> ProjectSnapshot {
    ProjectSnapshot {
        project_hash: hash.to_owned(),
        project_kind: ProjectKind::Unknown,
        signals: Vec::new(),
        cargo: None,
        rust: None,
        platform: None,
        npm: None,
        typescript: None,
        file_count: 0,
        total_bytes: 0,
        state: SnapshotState::Complete,
        warnings: Vec::new(),
        files: Vec::new(),
    }
}

fn expected_feature_refs(id: &str) -> Vec<String> {
    let features: &[&str] = match id {
        STATE_EMBEDDING | SEARCH_GRAPH | GEOMETRIC_STRATEGY => &[NETWORK_FEATURE],
        TRACE_ENCODING | TRACE_RECALL => &[HOLOGRAPHIC_FEATURE],
        HYBRID_STRATEGY => &[],
        other => panic!("unexpected guidance capability {other}"),
    };
    features
        .iter()
        .map(|feature| (*feature).to_owned())
        .collect()
}

fn required_symbols(id: &str) -> &'static [&'static str] {
    match id {
        STATE_EMBEDDING => &["amari_rewrite::network::InverseStateEmbeddingV1"],
        SEARCH_GRAPH => &[
            "amari_rewrite::network::InverseSearchGraph",
            "amari_rewrite::network::InverseGraphConfig",
        ],
        GEOMETRIC_STRATEGY => &[
            "amari_rewrite::network::GeometricStrategy",
            "amari_rewrite::network::GeometricStrategyConfig",
        ],
        TRACE_ENCODING => &[
            "amari_rewrite::holographic::HolographicEncoder",
            "amari_rewrite::holographic::RecallTrace",
        ],
        TRACE_RECALL => &[
            "amari_rewrite::holographic::TraceCorpus",
            "amari_rewrite::holographic::TraceRecall",
            "amari_rewrite::holographic::RecallConfig",
        ],
        HYBRID_STRATEGY => &[
            "amari_rewrite::inverse::hybrid::HybridStrategy",
            "amari_rewrite::inverse::hybrid::HybridEvaluation",
            "amari_rewrite::inverse::hybrid::HybridWeights",
            "amari_rewrite::inverse::hybrid::HybridConfig",
            "amari_rewrite::inverse::hybrid::RecallProjection",
        ],
        other => panic!("unexpected guidance capability {other}"),
    }
}

#[test]
fn semantic_catalog_marks_guidance_capabilities_experimental_and_probe_free() {
    let catalog = Catalog::embedded().unwrap();
    let known: BTreeSet<String> = catalog
        .capabilities()
        .iter()
        .map(|capability| capability.id.to_string())
        .collect();
    for id in CAPABILITIES {
        assert!(known.contains(id), "missing guidance capability {id}");
    }

    for id in CAPABILITIES {
        let record = capability(id);
        assert_eq!(
            record.stability,
            StabilityTier::Experimental,
            "{id} must carry the experimental designation"
        );
        assert_eq!(
            record.crate_refs,
            vec!["amari-rewrite".to_string()],
            "{id} must name only the rewrite crate"
        );
        assert_eq!(
            record.feature_refs,
            expected_feature_refs(id),
            "{id} must advertise exactly its documented feature gate"
        );
        assert!(
            record.probe_refs.is_empty(),
            "{id} must not claim a probe: guidance metadata is not executable here"
        );
        assert!(
            !record.symbol_refs.is_empty(),
            "{id} must name the symbols it curates"
        );
        assert!(!record.aliases.is_empty(), "{id} must offer search aliases");
        assert!(
            !record.concepts.is_empty(),
            "{id} must classify its concepts"
        );
    }
}

#[test]
fn structural_catalog_resolves_every_guidance_symbol_and_feature() {
    let catalog = Catalog::embedded().unwrap();
    let rewrite = catalog
        .crates()
        .iter()
        .find(|record| record.name == "amari-rewrite")
        .expect("amari-rewrite structural record");
    for feature in ["network", "holographic-guidance", "neural"] {
        assert!(
            rewrite
                .features
                .iter()
                .any(|candidate| candidate.name == feature),
            "the structural catalog must record the {feature} feature"
        );
    }

    let item_paths: BTreeSet<&str> = rewrite
        .items
        .iter()
        .map(|item| item.path.as_str())
        .collect();
    for id in CAPABILITIES {
        let record = capability(id);
        for symbol in required_symbols(id) {
            assert!(
                record
                    .symbol_refs
                    .iter()
                    .any(|reference| reference == symbol),
                "{id} must curate the required symbol {symbol}: {:?}",
                record.symbol_refs
            );
            assert!(
                item_paths.contains(symbol),
                "{id} references {symbol}, which the structural catalog does not record"
            );
        }
        for symbol in &record.symbol_refs {
            assert!(
                item_paths.contains(symbol.as_str()),
                "{id} references {symbol}, which the structural catalog does not record"
            );
        }
    }

    // Discovery never indexes itself, so its own dependency set can never
    // silently acquire a guidance backend through catalog metadata.
    assert!(
        catalog
            .crates()
            .iter()
            .all(|record| record.name != "amari-discovery"),
        "amari-discovery must never index itself"
    );
}

#[test]
fn search_finds_each_guidance_capability() {
    let queries = [
        (STATE_EMBEDDING, "state embedding"),
        (SEARCH_GRAPH, "inverse search graph"),
        (GEOMETRIC_STRATEGY, "geometric guidance"),
        (TRACE_ENCODING, "trace encoding"),
        (TRACE_RECALL, "trace recall"),
        (HYBRID_STRATEGY, "hybrid guidance"),
    ];
    for (id, query) in queries {
        let ids = result_ids(&command_json(&["discover", "search", query]));
        assert!(
            ids.iter().any(|result| result == id),
            "search for `{query}` should find {id}: {ids:?}"
        );
    }
}

#[test]
fn detail_exposes_guidance_feature_gates_and_experimental_tier() {
    for id in CAPABILITIES {
        let value = command_json(&["discover", "detail", id]);
        assert_eq!(value["data"]["id"], id);
        assert_eq!(
            value["data"]["stability"], "experimental",
            "{id} must render the experimental tier"
        );
        let features: Vec<String> = value["data"]["feature_refs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|feature| feature.as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            features,
            expected_feature_refs(id),
            "detail for {id} must expose its exact feature gate"
        );
        let description = value["data"]["description"].as_str().unwrap();
        match id {
            TRACE_ENCODING => {
                assert!(description.contains("replay-validated"));
                assert!(description.contains("encoder digest"));
                assert!(description.contains("admission"));
                assert!(description.contains("independent"));
            }
            TRACE_RECALL => {
                assert!(description.contains("ranked trace"));
                assert!(description.contains("RecallLane"));
            }
            HYBRID_STRATEGY => {
                assert!(description.contains("available, supplied lanes"));
                assert!(description.contains(
                    "Either feature `neural` or `holographic-guidance` exposes the module \
                     (`network` implies `neural`)"
                ));
                assert!(description.contains(
                    "each optional lane retains its own feature gate and unsupplied or \
                     uncompiled lanes are `Missing`"
                ));
            }
            _ => {}
        }
        let probes = value["data"]["probe_refs"].as_array().unwrap();
        assert!(
            probes.is_empty(),
            "detail for {id} must not advertise a probe: {probes:?}"
        );
    }
}

#[test]
fn graph_carries_every_required_guidance_relation() {
    let expected = [
        (STATE_EMBEDDING, SEARCH_GRAPH, "produces_input_for"),
        (SEARCH_GRAPH, GEOMETRIC_STRATEGY, "composes_with"),
        (GEOMETRIC_STRATEGY, BACKWARD_SEARCH, "composes_with"),
        (TRACE_ENCODING, TRACE_RECALL, "produces_input_for"),
        (TRACE_RECALL, HYBRID_STRATEGY, "composes_with"),
        (NEURAL_STRATEGY, HYBRID_STRATEGY, "composes_with"),
        (GEOMETRIC_STRATEGY, HYBRID_STRATEGY, "composes_with"),
        (HYBRID_STRATEGY, BACKWARD_SEARCH, "composes_with"),
    ];
    for (from, to, kind) in expected {
        let relations = graph_relations(&command_json(&["discover", "graph", from]));
        assert!(
            relations
                .iter()
                .any(|(source, target, actual)| source == from && target == to && actual == kind),
            "graph for {from} must carry {kind} -> {to}: {relations:?}"
        );
    }
    for from in [GEOMETRIC_STRATEGY, HYBRID_STRATEGY, INVERSE_PREDECESSORS] {
        let relations = graph_relations(&command_json(&["discover", "graph", from]));
        assert!(
            !relations.iter().any(|(source, target, kind)| {
                kind == "composes_with"
                    && ((matches!(source.as_str(), GEOMETRIC_STRATEGY | HYBRID_STRATEGY)
                        && target == INVERSE_PREDECESSORS)
                        || (source == INVERSE_PREDECESSORS
                            && matches!(target.as_str(), GEOMETRIC_STRATEGY | HYBRID_STRATEGY)))
            }),
            "{from} must not claim composition with the legacy BackwardSearch iterator"
        );
    }
}

#[test]
fn recommendation_recalls_hybrid_strategy_for_hybrid_guidance_goal() {
    let catalog = Catalog::embedded().unwrap();
    let retriever = CandidateRetriever::default();
    let candidates = retriever
        .retrieve(
            &catalog,
            &empty_snapshot("hybrid-guidance-project"),
            "hybrid inverse guidance combining geometric neural and holographic recall lanes",
        )
        .unwrap();
    let strategy = candidates
        .iter()
        .find(|candidate| candidate.capability_id.to_string() == HYBRID_STRATEGY)
        .unwrap_or_else(|| {
            panic!(
                "recommendation recall must surface {HYBRID_STRATEGY}: {:?}",
                candidates
                    .iter()
                    .map(|candidate| candidate.capability_id.to_string())
                    .collect::<Vec<_>>()
            )
        });
    assert!(
        strategy.retrieval_score.is_finite() && strategy.retrieval_score > 0.0,
        "recalled hybrid strategy must carry a finite positive retrieval score"
    );
    assert!(
        strategy
            .matched_evidence
            .iter()
            .any(|term| term == "hybrid"),
        "recalled hybrid strategy must match on the hybrid evidence: {:?}",
        strategy.matched_evidence
    );
}

#[test]
fn recall_only_hybrid_plan_has_no_unconditional_feature_steps() {
    let catalog = Catalog::embedded().unwrap();
    let target: amari_discovery::CapabilityId = HYBRID_STRATEGY.parse().unwrap();
    let candidate = amari_discovery::RankedCandidate {
        capability_id: target.clone(),
        path: GraphPath {
            target: target.clone(),
            source_seed: target.clone(),
            capabilities: vec![target],
            steps: Vec::new(),
            total_cost: 0.0,
        },
        components: RankingComponents {
            applicability: 1.0,
            evidence: 0.8,
            effort: 0.2,
            maturity: 1.0,
            runtime: 0.2,
            platform: 1.0,
            verification: 0.5,
            risk: 0.1,
        },
        objectives: [0.0, 0.2, 0.2, 0.0, 0.2, 0.0, 0.5, 0.1],
        confidence: 0.8,
        evidence: Vec::new(),
        validated_assumptions: Vec::new(),
    };
    let context = amari_discovery::PlanningContext {
        snapshot: ProjectSnapshot {
            project_kind: ProjectKind::RustCargo,
            ..empty_snapshot("recall-only-hybrid")
        },
        goal: GoalSpec {
            statement: "use holographic recall guidance only".to_owned(),
            constraints: Vec::new(),
        },
        probe_results: Vec::new(),
    };
    let plan = PlanGenerator::default()
        .generate(&catalog, &context, &candidate)
        .expect("the recall-only hybrid plan generates");
    let features: Vec<String> = plan
        .steps
        .iter()
        .filter_map(|step| match step {
            PlanStep::Feature { feature, .. } => Some(feature.clone()),
            _ => None,
        })
        .collect();
    assert!(
        features.is_empty(),
        "the OR-gate is descriptive metadata and must not become two \
         unconditional Cargo feature actions: {features:?}"
    );
}

#[test]
fn catalog_probe_descriptor_set_is_unchanged() {
    let catalog = Catalog::embedded().unwrap();
    let actual: BTreeSet<String> = catalog
        .probes()
        .iter()
        .map(|probe| probe.id.to_string())
        .collect();
    let expected: BTreeSet<String> = BASE_PROBE_IDS.iter().map(|id| (*id).to_owned()).collect();
    assert_eq!(
        actual, expected,
        "Task 49 is metadata only: the catalog probe descriptor set must equal the base set"
    );
    assert_eq!(catalog.probes().len(), BASE_PROBE_IDS.len());
}

fn collect_manifest_strings(value: &toml::Value, output: &mut Vec<String>) {
    match value {
        toml::Value::String(text) => output.push(text.clone()),
        toml::Value::Array(values) => {
            for value in values {
                collect_manifest_strings(value, output);
            }
        }
        toml::Value::Table(table) => {
            for value in table.values() {
                collect_manifest_strings(value, output);
            }
        }
        _ => {}
    }
}

fn assert_no_guidance_features(manifest: &str) {
    let value: toml::Value = toml::from_str(manifest).expect("valid discovery manifest");
    for (section, dependencies) in [
        ("dependencies", value.get("dependencies")),
        (
            "workspace.dependencies",
            value
                .get("workspace")
                .and_then(|workspace| workspace.get("dependencies")),
        ),
    ] {
        let Some(rewrite) = dependencies
            .and_then(|table| table.get("amari-rewrite"))
            .and_then(toml::Value::as_table)
        else {
            continue;
        };
        for forbidden in ["neural", "network", "holographic-guidance"] {
            assert!(
                !rewrite
                    .get("features")
                    .and_then(toml::Value::as_array)
                    .is_some_and(|features| {
                        features
                            .iter()
                            .any(|feature| feature.as_str() == Some(forbidden))
                    }),
                "the {section} amari-rewrite dependency must not enable {forbidden}"
            );
        }
    }
    let mut strings = Vec::new();
    if let Some(features) = value.get("features").and_then(toml::Value::as_table) {
        for definition in features.values() {
            collect_manifest_strings(definition, &mut strings);
        }
    }
    for reference in strings {
        for forbidden in ["neural", "network", "holographic-guidance"] {
            assert!(
                reference != format!("amari-rewrite/{forbidden}"),
                "feature forwarding must not enable {forbidden}: {reference}"
            );
        }
    }
}

#[test]
fn discovery_never_enables_a_guidance_feature() {
    assert_no_guidance_features(include_str!("../Cargo.toml"));
    assert_no_guidance_features(include_str!("../../Cargo.toml"));
    let manifest = include_str!("../Cargo.toml").to_ascii_lowercase();
    for forbidden in ["katakuri", "borsalino"] {
        assert!(
            !manifest.contains(forbidden),
            "amari-discovery must not depend on {forbidden}"
        );
    }
}

#[test]
fn manifest_guard_rejects_feature_forwarding() {
    let forwarded = r#"
[dependencies]
amari-rewrite = { workspace = true, features = ["completion"] }
[features]
ai = ["amari-rewrite/holographic-guidance"]
"#;
    let error = std::panic::catch_unwind(|| assert_no_guidance_features(forwarded))
        .expect_err("the feature forward must be rejected");
    let message = error
        .downcast_ref::<String>()
        .expect("a formatted feature rejection");
    assert!(
        message.contains("feature forwarding must not enable holographic-guidance"),
        "unexpected rejection: {message}"
    );
    let inherited = r#"
[workspace.dependencies]
amari-rewrite = { path = "amari-rewrite", features = ["holographic-guidance"] }
[dependencies]
amari-rewrite = { workspace = true, features = ["completion"] }
"#;
    let error = std::panic::catch_unwind(|| assert_no_guidance_features(inherited))
        .expect_err("inherited workspace guidance must be rejected");
    let message = error
        .downcast_ref::<String>()
        .expect("a formatted feature rejection");
    assert!(
        message.contains(
            "workspace.dependencies amari-rewrite dependency must not enable holographic-guidance"
        ),
        "unexpected rejection: {message}"
    );
}
