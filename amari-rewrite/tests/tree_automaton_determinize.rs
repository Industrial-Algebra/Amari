// SPDX-License-Identifier: MIT OR Apache-2.0

//! Task 20 (0.25 Cohort 4): determinization, completion, complement,
//! and minimization of tree automata.
//!
//! RED contract: bounded subset construction that never returns a
//! truncated automaton as complete; deterministic/completeness
//! checks; complement only for deterministic complete automata;
//! Myhill–Nerode minimization with language parity, idempotence,
//! and canonical bytes.

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

/// Nondeterministic: z enters q0 AND q1; only f(q1) reaches the
/// final. Language: {f(z)}.
fn nondeterministic_fz() -> TreeAutomaton {
    automaton(
        vec![ranked("z", 0), ranked("f", 1)],
        states(&["q0", "q1", "qf"]),
        vec![
            transition("z", &[], "q0"),
            transition("z", &[], "q1"),
            transition("f", &["q0"], "q0"),
            transition("f", &["q1"], "qf"),
        ],
        states(&["qf"]),
    )
}

/// Deterministic but incomplete parity automaton.
fn parity() -> TreeAutomaton {
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

// ---- determinization

#[test]
fn determinization_preserves_the_language_and_is_deterministic() {
    let automaton = nondeterministic_fz();
    assert!(!automaton.is_deterministic());
    let deterministic = automaton
        .determinize(&TreeAutomatonLimits::default())
        .expect("determinize");
    assert!(deterministic.is_deterministic());
    let z = Term::constant("z");
    let fz = Term::sym("f", vec![Term::constant("z")]);
    let ffz = Term::sym("f", vec![fz.clone()]);
    for term in [z, fz, ffz] {
        assert_eq!(
            automaton.accepts(&term).expect("ground"),
            deterministic.accepts(&term).expect("ground"),
            "parity for {term:?}"
        );
    }
}

#[test]
fn determinization_explosion_is_a_typed_limit_not_a_truncated_automaton() {
    // Non-deterministic branching engineered so the subset
    // construction needs more macro-states than a tight caller
    // limit permits.
    let automaton = automaton(
        vec![ranked("a", 0), ranked("b", 0), ranked("f", 1)],
        states(&["q0", "q1", "q2"]),
        vec![
            transition("a", &[], "q0"),
            transition("b", &[], "q0"),
            transition("a", &[], "q1"),
            transition("b", &[], "q2"),
            transition("f", &["q0"], "q0"),
            transition("f", &["q1"], "q1"),
            transition("f", &["q2"], "q2"),
            transition("f", &["q0"], "q2"),
            transition("f", &["q1"], "q2"),
        ],
        states(&["q2"]),
    );
    // The subset construction reaches {q0}, {q0,q1}, {q0,q2},
    // {q0,q1,q2}: four macro-states. A two-state caller limit must
    // produce a typed error — never a truncated automaton.
    let tight = TreeAutomatonLimits::new(2, 65_536, 16).expect("limits");
    assert!(matches!(
        automaton.determinize(&tight),
        Err(RewriteError::InvalidLimit { .. })
    ));
    let fits = TreeAutomatonLimits::new(4, 65_536, 16).expect("limits");
    assert!(automaton.determinize(&fits).is_ok());
}

// ---- determinism and completeness

#[test]
fn determinism_and_completeness_predicates() {
    let parity = parity();
    assert!(parity.is_deterministic());
    // Parity covers z/0 and s/1 x {even, odd}: it IS complete.
    assert!(parity.is_complete());
    let partial = automaton(
        vec![ranked("z", 0), ranked("s", 1)],
        states(&["q"]),
        vec![transition("z", &[], "q")],
        states(&["q"]),
    );
    assert!(partial.is_deterministic());
    assert!(!partial.is_complete());
}

#[test]
fn completion_adds_a_sink_and_preserves_the_language() {
    let partial = automaton(
        vec![ranked("z", 0), ranked("s", 1)],
        states(&["q"]),
        vec![transition("z", &[], "q")],
        states(&["q"]),
    );
    let complete = partial.completed().expect("completion");
    assert!(complete.is_complete());
    assert!(complete.is_deterministic());
    let z = Term::constant("z");
    let sz = Term::sym("s", vec![Term::constant("z")]);
    assert!(complete.accepts(&z).expect("ground"));
    assert!(!complete.accepts(&sz).expect("ground"));
    // Completion of a complete automaton returns it unchanged.
    assert_eq!(parity().completed().expect("complete"), parity());
    // Completion requires determinism.
    assert!(matches!(
        nondeterministic_fz().completed(),
        Err(RewriteError::MalformedAutomaton { .. })
    ));
}

// ---- complement

#[test]
fn complement_laws_hold() {
    let complete = parity().completed().expect("completion");
    let complement = complete.complemented().expect("complement");
    let z = Term::constant("z");
    let sz = Term::sym("s", vec![Term::constant("z")]);
    let ssz = Term::sym("s", vec![sz.clone()]);
    for term in [z.clone(), sz.clone(), ssz.clone()] {
        assert_ne!(
            complete.accepts(&term).expect("ground"),
            complement.accepts(&term).expect("ground"),
            "complement flips {term:?}"
        );
    }
    // Double complement is the original language.
    let twice = complement.complemented().expect("double complement");
    for term in [z, sz, ssz] {
        assert_eq!(
            complete.accepts(&term).expect("ground"),
            twice.accepts(&term).expect("ground")
        );
    }
    // Complement requires deterministic + complete.
    assert!(matches!(
        nondeterministic_fz().complemented(),
        Err(RewriteError::MalformedAutomaton { .. })
    ));
    assert!(matches!(
        parity_incomplete().complemented(),
        Err(RewriteError::MalformedAutomaton { .. })
    ));
}

fn parity_incomplete() -> TreeAutomaton {
    automaton(
        vec![ranked("z", 0), ranked("s", 1)],
        states(&["even"]),
        vec![transition("z", &[], "even")],
        states(&["even"]),
    )
}

// ---- minimization

/// Deterministic with redundant context-equivalent states: q1 and
/// q2 behave identically (both feed f(_) -> qf, neither is final),
/// so the known minimal state count is 2, not 3.
fn redundant_ab() -> TreeAutomaton {
    automaton(
        vec![ranked("a", 0), ranked("b", 0), ranked("f", 1)],
        states(&["q1", "q2", "qf"]),
        vec![
            transition("a", &[], "q1"),
            transition("b", &[], "q2"),
            transition("f", &["q1"], "qf"),
            transition("f", &["q2"], "qf"),
        ],
        states(&["qf"]),
    )
}

/// Hand-written minimal equivalent of `redundant_ab`: {f(a), f(b)}.
fn direct_ab() -> TreeAutomaton {
    automaton(
        vec![ranked("a", 0), ranked("b", 0), ranked("f", 1)],
        states(&["p", "qf"]),
        vec![
            transition("a", &[], "p"),
            transition("b", &[], "p"),
            transition("f", &["p"], "qf"),
        ],
        states(&["qf"]),
    )
}

#[test]
fn minimization_merges_context_equivalent_states() {
    let redundant = redundant_ab();
    assert!(redundant.is_deterministic());
    let minimal = redundant.minimized().expect("minimization");
    assert_eq!(minimal.states().len(), 2);
    let fa = Term::sym("f", vec![Term::constant("a")]);
    let fb = Term::sym("f", vec![Term::constant("b")]);
    let ffa = Term::sym("f", vec![fa.clone()]);
    for term in [fa, fb] {
        assert!(minimal.accepts(&term).expect("ground"), "{term:?}");
    }
    for term in [Term::constant("a"), Term::constant("b"), ffa] {
        assert!(!minimal.accepts(&term).expect("ground"), "{term:?}");
    }
    // Idempotence: minimizing again changes nothing.
    let twice = minimal.minimized().expect("idempotent");
    assert_eq!(minimal, twice);
    assert_eq!(minimal.canonical_bytes(), twice.canonical_bytes());
}

#[test]
fn minimization_preserves_the_language_on_parity() {
    let parity = parity();
    let minimal = parity.minimized().expect("minimization");
    let mut term = Term::constant("z");
    for _ in 0..6 {
        assert_eq!(
            parity.accepts(&term).expect("ground"),
            minimal.accepts(&term).expect("ground"),
            "{term:?}"
        );
        term = Term::sym("s", vec![term]);
    }
    // Parity is already minimal: two states.
    assert_eq!(minimal.states().len(), 2);
}

#[test]
fn minimization_requires_determinism() {
    assert!(matches!(
        nondeterministic_fz().minimized(),
        Err(RewriteError::MalformedAutomaton { .. })
    ));
}

#[test]
fn canonical_bytes_are_construction_order_independent() {
    let forward = parity();
    let shuffled = automaton(
        vec![ranked("s", 1), ranked("z", 0)],
        states(&["odd", "even"]),
        vec![
            transition("s", &["odd"], "even"),
            transition("s", &["even"], "odd"),
            transition("z", &[], "even"),
        ],
        states(&["even"]),
    );
    assert_eq!(forward.canonical_bytes(), shuffled.canonical_bytes());
    // Different languages give different bytes.
    let other = nondeterministic_fz();
    assert_ne!(forward.canonical_bytes(), other.canonical_bytes());
}

#[test]
fn language_equivalent_automata_minimize_to_identical_bytes() {
    // The redundant automaton and the hand-written minimal
    // equivalent must minimize to byte-identical canonical forms
    // (the minimal automaton is unique up to renaming, and the
    // canonical renumbering is deterministic).
    let a = redundant_ab().minimized().expect("minimize redundant");
    let b = direct_ab().minimized().expect("minimize direct");
    assert_eq!(a, b);
    assert_eq!(a.canonical_bytes(), b.canonical_bytes());
}

// ---- property: bounded De Morgan / parity laws on random automata

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

fn random_automaton(rng: &mut Lcg, alphabet: &[RankedSymbol]) -> TreeAutomaton {
    let names = ["p0", "p1", "p2"];
    let state_count = 1 + rng.below(3) as usize;
    let state_names = &names[..state_count];
    let mut transitions = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..(1 + rng.below(7)) {
        let ranked_symbol = &alphabet[rng.below(alphabet.len() as u64) as usize];
        let children: Vec<&str> = (0..ranked_symbol.arity())
            .map(|_| state_names[rng.below(state_count as u64) as usize])
            .collect();
        let parent = state_names[rng.below(state_count as u64) as usize];
        let key = format!(
            "{}({})->{}",
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
fn determinize_minimize_complement_laws_on_random_automata() {
    let mut rng = Lcg(0xDE12_0C20_2026_0918);
    let alphabet = vec![
        ranked("a", 0),
        ranked("b", 0),
        ranked("f", 1),
        ranked("g", 2),
    ];
    for instance in 0..60 {
        let left = random_automaton(&mut rng, &alphabet);
        let right = random_automaton(&mut rng, &alphabet);
        let left_d = left
            .determinize(&TreeAutomatonLimits::default())
            .expect("determinize left");
        let right_d = right
            .determinize(&TreeAutomatonLimits::default())
            .expect("determinize right");
        assert!(left_d.is_deterministic());
        let left_c = left_d.completed().expect("complete");
        let right_c = right_d.completed().expect("complete");
        let not_left = left_c.complemented().expect("complement");
        let not_right = right_c.complemented().expect("complement");
        // De Morgan: complement(A ∪ B) == complement(A) ∩ complement(B)
        let union = left
            .union(&right, &TreeAutomatonLimits::default())
            .expect("union");
        let union_d = union
            .determinize(&TreeAutomatonLimits::default())
            .expect("determinize union");
        let not_union = union_d
            .completed()
            .expect("complete")
            .complemented()
            .expect("comp");
        let inter_of_complements = not_left
            .intersection(&not_right, &TreeAutomatonLimits::default())
            .expect("intersection");
        // Minimization parity + bounded size.
        let left_min = left_d.minimized().expect("minimize");
        assert!(left_min.states().len() <= left_d.trimmed().expect("trim").states().len() + 1);
        for _ in 0..25 {
            let term = random_term(&mut rng, &alphabet, 3);
            let in_left = left.accepts(&term).expect("ground");
            let in_right = right.accepts(&term).expect("ground");
            // determinize parity
            assert_eq!(
                left_d.accepts(&term).expect("ground"),
                in_left,
                "det {instance}"
            );
            // completion preserves
            assert_eq!(
                left_c.accepts(&term).expect("ground"),
                in_left,
                "comp {instance}"
            );
            // complement flips
            assert_eq!(
                not_left.accepts(&term).expect("ground"),
                !in_left,
                "compl {instance}"
            );
            // minimization preserves
            assert_eq!(
                left_min.accepts(&term).expect("ground"),
                in_left,
                "min {instance}"
            );
            // De Morgan
            assert_eq!(
                not_union.accepts(&term).expect("ground"),
                inter_of_complements.accepts(&term).expect("ground"),
                "de-morgan {instance}"
            );
            assert_eq!(
                not_union.accepts(&term).expect("ground"),
                !(in_left || in_right)
            );
        }
        // Idempotent minimization and stable canonical bytes.
        let twice = left_min.minimized().expect("idempotent");
        assert_eq!(left_min, twice);
        assert_eq!(left_min.canonical_bytes(), twice.canonical_bytes());
        // Renaming invariance: permuting input state names must not
        // change canonical bytes.
        let mut renamed_states: Vec<TreeState> = left.states().to_vec();
        if renamed_states.len() > 1 {
            renamed_states.rotate_left(1);
        }
        let name_of: std::collections::BTreeMap<String, String> = left
            .states()
            .iter()
            .zip(renamed_states.iter())
            .map(|(from, to)| {
                (
                    from.name().as_str().to_string(),
                    to.name().as_str().to_string(),
                )
            })
            .collect();
        let renamed = automaton(
            left.alphabet().to_vec(),
            renamed_states,
            left.transitions()
                .iter()
                .map(|t| {
                    transition(
                        t.symbol().as_str(),
                        &t.children()
                            .iter()
                            .map(|c| name_of[c.name().as_str()].as_str())
                            .collect::<Vec<_>>(),
                        &name_of[t.parent().name().as_str()],
                    )
                })
                .collect(),
            left.finals()
                .iter()
                .map(|fim| state(&name_of[fim.name().as_str()]))
                .collect(),
        );
        let renamed_min = renamed
            .determinize(&TreeAutomatonLimits::default())
            .expect("det")
            .minimized()
            .expect("min");
        assert_eq!(
            left_min.canonical_bytes(),
            renamed_min.canonical_bytes(),
            "renaming invariance violated at {instance}"
        );
    }
}

// ---- P1/P2 review regressions (4571eec review)

/// P1: with binary symbols, one-hole context refinement must track
/// the correspondence between a concrete sibling and the resulting
/// parent block. `f(a,a)` and `f(b,b)` accepted but not `f(a,b)`.
#[test]
fn binary_contexts_distinguish_states() {
    let automaton = automaton(
        vec![ranked("a", 0), ranked("b", 0), ranked("f", 2)],
        states(&["p", "q", "r"]),
        vec![
            transition("a", &[], "p"),
            transition("b", &[], "q"),
            transition("f", &["p", "p"], "r"),
            transition("f", &["q", "q"], "r"),
        ],
        states(&["r"]),
    );
    let minimal = automaton.minimized().expect("minimized");
    // p and q are context-equivalent ONLY to themselves here: the
    // minimal automaton has three blocks ([p], [q], [r]).
    assert_eq!(minimal.states().len(), 3, "{minimal:?}");
    assert!(minimal
        .accepts(&Term::sym(
            "f",
            vec![Term::constant("a"), Term::constant("a")]
        ))
        .unwrap());
    assert!(minimal
        .accepts(&Term::sym(
            "f",
            vec![Term::constant("b"), Term::constant("b")]
        ))
        .unwrap());
    assert!(
        !minimal
            .accepts(&Term::sym(
                "f",
                vec![Term::constant("a"), Term::constant("b")]
            ))
            .unwrap(),
        "merged p and q: now accepts f(a,b) — language changed"
    );
    assert!(!minimal
        .accepts(&Term::sym(
            "f",
            vec![Term::constant("b"), Term::constant("a")]
        ))
        .unwrap());
}

/// P2a: canonical numbering must be a function of the language
/// (structure), not of input state-name spellings.
#[test]
fn canonical_numbering_is_renaming_invariant() {
    let alphabet = vec![ranked("a", 0), ranked("f", 1)];
    let original = automaton(
        alphabet.clone(),
        states(&["p", "q"]),
        vec![transition("a", &[], "p"), transition("f", &["p"], "q")],
        states(&["q"]),
    );
    let renamed = automaton(
        alphabet,
        states(&["z", "b"]),
        vec![transition("a", &[], "z"), transition("f", &["z"], "b")],
        states(&["b"]),
    );
    assert_eq!(
        original.minimized().expect("min").canonical_bytes(),
        renamed.minimized().expect("min").canonical_bytes(),
        "renaming input states changed canonical bytes"
    );
}

/// P2b: generated names reaching two digits must not break
/// idempotence through lexical re-ordering.
#[test]
fn minimize_idempotent_beyond_ten_states() {
    let alphabet = vec![ranked("a", 0), ranked("f", 1)];
    let names: Vec<String> = (0..12).map(|i| format!("q{i:02}")).collect();
    let mut transitions = vec![transition("a", &[], &names[0])];
    for i in 1..12 {
        transitions.push(transition("f", &[&names[i - 1]], &names[i]));
    }
    let name_refs: Vec<&str> = names.iter().map(String::as_str).collect();
    let source = automaton(
        alphabet,
        states(&name_refs),
        transitions,
        states(&[&names[11]]),
    );
    let once = source.minimized().expect("once");
    let twice = once.minimized().expect("twice");
    assert_eq!(once, twice, "minimization not idempotent past ten states");
    assert_eq!(once.canonical_bytes(), twice.canonical_bytes());
}
