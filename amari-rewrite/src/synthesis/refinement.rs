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
//! yields a term structurally equal to `rhs`. Application uses the
//! one-level matcher substitution
//! ([`Substitution::apply_matcher`](crate::trs::Substitution::apply_matcher)),
//! so example variable names cannot be captured by rule variables
//! (Task 31 provenance discipline).
//!
//! # Specialization
//!
//! 1. Both example lists are canonicalized (sorted and deduplicated);
//!    a pair appearing as both positive and negative is a typed
//!    [`RewriteError::InvalidRule`].
//! 2. Each partition of positives is generalized with the shared
//!    disagreement table used by
//!    [`infer_rule`](crate::synthesis::infer_rule), reserving every
//!    example variable name so generated `_I*` variables cannot collide
//!    with example variables (the #303 interaction).
//! 3. A candidate that covers a negative (or fails to replay against
//!    its own positives) is split at the first discriminating position
//!    in canonical preorder over the candidate's left-hand side and
//!    then its right-hand side. A position discriminates when the
//!    positives' subterms there fall into at least two classes: a
//!    sub-rule built from one class can only cover a negative whose
//!    subterm at that position is in the same class.
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
//! covering the whole run. Every violation is a typed error; nothing
//! is truncated silently and nothing panics.

use alloc::{
    collections::{BTreeMap, BTreeSet},
    string::{String, ToString},
    vec,
    vec::Vec,
};

use crate::{
    relation::{RelationLimits, RelationResources},
    trs::{Rule, Term, Variable},
    Path, RewriteError, RewriteResult,
};

/// Caller-tightenable specialization limits.
///
/// The associated constants are the hard ceilings; [`Self::new`] only
/// accepts values inside `(0, ceiling]`. Term size and operation
/// ceilings come from the enclosed [`RelationLimits`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct SpecializationConfig {
    max_examples: usize,
    max_candidates: usize,
    max_partitions: usize,
    limits: RelationLimits,
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
        config.check("examples", max_examples, Self::MAX_EXAMPLES)?;
        config.check("candidates", max_candidates, Self::MAX_CANDIDATES)?;
        config.check("partitions", max_partitions, Self::MAX_PARTITIONS)?;
        Ok(config)
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
/// Examples are canonicalized by sorting and deduplicating, so the
/// coverage indices and the rule order do not depend on caller order.
///
/// # Errors
///
/// - [`RewriteError::InvalidRule`] when there are no positive
///   examples, or when one example pair is supplied as both positive
///   and negative.
/// - [`RewriteError::RelationLimitExceeded`] when the example,
///   candidate, partition, term node/depth, or operation ceilings are
///   exceeded.
pub fn specialize_rules(
    positives: &[(Term, Term)],
    negatives: &[(Term, Term)],
    config: &SpecializationConfig,
) -> RewriteResult<SpecializationOutcome> {
    let positives = canonical_examples(positives);
    let negatives = canonical_examples(negatives);
    if positives.is_empty() {
        return Err(RewriteError::InvalidRule {
            message: String::from("cannot specialize without positive examples"),
        });
    }
    if positives
        .iter()
        .any(|pair| negatives.binary_search(pair).is_ok())
    {
        return Err(RewriteError::InvalidRule {
            message: String::from("example pair is both positive and negative"),
        });
    }

    let mut resources = RelationResources::new(config.limits());
    let total = positives.len().saturating_add(negatives.len());
    if total > config.max_examples() {
        return Err(RewriteError::RelationLimitExceeded {
            resource: "examples",
            limit: config.max_examples(),
        });
    }
    for example in positives.iter().chain(negatives.iter()) {
        charge_term(&example.0, &mut resources)?;
        charge_term(&example.1, &mut resources)?;
    }

    let reserved = example_variables(&positives, &negatives);
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
            &mut resources,
        )?;

        let mut retained = None;
        if let Some(rule) = candidate.as_ref() {
            if covers_partition(rule, &partition, &positives, &mut resources)?
                && !covers_any(rule, &negatives, &mut resources)?
            {
                retained = Some(rule.clone());
            }
        }
        if let Some(rule) = retained {
            resolved.insert(partition, rule);
            continue;
        }

        let positions = discriminating_positions(candidate.as_ref(), &partition, &positives);
        match first_split(&positions, &partition, &positives) {
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
    finish_refined(resolved, &positives, &negatives, &mut resources)
}

