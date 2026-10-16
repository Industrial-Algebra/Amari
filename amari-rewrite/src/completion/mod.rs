// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded Knuth–Bendix completion (feature `completion`).
//!
//! Completion turns an equational presentation — a left-linear rewrite
//! system plus an LPO [`Precedence`] — into a convergent rewrite
//! system. The procedure orients every input rule, then repeatedly
//! computes critical pairs (Task 30), joins their sides by
//! normalization, and orients any surviving equation into a new rule,
//! simplifying existing right-hand sides until no new equation remains.
//!
//! # Outcomes
//!
//! [`CompletionOutcome::Complete`] is returned ONLY after all three
//! postchecks pass: every rule strictly decreases under the LPO
//! precedence ([`prove_terminating`] reports
//! [`TerminationProof::ProvedTerminating`]), the final system is
//! left-linear, and [`local_confluence`] certifies that every critical
//! pair joins. By Newman's lemma a terminating, locally-confluent
//! system is confluent, so the returned system is convergent.
//!
//! [`CompletionOutcome::Partial`] is an honest bounded stop: the
//! intermediate system is returned as evidence, and every rule in it
//! was oriented when it was added. It carries a [`PartialReason`].
//!
//! [`CompletionOutcome::Failed`] is a fatal precondition failure —
//! a non-left-linear input rule or a precedence that does not cover
//! the input. Limit exhaustion is NOT a `Failed` or `Partial`
//! outcome: it is [`RewriteError::RelationLimitExceeded`] with no
//! system, matching the codebase's no-partial-report discipline.
//!
//! # Determinism
//!
//! The run is deterministic: critical pairs enumerate canonically
//! (Task 30), equation deduplication uses an unordered
//! alpha-canonical [`Sha256Digest`] fingerprint, orientation prefers
//! `l > r` before `r > l`, and every collection iterated is ordered.
//! The same input therefore produces the same outcome.
//!
//! # Accounting
//!
//! One [`RelationResources`] budget covers the whole run. Every rule
//! and pair materialization is charged with [`RelationResources::record_term`]
//! BEFORE it is constructed and every orientation, normalization, or
//! deduction step charges [`RelationResources::record_operations`].

mod config;
pub mod inverse;
mod trace;

use alloc::collections::BTreeSet;
use alloc::vec::Vec;

use crate::analysis::{
    critical_pairs, local_confluence, lpo_greater, prove_terminating, Precedence, TerminationProof,
};
use crate::relation::{RelationResources, Sha256Digest};
use crate::trs::{Rule, Substitution, Term, TermSystem};
use crate::{Path, RewriteError, RewriteResult};

pub use config::CompletionConfig;
pub use inverse::{
    guided_predecessors, prepare_guidance, verify_equivalence, CandidateRejection,
    CompletionGuidance, GuidedSearchOutcome, InconclusiveReason, RelationEquivalenceCertificate,
    RuleDerivation,
};
pub use trace::{CompletionTrace, TraceStep};

/// Frame label for the alpha-canonical equation fingerprint.
const EQUATION_FRAME: &str = "amari.completion.equation/v1";

/// Result of a bounded completion run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionOutcome {
    /// All input rules oriented, the bounded loop exhausted its
    /// critical pairs, and all three postchecks passed: every rule
    /// strictly decreases under the LPO precedence, the final system
    /// is left-linear, and [`local_confluence`] certifies every
    /// critical pair joins. By Newman's lemma a terminating,
    /// locally-confluent system is convergent, so `system` is a
    /// convergent presentation of the input equations.
    Complete {
        /// The convergent rewrite system.
        system: TermSystem,
        /// Bounded completion trace.
        trace: CompletionTrace,
    },
    /// A bounded stop with a working intermediate system. Every rule
    /// in `system` was oriented when it was added; the stop reason is
    /// in `reason`.
    Partial {
        /// The honest intermediate system.
        system: TermSystem,
        /// Bounded completion trace.
        trace: CompletionTrace,
        /// Why completion stopped early.
        reason: PartialReason,
    },
    /// A fatal precondition failure; no rewrite system is produced.
    Failed {
        /// Why the run failed before producing a system.
        reason: FailedReason,
    },
}

