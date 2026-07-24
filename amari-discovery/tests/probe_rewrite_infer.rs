// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded rewrite-rule inference probe parity and limits.

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    DiscoveryError, ProbeEngine, ProbeEngineLimits, ProbeExecution, RewriteExample,
    RewriteInferRuleOutput, RewriteInferRuleRequest, RewriteRule, RewriteTerm,
};
use amari_rewrite::{synthesis::infer_rule, trs::Term};
use serde_json::json;

const INFER_RULE: &str = "amari-probe:rewrite:infer-rule:v1";

fn sym(name: &str, arguments: Vec<RewriteTerm>) -> RewriteTerm {
    RewriteTerm::Symbol {
        name: name.to_owned(),
        arguments,
    }
}

fn example(symbol: &str) -> RewriteExample {
    RewriteExample {
        before: sym("add", vec![sym("zero", vec![]), sym(symbol, vec![])]),
        after: sym(symbol, vec![]),
    }
}

fn request() -> RewriteInferRuleRequest {
    RewriteInferRuleRequest {
        examples: vec![example("a"), example("b")],
    }
}

fn execute(engine: &ProbeEngine, request: RewriteInferRuleRequest) -> ProbeExecution {
    engine
        .execute(
            &INFER_RULE.parse().unwrap(),
            &serde_json::to_value(request).unwrap(),
        )
        .unwrap()
}

#[test]
fn inferred_rule_matches_direct_amari_rewrite_inference() {
    let direct_examples = vec![
        (
            Term::sym("add", [Term::constant("zero"), Term::constant("a")]),
            Term::constant("a"),
        ),
        (
            Term::sym("add", [Term::constant("zero"), Term::constant("b")]),
            Term::constant("b"),
        ),
    ];
    let direct = infer_rule(&direct_examples).unwrap();
    let engine = ProbeEngine::new().unwrap();
    let first = execute(&engine, request());
    let second = execute(&engine, request());
    let output: RewriteInferRuleOutput = serde_json::from_value(first.output.clone()).unwrap();

    assert_eq!(first, second);
    assert_eq!(
        direct.lhs(),
        &Term::sym("add", [Term::constant("zero"), Term::var("_I0")])
    );
    assert_eq!(direct.rhs(), &Term::var("_I0"));
    assert_eq!(
        output.rule,
        RewriteRule {
            lhs: sym(
                "add",
                vec![
                    sym("zero", vec![]),
                    RewriteTerm::Variable {
                        name: "_I0".to_owned(),
                    },
                ],
            ),
            rhs: RewriteTerm::Variable {
                name: "_I0".to_owned(),
            },
        }
    );
    assert_eq!(first.resources.operations, 16);
    assert_eq!(first.resources.nodes, 12);
    assert_eq!(first.resources.iterations, 2);
}

#[test]
fn empty_and_oversized_example_sets_are_rejected() {
    let empty = RewriteInferRuleRequest {
        examples: Vec::new(),
    };
    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &INFER_RULE.parse().unwrap(),
            &serde_json::to_value(empty).unwrap()
        ),
        Err(DiscoveryError::InvalidInput(message)) if message.contains("at least one")
    ));

    let oversized = RewriteInferRuleRequest {
        examples: vec![example("a"); 257],
    };
    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &INFER_RULE.parse().unwrap(),
            &serde_json::to_value(oversized).unwrap()
        ),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("example count")
    ));
}

#[test]
fn generated_rules_with_duplicate_rhs_variables_are_rejected() {
    let request = RewriteInferRuleRequest {
        examples: vec![
            RewriteExample {
                before: sym("f", vec![sym("a", vec![])]),
                after: sym("pair", vec![sym("a", vec![]), sym("a", vec![])]),
            },
            RewriteExample {
                before: sym("f", vec![sym("b", vec![])]),
                after: sym("pair", vec![sym("b", vec![]), sym("b", vec![])]),
            },
        ],
    };

    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &INFER_RULE.parse().unwrap(),
            &serde_json::to_value(request).unwrap()
        ),
        Err(DiscoveryError::InvalidInput(message)) if message.contains("duplicates variable")
    ));
}

#[test]
fn generated_rule_nodes_are_included_in_the_cooperative_limit() {
    let input = serde_json::to_value(request()).unwrap();
    assert!(matches!(
        ProbeEngine::with_limits(ProbeEngineLimits {
            max_nodes: 11,
            ..ProbeEngineLimits::default()
        })
        .unwrap()
        .execute(&INFER_RULE.parse().unwrap(), &input),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("generated rule nodes")
    ));
}

#[test]
fn malformed_examples_and_deep_terms_are_rejected() {
    let malformed = json!({
        "examples": [{
            "before": { "kind": "unknown", "name": "a" },
            "after": { "kind": "symbol", "name": "a", "arguments": [] }
        }]
    });
    assert!(matches!(
        ProbeEngine::new()
            .unwrap()
            .execute(&INFER_RULE.parse().unwrap(), &malformed),
        Err(DiscoveryError::InvalidInput(_))
    ));

    let mut deep = sym("leaf", vec![]);
    for _ in 0..64 {
        deep = sym("f", vec![deep]);
    }
    let deep_request = RewriteInferRuleRequest {
        examples: vec![RewriteExample {
            before: deep,
            after: sym("a", vec![]),
        }],
    };
    assert!(matches!(
        ProbeEngine::new().unwrap().execute(
            &INFER_RULE.parse().unwrap(),
            &serde_json::to_value(deep_request).unwrap()
        ),
        Err(DiscoveryError::LimitExceeded(message)) if message.contains("depth")
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
                max_operations: 15,
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
            .execute(&INFER_RULE.parse().unwrap(), &input)
            .unwrap_err();
        assert!(
            matches!(error, DiscoveryError::LimitExceeded(ref message) if message.contains(expected)),
            "unexpected error for {expected}: {error}"
        );
    }
}
