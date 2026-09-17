// SPDX-License-Identifier: MIT OR Apache-2.0

//! Epsilon-free bottom-up nondeterministic tree automata with
//! private, validated, canonical storage.

use alloc::collections::BTreeSet;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::language::{RankedSymbol, TreeAutomatonLimits};
use crate::relation::{RelationLimits, RelationResources};
use crate::trs::{Symbol, Term};

/// An automaton state. States are named; the automaton stores them
/// in canonical (sorted) order regardless of construction order.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct TreeState(Symbol);

impl TreeState {
    /// Create a state from a name.
    pub fn new(name: impl Into<Symbol>) -> Self {
        Self(name.into())
    }

    /// Borrow the state name.
    pub fn name(&self) -> &Symbol {
        &self.0
    }
}

/// One bottom-up transition: reading `symbol` over child states
/// `children` (exactly `arity` many) moves into `parent`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct TreeTransition {
    symbol: Symbol,
    children: Vec<TreeState>,
    parent: TreeState,
}

impl TreeTransition {
    /// Create a transition. Arity consistency against the ranked
    /// alphabet is checked at automaton construction.
    pub fn new(symbol: Symbol, children: Vec<TreeState>, parent: TreeState) -> Self {
        Self {
            symbol,
            children,
            parent,
        }
    }

    /// The read symbol.
    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }

    /// The required child states, left to right.
    pub fn children(&self) -> &[TreeState] {
        &self.children
    }

    /// The resulting parent state.
    pub fn parent(&self) -> &TreeState {
        &self.parent
    }
}

/// Evidence that a ground term is accepted: an assignment of one
/// state per node position that is backed by real transitions and
/// ends in a final state at the root.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct AcceptingRun {
    assignments: Vec<(Vec<usize>, TreeState)>,
}

impl AcceptingRun {
    /// All node assignments, ordered by position. Every node of the
    /// term appears exactly once.
    pub fn assignments(&self) -> &[(Vec<usize>, TreeState)] {
        &self.assignments
    }

    /// The state assigned at the root. Always a final state.
    pub fn root_state(&self) -> &TreeState {
        &self
            .assignments
            .first()
            .expect("a run assigns at least the root")
            .1
    }
}

/// A validated, canonically stored bottom-up tree automaton.
///
/// Construction enforces ranked-alphabet consistency, known states,
/// exact child arity, duplicate freedom, valid finals, and the hard
/// state/transition/rank ceilings — all before allocation of the
/// canonical storage. Equal automata are byte-identical regardless
/// of the order their parts were supplied in.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct TreeAutomaton {
    alphabet: Vec<RankedSymbol>,
    states: Vec<TreeState>,
    transitions: Vec<TreeTransition>,
    finals: Vec<TreeState>,
    limits: TreeAutomatonLimits,
}

