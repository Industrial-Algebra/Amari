// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded concrete oracles for the regular-preimage research gate
//! (0.25 Cohort 5, Task 23).
//!
//! These tests ground the closure matrix
//! (`docs/research/rewrite-preimage-closure-matrix.md`) BEFORE any
//! preimage class is selected or implemented. Every fact here is
//! computed by exhaustive bounded enumeration over a small ranked
//! alphabet, independent of the preimage constructions that Tasks
//! 24–26 may add — those tasks must differential-test against these
//! oracles.
//!
//! Two rows are RED counterexamples (unrestricted exact-regular
//! claims fail), two are positive sanity rows (the approved-class
//! predictions the construction must reproduce), and one pins the
//! rule variable contract the gate relies on.

use amari_rewrite::trs::{match_pattern, Rule, Term};

/// Ranked alphabet for the oracle: a/0, g/1, f/2.
fn node_count(term: &Term) -> usize {
    match term {
        Term::Var(_) => 1,
        Term::Sym(_, arguments) => 1 + arguments.iter().map(node_count).sum::<usize>(),
    }
}

/// All ground terms over {a/0, g/1, f/2} with at most `max_nodes`
/// nodes, sorted and deduplicated.
fn ground_terms(max_nodes: usize) -> Vec<Term> {
    let mut all: Vec<Term> = vec![Term::constant("a")];
    loop {
        let mut next: Vec<Term> = Vec::new();
        for term in &all {
            let grown = Term::sym("g", [term.clone()]);
            if node_count(&grown) <= max_nodes && !all.contains(&grown) {
                next.push(grown);
            }
        }
        for (index, left) in all.iter().enumerate() {
            for right in all.iter().take(index + 1) {
                let pair = Term::sym("f", [left.clone(), right.clone()]);
                if node_count(&pair) <= max_nodes && !all.contains(&pair) {
                    next.push(pair);
                }
            }
        }
        if next.is_empty() {
            break;
        }
        all.extend(next);
    }
    all.sort();
    all
}