/// Reason a run returned [`CompletionOutcome::Partial`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PartialReason {
    /// A generated (or input) equation could not be oriented in either
    /// direction under the configured precedence.
    UnorientableRule {
        /// The two sides of the offending equation, in the order they
        /// were compared: `0 > 1` and `1 > 0` both failed.
        rule_terms: (Term, Term),
    },
    /// A generated rule's left-hand side was not left-linear.
    NonLeftLinear,
    /// The left-linearity or local-confluence postcheck did not
    /// certify the final system.
    ConfluenceNotCertified,
}

/// Reason a run returned [`CompletionOutcome::Failed`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FailedReason {
    /// The input was not a valid completion problem: a non-left-linear
    /// input rule, or a precedence that does not cover the input
    /// symbols at their used arities.
    InvalidInput,
}

/// Run bounded Knuth–Bendix completion.
///
/// See the module documentation for the outcome semantics, the
/// determinism guarantee, and the accounting contract. Exhausting the
/// resource budget returns [`RewriteError::RelationLimitExceeded`]
/// without a partial report.
pub fn complete(
    system: &TermSystem,
    config: &CompletionConfig,
    resources: &mut RelationResources,
) -> RewriteResult<CompletionOutcome> {
    // 1. Validate input: every input rule must be left-linear, and the
    // precedence must cover the input at the used arities. Coverage is
    // checked with the `prove_terminating` pattern; an uncovered
    // symbol surfaces as a typed `InvalidRule`, which is a fatal input
    // failure rather than an escaping error.
    for rule in system.rules() {
        if !rule.is_left_linear() {
            return Ok(CompletionOutcome::Failed {
                reason: FailedReason::InvalidInput,
            });
        }
    }
    if let Err(error) = prove_terminating(system, &config.precedence, resources) {
        return match error {
            RewriteError::InvalidRule { .. } => Ok(CompletionOutcome::Failed {
                reason: FailedReason::InvalidInput,
            }),
            other => Err(other),
        };
    }

    let mut trace = CompletionTrace::default();
    let mut current: Vec<Rule> = Vec::new();

    // 2. Orient every input rule. An unorientable input rule stops the
    // run with the rules oriented so far as the intermediate system.
    for rule in system.rules() {
        match orient_pair(rule.lhs(), rule.rhs(), &config.precedence, resources)? {
            Orientation::Rule(oriented) => {
                if !oriented.is_left_linear() {
                    return Ok(CompletionOutcome::Partial {
                        system: TermSystem::new(current),
                        trace,
                        reason: PartialReason::NonLeftLinear,
                    });
                }
                current.push(oriented);
            }
            Orientation::Unorientable(left, right) => {
                return Ok(CompletionOutcome::Partial {
                    system: TermSystem::new(current),
                    trace,
                    reason: PartialReason::UnorientableRule {
                        rule_terms: (left, right),
                    },
                });
            }
        }
    }

    // 3. Bounded Knuth–Bendix loop. Each pass enumerates the critical
    // pairs of one frozen snapshot; every distinct equation is
    // processed at most once, and a pass that adds a rule restarts so
    // the next pass sees the simplified system.
    let mut seen: BTreeSet<(Sha256Digest, Sha256Digest)> = BTreeSet::new();
    loop {
        let snapshot = TermSystem::new(current.clone());
        let report = critical_pairs(&snapshot, resources)?;
        let mut added_rule = false;
        for pair in report.pairs {
            resources.record_operations(1)?;
            let key = equation_key(&pair.root_step, &pair.position_step, resources)?;
            if !seen.insert(key) {
                continue;
            }
            trace.pairs_processed += 1;
            trace.steps.push(TraceStep::Pair {
                left_rule: pair.left_rule,
                right_rule: pair.right_rule,
                path: pair.path.clone(),
            });

            let left = normalize(&snapshot, &pair.root_step, resources)?;
            let right = normalize(&snapshot, &pair.position_step, resources)?;
            if left == right {
                continue;
            }
            match orient_pair(&left, &right, &config.precedence, resources)? {
                Orientation::Unorientable(l, r) => {
                    return Ok(CompletionOutcome::Partial {
                        system: TermSystem::new(current),
                        trace,
                        reason: PartialReason::UnorientableRule { rule_terms: (l, r) },
                    });
                }
                Orientation::Rule(rule) => {
                    if !rule.is_left_linear() {
                        return Ok(CompletionOutcome::Partial {
                            system: TermSystem::new(current),
                            trace,
                            reason: PartialReason::NonLeftLinear,
                        });
                    }
                    charge_rule(&rule, resources)?;
                    current.push(rule);
                    let rule_index = current.len() - 1;
                    trace.rules_added += 1;
                    trace.steps.push(TraceStep::Oriented { rule_index });
                    simplify(&mut current, resources, &mut trace)?;
                    added_rule = true;
                }
            }
        }
        if !added_rule {
            break;
        }
    }

    // 4. Postchecks on the final system: (a) LPO termination, (b)
    // left-linearity, (c) certified local confluence. `Complete` is
    // returned only when all three pass.
    let final_system = TermSystem::new(current);
    match prove_terminating(&final_system, &config.precedence, resources)? {
        TerminationProof::ProvedTerminating { .. } => {}
        TerminationProof::NotDecreasing { rule_index } => {
            let rule = &final_system.rules()[rule_index];
            let rule_terms = (rule.lhs().clone(), rule.rhs().clone());
            return Ok(CompletionOutcome::Partial {
                system: final_system,
                trace,
                reason: PartialReason::UnorientableRule { rule_terms },
            });
        }
    }
    if final_system
        .rules()
        .iter()
        .any(|rule| !rule.is_left_linear())
    {
        return Ok(CompletionOutcome::Partial {
            system: final_system,
            trace,
            reason: PartialReason::NonLeftLinear,
        });
    }
    let report = local_confluence(&final_system, resources)?;
    if !report.locally_confluent {
        return Ok(CompletionOutcome::Partial {
            system: final_system,
            trace,
            reason: PartialReason::ConfluenceNotCertified,
        });
    }
    Ok(CompletionOutcome::Complete {
        system: final_system,
        trace,
    })
}