impl TreeAutomaton {
    /// Construct and validate an automaton. Every malformed input is
    /// a typed [`RewriteError::MalformedAutomaton`]; every limit
    /// violation is [`RewriteError::InvalidLimit`].
    pub fn new(
        alphabet: Vec<RankedSymbol>,
        states: Vec<TreeState>,
        transitions: Vec<TreeTransition>,
        finals: Vec<TreeState>,
        limits: TreeAutomatonLimits,
    ) -> RewriteResult<Self> {
        // Limits first, before any allocation of canonical storage.
        for (resource, value, ceiling) in [
            ("tree automaton states", states.len(), limits.max_states()),
            (
                "tree automaton transitions",
                transitions.len(),
                limits.max_transitions(),
            ),
        ] {
            if value > ceiling {
                return Err(RewriteError::InvalidLimit {
                    resource,
                    value,
                    ceiling,
                });
            }
        }
        for ranked in &alphabet {
            if usize::from(ranked.arity()) > limits.max_rank() {
                return Err(RewriteError::InvalidLimit {
                    resource: "tree automaton rank",
                    value: usize::from(ranked.arity()),
                    ceiling: limits.max_rank(),
                });
            }
        }

        // Canonicalize the alphabet and detect rank conflicts and
        // duplicates.
        let mut alphabet = alphabet;
        alphabet.sort();
        for pair in alphabet.windows(2) {
            if pair[0].symbol() == pair[1].symbol() {
                if pair[0].arity() == pair[1].arity() {
                    return Err(malformed(format!(
                        "duplicate alphabet entry for symbol {}",
                        pair[0].symbol()
                    )));
                }
                return Err(malformed(format!(
                    "rank conflict for symbol {}: {} vs {}",
                    pair[0].symbol(),
                    pair[0].arity(),
                    pair[1].arity()
                )));
            }
        }

        // Canonicalize states and reject duplicates.
        let mut states = states;
        states.sort();
        for pair in states.windows(2) {
            if pair[0] == pair[1] {
                return Err(malformed(format!("duplicate state {}", pair[0].name())));
            }
        }
        let state_set: BTreeSet<&TreeState> = states.iter().collect();

        // Finals must be declared states, without duplicates.
        let mut finals = finals;
        finals.sort();
        for pair in finals.windows(2) {
            if pair[0] == pair[1] {
                return Err(malformed(format!(
                    "duplicate final state {}",
                    pair[0].name()
                )));
            }
        }
        for final_state in &finals {
            if !state_set.contains(final_state) {
                return Err(malformed(format!(
                    "final state {} is not a declared state",
                    final_state.name()
                )));
            }
        }

        // Every transition must reference the ranked alphabet with
        // exact arity and known states only.
        for transition in &transitions {
            let arity = alphabet
                .iter()
                .find(|ranked| ranked.symbol() == transition.symbol())
                .map(RankedSymbol::arity)
                .ok_or_else(|| {
                    malformed(format!(
                        "unknown symbol {} in transition",
                        transition.symbol()
                    ))
                })?;
            if transition.children().len() != usize::from(arity) {
                return Err(malformed(format!(
                    "arity mismatch for symbol {}: transition has {} children, rank is {}",
                    transition.symbol(),
                    transition.children().len(),
                    arity
                )));
            }
            if !state_set.contains(transition.parent()) {
                return Err(malformed(format!(
                    "unknown state {} in transition parent",
                    transition.parent().name()
                )));
            }
            for child in transition.children() {
                if !state_set.contains(child) {
                    return Err(malformed(format!(
                        "unknown state {} in transition children",
                        child.name()
                    )));
                }
            }
        }
        let mut transitions = transitions;
        transitions.sort();
        for pair in transitions.windows(2) {
            if pair[0] == pair[1] {
                return Err(malformed(format!(
                    "duplicate transition for symbol {}",
                    pair[0].symbol()
                )));
            }
        }

        Ok(Self {
            alphabet,
            states,
            transitions,
            finals,
            limits,
        })
    }

    /// The ranked alphabet, sorted by symbol.
    pub fn alphabet(&self) -> &[RankedSymbol] {
        &self.alphabet
    }

    /// The states, in canonical (sorted) order.
    pub fn states(&self) -> &[TreeState] {
        &self.states
    }

    /// The transitions, in canonical (sorted) order.
    pub fn transitions(&self) -> &[TreeTransition] {
        &self.transitions
    }

    /// The final states, in canonical (sorted) order.
    pub fn finals(&self) -> &[TreeState] {
        &self.finals
    }

    /// The limits this automaton was constructed under.
    pub fn limits(&self) -> &TreeAutomatonLimits {
        &self.limits
    }

    /// Whether the ground term is accepted. Errors are typed:
    /// non-ground terms are rejected, and oversized terms hit the
    /// fixed term limits.
    pub fn accepts(&self, term: &Term) -> RewriteResult<bool> {
        Ok(self.accepting_run(term)?.is_some())
    }

