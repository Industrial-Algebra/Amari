// SPDX-License-Identifier: MIT OR Apache-2.0

//! Exact unbounded-saturation preimage `R*(L)` for the ADR 0001 classes
//! `Ground` and `LinearVariableDisjoint` — under the input contract,
//! left-linear RIGHT-GROUND systems. Construction: target-specialized
//! backward epsilon saturation over a FIXED state universe (Base/Any/Pat),
//! per the reviewed design (same finite-state closure principle as TATA
//! Thm. 3.2.14; pattern-instance automata per Prop. 3.4.7; not a literal
//! two-automaton GTT closure). NOT naive layer-union (matrix §S2).
//!
//! # Construction sketch
//!
//! The base automaton is the caller's language AS SUPPLIED (partial and
//! nondeterministic presentations are fine — no determinization or
//! completion is performed first). States are drawn from a fixed,
//! injectively tagged universe: `Base(q)` for each input state, one
//! universal `Any`, and `Pat(i,p)` for every NON-VARIABLE occurrence at
//! path `p` of rule `i`'s left side. Because the tags are disjoint, a
//! pattern state never collapses onto the language automaton's state even
//! when their languages coincide.
//!
//! The fixed symbol transitions `D` are (a) every base transition with all
//! states tagged `Base`, (b) `f(Any,...,Any) -> Any` for every alphabet
//! symbol (including constants), and (c) for every left-side pattern
//! occurrence `f(t1..tk)` at `(i,p)`, the transition
//! `f(state(i,p.1),...,state(i,p.k)) -> Pat(i,p)` where a variable child is
//! `Any`. The `Any` transitions make `Any` recognize every ground term over
//! the alphabet, supplying the totality that erased-variable arguments
//! need. No transition is added from `Any` except for a genuine
//! variable-rooted left side.
//!
//! Starting from an empty epsilon edge set `E`, synchronous rounds add
//! `(root_i, q)` for every rule `i` and every `q` in the set-valued
//! evaluation
//!
//! ```text
//! Run_E(f(t1..tk)) = C_E({ p | (f(s1..sk)->p) in D,
//!                            sj in Run_E(tj) for every j })
//! ```
//!
//! where `C_E` is the reflexive-transitive closure of `E`. All state
//! families (Base, Any, Pat) are eligible targets: edges into `Pat` states
//! implement the rewritten-material feedback (a right side that itself
//! matches a left-side pattern). A round is productive when it adds at
//! least one previously absent edge; the fixpoint stops only after a
//! complete no-change round. There are at most `min(m*N, N^2)` productive
//! rounds.
//!
//! The resulting epsilon-NFTA is eliminated with the Task 25 primitive
//! (`eliminate_epsilons`: children range over epsilon-PREIMAGES, parents
//! over epsilon-POSTIMAGES, finals are closure-saturated), then
//! canonicalized (trim/determinize/complete/minimize). The certificate is
//! completed only after that final stage.
//!
//! # Soundness and completeness (design §2)
//!
//! Semantically, `I_s = Pre*_R(B_s)` where `B_s` is the language of state
//! `s` in the fixed symbol automaton. Each fixed transition preserves the
//! interpretations `I` (rewrite each child into its witness in the child's
//! `B`, then apply the transition). An edge `root_i -> q` is justified
//! because the ground right side `r_i` rewrites to a witness `u ∈ B_q`;
//! every term in `I_root_i` rewrites to a ground instance `l_i σ`, then by
//! rule `i` to `r_i`, then to `u`. All edges of a synchronous round
//! preserve `I` even when they create cycles, and epsilon closure preserves
//! `I`; at the fixed point the accepted union is exactly `Pre*_R(L)`.
//!
//! For completeness, every left-side ground instance `l_i σ` has a literal
//! pattern run to `root_i` (linearity makes the independent child witnesses
//! combinable), and closure under a single backward rewrite holds in every
//! state: if `u = C[r_i]` runs to `s` and `t = C[l_i σ] -> u`, the state `q`
//! at the root of the displayed `r_i` subtree lies in `Run_E(r_i)`, the
//! fixed point contains `root_i -> q`, and replacing the subtree run by the
//! literal pattern run followed by that edge takes `t` to `s`. This remains
//! valid when `q` is a `Pat` state, which is exactly the
//! inner-rewrite-created-redex mechanism. No confluence, termination,
//! size-decrease, or rewrite-length bound is assumed.
//!
//! # Metering (design §4)
//!
//! All work charges one shared [`RelationResources`] opened from `limits`,
//! including epsilon elimination and canonicalization. A base operation is
//! charged at entry (even for empty rules/alphabet); every left and right
//! side is measured with a bounded iterative traversal and charged via
//! `record_term`. The state and fixed-transition ceilings are preflighted
//! with checked arithmetic BEFORE allocation (typed
//! [`RewriteError::RelationLimitExceeded`]). Each round recomputes `C_E`
//! with a metered boolean Floyd-Warshall: the `N^2` matrix reservation is
//! charged via `record_constraints` and the `N^3` triple checks via
//! `record_operations`, both checked before the work. Per right-side
//! evaluation the fixed-transition scans, child-membership checks, and
//! closure-union membership steps are charged; every round charges at
//! least one operation and a final no-change round is required before
//! stopping. Any checked-arithmetic overflow is a typed limit outcome,
//! never a wrap or panic, and no certificate is completed after any
//! failure.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use crate::error::{RewriteError, RewriteResult};
use crate::language::preimage::{canonicalize, eliminate_epsilons, EpsilonNfta, PreimageOutcome};
use crate::language::{
    PreimageCertificate, PreimageOperation, RankedSymbol, TreeAutomaton, TreeAutomatonLimits,
    TreeState, TreeTransition,
};
use crate::relation::{RelationLimits, RelationResources};
use crate::trs::{Symbol, Term, TermSystem, Variable};
use crate::Path;

