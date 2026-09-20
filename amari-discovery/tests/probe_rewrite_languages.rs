// SPDX-License-Identifier: MIT OR Apache-2.0

//! Engine-level contract tests for the regular-language rewrite
//! probe (0.25 Cohort 4, Task 22).

#![cfg(feature = "standard-probes")]

use amari_discovery::{ProbeEngine, RewriteLanguagesOutput};
use serde_json::json;

const LANGUAGES: &str = "amari-probe:rewrite:languages:v1";

/// Parity automaton: states q0/q1, z -> q0, s(q0) -> q1, s(q1) ->
/// q0; finals [q0]. Accepts even-length s-chains over z.
fn parity_automaton() -> serde_json::Value {
    json!({
        "alphabet": [{"name": "z", "arity": 0}, {"name": "s", "arity": 1}],
        "states": ["q0", "q1"],
        "transitions": [
            {"symbol": "z", "children": [], "parent": "q0"},
            {"symbol": "s", "children": ["q0"], "parent": "q1"},
            {"symbol": "s", "children": ["q1"], "parent": "q0"}
        ],
        "finals": ["q0"]
    })
}

fn s_z(n: usize) -> serde_json::Value {
    let mut term = json!({"kind": "symbol", "name": "z", "arguments": []});
    for _ in 0..n {
        term = json!({"kind": "symbol", "name": "s", "arguments": [term]});
    }
    term
}

fn run(input: &serde_json::Value) -> serde_json::Value {
    ProbeEngine::new()
        .unwrap()
        .execute(&LANGUAGES.parse().unwrap(), input)
        .unwrap()
        .output
}

fn run_typed(input: &serde_json::Value) -> RewriteLanguagesOutput {
    serde_json::from_value(run(input)).unwrap()
}

#[test]
fn membership_accepts_and_rejects() {
    let accepted = run_typed(&json!({
        "operation": "membership",
        "automaton": parity_automaton(),
        "term": s_z(4)
    }));
    assert_eq!(accepted.outcome, "ok");
    assert_eq!(accepted.accepted, Some(true));
    let rejected = run_typed(&json!({
        "operation": "membership",
        "automaton": parity_automaton(),
        "term": s_z(3)
    }));
    assert_eq!(rejected.accepted, Some(false));
}

#[test]
fn emptiness_and_witness() {
    let nonempty = run_typed(&json!({
        "operation": "emptiness",
        "automaton": parity_automaton()
    }));
    assert_eq!(nonempty.language_empty, Some(false));
    let witness = run_typed(&json!({
        "operation": "witness",
        "automaton": parity_automaton()
    }));
    let expected: amari_discovery::RewriteTerm = serde_json::from_value(s_z(0)).unwrap();
    assert_eq!(witness.witness, Some(expected));
    // An automaton with no nullary transitions is empty.
    let empty = run_typed(&json!({
        "operation": "emptiness",
        "automaton": {
            "alphabet": [{"name": "s", "arity": 1}],
            "states": ["q0"],
            "transitions": [{"symbol": "s", "children": ["q0"], "parent": "q0"}],
            "finals": ["q0"]
        }
    }));
    assert_eq!(empty.language_empty, Some(true));
    let no_witness = run_typed(&json!({
        "operation": "witness",
        "automaton": {
            "alphabet": [{"name": "s", "arity": 1}],
            "states": ["q0"],
            "transitions": [{"symbol": "s", "children": ["q0"], "parent": "q0"}],
            "finals": ["q0"]
        }
    }));
    assert_eq!(no_witness.witness, None);
}

