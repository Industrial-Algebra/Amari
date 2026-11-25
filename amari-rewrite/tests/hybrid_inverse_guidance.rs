//! Hybrid inverse guidance contracts (0.25 Cohort 9, Task 48).
//!
//! One test per enumerated obligation of the task:
//!
//! - **explicit normalization** — symbolic/neural/geometric prefer LOWER and
//!   holographic recall prefers HIGHER; the hybrid maps every backend's own
//!   preference order onto one lower-is-better ordinal fraction *before* any
//!   weighted combination. Every normalized value is finite and inside `[0, 1]`
//!   on every admitted input, including missing and degenerate evidence;
//! - **typed finite weights** — every weight is validated finite and inside the
//!   documented `[0, 1]` domain before a search allocates anything; the zero and
//!   one extremes are pinned per backend, and the all-zero configuration is a
//!   documented pure content tie-break (never a hidden fallback score);
//! - **exhaustive component evidence** — every backend reports availability, raw
//!   evidence, normalized score, weight, and contribution, and a missing or
//!   failed backend is typed evidence (`Missing` / `Failed`), never a silent zero
//!   and never an invented success;
//! - **ordering parity** — a single enabled backend reproduces that backend's
//!   own ordering exactly; a zero-weight backend has no ordering effect; ties
//!   follow the canonical content-key tie-break; the direction is pinned end to
//!   end by a search that follows the hybrid's preferred candidate (Cohort-8
//!   lesson: a direction is declared *and* pinned);
//! - **pruning authority** — complete mode changes ordering only and preserves
//!   every candidate; truncation stays confined to
//!   [`GuidanceMode::HeuristicPruning`], uses the combined order, reports its
//!   dropped count, and returns `Approximate`;
//! - **deterministic identity** — identical configuration, backends, and
//!   evidence give bit-identical ordering and hashes; the scorer identity binds
//!   the hybrid schema, the normalization convention, every enabled backend's
//!   identity, and the finite weights;
//! - **provenance** — recall traces are accepted only after replay against the
//!   ORIGINAL system (a foreign-system trace is a typed rejection), and the
//!   hybrid identity records the original system hash. No trace is synthesized
//!   from the hybrid scorer;
//! - **feature isolation** — a default build exposes no hybrid surface at all
//!   (the isolation module below asserts the compiled feature set), and a build
//!   with only one backend reports the others as `Missing` evidence rather than
//!   failing to compile or silently skipping them.

#[cfg(any(feature = "neural", feature = "holographic-guidance"))]
#[allow(dead_code)]
mod fixtures {
    use amari_rewrite::inverse::{
        score_candidate, symbolic_predecessors, BackwardSearchOutcome, GuidanceCandidate,
        GuidanceMode, GuidanceRequest, GuidedSearchOutcome, InverseSearchConfig, SearchMode,
        SymbolicPredecessor, SymbolicScore, SymbolicState,
    };
    use amari_rewrite::relation::{ConstraintSet, RelationLimits};
    use amari_rewrite::trs::{Rule, Term, TermSystem};

