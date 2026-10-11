//! Finite grounding domain contract tests (0.25 Cohort 2, Task 9).
//!
//! `GroundingDomain` is a caller-supplied finite ranked symbol domain
//! with depth/count ceilings. Grounding enumerates existential
//! assignments in canonical size/lexicographic order and validates
//! constraints and forward replay before emitting terms. Empty or
//! oversized domains fail before allocation.

use amari_rewrite::inverse::{symbolic_predecessors, SymbolicPredecessor};
#[cfg(feature = "serialize")]
use amari_rewrite::relation::Sha256Digest;
use amari_rewrite::relation::{
    ConstraintSet, GroundingDomain, GroundingOutcome, RankedSymbol, RelationLimits, TermConstraint,
};
#[cfg(feature = "serialize")]
use amari_rewrite::trs::Substitution;
use amari_rewrite::trs::{Rule, Term, TermSystem};
use amari_rewrite::RewriteError;

fn domain(
    symbols: &[(&str, usize)],
    max_depth: usize,
    max_terms: usize,
) -> amari_rewrite::error::RewriteResult<GroundingDomain> {
    let ranked = symbols
        .iter()
        .map(|(name, arity)| RankedSymbol::new(*name, *arity))
        .collect();
    GroundingDomain::new(ranked, max_depth, max_terms)
}

fn resources_limits() -> RelationLimits {
    RelationLimits::default()
}

#[test]
fn domain_validation_rejects_invalid_specifications() {
    // Empty domain.
    assert!(matches!(
        domain(&[], 4, 16),
        Err(RewriteError::InvalidLimit { .. }) | Err(RewriteError::InvalidRule { .. })
    ));
    // No constants: no finite term is constructible.
    assert!(domain(&[("f", 1)], 4, 16).is_err());
    // Rank above ceiling.
    assert!(domain(&[("a", 0), ("f", 17)], 4, 16).is_err());
    // Too many symbols.
    let many: Vec<RankedSymbol> = (0..257)
        .map(|i| RankedSymbol::new(format!("c{i}"), 0))
        .collect();
    assert!(GroundingDomain::new(many, 4, 16).is_err());
    // Depth/terms above ceilings, or zero.
    assert!(domain(&[("a", 0)], 17, 16).is_err());
    assert!(domain(&[("a", 0)], 0, 16).is_err());
    assert!(domain(&[("a", 0)], 4, 65_537).is_err());
    assert!(domain(&[("a", 0)], 4, 0).is_err());
    // Duplicate ranked symbols.
    assert!(domain(&[("a", 0), ("a", 0)], 4, 16).is_err());
    // Same name at different arity is a distinct ranked symbol.
    assert!(domain(&[("a", 0), ("a", 1)], 4, 16).is_ok());
}

#[test]
fn enumeration_is_canonical_size_then_lexicographic() {
    let dom = domain(&[("b", 0), ("a", 0), ("f", 1)], 8, 8).unwrap();
    let mut resources = amari_rewrite::relation::RelationResources::new(&resources_limits());
    let terms = dom.enumerate(&mut resources).unwrap();
    let rendered: Vec<String> = terms.iter().map(|t| format!("{t:?}")).collect();
    // Size 1: constants in name order. Size 2: f of each constant.
    // Size 3: f of each size-2 term. Total capped at 8.
    let expected: Vec<String> = [
        "a",
        "b",
        "f(a)",
        "f(b)",
        "f(f(a))",
        "f(f(b))",
        "f(f(f(a)))",
        "f(f(f(b)))",
    ]
    .iter()
    .map(|s| format!("{:?}", parse(s)))
    .collect();
    assert_eq!(rendered, expected);
}

#[test]
fn enumeration_is_declaration_order_independent() {
    let first = domain(&[("b", 0), ("a", 0), ("f", 1)], 4, 6).unwrap();
    let second = domain(&[("f", 1), ("a", 0), ("b", 0)], 4, 6).unwrap();
    let mut r1 = amari_rewrite::relation::RelationResources::new(&resources_limits());
    let mut r2 = amari_rewrite::relation::RelationResources::new(&resources_limits());
    assert_eq!(
        first.enumerate(&mut r1).unwrap(),
        second.enumerate(&mut r2).unwrap(),
        "same symbol set enumerates identically regardless of order"
    );
}

#[test]
fn depth_and_term_ceilings_bound_enumeration() {
    // Depth 2 with unary f: a, b, f(a), f(b) — f(f(a)) is depth 3.
    let dom = domain(&[("a", 0), ("b", 0), ("f", 1)], 2, 64).unwrap();
    let mut resources = amari_rewrite::relation::RelationResources::new(&resources_limits());
    let terms = dom.enumerate(&mut resources).unwrap();
    assert_eq!(terms.len(), 4);
    assert!(terms.contains(&parse("f(a)")));
    assert!(!terms.contains(&parse("f(f(a))")));

    // Term count truncation.
    let dom = domain(&[("a", 0), ("f", 1)], 16, 3).unwrap();
    let mut resources = amari_rewrite::relation::RelationResources::new(&resources_limits());
    assert_eq!(dom.enumerate(&mut resources).unwrap().len(), 3);
}

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
    } else if text.chars().next().is_some_and(char::is_uppercase) || text.starts_with('?') {
        Term::var(text)
    } else {
        Term::constant(text)
    }
}

fn predecessor_system() -> (TermSystem, Term, SymbolicPredecessor) {
    // f(X, Y) -> g(X): Y is erased forward, existential backward.
    let sys = TermSystem::new(vec![Rule::new(parse("f(X, Y)"), parse("g(X)")).unwrap()]);
    let target = parse("g(a)");
    let pred = symbolic_predecessors(&sys, &target, &resources_limits(), 0)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    (sys, target, pred)
}

