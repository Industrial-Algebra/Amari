// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic smallest-witness extraction for tree automata.
//!
//! Memory and correctness discipline (PR #267 review, both rounds):
//!
//! - The fixpoint computes minimal COSTS plus transition/child
//!   backpointers only; term trees are never materialized per state
//!   (DAG-shared derivations expand to exponential trees).
//! - Costs saturate at [`COST_CAP`], one above the representable
//!   term-node ceiling. Saturated ("oversized") derivations carry NO
//!   backpointers, never displace an existing entry, and are never
//!   fed to the canonical comparator — so backpointer graphs are
//!   acyclic by construction and oversized languages surface as a
//!   typed limit error, not a crash or an empty-language lie.
//! - Canonical tie-breaking reproduces the shared `encode_term`
//!   byte order exactly: arity and name length as little-endian
//!   u32 bytes, then name bytes, then children recursively.
//! - Depth is edge-based (a constant has depth 0, matching
//!   membership) and is recomputed over the FINALIZED derivation
//!   graph before the guard runs: backpointers refer to mutable
//!   per-state entries, so cached depths can go stale when an
//!   equal-cost canonically-smaller child derivation replaces one.
//! - Only the single chosen witness is materialized.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::error::{RewriteError, RewriteResult};
use crate::language::automaton::TreeAutomaton;
use crate::relation::{RelationLimits, RelationResources};
use crate::trs::Term;

/// Costs at or above this value cannot be materialized within the
/// term-node ceiling. Derivations at the cap are reachability
/// markers only: no backpointers, no comparisons.
const COST_CAP: usize = RelationLimits::MAX_TERM_NODES + 1;

/// The chosen derivation of one state: minimal exact cost plus the
/// transition and child-state selections that achieve it.
/// `children` is empty for capped (oversized) entries, which exist
/// only to record reachability.
#[derive(Clone, Debug)]
struct BestDerivation {
    cost: usize,
    transition: usize,
    children: Vec<usize>,
}

impl BestDerivation {
    fn is_oversized(&self) -> bool {
        self.cost >= COST_CAP
    }
}

impl TreeAutomaton {
    /// The smallest accepted ground term: fewest nodes first, ties
    /// broken by the shared canonical term bytes. Deterministic —
    /// repeated calls and canonically equal automata return
    /// identical terms. `None` exactly when the language is empty.
    ///
    /// Returns a typed limit error when the smallest witness itself
    /// exceeds the fixed term node/depth ceilings: the language is
    /// nonempty, but its canonical smallest member cannot be
    /// represented within the authority.
    ///
    /// The unmetered entry point retains an unbounded operation
    /// workspace (matching its pre-instrumentation behavior); the
    /// budgeted variant is [`Self::witness_with_resources`].
    pub fn witness(&self) -> RewriteResult<Option<Term>> {
        let mut resources = RelationResources::unbounded();
        self.witness_with_resources(&mut resources)
    }

