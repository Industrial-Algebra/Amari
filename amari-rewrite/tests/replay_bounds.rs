// SPDX-License-Identifier: MIT OR Apache-2.0

//! Replay-generated state bounds (Task 47, PR #384 round 3).
//!
//! A certificate's *borrowed* terms are admitted by
//! [`amari_rewrite::holographic::RecallTrace::validate`], but a replay also
//! GENERATES states: every recorded step is re-applied through the system, so
//! the intermediate terms are derived, not borrowed. Those generated states are
//! metered here:
//!
//! - **F12a** — each derivation half's step count is validated against the
//!   embedded configuration's ceilings *before* the half is replayed. Ten
//!   thousand alternating forward steps can never come from a search whose
//!   depth ceiling is 64, so the certificate is rejected before the replay
//!   walks it. A derivation with exactly the embedded depth ceiling is still
//!   admitted (near-boundary positive control);
//! - **F12b** — every generated/instantiated/rewritten state's node and depth
//!   shape is preflighted before it is allocated, including the matcher RHS
//!   application and the rebuild at the recorded position. A forward rule whose
//!   RHS has 5,001 nodes is rejected before the intermediate is materialized,
//!   even though both borrowed endpoints are constants. A near-boundary wide
//!   intermediate (4,095 nodes) is still admitted.
//!
//! Round 4 adds the producer-derived bounds: an embedded configuration is the
//! only witness authority a certificate has, so each derivation half is replayed
//! only when the *producing search* could have run inside it.
//!
//! - **F17** — the bounded preflight counts TERM frames, not bookkeeping
//!   frames. A substitution application whose expansion has exactly the node
//!   ceiling is admissible; one node more is not. The boundary is pinned on the
//!   zero-step bidirectional path, where the only work is a checked
//!   substitution application of the recorded meeting unifier.
//! - **F18** — retained-state lower bounds are derived PER PRODUCER KIND. A
//!   backward witness retains the target root plus its non-winning
//!   predecessors (a goal-matching leaf is checked before it is retained), so
//!   `n` steps retain `n` states. A bidirectional witness retains both roots
//!   plus the chains up to the meeting *and* the meeting state one side
//!   reached, so `n` total steps retain `n + 1`. Real searches are run at the
//!   exact ceiling and one below.
//! - **F20** — the embedded `max_operations` is checked against the producer's
//!   minimum expansion cost (one expansion per edge of each half), separately
//!   from the replay's own metered term-inspection work.

use amari_rewrite::inverse::{
    BackwardDerivation, BackwardExplorer, BackwardSearchOutcome, BidirectionalDerivation,
    BidirectionalExplorer, BidirectionalSearchOutcome, ForwardStep, GuidanceMode,
    InverseSearchConfig, ReplayCertificate, ReplayDerivation, ReplayQuery, ResourceObservation,
    SearchMode,
};
use amari_rewrite::relation::{RelationLimits, RuleId};
use amari_rewrite::trs::{Rule, Substitution, Term, TermSystem};
use amari_rewrite::Path;

fn observation() -> ResourceObservation {
    ResourceObservation {
        states: 4,
        transitions: 2,
        retained_bytes: 256,
        operations: 64,
    }
}

