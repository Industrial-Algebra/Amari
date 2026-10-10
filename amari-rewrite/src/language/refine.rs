// SPDX-License-Identifier: MIT OR Apache-2.0

//! Three-valued preimage membership and monotone refinement
//! (ADR 0001 §Obligations; Task 28).
//!
//! Task 27 supplies a sound witnessed lower bound for the cells the
//! ADR approves no exact construction for, and the exact preimage
//! automata for the approved cells. This module turns those
//! constructions into a membership verdict for a single ground term
//! and into a monotone refinement of a witnessed lower bound.
//!
//! # Verdicts
//!
//! [`MembershipVerdict`] is three-valued:
//!
//! - [`Proven`](MembershipVerdict::Proven) — the term is in the
//!   concrete preimage, established by an actual witness: membership
//!   in the exact preimage automaton for an exact cell, or membership
//!   in the Task 27 witnessed lower bound for an approximation cell.
//! - [`Excluded`](MembershipVerdict::Excluded) — the term is
//!   definitively NOT in the concrete preimage. This is available
//!   ONLY where the underlying construction is complete for the cell:
//!   an exact-approved cell (the canonical automaton is complete), or
//!   an approximation cell whose enumeration ran to completion (no
//!   [`ApproximationEvent::EnumerationTruncated`]), whose candidate
//!   domain covers the term (node count, depth, and every ranked
//!   symbol), and whose replay horizon is the operation's full
//!   semantics. `Saturation` is unbounded, so its replay is capped by
//!   the caller-supplied step budget derived below and NEVER yields
//!   `Excluded`.
//! - [`Unknown`](MembershipVerdict::Unknown) — everything else: the
//!   cell's construction was truncated, the term exceeds the
//!   enumerated domain, or no complete construction exists. The
//!   design's historical name for this state is `Possible`
//!   (`docs/plans/2026-07-24-amari-rewrite-inverse-expansion-design.md`);
//!   because ADR 0001 approves no sound upper construction, a
//!   "possible" term can never be excluded by an upper bound, so
//!   `Possible` and `Unknown` coincide and the module keeps the three
//!   sound variants.
//!
//! # Soundness pin
//!
//! `Excluded` is NEVER inferred from mere non-membership in a
//! truncated lower bound: truncation means unenumerated candidates
//! were never replayed, so their absence proves nothing.
//!
//! # Refinement
//!
//! [`refine_lower_bound`] re-runs the witnessed lower-bound
//! construction under a (presumably larger) limit profile and returns
//! the refined outcome only when its lower language is a SUPERSET of
//! the prior one. Any shrink — or a non-`None` upper where the prior
//! had none — is a typed [`RewriteError::RefinementViolation`], never
//! a silently returned result. The prior bound's language is exactly
//! its witnessed term set (Task 27 builds one state per distinct
//! witness subterm), so membership-testing every prior witness in the
//! refined automaton is a complete superset check; it runs against the
//! same per-query resource pool as the re-run.
//!
//! The refinement is "seeded by the prior outcome" in the sense that
//! the prior lower bound is the baseline the guard preserves: the
//! deterministic re-enumeration naturally re-admits prior witnesses
//! under larger limits, and the guard refuses any outcome that does
//! not. Prior witnesses are deliberately NOT injected, since that
//! would make the guard vacuous.
//!
//! All term traversals here are iterative (explicit worklists), so no
//! call-stack depth depends on caller terms.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::language::approximate::{
    candidate_alphabet, replay_term, witnessed_lower_bound_with_resources, DirectReplay,
};
use crate::language::certificate::PreimageCertificate;
use crate::language::classify::{classify_system, PreimageCapability, PreimageOperation};
use crate::language::preimage::{
    finite_horizon_preimage_with_resources, one_step_preimage_with_resources,
};
use crate::language::saturation::saturation_preimage_with_resources;
use crate::language::{
    ApproximationEvent, ApproximationOutcome, RankedSymbol, TreeAutomaton, TreeAutomatonLimits,
};
use crate::relation::{RelationLimits, RelationResources};
use crate::trs::{Term, TermSystem};

/// The three-valued membership verdict for a ground term against a
/// `(system, language, operation)` cell.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MembershipVerdict {
    /// The term is in the concrete preimage, established by an actual
    /// witness; carries the construction automaton that accepts it.
    Proven(TreeAutomaton),
    /// The term is definitively outside the concrete preimage. Only
    /// produced where the cell's construction is complete for the
    /// term's domain (exact cells, or a fully enumerated
    /// approximation with a full-semantics replay horizon).
    Excluded,
    /// The construction cannot decide the term: the enumeration was
    /// truncated, the term is outside the enumerated domain, or the
    /// operation's replay horizon is not its full semantics. The
    /// design's `Possible` state folds into this variant (see the
    /// module docs).
    Unknown,
}

