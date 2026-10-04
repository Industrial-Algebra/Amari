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

// ---- Review round 1 regressions (PR #270)

/// Review P1: the probe validates membership terms against the
/// caller-tightened node budget before running the operation.
#[test]
fn membership_term_respects_caller_node_budget() {
    let engine = ProbeEngine::with_limits(amari_discovery::ProbeEngineLimits {
        max_nodes: 1,
        ..Default::default()
    })
    .unwrap();
    let automaton = json!({
        "alphabet": [{"name": "a", "arity": 0}, {"name": "f", "arity": 1}],
        "states": ["q0", "q1"],
        "transitions": [
            {"symbol": "a", "children": [], "parent": "q0"},
            {"symbol": "f", "children": ["q0"], "parent": "q1"}
        ],
        "finals": ["q1"]
    });
    let error = engine
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({
                "operation": "membership",
                "automaton": automaton,
                "term": {"kind": "symbol", "name": "f", "arguments": [
                    {"kind": "symbol", "name": "a", "arguments": []}
                ]}
            }),
        )
        .expect_err("two-node term exceeds the one-node budget");
    assert!(
        error.to_string().contains("node"),
        "unexpected error: {error}"
    );
}

/// Review P1: determinization reports domain work (subset-construction
/// tuple evaluations), so the engine's post-execution check enforces
/// the caller's operation budget.
#[test]
fn determinize_reports_domain_work_against_caller_budget() {
    let engine = ProbeEngine::with_limits(amari_discovery::ProbeEngineLimits {
        max_operations: 500,
        ..Default::default()
    })
    .unwrap();
    let states: Vec<String> = (0..64).map(|index| format!("q{index}")).collect();
    let mut transitions = vec![json!({"symbol": "a", "children": [], "parent": "q0"})];
    for index in 1..64 {
        transitions.push(json!({
            "symbol": "f",
            "children": [format!("q{}", index - 1)],
            "parent": format!("q{index}")
        }));
    }
    let automaton = json!({
        "alphabet": [{"name": "a", "arity": 0}, {"name": "f", "arity": 1}],
        "states": states,
        "transitions": transitions,
        "finals": ["q63"]
    });
    let error = engine
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({"operation": "determinize", "automaton": automaton}),
        )
        .expect_err("2080 tuple evaluations exceed the 500-operation budget");
    assert!(
        error.to_string().contains("operation"),
        "unexpected error: {error}"
    );
}

/// Review P2: grammar-derived names face the same 256-byte ceiling as
/// automaton DTO names, and witnesses replay through membership under
/// both source representations.
#[test]
fn grammar_names_respect_probe_ceiling_and_witnesses_replay() {
    let long_name = |bytes: usize| "x".repeat(bytes);
    let grammar_with = |bytes: usize| {
        format!(
            "amari-tree-grammar/v1\nnonterminals: S\nstart: S\nS -> {}\n",
            long_name(bytes)
        )
    };
    // 256-byte terminal names are accepted end to end.
    let witness = run_typed(&json!({
        "operation": "witness",
        "grammar": grammar_with(256)
    }));
    let witness_term = witness.witness.expect("256-byte witness");
    let accepted = run_typed(&json!({
        "operation": "membership",
        "grammar": grammar_with(256),
        "term": serde_json::to_value(&witness_term).unwrap()
    }));
    assert_eq!(accepted.accepted, Some(true), "grammar replay");
    // The same witness replays against the equivalent automaton DTO.
    let automaton = json!({
        "alphabet": [{"name": long_name(256), "arity": 0}],
        "states": ["S"],
        "transitions": [{"symbol": long_name(256), "children": [], "parent": "S"}],
        "finals": ["S"]
    });
    let accepted = run_typed(&json!({
        "operation": "membership",
        "automaton": automaton,
        "term": serde_json::to_value(&witness_term).unwrap()
    }));
    assert_eq!(accepted.accepted, Some(true), "automaton replay");
    // 257-byte terminal names are typed errors.
    let error = ProbeEngine::new()
        .unwrap()
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({"operation": "witness", "grammar": grammar_with(257)}),
        )
        .expect_err("257-byte grammar terminal exceeds the name ceiling");
    assert!(
        error.to_string().contains("257"),
        "unexpected error: {error}"
    );
}

// ---- Review round 2 regressions (PR #270): accounting must cover
// the library's actual charge model, not optimistic estimates.

/// Review P1 (round 2): membership charges one operation per
/// transition at EVERY term node (2 nodes x 2 transitions = 4).
#[test]
fn membership_charges_transitions_per_node() {
    let engine = ProbeEngine::with_limits(amari_discovery::ProbeEngineLimits {
        max_operations: 2,
        ..Default::default()
    })
    .unwrap();
    let automaton = json!({
        "alphabet": [{"name": "a", "arity": 0}, {"name": "f", "arity": 1}],
        "states": ["q0", "q1"],
        "transitions": [
            {"symbol": "a", "children": [], "parent": "q0"},
            {"symbol": "f", "children": ["q0"], "parent": "q1"}
        ],
        "finals": ["q1"]
    });
    let error = engine
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({
                "operation": "membership",
                "automaton": automaton,
                "term": {"kind": "symbol", "name": "f", "arguments": [
                    {"kind": "symbol", "name": "a", "arguments": []}
                ]}
            }),
        )
        .expect_err("4 transition checks exceed the 2-operation budget");
    assert!(
        error.to_string().contains("operation"),
        "unexpected error: {error}"
    );
}