/// Result of orienting one equation.
enum Orientation {
    /// The equation is a valid strictly-decreasing rule.
    Rule(Rule),
    /// Neither direction is a valid strictly-decreasing rule.
    Unorientable(Term, Term),
}

/// Orient `left ≈ right` under `precedence`, preferring `left -> right`.
///
/// `left > right` wins first; otherwise `right > left` is tried. A
/// direction only counts when [`Rule::new`] accepts it: an equation
/// whose reverse direction would need a right-hand-side variable that
/// does not occur on its left is unorientable, never a malformed rule.
fn orient_pair(
    left: &Term,
    right: &Term,
    precedence: &Precedence,
    resources: &mut RelationResources,
) -> RewriteResult<Orientation> {
    resources.record_operations(1)?;
    if lpo_greater(left, right, precedence, resources)? {
        return Ok(match Rule::new(left.clone(), right.clone()) {
            Ok(rule) => Orientation::Rule(rule),
            Err(_) => Orientation::Unorientable(left.clone(), right.clone()),
        });
    }
    resources.record_operations(1)?;
    if lpo_greater(right, left, precedence, resources)? {
        if let Ok(rule) = Rule::new(right.clone(), left.clone()) {
            return Ok(Orientation::Rule(rule));
        }
    }
    Ok(Orientation::Unorientable(left.clone(), right.clone()))
}