impl MembershipVerdict {
    /// The accepting construction automaton, when the verdict is
    /// [`MembershipVerdict::Proven`].
    pub fn proven_automaton(&self) -> Option<&TreeAutomaton> {
        match self {
            MembershipVerdict::Proven(automaton) => Some(automaton),
            MembershipVerdict::Excluded | MembershipVerdict::Unknown => None,
        }
    }
}

/// A membership query result: the verdict, the operation it answers,
/// the evidence certificate, and the construction trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MembershipQuery {
    verdict: MembershipVerdict,
    operation: PreimageOperation,
    certificate: PreimageCertificate,
    trace: Vec<ApproximationEvent>,
}

impl MembershipQuery {
    /// The three-valued verdict.
    pub fn verdict(&self) -> &MembershipVerdict {
        &self.verdict
    }

    /// The operation the verdict answers.
    pub fn operation(&self) -> PreimageOperation {
        self.operation
    }

    /// The evidence certificate binding the inputs, limits, and (for
    /// exact and approximation cells) the construction result.
    pub fn certificate(&self) -> &PreimageCertificate {
        &self.certificate
    }

    /// The construction trace. Empty for exact cells; the Task 27
    /// approximation trace otherwise.
    pub fn trace(&self) -> &[ApproximationEvent] {
        &self.trace
    }
}