/// Index of a universe state.
type StateIndex = usize;

/// The children/parent index pair of one fixed symbol transition.
type FixedEntry = (Vec<StateIndex>, StateIndex);

/// Fixed symbol transitions grouped by their read symbol.
type FixedBySymbol = BTreeMap<Symbol, Vec<FixedEntry>>;

/// Exact saturation preimage `R*(L) = { t | ∃ s ∈ L, t →* s }`.
///
/// Hard `UnsupportedPreimage` for classes ADR 0001 does not approve
/// (via [`PreimageCertificate::issue`]).
pub fn saturation_preimage(
    system: &TermSystem,
    language: &TreeAutomaton,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
) -> RewriteResult<PreimageOutcome> {
    let mut resources = RelationResources::new(limits);
    saturation_preimage_with_resources(system, language, limits, automaton_limits, &mut resources)
}

/// The exact saturation preimage drawing on the CALLER's pool, so a
/// query can share one per-query budget across construction and
/// membership (Task 28, PR #284 round 1 P2).
pub(crate) fn saturation_preimage_with_resources(
    system: &TermSystem,
    language: &TreeAutomaton,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
    resources: &mut RelationResources,
) -> RewriteResult<PreimageOutcome> {
    let certificate =
        PreimageCertificate::issue(PreimageOperation::Saturation, system, language, limits)?;
    // Base construction charge: every call performs work (and the
    // saturation must be metered even when a degenerate input makes the
    // symbol-level charges zero).
    charge_operations(resources, limits, 1)?;

    // Saturation of the empty system is the identity. Bind the input
    // representation itself: the empty-system result semantics are digest
    // equality (Task 24), and canonicalization would substitute an
    // equivalent-but-different representation.
    if system.rules().is_empty() {
        // The identity is NOT exempt from the caller's ceilings or
        // pool: the input must fit the supplied automaton limits, and
        // the retained clone is billed (cohort 5 closeout F2).
        language.check_within_limits(automaton_limits)?;
        resources.record_constraints(
            language.states().len() + language.transitions().len() + language.alphabet().len(),
        )?;
        let result = language.clone();
        let certificate = certificate.complete(&result);
        return Ok(PreimageOutcome::from_parts(result, certificate));
    }

    let universe = build_universe(system, language, limits, automaton_limits, resources)?;
    let (edges, _productive_rounds) = saturate(&universe, system, limits, resources)?;
    // The assembly clones below are freshly allocated buffers: the
    // epsilon pairs, the state/transition/alphabet copies handed to
    // the NFTA — billed as constraints before allocation (round 6).
    let assembly_cells = edges
        .len()
        .checked_mul(2)
        .and_then(|cells| {
            cells
                .checked_add(universe.states.len())
                .and_then(|c| c.checked_add(universe.fixed.len()))
                .and_then(|c| c.checked_add(universe.alphabet.len()))
        })
        .ok_or(RewriteError::RelationLimitExceeded {
            resource: "saturation assembly storage",
            limit: limits.max_constraints(),
        })?;
    charge_constraints(resources, limits, assembly_cells)?;
    let epsilons: Vec<(TreeState, TreeState)> = edges
        .iter()
        .map(|&(from, to)| (universe.states[from].clone(), universe.states[to].clone()))
        .collect();
    let nfta = EpsilonNfta {
        alphabet: universe.alphabet.clone(),
        states: universe.states.clone(),
        transitions: universe.fixed.clone(),
        epsilons,
        finals: universe.base_finals.clone(),
    };
    let (transitions, finals) = eliminate_epsilons(&nfta, automaton_limits, resources)?;
    let assembled = TreeAutomaton::new(
        nfta.alphabet,
        nfta.states,
        transitions,
        finals,
        *automaton_limits,
    )?;
    let result = canonicalize(assembled, automaton_limits, resources)?;
    let certificate = certificate.complete(&result);
    Ok(PreimageOutcome::from_parts(result, certificate))
}

