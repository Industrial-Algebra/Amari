// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded inverse-rewrite predecessor probe parity and limits.

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    DiscoveryError, ProbeEngine, ProbeEngineLimits, ProbeExecution, RewritePredecessorsOutput,
    RewritePredecessorsRequest, RewriteRule, RewriteTerm,
};
use amari_rewrite::{
    inverse::BackwardSearch,
    trs::{Rule, Term, TermSystem},
};
use serde_json::json;

const PREDECESSORS: &str = "amari-probe:rewrite:predecessors:v1";

fn var(name: &str) -> RewriteTerm {
    RewriteTerm::Variable {
        name: name.to_owned(),
    }
}

fn sym(name: &str, arguments: Vec<RewriteTerm>) -> RewriteTerm {
    RewriteTerm::Symbol {
        name: name.to_owned(),
        arguments,
    }
}

fn rule(lhs: RewriteTerm, rhs: RewriteTerm) -> RewriteRule {
    RewriteRule { lhs, rhs }
}

fn request() -> RewritePredecessorsRequest {
    RewritePredecessorsRequest {
        target: sym("a", vec![]),
        rules: vec![rule(
            sym("add", vec![sym("zero", vec![]), var("X")]),
            var("X"),
        )],
        max_depth: 1,
        max_results: 16,
        max_frontier: 16,
    }
}

fn execute(engine: &ProbeEngine, request: RewritePredecessorsRequest) -> ProbeExecution {
    engine
        .execute(
            &PREDECESSORS.parse().unwrap(),
            &serde_json::to_value(request).unwrap(),
        )
        .unwrap()
}

#[test]
fn one_step_predecessors_match_bounded_backward_search() {
    let system = TermSystem::new(vec![Rule::new(
        Term::sym("add", [Term::constant("zero"), Term::var("X")]),
        Term::var("X"),
    )
    .unwrap()]);
    let expected = BackwardSearch::new(&system, Term::constant("a"))
        .max_depth(1)
        .max_nodes(16)
        .collect::<Vec<_>>();
    let engine = ProbeEngine::new().unwrap();
    let first = execute(&engine, request());
    let second = execute(&engine, request());
    let output: RewritePredecessorsOutput = serde_json::from_value(first.output.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(expected.len(), 1);
    assert_eq!(
        output.predecessors,
        vec![sym("add", vec![sym("zero", vec![]), sym("a", vec![])])]
    );
    assert!(!output.truncated);
    assert_eq!(first.resources.operations, 1);
    assert_eq!(first.resources.nodes, 4);
    assert_eq!(first.resources.iterations, 1);
}

#[test]
fn output_is_deduplicated_and_canonically_ordered() {
    let duplicate = rule(sym("add", vec![sym("zero", vec![]), var("X")]), var("X"));
    let request = RewritePredecessorsRequest {
        target: sym("a", vec![]),
        rules: vec![
            rule(sym("add", vec![var("X"), sym("zero", vec![])]), var("X")),
            duplicate.clone(),
            duplicate,
        ],
        max_depth: 1,
        max_results: 16,
        max_frontier: 16,
    };
    let output: RewritePredecessorsOutput =
        serde_json::from_value(execute(&ProbeEngine::new().unwrap(), request).output).unwrap();

    assert_eq!(output.predecessors.len(), 2);
    assert!(output.predecessors[0] < output.predecessors[1]);
}

#[test]
fn result_cap_returns_a_typed_deterministic_truncation() {
    let request = RewritePredecessorsRequest {
        target: sym("a", vec![]),
        rules: vec![
            rule(sym("left", vec![var("X")]), var("X")),
            rule(sym("right", vec![var("X")]), var("X")),
        ],
        max_depth: 1,
        max_results: 1,
        max_frontier: 16,
    };
    let first = execute(&ProbeEngine::new().unwrap(), request.clone());
    let second = execute(&ProbeEngine::new().unwrap(), request);
    let output: RewritePredecessorsOutput = serde_json::from_value(first.output.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(output.predecessors.len(), 1);
    assert!(output.truncated);
}

#[test]
fn depth_result_and_frontier_bounds_are_enforced() {
    for (mut request, expected) in [
        (
            {
                let mut request = request();
                request.max_depth = 17;
                request
            },
            "depth",
        ),
        (
            {
                let mut request = request();
                request.max_results = 0;
                request
            },
            "results",
        ),
        (
            {
                let mut request = request();
                request.max_frontier = 0;
                request
            },
            "frontier",
        ),
    ] {
        assert!(matches!(
            ProbeEngine::new().unwrap().execute(
                &PREDECESSORS.parse().unwrap(),
                &serde_json::to_value(&mut request).unwrap()
            ),
            Err(DiscoveryError::InvalidInput(message) | DiscoveryError::LimitExceeded(message))
                if message.contains(expected)
        ));
    }

    let mut frontier = request();
    frontier.max_depth = 3;
    frontier.max_frontier = 1;
    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &PREDECESSORS.parse().unwrap(),
            &serde_json::to_value(frontier).unwrap()
        ),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("frontier")
    ));
}

