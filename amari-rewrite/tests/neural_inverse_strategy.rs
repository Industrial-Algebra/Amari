//! Neural inverse guidance contracts (0.25 Cohort 8, Task 44).
//!
//! One test per enumerated rule of the task:
//!
//! - **ordering only** — complete mode never drops a candidate: the explored
//!   set (and therefore the replay/exhaustion hash) is identical with and
//!   without the neural adapter, while the visit order may differ (pinned by
//!   a witness differential);
//! - **preferred valid predecessor** — a ranker that scores a candidate
//!   better puts it first, and the search actually follows that order;
//! - **ties** — equal scores resolve by the Task 40 canonical content key, so
//!   the ranking is a function of content, not of request order;
//! - **model failure** — an encoder/ranker failure mid-search is a typed,
//!   sanitized, metered outcome: never a panic and never a silent reorder;
//! - **beam/top-k drops** — heuristic pruning records dropped counts;
//! - **approximate authority** — a pruned run cannot construct exhaustion
//!   (the Task 34 tri-state doctrine);
//! - **replay hashes** — the adapter's identity enters replay identity
//!   through the scorer hash, and identical inputs replay identically.
//!
//! Plus the Task 40 schema binding: a declared [`FeatureSchemaV1`] digest that
//! does not match the frozen schema is a typed rejection at construction.
//!
//! Fixtures are otherwise minimal: the symbolic engine is the only
//! transition generator, so every candidate here is an engine-produced
//! [`SymbolicPredecessor`] and the tests may only reorder it.

#![cfg(feature = "neural")]

use amari_rewrite::inverse::{
    config_hash, guidance_hash, score_candidate, scorer_hash, scorer_hash_with,
    symbolic_predecessors, BackwardExplorer, BackwardSearchOutcome, GuidanceCandidate,
    GuidanceFailureKind, GuidanceMode, GuidanceRequest, GuidanceStrategy, GuidedSearchOutcome,
    InverseSearchConfig, ReplayCertificate, ReplayDerivation, ReplayQuery, ResourceObservation,
    SearchMode, SymbolicPredecessor, SymbolicScore, SymbolicState,
};
use amari_rewrite::neural::{
    FeatureEncoder, FeatureSchemaV1, MlpRanker, NeuralStrategy, NeuralStrategyConfig, RankerConfig,
    RankerWeights, StrategyError, MAX_RANKED_CANDIDATES,
};
use amari_rewrite::relation::{ConstraintSet, RelationLimits, Sha256Digest};
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

/// A validated ranker spec with the given hidden widths.
fn spec(hidden: &[u32]) -> RankerConfig {
    let hidden = hidden
        .iter()
        .map(|width| format!("{{\"width\":{width}}}"))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        "{{\"model_type\":\"ia-mlp\",\"schema_version\":1,\"input_width\":64,\
         \"hidden\":[{hidden}],\"head\":{{\"width\":1}},\
         \"hidden_activation\":\"relu\"}}"
    );
    RankerConfig::from_spec_json(&json).expect("valid spec")
}

/// Build a one-hidden-unit ranker from explicit tensors.
fn ranker(tensors: Vec<(&str, Vec<f32>)>) -> MlpRanker {
    let config = spec(&[1]);
    let borrowed: Vec<(&str, &[f32])> = tensors
        .iter()
        .map(|(name, values)| (*name, values.as_slice()))
        .collect();
    let weights = RankerWeights::from_tensors(&config, &borrowed).expect("admissible weights");
    MlpRanker::new(config, weights).expect("matching weights")
}

/// A ranker whose score is `relu(1 - lane0)`, where lane 0 is the bit-bucketed
/// term-node count. Lower is better (Task 40's contract), so the *larger* term
/// scores lower — this ranker prefers bigger terms.
fn prefers_larger_terms() -> MlpRanker {
    let mut hidden_weight = vec![0.0f32; 64];
    hidden_weight[0] = -1.0;
    ranker(vec![
        ("hidden.0.weight", hidden_weight),
        ("hidden.0.bias", vec![1.0]),
        ("head.weight", vec![1.0]),
        ("head.bias", vec![0.0]),
    ])
}

