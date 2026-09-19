// SPDX-License-Identifier: MIT OR Apache-2.0

//! RED tests for regular tree grammars (0.25 Cohort 4 Task 21):
//! checked production/start/rank validation, canonical storage,
//! fixed parse/render syntax, malformed/oversized input errors,
//! membership/witness through automaton conversion, and lossless
//! automaton <-> grammar round trips.

use amari_rewrite::language::{
    GrammarProduction, Nonterminal, RankedSymbol, RegularTreeGrammar, TreeAutomaton,
    TreeAutomatonLimits, TreeState, TreeTransition,
};
use amari_rewrite::trs::{Symbol, Term};
use amari_rewrite::RewriteError;

fn nt(name: &str) -> Nonterminal {
    Nonterminal::new(name)
}

fn production(lhs: &str, symbol: &str, arity: u16, children: &[&str]) -> GrammarProduction {
    GrammarProduction::new(
        nt(lhs),
        RankedSymbol::new(Symbol::new(symbol), arity),
        children.iter().map(|c| nt(c)).collect(),
    )
    .expect("well-formed production")
}

fn arithmetic_grammar() -> RegularTreeGrammar {
    // S -> add S S | mul S S | num ; generates arithmetic expressions
    RegularTreeGrammar::new(
        vec![nt("S")],
        vec![
            production("S", "add", 2, &["S", "S"]),
            production("S", "mul", 2, &["S", "S"]),
            production("S", "num", 0, &[]),
        ],
        vec![nt("S")],
        &TreeAutomatonLimits::default(),
    )
    .expect("valid grammar")
}

// ---- checked construction

#[test]
fn rank_mismatch_is_a_typed_error() {
    let result = GrammarProduction::new(
        nt("S"),
        RankedSymbol::new(Symbol::new("f"), 2),
        vec![nt("S")], // one child for a binary symbol
    );
    assert!(matches!(result, Err(RewriteError::MalformedGrammar { .. })));
}

#[test]
fn undeclared_lhs_child_and_start_are_typed_errors() {
    let limits = TreeAutomatonLimits::default();
    let undeclared_lhs = RegularTreeGrammar::new(
        vec![nt("S")],
        vec![production("T", "a", 0, &[])],
        vec![nt("S")],
        &limits,
    );
    assert!(matches!(
        undeclared_lhs,
        Err(RewriteError::MalformedGrammar { .. })
    ));
    let undeclared_child = RegularTreeGrammar::new(
        vec![nt("S")],
        vec![production("S", "f", 1, &["T"])],
        vec![nt("S")],
        &limits,
    );
    assert!(matches!(
        undeclared_child,
        Err(RewriteError::MalformedGrammar { .. })
    ));
    let undeclared_start = RegularTreeGrammar::new(
        vec![nt("S")],
        vec![production("S", "a", 0, &[])],
        vec![nt("T")],
        &limits,
    );
    assert!(matches!(
        undeclared_start,
        Err(RewriteError::MalformedGrammar { .. })
    ));
}

#[test]
fn nonterminal_names_must_be_renderable() {
    let limits = TreeAutomatonLimits::default();
    for bad in ["", "has space", "tab\there", "->"] {
        let result = RegularTreeGrammar::new(vec![nt(bad)], vec![], vec![], &limits);
        assert!(
            matches!(result, Err(RewriteError::MalformedGrammar { .. })),
            "name {bad:?} accepted"
        );
    }
}

#[test]
fn grammar_limits_are_enforced() {
    let tight = TreeAutomatonLimits::new(1, 8, 4).expect("tight");
    // Two nonterminals under a one-state ceiling.
    let too_many = RegularTreeGrammar::new(vec![nt("S"), nt("T")], vec![], vec![], &tight);
    assert!(matches!(too_many, Err(RewriteError::InvalidLimit { .. })));
    // Production arity above the rank ceiling.
    let too_wide = RegularTreeGrammar::new(
        vec![nt("S")],
        vec![production("S", "f", 5, &["S", "S", "S", "S", "S"])],
        vec![nt("S")],
        &TreeAutomatonLimits::new(8, 8, 4).expect("rank 4"),
    );
    assert!(matches!(too_wide, Err(RewriteError::InvalidLimit { .. })));
    // Too many productions.
    let too_many_productions = RegularTreeGrammar::new(
        vec![nt("S")],
        vec![production("S", "a", 0, &[]), production("S", "b", 0, &[])],
        vec![nt("S")],
        &TreeAutomatonLimits::new(4, 1, 4).expect("one transition"),
    );
    assert!(matches!(
        too_many_productions,
        Err(RewriteError::InvalidLimit { .. })
    ));
}