    /// The smallest accepted ground term: fewest nodes first, ties
    /// broken by the shared canonical term bytes. Deterministic —
    /// repeated calls and canonically equal automata return
    /// identical terms. `None` exactly when the language is empty.
    ///
    /// Work is charged to the caller-supplied budget: one operation
    /// per transition evaluation in every fixpoint round, plus one
    /// per state per canonical tie-break comparison, in the fixpoint
    /// AND in final-state selection (each comparison traverses
    /// derivations whose size is bounded by the state count). An exhausted budget is a typed
    /// [`RewriteError::RelationLimitExceeded`], so callers can
    /// enforce their own ceilings during execution.
    ///
    /// Returns a typed limit error when the smallest witness itself
    /// exceeds the fixed term node/depth ceilings: the language is
    /// nonempty, but its canonical smallest member cannot be
    /// represented within the authority.
    pub fn witness_with_resources(
        &self,
        resources: &mut RelationResources,
    ) -> RewriteResult<Option<Term>> {
        let best = self.best_derivations(resources)?;
        let finals: Vec<usize> = self
            .finals()
            .iter()
            .filter_map(|final_state| self.states().iter().position(|state| state == final_state))
            .filter(|index| best[*index].is_some())
            .collect();
        if finals.is_empty() {
            return Ok(None);
        }
        // Exact derivations always beat oversized ones (their true
        // costs exceed every exact cost).
        let exact: Vec<usize> = finals
            .iter()
            .copied()
            .filter(|index| !best[*index].as_ref().expect("present").is_oversized())
            .collect();
        // Fallible selection loop (min_by cannot surface charge
        // failures): the canonical comparison between equal-cost
        // finals is charged exactly like a fixpoint tie-break — one
        // operation per state.
        let mut chosen: Option<usize> = None;
        for candidate in exact.iter().copied() {
            let Some(current) = chosen else {
                chosen = Some(candidate);
                continue;
            };
            let (candidate_entry, current_entry) = (
                best[candidate].as_ref().expect("present"),
                best[current].as_ref().expect("present"),
            );
            let mut ordering = candidate_entry.cost.cmp(&current_entry.cost);
            if ordering == Ordering::Equal {
                resources.record_operations(self.states().len())?;
                let mut memo = BTreeMap::new();
                ordering = self.compare_state_pair(&best, candidate, current, &mut memo);
            }
            if ordering == Ordering::Less {
                chosen = Some(candidate);
            }
        }
        let Some(chosen) = chosen else {
            // Every reachable final is oversized: the language is
            // nonempty but its smallest member exceeds the ceilings.
            return Err(RewriteError::RelationLimitExceeded {
                resource: "witness term nodes",
                limit: RelationLimits::MAX_TERM_NODES,
            });
        };
        // Depth is measured on the FINALIZED graph, edge-based,
        // computed iteratively over ascending costs.
        let depths = self.finalized_depths(&best);
        let depth = depths[chosen];
        if depth > RelationLimits::MAX_TERM_DEPTH {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "witness term depth",
                limit: RelationLimits::MAX_TERM_DEPTH,
            });
        }
        Ok(Some(self.materialize(&best, chosen)))
    }

    /// Materialize exactly one derivation by unfolding its
    /// backpointers. Callers must have verified the cost/depth
    /// ceilings, so the unfold is bounded; oversized entries carry
    /// no backpointers and never reach this function.
    fn materialize(&self, best: &[Option<BestDerivation>], index: usize) -> Term {
        let entry = best[index].as_ref().expect("chosen state is reachable");
        debug_assert!(!entry.is_oversized());
        let transition = &self.transitions()[entry.transition];
        Term::sym(
            transition.symbol().clone(),
            entry
                .children
                .iter()
                .map(|child| self.materialize(best, *child)),
        )
    }

    /// Edge-based depths of all exact derivations (constants are
    /// depth 0, matching membership), computed ITERATIVELY: children
    /// always carry strictly smaller exact costs, so evaluating
    /// states in ascending-cost order needs no recursion and no
    /// stack growth (PR #267 round 3: deep-but-exact derivations
    /// must surface the typed depth error, not a stack overflow).
    /// Values are capped one past the depth ceiling.
    fn finalized_depths(&self, best: &[Option<BestDerivation>]) -> Vec<usize> {
        const DEPTH_CAP: usize = RelationLimits::MAX_TERM_DEPTH + 1;
        let mut depths = alloc::vec![0usize; self.states().len()];
        let mut order: Vec<usize> = best
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.as_ref().is_some_and(|entry| !entry.is_oversized()))
            .map(|(index, _)| index)
            .collect();
        order.sort_by_key(|index| best[*index].as_ref().expect("filtered exact").cost);
        for index in order {
            let entry = best[index].as_ref().expect("filtered exact");
            let depth = if entry.children.is_empty() {
                0
            } else {
                1 + entry
                    .children
                    .iter()
                    .map(|child| depths[*child])
                    .max()
                    .unwrap_or(0)
            };
            depths[index] = depth.min(DEPTH_CAP);
        }
        depths
    }

    /// For each state (indexed by canonical state order), the
    /// minimal-cost derivation a bottom-up run can produce there, if
    /// reachable. Monotone fixpoint; improvements strictly decrease
    /// `(cost, canonical form)`, and oversized candidates only fill
    /// empty slots, so termination is guaranteed and every exact
    /// backpointer graph is acyclic.
    fn best_derivations(
        &self,
        resources: &mut RelationResources,
    ) -> RewriteResult<Vec<Option<BestDerivation>>> {
        let mut best: Vec<Option<BestDerivation>> = alloc::vec![None; self.states().len()];
        let state_index = |target: &crate::language::TreeState| {
            self.states().iter().position(|state| state == target)
        };
        loop {
            let mut improved = false;
            for (transition_index, transition) in self.transitions().iter().enumerate() {
                resources.record_operations(1)?;
                let mut cost = 1usize;
                let mut children: Vec<usize> = Vec::with_capacity(transition.children().len());
                let mut complete = true;
                let mut oversized = false;
                for child in transition.children() {
                    let Some(index) = state_index(child) else {
                        complete = false;
                        break;
                    };
                    match &best[index] {
                        Some(entry) => {
                            oversized |= entry.is_oversized();
                            cost = cost.saturating_add(entry.cost).min(COST_CAP);
                            children.push(index);
                        }
                        None => {
                            complete = false;
                            break;
                        }
                    }
                }
                if !complete {
                    continue;
                }
                let Some(parent_index) = state_index(transition.parent()) else {
                    continue;
                };
                let candidate = if oversized || cost >= COST_CAP {
                    // Reachability marker only: no backpointers, so
                    // oversized entries can never create cycles.
                    BestDerivation {
                        cost: COST_CAP,
                        transition: transition_index,
                        children: Vec::new(),
                    }
                } else {
                    BestDerivation {
                        cost,
                        transition: transition_index,
                        children,
                    }
                };
                let dominated = match &best[parent_index] {
                    None => true,
                    // Oversized candidates never displace an existing
                    // entry (existing is equally "too big" or exact).
                    Some(_) if candidate.is_oversized() => false,
                    // An exact derivation always displaces an
                    // oversized marker.
                    Some(current) if current.is_oversized() => true,
                    Some(current) => {
                        if candidate.cost == current.cost {
                            // The lazy canonical comparator traverses
                            // derivations bounded by the state count.
                            resources.record_operations(self.states().len())?;
                        }
                        candidate.cost < current.cost
                            || (candidate.cost == current.cost
                                && self.compare_forms_fresh(&best, &candidate, current)
                                    == Ordering::Less)
                    }
                };
                if dominated {
                    best[parent_index] = Some(candidate);
                    improved = true;
                }
            }
            if !improved {
                return Ok(best);
            }
        }
    }

    /// Compare a candidate form against a table entry during the
    /// fixpoint. Top-level fields are compared inline; child
    /// derivations go through the iterative pair comparator.
    fn compare_forms_fresh(
        &self,
        best: &[Option<BestDerivation>],
        left: &BestDerivation,
        right: &BestDerivation,
    ) -> Ordering {
        let left_symbol = self.transitions()[left.transition].symbol();
        let right_symbol = self.transitions()[right.transition].symbol();
        let head = le_u32(left.children.len())
            .cmp(&le_u32(right.children.len()))
            .then_with(|| {
                le_u32(left_symbol.as_str().len()).cmp(&le_u32(right_symbol.as_str().len()))
            })
            .then_with(|| left_symbol.as_str().cmp(right_symbol.as_str()));
        if head != Ordering::Equal {
            return head;
        }
        let mut memo = BTreeMap::new();
        left.children
            .iter()
            .zip(right.children.iter())
            .map(|(a, b)| self.compare_state_pair(best, *a, *b, &mut memo))
            .find(|ordering| *ordering != Ordering::Equal)
            .unwrap_or(Ordering::Equal)
    }

    /// Canonical-order comparison of two state derivations with an
    /// EXPLICIT work stack, reproducing the shared `encode_term`
    /// byte order exactly (arity and name length as little-endian
    /// u32 bytes, then name bytes, then children left to right).
    /// Only exact derivations are compared, so costs strictly
    /// decrease along edges and no cycle guard is needed — but
    /// acyclicity alone does not bound recursion depth, and
    /// deep-but-exact derivations must not grow the call stack
    /// (PR #267 round 3). Memoized per pair.
    fn compare_state_pair(
        &self,
        best: &[Option<BestDerivation>],
        left: usize,
        right: usize,
        memo: &mut BTreeMap<(usize, usize), Ordering>,
    ) -> Ordering {
        /// Post-order frame: `Visit` expands a pair, `Resolve`
        /// combines child results once they are memoized.
        enum Work {
            Visit(usize, usize),
            Resolve(usize, usize),
        }
        let mut work = alloc::vec![Work::Visit(left, right)];
        while let Some(item) = work.pop() {
            match item {
                Work::Visit(a, b) => {
                    if a == b || memo.contains_key(&(a, b)) {
                        continue;
                    }
                    let (Some(left_entry), Some(right_entry)) = (&best[a], &best[b]) else {
                        memo.insert((a, b), Ordering::Equal);
                        continue;
                    };
                    let left_symbol = self.transitions()[left_entry.transition].symbol();
                    let right_symbol = self.transitions()[right_entry.transition].symbol();
                    let head = le_u32(left_entry.children.len())
                        .cmp(&le_u32(right_entry.children.len()))
                        .then_with(|| {
                            le_u32(left_symbol.as_str().len())
                                .cmp(&le_u32(right_symbol.as_str().len()))
                        })
                        .then_with(|| left_symbol.as_str().cmp(right_symbol.as_str()));
                    if head != Ordering::Equal {
                        memo.insert((a, b), head);
                        continue;
                    }
                    work.push(Work::Resolve(a, b));
                    // Children resolve left to right; push reversed
                    // so the leftmost is processed first.
                    for (child_a, child_b) in left_entry
                        .children
                        .iter()
                        .zip(right_entry.children.iter())
                        .rev()
                    {
                        work.push(Work::Visit(*child_a, *child_b));
                    }
                }
                Work::Resolve(a, b) => {
                    let (Some(left_entry), Some(right_entry)) = (&best[a], &best[b]) else {
                        memo.insert((a, b), Ordering::Equal);
                        continue;
                    };
                    let ordering = left_entry
                        .children
                        .iter()
                        .zip(right_entry.children.iter())
                        .map(|(child_a, child_b)| {
                            if child_a == child_b {
                                Ordering::Equal
                            } else {
                                memo.get(&(*child_a, *child_b)).copied().unwrap_or_else(|| {
                                    unreachable!("children are visited before Resolve")
                                })
                            }
                        })
                        .find(|ordering| *ordering != Ordering::Equal)
                        .unwrap_or(Ordering::Equal);
                    memo.insert((a, b), ordering);
                }
            }
        }
        if left == right {
            return Ordering::Equal;
        }
        memo.get(&(left, right)).copied().unwrap_or(Ordering::Equal)
    }
}

/// Little-endian u32 bytes, matching the shared term encoder's
/// fixed-width fields (byte-lexicographic order of the encoding).
fn le_u32(value: usize) -> [u8; 4] {
    (value as u32).to_le_bytes()
}
