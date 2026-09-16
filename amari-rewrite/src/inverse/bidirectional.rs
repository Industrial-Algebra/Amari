// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded bidirectional explorer.
//!
//! Expands forward from a source term and backward from a goal term
//! with shared resource accounting. Frontier entries meet when their
//! terms UNIFY (raw equality is not required) and the combined
//! constraints normalize satisfiable — including contradictions that
//! only appear after the meeting substitution is applied. A meet is
//! never accepted until the full source-to-meet-to-goal path replays
//! through the original [`TermSystem`].

use alloc::collections::{BTreeMap, VecDeque};
use alloc::string::ToString;
use alloc::vec::Vec;

use crate::analysis::unify;
use crate::error::{RewriteError, RewriteResult};
use crate::inverse::outcome::{CertifiedExhaustion, ExhaustionAuthority, UnsupportedRelation};
use crate::inverse::state::{SearchResources, SymbolicState};
use crate::inverse::{symbolic_predecessors, SymbolicPredecessor};
use crate::relation::{
    ConstraintOutcome, ConstraintSet, RelationLimits, RelationResources, RuleId, Sha256Digest,
};
use crate::rewritable::Path;
use crate::trs::{match_pattern, Substitution, Term, TermSystem};

/// One forward transition with provenance.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct ForwardStep {
    /// The applied rule's stable identity.
    pub rule_id: RuleId,
    /// Position of the rewritten subterm.
    pub position: Path,
    /// Canonical digest of the source term.
    pub source_hash: Sha256Digest,
    /// Canonical digest of the result term.
    pub result_hash: Sha256Digest,
}

/// A certified meeting of the two frontiers.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct BidirectionalDerivation {
    /// Forward steps from the source to the meeting point.
    pub forward_steps: Vec<ForwardStep>,
    /// Backward steps from the goal to the meeting point (ordered
    /// goal-to-meeting).
    pub backward_steps: Vec<SymbolicPredecessor>,
    /// The meeting term on the forward side.
    pub meeting_forward: Term,
    /// The meeting term on the backward side.
    pub meeting_backward: Term,
    /// The unifier witnessing the meeting.
    pub unifier: Substitution,
}

/// Retained unexpanded states when a budget cuts the search.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct BidirectionalFrontier {
    /// Retained unexpanded forward states.
    pub forward: Vec<SymbolicState>,
    /// Retained unexpanded backward states.
    pub backward: Vec<SymbolicState>,
    /// Deepest depth reached on either side.
    pub depth_reached: u64,
}

/// Typed outcome of a bounded bidirectional search.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum BidirectionalSearchOutcome {
    /// The frontiers met and the full path replayed.
    Witness(BidirectionalDerivation),
    /// Both frontiers closed with nothing dropped and no meeting.
    Exhausted(CertifiedExhaustion),
    /// A budget cut the search; both retained frontiers returned.
    Partial(BidirectionalFrontier),
    /// The relation is unsupported; the reason is carried.
    Unsupported(UnsupportedRelation),
}

struct ForwardNode {
    state: SymbolicState,
    depth: u64,
    parent: Option<([u8; 32], ForwardStep)>,
    expanded: bool,
}

struct BackwardNode {
    state: SymbolicState,
    depth: u64,
    parent: Option<([u8; 32], SymbolicPredecessor)>,
    expanded: bool,
}

/// A meeting candidate that survived combined-constraint
/// normalization.
pub(crate) struct MeetCandidate {
    unifier: Substitution,
}

/// The result of checking one forward/backward state pair.
pub(crate) enum MeetCheck {
    /// Terms unify and the combined constraints normalize
    /// satisfiable under the meeting substitution.
    Met(MeetCandidate),
    /// The pair genuinely cannot meet.
    NotMet,
    /// The check itself exhausted the relation limits, so no
    /// verdict exists. This must never be read as `NotMet`: a
    /// limit event forfeits any exhaustion certificate.
    Undecided,
}