/// A ranker with zero weights: every candidate scores `0.0`, so every round is
/// a pure tie resolved by the canonical content key.
fn constant_ranker() -> MlpRanker {
    ranker(vec![
        ("hidden.0.weight", vec![0.0f32; 64]),
        ("hidden.0.bias", vec![0.0f32]),
        ("head.weight", vec![0.0f32]),
        ("head.bias", vec![0.0f32]),
    ])
}

fn encoder() -> FeatureEncoder {
    FeatureEncoder::new(RelationLimits::default())
}

/// The default (frozen-schema-declaring, default-bounded) strategy.
fn strategy(ranker: MlpRanker) -> NeuralStrategy {
    NeuralStrategy::from_ranker(ranker, encoder())
}

fn guided(
    outcome: Result<GuidedSearchOutcome, amari_rewrite::RewriteError>,
) -> GuidedSearchOutcome {
    outcome.expect("search must not return a hard error")
}

fn completed(outcome: GuidedSearchOutcome) -> BackwardSearchOutcome {
    match outcome {
        GuidedSearchOutcome::Completed(outcome) => outcome,
        GuidedSearchOutcome::StrategyFailed(failure) => {
            panic!("unexpected strategy failure: {failure:?}")
        }
    }
}

fn approximate(
    outcome: &BackwardSearchOutcome,
) -> &amari_rewrite::inverse::ApproximateSearchEvidence {
    match outcome {
        BackwardSearchOutcome::Approximate(evidence) => evidence,
        other => panic!("expected an approximate outcome, got {other:?}"),
    }
}

/// `a -> b -> c -> a`: a finite backward closure, so complete-mode guidance
/// must explore exactly the same state set as unguided search.
fn cycle_system() -> TermSystem {
    system(vec![
        Rule::new(parse("a"), parse("b")).unwrap(),
        Rule::new(parse("b"), parse("c")).unwrap(),
        Rule::new(parse("c"), parse("a")).unwrap(),
    ])
}

/// `b => x`, `f(c) => x`: backward from `x` has exactly two valid candidates,
/// one of one node and one of two.
fn two_candidate_system() -> TermSystem {
    system(vec![
        Rule::new(parse("b"), parse("x")).unwrap(),
        Rule::new(parse("f(c)"), parse("x")).unwrap(),
    ])
}

/// `b => x`, `f(c) => x`, `d => b`, `d => f(c)`: both candidates reach the
/// goal, so the visit order decides which (equally valid) witness is returned.
fn two_witness_system() -> TermSystem {
    system(vec![
        Rule::new(parse("b"), parse("x")).unwrap(),
        Rule::new(parse("f(c)"), parse("x")).unwrap(),
        Rule::new(parse("d"), parse("b")).unwrap(),
        Rule::new(parse("d"), parse("f(c)")).unwrap(),
    ])
}

/// One expansion round's engine candidates, their canonical states, and their
/// transparent symbolic scores — in the engine's own order.
struct Round {
    parent: SymbolicState,
    states: Vec<SymbolicState>,
    predecessors: Vec<SymbolicPredecessor>,
    scores: Vec<SymbolicScore>,
}

impl Round {
    fn of(system: &TermSystem, target: &Term) -> Self {
        let limits = RelationLimits::default();
        let predecessors =
            symbolic_predecessors(system, target, &limits, 0).expect("the engine round");
        let parent = SymbolicState::new(target.clone(), ConstraintSet::new());
        let states: Vec<SymbolicState> = predecessors
            .iter()
            .map(|predecessor| {
                SymbolicState::new(predecessor.term.clone(), predecessor.constraints.clone())
            })
            .collect();
        let branch_factor = predecessors.len() as u64;
        let scores: Vec<SymbolicScore> = predecessors
            .iter()
            .zip(&states)
            .map(|(predecessor, state)| {
                score_candidate(&parent, state, predecessor, 1, branch_factor, false, 0)
            })
            .collect();
        Self {
            parent,
            states,
            predecessors,
            scores,
        }
    }