/// The fixed state universe `Base ∪ {Any} ∪ Pat` together with the fixed
/// symbol transitions `D` grouped for right-side evaluation.
struct Universe {
    alphabet: Vec<RankedSymbol>,
    states: Vec<TreeState>,
    fixed: Vec<TreeTransition>,
    fixed_by_symbol: FixedBySymbol,
    base_finals: Vec<TreeState>,
    /// `root_i` for each rule (the pattern-root state, or `Any` for a
    /// bare-variable left side).
    root: Vec<StateIndex>,
}

impl Universe {
    /// Set-valued right-side evaluation `Run_E(term)` under the current
    /// closure matrix, in postorder. Charges one operation per fixed
    /// transition scanned, per child-membership check, and per closure
    /// membership examined.
    fn run(
        &self,
        term: &Term,
        closure: &[Vec<bool>],
        resources: &mut RelationResources,
        limits: &RelationLimits,
    ) -> RewriteResult<BTreeSet<StateIndex>> {
        let positions = term.positions();
        let mut order: Vec<usize> = (0..positions.len()).collect();
        order.sort_by_key(|&index| core::cmp::Reverse(positions[index].as_slice().len()));
        let mut results: BTreeMap<Path, BTreeSet<StateIndex>> = BTreeMap::new();
        for index in order {
            let path = positions[index].clone();
            let subterm = term.subterm(&path).expect("positions are valid");
            let set: BTreeSet<StateIndex> = match subterm {
                Term::Var(_) => {
                    return Err(RewriteError::UnsupportedPreimage {
                        message: "saturation right side is not ground".into(),
                    });
                }
                Term::Sym(symbol, arguments) => {
                    let child_sets: Vec<&BTreeSet<StateIndex>> = (0..arguments.len())
                        .map(|child| {
                            results
                                .get(&path.child(child))
                                .expect("children are evaluated before their parent")
                        })
                        .collect();
                    let empty: Vec<FixedEntry> = Vec::new();
                    let candidates = self.fixed_by_symbol.get(symbol).unwrap_or(&empty);
                    let mut raw: BTreeSet<StateIndex> = BTreeSet::new();
                    for (children, parent) in candidates {
                        charge_operations(resources, limits, 1)?;
                        let mut matched = true;
                        for (child, required) in children.iter().enumerate() {
                            charge_operations(resources, limits, 1)?;
                            if !child_sets[child].contains(required) {
                                matched = false;
                                break;
                            }
                        }
                        if matched {
                            raw.insert(*parent);
                        }
                    }
                    // Freshly allocated per node per call: bill the
                    // raw set's cells as constraints (review round 6).
                    charge_constraints(resources, limits, raw.len())?;
                    let mut closed: BTreeSet<StateIndex> = BTreeSet::new();
                    for parent in &raw {
                        for (target, reachable) in closure[*parent].iter().enumerate() {
                            charge_operations(resources, limits, 1)?;
                            if *reachable {
                                closed.insert(target);
                            }
                        }
                    }
                    // The closed set is freshly allocated too.
                    charge_constraints(resources, limits, closed.len())?;
                    closed
                }
            };
            results.insert(path, set);
        }
        Ok(results
            .remove(&Path::root())
            .expect("the root is evaluated"))
    }
}

