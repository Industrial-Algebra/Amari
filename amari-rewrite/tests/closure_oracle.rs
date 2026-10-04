// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded concrete oracles for the regular-preimage research gate
//! (0.25 Cohort 5, Task 23; remediated per the independent
//! mathematical review of PR #277).
//!
//! These tests ground the closure matrix
//! (`docs/research/rewrite-preimage-closure-matrix.md`) BEFORE any
//! preimage class is selected or implemented. Every fact here is
//! computed by exhaustive bounded enumeration over small ranked
//! alphabets, independent of the preimage constructions that Tasks
//! 24–26 may add — those tasks must differential-test against these
//! oracles.
//!
//! SEMANTICS (review finding R1): the oracle models the standard
//! one-step APPLICATION relation — a rule applied at a position
//! counts as a step even when its result equals the source term.
//! `TermSystem::successors` deliberately filters such identity
//! results (a search optimization), so it is NOT the relation this
//! gate certifies; `swap_rule_pins_application_semantics` pins the
//! divergence. The exact one-step preimage API and its forward
//! replay contract must use an application-level successor relation.
//! Reflexive `≤n`-horizon unions and unbounded saturation coincide
//! under the two semantics (self-steps can be deleted from paths),
//! so the horizon/saturation cells are unaffected by the choice.

use amari_rewrite::trs::{match_pattern, Rule, Term, TermSystem};

fn node_count(term: &Term) -> usize {
    match term {
        Term::Var(_) => 1,
        Term::Sym(_, arguments) => 1 + arguments.iter().map(node_count).sum::<usize>(),
    }
}

/// A tiny ranked alphabet: named constants, unary, and binary
/// symbols.
struct Ranked {
    constants: &'static [&'static str],
    unaries: &'static [&'static str],
    binaries: &'static [&'static str],
}

/// Three-symbol alphabet {a/0, g/1, f/2} used by most rows.
const AGF: Ranked = Ranked {
    constants: &["a"],
    unaries: &["g"],
    binaries: &["f"],
};