#[test]
fn storage_is_canonical_and_construction_order_independent() {
    let forward = RegularTreeGrammar::new(
        vec![nt("A"), nt("S")],
        vec![
            production("S", "f", 1, &["A"]),
            production("A", "a", 0, &[]),
            production("S", "g", 1, &["A"]),
        ],
        vec![nt("S")],
        &TreeAutomatonLimits::default(),
    )
    .expect("valid");
    let reversed = RegularTreeGrammar::new(
        vec![nt("S"), nt("A")],
        vec![
            production("S", "g", 1, &["A"]),
            production("A", "a", 0, &[]),
            production("S", "f", 1, &["A"]),
        ],
        vec![nt("S")],
        &TreeAutomatonLimits::default(),
    )
    .expect("valid");
    assert_eq!(forward, reversed);
    assert_eq!(forward.render(), reversed.render());
}

// ---- fixed parse/render syntax

const ARITHMETIC_TEXT: &str = "\
amari-tree-grammar/v1
nonterminals: S
start: S
S -> add S S
S -> mul S S
S -> num
";

#[test]
fn parse_render_round_trip_is_canonical() {
    let parsed =
        RegularTreeGrammar::parse(ARITHMETIC_TEXT, &TreeAutomatonLimits::default()).expect("parse");
    assert_eq!(parsed, arithmetic_grammar());
    assert_eq!(parsed.render(), ARITHMETIC_TEXT);
    // Reparsing the render is a fixed point.
    let reparsed = RegularTreeGrammar::parse(&parsed.render(), &TreeAutomatonLimits::default())
        .expect("reparse");
    assert_eq!(reparsed, parsed);
}

#[test]
fn parser_accepts_declaration_order_and_blank_lines() {
    let shuffled = "\
amari-tree-grammar/v1

start: S
S -> num
nonterminals: S
S -> mul S S

S -> add S S
";
    let parsed = RegularTreeGrammar::parse(shuffled, &TreeAutomatonLimits::default())
        .expect("parse shuffled");
    assert_eq!(parsed, arithmetic_grammar());
}

#[test]
fn malformed_inputs_are_typed_errors_with_line_numbers() {
    let limits = TreeAutomatonLimits::default();
    let cases: &[&str] = &[
        "",                                                   // empty
        "not-a-grammar\n",                                    // bad header
        "amari-tree-grammar/v1\nS a\n",                       // junk line
        "amari-tree-grammar/v1\nstart: T\n",                  // undeclared start
        "amari-tree-grammar/v1\nnonterminals: S\nS -> f T\n", // undeclared child
        "amari-tree-grammar/v1\nnonterminals: S\nS f S\n",    // missing arrow
    ];
    for case in cases {
        let result = RegularTreeGrammar::parse(case, &limits);
        assert!(
            matches!(result, Err(RewriteError::MalformedGrammar { .. })),
            "{case:?} accepted"
        );
        if let Err(RewriteError::MalformedGrammar { message }) = result {
            assert!(message.contains("line"), "no line number: {message}");
        }
    }
}

#[test]
fn oversized_parse_is_a_typed_error() {
    let limits = TreeAutomatonLimits::new(2, 4, 4).expect("tight");
    let text = "\
amari-tree-grammar/v1
nonterminals: S A B
start: S
S -> a
";
    let result = RegularTreeGrammar::parse(text, &limits);
    assert!(matches!(result, Err(RewriteError::InvalidLimit { .. })));
}

// ---- membership / witness through automaton conversion

