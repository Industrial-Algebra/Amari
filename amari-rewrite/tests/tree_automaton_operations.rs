// SPDX-License-Identifier: MIT OR Apache-2.0

//! Task 19 (0.25 Cohort 4): language operations and witnesses.
//!
//! RED contract: emptiness and smallest canonical witness,
//! union/intersection with disjoint state renaming, reachable/
//! co-reachable trimming, empty-language behavior, and limits — all
//! with typed errors and canonical results.

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
    TreeTransition::new(Symbol::new(name), states(children), state(parent))
}

fn automaton(
    alphabet: Vec<RankedSymbol>,
    states: Vec<TreeState>,
    transitions: Vec<TreeTransition>,
    finals: Vec<TreeState>,
) -> TreeAutomaton {
    TreeAutomaton::new(
        alphabet,
        states,
        transitions,
        finals,
        TreeAutomatonLimits::default(),
    )
    .expect("test automata are valid")
}

/// {z/0, s/1} parity automaton accepting even-length chains.
fn parity_automaton() -> TreeAutomaton {
    automaton(
        vec![ranked("z", 0), ranked("s", 1)],
        states(&["even", "odd"]),
        vec![
            transition("z", &[], "even"),
            transition("s", &["even"], "odd"),
            transition("s", &["odd"], "even"),
        ],
        states(&["even"]),
    )
}

/// Automaton accepting exactly {a}.
fn just_a() -> TreeAutomaton {
    automaton(
        vec![ranked("a", 0), ranked("b", 0)],
        states(&["qa"]),
        vec![transition("a", &[], "qa")],
        states(&["qa"]),
    )
}

/// Automaton accepting exactly {b}, deliberately reusing the state
/// name "qa" so union must rename disjointly.
fn just_b_same_names() -> TreeAutomaton {
    automaton(
        vec![ranked("a", 0), ranked("b", 0)],
        states(&["qa"]),
        vec![transition("b", &[], "qa")],
        states(&["qa"]),
    )
}

// ---- emptiness and witnesses

#[test]
fn emptiness_reflects_final_reachability() {
    assert!(!parity_automaton().language_is_empty());
    // No finals: empty.
    let no_finals = automaton(
        vec![ranked("a", 0)],
        states(&["q"]),
        vec![transition("a", &[], "q")],
        vec![],
    );
    assert!(no_finals.language_is_empty());
    // Finals unreachable: no constant transition exists.
    let unreachable = automaton(
        vec![ranked("f", 1)],
        states(&["q"]),
        vec![transition("f", &["q"], "q")],
        states(&["q"]),
    );
    assert!(unreachable.language_is_empty());
}

#[test]
fn witness_is_the_smallest_accepted_term() {
    let parity = parity_automaton();
    assert_eq!(parity.witness(), Some(Term::constant("z")));
    // No witness for an empty language.
    let empty = automaton(vec![ranked("a", 0)], states(&["q"]), vec![], vec![]);
    assert_eq!(empty.witness(), None);
}

#[test]
fn witness_tie_breaks_canonically_and_is_deterministic() {
    // Accepts both a and b; "a" sorts first canonically.
    let both = automaton(
        vec![ranked("a", 0), ranked("b", 0)],
        states(&["q"]),
        vec![transition("b", &[], "q"), transition("a", &[], "q")],
        states(&["q"]),
    );
    let first = both.witness().expect("nonempty");
    assert_eq!(first, Term::constant("a"));
    assert_eq!(both.witness(), Some(first.clone()));
    // The witness is genuinely accepted.
    assert!(both.accepts(&first).expect("ground term"));
    // A deeper witness: only even chains of length >= 2 accepted.
    let only_ssz = automaton(
        vec![ranked("z", 0), ranked("s", 1)],
        states(&["p0", "p1", "p2"]),
        vec![
            transition("z", &[], "p0"),
            transition("s", &["p0"], "p1"),
            transition("s", &["p1"], "p2"),
        ],
        states(&["p2"]),
    );
    assert_eq!(
        only_ssz.witness(),
        Some(Term::sym(
            "s",
            vec![Term::sym("s", vec![Term::constant("z")])]
        ))
    );
}

// ---- union

