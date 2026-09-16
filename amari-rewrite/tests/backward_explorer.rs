//! Bounded BackwardExplorer contract tests (0.25 Cohort 3, Task 14).

use amari_rewrite::inverse::{
    BackwardExplorer, BackwardSearchOutcome, ExhaustionAuthority, InverseSearchConfig, SearchMode,
};
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

fn witness_steps(outcome: BackwardSearchOutcome) -> usize {
    match outcome {
        BackwardSearchOutcome::Witness(derivation) => derivation.steps.len(),
        other => panic!("expected witness, got {other:?}"),
    }
}

/// Manually replay a witness derivation forward through the original
/// system: every returned witness must reconstruct the target.
fn replay_witness(
    system: &TermSystem,
    target: &Term,
    goal: &Term,
    outcome: &BackwardSearchOutcome,
) {
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("expected witness");
    };
    assert!(!derivation.steps.is_empty());
    // The final (deepest) step's predecessor term must unify with the
    // goal (existential variables may be instantiated either way).
    let leaf = derivation.steps.last().unwrap();
    let limits = amari_rewrite::relation::RelationLimits::default();
    let mut resources = amari_rewrite::relation::RelationResources::new(&limits);
    assert!(amari_rewrite::analysis::unify(&leaf.term, goal, &mut resources).is_ok());
    // Replay forward from the leaf term through each step in reverse.
    let mut current = leaf.term.clone();
    for step in derivation.steps.iter().rev() {
        let rule = system
            .rules()
            .iter()
            .find(|rule| {
                amari_rewrite::relation::RuleId::from_rule(rule) == step.provenance.rule_id
            })
            .expect("step rule must exist in the system");
        let position = current
            .subterm(&step.provenance.position)
            .expect("match path must be valid");
        let bindings =
            match_pattern(rule.lhs(), position).expect("lhs must match at the recorded path");
        let rewritten = bindings.apply(rule.rhs());
        current = current
            .replace_at(&step.provenance.position, rewritten)
            .expect("replacement must succeed");
    }
    assert_eq!(
        amari_rewrite::relation::Sha256Digest::canonical_term("amari.relation.term/v1", &current,),
        amari_rewrite::relation::Sha256Digest::canonical_term("amari.relation.term/v1", target,),
        "forward replay must reconstruct the target exactly"
    );
}

#[test]
fn direct_and_multi_step_witnesses_replay() {
    let rules = system(vec![
        Rule::new(parse("add(X, zero)"), parse("X")).unwrap(),
        Rule::new(parse("h(X)"), parse("X")).unwrap(),
    ]);
    let explorer = BackwardExplorer::new(&rules, small_config());
    // Direct: a <- add(a, zero).
    let direct = explorer
        .search(
            &parse("a"),
            &parse("add(a, zero)"),
            SearchMode::BreadthFirst,
        )
        .unwrap();
    assert_eq!(witness_steps(direct.clone()), 1);
    replay_witness(&rules, &parse("a"), &parse("add(a, zero)"), &direct);
    // Multi-step: a <- add(a, zero) <- h(add(a, zero)).
    let goal = parse("h(add(a, zero))");
    let multi = explorer
        .search(&parse("a"), &goal, SearchMode::BreadthFirst)
        .unwrap();
    assert_eq!(witness_steps(multi.clone()), 2);
    replay_witness(&rules, &parse("a"), &goal, &multi);
}

#[test]
fn existential_states_are_explored() {
    // Y is erased backward: predecessors of `a` include pair(a, ?).
    let rules = system(vec![Rule::new(parse("pair(X, Y)"), parse("X")).unwrap()]);
    let explorer = BackwardExplorer::new(&rules, small_config());
    let outcome = explorer
        .search(&parse("a"), &parse("pair(a, b)"), SearchMode::BreadthFirst)
        .unwrap();
    let BackwardSearchOutcome::Witness(derivation) = &outcome else {
        panic!("expected witness, got {outcome:?}");
    };
    assert_eq!(derivation.steps.len(), 1);
    assert_eq!(derivation.steps[0].existentials.len(), 1);
    replay_witness(&rules, &parse("a"), &parse("pair(a, b)"), &outcome);
}

#[test]
fn cycles_deduplicate_and_closed_frontier_certifies_exhaustion() {
    // The constant cycle a -> b -> c -> a closes backward: from `a`
    // the explorer reaches c, b, then `a` again (deduplicated), and
    // the frontier closes with nothing dropped.
    let rules = system(vec![
        Rule::new(parse("a"), parse("b")).unwrap(),
        Rule::new(parse("b"), parse("c")).unwrap(),
        Rule::new(parse("c"), parse("a")).unwrap(),
    ]);
    let explorer = BackwardExplorer::new(&rules, small_config());
    let outcome = explorer
        .search(&parse("a"), &parse("z"), SearchMode::BreadthFirst)
        .unwrap();
    let BackwardSearchOutcome::Exhausted(certified) = outcome else {
        panic!("expected certified exhaustion, got {outcome:?}");
    };
    assert_eq!(
        certified.authority(),
        &ExhaustionAuthority::ClosedSymbolicSearch
    );
    // In contrast, the ascending chain add(X, zero) -> X never closes
    // backward: hitting the depth ceiling is Partial, never Exhausted.
    let chain = system(vec![Rule::new(parse("add(X, zero)"), parse("X")).unwrap()]);
    let outcome = BackwardExplorer::new(&chain, small_config())
        .search(&parse("a"), &parse("z"), SearchMode::BreadthFirst)
        .unwrap();
    assert!(matches!(outcome, BackwardSearchOutcome::Partial(_)));
}

