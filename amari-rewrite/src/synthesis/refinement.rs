// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic negative-example specialization (Task 36).
//!
//! [`infer_rules`](crate::synthesis::infer_rules) only accepts a
//! positive-example rule when it happens to reproduce no supplied
//! negative. This module replaces that all-or-nothing check with a
//! bounded specialization search: positives that share a covering rule
//! are partitioned along discriminating positions until every returned
//! rule covers at least one positive example and no supplied negative
//! example.
//!
//! # Coverage
//!
//! A rule covers an example `(lhs, rhs)` exactly when the rule's
//! left-hand pattern matches `lhs` and applying the rule at the root
//! yields a term structurally equal to `rhs`. Application uses
//! one-level matcher provenance — bindings are verbatim example
//! fragments and are never chain-resolved — so example variable names
//! cannot be captured by rule variables (Task 31 discipline). Replay is
//! purely virtual: bindings are BORROWED from the example, the
//! prospective shape of the applied right-hand side is accounted
//! BEFORE any term is materialized (Task 34 doctrine), and the
//! virtually substituted template is compared node by node against the
//! example right-hand side. Nothing is cloned to replay a rule.
//!
//! # Admission
//!
//! Raw inputs are validated while still borrowed, iteratively, before
//! any sort, dedup, comparison, or variable collection runs: node and
//! depth ceilings are checked with an explicit stack and the
//! cumulative operation budget is charged per visited node. Deep or
//! huge terms therefore surface as typed errors instead of aborting in
//! a recursive clone or comparison (PR #320 round 1 P1).
//!
//! # Specialization
//!
//! 1. Both example lists are canonicalized into structural order and
//!    deduplicated, all from borrowed references. The canonical order
//!    is computed by ranking every unordered pair once, so the
//!    comparison charge is a property of the example SET rather than
//!    of the caller's list order; no example is cloned, and a pair
//!    appearing as both positive and negative is a typed
//!    [`RewriteError::InvalidRule`]. The examples ceiling counts the
//!    deduped examples; the raw duplicate stream stays bounded by the
//!    operation budget charged during admission and canonicalization.
//! 2. Each partition of positives is generalized with the shared
//!    disagreement table used by
//!    [`infer_rule`](crate::synthesis::infer_rule), reserving every
//!    example variable name so generated `_I*` variables cannot collide
//!    with example variables (the #303 interaction). Partition
//!    examples and the reserved-variable set are borrowed, never
//!    cloned, and structural equality and cloning charge per visited
//!    node.
//! 3. A candidate that covers a negative (or fails to replay against
//!    its own positives) is split at the first discriminating position
//!    in canonical preorder over the candidate's left-hand side and
//!    then its right-hand side. A position discriminates when the
//!    positives' subterms there fall into at least two classes: a
//!    sub-rule built from one class can only cover a negative whose
//!    subterm at that position is in the same class. Grouping compares
//!    borrowed subterms with per-node charging; subtrees are never
//!    cloned.
//! 4. Partitions are processed in lexicographic order, so candidate,
//!    partition, rule, and coverage order are identical across runs
//!    and across input permutations. No random state is seeded or
//!    consulted.
//! 5. A refined outcome covers every deduped positive; when a positive
//!    cannot be separated, the outcome is [`InconclusiveReason`] rather
//!    than a rule that covers a negative. Inconclusive never claims
//!    that no rule exists.
//!
//! # Limits
//!
//! Example counts, candidates, partitions, and the per-term node/depth
//! and cumulative operation ceilings are enforced by
//! [`SpecializationConfig`] and a single [`RelationResources`] budget
//! covering the whole run. One operation unit is charged per visited
//! term node, per compared node pair, per generalized node pair, per
//! partition member and position examined while splitting, per binding
//! lookup, and per replay/equality node. Bulk work is never funded by
//! a single unit: equality, cloning, canonical ordering, grouping, and
//! replay are all charged before the work they meter. Every violation
//! is a typed error; nothing is truncated silently and nothing panics
//! (PR #320 round 2).

use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    vec,
    vec::Vec,
};
use core::cmp::Ordering;

use crate::{
    relation::{RelationLimits, RelationResources},
    trs::{Rule, Term, Variable},
    Path, RewriteError, RewriteResult,
};

/// Caller-tightenable specialization limits.
///
/// The associated constants are the hard ceilings; [`Self::new`] only
/// accepts values inside `(0, ceiling]`. Term size and operation
/// ceilings come from the enclosed [`RelationLimits`], which is
/// revalidated at construction, on deserialization, and at every
/// entry point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
pub struct SpecializationConfig {
    max_examples: usize,
    max_candidates: usize,
    max_partitions: usize,
    limits: RelationLimits,
}

#[cfg(feature = "serialize")]
impl<'de> serde::Deserialize<'de> for SpecializationConfig {
    /// Deserialization routes through the checked constructor, which
    /// also revalidates the enclosed [`RelationLimits`]: its own
    /// derive bypasses [`RelationLimits::new`], so a raw
    /// `{"max_examples":99999,...}` or a zero/above-ceiling nested
    /// limit never becomes a usable config (Task 32 precedent;
    /// PR #320 round 1 P2).
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            max_examples: usize,
            max_candidates: usize,
            max_partitions: usize,
            limits: RelationLimits,
        }
        let raw = <Raw as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(
            raw.max_examples,
            raw.max_candidates,
            raw.max_partitions,
            raw.limits,
        )
        .map_err(serde::de::Error::custom)
    }
}

