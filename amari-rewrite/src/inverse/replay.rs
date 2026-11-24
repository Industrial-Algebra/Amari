// SPDX-License-Identifier: MIT OR Apache-2.0

//! Replay certificates bound to system, query, config, and guidance
//! hashes plus exact resource authority.
//!
//! A certificate is only accepted when every recorded hash
//! recomputes exactly, the recorded resources fit the configuration
//! ceilings, and the derivation replays through the supplied system
//! to the recorded query. Any tampering is a hard error.

use alloc::string::ToString;
use alloc::vec::Vec;

use crate::analysis::unify;
use crate::error::{RewriteError, RewriteResult};
use crate::inverse::bounded::{
    apply_checked, measure_borrowed, measure_matcher, shape_after_replacement, TermShape,
    TraceVariables,
};
use crate::inverse::{
    BackwardDerivation, BidirectionalDerivation, GuidanceMode, InverseSearchConfig,
};
use crate::relation::{RelationLimits, RelationResources, RuleId, Sha256Digest};
use crate::trs::{match_pattern, match_pattern_into, Substitution, Term, TermSystem};

/// The query a derivation answers.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum ReplayQuery {
    /// A backward search from target toward goal.
    Backward { target: Term, goal: Term },
    /// A bidirectional search between source and goal.
    Bidirectional { source: Term, goal: Term },
}

/// The derivation a certificate attests to.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum ReplayDerivation {
    /// A backward derivation.
    Backward(BackwardDerivation),
    /// A bidirectional meeting derivation.
    Bidirectional(BidirectionalDerivation),
}

/// Exact resource observations recorded at search time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct ResourceObservation {
    /// Retained states observed.
    pub states: u64,
    /// Transitions explored.
    pub transitions: u64,
    /// Retained frontier bytes.
    pub retained_bytes: u64,
    /// Abstract operations spent.
    pub operations: u64,
}

/// A replay certificate: derivation plus the hashes and resource
/// authority binding it to a specific system, query, configuration,
/// and guidance mode.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(
    feature = "serialize",
    derive(serde::Serialize, serde::Deserialize),
    serde(deny_unknown_fields)
)]
pub struct ReplayCertificate {
    /// The answered query.
    pub query: ReplayQuery,
    /// The attested derivation.
    pub derivation: ReplayDerivation,
    /// The search configuration.
    pub config: InverseSearchConfig,
    /// The guidance mode used.
    pub guidance: GuidanceMode,
    /// Recorded resource observations.
    pub resources: ResourceObservation,
    /// Binding hash of the term system.
    pub system_hash: Sha256Digest,
    /// Binding hash of the query.
    pub query_hash: Sha256Digest,
    /// Binding hash of the configuration.
    pub config_hash: Sha256Digest,
    /// Binding hash of the guidance mode.
    pub guidance_hash: Sha256Digest,
}

/// Digest binding a certificate to a term system (ordered rule
/// identities).
pub fn system_hash(system: &TermSystem) -> Sha256Digest {
    let mut encoding = Vec::new();
    for rule in system.rules() {
        encoding.extend_from_slice(RuleId::from_rule(rule).digest().as_bytes());
    }
    Sha256Digest::framed("amari.inverse.system/v1", &encoding)
}

/// Digest binding a certificate to its query.
pub fn query_hash(query: &ReplayQuery) -> Sha256Digest {
    let mut encoding = Vec::new();
    let mut variables = Vec::new();
    match query {
        ReplayQuery::Backward { target, goal } => {
            encoding.push(0x00);
            crate::relation::digest::encode_term(target, &mut variables, &mut encoding);
            crate::relation::digest::encode_term(goal, &mut variables, &mut encoding);
        }
        ReplayQuery::Bidirectional { source, goal } => {
            encoding.push(0x01);
            crate::relation::digest::encode_term(source, &mut variables, &mut encoding);
            crate::relation::digest::encode_term(goal, &mut variables, &mut encoding);
        }
    }
    Sha256Digest::framed("amari.inverse.query/v1", &encoding)
}