    fn candidates(&self) -> Vec<GuidanceCandidate<'_>> {
        self.states
            .iter()
            .zip(&self.predecessors)
            .zip(&self.scores)
            .map(|((state, predecessor), score)| GuidanceCandidate {
                state,
                predecessor,
                score,
            })
            .collect()
    }

    fn request<'a>(
        &'a self,
        candidates: &'a [GuidanceCandidate<'a>],
        mode: &'a GuidanceMode,
    ) -> GuidanceRequest<'a> {
        GuidanceRequest {
            parent: &self.parent,
            depth: 0,
            mode,
            candidates,
        }
    }
}

/// Ordering only: complete mode never drops a candidate, so the explored set —
/// and therefore the certified exhaustion evidence — is identical with and
/// without the neural adapter.
#[test]
fn complete_mode_preserves_the_candidate_set() {
    let system = cycle_system();
    let baseline = BackwardExplorer::new(&system, small_config())
        .search(&parse("a"), &parse("z"), SearchMode::BreadthFirst)
        .unwrap();
    let adapter = strategy(constant_ranker());
    let guided_outcome = completed(guided(
        BackwardExplorer::new(&system, small_config()).search_with_strategy(
            &parse("a"),
            &parse("z"),
            SearchMode::BreadthFirst,
            &GuidanceMode::CompleteWithinLimits,
            Some(&adapter),
        ),
    ));
    let BackwardSearchOutcome::Exhausted(plain) = &baseline else {
        panic!("baseline must exhaust: {baseline:?}");
    };
    let BackwardSearchOutcome::Exhausted(neural) = &guided_outcome else {
        panic!("neural complete mode must still exhaust: {guided_outcome:?}");
    };
    // Identical evidence hash == identical explored set == identical replay
    // identity: ordering changed nothing about authority.
    assert_eq!(plain.evidence_hash(), neural.evidence_hash());
    assert_eq!(plain.authority(), neural.authority());
}

/// Ordering only, the observable half: complete mode may return a different
/// (equally valid) witness because the visit order changed.
#[test]
fn complete_mode_ordering_steers_without_dropping() {
    let system = two_witness_system();
    let baseline = BackwardExplorer::new(&system, small_config())
        .search(&parse("x"), &parse("d"), SearchMode::BreadthFirst)
        .unwrap();
    let adapter = strategy(prefers_larger_terms());
    let guided_outcome = completed(guided(
        BackwardExplorer::new(&system, small_config()).search_with_strategy(
            &parse("x"),
            &parse("d"),
            SearchMode::BreadthFirst,
            &GuidanceMode::CompleteWithinLimits,
            Some(&adapter),
        ),
    ));
    let BackwardSearchOutcome::Witness(baseline) = baseline else {
        panic!("baseline must witness");
    };
    let BackwardSearchOutcome::Witness(neural) = guided_outcome else {
        panic!("neural complete mode must witness");
    };
    // The symbolic order takes the smallest-growth candidate first.
    assert_eq!(baseline.steps.first().unwrap().term, parse("b"));
    // The neural order takes the candidate the ranker scored better first.
    assert_eq!(neural.steps.first().unwrap().term, parse("f(c)"));
    // Steering never changes what a derivation IS: the neural witness replays
    // through the original system and verifies as a certificate under the same
    // guidance-mode identity as the symbolic one.
    let query = ReplayQuery::Backward {
        target: parse("x"),
        goal: parse("d"),
    };
    let resources = ResourceObservation {
        states: 3,
        transitions: 2,
        retained_bytes: 256,
        operations: 8,
    };
    let certificate = ReplayCertificate::issue(
        &system,
        query,
        ReplayDerivation::Backward(neural),
        small_config(),
        GuidanceMode::CompleteWithinLimits,
        resources,
    );
    certificate
        .verify(&system)
        .expect("a neural-ordered derivation must replay and verify");
    assert_eq!(
        certificate.guidance_hash,
        guidance_hash(&GuidanceMode::CompleteWithinLimits)
    );
}

