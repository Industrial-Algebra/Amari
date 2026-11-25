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

use crate::analysis::unify_admitted;
use crate::error::{RewriteError, RewriteResult};
use crate::inverse::bounded::{
    apply_checked, measure_borrowed, measure_borrowed_detailed, measure_matcher,
    shape_after_replacement, BorrowedMeasurementError, TermShape, TraceVariables,
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

    /// Verify the certificate against a supplied system: every binding hash must
    /// recompute exactly, the recorded resources must fit the configuration
    /// ceilings, every borrowed term must fit the relation ceilings, the
    /// derivation must be one the embedded search could have produced, and the
    /// derivation must replay through the system to the recorded query.
    ///
    /// The order is the shared crate-internal preflight and admission slice
    /// (cheap authority first, borrowed admission last, both before any hash or
    /// clone), followed by the crate-internal `verify_admitted` continuation
    /// (the hashes and the replay).
    pub fn verify(&self, system: &TermSystem) -> RewriteResult<()> {
        let mut resources = RelationResources::new(&RelationLimits::default());
        let admission =
            preflight_and_admit(self, &mut resources).map_err(certificate_rejection_error)?;
        self.verify_admitted(system, &admission)
    }

    /// Verify a certificate whose cheap preflight and borrowed admission have
    /// already run: recompute every binding hash, then replay the derivation.
    ///
    /// Crate-internal continuation of [`Self::verify`]. `RecallTrace::validate`
    /// runs the shared [`preflight_and_admit`] itself (so it can map the
    /// rejection onto its own public error type) and then enters here, which is
    /// what keeps a valid certificate's borrowed terms from being walked twice
    /// (F27, PR #384 round 5).
    pub(crate) fn verify_admitted(
        &self,
        system: &TermSystem,
        admission: &CertificateAdmission,
    ) -> RewriteResult<()> {
        let mismatch = |message: &str| RewriteError::ResidualMismatch {
            message: message.to_string(),
        };
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
        match (&self.query, &self.derivation, admission) {
            (
                ReplayQuery::Backward { target, goal },
                ReplayDerivation::Backward(derivation),
                CertificateAdmission::Backward { leaf },
            ) => replay_backward(system, target, goal, derivation, *leaf),
            (
                ReplayQuery::Bidirectional { source, goal },
                ReplayDerivation::Bidirectional(derivation),
                CertificateAdmission::Bidirectional {
                    source: source_shape,
                },
            ) => replay_bidirectional(system, source, goal, derivation, *source_shape),
            _ => Err(mismatch("query kind does not match derivation kind")),
        }
    }
}

/// Shapes computed by the single borrowed-admission pass and needed by replay.
/// Every other borrowed term is still admitted; only these shapes have a
/// consumer after preflight, so carrying them avoids a second measurement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CertificateAdmission {
    Backward { leaf: Option<TermShape> },
    Bidirectional { source: TermShape },
}

/// One rejection from the shared certificate preflight and borrowed admission.
///
/// Each public entry point maps these onto its own error type:
/// [`ReplayCertificate::verify`] reports the certificate error itself, while the
/// holographic recall boundary wraps a certificate error as unvalidated
/// provenance and reports both an integer ceiling and a borrowed shape as its
/// own admission limit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CertificateRejection {
    /// The embedded configuration, guidance mode, or recorded observation is
    /// invalid, or the query and derivation kinds disagree.
    Certificate(RewriteError),
    /// A derivation half exceeds the embedded search's integer ceilings.
    StepCeiling(StepCeiling),
    /// A borrowed term or substitution range exceeds the relation ceilings.
    Shape {
        /// The rejected resource (`"term nodes"` or `"term depth"`).
        resource: &'static str,
        /// The measured offending lower bound.
        value: usize,
        /// The applied ceiling.
        limit: usize,
    },
}