#[test]
fn union_membership_is_the_disjunction() {
    let a = just_a();
    let b = just_b_same_names();
    let union = a
        .union(&b, &TreeAutomatonLimits::default())
        .expect("union of compatible alphabets");
    assert!(union.accepts(&Term::constant("a")).expect("ground"));
    assert!(union.accepts(&Term::constant("b")).expect("ground"));
    // State name collision between the operands must not merge
    // behavior: with shared names and no renaming, qa would gain
    // both transitions but stay ONE state; union semantics would
    // still hold here, so also check the harder case where naive
    // merging breaks: a chain over shared state names.
    // The collision counterexample: both chains use q0/q1, and a
    // naive merge (no renaming) would spuriously accept z and
    // s(s(z)) by mixing A's entry with B's exit.
    // A = {z->q0, s(q0)->q1; F={q1}}: accepts exactly s(z).
    let chain_a = automaton(
        vec![ranked("z", 0), ranked("s", 1)],
        states(&["q0", "q1"]),
        vec![transition("z", &[], "q0"), transition("s", &["q0"], "q1")],
        states(&["q1"]),
    );
    // B = {z->q1, s(q1)->q0; F={q0}}: accepts exactly s(z).
    let chain_b = automaton(
        vec![ranked("z", 0), ranked("s", 1)],
        states(&["q0", "q1"]),
        vec![transition("z", &[], "q1"), transition("s", &["q1"], "q0")],
        states(&["q0"]),
    );
    let merged = chain_a
        .union(&chain_b, &TreeAutomatonLimits::default())
        .expect("union");
    let z = Term::constant("z");
    let sz = Term::sym("s", vec![Term::constant("z")]);
    let ssz = Term::sym("s", vec![sz.clone()]);
    assert!(merged.accepts(&sz).expect("ground"));
    // z and s(s(z)) are in NEITHER operand language; naive state
    // merging would accept both.
    assert!(!merged.accepts(&z).expect("ground"));
    assert!(!merged.accepts(&ssz).expect("ground"));
    // Disjoint renaming is visible: four distinct states.
    assert_eq!(merged.states().len(), 4);
}

#[test]
fn union_rejects_alphabet_mismatch() {
    let a = just_a();
    let other = automaton(
        vec![ranked("c", 0)],
        states(&["qc"]),
        vec![transition("c", &[], "qc")],
        states(&["qc"]),
    );
    assert!(matches!(
        a.union(&other, &TreeAutomatonLimits::default()),
        Err(RewriteError::MalformedAutomaton { .. })
    ));
}

#[test]
fn union_with_empty_language_is_the_other_language() {
    let a = just_a();
    let empty = automaton(
        vec![ranked("a", 0), ranked("b", 0)],
        states(&["qe"]),
        vec![],
        vec![],
    );
    let union = a
        .union(&empty, &TreeAutomatonLimits::default())
        .expect("union");
    assert!(union.accepts(&Term::constant("a")).expect("ground"));
    assert!(!union.accepts(&Term::constant("b")).expect("ground"));
}

#[test]
fn union_enforces_state_limits() {
    let tight = TreeAutomatonLimits::new(2, 65_536, 16).expect("limits");
    let a = just_a();
    let b = just_b_same_names();
    // 1 + 1 renamed states fits; but against max_states = 1 it must
    // fail.
    let too_tight = TreeAutomatonLimits::new(1, 65_536, 16).expect("limits");
    assert!(matches!(
        a.union(&b, &too_tight),
        Err(RewriteError::InvalidLimit { .. })
    ));
    assert!(a.union(&b, &tight).is_ok());
}

// ---- intersection

#[test]
fn intersection_membership_is_the_conjunction() {
    let parity = parity_automaton();
    // Accepts z only (chain depth 0).
    let only_z = automaton(
        vec![ranked("z", 0), ranked("s", 1)],
        states(&["r0", "r1"]),
        vec![
            transition("z", &[], "r0"),
            transition("s", &["r0"], "r1"),
            transition("s", &["r1"], "r1"),
        ],
        states(&["r0"]),
    );
    let both = parity
        .intersection(&only_z, &TreeAutomatonLimits::default())
        .expect("intersection");
    let z = Term::constant("z");
    let ssz = Term::sym("s", vec![Term::sym("s", vec![Term::constant("z")])]);
    assert!(both.accepts(&z).expect("ground"));
    assert!(!both.accepts(&ssz).expect("ground"));
    assert_eq!(both.witness(), Some(z));
}

#[test]
fn intersection_with_empty_language_is_empty() {
    let parity = parity_automaton();
    let empty = automaton(
        vec![ranked("z", 0), ranked("s", 1)],
        states(&["qe"]),
        vec![],
        vec![],
    );
    let both = parity
        .intersection(&empty, &TreeAutomatonLimits::default())
        .expect("intersection");
    assert!(both.language_is_empty());
    assert_eq!(both.witness(), None);
}

#[test]
fn intersection_enforces_product_state_limits() {
    let tight = TreeAutomatonLimits::new(3, 65_536, 16).expect("limits");
    let parity = parity_automaton();
    let other = parity_automaton();
    // The product needs 2 x 2 = 4 states.
    assert!(matches!(
        parity.intersection(&other, &tight),
        Err(RewriteError::InvalidLimit { .. })
    ));
}

// ---- trimming

#[test]
fn trimming_removes_unreachable_and_non_co_reachable_states() {
    // q_dead is unreachable (no constant produces it); q_barren is
    // reachable but cannot reach any final.
    let automaton = automaton(
        vec![ranked("z", 0), ranked("s", 1)],
        states(&["q", "q_dead", "q_barren"]),
        vec![
            transition("z", &[], "q"),
            transition("z", &[], "q_barren"),
            transition("s", &["q_barren"], "q_barren"),
            transition("s", &["q_dead"], "q_dead"),
        ],
        states(&["q"]),
    );
    let trimmed = automaton.trimmed().expect("trimming is sound");
    assert_eq!(trimmed.states(), &states(&["q"]));
    assert_eq!(trimmed.transitions().len(), 1);
    assert!(trimmed.accepts(&Term::constant("z")).expect("ground"));
    assert!(!trimmed
        .accepts(&Term::sym("s", vec![Term::constant("z")]))
        .expect("ground"));
    // Trimming is idempotent.
    let twice = trimmed.trimmed().expect("idempotent");
    assert_eq!(trimmed, twice);
}

