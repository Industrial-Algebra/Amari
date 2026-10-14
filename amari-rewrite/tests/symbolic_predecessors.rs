//! Symbolic predecessor contract tests (0.25 Cohort 2, Task 8).
//!
//! Backward application unifies a freshened rule RHS with each target
//! subterm and instantiates the LHS. Results carry existentials (LHS
//! variables absent from the RHS), constraints, substitution,
//! provenance, and resources. Every symbolic predecessor must satisfy
//! the bounded-grounding forward-replay oracle. The legacy
//! `predecessors` iterator behavior is pinned unchanged.

use amari_rewrite::inverse::{predecessors, symbolic_predecessors, SymbolicPredecessor};
use amari_rewrite::relation::{LogicVar, RelationLimits};
use amari_rewrite::trs::{Rule, Substitution, Term, TermSystem};
use amari_rewrite::RewriteError;

fn system(rules: Vec<(&str, &str)>) -> TermSystem {
    let parsed = rules
        .into_iter()
        .map(|(lhs, rhs)| Rule::new(parse(lhs), parse(rhs)).unwrap())
        .collect();
    TermSystem::new(parsed)
}

/// Tiny parser for test terms: `a`, `f(a)`, `f(X, a)`.
fn parse(text: &str) -> Term {
    let text = text.trim();
    if let Some(open) = text.find('(') {
        let head = &text[..open];
        let inner = &text[open + 1..text.len() - 1];
        let mut args = Vec::new();
        let mut depth = 0i32;
        let mut start = 0;
        for (index, ch) in inner.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    args.push(parse(&inner[start..index]));
                    start = index + 1;
                }
                _ => {}
            }
        }
        args.push(parse(&inner[start..]));
        Term::sym(head, args)
    } else if text.chars().next().is_some_and(char::is_uppercase) {
        Term::var(text)
    } else {
        Term::constant(text)
    }
}

fn limits() -> RelationLimits {
    RelationLimits::default()
}

/// The mandatory oracle: ground every free variable with one constant
/// and confirm the predecessor forward-rewrites to the target.
fn assert_forward_replays(sys: &TermSystem, target: &Term, predecessor: &SymbolicPredecessor) {
    let mut grounding = Substitution::new();
    for variable in target.variables() {
        grounding.insert(variable, Term::constant("zz"));
    }
    for variable in predecessor.term.variables() {
        grounding.insert(variable, Term::constant("zz"));
    }
    let concrete_source = grounding.apply(&predecessor.term);
    let concrete_target = grounding.apply(target);
    let successors = sys.successors(&concrete_source).unwrap();
    assert!(
        successors.contains(&concrete_target),
        "forward replay failed: {concrete_source:?} does not reach \
         {concrete_target:?} via {successors:?}"
    );
}

#[test]
fn direct_and_nested_predecessors() {
    let sys = system(vec![("a", "b")]);
    let preds = symbolic_predecessors(&sys, &parse("b"), &limits(), 0).unwrap();
    assert_eq!(preds.len(), 1);
    assert_eq!(preds[0].term, parse("a"));
    assert!(preds[0].provenance.position.is_root());
    assert!(preds[0].existentials.is_empty());

    let sys = system(vec![("f(X)", "g(X)")]);
    let preds = symbolic_predecessors(&sys, &parse("h(g(a))"), &limits(), 0).unwrap();
    assert_eq!(preds.len(), 1);
    assert_eq!(preds[0].term, parse("h(f(a))"));
    assert_eq!(preds[0].provenance.position.as_slice(), &[0]);
}