/// Build the fixed state universe and the fixed symbol transitions `D`,
/// run the defensive boundary checks, and preflight the state and
/// transition ceilings with checked arithmetic.
fn build_universe(
    system: &TermSystem,
    language: &TreeAutomaton,
    limits: &RelationLimits,
    automaton_limits: &TreeAutomatonLimits,
    resources: &mut RelationResources,
) -> RewriteResult<Universe> {
    // Defensive internal checks plus bounded term measurement. The class
    // classifier already guarantees left-linearity and ground right sides
    // for the approved classes; any violation here is a classifier bug and
    // must never be silently accommodated.
    let mut occurrences: usize = 0;
    for rule in system.rules() {
        let lhs = rule.lhs();
        let (lhs_nodes, lhs_depth) = measure_term(lhs, limits)?;
        resources.record_term(lhs_nodes, lhs_depth)?;
        if !term_is_linear(lhs) {
            return Err(RewriteError::UnsupportedPreimage {
                message: "saturation requires a linear left side (classifier bug)".into(),
            });
        }
        let rhs = rule.rhs();
        let (rhs_nodes, rhs_depth) = measure_term(rhs, limits)?;
        resources.record_term(rhs_nodes, rhs_depth)?;
        if !term_is_ground(rhs) {
            return Err(RewriteError::UnsupportedPreimage {
                message: "saturation requires a ground right side (classifier bug)".into(),
            });
        }
        charge_operations(resources, limits, lhs.positions().len())?;
        for position in lhs.positions() {
            if matches!(lhs.subterm(&position), Some(Term::Sym(_, _))) {
                occurrences =
                    occurrences
                        .checked_add(1)
                        .ok_or(RewriteError::RelationLimitExceeded {
                            resource: "saturation pattern states",
                            limit: automaton_limits.max_states(),
                        })?;
            }
        }
    }

    let base_count = language.states().len();
    let total_states = base_count
        .checked_add(1)
        .and_then(|count| count.checked_add(occurrences))
        .ok_or(RewriteError::RelationLimitExceeded {
            resource: "saturation state universe",
            limit: automaton_limits.max_states(),
        })?;
    if total_states > automaton_limits.max_states() {
        return Err(RewriteError::RelationLimitExceeded {
            resource: "saturation state universe",
            limit: automaton_limits.max_states(),
        });
    }
    let base_transitions = language.transitions().len();
    let alphabet_size = language.alphabet().len();
    let total_transitions = base_transitions
        .checked_add(alphabet_size)
        .and_then(|count| count.checked_add(occurrences))
        .ok_or(RewriteError::RelationLimitExceeded {
            resource: "saturation fixed transitions",
            limit: automaton_limits.max_transitions(),
        })?;
    if total_transitions > automaton_limits.max_transitions() {
        return Err(RewriteError::RelationLimitExceeded {
            resource: "saturation fixed transitions",
            limit: automaton_limits.max_transitions(),
        });
    }
    charge_operations(resources, limits, total_transitions)?;

    let mut states: Vec<TreeState> = Vec::with_capacity(total_states);
    let mut base_index: BTreeMap<TreeState, StateIndex> = BTreeMap::new();
    for state in language.states() {
        let tagged = TreeState::new(format!("b/{}", state.name().as_str()));
        base_index.insert(state.clone(), states.len());
        states.push(tagged);
    }
    let any_index = states.len();
    let any = TreeState::new("any");
    states.push(any.clone());

    // Assign a Pat state to every non-variable left-side occurrence, in
    // preorder so a child's state exists before its parent transition.
    let mut pattern_index: BTreeMap<(usize, Path), StateIndex> = BTreeMap::new();
    let mut root: Vec<StateIndex> = Vec::with_capacity(system.rules().len());
    for (rule_index, rule) in system.rules().iter().enumerate() {
        let lhs = rule.lhs();
        for position in lhs.positions() {
            if matches!(lhs.subterm(&position), Some(Term::Sym(_, _))) {
                let state = TreeState::new(format!("p/{rule_index}/{}", render_path(&position)));
                pattern_index.insert((rule_index, position.clone()), states.len());
                states.push(state);
            }
        }
        match lhs {
            Term::Sym(_, _) => {
                root.push(pattern_index[&(rule_index, Path::root())]);
            }
            Term::Var(_) => root.push(any_index),
        }
    }

    let mut fixed: Vec<TreeTransition> = Vec::with_capacity(total_transitions);
    let mut fixed_by_symbol: FixedBySymbol = BTreeMap::new();

    // (a) base transitions, all states tagged Base.
    for transition in language.transitions() {
        let children: Vec<StateIndex> = transition
            .children()
            .iter()
            .map(|child| base_index[child])
            .collect();
        let parent = base_index[transition.parent()];
        fixed.push(TreeTransition::new(
            transition.symbol().clone(),
            children
                .iter()
                .map(|index| states[*index].clone())
                .collect(),
            states[parent].clone(),
        ));
        fixed_by_symbol
            .entry(transition.symbol().clone())
            .or_default()
            .push((children, parent));
    }

    // (b) the universal state recognizes every ground term.
    for ranked in language.alphabet() {
        let arity = usize::from(ranked.arity());
        let children = vec![any_index; arity];
        fixed.push(TreeTransition::new(
            ranked.symbol().clone(),
            vec![any.clone(); arity],
            any.clone(),
        ));
        fixed_by_symbol
            .entry(ranked.symbol().clone())
            .or_default()
            .push((children, any_index));
    }

    // (c) the literal pattern transitions.
    for (rule_index, rule) in system.rules().iter().enumerate() {
        let lhs = rule.lhs();
        for position in lhs.positions() {
            let Some(Term::Sym(symbol, arguments)) = lhs.subterm(&position) else {
                continue;
            };
            let parent = pattern_index[&(rule_index, position.clone())];
            let children: Vec<StateIndex> = arguments
                .iter()
                .enumerate()
                .map(|(child, argument)| {
                    if argument.is_var() {
                        any_index
                    } else {
                        pattern_index[&(rule_index, position.child(child))]
                    }
                })
                .collect();
            fixed.push(TreeTransition::new(
                symbol.clone(),
                children
                    .iter()
                    .map(|index| states[*index].clone())
                    .collect(),
                states[parent].clone(),
            ));
            fixed_by_symbol
                .entry(symbol.clone())
                .or_default()
                .push((children, parent));
        }
    }

    let base_finals: Vec<TreeState> = language
        .finals()
        .iter()
        .map(|state| states[base_index[state]].clone())
        .collect();

    Ok(Universe {
        alphabet: language.alphabet().to_vec(),
        states,
        fixed,
        fixed_by_symbol,
        base_finals,
        root,
    })
}