/// Meet check: terms unify AND the combined constraints (including
/// the meeting substitution's instantiations) normalize satisfiable.
/// A `RelationLimitExceeded` from inside the check is NOT a failed
/// meet: it yields `Undecided`, which callers must propagate into
/// truncation (Partial), never into exhaustion.
pub(crate) fn check_meet(
    forward: &SymbolicState,
    backward: &SymbolicState,
    limits: &RelationLimits,
) -> MeetCheck {
    let mut resources = RelationResources::new(limits);
    let unifier = match unify(forward.term(), backward.term(), &mut resources) {
        Ok(unifier) => unifier,
        Err(crate::RewriteError::RelationLimitExceeded { .. }) => return MeetCheck::Undecided,
        Err(_) => return MeetCheck::NotMet,
    };
    // Apply the meeting substitution to the combined constraints
    // BEFORE normalizing: a contradiction that only appears after
    // instantiation (X != c with X := c) must reject the meet.
    let mut merged = ConstraintSet::new();
    let apply = |constraint: &crate::relation::TermConstraint| match constraint {
        crate::relation::TermConstraint::Equal(left, right) => {
            crate::relation::TermConstraint::Equal(unifier.apply(left), unifier.apply(right))
        }
        crate::relation::TermConstraint::NotEqual(left, right) => {
            crate::relation::TermConstraint::NotEqual(unifier.apply(left), unifier.apply(right))
        }
    };
    for constraint in forward.constraints().constraints() {
        merged.insert(apply(constraint));
    }
    for constraint in backward.constraints().constraints() {
        merged.insert(apply(constraint));
    }
    match merged.normalize(limits) {
        Ok(ConstraintOutcome::Satisfiable { .. }) => MeetCheck::Met(MeetCandidate { unifier }),
        Err(crate::RewriteError::RelationLimitExceeded { .. }) => MeetCheck::Undecided,
        _ => MeetCheck::NotMet,
    }
}

/// Bounded bidirectional explorer over a term system.
#[derive(Clone, Debug)]
pub struct BidirectionalExplorer<'a> {
    system: &'a TermSystem,
    config: crate::inverse::InverseSearchConfig,
}

impl<'a> BidirectionalExplorer<'a> {
    /// Bind an explorer to a system and a validated configuration.
    pub fn new(system: &'a TermSystem, config: crate::inverse::InverseSearchConfig) -> Self {
        Self { system, config }
    }

