// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded backward explorer over alpha-canonical symbolic states.
//!
//! Expansion only follows valid [`crate::relation::BackwardClause`]
//! transitions via [`symbolic_predecessors`]; every retained edge
//! carries rule/path/unifier/constraint provenance. Witnesses are
//! replayed forward through the original [`TermSystem`] before they
//! are returned. Budget cuts are always `Partial`; `Exhausted` is
//! certified only when the frontier closes with nothing dropped.

use alloc::collections::{BTreeMap, BTreeSet, VecDeque};
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::analysis::unify;
use crate::error::{RewriteError, RewriteResult};
use crate::inverse::guidance::{GuidanceMode, SymbolicScore};
use crate::inverse::outcome::{
    ApproximateSearchEvidence, BackwardDerivation, BackwardFrontier, BackwardSearchOutcome,
    CertifiedExhaustion, ExhaustionAuthority, UnsupportedRelation,
};
use crate::inverse::state::{SearchResources, SymbolicState};
use crate::inverse::{symbolic_predecessors, SymbolicPredecessor};
use crate::relation::{
    ConstraintOutcome, ConstraintSet, RelationLimits, RelationResources, RuleId, Sha256Digest,
};
use crate::trs::{match_pattern, Term, TermSystem};

/// The result of checking one state against the goal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GoalCheck {
    /// The terms unify.
    Match,
    /// The terms genuinely clash.
    NoMatch,
    /// The check itself exhausted the relation limits, so no
    /// verdict exists. This must never be read as `NoMatch`: a
    /// limit event forfeits any exhaustion certificate.
    Undecided,
}

/// A state matches the goal when the two terms UNIFY: existential
/// logic variables in the state may be instantiated by the goal, so
/// one-way pattern matching is not the right check. A
/// `RelationLimitExceeded` from inside unification is NOT a clash:
/// it yields `Undecided`, which callers must propagate into
/// truncation (Partial), never into exhaustion.
fn goal_check(limits: &RelationLimits, goal: &Term, term: &Term) -> GoalCheck {
    let mut resources = RelationResources::new(limits);
    match unify(term, goal, &mut resources) {
        Ok(_) => GoalCheck::Match,
        Err(crate::RewriteError::RelationLimitExceeded { .. }) => GoalCheck::Undecided,
        Err(_) => GoalCheck::NoMatch,
    }
}

/// Exact expansion ordering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum SearchMode {
    /// Canonical breadth-first order.
    BreadthFirst,
    /// Deterministic symbolic cost order: fewest term nodes first,
    /// ties broken by canonical digest.
    CostFirst,
}

/// Bounded backward explorer over a term system.
#[derive(Clone, Debug)]
pub struct BackwardExplorer<'a> {
    system: &'a TermSystem,
    config: crate::inverse::InverseSearchConfig,
}

struct SearchNode {
    state: SymbolicState,
    depth: u64,
    parent: Option<([u8; 32], SymbolicPredecessor)>,
    expanded: bool,
}

enum Pending {
    BreadthFirst(VecDeque<[u8; 32]>),
    CostFirst(BTreeSet<(u64, [u8; 32])>),
}

impl Pending {
    fn push(&mut self, cost: u64, digest: [u8; 32]) {
        match self {
            Self::BreadthFirst(queue) => queue.push_back(digest),
            Self::CostFirst(set) => {
                set.insert((cost, digest));
            }
        }
    }

    fn pop(&mut self) -> Option<[u8; 32]> {
        match self {
            Self::BreadthFirst(queue) => queue.pop_front(),
            Self::CostFirst(set) => set.pop_first().map(|(_, digest)| digest),
        }
    }
}

impl<'a> BackwardExplorer<'a> {
    /// Bind an explorer to a system and a validated configuration.
    pub fn new(system: &'a TermSystem, config: crate::inverse::InverseSearchConfig) -> Self {
        Self { system, config }
    }

    /// Search backward from `target` for a state matching the `goal`
    /// pattern. Equivalent to [`Self::search_with_guidance`] with
    /// [`GuidanceMode::CompleteWithinLimits`].
    pub fn search(
        &self,
        target: &Term,
        goal: &Term,
        mode: SearchMode,
    ) -> RewriteResult<BackwardSearchOutcome> {
        self.search_with_guidance(target, goal, mode, &GuidanceMode::CompleteWithinLimits)
    }