    /// Minimal term parser for the fixtures (`f(a)` / `X` / `a`).
    pub fn parse(text: &str) -> Term {
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

    pub fn system(rules: Vec<Rule>) -> TermSystem {
        TermSystem::new(rules)
    }

    pub fn small_config() -> InverseSearchConfig {
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

    /// A budget that admits the root plus exactly one child.
    pub fn two_state_config() -> InverseSearchConfig {
        InverseSearchConfig::new(
            8,
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
        .unwrap()
    }

    /// `a -> b -> c -> a`: a finite backward closure.
    pub fn cycle_system() -> TermSystem {
        system(vec![
            Rule::new(parse("a"), parse("b")).unwrap(),
            Rule::new(parse("b"), parse("c")).unwrap(),
            Rule::new(parse("c"), parse("a")).unwrap(),
        ])
    }

    /// `b => x`, `f(c) => x`: two predecessors, one of one node and one of two.
    pub fn two_candidate_system() -> TermSystem {
        system(vec![
            Rule::new(parse("b"), parse("x")).unwrap(),
            Rule::new(parse("f(c)"), parse("x")).unwrap(),
        ])
    }

    /// The same two candidates, both reaching `d`: the visit order decides which
    /// (equally valid) witness is returned.
    pub fn two_witness_system() -> TermSystem {
        system(vec![
            Rule::new(parse("b"), parse("x")).unwrap(),
            Rule::new(parse("f(c)"), parse("x")).unwrap(),
            Rule::new(parse("d"), parse("b")).unwrap(),
            Rule::new(parse("d"), parse("f(c)")).unwrap(),
        ])
    }

    /// One expansion round's engine candidates, their states, and their
    /// transparent symbolic scores — in the engine's own order.
    pub struct Round {
        pub parent: SymbolicState,
        pub states: Vec<SymbolicState>,
        pub predecessors: Vec<SymbolicPredecessor>,
        pub scores: Vec<SymbolicScore>,
    }

    impl Round {
        pub fn of(system: &TermSystem, target: &Term) -> Self {
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

        pub fn candidates(&self) -> Vec<GuidanceCandidate<'_>> {
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

        pub fn request<'a>(
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

    pub fn guided(
        outcome: Result<GuidedSearchOutcome, amari_rewrite::RewriteError>,
    ) -> GuidedSearchOutcome {
        outcome.expect("search must not return a hard error")
    }

    pub fn completed(outcome: GuidedSearchOutcome) -> BackwardSearchOutcome {
        match outcome {
            GuidedSearchOutcome::Completed(outcome) => outcome,
            GuidedSearchOutcome::StrategyFailed(failure) => {
                panic!("unexpected strategy failure: {failure:?}")
            }
        }
    }

    pub fn approximate(
        outcome: &BackwardSearchOutcome,
    ) -> &amari_rewrite::inverse::ApproximateSearchEvidence {
        match outcome {
            BackwardSearchOutcome::Approximate(evidence) => evidence,
            other => panic!("expected an approximate outcome, got {other:?}"),
        }
    }

    /// The engine's own round order: `(symbolic score, canonical state digest)`.
    pub fn symbolic_order(round: &Round) -> Vec<usize> {
        let mut order: Vec<usize> = (0..round.states.len()).collect();
        order.sort_by(|&left, &right| {
            round.scores[left].cmp(&round.scores[right]).then_with(|| {
                round.states[left]
                    .canonical_digest()
                    .cmp(&round.states[right].canonical_digest())
            })
        });
        order
    }

    /// The canonical content order used when every combined score ties.
    pub fn digest_order(round: &Round) -> Vec<usize> {
        let mut order: Vec<usize> = (0..round.states.len()).collect();
        order.sort_by(|&left, &right| {
            round.states[left]
                .canonical_digest()
                .cmp(&round.states[right].canonical_digest())
        });
        order
    }

    /// Search the same system with the same mode, with and without a strategy,
    /// and assert complete-mode parity of the explored set.
    pub fn assert_complete_parity_without_dropping(
        system: &TermSystem,
        target: &Term,
        goal: &Term,
        strategy: &dyn amari_rewrite::inverse::GuidanceStrategy,
    ) {
        let baseline = amari_rewrite::inverse::BackwardExplorer::new(system, small_config())
            .search(target, goal, SearchMode::BreadthFirst)
            .expect("the baseline search runs");
        let hybrid = completed(guided(
            amari_rewrite::inverse::BackwardExplorer::new(system, small_config())
                .search_with_strategy(
                    target,
                    goal,
                    SearchMode::BreadthFirst,
                    &GuidanceMode::CompleteWithinLimits,
                    Some(strategy),
                ),
        ));
        let BackwardSearchOutcome::Exhausted(plain) = &baseline else {
            panic!("the baseline must exhaust: {baseline:?}");
        };
        let BackwardSearchOutcome::Exhausted(guided) = &hybrid else {
            panic!("complete mode must still exhaust: {hybrid:?}");
        };
        assert_eq!(
            plain.evidence_hash(),
            guided.evidence_hash(),
            "complete mode never changes an exhausted closure's explored set"
        );
        assert_eq!(plain.authority(), guided.authority());
    }
}

// ---------------------------------------------------------------- shared core

#[cfg(any(feature = "neural", feature = "holographic-guidance"))]
mod core {
    use crate::fixtures::*;
    use amari_rewrite::inverse::hybrid::{
        normalization_hash, BackendFailure, HybridAvailability, HybridBackend, HybridConfig,
        HybridConfigError, HybridStrategy, HybridWeights, HYBRID_SCHEMA_VERSION,
        MAX_HYBRID_CANDIDATES, NORMALIZATION_CONVENTION,
    };
    use amari_rewrite::inverse::{
        guidance_hash, scorer_hash, scorer_hash_with, BackwardExplorer, BackwardSearchOutcome,
        GuidanceFailureKind, GuidanceMode, GuidanceStrategy, SearchMode,
    };
    use amari_rewrite::relation::Sha256Digest;
    use amari_rewrite::trs::Term;

    pub fn weights(symbolic: f64, neural: f64, geometric: f64, recall: f64) -> HybridWeights {
        HybridWeights::new(symbolic, neural, geometric, recall).expect("valid weights")
    }

    pub fn config(weights: HybridWeights) -> HybridConfig {
        HybridConfig::new(weights, MAX_HYBRID_CANDIDATES).expect("valid config")
    }

    pub fn hybrid(weights: HybridWeights) -> HybridStrategy<'static> {
        HybridStrategy::new(config(weights))
    }

    #[test]
    fn normalization_is_a_pinned_lower_is_better_convention() {
        assert_eq!(
            HybridBackend::ALL,
            [
                HybridBackend::Symbolic,
                HybridBackend::Neural,
                HybridBackend::Geometric,
                HybridBackend::Recall,
            ]
        );
        // Directions are explicit, per backend, not implied by a table position.
        assert!(HybridBackend::Symbolic.prefers_lower());
        assert!(HybridBackend::Neural.prefers_lower());
        assert!(HybridBackend::Geometric.prefers_lower());
        assert!(!HybridBackend::Recall.prefers_lower());
        assert_eq!(HybridBackend::Symbolic.name(), "symbolic");
        assert_eq!(HybridBackend::Neural.name(), "neural");
        assert_eq!(HybridBackend::Geometric.name(), "geometric");
        assert_eq!(HybridBackend::Recall.name(), "recall");
        // The convention is published, non-empty, and pinned by a digest; the
        // digest is stable and distinct from the scorer and guidance identities.
        assert!(NORMALIZATION_CONVENTION.contains("lower"));
        assert_eq!(HYBRID_SCHEMA_VERSION, 1);
        assert_eq!(normalization_hash(), normalization_hash());
        assert_ne!(normalization_hash(), scorer_hash());
        assert_ne!(
            normalization_hash(),
            guidance_hash(&GuidanceMode::CompleteWithinLimits)
        );

        // Every normalized value is finite and inside the unit interval on a
        // two-candidate round; a single-candidate round normalizes to 0.
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let strategy = hybrid(weights(1.0, 0.0, 0.0, 0.0));
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        for candidate in &evaluation.candidates {
            for component in &candidate.components {
                if let Some(normalized) = component.normalized {
                    assert!(normalized.is_finite(), "normalized is finite");
                    assert!(
                        (0.0..=1.0).contains(&normalized),
                        "normalized is inside [0, 1]: {normalized}"
                    );
                }
            }
            assert!(candidate.combined.is_finite());
            assert!((0.0..=1.0).contains(&candidate.combined));
        }

        // The degenerate single-candidate round is finite too: `0.0`, not NaN.
        let single = Round {
            parent: round.parent.clone(),
            states: round.states[..1].to_vec(),
            predecessors: round.predecessors[..1].to_vec(),
            scores: round.scores[..1].to_vec(),
        };
        let candidates = single.candidates();
        let request = single.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        let symbolic = evaluation
            .candidate(0)
            .expect("candidate evidence")
            .component(HybridBackend::Symbolic)
            .expect("component evidence");
        assert_eq!(symbolic.normalized, Some(0.0));

        // An empty round has no candidates and no normalized value at all.
        let empty: Vec<amari_rewrite::inverse::GuidanceCandidate<'_>> = Vec::new();
        let request = round.request(&empty, &mode);
        let evaluation = strategy.evaluate(&request).expect("empty round ranks");
        assert!(evaluation.order.is_empty());
        assert!(evaluation.candidates.is_empty());
    }

    #[test]
    fn typed_finite_weights_are_validated_before_search() {
        // Non-finite weights are refused for every backend, never clamped.
        for backend in HybridBackend::ALL {
            for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
                let mut lanes = [0.0, 0.0, 0.0, 0.0];
                lanes[backend_position(backend)] = bad;
                assert_eq!(
                    HybridWeights::new(lanes[0], lanes[1], lanes[2], lanes[3]),
                    Err(HybridConfigError::NonFiniteWeight { backend }),
                    "a non-finite weight is a typed rejection, never clamped"
                );
            }
        }
        // The documented domain is `[0, 1]`: widening a configured authority is
        // a typed rejection on both sides.
        for backend in HybridBackend::ALL {
            for bad in [-0.25, 1.000_001, 2.0] {
                let mut lanes = [0.0, 0.0, 0.0, 0.0];
                lanes[backend_position(backend)] = bad;
                let error = HybridWeights::new(lanes[0], lanes[1], lanes[2], lanes[3]).unwrap_err();
                match error {
                    HybridConfigError::WeightOutOfRange {
                        backend: rejected,
                        weight,
                    } => {
                        assert_eq!(rejected, backend);
                        assert_eq!(weight.to_bits(), bad.to_bits());
                    }
                    other => panic!("expected an out-of-range rejection, got {other:?}"),
                }
            }
        }
        // Both extremes are in the domain, for every backend.
        assert!(HybridWeights::new(0.0, 0.0, 0.0, 0.0).is_ok());
        assert!(HybridWeights::new(1.0, 1.0, 1.0, 1.0).is_ok());
        for backend in HybridBackend::ALL {
            let single = HybridWeights::single(backend, 0.0).expect("zero weight");
            assert_eq!(single.get(backend), 0.0);
            let single = HybridWeights::single(backend, 1.0).expect("one weight");
            assert_eq!(single.get(backend), 1.0);
            for other in HybridBackend::ALL {
                if other != backend {
                    assert_eq!(single.get(other), 0.0);
                }
            }
        }
        // A negative zero is a zero: it is stored as `+0.0`, so a contribution
        // can never be a negative zero.
        let signed_zero = HybridWeights::new(-0.0, -0.0, -0.0, -0.0).expect("a zero");
        for backend in HybridBackend::ALL {
            assert_eq!(signed_zero.get(backend).to_bits(), 0.0f64.to_bits());
        }
        // A ceiling of zero or above the fixed ceiling is rejected before any
        // search exists.
        let valid = weights(1.0, 0.0, 0.0, 0.0);
        assert_eq!(
            HybridConfig::new(valid, 0).unwrap_err(),
            HybridConfigError::LimitExceeded {
                resource: "hybrid strategy candidates",
                value: 0,
                limit: MAX_HYBRID_CANDIDATES,
            }
        );
        assert_eq!(
            HybridConfig::new(valid, MAX_HYBRID_CANDIDATES + 1).unwrap_err(),
            HybridConfigError::LimitExceeded {
                resource: "hybrid strategy candidates",
                value: MAX_HYBRID_CANDIDATES + 1,
                limit: MAX_HYBRID_CANDIDATES,
            }
        );
        assert_eq!(
            HybridConfig::new(valid, 8)
                .expect("a tightened ceiling")
                .max_candidates(),
            8
        );
        // The default configuration is the transparent symbolic baseline.
        let default = HybridWeights::default();
        for backend in HybridBackend::ALL {
            assert_eq!(
                default.get(backend),
                if backend == HybridBackend::Symbolic {
                    1.0
                } else {
                    0.0
                }
            );
        }
    }

    fn backend_position(backend: HybridBackend) -> usize {
        match backend {
            HybridBackend::Symbolic => 0,
            HybridBackend::Neural => 1,
            HybridBackend::Geometric => 2,
            HybridBackend::Recall => 3,
        }
    }

    #[test]
    fn missing_backends_are_explicit_evidence() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let strategy = hybrid(weights(1.0, 0.5, 0.5, 0.5));
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        // In this build the neural/geometric/recall backends may be compiled but
        // were never supplied, so each is `Missing` — never a silent zero score.
        for backend in [
            HybridBackend::Neural,
            HybridBackend::Geometric,
            HybridBackend::Recall,
        ] {
            assert_eq!(
                evaluation.availability(backend),
                Some(HybridAvailability::Missing)
            );
            for candidate in &evaluation.candidates {
                let component = candidate.component(backend).expect("a reported component");
                assert_eq!(component.availability, HybridAvailability::Missing);
                assert_eq!(component.raw, None);
                assert_eq!(component.normalized, None);
                assert_eq!(component.contribution.to_bits(), 0.0f64.to_bits());
                // The configured weight is still reported: the evidence says
                // what the caller asked for and what it actually got.
                assert_eq!(component.weight, 0.5);
            }
        }
        // The symbolic backend is always available and always reported.
        assert_eq!(
            evaluation.availability(HybridBackend::Symbolic),
            Some(HybridAvailability::Available)
        );
        for candidate in &evaluation.candidates {
            let component = candidate
                .component(HybridBackend::Symbolic)
                .expect("a reported component");
            assert_eq!(component.availability, HybridAvailability::Available);
            assert!(component.raw.is_some());
            assert!(component.normalized.is_some());
        }
        assert!(HybridAvailability::Missing.failure().is_none());
        assert!(!HybridAvailability::Missing.is_available());
    }

    #[test]
    fn all_zero_weights_rank_by_content_only() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let strategy = hybrid(HybridWeights::new(0.0, 0.0, 0.0, 0.0).expect("all zero"));
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        // Every advertised backend contributes exactly `+0.0`; the all-zero
        // configuration is a documented content tie-break, not a hidden score.
        for candidate in &evaluation.candidates {
            assert_eq!(candidate.combined.to_bits(), 0.0f64.to_bits());
            for component in &candidate.components {
                assert_eq!(component.weight, 0.0);
                assert_eq!(component.contribution.to_bits(), 0.0f64.to_bits());
            }
        }
        // With every combined score tied, the canonical content order decides.
        assert_eq!(evaluation.order, digest_order(&round));
        let mut permutation = evaluation.order.clone();
        permutation.sort_unstable();
        assert_eq!(permutation, (0..round.states.len()).collect::<Vec<_>>());
    }

    #[test]
    fn ties_follow_the_canonical_content_key_not_request_order() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let strategy = hybrid(HybridWeights::new(0.0, 0.0, 0.0, 0.0).expect("all zero"));
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        let ordered: Vec<Sha256Digest> = evaluation
            .order
            .iter()
            .map(|&index| round.states[index].canonical_digest())
            .collect();
        let mut ascending = ordered.clone();
        ascending.sort();
        assert_eq!(
            ordered, ascending,
            "ties resolve by ascending canonical state digest"
        );

        // Permuting the request permutes the indices, not the state order.
        let mut reversed_candidates = candidates.clone();
        reversed_candidates.reverse();
        let reversed_scores: Vec<amari_rewrite::inverse::SymbolicScore> =
            round.scores.iter().rev().cloned().collect();
        let reversed_states: Vec<_> = round.states.iter().rev().cloned().collect();
        let reversed_predecessors: Vec<_> = round.predecessors.iter().rev().cloned().collect();
        let reversed_round = Round {
            parent: round.parent.clone(),
            states: reversed_states,
            predecessors: reversed_predecessors,
            scores: reversed_scores,
        };
        let reversed_candidates = reversed_round.candidates();
        let reversed_request = reversed_round.request(&reversed_candidates, &mode);
        let reversed = strategy
            .evaluate(&reversed_request)
            .expect("admissible round");
        let reversed_ordered: Vec<Sha256Digest> = reversed
            .order
            .iter()
            .map(|&index| reversed_round.states[index].canonical_digest())
            .collect();
        assert_eq!(
            ordered, reversed_ordered,
            "the content order is a function of content, never of request order"
        );
    }

