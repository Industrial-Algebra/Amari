// SPDX-License-Identifier: MIT OR Apache-2.0

//! Language operations on tree automata: union, intersection,
//! trimming, and emptiness.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::language::automaton::TreeAutomaton;
use crate::language::{TreeAutomatonLimits, TreeState, TreeTransition};
use crate::trs::Symbol;

impl TreeAutomaton {
    /// Union of two automata over the SAME ranked alphabet. Operand
    /// states are renamed disjointly (`L:` / `R:` prefixes — the
    /// encoding is injective per side and the prefixes never
    /// collide), so shared state names in the inputs can never merge
    /// behavior. The result is canonical.
    pub fn union(
        &self,
        other: &TreeAutomaton,
        limits: &TreeAutomatonLimits,
    ) -> RewriteResult<TreeAutomaton> {
        self.require_same_alphabet(other)?;
        let total = self.states().len() + other.states().len();
        if total > limits.max_states() {
            return Err(RewriteError::InvalidLimit {
                resource: "tree automaton states",
                value: total,
                ceiling: limits.max_states(),
            });
        }
        let rename = |prefix: &str, state: &TreeState| {
            TreeState::new(Symbol::new(format!("{prefix}:{}", state.name())))
        };
        let mut states = Vec::with_capacity(total);
        let mut transitions =
            Vec::with_capacity(self.transitions().len() + other.transitions().len());
        let mut finals = Vec::with_capacity(self.finals().len() + other.finals().len());
        for (prefix, automaton) in [("L", self), ("R", other)] {
            states.extend(automaton.states().iter().map(|s| rename(prefix, s)));
            finals.extend(automaton.finals().iter().map(|s| rename(prefix, s)));
            transitions.extend(automaton.transitions().iter().map(|t| {
                TreeTransition::new(
                    t.symbol().clone(),
                    t.children().iter().map(|c| rename(prefix, c)).collect(),
                    rename(prefix, t.parent()),
                )
            }));
        }
        TreeAutomaton::new(
            self.alphabet().to_vec(),
            states,
            transitions,
            finals,
            *limits,
        )
    }

    /// Product intersection of two automata over the SAME ranked
    /// alphabet. Product states are encoded injectively
    /// (`len#name/len#name`), so distinct pairs never collide. Work
    /// is charged to the workspace operation budget; an oversized
    /// product is [`RewriteError::InvalidLimit`], never a silently
    /// truncated automaton.
    pub fn intersection(
        &self,
        other: &TreeAutomaton,
        limits: &TreeAutomatonLimits,
    ) -> RewriteResult<TreeAutomaton> {
        self.require_same_alphabet(other)?;
        let mut resources = RelationBudget::new();
        // Group the right-hand transitions by symbol so only
        // same-symbol pairs are scanned.
        let mut right_by_symbol: BTreeMap<&Symbol, Vec<&TreeTransition>> = BTreeMap::new();
        for transition in other.transitions() {
            right_by_symbol
                .entry(transition.symbol())
                .or_default()
                .push(transition);
        }
        let encode_pair = |left: &TreeState, right: &TreeState| {
            let (a, b) = (left.name().as_str(), right.name().as_str());
            TreeState::new(Symbol::new(format!("{}#{}/{}#{}", a.len(), a, b.len(), b)))
        };
        let mut transitions: Vec<TreeTransition> = Vec::new();
        let mut seen: BTreeSet<TreeTransition> = BTreeSet::new();
        let mut states: BTreeSet<TreeState> = BTreeSet::new();
        for left in self.transitions() {
            let Some(candidates) = right_by_symbol.get(left.symbol()) else {
                continue;
            };
            for right in candidates {
                resources.charge()?;
                // Same alphabet, so same symbol implies same arity.
                let parent = encode_pair(left.parent(), right.parent());
                let children: Vec<TreeState> = left
                    .children()
                    .iter()
                    .zip(right.children().iter())
                    .map(|(a, b)| encode_pair(a, b))
                    .collect();
                let transition = TreeTransition::new(left.symbol().clone(), children, parent);
                if seen.insert(transition.clone()) {
                    states.insert(transition.parent().clone());
                    states.extend(transition.children().iter().cloned());
                    transitions.push(transition);
                }
            }
        }
        let finals: Vec<TreeState> = self
            .finals()
            .iter()
            .flat_map(|a| other.finals().iter().map(move |b| encode_pair(a, b)))
            .collect();
        for final_state in &finals {
            states.insert(final_state.clone());
        }
        if states.len() > limits.max_states() {
            return Err(RewriteError::InvalidLimit {
                resource: "tree automaton states",
                value: states.len(),
                ceiling: limits.max_states(),
            });
        }
        if transitions.len() > limits.max_transitions() {
            return Err(RewriteError::InvalidLimit {
                resource: "tree automaton transitions",
                value: transitions.len(),
                ceiling: limits.max_transitions(),
            });
        }
        TreeAutomaton::new(
            self.alphabet().to_vec(),
            states.into_iter().collect(),
            transitions,
            finals,
            *limits,
        )
    }