#[test]
fn cumulative_node_and_term_depth_limits_are_enforced() {
    let input = serde_json::to_value(request()).unwrap();
    assert!(matches!(
        ProbeEngine::with_limits(ProbeEngineLimits {
            max_nodes: 3,
            ..ProbeEngineLimits::default()
        })
        .unwrap()
        .execute(&PREDECESSORS.parse().unwrap(), &input),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("nodes")
    ));

    let mut deep = sym("leaf", vec![]);
    for _ in 0..64 {
        deep = sym("f", vec![deep]);
    }
    let deep_request = RewritePredecessorsRequest {
        target: deep,
        rules: Vec::new(),
        max_depth: 1,
        max_results: 1,
        max_frontier: 1,
    };
    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &PREDECESSORS.parse().unwrap(),
            &serde_json::to_value(deep_request).unwrap()
        ),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("depth")
    ));
}

#[test]
fn reverse_variable_duplication_is_rejected_as_unbounded_growth() {
    let input = json!({
        "target": { "kind": "symbol", "name": "a", "arguments": [] },
        "rules": [{
            "lhs": { "kind": "symbol", "name": "pair", "arguments": [
                { "kind": "variable", "name": "X" },
                { "kind": "variable", "name": "X" }
            ] },
            "rhs": { "kind": "variable", "name": "X" }
        }],
        "max_depth": 1,
        "max_results": 16,
        "max_frontier": 16
    });

    assert!(matches!(
        ProbeEngine::new()
            .unwrap()
            .execute(&PREDECESSORS.parse().unwrap(), &input),
        Err(DiscoveryError::InvalidInput(message)) if message.contains("duplicates")
    ));
}

#[test]
fn cooperative_input_work_iteration_and_output_limits_are_enforced() {
    let input = serde_json::to_value(request()).unwrap();
    for (limits, expected) in [
        (
            ProbeEngineLimits {
                max_input_bytes: 8,
                ..ProbeEngineLimits::default()
            },
            "input bytes",
        ),
        (
            ProbeEngineLimits {
                max_operations: 1,
                ..ProbeEngineLimits::default()
            },
            "operations",
        ),
        (
            ProbeEngineLimits {
                max_iterations: 1,
                ..ProbeEngineLimits::default()
            },
            "iterations",
        ),
        (
            ProbeEngineLimits {
                max_output_bytes: 1,
                ..ProbeEngineLimits::default()
            },
            "output bytes",
        ),
    ] {
        if expected == "operations" || expected == "iterations" {
            let mut deeper = request();
            deeper.max_depth = 2;
            let error = ProbeEngine::with_limits(limits)
                .unwrap()
                .execute(
                    &PREDECESSORS.parse().unwrap(),
                    &serde_json::to_value(deeper).unwrap(),
                )
                .unwrap_err();
            assert!(
                matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
                "unexpected error for {expected}: {error}"
            );
        } else {
            let error = ProbeEngine::with_limits(limits)
                .unwrap()
                .execute(&PREDECESSORS.parse().unwrap(), &input)
                .unwrap_err();
            assert!(
                matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
                "unexpected error for {expected}: {error}"
            );
        }
    }
}