impl SpecializationConfig {
    /// Hard ceiling for the total number of deduped examples.
    pub const MAX_EXAMPLES: usize = 4_096;
    /// Hard ceiling for candidate rules built in one run.
    pub const MAX_CANDIDATES: usize = 8_192;
    /// Hard ceiling for positive partitions processed in one run.
    pub const MAX_PARTITIONS: usize = 8_192;

    /// Construct tightened limits. Zero or above-ceiling values are
    /// rejected: limits bound work, they do not disable it.
    pub fn new(
        max_examples: usize,
        max_candidates: usize,
        max_partitions: usize,
        limits: RelationLimits,
    ) -> RewriteResult<Self> {
        let config = Self {
            max_examples,
            max_candidates,
            max_partitions,
            limits,
        };
        config.revalidate()?;
        Ok(config)
    }

    /// Revalidate every invariant, including the enclosed limits.
    ///
    /// `RelationLimits` deserializes through its derived impl, which
    /// bypasses [`RelationLimits::new`]; re-running the checked
    /// constructor here closes that route for a config received from
    /// anywhere.
    fn revalidate(&self) -> RewriteResult<()> {
        self.check("examples", self.max_examples, Self::MAX_EXAMPLES)?;
        self.check("candidates", self.max_candidates, Self::MAX_CANDIDATES)?;
        self.check("partitions", self.max_partitions, Self::MAX_PARTITIONS)?;
        let _limits = RelationLimits::new(
            self.limits.max_term_nodes(),
            self.limits.max_term_depth(),
            self.limits.max_constraints(),
            self.limits.max_operations(),
        )?;
        Ok(())
    }

    fn check(&self, resource: &'static str, value: usize, ceiling: usize) -> RewriteResult<()> {
        let _ = self;
        if value == 0 || value > ceiling {
            return Err(RewriteError::InvalidLimit {
                resource,
                value,
                ceiling,
            });
        }
        Ok(())
    }

    /// Maximum total number of deduped examples.
    pub fn max_examples(&self) -> usize {
        self.max_examples
    }

    /// Maximum number of candidate rules.
    pub fn max_candidates(&self) -> usize {
        self.max_candidates
    }

    /// Maximum number of processed positive partitions.
    pub fn max_partitions(&self) -> usize {
        self.max_partitions
    }

    /// The enclosed relation limits.
    pub fn limits(&self) -> &RelationLimits {
        &self.limits
    }
}

impl Default for SpecializationConfig {
    /// The default profile is the fixed ceiling profile.
    fn default() -> Self {
        Self {
            max_examples: Self::MAX_EXAMPLES,
            max_candidates: Self::MAX_CANDIDATES,
            max_partitions: Self::MAX_PARTITIONS,
            limits: RelationLimits::default(),
        }
    }
}

/// Why no refined rule set was returned.
///
/// An inconclusive outcome reports that this bounded search did not
/// separate the examples. It never asserts that no separating rule
/// exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InconclusiveReason {
    /// Every candidate covering a remaining positive also covers at
    /// least one negative, and no finer partition separates them.
    Inseparable,
    /// A remaining partition cannot be generalized into a checked
    /// rule: its right-hand side would introduce a variable that does
    /// not occur on the left, or the candidate failed positive replay.
    Unrepresentable,
}

/// Coverage of one refined rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleCoverage {
    rule_index: usize,
    positives: Vec<usize>,
    negatives: Vec<usize>,
}

impl RuleCoverage {
    /// Index of the rule in the refined rule list.
    pub fn rule_index(&self) -> usize {
        self.rule_index
    }

    /// Indices (in canonical deduped order) of covered positives.
    pub fn positives(&self) -> &[usize] {
        &self.positives
    }

    /// Indices (in canonical deduped order) of covered negatives.
    ///
    /// Always empty for a refined outcome.
    pub fn negatives(&self) -> &[usize] {
        &self.negatives
    }
}

/// Evidence that a refined rule set was validated against every
/// example.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoverageReport {
    positives: usize,
    negatives: usize,
    rules: Vec<RuleCoverage>,
}

impl CoverageReport {
    /// Number of deduped positive examples supplied.
    pub fn positives(&self) -> usize {
        self.positives
    }

    /// Number of deduped negative examples supplied.
    pub fn negatives(&self) -> usize {
        self.negatives
    }

    /// Per-rule coverage, aligned with the refined rule list.
    pub fn rules(&self) -> &[RuleCoverage] {
        &self.rules
    }
}

/// The typed result of negative-example specialization.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecializationOutcome {
    /// Rules that together cover every deduped positive and no
    /// supplied negative, in canonical partition order.
    Refined {
        /// The refined rules.
        rules: Vec<Rule>,
        /// Replayed coverage evidence for every rule.
        coverage: CoverageReport,
    },
    /// No refined rule set was found within the configured limits.
    Inconclusive {
        /// Why the search stopped without a refined rule set.
        reason: InconclusiveReason,
    },
}

/// Specialize a rule set from positive and negative example pairs.
///
/// Returns [`SpecializationOutcome::Refined`] only when every deduped
/// positive is covered by at least one returned rule and every
/// returned rule covers no supplied negative. Otherwise the bounded
/// search reports [`SpecializationOutcome::Inconclusive`] or a typed
/// limit error.
///
/// Examples are canonicalized into structural order and deduplicated,
/// so the coverage indices and the rule order do not depend on caller
/// order — and, because the canonicalization charge is order
/// independent, neither does a tightened-budget outcome.
///
/// # Errors
///
/// - [`RewriteError::InvalidRule`] when there are no positive
///   examples, or when one example pair is supplied as both positive
///   and negative.
/// - [`RewriteError::InvalidLimit`] when the configuration (including
///   the enclosed relation limits) violates its invariants.
/// - [`RewriteError::RelationLimitExceeded`] when the example,
///   candidate, partition, term node/depth, or operation ceilings are
///   exceeded.
pub fn specialize_rules(
    positives: &[(Term, Term)],
    negatives: &[(Term, Term)],
    config: &SpecializationConfig,
) -> RewriteResult<SpecializationOutcome> {
    let limits = *config.limits();
    let mut budget = RelationResources::new(&limits);
    specialize_rules_with(positives, negatives, config, &mut budget)
}

