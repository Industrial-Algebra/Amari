// SPDX-License-Identifier: MIT OR Apache-2.0

//! Exact regular preimage constructions (ADR 0001 §Construction).
//!
//! This module implements the approved left-linear one-step preimage
//! `R⁻¹(L)` and its finite-horizon iteration, for the classes ADR 0001
//! admits: `Ground`, `LinearVariableDisjoint`, and `LeftLinearShared`.
//! The theorem is the assembled construction of the ADR: for a finite
//! TRS `R` whose left sides are linear and a recognizable language
//! `L`, `R⁻¹(L)` is recognizable — computed here as an explicit
//! epsilon-NFTA and then canonicalized (determinized, completed,
//! minimized). The relation is the application relation
//! ([`TermSystem::application_successors`]), which counts a rule
//! application even when the result equals the source, so the
//! swap-rule self-step is a genuine successor.
//!
//! # Marked-position automaton
//!
//! The intermediate epsilon-NFTA carries three families of states:
//! `U_q` simulates the complete deterministic automaton `A` of `L` on
//! unmarked context; `M` states record a left-side skeleton match
//! whose variable leaves are pinned to `U` states; `V_q` marks a
//! completed rewrite, evaluated as `A` on the replacement. A
//! successful `V` run contains exactly one conversion from the
//! matching state to the rewritten state; above the mark, transitions
//! carry exactly one `V` child and apply `A`'s transition. Epsilon
//! elimination folds the variable-leaf recordings and the conversion
//! into ordinary transitions; the direction is load-bearing (each
//! child is replaced by an epsilon-PREIMAGE, each parent by an
//! epsilon-POSTIMAGE).
//!
//! # Metering
//!
//! Every construction step charges the caller's [`RelationResources`]
//! — pipeline operations, the evaluation table, matcher states and
//! transitions, `U`/`V` transitions, epsilon-closure relaxations, and
//! every candidate eliminated transition. Structural preflights
//! (checked exponentiation, the matcher-state bound, and the automaton
//! state ceiling) run BEFORE allocation and fail with
//! [`RewriteError::RelationLimitExceeded`]; nothing panics and nothing
//! is silently truncated.
//!
//! # Certificates
//!
//! A construction starts by issuing a pending [`PreimageCertificate`]
//! for the operation ([`PreimageCertificate::issue`]), which enforces
//! the ADR 0001 input contract and refuses cells without an exact
//! construction. After the pipeline completes, the result is bound
//! with the crate-visible [`PreimageCertificate::complete`]. Horizon
//! zero is the identity (the input language itself); `Saturation` is
//! Task 26 scope and is intentionally not offered here.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::language::certificate::PreimageCertificate;
use crate::language::classify::PreimageOperation;
use crate::language::limits::TreeAutomatonLimits;
use crate::language::{RankedSymbol, TreeAutomaton, TreeState, TreeTransition};
use crate::relation::{RelationLimits, RelationResources};
use crate::trs::{Symbol, Term, TermSystem, Variable};
use crate::Path;

/// The output of an approved exact preimage construction: the result
/// automaton and its completed evidence certificate.
#[derive(Clone, Debug)]
pub struct PreimageOutcome {
    automaton: TreeAutomaton,
    certificate: PreimageCertificate,
}

impl PreimageOutcome {
    /// The computed preimage automaton (canonicalized: determinized,
    /// completed, minimized).
    pub fn automaton(&self) -> &TreeAutomaton {
        &self.automaton
    }

    /// The completed exact certificate binding inputs, construction,
    /// and result digest.
    pub fn certificate(&self) -> &PreimageCertificate {
        &self.certificate
    }

    /// Split into automaton and certificate.
    pub fn into_parts(self) -> (TreeAutomaton, PreimageCertificate) {
        (self.automaton, self.certificate)
    }

    /// Assemble an outcome from a completed construction result and
    /// its already-completed certificate. Crate-visible: the trusted
    /// in-crate constructions (Tasks 25-26) are the only producers of
    /// completed `Exact` evidence.
    pub(crate) fn from_parts(automaton: TreeAutomaton, certificate: PreimageCertificate) -> Self {
        Self {
            automaton,
            certificate,
        }
    }
}

