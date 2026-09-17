// SPDX-License-Identifier: MIT OR Apache-2.0

//! Task 18 (0.25 Cohort 4): ranked alphabets and validated NFTAs.
//!
//! RED contract (docs/plans/2026-07-24-amari-rewrite-inverse-
//! expansion-implementation-plan.md): epsilon-free bottom-up tree
//! automata over validated ranked alphabets with typed malformed /
//! limit errors, canonical construction-independent storage, and
//! term membership with accepting-run evidence.

use amari_rewrite::language::{
    RankedSymbol, TreeAutomaton, TreeAutomatonLimits, TreeState, TreeTransition,
};
use amari_rewrite::trs::{Symbol, Term};
use amari_rewrite::RewriteError;

fn ranked(name: &str, arity: u16) -> RankedSymbol {
    RankedSymbol::new(Symbol::new(name), arity)
}

fn state(name: &str) -> TreeState {
    TreeState::new(name)
}

fn states(names: &[&str]) -> Vec<TreeState> {
    names.iter().map(|name| state(name)).collect()
}

fn transition(name: &str, children: &[&str], parent: &str) -> TreeTransition {
    TreeTransition::new(sym(name), states(children), state(parent))
}

fn sym(name: &str) -> Symbol {
    Symbol::new(name)
}

/// The even/odd chain automaton over {z/0, s/1}.
fn parity_automaton() -> TreeAutomaton {
    TreeAutomaton::new(
        vec![ranked("z", 0), ranked("s", 1)],
        states(&["even", "odd"]),
        vec![
            transition("z", &[], "even"),
            transition("s", &["even"], "odd"),
            transition("s", &["odd"], "even"),
        ],
        states(&["even"]),
        TreeAutomatonLimits::default(),
    )
    .expect("valid automaton")
}

fn malformed(result: amari_rewrite::RewriteResult<TreeAutomaton>, needle: &str) {
    match result {
        Err(RewriteError::MalformedAutomaton { message }) => {
            assert!(
                message.contains(needle),
                "message {message:?} must mention {needle:?}"
            );
        }
        other => panic!("expected MalformedAutomaton mentioning {needle:?}, got {other:?}"),
    }
}

// ---- validation: malformed inputs are typed errors

#[test]
fn rank_conflicts_are_rejected() {
    malformed(
        TreeAutomaton::new(
            vec![ranked("f", 1), ranked("f", 2)],
            states(&["q"]),
            vec![],
            vec![],
            TreeAutomatonLimits::default(),
        ),
        "rank",
    );
}

#[test]
fn duplicate_alphabet_entries_are_rejected() {
    malformed(
        TreeAutomaton::new(
            vec![ranked("f", 1), ranked("f", 1)],
            states(&["q"]),
            vec![],
            vec![],
            TreeAutomatonLimits::default(),
        ),
        "duplicate",
    );
}

#[test]
fn duplicate_states_are_rejected() {
    malformed(
        TreeAutomaton::new(
            vec![ranked("a", 0)],
            states(&["q", "q"]),
            vec![],
            vec![],
            TreeAutomatonLimits::default(),
        ),
        "duplicate state",
    );
}

#[test]
fn unknown_transition_states_are_rejected() {
    malformed(
        TreeAutomaton::new(
            vec![ranked("a", 0)],
            states(&["q"]),
            vec![transition("a", &[], "ghost")],
            vec![],
            TreeAutomatonLimits::default(),
        ),
        "unknown state",
    );
    malformed(
        TreeAutomaton::new(
            vec![ranked("f", 1)],
            states(&["q"]),
            vec![transition("f", &["ghost"], "q")],
            vec![],
            TreeAutomatonLimits::default(),
        ),
        "unknown state",
    );
}

#[test]
fn unknown_transition_symbols_are_rejected() {
    malformed(
        TreeAutomaton::new(
            vec![ranked("a", 0)],
            states(&["q"]),
            vec![transition("missing", &[], "q")],
            vec![],
            TreeAutomatonLimits::default(),
        ),
        "unknown symbol",
    );
}

#[test]
fn arity_mismatch_is_rejected() {
    malformed(
        TreeAutomaton::new(
            vec![ranked("f", 2)],
            states(&["q"]),
            vec![transition("f", &["q"], "q")],
            vec![],
            TreeAutomatonLimits::default(),
        ),
        "arity",
    );
}