#[test]
fn union_and_intersection_return_canonical_certificates() {
    // odds = parity with finals [q1]
    let mut odds = parity_automaton();
    odds["finals"] = json!(["q1"]);
    let union = run_typed(&json!({
        "operation": "union",
        "automaton": parity_automaton(),
        "other": odds
    }));
    assert_eq!(union.outcome, "ok");
    let hash = union.result_hash.clone().expect("union hash");
    assert_eq!(hash.len(), 64, "sha256 hex");
    assert!(union.result_states.unwrap() >= 2);
    // Certificates are deterministic: the same request produces the
    // same canonical hash.
    let union_again = run_typed(&json!({
        "operation": "union",
        "automaton": parity_automaton(),
        "other": odds
    }));
    assert_eq!(union.result_hash, union_again.result_hash);
    // A different second operand changes the certificate.
    let union_self = run_typed(&json!({
        "operation": "union",
        "automaton": parity_automaton(),
        "other": parity_automaton()
    }));
    assert_ne!(union.result_hash, union_self.result_hash);
    let intersection = run_typed(&json!({
        "operation": "intersection",
        "automaton": parity_automaton(),
        "other": odds
    }));
    // evens ∩ odds is empty: the certificate is the empty-product
    // automaton (0 states), and its hash differs from self ∩ self.
    let intersection_self = run_typed(&json!({
        "operation": "intersection",
        "automaton": parity_automaton(),
        "other": parity_automaton()
    }));
    assert_ne!(intersection.result_hash, intersection_self.result_hash);
}

#[test]
fn determinize_and_minimize_produce_stable_certificates() {
    let minimized = run_typed(&json!({
        "operation": "minimize",
        "automaton": parity_automaton()
    }));
    let minimized_again = run_typed(&json!({
        "operation": "minimize",
        "automaton": parity_automaton()
    }));
    assert_eq!(minimized.result_hash, minimized_again.result_hash);
    // Parity is already minimal: two states.
    assert_eq!(minimized.result_states, Some(2));
    let determinized = run_typed(&json!({
        "operation": "determinize",
        "automaton": parity_automaton()
    }));
    assert_eq!(determinized.result_states, Some(2));
}

#[test]
fn grammar_text_input_is_parsed_and_converted() {
    let grammar = "amari-tree-grammar/v1\nnonterminals: S\nstart: S\nS -> num\n";
    let output = run_typed(&json!({
        "operation": "membership",
        "grammar": grammar,
        "term": {"kind": "symbol", "name": "num", "arguments": []}
    }));
    assert_eq!(output.accepted, Some(true));
    let rejected = run_typed(&json!({
        "operation": "membership",
        "grammar": grammar,
        "term": {"kind": "symbol", "name": "zero", "arguments": []}
    }));
    assert_eq!(rejected.accepted, Some(false));
}

#[test]
fn malformed_inputs_are_typed_errors() {
    // Unknown operation.
    let error = ProbeEngine::new()
        .unwrap()
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({"operation": "frobnicate", "automaton": parity_automaton()}),
        )
        .unwrap_err();
    assert!(format!("{error}").contains("operation"), "{error}");
    // Transition referencing an undeclared state.
    let error = ProbeEngine::new()
        .unwrap()
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({
                "operation": "emptiness",
                "automaton": {
                    "alphabet": [{"name": "a", "arity": 0}],
                    "states": ["q0"],
                    "transitions": [{"symbol": "a", "children": [], "parent": "q9"}],
                    "finals": []
                }
            }),
        )
        .unwrap_err();
    assert!(format!("{error}").contains("q9"), "{error}");
    // Membership without a term.
    let error = ProbeEngine::new()
        .unwrap()
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({"operation": "membership", "automaton": parity_automaton()}),
        )
        .unwrap_err();
    assert!(format!("{error}").contains("term"), "{error}");
    // Union without a second automaton.
    let error = ProbeEngine::new()
        .unwrap()
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({"operation": "union", "automaton": parity_automaton()}),
        )
        .unwrap_err();
    assert!(format!("{error}").contains("other"), "{error}");
    // Malformed grammar text.
    let error = ProbeEngine::new()
        .unwrap()
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({"operation": "emptiness", "grammar": "not a grammar"}),
        )
        .unwrap_err();
    assert!(format!("{error}").contains("line 1"), "{error}");
}

#[test]
fn oversized_inputs_are_typed_errors() {
    // 65 states exceeds the 64-state probe ceiling.
    let states: Vec<String> = (0..65).map(|i| format!("q{i}")).collect();
    let error = ProbeEngine::new()
        .unwrap()
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({
                "operation": "emptiness",
                "automaton": {
                    "alphabet": [{"name": "a", "arity": 0}],
                    "states": states,
                    "transitions": [],
                    "finals": []
                }
            }),
        )
        .unwrap_err();
    assert!(format!("{error}").contains("state"), "{error}");
}
