// SPDX-License-Identifier: MIT OR Apache-2.0

//! Metered language APIs: caller-supplied budgets are enforced
//! DURING execution and report the library's actual charge counts
//! (0.25 Cohort 4 closeout remediation, review findings 1 and 3).

use amari_rewrite::language::{RankedSymbol, TreeAutomaton, TreeState, TreeTransition};
use amari_rewrite::relation::{RelationLimits, RelationResources};
use amari_rewrite::trs::{Symbol, Term};
use amari_rewrite::RewriteError;

fn chain_automaton(states: usize) -> TreeAutomaton {
    // a -> q0; f(q{i-1}) -> qi; q{states-1} final.
    let alphabet = vec![
        RankedSymbol::new(Symbol::new("a"), 0),
        RankedSymbol::new(Symbol::new("f"), 1),
    ];
    let state_names: Vec<TreeState> = (0..states)
        .map(|index| TreeState::new(Symbol::new(format!("q{index}"))))
        .collect();
    let mut transitions = vec![TreeTransition::new(
        Symbol::new("a"),
        vec![],
        state_names[0].clone(),
    )];
    for index in 1..states {
        transitions.push(TreeTransition::new(
            Symbol::new("f"),
            vec![state_names[index - 1].clone()],
            state_names[index].clone(),
        ));
    }
    TreeAutomaton::new(
        alphabet,
        state_names,
        transitions,
        vec![TreeState::new(Symbol::new(format!("q{}", states - 1)))],
        amari_rewrite::language::TreeAutomatonLimits::default(),
    )
    .unwrap()
}

fn chain_term(depth: usize) -> Term {
    let mut term = Term::sym("a", vec![]);
    for _ in 0..depth {
        term = Term::sym("f", vec![term]);
    }
    term
}

fn budgeted(operations: usize) -> RelationResources {
    RelationResources::new(
        &RelationLimits::new(
            RelationLimits::MAX_TERM_NODES,
            RelationLimits::MAX_TERM_DEPTH,
            RelationLimits::MAX_CONSTRAINTS,
            operations,
        )
        .unwrap(),
    )
}

#[test]
fn membership_charges_transitions_per_node_and_enforces_budget() {
    let automaton = chain_automaton(2);
    let term = chain_term(1); // f(a): two nodes
    let mut resources = budgeted(1_000);
    let accepted = automaton
        .accepts_with_resources(&term, &mut resources)
        .unwrap();
    assert!(accepted);
    // 2 nodes x 2 transitions: the exact library charge.
    assert_eq!(resources.operations(), 4);
    // A budget of 3 aborts the run during execution.
    let mut tight = budgeted(3);
    let error = automaton
        .accepts_with_resources(&term, &mut tight)
        .expect_err("budget 3 < 4 charged operations");
    assert!(matches!(error, RewriteError::RelationLimitExceeded { .. }));
}

