// SPDX-License-Identifier: MIT OR Apache-2.0

//! Witnessed lower bounds for approximation-only preimage cells
//! (ADR 0001 §Obligations; Task 27).
//!
//! ADR 0001 approves NO general upper construction. The only approved
//! under-approximation is a FINITE REPLAY: enumerate ground candidate
//! terms, replay the application relation
//! ([`TermSystem::application_successors`], self-steps included) for a
//! bounded number of steps, and admit exactly those that reach the
//! input language. Soundness is by construction — every admitted
//! witness is individually replayed — so the bound carries
//! [`CertificateAuthority::Partial`] and the upper bound is always
//! `None`.
//!
//! # Class gate
//!
//! The surface exists only for class/operation cells the capability
//! table marks approximation-only. A cell with an approved exact
//! construction is rejected with [`RewriteError::UnsupportedPreimage`]
//! naming the exact entry point.
//!
//! # Enumeration and metering
//!
//! Candidates are the ground terms over the candidate alphabet (every
//! ranked symbol on some rule's left side, unioned with the language's
//! ranked alphabet). They are enumerated in increasing node-count
//! order; ties use a fixed deterministic order (root symbol in ranked
//! order, then the fixed child-size-composition and recursive order).
//! Every constructed node charges operations and, once retained, a
//! relation-storage constraint; every replayed successor and every
//! membership test charges operations. The enumeration's effective
//! ceiling is the configured limit MINUS a documented assembly
//! headroom (one eighth of the limit when the limit is at least 8):
//! when a metered resource crosses that ceiling the enumeration STOPS
//! — an [`ApproximationEvent::EnumerationTruncated`] is recorded and
//! the witnesses collected so far are returned. Enumeration
//! exhaustion is never an error. Output assembly (DAG construction +
//! canonicalize) draws from the SAME shared pool against the FULL
//! limit — the reserved headroom is what makes a truncated outcome
//! affordable — and assembly exhaustion IS a typed
//! [`RewriteError::RelationLimitExceeded`]. Total accounted work
//! never exceeds the caller's per-query limits.
//!
//! The input contract here differs from the exact constructions: the
//! candidate alphabet is a union, so a rule left side may use a symbol
//! the language alphabet does not mention. The shared-alphabet clause
//! is therefore not required for a sound replay (the language
//! membership test simply finds no transition for a foreign symbol).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::language::certificate::PreimageCertificate;
use crate::language::classify::{classify_system, PreimageCapability, PreimageOperation};
use crate::language::preimage::canonicalize;
use crate::language::{
    RankedSymbol, TreeAutomaton, TreeAutomatonLimits, TreeState, TreeTransition,
};
use crate::relation::{RelationLimits, RelationResources};
use crate::trs::{Symbol, Term, TermSystem};

/// One entry of an [`ApproximationOutcome`] trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApproximationEvent {
    /// A ground witness was replayed into the input language and
    /// retained in the lower-bound automaton. `term_size` is the
    /// witness's node count.
    WitnessAdmitted {
        /// Node count of the admitted witness.
        term_size: usize,
    },
    /// Enumeration stopped because a metered resource was exhausted;
    /// the lower bound is the witnesses admitted before the stop.
    EnumerationTruncated {
        /// The exhausted resource (`"operations"` or `"constraints"`).
        resource: &'static str,
        /// The configured ceiling for that resource.
        limit: usize,
    },
    /// No sound upper construction is approved for the class
    /// (ADR 0001 §Obligations); `upper()` is therefore `None`.
    UpperUnavailable {
        /// A short fixed explanation of the absent upper bound.
        reason: &'static str,
    },
}

/// The result of a witnessed lower-bound approximation: the sound
/// under-approximation automaton, the (always absent) upper bound, the
/// construction trace, and the `Partial`-authority certificate.
///
/// # Item Scope
///
/// Borrows the lower bound, the optional upper bound, the trace, and
/// the certificate. The upper bound is `None` in this revision because
/// ADR 0001 approves no sound upper construction.
#[derive(Clone, Debug)]
pub struct ApproximationOutcome {
    lower: TreeAutomaton,
    upper: Option<TreeAutomaton>,
    trace: Vec<ApproximationEvent>,
    certificate: PreimageCertificate,
}

