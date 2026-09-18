// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic smallest-witness extraction for tree automata.
//!
//! Memory discipline (PR #267 review): minimal COSTS and
//! backpointers are computed for every reachable state, but term
//! trees are never materialized per state — a chain like
//! `f(q{i-1}, q{i-1}) -> qi` would otherwise expand to exponential
//! trees for states that cannot even contribute to acceptance.
//! Canonical tie-breaking compares candidates lazily over the
//! backpointer graph (costs strictly decrease along backpointers,
//! so comparison is acyclic), and only the single chosen final
//! witness is materialized. A smallest witness that itself exceeds
//! the fixed term ceilings is a typed
//! [`crate::error::RewriteError::RelationLimitExceeded`] error —
//! never conflated with an empty language.

use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::cmp::Ordering;

use crate::error::{RewriteError, RewriteResult};
use crate::language::automaton::TreeAutomaton;
use crate::relation::RelationLimits;
use crate::trs::Term;

/// The chosen derivation of one state: minimal cost plus the
/// transition and child-state selections that achieve it.
#[derive(Clone, Debug)]
struct BestDerivation {
    cost: usize,
    depth: usize,
    transition: usize,
    children: Vec<usize>,
}

impl TreeAutomaton {
    /// The smallest accepted ground term: fewest nodes first, ties
    /// broken by canonical term bytes. Deterministic — repeated
    /// calls and canonically equal automata return identical terms.
    /// `None` exactly when the language is empty.
    ///
    /// Returns a typed limit error when the smallest witness itself
    /// exceeds the fixed term node/depth ceilings: the language is
    /// nonempty, but its smallest member cannot be represented
    /// within the authority.
    pub fn witness(&self) -> RewriteResult<Option<Term>> {
        let best = self.best_derivations();
        let chosen = self
            .finals()
            .iter()
            .filter_map(|final_state| {
                self.states()
                    .iter()
                    .position(|state| state == final_state)
                    .and_then(|index| best[index].as_ref().map(|entry| (index, entry)))
            })
            .min_by(|(left_index, left), (right_index, right)| {
                left.cost
                    .cmp(&right.cost)
                    .then_with(|| self.compare_states(&best, *left_index, *right_index))
            });
        let Some((_, entry)) = chosen else {
            return Ok(None);
        };
        if entry.cost > RelationLimits::MAX_TERM_NODES {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "witness term nodes",
                limit: RelationLimits::MAX_TERM_NODES,
            });
        }
        if entry.depth > RelationLimits::MAX_TERM_DEPTH {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "witness term depth",
                limit: RelationLimits::MAX_TERM_DEPTH,
            });
        }
        Ok(Some(self.materialize(&best, entry)))
    }

    /// Materialize exactly one derivation by unfolding its
    /// backpointers. Callers must have verified the cost/depth
    /// ceilings, so the unfold is bounded.
    fn materialize(&self, best: &[Option<BestDerivation>], entry: &BestDerivation) -> Term {
        let transition = &self.transitions()[entry.transition];
        Term::sym(
            transition.symbol().clone(),
            entry.children.iter().map(|child| {
                let child_entry = best[*child]
                    .as_ref()
                    .expect("derivation children are complete by construction");
                self.materialize(best, child_entry)
            }),
        )
    }

    /// For each state (indexed by canonical state order), the
    /// minimal-cost derivation a bottom-up run can produce there,
    /// if reachable. Monotone fixpoint: improvements strictly
    /// decrease `(cost, canonical form)`, so termination is
    /// guaranteed. No term trees are built here — derivations are
    /// transition/child backpointers only.
    fn best_derivations(&self) -> Vec<Option<BestDerivation>> {
        let mut best: Vec<Option<BestDerivation>> = alloc::vec![None; self.states().len()];
        let state_index = |target: &crate::language::TreeState| {
            self.states().iter().position(|state| state == target)
        };
        loop {
            let mut improved = false;
            for (transition_index, transition) in self.transitions().iter().enumerate() {
                let mut cost = 1usize;
                let mut depth = 1usize;
                let mut children: Vec<usize> = Vec::with_capacity(transition.children().len());
                let mut complete = true;
                for child in transition.children() {
                    let Some(index) = state_index(child) else {
                        complete = false;
                        break;
                    };
                    match &best[index] {
                        Some(entry) => {
                            cost = cost.saturating_add(entry.cost);
                            depth = depth.max(entry.depth.saturating_add(1));
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
                let candidate = BestDerivation {
                    cost,
                    depth,
                    transition: transition_index,
                    children,
                };
                let dominated = match &best[parent_index] {
                    None => true,
                    Some(current) => {
                        candidate.cost < current.cost
                            || (candidate.cost == current.cost
                                && self.compare_derivations(&best, &candidate, current)
                                    == Ordering::Less)
                    }
                };
                if dominated {
                    best[parent_index] = Some(candidate);
                    improved = true;
                }
            }
            if !improved {
                return best;
            }
        }
    }

    /// Canonical-order comparison of two derivations of the same
    /// state, WITHOUT materializing terms: compare the transition
    /// symbol, then child derivations left to right. Costs strictly
    /// decrease along backpointers, so recursion is acyclic.
    fn compare_derivations(
        &self,
        best: &[Option<BestDerivation>],
        left: &BestDerivation,
        right: &BestDerivation,
    ) -> Ordering {
        let mut memo = BTreeMap::new();
        self.compare_forms(best, left, right, &mut memo)
    }

    fn compare_states(
        &self,
        best: &[Option<BestDerivation>],
        left: usize,
        right: usize,
    ) -> Ordering {
        let mut memo = BTreeMap::new();
        self.compare_state_indices(best, left, right, &mut memo)
    }

    fn compare_state_indices(
        &self,
        best: &[Option<BestDerivation>],
        left: usize,
        right: usize,
        memo: &mut BTreeMap<(usize, usize), Ordering>,
    ) -> Ordering {
        if left == right {
            return Ordering::Equal;
        }
        if let Some(ordering) = memo.get(&(left, right)) {
            return *ordering;
        }
        let (Some(left_entry), Some(right_entry)) = (&best[left], &best[right]) else {
            // Unreachable states have no derivation; they compare
            // greater so reachable states always win ties.
            return match (&best[left], &best[right]) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                _ => unreachable!(),
            };
        };
        let ordering = self.compare_forms(best, left_entry, right_entry, memo);
        memo.insert((left, right), ordering);
        ordering
    }

    fn compare_forms(
        &self,
        best: &[Option<BestDerivation>],
        left: &BestDerivation,
        right: &BestDerivation,
        memo: &mut BTreeMap<(usize, usize), Ordering>,
    ) -> Ordering {
        let left_symbol = self.transitions()[left.transition].symbol();
        let right_symbol = self.transitions()[right.transition].symbol();
        left_symbol
            .as_str()
            .cmp(right_symbol.as_str())
            .then_with(|| left.children.len().cmp(&right.children.len()))
            .then_with(|| {
                left.children
                    .iter()
                    .zip(right.children.iter())
                    .map(|(a, b)| self.compare_state_indices(best, *a, *b, memo))
                    .find(|ordering| *ordering != Ordering::Equal)
                    .unwrap_or(Ordering::Equal)
            })
    }
}