/// Digest binding a certificate to a search configuration.
pub fn config_hash(config: &InverseSearchConfig) -> Sha256Digest {
    let mut encoding = Vec::new();
    for value in [
        config.max_depth(),
        config.max_states(),
        config.max_transitions(),
        config.max_term_nodes(),
        config.max_term_depth(),
        config.max_constraints(),
        config.max_groundings(),
        config.max_operations(),
        config.max_frontier_bytes(),
        config.max_trace_bytes(),
    ] {
        encoding.extend_from_slice(&value.to_le_bytes());
    }
    Sha256Digest::framed("amari.inverse.config/v1", &encoding)
}

impl ReplayCertificate {
    /// Issue a certificate, computing all binding hashes.
    pub fn issue(
        system: &TermSystem,
        query: ReplayQuery,
        derivation: ReplayDerivation,
        config: InverseSearchConfig,
        guidance: GuidanceMode,
        resources: ResourceObservation,
    ) -> Self {
        Self {
            system_hash: system_hash(system),
            query_hash: query_hash(&query),
            config_hash: config_hash(&config),
            guidance_hash: crate::inverse::guidance_hash(&guidance),
            query,
            derivation,
            config,
            guidance,
            resources,
        }
    }

    /// Verify the certificate against a supplied system: every
    /// binding hash must recompute exactly, the recorded resources
    /// must fit the configuration ceilings, and the derivation must
    /// replay through the system to the recorded query.
    pub fn verify(&self, system: &TermSystem) -> RewriteResult<()> {
        let mismatch = |message: &str| RewriteError::ResidualMismatch {
            message: message.to_string(),
        };
        // The embedded configuration is part of the certified
        // statement, so it must be a VALID configuration: a
        // certificate carrying limits no search could run under
        // (for example max_depth = 0) is rejected regardless of
        // whether its hashes match.
        self.config
            .validate()
            .map_err(|error| RewriteError::ResidualMismatch {
                message: alloc::format!("certificate configuration is invalid: {error}"),
            })?;
        if system_hash(system) != self.system_hash {
            return Err(mismatch("certificate system hash mismatch"));
        }
        if query_hash(&self.query) != self.query_hash {
            return Err(mismatch("certificate query hash mismatch"));
        }
        if config_hash(&self.config) != self.config_hash {
            return Err(mismatch("certificate config hash mismatch"));
        }
        if crate::inverse::guidance_hash(&self.guidance) != self.guidance_hash {
            return Err(mismatch("certificate guidance hash mismatch"));
        }
        self.guidance.validate(&self.config)?;
        let observed = self.resources;
        let config = &self.config;
        for (resource, value, ceiling) in [
            ("certificate states", observed.states, config.max_states()),
            (
                "certificate transitions",
                observed.transitions,
                config.max_transitions(),
            ),
            (
                "certificate retained bytes",
                observed.retained_bytes,
                config.max_frontier_bytes(),
            ),
            (
                "certificate operations",
                observed.operations,
                config.max_operations(),
            ),
        ] {
            if value > ceiling {
                return Err(RewriteError::RelationLimitExceeded {
                    resource,
                    limit: ceiling as usize,
                });
            }
        }
        match (&self.query, &self.derivation) {
            (ReplayQuery::Backward { target, goal }, ReplayDerivation::Backward(derivation)) => {
                replay_backward(system, target, goal, derivation, &self.config)
            }
            (
                ReplayQuery::Bidirectional { source, goal },
                ReplayDerivation::Bidirectional(derivation),
            ) => replay_bidirectional(system, source, goal, derivation, &self.config),
            _ => Err(mismatch("query kind does not match derivation kind")),
        }
    }
}

/// One derivation half whose step count exceeds what the embedded search
/// configuration could have produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StepCeiling {
    /// The rejected resource.
    pub(crate) resource: &'static str,
    /// The observed value.
    pub(crate) value: usize,
    /// The applied ceiling.
    pub(crate) limit: usize,
}

