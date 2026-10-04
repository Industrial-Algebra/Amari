// SPDX-License-Identifier: MIT OR Apache-2.0

//! Myhill–Nerode minimization for deterministic tree automata.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::language::automaton::TreeAutomaton;
use crate::language::{TreeState, TreeTransition};
use crate::relation::{RelationLimits, RelationResources};
use crate::trs::Symbol;

impl TreeAutomaton {
    /// The unique minimal deterministic automaton for this
    /// language, canonically renumbered: trim unreachable and
    /// non-co-reachable states, complete with a sink, refine the
    /// state partition by context equivalence (Myhill–Nerode),
    /// quotient, number blocks by a structural bottom-up traversal
    /// (independent of input state names), and trim again.
    /// Language-exact, idempotent, and byte-identical for
    /// language-equivalent inputs.
    ///
    /// Requires a deterministic automaton; minimization of a
    /// nondeterministic automaton is a typed error (determinize
    /// first).
    pub fn minimized(&self) -> RewriteResult<TreeAutomaton> {
        let mut resources = RelationResources::new(&RelationLimits::default());
        self.minimized_with_resources(&mut resources)
    }

    /// [`Self::minimized`] under a caller-supplied operation budget,
    /// charged across the whole pipeline: the trim fixpoints (one
    /// operation per transition examined per round), completion (one
    /// per state tuple per symbol), refinement (one per child
    /// position per round), and canonical numbering (one per ready
    /// transition).
    pub fn minimized_with_resources(
        &self,
        resources: &mut RelationResources,
    ) -> RewriteResult<TreeAutomaton> {
        if !self.is_deterministic() {
            return Err(RewriteError::MalformedAutomaton {
                message: "minimization requires a deterministic automaton".into(),
            });
        }
        let trimmed = self.trimmed_with_resources(resources)?;
        if trimmed.states().is_empty() {
            // The canonical minimal automaton for the empty language.
            return Ok(trimmed);
        }
        let complete = trimmed.completed_with_resources(resources)?;
        let n = complete.states().len();
        let state_index: BTreeMap<&TreeState, usize> = complete
            .states()
            .iter()
            .enumerate()
            .map(|(index, state)| (state, index))
            .collect();
        // Indexed transitions: (symbol, children, parent).
        let indexed: Vec<(Symbol, Vec<usize>, usize)> = complete
            .transitions()
            .iter()
            .map(|transition| {
                (
                    transition.symbol().clone(),
                    transition
                        .children()
                        .iter()
                        .map(|child| state_index[child])
                        .collect(),
                    state_index[transition.parent()],
                )
            })
            .collect();

        // Initial partition: final vs non-final.
        let finality: Vec<bool> = complete
            .states()
            .iter()
            .map(|state| complete.finals().contains(state))
            .collect();
        let mut block: Vec<usize> = finality.iter().map(|final_| usize::from(*final_)).collect();

        // Refine by concrete one-hole contexts: a state's signature
        // records, for every transition where it appears as a child,
        // the symbol, the child position, the CONCRETE sibling
        // states, and the parent's current block. Because the
        // automaton is complete and deterministic, every state has
        // an entry for every (symbol, position, sibling tuple)
        // context, so equal signatures within a block exactly mean
        // equal parent blocks under every concrete context; a stable
        // partition is therefore a congruence. (Using sibling BLOCKS
        // here is unsound: the multiset would lose which concrete
        // sibling produced which parent block.)
        loop {
            let mut signatures: Vec<Vec<SignatureKey>> = (0..n).map(|_| Vec::new()).collect();
            for (symbol, children, parent) in &indexed {
                let parent_block = block[*parent];
                for (position, child) in children.iter().enumerate() {
                    resources.record_operations(1)?;
                    let siblings: Vec<usize> = children
                        .iter()
                        .enumerate()
                        .filter(|(other, _)| *other != position)
                        .map(|(_, other)| *other)
                        .collect();
                    signatures[*child].push(SignatureKey {
                        symbol: symbol.clone(),
                        position,
                        siblings,
                        parent_block,
                    });
                }
            }
            for keys in &mut signatures {
                keys.sort();
            }
            // Group by (old block, signature): sort and assign new
            // block ids to maximal equal runs.
            let mut keyed: Vec<(usize, Vec<SignatureKey>, usize)> = (0..n)
                .map(|state| (block[state], signatures[state].clone(), state))
                .collect();
            keyed.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            let mut fresh: Vec<usize> = vec![0; n];
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

        // Quotient, validating congruence: repeated
        // (symbol, child blocks) keys must agree on the parent
        // block. Disagreement is impossible for a stable partition
        // and is surfaced as a typed error rather than silently
        // keeping the first transition.
        let block_count = block.iter().copied().max().map_or(0, |b| b + 1);
        let mut quotient: Vec<(Symbol, Vec<usize>, usize)> = Vec::new();
        let mut seen: BTreeMap<(Symbol, Vec<usize>), usize> = BTreeMap::new();
        for (symbol, children, parent) in &indexed {
            let child_blocks: Vec<usize> = children.iter().map(|child| block[*child]).collect();
            let parent_block = block[*parent];
            match seen.insert((symbol.clone(), child_blocks.clone()), parent_block) {
                None => quotient.push((symbol.clone(), child_blocks, parent_block)),
                Some(existing) if existing == parent_block => {}
                Some(_) => {
                    return Err(RewriteError::MalformedAutomaton {
                        message: "minimization partition is not a congruence".into(),
                    });
                }
            }
        }

        // Canonical structural numbering: bottom-up traversal from
        // nullary transitions, always expanding the smallest
        // (symbol, child numbers) ready key. Deterministic quotient
        // ⇒ (symbol, child blocks) determines the parent block, so
        // ties never consult internal block ids, and the numbering
        // is a function of the transition structure alone.
        let mut child_uses: Vec<Vec<usize>> = (0..block_count).map(|_| Vec::new()).collect();
        for (transition_index, (_, children, _)) in quotient.iter().enumerate() {
            for child in children {
                child_uses[*child].push(transition_index);
            }
        }
        let mut number: Vec<Option<usize>> = vec![None; block_count];
        let mut ready: BTreeSet<(Symbol, Vec<usize>, usize)> = BTreeSet::new();
        for (symbol, children, parent) in &quotient {
            if children.is_empty() {
                ready.insert((symbol.clone(), Vec::new(), *parent));
            }
        }
        let mut next = 0usize;
        while let Some((symbol, child_numbers, parent)) = ready.iter().next().cloned() {
            ready.remove(&(symbol, child_numbers, parent));
            resources.record_operations(1)?;
            if number[parent].is_some() {
                continue;
            }
            number[parent] = Some(next);
            next += 1;
            for transition_index in &child_uses[parent] {
                let (symbol, children, parent_of) = &quotient[*transition_index];
                if children.iter().all(|child| number[*child].is_some()) {
                    ready.insert((
                        symbol.clone(),
                        children
                            .iter()
                            .map(|child| number[*child].expect("checked"))
                            .collect(),
                        *parent_of,
                    ));
                }
            }
        }
        // Every quotient state is bottom-up reachable (the input was
        // trimmed and the sink is transition-produced), so every
        // block is numbered.
        let names: Vec<String> = (0..block_count)
            .map(|block_id| {
                format!(
                    "m{}",
                    number[block_id].expect("reachable blocks are numbered")
                )
            })
            .collect();
        let block_state: Vec<TreeState> = names
            .into_iter()
            .map(|name| TreeState::new(Symbol::new(name)))
            .collect();
        let transitions: Vec<TreeTransition> = quotient
            .into_iter()
            .map(|(symbol, children, parent)| {
                TreeTransition::new(
                    symbol,
                    children
                        .into_iter()
                        .map(|child| block_state[child].clone())
                        .collect(),
                    block_state[parent].clone(),
                )
            })
            .collect();
        let finals: Vec<TreeState> = (0..block_count)
            .filter(|block_id| {
                let member = (0..n)
                    .find(|state| block[*state] == *block_id)
                    .expect("nonempty block");
                complete.finals().contains(&complete.states()[member])
            })
            .map(|block_id| block_state[block_id].clone())
            .collect();
        let quotient_automaton = TreeAutomaton::new(
            complete.alphabet().to_vec(),
            block_state,
            transitions,
            finals,
            *self.limits(),
        )?;
        // Final trim: drops the sink. Survivors keep their
        // structural numbers, so re-minimizing reproduces the same
        // names — idempotence does not depend on lexical ordering.
        quotient_automaton.trimmed_with_resources(resources)
    }
}

/// One entry in a state's refinement signature: the parent block
/// produced when this state is the `position`-th child under
/// `symbol` with the given concrete sibling states.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SignatureKey {
    symbol: Symbol,
    position: usize,
    siblings: Vec<usize>,
    parent_block: usize,
}
