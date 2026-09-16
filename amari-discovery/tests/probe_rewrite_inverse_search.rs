//! Engine-level contract tests for the inverse-search rewrite probes
//! (0.25 Cohort 3, Task 17).

#![cfg(feature = "standard-probes")]

use amari_discovery::{
    ProbeEngine, RewriteBackwardSearchOutput, RewriteBackwardSearchRequest,
    RewriteBidirectionalSearchOutput, RewriteGuidance, RewriteSearchMode, RewriteTerm,
};
use serde_json::json;

const BACKWARD: &str = "amari-probe:rewrite:backward-search:v1";
const BIDIRECTIONAL: &str = "amari-probe:rewrite:bidirectional-search:v1";

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

fn add_zero_rule() -> serde_json::Value {
    json!([{
        "lhs": {"kind": "symbol", "name": "add", "arguments": [
            {"kind": "variable", "name": "X"},
            {"kind": "symbol", "name": "zero", "arguments": []}
        ]},
        "rhs": {"kind": "variable", "name": "X"}
    }])
}

fn backward_request(guidance: serde_json::Value) -> serde_json::Value {
    json!({
        "target": {"kind": "symbol", "name": "a", "arguments": []},
        "goal": {"kind": "symbol", "name": "add", "arguments": [
            {"kind": "symbol", "name": "a", "arguments": []},
            {"kind": "symbol", "name": "zero", "arguments": []}
        ]},
        "rules": add_zero_rule(),
        "mode": "breadth_first",
        "guidance": guidance,
        "max_depth": 8
    })
}

fn run(probe: &str, input: &serde_json::Value) -> serde_json::Value {
    ProbeEngine::new()
        .unwrap()
        .execute(&probe.parse().unwrap(), input)
        .unwrap()
        .output
}

#[test]
fn backward_search_witness_matches_library_search() {
    let output: RewriteBackwardSearchOutput = serde_json::from_value(run(
        BACKWARD,
        &backward_request(json!({"kind": "complete_within_limits"})),
    ))
    .unwrap();
    assert_eq!(output.outcome, "witness");
    assert_eq!(output.steps.len(), 1);
    // Library agreement: the same search through amari-rewrite finds
    // the same one-step witness.
    let rules = vec![amari_rewrite::trs::Rule::new(
        amari_rewrite::trs::Term::sym(
            "add",
            vec![
                amari_rewrite::trs::Term::var("X"),
                amari_rewrite::trs::Term::constant("zero"),
            ],
        ),
        amari_rewrite::trs::Term::var("X"),
    )
    .unwrap()];
    let system = amari_rewrite::trs::TermSystem::new(rules);
    let library = amari_rewrite::inverse::BackwardExplorer::new(
        &system,
        amari_rewrite::inverse::InverseSearchConfig::new(
            8,
            4_096,
            16_384,
            4_096,
            64,
            4_096,
            65_536,
            50_000,
            1 << 20,
            1 << 20,
        )
        .unwrap(),
    )
    .search(
        &amari_rewrite::trs::Term::constant("a"),
        &amari_rewrite::trs::Term::sym(
            "add",
            vec![
                amari_rewrite::trs::Term::constant("a"),
                amari_rewrite::trs::Term::constant("zero"),
            ],
        ),
        amari_rewrite::inverse::SearchMode::BreadthFirst,
    )
    .unwrap();
    let amari_rewrite::inverse::BackwardSearchOutcome::Witness(derivation) = library else {
        panic!("library must witness");
    };
    assert_eq!(derivation.steps.len(), output.steps.len());
    assert_eq!(
        derivation.steps[0].provenance.rule_id.to_string(),
        output.steps[0].provenance.rule_id
    );
}