/// Specialize under a caller-supplied budget.
///
/// Identical to [`specialize_rules`] except that every operation is
/// charged to `budget` instead of a freshly opened budget. Composite
/// entry points (inverse synthesis, Task 37) use this so one run has
/// exactly one operations total: nested inference is never funded by
/// a silently separate allowance. The budget is charged BEFORE the
/// work it meters, so exhaustion is a typed
/// [`RewriteError::RelationLimitExceeded`] at the same point it would
/// be with a private budget, just sooner on a shared one.
pub(crate) fn specialize_rules_with(
    positives: &[(Term, Term)],
    negatives: &[(Term, Term)],
    config: &SpecializationConfig,
    budget: &mut RelationResources,
) -> RewriteResult<SpecializationOutcome> {
    // Revalidate the caller's config: deserialization and construction
    // are checked, but a config is a public value and the entry point
    // is the last line of defence (PR #320 round 1 P2).
    config.revalidate()?;
    if positives.is_empty() {
        return Err(RewriteError::InvalidRule {
            message: String::from("cannot specialize without positive examples"),
        });
    }

    let limits = *config.limits();
    let resources: &mut RelationResources = &mut *budget;

    // 1. Raw admission over the BORROWED inputs. Nothing below this
    // loop may sort, dedup, compare, or collect variables from an
    // unvalidated term: a deep term must fail typed here instead of
    // aborting in a recursive clone/sort (PR #320 round 1 P1).
    for example in positives.iter().chain(negatives.iter()) {
        resources.record_operations(1)?;
        admit_term(&example.0, &limits, resources)?;
        admit_term(&example.1, &limits, resources)?;
    }

    // 2. Canonicalize borrowed references (charged comparisons).
    let positives = canonical_examples(positives, resources)?;
    let negatives = canonical_examples(negatives, resources)?;

    // 3. Deduped-count ceiling. The raw duplicate stream was bounded
    // by the admission and canonicalization charges above.
    let total = positives.len().saturating_add(negatives.len());
    if total > config.max_examples() {
        return Err(RewriteError::RelationLimitExceeded {
            resource: "examples",
            limit: config.max_examples(),
        });
    }

    // 4. Conflicting duplicates, with charged comparisons.
    for pair in &positives {
        if charged_contains(&negatives, pair, resources)? {
            return Err(RewriteError::InvalidRule {
                message: String::from("example pair is both positive and negative"),
            });
        }
    }

    // 5. Reserve every example variable name (charged traversal).
    let reserved = example_variables(&positives, &negatives, resources)?;

    let mut pending = BTreeSet::new();
    pending.insert((0..positives.len()).collect::<Vec<usize>>());
    let mut resolved: BTreeMap<Vec<usize>, Rule> = BTreeMap::new();
    let mut unresolved: Vec<(Vec<usize>, InconclusiveReason)> = Vec::new();
    let mut partitions = 0usize;
    let mut candidates = 0usize;

    while let Some(partition) = pending.pop_first() {
        partitions = partitions.saturating_add(1);
        if partitions > config.max_partitions() {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "partitions",
                limit: config.max_partitions(),
            });
        }

        let candidate = build_candidate(
            &partition,
            &positives,
            &reserved,
            config.max_candidates(),
            &mut candidates,
            &limits,
            resources,
        )?;

        let mut retained = None;
        if let Some(rule) = candidate.as_ref() {
            if covers_partition(rule, &partition, &positives, &limits, resources)?
                && !covers_any(rule, &negatives, &limits, resources)?
            {
                // Retention copies the accepted rule into the result;
                // charge both sides per node BEFORE the clone is
                // allocated (PR #320 round 4 P2c).
                let _lhs_shape = admit_term(rule.lhs(), &limits, resources)?;
                let _rhs_shape = admit_term(rule.rhs(), &limits, resources)?;
                retained = Some(rule.clone());
            }
        }
        if let Some(rule) = retained {
            resolved.insert(partition, rule);
            continue;
        }

        let positions = discriminating_positions(
            candidate.as_ref(),
            &partition,
            &positives,
            &limits,
            resources,
        )?;
        match first_split(&positions, &partition, &positives, resources)? {
            Some(groups) => pending.extend(groups),
            None => {
                let reason = if candidate.is_some() {
                    InconclusiveReason::Inseparable
                } else {
                    InconclusiveReason::Unrepresentable
                };
                unresolved.push((partition, reason));
            }
        }
    }

    if let Some((_, reason)) = unresolved
        .into_iter()
        .min_by(|left, right| left.0.cmp(&right.0))
    {
        return Ok(SpecializationOutcome::Inconclusive { reason });
    }
    finish_refined(resolved, &positives, &negatives, &limits, resources)
}