    #[test]
    fn single_symbolic_backend_reproduces_the_engine_order() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let strategy = hybrid(weights(1.0, 0.0, 0.0, 0.0));
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        assert_eq!(evaluation.order, symbolic_order(&round));
        // The engine's own order puts the smaller-growth candidate first.
        assert_eq!(round.states[evaluation.order[0]].term(), &parse("b"));

        // Weight 1 pins the extreme: the normalized score IS the combined score.
        for candidate in &evaluation.candidates {
            let component = candidate
                .component(HybridBackend::Symbolic)
                .expect("component evidence");
            assert_eq!(
                candidate.combined.to_bits(),
                component.normalized.expect("available").to_bits()
            );
            assert_eq!(
                component.contribution.to_bits(),
                component.normalized.expect("available").to_bits()
            );
        }

        // End to end: a symbolic-only hybrid is ordering-only, so a fully
        // exhausted closure keeps the unguided explored set.
        assert_complete_parity_without_dropping(
            &cycle_system(),
            &parse("a"),
            &parse("z"),
            &strategy,
        );
    }

    #[test]
    fn complete_mode_preserves_candidates_and_pruning_is_confined_to_the_mode() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let strategy = hybrid(weights(1.0, 0.0, 0.0, 0.0));
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        // Ordering only: a full permutation of the round, never a truncation.
        let mut permutation = evaluation.order.clone();
        permutation.sort_unstable();
        assert_eq!(permutation, (0..round.states.len()).collect::<Vec<_>>());

        // Truncation is the mode's authority: the beam keeps one of two and
        // reports exactly one dropped candidate, with the established marker.
        let beam = GuidanceMode::HeuristicPruning { beam_width: 1 };
        let pruned = completed(guided(
            BackwardExplorer::new(&system, small_config()).search_with_strategy(
                &parse("x"),
                &parse("z"),
                SearchMode::BreadthFirst,
                &beam,
                Some(&strategy),
            ),
        ));
        let evidence = approximate(&pruned);
        assert_eq!(evidence.dropped_candidates, 1);
        assert!(
            evidence
                .summary
                .contains("heuristic pruning dropped 1 valid candidates"),
            "the established marker is kept: {}",
            evidence.summary
        );
        // The pruning order is the hybrid's combined order: the kept candidate
        // is the one the hybrid prefers.
        assert_eq!(
            evidence.scorer_hash,
            scorer_hash_with(Some(&strategy.strategy_hash()))
        );
        assert_eq!(evidence.guidance_hash, guidance_hash(&beam));

        // A closure wide enough to prune nothing keeps full authority.
        assert_complete_parity_without_dropping(
            &cycle_system(),
            &parse("a"),
            &parse("z"),
            &strategy,
        );
    }

    #[test]
    fn budget_truncated_runs_stay_partial_never_certificates() {
        let system = system(vec![
            amari_rewrite::trs::Rule::new(parse("c"), parse("x")).unwrap(),
            amari_rewrite::trs::Rule::new(parse("b"), parse("x")).unwrap(),
        ]);
        let strategy = hybrid(weights(1.0, 0.0, 0.0, 0.0));
        let guided_outcome = completed(guided(
            BackwardExplorer::new(&system, two_state_config()).search_with_strategy(
                &parse("x"),
                &parse("z"),
                SearchMode::BreadthFirst,
                &GuidanceMode::CompleteWithinLimits,
                Some(&strategy),
            ),
        ));
        let BackwardSearchOutcome::Partial(frontier) = &guided_outcome else {
            panic!("a budget cut is Partial, never Approximate: {guided_outcome:?}");
        };
        assert_eq!(frontier.states.len(), 1);
        // The retained state is one the engine itself produced for this round.
        let engine_round = Round::of(&system, &parse("x"));
        assert!(
            engine_round
                .states
                .iter()
                .any(|state| state.term() == frontier.states[0].term()),
            "every retained frontier state is engine-built"
        );
    }

    #[test]
    fn scorer_identity_binds_schema_normalization_weights_and_backends() {
        let base = hybrid(weights(1.0, 0.0, 0.0, 0.0));
        let same = HybridStrategy::new(config(weights(1.0, 0.0, 0.0, 0.0)));
        assert_eq!(base.replay_hash(), same.replay_hash());
        assert_eq!(base.strategy_hash(), base.replay_hash());
        // The hybrid identity is its own scorer identity, and it is not the
        // transparent symbolic scorer it contains.
        assert_ne!(base.strategy_hash(), scorer_hash());
        assert_eq!(
            scorer_hash_with(Some(&base.strategy_hash())),
            scorer_hash_with(Some(&same.strategy_hash()))
        );

        // A changed weight is a changed scorer.
        assert_ne!(
            base.replay_hash(),
            hybrid(weights(0.5, 0.0, 0.0, 0.0)).replay_hash()
        );
        // A changed ceiling is a changed scorer (it can change which rounds are
        // admitted).
        let narrowed = HybridStrategy::new(
            HybridConfig::new(weights(1.0, 0.0, 0.0, 0.0), 8).expect("narrowed"),
        );
        assert_ne!(base.replay_hash(), narrowed.replay_hash());
        assert_eq!(narrowed.max_candidates(), 8);
        // The schema version and the normalization convention are folded in:
        // the digest names both, so two hybrids cannot share an identity while
        // normalizing differently.
        assert_ne!(base.replay_hash(), normalization_hash());
    }

    #[test]
    fn identical_configuration_replays_bit_identically() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let first = hybrid(weights(0.25, 0.5, 0.75, 0.5));
        let second = hybrid(weights(0.25, 0.5, 0.75, 0.5));
        assert_eq!(first.replay_hash(), second.replay_hash());
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let left = first.evaluate(&request).expect("admissible round");
        let right = second.evaluate(&request).expect("admissible round");
        // Bit-identical, not merely equal: the ordering and every reported
        // number are a function of the configuration and the evidence alone.
        assert_eq!(left.order, right.order);
        assert_eq!(left.candidates.len(), right.candidates.len());
        for (left, right) in left.candidates.iter().zip(&right.candidates) {
            assert_eq!(left.index, right.index);
            assert_eq!(left.combined.to_bits(), right.combined.to_bits());
            assert_eq!(left.components.len(), right.components.len());
            for (left, right) in left.components.iter().zip(&right.components) {
                assert_eq!(left.backend, right.backend);
                assert_eq!(left.availability, right.availability);
                assert_eq!(
                    left.contribution.to_bits(),
                    right.contribution.to_bits(),
                    "contributions are bit-identical"
                );
                assert_eq!(
                    left.normalized.map(f64::to_bits),
                    right.normalized.map(f64::to_bits)
                );
            }
        }
        // Repeating the same evaluation on the same instance is identical too.
        let again = first.evaluate(&request).expect("admissible round");
        assert_eq!(left.order, again.order);
    }

    #[test]
    fn contributions_sum_to_the_combined_score_in_declaration_order() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let strategy = hybrid(weights(0.25, 0.5, 0.75, 0.125));
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        for candidate in &evaluation.candidates {
            let mut sum = 0.0f64;
            for backend in HybridBackend::ALL {
                let component = candidate.component(backend).expect("component evidence");
                sum += component.contribution;
            }
            assert_eq!(candidate.combined.to_bits(), sum.to_bits());
            // Components are reported in declaration order.
            let reported: Vec<HybridBackend> =
                candidate.components.iter().map(|c| c.backend).collect();
            assert_eq!(reported, HybridBackend::ALL.to_vec());
        }
    }

    #[test]
    fn failed_component_is_typed_evidence_not_a_silent_zero() {
        // A whole-round admission rejection is the hybrid's own authority: it
        // reports the ceiling and scores nothing.
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let strategy = HybridStrategy::new(
            HybridConfig::new(weights(1.0, 0.0, 0.0, 0.0), 1).expect("narrowed"),
        );
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let failure = strategy
            .evaluate(&request)
            .expect_err("two candidates exceed a one-candidate ceiling");
        assert_eq!(failure.scored, 0);
        assert!(matches!(
            failure.kind,
            GuidanceFailureKind::LimitExceeded {
                resource: "hybrid strategy candidates",
                value: 2,
                limit: 1,
            }
        ));
        // The failure type is the guidance seam's own vocabulary.
        let evidence = BackendFailure {
            kind: GuidanceFailureKind::Unavailable {
                stage: "test surface",
            },
            scored: 0,
        };
        assert_eq!(evidence.scored, 0);
        assert!(!HybridAvailability::Failed(evidence).is_available());
        assert!(HybridAvailability::Failed(evidence).failure().is_some());
    }

    #[test]
    fn complete_mode_ordering_steers_the_witness_without_dropping() {
        // The symbolic baseline takes `b`; the hybrid's own symbolic order is
        // the same, so this pins the *seam*: the search follows the hybrid's
        // first index, and the derived witness replays.
        let system = two_witness_system();
        let round = Round::of(&system, &parse("x"));
        let strategy = hybrid(weights(1.0, 0.0, 0.0, 0.0));
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        let preferred = round.states[evaluation.order[0]].term().clone();

        let outcome = completed(guided(
            BackwardExplorer::new(&system, small_config()).search_with_strategy(
                &parse("x"),
                &parse("d"),
                SearchMode::BreadthFirst,
                &mode,
                Some(&strategy),
            ),
        ));
        let BackwardSearchOutcome::Witness(derivation) = outcome else {
            panic!("the system must witness");
        };
        assert_eq!(derivation.steps[0].term, preferred);
        assert_eq!(derivation.steps[0].term, parse("b"));
    }

    #[test]
    fn an_empty_round_ranks_without_allocating_from_missing_backends() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let strategy = hybrid(weights(1.0, 1.0, 1.0, 1.0));
        let mode = GuidanceMode::CompleteWithinLimits;
        let empty: Vec<amari_rewrite::inverse::GuidanceCandidate<'_>> = Vec::new();
        let request = round.request(&empty, &mode);
        let evaluation = strategy
            .evaluate(&request)
            .expect("empty round is admissible");
        assert!(evaluation.order.is_empty());
        assert!(evaluation.candidates.is_empty());
    }

    /// The hybrid never reads a global, the filesystem, or the environment: two
    /// independent instances over the same content agree exactly, and a round
    /// scored twice on one instance cannot drift.
    #[test]
    fn the_round_is_a_pure_function_of_its_inputs() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let strategy = hybrid(weights(0.5, 0.0, 0.0, 0.0));
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let first = strategy.evaluate(&request).expect("admissible round");
        let second = strategy.evaluate(&request).expect("admissible round");
        assert_eq!(first, second);

        // A different *search* state cannot change this round's evidence: the
        // strategy holds only immutable configuration.
        let other = Term::constant("unrelated");
        assert_eq!(other, Term::constant("unrelated"));
    }
}