    /// Search for a certified meeting between `source` and `goal`.
    pub fn search(&self, source: &Term, goal: &Term) -> RewriteResult<BidirectionalSearchOutcome> {
        // Re-validate the whole configuration (see backward.rs).
        self.config.validate()?;
        let limits = RelationLimits::new(
            self.config.max_term_nodes() as usize,
            self.config.max_term_depth() as usize,
            self.config.max_constraints() as usize,
            self.config.max_operations() as usize,
        )?;
        let mut resources = SearchResources::new(&self.config);
        let mut forward: BTreeMap<[u8; 32], ForwardNode> = BTreeMap::new();
        let mut backward: BTreeMap<[u8; 32], BackwardNode> = BTreeMap::new();
        let mut forward_pending: VecDeque<[u8; 32]> = VecDeque::new();
        let mut backward_pending: VecDeque<[u8; 32]> = VecDeque::new();
        let mut truncated = false;
        let mut scope: u64 = 0;

        let source_state = SymbolicState::new(source.clone(), ConstraintSet::new());
        let goal_state = SymbolicState::new(goal.clone(), ConstraintSet::new());
        match check_meet(&source_state, &goal_state, &limits) {
            MeetCheck::Met(meet) => {
                return self.replay_witness(
                    BidirectionalDerivation {
                        forward_steps: Vec::new(),
                        backward_steps: Vec::new(),
                        meeting_forward: source.clone(),
                        meeting_backward: goal.clone(),
                        unifier: meet.unifier,
                    },
                    source,
                    goal,
                );
            }
            MeetCheck::NotMet => {}
            // The initial pair cannot even be checked inside the
            // limits: truncated before the search begins, so the
            // only honest outcome is Partial.
            MeetCheck::Undecided => truncated = true,
        }
        if resources.record_state().is_err()
            || resources
                .record_bytes(source_state.retained_bytes())
                .is_err()
            || resources.record_state().is_err()
            || resources.record_bytes(goal_state.retained_bytes()).is_err()
        {
            return Ok(BidirectionalSearchOutcome::Partial(BidirectionalFrontier {
                forward: Vec::new(),
                backward: Vec::new(),
                depth_reached: 0,
            }));
        }
        let source_digest = *source_state.canonical_digest().as_bytes();
        let goal_digest = *goal_state.canonical_digest().as_bytes();
        forward_pending.push_back(source_digest);
        backward_pending.push_back(goal_digest);
        forward.insert(
            source_digest,
            ForwardNode {
                state: source_state,
                depth: 0,
                parent: None,
                expanded: false,
            },
        );
        backward.insert(
            goal_digest,
            BackwardNode {
                state: goal_state,
                depth: 0,
                parent: None,
                expanded: false,
            },
        );

        loop {
            if truncated || (forward_pending.is_empty() && backward_pending.is_empty()) {
                break;
            }
            // Expand one forward state, then one backward state
            // (deterministic interleaving).
            if let Some(digest) = forward_pending.pop_front() {
                let (term, depth) = {
                    let node = forward.get(&digest).expect("pending exists");
                    (node.state.term().clone(), node.depth)
                };
                if depth >= self.config.max_depth() {
                    truncated = true;
                    continue;
                }
                if resources.record_operations(1).is_err() {
                    truncated = true;
                    break;
                }
                if let Some(node) = forward.get_mut(&digest) {
                    node.expanded = true;
                }
                for (step, result) in forward_successors(self.system, &term) {
                    if resources.record_transition().is_err() {
                        truncated = true;
                        break;
                    }
                    if term_cost(&result) > self.config.max_term_nodes()
                        || term_depth(&result) > self.config.max_term_depth()
                    {
                        truncated = true;
                        continue;
                    }
                    let child_depth = depth + 1;
                    if child_depth > self.config.max_depth() {
                        truncated = true;
                        continue;
                    }
                    let child = SymbolicState::new(result, ConstraintSet::new());
                    let child_digest = *child.canonical_digest().as_bytes();
                    if forward.contains_key(&child_digest) {
                        continue;
                    }
                    // Meet check against every retained backward
                    // state, in digest order (deterministic).
                    let mut met = None;
                    for (other_digest, other) in &backward {
                        let meet = match check_meet(&child, &other.state, &limits) {
                            MeetCheck::Met(meet) => meet,
                            MeetCheck::NotMet => continue,
                            MeetCheck::Undecided => {
                                truncated = true;
                                continue;
                            }
                        };
                        let mut forward_steps = forward_chain(&forward, &digest);
                        forward_steps.push(step.clone());
                        let derivation = BidirectionalDerivation {
                            forward_steps,
                            backward_steps: backward_chain(&backward, other_digest),
                            meeting_forward: child.term().clone(),
                            meeting_backward: other.state.term().clone(),
                            unifier: meet.unifier,
                        };
                        met = Some(self.replay_witness(derivation, source, goal)?);
                        break;
                    }
                    if let Some(outcome) = met {
                        return Ok(outcome);
                    }
                    if resources.record_state().is_err()
                        || resources.record_bytes(child.retained_bytes()).is_err()
                    {
                        truncated = true;
                        break;
                    }
                    let enqueue = child_depth < self.config.max_depth();
                    if !enqueue {
                        truncated = true;
                    }
                    forward.insert(
                        child_digest,
                        ForwardNode {
                            state: child,
                            depth: child_depth,
                            parent: Some((digest, step)),
                            expanded: false,
                        },
                    );
                    if enqueue {
                        forward_pending.push_back(child_digest);
                    }
                }
            }
            if truncated {
                break;
            }
            if let Some(digest) = backward_pending.pop_front() {
                let (term, constraints, depth) = {
                    let node = backward.get(&digest).expect("pending exists");
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
                        // Defensive: unreachable today (see the
                        // note in backward.rs); a clash at
                        // expansion time is unsupported.
                        Err(RewriteError::UnificationFailure { reason }) => {
                            return Ok(BidirectionalSearchOutcome::Unsupported(
                                UnsupportedRelation { reason },
                            ));
                        }
                        Err(_) => {
                            truncated = true;
                            break;
                        }
                    };
                if let Some(node) = backward.get_mut(&digest) {
                    node.expanded = true;
                }
                for predecessor in predecessors {
                    if resources.record_transition().is_err() {
                        truncated = true;
                        break;
                    }
                    let Some(child) = child_state(
                        &self.config,
                        &constraints,
                        &predecessor,
                        &limits,
                        &mut truncated,
                    ) else {
                        continue;
                    };
                    let child_depth = depth + 1;
                    if child_depth > self.config.max_depth() {
                        truncated = true;
                        continue;
                    }
                    let child_digest = *child.canonical_digest().as_bytes();
                    if backward.contains_key(&child_digest) {
                        continue;
                    }
                    // Meet check against every retained forward state.
                    let mut met = None;
                    for (other_digest, other) in &forward {
                        let meet = match check_meet(&other.state, &child, &limits) {
                            MeetCheck::Met(meet) => meet,
                            MeetCheck::NotMet => continue,
                            MeetCheck::Undecided => {
                                truncated = true;
                                continue;
                            }
                        };
                        let mut backward_steps = backward_chain(&backward, &digest);
                        backward_steps.push(predecessor.clone());
                        let derivation = BidirectionalDerivation {
                            forward_steps: forward_chain(&forward, other_digest),
                            backward_steps,
                            meeting_forward: other.state.term().clone(),
                            meeting_backward: child.term().clone(),
                            unifier: meet.unifier,
                        };
                        met = Some(self.replay_witness(derivation, source, goal)?);
                        break;
                    }
                    if let Some(outcome) = met {
                        return Ok(outcome);
                    }
                    if resources.record_state().is_err()
                        || resources.record_bytes(child.retained_bytes()).is_err()
                    {
                        truncated = true;
                        break;
                    }
                    let enqueue = child_depth < self.config.max_depth();
                    if !enqueue {
                        truncated = true;
                    }
                    backward.insert(
                        child_digest,
                        BackwardNode {
                            state: child,
                            depth: child_depth,
                            parent: Some((digest, predecessor)),
                            expanded: false,
                        },
                    );
                    if enqueue {
                        backward_pending.push_back(child_digest);
                    }
                }
            }
        }

        if truncated {
            let forward_states = forward
                .values()
                .filter(|node| !node.expanded)
                .map(|node| node.state.clone())
                .collect();
            let backward_states = backward
                .values()
                .filter(|node| !node.expanded)
                .map(|node| node.state.clone())
                .collect();
            let depth_reached = forward
                .values()
                .map(|node| node.depth)
                .chain(backward.values().map(|node| node.depth))
                .max()
                .unwrap_or(0);
            return Ok(BidirectionalSearchOutcome::Partial(BidirectionalFrontier {
                forward: forward_states,
                backward: backward_states,
                depth_reached,
            }));
        }

        let mut evidence = Vec::new();
        for digest in forward.keys().chain(backward.keys()) {
            evidence.extend_from_slice(digest);
        }
        Ok(BidirectionalSearchOutcome::Exhausted(
            CertifiedExhaustion::certify(ExhaustionAuthority::ClosedSymbolicSearch, &evidence),
        ))
    }