#[test]
fn backward_search_partial_and_pruned_approximate_are_truthful() {
    // Ascending chain: add(X, zero) => X never closes backward from
    // an unrelated goal, so depth 1 is partial.
    let mut request = backward_request(json!({"kind": "complete_within_limits"}));
    request["goal"] = json!({"kind": "symbol", "name": "z", "arguments": []});
    request["max_depth"] = json!(1);
    let output: RewriteBackwardSearchOutput =
        serde_json::from_value(run(BACKWARD, &request)).unwrap();
    assert_eq!(output.outcome, "partial");
    assert_eq!(output.depth_reached, 1);
    assert_eq!(output.frontier.len(), 1);
    // Beam pruning over a branching closure: b => x, c => x.
    let pruned = json!({
        "target": {"kind": "symbol", "name": "x", "arguments": []},
        "goal": {"kind": "symbol", "name": "z", "arguments": []},
        "rules": [
            {"lhs": {"kind": "symbol", "name": "b", "arguments": []},
             "rhs": {"kind": "symbol", "name": "x", "arguments": []}},
            {"lhs": {"kind": "symbol", "name": "c", "arguments": []},
             "rhs": {"kind": "symbol", "name": "x", "arguments": []}}
        ],
        "mode": "breadth_first",
        "guidance": {"kind": "heuristic_pruning", "beam_width": 1},
        "max_depth": 8
    });
    let output: RewriteBackwardSearchOutput =
        serde_json::from_value(run(BACKWARD, &pruned)).unwrap();
    assert_eq!(output.outcome, "approximate");
    assert_eq!(output.dropped_candidates, 1);
    assert!(output.exhaustion_authority.is_none());
}

#[test]
fn backward_search_certifies_closed_frontier_exhaustion() {
    let closed = json!({
        "target": {"kind": "symbol", "name": "a", "arguments": []},
        "goal": {"kind": "symbol", "name": "z", "arguments": []},
        "rules": [
            {"lhs": {"kind": "symbol", "name": "a", "arguments": []},
             "rhs": {"kind": "symbol", "name": "b", "arguments": []}},
            {"lhs": {"kind": "symbol", "name": "b", "arguments": []},
             "rhs": {"kind": "symbol", "name": "a", "arguments": []}}
        ],
        "mode": "breadth_first",
        "guidance": {"kind": "complete_within_limits"},
        "max_depth": 8
    });
    let output: RewriteBackwardSearchOutput =
        serde_json::from_value(run(BACKWARD, &closed)).unwrap();
    assert_eq!(output.outcome, "exhausted");
    assert_eq!(
        output.exhaustion_authority.as_deref(),
        Some("ClosedSymbolicSearch")
    );
}

#[test]
fn backward_search_input_is_strict_and_bounded() {
    // Unknown fields are rejected.
    let mut bad = backward_request(json!({"kind": "complete_within_limits"}));
    bad["bogus"] = json!(1);
    assert!(ProbeEngine::new()
        .unwrap()
        .execute(&BACKWARD.parse().unwrap(), &bad)
        .is_err());
    // Depth above the probe ceiling is rejected.
    let mut bad = backward_request(json!({"kind": "complete_within_limits"}));
    bad["max_depth"] = json!(17);
    assert!(ProbeEngine::new()
        .unwrap()
        .execute(&BACKWARD.parse().unwrap(), &bad)
        .is_err());
    // Beam width zero is rejected.
    let mut bad = backward_request(json!({"kind": "heuristic_pruning", "beam_width": 0}));
    assert!(ProbeEngine::new()
        .unwrap()
        .execute(&BACKWARD.parse().unwrap(), &bad)
        .is_err());
    // Unknown mode strings are rejected.
    let mut bad = backward_request(json!({"kind": "complete_within_limits"}));
    bad["mode"] = json!("random_walk");
    assert!(ProbeEngine::new()
        .unwrap()
        .execute(&BACKWARD.parse().unwrap(), &bad)
        .is_err());
}

#[test]
fn backward_search_ordering_is_deterministic_in_both_modes() {
    for mode in ["breadth_first", "cost_first"] {
        let mut request = backward_request(json!({"kind": "complete_within_limits"}));
        request["goal"] = json!({"kind": "symbol", "name": "z", "arguments": []});
        request["mode"] = json!(mode);
        request["max_depth"] = json!(2);
        let first = run(BACKWARD, &request);
        let second = run(BACKWARD, &request);
        assert_eq!(first, second);
    }
}