/// Replay every refined rule against every example and package the
/// outcome. Any rule that covers a negative — impossible by the
/// per-partition checks — is reported as a typed error instead of
/// being returned.
fn finish_refined(
    resolved: BTreeMap<Vec<usize>, Rule>,
    positives: &[&(Term, Term)],
    negatives: &[&(Term, Term)],
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<SpecializationOutcome> {
    let rules: Vec<Rule> = resolved.into_values().collect();
    let mut entries = Vec::with_capacity(rules.len());
    for (index, rule) in rules.iter().enumerate() {
        let mut covered_positives = Vec::new();
        for (positive_index, example) in positives.iter().copied().enumerate() {
            if covers(rule, example, limits, resources)? {
                covered_positives.push(positive_index);
            }
        }
        let mut covered_negatives = Vec::new();
        for (negative_index, example) in negatives.iter().copied().enumerate() {
            if covers(rule, example, limits, resources)? {
                covered_negatives.push(negative_index);
            }
        }
        if !covered_negatives.is_empty() {
            return Err(RewriteError::InvalidRule {
                message: String::from("refined rule covers a negative example"),
            });
        }
        entries.push(RuleCoverage {
            rule_index: index,
            positives: covered_positives,
            negatives: covered_negatives,
        });
    }
    let coverage = CoverageReport {
        positives: positives.len(),
        negatives: negatives.len(),
        rules: entries,
    };
    Ok(SpecializationOutcome::Refined { rules, coverage })
}

/// Charge one unit, enforce the per-term node and depth ceilings, and
/// return `(nodes, depth)` (leaf depth one) for a borrowed term.
///
/// Iterative with an explicit stack: the walk never depends on the
/// call stack, so deeply nested input is rejected before any recursive
/// clone or comparison can run. The frontier is bounded before
/// children are enqueued, so an adversarially wide term fails typed
/// instead of allocating the whole frontier.
pub(crate) fn admit_term(
    term: &Term,
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<(usize, usize)> {
    let mut nodes = 0usize;
    let mut depth = 0usize;
    let mut stack = vec![(term, 1usize)];
    while let Some((current, current_depth)) = stack.pop() {
        // Charge BEFORE inspecting the node or enqueueing its
        // children.
        resources.record_operations(1)?;
        nodes = nodes.saturating_add(1);
        depth = depth.max(current_depth);
        if nodes > limits.max_term_nodes() {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "term nodes",
                limit: limits.max_term_nodes(),
            });
        }
        if depth > limits.max_term_depth() {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "term depth",
                limit: limits.max_term_depth(),
            });
        }
        if let Term::Sym(_, args) = current {
            if args.is_empty() {
                continue;
            }
            let child_depth = current_depth.saturating_add(1);
            let prospective = stack.len().saturating_add(args.len());
            if prospective > limits.max_term_nodes() {
                return Err(RewriteError::RelationLimitExceeded {
                    resource: "term nodes",
                    limit: limits.max_term_nodes(),
                });
            }
            if child_depth > limits.max_term_depth() {
                return Err(RewriteError::RelationLimitExceeded {
                    resource: "term depth",
                    limit: limits.max_term_depth(),
                });
            }
            for arg in args {
                stack.push((arg, child_depth));
            }
        }
    }
    Ok((nodes, depth))
}