    /// Replay the full source-to-meeting-to-goal path through the
    /// original system. Divergence is a hard internal error, never a
    /// silently accepted meet.
    fn replay_witness(
        &self,
        derivation: BidirectionalDerivation,
        source: &Term,
        goal: &Term,
    ) -> RewriteResult<BidirectionalSearchOutcome> {
        let mismatch = |message: &str| RewriteError::ResidualMismatch {
            message: message.to_string(),
        };
        let canonical = |term: &Term| Sha256Digest::canonical_term("amari.relation.term/v1", term);
        // Forward half: source -> meeting_forward.
        let mut current = source.clone();
        for step in &derivation.forward_steps {
            let rule = self
                .system
                .rules()
                .iter()
                .find(|rule| RuleId::from_rule(rule) == step.rule_id)
                .ok_or_else(|| mismatch("forward step references unknown rule"))?;
            let position = current
                .subterm(&step.position)
                .ok_or_else(|| mismatch("replay lost the forward path"))?;
            let bindings = match_pattern(rule.lhs(), position)
                .ok_or_else(|| mismatch("replay failed the forward lhs"))?;
            current = current
                .replace_at(&step.position, bindings.apply(rule.rhs()))
                .map_err(|_| mismatch("replay failed a forward rebuild"))?;
        }
        if canonical(&current) != canonical(&derivation.meeting_forward) {
            return Err(mismatch("forward half does not reach the meeting term"));
        }
        // Backward half, applied forward: instantiated meeting term
        // -> goal.
        let mut current = derivation.unifier.apply(&derivation.meeting_backward);
        for step in derivation.backward_steps.iter().rev() {
            let rule = self
                .system
                .rules()
                .iter()
                .find(|rule| RuleId::from_rule(rule) == step.provenance.rule_id)
                .ok_or_else(|| mismatch("backward step references unknown rule"))?;
            let position = current
                .subterm(&step.provenance.position)
                .ok_or_else(|| mismatch("replay lost the backward path"))?;
            let bindings = match_pattern(rule.lhs(), position)
                .ok_or_else(|| mismatch("replay failed the backward lhs"))?;
            current = current
                .replace_at(&step.provenance.position, bindings.apply(rule.rhs()))
                .map_err(|_| mismatch("replay failed a backward rebuild"))?;
        }
        // The backward half starts from the meeting substitution's
        // instantiation of the meeting term, so a goal whose
        // variables were bound by the meet reconstructs to an
        // INSTANCE of the goal, not the raw goal. Acceptance is
        // therefore pattern matching with the goal as the pattern:
        // the reconstructed endpoint must be an instance of the
        // declared goal. (Strict canonical equality wrongly rejects
        // every non-ground goal.)
        if match_pattern(goal, &current).is_none() {
            return Err(mismatch(
                "bidirectional replay does not reconstruct an instance of the goal",
            ));
        }
        Ok(BidirectionalSearchOutcome::Witness(derivation))
    }
}

