// SPDX-License-Identifier: MIT OR Apache-2.0

//! Whole-trace replay consistency contracts (Task 47, PR #384 round 3).
//!
//! Two acceptance invariants are pinned here, each as the reviewer's
//! reproduction plus the positive control it must not break:
//!
//! - **F9** — a backward replay has ONE trace-wide free-variable
//!   correspondence, anchored by the leaf/goal unifier and extended through
//!   every recorded step term and the declared target. Comparing recorded terms
//!   under an independent per-term alpha-canonicalization admits a trace whose
//!   middle term is an alpha-variant of the replayed state but whose variables
//!   denote *different* instances than the anchored goal does. The relation is
//!   alpha equivalence (one consistent bijective free-variable renaming), never
//!   an arbitrary per-record instance match;
//! - **F10** — the two halves of a bidirectional derivation must actually meet.
//!   The recorded unifier is validated on BOTH meeting terms *and* on the
//!   actual forward endpoint, and the backward replay starts from that one
//!   checked common instantiated meeting. Two unrelated caller-supplied meeting
//!   terms with an empty unifier are not a meeting.

use amari_rewrite::inverse::{
    BackwardDerivation, BackwardExplorer, BackwardProvenance, BackwardSearchOutcome,
    BidirectionalDerivation, BidirectionalExplorer, BidirectionalSearchOutcome, GuidanceMode,
    InverseSearchConfig, ReplayCertificate, ReplayDerivation, ReplayQuery, ResourceObservation,
    SearchMode, SymbolicPredecessor,
};
use amari_rewrite::relation::{ConstraintSet, RelationLimits, RelationResources, RuleId};
use amari_rewrite::trs::{Rule, Substitution, Term, TermSystem};
use amari_rewrite::Path;

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

fn observation() -> ResourceObservation {
    ResourceObservation {
        states: 4,
        transitions: 2,
        retained_bytes: 256,
        operations: 64,
    }
}

/// A recorded backward step at the root position. Provenance hashes are the
/// canonical digests of the terms involved; the replay never trusts them.
fn recorded(term: Term, rule_id: RuleId, target: &Term, scope: u32) -> SymbolicPredecessor {
    SymbolicPredecessor {
        provenance: BackwardProvenance {
            rule_id,
            position: Path::root(),
            scope,
            target_hash: amari_rewrite::relation::Sha256Digest::canonical_term(
                "amari.relation.term/v1",
                target,
            ),
            predecessor_hash: amari_rewrite::relation::Sha256Digest::canonical_term(
                "amari.relation.term/v1",
                &term,
            ),
        },
        term,
        existentials: Vec::new(),
        constraints: ConstraintSet::new(),
        substitution: Substitution::new(),
        resources: RelationResources::new(&RelationLimits::default()),
    }
}

