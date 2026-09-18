// SPDX-License-Identifier: MIT OR Apache-2.0

//! Myhill–Nerode minimization for deterministic tree automata.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::language::automaton::TreeAutomaton;
use crate::language::{TreeState, TreeTransition};
use crate::trs::Symbol;

impl TreeAutomaton {
    /// The unique minimal deterministic automaton for this
    /// language, canonically renumbered: trim unreachable and
    /// non-co-reachable states, complete with a sink, refine the
    /// state partition by context equivalence (Myhill–Nerode),
    /// quotient, and trim again. Language-exact, idempotent, and
    /// byte-identical for language-equivalent inputs.
    ///
    /// Requires a deterministic automaton; minimization of a
    /// nondeterministic automaton is a typed error (determinize
    /// first).
    pub fn minimized(&self) -> RewriteResult<TreeAutomaton> {
        if !self.is_deterministic() {
            return Err(RewriteError::MalformedAutomaton {
                message: "minimization requires a deterministic automaton".into(),
            });
        }
        let trimmed = self.trimmed()?;
        if trimmed.states().is_empty() {
            // The canonical minimal automaton for the empty language.
            return Ok(trimmed);
        }
        let complete = trimmed.completed()?;
        let n = complete.states().len();

        // Deterministic + complete: the transition function is a
        // total map (symbol, children) -> parent.
        let mut delta: BTreeMap<(Symbol, Vec<usize>), usize> = BTreeMap::new();
        let state_index: BTreeMap<&TreeState, usize> = complete
            .states()
            .iter()
            .enumerate()
            .map(|(index, state)| (state, index))
            .collect();
        for transition in complete.transitions() {
            let children: Vec<usize> = transition
                .children()
                .iter()
                .map(|child| state_index[child])
                .collect();
            delta.insert(
                (transition.symbol().clone(), children),
                state_index[transition.parent()],
            );
        }

        // Initial partition: final vs non-final.
        let finality: Vec<bool> = complete
            .states()
            .iter()
            .map(|state| complete.finals().contains(state))
            .collect();
        let mut block: Vec<usize> = finality.iter().map(|final_| usize::from(*final_)).collect();

        // Refine: states split when their transition signatures —
        // for every transition where the state appears as a child,
        // (symbol, position, other children's blocks, parent's
        // block) — differ within a block.
        loop {
            let mut signatures: BTreeMap<usize, Vec<SignatureKey>> = BTreeMap::new();
            for transition in complete.transitions() {
                let children: Vec<usize> = transition
                    .children()
                    .iter()
                    .map(|child| state_index[child])
                    .collect();
                let parent_block = block[state_index[transition.parent()]];
                for (position, child) in children.iter().enumerate() {
                    let others: Vec<usize> = children
                        .iter()
                        .enumerate()
                        .filter(|(other, _)| *other != position)
                        .map(|(_, other)| block[*other])
                        .collect();
                    signatures.entry(*child).or_default().push(SignatureKey {
                        symbol: transition.symbol().clone(),
                        position,
                        others,
                        parent_block,
                    });
                }
            }
            for keys in signatures.values_mut() {
                keys.sort();
            }
            // Group by (old block, signature): sort and assign new
            // block ids to maximal equal runs.
            let mut keyed: Vec<(usize, Vec<SignatureKey>, usize)> = (0..n)
                .map(|state| {
                    (
                        block[state],
                        signatures.get(&state).cloned().unwrap_or_default(),
                        state,
                    )
                })
                .collect();
            keyed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            let mut fresh: Vec<usize> = alloc::vec![0; n];
            let mut fresh_count = 0;
            for (index, entry) in keyed.iter().enumerate() {
                if index > 0 && (entry.0, &entry.1) != (keyed[index - 1].0, &keyed[index - 1].1) {
                    fresh_count += 1;
                }
                fresh[entry.2] = fresh_count;
            }
            if fresh == block {
                break;
            }
            block = fresh;
        }

        // Quotient: canonical block names m0.. ordered by the
        // smallest member state name (deterministic renaming).
        let block_count = block.iter().copied().max().map_or(0, |b| b + 1);
        let mut representative: Vec<Option<usize>> = alloc::vec![None; block_count];
        for (state, block_id) in block.iter().enumerate() {
            let better = representative[*block_id].is_none_or(|current| {
                complete.states()[state].name() < complete.states()[current].name()
            });
            if better {
                representative[*block_id] = Some(state);
            }
        }
        let mut name_order: Vec<usize> = (0..block_count).collect();
        name_order.sort_by_key(|block_id| {
            complete.states()[representative[*block_id].expect("nonempty block")].name()
        });
        let mut canonical: Vec<String> = alloc::vec![String::new(); block_count];
        for (rank, block_id) in name_order.into_iter().enumerate() {
            canonical[block_id] = format!("m{rank}");
        }
        let block_state: Vec<TreeState> = (0..block_count)
            .map(|block_id| TreeState::new(Symbol::new(canonical[block_id].clone())))
            .collect();

        let mut transitions: Vec<TreeTransition> = Vec::new();
        let mut seen: BTreeMap<(Symbol, Vec<usize>), usize> = BTreeMap::new();
        for ((symbol, children), parent) in &delta {
            let child_blocks: Vec<usize> = children.iter().map(|child| block[*child]).collect();
            let parent_block = block[*parent];
            if seen
                .insert((symbol.clone(), child_blocks.clone()), parent_block)
                .is_none()
            {
                transitions.push(TreeTransition::new(
                    symbol.clone(),
                    child_blocks
                        .into_iter()
                        .map(|child| block_state[child].clone())
                        .collect(),
                    block_state[parent_block].clone(),
                ));
            }
        }
        let finals: Vec<TreeState> = (0..block_count)
            .filter(|block_id| {
                let member = representative[*block_id].expect("nonempty block");
                complete.finals().contains(&complete.states()[member])
            })
            .map(|block_id| block_state[block_id].clone())
            .collect();
        let quotient = TreeAutomaton::new(
            complete.alphabet().to_vec(),
            block_state,
            transitions,
            finals,
            *self.limits(),
        )?;
        quotient.trimmed()
    }
}

/// One entry in a state's refinement signature: how this state
/// behaves as the `position`-th child under `symbol`, given the
/// current blocks of the other children and of the parent.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SignatureKey {
    symbol: Symbol,
    position: usize,
    others: Vec<usize>,
    parent_block: usize,
}
