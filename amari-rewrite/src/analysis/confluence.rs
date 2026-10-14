// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded joinability and local-confluence reports.
//!
//! Newman's lemma connects local confluence to the critical pairs of a
//! terminating system: a left-linear system is locally confluent iff
//! every critical pair joins. This module analyzes Task 30's critical
//! pairs with one bounded bidirectional successor search per pair and
//! reports both the per-pair outcome and a whole-system certification.
//!
//! # Search semantics
//!
//! Rewriting is directed, so "bidirectional" here means both closures
//! grow *forward* from the two sides of the pair — the root side
//! (`pair.root_step`) and the position side (`pair.position_step`) —
//! and the meet is a frontier intersection, never backward rewriting.
//! Each side runs a breadth-first enumeration of shape-preflighted
//! successors (validated before construction; see `metered_successors`)
//! in its deterministic order; a `BTreeSet` visited set kills cycles,
//! so a cyclic successor graph terminates as soon as both closures are
//! exhausted.
//!
//! The two frontiers advance in lockstep, one level per round, left
//! side first. The first newly discovered term that belongs to the
//! other side's visited set is the witness: the exact discovered chains
//! are returned source-first with the join term as the last element of
//! both chains (a chain is length one when its source is the join).
//!
//! # Accounting
//!
//! The same [`RelationResources`] pool covers critical-pair enumeration
//! and every joinability search. Each expanded node charges one
//! operation; each generated successor is validated against the term
//! bounds before it enters the visited set. No new configuration knobs
//! exist: the search shares the caller's limits. Exhausting any budget
//! returns [`crate::error::RewriteError::RelationLimitExceeded`] with no
//! partial report — there is no "unknown" outcome.
//!
//! # Certification
//!
//! `locally_confluent` is true only when every analyzed pair joins AND
//! the system is left-linear. A non-left-linear rule blocks
//! certification, but its pairs are still analyzed and reported, so the
//! report stays complete.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::vec::Vec;

use crate::analysis::critical_pairs::critical_pairs;
use crate::analysis::unify::{term_depth, term_nodes};
use crate::analysis::CriticalPair;
use crate::error::{RewriteError, RewriteResult};
use crate::relation::RelationResources;
use crate::trs::{Substitution, Term, TermSystem};
use crate::Path;

/// A concrete common reduct of a critical pair, with both derivation
/// chains.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct JoinabilityWitness {
    /// The common reduct reached from both sides.
    pub join_term: Term,
    /// Chain from `pair.root_step`, source first, `join_term` last.
    pub left_steps: Vec<Term>,
    /// Chain from `pair.position_step`, source first, `join_term` last.
    pub right_steps: Vec<Term>,
}

/// Outcome of analyzing one critical pair.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum PairJoinability {
    /// Both sides reach a common reduct.
    Joinable(JoinabilityWitness),
    /// Both closures were fully enumerated without a common reduct.
    NotJoinable,
}

/// Local-confluence report over every critical pair of a system.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ConfluenceReport {
    /// Pairs and their outcomes, in critical-pair enumeration order.
    pub pairs: Vec<(CriticalPair, PairJoinability)>,
    /// True only when every pair joins AND the system is left-linear.
    pub locally_confluent: bool,
    /// Indices of rules whose LHS is not left-linear; non-empty blocks
    /// certification even when every pair joins.
    pub certification_blocked: Vec<usize>,
}

/// Analyze the critical pairs of `system` for local confluence.
///
/// Enumerates the critical pairs (Task 30) and, for each, runs one
/// bounded bidirectional successor search. Every operation and term is
/// accounted against `resources`; exhausting a budget returns
/// [`crate::error::RewriteError::RelationLimitExceeded`] without a partial
/// report.
pub fn local_confluence(
    system: &TermSystem,
    resources: &mut RelationResources,
) -> RewriteResult<ConfluenceReport> {
    let critical = critical_pairs(system, resources)?;
    let certification_blocked = critical.non_left_linear_rules;

    let mut pairs = Vec::with_capacity(critical.pairs.len());
    let mut every_pair_joins = true;
    for pair in critical.pairs {
        let outcome = analyze_pair(system, &pair, resources)?;
        if matches!(outcome, PairJoinability::NotJoinable) {
            every_pair_joins = false;
        }
        pairs.push((pair, outcome));
    }

    // A non-left-linear rule blocks certification even when every pair
    // joins; the pairs above are still reported, so the report is
    // complete either way.
    let locally_confluent = every_pair_joins && certification_blocked.is_empty();
    Ok(ConfluenceReport {
        pairs,
        locally_confluent,
        certification_blocked,
    })
}