#[test]
fn freshening_avoids_capture_and_is_deterministic() {
    // Rule variable X collides with the target's free variable Y only
    // in name space; freshening must keep them apart.
    let sys = system(vec![("f(X)", "X")]);
    let target = parse("g(Y)");
    let first = symbolic_predecessors(&sys, &target, &limits(), 0).unwrap();
    let second = symbolic_predecessors(&sys, &target, &limits(), 0).unwrap();
    // Positions: root (f(g(Y))) and the variable leaf (g(f(Y))).
    assert_eq!(first.len(), 2);
    // Same query scope: identical results across runs.
    assert_eq!(first, second);
    for predecessor in &first {
        let names: Vec<String> = predecessor
            .term
            .variables()
            .iter()
            .map(|v| v.to_string())
            .collect();
        // The rule's raw variable name must never leak: freshening
        // prevents capture between rule and target variables.
        assert!(!names.iter().any(|n| n == "X"));
        // Freshened logic variables appear in root-position results.
        assert!(names.iter().all(|n| n == "Y" || n.starts_with('?')));
    }
    // Different scope: disjoint freshened variables.
    let other = symbolic_predecessors(&sys, &target, &limits(), 1).unwrap();
    assert_ne!(first, other);
}

#[test]
fn erased_variables_become_existentials() {
    // Y occurs only in the LHS: backward application introduces it as
    // an existential.
    let sys = system(vec![("f(X, Y)", "g(X)")]);
    let preds = symbolic_predecessors(&sys, &parse("g(a)"), &limits(), 0).unwrap();
    assert_eq!(preds.len(), 1);
    assert_eq!(preds[0].existentials.len(), 1);
    let existential = preds[0].existentials[0];
    let rendered = format!("{:?}", preds[0].term);
    assert!(rendered.contains(&existential.to_string()));
    // The oracle must hold despite the existential.
    assert_forward_replays(&sys, &parse("g(a)"), &preds[0]);
}

#[test]
fn repeated_rhs_variables_and_impossible_matches() {
    let sys = system(vec![("h(X)", "k(X, X)")]);
    // k(a, b) cannot unify with k(X, X).
    assert!(symbolic_predecessors(&sys, &parse("k(a, b)"), &limits(), 0)
        .unwrap()
        .is_empty());
    // k(a, a) yields h(a).
    let preds = symbolic_predecessors(&sys, &parse("k(a, a)"), &limits(), 0).unwrap();
    assert_eq!(preds.len(), 1);
    assert_eq!(preds[0].term, parse("h(a)"));

    // No rule RHS matches any subterm.
    let sys = system(vec![("f(X)", "g(X)")]);
    assert!(symbolic_predecessors(&sys, &parse("h(a)"), &limits(), 0)
        .unwrap()
        .is_empty());
}

#[test]
fn cyclic_self_loop_predecessors_are_excluded() {
    let sys = system(vec![("X", "X")]);
    assert!(symbolic_predecessors(&sys, &parse("a"), &limits(), 0)
        .unwrap()
        .is_empty());
}

#[test]
fn provenance_records_identity_position_and_order() {
    let sys = system(vec![("a", "b"), ("c", "b")]);
    let preds = symbolic_predecessors(&sys, &parse("b"), &limits(), 0).unwrap();
    assert_eq!(preds.len(), 2);
    // Enumeration order is position-major, rule order within.
    assert_eq!(preds[0].term, parse("a"));
    assert_eq!(preds[1].term, parse("c"));
    // Rule identities are stable digests and distinct per rule.
    assert_ne!(preds[0].provenance.rule_id, preds[1].provenance.rule_id);
    // Authority hashes match the actual terms.
    assert_eq!(
        preds[0].provenance.predecessor_hash,
        amari_rewrite::relation::Sha256Digest::canonical_term(
            "amari.relation.term/v1",
            &preds[0].term
        )
    );
    assert_eq!(preds[0].provenance.scope, 0);

    // Nested positions enumerate in preorder before deeper ones.
    let sys = system(vec![("a", "b")]);
    let preds = symbolic_predecessors(&sys, &parse("f(b, b)"), &limits(), 0).unwrap();
    let positions: Vec<_> = preds
        .iter()
        .map(|p| p.provenance.position.as_slice().to_vec())
        .collect();
    assert_eq!(positions, vec![vec![0], vec![1]]);
}