fn certificate(
    system: &TermSystem,
    source: Term,
    goal: Term,
    derivation: BidirectionalDerivation,
) -> ReplayCertificate {
    ReplayCertificate::issue(
        system,
        ReplayQuery::Bidirectional { source, goal },
        ReplayDerivation::Bidirectional(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    )
}

/// A recorded root forward step under `rule`.
fn step(rule: &Rule) -> ForwardStep {
    ForwardStep {
        rule_id: RuleId::from_rule(rule),
        position: Path::root(),
        source_hash: amari_rewrite::relation::Sha256Digest::framed(
            "amari.relation.term/v1",
            b"recorded-step",
        ),
        result_hash: amari_rewrite::relation::Sha256Digest::framed(
            "amari.relation.term/v1",
            b"recorded-step",
        ),
    }
}

/// F12a (P2): a derivation half longer than the embedded depth ceiling is
/// rejected before it is replayed.
///
/// Rules `a -> b` and `b -> a`, source `a`, goal `a`: ten thousand alternating
/// root steps replay successfully (and were admitted pre-remediation) although
/// no search under `max_depth = 64` can produce a 10,000-step half.
#[test]
fn replay_rejects_a_derivation_deeper_than_the_embedded_ceiling() {
    let ab = Rule::new(Term::constant("a"), Term::constant("b")).expect("checked rule");
    let ba = Rule::new(Term::constant("b"), Term::constant("a")).expect("checked rule");
    let system = TermSystem::new(vec![ab.clone(), ba.clone()]);
    let forward_steps: Vec<ForwardStep> = (0..10_000)
        .map(|index| if index % 2 == 0 { step(&ab) } else { step(&ba) })
        .collect();
    let certificate = certificate(
        &system,
        Term::constant("a"),
        Term::constant("a"),
        BidirectionalDerivation {
            forward_steps,
            backward_steps: Vec::new(),
            meeting_forward: Term::constant("a"),
            meeting_backward: Term::constant("a"),
            unifier: Substitution::new(),
        },
    );
    let error = certificate
        .verify(&system)
        .expect_err("a half deeper than the embedded ceiling must not verify");
    assert!(
        matches!(
            error,
            amari_rewrite::RewriteError::RelationLimitExceeded {
                resource: "replay forward steps",
                ..
            }
        ),
        "the typed rejection names the forward step ceiling, got {error}"
    );
}

/// F12a positive control: exactly the embedded depth ceiling verifies.
#[test]
fn replay_admits_a_derivation_at_the_embedded_depth_ceiling() {
    let ab = Rule::new(Term::constant("a"), Term::constant("b")).expect("checked rule");
    let ba = Rule::new(Term::constant("b"), Term::constant("a")).expect("checked rule");
    let system = TermSystem::new(vec![ab.clone(), ba.clone()]);
    let depth = InverseSearchConfig::default().max_depth() as usize;
    assert_eq!(depth, 64, "the control is the exact embedded ceiling");
    let forward_steps: Vec<ForwardStep> = (0..depth)
        .map(|index| if index % 2 == 0 { step(&ab) } else { step(&ba) })
        .collect();
    let certificate = certificate(
        &system,
        Term::constant("a"),
        Term::constant("a"),
        BidirectionalDerivation {
            forward_steps,
            backward_steps: Vec::new(),
            meeting_forward: Term::constant("a"),
            meeting_backward: Term::constant("a"),
            unifier: Substitution::new(),
        },
    );
    certificate
        .verify(&system)
        .expect("a half at the embedded depth ceiling must verify");
}

/// A resource observation that fits `config`. The observed values are search
/// metadata; these boundary tests pin the DERIVED lower bounds, so the
/// observation must not trip the (independent) recorded-value ceilings first.
fn observation_for(config: &InverseSearchConfig) -> ResourceObservation {
    ResourceObservation {
        states: config.max_states().min(4),
        transitions: config.max_transitions().min(2),
        retained_bytes: config.max_frontier_bytes().min(256),
        operations: config.max_operations().min(64),
    }
}

/// A certificate under an explicitly supplied (validated) configuration.
fn certificate_with(
    system: &TermSystem,
    source: Term,
    goal: Term,
    derivation: BidirectionalDerivation,
    config: InverseSearchConfig,
) -> ReplayCertificate {
    ReplayCertificate::issue(
        system,
        ReplayQuery::Bidirectional { source, goal },
        ReplayDerivation::Bidirectional(derivation),
        config,
        GuidanceMode::CompleteWithinLimits,
        observation_for(&config),
    )
}

/// The default configuration with only `max_states` tightened.
fn states_config(states: u64) -> InverseSearchConfig {
    let defaults = InverseSearchConfig::default();
    InverseSearchConfig::new(
        defaults.max_depth(),
        states,
        defaults.max_transitions(),
        defaults.max_term_nodes(),
        defaults.max_term_depth(),
        defaults.max_constraints(),
        defaults.max_groundings(),
        defaults.max_operations(),
        defaults.max_frontier_bytes(),
        defaults.max_trace_bytes(),
    )
    .expect("a tightened retained-state ceiling is valid")
}

/// The default configuration with only `max_operations` tightened.
fn operations_config(operations: u64) -> InverseSearchConfig {
    let defaults = InverseSearchConfig::default();
    InverseSearchConfig::new(
        defaults.max_depth(),
        defaults.max_states(),
        defaults.max_transitions(),
        defaults.max_term_nodes(),
        defaults.max_term_depth(),
        defaults.max_constraints(),
        defaults.max_groundings(),
        operations,
        defaults.max_frontier_bytes(),
        defaults.max_trace_bytes(),
    )
    .expect("a tightened operation ceiling is valid")
}

/// A zero-step bidirectional certificate whose recorded meeting unifier binds
/// `X` to an `f`-headed range with `children` constant children.
fn zero_step_substitution_certificate(children: usize) -> (TermSystem, ReplayCertificate) {
    let system = TermSystem::new(Vec::new());
    let meeting = Term::var("X");
    let range = Term::sym("f", (0..children).map(|_| Term::constant("a")));
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: meeting.clone(),
            goal: meeting.clone(),
        },
        ReplayDerivation::Bidirectional(BidirectionalDerivation {
            forward_steps: Vec::new(),
            backward_steps: Vec::new(),
            meeting_forward: meeting.clone(),
            meeting_backward: meeting,
            unifier: Substitution::new().with("X", range),
        }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    (system, certificate)
}

/// F17 (P1): the preflight counts term frames, not cleanup frames.
///
/// `{X := f(a, ..., a)}` with 4,095 children applied to `X` is exactly 4,096
/// nodes at depth 2 — the ceiling, and therefore admissible. Counting the
/// bookkeeping frame that pops the resolved variable as another output node
/// rejected the boundary case at 4,097.
#[test]
fn replay_admits_a_substitution_application_at_the_exact_node_ceiling() {
    let children = RelationLimits::MAX_TERM_NODES - 1;
    let (system, certificate) = zero_step_substitution_certificate(children);
    certificate
        .verify(&system)
        .expect("an expansion of exactly the node ceiling must verify");
}

/// F17 negative control: one child more is one node over the ceiling.
#[test]
fn replay_rejects_a_substitution_application_one_node_over_the_ceiling() {
    let children = RelationLimits::MAX_TERM_NODES;
    let (system, certificate) = zero_step_substitution_certificate(children);
    let error = certificate
        .verify(&system)
        .expect_err("an expansion one node over the ceiling must not verify");
    assert!(
        matches!(
            error,
            amari_rewrite::RewriteError::RelationLimitExceeded {
                resource: "term nodes",
                limit,
            } if limit == RelationLimits::MAX_TERM_NODES
        ),
        "the typed rejection is the term node ceiling, got {error}"
    );
}

/// Rules `a -> b` and `b -> c`.
fn chain_system() -> TermSystem {
    TermSystem::new(vec![
        Rule::new(Term::constant("a"), Term::constant("b")).expect("checked rule"),
        Rule::new(Term::constant("b"), Term::constant("c")).expect("checked rule"),
    ])
}

/// F18 (P1): a backward witness retains `n` states for `n` steps, not `n + 1`.
///
/// Rule `a -> b`, target `b`, goal `a`, `max_states = 1`. The producer retains
/// the target root and returns the goal-matching child `a` BEFORE retaining it,
/// so a real one-step witness exists at exactly one retained state. The old
/// `steps + 1` formula demanded two and rejected a witness the search really
/// produced.
#[test]
fn a_backward_witness_at_the_exact_retained_state_ceiling_verifies() {
    let system = TermSystem::new(vec![
        Rule::new(Term::constant("a"), Term::constant("b")).expect("checked rule")
    ]);
    let config = states_config(1);
    let outcome = BackwardExplorer::new(&system, config)
        .search(
            &Term::constant("b"),
            &Term::constant("a"),
            SearchMode::BreadthFirst,
        )
        .expect("search must run");
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("one retained state is enough for a one-step witness, got {outcome:?}");
    };
    assert_eq!(derivation.steps.len(), 1, "the witness walks one edge");
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward {
            target: Term::constant("b"),
            goal: Term::constant("a"),
        },
        ReplayDerivation::Backward(derivation),
        config,
        GuidanceMode::CompleteWithinLimits,
        observation_for(&config),
    );
    certificate
        .verify(&system)
        .expect("a witness that retains exactly max_states must verify");
}