/// The adapter's ranking really prefers what the ranker scored better: the
/// engine's own candidate order is `[b, f(c)]`, and the neural order is
/// `[f(c), b]`.
#[test]
fn preferred_valid_predecessor_ranks_first() {
    let system = two_candidate_system();
    let target = parse("x");
    let round = Round::of(&system, &target);
    let mode = GuidanceMode::CompleteWithinLimits;
    let adapter = strategy(prefers_larger_terms());
    // The symbolic baseline orders by the transparent score (smaller growth
    // first, i.e. `b` at index 0).
    assert_eq!(round.states[0].term(), &parse("b"));
    assert_eq!(round.states[1].term(), &parse("f(c)"));
    let candidates = round.candidates();
    let request = round.request(&candidates, &mode);
    let order = adapter.rank(&request).expect("the round is admissible");
    assert_eq!(order, vec![1, 0]);
    assert_eq!(round.states[order[0]].term(), &parse("f(c)"));
}

/// Equal scores resolve by the Task 40 canonical content key, so the ranking
/// is a function of the candidates' content and never of request order.
#[test]
fn ties_resolve_by_the_canonical_content_order() {
    let system = system(vec![
        Rule::new(parse("b"), parse("x")).unwrap(),
        Rule::new(parse("c"), parse("x")).unwrap(),
    ]);
    let target = parse("x");
    let round = Round::of(&system, &target);
    assert_eq!(round.states.len(), 2);
    let digest_b = SymbolicState::new(parse("b"), ConstraintSet::new()).canonical_digest();
    let digest_c = SymbolicState::new(parse("c"), ConstraintSet::new()).canonical_digest();
    // Ascending state digest decides the tie; `b`/`c` differ only by content.
    let first = usize::from(digest_c < digest_b);
    let mode = GuidanceMode::CompleteWithinLimits;
    let adapter = strategy(constant_ranker());
    let candidates = round.candidates();
    let request = round.request(&candidates, &mode);
    let order = adapter.rank(&request).expect("the round is admissible");
    assert_eq!(order, vec![first, 1 - first]);
    // Repeating the request is bit-identical (the ranking is deterministic).
    assert_eq!(adapter.rank(&request).expect("admissible"), order);
    // Permuting the request cannot change the identity order: the same
    // candidate is first either way.
    let swapped = Round {
        parent: SymbolicState::new(target.clone(), ConstraintSet::new()),
        states: round.states.iter().rev().cloned().collect(),
        predecessors: round.predecessors.iter().rev().cloned().collect(),
        scores: round.scores.iter().rev().cloned().collect(),
    };
    let swapped_candidates = swapped.candidates();
    let swapped_request = swapped.request(&swapped_candidates, &mode);
    let swapped_order = adapter
        .rank(&swapped_request)
        .expect("the permuted round is admissible");
    assert_eq!(
        swapped.states[swapped_order[0]].canonical_digest(),
        round.states[order[0]].canonical_digest()
    );
}

/// Heuristic mode prunes by beam after the neural order and records the
/// dropped count.
#[test]
fn heuristic_mode_records_beam_drops() {
    let system = two_candidate_system();
    let adapter = strategy(prefers_larger_terms());
    let mode = GuidanceMode::HeuristicPruning { beam_width: 1 };
    let outcome = completed(guided(
        BackwardExplorer::new(&system, small_config()).search_with_strategy(
            &parse("x"),
            &parse("z"),
            SearchMode::BreadthFirst,
            &mode,
            Some(&adapter),
        ),
    ));
    let evidence = approximate(&outcome);
    assert_eq!(evidence.dropped_candidates, 1);
    assert_eq!(evidence.guidance_hash, guidance_hash(&mode));
    assert_eq!(evidence.config_hash, config_hash(&small_config()));
}

/// The neural order decides which candidate survives the beam. With a beam of
/// one the symbolic order keeps the dead-end candidate and finds nothing,
/// while the neural order keeps the candidate that reaches the goal.
#[test]
fn heuristic_mode_ordering_decides_what_survives_the_beam() {
    let system = system(vec![
        Rule::new(parse("b"), parse("x")).unwrap(),
        Rule::new(parse("f(c)"), parse("x")).unwrap(),
        Rule::new(parse("d"), parse("f(c)")).unwrap(),
    ]);
    let mode = GuidanceMode::HeuristicPruning { beam_width: 1 };
    let baseline = BackwardExplorer::new(&system, small_config())
        .search_with_guidance(&parse("x"), &parse("d"), SearchMode::BreadthFirst, &mode)
        .unwrap();
    assert!(
        matches!(baseline, BackwardSearchOutcome::Approximate(_)),
        "the symbolic beam keeps the dead end: {baseline:?}"
    );
    let adapter = strategy(prefers_larger_terms());
    let guided_outcome = completed(guided(
        BackwardExplorer::new(&system, small_config()).search_with_strategy(
            &parse("x"),
            &parse("d"),
            SearchMode::BreadthFirst,
            &mode,
            Some(&adapter),
        ),
    ));
    let BackwardSearchOutcome::Witness(derivation) = guided_outcome else {
        panic!("the neural beam keeps the route to the goal: {guided_outcome:?}");
    };
    assert_eq!(derivation.steps.first().unwrap().term, parse("f(c)"));
}