/// All one-step successors of `term` under `rules`, at every
/// position, computed concretely via pattern matching and
/// substitution — no automata involved.
fn successors(rules: &[Rule], term: &Term) -> Vec<Term> {
    let mut out = Vec::new();
    for path in term.positions() {
        let subterm = term.subterm(&path).expect("position is valid");
        for rule in rules {
            if let Some(substitution) = match_pattern(rule.lhs(), subterm) {
                out.push(
                    term.replace_at(&path, substitution.apply(rule.rhs()))
                        .expect("oracle terms stay within limits"),
                );
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Concrete bounded one-step preimage: the universe terms that have
/// at least one one-step successor in `target`.
fn bounded_preimage(rules: &[Rule], target: &[Term], universe: &[Term]) -> Vec<Term> {
    universe
        .iter()
        .filter(|term| {
            successors(rules, term)
                .iter()
                .any(|successor| target.contains(successor))
        })
        .cloned()
        .collect()
}

fn f(left: Term, right: Term) -> Term {
    Term::sym("f", [left, right])
}

fn g(inner: Term) -> Term {
    Term::sym("g", [inner])
}

fn a() -> Term {
    Term::constant("a")
}

/// RED counterexample N1 (one-step, non-left-linear): for
/// R = {f(x,x) -> a} and L = {a}, the one-step preimage is exactly
/// {f(t,t) : t ground} — the equality-correlated language that is
/// NOT recognizable (TATA Example 1.2.1 pumping argument). An exact
/// preimage construction must therefore REJECT non-left-linear
/// systems; the bounded oracle pins the correlation concretely.
#[test]
fn non_left_linear_one_step_preimage_is_equality_correlated() {
    let rules = [Rule::new(f(Term::var("x"), Term::var("x")), a()).unwrap()];
    let universe = ground_terms(7);
    let preimage = bounded_preimage(&rules, &[a()], &universe);

    // f(t,t) has at most 7 nodes exactly when t has at most 3:
    // t in {a, g(a), g(g(a)), f(a,a)}.
    let expected: Vec<Term> = {
        let mut terms = vec![
            f(a(), a()),
            f(g(a()), g(a())),
            f(g(g(a())), g(g(a()))),
            f(f(a(), a()), f(a(), a())),
        ];
        terms.sort();
        terms
    };
    assert_eq!(
        preimage, expected,
        "the bounded one-step preimage of {{a}} under f(x,x)->a is \
         exactly the equality-correlated set"
    );
    // Explicit discriminators: equality is required, similarity is
    // not enough.
    assert!(!preimage.contains(&f(a(), g(a()))));
    assert!(!preimage.contains(&f(a(), f(a(), a()))));
    assert!(preimage.contains(&f(g(a()), g(a()))));
}

/// RED witness N2 (contrast, forward direction): the LINEAR rule
/// g(x) -> f(x,x) duplicates its argument, so with a redex-renewing
/// companion rule g(x) -> g(g(x)) the descendants of g(a) grow
/// balanced, leaf-correlated trees: every f-rooted descendant has
/// EQUAL children (TATA Exercise 1.17.3: forward closure of a
/// regular language need not be recognizable). The gate's exact
/// classes are BACKWARD (preimage) classes; this witness documents
/// why forward results do not transfer.
#[test]
fn duplicating_rule_grows_leaf_correlated_descendants() {
    let rules = [
        Rule::new(g(Term::var("x")), f(Term::var("x"), Term::var("x"))).unwrap(),
        Rule::new(g(Term::var("x")), g(g(Term::var("x")))).unwrap(),
    ];
    let universe = ground_terms(7);
    // Bounded descendant closure of {g(a)} within the universe.
    let mut descendants = vec![g(a())];
    let mut frontier = vec![g(a())];
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for term in &frontier {
            for successor in successors(&rules, term) {
                if universe.contains(&successor) && !descendants.contains(&successor) {
                    next.push(successor);
                }
            }
        }
        next.sort();
        next.dedup();
        descendants.extend(next.iter().cloned());
        frontier = next;
    }
    descendants.sort();
    // Correlation pin: every IRREDUCIBLE (g-free) f-rooted
    // descendant has equal children — the balanced trees that
    // truncations of the non-recognizable closure are made of.
    // (Intermediate descendants rewrite one child at a time, so the
    // correlation only stabilizes at normal forms.)
    fn contains_g(term: &Term) -> bool {
        match term {
            Term::Var(_) => false,
            Term::Sym(symbol, arguments) => {
                symbol.as_str() == "g" || arguments.iter().any(contains_g)
            }
        }
    }
    assert!(descendants.contains(&f(a(), a())));
    assert!(descendants.contains(&f(f(a(), a()), f(a(), a()))));
    assert!(!descendants.contains(&f(g(a()), a())));
    for descendant in &descendants {
        if let Term::Sym(symbol, arguments) = descendant {
            if symbol.as_str() == "f" && !contains_g(descendant) {
                assert_eq!(
                    arguments[0], arguments[1],
                    "irreducible f-rooted descendant has unequal children: {descendant:?}"
                );
            }
        }
    }
}

/// Positive row P1 (one-step, left-linear with shared variable):
/// for R = {g(x) -> f(x,x)}, the one-step preimage of {f(a,a)} is
/// exactly {g(a)}, and of {f(a,a), f(g(a),g(a))} exactly
/// {g(a), g(g(a))}. This is the concrete prediction that the ADR's
/// determinized-evaluation construction must reproduce; Task 25
/// differential-tests against it.
#[test]
fn left_linear_one_step_preimage_matches_oracle() {
    let rules = [Rule::new(g(Term::var("x")), f(Term::var("x"), Term::var("x"))).unwrap()];
    let universe = ground_terms(7);

    let preimage = bounded_preimage(&rules, &[f(a(), a())], &universe);
    assert_eq!(preimage, vec![g(a())]);

    let preimage = bounded_preimage(&rules, &[f(a(), a()), f(g(a()), g(a()))], &universe);
    assert_eq!(preimage, vec![g(a()), g(g(a()))]);
}

/// Positive row P2 (unbounded, ground ⊂ linear variable-disjoint):
/// for R = {a -> g(a)} and L = {g(g(a))}, iterated bounded preimage
/// reaches a fixpoint after two iterations, yielding the saturation
/// {a, g(a), g(g(a))}. Pins the idempotent-fixpoint behavior that
/// Task 26 must exhibit on the ground row, including the
/// non-membership of g(g(g(a))).
#[test]
fn ground_saturation_reaches_fixpoint() {
    let rules = [Rule::new(a(), g(a())).unwrap()];
    let universe = ground_terms(7);

    let layer0 = vec![g(g(a()))];
    let layer1 = bounded_preimage(&rules, &layer0, &universe);
    assert_eq!(layer1, vec![g(a())]);
    let layer2 = bounded_preimage(&rules, &layer1, &universe);
    assert_eq!(layer2, vec![a()]);
    let layer3 = bounded_preimage(&rules, &layer2, &universe);
    assert!(layer3.is_empty(), "no term in the universe rewrites to a");

    // Saturation: L union the nonempty preimage layers.
    let mut saturation = layer0.clone();
    saturation.extend(layer1.iter().cloned());
    saturation.extend(layer2.iter().cloned());
    saturation.sort();
    saturation.dedup();
    assert_eq!(saturation, vec![a(), g(a()), g(g(a()))]);
    assert!(!saturation.contains(&g(g(g(a())))));
}

/// The gate relies on the TRS contract Var(rhs) ⊆ Var(lhs): every
/// right-hand variable is pinned by the match, so one-step preimage
/// evaluation never faces unconstrained variables.
#[test]
fn rule_rhs_variable_contract_holds() {
    let rejected = Rule::new(f(Term::var("x"), Term::var("x")), g(Term::var("y")));
    assert!(rejected.is_err(), "rhs variables must occur in the lhs");
}
