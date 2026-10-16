// SPDX-License-Identifier: MIT OR Apache-2.0

//! Completion-assisted inverse search (feature `completion`).
//!
//! This module is the only place where bounded Knuth–Bendix
//! completion meets inverse authority. The discipline is the point:
//!
//! * A completion trace is EQUATIONAL-THEORY EVIDENCE ONLY. The
//!   convergent [`CompletionGuidance::completed`] system preserves
//!   the input's equational theory, not its directed reachability.
//! * Completed rules MAY generate candidate predecessor terms: a
//!   backward search against the completed system can surface
//!   candidates the original directed rules hide.
//! * EVERY positive candidate is REPLAYED against the ORIGINAL
//!   system. [`guided_predecessors`] only reports
//!   [`GuidedSearchOutcome::VerifiedWitness`] when the candidate is a
//!   genuine original predecessor and the returned `path` is an
//!   original directed derivation. A candidate that fails original
//!   replay is reported as [`CandidateRejection`] evidence, never as
//!   a witness.
//! * NO negative/exhaustion authority transfers. An empty or
//!   budget-cut completed-system search yields
//!   [`GuidedSearchOutcome::Inconclusive`]; no variant claims "no
//!   witness exists" from completed-system search alone.
//! * [`verify_equivalence`] may ADDITIONALLY support a claim with a
//!   both-direction [`RelationEquivalenceCertificate`]; it never
//!   replaces original-system replay, and [`guided_predecessors`]
//!   does not consume it.
//!
//! All work is bounded: every materialized term is charged against
//! the shared [`RelationResources`] budget BEFORE it is constructed,
//! and exhausting that budget is a typed
//! [`crate::RewriteError::RelationLimitExceeded`] error.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::vec::Vec;

use crate::analysis::Precedence;
use crate::completion::{complete, CompletionConfig, CompletionOutcome, CompletionTrace};
use crate::error::RewriteResult;
use crate::inverse::{system_hash, InverseSearchConfig};
use crate::relation::{RelationLimits, RelationResources, Sha256Digest};
use crate::trs::{match_pattern, Term, TermSystem};

use super::{matched_shape, shape_of, spine_shape};

/// The equational-evidence bundle produced by a convergent
/// completion run.
///
/// Only a `Complete` outcome carries guidance: the `completed` system
/// is then provably terminating, left-linear, and locally confluent,
/// so it is an honest convergent presentation of the input's
/// equations. A `Partial` or `Failed` run offers no sound evidence
/// and yields no guidance at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionGuidance {
    /// The convergent completed system (equational evidence).
    pub completed: TermSystem,
    /// The bounded completion trace that produced it.
    pub trace: CompletionTrace,
}

/// Run completion and keep the result only when it is convergent.
///
/// Returns `Some` exactly on [`CompletionOutcome::Complete`]. A
/// partial or failed run returns `None`: an intermediate or rejected
/// system is not equational evidence, and using it to guide inverse
/// search would be unsound. Budget exhaustion propagates as the typed
/// [`crate::RewriteError::RelationLimitExceeded`].
pub fn prepare_guidance(
    system: &TermSystem,
    precedence: Precedence,
    resources: &mut RelationResources,
) -> RewriteResult<Option<CompletionGuidance>> {
    let config = CompletionConfig::new(precedence);
    match complete(system, &config, resources)? {
        CompletionOutcome::Complete { system, trace } => Ok(Some(CompletionGuidance {
            completed: system,
            trace,
        })),
        CompletionOutcome::Partial { .. } | CompletionOutcome::Failed { .. } => Ok(None),
    }
}

/// A completed-system candidate that failed original-system replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CandidateRejection {
    /// The candidate predecessor term produced by the completed
    /// system is not reachable from the target under the ORIGINAL
    /// directed system within the configured bound. It is a
    /// completed-system artifact, never a witness.
    NotAnOriginalPredecessor {
        /// The rejected candidate predecessor term.
        predecessor: Term,
    },
}

/// Why a guided search is inconclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InconclusiveReason {
    /// The completed-system search produced no candidate
    /// predecessors within the configured bound. This is NOT a
    /// negative result: the completed system does not exhaust the
    /// original relation.
    NoCandidatesGenerated,
    /// A search was cut by the configured depth or state budget
    /// before it could decide the candidate set. No negative or
    /// positive conclusion follows.
    BudgetCut,
}