/// Synchronous epsilon-edges fixpoint. Returns the deduplicated edge set
/// and the number of productive rounds.
fn saturate(
    universe: &Universe,
    system: &TermSystem,
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<(Vec<(StateIndex, StateIndex)>, usize)> {
    let n = universe.states.len();
    let rule_count = system.rules().len();
    let n_squared = n
        .checked_mul(n)
        .ok_or(RewriteError::RelationLimitExceeded {
            resource: "saturation closure cells",
            limit: limits.max_constraints(),
        })?;
    let n_cubed = n_squared
        .checked_mul(n)
        .ok_or(RewriteError::RelationLimitExceeded {
            resource: "saturation closure operations",
            limit: limits.max_operations(),
        })?;
    let rule_edges = rule_count
        .checked_mul(n)
        .ok_or(RewriteError::RelationLimitExceeded {
            resource: "saturation rule edges",
            limit: limits.max_operations(),
        })?;
    let max_productive = rule_edges.min(n_squared);

    // Reserve ALL relation storage once (checked before allocation):
    // the N² closure cells, the right-side evaluation sets (H·N cells,
    // H = total rhs nodes), and the edge-membership set (at most
    // min(m·N, N²) pairs) — review round 1's storage accounting.
    let rhs_nodes: usize = system
        .rules()
        .iter()
        .try_fold(0usize, |acc, rule| {
            acc.checked_add(rule.rhs().positions().len())
        })
        .ok_or(RewriteError::RelationLimitExceeded {
            resource: "saturation evaluation storage",
            limit: limits.max_constraints(),
        })?;
    let evaluation_cells = rhs_nodes
        .checked_mul(n)
        .ok_or(RewriteError::RelationLimitExceeded {
            resource: "saturation evaluation storage",
            limit: limits.max_constraints(),
        })?;
    charge_constraints(resources, limits, n_squared)?;
    charge_constraints(resources, limits, evaluation_cells)?;
    charge_constraints(resources, limits, max_productive)?;
    let mut matrix: Vec<Vec<bool>> = vec![vec![false; n]; n];
    let mut edges: BTreeSet<(StateIndex, StateIndex)> = BTreeSet::new();
    let mut productive = 0usize;
    let mut rounds = 0usize;
    loop {
        rounds = rounds
            .checked_add(1)
            .ok_or(RewriteError::RelationLimitExceeded {
                resource: "saturation rounds",
                limit: limits.max_operations(),
            })?;
        if rounds
            > max_productive
                .checked_add(1)
                .ok_or(RewriteError::RelationLimitExceeded {
                    resource: "saturation rounds",
                    limit: limits.max_operations(),
                })?
        {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "saturation rounds",
                limit: limits.max_operations(),
            });
        }
        // At least one charged operation per round.
        charge_operations(resources, limits, 1)?;
        // The right-side evaluation buffers are allocated fresh every
        // round — charge their capacity per round, not once (review
        // round 2's cumulative storage accounting).
        charge_constraints(resources, limits, evaluation_cells)?;
        compute_closure(&mut matrix, n, n_cubed, &edges, resources, limits)?;
        let mut added = false;
        for (rule_index, rule) in system.rules().iter().enumerate() {
            let reached = universe.run(rule.rhs(), &matrix, resources, limits)?;
            for target in reached {
                charge_operations(resources, limits, 1)?;
                if edges.insert((universe.root[rule_index], target)) {
                    added = true;
                }
            }
        }
        if !added {
            break;
        }
        productive = productive
            .checked_add(1)
            .ok_or(RewriteError::RelationLimitExceeded {
                resource: "saturation productive rounds",
                limit: limits.max_operations(),
            })?;
    }
    Ok((edges.into_iter().collect(), productive))
}