/// F18 boundary, one below: a two-step backward witness needs exactly two
/// retained states.
///
/// Target `c`, goal `a`: the producer retains `c` and `b` and returns the
/// goal-matching `a` before retaining it, so the witness exists at exactly two
/// states and cannot exist at one. The certificate's own embedded ceiling is
/// what admission reads, so the same derivation is rejected at one state.
#[test]
fn a_two_step_backward_witness_needs_two_retained_states() {
    let system = chain_system();
    let config = states_config(2);
    let outcome = BackwardExplorer::new(&system, config)
        .search(
            &Term::constant("c"),
            &Term::constant("a"),
            SearchMode::BreadthFirst,
        )
        .expect("search must run");
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("two retained states are enough for a two-step witness, got {outcome:?}");
    };
    assert_eq!(derivation.steps.len(), 2, "the witness walks two edges");
    let issue = |config: InverseSearchConfig, derivation: BackwardDerivation| {
        ReplayCertificate::issue(
            &system,
            ReplayQuery::Backward {
                target: Term::constant("c"),
                goal: Term::constant("a"),
            },
            ReplayDerivation::Backward(derivation),
            config,
            GuidanceMode::CompleteWithinLimits,
            observation_for(&config),
        )
    };
    issue(config, derivation.clone())
        .verify(&system)
        .expect("a two-step witness at exactly two retained states must verify");
    let below = states_config(1);
    let error = issue(below, derivation)
        .verify(&system)
        .expect_err("a two-step witness cannot come from one retained state");
    assert!(
        matches!(
            error,
            amari_rewrite::RewriteError::RelationLimitExceeded {
                resource: "replay states",
                limit: 1,
            }
        ),
        "the typed rejection is the retained-state ceiling, got {error}"
    );
    // The ceiling is not conservative: the producer genuinely cannot retain the
    // intermediate state, so it truncates instead of witnessing.
    let outcome = BackwardExplorer::new(&system, below)
        .search(
            &Term::constant("c"),
            &Term::constant("a"),
            SearchMode::BreadthFirst,
        )
        .expect("search must run");
    assert!(
        matches!(outcome, BackwardSearchOutcome::Partial(_)),
        "one retained state truncates the two-step search, got {outcome:?}"
    );
}

