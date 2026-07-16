// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded capability-graph expansion for planner candidates.

use std::collections::{BTreeMap, BTreeSet};

use amari_core::Multivector;
use amari_discovery::{
    CapabilityGraphExpander, CapabilityId, Catalog, GraphConstraints, GraphExpansionState,
    GraphLimit, GraphLimits, RelationCostPolicy,
};
use amari_network::GeometricNetwork;
use amari_tropical::verified::{MinPlus, VerifiedTropicalNumber};

fn id(value: &str) -> CapabilityId {
    value.parse().unwrap()
}

fn path_target<'a>(
    expansion: &'a amari_discovery::GraphExpansion,
    target: &str,
) -> Option<&'a amari_discovery::GraphPath> {
    expansion
        .paths
        .iter()
        .find(|path| path.target.to_string() == target)
}

#[test]
fn prerequisite_and_composition_relations_expand_candidates() {
    let catalog = Catalog::embedded().unwrap();
    let expander = CapabilityGraphExpander::default();
    let seeds = [
        id("amari:amari-core:rotor:rotation"),
        id("amari:amari-tropical:paths:shortest-path"),
    ];

    let expansion = expander
        .expand(&catalog, &seeds, &GraphConstraints::default())
        .unwrap();

    let prerequisite = path_target(&expansion, "amari:amari-core:product:geometric-product")
        .expect("rotor expansion should include its geometric-product prerequisite");
    assert_eq!(prerequisite.source_seed, seeds[0]);
    assert!(prerequisite.steps.iter().any(|step| step.reverse));

    let composition = path_target(
        &expansion,
        "amari:amari-network:paths:geometric-shortest-path",
    )
    .expect("tropical shortest paths should compose with geometric paths");
    assert_eq!(composition.source_seed, seeds[1]);
    assert!(composition
        .steps
        .iter()
        .any(|step| step.relation_kind == "composes_with"));
    assert_eq!(expansion.state, GraphExpansionState::Complete);
}

#[test]
fn invalidating_constraints_block_capabilities() {
    let catalog = Catalog::embedded().unwrap();
    let blocked = id("amari:amari-network:paths:geometric-shortest-path");
    let constraints = GraphConstraints {
        blocked_capabilities: BTreeSet::from([blocked.clone()]),
    };

    let expansion = CapabilityGraphExpander::default()
        .expand(
            &catalog,
            &[id("amari:amari-tropical:paths:shortest-path")],
            &constraints,
        )
        .unwrap();

    assert!(path_target(&expansion, &blocked.to_string()).is_none());
    assert_eq!(expansion.blocked_capabilities, vec![blocked]);
}

#[test]
fn node_limit_preserves_deterministic_partial_paths() {
    let catalog = Catalog::embedded().unwrap();
    let expander = CapabilityGraphExpander::new(
        GraphLimits {
            max_nodes: 1,
            max_depth: 4,
        },
        RelationCostPolicy::default(),
    )
    .unwrap();

    let expansion = expander
        .expand(
            &catalog,
            &[id("amari:amari-tropical:paths:shortest-path")],
            &GraphConstraints::default(),
        )
        .unwrap();

    assert_eq!(expansion.paths.len(), 1);
    assert_eq!(expansion.paths[0].capabilities.len(), 1);
    assert!(matches!(
        expansion.state,
        GraphExpansionState::Partial { ref limits }
            if limits == &[GraphLimit::NodeCount { max: 1, observed: 2 }]
    ));
}

#[test]
fn depth_limit_preserves_frontier_paths() {
    let catalog = Catalog::embedded().unwrap();
    let expander = CapabilityGraphExpander::new(
        GraphLimits {
            max_nodes: 16,
            max_depth: 0,
        },
        RelationCostPolicy::default(),
    )
    .unwrap();

    let expansion = expander
        .expand(
            &catalog,
            &[id("amari:amari-tropical:paths:shortest-path")],
            &GraphConstraints::default(),
        )
        .unwrap();

    assert_eq!(expansion.paths.len(), 1);
    assert!(matches!(
        expansion.state,
        GraphExpansionState::Partial { ref limits }
            if limits == &[GraphLimit::Depth { max: 0, observed: 1 }]
    ));
}

#[test]
fn edge_costs_must_be_finite_and_nonnegative() {
    let negative = RelationCostPolicy {
        default_cost: 1.0,
        relation_costs: BTreeMap::from([("composes_with".to_owned(), -1.0)]),
    };
    assert!(CapabilityGraphExpander::new(GraphLimits::default(), negative).is_err());

    let non_finite = RelationCostPolicy {
        default_cost: f64::NAN,
        relation_costs: BTreeMap::new(),
    };
    assert!(CapabilityGraphExpander::new(GraphLimits::default(), non_finite).is_err());
}

#[test]
fn network_shortest_path_and_min_plus_costs_select_the_same_route() {
    let mut network = GeometricNetwork::<1, 0, 0>::new();
    let start = network.add_node(Multivector::scalar(0.0));
    let middle = network.add_node(Multivector::scalar(1.0));
    let finish = network.add_node(Multivector::scalar(2.0));
    network.add_edge(start, middle, 1.25).unwrap();
    network.add_edge(middle, finish, 2.0).unwrap();
    network.add_edge(start, finish, 5.0).unwrap();

    let (path, network_cost) = network.shortest_path(start, finish).unwrap().unwrap();
    assert_eq!(path, vec![start, middle, finish]);

    type MinPlusNumber = VerifiedTropicalNumber<f64, MinPlus>;
    let indirect = MinPlusNumber::tropical_one()
        .tropical_mul(MinPlusNumber::new(1.25))
        .tropical_mul(MinPlusNumber::new(2.0));
    let direct = MinPlusNumber::tropical_one().tropical_mul(MinPlusNumber::new(5.0));
    let selected = indirect.tropical_add(direct);

    assert_eq!(selected.value(), 3.25);
    assert_eq!(selected.value(), network_cost);
}

#[test]
fn graph_output_is_deterministic_and_costs_are_valid() {
    let catalog = Catalog::embedded().unwrap();
    let seeds = [id("amari:amari-holographic:memory:retrieval")];
    let expander = CapabilityGraphExpander::default();

    let first = expander
        .expand(&catalog, &seeds, &GraphConstraints::default())
        .unwrap();
    let second = expander
        .expand(&catalog, &seeds, &GraphConstraints::default())
        .unwrap();

    assert_eq!(
        serde_json::to_vec(&first).unwrap(),
        serde_json::to_vec(&second).unwrap()
    );
    assert!(first
        .paths
        .iter()
        .all(|path| path.total_cost.is_finite() && path.total_cost >= 0.0));
}