/// Typed outcome of a completion-guided predecessor search.
///
/// Every variant carries honest rejection accounting. No variant
/// claims that no predecessor exists: completed-system search holds
/// no negative authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GuidedSearchOutcome {
    /// A candidate replayed as a forward derivation through the
    /// ORIGINAL directed system.
    VerifiedWitness {
        /// The original forward derivation `predecessor → … →
        /// target`, in order.
        path: Vec<Term>,
        /// Candidates that failed original replay while scanning.
        candidates_rejected: Vec<CandidateRejection>,
    },
    /// The completed system proposed candidates, but none of them
    /// replayed against the original system. This says nothing about
    /// predecessors the completed search did not propose.
    NoVerifiedWitness {
        /// Every candidate, with its rejection reason.
        candidates_rejected: Vec<CandidateRejection>,
    },
    /// No sound positive or negative conclusion was reached.
    Inconclusive {
        /// Why the search is inconclusive.
        reason: InconclusiveReason,
        /// Candidates rejected before the inconclusive stop.
        candidates_rejected: Vec<CandidateRejection>,
    },
}

/// A directed derivation witnessing one equation in the other system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleDerivation {
    /// The equation's sides `(left, right)`.
    pub equation: (Term, Term),
    /// A forward derivation from one side to the other under the
    /// system being checked.
    pub path: Vec<Term>,
}

/// A both-direction relation-equivalence certificate.
///
/// The certificate records, for every rule equation of the first
/// system, the equal normal forms under the convergent system, and,
/// for every rule equation of the convergent system, a directed
/// derivation in the first system. It SUPPORTS but never replaces
/// original-system replay.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelationEquivalenceCertificate {
    /// Digest of the first system.
    pub a_hash: Sha256Digest,
    /// Digest of the convergent system.
    pub b_hash: Sha256Digest,
    /// For each rule of the first system, the `(lhs, rhs)` normal
    /// forms under the convergent system, which were found equal.
    pub a_normal_form_witnesses: Vec<(Term, Term)>,
    /// For each rule of the convergent system, a directed derivation
    /// in the first system.
    pub b_derivations: Vec<RuleDerivation>,
}

/// Search the completed system for candidate predecessors of
/// `target`, then require every positive candidate to replay against
/// the ORIGINAL system.
///
/// Candidates are the predecessor terms the convergent completed
/// system reaches backward from `target` within `config`. Each
/// candidate is accepted only if the original directed system also
/// reaches `target` from it within the same bound, in which case its
/// original forward `path` is returned. Everything else is reported
/// as [`CandidateRejection`] evidence. No negative authority transfers
/// from the completed search.
pub fn guided_predecessors(
    original: &TermSystem,
    guidance: &CompletionGuidance,
    target: &Term,
    config: &InverseSearchConfig,
    resources: &mut RelationResources,
) -> RewriteResult<GuidedSearchOutcome> {
    // A tampered or under-specified configuration must not run.
    config.validate()?;
    let (completed_candidates, completed_cut) =
        bounded_backward(&guidance.completed, target, config, resources)?;
    if completed_candidates.is_empty() {
        let reason = if completed_cut {
            InconclusiveReason::BudgetCut
        } else {
            InconclusiveReason::NoCandidatesGenerated
        };
        return Ok(GuidedSearchOutcome::Inconclusive {
            reason,
            candidates_rejected: Vec::new(),
        });
    }

    // Original-system replay: index every predecessor the original
    // system reaches backward from `target` by its alpha-canonical
    // digest, carrying the forward derivation.
    let (original_candidates, original_cut) =
        bounded_backward(original, target, config, resources)?;
    let original_index: BTreeMap<Sha256Digest, Vec<Term>> = original_candidates
        .into_iter()
        .map(|candidate| (canonical(&candidate.term), candidate.forward_path))
        .collect();

    let mut verified: Option<Vec<Term>> = None;
    let mut rejected: Vec<CandidateRejection> = Vec::new();
    for candidate in completed_candidates {
        match original_index.get(&canonical(&candidate.term)) {
            Some(path) => {
                if verified.is_none() {
                    verified = Some(path.clone());
                }
            }
            None => rejected.push(CandidateRejection::NotAnOriginalPredecessor {
                predecessor: candidate.term,
            }),
        }
    }

    if let Some(path) = verified {
        return Ok(GuidedSearchOutcome::VerifiedWitness {
            path,
            candidates_rejected: rejected,
        });
    }
    if original_cut {
        // The original replay was cut before it could decide: absence
        // of a match is not a rejection of the candidates.
        return Ok(GuidedSearchOutcome::Inconclusive {
            reason: InconclusiveReason::BudgetCut,
            candidates_rejected: rejected,
        });
    }
    Ok(GuidedSearchOutcome::NoVerifiedWitness {
        candidates_rejected: rejected,
    })
}