/// F18 per-kind accounting: a bidirectional witness retains `n + 1` states for
/// `n` total steps.
///
/// Rules `a -> b` and `b -> c`, source `a`, goal `c`. The producer retains the
/// source, the goal, and the forward meeting state `b` (the backward half's
/// winning child is checked before it is retained), so the witness exists at
/// exactly three states and truncates at two. A blanket `steps + 1` formula
/// applied to the backward kind would have conflated the two producers.
#[test]
fn a_bidirectional_witness_needs_one_more_state_than_its_steps() {
    let system = chain_system();
    let exact = states_config(3);
    let outcome = BidirectionalExplorer::new(&system, exact)
        .search(&Term::constant("a"), &Term::constant("c"))
        .expect("search must run");
    let BidirectionalSearchOutcome::Witness(derivation) = outcome else {
        panic!("three retained states are enough for a two-step meet, got {outcome:?}");
    };
    assert_eq!(
        derivation.forward_steps.len() + derivation.backward_steps.len(),
        2,
        "the meet is two steps from the source"
    );
    let issue = |config: InverseSearchConfig, derivation: BidirectionalDerivation| {
        certificate_with(
            &system,
            Term::constant("a"),
            Term::constant("c"),
            derivation,
            config,
        )
    };
    issue(exact, derivation.clone())
        .verify(&system)
        .expect("a bidirectional witness at exactly n + 1 states must verify");
    let below = states_config(2);
    let error = issue(below, derivation)
        .verify(&system)
        .expect_err("a two-step meet cannot come from two retained states");
    assert!(
        matches!(
            error,
            amari_rewrite::RewriteError::RelationLimitExceeded {
                resource: "replay states",
                limit: 2,
            }
        ),
        "the typed rejection is the retained-state ceiling, got {error}"
    );
    let outcome = BidirectionalExplorer::new(&system, below)
        .search(&Term::constant("a"), &Term::constant("c"))
        .expect("search must run");
    assert!(
        matches!(outcome, BidirectionalSearchOutcome::Partial(_)),
        "two retained states truncate the two-step meet, got {outcome:?}"
    );
}