/// All ground terms over `ranked` with at most `max_nodes` nodes,
/// sorted and deduplicated. Enumeration is EXHAUSTIVE: every ordered
/// child combination appears (review finding R2 — an earlier version
/// enumerated only one orientation of each pair).
fn ground_terms(ranked: &Ranked, max_nodes: usize) -> Vec<Term> {
    let mut all: Vec<Term> = ranked
        .constants
        .iter()
        .map(|c| Term::constant(*c))
        .collect();
    loop {
        let mut next: Vec<Term> = Vec::new();
        for term in &all {
            for unary in ranked.unaries {
                let grown = Term::sym(*unary, [term.clone()]);
                if node_count(&grown) <= max_nodes && !all.contains(&grown) {
                    next.push(grown);
                }
            }
        }
        for left in &all {
            for right in &all {
                for binary in ranked.binaries {
                    let pair = Term::sym(*binary, [left.clone(), right.clone()]);
                    if node_count(&pair) <= max_nodes && !all.contains(&pair) {
                        next.push(pair);
                    }
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
/// position, under the APPLICATION relation: every position where a
/// left side matches contributes a successor, even when the result
/// equals the source (see the module's semantics note).
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

/// Concrete bounded saturation: `target` plus iterated bounded
/// preimages until no new universe term appears.
fn bounded_saturation(rules: &[Rule], target: &[Term], universe: &[Term]) -> Vec<Term> {
    let mut known: Vec<Term> = target.to_vec();
    loop {
        let layer = bounded_preimage(rules, &known, universe);
        let grown: Vec<Term> = layer
            .into_iter()
            .filter(|term| !known.contains(term))
            .collect();
        if grown.is_empty() {
            break;
        }
        known.extend(grown);
    }
    known.sort();
    known.dedup();
    known
}

fn f(left: Term, right: Term) -> Term {
    Term::sym("f", [left, right])
}

fn g(inner: Term) -> Term {
    Term::sym("g", [inner])
}

fn h(inner: Term) -> Term {
    Term::sym("h", [inner])
}

fn a() -> Term {
    Term::constant("a")
}

/// The 3-symbol universe through 7 nodes is COMPLETE: node-exact
/// counts 1,1,2,4,9,21,51 sum to 89 (independently recomputed during
/// review). Pins both orientations of ordered pairs.
#[test]
fn universe_is_exhaustive() {
    let universe = ground_terms(&AGF, 7);
    assert_eq!(universe.len(), 89, "ordered terms over a/0,g/1,f/2");
    assert!(universe.contains(&f(a(), g(a()))));
    assert!(universe.contains(&f(g(a()), a())));
    assert!(universe.contains(&f(a(), f(a(), a()))));
    assert!(universe.contains(&f(f(a(), a()), a())));
}

/// RED counterexample C1 (one-step, non-left-linear): for
/// R = {f(x,x) -> a} and L = {a}, the one-step preimage is exactly
/// {f(t,t) : t ground} — the only contraction REACHING `a` is at the
/// root (nested redexes rewrite elsewhere). This is the
/// equality-correlated diagonal language, not recognizable by the
/// TATA Example 1.2.1 pumping argument.
#[test]
fn non_left_linear_one_step_preimage_is_equality_correlated() {
    let rules = [Rule::new(f(Term::var("x"), Term::var("x")), a()).unwrap()];
    let universe = ground_terms(&AGF, 7);
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
    // Explicit discriminators, asserted IN the universe first so the
    // exclusions are non-vacuous (review finding R2).
    let mismatch1 = f(a(), g(a()));
    let mismatch2 = f(a(), f(a(), a()));
    assert!(universe.contains(&mismatch1));
    assert!(universe.contains(&mismatch2));
    assert!(!preimage.contains(&mismatch1));
    assert!(!preimage.contains(&mismatch2));
    assert!(preimage.contains(&f(g(a()), g(a()))));
}

/// R1 regression: the gate's one-step relation is APPLICATION
/// rewriting. Under R = {f(x,y) -> f(y,x)} the term f(a,a) has the
/// identity application (x,y both bound to a), so the application
/// preimage of {f(a,a)} contains f(a,a) — and exactly the diagonal
/// pairs, since a swap reaching f(a,a) requires equal children.
/// `TermSystem::successors` instead filters identity results and
/// returns NO successor for f(a,a); the strict one-step preimage of
/// a universal target on the slice K = {f(g^m(a), g^n(a))} would be
/// {m != n}, whose complement in K is the non-recognizable diagonal
/// (review finding R1). The exact one-step API therefore uses an
/// application-level successor relation, NOT the strict search
/// filter; this test pins the divergence concretely.
#[test]
fn swap_rule_pins_application_semantics() {
    let rules = [Rule::new(
        f(Term::var("x"), Term::var("y")),
        f(Term::var("y"), Term::var("x")),
    )
    .unwrap()];
    let universe = ground_terms(&AGF, 7);

    // Application semantics: f(a,a) is its own predecessor via the
    // identity application (x,y both bound to a). Nothing else maps
    // to f(a,a): a swap reaching it requires equal children equal
    // to a.
    let preimage = bounded_preimage(&rules, &[f(a(), a())], &universe);
    assert_eq!(preimage, vec![f(a(), a())]);

    // The strict TermSystem filter disagrees on the identity
    // instance: documented divergence, not a gate relation.
    let system = TermSystem::new(rules.to_vec());
    assert!(
        system
            .successors(&f(a(), a()))
            .expect("system call succeeds")
            .is_empty(),
        "TermSystem::successors filters identity applications; the \
         replay contract for certified preimages must use an \
         application-level relation instead (Task 25)"
    );
}

/// Forward contrast witness (TATA Exercise 1.17.3, stated
/// correctly): over {a/0, h/1, g/1, f/2} with the LEFT-LINEAR (not
/// linear — x is repeated on the right) rule g(x) -> f(x,x) and the
/// regular seed language L = {g(h^n(a))}, one-step descendants at
/// the root are exactly the correlated pairs {f(h^n(a), h^n(a))}.
/// Over all n the descendant set intersected with f-rooted terms is
/// the Example 1.2.1 diagonal — forward closure need not be
/// recognizable. The gate's exact classes are BACKWARD (preimage)
/// classes; this row documents why forward results do not transfer.
/// (Replaces an earlier false witness whose closure was in fact
/// recognizable — review finding R3.)
#[test]
fn forward_duplication_witness_is_the_real_exercise_1_17_3() {
    const AHF: Ranked = Ranked {
        constants: &["a"],
        unaries: &["h", "g"],
        binaries: &["f"],
    };
    let universe = ground_terms(&AHF, 5);
    assert_eq!(universe.len(), 64, "ordered terms over a/0,h/1,g/1,f/2");

    let rules = [Rule::new(g(Term::var("x")), f(Term::var("x"), Term::var("x"))).unwrap()];
    // Seeds g(a), g(h(a)), g(h(h(a))) — the bounded slice of
    // {g(h^n(a))}. Seeds contain g only at the root.
    let seeds = vec![g(a()), g(h(a())), g(h(h(a())))];
    let descendants: Vec<Term> = seeds
        .iter()
        .flat_map(|seed| successors(&rules, seed))
        .collect();

    // Every root descendant is a perfectly correlated pair of
    // h-chains.
    for descendant in &descendants {
        if let Term::Sym(symbol, arguments) = descendant {
            if symbol.as_str() == "f" {
                assert_eq!(
                    arguments[0], arguments[1],
                    "root descendant of a chain seed is correlated"
                );
            }
        }
    }
    assert!(descendants.contains(&f(h(a()), h(a()))));
    // The mismatched pair is in the universe but NOT a descendant.
    let mismatch = f(a(), h(a()));
    assert!(universe.contains(&mismatch));
    assert!(!descendants.contains(&mismatch));
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
    let universe = ground_terms(&AGF, 7);

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
/// non-membership of g(g(g(a))). NOTE (review): this row's finite
/// fixpoint does NOT justify naive layer-union saturation in
/// general — {g(a)->a} ancestors of {a} grow forever while g*(a) is
/// recognizable; Task 26 must implement representation-level
/// saturation (e.g., epsilon-edge closure) or return partial
/// frontiers.
#[test]
fn ground_saturation_reaches_fixpoint() {
    let rules = [Rule::new(a(), g(a())).unwrap()];
    let universe = ground_terms(&AGF, 7);

    let layer0 = vec![g(g(a()))];
    let layer1 = bounded_preimage(&rules, &layer0, &universe);
    assert_eq!(layer1, vec![g(a())]);
    let layer2 = bounded_preimage(&rules, &layer1, &universe);
    assert_eq!(layer2, vec![a()]);
    let layer3 = bounded_preimage(&rules, &layer2, &universe);
    assert!(layer3.is_empty(), "no term in the universe rewrites to a");

    let saturation = bounded_saturation(&rules, &layer0, &universe);
    assert_eq!(saturation, vec![a(), g(a()), g(g(a()))]);
    assert!(!saturation.contains(&g(g(g(a())))));
}

/// Direct C2 witness (review finding R4 — replaces the fragile
/// TM-simulation/undecidability chain): the LINEAR shared-variable
/// rules R = {f(g(x),g(y)) -> f(x,y), f(a,a) -> a} peel one `g`
/// from BOTH branches simultaneously, so the ancestors of {a}
/// intersected with the recognizable slice
/// K = {f(g^m(a), g^n(a))} are exactly the diagonal {m == n} — not
/// recognizable by the Example 1.2.1 argument. Hence no exact
/// saturation API exists for left-linear systems with left/right
/// shared variables. Every step here is strict, so the witness
/// holds under both one-step semantics.
#[test]
fn shared_variable_saturation_intersects_to_the_diagonal() {
    let rules = [
        Rule::new(
            f(g(Term::var("x")), g(Term::var("y"))),
            f(Term::var("x"), Term::var("y")),
        )
        .unwrap(),
        Rule::new(f(a(), a()), a()).unwrap(),
    ];
    let universe = ground_terms(&AGF, 7);
    let saturation = bounded_saturation(&rules, &[a()], &universe);

    // Diagonal members within the bound are ancestors.
    assert!(saturation.contains(&f(a(), a())));
    assert!(saturation.contains(&f(g(a()), g(a()))));
    assert!(saturation.contains(&f(g(g(a())), g(g(a())))));
    // Off-diagonal slice members are not; assert them in-universe
    // first so the exclusions are non-vacuous.
    let off1 = f(g(a()), a());
    let off2 = f(g(g(a())), g(a()));
    assert!(universe.contains(&off1));
    assert!(universe.contains(&off2));
    assert!(!saturation.contains(&off1));
    assert!(!saturation.contains(&off2));
}

/// The gate relies on the TRS contract Var(rhs) ⊆ Var(lhs) — but
/// only the CHECKED constructor enforces it (review finding R5).
/// `Rule::new_unchecked` admits rules whose right side has free
/// variables; rewriting with them produces nonground terms, so the
/// T(F) semantics the gate proves does not hold for arbitrary
/// admitted values. The Task 24 classifier / exact API boundary must
/// therefore validate the containment for EVERY rule (and the common
/// finite ranked alphabet of rules and language) before emitting any
/// class or certificate.
#[test]
fn rule_contract_is_a_boundary_obligation_not_a_constructor_fact() {
    // Checked constructor: rejects.
    assert!(Rule::new(f(Term::var("x"), Term::var("x")), g(Term::var("y"))).is_err());
    // Unchecked constructor: admits. The oracle then derives a
    // nonground "successor", demonstrating the gap concretely.
    let unchecked = Rule::new_unchecked(g(Term::var("x")), Term::var("y"));
    let derived = successors(&[unchecked], &g(a()));
    assert_eq!(derived, vec![Term::var("y")]);
    assert!(!derived[0].variables().is_empty());
}