/// Classify the ground term `term` against `(system, language,
/// operation)`, returning the verdict, evidence certificate, and
/// construction trace.
///
/// Exact-approved cells run the approved exact construction and decide
/// membership in its canonical automaton. Approximation-only cells run
/// the Task 27 witnessed lower bound under the caller's limits and
/// decide membership in the witnessed set; `Excluded` is returned only
/// when the enumeration completed, the term lies inside the enumerated
/// domain, and the operation's replay horizon is its full semantics.
///
/// For `Saturation` the concrete relation is unbounded; the witnessed
/// replay is capped by a documented default horizon and `Excluded` is
/// never returned.
pub fn classify_and_query(
    system: &TermSystem,
    language: &TreeAutomaton,
    operation: PreimageOperation,
    term: &Term,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> RewriteResult<MembershipQuery> {
    let classification = classify_system(system)?;
    match classification.capability(operation) {
        PreimageCapability::Exact(_) => {
            exact_query(system, language, operation, term, limits, automaton_limits)
        }
        PreimageCapability::ApproximationOnly => {
            approximation_query(system, language, operation, term, limits, automaton_limits)
        }
    }
}

/// Membership in an approved exact construction's canonical automaton.
fn exact_query(
    system: &TermSystem,
    language: &TreeAutomaton,
    operation: PreimageOperation,
    term: &Term,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> RewriteResult<MembershipQuery> {
    // ONE shared pool across construction and membership: the
    // per-query ceiling covers both (PR #284 round 1 P2).
    let mut resources = RelationResources::new(limits);
    let outcome = match operation {
        PreimageOperation::OneStep => one_step_preimage_with_resources(
            system,
            language,
            limits,
            automaton_limits,
            &mut resources,
        )?,
        PreimageOperation::FiniteHorizon(horizon) => finite_horizon_preimage_with_resources(
            system,
            language,
            horizon,
            limits,
            automaton_limits,
            &mut resources,
        )?,
        PreimageOperation::Saturation => saturation_preimage_with_resources(
            system,
            language,
            limits,
            automaton_limits,
            &mut resources,
        )?,
    };
    let (automaton, certificate) = outcome.into_parts();
    let member = automaton.accepts_with_resources(term, &mut resources)?;
    let verdict = if member {
        MembershipVerdict::Proven(automaton)
    } else {
        MembershipVerdict::Excluded
    };
    Ok(MembershipQuery {
        verdict,
        operation,
        certificate,
        trace: Vec::new(),
    })
}

/// Membership in the Task 27 witnessed lower bound, with the sound
/// `Excluded` gate.
fn approximation_query(
    system: &TermSystem,
    language: &TreeAutomaton,
    operation: PreimageOperation,
    term: &Term,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> RewriteResult<MembershipQuery> {
    let max_steps = operation_step_budget(operation, limits);
    let mut resources = RelationResources::new(limits);
    let outcome = witnessed_lower_bound_with_resources(
        system,
        language,
        operation,
        max_steps,
        limits,
        automaton_limits,
        &mut resources,
    )?;
    let candidate = candidate_alphabet(system, language);
    let in_domain = term_in_domain(term, &candidate, limits);
    let lower = outcome.lower().clone();
    let member = if in_domain {
        lower.accepts_with_resources(term, &mut resources)?
    } else {
        false
    };
    // The exclusion gate additionally requires a COMPLETE direct
    // replay of the query term itself (PR #284 round 1 P1): an
    // untruncated enumeration is not enough, because a replay can
    // silently discard evidence — an oversized successor is never
    // explored further, and a membership evaluation that hits a
    // per-term bound yields no answer. Without completeness,
    // non-membership is `Unknown`, never `Excluded`.
    let direct = if in_domain {
        replay_term(
            system,
            language,
            term,
            max_steps,
            operation_min_steps(operation),
            &mut resources,
            limits,
        )?
    } else {
        DirectReplay { complete: false }
    };
    let complete = in_domain
        && operation_covers_full_semantics(operation)
        && !outcome
            .trace()
            .iter()
            .any(|event| matches!(event, ApproximationEvent::EnumerationTruncated { .. }))
        && direct.complete;
    let verdict = if member {
        MembershipVerdict::Proven(lower)
    } else if complete {
        MembershipVerdict::Excluded
    } else {
        MembershipVerdict::Unknown
    };
    Ok(MembershipQuery {
        verdict,
        operation,
        certificate: outcome.certificate().clone(),
        trace: outcome.trace().to_vec(),
    })
}

/// The minimum number of applications a witness needs: exactly one
/// for `OneStep` (no reflexive closure), zero otherwise.
fn operation_min_steps(operation: PreimageOperation) -> u32 {
    match operation {
        PreimageOperation::OneStep => 1,
        PreimageOperation::FiniteHorizon(_) | PreimageOperation::Saturation => 0,
    }
}

/// The replay step budget a query derives from an operation. `OneStep`
/// is exactly one application; `FiniteHorizon(n)` is its full horizon;
/// `Saturation` is unbounded, so its witnessed replay uses the
/// term-node ceiling as the documented default horizon (and never
/// yields `Excluded`).
fn operation_step_budget(operation: PreimageOperation, limits: &RelationLimits) -> u32 {
    match operation {
        PreimageOperation::OneStep => 1,
        PreimageOperation::FiniteHorizon(horizon) => horizon,
        PreimageOperation::Saturation => u32::try_from(limits.max_term_nodes()).unwrap_or(u32::MAX),
    }
}

/// Whether non-membership under the operation's replay horizon is a
/// definitive exclusion. `Saturation`'s concrete relation is unbounded,
/// so a bounded replay can never exclude.
fn operation_covers_full_semantics(operation: PreimageOperation) -> bool {
    match operation {
        PreimageOperation::OneStep | PreimageOperation::FiniteHorizon(_) => true,
        PreimageOperation::Saturation => false,
    }
}

/// Whether a ground term lies inside the enumerated candidate domain:
/// within the term node and depth ceilings, built only from ranked
/// symbols of the candidate alphabet. Iterative (explicit worklist).
fn term_in_domain(term: &Term, candidate: &[RankedSymbol], limits: &RelationLimits) -> bool {
    let mut nodes = 0usize;
    let mut depth = 0usize;
    let mut stack: Vec<(&Term, usize)> = Vec::from([(term, 0usize)]);
    while let Some((node, level)) = stack.pop() {
        nodes += 1;
        if level > depth {
            depth = level;
        }
        if nodes > limits.max_term_nodes() || depth > limits.max_term_depth() {
            return false;
        }
        match node {
            Term::Var(_) => return false,
            Term::Sym(symbol, arguments) => {
                let arity = arguments.len() as u16;
                let known = candidate
                    .iter()
                    .any(|ranked| ranked.symbol() == symbol && ranked.arity() == arity);
                if !known {
                    return false;
                }
                for argument in arguments {
                    stack.push((argument, level + 1));
                }
            }
        }
    }
    true
}

/// Re-run the witnessed lower bound for `operation` under `limits`,
/// returning the refined outcome only when its lower language contains
/// the prior one.
///
/// The prior outcome's language is exactly its witnessed term set, so
/// membership-testing every prior witness in the refined automaton is
/// a complete superset check. A shrink (or a newly materialized upper
/// bound) is a typed [`RewriteError::RefinementViolation`]; the prior
/// outcome — borrowed, and never mutated — is preserved.
pub fn refine_lower_bound(
    system: &TermSystem,
    language: &TreeAutomaton,
    operation: PreimageOperation,
    prior: &ApproximationOutcome,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> RewriteResult<ApproximationOutcome> {
    if prior.certificate().operation() != operation {
        return Err(RewriteError::RefinementViolation {
            message: format!(
                "prior outcome covers {:?}, not {:?}",
                prior.certificate().operation(),
                operation
            ),
        });
    }
    let max_steps = operation_step_budget(operation, limits);
    let mut resources = RelationResources::new(limits);
    let refined = witnessed_lower_bound_with_resources(
        system,
        language,
        operation,
        max_steps,
        limits,
        automaton_limits,
        &mut resources,
    )?;
    let candidate = candidate_alphabet(system, language);
    for witness in prior.witnesses() {
        if !term_in_domain(witness, &candidate, limits)
            || !refined
                .lower()
                .accepts_with_resources(witness, &mut resources)?
        {
            return Err(RewriteError::RefinementViolation {
                message: format!(
                    "refinement dropped prior witness {witness:?}; a \
                     refinement must not shrink the lower bound"
                ),
            });
        }
    }
    if refined.upper().is_some() && prior.upper().is_none() {
        return Err(RewriteError::RefinementViolation {
            message: String::from("refinement produced an upper bound the prior lacked"),
        });
    }
    Ok(refined)
}