/// One-step successors as an EAGER batch with PRE-CONSTRUCTION shape
/// validation (PR #302 rounds 1–2): each matched application's
/// rewritten shape is computed from the match substitution and
/// validated with `record_term` BEFORE `replace_at` constructs it,
/// so an oversized candidate errors regardless of rule order and no
/// oversized term is ever materialized. The batch itself is built
/// eagerly (like `TermSystem::successors`); accounting is
/// expansion-only: the caller charges one operation per expanded
/// node, and `record_term` validates each candidate's shape without
/// charging cumulative work. Search storage (visited sets, parent
/// maps, frontier queues) is bounded INDIRECTLY — by the admitted
/// rules, the term-shape bounds, and the charged expansions — not
/// metered per successor. Identity instances are constructed,
/// validated, and then filtered, matching
/// `TermSystem::successors` semantics.
fn metered_successors(
    system: &TermSystem,
    term: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<Vec<Term>> {
    let mut out = Vec::new();
    for path in term.positions() {
        let subterm = term.subterm(&path).ok_or(RewriteError::InvalidPath)?;
        let (spine_nodes, spine_depth) = spine_shape(term, &path);
        for rule in system.rules() {
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
                out.push(rewritten);
            }
        }
    }
    Ok(out)
}

/// Exact `(nodes, depth)` of `template` under a MATCH substitution:
/// one-level, ranges counted verbatim (never chain-resolved), so the
/// shape equals what [`Substitution::apply_matcher`] will build.
/// Saturating arithmetic fails closed.
fn matched_shape(template: &Term, subst: &Substitution) -> (usize, usize) {
    struct Frame<'a> {
        term: &'a Term,
        path_len: usize,
    }
    let mut nodes = 0usize;
    let mut depth = 0usize;
    let mut stack = Vec::from([Frame {
        term: template,
        path_len: 0,
    }]);
    while let Some(frame) = stack.pop() {
        match frame.term {
            Term::Var(variable) => {
                let (bound_nodes, bound_depth) = subst
                    .get_var(variable)
                    .map(|bound| (term_nodes(bound), term_depth(bound)))
                    .unwrap_or((1, 1));
                nodes = nodes.saturating_add(bound_nodes);
                depth = depth.max(frame.path_len.saturating_add(bound_depth));
            }
            Term::Sym(_, args) => {
                nodes = nodes.saturating_add(1);
                if args.is_empty() {
                    depth = depth.max(frame.path_len + 1);
                }
                for arg in args.iter() {
                    stack.push(Frame {
                        term: arg,
                        path_len: frame.path_len + 1,
                    });
                }
            }
        }
    }
    (nodes, depth)
}

/// Exact `(nodes, depth)` of `term` with the subtree at `except`
/// removed (the spine): `spine_shape(t, p)` plus the shape of the
/// replacement gives the exact shape of `t.replace_at(p, _)`.
fn spine_shape(term: &Term, except: &Path) -> (usize, usize) {
    struct Frame<'a> {
        term: &'a Term,
        path: Path,
    }
    let mut nodes = 0usize;
    let mut depth = 0usize;
    let mut stack = Vec::from([Frame {
        term,
        path: Path::root(),
    }]);
    while let Some(frame) = stack.pop() {
        if frame.path.as_slice() == except.as_slice() {
            continue;
        }
        match frame.term {
            Term::Var(_) => {
                nodes = nodes.saturating_add(1);
                depth = depth.max(frame.path.as_slice().len() + 1);
            }
            Term::Sym(_, args) => {
                nodes = nodes.saturating_add(1);
                if args.is_empty() {
                    depth = depth.max(frame.path.as_slice().len() + 1);
                }
                for (index, arg) in args.iter().enumerate() {
                    stack.push(Frame {
                        term: arg,
                        path: frame.path.child(index),
                    });
                }
            }
        }
    }
    (nodes, depth)
}

