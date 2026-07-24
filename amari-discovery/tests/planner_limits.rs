// SPDX-License-Identifier: MIT OR Apache-2.0

//! Adversarial planner resource-limit and domain-outcome contracts.

use std::collections::BTreeSet;

use amari_discovery::{
    CapabilityGraphExpander, CapabilityId, Catalog, DiscoveryError, DiscoveryOutcome,
    GraphConstraints, GraphExpansionState, GraphLimit, GraphLimits, NormalizationLimits,
    PlanNormalizer, RelationCostPolicy,
};
use serde_json::{json, Value};

fn id(value: &str) -> CapabilityId {
    value.parse().unwrap()
}

#[test]
fn graph_configuration_has_non_bypassable_hard_ceilings() {
    assert!(CapabilityGraphExpander::new(
        GraphLimits {
            max_nodes: GraphLimits::MAX_ALLOWED_NODES,
            max_depth: GraphLimits::MAX_ALLOWED_DEPTH,
        },
        RelationCostPolicy::default(),
    )
    .is_ok());

    for limits in [
        GraphLimits {
            max_nodes: GraphLimits::MAX_ALLOWED_NODES + 1,
            max_depth: GraphLimits::default().max_depth,
        },
        GraphLimits {
            max_nodes: GraphLimits::default().max_nodes,
            max_depth: GraphLimits::MAX_ALLOWED_DEPTH + 1,
        },
    ] {
        assert!(matches!(
            CapabilityGraphExpander::new(limits, RelationCostPolicy::default()),
            Err(DiscoveryError::LimitExceeded(_))
        ));
    }

    assert!(matches!(
        CapabilityGraphExpander::new(
            GraphLimits {
                max_nodes: 0,
                max_depth: 0,
            },
            RelationCostPolicy::default(),
        ),
        Err(DiscoveryError::InvalidInput(_))
    ));
}

#[test]
fn graph_limit_keeps_deterministic_partial_path_evidence() {
    let seed = id("amari:amari-tropical:paths:shortest-path");
    let expander = CapabilityGraphExpander::new(
        GraphLimits {
            max_nodes: 1,
            max_depth: GraphLimits::default().max_depth,
        },
        RelationCostPolicy::default(),
    )
    .unwrap();
    let catalog = Catalog::embedded().unwrap();

    let first = expander
        .expand(
            &catalog,
            std::slice::from_ref(&seed),
            &GraphConstraints::default(),
        )
        .unwrap();
    let second = expander
        .expand(
            &catalog,
            std::slice::from_ref(&seed),
            &GraphConstraints::default(),
        )
        .unwrap();

    assert_eq!(first, second);
    assert_eq!(first.paths.len(), 1);
    assert_eq!(first.paths[0].target, seed);
    assert_eq!(first.paths[0].capabilities, vec![seed]);
    assert!(matches!(
        first.state,
        GraphExpansionState::Partial { ref limits }
            if limits == &[GraphLimit::NodeCount { max: 1, observed: 2 }]
    ));
}

#[test]
fn normalization_configuration_has_non_bypassable_hard_ceilings() {
    assert!(PlanNormalizer::new(NormalizationLimits {
        max_plan_steps: NormalizationLimits::MAX_ALLOWED_PLAN_STEPS,
        max_rewrites: NormalizationLimits::MAX_ALLOWED_REWRITES,
    })
    .is_ok());

    for limits in [
        NormalizationLimits {
            max_plan_steps: NormalizationLimits::MAX_ALLOWED_PLAN_STEPS + 1,
            max_rewrites: 1,
        },
        NormalizationLimits {
            max_plan_steps: 1,
            max_rewrites: NormalizationLimits::MAX_ALLOWED_REWRITES + 1,
        },
    ] {
        assert!(matches!(
            PlanNormalizer::new(limits),
            Err(DiscoveryError::LimitExceeded(_))
        ));
    }
}

#[test]
fn planner_limit_dtos_reject_unknown_authority_fields() {
    assert!(serde_json::from_value::<GraphLimits>(json!({
        "max_nodes": 8,
        "max_depth": 2,
        "unbounded": true
    }))
    .is_err());
    assert!(serde_json::from_value::<NormalizationLimits>(json!({
        "max_plan_steps": 8,
        "max_rewrites": 32,
        "max_trace_bytes": 999999999
    }))
    .is_err());
}

#[test]
fn non_recommendation_domain_conditions_are_successful_typed_values() {
    let outcomes = [
        DiscoveryOutcome::<Value>::NoApplicableCapability {
            evidence: Vec::new(),
        },
        DiscoveryOutcome::<Value>::InsufficientEvidence {
            missing: vec!["project_inspection_partial".to_owned()],
        },
        DiscoveryOutcome::<Value>::Blocked {
            reasons: vec!["platform_incompatible".to_owned()],
        },
    ];

    for outcome in outcomes {
        let bytes = serde_json::to_vec(&outcome).unwrap();
        let decoded: DiscoveryOutcome<Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded, outcome);
        let value = serde_json::to_value(outcome).unwrap();
        assert!(matches!(
            value["status"].as_str(),
            Some("no_applicable_capability" | "insufficient_evidence" | "blocked")
        ));
    }
}

#[test]
fn domain_outcome_protocol_rejects_unknown_fields() {
    let malformed = [
        json!({
            "status": "no_applicable_capability",
            "data": {"evidence": [], "error": true}
        }),
        json!({
            "status": "insufficient_evidence",
            "data": {"missing": [], "execute": "cargo build"}
        }),
        json!({
            "status": "blocked",
            "data": {"reasons": [], "override": true}
        }),
    ];

    for value in malformed {
        assert!(serde_json::from_value::<DiscoveryOutcome<Value>>(value).is_err());
    }
}

#[test]
fn blocked_seed_is_preserved_as_typed_graph_evidence() {
    let seed = id("amari:amari-tropical:paths:shortest-path");
    let expansion = CapabilityGraphExpander::default()
        .expand(
            &Catalog::embedded().unwrap(),
            std::slice::from_ref(&seed),
            &GraphConstraints {
                blocked_capabilities: BTreeSet::from([seed.clone()]),
            },
        )
        .unwrap();

    assert!(expansion.paths.is_empty());
    assert_eq!(expansion.blocked_capabilities, vec![seed]);
}
