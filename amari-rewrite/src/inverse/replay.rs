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
                replay_backward(system, target, goal, derivation)
            }
            (
                ReplayQuery::Bidirectional { source, goal },
                ReplayDerivation::Bidirectional(derivation),
            ) => replay_bidirectional(system, source, goal, derivation),
            _ => Err(mismatch("query kind does not match derivation kind")),
        }
    }
}

/// Replay a backward derivation forward to its target and confirm the
/// recorded trace under ONE assignment.
///
/// Acceptance invariant (PR #384 round 2, F7): the leaf/goal unifier is the
/// certificate's single assignment. It is retained, and every comparison —
/// each recorded step term against the state the replay reaches, and the
/// reconstructed endpoint against the declared target — is made after applying
/// that one assignment to both sides. So a certificate is admitted only when
/// ONE instantiation of its variables makes the recorded trace denote the
/// declared query: a discarded leaf/goal unifier would let the replay
/// reconstruct an endpoint that merely alpha-matches the target (the reviewer's
/// `h(R)` against `h(Q)` under `Q := a`, `R := b`), which is a different term
/// instance and is now a typed mismatch. Variables the unifier does not fix
/// stay free, so valid symbolic certificates are still admitted (comparison is
/// alpha-invariant, as before).
fn replay_backward(
    system: &TermSystem,
    target: &Term,
    goal: &Term,
    derivation: &BackwardDerivation,
) -> RewriteResult<()> {
    let mismatch = |message: &str| RewriteError::ResidualMismatch {
        message: message.to_string(),
    };
    let canonical = |term: &Term| Sha256Digest::canonical_term("amari.relation.term/v1", term);
    let limits = RelationLimits::default();
    let mut resources = RelationResources::new(&limits);
    if derivation.steps.is_empty() {
        // Zero-step witness: target must unify with the goal.
        return unify(target, goal, &mut resources)
            .map(|_| ())
            .map_err(|_| mismatch("zero-step witness does not unify"));
    }
    let leaf = &derivation.steps.last().expect("non-empty").term;
    // F7 (PR #384 round 2): the leaf/goal unifier is retained, never discarded.
    // It is the one assignment every reconstructed term is read under.
    let assignment = unify(leaf, goal, &mut resources)
        .map_err(|_| mismatch("derivation leaf does not unify with the goal"))?;
    // Instantiation is metered against the same relation budget the unifier
    // consumed, so a substitution cannot inflate a borrowed term past the
    // crate's hard node/depth ceilings unobserved.
    let instantiate = |term: &Term, resources: &mut RelationResources| {
        let instantiated = assignment.apply(term);
        resources.record_term(
            super::term_nodes(&instantiated),
            super::term_depth(&instantiated),
        )?;
        Ok::<Term, RewriteError>(instantiated)
    };
    let mut current = leaf.clone();
    for step in derivation.steps.iter().rev() {
        // F1 (PR #384 round 1): the recorded term of each step IS the state
        // this replay must reach, so it is compared before the step is applied.
        // Without this, a certificate whose recorded intermediate terms were
        // replaced (no hash touched) would replay to the right endpoint and
        // still be admitted as trace content. F7 (round 2): both sides of that
        // comparison are read under the ONE leaf/goal assignment.
        if canonical(&instantiate(&current, &mut resources)?)
            != canonical(&instantiate(&step.term, &mut resources)?)
        {
            return Err(mismatch(
                "replayed state does not match the recorded step term",
            ));
        }
        let rule = system
            .rules()
            .iter()
            .find(|rule| RuleId::from_rule(rule) == step.provenance.rule_id)
            .ok_or_else(|| mismatch("step references an unknown rule"))?;
        let position = current
            .subterm(&step.provenance.position)
            .ok_or_else(|| mismatch("replay lost the recorded path"))?;
        let bindings = match_pattern(rule.lhs(), position)
            .ok_or_else(|| mismatch("replay failed to match the lhs"))?;
        current = current
            .replace_at(
                &step.provenance.position,
                bindings.apply_matcher(rule.rhs()),
            )
            .map_err(|_| mismatch("replay failed to rebuild the term"))?;
    }
    // The reconstructed endpoint and the declared target are compared under
    // the same single assignment, so the certificate commits to exactly one
    // endpoint instance.
    if canonical(&instantiate(&current, &mut resources)?)
        != canonical(&instantiate(target, &mut resources)?)
    {
        return Err(mismatch("replay does not reconstruct the target"));
    }
    Ok(())
}

