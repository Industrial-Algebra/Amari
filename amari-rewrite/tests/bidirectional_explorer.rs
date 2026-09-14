//! Bounded BidirectionalExplorer contract tests
//! (0.25 Cohort 3, Task 15).

use amari_rewrite::inverse::{
    BidirectionalExplorer, BidirectionalSearchOutcome, ExhaustionAuthority, InverseSearchConfig,
};
use amari_rewrite::relation::{RelationLimits, RelationResources, RuleId};
use amari_rewrite::trs::{match_pattern, Rule, Term, TermSystem};

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

fn system(rules: Vec<Rule>) -> TermSystem {
    TermSystem::new(rules)
}

fn small_config() -> InverseSearchConfig {
    InverseSearchConfig::new(
        8,
        1_024,
        4_096,
        4_096,
        64,
        4_096,
        65_536,
        100_000,
        1 << 20,
        1 << 20,
    )
    .unwrap()
}

/// Replay the ENTIRE meeting path through the original system:
/// forward steps from the source, then the backward chain applied
/// forward from the meeting point to the goal.
fn replay_full_path(
    system: &TermSystem,
    source: &Term,
    goal: &Term,
    outcome: &BidirectionalSearchOutcome,
) {
    let BidirectionalSearchOutcome::Witness(derivation) = outcome else {
        panic!("expected witness, got {outcome:?}");
    };
    let forward_rule = |step: &amari_rewrite::inverse::ForwardStep| {
        system
            .rules()
            .iter()
            .find(|rule| RuleId::from_rule(rule) == step.rule_id)
            .expect("forward step rule must exist")
    };
    // Forward half: source -> meeting term.
    let mut current = source.clone();
    for step in &derivation.forward_steps {
        let rule = forward_rule(step);
        let bindings = match_pattern(
            rule.lhs(),
            current.subterm(&step.position).expect("forward path valid"),
        )
        .expect("forward lhs matches");
        current = current
            .replace_at(&step.position, bindings.apply(rule.rhs()))
            .expect("forward replacement");
    }
    assert_eq!(
        amari_rewrite::relation::Sha256Digest::canonical_term("amari.relation.term/v1", &current,),
        amari_rewrite::relation::Sha256Digest::canonical_term(
            "amari.relation.term/v1",
            &derivation.meeting_forward,
        ),
        "forward half must reach the meeting term"
    );
    // The two meeting terms must unify.
    let limits = RelationLimits::default();
    let mut resources = RelationResources::new(&limits);
    let unifier = amari_rewrite::analysis::unify(
        &derivation.meeting_forward,
        &derivation.meeting_backward,
        &mut resources,
    )
    .expect("meeting terms must unify");
    // Backward half, applied forward: meeting -> goal.
    let mut current = unifier.apply(&derivation.meeting_backward);
    for step in derivation.backward_steps.iter().rev() {
        let rule = system
            .rules()
            .iter()
            .find(|rule| RuleId::from_rule(rule) == step.provenance.rule_id)
            .expect("backward step rule must exist");
        let bindings = match_pattern(
            rule.lhs(),
            current
                .subterm(&step.provenance.position)
                .expect("path valid"),
        )
        .expect("lhs matches");
        current = current
            .replace_at(&step.provenance.position, bindings.apply(rule.rhs()))
            .expect("replacement");
    }
    assert_eq!(
        amari_rewrite::relation::Sha256Digest::canonical_term("amari.relation.term/v1", &current,),
        amari_rewrite::relation::Sha256Digest::canonical_term("amari.relation.term/v1", goal,),
        "full path must reconstruct the goal"
    );
}

#[test]
fn frontiers_meet_by_unification() {
    // add(X, zero) => X: forward from add(a, zero) reaches `a`, which
    // meets the backward root at the goal.
    let rules = system(vec![Rule::new(parse("add(X, zero)"), parse("X")).unwrap()]);
    let explorer = BidirectionalExplorer::new(&rules, small_config());
    let outcome = explorer
        .search(&parse("add(a, zero)"), &parse("a"))
        .unwrap();
    let BidirectionalSearchOutcome::Witness(derivation) = &outcome else {
        panic!("expected witness, got {outcome:?}");
    };
    assert_eq!(derivation.forward_steps.len(), 1);
    assert_eq!(derivation.backward_steps.len(), 0);
    replay_full_path(&rules, &parse("add(a, zero)"), &parse("a"), &outcome);
}

#[test]
fn meet_does_not_require_raw_term_equality() {
    // g(X) => f(X): forward from g(X) produces f(X); backward from
    // f(c) produces g(c). The forward f(X) meets the backward root
    // f(c) by UNIFICATION (X := c), not by raw equality.
    let rules = system(vec![Rule::new(parse("g(X)"), parse("f(X)")).unwrap()]);
    let explorer = BidirectionalExplorer::new(&rules, small_config());
    let outcome = explorer.search(&parse("g(X)"), &parse("f(c)")).unwrap();
    let BidirectionalSearchOutcome::Witness(derivation) = &outcome else {
        panic!("expected witness, got {outcome:?}");
    };
    assert_ne!(derivation.meeting_forward, derivation.meeting_backward);
    replay_full_path(&rules, &parse("g(X)"), &parse("f(c)"), &outcome);
}