/// The cheapest authority first, the borrowed admission last, and no hashing or
/// cloning in between (F19/F21/F27, PR #384 rounds 4-5).
///
/// 1. the embedded configuration and guidance mode are validated (fixed
///    ceilings, no term work);
/// 2. the cheap structural ceilings are decided on integers — each half's step
///    count, the producer-derived retained-state and operation bounds
///    (documented on the crate-internal step checker), and the recorded resource
///    observation;
/// 3. every borrowed term the replay reads — both query endpoints, every
///    recorded step term, both meeting terms, and every recorded unifier range —
///    is admitted under ONE caller-supplied [`RelationResources`] meter, so the
///    aggregate admission is bounded instead of one fresh walk per binding;
/// 4. only then may the caller recompute the binding hashes (`query_hash`
///    encodes the whole query) and run the replay, which is where the first
///    clone of the source can happen.
///
/// Both public entries share this exact pass, so a certificate with many unused
/// ranges and an over-ceiling recorded observation is rejected on integers
/// before the first range walk, and a valid certificate's borrowed terms are
/// admitted once on the whole path.
pub(crate) fn preflight_and_admit(
    certificate: &ReplayCertificate,
    resources: &mut RelationResources,
) -> Result<CertificateAdmission, CertificateRejection> {
    // The embedded configuration is part of the certified statement, so it must
    // be a VALID configuration: a certificate carrying limits no search could
    // run under (for example max_depth = 0) is rejected regardless of whether
    // its hashes match.
    certificate.config.validate().map_err(|error| {
        CertificateRejection::Certificate(RewriteError::ResidualMismatch {
            message: alloc::format!("certificate configuration is invalid: {error}"),
        })
    })?;
    certificate
        .guidance
        .validate(&certificate.config)
        .map_err(CertificateRejection::Certificate)?;
    // Cheap integer ceilings first: a half longer than the embedded search could
    // walk is rejected before any term is touched (F21).
    check_derivation_bounds(&certificate.derivation, &certificate.config)
        .map_err(CertificateRejection::StepCeiling)?;
    let observed = certificate.resources;
    let config = &certificate.config;
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
            return Err(CertificateRejection::Certificate(
                RewriteError::RelationLimitExceeded {
                    resource,
                    limit: ceiling as usize,
                },
            ));
        }
    }
    // Borrowed admission: one shared meter, no clones, no hashes yet. Carry
    // the shapes replay consumes into its continuation (F32, PR #384 round 6).
    admit_certificate(&certificate.query, &certificate.derivation, resources)
}

/// Map one shared rejection onto [`ReplayCertificate::verify`]'s error type.
fn certificate_rejection_error(rejection: CertificateRejection) -> RewriteError {
    match rejection {
        CertificateRejection::Certificate(error) => error,
        CertificateRejection::StepCeiling(ceiling) => ceiling_error(ceiling),
        CertificateRejection::Shape {
            resource, limit, ..
        } => RewriteError::RelationLimitExceeded { resource, limit },
    }
}

/// Admit every borrowed term the replay reads, under one shared meter.
///
/// This is the exact set the replay touches — the query's endpoints, every
/// recorded step term, both meeting terms, and every recorded unifier range —
/// measured by reference before anything is hashed, cloned, or applied. The
/// meter is shared, so `n` bindings cost one bounded aggregate walk instead of
/// `n` independent unmetered ones (F21, PR #384 round 4).
fn admit_certificate(
    query: &ReplayQuery,
    derivation: &ReplayDerivation,
    resources: &mut RelationResources,
) -> Result<CertificateAdmission, CertificateRejection> {
    match (query, derivation) {
        (ReplayQuery::Backward { target, goal }, ReplayDerivation::Backward(derivation)) => {
            let _ = admit_borrowed(target, resources)?;
            let _ = admit_borrowed(goal, resources)?;
            let mut leaf = None;
            for step in &derivation.steps {
                leaf = Some(admit_borrowed(&step.term, resources)?);
            }
            Ok(CertificateAdmission::Backward { leaf })
        }
        (
            ReplayQuery::Bidirectional { source, goal },
            ReplayDerivation::Bidirectional(derivation),
        ) => {
            let source_shape = admit_borrowed(source, resources)?;
            let _ = admit_borrowed(goal, resources)?;
            let _ = admit_borrowed(&derivation.meeting_forward, resources)?;
            let _ = admit_borrowed(&derivation.meeting_backward, resources)?;
            for step in &derivation.backward_steps {
                let _ = admit_borrowed(&step.term, resources)?;
            }
            for (_, range) in derivation.unifier.iter() {
                let _ = admit_borrowed(range, resources)?;
            }
            Ok(CertificateAdmission::Bidirectional {
                source: source_shape,
            })
        }
        _ => Err(CertificateRejection::Certificate(mismatch(
            "query kind does not match derivation kind".to_string(),
        ))),
    }
}