/// Validate one derivation half's step count against the embedded
/// configuration's ceilings, before the half is replayed.
///
/// A recorded half is *replay* content, not encoded trace content: its length
/// is bounded by the search that could have produced it. Every step walks one
/// edge, so a half cannot be longer than the embedded `max_depth`, and it
/// cannot consume fewer than one retained state (`steps + 1`) or one transition
/// per step. This is deliberately NOT
/// [`crate::holographic::MAX_TRACE_TERMS`]: that ceiling bounds the encoded
/// terms a corpus stores, not the non-encoded forward steps a bidirectional
/// derivation replays (F12, PR #384 round 3).
pub(crate) fn check_replay_steps(
    resource: &'static str,
    steps: usize,
    config: &InverseSearchConfig,
) -> Result<(), StepCeiling> {
    let steps_wide = u64::try_from(steps).unwrap_or(u64::MAX);
    let narrow = |value: u64| usize::try_from(value).unwrap_or(usize::MAX);
    if steps_wide > config.max_depth() {
        return Err(StepCeiling {
            resource,
            value: steps,
            limit: narrow(config.max_depth()),
        });
    }
    if steps_wide.saturating_add(1) > config.max_states() {
        return Err(StepCeiling {
            resource: "replay states",
            value: steps.saturating_add(1),
            limit: narrow(config.max_states()),
        });
    }
    if steps_wide > config.max_transitions() {
        return Err(StepCeiling {
            resource: "replay transitions",
            value: steps,
            limit: narrow(config.max_transitions()),
        });
    }
    Ok(())
}

fn mismatch(message: alloc::string::String) -> RewriteError {
    RewriteError::ResidualMismatch { message }
}

fn ceiling_error(ceiling: StepCeiling) -> RewriteError {
    RewriteError::RelationLimitExceeded {
        resource: ceiling.resource,
        limit: ceiling.limit,
    }
}

/// Admit one borrowed term under the relation budget before it is cloned,
/// unified, or otherwise materialized.
fn admit(term: &Term, resources: &mut RelationResources) -> RewriteResult<TermShape> {
    let shape = measure_borrowed(term, resources)?;
    resources.record_term(shape.nodes, shape.depth)?;
    Ok(shape)
}

/// Apply one recorded replay step, preflighting every generated state before it
/// is allocated (F12, PR #384 round 3).
///
/// Order inside one step: the recorded path is followed, the matched position's
/// shape is measured, the matcher RHS application is measured (verbatim matcher
/// semantics — ranges are data), the REBUILT whole term's node count and depth
/// are computed from those shapes and charged against the budget, and only then
/// are the replacement and the rebuilt term allocated. The 5,001-node
/// intermediate a wide forward rule would build is therefore rejected before it
/// exists.
fn replay_step(
    system: &TermSystem,
    rule_id: RuleId,
    position: &crate::rewritable::Path,
    current: &Term,
    current_shape: TermShape,
    resources: &mut RelationResources,
    side: &str,
) -> RewriteResult<(Term, TermShape)> {
    resources.record_operations(1)?;
    let rule = system
        .rules()
        .iter()
        .find(|rule| RuleId::from_rule(rule) == rule_id)
        .ok_or_else(|| mismatch(alloc::format!("{side} step references unknown rule")))?;
    let subterm = current
        .subterm(position)
        .ok_or_else(|| mismatch(alloc::format!("replay lost the {side} path")))?;
    let subterm_shape = measure_borrowed(subterm, resources)?;
    let bindings = match_pattern(rule.lhs(), subterm)
        .ok_or_else(|| mismatch(alloc::format!("replay failed the {side} lhs")))?;
    let replacement_shape = measure_matcher(&bindings, rule.rhs(), resources)?;
    resources.record_term(replacement_shape.nodes, replacement_shape.depth)?;
    let next_shape = shape_after_replacement(
        current,
        position.as_slice(),
        current_shape,
        subterm_shape,
        replacement_shape,
        resources,
    )?;
    resources.record_term(next_shape.nodes, next_shape.depth)?;
    // Admitted and charged: the allocation is now bounded by the budget.
    let next = current
        .replace_at(position, bindings.apply_matcher(rule.rhs()))
        .map_err(|_| mismatch(alloc::format!("replay failed a {side} rebuild")))?;
    Ok((next, next_shape))
}