#[test]
fn multi_step_both_sides_replay() {
    // h(X) => X and add(X, zero) => X: source h(add(a, zero)), goal
    // `a`. Forward: h(add(a, zero)) -> add(a, zero); that meets the
    // backward expansion a <- add(a, zero).
    let rules = system(vec![
        Rule::new(parse("h(X)"), parse("X")).unwrap(),
        Rule::new(parse("add(X, zero)"), parse("X")).unwrap(),
    ]);
    let explorer = BidirectionalExplorer::new(&rules, small_config());
    let outcome = explorer
        .search(&parse("h(add(a, zero))"), &parse("a"))
        .unwrap();
    let BidirectionalSearchOutcome::Witness(derivation) = &outcome else {
        panic!("expected witness, got {outcome:?}");
    };
    assert!(
        !derivation.forward_steps.is_empty() && !derivation.backward_steps.is_empty(),
        "both halves should contribute steps: {derivation:?}"
    );
    replay_full_path(&rules, &parse("h(add(a, zero))"), &parse("a"), &outcome);
}

#[test]
fn cyclic_system_with_no_meet_certifies_exhaustion() {
    // a => b, b => a forward cycle; goal z has no predecessors.
    let rules = system(vec![
        Rule::new(parse("a"), parse("b")).unwrap(),
        Rule::new(parse("b"), parse("a")).unwrap(),
    ]);
    let explorer = BidirectionalExplorer::new(&rules, small_config());
    let outcome = explorer.search(&parse("a"), &parse("z")).unwrap();
    let BidirectionalSearchOutcome::Exhausted(certified) = outcome else {
        panic!("expected certified exhaustion, got {outcome:?}");
    };
    assert_eq!(
        certified.authority(),
        &ExhaustionAuthority::ClosedSymbolicSearch
    );
}

#[test]
fn ceilings_yield_partial_with_both_frontiers() {
    let rules = system(vec![Rule::new(parse("add(X, zero)"), parse("X")).unwrap()]);
    let tight = InverseSearchConfig::new(
        64,
        2,
        4_096,
        4_096,
        64,
        4_096,
        65_536,
        100_000,
        1 << 20,
        1 << 20,
    )
    .unwrap();
    let outcome = BidirectionalExplorer::new(&rules, tight)
        .search(&parse("add(a, zero)"), &parse("z"))
        .unwrap();
    assert!(matches!(outcome, BidirectionalSearchOutcome::Partial(_)));
    let one_op = InverseSearchConfig::new(
        64,
        1_024,
        4_096,
        4_096,
        64,
        4_096,
        65_536,
        1,
        1 << 20,
        1 << 20,
    )
    .unwrap();
    let outcome = BidirectionalExplorer::new(&rules, one_op)
        .search(&parse("add(a, zero)"), &parse("z"))
        .unwrap();
    assert!(matches!(outcome, BidirectionalSearchOutcome::Partial(_)));
}

#[test]
fn search_is_deterministic() {
    let rules = system(vec![
        Rule::new(parse("h(X)"), parse("X")).unwrap(),
        Rule::new(parse("add(X, zero)"), parse("X")).unwrap(),
        Rule::new(parse("pair(X, Y)"), parse("X")).unwrap(),
    ]);
    let explorer = BidirectionalExplorer::new(&rules, small_config());
    let first = explorer
        .search(&parse("h(add(a, zero))"), &parse("a"))
        .unwrap();
    let second = explorer
        .search(&parse("h(add(a, zero))"), &parse("a"))
        .unwrap();
    assert_eq!(first, second);
}

#[test]
fn parity_with_one_directional_search() {
    // The one-directional BackwardExplorer finds add(a, zero) as a
    // predecessor of `a`; the bidirectional explorer with that
    // witness as its source must find a meeting witness for the same
    // relation, and its full path must replay.
    use amari_rewrite::inverse::{BackwardExplorer, BackwardSearchOutcome, SearchMode};
    let rules = system(vec![Rule::new(parse("add(X, zero)"), parse("X")).unwrap()]);
    let backward = BackwardExplorer::new(&rules, small_config())
        .search(
            &parse("a"),
            &parse("add(a, zero)"),
            SearchMode::BreadthFirst,
        )
        .unwrap();
    assert!(matches!(backward, BackwardSearchOutcome::Witness(_)));
    let bidirectional = BidirectionalExplorer::new(&rules, small_config())
        .search(&parse("add(a, zero)"), &parse("a"))
        .unwrap();
    assert!(matches!(
        bidirectional,
        BidirectionalSearchOutcome::Witness(_)
    ));
    replay_full_path(&rules, &parse("add(a, zero)"), &parse("a"), &bidirectional);
}

#[cfg(feature = "serialize")]
#[test]
fn outcomes_roundtrip_through_serde() {
    let rules = system(vec![Rule::new(parse("add(X, zero)"), parse("X")).unwrap()]);
    let outcome = BidirectionalExplorer::new(&rules, small_config())
        .search(&parse("add(a, zero)"), &parse("a"))
        .unwrap();
    let json = serde_json::to_string(&outcome).unwrap();
    let back: BidirectionalSearchOutcome = serde_json::from_str(&json).unwrap();
    assert_eq!(outcome, back);
}
