// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded determinization, completion, and complement for tree
//! automata.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::language::automaton::TreeAutomaton;
use crate::language::{TreeAutomatonLimits, TreeState, TreeTransition};
use crate::relation::{RelationLimits, RelationResources};
use crate::trs::Symbol;

/// Hard exploration ceiling for the subset construction (design
/// resource authority): at most this many macro-states may be
/// discovered before the search is a typed limit error.
pub const MAX_DETERMINIZED_SUBSETS: usize = 65_536;

impl TreeAutomaton {
    /// Whether the automaton is deterministic: no two transitions
    /// share the same symbol and child-state tuple. Canonical
    /// storage is sorted by exactly that key, so a window scan
    /// suffices.
    pub fn is_deterministic(&self) -> bool {
        self.transitions().windows(2).all(|pair| {
            pair[0].symbol() != pair[1].symbol() || pair[0].children() != pair[1].children()
        })
    }

    /// Whether every (symbol, state-tuple) pair has a transition.
    /// Computed arithmetically (no tuple enumeration): the required
    /// coverage is Σ_symbol |Q|^arity; the present coverage is the
    /// number of distinct (symbol, children) keys.
    pub fn is_complete(&self) -> bool {
        let Some(required) = self.required_transition_count(self.states().len()) else {
            return false;
        };
        let mut present: u128 = 0;
        let mut previous: Option<&TreeTransition> = None;
        for transition in self.transitions() {
            let is_new_key = previous.is_none_or(|p| {
                p.symbol() != transition.symbol() || p.children() != transition.children()
            });
            if is_new_key {
                present += 1;
            }
            previous = Some(transition);
        }
        present >= required
    }

    /// Σ_symbol state_count^arity, or `None` on overflow (which
    /// always exceeds every ceiling).
    fn required_transition_count(&self, state_count: usize) -> Option<u128> {
        let mut required: u128 = 0;
        for ranked in self.alphabet() {
            let coverage = (state_count as u128).checked_pow(u32::from(ranked.arity()))?;
            required = required.checked_add(coverage)?;
        }
        Some(required)
    }

    /// Bounded subset construction. The result is deterministic and
    /// language-equivalent. Exploration is capped at
    /// [`MAX_DETERMINIZED_SUBSETS`] macro-states (a typed limit
    /// error, never a truncated automaton presented as complete),
    /// every tuple evaluation is charged to the workspace operation
    /// budget, and the result must satisfy the caller's limits.
    pub fn determinize(&self, limits: &TreeAutomatonLimits) -> RewriteResult<TreeAutomaton> {
        let mut resources = RelationResources::new(&RelationLimits::default());
        self.determinize_with_resources(limits, &mut resources)
    }