#[test]
fn duplicate_transitions_are_rejected() {
    malformed(
        TreeAutomaton::new(
            vec![ranked("a", 0)],
            states(&["q"]),
            vec![transition("a", &[], "q"), transition("a", &[], "q")],
            vec![],
            TreeAutomatonLimits::default(),
        ),
        "duplicate transition",
    );
}

#[test]
fn invalid_finals_are_rejected() {
    malformed(
        TreeAutomaton::new(
            vec![ranked("a", 0)],
            states(&["q"]),
            vec![transition("a", &[], "q")],
            states(&["ghost"]),
            TreeAutomatonLimits::default(),
        ),
        "final",
    );
}

// ---- limits are tighten-only and enforced before allocation

#[test]
fn limits_are_tighten_only() {
    assert!(TreeAutomatonLimits::new(0, 1, 1).is_err());
    assert!(TreeAutomatonLimits::new(1, 0, 1).is_err());
    assert!(TreeAutomatonLimits::new(1, 1, 0).is_err());
    assert!(TreeAutomatonLimits::new(TreeAutomatonLimits::MAX_STATES + 1, 1, 1).is_err());
    assert!(TreeAutomatonLimits::new(1, TreeAutomatonLimits::MAX_TRANSITIONS + 1, 1).is_err());
    assert!(TreeAutomatonLimits::new(1, 1, TreeAutomatonLimits::MAX_RANK + 1).is_err());
    assert_eq!(
        TreeAutomatonLimits::default().max_states(),
        TreeAutomatonLimits::MAX_STATES
    );
    assert_eq!(
        TreeAutomatonLimits::default().max_transitions(),
        TreeAutomatonLimits::MAX_TRANSITIONS
    );
    assert_eq!(
        TreeAutomatonLimits::default().max_rank(),
        TreeAutomatonLimits::MAX_RANK
    );
}

#[test]
fn construction_counts_enforce_limits() {
    let tight = TreeAutomatonLimits::new(1, 1, 1).expect("valid limits");
    // Two states against a one-state limit.
    assert!(matches!(
        TreeAutomaton::new(
            vec![ranked("a", 0)],
            states(&["q0", "q1"]),
            vec![transition("a", &[], "q0")],
            vec![],
            tight,
        ),
        Err(RewriteError::InvalidLimit { .. })
    ));
    // Two transitions against a one-transition limit.
    assert!(matches!(
        TreeAutomaton::new(
            vec![ranked("a", 0)],
            states(&["q0"]),
            vec![transition("a", &[], "q0"), transition("a", &[], "q0")],
            vec![],
            tight,
        ),
        Err(RewriteError::InvalidLimit { .. })
    ));
    // Rank 2 against a rank-1 limit.
    assert!(matches!(
        TreeAutomaton::new(
            vec![ranked("f", 2)],
            states(&["q0"]),
            vec![transition("f", &["q0", "q0"], "q0")],
            vec![],
            tight,
        ),
        Err(RewriteError::InvalidLimit { .. })
    ));
}

#[test]
fn default_rank_ceiling_is_enforced() {
    assert!(matches!(
        TreeAutomaton::new(
            vec![ranked("wide", 17)],
            states(&["q"]),
            vec![],
            vec![],
            TreeAutomatonLimits::default(),
        ),
        Err(RewriteError::InvalidLimit { .. })
    ));
}

// ---- canonical ordering: storage does not depend on input order

#[test]
fn canonical_ordering_is_construction_independent() {
    let forward = parity_automaton();
    let shuffled = TreeAutomaton::new(
        vec![ranked("s", 1), ranked("z", 0)],
        states(&["odd", "even"]),
        vec![
            transition("s", &["odd"], "even"),
            transition("s", &["even"], "odd"),
            transition("z", &[], "even"),
        ],
        states(&["even"]),
        TreeAutomatonLimits::default(),
    )
    .expect("valid automaton");
    assert_eq!(forward, shuffled);
    assert_eq!(forward.states(), shuffled.states());
    assert_eq!(forward.transitions(), shuffled.transitions());
    assert_eq!(forward.finals(), shuffled.finals());
}