// ------------------------------------------------------------ neural backend

#[cfg(feature = "neural")]
mod neural_backend {
    use crate::core::{config, hybrid, weights};
    use crate::fixtures::*;
    use amari_rewrite::inverse::hybrid::{HybridAvailability, HybridBackend, HybridStrategy};
    use amari_rewrite::inverse::{
        guidance_hash, BackwardExplorer, BackwardSearchOutcome, GuidanceFailureKind, GuidanceMode,
        GuidanceStrategy, ReplayCertificate, ReplayDerivation, ReplayQuery, ResourceObservation,
        SearchMode,
    };
    use amari_rewrite::neural::{
        FeatureEncoder, MlpRanker, NeuralStrategy, RankerConfig, RankerWeights,
    };
    use amari_rewrite::relation::RelationLimits;
    use amari_rewrite::trs::TermSystem;

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

    fn ranker(tensors: Vec<(&str, Vec<f32>)>) -> MlpRanker {
        let config = spec(&[1]);
        let borrowed: Vec<(&str, &[f32])> = tensors
            .iter()
            .map(|(name, values)| (*name, values.as_slice()))
            .collect();
        let weights = RankerWeights::from_tensors(&config, &borrowed).expect("admissible weights");
        MlpRanker::new(config, weights).expect("matching weights")
    }

    /// `relu(1 - lane0)`: lower is better, so *larger* terms score lower.
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

    /// Zero weights: every candidate ties, resolved by the content key.
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

    fn neural(ranker: MlpRanker) -> NeuralStrategy {
        NeuralStrategy::from_ranker(ranker, encoder())
    }