/// One bounded bidirectional search over the pair's two sides.
fn analyze_pair(
    system: &TermSystem,
    pair: &CriticalPair,
    resources: &mut RelationResources,
) -> RewriteResult<PairJoinability> {
    let left_source = &pair.root_step;
    let right_source = &pair.position_step;
    if left_source == right_source {
        return Ok(PairJoinability::Joinable(JoinabilityWitness {
            join_term: left_source.clone(),
            left_steps: Vec::from([left_source.clone()]),
            right_steps: Vec::from([right_source.clone()]),
        }));
    }

    let mut left_visited = BTreeSet::from([left_source.clone()]);
    let mut right_visited = BTreeSet::from([right_source.clone()]);
    let mut left_parents: BTreeMap<Term, Term> = BTreeMap::new();
    let mut right_parents: BTreeMap<Term, Term> = BTreeMap::new();
    let mut left_frontier = VecDeque::from([left_source.clone()]);
    let mut right_frontier = VecDeque::from([right_source.clone()]);

    loop {
        if left_frontier.is_empty() && right_frontier.is_empty() {
            return Ok(PairJoinability::NotJoinable);
        }

        // Left level: left side first, enumeration order preserved.
        let mut next_left = VecDeque::new();
        while let Some(node) = left_frontier.pop_front() {
            resources.record_operations(1)?;
            for successor in metered_successors(system, &node, resources)? {
                if !left_visited.insert(successor.clone()) {
                    continue;
                }
                left_parents.insert(successor.clone(), node.clone());
                if right_visited.contains(&successor) {
                    return Ok(PairJoinability::Joinable(build_witness(
                        successor,
                        left_source,
                        &left_parents,
                        right_source,
                        &right_parents,
                    )));
                }
                next_left.push_back(successor);
            }
        }
        left_frontier = next_left;

        // Right level: checked against the left level just discovered.
        let mut next_right = VecDeque::new();
        while let Some(node) = right_frontier.pop_front() {
            resources.record_operations(1)?;
            for successor in metered_successors(system, &node, resources)? {
                if !right_visited.insert(successor.clone()) {
                    continue;
                }
                right_parents.insert(successor.clone(), node.clone());
                if left_visited.contains(&successor) {
                    return Ok(PairJoinability::Joinable(build_witness(
                        successor,
                        left_source,
                        &left_parents,
                        right_source,
                        &right_parents,
                    )));
                }
                next_right.push_back(successor);
            }
        }
        right_frontier = next_right;
    }
}

/// Build the witness for a discovered `join`, reconstructing both
/// chains through the parent maps.
fn build_witness(
    join: Term,
    left_source: &Term,
    left_parents: &BTreeMap<Term, Term>,
    right_source: &Term,
    right_parents: &BTreeMap<Term, Term>,
) -> JoinabilityWitness {
    JoinabilityWitness {
        join_term: join.clone(),
        left_steps: reconstruct_chain(left_source, &join, left_parents),
        right_steps: reconstruct_chain(right_source, &join, right_parents),
    }
}

/// Walk parent links from `join` back to `source`, then reverse so the
/// source is first and `join` is last.
fn reconstruct_chain(source: &Term, join: &Term, parents: &BTreeMap<Term, Term>) -> Vec<Term> {
    let mut chain = Vec::from([join.clone()]);
    let mut current = join.clone();
    while current != *source {
        let parent = parents
            .get(&current)
            .cloned()
            .expect("visited term has a parent back to its source");
        chain.push(parent.clone());
        current = parent;
    }
    chain.reverse();
    chain
}