/// Verify a both-direction relation equivalence claim.
///
/// Returns `Some` only when:
///
/// 1. every rule equation of `a` has equal normal forms under the
///    convergent system `b_convergent` (bounded + metered
///    normalization); and
/// 2. every rule equation of `b_convergent` is directionally
///    derivable in `a` — unless `a` is itself known-convergent (which
///    this function does not assume), the honest check is bounded
///    directed derivability `l →* r` or `r →* l` under `a`, NOT
///    normal-form equality, because normalizing under a
///    non-convergent system is not well defined.
///
/// The certificate SUPPORTS an equivalence claim but never replaces
/// original-system replay in [`guided_predecessors`].
pub fn verify_equivalence(
    a: &TermSystem,
    b_convergent: &TermSystem,
    resources: &mut RelationResources,
) -> RewriteResult<Option<RelationEquivalenceCertificate>> {
    let mut a_normal_form_witnesses = Vec::new();
    for rule in a.rules() {
        let lhs_normal = metered_normalize(b_convergent, rule.lhs(), resources)?;
        let rhs_normal = metered_normalize(b_convergent, rule.rhs(), resources)?;
        if canonical(&lhs_normal) != canonical(&rhs_normal) {
            return Ok(None);
        }
        a_normal_form_witnesses.push((lhs_normal, rhs_normal));
    }

    let mut b_derivations = Vec::new();
    for rule in b_convergent.rules() {
        let path = match directed_path(a, rule.lhs(), rule.rhs(), resources)? {
            Some(path) => path,
            None => match directed_path(a, rule.rhs(), rule.lhs(), resources)? {
                Some(path) => path,
                None => return Ok(None),
            },
        };
        b_derivations.push(RuleDerivation {
            equation: (rule.lhs().clone(), rule.rhs().clone()),
            path,
        });
    }

    Ok(Some(RelationEquivalenceCertificate {
        a_hash: system_hash(a),
        b_hash: system_hash(b_convergent),
        a_normal_form_witnesses,
        b_derivations,
    }))
}

/// One candidate predecessor together with its forward derivation.
struct Candidate {
    term: Term,
    forward_path: Vec<Term>,
}

/// Alpha-canonical digest of a term under the completion-inverse
/// frame.
fn canonical(term: &Term) -> Sha256Digest {
    Sha256Digest::canonical_term("amari.completion.inverse.term/v1", term)
}

/// Bounded backward breadth-first enumeration.
///
/// Returns every distinct predecessor (by alpha-canonical digest)
/// reachable backward from `target` within the configured depth,
/// each with the forward derivation `predecessor → … → target`, plus
/// a flag that is set when a configured bound cut the enumeration.
/// Every retained term is charged against `resources` before it is
/// materialized.
fn bounded_backward(
    system: &TermSystem,
    target: &Term,
    config: &InverseSearchConfig,
    resources: &mut RelationResources,
) -> RewriteResult<(Vec<Candidate>, bool)> {
    let max_depth = config.max_depth();
    let max_states = config.max_states() as usize;
    let (nodes, depth) = shape_of(target);
    resources.record_term(nodes, depth)?;

    let mut visited: BTreeSet<Sha256Digest> = BTreeSet::new();
    visited.insert(canonical(target));
    let mut queue: VecDeque<(Term, Vec<Term>)> = VecDeque::new();
    queue.push_back((target.clone(), alloc::vec![target.clone()]));
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut cut = false;

    while let Some((term, path)) = queue.pop_front() {
        let depth = (path.len() - 1) as u64;
        if depth >= max_depth {
            cut = true;
            continue;
        }
        for predecessor in one_step_predecessors(system, &term, resources)? {
            if !visited.insert(canonical(&predecessor)) {
                continue;
            }
            if candidates.len() >= max_states {
                cut = true;
                break;
            }
            let mut forward_path = Vec::with_capacity(path.len() + 1);
            forward_path.push(predecessor.clone());
            forward_path.extend(path.iter().cloned());
            candidates.push(Candidate {
                term: predecessor.clone(),
                forward_path: forward_path.clone(),
            });
            queue.push_back((predecessor, forward_path));
        }
        if cut {
            break;
        }
    }
    Ok((candidates, cut))
}