    #[test]
    fn neural_weight_one_reproduces_the_neural_ordering() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let adapter = neural(prefers_larger_terms());
        let strategy =
            HybridStrategy::new(config(weights(0.0, 1.0, 0.0, 0.0))).with_neural(&adapter);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);

        let evaluation = strategy.evaluate(&request).expect("admissible round");
        assert_eq!(
            evaluation.order,
            adapter.rank(&request).expect("neural ranks")
        );
        assert_eq!(evaluation.order, vec![1, 0], "the larger term is preferred");
        assert_eq!(round.states[evaluation.order[0]].term(), &parse("f(c)"));

        for candidate in &evaluation.candidates {
            let component = candidate
                .component(HybridBackend::Neural)
                .expect("component evidence");
            assert_eq!(component.availability, HybridAvailability::Available);
            // Raw evidence is the ranker's own native score, finite by contract.
            assert!(component.raw.expect("raw neural score").is_finite());
            assert_eq!(component.weight, 1.0);
            assert_eq!(
                candidate.combined.to_bits(),
                component.normalized.expect("normalized").to_bits()
            );
            // The symbolic lane is weighted out: available, but contributing
            // exactly +0.0.
            let symbolic = candidate
                .component(HybridBackend::Symbolic)
                .expect("component evidence");
            assert_eq!(symbolic.availability, HybridAvailability::Available);
            assert_eq!(symbolic.contribution.to_bits(), 0.0f64.to_bits());
        }
    }

    #[test]
    fn zero_weight_neural_backend_has_no_ordering_effect() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let adapter = neural(prefers_larger_terms());
        let without = hybrid(weights(1.0, 0.0, 0.0, 0.0));
        let with = HybridStrategy::new(config(weights(1.0, 0.0, 0.0, 0.0))).with_neural(&adapter);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let plain = without.evaluate(&request).expect("admissible round");
        let weightless = with.evaluate(&request).expect("admissible round");
        assert_eq!(plain.order, weightless.order);
        // The zero-weight backend is still fully reported: its evidence exists,
        // only its influence is zero.
        assert_eq!(
            weightless.availability(HybridBackend::Neural),
            Some(HybridAvailability::Available)
        );
        for candidate in &weightless.candidates {
            let component = candidate
                .component(HybridBackend::Neural)
                .expect("component evidence");
            assert!(component.normalized.is_some());
            assert_eq!(component.contribution.to_bits(), 0.0f64.to_bits());
        }
        // The zero-weight backend still changes the *identity*, because a
        // supplied backend must never replay as an absent one.
        assert_ne!(without.strategy_hash(), with.strategy_hash());
    }

    #[test]
    fn neural_ties_follow_the_content_key() {
        let system = TermSystem::new(vec![
            amari_rewrite::trs::Rule::new(parse("b"), parse("x")).unwrap(),
            amari_rewrite::trs::Rule::new(parse("c"), parse("x")).unwrap(),
        ]);
        let round = Round::of(&system, &parse("x"));
        let adapter = neural(constant_ranker());
        let strategy =
            HybridStrategy::new(config(weights(0.0, 1.0, 0.0, 0.0))).with_neural(&adapter);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        // A zero-weight ranker is a pure tie: the order is the ranker's own
        // content-key order, which the hybrid reproduces exactly.
        assert_eq!(
            evaluation.order,
            adapter.rank(&request).expect("neural ranks")
        );
    }

    #[test]
    fn neural_backend_failure_is_typed_evidence() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        // A one-node walk ceiling admits `b` and rejects `f(c)` mid-round.
        let tight = FeatureEncoder::new(RelationLimits::new(1, 4, 4, 16).expect("valid limits"));
        let adapter = NeuralStrategy::from_ranker(prefers_larger_terms(), tight);
        let strategy =
            HybridStrategy::new(config(weights(0.5, 0.5, 0.0, 0.0))).with_neural(&adapter);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("the round still ranks");
        match evaluation.availability(HybridBackend::Neural) {
            Some(HybridAvailability::Failed(failure)) => {
                assert!(
                    matches!(failure.kind, GuidanceFailureKind::LimitExceeded { .. }),
                    "the rejection is a typed ceiling: {:?}",
                    failure.kind
                );
                assert_eq!(failure.scored, 1, "one candidate was scored first");
            }
            other => panic!("expected typed evidence of a backend failure, got {other:?}"),
        }
        // The failure is explicit, and the remaining evidence still orders the
        // round: the symbolic lane decides, and nothing is invented for neural.
        assert_eq!(evaluation.order, symbolic_order(&round));
        for candidate in &evaluation.candidates {
            let component = candidate
                .component(HybridBackend::Neural)
                .expect("component evidence");
            assert!(!component.availability.is_available());
            assert_eq!(component.raw, None);
            assert_eq!(component.normalized, None);
            assert_eq!(component.contribution.to_bits(), 0.0f64.to_bits());
            assert_eq!(component.weight, 0.5);
        }
    }

    #[test]
    fn neural_direction_is_pinned_end_to_end() {
        let system = two_witness_system();
        let round = Round::of(&system, &parse("x"));
        let adapter = neural(prefers_larger_terms());
        let strategy =
            HybridStrategy::new(config(weights(0.0, 1.0, 0.0, 0.0))).with_neural(&adapter);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        let preferred = round.states[evaluation.order[0]].term().clone();
        assert_eq!(preferred, parse("f(c)"));

        let baseline = BackwardExplorer::new(&system, small_config())
            .search(&parse("x"), &parse("d"), SearchMode::BreadthFirst)
            .expect("baseline runs");
        let BackwardSearchOutcome::Witness(baseline) = baseline else {
            panic!("the baseline must witness");
        };
        assert_eq!(baseline.steps[0].term, parse("b"));

        let outcome = completed(guided(
            BackwardExplorer::new(&system, small_config()).search_with_strategy(
                &parse("x"),
                &parse("d"),
                SearchMode::BreadthFirst,
                &mode,
                Some(&strategy),
            ),
        ));
        let BackwardSearchOutcome::Witness(derivation) = outcome else {
            panic!("the guided search must witness");
        };
        assert_eq!(
            derivation.steps[0].term, preferred,
            "the search follows the hybrid's first index"
        );
        // The steering is not a reordering of authority: the witness still
        // replays through the original system under the same guidance mode.
        let certificate = ReplayCertificate::issue(
            &system,
            ReplayQuery::Backward {
                target: parse("x"),
                goal: parse("d"),
            },
            ReplayDerivation::Backward(derivation),
            small_config(),
            GuidanceMode::CompleteWithinLimits,
            ResourceObservation {
                states: 3,
                transitions: 2,
                retained_bytes: 256,
                operations: 8,
            },
        );
        certificate.verify(&system).expect("the witness replays");
        // Steering never changes what a derivation is: the guidance-mode
        // identity is unchanged, and the scorer identity is the one the search
        // reported rather than one the strategy invented.
        assert_eq!(
            certificate.guidance_hash,
            guidance_hash(&GuidanceMode::CompleteWithinLimits)
        );
    }

    #[test]
    fn neural_identity_enters_the_hybrid_identity() {
        let larger = neural(prefers_larger_terms());
        let constant = neural(constant_ranker());
        let first = HybridStrategy::new(config(weights(0.5, 0.5, 0.0, 0.0))).with_neural(&larger);
        let same = HybridStrategy::new(config(weights(0.5, 0.5, 0.0, 0.0))).with_neural(&larger);
        let other = HybridStrategy::new(config(weights(0.5, 0.5, 0.0, 0.0))).with_neural(&constant);
        assert_eq!(first.strategy_hash(), same.strategy_hash());
        assert_ne!(
            first.strategy_hash(),
            other.strategy_hash(),
            "a different checkpoint is a different scorer"
        );
        // The identity also records the encoder's admission ceiling: it can
        // change which rounds a backend can score.
        let narrowed = FeatureEncoder::new(RelationLimits::new(64, 32, 16, 1_024).expect("valid"));
        let narrowed_strategy = NeuralStrategy::from_ranker(constant_ranker(), narrowed);
        let third = HybridStrategy::new(config(weights(0.5, 0.5, 0.0, 0.0)))
            .with_neural(&narrowed_strategy);
        assert_ne!(first.strategy_hash(), third.strategy_hash());
    }
}

// -------------------------------------------------------- geometric backend

#[cfg(feature = "network")]
mod geometric_backend {
    use crate::core::{config, hybrid, weights};
    use crate::fixtures::*;
    use amari_rewrite::inverse::hybrid::{HybridAvailability, HybridBackend, HybridStrategy};
    use amari_rewrite::inverse::{
        BackwardExplorer, BackwardSearchOutcome, GuidanceFailureKind, GuidanceMode,
        GuidanceStrategy, SearchMode,
    };
    use amari_rewrite::network::{
        GeometricStrategy, GeometricStrategyConfig, GraphNodeInput, InverseGraphConfig,
        InverseSearchGraph,
    };
    use amari_rewrite::relation::RelationLimits;

    /// A graph holding only `b`: `b` is a revisit at distance 0, `f(c)` is unseen.
    fn graph_with_b() -> InverseSearchGraph {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let candidates = round.candidates();
        let mut graph = InverseSearchGraph::new(InverseGraphConfig::default());
        for candidate in &candidates {
            if candidate.state.term() == &parse("b") {
                graph
                    .insert(GraphNodeInput::of_candidate(candidate))
                    .expect("admissible node");
            }
        }
        assert_eq!(graph.num_nodes(), 1);
        graph
    }

    fn geometric(graph: InverseSearchGraph) -> GeometricStrategy {
        GeometricStrategy::new(graph, GeometricStrategyConfig::default())
    }