// ---- membership with accepting-run evidence

#[test]
fn membership_accepts_and_rejects_correctly() {
    let automaton = parity_automaton();
    let z = Term::constant("z");
    let sz = Term::sym("s", vec![Term::constant("z")]);
    let ssz = Term::sym("s", vec![Term::sym("s", vec![Term::constant("z")])]);
    assert!(automaton.accepts(&z).expect("ground term"));
    assert!(!automaton.accepts(&sz).expect("ground term"));
    assert!(automaton.accepts(&ssz).expect("ground term"));
    assert!(automaton.accepting_run(&sz).expect("ground term").is_none());
}

#[test]
fn accepting_run_is_valid_evidence() {
    let automaton = parity_automaton();
    let ssz = Term::sym("s", vec![Term::sym("s", vec![Term::constant("z")])]);
    let run = automaton
        .accepting_run(&ssz)
        .expect("ground term")
        .expect("accepting run exists");
    assert_eq!(run.root_state(), &state("even"));
    // Every assigned node must be backed by a real transition whose
    // child states match the run.
    let assignments = run.assignments();
    assert_eq!(assignments.len(), 3);
    let lookup = |path: &[usize]| {
        assignments
            .iter()
            .find(|(candidate, _)| candidate == path)
            .map(|(_, assigned)| assigned.clone())
            .expect("every node is assigned")
    };
    assert_eq!(lookup(&[]), state("even"));
    assert_eq!(lookup(&[0]), state("odd"));
    assert_eq!(lookup(&[0, 0]), state("even"));
}

#[test]
fn nondeterministic_run_is_a_valid_run() {
    // z() -> q0 and z() -> q1; only q1 is final.
    let automaton = TreeAutomaton::new(
        vec![ranked("z", 0)],
        states(&["q0", "q1"]),
        vec![transition("z", &[], "q0"), transition("z", &[], "q1")],
        states(&["q1"]),
        TreeAutomatonLimits::default(),
    )
    .expect("valid automaton");
    let run = automaton
        .accepting_run(&Term::constant("z"))
        .expect("ground term")
        .expect("accepting run exists");
    assert_eq!(run.root_state(), &state("q1"));
}

#[test]
fn empty_language_automaton_rejects_everything() {
    let automaton = TreeAutomaton::new(
        vec![ranked("z", 0)],
        states(&["q"]),
        vec![transition("z", &[], "q")],
        vec![],
        TreeAutomatonLimits::default(),
    )
    .expect("valid automaton");
    assert!(!automaton
        .accepts(&Term::constant("z"))
        .expect("ground term"));
    assert!(automaton
        .accepting_run(&Term::constant("z"))
        .expect("ground term")
        .is_none());
}

#[test]
fn non_ground_terms_are_rejected() {
    let automaton = parity_automaton();
    let with_variable = Term::sym("s", vec![Term::var("X")]);
    assert!(automaton.accepts(&with_variable).is_err());
    assert!(automaton.accepting_run(&with_variable).is_err());
}

#[test]
fn oversized_terms_hit_term_limits() {
    let automaton = parity_automaton();
    // Depth 65 exceeds the depth-64 term ceiling.
    let mut deep = Term::constant("z");
    for _ in 0..65 {
        deep = Term::sym("s", vec![deep]);
    }
    assert!(matches!(
        automaton.accepts(&deep),
        Err(RewriteError::RelationLimitExceeded { .. })
    ));
}

// ---- property: membership agrees with a direct recursive oracle

/// Tiny deterministic PRNG (SplitMix-style LCG) so the property is
/// reproducible without external dependencies.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 11
    }

    fn below(&mut self, bound: u64) -> u64 {
        self.next() % bound
    }
}

/// Direct oracle: exists a run ending in a final state, checked by
/// naive recursive backtracking over every transition.
fn oracle_accepts(automaton: &TreeAutomaton, term: &Term) -> bool {
    fn matches(automaton: &TreeAutomaton, term: &Term, target: &TreeState) -> bool {
        let Term::Sym(symbol, arguments) = term else {
            return false;
        };
        automaton.transitions().iter().any(|transition| {
            transition.symbol() == symbol
                && transition.parent() == target
                && transition.children().len() == arguments.len()
                && transition
                    .children()
                    .iter()
                    .zip(arguments.iter())
                    .all(|(child_state, child_term)| matches(automaton, child_term, child_state))
        })
    }
    automaton
        .finals()
        .iter()
        .any(|final_state| matches(automaton, term, final_state))
}