/// Exact one-step preimage `R⁻¹(L)` under the application relation,
/// for every class ADR 0001 approves (Ground, LinearVariableDisjoint,
/// LeftLinearShared). Hard `UnsupportedPreimage` otherwise (via
/// `PreimageCertificate::issue`).
pub fn one_step_preimage(
    system: &TermSystem,
    language: &TreeAutomaton,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> RewriteResult<PreimageOutcome> {
    let mut resources = RelationResources::new(limits);
    one_step_preimage_with_resources(system, language, limits, automaton_limits, &mut resources)
}

/// The exact one-step preimage drawing on the CALLER's pool, so a
/// query can share one per-query budget across construction and
/// membership (Task 28, PR #284 round 1 P2).
pub(crate) fn one_step_preimage_with_resources(
    system: &TermSystem,
    language: &TreeAutomaton,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
    resources: &mut RelationResources,
) -> RewriteResult<PreimageOutcome> {
    let certificate =
        PreimageCertificate::issue(PreimageOperation::OneStep, system, language, limits)?;
    let result = one_step_construction(system, language, limits, automaton_limits, resources)?;
    Ok(PreimageOutcome {
        certificate: certificate.complete(&result),
        automaton: result,
    })
}

/// Exact finite-horizon preimage `(R⁻¹)^≤n(L) = ⋃_{i≤n} (R⁻¹)^i(L)`:
/// the identity at `n = 0` (construction `Identity`, result binds the
/// input language itself), else `n` one-step iterations with union and
/// per-step canonicalization (construction `FiniteHorizonIteration`).
pub fn finite_horizon_preimage(
    system: &TermSystem,
    language: &TreeAutomaton,
    horizon: u32,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> RewriteResult<PreimageOutcome> {
    let mut resources = RelationResources::new(limits);
    finite_horizon_preimage_with_resources(
        system,
        language,
        horizon,
        limits,
        automaton_limits,
        &mut resources,
    )
}

/// The exact finite-horizon preimage drawing on the CALLER's pool
/// (Task 28, PR #284 round 1 P2).
#[allow(clippy::too_many_arguments)]
pub(crate) fn finite_horizon_preimage_with_resources(
    system: &TermSystem,
    language: &TreeAutomaton,
    horizon: u32,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
    resources: &mut RelationResources,
) -> RewriteResult<PreimageOutcome> {
    let certificate = PreimageCertificate::issue(
        PreimageOperation::FiniteHorizon(horizon),
        system,
        language,
        limits,
    )?;
    if horizon == 0 || system.rules().is_empty() {
        // Horizon zero is the identity; an empty system has an empty
        // one-step preimage, so every horizon equals the input
        // language. Bind the input representation itself — the
        // empty-system result semantics are digest equality (Task 24
        // round 2), and canonicalization would substitute an
        // equivalent-but-different representation. The identity is
        // NOT exempt from the caller's ceilings or pool: the input
        // must fit the supplied automaton limits, and the retained
        // clone is billed (cohort 5 closeout F2).
        language.check_within_limits(automaton_limits)?;
        resources.record_constraints(
            language.states().len() + language.transitions().len() + language.alphabet().len(),
        )?;
        resources.record_operations(1)?;
        let result = language.clone();
        return Ok(PreimageOutcome {
            certificate: certificate.complete(&result),
            automaton: result,
        });
    }
    let mut accumulator = canonicalize(language.clone(), automaton_limits, resources)?;
    for _ in 0..horizon {
        let preimage =
            one_step_construction(system, &accumulator, limits, automaton_limits, resources)?;
        let union = accumulator.union_with_resources(&preimage, automaton_limits, resources)?;
        accumulator = canonicalize(union, automaton_limits, resources)?;
    }
    Ok(PreimageOutcome {
        certificate: certificate.complete(&accumulator),
        automaton: accumulator,
    })
}

/// The identity preimage: result is the input language itself,
/// construction `Identity`.
pub fn identity_preimage(
    system: &TermSystem,
    language: &TreeAutomaton,
    limits: &RelationLimits,
) -> RewriteResult<PreimageOutcome> {
    let certificate = PreimageCertificate::issue(
        PreimageOperation::FiniteHorizon(0),
        system,
        language,
        limits,
    )?;
    let result = language.clone();
    Ok(PreimageOutcome {
        certificate: certificate.complete(&result),
        automaton: result,
    })
}

/// Epsilon-NFTA intermediate representation (ADR 0001 §Construction:
/// disjoint ordinary states `U_q` simulating `A`, rewritten states
/// `V_q`, and matching states `M`). Eliminated into a plain
/// [`TreeAutomaton`] before canonicalization. Never public.
pub(crate) struct EpsilonNfta {
    pub(crate) alphabet: Vec<RankedSymbol>,
    pub(crate) states: Vec<TreeState>,
    pub(crate) transitions: Vec<TreeTransition>,
    pub(crate) epsilons: Vec<(TreeState, TreeState)>,
    pub(crate) finals: Vec<TreeState>,
}

/// The preflight/elimination core shared by the one-step and
/// finite-horizon entry points. Computes `R⁻¹(language)` and
/// canonicalizes it, charging the caller's shared budget.
fn one_step_construction(
    system: &TermSystem,
    language: &TreeAutomaton,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
    resources: &mut RelationResources,
) -> RewriteResult<TreeAutomaton> {
    // Base construction charge: every call performs work (and every
    // finite-horizon iteration must be metered even when a degenerate
    // alphabet makes the symbol-level charges zero).
    resources.record_operations(1)?;
    // A = complete deterministic automaton for `language`. Completion
    // is load-bearing: erased-variable arguments may have no
    // accepting run in a partial automaton but still need an
    // evaluation state.
    let complete = language
        .determinize_with_resources(automaton_limits, resources)?
        .completed_with_resources(resources)?;
    let nfta = build_epsilon_nfta(system, &complete, limits, automaton_limits, resources)?;
    let (transitions, finals) = eliminate_epsilons(&nfta, automaton_limits, resources)?;
    let assembled = TreeAutomaton::new(
        nfta.alphabet,
        nfta.states,
        transitions,
        finals,
        *automaton_limits,
    )?;
    canonicalize(assembled, automaton_limits, resources)
}

/// Determinize, complete, and minimize under the caller's budget.
pub(crate) fn canonicalize(
    automaton: TreeAutomaton,
    automaton_limits: &TreeAutomatonLimits,
    resources: &mut RelationResources,
) -> RewriteResult<TreeAutomaton> {
    automaton
        .trimmed_with_resources(resources)?
        .determinize_with_resources(automaton_limits, resources)?
        .completed_with_resources(resources)?
        .minimized_with_resources(resources)
}

/// Build the marked-position epsilon-NFTA for `R⁻¹(A)`.
///
/// All structural preflights run before the corresponding allocations;
/// see the module docs for the charge model.
fn build_epsilon_nfta(
    system: &TermSystem,
    a: &TreeAutomaton,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
    resources: &mut RelationResources,
) -> RewriteResult<EpsilonNfta> {
    let q: Vec<TreeState> = a.states().to_vec();

    // Determinized evaluation lookup (claim (⋆)): A is deterministic
    // and complete, so each (symbol, children) key has exactly one
    // parent and every key is present.
    let mut lookup: BTreeMap<(Symbol, Vec<TreeState>), TreeState> = BTreeMap::new();
    for transition in a.transitions() {
        lookup.insert(
            (transition.symbol().clone(), transition.children().to_vec()),
            transition.parent().clone(),
        );
    }

    // Preflight: matcher states `Σ_ri Σ_p q^{|below_p|}` and matching
    // transitions `Σ_ri (Σ_p q^{|below_p|} + q^{|Var(l)|})`, checked
    // before any matcher allocation.
    let mut matcher_states = 0usize;
    let mut matcher_transitions = 0usize;
    for rule in system.rules() {
        let root_variables = rule.lhs().variables();
        let root_assignments = checked_pow(
            q.len(),
            root_variables.len(),
            "preimage matcher states",
            limits.max_operations(),
        )?;
        matcher_transitions = matcher_transitions
            .checked_add(root_assignments)
            .ok_or_else(|| RewriteError::RelationLimitExceeded {
                resource: "preimage matcher states",
                limit: limits.max_operations(),
            })?;
        for position in rule.lhs().positions() {
            let subterm = rule
                .lhs()
                .subterm(&position)
                .expect("positions enumerate valid subterms");
            let below = subterm.variables();
            let below_assignments = checked_pow(
                q.len(),
                below.len(),
                "preimage matcher states",
                limits.max_operations(),
            )?;
            matcher_states = matcher_states
                .checked_add(below_assignments)
                .ok_or_else(|| RewriteError::RelationLimitExceeded {
                    resource: "preimage matcher states",
                    limit: limits.max_operations(),
                })?;
            matcher_transitions = matcher_transitions
                .checked_add(below_assignments)
                .ok_or_else(|| RewriteError::RelationLimitExceeded {
                    resource: "preimage matcher states",
                    limit: limits.max_operations(),
                })?;
        }
    }
    let simulation_states =
        q.len()
            .checked_mul(2)
            .ok_or_else(|| RewriteError::RelationLimitExceeded {
                resource: "preimage automaton states",
                limit: automaton_limits.max_states(),
            })?;
    let state_total = matcher_states
        .checked_add(simulation_states)
        .ok_or_else(|| RewriteError::RelationLimitExceeded {
            resource: "preimage automaton states",
            limit: automaton_limits.max_states(),
        })?;
    if state_total > automaton_limits.max_states() {
        return Err(RewriteError::RelationLimitExceeded {
            resource: "preimage automaton states",
            limit: automaton_limits.max_states(),
        });
    }
    resources.record_operations(matcher_states)?;
    resources.record_operations(matcher_transitions)?;

    let mut states: BTreeSet<TreeState> = BTreeSet::new();
    let mut transitions: Vec<TreeTransition> = Vec::new();
    let mut epsilons: Vec<(TreeState, TreeState)> = Vec::new();

    // U simulation of A: `f(U_q1..U_qm) -> U_q`.
    // V simulation: exactly one marked child, `m >= 1`.
    resources.record_operations(
        a.transitions()
            .len()
            .saturating_mul(1 + automaton_limits.max_rank()),
    )?;
    for state in &q {
        states.insert(ordinary_state(state));
        states.insert(rewritten_state(state));
    }
    for transition in a.transitions() {
        let u_children: Vec<TreeState> = transition.children().iter().map(ordinary_state).collect();
        transitions.push(TreeTransition::new(
            transition.symbol().clone(),
            u_children.clone(),
            ordinary_state(transition.parent()),
        ));
        if !transition.children().is_empty() {
            for (index, child) in transition.children().iter().enumerate() {
                let mut children = u_children.clone();
                children[index] = rewritten_state(child);
                transitions.push(TreeTransition::new(
                    transition.symbol().clone(),
                    children,
                    rewritten_state(transition.parent()),
                ));
            }
        }
    }

    // Matching skeleton and root conversions, per rule.
    for (rule_index, rule) in system.rules().iter().enumerate() {
        let lhs = rule.lhs();
        let rhs = rule.rhs();
        let root_variables = lhs.variables();

        // Eval-table preflight, before allocating the table.
        let table_entries = checked_pow(
            q.len(),
            root_variables.len(),
            "preimage eval-table exponentiation",
            limits.max_operations(),
        )?;
        let rhs_nodes = rhs.positions().len();
        let eval_operations = table_entries.checked_mul(rhs_nodes).ok_or_else(|| {
            RewriteError::RelationLimitExceeded {
                resource: "preimage eval-table exponentiation",
                limit: limits.max_operations(),
            }
        })?;
        resources.record_operations(eval_operations)?;
        resources.record_term(rhs_nodes, term_depth(rhs))?;
        resources.record_term(lhs.positions().len(), term_depth(lhs))?;

        let mut eval_table: BTreeMap<Vec<TreeState>, TreeState> = BTreeMap::new();
        for assignment in enumerate_tuples(&q, root_variables.len()) {
            let substitution: BTreeMap<Variable, TreeState> = root_variables
                .iter()
                .cloned()
                .zip(assignment.iter().cloned())
                .collect();
            let evaluated = evaluate(&lookup, rhs, &substitution).ok_or_else(|| {
                RewriteError::MalformedAutomaton {
                    message: "rule right side is not evaluable over the completed alphabet".into(),
                }
            })?;
            eval_table.insert(assignment, evaluated);
        }

        let is_bare_variable = matches!(lhs, Term::Var(_));
        for position in lhs.positions() {
            let subterm = lhs
                .subterm(&position)
                .expect("positions enumerate valid subterms");
            match subterm {
                Term::Var(_) => {
                    // Variable leaf: epsilon `(U_q, M(ri, p, {x ↦ q}))`.
                    for state in &q {
                        let matcher =
                            matcher_state(rule_index, &position, core::slice::from_ref(state));
                        states.insert(matcher.clone());
                        epsilons.push((ordinary_state(state), matcher));
                    }
                    if position.is_root() && is_bare_variable {
                        // Bare-variable left side: the conversion is
                        // `(U_q, V(eval({x ↦ q})))`.
                        for state in &q {
                            let evaluated = eval_table
                                .get(core::slice::from_ref(state))
                                .expect("bare-variable eval table is keyed by single states");
                            epsilons.push((ordinary_state(state), rewritten_state(evaluated)));
                        }
                    }
                }
                Term::Sym(symbol, arguments) if arguments.is_empty() => {
                    // Constant: `c() -> M(ri, p, ∅)`.
                    let matcher = matcher_state(rule_index, &position, &[]);
                    states.insert(matcher.clone());
                    transitions.push(TreeTransition::new(
                        symbol.clone(),
                        Vec::new(),
                        matcher.clone(),
                    ));
                    if position.is_root() {
                        let evaluated = eval_table
                            .get(&Vec::new())
                            .expect("ground rules have the single empty assignment");
                        epsilons.push((matcher, rewritten_state(evaluated)));
                    }
                }
                Term::Sym(symbol, arguments) => {
                    // Function skeleton node: enumerate the assignments
                    // below the position and project them onto the
                    // children (left-linearity keeps the projections
                    // disjoint and well-defined).
                    let below = subterm.variables();
                    let child_variables: Vec<Vec<Variable>> = arguments
                        .iter()
                        .map(|argument| argument.variables())
                        .collect();
                    for assignment in enumerate_tuples(&q, below.len()) {
                        let children: Vec<TreeState> = child_variables
                            .iter()
                            .enumerate()
                            .map(|(index, child_below)| {
                                let projected = project(&assignment, &below, child_below);
                                matcher_state(rule_index, &position.child(index), &projected)
                            })
                            .collect();
                        for child in &children {
                            states.insert(child.clone());
                        }
                        let matcher = matcher_state(rule_index, &position, &assignment);
                        states.insert(matcher.clone());
                        transitions.push(TreeTransition::new(
                            symbol.clone(),
                            children,
                            matcher.clone(),
                        ));
                        if position.is_root() {
                            let evaluated = eval_table
                                .get(&assignment)
                                .expect("root assignments are exactly the rule's full assignments");
                            epsilons.push((matcher, rewritten_state(evaluated)));
                        }
                    }
                }
            }
        }
    }

    let finals: Vec<TreeState> = a.finals().iter().map(rewritten_state).collect();
    Ok(EpsilonNfta {
        alphabet: a.alphabet().to_vec(),
        states: states.into_iter().collect(),
        transitions,
        epsilons,
        finals,
    })
}

/// Eliminate the epsilon edges of `nfta`, returning the deduplicated
/// transitions and finals of the equivalent plain automaton.
///
/// DIRECTION IS LOAD-BEARING (ADR 0001 §Construction): for each
/// transition `f(s1..sm) -> s`, every child `ci` ranges over the
/// epsilon-PREIMAGES of `si` (states that can silently reach `si`)
/// and the parent `p` ranges over the epsilon-POSTIMAGES of `s`
/// (states `s` can silently reach).
pub(crate) fn eliminate_epsilons(
    nfta: &EpsilonNfta,
    automaton_limits: &TreeAutomatonLimits,
    resources: &mut RelationResources,
) -> RewriteResult<(Vec<TreeTransition>, Vec<TreeState>)> {
    // Reserve the closure and preimage maps before allocation: each
    // holds up to N² cells (review round 2's storage accounting).
    let state_count = nfta.states.len();
    let squared = state_count
        .checked_mul(state_count)
        .and_then(|n_sq| n_sq.checked_mul(2))
        .ok_or(RewriteError::RelationLimitExceeded {
            resource: "elimination relation storage",
            limit: usize::MAX,
        })?;
    for _ in 0..squared {
        resources.record_constraints(1)?;
    }
    // Reflexive-transitive closure of the epsilon relation. Our edges
    // only go U -> M and M -> V, so it converges immediately, but the
    // generic fixpoint is implemented.
    let mut closure: BTreeMap<TreeState, BTreeSet<TreeState>> = BTreeMap::new();
    for state in &nfta.states {
        closure
            .entry(state.clone())
            .or_default()
            .insert(state.clone());
    }
    for (from, to) in &nfta.epsilons {
        closure
            .entry(from.clone())
            .or_default()
            .insert(from.clone());
        closure.entry(to.clone()).or_default().insert(to.clone());
    }
    loop {
        let mut grew = false;
        for (from, to) in &nfta.epsilons {
            let reached: Vec<TreeState> = closure
                .get(to)
                .map(|set| set.iter().cloned().collect())
                .unwrap_or_default();
            // Charge the edge visit AND every copied closure cell —
            // set copies are not constant work (review round 1) — and
            // reserve the scratch vector as constraints: it is
            // independently allocated relation storage (review round 5).
            resources.record_operations(1 + reached.len())?;
            resources.record_constraints(reached.len())?;
            let from_set = closure.entry(from.clone()).or_default();
            for state in reached {
                if from_set.insert(state) {
                    grew = true;
                }
            }
        }
        if !grew {
            break;
        }
    }

    // Invert the closure into epsilon-preimage sets.
    let mut preimages: BTreeMap<TreeState, Vec<TreeState>> = BTreeMap::new();
    for (from, reached) in &closure {
        resources.record_operations(reached.len())?;
        for target in reached {
            preimages
                .entry(target.clone())
                .or_default()
                .push(from.clone());
        }
    }

    let mut eliminated: BTreeSet<TreeTransition> = BTreeSet::new();
    for transition in &nfta.transitions {
        // Copy the child-preimage lists — charged per copied cell
        // (review round 2): tuple construction is not constant work.
        let mut child_sets: Vec<Vec<TreeState>> = Vec::new();
        for child in transition.children() {
            let set = preimages
                .get(child)
                .cloned()
                .unwrap_or_else(|| Vec::from([child.clone()]));
            // The clone is billed twice under the metering model:
            // operations for the copy work, constraints for the
            // independently-retained relation storage (review round 3).
            resources.record_operations(set.len())?;
            resources.record_constraints(set.len())?;
            child_sets.push(set);
        }
        let parent_set: Vec<TreeState> = closure
            .get(transition.parent())
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default();
        resources.record_operations(parent_set.len())?;
        resources.record_constraints(parent_set.len())?;
        for_each_combination(&child_sets, |children| {
            for parent in &parent_set {
                // Candidate visit plus BOTH tuple materializations:
                // the combination iterator's children vector and the
                // to_vec copy into the deduplicated set (review round 3).
                resources.record_operations(1 + 2 * children.len())?;
                eliminated.insert(TreeTransition::new(
                    transition.symbol().clone(),
                    children.to_vec(),
                    parent.clone(),
                ));
                // The transition ceiling is enforced as the deduplicated
                // set grows — a typed exhaustion outcome, not a late
                // assembly error after the blowup is materialized.
                if eliminated.len() > automaton_limits.max_transitions() {
                    return Err(RewriteError::RelationLimitExceeded {
                        resource: "preimage automaton transitions",
                        limit: automaton_limits.max_transitions(),
                    });
                }
            }
            Ok(())
        })?;
    }

    // Final membership uses an indexed set — metered construction
    // (operations for the inserts, constraints for the retained
    // cells) — instead of unmetered linear scans of the final-state
    // vector (review round 5).
    resources.record_operations(nfta.finals.len())?;
    resources.record_constraints(nfta.finals.len())?;
    let final_set: BTreeSet<&TreeState> = nfta.finals.iter().collect();
    // The final-state scan visits every state, and each indexed
    // lookup in its reached set is unit work (review round 6).
    resources.record_operations(nfta.states.len())?;
    let mut eliminated_finals: Vec<TreeState> = Vec::new();
    for state in &nfta.states {
        if let Some(reached) = closure.get(state) {
            resources.record_operations(reached.len())?;
            if reached.iter().any(|s| final_set.contains(s)) {
                eliminated_finals.push(state.clone());
            }
        }
    }

    Ok((eliminated.into_iter().collect(), eliminated_finals))
}

/// Evaluate `term` bottom-up against the complete deterministic `A`,
/// reading each variable leaf from `assignment`. `Some` always for
/// ground-over-alphabet terms; `None` only if a variable is unassigned
/// or the term uses a symbol outside `A`'s alphabet.
///
/// Positions are evaluated deepest-first (parents after children), so
/// no recursion on term depth is performed.
fn evaluate(
    lookup: &BTreeMap<(Symbol, Vec<TreeState>), TreeState>,
    term: &Term,
    assignment: &BTreeMap<Variable, TreeState>,
) -> Option<TreeState> {
    let mut positions = term.positions();
    positions.sort_by_key(|path| core::cmp::Reverse(path.as_slice().len()));
    let mut states: BTreeMap<Path, TreeState> = BTreeMap::new();
    for path in &positions {
        let subterm = term.subterm(path)?;
        let state = match subterm {
            Term::Var(variable) => assignment.get(variable).cloned()?,
            Term::Sym(symbol, arguments) => {
                let mut children = Vec::with_capacity(arguments.len());
                for (index, _) in arguments.iter().enumerate() {
                    children.push(states.get(&path.child(index)).cloned()?);
                }
                lookup.get(&(symbol.clone(), children)).cloned()?
            }
        };
        states.insert(path.clone(), state);
    }
    states.remove(&Path::root())
}

/// Enumerate the `states.len()^k` assignments of `k` variables
/// lexicographically (odometer over `states`); `k = 0` yields exactly
/// one empty tuple.
fn enumerate_tuples(states: &[TreeState], k: usize) -> Vec<Vec<TreeState>> {
    if states.is_empty() {
        return if k == 0 {
            Vec::from([Vec::new()])
        } else {
            Vec::new()
        };
    }
    let mut result = Vec::new();
    let mut indices = alloc_zeroed_indices(k);
    loop {
        result.push(indices.iter().map(|index| states[*index].clone()).collect());
        let mut position = k;
        loop {
            if position == 0 {
                return result;
            }
            position -= 1;
            indices[position] += 1;
            if indices[position] < states.len() {
                break;
            }
            indices[position] = 0;
        }
    }
}

/// `k` zero-valued indices, allocation-friendly.
fn alloc_zeroed_indices(k: usize) -> Vec<usize> {
    core::iter::repeat_n(0usize, k).collect()
}

/// Iterate the Cartesian product of `sets` in lexicographic order,
/// calling `f` once per combination. An empty `sets` yields exactly
/// the empty combination; any empty member yields no combinations.
fn for_each_combination<T: Clone, F: FnMut(&[T]) -> RewriteResult<()>>(
    sets: &[Vec<T>],
    mut f: F,
) -> RewriteResult<()> {
    if sets.is_empty() {
        return f(&[]);
    }
    if sets.iter().any(Vec::is_empty) {
        return Ok(());
    }
    let mut indices = alloc_zeroed_indices(sets.len());
    loop {
        let combination: Vec<T> = indices
            .iter()
            .enumerate()
            .map(|(position, index)| sets[position][*index].clone())
            .collect();
        f(&combination)?;
        let mut position = sets.len();
        loop {
            if position == 0 {
                return Ok(());
            }
            position -= 1;
            indices[position] += 1;
            if indices[position] < sets[position].len() {
                break;
            }
            indices[position] = 0;
        }
    }
}

/// Project the assignment aligned to `below` onto the sub-list
/// `sub_below` (which must be a subset of `below`).
fn project(alpha: &[TreeState], below: &[Variable], sub_below: &[Variable]) -> Vec<TreeState> {
    sub_below
        .iter()
        .map(|variable| {
            let index = below
                .iter()
                .position(|candidate| candidate == variable)
                .expect("sub-variables occur below their parent node");
            alpha[index].clone()
        })
        .collect()
}

/// The ordinary (`U`) state name simulating `q`.
fn ordinary_state(state: &TreeState) -> TreeState {
    TreeState::new(format!("u/{}", state.name().as_str()))
}

/// The rewritten (`V`) state name for `q`.
fn rewritten_state(state: &TreeState) -> TreeState {
    TreeState::new(format!("v/{}", state.name().as_str()))
}

/// The matching (`M`) state name for rule `rule_index`, skeleton
/// position `path`, and the assignment `states` aligned to the
/// position's sorted variable list.
fn matcher_state(rule_index: usize, path: &Path, states: &[TreeState]) -> TreeState {
    TreeState::new(format!(
        "m/{rule_index}/{}/{}",
        render_path(path),
        render_assignment(states)
    ))
}

/// Render a skeleton position as child indices joined by `.` (the root
/// is the empty string).
fn render_path(path: &Path) -> String {
    let mut rendered = String::new();
    for (index, component) in path.as_slice().iter().enumerate() {
        if index > 0 {
            rendered.push('.');
        }
        rendered.push_str(&format!("{component}"));
    }
    rendered
}

/// Render a state assignment as the comma-joined state names.
fn render_assignment(states: &[TreeState]) -> String {
    let mut rendered = String::new();
    for (index, state) in states.iter().enumerate() {
        if index > 0 {
            rendered.push(',');
        }
        rendered.push_str(state.name().as_str());
    }
    rendered
}

/// The edge-based depth of a term (a constant has depth 0).
fn term_depth(term: &Term) -> usize {
    term.positions()
        .iter()
        .map(|path| path.as_slice().len())
        .max()
        .unwrap_or(0)
}

/// Checked exponentiation with a typed limit outcome on overflow.
fn checked_pow(
    base: usize,
    exponent: usize,
    resource: &'static str,
    limit: usize,
) -> RewriteResult<usize> {
    let exponent = u32::try_from(exponent)
        .map_err(|_| RewriteError::RelationLimitExceeded { resource, limit })?;
    base.checked_pow(exponent)
        .ok_or(RewriteError::RelationLimitExceeded { resource, limit })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trs::Rule;
    use alloc::vec;

    fn f(left: Term, right: Term) -> Term {
        Term::sym("f", [left, right])
    }

    fn a() -> Term {
        Term::constant("a")
    }

    fn x() -> Term {
        Term::var("x")
    }

    /// A complete deterministic automaton over {a/0, g/1, f/2} with
    /// two states, used to exercise repetition and erasure.
    fn two_state_automaton() -> TreeAutomaton {
        let q0 = TreeState::new("q0");
        let q1 = TreeState::new("q1");
        TreeAutomaton::new(
            vec![
                RankedSymbol::new(Symbol::new("a"), 0),
                RankedSymbol::new(Symbol::new("g"), 1),
                RankedSymbol::new(Symbol::new("f"), 2),
            ],
            vec![q0.clone(), q1.clone()],
            vec![
                TreeTransition::new(Symbol::new("a"), vec![], q0.clone()),
                TreeTransition::new(Symbol::new("g"), vec![q0.clone()], q1.clone()),
                TreeTransition::new(Symbol::new("g"), vec![q1.clone()], q0.clone()),
                TreeTransition::new(Symbol::new("f"), vec![q0.clone(), q0.clone()], q0.clone()),
                TreeTransition::new(Symbol::new("f"), vec![q0.clone(), q1.clone()], q1.clone()),
                TreeTransition::new(Symbol::new("f"), vec![q1.clone(), q0.clone()], q1.clone()),
                TreeTransition::new(Symbol::new("f"), vec![q1.clone(), q1.clone()], q1.clone()),
            ],
            vec![q1],
            TreeAutomatonLimits::default(),
        )
        .expect("fixture automaton is valid")
    }

    fn lookup_of(automaton: &TreeAutomaton) -> BTreeMap<(Symbol, Vec<TreeState>), TreeState> {
        automaton
            .transitions()
            .iter()
            .map(|transition| {
                (
                    (transition.symbol().clone(), transition.children().to_vec()),
                    transition.parent().clone(),
                )
            })
            .collect()
    }

    #[test]
    fn evaluate_reuses_state_for_repetition_and_erasure() {
        let automaton = two_state_automaton();
        let lookup = lookup_of(&automaton);
        let q0 = TreeState::new("q0");
        let mut assignment = BTreeMap::new();
        assignment.insert(Variable::new("x"), q0.clone());

        // Repetition on the right reuses the single state value.
        assert_eq!(
            evaluate(&lookup, &f(x(), x()), &assignment),
            Some(q0.clone())
        );
        // A bare variable leaf reads its assignment.
        assert_eq!(evaluate(&lookup, &x(), &assignment), Some(q0.clone()));
        // A ground constant ignores the assignment entirely.
        assert_eq!(
            evaluate(&lookup, &a(), &assignment),
            Some(TreeState::new("q0"))
        );
        // An unassigned variable has no evaluation.
        assert_eq!(
            evaluate(&lookup, &f(x(), Term::var("y")), &assignment),
            None
        );
    }

    #[test]
    fn tuple_enumeration_counts() {
        let states = vec![TreeState::new("q0"), TreeState::new("q1")];
        let empty = enumerate_tuples(&states, 0);
        assert_eq!(empty.len(), 1);
        assert!(empty[0].is_empty());
        assert_eq!(enumerate_tuples(&states, 2).len(), 4);
        let single = enumerate_tuples(&states, 1);
        assert_eq!(single.len(), 2);
        assert_eq!(single[0][0], TreeState::new("q0"));
        assert_eq!(single[1][0], TreeState::new("q1"));
    }

    #[test]
    fn epsilon_elimination_folds_leaf_recording_and_conversion() {
        let u_q = TreeState::new("u/q");
        let v_q = TreeState::new("v/q");
        let m_leaf = TreeState::new("m/0/0/q");
        let m_root = TreeState::new("m/0//q");
        let nfta = EpsilonNfta {
            alphabet: vec![
                RankedSymbol::new(Symbol::new("a"), 0),
                RankedSymbol::new(Symbol::new("g"), 1),
                RankedSymbol::new(Symbol::new("f"), 2),
            ],
            states: vec![u_q.clone(), v_q.clone(), m_leaf.clone(), m_root.clone()],
            transitions: vec![
                TreeTransition::new(Symbol::new("g"), vec![u_q.clone()], u_q.clone()),
                TreeTransition::new(Symbol::new("g"), vec![m_leaf.clone()], m_root.clone()),
            ],
            // U -> M (leaf recording) and M -> V (conversion).
            epsilons: vec![(u_q.clone(), m_leaf.clone()), (m_root.clone(), v_q.clone())],
            finals: vec![v_q.clone()],
        };
        let mut resources = RelationResources::new(&RelationLimits::default());
        let (transitions, finals) =
            eliminate_epsilons(&nfta, &TreeAutomatonLimits::default(), &mut resources)
                .expect("elimination succeeds");
        let folded_leaf = TreeTransition::new(Symbol::new("g"), vec![u_q.clone()], m_leaf.clone());
        let folded_conversion =
            TreeTransition::new(Symbol::new("g"), vec![m_leaf.clone()], v_q.clone());
        assert!(
            transitions.contains(&folded_leaf),
            "the variable-leaf recording must fold into a U transition"
        );
        assert!(
            transitions.contains(&folded_conversion),
            "the conversion must fold into the skeleton transition"
        );
        // The converted matcher and the rewritten state are accepting,
        // and the folded transitions consume ordinary states directly,
        // so no epsilon edge is needed for productivity.
        assert!(finals.contains(&v_q));
        assert!(finals.contains(&m_root));
        assert_eq!(folded_leaf.children(), core::slice::from_ref(&u_q));
        assert!(transitions
            .iter()
            .all(|transition| transition.symbol().as_str() != "epsilon"));
    }

    /// Review round 1 (P2): elimination meters the closure's copied
    /// cells, not just edge visits. Fixture: a()→p with the epsilon
    /// chain p→q→r. Two relaxation rounds visit 4 edges and copy 6
    /// closure cells (1+2 + 1+1 per round); charging only per visit
    /// yields 4 — the test demands at least the visits + copies = 10.
    /// Review round 2 (P2): candidate tuple construction is metered.
    /// A single rank-100 transition copies 100 child-preimage lists
    /// (one cell each) and a 100-cell children vector — the budget of
    /// 150 must be exhausted by those copies (the old metering charged
    /// only the single candidate visit).
    fn high_rank_nfta() -> EpsilonNfta {
        let s = TreeState::new("s");
        let p = TreeState::new("p");
        EpsilonNfta {
            alphabet: vec![
                RankedSymbol::new(Symbol::new("a"), 0),
                RankedSymbol::new(Symbol::new("f"), 100),
            ],
            states: vec![s.clone(), p.clone()],
            transitions: vec![
                TreeTransition::new(Symbol::new("a"), vec![], s.clone()),
                TreeTransition::new(
                    Symbol::new("f"),
                    (0..100).map(|_| s.clone()).collect(),
                    p.clone(),
                ),
            ],
            epsilons: vec![],
            finals: vec![p],
        }
    }

    /// Review round 3 (P2): cloned child-preimage lists are relation
    /// storage and must be reserved as constraints — 100 copied cells
    /// on top of the 2·N² map reservation (N=2 here) must exhaust a
    /// 9-cell budget.
    #[test]
    fn elimination_reserves_cloned_relation_buffers() {
        let nfta = high_rank_nfta();
        let tight = RelationLimits::new(4_096, 64, 9, 1_000_000).expect("valid limits");
        let mut resources = RelationResources::new(&tight);
        let result = eliminate_epsilons(&nfta, &TreeAutomatonLimits::default(), &mut resources);
        assert!(
            matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
            "cloned relation buffers must be reserved as constraints, got {result:?}"
        );
    }

    /// Review round 3 (P2): the combination iterator's tuple
    /// materialization and the parent/final scans are metered. With
    /// generous constraints: inversion rows charge 2, child-list
    /// copies 100, the candidate visit + tuple materialization +
    /// to_vec copy 1+100+100, the parent-set copy 1, the final scan 2
    /// — 306 in total, so a 205-operation budget must exhaust.
    /// Review round 4 (P2): the cloned parent-set buffer is relation
    /// storage too — a nullary-only fixture (N=1) reserves 2 cells for
    /// the maps, and the cloned parent set needs a third, so a 2-cell
    /// budget must exhaust.
    /// Review round 5 (P2): the per-visit closure scratch (the cloned
    /// reached vector) is independently allocated relation storage —
    /// with one state and a self epsilon-edge the maps reserve 2
    /// cells and the scratch needs a third, so a 2-cell budget must
    /// exhaust.
    #[test]
    fn elimination_reserves_closure_scratch() {
        let s = TreeState::new("s");
        let nfta = EpsilonNfta {
            alphabet: vec![RankedSymbol::new(Symbol::new("a"), 0)],
            states: vec![s.clone()],
            transitions: vec![],
            epsilons: vec![(s.clone(), s.clone())],
            finals: vec![],
        };
        let tight = RelationLimits::new(4_096, 64, 2, 4_096).expect("valid limits");
        let mut resources = RelationResources::new(&tight);
        let result = eliminate_epsilons(&nfta, &TreeAutomatonLimits::default(), &mut resources);
        assert!(
            matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
            "closure scratch must be reserved as constraints, got {result:?}"
        );
    }

    /// Review round 5 (P2): final membership uses an indexed set whose
    /// construction is metered (operations for the inserts,
    /// constraints for the retained cells) instead of unmetered linear
    /// scans of the final-state vector. Ten states all final: the
    /// build (10) plus inversion (10) plus the per-state visit (10)
    /// is 30, so a 25-operation budget must exhaust.
    /// Review round 6 (P2): every indexed final lookup is unit work.
    /// Three-state epsilon cycle with final {q2}: closure visits 22,
    /// inversion 9, index build 1, outer visits 3 — 35 — and the 9
    /// lookups bring the total to 44, so a 40-operation budget must
    /// exhaust.
    #[test]
    fn elimination_meters_indexed_final_lookups() {
        let q0 = TreeState::new("q0");
        let q1 = TreeState::new("q1");
        let q2 = TreeState::new("q2");
        let nfta = EpsilonNfta {
            alphabet: vec![RankedSymbol::new(Symbol::new("a"), 0)],
            states: vec![q0.clone(), q1.clone(), q2.clone()],
            transitions: vec![],
            epsilons: vec![
                (q0.clone(), q1.clone()),
                (q1.clone(), q2.clone()),
                (q2.clone(), q0.clone()),
            ],
            finals: vec![q2],
        };
        let tight = RelationLimits::new(4_096, 64, 4_096, 40).expect("valid limits");
        let mut resources = RelationResources::new(&tight);
        let result = eliminate_epsilons(&nfta, &TreeAutomatonLimits::default(), &mut resources);
        assert!(
            matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
            "indexed final lookups must be metered, got {result:?}"
        );
    }

    #[test]
    fn elimination_uses_metered_final_index() {
        let states: Vec<TreeState> = (0..10)
            .map(|i| TreeState::new(alloc::format!("s{i}")))
            .collect();
        let nfta = EpsilonNfta {
            alphabet: vec![RankedSymbol::new(Symbol::new("a"), 0)],
            states: states.clone(),
            transitions: vec![],
            epsilons: vec![],
            finals: states,
        };
        let tight = RelationLimits::new(4_096, 64, 4_096, 25).expect("valid limits");
        let mut resources = RelationResources::new(&tight);
        let result = eliminate_epsilons(&nfta, &TreeAutomatonLimits::default(), &mut resources);
        assert!(
            matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
            "the indexed final set must be metered, got {result:?}"
        );
    }

    #[test]
    fn elimination_reserves_cloned_parent_buffers() {
        let s = TreeState::new("s");
        let nfta = EpsilonNfta {
            alphabet: vec![RankedSymbol::new(Symbol::new("a"), 0)],
            states: vec![s.clone()],
            transitions: vec![TreeTransition::new(Symbol::new("a"), vec![], s.clone())],
            epsilons: vec![],
            finals: vec![s],
        };
        let tight = RelationLimits::new(4_096, 64, 2, 4_096).expect("valid limits");
        let mut resources = RelationResources::new(&tight);
        let result = eliminate_epsilons(&nfta, &TreeAutomatonLimits::default(), &mut resources);
        assert!(
            matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
            "the cloned parent buffer must be reserved as constraints, got {result:?}"
        );
    }

    #[test]
    fn elimination_meters_tuple_materialization_and_scans() {
        let nfta = high_rank_nfta();
        let tight = RelationLimits::new(4_096, 64, 4_096, 205).expect("valid limits");
        let mut resources = RelationResources::new(&tight);
        let result = eliminate_epsilons(&nfta, &TreeAutomatonLimits::default(), &mut resources);
        assert!(
            matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
            "tuple materialization must be metered, got {result:?}"
        );
    }

    #[test]
    fn elimination_meters_high_rank_copies() {
        let states: Vec<TreeState> = (0..100)
            .map(|i| TreeState::new(alloc::format!("s{i}")))
            .collect();
        let parent = TreeState::new("p");
        let nfta = EpsilonNfta {
            alphabet: vec![
                RankedSymbol::new(Symbol::new("a"), 0),
                RankedSymbol::new(Symbol::new("f"), 100),
            ],
            states: states
                .iter()
                .cloned()
                .chain(core::iter::once(parent.clone()))
                .collect(),
            transitions: vec![
                TreeTransition::new(Symbol::new("a"), vec![], states[0].clone()),
                TreeTransition::new(
                    Symbol::new("f"),
                    (0..100).map(|i| states[i].clone()).collect(),
                    parent.clone(),
                ),
            ],
            epsilons: vec![],
            finals: vec![parent],
        };
        let tight = RelationLimits::new(4_096, 64, 4_096, 150).expect("valid limits");
        let mut resources = RelationResources::new(&tight);
        let result = eliminate_epsilons(&nfta, &TreeAutomatonLimits::default(), &mut resources);
        assert!(
            matches!(result, Err(RewriteError::RelationLimitExceeded { .. })),
            "rank-100 tuple copies must exhaust the operations budget, got {result:?}"
        );
    }

    #[test]
    fn elimination_meters_closure_copies() {
        let p = TreeState::new("p");
        let q = TreeState::new("q");
        let r = TreeState::new("r");
        let nfta = EpsilonNfta {
            alphabet: vec![RankedSymbol::new(Symbol::new("a"), 0)],
            states: vec![p.clone(), q.clone(), r.clone()],
            transitions: vec![TreeTransition::new(Symbol::new("a"), vec![], p.clone())],
            epsilons: vec![(p.clone(), q.clone()), (q.clone(), r.clone())],
            finals: vec![r.clone()],
        };
        let mut resources = RelationResources::new(&RelationLimits::default());
        eliminate_epsilons(&nfta, &TreeAutomatonLimits::default(), &mut resources)
            .expect("elimination succeeds");
        assert!(
            resources.operations() >= 10,
            "closure copies must be charged (visits + copied cells), got {}",
            resources.operations()
        );
    }

    #[test]
    fn assignment_enumeration_below_a_node() {
        let automaton = two_state_automaton();
        let system = TermSystem::new(vec![Rule::new(f(x(), Term::var("y")), x()).unwrap()]);
        let mut resources = RelationResources::new(&RelationLimits::default());
        let limits = TreeAutomatonLimits::default();
        let nfta = build_epsilon_nfta(
            &system,
            &automaton,
            &RelationLimits::default(),
            &limits,
            &mut resources,
        )
        .expect("construction succeeds");
        let q = automaton.states().len();
        let root_states: Vec<&TreeState> = nfta
            .states
            .iter()
            .filter(|state| state.name().as_str().starts_with("m/0//"))
            .collect();
        assert_eq!(root_states.len(), q * q);
        let names: BTreeSet<&str> = root_states
            .iter()
            .map(|state| state.name().as_str())
            .collect();
        assert_eq!(names.len(), q * q);
        for name in names {
            assert!(
                name.contains("q0") || name.contains("q1"),
                "the assignment must be encoded in the state name, got {name}"
            );
        }
    }
}