/// A pruned run cannot claim exhaustion: the same system certifies exhaustion
/// unguided, and the pruned neural run is `Approximate` with the drop marked.
#[test]
fn pruned_search_cannot_claim_exhaustion() {
    let system = two_candidate_system();
    let unguided = BackwardExplorer::new(&system, small_config())
        .search(&parse("x"), &parse("z"), SearchMode::BreadthFirst)
        .unwrap();
    assert!(matches!(unguided, BackwardSearchOutcome::Exhausted(_)));
    let adapter = strategy(prefers_larger_terms());
    let mode = GuidanceMode::HeuristicPruning { beam_width: 1 };
    let outcome = completed(guided(
        BackwardExplorer::new(&system, small_config()).search_with_strategy(
            &parse("x"),
            &parse("z"),
            SearchMode::BreadthFirst,
            &mode,
            Some(&adapter),
        ),
    ));
    assert!(!matches!(outcome, BackwardSearchOutcome::Exhausted(_)));
    let evidence = approximate(&outcome);
    assert_eq!(evidence.dropped_candidates, 1);
    assert!(
        evidence.summary.contains("dropped"),
        "the pruning marker must be explicit: {}",
        evidence.summary
    );
}

/// A ranker/encoder failure mid-search is a typed, sanitized, metered outcome:
/// no panic, no silent reordering.
#[test]
fn model_failure_mid_search_is_a_typed_outcome() {
    let system = two_candidate_system();
    // The search's own ceilings admit both candidates; the strategy's encoder
    // is the tighter gate, so the failure happens INSIDE guidance.
    let tight = FeatureEncoder::new(RelationLimits::new(1, 64, 4_096, 1_000_000).unwrap());
    let adapter = NeuralStrategy::from_ranker(constant_ranker(), tight);
    let outcome = guided(
        BackwardExplorer::new(&system, small_config()).search_with_strategy(
            &parse("x"),
            &parse("z"),
            SearchMode::BreadthFirst,
            &GuidanceMode::CompleteWithinLimits,
            Some(&adapter),
        ),
    );
    let GuidedSearchOutcome::StrategyFailed(failure) = outcome else {
        panic!("a mid-search scoring failure must be a typed outcome: {outcome:?}");
    };
    assert_eq!(
        failure.kind,
        GuidanceFailureKind::LimitExceeded {
            resource: "term nodes",
            value: 2,
            limit: 1,
        }
    );
    // Metered: two candidates in the round, one scored before the failure.
    assert_eq!(failure.candidates, 2);
    assert_eq!(failure.scored, 1);
    // Sanitized: the rendered failure names the stage and the meter only.
    let rendered = failure.to_string();
    assert!(
        !rendered.contains('/'),
        "no paths in a diagnostic: {rendered}"
    );
}

/// The candidate set is admitted against the strategy's own ceiling BEFORE
/// anything is encoded or scored.
#[test]
fn bounded_admission_rejects_an_oversized_candidate_set() {
    let system = two_candidate_system();
    let config = NeuralStrategyConfig::new(1).expect("one candidate is a valid ceiling");
    let adapter = NeuralStrategy::new(
        constant_ranker(),
        encoder(),
        FeatureSchemaV1.digest(),
        config,
    )
    .expect("declared schema matches");
    let outcome = guided(
        BackwardExplorer::new(&system, small_config()).search_with_strategy(
            &parse("x"),
            &parse("z"),
            SearchMode::BreadthFirst,
            &GuidanceMode::CompleteWithinLimits,
            Some(&adapter),
        ),
    );
    let GuidedSearchOutcome::StrategyFailed(failure) = outcome else {
        panic!("an over-wide round must be rejected: {outcome:?}");
    };
    assert_eq!(
        failure.kind,
        GuidanceFailureKind::LimitExceeded {
            resource: "neural strategy candidates",
            value: 2,
            limit: 1,
        }
    );
    assert_eq!(failure.scored, 0, "nothing may be scored before admission");
    // The fixed ceiling cannot be raised, and zero is not a ceiling.
    assert!(NeuralStrategyConfig::new(0).is_err());
    assert!(NeuralStrategyConfig::new(MAX_RANKED_CANDIDATES + 1).is_err());
}