    /// [`Self::determinize`] under a caller-supplied operation
    /// budget: one operation is charged per odometer step (each
    /// registering macro-state m walks (m+1)^arity steps per
    /// non-nullary symbol) plus one per source transition examined
    /// for every qualifying tuple.
    pub fn determinize_with_resources(
        &self,
        limits: &TreeAutomatonLimits,
        resources: &mut RelationResources,
    ) -> RewriteResult<TreeAutomaton> {
        let state_index: BTreeMap<&TreeState, usize> = self
            .states()
            .iter()
            .enumerate()
            .map(|(index, state)| (state, index))
            .collect();
        let mut by_symbol: BTreeMap<&Symbol, Vec<&TreeTransition>> = BTreeMap::new();
        for transition in self.transitions() {
            by_symbol
                .entry(transition.symbol())
                .or_default()
                .push(transition);
        }

        let mut macros: Vec<BTreeSet<usize>> = Vec::new();
        let mut macro_index: BTreeMap<BTreeSet<usize>, usize> = BTreeMap::new();
        let mut out: Vec<(Symbol, Vec<usize>, usize)> = Vec::new();
        let register = |macros: &mut Vec<BTreeSet<usize>>,
                        macro_index: &mut BTreeMap<BTreeSet<usize>, usize>,
                        set: BTreeSet<usize>|
         -> RewriteResult<Option<usize>> {
            if set.is_empty() {
                return Ok(None);
            }
            if let Some(index) = macro_index.get(&set) {
                return Ok(Some(*index));
            }
            if macros.len() >= MAX_DETERMINIZED_SUBSETS {
                return Err(RewriteError::RelationLimitExceeded {
                    resource: "determinized subsets",
                    limit: MAX_DETERMINIZED_SUBSETS,
                });
            }
            let index = macros.len();
            macros.push(set.clone());
            macro_index.insert(set, index);
            Ok(Some(index))
        };

        // Seed with nullary symbols.
        for ranked in self.alphabet() {
            if ranked.arity() != 0 {
                continue;
            }
            let parents: BTreeSet<usize> = by_symbol
                .get(ranked.symbol())
                .into_iter()
                .flatten()
                .map(|t| state_index[t.parent()])
                .collect();
            if let Some(index) = register(&mut macros, &mut macro_index, parents)? {
                out.push((ranked.symbol().clone(), Vec::new(), index));
            }
        }
        // Expand: when macro m is registered, evaluate every tuple
        // whose highest-index component is m (each tuple is then
        // evaluated exactly once).
        let mut cursor = 0;
        while cursor < macros.len() {
            let m = cursor;
            cursor += 1;
            for ranked in self.alphabet() {
                let arity = usize::from(ranked.arity());
                if arity == 0 {
                    continue;
                }
                // Enumerate tuples over 0..=m containing m, via an
                // odometer over component positions. Every odometer
                // step is charged to the operation budget.
                let mut odometer = alloc::vec![0usize; arity];
                loop {
                    resources.record_operations(1)?;
                    let components = &odometer;
                    let contains_m = components.contains(&m);
                    let max_component = components.iter().copied().max().unwrap_or(0);
                    if contains_m && max_component == m {
                        let mut target: BTreeSet<usize> = BTreeSet::new();
                        let scanned = by_symbol
                            .get(ranked.symbol())
                            .map_or(0, |transitions| transitions.len());
                        resources.record_operations(scanned)?;
                        for transition in by_symbol.get(ranked.symbol()).into_iter().flatten() {
                            if transition
                                .children()
                                .iter()
                                .enumerate()
                                .all(|(position, child)| {
                                    macros[components[position]].contains(&state_index[child])
                                })
                            {
                                target.insert(state_index[transition.parent()]);
                            }
                        }
                        if let Some(target_index) = register(&mut macros, &mut macro_index, target)?
                        {
                            out.push((ranked.symbol().clone(), components.clone(), target_index));
                        }
                    }
                    // Advance the odometer over 0..=m.
                    let mut position = arity;
                    while position > 0 {
                        position -= 1;
                        odometer[position] += 1;
                        if odometer[position] <= m {
                            break;
                        }
                        odometer[position] = 0;
                        if position == 0 {
                            break;
                        }
                    }
                    if odometer.iter().all(|component| *component == 0) {
                        break;
                    }
                }
            }
        }

        // Canonical naming: sort macro-states by their sorted member
        // names, then name them d0..dn in that order.
        let mut order: Vec<usize> = (0..macros.len()).collect();
        order.sort_by_key(|index| {
            macros[*index]
                .iter()
                .map(|member| self.states()[*member].name().as_str())
                .collect::<Vec<_>>()
        });
        let mut canonical_name: Vec<String> = alloc::vec![String::new(); macros.len()];
        for (rank, index) in order.into_iter().enumerate() {
            canonical_name[index] = format!("d{rank}");
        }
        let states: Vec<TreeState> = (0..macros.len())
            .map(|index| TreeState::new(Symbol::new(canonical_name[index].clone())))
            .collect();
        let transitions: Vec<TreeTransition> = out
            .into_iter()
            .map(|(symbol, children, parent)| {
                TreeTransition::new(
                    symbol,
                    children
                        .into_iter()
                        .map(|child| states[child].clone())
                        .collect(),
                    states[parent].clone(),
                )
            })
            .collect();
        let finals: Vec<TreeState> = macros
            .iter()
            .enumerate()
            .filter(|(_, members)| {
                members.iter().any(|member| {
                    let original = &self.states()[*member];
                    self.finals().contains(original)
                })
            })
            .map(|(index, _)| states[index].clone())
            .collect();
        TreeAutomaton::new(
            self.alphabet().to_vec(),
            states,
            transitions,
            finals,
            *limits,
        )
    }