/// Replay a bidirectional derivation: forward half from the source
/// to the meeting term, then the instantiated backward half to the
/// goal.
///
/// Acceptance invariant (PR #384 round 2, F8): the whole backward half and the
/// goal/endpoint check are read through ONE recorded-term-to-replay
/// correspondence. Each successive match must EXTEND the correspondence, so a
/// recorded variable shared by two records (the reviewer's `[g(Z), f(Z)]`) must
/// denote the same value everywhere; a conflicting binding is a typed mismatch.
/// Genuine generalization is still accepted — `Z := a` for `f(Z)` and `g(Z)`
/// and the goal — because one substitution instantiates the entire recorded
/// trace. Independent per-record matches, and per-term digest comparisons, are
/// exactly what this replaces.
fn replay_bidirectional(
    system: &TermSystem,
    source: &Term,
    goal: &Term,
    derivation: &BidirectionalDerivation,
) -> RewriteResult<()> {
    let mismatch = |message: &str| RewriteError::ResidualMismatch {
        message: message.to_string(),
    };
    let canonical = |term: &Term| Sha256Digest::canonical_term("amari.relation.term/v1", term);
    let mut current = source.clone();
    for step in &derivation.forward_steps {
        let rule = system
            .rules()
            .iter()
            .find(|rule| RuleId::from_rule(rule) == step.rule_id)
            .ok_or_else(|| mismatch("forward step references unknown rule"))?;
        let position = current
            .subterm(&step.position)
            .ok_or_else(|| mismatch("replay lost the forward path"))?;
        let bindings = match_pattern(rule.lhs(), position)
            .ok_or_else(|| mismatch("replay failed the forward lhs"))?;
        current = current
            .replace_at(&step.position, bindings.apply_matcher(rule.rhs()))
            .map_err(|_| mismatch("replay failed a forward rebuild"))?;
    }
    if canonical(&current) != canonical(&derivation.meeting_forward) {
        return Err(mismatch("forward half does not reach the meeting term"));
    }
    let mut current = derivation.unifier.apply(&derivation.meeting_backward);
    // F8 (PR #384 round 2): ONE correspondence, extended by every record and
    // carried into the goal check. Bindings are subterms of already-borrowed
    // terms, so no new term shape is constructed here.
    let mut correspondence = Substitution::new();
    for step in derivation.backward_steps.iter().rev() {
        // F1 (PR #384 round 1): the recorded backward term of each step IS the
        // state this replay must reach, so it is compared before the step is
        // applied. The recorded terms carry freshened logic variables while the
        // replay starts from the meeting substitution's instantiation, so the
        // acceptance shape is the same one the goal endpoint uses: the replayed
        // state must be an INSTANCE of the recorded term — under the one
        // correspondence accumulated so far. A constant or a term of another
        // shape is still a hard rejection, so a fabricated intermediate term
        // cannot be certified, and a variable bound to two different instances
        // by two records is a hard rejection too (F8).
        if !match_pattern_into(&mut correspondence, &step.term, &current) {
            return Err(mismatch(
                "replayed state does not match the recorded backward term",
            ));
        }
        let rule = system
            .rules()
            .iter()
            .find(|rule| RuleId::from_rule(rule) == step.provenance.rule_id)
            .ok_or_else(|| mismatch("backward step references unknown rule"))?;
        let position = current
            .subterm(&step.provenance.position)
            .ok_or_else(|| mismatch("replay lost the backward path"))?;
        let bindings = match_pattern(rule.lhs(), position)
            .ok_or_else(|| mismatch("replay failed the backward lhs"))?;
        current = current
            .replace_at(
                &step.provenance.position,
                bindings.apply_matcher(rule.rhs()),
            )
            .map_err(|_| mismatch("replay failed a backward rebuild"))?;
    }
    // The backward half starts from the meeting substitution's
    // instantiation of the meeting term, so a goal whose variables
    // were bound by the meet reconstructs to an INSTANCE of the
    // goal, not the raw goal. Acceptance is pattern matching with
    // the goal as the pattern — extended into the SAME correspondence, so the
    // goal's variables share the recorded trace's instantiation.
    if !match_pattern_into(&mut correspondence, goal, &current) {
        return Err(mismatch(
            "replay does not reconstruct an instance of the goal",
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