/// Metered reflexive-transitive closure of the current edge set, in place.
#[allow(clippy::needless_range_loop)]
fn compute_closure(
    matrix: &mut [Vec<bool>],
    n: usize,
    n_cubed: usize,
    edges: &BTreeSet<(StateIndex, StateIndex)>,
    resources: &mut RelationResources,
    limits: &RelationLimits,
) -> RewriteResult<()> {
    for row in matrix.iter_mut() {
        for value in row.iter_mut() {
            *value = false;
        }
    }
    for (index, row) in matrix.iter_mut().enumerate() {
        row[index] = true;
    }
    for &(from, to) in edges {
        matrix[from][to] = true;
    }
    charge_operations(resources, limits, n_cubed)?;
    for k in 0..n {
        for i in 0..n {
            if matrix[i][k] {
                for j in 0..n {
                    if matrix[k][j] {
                        matrix[i][j] = true;
                    }
                }
            }
        }
    }
    Ok(())
}

/// Charge `count` operations after a checked preflight against the
/// configured ceiling (the resource counters themselves saturate).
fn charge_operations(
    resources: &mut RelationResources,
    limits: &RelationLimits,
    count: usize,
) -> RewriteResult<()> {
    let limit = limits.max_operations();
    let next =
        resources
            .operations()
            .checked_add(count)
            .ok_or(RewriteError::RelationLimitExceeded {
                resource: "saturation operations",
                limit,
            })?;
    if next > limit {
        return Err(RewriteError::RelationLimitExceeded {
            resource: "saturation operations",
            limit,
        });
    }
    resources.record_operations(count)
}