    /// Add a sink state covering every missing (symbol, tuple), for
    /// a DETERMINISTIC automaton. The completed automaton is
    /// deterministic, complete, and language-equivalent. Completion
    /// that would exceed the transition ceiling is a typed
    /// `InvalidLimit`, never a partial completion.
    pub fn completed(&self) -> RewriteResult<TreeAutomaton> {
        let mut resources = RelationResources::new(&RelationLimits::default());
        self.completed_with_resources(&mut resources)
    }

    /// [`Self::completed`] under a caller-supplied operation budget:
    /// one operation is charged per odometer step (one per state
    /// tuple per symbol).
    pub fn completed_with_resources(
        &self,
        resources: &mut RelationResources,
    ) -> RewriteResult<TreeAutomaton> {
        if !self.is_deterministic() {
            return Err(RewriteError::MalformedAutomaton {
                message: "completion requires a deterministic automaton".into(),
            });
        }
        if self.is_complete() {
            return Ok(self.clone());
        }
        let limits = *self.limits();
        let new_state_count = self.states().len() + 1;
        if new_state_count > limits.max_states() {
            return Err(RewriteError::InvalidLimit {
                resource: "tree automaton states",
                value: new_state_count,
                ceiling: limits.max_states(),
            });
        }
        let required = self
            .required_transition_count(new_state_count)
            .filter(|count| *count <= limits.max_transitions() as u128)
            .ok_or(RewriteError::InvalidLimit {
                resource: "tree automaton transitions",
                value: usize::MAX,
                ceiling: limits.max_transitions(),
            })?;
        let _ = required;

        // Fresh sink name, deterministically chosen.
        let mut sink_name = String::from("#sink");
        while self.states().iter().any(|s| s.name().as_str() == sink_name) {
            sink_name = format!("#{sink_name}");
        }
        let sink = TreeState::new(Symbol::new(sink_name));
        let mut states = self.states().to_vec();
        states.push(sink.clone());

        // Enumerate every (symbol, tuple) over the extended state
        // set, skipping covered keys.
        let covered: BTreeSet<(Symbol, Vec<TreeState>)> = self
            .transitions()
            .iter()
            .map(|t| (t.symbol().clone(), t.children().to_vec()))
            .collect();
        let mut transitions = self.transitions().to_vec();
        for ranked in self.alphabet() {
            let arity = usize::from(ranked.arity());
            let mut odometer = alloc::vec![0usize; arity];
            loop {
                let tuple: Vec<TreeState> = odometer
                    .iter()
                    .map(|index| states[*index].clone())
                    .collect();
                resources.record_operations(1)?;
                if !covered.contains(&(ranked.symbol().clone(), tuple.clone())) {
                    transitions.push(TreeTransition::new(
                        ranked.symbol().clone(),
                        tuple,
                        sink.clone(),
                    ));
                    if transitions.len() > limits.max_transitions() {
                        return Err(RewriteError::InvalidLimit {
                            resource: "tree automaton transitions",
                            value: transitions.len(),
                            ceiling: limits.max_transitions(),
                        });
                    }
                }
                // Advance; arity 0 runs exactly once.
                let mut position = arity;
                while position > 0 {
                    position -= 1;
                    odometer[position] += 1;
                    if odometer[position] < states.len() {
                        break;
                    }
                    odometer[position] = 0;
                    if position == 0 {
                        break;
                    }
                }
                if odometer.iter().all(|index| *index == 0) {
                    break;
                }
            }
        }
        TreeAutomaton::new(
            self.alphabet().to_vec(),
            states,
            transitions,
            self.finals().to_vec(),
            limits,
        )
    }

    /// Swap accepting and rejecting states. Only meaningful — and
    /// only permitted — for deterministic complete automata.
    pub fn complemented(&self) -> RewriteResult<TreeAutomaton> {
        if !self.is_deterministic() || !self.is_complete() {
            return Err(RewriteError::MalformedAutomaton {
                message: "complement requires a deterministic complete automaton".into(),
            });
        }
        let finals: BTreeSet<&TreeState> = self.finals().iter().collect();
        let complement: Vec<TreeState> = self
            .states()
            .iter()
            .filter(|state| !finals.contains(state))
            .cloned()
            .collect();
        TreeAutomaton::new(
            self.alphabet().to_vec(),
            self.states().to_vec(),
            self.transitions().to_vec(),
            complement,
            *self.limits(),
        )
    }
}