    /// Search backward under an explicit guidance mode. Guidance
    /// never creates transitions: it orders (or, in heuristic mode,
    /// explicitly prunes and counts) already valid candidates.
    /// Pruned searches never return `Exhausted`.
    pub fn search_with_guidance(
        &self,
        target: &Term,
        goal: &Term,
        mode: SearchMode,
        guidance: &GuidanceMode,
    ) -> RewriteResult<BackwardSearchOutcome> {
        guidance.validate(&self.config)?;
        // Re-validate the whole configuration, not just the four
        // relation-limit fields: a config built outside `new`
        // (e.g. tampered struct state inside this crate) must not
        // raise the fixed ceilings.
        self.config.validate()?;
        let limits = RelationLimits::new(
            self.config.max_term_nodes() as usize,
            self.config.max_term_depth() as usize,
            self.config.max_constraints() as usize,
            self.config.max_operations() as usize,
        )?;
        let mut resources = SearchResources::new(&self.config);
        let mut nodes: BTreeMap<[u8; 32], SearchNode> = BTreeMap::new();
        let mut pending = match mode {
            SearchMode::BreadthFirst => Pending::BreadthFirst(VecDeque::new()),
            SearchMode::CostFirst => Pending::CostFirst(BTreeSet::new()),
        };
        let mut truncated = false;
        let mut dropped: u64 = 0;
        let mut scope: u64 = 0;

        let root = SymbolicState::new(target.clone(), ConstraintSet::new());
        match goal_check(&limits, goal, root.term()) {
            GoalCheck::Match => {
                return Ok(BackwardSearchOutcome::Witness(BackwardDerivation {
                    steps: Vec::new(),
                }));
            }
            GoalCheck::NoMatch => {}
            // The root cannot even be checked inside the limits:
            // the search is truncated before it begins, so the only
            // honest outcome is Partial.
            GoalCheck::Undecided => truncated = true,
        }
        if resources.record_state().is_err()
            || resources.record_bytes(root.retained_bytes()).is_err()
        {
            return Ok(BackwardSearchOutcome::Partial(BackwardFrontier {
                states: Vec::new(),
                depth_reached: 0,
            }));
        }
        let root_digest = *root.canonical_digest().as_bytes();
        pending.push(term_cost(root.term()), root_digest);
        nodes.insert(
            root_digest,
            SearchNode {
                state: root,
                depth: 0,
                parent: None,
                expanded: false,
            },
        );

        while let Some(digest) = pending.pop() {
            let (term, constraints, depth) = {
                let node = nodes.get(&digest).expect("pending digest exists");
                (
                    node.state.term().clone(),
                    node.state.constraints().clone(),
                    node.depth,
                )
            };
            if depth >= self.config.max_depth() {
                truncated = true;
                continue;
            }
            if resources.record_operations(1).is_err() {
                truncated = true;
                break;
            }
            scope += 1;
            let predecessors =
                match symbolic_predecessors(self.system, &term, &limits, scope as u32) {
                    Ok(predecessors) => predecessors,
                    Err(RewriteError::UnificationFailure { reason }) => {
                        // Defensive: `symbolic_predecessors` skips
                        // clashing positions instead of returning
                        // this variant (inverse/mod.rs), so this
                        // branch is unreachable today. It is kept
                        // because a clash at expansion time IS an
                        // unsupported-search condition should the
                        // expansion contract ever change.
                        return Ok(BackwardSearchOutcome::Unsupported(UnsupportedRelation {
                            reason,
                        }));
                    }
                    Err(_) => {
                        truncated = true;
                        break;
                    }
                };
            if let Some(node) = nodes.get_mut(&digest) {
                node.expanded = true;
            }
            // Buffer the valid candidates for this expansion, then
            // order (or explicitly prune) them via the guidance mode.
            let parent_state = SymbolicState::new(term.clone(), constraints.clone());
            let mut candidates: Vec<(SymbolicState, SymbolicPredecessor)> = Vec::new();
            for predecessor in predecessors {
                if resources.record_transition().is_err() {
                    truncated = true;
                    break;
                }
                let Some(child) =
                    self.child_state(&constraints, &predecessor, &limits, &mut truncated)
                else {
                    continue;
                };
                candidates.push((child, predecessor));
            }
            if truncated {
                break;
            }
            let branch_factor = candidates.len() as u64;
            let rule_priority = |predecessor: &SymbolicPredecessor| {
                self.system
                    .rules()
                    .iter()
                    .position(|rule| RuleId::from_rule(rule) == predecessor.provenance.rule_id)
                    .unwrap_or(usize::MAX) as u64
            };
            let mut scored: Vec<(SymbolicScore, [u8; 32], SymbolicState, SymbolicPredecessor)> =
                candidates
                    .into_iter()
                    .map(|(child, predecessor)| {
                        let child_digest = *child.canonical_digest().as_bytes();
                        let score = crate::inverse::guidance::score_candidate(
                            &parent_state,
                            &child,
                            &predecessor,
                            depth + 1,
                            branch_factor,
                            nodes.contains_key(&child_digest),
                            rule_priority(&predecessor),
                        );
                        (score, child_digest, child, predecessor)
                    })
                    .collect();
            scored.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| left.1.cmp(&right.1)));
            if let GuidanceMode::HeuristicPruning { beam_width } = guidance {
                let keep = (*beam_width).min(usize::MAX as u64) as usize;
                if scored.len() > keep {
                    dropped += (scored.len() - keep) as u64;
                    scored.truncate(keep);
                }
            }
            for (_, child_digest, child, predecessor) in scored {
                let child_depth = depth + 1;
                if child_depth > self.config.max_depth() {
                    truncated = true;
                    continue;
                }
                if nodes.contains_key(&child_digest) {
                    continue;
                }
                let mut undecided = false;
                match goal_check(&limits, goal, child.term()) {
                    GoalCheck::Match => {
                        let steps = build_chain(&nodes, &digest, predecessor);
                        return self.replay_witness(steps, target);
                    }
                    GoalCheck::NoMatch => {}
                    // The check ran out of budget: the child may or
                    // may not be a witness. Retain it as unexplored
                    // and forfeit certification.
                    GoalCheck::Undecided => {
                        truncated = true;
                        undecided = true;
                    }
                }
                if resources.record_state().is_err()
                    || resources.record_bytes(child.retained_bytes()).is_err()
                {
                    truncated = true;
                    break;
                }
                let enqueue = child_depth < self.config.max_depth() && !undecided;
                if !enqueue {
                    truncated = true;
                }
                let cost = term_cost(child.term());
                nodes.insert(
                    child_digest,
                    SearchNode {
                        state: child,
                        depth: child_depth,
                        parent: Some((digest, predecessor)),
                        expanded: false,
                    },
                );
                if enqueue {
                    pending.push(cost, child_digest);
                }
            }
            if truncated {
                break;
            }
        }

        let approximate = |frontier: Option<BackwardFrontier>, dropped: u64, explored: u64| {
            BackwardSearchOutcome::Approximate(ApproximateSearchEvidence {
                summary: alloc::format!(
                    "heuristic pruning dropped {dropped} valid candidates; \
                     nothing about unexplored space is certified"
                ),
                explored_states: explored,
                dropped_candidates: dropped,
                scorer_hash: crate::inverse::guidance::scorer_hash(),
                config_hash: crate::inverse::replay::config_hash(&self.config),
                guidance_hash: crate::inverse::guidance::guidance_hash(guidance),
                frontier,
            })
        };
        if truncated {
            let frontier: Vec<SymbolicState> = nodes
                .values()
                .filter(|node| !node.expanded)
                .map(|node| node.state.clone())
                .collect();
            let depth_reached = nodes.values().map(|node| node.depth).max().unwrap_or(0);
            let frontier = BackwardFrontier {
                states: frontier,
                depth_reached,
            };
            if dropped > 0 {
                return Ok(approximate(Some(frontier), dropped, resources.states()));
            }
            return Ok(BackwardSearchOutcome::Partial(frontier));
        }

        if dropped > 0 {
            return Ok(approximate(None, dropped, resources.states()));
        }
        let mut evidence = Vec::new();
        for digest in nodes.keys() {
            evidence.extend_from_slice(digest);
        }
        Ok(BackwardSearchOutcome::Exhausted(
            CertifiedExhaustion::certify(ExhaustionAuthority::ClosedSymbolicSearch, &evidence),
        ))
    }

    /// Merge parent and step constraints, normalize, and build the
    /// child state. Unsatisfiable candidates are genuinely impossible
    /// and dropped; size-cap drops mark the search truncated.
    fn child_state(
        &self,
        parent_constraints: &ConstraintSet,
        predecessor: &SymbolicPredecessor,
        limits: &RelationLimits,
        truncated: &mut bool,
    ) -> Option<SymbolicState> {
        let mut merged = ConstraintSet::new();
        for constraint in parent_constraints.constraints() {
            merged.insert(constraint.clone());
        }
        for constraint in predecessor.constraints.constraints() {
            merged.insert(constraint.clone());
        }
        let (substitution, constraints) = match merged.normalize(limits) {
            Ok(ConstraintOutcome::Satisfiable {
                substitution,
                residuals,
            }) => {
                let mut set = ConstraintSet::new();
                for residual in residuals {
                    set.insert(residual);
                }
                (substitution, set)
            }
            Ok(ConstraintOutcome::Unsatisfiable) => return None,
            Ok(ConstraintOutcome::UnsupportedTheory) => {
                *truncated = true;
                return None;
            }
            Err(_) => {
                *truncated = true;
                return None;
            }
        };
        let term = substitution.apply(&predecessor.term);
        if term_cost(&term) > self.config.max_term_nodes()
            || term_depth(&term) > self.config.max_term_depth()
        {
            *truncated = true;
            return None;
        }
        Some(SymbolicState::new(term, constraints))
    }

    /// Replay a witness chain forward through the original system.
    /// A mismatch is an internal-consistency hard error, never a
    /// silently returned witness.
    fn replay_witness(
        &self,
        steps: Vec<SymbolicPredecessor>,
        target: &Term,
    ) -> RewriteResult<BackwardSearchOutcome> {
        let mut current = steps
            .last()
            .map(|step| step.term.clone())
            .unwrap_or_else(|| target.clone());
        for step in steps.iter().rev() {
            let rule = self
                .system
                .rules()
                .iter()
                .find(|rule| RuleId::from_rule(rule) == step.provenance.rule_id)
                .ok_or_else(|| RewriteError::ResidualMismatch {
                    message: "witness step references an unknown rule".to_string(),
                })?;
            let position = current.subterm(&step.provenance.position).ok_or_else(|| {
                RewriteError::ResidualMismatch {
                    message: "witness replay lost the recorded path".to_string(),
                }
            })?;
            let bindings = match_pattern(rule.lhs(), position).ok_or_else(|| {
                RewriteError::ResidualMismatch {
                    message: "witness replay failed to match the lhs".to_string(),
                }
            })?;
            current = current
                .replace_at(&step.provenance.position, bindings.apply(rule.rhs()))
                .map_err(|_| RewriteError::ResidualMismatch {
                    message: "witness replay failed to rebuild the term".to_string(),
                })?;
        }
        let replayed = Sha256Digest::canonical_term("amari.relation.term/v1", &current);
        let expected = Sha256Digest::canonical_term("amari.relation.term/v1", target);
        if replayed != expected {
            return Err(RewriteError::ResidualMismatch {
                message: "witness replay does not reconstruct the target".to_string(),
            });
        }
        Ok(BackwardSearchOutcome::Witness(BackwardDerivation { steps }))
    }
}

/// Walk parent edges from the node at `digest` back to the root,
/// returning the chain ordered root-to-leaf with `leaf_edge` last.
fn build_chain(
    nodes: &BTreeMap<[u8; 32], SearchNode>,
    digest: &[u8; 32],
    leaf_edge: SymbolicPredecessor,
) -> Vec<SymbolicPredecessor> {
    let mut steps = alloc::vec![leaf_edge];
    let mut cursor = *digest;
    while let Some(node) = nodes.get(&cursor) {
        match &node.parent {
            Some((parent, edge)) => {
                steps.push(edge.clone());
                cursor = *parent;
            }
            None => break,
        }
    }
    steps.reverse();
    steps
}

fn term_cost(term: &Term) -> u64 {
    term.positions().len() as u64
}

fn term_depth(term: &Term) -> u64 {
    match term {
        Term::Var(_) => 1,
        Term::Sym(_, args) => 1 + args.iter().map(term_depth).max().unwrap_or(0),
    }
}