/// Charge `count` reserved relation-storage cells after a checked
/// preflight against the configured ceiling.
fn charge_constraints(
    resources: &mut RelationResources,
    limits: &RelationLimits,
    count: usize,
) -> RewriteResult<()> {
    let limit = limits.max_constraints();
    let next =
        resources
            .constraints()
            .checked_add(count)
            .ok_or(RewriteError::RelationLimitExceeded {
                resource: "saturation constraints",
                limit,
            })?;
    if next > limit {
        return Err(RewriteError::RelationLimitExceeded {
            resource: "saturation constraints",
            limit,
        });
    }
    resources.record_constraints(count)
}

/// Node count and edge-depth of a term, via an explicit worklist (no
/// recursive descent). Variables are permitted (left-side measurement).
fn measure_term(term: &Term, limits: &RelationLimits) -> RewriteResult<(usize, usize)> {
    let mut nodes = 0usize;
    let mut depth = 0usize;
    let mut stack: Vec<(&Term, usize)> = vec![(term, 0)];
    while let Some((node, level)) = stack.pop() {
        nodes = nodes
            .checked_add(1)
            .ok_or(RewriteError::RelationLimitExceeded {
                resource: "term nodes",
                limit: limits.max_term_nodes(),
            })?;
        if nodes > limits.max_term_nodes() {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "term nodes",
                limit: limits.max_term_nodes(),
            });
        }
        depth = depth.max(level);
        if depth > limits.max_term_depth() {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "term depth",
                limit: limits.max_term_depth(),
            });
        }
        if let Term::Sym(_, arguments) = node {
            for argument in arguments {
                stack.push((argument, level + 1));
            }
        }
    }
    Ok((nodes, depth))
}

/// Whether the term contains no variables.
fn term_is_ground(term: &Term) -> bool {
    match term {
        Term::Var(_) => false,
        Term::Sym(_, arguments) => arguments.iter().all(term_is_ground),
    }
}

/// Whether every variable occurs at most once.
fn term_is_linear(term: &Term) -> bool {
    let mut counts: BTreeMap<Variable, usize> = BTreeMap::new();
    count_variables(term, &mut counts);
    counts.values().all(|count| *count <= 1)
}

fn count_variables(term: &Term, counts: &mut BTreeMap<Variable, usize>) {
    match term {
        Term::Var(variable) => *counts.entry(variable.clone()).or_insert(0) += 1,
        Term::Sym(_, arguments) => {
            for argument in arguments {
                count_variables(argument, counts);
            }
        }
    }
}