/// Replay a backward derivation forward to its target and confirm the
/// recorded trace under ONE assignment and ONE free-variable correspondence.
///
/// Acceptance invariant (PR #384 round 2, F7): the leaf/goal unifier is the
/// certificate's single assignment. It is retained, and every comparison — each
/// recorded step term against the state the replay reaches, and the
/// reconstructed endpoint against the declared target — is made after applying
/// that one assignment to both sides. A discarded leaf/goal unifier would let
/// the replay reconstruct an endpoint that merely alpha-matches the target (the
/// reviewer's `h(R)` against `h(Q)` under `Q := a`, `R := b`), which is a
/// different term instance and is now a typed mismatch.
///
/// Acceptance invariant (PR #384 round 3, F9): the comparison at every record is
/// ALPHA EQUIVALENCE under ONE trace-wide bijective free-variable renaming,
/// anchored by the leaf/goal unifier (the seed comparison) and extended through
/// every record and the declared target. Variables the assignment does not fix
/// stay free and are renamed consistently, so a valid symbolic certificate is
/// still admitted; a middle record whose variables denote different instances
/// than the anchor does (the reviewer's `[g(Y,X), f(X,Y)]` against the goal
/// `f(X,Y)`) is a typed mismatch. Independent per-term canonicalization and
/// per-record instance matching are exactly what this replaces.
///
/// Bounded-application order (F11/F12): borrowed endpoints are admitted before
/// they are cloned, and every instantiated term is preflighted, charged, and
/// only then materialized (see [`crate::inverse::bounded`]).
fn replay_backward(
    system: &TermSystem,
    target: &Term,
    goal: &Term,
    derivation: &BackwardDerivation,
    config: &InverseSearchConfig,
) -> RewriteResult<()> {
    let mut resources = RelationResources::new(&RelationLimits::default());
    check_replay_steps("replay backward steps", derivation.steps.len(), config)
        .map_err(ceiling_error)?;
    // Admit every borrowed endpoint before it is cloned or unified.
    let _ = admit(target, &mut resources)?;
    let _ = admit(goal, &mut resources)?;
    if derivation.steps.is_empty() {
        // Zero-step witness: target must unify with the goal.
        return unify(target, goal, &mut resources)
            .map(|_| ())
            .map_err(|_| mismatch("zero-step witness does not unify".to_string()));
    }
    let leaf = &derivation.steps.last().expect("non-empty").term;
    let leaf_shape = admit(leaf, &mut resources)?;
    // F7 (PR #384 round 2): the leaf/goal unifier is retained, never discarded.
    // It is the one assignment every reconstructed term is read under.
    let assignment = unify(leaf, goal, &mut resources)
        .map_err(|_| mismatch("derivation leaf does not unify with the goal".to_string()))?;
    // F9 (PR #384 round 3): the anchor of the one whole-trace correspondence.
    let mut correspondence = TraceVariables::new();
    let anchored_leaf = apply_checked(&assignment, leaf, &mut resources)?;
    let anchored_goal = apply_checked(&assignment, goal, &mut resources)?;
    if !correspondence.match_alpha(&anchored_leaf, &anchored_goal) {
        return Err(mismatch(
            "derivation leaf does not match the goal under one correspondence".to_string(),
        ));
    }
    let mut current = leaf.clone();
    let mut current_shape = leaf_shape;
    for step in derivation.steps.iter().rev() {
        // F1 (PR #384 round 1): the recorded term of each step IS the state
        // this replay must reach, so it is compared before the step is applied.
        // F7/F9: both sides are read under the one assignment and compared as
        // alpha-equivalent under the one trace-wide renaming.
        let recorded = apply_checked(&assignment, &step.term, &mut resources)?;
        let replayed = apply_checked(&assignment, &current, &mut resources)?;
        if !correspondence.match_alpha(&recorded, &replayed) {
            return Err(mismatch(
                "replayed state does not match the recorded step term".to_string(),
            ));
        }
        let (next, next_shape) = replay_step(
            system,
            step.provenance.rule_id,
            &step.provenance.position,
            &current,
            current_shape,
            &mut resources,
            "backward",
        )?;
        current = next;
        current_shape = next_shape;
    }
    // The reconstructed endpoint and the declared target are compared under the
    // same single assignment and the same trace-wide correspondence, so the
    // certificate commits to exactly one endpoint instance.
    let recorded_target = apply_checked(&assignment, target, &mut resources)?;
    let replayed = apply_checked(&assignment, &current, &mut resources)?;
    if !correspondence.match_alpha(&recorded_target, &replayed) {
        return Err(mismatch(
            "replay does not reconstruct the target".to_string(),
        ));
    }
    Ok(())
}