impl ApproximationOutcome {
    /// The canonicalized lower-bound automaton (the admitted witnesses).
    pub fn lower(&self) -> &TreeAutomaton {
        &self.lower
    }

    /// The upper bound, always `None` in this revision.
    pub fn upper(&self) -> Option<&TreeAutomaton> {
        self.upper.as_ref()
    }

    /// The construction trace (admissions, truncation, absent upper).
    pub fn trace(&self) -> &[ApproximationEvent] {
        &self.trace
    }

    /// The completed `Partial`-authority certificate binding the
    /// inputs, limits, construction, and lower result.
    pub fn certificate(&self) -> &PreimageCertificate {
        &self.certificate
    }
}

/// Approximate the one-step preimage `R⁻¹(L)` from below by replaying
/// every enumerated ground candidate for at most one application step.
///
/// Returns [`RewriteError::UnsupportedPreimage`] when the class has an
/// approved exact one-step construction (use [`one_step_preimage`]).
///
/// [`one_step_preimage`]: crate::language::one_step_preimage
pub fn one_step_lower_bound(
    system: &TermSystem,
    language: &TreeAutomaton,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> RewriteResult<ApproximationOutcome> {
    witnessed_lower_bound(
        system,
        language,
        PreimageOperation::OneStep,
        1,
        limits,
        automaton_limits,
    )
}

/// Approximate the finite-horizon preimage `(R⁻¹)^≤horizon(L)` from
/// below by replaying every enumerated ground candidate for at most
/// `horizon` application steps.
///
/// `horizon = 0` is the identity, which every class approves exactly,
/// so it is rejected with [`RewriteError::UnsupportedPreimage`]
/// directing the caller to [`finite_horizon_preimage`].
///
/// [`finite_horizon_preimage`]: crate::language::finite_horizon_preimage
pub fn finite_horizon_lower_bound(
    system: &TermSystem,
    language: &TreeAutomaton,
    horizon: u32,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> RewriteResult<ApproximationOutcome> {
    witnessed_lower_bound(
        system,
        language,
        PreimageOperation::FiniteHorizon(horizon),
        horizon,
        limits,
        automaton_limits,
    )
}

/// Approximate the unbounded saturation preimage `R*(L)` from below by
/// replaying every enumerated ground candidate for at most `max_steps`
/// application steps.
///
/// Returns [`RewriteError::UnsupportedPreimage`] when the class has an
/// approved exact saturation construction (use [`saturation_preimage`]).
///
/// [`saturation_preimage`]: crate::language::saturation_preimage
pub fn saturation_lower_bound(
    system: &TermSystem,
    language: &TreeAutomaton,
    max_steps: u32,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> RewriteResult<ApproximationOutcome> {
    witnessed_lower_bound(
        system,
        language,
        PreimageOperation::Saturation,
        max_steps,
        limits,
        automaton_limits,
    )
}

/// The shared witnessed-lower-bound engine.
fn witnessed_lower_bound(
    system: &TermSystem,
    language: &TreeAutomaton,
    operation: PreimageOperation,
    max_steps: u32,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> RewriteResult<ApproximationOutcome> {
    // Class gate: refuse cells the capability table approves exactly.
    let classification = classify_system(system)?;
    if let PreimageCapability::Exact(_) = classification.capability(operation) {
        return Err(RewriteError::UnsupportedPreimage {
            message: format!(
                "class {:?} has an approved exact construction for {:?}; use {} (ADR 0001)",
                classification.class(),
                operation,
                exact_entry_point(operation)
            ),
        });
    }
    let certificate = PreimageCertificate::issue_partial(
        operation,
        system,
        language,
        limits,
        "witnessed finite replay; no exact construction is approved for this cell",
    )?;

    let candidate_alphabet = candidate_alphabet(system, language);
    // Right-side shape, precomputed once per call: (lhs nodes,
    // non-variable rhs nodes, rhs variable occurrences). The successor
    // preflight bills variable-binding expansion and per-position
    // multiplicity from these (review round 3).
    let rule_shapes: Vec<(usize, usize, usize)> = system
        .rules()
        .iter()
        .map(|rule| {
            (
                term_nodes(rule.lhs()),
                term_nodes(rule.rhs()).saturating_sub(variable_occurrences(rule.rhs())),
                variable_occurrences(rule.rhs()),
            )
        })
        .collect();
    // The one-step relation is exactly one application (no reflexive
    // closure); finite horizon and saturation include zero steps.
    let min_steps = match operation {
        PreimageOperation::OneStep => 1,
        PreimageOperation::FiniteHorizon(_) | PreimageOperation::Saturation => 0,
    };
    let mut trace: Vec<ApproximationEvent> = Vec::new();
    let mut resources = RelationResources::new(limits);
    let witnesses = if candidate_alphabet.iter().any(|ranked| ranked.arity() == 0) {
        let mut witnesses: BTreeSet<Term> = BTreeSet::new();
        match enumerate_witnesses(
            system,
            language,
            &candidate_alphabet,
            max_steps,
            min_steps,
            &rule_shapes,
            &mut resources,
            limits,
            &mut witnesses,
            &mut trace,
        ) {
            Ok(()) => {}
            Err(Stop::Truncated { resource, limit }) => {
                trace.push(ApproximationEvent::EnumerationTruncated { resource, limit });
            }
            Err(Stop::Hard(error)) => return Err(error),
        }
        witnesses
    } else {
        // No nullary symbol: there are no ground candidate terms.
        BTreeSet::new()
    };

    // ADR 0001 approves no upper construction: the bound is always
    // absent, and the trace says why.
    trace.push(ApproximationEvent::UpperUnavailable {
        reason: "no sound upper construction is approved for this class (ADR 0001 §Obligations)",
    });

    // Output assembly draws from the SAME shared pool (one per-query
    // budget, review round 2): the determinization odometer is
    // metered per step (determinize.rs), so a runaway construction
    // trips the operation budget with a typed `RelationLimitExceeded`
    // — but that also means exhaustion during assembly is a typed
    // error, and truncation-with-Ok survives only when the truncated
    // witness set assembles within the budget the enumeration left
    // unspent. Total accounted work never exceeds the caller's
    // configured per-query limits. The automaton ceilings in
    // `automaton_limits` still apply.
    let lower = build_lower_automaton(
        &witnesses,
        &candidate_alphabet,
        language,
        automaton_limits,
        &mut resources,
    )?;
    let certificate = certificate.complete(&lower);
    Ok(ApproximationOutcome {
        lower,
        upper: None,
        trace,
        certificate,
    })
}

/// The exact entry point a rejected approximation cell should use.
fn exact_entry_point(operation: PreimageOperation) -> &'static str {
    match operation {
        PreimageOperation::OneStep => "one_step_preimage",
        PreimageOperation::FiniteHorizon(_) => "finite_horizon_preimage",
        PreimageOperation::Saturation => "saturation_preimage",
    }
}

/// Why the enumeration loop stopped early.
enum Stop {
    /// A metered resource was exhausted: stop and return the witnesses
    /// collected so far.
    Truncated {
        resource: &'static str,
        limit: usize,
    },
    /// A hard construction violation.
    Hard(RewriteError),
}

/// The assembly headroom reserved under a configured limit: the
/// enumeration stops at limit − reserve so the output automaton can
/// be assembled within the same per-query budget (review round 2).
fn headroom_reserve(limit: usize) -> usize {
    if limit >= 8 {
        limit / 8
    } else {
        0
    }
}

/// Charge `count` operations after a checked preflight; exhaustion is
/// a [`Stop::Truncated`], never an error.
fn charge_operations(
    resources: &mut RelationResources,
    limit: usize,
    count: usize,
) -> Result<(), Stop> {
    let ceiling = limit.saturating_sub(headroom_reserve(limit));
    let exhausted = resources
        .operations()
        .checked_add(count)
        .is_none_or(|next| next > ceiling);
    if exhausted {
        return Err(Stop::Truncated {
            resource: "operations",
            limit,
        });
    }
    resources.record_operations(count).map_err(Stop::Hard)
}

/// Charge `count` retained relation-storage cells after a checked
/// preflight; exhaustion is a [`Stop::Truncated`], never an error.
fn charge_constraints(
    resources: &mut RelationResources,
    limit: usize,
    count: usize,
) -> Result<(), Stop> {
    let ceiling = limit.saturating_sub(headroom_reserve(limit));
    let exhausted = resources
        .constraints()
        .checked_add(count)
        .is_none_or(|next| next > ceiling);
    if exhausted {
        return Err(Stop::Truncated {
            resource: "constraints",
            limit,
        });
    }
    resources.record_constraints(count).map_err(Stop::Hard)
}

/// The candidate ranked alphabet: every ranked symbol on a rule left
/// side, unioned with the language alphabet, in canonical order.
fn candidate_alphabet(system: &TermSystem, language: &TreeAutomaton) -> Vec<RankedSymbol> {
    let mut ranked: BTreeSet<RankedSymbol> = BTreeSet::new();
    for entry in language.alphabet() {
        ranked.insert(entry.clone());
    }
    for rule in system.rules() {
        let mut stack = vec![rule.lhs()];
        while let Some(node) = stack.pop() {
            if let Term::Sym(symbol, arguments) = node {
                ranked.insert(RankedSymbol::new(symbol.clone(), arguments.len() as u16));
                stack.extend(arguments.iter());
            }
        }
    }
    ranked.into_iter().collect()
}

/// Enumerate candidates by increasing node count and retain the
/// individually replayed witnesses.
#[allow(clippy::too_many_arguments)]
fn enumerate_witnesses(
    system: &TermSystem,
    language: &TreeAutomaton,
    alphabet: &[RankedSymbol],
    max_steps: u32,
    min_steps: u32,
    rule_shapes: &[(usize, usize, usize)],
    resources: &mut RelationResources,
    limits: &RelationLimits,
    witnesses: &mut BTreeSet<Term>,
    trace: &mut Vec<ApproximationEvent>,
) -> Result<(), Stop> {
    let max_size = limits.max_term_nodes();
    let mut by_size: Vec<Vec<Term>> = vec![Vec::new(); max_size + 1];
    for size in 1..=max_size {
        let terms = build_size(alphabet, size, &by_size, resources, limits)?;
        for term in &terms {
            if term_depth(term) > limits.max_term_depth() {
                continue;
            }
            if replays_into(
                system,
                language,
                term,
                max_steps,
                min_steps,
                rule_shapes,
                resources,
                limits,
            )? {
                let nodes = term_nodes(term);
                charge_constraints(resources, limits.max_constraints(), 1 + nodes)?;
                if witnesses.insert(term.clone()) {
                    trace.push(ApproximationEvent::WitnessAdmitted { term_size: nodes });
                }
            }
        }
        by_size[size] = terms;
    }
    Ok(())
}

/// Build every ground term of exactly `size` nodes over `alphabet` in a
/// fixed deterministic order.
fn build_size(
    alphabet: &[RankedSymbol],
    size: usize,
    by_size: &[Vec<Term>],
    resources: &mut RelationResources,
    limits: &RelationLimits,
) -> Result<Vec<Term>, Stop> {
    let mut out: Vec<Term> = Vec::new();
    for ranked in alphabet {
        let arity = usize::from(ranked.arity());
        if arity == 0 {
            if size == 1 {
                charge_operations(resources, limits.max_operations(), 1)?;
                charge_constraints(resources, limits.max_constraints(), 2)?;
                out.push(Term::constant(ranked.symbol().clone()));
            }
            continue;
        }
        if arity + 1 > size {
            continue;
        }
        let compositions = compositions(size - 1, arity, resources, limits)?;
        for composition in &compositions {
            let mut prefix: Vec<Term> = Vec::with_capacity(arity);
            fill_product(
                ranked.symbol(),
                composition,
                0,
                &mut prefix,
                by_size,
                resources,
                limits,
                &mut out,
            )?;
        }
    }
    out.sort();
    out.dedup();
    charge_operations(resources, limits.max_operations(), out.len())?;
    Ok(out)
}

/// Fill the Cartesian product of the child lists named by one
/// size-composition, appending the constructed parent terms to `out`.
#[allow(clippy::too_many_arguments)]
fn fill_product(
    symbol: &Symbol,
    parts: &[usize],
    index: usize,
    prefix: &mut Vec<Term>,
    by_size: &[Vec<Term>],
    resources: &mut RelationResources,
    limits: &RelationLimits,
    out: &mut Vec<Term>,
) -> Result<(), Stop> {
    if index == parts.len() {
        let nodes = prefix.iter().map(term_nodes).sum::<usize>() + 1;
        charge_operations(resources, limits.max_operations(), nodes)?;
        // The parent retained in the size buffer stores `nodes` cells,
        // and every cloned child argument is its own relation-storage
        // cell as well.
        charge_constraints(resources, limits.max_constraints(), 2 * nodes)?;
        out.push(Term::Sym(symbol.clone(), prefix.clone()));
        return Ok(());
    }
    for child in &by_size[parts[index]] {
        prefix.push(child.clone());
        fill_product(
            symbol,
            parts,
            index + 1,
            prefix,
            by_size,
            resources,
            limits,
            out,
        )?;
        prefix.pop();
    }
    Ok(())
}

/// All compositions of `total` into `parts` positive integers, in a
/// fixed deterministic (ascending-first) order.
fn compositions(
    total: usize,
    parts: usize,
    resources: &mut RelationResources,
    limits: &RelationLimits,
) -> Result<Vec<Vec<usize>>, Stop> {
    if parts == 0 {
        return Ok(if total == 0 {
            vec![Vec::new()]
        } else {
            Vec::new()
        });
    }
    if parts == 1 {
        return Ok(if total >= 1 {
            charge_operations(resources, limits.max_operations(), 1)?;
            charge_constraints(resources, limits.max_constraints(), 1)?;
            vec![vec![total]]
        } else {
            Vec::new()
        });
    }
    let mut result: Vec<Vec<usize>> = Vec::new();
    let upper = total.saturating_sub(parts - 1);
    for first in 1..=upper {
        let suffixes = compositions(total - first, parts - 1, resources, limits)?;
        for mut suffix in suffixes {
            let mut composition = Vec::with_capacity(parts);
            composition.push(first);
            composition.append(&mut suffix);
            charge_operations(resources, limits.max_operations(), parts)?;
            charge_constraints(resources, limits.max_constraints(), parts)?;
            result.push(composition);
        }
    }
    Ok(result)
}

/// Whether forward replay from `start` reaches a member of `language`
/// within `budget` application steps. `min_steps` is 1 for the
/// one-step relation (EXACTLY one application — no reflexive closure)
/// and 0 for finite horizon/saturation (zero steps included). A
/// freshly generated successor is membership-tested even when already
/// visited, so a GENUINE self-application (a rule that returns its
/// own input) still counts as an application.
#[allow(clippy::too_many_arguments)]
fn replays_into(
    system: &TermSystem,
    language: &TreeAutomaton,
    start: &Term,
    budget: u32,
    min_steps: u32,
    rule_shapes: &[(usize, usize, usize)],
    resources: &mut RelationResources,
    limits: &RelationLimits,
) -> Result<bool, Stop> {
    let mut visited: BTreeSet<Term> = BTreeSet::new();
    let mut frontier: Vec<Term> = vec![start.clone()];
    visited.insert(start.clone());
    charge_constraints(resources, limits.max_constraints(), 2 * term_nodes(start))?;
    if min_steps == 0 {
        match language.accepts_with_resources(start, resources) {
            Ok(true) => return Ok(true),
            Ok(false) => {}
            Err(RewriteError::RelationLimitExceeded { resource, limit }) => {
                if resource == "operations" {
                    return Err(Stop::Truncated {
                        resource: "operations",
                        limit,
                    });
                }
            }
            Err(error) => return Err(Stop::Hard(error)),
        }
    }
    let mut step = 0u32;
    loop {
        if step >= budget {
            break;
        }
        let mut next: Vec<Term> = Vec::new();
        for term in &frontier {
            // Preflight the successor work BEFORE the successor
            // vector is allocated (review rounds 2–4). Per rule,
            // the eager TRS layer can emit one replacement per
            // MATCHING POSITION (at most nodes(term) of them), and
            // each instantiated right side can be as large as its
            // non-variable skeleton plus every variable occurrence
            // expanded to a full copy of the term (a binding is a
            // subterm of the term, so nodes(term) bounds each).
            // Replacing at a position also REBUILDS the context:
            // every ancestor of the redex clones its argument
            // subtree, and there are at most depth ≤ nodes(term)
            // ancestors each cloning at most the whole term —
            // nodes(term)^2 per position dominates that cumulative
            // eager construction work (review round 4: a 16-node
            // chain measured 1,360 constructed nodes where the
            // bound without the context term estimated 144).
            // Operations bill the attempts plus substitution plus
            // INTERMEDIATE binding/path copies plus context
            // rebuilding; constraints bill the complete potential
            // RETAINED output vector. All arithmetic saturates; the
            // estimate is conservative, never an under-count.
            let mut ops_estimate = 0usize;
            let mut storage_estimate = 0usize;
            let term_size = term_nodes(term);
            let context_work = term_size.saturating_mul(term_size);
            for &(lhs_size, rhs_skeleton, rhs_var_occurrences) in rule_shapes {
                let instantiated_worst =
                    rhs_skeleton.saturating_add(rhs_var_occurrences.saturating_mul(term_size));
                // Construction WORK doubles the expansion: substitution
                // clones each binding and then reconstructs it into the
                // output (review round 5: x → z(x ×16) applied to `a`
                // constructs 34 nodes — the 17-node successor plus 16
                // intermediate binding copies plus overhead — where a
                // single-copy bound estimates 20). The doubled context
                // term likewise covers per-position path buffers beside
                // the ancestor rebuilds.
                let construction_worst = rhs_skeleton.saturating_add(
                    rhs_var_occurrences
                        .saturating_mul(term_size)
                        .saturating_mul(2),
                );
                let per_position = 1usize
                    .saturating_add(lhs_size)
                    .saturating_add(construction_worst)
                    .saturating_add(context_work)
                    .saturating_add(context_work);
                ops_estimate = ops_estimate.saturating_add(term_size.saturating_mul(per_position));
                storage_estimate = storage_estimate.saturating_add(
                    term_size.saturating_mul(term_size.saturating_add(instantiated_worst)),
                );
            }
            charge_operations(resources, limits.max_operations(), ops_estimate.max(1))?;
            charge_constraints(resources, limits.max_constraints(), storage_estimate)?;
            let successors = system.application_successors(term).map_err(Stop::Hard)?;
            // Post-hoc truth: charge the ACTUAL instantiated successor
            // node counts (not the vector length) as both work and
            // retained storage.
            let actual_nodes: usize = successors
                .iter()
                .map(term_nodes)
                .fold(0usize, |acc, nodes| acc.saturating_add(nodes));
            charge_operations(resources, limits.max_operations(), actual_nodes)?;
            charge_constraints(resources, limits.max_constraints(), actual_nodes)?;
            for successor in successors {
                // Membership of the freshly generated successor is
                // tested BEFORE the visited check: a successor equal
                // to a visited term (a genuine self-application) is
                // still a real application.
                if step + 1 >= min_steps {
                    match language.accepts_with_resources(&successor, resources) {
                        Ok(true) => return Ok(true),
                        Ok(false) => {}
                        Err(RewriteError::RelationLimitExceeded { resource, limit }) => {
                            if resource == "operations" {
                                return Err(Stop::Truncated {
                                    resource: "operations",
                                    limit,
                                });
                            }
                            // A per-term bound: this term cannot be a member.
                        }
                        Err(error) => return Err(Stop::Hard(error)),
                    }
                }
                if term_nodes(&successor) > limits.max_term_nodes()
                    || term_depth(&successor) > limits.max_term_depth()
                {
                    continue;
                }
                if visited.contains(&successor) {
                    charge_operations(resources, limits.max_operations(), 1)?;
                    continue;
                }
                let nodes = term_nodes(&successor);
                charge_constraints(resources, limits.max_constraints(), 2 * nodes)?;
                visited.insert(successor.clone());
                next.push(successor);
            }
        }
        frontier = next;
        step += 1;
        if frontier.is_empty() {
            break;
        }
    }
    Ok(false)
}

/// The canonicalized shared-subterm (DAG) automaton of the witnesses:
/// one state per distinct subterm, in first-encounter order over the
/// sorted witnesses, finals at the witness roots, then canonicalized.
///
/// The alphabet is the ranked union of the language alphabet and the
/// witness symbols; an empty witness set has no witness symbols, so it
/// uses the full candidate alphabet (the canonical empty-language
/// automaton over that alphabet).
fn build_lower_automaton(
    witnesses: &BTreeSet<Term>,
    candidate_alphabet: &[RankedSymbol],
    language: &TreeAutomaton,
    automaton_limits: &TreeAutomatonLimits,
    resources: &mut RelationResources,
) -> RewriteResult<TreeAutomaton> {
    let alphabet = if witnesses.is_empty() {
        candidate_alphabet.to_vec()
    } else {
        let mut ranked: BTreeSet<RankedSymbol> = BTreeSet::new();
        for entry in language.alphabet() {
            ranked.insert(entry.clone());
        }
        for witness in witnesses {
            let mut stack = vec![witness];
            while let Some(node) = stack.pop() {
                if let Term::Sym(symbol, arguments) = node {
                    ranked.insert(RankedSymbol::new(symbol.clone(), arguments.len() as u16));
                    stack.extend(arguments.iter());
                }
            }
        }
        ranked.into_iter().collect()
    };
    let mut states_by_subterm: BTreeMap<Term, TreeState> = BTreeMap::new();
    let mut counter = 0usize;
    for witness in witnesses {
        for path in witness.positions() {
            let subterm = witness
                .subterm(&path)
                .expect("positions enumerate subterms");
            if !states_by_subterm.contains_key(subterm) {
                let state = TreeState::new(format!("w{counter}"));
                counter += 1;
                resources.record_operations(1)?;
                resources.record_constraints(1)?;
                states_by_subterm.insert(subterm.clone(), state);
            }
        }
    }
    let mut transitions: Vec<TreeTransition> = Vec::new();
    for (subterm, state) in &states_by_subterm {
        if let Term::Sym(symbol, arguments) = subterm {
            let children: Vec<TreeState> = arguments
                .iter()
                .map(|argument| {
                    states_by_subterm
                        .get(argument)
                        .expect("every subterm of a witness is assigned a state")
                        .clone()
                })
                .collect();
            resources.record_operations(1 + children.len())?;
            resources.record_constraints(children.len())?;
            transitions.push(TreeTransition::new(symbol.clone(), children, state.clone()));
        }
    }
    let mut finals: Vec<TreeState> = Vec::with_capacity(witnesses.len());
    for witness in witnesses {
        resources.record_operations(1)?;
        resources.record_constraints(1)?;
        finals.push(
            states_by_subterm
                .get(witness)
                .expect("every witness root is assigned a state")
                .clone(),
        );
    }
    let states: Vec<TreeState> = states_by_subterm.values().cloned().collect();
    let assembled = TreeAutomaton::new(
        alphabet.to_vec(),
        states,
        transitions,
        finals,
        *automaton_limits,
    )?;
    canonicalize(assembled, automaton_limits, resources)
}

/// Iterative node count of a term (no recursion on depth).
/// Iterative variable-occurrence count of a term (with multiplicity —
/// a duplicated variable counts once per occurrence), used to bound
/// instantiated right-side sizes in the successor preflight.
fn variable_occurrences(term: &Term) -> usize {
    let mut occurrences = 0usize;
    let mut stack = vec![term];
    while let Some(node) = stack.pop() {
        match node {
            Term::Var(_) => occurrences += 1,
            Term::Sym(_, arguments) => stack.extend(arguments.iter()),
        }
    }
    occurrences
}

/// Iterative node count of a term.
fn term_nodes(term: &Term) -> usize {
    let mut nodes = 0usize;
    let mut stack = vec![term];
    while let Some(node) = stack.pop() {
        nodes += 1;
        if let Term::Sym(_, arguments) = node {
            stack.extend(arguments.iter());
        }
    }
    nodes
}

/// Iterative edge-depth of a term (a constant has depth 0).
fn term_depth(term: &Term) -> usize {
    let mut depth = 0usize;
    let mut stack = vec![(term, 0usize)];
    while let Some((node, level)) = stack.pop() {
        if level > depth {
            depth = level;
        }
        if let Term::Sym(_, arguments) = node {
            for argument in arguments {
                stack.push((argument, level + 1));
            }
        }
    }
    depth
}