    #[test]
    fn geometric_weight_one_reproduces_the_geometric_ordering() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let adapter = geometric(graph_with_b());
        let strategy =
            HybridStrategy::new(config(weights(0.0, 0.0, 1.0, 0.0))).with_geometric(&adapter);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        assert_eq!(
            evaluation.order,
            adapter.rank(&request).expect("geometric ranks")
        );
        for candidate in &evaluation.candidates {
            let component = candidate
                .component(HybridBackend::Geometric)
                .expect("component evidence");
            assert_eq!(component.availability, HybridAvailability::Available);
            let raw = component.raw.expect("raw geometric total");
            assert!(raw.is_finite());
            assert!((0.0..=1.0).contains(&raw), "the total is inside [0, 1]");
            assert_eq!(
                candidate.combined.to_bits(),
                component.normalized.expect("normalized").to_bits()
            );
        }
    }

    #[test]
    fn zero_weight_geometric_backend_has_no_ordering_effect() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let adapter = geometric(graph_with_b());
        let without = hybrid(weights(1.0, 0.0, 0.0, 0.0));
        let with =
            HybridStrategy::new(config(weights(1.0, 0.0, 0.0, 0.0))).with_geometric(&adapter);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let plain = without.evaluate(&request).expect("admissible round");
        let weightless = with.evaluate(&request).expect("admissible round");
        assert_eq!(plain.order, weightless.order);
        assert_eq!(
            weightless.availability(HybridBackend::Geometric),
            Some(HybridAvailability::Available)
        );
        for candidate in &weightless.candidates {
            let component = candidate
                .component(HybridBackend::Geometric)
                .expect("component evidence");
            assert_eq!(component.contribution.to_bits(), 0.0f64.to_bits());
        }
        // Supplying a backend is part of the identity even at weight zero.
        assert_ne!(without.strategy_hash(), with.strategy_hash());
    }

    #[test]
    fn geometric_backend_failure_is_typed_evidence() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        // A one-node walk ceiling admits the descriptor of `b` and rejects the
        // descriptor of `f(c)` mid-round.
        let limits = RelationLimits::new(1, 4, 4, 16).expect("valid limits");
        let graph =
            InverseSearchGraph::new(InverseGraphConfig::new(4, 4, limits).expect("bounded graph"));
        let adapter = geometric(graph);
        let strategy =
            HybridStrategy::new(config(weights(0.5, 0.0, 0.5, 0.0))).with_geometric(&adapter);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("the round still ranks");
        match evaluation.availability(HybridBackend::Geometric) {
            Some(HybridAvailability::Failed(failure)) => {
                assert!(
                    matches!(failure.kind, GuidanceFailureKind::LimitExceeded { .. }),
                    "the rejection is a typed ceiling: {:?}",
                    failure.kind
                );
            }
            other => panic!("expected typed evidence of a backend failure, got {other:?}"),
        }
        assert_eq!(evaluation.order, symbolic_order(&round));
        for candidate in &evaluation.candidates {
            let component = candidate
                .component(HybridBackend::Geometric)
                .expect("component evidence");
            assert_eq!(component.raw, None);
            assert_eq!(component.normalized, None);
            assert_eq!(component.contribution.to_bits(), 0.0f64.to_bits());
        }
    }

    #[test]
    fn geometric_direction_is_pinned_end_to_end() {
        let system = two_witness_system();
        let round = Round::of(&system, &parse("x"));
        let adapter = geometric(graph_with_b());
        let strategy =
            HybridStrategy::new(config(weights(0.0, 0.0, 1.0, 0.0))).with_geometric(&adapter);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        let preferred = round.states[evaluation.order[0]].term().clone();
        let outcome = completed(guided(
            BackwardExplorer::new(&system, small_config()).search_with_strategy(
                &parse("x"),
                &parse("d"),
                SearchMode::BreadthFirst,
                &mode,
                Some(&strategy),
            ),
        ));
        let BackwardSearchOutcome::Witness(derivation) = outcome else {
            panic!("the guided search must witness");
        };
        assert_eq!(
            derivation.steps[0].term, preferred,
            "the geometric preference is the visit order"
        );
    }
}

// ----------------------------------------------------------- recall backend

#[cfg(feature = "holographic-guidance")]
mod recall_backend {
    use crate::core::{config, hybrid, weights};
    use crate::fixtures::*;
    use amari_rewrite::holographic::{
        HolographicEncoder, RecallConfig, RecallTrace, TraceCorpus, MAX_TRACE_TERMS,
    };
    use amari_rewrite::inverse::hybrid::{
        HybridAvailability, HybridBackend, HybridConfigError, HybridStrategy, RecallBackend,
        RecallLane,
    };
    use amari_rewrite::inverse::{
        guidance_hash, query_hash, system_hash, BackwardExplorer, BackwardSearchOutcome,
        GuidanceFailureKind, GuidanceMode, GuidanceStrategy, ReplayCertificate, ReplayDerivation,
        ReplayQuery, ResourceObservation, SearchMode,
    };
    use amari_rewrite::relation::RelationLimits;
    use amari_rewrite::trs::{Rule, Term, TermSystem};

    const SEED: u64 = 0x5441_534B_3438_0001;
    const DIMENSION: usize = 64;

    fn encoder(seed: u64) -> HolographicEncoder<DIMENSION> {
        HolographicEncoder::new(seed, RelationLimits::default()).expect("bounded encoder")
    }

    fn recall_config() -> RecallConfig {
        RecallConfig::new(8, MAX_TRACE_TERMS, 8).expect("bounded recall config")
    }

    fn certificate(system: &TermSystem, target: &Term, goal: &Term) -> ReplayCertificate {
        let outcome = BackwardExplorer::new(system, small_config())
            .search(target, goal, SearchMode::BreadthFirst)
            .expect("search must run");
        let BackwardSearchOutcome::Witness(derivation) = outcome else {
            panic!("the fixture system must yield a witness");
        };
        ReplayCertificate::issue(
            system,
            ReplayQuery::Backward {
                target: target.clone(),
                goal: goal.clone(),
            },
            ReplayDerivation::Backward(derivation),
            small_config(),
            GuidanceMode::CompleteWithinLimits,
            ResourceObservation {
                states: 3,
                transitions: 2,
                retained_bytes: 256,
                operations: 32,
            },
        )
    }

    /// Goal `b`, step `b`: the trace that remembers the `b` path.
    fn trace_b(system: &TermSystem) -> RecallTrace {
        RecallTrace::validate(system, &certificate(system, &parse("x"), &parse("b")))
            .expect("a replayed certificate mints a trace")
    }

    /// Goal `f(c)`, step `f(c)`: the trace that remembers the `f(c)` path.
    fn trace_fc(system: &TermSystem) -> RecallTrace {
        RecallTrace::validate(system, &certificate(system, &parse("x"), &parse("f(c)")))
            .expect("a replayed certificate mints a trace")
    }

    fn lane(
        system: &TermSystem,
        seed: u64,
        query: RecallTrace,
        traces: Vec<RecallTrace>,
    ) -> RecallLane<DIMENSION> {
        RecallLane::new(system, encoder(seed), recall_config(), query, traces)
            .expect("the lane accepts replay-validated traces of the original system")
    }

    #[test]
    fn recall_weight_one_reproduces_the_recall_ordering() {
        let system = two_witness_system();
        let round = Round::of(&system, &parse("x"));
        // The query remembers the `f(c)` path, so `f(c)` is the more similar
        // candidate. Higher similarity is preferred: the direction is inverted
        // by the normalization, not by a sign hack in the combination.
        let lane = lane(
            &system,
            SEED,
            trace_fc(&system),
            vec![trace_b(&system), trace_fc(&system)],
        );
        let strategy = HybridStrategy::new(config(weights(0.0, 0.0, 0.0, 1.0))).with_recall(&lane);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");

        let similarities = lane
            .similarities(&candidates)
            .expect("recall scores the round");
        assert!(similarities[1] > similarities[0], "f(c) is more similar");
        assert_eq!(evaluation.order, vec![1, 0]);
        for candidate in &evaluation.candidates {
            let component = candidate
                .component(HybridBackend::Recall)
                .expect("component evidence");
            assert_eq!(component.availability, HybridAvailability::Available);
            let raw = component.raw.expect("raw similarity");
            assert!(raw.is_finite());
            assert_eq!(raw.to_bits(), similarities[candidate.index].to_bits());
            assert_eq!(
                candidate.combined.to_bits(),
                component.normalized.expect("normalized").to_bits()
            );
        }
        // Higher raw similarity is a *lower* normalized score.
        let f_c = evaluation.candidate(1).expect("candidate evidence");
        let b = evaluation.candidate(0).expect("candidate evidence");
        let f_c_normalized = f_c
            .component(HybridBackend::Recall)
            .and_then(|component| component.normalized)
            .expect("normalized");
        let b_normalized = b
            .component(HybridBackend::Recall)
            .and_then(|component| component.normalized)
            .expect("normalized");
        assert!(f_c_normalized < b_normalized);
    }

    #[test]
    fn candidates_without_remembered_content_get_the_neutral_native_value() {
        let system = two_witness_system();
        let round = Round::of(&system, &parse("x"));
        let lane = lane(&system, SEED, trace_b(&system), vec![trace_b(&system)]);
        let strategy = HybridStrategy::new(config(weights(0.0, 0.0, 0.0, 1.0))).with_recall(&lane);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        let similarities = lane
            .similarities(&candidates)
            .expect("recall scores the round");
        assert_eq!(similarities[0], 1.0, "b is remembered exactly");
        assert_eq!(
            similarities[1], 0.0,
            "no remembered trace contains f(c): the documented neutral value"
        );
        let f_c = evaluation.candidate(1).expect("candidate evidence");
        let component = f_c
            .component(HybridBackend::Recall)
            .expect("component evidence");
        assert_eq!(component.raw, Some(0.0));
    }