/// Compare two borrowed terms in their structural order, charging one
/// unit per compared node pair BEFORE the comparison.
///
/// The order is exactly the derived `Ord` for [`Term`] (variable
/// before symbol, symbol name, then lexicographic arguments with
/// length as the final tie-break), so charged ordering and charged
/// binary search agree with the sort that produced the list.
fn charged_compare(
    left: &Term,
    right: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<Ordering> {
    resources.record_operations(1)?;
    match (left, right) {
        (Term::Var(left_var), Term::Var(right_var)) => Ok(left_var.cmp(right_var)),
        (Term::Var(_), Term::Sym(_, _)) => Ok(Ordering::Less),
        (Term::Sym(_, _), Term::Var(_)) => Ok(Ordering::Greater),
        (Term::Sym(left_symbol, left_args), Term::Sym(right_symbol, right_args)) => {
            let symbol_order = left_symbol.cmp(right_symbol);
            if symbol_order != Ordering::Equal {
                return Ok(symbol_order);
            }
            for (left_arg, right_arg) in left_args.iter().zip(right_args) {
                let order = charged_compare(left_arg, right_arg, resources)?;
                if order != Ordering::Equal {
                    return Ok(order);
                }
            }
            Ok(left_args.len().cmp(&right_args.len()))
        }
    }
}

/// Charged structural order for example pairs: left-hand side first,
/// then right-hand side.
fn charged_pair_compare(
    left: &(Term, Term),
    right: &(Term, Term),
    resources: &mut RelationResources,
) -> RewriteResult<Ordering> {
    resources.record_operations(1)?;
    let order = charged_compare(&left.0, &right.0, resources)?;
    if order != Ordering::Equal {
        return Ok(order);
    }
    charged_compare(&left.1, &right.1, resources)
}

/// Charged binary search over a canonically sorted example list.
fn charged_contains(
    sorted: &[&(Term, Term)],
    needle: &(Term, Term),
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    let mut low = 0usize;
    let mut high = sorted.len();
    while low < high {
        let middle = low + (high - low) / 2;
        match charged_pair_compare(sorted[middle], needle, resources)? {
            Ordering::Less => low = middle + 1,
            Ordering::Greater => high = middle,
            Ordering::Equal => return Ok(true),
        }
    }
    Ok(false)
}

/// Order and deduplicate borrowed example references.
///
/// Both the boundary-bound allocation (`len` units, charged first), the
/// structural comparisons, and the placement pass are metered; no
/// example is cloned. The caller must have admitted the underlying
/// terms first, so the comparisons are bounded by the validated node
/// and depth ceilings.
///
/// The comparison SCHEDULE is a property of the example SET, not of the
/// caller's list order: every unordered pair of examples is compared
/// exactly once, and an example's rank is the number of strictly
/// smaller examples. A comparison-based sort would perform an
/// order-dependent number of comparisons and swaps — insertion sort on
/// the same five examples needs 4..10 comparisons — so at a tightened
/// budget one permutation exhausted where another succeeded (322 vs
/// 337 for specialization, 612 vs 642 for inverse synthesis at the
/// reviewer's budgets; PR #338 closeout P2). Charging every pair once
/// costs the same units for every order of the same set, so bounded
/// outcomes are permutation-invariant.
///
/// Exhaustion anywhere in this function is the typed
/// [`RewriteError::RelationLimitExceeded`] from the exact comparison
/// the budget could not fund. No ordering is ever fabricated: a
/// comparator that keeps returning equalities after exhaustion made
/// std's sort validate a violated total order and PANIC (PR #338
/// closeout P1), so the exhausted comparison aborts instead.
pub(crate) fn canonical_examples<'a>(
    examples: &'a [(Term, Term)],
    resources: &mut RelationResources,
) -> RewriteResult<Vec<&'a (Term, Term)>> {
    let count = examples.len();
    resources.record_operations(count)?;

    let mut ranks = vec![0usize; count];
    for left in 0..count {
        for right in (left + 1)..count {
            match charged_pair_compare(&examples[left], &examples[right], resources)? {
                Ordering::Less => ranks[right] = ranks[right].saturating_add(1),
                Ordering::Greater => ranks[left] = ranks[left].saturating_add(1),
                // Structurally equal examples share a rank. They are
                // interchangeable, so their relative order cannot change
                // the canonical content, and the dedup below collapses
                // them to one entry.
                Ordering::Equal => {}
            }
        }
    }

    // Counting placement over the content-determined ranks: one unit
    // per placed example, charged before the pass.
    resources.record_operations(count)?;
    let mut starts = vec![0usize; count];
    for rank in &ranks {
        starts[*rank] = starts[*rank].saturating_add(1);
    }
    let mut offset = 0usize;
    for start in &mut starts {
        let size = *start;
        *start = offset;
        offset = offset.saturating_add(size);
    }
    let mut order = vec![0usize; count];
    for (index, rank) in ranks.iter().enumerate() {
        order[starts[*rank]] = index;
        starts[*rank] = starts[*rank].saturating_add(1);
    }
    let canonical: Vec<&'a (Term, Term)> = order.iter().map(|index| &examples[*index]).collect();

    let mut unique: Vec<&'a (Term, Term)> = Vec::with_capacity(canonical.len());
    for pair in canonical {
        let duplicate = match unique.last() {
            Some(previous) => charged_pair_compare(previous, pair, resources)? == Ordering::Equal,
            None => false,
        };
        if !duplicate {
            unique.push(pair);
        }
    }
    Ok(unique)
}

/// Every variable name appearing in the supplied examples.
fn example_variables(
    positives: &[&(Term, Term)],
    negatives: &[&(Term, Term)],
    resources: &mut RelationResources,
) -> RewriteResult<BTreeSet<Variable>> {
    let mut variables = BTreeSet::new();
    for (lhs, rhs) in positives.iter().chain(negatives.iter()) {
        collect_variables(lhs, &mut variables, resources)?;
        collect_variables(rhs, &mut variables, resources)?;
    }
    Ok(variables)
}

/// Charge one unit per visited node while collecting variables.
pub(crate) fn collect_variables(
    term: &Term,
    variables: &mut BTreeSet<Variable>,
    resources: &mut RelationResources,
) -> RewriteResult<()> {
    let mut stack = vec![term];
    while let Some(current) = stack.pop() {
        resources.record_operations(1)?;
        match current {
            Term::Var(variable) => {
                variables.insert(variable.clone());
            }
            Term::Sym(_, args) => stack.extend(args.iter()),
        }
    }
    Ok(())
}

/// Borrowed one-level matcher bindings.
///
/// Ranges are verbatim example fragments, never chain-resolved, and
/// are never cloned: replay borrows them. Lookups charge one unit and
/// comparisons charge per visited node.
pub(crate) struct Bindings<'a> {
    entries: Vec<(&'a Variable, &'a Term)>,
}

impl<'a> Bindings<'a> {
    pub(crate) fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub(crate) fn get(
        &self,
        variable: &Variable,
        resources: &mut RelationResources,
    ) -> RewriteResult<Option<&'a Term>> {
        // Charge EVERY visited entry BEFORE comparing it: a miss costs
        // the full walk, and an exhausted budget aborts the scan at the
        // first unchargeable entry instead of visiting the rest for free
        // (PR #320 round 3).
        for (bound, term) in &self.entries {
            resources.record_operations(1)?;
            if *bound == variable {
                return Ok(Some(term));
            }
        }
        Ok(None)
    }

    pub(crate) fn insert(
        &mut self,
        variable: &'a Variable,
        term: &'a Term,
        resources: &mut RelationResources,
    ) -> RewriteResult<()> {
        resources.record_operations(1)?;
        self.entries.push((variable, term));
        Ok(())
    }
}

/// Exact coverage: the pattern matches and the virtually applied rule
/// yields the example right-hand side.
///
/// The applied right-hand side is never materialized. Bindings are
/// borrowed, the prospective shape is accounted first (Task 34
/// `matched_shape` doctrine), and the substituted template is then
/// compared node by node with one-level matcher provenance.
pub(crate) fn covers<'a>(
    rule: &'a Rule,
    example: &'a (Term, Term),
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    covers_pair(rule, &example.0, &example.1, limits, resources)
}