    /// An accepting run for the ground term, if one exists. The run
    /// is deterministic: among all valid runs, the lexicographically
    /// smallest state assignment is returned.
    pub fn accepting_run(&self, term: &Term) -> RewriteResult<Option<AcceptingRun>> {
        // Input validation before any search: ground, within the
        // fixed term ceilings, and inside the operation budget.
        if !term.variables().is_empty() {
            return Err(RewriteError::NonGroundTerm {
                message: "tree automaton membership requires a ground term".into(),
            });
        }
        let position_paths = term.positions();
        let positions: Vec<Vec<usize>> = position_paths
            .iter()
            .map(|path| path.as_slice().to_vec())
            .collect();
        let node_count = positions.len();
        if node_count > RelationLimits::MAX_TERM_NODES {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "term nodes",
                limit: RelationLimits::MAX_TERM_NODES,
            });
        }
        let depth = positions.iter().map(Vec::len).max().unwrap_or(0);
        if depth > RelationLimits::MAX_TERM_DEPTH {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "term depth",
                limit: RelationLimits::MAX_TERM_DEPTH,
            });
        }
        let mut resources = RelationResources::new(&RelationLimits::default());
        resources.record_term(node_count, depth)?;
        let path_index: alloc::collections::BTreeMap<&[usize], usize> = positions
            .iter()
            .enumerate()
            .map(|(index, path)| (path.as_slice(), index))
            .collect();

        // Bottom-up candidate sets, aligned with the preorder
        // position list. Processing positions in reverse preorder
        // visits children before parents.
        let mut candidates: Vec<BTreeSet<TreeState>> = alloc::vec![BTreeSet::new(); node_count];
        for (index, path) in positions.iter().enumerate().rev() {
            let node = term
                .subterm(&position_paths[index])
                .expect("positions are valid by construction");
            let Term::Sym(symbol, arguments) = node else {
                return Err(RewriteError::NonGroundTerm {
                    message: "tree automaton membership requires a ground term".into(),
                });
            };
            for transition in &self.transitions {
                resources.record_operations(1)?;
                if transition.symbol() != symbol || transition.children().len() != arguments.len() {
                    continue;
                }
                let children_match =
                    transition
                        .children()
                        .iter()
                        .enumerate()
                        .all(|(child_index, required)| {
                            let mut child_path = path.clone();
                            child_path.push(child_index);
                            path_index
                                .get(child_path.as_slice())
                                .is_some_and(|sub_index| candidates[*sub_index].contains(required))
                        });
                if children_match {
                    candidates[index].insert(transition.parent().clone());
                }
            }
        }

        // Extract the lexicographically smallest valid run, if any
        // final state is reachable at the root.
        let mut assignments: Vec<(Vec<usize>, TreeState)> = Vec::with_capacity(node_count);
        let Some(root_state) = self
            .finals
            .iter()
            .find(|final_state| candidates[0].contains(final_state))
            .cloned()
        else {
            return Ok(None);
        };
        assignments.push((Vec::new(), root_state));
        let mut cursor = 0;
        while cursor < assignments.len() {
            let (path, assigned) = assignments[cursor].clone();
            cursor += 1;
            let node = term
                .subterm(
                    position_paths
                        .iter()
                        .find(|candidate| candidate.as_slice() == path.as_slice())
                        .expect("assigned paths are valid by construction"),
                )
                .expect("assigned paths are valid by construction");
            let Term::Sym(symbol, arguments) = node else {
                continue;
            };
            if arguments.is_empty() {
                continue;
            }
            // The first transition in canonical order that matches
            // the symbol, the assigned parent, and the children's
            // candidate sets gives the smallest consistent tuple.
            let transition = self
                .transitions
                .iter()
                .find(|transition| {
                    transition.symbol() == symbol
                        && transition.parent() == &assigned
                        && transition.children().len() == arguments.len()
                        && transition.children().iter().enumerate().all(
                            |(child_index, required)| {
                                let mut child_path = path.clone();
                                child_path.push(child_index);
                                path_index
                                    .get(child_path.as_slice())
                                    .is_some_and(|sub_index| {
                                        candidates[*sub_index].contains(required)
                                    })
                            },
                        )
                })
                .expect("candidate construction guarantees a backing transition");
            for (child_index, child_state) in transition.children().iter().enumerate() {
                let mut child_path = path.clone();
                child_path.push(child_index);
                assignments.push((child_path, child_state.clone()));
            }
        }
        assignments.sort();
        Ok(Some(AcceptingRun { assignments }))
    }
}

fn malformed(message: String) -> RewriteError {
    RewriteError::MalformedAutomaton { message }
}