/// Replay every refined rule against every example and package the
/// outcome. Any rule that covers a negative — impossible by the
/// per-partition checks — is reported as a typed error instead of
/// being returned.
fn finish_refined(
    resolved: BTreeMap<Vec<usize>, Rule>,
    positives: &[(Term, Term)],
    negatives: &[(Term, Term)],
    resources: &mut RelationResources,
) -> RewriteResult<SpecializationOutcome> {
    let rules: Vec<Rule> = resolved.into_values().collect();
    let mut entries = Vec::with_capacity(rules.len());
    for (index, rule) in rules.iter().enumerate() {
        let mut covered_positives = Vec::new();
        for (positive_index, example) in positives.iter().enumerate() {
            if covers(rule, example, resources)? {
                covered_positives.push(positive_index);
            }
        }
        let mut covered_negatives = Vec::new();
        for (negative_index, example) in negatives.iter().enumerate() {
            if covers(rule, example, resources)? {
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

/// Sort and deduplicate example pairs by structural term order.
fn canonical_examples(examples: &[(Term, Term)]) -> Vec<(Term, Term)> {
    let mut canonical = examples.to_vec();
    canonical.sort();
    canonical.dedup();
    canonical
}

/// Every variable name appearing in the supplied examples.
fn example_variables(positives: &[(Term, Term)], negatives: &[(Term, Term)]) -> BTreeSet<Variable> {
    let mut variables = BTreeSet::new();
    for (lhs, rhs) in positives.iter().chain(negatives.iter()) {
        variables.extend(lhs.variables());
        variables.extend(rhs.variables());
    }
    variables
}

/// Charge one term against the node and depth ceilings.
fn charge_term(term: &Term, resources: &mut RelationResources) -> RewriteResult<()> {
    let (nodes, depth) = term_shape(term);
    resources.record_term(nodes, depth)
}

/// Node count and maximum depth, computed iteratively.
fn term_shape(term: &Term) -> (usize, usize) {
    let mut nodes = 0usize;
    let mut depth = 0usize;
    let mut stack = vec![(term, 0usize)];
    while let Some((current, current_depth)) = stack.pop() {
        nodes = nodes.saturating_add(1);
        depth = depth.max(current_depth);
        if let Term::Sym(_, args) = current {
            for arg in args {
                stack.push((arg, current_depth.saturating_add(1)));
            }
        }
    }
    (nodes, depth)
}

/// Exact coverage: the pattern matches and the applied rule yields the
/// example right-hand side.
fn covers(
    rule: &Rule,
    example: &(Term, Term),
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    resources.record_operations(1)?;
    Ok(rule
        .apply_root(&example.0)
        .map(|produced| produced == example.1)
        .unwrap_or(false))
}

/// True when the rule replays every positive in the partition.
fn covers_partition(
    rule: &Rule,
    partition: &[usize],
    positives: &[(Term, Term)],
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    for &index in partition {
        if !covers(rule, &positives[index], resources)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// True when the rule covers at least one negative.
fn covers_any(
    rule: &Rule,
    negatives: &[(Term, Term)],
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    for example in negatives {
        if covers(rule, example, resources)? {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Fresh generated variables that never collide with example names
/// (#303 interaction handling).
struct FreshNames {
    next: usize,
    reserved: BTreeSet<Variable>,
}

impl FreshNames {
    fn new(reserved: BTreeSet<Variable>) -> Self {
        Self { next: 0, reserved }
    }

    fn fresh(&mut self) -> Variable {
        loop {
            let mut name = String::from("_I");
            name.push_str(&self.next.to_string());
            self.next = self.next.saturating_add(1);
            let variable = Variable::new(name);
            if !self.reserved.contains(&variable) {
                return variable;
            }
        }
    }
}

/// Shared-disagreement-table generalization, as in `infer_rule`.
struct Generalizer {
    names: FreshNames,
    disagreements: BTreeMap<(Term, Term), Variable>,
}

impl Generalizer {
    fn new(reserved: BTreeSet<Variable>) -> Self {
        Self {
            names: FreshNames::new(reserved),
            disagreements: BTreeMap::new(),
        }
    }

    fn generalize(&mut self, examples: &[(Term, Term)]) -> Option<(Term, Term)> {
        let lhs_terms: Vec<&Term> = examples.iter().map(|(lhs, _)| lhs).collect();
        let rhs_terms: Vec<&Term> = examples.iter().map(|(_, rhs)| rhs).collect();
        let lhs = self.anti_unify_all(&lhs_terms)?;
        let rhs = self.anti_unify_all(&rhs_terms)?;
        Some((lhs, rhs))
    }

    fn anti_unify_all(&mut self, terms: &[&Term]) -> Option<Term> {
        let mut iter = terms.iter();
        let mut current = (*iter.next()?).clone();
        for term in iter {
            current = self.anti_unify(&current, term);
        }
        Some(current)
    }

    fn anti_unify(&mut self, left: &Term, right: &Term) -> Term {
        if left == right {
            return left.clone();
        }
        match (left, right) {
            (Term::Sym(left_symbol, left_args), Term::Sym(right_symbol, right_args))
                if left_symbol == right_symbol && left_args.len() == right_args.len() =>
            {
                Term::Sym(
                    left_symbol.clone(),
                    left_args
                        .iter()
                        .zip(right_args)
                        .map(|(left_arg, right_arg)| self.anti_unify(left_arg, right_arg))
                        .collect(),
                )
            }
            _ => Term::Var(self.variable_for(left, right)),
        }
    }

    fn variable_for(&mut self, left: &Term, right: &Term) -> Variable {
        let key = (left.clone(), right.clone());
        if let Some(variable) = self.disagreements.get(&key) {
            return variable.clone();
        }
        let variable = self.names.fresh();
        self.disagreements.insert(key, variable.clone());
        variable
    }
}

/// Build the checked rule generalizing one positive partition.
fn build_candidate(
    partition: &[usize],
    positives: &[(Term, Term)],
    reserved: &BTreeSet<Variable>,
    max_candidates: usize,
    candidates: &mut usize,
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

    let examples: Vec<(Term, Term)> = partition
        .iter()
        .map(|&index| positives[index].clone())
        .collect();
    let mut generalizer = Generalizer::new(reserved.clone());
    let Some((lhs, rhs)) = generalizer.generalize(&examples) else {
        return Ok(None);
    };
    charge_term(&lhs, resources)?;
    charge_term(&rhs, resources)?;
    Ok(Rule::new(lhs, rhs).ok())
}

/// Canonical discriminating positions: the candidate's left-hand
/// positions in preorder, then its right-hand positions. Without a
/// candidate, the first partition example supplies the positions.
fn discriminating_positions(
    candidate: Option<&Rule>,
    partition: &[usize],
    positives: &[(Term, Term)],
) -> Vec<(bool, Path)> {
    let (lhs, rhs) = match candidate {
        Some(rule) => (rule.lhs().clone(), rule.rhs().clone()),
        None => positives[partition[0]].clone(),
    };
    let mut positions: Vec<(bool, Path)> = lhs
        .positions()
        .into_iter()
        .map(|path| (true, path))
        .collect();
    positions.extend(rhs.positions().into_iter().map(|path| (false, path)));
    positions
}

/// Split at the first position whose subterms separate the partition
/// into at least two classes, in class order.
fn first_split(
    positions: &[(bool, Path)],
    partition: &[usize],
    positives: &[(Term, Term)],
) -> Option<Vec<Vec<usize>>> {
    for (is_lhs, path) in positions {
        let mut groups: BTreeMap<Option<Term>, Vec<usize>> = BTreeMap::new();
        for &index in partition {
            let (lhs, rhs) = &positives[index];
            let term = if *is_lhs { lhs } else { rhs };
            let key = term.subterm(path).cloned();
            groups.entry(key).or_default().push(index);
        }
        if groups.len() > 1 {
            return Some(groups.into_values().collect());
        }
    }
    None
}