    /// Remove states that are unreachable (no bottom-up run reaches
    /// them) or not co-reachable (no run through them reaches a
    /// final), together with every transition touching them. The
    /// language is preserved exactly; the alphabet is retained so
    /// the result stays composable. Idempotent.
    pub fn trimmed(&self) -> RewriteResult<TreeAutomaton> {
        let reachable = self.reachable_states();
        let co_reachable = self.co_reachable_states();
        let keep: BTreeSet<&TreeState> = self
            .states()
            .iter()
            .filter(|s| reachable.contains(*s) && co_reachable.contains(*s))
            .collect();
        let states: Vec<TreeState> = keep.iter().map(|s| (*s).clone()).collect();
        let transitions: Vec<TreeTransition> = self
            .transitions()
            .iter()
            .filter(|t| keep.contains(t.parent()) && t.children().iter().all(|c| keep.contains(c)))
            .cloned()
            .collect();
        let finals: Vec<TreeState> = self
            .finals()
            .iter()
            .filter(|f| keep.contains(*f))
            .cloned()
            .collect();
        TreeAutomaton::new(
            self.alphabet().to_vec(),
            states,
            transitions,
            finals,
            *self.limits(),
        )
    }

    /// Whether the accepted language is empty: no final state is
    /// bottom-up reachable.
    pub fn language_is_empty(&self) -> bool {
        let reachable = self.reachable_states();
        !self.finals().iter().any(|f| reachable.contains(f))
    }

    fn require_same_alphabet(&self, other: &TreeAutomaton) -> RewriteResult<()> {
        if self.alphabet() != other.alphabet() {
            return Err(RewriteError::MalformedAutomaton {
                message: "alphabet mismatch: language operations require the same \
                          ranked alphabet"
                    .into(),
            });
        }
        Ok(())
    }

    /// States reachable by some bottom-up run (from constants).
    pub(crate) fn reachable_states(&self) -> BTreeSet<TreeState> {
        let mut reachable: BTreeSet<TreeState> = BTreeSet::new();
        loop {
            let mut grew = false;
            for transition in self.transitions() {
                if reachable.contains(transition.parent()) {
                    continue;
                }
                if transition.children().iter().all(|c| reachable.contains(c)) {
                    reachable.insert(transition.parent().clone());
                    grew = true;
                }
            }
            if !grew {
                return reachable;
            }
        }
    }

    /// States that can reach a final state (as the parent of a run
    /// suffix ending in a final at the root).
    fn co_reachable_states(&self) -> BTreeSet<TreeState> {
        let mut co_reachable: BTreeSet<TreeState> = self.finals().iter().cloned().collect();
        loop {
            let mut grew = false;
            for transition in self.transitions() {
                if co_reachable.contains(transition.parent()) {
                    for child in transition.children() {
                        if co_reachable.insert(child.clone()) {
                            grew = true;
                        }
                    }
                }
            }
            if !grew {
                return co_reachable;
            }
        }
    }
}

/// Fixed operation budget for language construction scans (the
/// workspace operations ceiling).
struct RelationBudget {
    remaining: usize,
}

impl RelationBudget {
    fn new() -> Self {
        Self {
            remaining: crate::relation::RelationLimits::MAX_OPERATIONS,
        }
    }

    fn charge(&mut self) -> RewriteResult<()> {
        if self.remaining == 0 {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "operations",
                limit: crate::relation::RelationLimits::MAX_OPERATIONS,
            });
        }
        self.remaining -= 1;
        Ok(())
    }
}