/// One metered backward step: every rule whose RHS matches a subterm
/// yields the instantiated LHS predecessor. The replacement shape is
/// charged BEFORE the term is rebuilt.
fn one_step_predecessors(
    system: &TermSystem,
    term: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<Vec<Term>> {
    let mut out = Vec::new();
    for path in term.positions() {
        let Some(subterm) = term.subterm(&path) else {
            continue;
        };
        for rule in system.rules() {
            resources.record_operations(1)?;
            let Some(subst) = match_pattern(rule.rhs(), subterm) else {
                continue;
            };
            charge_replacement(term, &path, rule.lhs(), &subst, resources)?;
            let replacement = subst.apply_matcher(rule.lhs());
            let predecessor = term.replace_at(&path, replacement)?;
            if predecessor != *term {
                out.push(predecessor);
            }
        }
    }
    Ok(out)
}

/// One metered forward step: every rule whose LHS matches a subterm
/// yields its instantiated RHS successor. The replacement shape is
/// charged BEFORE the term is rebuilt.
fn one_step_successors(
    system: &TermSystem,
    term: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<Vec<Term>> {
    let mut out = Vec::new();
    for path in term.positions() {
        let Some(subterm) = term.subterm(&path) else {
            continue;
        };
        for rule in system.rules() {
            resources.record_operations(1)?;
            let Some(subst) = match_pattern(rule.lhs(), subterm) else {
                continue;
            };
            charge_replacement(term, &path, rule.rhs(), &subst, resources)?;
            let replacement = subst.apply_matcher(rule.rhs());
            let successor = term.replace_at(&path, replacement)?;
            if successor != *term {
                out.push(successor);
            }
        }
    }
    Ok(out)
}

/// Charge the exact shape of `term.replace_at(path, template[subst])`
/// before it is built: the spine outside `path` plus the instantiated
/// template. Saturating arithmetic fails closed.
fn charge_replacement(
    term: &Term,
    path: &crate::Path,
    template: &Term,
    subst: &crate::trs::Substitution,
    resources: &mut RelationResources,
) -> RewriteResult<()> {
    let (spine_nodes, spine_depth) = spine_shape(term, path);
    let (inner_nodes, inner_depth) = matched_shape(template, subst);
    let nodes = spine_nodes.saturating_add(inner_nodes);
    let depth = spine_depth.max(path.as_slice().len().saturating_add(inner_depth));
    resources.record_term(nodes, depth)
}

/// Bounded forward derivation search from `start` to `goal` under
/// `system`, metered against `resources`. The returned path starts at
/// `start` and ends at a term alpha-equivalent to `goal`.
fn directed_path(
    system: &TermSystem,
    start: &Term,
    goal: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<Option<Vec<Term>>> {
    let (nodes, depth) = shape_of(start);
    resources.record_term(nodes, depth)?;
    let goal_digest = canonical(goal);
    if canonical(start) == goal_digest {
        return Ok(Some(alloc::vec![start.clone()]));
    }
    // Directed derivability is bounded by the fixed term-depth
    // ceiling; the shared operation budget is the tighter bound.
    let max_depth = RelationLimits::MAX_TERM_DEPTH as u64;
    let mut visited: BTreeSet<Sha256Digest> = BTreeSet::new();
    visited.insert(canonical(start));
    let mut queue: VecDeque<(Term, Vec<Term>)> = VecDeque::new();
    queue.push_back((start.clone(), alloc::vec![start.clone()]));

    while let Some((term, path)) = queue.pop_front() {
        let depth = (path.len() - 1) as u64;
        if depth >= max_depth {
            continue;
        }
        for successor in one_step_successors(system, &term, resources)? {
            let digest = canonical(&successor);
            if !visited.insert(digest) {
                continue;
            }
            let mut next_path = path.clone();
            next_path.push(successor.clone());
            if digest == goal_digest {
                return Ok(Some(next_path));
            }
            queue.push_back((successor, next_path));
        }
    }
    Ok(None)
}

/// Bounded, metered normalization under `system`: repeatedly apply
/// the first outermost rewrite until a fixed point. The system must be
/// terminating (the caller passes a convergent system for the
/// certificate's forward direction); a non-terminating input is
/// stopped by the shared operation budget with a typed error.
fn metered_normalize(
    system: &TermSystem,
    term: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<Term> {
    let (nodes, depth) = shape_of(term);
    resources.record_term(nodes, depth)?;
    let mut current = term.clone();
    loop {
        resources.record_operations(1)?;
        match system.apply_once(&current)? {
            Some(next) => {
                let (nodes, depth) = shape_of(&next);
                resources.record_term(nodes, depth)?;
                current = next;
            }
            None => return Ok(current),
        }
    }
}