#[test]
fn trimming_an_empty_language_yields_an_empty_automaton() {
    let automaton = automaton(
        vec![ranked("z", 0), ranked("s", 1)],
        states(&["q"]),
        vec![transition("z", &[], "q"), transition("s", &["q"], "q")],
        vec![],
    );
    let trimmed = automaton.trimmed().expect("trimming is sound");
    assert!(trimmed.states().is_empty());
    assert!(trimmed.transitions().is_empty());
    assert!(trimmed.language_is_empty());
    // The alphabet is retained so further operations stay
    // well-formed.
    assert_eq!(trimmed.alphabet().len(), 2);
}

#[test]
fn trimming_preserves_the_language() {
    let parity = parity_automaton();
    let trimmed = parity.trimmed().expect("trimming is sound");
    assert_eq!(parity, trimmed);
    let mut term = Term::constant("z");
    for depth in 0..6 {
        assert_eq!(
            parity.accepts(&term).expect("ground"),
            trimmed.accepts(&term).expect("ground"),
            "depth {depth}"
        );
        term = Term::sym("s", vec![term]);
    }
}

// ---- property: Boolean operations and trimming on random automata

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

fn random_automaton(rng: &mut Lcg, alphabet: &[RankedSymbol], tag: u64) -> TreeAutomaton {
    // State names are deliberately shared across calls so union
    // renaming is exercised.
    let names = ["p0", "p1", "p2"];
    let state_count = 1 + rng.below(3) as usize;
    let state_names = &names[..state_count];
    let mut transitions = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..(1 + rng.below(8)) {
        let ranked_symbol = &alphabet[rng.below(alphabet.len() as u64) as usize];
        let children: Vec<&str> = (0..ranked_symbol.arity())
            .map(|_| state_names[rng.below(state_count as u64) as usize])
            .collect();
        let parent = state_names[rng.below(state_count as u64) as usize];
        let key = format!(
            "{}:{}({})->{}",
            tag,
            ranked_symbol.symbol(),
            children.join(","),
            parent
        );
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
    automaton(alphabet.to_vec(), states(state_names), transitions, finals)
}

fn random_term(rng: &mut Lcg, alphabet: &[RankedSymbol], depth: u64) -> Term {
    let constants: Vec<&RankedSymbol> = alphabet
        .iter()
        .filter(|ranked| ranked.arity() == 0)
        .collect();
    let choices: Vec<&RankedSymbol> = if depth == 0 {
        constants
    } else {
        alphabet.iter().collect()
    };
    let choice = choices[rng.below(choices.len() as u64) as usize];
    let arguments: Vec<Term> = (0..choice.arity())
        .map(|_| random_term(rng, alphabet, depth - 1))
        .collect();
    Term::sym(choice.symbol().as_str(), arguments)
}

#[test]
fn boolean_operations_and_trimming_preserve_membership() {
    let mut rng = Lcg(0xB00E_4A19_2026_0916);
    let alphabet = vec![
        ranked("a", 0),
        ranked("b", 0),
        ranked("f", 1),
        ranked("g", 2),
    ];
    for instance in 0..100 {
        let left = random_automaton(&mut rng, &alphabet, 0);
        let right = random_automaton(&mut rng, &alphabet, 1);
        let union = left
            .union(&right, &TreeAutomatonLimits::default())
            .expect("union");
        let intersection = left
            .intersection(&right, &TreeAutomatonLimits::default())
            .expect("intersection");
        let trimmed_left = left.trimmed().expect("trim");
        assert_eq!(left.language_is_empty(), trimmed_left.language_is_empty());
        assert_eq!(left.witness(), trimmed_left.witness());
        for _ in 0..30 {
            let term = random_term(&mut rng, &alphabet, 3);
            let in_left = left.accepts(&term).expect("ground");
            let in_right = right.accepts(&term).expect("ground");
            assert_eq!(
                union.accepts(&term).expect("ground"),
                in_left || in_right,
                "instance {instance} union"
            );
            assert_eq!(
                intersection.accepts(&term).expect("ground"),
                in_left && in_right,
                "instance {instance} intersection"
            );
            assert_eq!(
                trimmed_left.accepts(&term).expect("ground"),
                in_left,
                "instance {instance} trim"
            );
        }
        // Witness honesty: a reported witness is always accepted.
        if let Some(witness) = union.witness() {
            assert!(union.accepts(&witness).expect("ground"));
        }
        // Canonical results: union is insensitive to operand input
        // order only up to the L/R renaming, but repeated calls are
        // identical.
        let union_again = left
            .union(&right, &TreeAutomatonLimits::default())
            .expect("union");
        assert_eq!(union, union_again);
    }
}
