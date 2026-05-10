//! Bounded inverse rewriting for term rewriting systems.
//!
//! Inverse rewriting is predecessor generation, not a true functional inverse.
//! A rule `lhs -> rhs` is explored backward by matching `rhs` against a
//! subterm and instantiating `lhs` with the same substitution.

use alloc::collections::{BTreeSet, VecDeque};

use crate::trs::{match_pattern, Term, TermSystem};

/// Convenience helper returning all bounded predecessors as a vector.
pub fn predecessors(system: &TermSystem, target: Term, max_depth: usize) -> alloc::vec::Vec<Term> {
    BackwardSearch::new(system, target)
        .max_depth(max_depth)
        .collect()
}

/// Bounded breadth-first backward search over a `TermSystem`.
pub struct BackwardSearch<'a> {
    system: &'a TermSystem,
    queue: VecDeque<(Term, usize)>,
    visited: BTreeSet<Term>,
    max_depth: usize,
    max_nodes: usize,
    emitted: usize,
}

impl<'a> BackwardSearch<'a> {
    /// Create a search rooted at `target`.
    pub fn new(system: &'a TermSystem, target: Term) -> Self {
        let mut queue = VecDeque::new();
        let mut visited = BTreeSet::new();
        visited.insert(target.clone());
        queue.push_back((target, 0));

        Self {
            system,
            queue,
            visited,
            max_depth: 1,
            max_nodes: 1024,
            emitted: 0,
        }
    }

    /// Set the maximum backward depth.
    pub fn max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Set the maximum number of emitted predecessor nodes.
    pub fn max_nodes(mut self, max_nodes: usize) -> Self {
        self.max_nodes = max_nodes;
        self
    }

    fn one_step_predecessors(&self, term: &Term) -> alloc::vec::Vec<Term> {
        let mut out = alloc::vec::Vec::new();
        for path in term.positions() {
            let Some(subterm) = term.subterm(&path) else {
                continue;
            };

            for rule in self.system.rules() {
                if let Some(subst) = match_pattern(rule.rhs(), subterm) {
                    let predecessor_subterm = subst.apply(rule.lhs());
                    if let Ok(predecessor) = term.replace_at(&path, predecessor_subterm) {
                        if predecessor != *term {
                            out.push(predecessor);
                        }
                    }
                }
            }
        }
        out
    }
}

impl Iterator for BackwardSearch<'_> {
    type Item = Term;

    fn next(&mut self) -> Option<Self::Item> {
        if self.emitted >= self.max_nodes {
            return None;
        }

        while let Some((term, depth)) = self.queue.pop_front() {
            if depth >= self.max_depth {
                continue;
            }

            for predecessor in self.one_step_predecessors(&term) {
                if self.visited.insert(predecessor.clone()) {
                    self.queue.push_back((predecessor.clone(), depth + 1));
                    self.emitted += 1;
                    return Some(predecessor);
                }
            }
        }

        None
    }
}