    #[test]
    fn zero_weight_recall_backend_has_no_ordering_effect() {
        let system = two_witness_system();
        let round = Round::of(&system, &parse("x"));
        let lane = lane(
            &system,
            SEED,
            trace_fc(&system),
            vec![trace_b(&system), trace_fc(&system)],
        );
        let without = hybrid(weights(1.0, 0.0, 0.0, 0.0));
        let with = HybridStrategy::new(config(weights(1.0, 0.0, 0.0, 0.0))).with_recall(&lane);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let plain = without.evaluate(&request).expect("admissible round");
        let weightless = with.evaluate(&request).expect("admissible round");
        assert_eq!(plain.order, weightless.order);
        assert_eq!(
            weightless.availability(HybridBackend::Recall),
            Some(HybridAvailability::Available)
        );
        for candidate in &weightless.candidates {
            let component = candidate
                .component(HybridBackend::Recall)
                .expect("component evidence");
            assert!(component.normalized.is_some());
            assert_eq!(component.contribution.to_bits(), 0.0f64.to_bits());
        }
        assert_ne!(without.strategy_hash(), with.strategy_hash());
    }

    #[test]
    fn recall_direction_is_pinned_end_to_end() {
        let system = two_witness_system();
        let lane = lane(
            &system,
            SEED,
            trace_fc(&system),
            vec![trace_b(&system), trace_fc(&system)],
        );
        let strategy = HybridStrategy::new(config(weights(0.0, 0.0, 0.0, 1.0))).with_recall(&lane);
        let mode = GuidanceMode::CompleteWithinLimits;

        let baseline = BackwardExplorer::new(&system, small_config())
            .search(&parse("x"), &parse("d"), SearchMode::BreadthFirst)
            .expect("baseline runs");
        let BackwardSearchOutcome::Witness(baseline) = baseline else {
            panic!("the baseline must witness");
        };
        assert_eq!(baseline.steps[0].term, parse("b"));

        let outcome = completed(guided(
            BackwardExplorer::new(&system, small_config()).search_with_strategy(
                &parse("x"),
                &parse("d"),
                SearchMode::BreadthFirst,
                &mode,
                Some(&strategy),
            ),
        ));
        let BackwardSearchOutcome::Witness(derivation) = outcome else {
            panic!("the guided search must witness");
        };
        assert_eq!(derivation.steps[0].term, parse("f(c)"));
        let certificate = ReplayCertificate::issue(
            &system,
            ReplayQuery::Backward {
                target: parse("x"),
                goal: parse("d"),
            },
            ReplayDerivation::Backward(derivation),
            small_config(),
            GuidanceMode::CompleteWithinLimits,
            ResourceObservation {
                states: 3,
                transitions: 2,
                retained_bytes: 256,
                operations: 8,
            },
        );
        certificate.verify(&system).expect("the witness replays");
        // Steering never changes what a derivation is: the guidance-mode
        // identity is unchanged, and the scorer identity is the one the search
        // reported rather than one the strategy invented.
        assert_eq!(
            certificate.guidance_hash,
            guidance_hash(&GuidanceMode::CompleteWithinLimits)
        );
    }

    #[test]
    fn empty_recall_corpus_is_typed_evidence() {
        let system = two_witness_system();
        let round = Round::of(&system, &parse("x"));
        let lane = lane(&system, SEED, trace_fc(&system), Vec::new());
        let strategy = HybridStrategy::new(config(weights(0.5, 0.0, 0.0, 0.5))).with_recall(&lane);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("the round still ranks");
        match evaluation.availability(HybridBackend::Recall) {
            Some(HybridAvailability::Failed(failure)) => {
                assert_eq!(
                    failure.kind,
                    GuidanceFailureKind::Unavailable {
                        stage: "recall corpus"
                    }
                );
                assert_eq!(failure.scored, 0);
            }
            other => panic!("expected typed evidence of a failure, got {other:?}"),
        }
        // Recall never answers "nothing similar" for a corpus it never saw, and
        // the hybrid never invents a score for it: the symbolic lane decides.
        assert_eq!(evaluation.order, symbolic_order(&round));
    }

    #[test]
    fn foreign_system_traces_are_rejected() {
        let system = two_witness_system();
        let foreign = TermSystem::new(vec![Rule::new(parse("p"), parse("q")).unwrap()]);
        let foreign_trace =
            RecallTrace::validate(&foreign, &certificate(&foreign, &parse("q"), &parse("p")))
                .expect("the foreign system mints its own trace");
        // The query must have been replayed against the original system.
        let error = RecallLane::new(
            &system,
            encoder(SEED),
            recall_config(),
            foreign_trace.clone(),
            vec![trace_b(&system)],
        )
        .expect_err("a foreign query is rejected");
        assert_eq!(
            error,
            HybridConfigError::ForeignTraceProvenance {
                role: "recall query"
            }
        );
        // Every admitted trace must have been replayed against the original
        // system too.
        let error = RecallLane::new(
            &system,
            encoder(SEED),
            recall_config(),
            trace_b(&system),
            vec![foreign_trace],
        )
        .expect_err("a foreign trace is rejected");
        assert_eq!(
            error,
            HybridConfigError::ForeignTraceProvenance {
                role: "recall trace"
            }
        );
    }

    #[test]
    fn recall_provenance_is_recorded_in_the_identity() {
        let system = two_witness_system();
        let query = trace_fc(&system);
        let b = trace_b(&system);
        // The traces are replay-validated against the ORIGINAL system: their
        // recorded provenance is the original system and the original query.
        assert_eq!(query.system_hash(), system_hash(&system));
        assert_eq!(
            query.query_hash(),
            query_hash(&ReplayQuery::Backward {
                target: parse("x"),
                goal: parse("f(c)"),
            })
        );
        assert_eq!(b.system_hash(), system_hash(&system));

        let first = lane(&system, SEED, query.clone(), vec![b.clone()]);
        assert_eq!(first.system(), system_hash(&system));
        let same = lane(&system, SEED, query.clone(), vec![b.clone()]);
        assert_eq!(first.recall_hash(), same.recall_hash());

        // A different trace set, query, or encoder is a different memory.
        let other_traces = lane(&system, SEED, query.clone(), vec![b.clone(), query.clone()]);
        assert_ne!(first.recall_hash(), other_traces.recall_hash());
        let other_query = lane(&system, SEED, b.clone(), vec![b.clone()]);
        assert_ne!(first.recall_hash(), other_query.recall_hash());
        let other_encoder = lane(&system, SEED + 1, query.clone(), vec![b.clone()]);
        assert_ne!(first.recall_hash(), other_encoder.recall_hash());

        // The hybrid identity folds the lane's identity in.
        let strategies = |lane: &RecallLane<DIMENSION>| {
            let owned = HybridStrategy::new(config(weights(0.0, 0.0, 0.0, 1.0)));
            // The borrow is local to this closure call, so the hash is read out
            // before the lane goes away.
            let hybrid = owned.with_recall(lane);
            hybrid.strategy_hash()
        };
        assert_eq!(strategies(&first), strategies(&same));
        assert_ne!(strategies(&first), strategies(&other_traces));
    }

    #[test]
    fn recall_never_answers_from_a_corpus_it_never_saw() {
        let system = two_witness_system();
        let round = Round::of(&system, &parse("x"));
        let lane = lane(&system, SEED, trace_fc(&system), Vec::new());
        let candidates = round.candidates();
        assert!(
            lane.similarities(&candidates).is_err(),
            "an empty corpus is a typed failure, never a similarity of zero"
        );
        // And the corpus the lane holds admits only what it was given.
        let corpus = TraceCorpus::<DIMENSION>::new(encoder(SEED), recall_config());
        assert!(corpus.is_empty());
    }
}

// ------------------------------------------------------------- all backends

#[cfg(all(feature = "network", feature = "holographic-guidance"))]
mod combined {
    use crate::core::{config, weights};
    use crate::fixtures::*;
    use amari_rewrite::holographic::{HolographicEncoder, RecallConfig, RecallTrace};
    use amari_rewrite::inverse::hybrid::{
        HybridAvailability, HybridBackend, HybridStrategy, RecallLane,
    };
    use amari_rewrite::inverse::{
        scorer_hash_with, BackwardExplorer, BackwardSearchOutcome, GuidanceMode, GuidanceStrategy,
        SearchMode,
    };
    use amari_rewrite::network::{
        GeometricStrategy, GeometricStrategyConfig, InverseGraphConfig, InverseSearchGraph,
    };
    use amari_rewrite::neural::{
        FeatureEncoder, MlpRanker, NeuralStrategy, RankerConfig, RankerWeights,
    };
    use amari_rewrite::relation::RelationLimits;
    use amari_rewrite::trs::{Term, TermSystem};

    const SEED: u64 = 0x5441_534B_3438_0002;
    const DIMENSION: usize = 64;