/// Review P1 (round 2): determinization charges (m+1)^arity odometer
/// steps per non-nullary symbol at every registered macro-state m,
/// and discovered subsets can outnumber input states (7 subsets from
/// 3 states charges 2 x (1+...+7) = 56).
#[test]
fn determinize_charges_macro_state_odometer_steps() {
    let engine = ProbeEngine::with_limits(amari_discovery::ProbeEngineLimits {
        max_operations: 52,
        ..Default::default()
    })
    .unwrap();
    let automaton = json!({
        "alphabet": [
            {"name": "a", "arity": 0},
            {"name": "r", "arity": 1},
            {"name": "u", "arity": 1}
        ],
        "states": ["q0", "q1", "q2"],
        "transitions": [
            {"symbol": "a", "children": [], "parent": "q0"},
            {"symbol": "r", "children": ["q0"], "parent": "q1"},
            {"symbol": "r", "children": ["q1"], "parent": "q2"},
            {"symbol": "r", "children": ["q2"], "parent": "q0"},
            {"symbol": "u", "children": ["q0"], "parent": "q0"},
            {"symbol": "u", "children": ["q0"], "parent": "q1"},
            {"symbol": "u", "children": ["q1"], "parent": "q1"},
            {"symbol": "u", "children": ["q2"], "parent": "q2"}
        ],
        "finals": ["q0"]
    });
    let error = engine
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({"operation": "determinize", "automaton": automaton}),
        )
        .expect_err("56 odometer steps exceed the 52-operation budget");
    assert!(
        error.to_string().contains("operation"),
        "unexpected error: {error}"
    );
}

/// Review P1 (round 2): minimization completion adds one transition
/// per state TUPLE per symbol (2^4 = 16 f-transitions here), and
/// refinement charges once per child position per round.
#[test]
fn minimize_charges_completion_tuples_and_child_positions() {
    let engine = ProbeEngine::with_limits(amari_discovery::ProbeEngineLimits {
        max_operations: 20,
        ..Default::default()
    })
    .unwrap();
    let automaton = json!({
        "alphabet": [{"name": "a", "arity": 0}, {"name": "f", "arity": 4}],
        "states": ["q"],
        "transitions": [{"symbol": "a", "children": [], "parent": "q"}],
        "finals": ["q"]
    });
    let error = engine
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({"operation": "minimize", "automaton": automaton}),
        )
        .expect_err("completion + refinement charges exceed the 20-operation budget");
    assert!(
        error.to_string().contains("operation"),
        "unexpected error: {error}"
    );
}

// ---- Cohort 4 closeout review regressions (remediation branch)

/// Finding 2: the grammar source path enforces the same 64-state /
/// 256-transition / 64-symbol ceilings as the automaton DTO path.
#[test]
fn grammar_source_respects_automaton_ceilings() {
    let nonterminals: Vec<String> = (0..70).map(|index| format!("N{index}")).collect();
    let mut grammar = format!(
        "amari-tree-grammar/v1\nnonterminals: {}\nstart: N0\n",
        nonterminals.join(" ")
    );
    for index in 0..70 {
        grammar.push_str(&format!("N{index} -> c{index}\n"));
    }
    let error = ProbeEngine::new()
        .unwrap()
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({"operation": "witness", "grammar": grammar}),
        )
        .expect_err("70 grammar states exceed the 64-state probe ceiling");
    assert!(
        error.to_string().contains("64"),
        "unexpected error: {error}"
    );
}

/// Finding 3: emptiness work is metered by the library and enforced
/// during execution — the reverse-ordered 2-state chain costs 6
/// fixpoint transition evaluations, so a budget of 5 fails.
#[test]
fn emptiness_enforces_budget_during_execution() {
    let chain = json!({
        "alphabet": [{"name": "z", "arity": 0}, {"name": "a", "arity": 1}],
        "states": ["s0", "s1"],
        "transitions": [
            {"symbol": "z", "children": [], "parent": "s0"},
            {"symbol": "a", "children": ["s0"], "parent": "s1"}
        ],
        "finals": ["s1"]
    });
    // Default budget: the exact library count is reported.
    let engine = ProbeEngine::new().unwrap();
    let execution = engine
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({"operation": "emptiness", "automaton": chain}),
        )
        .expect("emptiness under default budget");
    assert_eq!(
        execution.resources.operations, 6,
        "3 rounds x 2 transitions"
    );
    // Caller-tightened budget below the actual work: typed limit
    // error, enforced during execution.
    let tight = ProbeEngine::with_limits(amari_discovery::ProbeEngineLimits {
        max_operations: 5,
        ..Default::default()
    })
    .unwrap();
    let error = tight
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({"operation": "emptiness", "automaton": chain}),
        )
        .expect_err("6 fixpoint evaluations exceed the 5-operation budget");
    assert!(
        error.to_string().contains("operation"),
        "unexpected error: {error}"
    );
}

/// Finding 4: library limit errors surface as limit_exceeded
/// (exit 7), not invalid_input (exit 2).
#[test]
fn library_limit_errors_keep_their_kind() {
    let grammar = "amari-tree-grammar/v1\nnonterminals: S\nstart: S\n\
                   S -> f a a a a a a a a a a a a a a a a a\n";
    let error = ProbeEngine::new()
        .unwrap()
        .execute(
            &LANGUAGES.parse().unwrap(),
            &json!({"operation": "witness", "grammar": grammar}),
        )
        .expect_err("rank-17 production exceeds the library rank ceiling");
    assert!(
        matches!(error, amari_discovery::DiscoveryError::LimitExceeded(_)),
        "rank-17 grammar must be limit_exceeded, got: {error}"
    );
}