#[test]
fn limits_are_enforced_during_backward_application() {
    let sys = system(vec![("f(X)", "g(X)")]);
    // Operation budget too small to process the unification pairs.
    let tight = RelationLimits::new(4_096, 64, 4_096, 1).unwrap();
    assert!(matches!(
        symbolic_predecessors(&sys, &parse("g(g(a))"), &tight, 0),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
    // Term depth budget rejects the target itself.
    let shallow = RelationLimits::new(4_096, 1, 4_096, 1_000_000).unwrap();
    assert!(matches!(
        symbolic_predecessors(&sys, &parse("g(g(a))"), &shallow, 0),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
}

#[test]
fn legacy_backward_search_behavior_is_unchanged() {
    // Regression pin: legacy iterator results for a known system.
    let sys = system(vec![("f(X)", "g(X)"), ("a", "b")]);
    let target = parse("g(a)");
    let legacy = predecessors(&sys, target.clone(), 1);
    assert_eq!(legacy, vec![parse("f(a)")]);

    let deep = predecessors(&sys, parse("g(b)"), 2);
    assert!(deep.contains(&parse("f(b)")));
    assert!(deep.contains(&parse("f(a)")));
}

#[test]
fn logic_var_display_is_stable() {
    let var = LogicVar::new(3, 7);
    assert_eq!(var.to_string(), "?3.7");
}

/// PR #302 round 3 P1 (reviewer repro): backward matching used deep
/// application on matcher bindings. Matching `g(X, Y)` (the rhs of
/// `f(X, Y) -> g(X, Y)`) against `g(Y, a)` bound `X := Y` and
/// `Y := a`, and deep resolution collapsed the predecessor to
/// `f(a, a)`. One-level matcher application yields `f(Y, a)`.
#[test]
fn backward_match_does_not_capture_same_named_target_variables() {
    let sys = TermSystem::new(vec![Rule::new(parse("f(X, Y)"), parse("g(X, Y)")).unwrap()]);
    let preds = predecessors(&sys, parse("g(Y, a)"), 1);
    assert_eq!(preds, vec![parse("f(Y, a)")], "{preds:?}");
}

/// PR #302 round 3 P2 (reviewer repro): clause freshening applied
/// the renaming DEEPLY, so a rule variable named like a fresh name
/// in the same scope (`?0.1` under scope 0) chained
/// `x -> ?0.1 -> ?0.0` and collapsed distinct rule variables —
/// `symbolic_predecessors` wrongly returned []. Simultaneous
/// (one-level) renaming keeps them distinct and finds `f(a, b)`.
#[test]
fn freshening_does_not_collapse_variables_named_like_fresh_names() {
    let collision = Term::var("?0.1");
    let plain = Term::var("x");
    let rule = Rule::new(
        Term::sym("f", [collision.clone(), plain.clone()]),
        Term::sym("g", [collision, plain]),
    )
    .unwrap();
    let sys = TermSystem::new(vec![rule]);
    let target = Term::sym("g", [Term::constant("a"), Term::constant("b")]);
    let preds = symbolic_predecessors(&sys, &target, &limits(), 0).unwrap();
    assert_eq!(preds.len(), 1, "{preds:?}");
    assert_eq!(
        preds[0].term,
        Term::sym("f", [Term::constant("a"), Term::constant("b")])
    );
}

/// PR #302 round 4 P2 (reviewer repro): fresh clause names were not
/// reserved against TARGET variables — a target variable literally
/// named `?0.0` collided with the freshened rule variable, producing
/// a spurious occurs-check failure and silently omitting the root
/// predecessor. Reserved namespaces keep them distinct.
#[test]
fn freshening_reserves_target_variable_names() {
    let sys = TermSystem::new(vec![Rule::new(parse("f(X)"), parse("g(X)")).unwrap()]);
    // The target's variable is named exactly like scope-0's first
    // fresh variable.
    let target = Term::sym("g", [Term::sym("h", [Term::var("?0.0")])]);
    let preds = symbolic_predecessors(&sys, &target, &limits(), 0).unwrap();
    let expected = Term::sym("f", [Term::sym("h", [Term::var("?0.0")])]);
    assert!(
        preds.iter().any(|p| p.term == expected),
        "missing root predecessor {expected:?}: {preds:?}"
    );
}
