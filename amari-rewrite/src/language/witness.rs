// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic smallest-witness extraction for tree automata.

use alloc::vec::Vec;

use crate::language::automaton::TreeAutomaton;
use crate::relation::digest::encode_term;
use crate::trs::Term;

impl TreeAutomaton {
    /// The smallest accepted ground term: fewest nodes first, ties
    /// broken by canonical term bytes. Deterministic — repeated
    /// calls and canonically equal automata return identical terms.
    /// `None` exactly when the language is empty.
    pub fn witness(&self) -> Option<Term> {
        let terms = self.smallest_terms_per_state();
        self.finals()
            .iter()
            .filter_map(|final_state| {
                self.states()
                    .iter()
                    .position(|state| state == final_state)
                    .and_then(|index| terms[index].clone())
            })
            .min_by(|(left_cost, left), (right_cost, right)| {
                left_cost
                    .cmp(right_cost)
                    .then_with(|| canonical_bytes(left).cmp(&canonical_bytes(right)))
            })
            .map(|(_, term)| term)
    }

    /// For each state (indexed by canonical state order), the
    /// smallest term a bottom-up run can produce in that state, if
    /// the state is reachable. Computed by monotone fixpoint;
    /// improvements strictly decrease `(cost, canonical bytes)`, so
    /// termination is guaranteed.
    pub(crate) fn smallest_terms_per_state(&self) -> Vec<Option<(usize, Term)>> {
        let mut best: Vec<Option<(usize, Term)>> = alloc::vec![None; self.states().len()];
        loop {
            let mut improved = false;
            for transition in self.transitions() {
                let mut child_terms: Vec<Term> = Vec::with_capacity(transition.children().len());
                let mut cost = 1usize;
                let mut complete = true;
                for child in transition.children() {
                    let Some(index) = self.states().iter().position(|s| s == child) else {
                        complete = false;
                        break;
                    };
                    match &best[index] {
                        Some((child_cost, child_term)) => {
                            cost = cost.saturating_add(*child_cost);
                            child_terms.push(child_term.clone());
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
                let Some(parent_index) =
                    self.states().iter().position(|s| s == transition.parent())
                else {
                    continue;
                };
                let candidate = Term::sym(transition.symbol().clone(), child_terms);
                let dominated = match &best[parent_index] {
                    None => true,
                    Some((best_cost, best_term)) => {
                        (cost, canonical_bytes(&candidate))
                            < (*best_cost, canonical_bytes(best_term))
                    }
                };
                if dominated {
                    best[parent_index] = Some((cost, candidate));
                    improved = true;
                }
            }
            if !improved {
                return best;
            }
        }
    }
}

/// Canonical term bytes (the shared relation-layer encoding) used
/// for deterministic tie-breaking.
fn canonical_bytes(term: &Term) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut names: Vec<alloc::string::String> = Vec::new();
    encode_term(term, &mut names, &mut bytes);
    bytes
}
