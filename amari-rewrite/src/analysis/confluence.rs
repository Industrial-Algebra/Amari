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
//! Each side runs a breadth-first enumeration of [`TermSystem::successors`]
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
use crate::error::RewriteResult;
use crate::relation::RelationResources;
use crate::trs::{Term, TermSystem};

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
            for successor in system.successors(&node)? {
                resources.record_term(term_nodes(&successor), term_depth(&successor))?;
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
            for successor in system.successors(&node)? {
                resources.record_term(term_nodes(&successor), term_depth(&successor))?;
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