/// Admit one borrowed certificate term, retaining the observed shape value for
/// the holographic boundary's public LimitExceeded payload.
fn admit_borrowed(
    term: &Term,
    resources: &mut RelationResources,
) -> Result<TermShape, CertificateRejection> {
    let shape = measure_borrowed_detailed(term, resources).map_err(|error| match error {
        BorrowedMeasurementError::Shape {
            resource,
            value,
            limit,
        } => CertificateRejection::Shape {
            resource,
            value,
            limit,
        },
        BorrowedMeasurementError::Operations { limit } => {
            CertificateRejection::Certificate(RewriteError::RelationLimitExceeded {
                resource: "operations",
                limit,
            })
        }
    })?;
    resources
        .record_term(shape.nodes, shape.depth)
        .map_err(CertificateRejection::Certificate)?;
    Ok(shape)
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

/// Retained-state and operation lower bounds a producing search must have paid
/// for one derivation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProducerBounds {
    /// The fewest retained states any producer of this derivation can end with.
    pub(crate) states: usize,
    /// The fewest expansion operations any producer of this derivation can spend.
    pub(crate) operations: usize,
}

/// Producer-derived retained-state and operation bounds, PER DERIVATION KIND.
///
/// A backward witness retains the target root plus every non-winning
/// predecessor: the goal-matching leaf is checked against the goal BEFORE it is
/// retained, so `n` steps retain `n` states, not `n + 1`.
///
/// A zero-step witness retains ZERO states: an immediate root hit returns a
/// witness BEFORE the producer's first `record_state`, so nothing is retained
/// and the derivation carries no step at all (F30, PR #384 round 5). The bound
/// reported here is therefore one — the smallest retained-state ceiling a
/// producer can run under — and not the number of retained states. A validated
/// configuration always has `max_states >= 1`, so the bound never rejects the
/// zero-step witness a producer can really issue.
///
/// A bidirectional witness retains BOTH roots before the search loop, and then
/// exactly one meeting state: the side that did not find the meet retains its
/// own chain (including the meeting node), while the side that found the meet
/// returns before retaining the new child. `n_f + n_b` steps therefore retain
/// `n_f + n_b + 1` states. Applying the backward formula to both kinds (the old
/// blanket `steps + 1` per half, or the backwards `n` here) misstates one of
/// them.
///
/// The operation bound is the same for both producers — expanding one node is
/// one accounted operation, and a chain of `n` steps requires expanding the `n`
/// nodes one edge before each step — but it is derived per kind so a future
/// producer that meters differently does not silently inherit a wrong bound.
pub(crate) fn backward_producer_bounds(steps: usize) -> ProducerBounds {
    ProducerBounds {
        states: steps.max(1),
        operations: steps,
    }
}

/// See [`backward_producer_bounds`] for the accounting; bidirectional halves
/// share one frontier, so their bound is derived from the total step count.
pub(crate) fn bidirectional_producer_bounds(forward: usize, backward: usize) -> ProducerBounds {
    let total = forward.saturating_add(backward);
    ProducerBounds {
        states: total.saturating_add(1),
        operations: total,
    }
}

/// Validate a derivation against the embedded configuration's ceilings, before
/// any half is replayed and before any borrowed term is admitted.
///
/// Four separate authorities are checked, in this order:
///
/// 1. **depth** — a half cannot be longer than the embedded `max_depth`, because
///    every step walks one edge of the derivation;
/// 2. **transitions** — one transition per step, shared across both halves of a
///    bidirectional derivation (they share one frontier budget);
/// 3. **retained states and operations** — the producer-derived lower bounds
///    from [`backward_producer_bounds`] / [`bidirectional_producer_bounds`], NOT
///    a blanket `steps + 1`.
///
/// The depth and transition bounds follow from the number of steps and the
/// producer's per-expansion checks alone: they hold for every traversal order,
/// so no search order (breadth-first, cost-first, guided) is claimed or required
/// here (F30, PR #384 round 5).
///
/// This is deliberately NOT [`crate::holographic::MAX_TRACE_TERMS`]: that
/// ceiling bounds the encoded terms a corpus stores, not the non-encoded forward
/// steps a bidirectional derivation replays (F12, PR #384 round 3). It is also
/// separate from the replay's own term-inspection meter: the embedded
/// `max_operations` is search authority, and the walks verification performs are
/// charged to a [`RelationResources`] budget (F20, PR #384 round 4).
pub(crate) fn check_derivation_bounds(
    derivation: &ReplayDerivation,
    config: &InverseSearchConfig,
) -> Result<(), StepCeiling> {
    match derivation {
        ReplayDerivation::Backward(derivation) => {
            let steps = derivation.steps.len();
            check_depth("replay backward steps", steps, config)?;
            check_transitions("replay transitions", steps, config)?;
            let bounds = backward_producer_bounds(steps);
            check_states(bounds.states, config)?;
            check_operations(bounds.operations, config)
        }
        ReplayDerivation::Bidirectional(derivation) => {
            let forward = derivation.forward_steps.len();
            let backward = derivation.backward_steps.len();
            check_depth("replay forward steps", forward, config)?;
            check_depth("replay backward steps", backward, config)?;
            let total = forward.saturating_add(backward);
            check_transitions("replay transitions", total, config)?;
            let bounds = bidirectional_producer_bounds(forward, backward);
            check_states(bounds.states, config)?;
            check_operations(bounds.operations, config)
        }
    }
}