/// Forward steps from the root to the node at `digest`.
fn forward_chain(forward: &BTreeMap<[u8; 32], ForwardNode>, digest: &[u8; 32]) -> Vec<ForwardStep> {
    let mut steps = Vec::new();
    let mut cursor = *digest;
    while let Some(node) = forward.get(&cursor) {
        match &node.parent {
            Some((parent, step)) => {
                steps.push(step.clone());
                cursor = *parent;
            }
            None => break,
        }
    }
    steps.reverse();
    steps
}

/// Backward steps from the goal root to the node at `digest`,
/// ordered goal-to-meeting.
fn backward_chain(
    backward: &BTreeMap<[u8; 32], BackwardNode>,
    digest: &[u8; 32],
) -> Vec<SymbolicPredecessor> {
    let mut steps = Vec::new();
    let mut cursor = *digest;
    while let Some(node) = backward.get(&cursor) {
        match &node.parent {
            Some((parent, step)) => {
                steps.push(step.clone());
                cursor = *parent;
            }
            None => break,
        }
    }
    steps.reverse();
    steps
}

/// All one-rule forward successors of a term, in canonical rule
/// order then position order (fully deterministic).
fn forward_successors(system: &TermSystem, term: &Term) -> Vec<(ForwardStep, Term)> {
    let mut successors = Vec::new();
    for rule in system.rules() {
        for position in term.positions() {
            let Some(subterm) = term.subterm(&position) else {
                continue;
            };
            let Some(bindings) = match_pattern(rule.lhs(), subterm) else {
                continue;
            };
            let Ok(result) = term.replace_at(&position, bindings.apply(rule.rhs())) else {
                continue;
            };
            let step = ForwardStep {
                rule_id: RuleId::from_rule(rule),
                position,
                source_hash: Sha256Digest::canonical_term("amari.relation.term/v1", term),
                result_hash: Sha256Digest::canonical_term("amari.relation.term/v1", &result),
            };
            successors.push((step, result));
        }
    }
    successors
}

/// Backward child construction mirroring the BackwardExplorer:
/// merge + normalize constraints, apply the solving substitution,
/// enforce term-size caps (drops mark truncation).
fn child_state(
    config: &crate::inverse::InverseSearchConfig,
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
        Ok(ConstraintOutcome::UnsupportedTheory) | Err(_) => {
            *truncated = true;
            return None;
        }
    };
    let term = substitution.apply(&predecessor.term);
    if term_cost(&term) > config.max_term_nodes() || term_depth(&term) > config.max_term_depth() {
        *truncated = true;
        return None;
    }
    Some(SymbolicState::new(term, constraints))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::TermConstraint;
    use alloc::vec;

    #[test]
    fn adversarial_contradiction_only_after_meeting_is_rejected() {
        // Forward state g(X) carries X != c; backward state g(c)
        // unifies with X := c, which makes the combined constraints
        // contradictory ONLY after the meeting substitution. The
        // meet must be rejected.
        let mut constraints = ConstraintSet::new();
        constraints.insert(TermConstraint::NotEqual(
            Term::var("X"),
            Term::constant("c"),
        ));
        let forward = SymbolicState::new(Term::sym("g", vec![Term::var("X")]), constraints);
        let backward = SymbolicState::new(
            Term::sym("g", vec![Term::constant("c")]),
            ConstraintSet::new(),
        );
        let limits = RelationLimits::default();
        assert!(matches!(
            check_meet(&forward, &backward, &limits),
            MeetCheck::NotMet
        ));
        // Control: without the constraint the same terms meet.
        let plain = SymbolicState::new(Term::sym("g", vec![Term::var("X")]), ConstraintSet::new());
        assert!(matches!(
            check_meet(&plain, &backward, &limits),
            MeetCheck::Met(_)
        ));
    }
}