/// A declared feature-schema digest that does not match the frozen schema is a
/// typed rejection at construction, never a silent reinterpretation.
#[test]
fn feature_schema_mismatch_is_a_typed_rejection() {
    let drifted = Sha256Digest::framed("amari.neural.feature-schema/v1", b"drifted");
    let error = NeuralStrategy::new(
        constant_ranker(),
        encoder(),
        drifted,
        NeuralStrategyConfig::default(),
    )
    .expect_err("a drifted schema declaration must be rejected");
    assert_eq!(
        error,
        StrategyError::FeatureSchemaMismatch {
            expected: FeatureSchemaV1.digest(),
            recorded: drifted,
        }
    );
    // The accepted strategy records the frozen digest, which is exactly what a
    // Task 43 checkpoint binds its weights to.
    let accepted = strategy(constant_ranker());
    assert_eq!(accepted.schema_digest(), FeatureSchemaV1.digest());
}

/// The adapter's identity enters replay identity through the scorer hash: the
/// symbolic scorer is unchanged, the composite names the adapter, and a
/// different checkpoint is a different identity.
#[test]
fn scorer_hash_participates_in_replay_identity() {
    let system = two_candidate_system();
    let mode = GuidanceMode::HeuristicPruning { beam_width: 1 };
    let adapter = strategy(prefers_larger_terms());
    let guided_outcome = completed(guided(
        BackwardExplorer::new(&system, small_config()).search_with_strategy(
            &parse("x"),
            &parse("z"),
            SearchMode::BreadthFirst,
            &mode,
            Some(&adapter),
        ),
    ));
    let evidence = approximate(&guided_outcome);
    assert_eq!(
        evidence.scorer_hash,
        scorer_hash_with(Some(&adapter.strategy_hash()))
    );
    assert_ne!(evidence.scorer_hash, scorer_hash());
    // The symbolic-only identity is untouched, and it is what an unguided run
    // records.
    assert_eq!(scorer_hash_with(None), scorer_hash());
    let unguided = BackwardExplorer::new(&system, small_config())
        .search_with_guidance(&parse("x"), &parse("z"), SearchMode::BreadthFirst, &mode)
        .unwrap();
    assert_eq!(approximate(&unguided).scorer_hash, scorer_hash());
    // A different checkpoint (different weights) is a different identity.
    let other = strategy(constant_ranker());
    assert_ne!(other.strategy_hash(), adapter.strategy_hash());
    assert_ne!(
        scorer_hash_with(Some(&other.strategy_hash())),
        evidence.scorer_hash
    );
}

/// Same (search input, checkpoint, config) => identical outcome, trajectory,
/// and replay hashes.
#[test]
fn identical_inputs_replay_identically() {
    let system = two_witness_system();
    let mode = GuidanceMode::CompleteWithinLimits;
    let run = || {
        let adapter = strategy(prefers_larger_terms());
        let outcome = completed(guided(
            BackwardExplorer::new(&system, small_config()).search_with_strategy(
                &parse("x"),
                &parse("d"),
                SearchMode::BreadthFirst,
                &mode,
                Some(&adapter),
            ),
        ));
        (adapter.strategy_hash(), outcome)
    };
    let (first_hash, first) = run();
    let (second_hash, second) = run();
    assert_eq!(first_hash, second_hash);
    assert_eq!(first, second);
    let BackwardSearchOutcome::Witness(derivation) = &first else {
        panic!("must witness");
    };
    assert_eq!(derivation.steps.first().unwrap().term, parse("f(c)"));
}