/// Exact coverage of one example given as separate borrowed sides.
///
/// This is [`covers`] without the pair wrapper, so callers that hold
/// the sides in reverse order (inverse synthesis replays the checked
/// reverse rule against `(target, source)`) never clone to build a
/// temporary pair.
pub(crate) fn covers_pair<'a>(
    rule: &'a Rule,
    example_lhs: &'a Term,
    example_rhs: &'a Term,
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    let mut bindings = Bindings::new();
    if !match_pattern(rule.lhs(), example_lhs, &mut bindings, resources)? {
        return Ok(false);
    }
    applied_shape(rule.rhs(), &bindings, limits, resources)?;
    one_level_equals(rule.rhs(), &bindings, example_rhs, resources)
}

/// True when the rule replays every positive in the partition.
fn covers_partition(
    rule: &Rule,
    partition: &[usize],
    positives: &[&(Term, Term)],
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    for &index in partition {
        if !covers(rule, positives[index], limits, resources)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// True when the rule covers at least one negative.
fn covers_any(
    rule: &Rule,
    negatives: &[&(Term, Term)],
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    for example in negatives {
        if covers(rule, example, limits, resources)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Match a borrowed pattern against a borrowed term with consistent
/// borrowed bindings, charging one unit per visited node pair.
///
/// Capture-free exactly like [`crate::trs::match_pattern`]: bindings
/// are verbatim fragments and repeated variables compare by structure.
/// Iterative, so matching never depends on the call stack.
pub(crate) fn match_pattern<'a>(
    pattern: &'a Term,
    term: &'a Term,
    bindings: &mut Bindings<'a>,
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    let mut stack = vec![(pattern, term)];
    while let Some((current_pattern, current_term)) = stack.pop() {
        resources.record_operations(1)?;
        match (current_pattern, current_term) {
            (Term::Var(variable), _) => match bindings.get(variable, resources)? {
                Some(existing) => {
                    if !charged_equal(existing, current_term, resources)? {
                        return Ok(false);
                    }
                }
                None => bindings.insert(variable, current_term, resources)?,
            },
            (Term::Sym(symbol, args), Term::Sym(term_symbol, term_args))
                if symbol == term_symbol && args.len() == term_args.len() =>
            {
                stack.extend(args.iter().zip(term_args));
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// Structural equality, charged per compared node pair BEFORE the
/// comparison and iterative.
pub(crate) fn charged_equal(
    left: &Term,
    right: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    let mut stack = vec![(left, right)];
    while let Some((current_left, current_right)) = stack.pop() {
        resources.record_operations(1)?;
        match (current_left, current_right) {
            (Term::Var(left_var), Term::Var(right_var)) if left_var == right_var => {}
            (Term::Sym(left_symbol, left_args), Term::Sym(right_symbol, right_args))
                if left_symbol == right_symbol && left_args.len() == right_args.len() =>
            {
                stack.extend(left_args.iter().zip(right_args));
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

/// Shape of `template` under borrowed MATCH bindings, one-level and
/// counted verbatim, exactly as the materialized application would be.
///
/// The shape is checked against the per-term ceilings BEFORE any term
/// is materialized — in fact nothing is materialized — and every
/// borrowed range is traversed with per-node charging (PR #320
/// round 2).
pub(crate) fn applied_shape(
    template: &Term,
    bindings: &Bindings<'_>,
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<(usize, usize)> {
    let mut nodes = 0usize;
    let mut depth = 0usize;
    let mut stack = vec![(template, 1usize)];
    while let Some((current, current_depth)) = stack.pop() {
        resources.record_operations(1)?;
        match current {
            Term::Var(variable) => {
                let (bound_nodes, bound_depth) = match bindings.get(variable, resources)? {
                    Some(bound) => admit_term(bound, limits, resources)?,
                    None => (1, 1),
                };
                nodes = nodes.saturating_add(bound_nodes);
                let occurrence_depth = current_depth.saturating_sub(1).saturating_add(bound_depth);
                depth = depth.max(occurrence_depth);
            }
            Term::Sym(_, args) => {
                nodes = nodes.saturating_add(1);
                depth = depth.max(current_depth);
                for arg in args {
                    stack.push((arg, current_depth.saturating_add(1)));
                }
            }
        }
        if nodes > limits.max_term_nodes() {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "term nodes",
                limit: limits.max_term_nodes(),
            });
        }
        if depth > limits.max_term_depth() {
            return Err(RewriteError::RelationLimitExceeded {
                resource: "term depth",
                limit: limits.max_term_depth(),
            });
        }
    }
    Ok((nodes, depth))
}

/// Compare the virtually applied `template` with `expected` without
/// materializing it. Bound variables compare by structure (verbatim
/// ranges), unbound variables must appear as themselves, and symbols
/// compare arity and children.
pub(crate) fn one_level_equals(
    template: &Term,
    bindings: &Bindings<'_>,
    expected: &Term,
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    let mut stack = vec![(template, expected)];
    while let Some((pattern, actual)) = stack.pop() {
        resources.record_operations(1)?;
        match pattern {
            Term::Var(variable) => match bindings.get(variable, resources)? {
                Some(bound) => {
                    if !charged_equal(bound, actual, resources)? {
                        return Ok(false);
                    }
                }
                None => {
                    if !matches!(actual, Term::Var(other) if other == variable) {
                        return Ok(false);
                    }
                }
            },
            Term::Sym(symbol, args) => match actual {
                Term::Sym(actual_symbol, actual_args)
                    if symbol == actual_symbol && args.len() == actual_args.len() =>
                {
                    stack.extend(args.iter().zip(actual_args));
                }
                _ => return Ok(false),
            },
        }
    }
    Ok(true)
}

/// Fresh generated variables that never collide with example names
/// (#303 interaction handling).
struct FreshNames<'a> {
    next: usize,
    exhausted: bool,
    reserved: &'a BTreeSet<Variable>,
}

impl<'a> FreshNames<'a> {
    fn new(reserved: &'a BTreeSet<Variable>) -> Self {
        Self {
            next: 0,
            exhausted: false,
            reserved,
        }
    }

    /// Allocate the next non-reserved `_I*` name.
    ///
    /// Every attempt is charged before the name is built, so a run of
    /// reserved names is metered and an exhausted budget aborts the
    /// loop promptly. At the `usize::MAX` boundary the namespace is
    /// marked exhausted and further attempts return a typed
    /// [`RewriteError::RelationLimitExceeded`] instead of spinning or
    /// duplicating the boundary name (the fallible-fresh convention of
    /// `relation::variable`; PR #320 round 4 P2a).
    fn fresh(&mut self, resources: &mut RelationResources) -> RewriteResult<Variable> {
        loop {
            // Charge each attempt BEFORE constructing the candidate.
            resources.record_operations(1)?;
            if self.exhausted {
                return Err(Self::exhausted_error());
            }
            let boundary = self.next == usize::MAX;
            let mut name = String::from("_I");
            name.push_str(&self.next.to_string());
            if boundary {
                self.exhausted = true;
            } else {
                self.next = self.next.saturating_add(1);
            }
            let variable = Variable::new(name);
            if !self.reserved.contains(&variable) {
                return Ok(variable);
            }
            if boundary {
                return Err(Self::exhausted_error());
            }
        }
    }

    fn exhausted_error() -> RewriteError {
        RewriteError::RelationLimitExceeded {
            resource: "fresh name namespace",
            limit: usize::MAX,
        }
    }
}

/// Shared-disagreement-table generalization, as in `infer_rule`.
///
/// Partition examples and the reserved-variable set are borrowed; no
/// partition or reserved-set clone is ever made (PR #320 round 2).
struct Generalizer<'a> {
    names: FreshNames<'a>,
    /// Disagreement keys in first-seen order. Lookups compare every
    /// visited key with the charged structural order: keyed
    /// comparisons are never free (PR #320 round 4 P2b).
    disagreements: Vec<((Term, Term), Variable)>,
    limits: RelationLimits,
}

impl<'a> Generalizer<'a> {
    fn new(reserved: &'a BTreeSet<Variable>, limits: RelationLimits) -> Self {
        Self {
            names: FreshNames::new(reserved),
            disagreements: Vec::new(),
            limits,
        }
    }

    fn generalize(
        &mut self,
        partition: &[usize],
        positives: &[&(Term, Term)],
        resources: &mut RelationResources,
    ) -> RewriteResult<Option<(Term, Term)>> {
        resources.record_operations(partition.len())?;
        let lhs_terms: Vec<&Term> = partition.iter().map(|&index| &positives[index].0).collect();
        let rhs_terms: Vec<&Term> = partition.iter().map(|&index| &positives[index].1).collect();
        let Some(lhs) = self.anti_unify_all(&lhs_terms, resources)? else {
            return Ok(None);
        };
        let Some(rhs) = self.anti_unify_all(&rhs_terms, resources)? else {
            return Ok(None);
        };
        Ok(Some((lhs, rhs)))
    }

    fn anti_unify_all(
        &mut self,
        terms: &[&Term],
        resources: &mut RelationResources,
    ) -> RewriteResult<Option<Term>> {
        let mut iter = terms.iter();
        let Some(first) = iter.next() else {
            return Ok(None);
        };
        // Charge the initial clone per node BEFORE cloning.
        let _shape = admit_term(first, &self.limits, resources)?;
        let mut current = (*first).clone();
        for term in iter {
            current = self.anti_unify(&current, term, resources)?;
        }
        Ok(Some(current))
    }

    fn anti_unify(
        &mut self,
        left: &Term,
        right: &Term,
        resources: &mut RelationResources,
    ) -> RewriteResult<Term> {
        // One generalized node pair, charged before the comparison or
        // clone.
        resources.record_operations(1)?;
        // Equality and the clone it may produce are each charged per
        // visited node: a four-thousand-node comparison or clone is
        // never funded by one unit (PR #320 round 2).
        if charged_equal(left, right, resources)? {
            let _shape = admit_term(left, &self.limits, resources)?;
            return Ok(left.clone());
        }
        match (left, right) {
            (Term::Sym(left_symbol, left_args), Term::Sym(right_symbol, right_args))
                if left_symbol == right_symbol && left_args.len() == right_args.len() =>
            {
                let mut generalized = Vec::with_capacity(left_args.len());
                for (left_arg, right_arg) in left_args.iter().zip(right_args) {
                    generalized.push(self.anti_unify(left_arg, right_arg, resources)?);
                }
                Ok(Term::Sym(left_symbol.clone(), generalized))
            }
            _ => Ok(Term::Var(self.variable_for(left, right, resources)?)),
        }
    }

    fn variable_for(
        &mut self,
        left: &Term,
        right: &Term,
        resources: &mut RelationResources,
    ) -> RewriteResult<Variable> {
        // Charge for the key's nodes BEFORE it is cloned.
        let _left_shape = admit_term(left, &self.limits, resources)?;
        let _right_shape = admit_term(right, &self.limits, resources)?;
        let key = (left.clone(), right.clone());
        if let Some(variable) = self.existing_variable(&key, resources)? {
            return Ok(variable);
        }
        let variable = self.names.fresh(resources)?;
        resources.record_operations(1)?;
        self.disagreements.push((key, variable.clone()));
        Ok(variable)
    }

    fn existing_variable(
        &self,
        key: &(Term, Term),
        resources: &mut RelationResources,
    ) -> RewriteResult<Option<Variable>> {
        for (existing, variable) in &self.disagreements {
            // Charge each visited key before comparing it, then charge
            // every compared node pair through the structural order.
            resources.record_operations(1)?;
            if charged_pair_compare(existing, key, resources)? == Ordering::Equal {
                return Ok(Some(variable.clone()));
            }
        }
        Ok(None)
    }
}

/// Build the checked rule generalizing one positive partition.
fn build_candidate(
    partition: &[usize],
    positives: &[&(Term, Term)],
    reserved: &BTreeSet<Variable>,
    max_candidates: usize,
    candidates: &mut usize,
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<Option<Rule>> {
    *candidates = candidates.saturating_add(1);
    if *candidates > max_candidates {
        return Err(RewriteError::RelationLimitExceeded {
            resource: "candidates",
            limit: max_candidates,
        });
    }
    resources.record_operations(1)?;

    let mut generalizer = Generalizer::new(reserved, *limits);
    let Some((lhs, rhs)) = generalizer.generalize(partition, positives, resources)? else {
        return Ok(None);
    };
    let (lhs_nodes, _lhs_depth) = admit_term(&lhs, limits, resources)?;
    let (rhs_nodes, _rhs_depth) = admit_term(&rhs, limits, resources)?;
    // Rule::new rescans both sides for variable validation: metered
    // before the scan, not after.
    resources.record_operations(lhs_nodes.saturating_add(rhs_nodes))?;
    Ok(Rule::new(lhs, rhs).ok())
}

/// Canonical discriminating positions: the candidate's left-hand
/// positions in preorder, then its right-hand positions. Without a
/// candidate, the first partition example supplies the positions.
fn discriminating_positions(
    candidate: Option<&Rule>,
    partition: &[usize],
    positives: &[&(Term, Term)],
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<Vec<(bool, Path)>> {
    let (lhs, rhs) = match candidate {
        Some(rule) => (rule.lhs(), rule.rhs()),
        None => (&positives[partition[0]].0, &positives[partition[0]].1),
    };
    // Charge the traversals before the position vectors are built.
    let _lhs_shape = admit_term(lhs, limits, resources)?;
    let _rhs_shape = admit_term(rhs, limits, resources)?;
    let mut positions: Vec<(bool, Path)> = lhs
        .positions()
        .into_iter()
        .map(|path| (true, path))
        .collect();
    positions.extend(rhs.positions().into_iter().map(|path| (false, path)));
    Ok(positions)
}

/// Compare two optional borrowed subterms with charged equality.
fn option_terms_equal(
    left: Option<&Term>,
    right: Option<&Term>,
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    resources.record_operations(1)?;
    match (left, right) {
        (None, None) => Ok(true),
        (Some(left), Some(right)) => charged_equal(left, right, resources),
        _ => Ok(false),
    }
}

/// Split at the first position whose subterms separate the partition
/// into at least two classes, in first-seen order.
///
/// Grouping compares BORROWED subterms with per-node charging; nothing
/// is cloned, and every `(position, partition member)` lookup is
/// charged BEFORE the comparison runs (PR #320 round 2).
fn first_split(
    positions: &[(bool, Path)],
    partition: &[usize],
    positives: &[&(Term, Term)],
    resources: &mut RelationResources,
) -> RewriteResult<Option<Vec<Vec<usize>>>> {
    for (is_lhs, path) in positions {
        let mut groups: Vec<(Option<&Term>, Vec<usize>)> = Vec::new();
        for &index in partition {
            resources.record_operations(1)?;
            let example = positives[index];
            let term = if *is_lhs { &example.0 } else { &example.1 };
            let key = term.subterm(path);
            let mut placed = false;
            for (existing, members) in groups.iter_mut() {
                if option_terms_equal(*existing, key, resources)? {
                    members.push(index);
                    placed = true;
                    break;
                }
            }
            if !placed {
                groups.push((key, vec![index]));
            }
        }
        if groups.len() > 1 {
            return Ok(Some(
                groups.into_iter().map(|(_key, members)| members).collect(),
            ));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::{FreshNames, RelationLimits, RelationResources, RewriteError};
    use alloc::collections::BTreeSet;

    #[test]
    fn exhausted_fresh_names_are_typed_and_sticky() {
        // The namespace boundary is a typed outcome, never a spin or a
        // duplicate boundary name (fallible-fresh convention,
        // PR #320 round 4 P2a).
        let reserved = BTreeSet::new();
        let limits = RelationLimits::default();
        let mut resources = RelationResources::new(&limits);
        let mut names = FreshNames {
            next: usize::MAX,
            exhausted: false,
            reserved: &reserved,
        };
        // The boundary index itself is usable once when free, exactly
        // like the logic-variable namespace convention.
        let boundary = names.fresh(&mut resources).unwrap();
        assert_eq!(boundary.as_str(), alloc::format!("_I{}", usize::MAX));
        assert!(names.exhausted);

        // Sticky and prompt: one attempt charge, then the typed error.
        let before = resources.operations();
        let error = names.fresh(&mut resources).unwrap_err();
        assert!(matches!(error, RewriteError::RelationLimitExceeded { .. }));
        assert_eq!(resources.operations(), before + 1);
    }
}