#[test]
fn grounding_assigns_existentials_and_replays_forward() {
    let (sys, target, pred) = predecessor_system();
    assert_eq!(pred.existentials.len(), 1);
    let dom = domain(&[("b", 0), ("c", 0)], 4, 16).unwrap();
    let outcome = amari_rewrite::relation::ground_predecessor(
        &sys,
        &target,
        &pred,
        &dom,
        &resources_limits(),
    )
    .unwrap();
    let GroundingOutcome::Complete { grounded } = outcome else {
        panic!("expected complete grounding");
    };
    let terms: Vec<Term> = grounded.iter().map(|g| g.term.clone()).collect();
    assert_eq!(terms, vec![parse("f(a, b)"), parse("f(a, c)")]);
    // Every emitted grounding replays forward to the target.
    for g in &grounded {
        let successors = sys.successors(&g.term).unwrap();
        assert!(successors.contains(&target));
    }
}

#[test]
fn grounding_validates_constraints_before_emission() {
    let (sys, target, mut pred) = predecessor_system();
    // Add a residual: the existential must differ from constant b.
    let existential = pred.existentials[0].to_string();
    let mut constraints = ConstraintSet::new();
    constraints.insert(TermConstraint::NotEqual(
        Term::var(existential),
        Term::constant("b"),
    ));
    pred.constraints = constraints;
    let dom = domain(&[("b", 0), ("c", 0)], 4, 16).unwrap();
    let outcome = amari_rewrite::relation::ground_predecessor(
        &sys,
        &target,
        &pred,
        &dom,
        &resources_limits(),
    )
    .unwrap();
    let GroundingOutcome::Complete { grounded } = outcome else {
        panic!("expected complete grounding");
    };
    let terms: Vec<Term> = grounded.iter().map(|g| g.term.clone()).collect();
    assert_eq!(terms, vec![parse("f(a, c)")]);
}

#[test]
fn grounding_filters_non_replaying_candidates() {
    let (sys, target, mut pred) = predecessor_system();
    // Tamper: the predecessor term no longer rewrites to the target.
    pred.term = parse("f(d, E)");
    let dom = domain(&[("b", 0), ("c", 0)], 4, 16).unwrap();
    let outcome = amari_rewrite::relation::ground_predecessor(
        &sys,
        &target,
        &pred,
        &dom,
        &resources_limits(),
    )
    .unwrap();
    let GroundingOutcome::Complete { grounded } = outcome else {
        panic!("expected complete grounding");
    };
    // Groundings exist but none replay: E is free and unassigned, so
    // grounding it via the domain still never reaches g(a).
    assert!(grounded.is_empty());
}

#[test]
fn budget_exhaustion_yields_partial_not_error() {
    let (sys, target, pred) = predecessor_system();
    // Rich domain, tiny operation budget: outcome is Partial.
    let dom = domain(&[("a", 0), ("b", 0), ("f", 1)], 6, 64).unwrap();
    let tight = RelationLimits::new(4_096, 64, 4_096, 12).unwrap();
    let outcome =
        amari_rewrite::relation::ground_predecessor(&sys, &target, &pred, &dom, &tight).unwrap();
    assert!(matches!(outcome, GroundingOutcome::Partial { .. }));
}

#[test]
fn grounded_terms_are_unique() {
    let (sys, target, pred) = predecessor_system();
    let dom = domain(&[("b", 0), ("c", 0)], 4, 16).unwrap();
    let outcome = amari_rewrite::relation::ground_predecessor(
        &sys,
        &target,
        &pred,
        &dom,
        &resources_limits(),
    )
    .unwrap();
    let GroundingOutcome::Complete { grounded } = outcome else {
        panic!("expected complete grounding");
    };
    let unique: std::collections::BTreeSet<_> = grounded.iter().map(|g| g.term.clone()).collect();
    assert_eq!(unique.len(), grounded.len());
}

#[test]
fn grounding_identity_when_no_existentials() {
    // h(X) -> k(X): nothing erased; the single grounding is the
    // predecessor itself (its free variables come from the target).
    let sys = TermSystem::new(vec![Rule::new(parse("h(X)"), parse("k(X)")).unwrap()]);
    let target = parse("k(a)");
    let pred = symbolic_predecessors(&sys, &target, &resources_limits(), 0)
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    assert!(pred.existentials.is_empty());
    let dom = domain(&[("b", 0)], 2, 4).unwrap();
    let outcome = amari_rewrite::relation::ground_predecessor(
        &sys,
        &target,
        &pred,
        &dom,
        &resources_limits(),
    )
    .unwrap();
    let GroundingOutcome::Complete { grounded } = outcome else {
        panic!("expected complete grounding");
    };
    assert_eq!(grounded.len(), 1);
    assert_eq!(grounded[0].term, parse("h(a)"));
}

#[cfg(feature = "serialize")]
#[test]
fn grounding_types_serde_roundtrip() {
    let dom = domain(&[("a", 0), ("f", 1)], 4, 8).unwrap();
    let json = serde_json::to_string(&dom).unwrap();
    let back: GroundingDomain = serde_json::from_str(&json).unwrap();
    assert_eq!(back, dom);
    let symbol = RankedSymbol::new("f", 1);
    let json = serde_json::to_string(&symbol).unwrap();
    assert_eq!(serde_json::from_str::<RankedSymbol>(&json).unwrap(), symbol);
    let _ = Substitution::new();
    let _ = Sha256Digest::from_bytes([0u8; 32]);
}