#[test]
fn depth_state_transition_byte_and_operation_ceilings_are_partial() {
    let rules = system(vec![Rule::new(parse("add(X, zero)"), parse("X")).unwrap()]);
    let target = parse("a");
    let goal = parse("c");
    // Depth 0: root cannot expand.
    let depth0 = InverseSearchConfig::new(
        1,
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
    .unwrap();
    // Depth 1 keeps the child but cannot expand it.
    let outcome = BackwardExplorer::new(&rules, depth0)
        .search(&target, &goal, SearchMode::BreadthFirst)
        .unwrap();
    assert!(matches!(outcome, BackwardSearchOutcome::Partial(_)));
    // One state only: the child cannot even be retained.
    let one_state = InverseSearchConfig::new(
        64,
        1,
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
    let outcome = BackwardExplorer::new(&rules, one_state)
        .search(&target, &goal, SearchMode::BreadthFirst)
        .unwrap();
    assert!(matches!(outcome, BackwardSearchOutcome::Partial(_)));
    // One transition only.
    let one_transition = InverseSearchConfig::new(
        64,
        1_024,
        1,
        4_096,
        64,
        4_096,
        65_536,
        100_000,
        1 << 20,
        1 << 20,
    )
    .unwrap();
    let outcome = BackwardExplorer::new(&rules, one_transition)
        .search(&target, &goal, SearchMode::BreadthFirst)
        .unwrap();
    assert!(matches!(outcome, BackwardSearchOutcome::Partial(_)));
    // Tiny frontier byte budget.
    let tiny_bytes = InverseSearchConfig::new(
        64,
        1_024,
        4_096,
        4_096,
        64,
        4_096,
        65_536,
        100_000,
        16,
        1 << 20,
    )
    .unwrap();
    let outcome = BackwardExplorer::new(&rules, tiny_bytes)
        .search(&target, &goal, SearchMode::BreadthFirst)
        .unwrap();
    assert!(matches!(outcome, BackwardSearchOutcome::Partial(_)));
    // One operation only.
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
    let outcome = BackwardExplorer::new(&rules, one_op)
        .search(&target, &goal, SearchMode::BreadthFirst)
        .unwrap();
    assert!(matches!(outcome, BackwardSearchOutcome::Partial(_)));
}

#[test]
fn partial_frontier_retains_unexpanded_states() {
    let rules = system(vec![Rule::new(parse("add(X, zero)"), parse("X")).unwrap()]);
    let depth1 = InverseSearchConfig::new(
        1,
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
    .unwrap();
    let outcome = BackwardExplorer::new(&rules, depth1)
        .search(&parse("a"), &parse("c"), SearchMode::BreadthFirst)
        .unwrap();
    let BackwardSearchOutcome::Partial(frontier) = outcome else {
        panic!("expected partial, got {outcome:?}");
    };
    assert_eq!(frontier.depth_reached, 1);
    assert_eq!(frontier.states.len(), 1);
    // The retained state is the canonical add(a, zero) predecessor.
    assert!(match_pattern(&parse("add(a, zero)"), frontier.states[0].term()).is_some());
}

#[test]
fn search_is_deterministic_in_both_modes() {
    let rules = system(vec![
        Rule::new(parse("add(X, zero)"), parse("X")).unwrap(),
        Rule::new(parse("h(X)"), parse("X")).unwrap(),
        Rule::new(parse("pair(X, Y)"), parse("X")).unwrap(),
    ]);
    for mode in [SearchMode::BreadthFirst, SearchMode::CostFirst] {
        let explorer = BackwardExplorer::new(&rules, small_config());
        let first = explorer.search(&parse("a"), &parse("c"), mode).unwrap();
        let second = explorer.search(&parse("a"), &parse("c"), mode).unwrap();
        assert_eq!(first, second);
    }
}

#[cfg(feature = "serialize")]
#[test]
fn outcomes_roundtrip_through_serde() {
    let rules = system(vec![Rule::new(parse("add(X, zero)"), parse("X")).unwrap()]);
    let explorer = BackwardExplorer::new(&rules, small_config());
    let outcome = explorer
        .search(
            &parse("a"),
            &parse("add(a, zero)"),
            SearchMode::BreadthFirst,
        )
        .unwrap();
    let json = serde_json::to_string(&outcome).unwrap();
    let back: BackwardSearchOutcome = serde_json::from_str(&json).unwrap();
    assert_eq!(outcome, back);
}