#[test]
fn witness_and_emptiness_are_metered_and_budgeted() {
    let automaton = chain_automaton(2);
    // In canonical order (a before f) the fixpoint propagates within
    // a single pass: 2 rounds x 2 transitions = 4 evaluations.
    let mut resources = budgeted(1_000);
    assert!(!automaton
        .language_is_empty_with_resources(&mut resources)
        .unwrap());
    assert_eq!(resources.operations(), 4);
    let mut tight = budgeted(3);
    let error = automaton
        .language_is_empty_with_resources(&mut tight)
        .expect_err("budget 3 < 4 fixpoint scans");
    assert!(matches!(error, RewriteError::RelationLimitExceeded { .. }));

    // Reverse dependency order (z -> s0, a(s0) -> s1) forces the
    // worst case: 3 rounds x 2 transitions = 6 evaluations.
    let reversed = TreeAutomaton::new(
        vec![
            RankedSymbol::new(Symbol::new("z"), 0),
            RankedSymbol::new(Symbol::new("a"), 1),
        ],
        vec![
            TreeState::new(Symbol::new("s0")),
            TreeState::new(Symbol::new("s1")),
        ],
        vec![
            TreeTransition::new(Symbol::new("z"), vec![], TreeState::new(Symbol::new("s0"))),
            TreeTransition::new(
                Symbol::new("a"),
                vec![TreeState::new(Symbol::new("s0"))],
                TreeState::new(Symbol::new("s1")),
            ),
        ],
        vec![TreeState::new(Symbol::new("s1"))],
        amari_rewrite::language::TreeAutomatonLimits::default(),
    )
    .unwrap();
    let mut resources = budgeted(1_000);
    assert!(!reversed
        .language_is_empty_with_resources(&mut resources)
        .unwrap());
    assert_eq!(resources.operations(), 6);

    let mut resources = budgeted(1_000);
    let witness = automaton
        .witness_with_resources(&mut resources)
        .unwrap()
        .expect("nonempty language");
    assert_eq!(witness, chain_term(1));
    assert!(
        resources.operations() >= 4,
        "witness charges the fixpoint scans: {}",
        resources.operations()
    );
    let mut tight = budgeted(1);
    let error = automaton
        .witness_with_resources(&mut tight)
        .expect_err("budget 1 cannot complete the fixpoint");
    assert!(matches!(error, RewriteError::RelationLimitExceeded { .. }));
}

#[test]
fn constructions_are_metered_and_budgeted() {
    let automaton = chain_automaton(3);
    let limits = amari_rewrite::language::TreeAutomatonLimits::default();

    let mut resources = budgeted(1_000);
    let union = automaton
        .union_with_resources(&automaton, &limits, &mut resources)
        .unwrap();
    assert!(!union.states().is_empty());
    assert!(resources.operations() > 0);
    let mut tight = budgeted(1);
    assert!(matches!(
        automaton.union_with_resources(&automaton, &limits, &mut tight),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));

    let mut resources = budgeted(1_000);
    automaton
        .intersection_with_resources(&automaton, &limits, &mut resources)
        .unwrap();
    assert!(resources.operations() > 0);
    let mut tight = budgeted(1);
    assert!(matches!(
        automaton.intersection_with_resources(&automaton, &limits, &mut tight),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));

    let mut resources = budgeted(1_000);
    automaton
        .determinize_with_resources(&limits, &mut resources)
        .unwrap();
    assert!(resources.operations() > 0);
    let mut tight = budgeted(1);
    assert!(matches!(
        automaton.determinize_with_resources(&limits, &mut tight),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));

    let mut resources = budgeted(1_000);
    automaton.minimized_with_resources(&mut resources).unwrap();
    assert!(resources.operations() > 0);
    let mut tight = budgeted(1);
    assert!(matches!(
        automaton.minimized_with_resources(&mut tight),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
}

/// Finding 1: a ~6000-deep chain term must be rejected with a typed
/// depth error, not an allocation abort from materializing
/// O(nodes*depth) path data first. Runs the body in a subprocess
/// under a 256 MiB address-space cap so the pre-fix quadratic
/// allocation actually aborts.
#[test]
fn deep_term_is_rejected_before_path_materialization() {
    if std::env::var_os("AMARI_DEEP_TERM_CHILD").is_some() {
        let automaton = chain_automaton(2);
        let term = chain_term(6_000);
        let error = automaton
            .accepts(&term)
            .expect_err("depth 6000 exceeds the depth ceiling");
        assert!(matches!(
            error,
            RewriteError::RelationLimitExceeded {
                resource: "term depth",
                ..
            }
        ));
        return;
    }
    let exe = std::env::current_exe().unwrap();
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "ulimit -v 262144 && exec \"$0\" --exact deep_term_is_rejected_before_path_materialization --nocapture"
        ))
        .arg(exe)
        .env("AMARI_DEEP_TERM_CHILD", "1")
        .status()
        .expect("spawn capped child");
    assert!(
        status.success(),
        "deep term must surface a typed error under a 256 MiB cap, not abort: {status}"
    );
}