#[test]
fn membership_and_witness_route_through_the_automaton() {
    let grammar = arithmetic_grammar();
    let num = Term::constant("num");
    let add = Term::sym("add", vec![num.clone(), num.clone()]);
    let mul_add = Term::sym("mul", vec![add.clone(), num.clone()]);
    assert!(grammar.accepts(&add).expect("ground"));
    assert!(grammar.accepts(&mul_add).expect("ground"));
    assert!(!grammar.accepts(&Term::constant("zero")).expect("ground"));
    // The smallest witness is `num`.
    assert_eq!(grammar.witness().expect("witness"), Some(num));
    // A grammar with an unproductive start has no witness.
    let empty = RegularTreeGrammar::new(
        vec![nt("S"), nt("T")],
        vec![production("S", "f", 1, &["T"])],
        vec![nt("S")],
        &TreeAutomatonLimits::default(),
    )
    .expect("valid");
    assert_eq!(empty.witness().expect("witness"), None);
    assert!(empty.language_is_empty().expect("emptiness"));
}

// ---- automaton <-> grammar conversions

#[test]
fn grammar_to_automaton_maps_starts_to_finals() {
    let automaton = arithmetic_grammar().to_automaton().expect("to automaton");
    assert_eq!(automaton.states().len(), 1);
    assert_eq!(automaton.finals(), &[TreeState::new(Symbol::new("S"))]);
    assert_eq!(automaton.transitions().len(), 3);
    let back = RegularTreeGrammar::from_automaton(&automaton).expect("from automaton");
    assert_eq!(back, arithmetic_grammar());
}

#[test]
fn automaton_round_trip_preserves_canonical_bytes() {
    let automaton = arithmetic_grammar().to_automaton().expect("automaton");
    let grammar = RegularTreeGrammar::from_automaton(&automaton).expect("grammar");
    let rebuilt = grammar.to_automaton().expect("automaton again");
    assert_eq!(automaton, rebuilt);
    assert_eq!(automaton.canonical_bytes(), rebuilt.canonical_bytes());
}

#[test]
fn from_automaton_honours_limits() {
    let automaton = TreeAutomaton::new(
        vec![
            RankedSymbol::new(Symbol::new("a"), 0),
            RankedSymbol::new(Symbol::new("f"), 1),
        ],
        vec![
            TreeState::new(Symbol::new("q0")),
            TreeState::new(Symbol::new("q1")),
        ],
        vec![
            TreeTransition::new(Symbol::new("a"), vec![], TreeState::new(Symbol::new("q0"))),
            TreeTransition::new(
                Symbol::new("f"),
                vec![TreeState::new(Symbol::new("q0"))],
                TreeState::new(Symbol::new("q1")),
            ),
        ],
        vec![TreeState::new(Symbol::new("q1"))],
        TreeAutomatonLimits::default(),
    )
    .expect("automaton");
    let grammar = RegularTreeGrammar::from_automaton(&automaton).expect("grammar");
    assert_eq!(grammar.starts(), &[nt("q1")]);
    // Round trip under tight limits fails as a typed error.
    let tight = TreeAutomatonLimits::new(1, 8, 4).expect("tight");
    let result = RegularTreeGrammar::from_automaton_with_limits(&automaton, &tight);
    assert!(matches!(result, Err(RewriteError::InvalidLimit { .. })));
}

// ---- property: conversion round trips preserve bytes and language

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