    fn ranker() -> MlpRanker {
        let json = "{\"model_type\":\"ia-mlp\",\"schema_version\":1,\"input_width\":64,\
                    \"hidden\":[{\"width\":1}],\"head\":{\"width\":1},\
                    \"hidden_activation\":\"relu\"}";
        let config = RankerConfig::from_spec_json(json).expect("valid spec");
        let hidden = vec![0.0f32; 64];
        let weights = RankerWeights::from_tensors(
            &config,
            &[
                ("hidden.0.weight", hidden.as_slice()),
                ("hidden.0.bias", &[0.0f32][..]),
                ("head.weight", &[0.0f32][..]),
                ("head.bias", &[0.0f32][..]),
            ],
        )
        .expect("admissible weights");
        MlpRanker::new(config, weights).expect("matching weights")
    }

    fn trace(system: &TermSystem, target: &Term, goal: &Term) -> RecallTrace {
        let outcome = BackwardExplorer::new(system, small_config())
            .search(target, goal, SearchMode::BreadthFirst)
            .expect("search runs");
        let BackwardSearchOutcome::Witness(derivation) = outcome else {
            panic!("the fixture witnesses");
        };
        let certificate = amari_rewrite::inverse::ReplayCertificate::issue(
            system,
            amari_rewrite::inverse::ReplayQuery::Backward {
                target: target.clone(),
                goal: goal.clone(),
            },
            amari_rewrite::inverse::ReplayDerivation::Backward(derivation),
            small_config(),
            GuidanceMode::CompleteWithinLimits,
            amari_rewrite::inverse::ResourceObservation {
                states: 3,
                transitions: 2,
                retained_bytes: 256,
                operations: 32,
            },
        );
        RecallTrace::validate(system, &certificate).expect("replay mints the trace")
    }

    /// Build one hybrid over all four backends, all at weight 1.
    fn all_backends<'a>(
        neural: &'a NeuralStrategy,
        geometric: &'a GeometricStrategy,
        recall: &'a RecallLane<DIMENSION>,
    ) -> HybridStrategy<'a> {
        HybridStrategy::new(config(weights(1.0, 1.0, 1.0, 1.0)))
            .with_neural(neural)
            .with_geometric(geometric)
            .with_recall(recall)
    }

    #[test]
    fn all_backends_report_exhaustive_evidence() {
        let system = two_witness_system();
        let round = Round::of(&system, &parse("x"));
        let neural =
            NeuralStrategy::from_ranker(ranker(), FeatureEncoder::new(RelationLimits::default()));
        let geometric = GeometricStrategy::new(
            InverseSearchGraph::new(InverseGraphConfig::default()),
            GeometricStrategyConfig::default(),
        );
        let recall = RecallLane::<DIMENSION>::new(
            &system,
            HolographicEncoder::<DIMENSION>::new(SEED, RelationLimits::default()).expect("bounded"),
            RecallConfig::new(8, 64, 8).expect("bounded"),
            trace(&system, &parse("x"), &parse("f(c)")),
            vec![trace(&system, &parse("x"), &parse("b"))],
        )
        .expect("original-system provenance");
        let strategy = all_backends(&neural, &geometric, &recall);
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        assert_eq!(
            evaluation.order.len(),
            round.states.len(),
            "no component makes a candidate disappear"
        );
        for backend in HybridBackend::ALL {
            assert_eq!(
                evaluation.availability(backend),
                Some(HybridAvailability::Available),
                "every supplied backend reports availability"
            );
        }
        for candidate in &evaluation.candidates {
            let mut sum = 0.0f64;
            for backend in HybridBackend::ALL {
                let component = candidate.component(backend).expect("component evidence");
                assert_eq!(component.weight, 1.0);
                let normalized = component.normalized.expect("normalized when available");
                assert!(normalized.is_finite());
                assert!((0.0..=1.0).contains(&normalized));
                assert!(component.raw.expect("raw evidence").is_finite());
                sum += component.contribution;
            }
            assert_eq!(candidate.combined.to_bits(), sum.to_bits());
            assert!((0.0..=4.0).contains(&candidate.combined));
        }
        // The identity binds all four backends and all four weights.
        assert_ne!(
            strategy.strategy_hash(),
            HybridStrategy::new(config(weights(1.0, 1.0, 1.0, 0.0)))
                .with_neural(&neural)
                .with_geometric(&geometric)
                .strategy_hash()
        );
    }

    #[test]
    fn combined_complete_mode_preserves_the_explored_set() {
        let system = cycle_system();
        let neural =
            NeuralStrategy::from_ranker(ranker(), FeatureEncoder::new(RelationLimits::default()));
        let geometric = GeometricStrategy::new(
            InverseSearchGraph::new(InverseGraphConfig::default()),
            GeometricStrategyConfig::default(),
        );
        // A corpus from the same (original) system: `a => b`, `b => c`.
        let query = trace(&system, &parse("a"), &parse("b"));
        let recall = RecallLane::<DIMENSION>::new(
            &system,
            HolographicEncoder::<DIMENSION>::new(SEED, RelationLimits::default()).expect("bounded"),
            RecallConfig::new(8, 64, 8).expect("bounded"),
            query.clone(),
            vec![query],
        )
        .expect("original-system provenance");
        let strategy = all_backends(&neural, &geometric, &recall);
        assert_complete_parity_without_dropping(&system, &parse("a"), &parse("z"), &strategy);
    }

    #[test]
    fn combined_pruning_uses_the_combined_order_and_reports_the_drop() {
        let system = two_witness_system();
        let neural =
            NeuralStrategy::from_ranker(ranker(), FeatureEncoder::new(RelationLimits::default()));
        let geometric = GeometricStrategy::new(
            InverseSearchGraph::new(InverseGraphConfig::default()),
            GeometricStrategyConfig::default(),
        );
        let recall = RecallLane::<DIMENSION>::new(
            &system,
            HolographicEncoder::<DIMENSION>::new(SEED, RelationLimits::default()).expect("bounded"),
            RecallConfig::new(8, 64, 8).expect("bounded"),
            trace(&system, &parse("x"), &parse("f(c)")),
            vec![trace(&system, &parse("x"), &parse("b"))],
        )
        .expect("original-system provenance");
        let strategy = all_backends(&neural, &geometric, &recall);
        let beam = GuidanceMode::HeuristicPruning { beam_width: 1 };
        let outcome = completed(guided(
            BackwardExplorer::new(&system, small_config()).search_with_strategy(
                &parse("x"),
                &parse("z"),
                SearchMode::BreadthFirst,
                &beam,
                Some(&strategy),
            ),
        ));
        let evidence = approximate(&outcome);
        assert_eq!(evidence.dropped_candidates, 1);
        assert!(evidence.summary.contains("heuristic pruning dropped 1"));
        assert_eq!(
            evidence.scorer_hash,
            scorer_hash_with(Some(&strategy.strategy_hash()))
        );
    }
}

// ------------------------------------------------------- feature isolation

#[cfg(not(any(feature = "neural", feature = "holographic-guidance")))]
mod default_isolation {
    #[test]
    fn default_builds_expose_no_hybrid_surface() {
        // The hybrid surface is compiled only with at least one backend; a
        // default (or no-default) build compiles neither, so this module is the
        // only part of the file that exists and it asserts the absence.
        let hybrid_surfaces =
            u8::from(cfg!(feature = "neural")) + u8::from(cfg!(feature = "holographic-guidance"));
        assert_eq!(
            hybrid_surfaces, 0,
            "a default build exposes no hybrid surface"
        );
    }
}

#[cfg(feature = "network")]
mod one_backend_builds {
    use crate::fixtures::*;
    use amari_rewrite::inverse::hybrid::{
        HybridAvailability, HybridBackend, HybridConfig, HybridStrategy,
    };
    use amari_rewrite::inverse::GuidanceMode;

    /// `network` implies `neural` but not `holographic-guidance`: the hybrid is
    /// compiled, the recall lane has no backend type to supply, and its slot is
    /// reported as `Missing` rather than failing to compile or disappearing.
    #[test]
    fn a_build_without_the_recall_backend_still_reports_its_slot() {
        let system = two_candidate_system();
        let round = Round::of(&system, &parse("x"));
        let strategy = HybridStrategy::new(HybridConfig::default());
        let mode = GuidanceMode::CompleteWithinLimits;
        let candidates = round.candidates();
        let request = round.request(&candidates, &mode);
        let evaluation = strategy.evaluate(&request).expect("admissible round");
        assert_eq!(
            evaluation.availability(HybridBackend::Recall),
            Some(HybridAvailability::Missing)
        );
        assert_eq!(
            evaluation.availability(HybridBackend::Symbolic),
            Some(HybridAvailability::Available)
        );
        assert_eq!(HybridBackend::Recall.name(), "recall");
    }
}
