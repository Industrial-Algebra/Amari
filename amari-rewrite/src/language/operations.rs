// SPDX-License-Identifier: MIT OR Apache-2.0

//! Language operations on tree automata: union, intersection,
//! trimming, and emptiness.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::language::automaton::TreeAutomaton;
use crate::language::{TreeAutomatonLimits, TreeState, TreeTransition};
use crate::relation::{RelationLimits, RelationResources};
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
        // The unmetered entry point retains an unbounded workspace
        // (status quo); [`Self::union_with_resources`] is the
        // budgeted variant.
        let mut resources = RelationResources::unbounded();
        self.union_with_resources(other, limits, &mut resources)
    }

    /// [`Self::union`] under a caller-supplied operation budget: one
    /// operation is charged per state, transition, and final
    /// renamed into the disjoint-union product.
    pub fn union_with_resources(
        &self,
        other: &TreeAutomaton,
        limits: &TreeAutomatonLimits,
        resources: &mut RelationResources,
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
            for state in automaton.states() {
                resources.record_operations(1)?;
                states.push(rename(prefix, state));
            }
            for final_state in automaton.finals() {
                resources.record_operations(1)?;
                finals.push(rename(prefix, final_state));
            }
            for transition in automaton.transitions() {
                resources.record_operations(1)?;
                transitions.push(TreeTransition::new(
                    transition.symbol().clone(),
                    transition
                        .children()
                        .iter()
                        .map(|c| rename(prefix, c))
                        .collect(),
                    rename(prefix, transition.parent()),
                ));
            }
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
        let mut resources = RelationResources::new(&RelationLimits::default());
        self.intersection_with_resources(other, limits, &mut resources)
    }

    /// [`Self::intersection`] under a caller-supplied operation
    /// budget: one operation is charged per compatible transition
    /// pair examined and per final-state pair enumerated.
    pub fn intersection_with_resources(
        &self,
        other: &TreeAutomaton,
        limits: &TreeAutomatonLimits,
        resources: &mut RelationResources,
    ) -> RewriteResult<TreeAutomaton> {
        self.require_same_alphabet(other)?;
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
                resources.record_operations(1)?;
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
                    // Enforce output ceilings as storage grows:
                    // never accumulate past a limit. Count DISTINCT
                    // missing states only — a parent/child already
                    // present (or repeated among the children) adds
                    // nothing.
                    if transitions.len() + 1 > limits.max_transitions() {
                        return Err(RewriteError::InvalidLimit {
                            resource: "tree automaton transitions",
                            value: transitions.len() + 1,
                            ceiling: limits.max_transitions(),
                        });
                    }
                    for candidate in
                        core::iter::once(transition.parent()).chain(transition.children().iter())
                    {
                        if !states.contains(candidate) {
                            if states.len() + 1 > limits.max_states() {
                                return Err(RewriteError::InvalidLimit {
                                    resource: "tree automaton states",
                                    value: states.len() + 1,
                                    ceiling: limits.max_states(),
                                });
                            }
                            states.insert(candidate.clone());
                        }
                    }
                    transitions.push(transition);
                }
            }
        }
        // Preflight the final-state product BEFORE allocation: the
        // pair encoding is injective, so the product size is exact.
        let final_pairs = self
            .finals()
            .len()
            .checked_mul(other.finals().len())
            .filter(|pairs| *pairs <= limits.max_states())
            .ok_or(RewriteError::InvalidLimit {
                resource: "tree automaton states",
                value: usize::MAX,
                ceiling: limits.max_states(),
            })?;
        let mut finals: Vec<TreeState> = Vec::with_capacity(final_pairs);
        for left_final in self.finals() {
            for right_final in other.finals() {
                resources.record_operations(1)?;
                finals.push(encode_pair(left_final, right_final));
            }
        }
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
        // The unmetered entry point retains an unbounded workspace
        // (status quo); [`Self::trimmed_with_resources`] is the
        // budgeted variant.
        let mut resources = RelationResources::unbounded();
        self.trimmed_with_resources(&mut resources)
    }

    /// [`Self::trimmed`] under a caller-supplied operation budget:
    /// one operation is charged per transition examined in every
    /// reachability and co-reachability fixpoint round.
    pub fn trimmed_with_resources(
        &self,
        resources: &mut RelationResources,
    ) -> RewriteResult<TreeAutomaton> {
        let reachable = self.reachable_states(resources)?;
        let co_reachable = self.co_reachable_states(&reachable, resources)?;
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
    /// bottom-up reachable. This non-fallible convenience runs with
    /// an unbounded workspace; the budgeted variant is
    /// [`Self::language_is_empty_with_resources`].
    pub fn language_is_empty(&self) -> bool {
        let mut resources = RelationResources::unbounded();
        self.language_is_empty_with_resources(&mut resources)
            .expect("unbounded resources never exhaust")
    }

    /// [`Self::language_is_empty`] under a caller-supplied operation
    /// budget: one operation is charged per transition examined in
    /// every reachability fixpoint round.
    pub fn language_is_empty_with_resources(
        &self,
        resources: &mut RelationResources,
    ) -> RewriteResult<bool> {
        let reachable = self.reachable_states(resources)?;
        Ok(!self.finals().iter().any(|f| reachable.contains(f)))
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
    pub(crate) fn reachable_states(
        &self,
        resources: &mut RelationResources,
    ) -> RewriteResult<BTreeSet<TreeState>> {
        let mut reachable: BTreeSet<TreeState> = BTreeSet::new();
        loop {
            let mut grew = false;
            for transition in self.transitions() {
                resources.record_operations(1)?;
                if reachable.contains(transition.parent()) {
                    continue;
                }
                if transition.children().iter().all(|c| reachable.contains(c)) {
                    reachable.insert(transition.parent().clone());
                    grew = true;
                }
            }
            if !grew {
                return Ok(reachable);
            }
        }
    }

    /// States that can reach a final state (as the parent of a run
    /// suffix ending in a final at the root). Propagation only
    /// crosses transitions whose children are ALL bottom-up
    /// reachable: a co-reachable parent with an unrealizable sibling
    /// contributes nothing, and without this restriction the first
    /// trim kept states the second pass removed (idempotence
    /// violation).
    fn co_reachable_states(
        &self,
        reachable: &BTreeSet<TreeState>,
        resources: &mut RelationResources,
    ) -> RewriteResult<BTreeSet<TreeState>> {
        let mut co_reachable: BTreeSet<TreeState> = self.finals().iter().cloned().collect();
        loop {
            let mut grew = false;
            for transition in self.transitions() {
                resources.record_operations(1)?;
                if co_reachable.contains(transition.parent())
                    && transition
                        .children()
                        .iter()
                        .all(|child| reachable.contains(child))
                {
                    for child in transition.children() {
                        if co_reachable.insert(child.clone()) {
                            grew = true;
                        }
                    }
                }
            }
            if !grew {
                return Ok(co_reachable);
            }
        }
    }
}