#[test]
fn automaton_grammar_round_trips_preserve_bytes_and_language() {
    let mut rng = Lcg(0x6A11_2026_0918_CA7A);
    let alphabet = vec![
        RankedSymbol::new(Symbol::new("a"), 0),
        RankedSymbol::new(Symbol::new("b"), 0),
        RankedSymbol::new(Symbol::new("f"), 1),
        RankedSymbol::new(Symbol::new("g"), 2),
    ];
    for instance in 0..80 {
        let names = ["q0", "q1", "q2"];
        let state_count = 1 + rng.below(3) as usize;
        let state_names = &names[..state_count];
        let states: Vec<TreeState> = state_names
            .iter()
            .map(|n| TreeState::new(Symbol::new(*n)))
            .collect();
        let mut transitions = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..(1 + rng.below(8)) {
            let ranked_symbol = &alphabet[rng.below(4) as usize];
            let children: Vec<TreeState> = (0..ranked_symbol.arity())
                .map(|_| states[rng.below(state_count as u64) as usize].clone())
                .collect();
            let parent = states[rng.below(state_count as u64) as usize].clone();
            let key = format!(
                "{}({})->{}",
                ranked_symbol.symbol(),
                children
                    .iter()
                    .map(|c| c.name().as_str())
                    .collect::<Vec<_>>()
                    .join(","),
                parent.name()
            );
            if seen.insert(key) {
                transitions.push(TreeTransition::new(
                    ranked_symbol.symbol().clone(),
                    children,
                    parent,
                ));
            }
        }
        let finals: Vec<TreeState> = state_names
            .iter()
            .filter(|_| rng.below(2) == 0)
            .map(|n| TreeState::new(Symbol::new(*n)))
            .collect();
        let automaton = TreeAutomaton::new(
            alphabet.clone(),
            states,
            transitions,
            finals,
            TreeAutomatonLimits::default(),
        )
        .expect("automaton");
        // Automaton -> grammar -> automaton: byte-identical after
        // the documented alphabet normalization (unused declared
        // symbols are dropped by the grammar conversion).
        let used: Vec<RankedSymbol> = {
            let mut used: Vec<RankedSymbol> = automaton
                .transitions()
                .iter()
                .map(|t| RankedSymbol::new(t.symbol().clone(), t.children().len() as u16))
                .collect();
            used.sort();
            used.dedup();
            used
        };
        let normalized = TreeAutomaton::new(
            used,
            automaton.states().to_vec(),
            automaton.transitions().to_vec(),
            automaton.finals().to_vec(),
            TreeAutomatonLimits::default(),
        )
        .expect("normalized");
        let grammar = RegularTreeGrammar::from_automaton(&automaton).expect("grammar");
        let rebuilt = grammar.to_automaton().expect("rebuilt");
        assert_eq!(normalized, rebuilt, "automaton mismatch at {instance}");
        assert_eq!(
            normalized.canonical_bytes(),
            rebuilt.canonical_bytes(),
            "bytes at {instance}"
        );
        // Grammar -> parse(render(grammar)) -> same grammar.
        let rendered = grammar.render();
        let reparsed =
            RegularTreeGrammar::parse(&rendered, &TreeAutomatonLimits::default()).expect("reparse");
        assert_eq!(grammar, reparsed, "render round trip at {instance}");
        // Membership parity on random terms.
        for _ in 0..20 {
            let mut depth_rng = Lcg(rng.next());
            let term = random_term(&mut depth_rng, &alphabet, 3);
            assert_eq!(
                automaton.accepts(&term).expect("ground"),
                grammar.accepts(&term).expect("ground"),
                "membership at {instance}"
            );
        }
    }
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

// ---- P1/P2 review regressions (18809bd review)

/// P1: parsing must enforce ceilings incrementally — two million
/// distinct productions under a one-production limit must be a fast
/// typed error, not an unbounded allocation.
#[test]
fn parse_enforces_limits_before_buffering() {
    let limits = TreeAutomatonLimits::new(1, 1, 4).expect("limits");
    let mut text = String::from("amari-tree-grammar/v1\nnonterminals: S\nstart: S\n");
    for i in 0..2_000_000u32 {
        text.push_str(&format!("S -> a{i}\n"));
    }
    let started = std::time::Instant::now();
    let result = RegularTreeGrammar::parse(&text, &limits);
    assert!(
        matches!(result, Err(RewriteError::InvalidLimit { .. })),
        "expected InvalidLimit, got {:?}",
        result.map(|g| g.productions().len())
    );
    assert!(
        started.elapsed().as_secs() < 10,
        "not rejected early: {:?}",
        started.elapsed()
    );
}

/// P1: duplicate production lines are typed errors as they are read
/// (memory stays bounded even for repeated identical lines).
#[test]
fn duplicate_production_lines_are_typed_errors() {
    let mut text = String::from("amari-tree-grammar/v1\nnonterminals: S\nstart: S\n");
    for _ in 0..100_000 {
        text.push_str("S -> a\n");
    }
    let result = RegularTreeGrammar::parse(&text, &TreeAutomatonLimits::default());
    assert!(matches!(result, Err(RewriteError::MalformedGrammar { .. })));
    // And the constructor rejects duplicate productions too.
    let dup = RegularTreeGrammar::new(
        vec![nt("S")],
        vec![production("S", "a", 0, &[]), production("S", "a", 0, &[])],
        vec![nt("S")],
        &TreeAutomatonLimits::default(),
    );
    assert!(matches!(dup, Err(RewriteError::MalformedGrammar { .. })));
}

/// P1: tokenization is bounded by the rank ceiling — a production
/// line with absurdly many children errors without collecting it.
#[test]
fn tokenization_is_bounded_by_rank_ceiling() {
    let limits = TreeAutomatonLimits::default(); // rank ceiling 16
    let mut line = String::from("amari-tree-grammar/v1\nnonterminals: S\nstart: S\nS -> f");
    for _ in 0..1_000_000 {
        line.push_str(" S");
    }
    line.push('\n');
    let started = std::time::Instant::now();
    let result = RegularTreeGrammar::parse(&line, &limits);
    assert!(matches!(result, Err(RewriteError::InvalidLimit { .. })));
    assert!(started.elapsed().as_secs() < 10);
}

/// P2a: terminal symbol names must be renderable for a lossless
/// render/parse round trip.
#[test]
fn unrenderable_terminal_symbols_are_rejected() {
    for bad_symbol in ["", "a S", "line\nbreak", "->"] {
        // Rejected at production construction...
        let production = GrammarProduction::new(
            nt("S"),
            RankedSymbol::new(Symbol::new(bad_symbol), 0),
            vec![],
        );
        assert!(
            matches!(production, Err(RewriteError::MalformedGrammar { .. })),
            "symbol {bad_symbol:?} accepted"
        );
    }
}

/// P2b: directive-prefixed nonterminal names break the parse
/// dispatch and must be rejected at construction — including via
/// automaton conversion.
#[test]
fn directive_prefixed_nonterminals_are_rejected() {
    for bad in ["start:S", "nonterminals:S", "start:", "nonterminals:"] {
        let result = RegularTreeGrammar::new(
            vec![nt(bad)],
            vec![production(bad, "a", 0, &[])],
            vec![nt(bad)],
            &TreeAutomatonLimits::default(),
        );
        assert!(
            matches!(result, Err(RewriteError::MalformedGrammar { .. })),
            "name {bad:?} accepted"
        );
    }
    // The same rule applies through automaton conversion.
    let automaton = TreeAutomaton::new(
        vec![RankedSymbol::new(Symbol::new("a"), 0)],
        vec![TreeState::new(Symbol::new("start:S"))],
        vec![TreeTransition::new(
            Symbol::new("a"),
            vec![],
            TreeState::new(Symbol::new("start:S")),
        )],
        vec![TreeState::new(Symbol::new("start:S"))],
        TreeAutomatonLimits::default(),
    )
    .expect("automaton");
    assert!(matches!(
        RegularTreeGrammar::from_automaton(&automaton),
        Err(RewriteError::MalformedGrammar { .. })
    ));
}

/// P2c: a symbol name has ONE rank per grammar. Mixed-rank use is a
/// typed grammar error at construction/parse, not a surprise
/// automaton rank-conflict at first use.
#[test]
fn rank_conflicts_are_typed_grammar_errors() {
    let conflict = RegularTreeGrammar::new(
        vec![nt("S")],
        vec![
            production("S", "f", 0, &[]),
            production("S", "f", 1, &["S"]),
        ],
        vec![nt("S")],
        &TreeAutomatonLimits::default(),
    );
    assert!(matches!(
        conflict,
        Err(RewriteError::MalformedGrammar { .. })
    ));
    let parsed = RegularTreeGrammar::parse(
        "amari-tree-grammar/v1\nnonterminals: S\nstart: S\nS -> f\nS -> f S\n",
        &TreeAutomatonLimits::default(),
    );
    assert!(matches!(parsed, Err(RewriteError::MalformedGrammar { .. })));
}
