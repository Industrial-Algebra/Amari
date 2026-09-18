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
use crate::relation::RelationLimits;
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
    pub fn witness(&self) -> RewriteResult<Option<Term>> {
        let best = self.best_derivations();
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
        let Some(chosen) = exact.iter().copied().min_by(|left, right| {
            let (left_entry, right_entry) = (
                best[*left].as_ref().expect("present"),
                best[*right].as_ref().expect("present"),
            );
            left_entry.cost.cmp(&right_entry.cost).then_with(|| {
                let mut memo = BTreeMap::new();
                self.compare_state_indices(&best, *left, *right, &mut memo)
            })
        }) else {
            // Every reachable final is oversized: the language is
            // nonempty but its smallest member exceeds the ceilings.
            return Err(RewriteError::RelationLimitExceeded {
                resource: "witness term nodes",
                limit: RelationLimits::MAX_TERM_NODES,
            });
        };
        // Depth is measured on the FINALIZED graph, edge-based.
        let mut depth_memo = BTreeMap::new();
        let depth = self.finalized_depth(&best, chosen, &mut depth_memo);
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

    /// Edge-based depth of the finalized derivation for `index`
    /// (constants are depth 0, matching membership). Memoized over
    /// the acyclic exact-cost backpointer graph.
    fn finalized_depth(
        &self,
        best: &[Option<BestDerivation>],
        index: usize,
        memo: &mut BTreeMap<usize, usize>,
    ) -> usize {
        if let Some(depth) = memo.get(&index) {
            return *depth;
        }
        let entry = best[index].as_ref().expect("reachable state");
        let depth = if entry.children.is_empty() {
            0
        } else {
            1 + entry
                .children
                .iter()
                .map(|child| self.finalized_depth(best, *child, memo))
                .max()
                .unwrap_or(0)
        };
        memo.insert(index, depth);
        depth
    }

    /// For each state (indexed by canonical state order), the
    /// minimal-cost derivation a bottom-up run can produce there, if
    /// reachable. Monotone fixpoint; improvements strictly decrease
    /// `(cost, canonical form)`, and oversized candidates only fill
    /// empty slots, so termination is guaranteed and every exact
    /// backpointer graph is acyclic.
    fn best_derivations(&self) -> Vec<Option<BestDerivation>> {
        let mut best: Vec<Option<BestDerivation>> = alloc::vec![None; self.states().len()];
        let state_index = |target: &crate::language::TreeState| {
            self.states().iter().position(|state| state == target)
        };
        loop {
            let mut improved = false;
            for (transition_index, transition) in self.transitions().iter().enumerate() {
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
                return best;
            }
        }
    }

    fn compare_forms_fresh(
        &self,
        best: &[Option<BestDerivation>],
        left: &BestDerivation,
        right: &BestDerivation,
    ) -> Ordering {
        let mut memo = BTreeMap::new();
        self.compare_forms(best, left, right, &mut memo)
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
            return Ordering::Equal;
        };
        let ordering = self.compare_forms(best, left_entry, right_entry, memo);
        memo.insert((left, right), ordering);
        ordering
    }

    /// Canonical-order comparison of two derivations, reproducing
    /// the shared `encode_term` byte order exactly: symbol arity and
    /// name length as little-endian u32 bytes, then name bytes, then
    /// children left to right. Only exact derivations reach this
    /// comparator, so recursion follows strictly decreasing costs
    /// and is acyclic.
    fn compare_forms(
        &self,
        best: &[Option<BestDerivation>],
        left: &BestDerivation,
        right: &BestDerivation,
        memo: &mut BTreeMap<(usize, usize), Ordering>,
    ) -> Ordering {
        let left_symbol = self.transitions()[left.transition].symbol();
        let right_symbol = self.transitions()[right.transition].symbol();
        le_u32(left.children.len())
            .cmp(&le_u32(right.children.len()))
            .then_with(|| {
                le_u32(left_symbol.as_str().len()).cmp(&le_u32(right_symbol.as_str().len()))
            })
            .then_with(|| left_symbol.as_str().cmp(right_symbol.as_str()))
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

/// Little-endian u32 bytes, matching the shared term encoder's
/// fixed-width fields (byte-lexicographic order of the encoding).
fn le_u32(value: usize) -> [u8; 4] {
    (value as u32).to_le_bytes()
}