/// Charge both sides of a rule before it is stored.
fn charge_rule(rule: &Rule, resources: &mut RelationResources) -> RewriteResult<()> {
    let (left_nodes, left_depth) = shape_of(rule.lhs());
    let (right_nodes, right_depth) = shape_of(rule.rhs());
    resources.record_term(left_nodes, left_depth)?;
    resources.record_term(right_nodes, right_depth)
}

/// Unordered alpha-canonical fingerprint of an equation.
///
/// Completion treats `l ≈ r` and `r ≈ l` as one equation, so the two
/// canonical digests are sorted before they form the set key. Both
/// sides are charged against the term bounds before the encoding is
/// materialized.
fn equation_key(
    left: &Term,
    right: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<(Sha256Digest, Sha256Digest)> {
    let (left_nodes, left_depth) = shape_of(left);
    let (right_nodes, right_depth) = shape_of(right);
    resources.record_term(left_nodes, left_depth)?;
    resources.record_term(right_nodes, right_depth)?;
    // Joint canonicalization: variables are renumbered across BOTH
    // sides, preserving cross-side variable identity — h(x,y) and
    // h(x,z) must not collide (PR #310 round 1). The fingerprint is
    // traversal-ordered: the SWAPPED equation (r, l) can hash
    // differently, so a swapped duplicate may be processed once
    // more. Dedup is sound (never merges distinct equations);
    // swap-completeness is deliberately not paid for (PR #310
    // round 2, P3 documented).
    let digests = Sha256Digest::canonical_terms(EQUATION_FRAME, &[left, right]);
    let (left_digest, right_digest) = (digests[0], digests[1]);
    Ok(if left_digest <= right_digest {
        (left_digest, right_digest)
    } else {
        (right_digest, left_digest)
    })
}

/// Normalize `term` under `system`, charging every step.
///
/// The first applicable rule at the first position (preorder) is
/// applied, matching [`TermSystem::apply_once`], and every candidate
/// rewrite is shape-validated before it is constructed. The current
/// system is always LPO-terminating, so this reaches a fixed point;
/// the shared operation budget bounds the search for it.
fn normalize(
    system: &TermSystem,
    term: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<Term> {
    let mut current = term.clone();
    // Bounded by the shared operation budget (every step charges
    // one operation plus step_once's internal accounting) — the
    // system is LPO-terminating, so the fixed point exists; the
    // budget is the only bound (PR #310 round 1).
    loop {
        resources.record_operations(1)?;
        match step_once(system, &current, resources)? {
            Some(next) => current = next,
            None => return Ok(current),
        }
    }
}

/// Apply the first outermost one-step rewrite, metering every attempt.
///
/// Each matched rewrite's shape is computed from the match
/// substitution and validated against the term bounds BEFORE
/// [`Term::replace_at`] constructs it, and the replacement is built
/// with [`Substitution::apply_matcher`] so match bindings are never
/// chain-resolved.
fn step_once(
    system: &TermSystem,
    term: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<Option<Term>> {
    for path in term.positions() {
        let Some(subterm) = term.subterm(&path) else {
            return Err(RewriteError::InvalidPath);
        };
        let (spine_nodes, spine_depth) = spine_shape(term, &path);
        for rule in system.rules() {
            resources.record_operations(1)?;
            let Some(subst) = rule.matches(subterm) else {
                continue;
            };
            let (inner_nodes, inner_depth) = matched_shape(rule.rhs(), &subst);
            let nodes = spine_nodes.saturating_add(inner_nodes);
            let depth = spine_depth.max(path.as_slice().len().saturating_add(inner_depth));
            resources.record_term(nodes, depth)?;
            let replacement = subst.apply_matcher(rule.rhs());
            let rewritten = term.replace_at(&path, replacement)?;
            if rewritten != *term {
                return Ok(Some(rewritten));
            }
        }
    }
    Ok(None)
}

/// Reduce the right-hand side of every rule in `rules` under the
/// current system.
///
/// Only right-hand sides are reduced: replacing a right-hand side by
/// a reduct strictly smaller under LPO preserves the orientation
/// `lhs > rhs`, whereas reducing a left-hand side can destroy it. A
/// reduced rule that fails checked construction keeps its original
/// form, so the intermediate system never holds an invalid rule.
fn simplify(
    rules: &mut Vec<Rule>,
    resources: &mut RelationResources,
    trace: &mut CompletionTrace,
) -> RewriteResult<()> {
    let system = TermSystem::new(rules.clone());
    let mut next = Vec::with_capacity(rules.len());
    for (index, rule) in rules.iter().enumerate() {
        resources.record_operations(1)?;
        let rhs = rule.rhs().clone();
        let reduced = normalize(&system, &rhs, resources)?;
        if reduced != rhs {
            if let Ok(rebuilt) = Rule::new(rule.lhs().clone(), reduced) {
                trace
                    .steps
                    .push(TraceStep::Simplified { rule_index: index });
                next.push(rebuilt);
                continue;
            }
        }
        next.push(rule.clone());
    }
    *rules = next;
    Ok(())
}

/// Exact `(nodes, depth)` of `term` (leaf depth one).
///
/// Iterative: callers must never depend on the call stack for term
/// shape, and this runs on terms that may be near the depth ceiling.
pub(crate) fn shape_of(term: &Term) -> (usize, usize) {
    let mut nodes = 0usize;
    let mut depth = 0usize;
    let mut stack = Vec::from([(term, 1usize)]);
    while let Some((current, current_depth)) = stack.pop() {
        nodes = nodes.saturating_add(1);
        depth = depth.max(current_depth);
        if let Term::Sym(_, args) = current {
            for arg in args {
                stack.push((arg, current_depth.saturating_add(1)));
            }
        }
    }
    (nodes, depth)
}

/// Exact `(nodes, depth)` of `term` with the subtree at `except`
/// removed (the spine): `spine_shape(t, p)` plus the shape of the
/// replacement is the exact shape of `t.replace_at(p, _)`.
pub(crate) fn spine_shape(term: &Term, except: &Path) -> (usize, usize) {
    let mut nodes = 0usize;
    let mut depth = 0usize;
    let mut stack = Vec::from([(term, Path::root())]);
    while let Some((current, path)) = stack.pop() {
        if path.as_slice() == except.as_slice() {
            continue;
        }
        nodes = nodes.saturating_add(1);
        depth = depth.max(path.as_slice().len() + 1);
        if let Term::Sym(_, args) = current {
            for (index, arg) in args.iter().enumerate() {
                stack.push((arg, path.child(index)));
            }
        }
    }
    (nodes, depth)
}

/// Exact `(nodes, depth)` of `template` under a MATCH substitution:
/// one-level, ranges counted verbatim, so the shape equals what
/// [`Substitution::apply_matcher`] will build. Saturating arithmetic
/// fails closed.
pub(crate) fn matched_shape(template: &Term, subst: &Substitution) -> (usize, usize) {
    let mut nodes = 0usize;
    let mut depth = 0usize;
    let mut stack = Vec::from([(template, 1usize)]);
    while let Some((current, current_depth)) = stack.pop() {
        match current {
            Term::Var(variable) => {
                let (bound_nodes, bound_depth) =
                    subst.get_var(variable).map(shape_of).unwrap_or((1, 1));
                nodes = nodes.saturating_add(bound_nodes);
                depth = depth.max(current_depth.saturating_sub(1).saturating_add(bound_depth));
            }
            Term::Sym(_, args) => {
                nodes = nodes.saturating_add(1);
                depth = depth.max(current_depth);
                for arg in args {
                    stack.push((arg, current_depth.saturating_add(1)));
                }
            }
        }
    }
    (nodes, depth)
}