/// F20 (P2): the embedded `max_operations` bounds the producer's expansion
/// cost, not just the replay's metered inspection.
///
/// Two root forward steps `a -> b -> c`, source `a`, goal and meetings `c`, no
/// backward steps. Producing that half requires expanding two nodes, so a
/// certificate carrying `max_operations = 1` describes a search that could not
/// have run. The exact ceiling (two) still verifies, and the replay's own
/// term-inspection meter is a separate budget that is unaffected.
#[test]
fn replay_rejects_a_derivation_cheaper_than_its_producer_minimum() {
    let system = chain_system();
    let ab = system.rules()[0].clone();
    let bc = system.rules()[1].clone();
    let derivation = BidirectionalDerivation {
        forward_steps: vec![step(&ab), step(&bc)],
        backward_steps: Vec::new(),
        meeting_forward: Term::constant("c"),
        meeting_backward: Term::constant("c"),
        unifier: Substitution::new(),
    };
    let exact = operations_config(2);
    certificate_with(
        &system,
        Term::constant("a"),
        Term::constant("c"),
        derivation.clone(),
        exact,
    )
    .verify(&system)
    .expect("a two-edge half at the exact operation ceiling must verify");
    let below = operations_config(1);
    let error = certificate_with(
        &system,
        Term::constant("a"),
        Term::constant("c"),
        derivation,
        below,
    )
    .verify(&system)
    .expect_err("one operation cannot expand two edges");
    assert!(
        matches!(
            error,
            amari_rewrite::RewriteError::RelationLimitExceeded {
                resource: "replay operations",
                limit: 1,
            }
        ),
        "the typed rejection is the producer operation ceiling, got {error}"
    );
    // Producer evidence: the real search produces exactly this two-forward-step
    // meet. Its expansion meter is coupled to the same ceiling through the
    // per-check relation budget, so the smallest configuration in which the
    // search can run at all is the observable threshold; it is never below the
    // two expansions admission demands. The measured threshold is 12: the two
    // expansions themselves plus the per-check unification admission, one
    // charged enqueue per equation, and each resolve walk, all drawn from the
    // same `max_operations` pool (F16, PR #384 round 4; F24, PR #384 round 5).
    let outcome = BidirectionalExplorer::new(&system, operations_config(12))
        .search(&Term::constant("a"), &Term::constant("c"))
        .expect("search must run");
    let BidirectionalSearchOutcome::Witness(real) = outcome else {
        panic!("the chain must meet, got {outcome:?}");
    };
    assert_eq!(
        real.forward_steps.len() + real.backward_steps.len(),
        2,
        "the meet is two edges from the source"
    );
    certificate_with(
        &system,
        Term::constant("a"),
        Term::constant("c"),
        real,
        operations_config(12),
    )
    .verify(&system)
    .expect("the real witness must verify under its own configuration");
}

/// Rules `a -> wide(b, ..., b)` and `wide(X1, ..., Xn) -> a` with `n` children.
fn wide_rules(children: usize) -> (TermSystem, Rule, Rule) {
    let wide = |head: &str| Term::sym(head, (0..children).map(|_| Term::constant("b")));
    let pattern = Term::sym(
        "wide",
        (0..children).map(|index| Term::var(format!("X{index}"))),
    );
    let up = Rule::new(Term::constant("a"), wide("wide")).expect("checked rule");
    let down = Rule::new(pattern, Term::constant("a")).expect("checked rule");
    let system = TermSystem::new(vec![up.clone(), down.clone()]);
    (system, up, down)
}