/// Render a skeleton position as child indices joined by `.` (the root is
/// the empty string).
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trs::Rule;

    fn a() -> Term {
        Term::constant("a")
    }

    fn b() -> Term {
        Term::constant("b")
    }

    fn g(inner: Term) -> Term {
        Term::sym("g", [inner])
    }

    /// A partial singleton automaton accepting exactly `terms` over the
    /// ranked alphabet `alphabet`, using a fresh state per node occurrence.
    fn partial_singleton(alphabet: &[(&str, u16)], terms: &[Term]) -> TreeAutomaton {
        let mut transitions: Vec<TreeTransition> = Vec::new();
        let mut finals: Vec<TreeState> = Vec::new();
        let mut counter = 0usize;
        for term in terms {
            let mut states_by_path: BTreeMap<Vec<usize>, TreeState> = BTreeMap::new();
            let mut positions = term.positions();
            positions.sort_by_key(|path| core::cmp::Reverse(path.as_slice().len()));
            for position in &positions {
                let Term::Sym(symbol, arguments) = term.subterm(position).expect("valid") else {
                    panic!("partial_singleton requires ground terms");
                };
                counter += 1;
                let state = TreeState::new(format!("s{counter}"));
                let children: Vec<TreeState> = (0..arguments.len())
                    .map(|child| {
                        states_by_path
                            .get(position.child(child).as_slice())
                            .cloned()
                            .expect("children precede parents")
                    })
                    .collect();
                transitions.push(TreeTransition::new(symbol.clone(), children, state.clone()));
                states_by_path.insert(position.as_slice().to_vec(), state);
            }
            finals.push(
                states_by_path
                    .get(&Vec::<usize>::new())
                    .cloned()
                    .expect("the root is evaluated"),
            );
        }
        let mut state_set: BTreeSet<TreeState> = BTreeSet::new();
        for transition in &transitions {
            state_set.insert(transition.parent().clone());
            state_set.extend(transition.children().iter().cloned());
        }
        TreeAutomaton::new(
            alphabet
                .iter()
                .map(|(name, arity)| RankedSymbol::new(Symbol::new(*name), *arity))
                .collect(),
            state_set.into_iter().collect(),
            transitions,
            finals,
            TreeAutomatonLimits::default(),
        )
        .expect("fixture automaton is valid")
    }

    #[test]
    fn state_universe_size_is_n_plus_one_plus_h() {
        let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
        let language = partial_singleton(&[("a", 0), ("g", 1)], &[g(g(a()))]);
        let mut resources = RelationResources::new(&RelationLimits::default());
        let universe = build_universe(
            &system,
            &language,
            &RelationLimits::default(),
            &TreeAutomatonLimits::default(),
            &mut resources,
        )
        .expect("universe builds");
        let n = language.states().len();
        // h = 1: the single non-variable occurrence (the root constant `a`).
        assert_eq!(universe.states.len(), n + 1 + 1);
    }

    #[test]
    fn fixed_transition_count_is_d_plus_z_plus_h() {
        let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
        let language = partial_singleton(&[("a", 0), ("g", 1)], &[g(g(a()))]);
        let mut resources = RelationResources::new(&RelationLimits::default());
        let universe = build_universe(
            &system,
            &language,
            &RelationLimits::default(),
            &TreeAutomatonLimits::default(),
            &mut resources,
        )
        .expect("universe builds");
        let d = language.transitions().len();
        let z = language.alphabet().len();
        assert_eq!(universe.fixed.len(), d + z + 1);
    }

    #[test]
    fn run_evaluation_is_set_valued_and_includes_pattern_targets() {
        // R = {g(a) -> b}; L = {b} over {a/0, b/0, g/1}.
        let system = TermSystem::new(vec![Rule::new(g(a()), b()).unwrap()]);
        let language = partial_singleton(&[("a", 0), ("b", 0), ("g", 1)], &[b()]);
        let mut resources = RelationResources::new(&RelationLimits::default());
        let universe = build_universe(
            &system,
            &language,
            &RelationLimits::default(),
            &TreeAutomatonLimits::default(),
            &mut resources,
        )
        .expect("universe builds");
        let n = universe.states.len();
        // Hand-built closure: the single base state can epsilon-reach the
        // `a`-pattern state at path 0.
        let base_state = 0usize; // b/s1
        let a_pattern = universe
            .states
            .iter()
            .position(|state| state.name().as_str() == "p/0/0")
            .expect("the a-pattern state exists");
        let mut closure = vec![vec![false; n]; n];
        for (index, row) in closure.iter_mut().enumerate() {
            row[index] = true;
        }
        closure[base_state][a_pattern] = true;

        let evaluated = universe
            .run(&b(), &closure, &mut resources, &RelationLimits::default())
            .expect("evaluation succeeds");
        assert!(
            evaluated.contains(&a_pattern),
            "Run_E must include pattern states reachable through the closure"
        );
        // A constant with a pattern transition is a set, not a scalar.
        assert!(evaluated.len() >= 2, "evaluation is set-valued");
    }

    #[test]
    fn completed_p2_needs_three_productive_rounds() {
        // R = {a -> g(a)}, L = {g(g(a))}. With the completed presentation
        // the sink contributes a third productive round (design §5).
        let system = TermSystem::new(vec![Rule::new(a(), g(a())).unwrap()]);
        let language = partial_singleton(&[("a", 0), ("g", 1)], &[g(g(a()))])
            .determinize(&TreeAutomatonLimits::default())
            .expect("fixture determinizes")
            .completed()
            .expect("fixture completes");
        let mut resources = RelationResources::new(&RelationLimits::default());
        let universe = build_universe(
            &system,
            &language,
            &RelationLimits::default(),
            &TreeAutomatonLimits::default(),
            &mut resources,
        )
        .expect("universe builds");
        let (edges, productive) = saturate(
            &universe,
            &system,
            &RelationLimits::default(),
            &mut resources,
        )
        .expect("saturation converges");
        assert_eq!(productive, 3, "P2 needs three productive rounds");
        assert!(!edges.is_empty());
    }
}
