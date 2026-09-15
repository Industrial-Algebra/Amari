//! Transparent guidance mode contract tests (0.25 Cohort 3, Task 16).

use amari_rewrite::inverse::{
    BackwardExplorer, BackwardSearchOutcome, GuidanceMode, InverseSearchConfig, SearchMode,
    SymbolicScore,
};
use amari_rewrite::trs::{Rule, Term, TermSystem};

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

#[test]
fn symbolic_score_dimensions_order_lexicographically() {
    let base = SymbolicScore {
        depth: 1,
        term_growth: 0,
        constraint_growth: 0,
        unresolved_existentials: 0,
        residual_cost: 0,
        branch_factor: 1,
        cycle: false,
        rule_priority: 0,
    };
    // Depth dominates every later dimension.
    let deeper = SymbolicScore {
        depth: 2,
        ..base.clone()
    };
    assert!(base < deeper);
    // Term growth breaks depth ties BEFORE existentials are
    // considered: higher growth loses no matter the existentials.
    let growing = SymbolicScore {
        term_growth: 3,
        ..base.clone()
    };
    let existential = SymbolicScore {
        unresolved_existentials: 9,
        ..base.clone()
    };
    assert!(existential < growing);
    // Novel states sort before cyclic ones.
    let cyclic = SymbolicScore {
        cycle: true,
        ..base.clone()
    };
    assert!(base < cyclic);
    // Rule priority is the final tiebreak: total and deterministic.
    let later_rule = SymbolicScore {
        rule_priority: 1,
        ..base.clone()
    };
    assert!(base < later_rule);
    assert_eq!(base, base.clone());
}

#[test]
fn complete_guidance_preserves_candidate_set() {
    // Constant cycle a -> b -> c -> a: the backward closure is finite,
    // so guided and unguided searches must explore the SAME state set
    // (pinned by identical exhaustion evidence) and agree on outcome.
    let rules = system(vec![
        Rule::new(parse("a"), parse("b")).unwrap(),
        Rule::new(parse("b"), parse("c")).unwrap(),
        Rule::new(parse("c"), parse("a")).unwrap(),
    ]);
    let unguided = BackwardExplorer::new(&rules, small_config())
        .search(&parse("a"), &parse("z"), SearchMode::BreadthFirst)
        .unwrap();
    let guided = BackwardExplorer::new(&rules, small_config())
        .search_with_guidance(
            &parse("a"),
            &parse("z"),
            SearchMode::BreadthFirst,
            &GuidanceMode::CompleteWithinLimits,
        )
        .unwrap();
    let BackwardSearchOutcome::Exhausted(plain) = &unguided else {
        panic!("unguided should exhaust: {unguided:?}");
    };
    let BackwardSearchOutcome::Exhausted(complete) = &guided else {
        panic!("guided complete must still exhaust: {guided:?}");
    };
    assert_eq!(plain.evidence_hash(), complete.evidence_hash());
}

#[test]
fn beam_pruning_reports_dropped_counts_and_hashes() {
    // Rules b => x and c => x: backward from `x` has TWO candidates.
    // Beam width 1 keeps one and must honestly report the other as
    // dropped — and since `c` is never explored, the outcome can
    // never be Exhausted.
    let rules = system(vec![
        Rule::new(parse("b"), parse("x")).unwrap(),
        Rule::new(parse("c"), parse("x")).unwrap(),
    ]);
    let outcome = BackwardExplorer::new(&rules, small_config())
        .search_with_guidance(
            &parse("x"),
            &parse("z"),
            SearchMode::BreadthFirst,
            &GuidanceMode::HeuristicPruning { beam_width: 1 },
        )
        .unwrap();
    let BackwardSearchOutcome::Approximate(evidence) = &outcome else {
        panic!("pruned search must be Approximate, got {outcome:?}");
    };
    assert_eq!(evidence.dropped_candidates, 1);
    assert_eq!(
        evidence.config_hash,
        amari_rewrite::inverse::config_hash(&small_config())
    );
    assert_eq!(
        evidence.guidance_hash,
        amari_rewrite::inverse::guidance_hash(&GuidanceMode::HeuristicPruning { beam_width: 1 },)
    );
    assert_eq!(evidence.scorer_hash.as_bytes().len(), 32);
}

#[test]
fn pruned_search_never_certifies_exhaustion() {
    // Same system: unguided closes and certifies exhaustion...
    let rules = system(vec![
        Rule::new(parse("b"), parse("x")).unwrap(),
        Rule::new(parse("c"), parse("x")).unwrap(),
    ]);
    let unguided = BackwardExplorer::new(&rules, small_config())
        .search(&parse("x"), &parse("z"), SearchMode::BreadthFirst)
        .unwrap();
    assert!(matches!(unguided, BackwardSearchOutcome::Exhausted(_)));
    // ...but the beam-1 guided search explores strictly less and must
    // return Approximate, never Exhausted.
    let guided = BackwardExplorer::new(&rules, small_config())
        .search_with_guidance(
            &parse("x"),
            &parse("z"),
            SearchMode::BreadthFirst,
            &GuidanceMode::HeuristicPruning { beam_width: 1 },
        )
        .unwrap();
    assert!(!matches!(guided, BackwardSearchOutcome::Exhausted(_)));
    assert!(matches!(guided, BackwardSearchOutcome::Approximate(_)));
}

#[test]
fn guidance_cannot_bypass_ceilings() {
    let rules = system(vec![Rule::new(parse("add(X, zero)"), parse("X")).unwrap()]);
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
        .search_with_guidance(
            &parse("a"),
            &parse("z"),
            SearchMode::BreadthFirst,
            &GuidanceMode::HeuristicPruning { beam_width: 1_000 },
        )
        .unwrap();
    // The operation ceiling fires before any pruning: Partial, and
    // definitely not a witness or exhaustion.
    assert!(matches!(outcome, BackwardSearchOutcome::Partial(_)));
}

#[cfg(feature = "serialize")]
#[test]
fn guidance_modes_roundtrip() {
    let mode = GuidanceMode::HeuristicPruning { beam_width: 7 };
    let json = serde_json::to_string(&mode).unwrap();
    let back: GuidanceMode = serde_json::from_str(&json).unwrap();
    assert_eq!(mode, back);
    let complete = GuidanceMode::CompleteWithinLimits;
    let json = serde_json::to_string(&complete).unwrap();
    let back: GuidanceMode = serde_json::from_str(&json).unwrap();
    assert_eq!(complete, back);
}