fn check_depth(
    resource: &'static str,
    steps: usize,
    config: &InverseSearchConfig,
) -> Result<(), StepCeiling> {
    let steps_wide = u64::try_from(steps).unwrap_or(u64::MAX);
    if steps_wide > config.max_depth() {
        return Err(StepCeiling {
            resource,
            value: steps,
            limit: narrow(config.max_depth()),
        });
    }
    Ok(())
}

fn check_transitions(
    resource: &'static str,
    steps: usize,
    config: &InverseSearchConfig,
) -> Result<(), StepCeiling> {
    let steps_wide = u64::try_from(steps).unwrap_or(u64::MAX);
    if steps_wide > config.max_transitions() {
        return Err(StepCeiling {
            resource,
            value: steps,
            limit: narrow(config.max_transitions()),
        });
    }
    Ok(())
}

fn check_states(states: usize, config: &InverseSearchConfig) -> Result<(), StepCeiling> {
    let states_wide = u64::try_from(states).unwrap_or(u64::MAX);
    if states_wide > config.max_states() {
        return Err(StepCeiling {
            resource: "replay states",
            value: states,
            limit: narrow(config.max_states()),
        });
    }
    Ok(())
}

fn check_operations(operations: usize, config: &InverseSearchConfig) -> Result<(), StepCeiling> {
    let operations_wide = u64::try_from(operations).unwrap_or(u64::MAX);
    if operations_wide > config.max_operations() {
        return Err(StepCeiling {
            resource: "replay operations",
            value: operations,
            limit: narrow(config.max_operations()),
        });
    }
    Ok(())
}

fn narrow(value: u64) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

fn mismatch(message: alloc::string::String) -> RewriteError {
    RewriteError::ResidualMismatch { message }
}