fn backward_certificate(
    rules: Vec<Rule>,
    target: Term,
    goal: Term,
    steps: Vec<SymbolicPredecessor>,
) -> (TermSystem, ReplayCertificate) {
    let system = TermSystem::new(rules);
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward { target, goal },
        ReplayDerivation::Backward(BackwardDerivation { steps }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    (system, certificate)
}

/// F9 (P1): a backward replay has ONE whole-trace free-variable
/// correspondence, anchored by the leaf/goal unifier.
///
/// Rules `f(U,V) -> g(U,V)` and `g(U,V) -> h(U,V)`; goal `f(X,Y)`, target
/// `h(X,Y)`; the recorded backward half is `[g(Y,X), f(X,Y)]` (goal side last,
/// as a replay walks it). The leaf/goal unifier is empty, so the anchor fixes
/// `X` and `Y`: the middle record `g(Y,X)` is an alpha-variant of the replayed
/// state `g(X,Y)` but its `Y` is the anchor's `X`, and independent
/// canonicalization per record is exactly what admits that inconsistency.
#[test]
fn backward_replay_rejects_a_per_record_renaming() {
    let system = TermSystem::new(vec![
        Rule::new(parse("f(U,V)"), parse("g(U,V)")).expect("checked rule"),
        Rule::new(parse("g(U,V)"), parse("h(U,V)")).expect("checked rule"),
    ]);
    let f_g = RuleId::from_rule(&system.rules()[0]);
    let g_h = RuleId::from_rule(&system.rules()[1]);
    let target = parse("h(X,Y)");
    let goal = parse("f(X,Y)");
    let (system, certificate) = backward_certificate(
        system.rules().to_vec(),
        target.clone(),
        goal.clone(),
        vec![
            recorded(parse("g(Y,X)"), g_h, &parse("f(X,Y)"), 1),
            recorded(parse("f(X,Y)"), f_g, &parse("g(Y,X)"), 2),
        ],
    );
    let error = certificate
        .verify(&system)
        .expect_err("an inconsistently renamed recorded term must not verify");
    assert!(
        error.to_string().contains("recorded step term"),
        "the typed rejection names the recorded step term, got {error}"
    );
}

/// F9 positive control: a consistent whole-trace renaming is alpha equivalent
/// to the replay and verifies.
///
/// The trace uses `R`/`S` throughout while the goal uses `P`/`Q`; the leaf/goal
/// unifier `{R |-> P, S |-> Q}` is ONE renaming applied to every record and to
/// the target, so the recorded trace denotes exactly the replayed states.
#[test]
fn backward_replay_admits_one_whole_trace_renaming() {
    let system = TermSystem::new(vec![
        Rule::new(parse("f(U,V)"), parse("g(U,V)")).expect("checked rule"),
        Rule::new(parse("g(U,V)"), parse("h(U,V)")).expect("checked rule"),
    ]);
    let f_g = RuleId::from_rule(&system.rules()[0]);
    let g_h = RuleId::from_rule(&system.rules()[1]);
    let target = parse("h(R,S)");
    let goal = parse("f(P,Q)");
    let (system, certificate) = backward_certificate(
        system.rules().to_vec(),
        target.clone(),
        goal.clone(),
        vec![
            recorded(parse("g(R,S)"), g_h, &parse("f(R,S)"), 1),
            recorded(parse("f(R,S)"), f_g, &parse("g(R,S)"), 2),
        ],
    );
    certificate
        .verify(&system)
        .expect("one consistent whole-trace renaming must verify");
}

/// F9 positive control: a valid certificate whose variables stay unbound still
/// verifies (the correspondence leaves free variables free).
///
/// Rule `h(V) -> c`, target `c`, goal `h(Y)`: the leaf `h(V)` unifies with the
/// goal under `V |-> Y`, and `Y` is never fixed, so the recorded trace is
/// symbolic after instantiation and must stay admissible.
#[test]
fn backward_replay_admits_an_unbound_variable_certificate() {
    let system = TermSystem::new(vec![
        Rule::new(parse("h(V)"), parse("c")).expect("checked rule")
    ]);
    let rule_id = RuleId::from_rule(&system.rules()[0]);
    let target = parse("c");
    let goal = parse("h(Y)");
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward {
            target: target.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Backward(BackwardDerivation {
            steps: vec![recorded(parse("h(V)"), rule_id, &target, 1)],
        }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    certificate
        .verify(&system)
        .expect("a symbolic certificate with an unbound variable must verify");
}

/// F9 positive control anchored by a real search: every genuine witness the
/// backward explorer produces still verifies under the whole-trace
/// correspondence.
#[test]
fn backward_search_witnesses_still_verify() {
    let system = TermSystem::new(vec![
        Rule::new(parse("f(U,V)"), parse("g(U,V)")).expect("checked rule"),
        Rule::new(parse("g(U,V)"), parse("h(U,V)")).expect("checked rule"),
    ]);
    let target = parse("h(X,Y)");
    let goal = parse("f(X,Y)");
    let outcome = BackwardExplorer::new(&system, InverseSearchConfig::default())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .expect("search must run");
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("the chain system must witness, got {outcome:?}");
    };
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Backward {
            target: target.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    certificate
        .verify(&system)
        .expect("a genuine witness must verify");
}

/// F10 (P1): the two bidirectional halves must meet under the recorded unifier.
///
/// Only rule `b -> c`; source `a`, goal `c`; zero forward steps; the recorded
/// meeting "unifier" is empty, `meeting_forward = a`, `meeting_backward = b`,
/// and the backward half is the single recorded root `b`. The replay reaches
/// `a` forward and starts the inverse half at `b`, so the two halves never meet
/// — yet an empty unifier compares nothing and the certificate was admitted.
#[test]
fn bidirectional_replay_rejects_halves_that_never_meet() {
    let system = TermSystem::new(vec![
        Rule::new(parse("b"), parse("c")).expect("checked rule")
    ]);
    let b_c = RuleId::from_rule(&system.rules()[0]);
    let source = parse("a");
    let goal = parse("c");
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: source.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Bidirectional(BidirectionalDerivation {
            forward_steps: Vec::new(),
            backward_steps: vec![recorded(parse("b"), b_c, &goal, 1)],
            meeting_forward: parse("a"),
            meeting_backward: parse("b"),
            unifier: Substitution::new(),
        }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    let error = certificate
        .verify(&system)
        .expect_err("halves that never meet must not verify");
    assert!(
        error.to_string().contains("meeting"),
        "the typed rejection names the meeting, got {error}"
    );
}

/// F10 positive control: a legitimate symbolic, capture-free meet verifies.
///
/// Rule `f(X,Y) -> X`; source `f(Y,Z)`, goal `Z`. The forward half reaches the
/// source variable `Y`, the goal side is the free variable `Z`, and the
/// recorded unifier `{Y |-> Z}` makes both meeting terms and the actual forward
/// endpoint ONE common instantiated term (`Z`) — which stays symbolic.
#[test]
fn bidirectional_replay_admits_a_symbolic_meet() {
    let system = TermSystem::new(vec![
        Rule::new(parse("f(X,Y)"), parse("X")).expect("checked rule")
    ]);
    let source = parse("f(Y,Z)");
    let goal = parse("Z");
    let outcome = BidirectionalExplorer::new(&system, InverseSearchConfig::default())
        .search(&source, &goal)
        .expect("search must run");
    let BidirectionalSearchOutcome::Witness(derivation) = outcome else {
        panic!("the symbolic meet must witness, got {outcome:?}");
    };
    assert!(
        derivation.meeting_forward.is_var(),
        "the control is a symbolic meeting, got {:?}",
        derivation.meeting_forward
    );
    assert_eq!(
        derivation.unifier.get("Y"),
        Some(&parse("Z")),
        "the meet is witnessed by the source-variable-to-goal-variable unifier"
    );
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: source.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Bidirectional(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    certificate
        .verify(&system)
        .expect("a symbolic capture-free meet must verify");
}

/// F10 junction audit: a recorded unifier that does not identify the meeting
/// terms is rejected even when the halves individually replay.
///
/// Same shape as the negative control, but the unifier is non-empty and binds
/// the wrong side (`{Q |-> a}`): `meeting_forward = a` and
/// `meeting_backward = b` still instantiate apart, so there is no common
/// meeting.
#[test]
fn bidirectional_replay_rejects_a_unifier_that_does_not_identify_the_meetings() {
    let system = TermSystem::new(vec![
        Rule::new(parse("b"), parse("c")).expect("checked rule")
    ]);
    let b_c = RuleId::from_rule(&system.rules()[0]);
    let source = parse("a");
    let goal = parse("c");
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: source.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Bidirectional(BidirectionalDerivation {
            forward_steps: Vec::new(),
            backward_steps: vec![recorded(parse("b"), b_c, &goal, 1)],
            meeting_forward: parse("a"),
            meeting_backward: parse("b"),
            unifier: Substitution::new().with("Q", parse("a")),
        }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    let error = certificate
        .verify(&system)
        .expect_err("an unrelated unifier must not certify a meeting");
    assert!(
        error.to_string().contains("meeting"),
        "the typed rejection names the meeting, got {error}"
    );
}

/// F10 positive control: a real bidirectional search witness verifies, and its
/// forward half is checked against the meeting before the inverse half runs.
#[test]
fn bidirectional_search_witnesses_still_verify() {
    let system = TermSystem::new(vec![
        Rule::new(parse("add(X, zero)"), parse("X")).expect("checked rule")
    ]);
    let source = parse("add(a, zero)");
    let goal = parse("a");
    let outcome = BidirectionalExplorer::new(&system, InverseSearchConfig::default())
        .search(&source, &goal)
        .expect("search must run");
    let BidirectionalSearchOutcome::Witness(derivation) = outcome else {
        panic!("the reducer must witness, got {outcome:?}");
    };
    assert_eq!(derivation.forward_steps.len(), 1);
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: source.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Bidirectional(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    certificate
        .verify(&system)
        .expect("a genuine bidirectional witness must verify");
}

/// F10 admission: a certificate whose halves never meet is rejected before a
/// trace can be minted from it.
#[cfg(feature = "holographic-guidance")]
#[test]
fn holographic_admission_rejects_halves_that_never_meet() {
    use amari_rewrite::holographic::{HolographicError, RecallTrace};

    let system = TermSystem::new(vec![
        Rule::new(parse("b"), parse("c")).expect("checked rule")
    ]);
    let b_c = RuleId::from_rule(&system.rules()[0]);
    let source = parse("a");
    let goal = parse("c");
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: source.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Bidirectional(BidirectionalDerivation {
            forward_steps: Vec::new(),
            backward_steps: vec![recorded(parse("b"), b_c, &goal, 1)],
            meeting_forward: parse("a"),
            meeting_backward: parse("b"),
            unifier: Substitution::new(),
        }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    let error = RecallTrace::validate(&system, &certificate)
        .expect_err("no trace can be minted from halves that never meet");
    assert!(
        matches!(error, HolographicError::UnvalidatedProvenance(_)),
        "the rejection is a provenance rejection, got {error}"
    );
}

/// F15 (P1): the bidirectional backward half is ANCHORED at the checked
/// meeting.
///
/// Rule `f(U,V) -> g(V)`; source and forward meeting `f(a,b)`; backward meeting
/// `f(X,Y)` under the checked unifier `{X := a, Y := b}`; the sole recorded
/// backward term is `f(Y,X)`; goal `g(X)`. The junction validates `f(a,b)` on
/// both meeting terms, but an EMPTY recorded-to-replay correspondence is then
/// free to bind `Y := a, X := b` from the record, which contradicts the checked
/// `X := a` and accepts `g(b)`. The correspondence must be seeded from the
/// checked meeting before any record or goal matching, so this certificate is a
/// typed mismatch.
#[test]
fn bidirectional_replay_anchors_the_backward_half_at_the_checked_meeting() {
    let system = TermSystem::new(vec![
        Rule::new(parse("f(U,V)"), parse("g(V)")).expect("checked rule")
    ]);
    let f_g = RuleId::from_rule(&system.rules()[0]);
    let source = parse("f(a,b)");
    let goal = parse("g(X)");
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: source.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Bidirectional(BidirectionalDerivation {
            forward_steps: Vec::new(),
            backward_steps: vec![recorded(parse("f(Y,X)"), f_g, &parse("g(X)"), 1)],
            meeting_forward: source.clone(),
            meeting_backward: parse("f(X,Y)"),
            unifier: Substitution::new()
                .with("X", parse("a"))
                .with("Y", parse("b")),
        }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    let error = certificate
        .verify(&system)
        .expect_err("a record that contradicts the checked meeting must not verify");
    assert!(
        error.to_string().contains("goal"),
        "the typed rejection names the goal the anchored trace cannot reach, got {error}"
    );
}

/// F15 positive control: a recorded half consistent with the checked meeting
/// still verifies.
///
/// Same junction and unifier as the negative control, but the record is the
/// meeting term itself (`f(X,Y)`) and the goal is `g(Y)`, so ONE instantiation
/// (`X := a`, `Y := b`) is shared by the anchor, the record, and the goal.
#[test]
fn bidirectional_replay_admits_a_record_consistent_with_the_checked_meeting() {
    let system = TermSystem::new(vec![
        Rule::new(parse("f(U,V)"), parse("g(V)")).expect("checked rule")
    ]);
    let f_g = RuleId::from_rule(&system.rules()[0]);
    let source = parse("f(a,b)");
    let goal = parse("g(Y)");
    let certificate = ReplayCertificate::issue(
        &system,
        ReplayQuery::Bidirectional {
            source: source.clone(),
            goal: goal.clone(),
        },
        ReplayDerivation::Bidirectional(BidirectionalDerivation {
            forward_steps: Vec::new(),
            backward_steps: vec![recorded(parse("f(X,Y)"), f_g, &parse("g(Y)"), 1)],
            meeting_forward: source.clone(),
            meeting_backward: parse("f(X,Y)"),
            unifier: Substitution::new()
                .with("X", parse("a"))
                .with("Y", parse("b")),
        }),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        observation(),
    );
    certificate
        .verify(&system)
        .expect("a record consistent with the checked meeting must verify");
}