#[test]
fn bidirectional_search_meets_and_matches_library() {
    let request = json!({
        "source": {"kind": "symbol", "name": "add", "arguments": [
            {"kind": "symbol", "name": "a", "arguments": []},
            {"kind": "symbol", "name": "zero", "arguments": []}
        ]},
        "goal": {"kind": "symbol", "name": "a", "arguments": []},
        "rules": add_zero_rule(),
        "max_depth": 8
    });
    let output: RewriteBidirectionalSearchOutput =
        serde_json::from_value(run(BIDIRECTIONAL, &request)).unwrap();
    assert_eq!(output.outcome, "witness");
    assert_eq!(output.forward_steps.len(), 1);
    assert_eq!(output.backward_steps.len(), 0);
    assert_eq!(
        output.meeting_forward,
        Some(sym("a", vec![])),
        "the forward half meets at the normal form"
    );
    // Library agreement.
    let rules = vec![amari_rewrite::trs::Rule::new(
        amari_rewrite::trs::Term::sym(
            "add",
            vec![
                amari_rewrite::trs::Term::var("X"),
                amari_rewrite::trs::Term::constant("zero"),
            ],
        ),
        amari_rewrite::trs::Term::var("X"),
    )
    .unwrap()];
    let system = amari_rewrite::trs::TermSystem::new(rules);
    let library = amari_rewrite::inverse::BidirectionalExplorer::new(
        &system,
        amari_rewrite::inverse::InverseSearchConfig::new(
            8,
            4_096,
            16_384,
            4_096,
            64,
            4_096,
            65_536,
            50_000,
            1 << 20,
            1 << 20,
        )
        .unwrap(),
    )
    .search(
        &amari_rewrite::trs::Term::sym(
            "add",
            vec![
                amari_rewrite::trs::Term::constant("a"),
                amari_rewrite::trs::Term::constant("zero"),
            ],
        ),
        &amari_rewrite::trs::Term::constant("a"),
    )
    .unwrap();
    assert!(matches!(
        library,
        amari_rewrite::inverse::BidirectionalSearchOutcome::Witness(_)
    ));
}

#[test]
fn bidirectional_search_certifies_exhaustion_and_rejects_bad_input() {
    let closed = json!({
        "source": {"kind": "symbol", "name": "a", "arguments": []},
        "goal": {"kind": "symbol", "name": "z", "arguments": []},
        "rules": [
            {"lhs": {"kind": "symbol", "name": "a", "arguments": []},
             "rhs": {"kind": "symbol", "name": "b", "arguments": []}},
            {"lhs": {"kind": "symbol", "name": "b", "arguments": []},
             "rhs": {"kind": "symbol", "name": "a", "arguments": []}}
        ],
        "max_depth": 8
    });
    let output: RewriteBidirectionalSearchOutput =
        serde_json::from_value(run(BIDIRECTIONAL, &closed)).unwrap();
    assert_eq!(output.outcome, "exhausted");
    assert_eq!(
        output.exhaustion_authority.as_deref(),
        Some("ClosedSymbolicSearch")
    );
    let mut bad = closed.clone();
    bad["max_depth"] = json!(0);
    assert!(ProbeEngine::new()
        .unwrap()
        .execute(&BIDIRECTIONAL.parse().unwrap(), &bad)
        .is_err());
    let mut bad = closed.clone();
    bad["extra"] = json!(true);
    assert!(ProbeEngine::new()
        .unwrap()
        .execute(&BIDIRECTIONAL.parse().unwrap(), &bad)
        .is_err());
}

// Keep the request DTO referenced for schema drift detection.
#[allow(dead_code)]
fn _request_dto_anchor(_: Option<RewriteBackwardSearchRequest>) {}
