// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded rewrite-normalization probe parity and limits.

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    DiscoveryError, ProbeEngine, ProbeEngineLimits, ProbeExecution, RewriteNormalizeOutput,
    RewriteNormalizeRequest, RewriteRule, RewriteTerm,
};
use amari_rewrite::trs::{Rule, Term, TermSystem};
use serde_json::json;

const NORMALIZE: &str = "amari-probe:rewrite:normalize:v1";

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

fn request() -> RewriteNormalizeRequest {
    RewriteNormalizeRequest {
        term: sym("add", vec![sym("zero", vec![]), sym("a", vec![])]),
        rules: vec![rule(
            sym("add", vec![sym("zero", vec![]), var("X")]),
            var("X"),
        )],
        max_steps: 1,
    }
}

fn execute(engine: &ProbeEngine, request: RewriteNormalizeRequest) -> ProbeExecution {
    engine
        .execute(
            &NORMALIZE.parse().unwrap(),
            &serde_json::to_value(request).unwrap(),
        )
        .unwrap()
}

#[test]
fn one_step_output_matches_term_system_apply_once() {
    let direct_term = Term::sym("add", [Term::constant("zero"), Term::constant("a")]);
    let direct_rule = Rule::new(
        Term::sym("add", [Term::constant("zero"), Term::var("X")]),
        Term::var("X"),
    )
    .unwrap();
    let expected = TermSystem::new(vec![direct_rule])
        .apply_once(&direct_term)
        .unwrap()
        .unwrap();
    let engine = ProbeEngine::new().unwrap();
    let first = execute(&engine, request());
    let second = execute(&engine, request());
    let output: RewriteNormalizeOutput = serde_json::from_value(first.output.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(expected, Term::constant("a"));
    assert_eq!(output.normal_form, sym("a", vec![]));
    assert_eq!(output.steps, 1);
    assert_eq!(first.resources.operations, 4);
    assert_eq!(first.resources.nodes, 3);
    assert_eq!(first.resources.iterations, 2);
}

#[test]
fn invalid_and_expanding_rules_are_rejected_before_execution() {
    for (input, expected) in [
        (
            json!({
                "term": { "kind": "symbol", "name": "a", "arguments": [] },
                "rules": [{
                    "lhs": { "kind": "variable", "name": "X" },
                    "rhs": { "kind": "variable", "name": "Y" }
                }],
                "max_steps": 1
            }),
            "does not occur",
        ),
        (
            json!({
                "term": { "kind": "symbol", "name": "a", "arguments": [] },
                "rules": [{
                    "lhs": { "kind": "variable", "name": "X" },
                    "rhs": { "kind": "symbol", "name": "f", "arguments": [
                        { "kind": "variable", "name": "X" }
                    ] }
                }],
                "max_steps": 1
            }),
            "expanding",
        ),
    ] {
        assert!(matches!(
            ProbeEngine::new()
                .unwrap()
                .execute(&NORMALIZE.parse().unwrap(), &input),
            Err(DiscoveryError::InvalidInput(message)) if message.contains(expected)
        ));
    }
}

#[test]
fn step_exhaustion_is_a_typed_limit_error() {
    let request = RewriteNormalizeRequest {
        term: sym("a", vec![]),
        rules: vec![
            rule(sym("a", vec![]), sym("b", vec![])),
            rule(sym("b", vec![]), sym("a", vec![])),
        ],
        max_steps: 1,
    };

    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &NORMALIZE.parse().unwrap(),
            &serde_json::to_value(request).unwrap()
        ),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("step limit")
    ));
}

#[test]
fn term_node_and_depth_limits_are_enforced() {
    let input = serde_json::to_value(request()).unwrap();
    assert!(matches!(
        ProbeEngine::with_limits(ProbeEngineLimits {
            max_nodes: 2,
            ..ProbeEngineLimits::default()
        })
        .unwrap()
        .execute(&NORMALIZE.parse().unwrap(), &input),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("nodes")
    ));

    let mut deep = sym("leaf", vec![]);
    for _ in 0..64 {
        deep = sym("f", vec![deep]);
    }
    let deep_request = RewriteNormalizeRequest {
        term: deep,
        rules: Vec::new(),
        max_steps: 1,
    };
    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &NORMALIZE.parse().unwrap(),
            &serde_json::to_value(deep_request).unwrap()
        ),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("depth")
    ));
}

#[test]
fn zero_and_oversized_step_limits_are_rejected() {
    for max_steps in [0, 4_097] {
        let mut request = request();
        request.max_steps = max_steps;
        assert!(matches!(
            ProbeEngine::new().unwrap().execute(
                &NORMALIZE.parse().unwrap(),
                &serde_json::to_value(request).unwrap()
            ),
            Err(DiscoveryError::InvalidInput(message) | DiscoveryError::LimitExceeded(message))
                if message.contains("steps")
        ));
    }
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
                max_operations: 3,
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
        let error = ProbeEngine::with_limits(limits)
            .unwrap()
            .execute(&NORMALIZE.parse().unwrap(), &input)
            .unwrap_err();
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
            "unexpected error for {expected}: {error}"
        );
    }
}