/// Replay a bidirectional derivation: forward half from the source to the
/// meeting term, then the instantiated backward half to the goal.
///
/// Meeting invariant (PR #384 round 3, F10): the recorded unifier is validated
/// on BOTH recorded meeting terms AND on the actual forward endpoint. All three
/// must instantiate to ONE common term, so two unrelated caller-supplied meeting
/// terms with an empty unifier (the reviewer's forward endpoint `a`,
/// `meeting_backward = b`) are a typed mismatch before the inverse half runs.
/// The backward replay starts from that checked common instantiated meeting, not
/// from a separately supplied term.
///
/// Backward-half invariant (PR #384 round 2, F8): the whole backward half and
/// the goal/endpoint check are read through ONE recorded-term-to-replay
/// correspondence. Each successive match must EXTEND the correspondence, so a
/// recorded variable shared by two records (the reviewer's `[g(Z), f(Z)]`) must
/// denote the same value everywhere; a conflicting binding is a typed mismatch.
/// Genuine generalization is still accepted — `Z := a` for `f(Z)` and `g(Z)`
/// and the goal — because one substitution instantiates the entire recorded
/// trace.
///
/// Bounded-application order (F11/F12): each half's step count is checked
/// against the embedded configuration first, borrowed endpoints and meeting
/// terms are admitted before they are cloned, and every generated state is
/// preflighted and charged before it is materialized.
fn replay_bidirectional(
    system: &TermSystem,
    source: &Term,
    goal: &Term,
    derivation: &BidirectionalDerivation,
    config: &InverseSearchConfig,
) -> RewriteResult<()> {
    let mut resources = RelationResources::new(&RelationLimits::default());
    check_replay_steps(
        "replay forward steps",
        derivation.forward_steps.len(),
        config,
    )
    .map_err(ceiling_error)?;
    check_replay_steps(
        "replay backward steps",
        derivation.backward_steps.len(),
        config,
    )
    .map_err(ceiling_error)?;
    // Each step retains at least one state and explores at least one
    // transition, and the two halves share ONE configuration budget.
    let total_steps = derivation
        .forward_steps
        .len()
        .saturating_add(derivation.backward_steps.len());
    let total_wide = u64::try_from(total_steps).unwrap_or(u64::MAX);
    let narrow = |value: u64| usize::try_from(value).unwrap_or(usize::MAX);
    if total_wide > config.max_transitions() {
        return Err(RewriteError::RelationLimitExceeded {
            resource: "replay transitions",
            limit: narrow(config.max_transitions()),
        });
    }
    if total_wide.saturating_add(1) > config.max_states() {
        return Err(RewriteError::RelationLimitExceeded {
            resource: "replay states",
            limit: narrow(config.max_states()),
        });
    }
    // Admit every borrowed endpoint and meeting term before any clone.
    let mut current = source.clone();
    let mut current_shape = admit(source, &mut resources)?;
    let _ = admit(goal, &mut resources)?;
    let _ = admit(&derivation.meeting_forward, &mut resources)?;
    let _ = admit(&derivation.meeting_backward, &mut resources)?;
    for step in &derivation.forward_steps {
        let (next, next_shape) = replay_step(
            system,
            step.rule_id,
            &step.position,
            &current,
            current_shape,
            &mut resources,
            "forward",
        )?;
        current = next;
        current_shape = next_shape;
    }
    // F10 (PR #384 round 3): one checked common instantiated meeting that
    // includes the ACTUAL forward endpoint.
    let endpoint = apply_checked(&derivation.unifier, &current, &mut resources)?;
    let meeting_forward = apply_checked(
        &derivation.unifier,
        &derivation.meeting_forward,
        &mut resources,
    )?;
    let meeting_backward = apply_checked(
        &derivation.unifier,
        &derivation.meeting_backward,
        &mut resources,
    )?;
    if meeting_forward != meeting_backward {
        return Err(mismatch(
            "the recorded unifier does not identify the two meeting terms".to_string(),
        ));
    }
    if endpoint != meeting_forward {
        return Err(mismatch(
            "forward half does not reach the checked meeting term".to_string(),
        ));
    }
    // Start the inverse half from the checked state, not from a raw record.
    let mut current = meeting_forward;
    let mut current_shape = admit(&current, &mut resources)?;
    let mut correspondence = Substitution::new();
    for step in derivation.backward_steps.iter().rev() {
        // F1 (PR #384 round 1): the recorded backward term of each step IS the
        // state this replay must reach, so it is compared before the step is
        // applied. F8: the replay state must be an INSTANCE of the recorded term
        // under the ONE correspondence accumulated so far; a constant or a term
        // of another shape is still a hard rejection.
        let _ = admit(&step.term, &mut resources)?;
        if !match_pattern_into(&mut correspondence, &step.term, &current) {
            return Err(mismatch(
                "replayed state does not match the recorded backward term".to_string(),
            ));
        }
        let (next, next_shape) = replay_step(
            system,
            step.provenance.rule_id,
            &step.provenance.position,
            &current,
            current_shape,
            &mut resources,
            "backward",
        )?;
        current = next;
        current_shape = next_shape;
    }
    // The backward half starts from the meeting substitution's instantiation of
    // the meeting term, so a goal whose variables were bound by the meet
    // reconstructs to an INSTANCE of the goal, not the raw goal. Acceptance is
    // pattern matching with the goal as the pattern, extended into the SAME
    // correspondence, so the goal's variables share the recorded trace's
    // instantiation.
    if !match_pattern_into(&mut correspondence, goal, &current) {
        return Err(mismatch(
            "replay does not reconstruct an instance of the goal".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inverse::{BackwardExplorer, BackwardSearchOutcome, SearchMode};
    use crate::trs::{Term, TermSystem};
    use alloc::vec;

    /// I3 (Cohort 3 closeout): a certificate whose embedded
    /// configuration is invalid must not verify, even when every
    /// hash is recomputed to match the tampered config.
    #[test]
    fn verify_rejects_an_invalid_embedded_configuration() {
        let system = TermSystem::new(vec![]);
        let target = Term::constant("a");
        let goal = Term::var("X");
        let outcome = BackwardExplorer::new(&system, InverseSearchConfig::default())
            .search(&target, &goal, SearchMode::BreadthFirst)
            .unwrap();
        let BackwardSearchOutcome::Witness(derivation) = outcome else {
            panic!("baseline must witness");
        };
        let query = ReplayQuery::Backward { target, goal };
        let resources = ResourceObservation {
            states: 1,
            transitions: 0,
            retained_bytes: 64,
            operations: 4,
        };
        let mut certificate = ReplayCertificate::issue(
            &system,
            query,
            ReplayDerivation::Backward(derivation),
            InverseSearchConfig::default(),
            GuidanceMode::CompleteWithinLimits,
            resources,
        );
        assert!(certificate.verify(&system).is_ok());
        // Tamper: no search with max_depth = 0 can produce a
        // witness, and `new` rejects such a configuration.
        certificate.config = certificate.config.with_max_depth_unchecked(0);
        certificate.config_hash = config_hash(&certificate.config);
        let result = certificate.verify(&system);
        assert!(result.is_err(), "max_depth = 0 must not verify");
    }
}