/// Translate ONLY a unification failure into a replay mismatch (F26, PR #384
/// round 5).
///
/// The replay shares one [`RelationResources`] meter between borrowed admission
/// and the unification it performs, so an exhausted budget surfaces from
/// `unify` as [`RewriteError::RelationLimitExceeded`]. That is the replay's own
/// typed limit and must never be flattened into a certificate mismatch; every
/// other typed error (for example an [`RewriteError::InvalidSubstitution`] from
/// an unsupported cyclic input) is propagated unchanged as well.
fn translate_unification(error: RewriteError, message: &str) -> RewriteError {
    match error {
        RewriteError::UnificationFailure { .. } => mismatch(message.to_string()),
        other => other,
    }
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
    admitted_leaf: Option<TermShape>,
) -> RewriteResult<()> {
    let mut resources = RelationResources::new(&RelationLimits::default());
    // The caller's shared preflight admitted every borrowed endpoint and step;
    // replay carries the leaf shape forward instead of measuring it again.
    if derivation.steps.is_empty() {
        // Zero-step witness: target must unify with the goal. ONLY a genuine
        // unification failure is a certificate mismatch; a resource rejection
        // from unification is the replay's own typed budget error and is
        // propagated unchanged (F26, PR #384 round 5).
        return unify_admitted(target, goal, &mut resources)
            .map(|_| ())
            .map_err(|error| translate_unification(error, "zero-step witness does not unify"));
    }
    let leaf = &derivation.steps.last().expect("non-empty").term;
    let leaf_shape = admitted_leaf.ok_or_else(|| {
        mismatch("certificate admission is missing the backward leaf shape".to_string())
    })?;
    // F7 (PR #384 round 2): the leaf/goal unifier is retained, never discarded.
    // It is the one assignment every reconstructed term is read under. F26: a
    // resource rejection is not a mismatch, so it keeps its type (and its
    // `RelationLimitExceeded` resource name) all the way out.
    let assignment = unify_admitted(leaf, goal, &mut resources).map_err(|error| {
        translate_unification(error, "derivation leaf does not unify with the goal")
    })?;
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
/// Meeting anchor (PR #384 round 4, F15): the recorded-to-replay correspondence
/// is SEEDED from the checked meeting — the borrowed `meeting_backward` matched
/// against the instantiated meeting the junction accepted — before any record or
/// goal is matched. Records therefore EXTEND a correspondence that already
/// carries the meeting's own bindings, instead of being free to re-interpret
/// them; the reviewer's record `f(Y,X)` against a meeting `f(X,Y)` under
/// `{X := a, Y := b}` is a typed mismatch, where an empty seed accepted `g(b)`.
///
/// Bounded-application order (F11/F12): the embedded step, state, transition, and
/// operation ceilings are decided by the caller (see the crate-internal step
/// checker); borrowed endpoints and meeting terms are admitted before they are
/// cloned, including the source clone the forward half needs; and every generated
/// state is preflighted and charged before it is materialized.
fn replay_bidirectional(
    system: &TermSystem,
    source: &Term,
    goal: &Term,
    derivation: &BidirectionalDerivation,
    source_shape: TermShape,
) -> RewriteResult<()> {
    let mut resources = RelationResources::new(&RelationLimits::default());
    // The shared preflight admitted all caller-owned terms. Only the source
    // shape is needed immediately; it is carried forward for the first rebuild.
    let mut current_shape = source_shape;
    let mut current = source.clone();
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
    // F15 (PR #384 round 4): ANCHOR the one recorded-to-replay correspondence at
    // the checked meeting, before any record or goal matching. The borrowed
    // backward meeting is a claim about the very state the replay stands on, so
    // matching it against the checked instantiated meeting fixes every recorded
    // variable the junction already fixed. Without this seed the record below is
    // free to bind them to another instance (the reviewer's record `f(Y,X)`
    // re-binding `X := a` to `X := b`, accepting `g(b)` for a meeting that says
    // `g(a)`).
    let mut correspondence = Substitution::new();
    if !match_pattern_into(&mut correspondence, &derivation.meeting_backward, &current) {
        return Err(mismatch(
            "recorded backward meeting does not match the checked meeting term".to_string(),
        ));
    }
    for step in derivation.backward_steps.iter().rev() {
        // F1 (PR #384 round 1): the recorded backward term of each step IS the
        // state this replay must reach, so it is compared before the step is
        // applied. Its borrowed shape was admitted by the shared preflight.
        // F8: the replay state must be an INSTANCE of the recorded term
        // under the ONE correspondence accumulated so far; a constant or a term
        // of another shape is still a hard rejection.
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

    /// F28 (P2, PR #384 round 5): the shared admission meters ONE pool for every
    /// borrowed term AND every recorded unifier range.
    ///
    /// Four constant terms (source, goal, both meeting terms) and two unused
    /// constant ranges are six borrowed one-node values, i.e. exactly six
    /// operations. A budget of six passes; a budget of five rejects on
    /// operations although every individual term is legal. A fresh meter per
    /// range would admit both ranges inside the five-operation budget, so this
    /// contract is what pins that the aggregate admission is shared.
    #[test]
    fn the_shared_admission_charges_one_pool_for_terms_and_ranges() {
        let system = TermSystem::new(vec![]);
        let meeting = Term::constant("a");
        let unifier = Substitution::new()
            .with("U1", Term::constant("u1"))
            .with("U2", Term::constant("u2"));
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
            ResourceObservation {
                states: 1,
                transitions: 0,
                retained_bytes: 64,
                operations: 0,
            },
        );
        let exact_limits = RelationLimits::new(4_096, 64, 4_096, 6).expect("valid limits");
        let mut exact = RelationResources::new(&exact_limits);
        preflight_and_admit(&certificate, &mut exact)
            .expect("six one-node borrowed values fit exactly six operations");
        assert_eq!(
            exact.operations(),
            6,
            "four terms plus two ranges, from one shared pool"
        );

        let short_limits = RelationLimits::new(4_096, 64, 4_096, 5).expect("valid limits");
        let mut short = RelationResources::new(&short_limits);
        let error = preflight_and_admit(&certificate, &mut short)
            .expect_err("the shared meter cannot pay for the sixth value");
        assert_eq!(
            error,
            CertificateRejection::Certificate(RewriteError::RelationLimitExceeded {
                resource: "operations",
                limit: 5,
            }),
            "the aggregate admission is one pool, not one meter per range"
        );
    }
}