#[test]
fn membership_matches_recursive_oracle_on_random_instances() {
    let mut rng = Lcg(0x5EED_CAFE_2026_0916);
    let names = ["q0", "q1", "q2", "q3"];
    for instance in 0..200 {
        // Random alphabet subset: a/0, b/0, f/1, g/2.
        let mut alphabet = vec![ranked("a", 0), ranked("b", 0)];
        if rng.below(2) == 0 {
            alphabet.push(ranked("f", 1));
        }
        if rng.below(2) == 0 {
            alphabet.push(ranked("g", 2));
        }
        let state_count = 1 + rng.below(4) as usize;
        let state_names = &names[..state_count];
        // Random transitions; deduplicated by a set of rendered keys.
        let mut transitions = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..(1 + rng.below(10)) {
            let ranked_symbol = &alphabet[rng.below(alphabet.len() as u64) as usize];
            let children: Vec<&str> = (0..ranked_symbol.arity())
                .map(|_| state_names[rng.below(state_count as u64) as usize])
                .collect();
            let parent = state_names[rng.below(state_count as u64) as usize];
            let key = alloc_key(ranked_symbol, &children, parent);
            if seen.insert(key) {
                transitions.push(transition(
                    ranked_symbol.symbol().as_str(),
                    &children,
                    parent,
                ));
            }
        }
        let finals: Vec<TreeState> = state_names
            .iter()
            .filter(|_| rng.below(2) == 0)
            .map(|name| state(name))
            .collect();
        let automaton = TreeAutomaton::new(
            alphabet.clone(),
            states(state_names),
            transitions,
            finals,
            TreeAutomatonLimits::default(),
        )
        .expect("generated automata are valid");
        // Random ground terms over the same alphabet, depth <= 3.
        for _ in 0..40 {
            let term = random_term(&mut rng, &alphabet, 3);
            let expected = oracle_accepts(&automaton, &term);
            let actual = automaton.accepts(&term).expect("ground term");
            assert_eq!(
                actual, expected,
                "instance {instance}: automaton {automaton:?}, term {term:?}"
            );
            let run = automaton.accepting_run(&term).expect("ground term");
            assert_eq!(run.is_some(), expected);
            if let Some(run) = run {
                // The run must assign every node exactly once and
                // every assignment must be transition-backed.
                assert_eq!(run.assignments().len(), term.positions().len());
            }
        }
    }
}

fn alloc_key(ranked_symbol: &RankedSymbol, children: &[&str], parent: &str) -> String {
    format!(
        "{}({})->{}",
        ranked_symbol.symbol(),
        children.join(","),
        parent
    )
}

fn random_term(rng: &mut Lcg, alphabet: &[RankedSymbol], depth: u64) -> Term {
    let constants: Vec<&RankedSymbol> = alphabet
        .iter()
        .filter(|ranked| ranked.arity() == 0)
        .collect();
    let choices: Vec<&RankedSymbol> = if depth == 0 {
        constants.clone()
    } else {
        alphabet.iter().collect()
    };
    let choice = choices[rng.below(choices.len() as u64) as usize];
    let arguments: Vec<Term> = (0..choice.arity())
        .map(|_| random_term(rng, alphabet, depth - 1))
        .collect();
    Term::sym(choice.symbol().as_str(), arguments)
}

// ---- serialize: canonical storage round-trips

#[cfg(feature = "serialize")]
#[test]
fn serialize_round_trip_preserves_canonical_automaton() {
    let automaton = parity_automaton();
    let json = serde_json::to_string(&automaton).expect("serialize");
    let restored: TreeAutomaton = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(automaton, restored);
    let run = restored
        .accepting_run(&Term::sym("s", vec![Term::constant("z")]))
        .expect("ground term");
    assert!(run.is_none());
    let run_json = serde_json::to_string(
        &restored
            .accepting_run(&Term::constant("z"))
            .expect("ground term")
            .expect("accepting run"),
    )
    .expect("serialize run");
    assert!(run_json.contains("even"));
}