/// F12b (P2): a generated intermediate larger than the term ceiling is
/// rejected before it is materialized.
///
/// Rules `a -> wide(b x 5000)` and `wide(X1, ..., X5000) -> a`, source `a`,
/// goal `a`. Both borrowed endpoints are constants, but the forward half
/// rewrites `a` into a 5,001-node intermediate: generated content, so it is
/// bounded by the same node ceiling as any other replay state.
#[test]
fn replay_rejects_an_over_limit_generated_intermediate() {
    let (system, up, down) = wide_rules(5_000);
    let certificate = certificate(
        &system,
        Term::constant("a"),
        Term::constant("a"),
        BidirectionalDerivation {
            forward_steps: vec![step(&up), step(&down)],
            backward_steps: Vec::new(),
            meeting_forward: Term::constant("a"),
            meeting_backward: Term::constant("a"),
            unifier: Substitution::new(),
        },
    );
    let error = certificate
        .verify(&system)
        .expect_err("an over-limit generated intermediate must not verify");
    assert!(
        matches!(
            error,
            amari_rewrite::RewriteError::RelationLimitExceeded {
                resource: "term nodes",
                limit,
            } if limit == RelationLimits::MAX_TERM_NODES
        ),
        "the typed rejection is the term node ceiling, got {error}"
    );
}

/// F12b positive control: a near-boundary wide intermediate verifies.
///
/// The generated intermediate has 4,095 nodes (4,094 children plus its own
/// node), one node below the 4,096 ceiling.
#[test]
fn replay_admits_a_near_boundary_generated_intermediate() {
    let children = RelationLimits::MAX_TERM_NODES - 2;
    let (system, up, down) = wide_rules(children);
    let certificate = certificate(
        &system,
        Term::constant("a"),
        Term::constant("a"),
        BidirectionalDerivation {
            forward_steps: vec![step(&up), step(&down)],
            backward_steps: Vec::new(),
            meeting_forward: Term::constant("a"),
            meeting_backward: Term::constant("a"),
            unifier: Substitution::new(),
        },
    );
    certificate
        .verify(&system)
        .expect("a near-boundary generated intermediate must verify");
}

/// F29 (P2): the exact node ceiling is still admissible when the expansion runs
/// through a SUBSTITUTED variable.
///
/// `{X := f(Y, a x 4094), Y := b}` expands to `f(b, a x 4094)` — 4,096 nodes at
/// depth 2, exactly the ceiling. Counting every pending Enter frame as one
/// inevitable output node keeps this admissible: the inner `Y` frame resolves to
/// the one-node range `b`, so the bound never exceeds the true node count.
#[test]
fn replay_admits_a_chained_application_at_the_exact_node_ceiling() {
    let system = TermSystem::new(Vec::new());
    let meeting = Term::var("X");
    let children = RelationLimits::MAX_TERM_NODES - 2;
    let range = Term::sym(
        "f",
        core::iter::once(Term::var("Y")).chain((0..children).map(|_| Term::constant("a"))),
    );
    let unifier = Substitution::new()
        .with("X", range)
        .with("Y", Term::constant("b"));
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: meeting.clone(),
            goal: meeting.clone(),
        },
        ReplayDerivation::Bidirectional(BidirectionalDerivation {
            forward_steps: Vec::new(),
            backward_steps: Vec::new(),
            meeting_forward: meeting.clone(),
            meeting_backward: meeting,
            unifier,
        }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    certificate
        .verify(&system)
        .expect("an expansion of exactly the node ceiling must verify");
}

/// F29 (P2) negative boundary: one child more is one node over the ceiling.
#[test]
fn replay_rejects_a_chained_application_one_node_over_the_ceiling() {
    let system = TermSystem::new(Vec::new());
    let meeting = Term::var("X");
    let children = RelationLimits::MAX_TERM_NODES - 1;
    let range = Term::sym(
        "f",
        core::iter::once(Term::var("Y")).chain((0..children).map(|_| Term::constant("a"))),
    );
    let unifier = Substitution::new()
        .with("X", range)
        .with("Y", Term::constant("b"));
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: meeting.clone(),
            goal: meeting.clone(),
        },
        ReplayDerivation::Bidirectional(BidirectionalDerivation {
            forward_steps: Vec::new(),
            backward_steps: Vec::new(),
            meeting_forward: meeting.clone(),
            meeting_backward: meeting,
            unifier,
        }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    let error = certificate
        .verify(&system)
        .expect_err("an expansion one node over the ceiling must not verify");
    assert!(
        matches!(
            error,
            amari_rewrite::RewriteError::RelationLimitExceeded {
                resource: "term nodes",
                limit,
            } if limit == RelationLimits::MAX_TERM_NODES
        ),
        "the typed rejection is the term node ceiling, got {error}"
    );
}
