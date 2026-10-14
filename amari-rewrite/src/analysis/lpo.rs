// SPDX-License-Identifier: MIT OR Apache-2.0

//! Lexicographic path ordering (LPO) termination certificates.
//!
//! LPO orients a rewrite rule `l -> r` when `l >_lpo r` under a
//! precedence on function symbols. A finite system terminates when
//! every rule is strictly decreasing, so [`prove_terminating`] lifts
//! the per-rule comparisons into a whole-system [`TerminationProof`].
//!
//! # Precedences
//!
//! [`Precedence`] is a validated strict total order over ranked
//! symbols. Construction rejects duplicate symbols, and
//! [`prove_terminating`] rejects a precedence that misses a symbol
//! used by the system or disagrees with its arity. A partial or
//! inconsistent ordering is a typed error; it is never silently
//! extended to cover the system.
//!
//! # The order
//!
//! The comparison `s >_lpo t` follows Baader–Nipkow definition
//! 5.4.12. It holds when one of the following does:
//!
//! 1. `s = f(s1..sm)` and some argument `si` equals `t` or already
//!    dominates it — the path descent that yields both the subterm
//!    property and transitivity;
//! 2. `s = f(s1..sm)`, `t = g(t1..tn)`, `f` outranks `g`, and `s` is
//!    greater than every `ti`;
//! 3. `s = f(s1..sm)`, `t = f(t1..tn)`, the argument lists share a
//!    (possibly empty) equal prefix, the first differing pair satisfies
//!    `si >_lpo ti`, and `s` is greater than every trailing `tj`
//!    (`j > i`).
//!
//! A variable is never greater than anything, and a term is greater
//! than exactly the variables it properly contains. With a total
//! precedence on the function symbols the order is a strict partial
//! order (irreflexive and transitive), which is what makes a
//! strictly-decreasing rule set terminating.
//!
//! # Accounting and memoization
//!
//! Each call memoizes its comparisons: every distinct term pair is
//! evaluated at most once, charging one operation, and the two memo
//! keys are term-bounds validated before the pair is cached.
//! [`prove_terminating`] shares one cache across every rule, so a
//! system that repeats comparisons pays for each pair once.
//! Exhausting a budget returns
//! [`crate::error::RewriteError::RelationLimitExceeded`] with no
//! partial proof.

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use crate::analysis::unify::{term_depth, term_nodes};
use crate::error::{RewriteError, RewriteResult};
use crate::relation::RelationResources;
use crate::trs::{Symbol, Term, TermSystem};

/// A validated strict total order over ranked function symbols.
///
/// Entries are supplied highest precedence first; the position in the
/// list is the rank, so a smaller rank outranks a larger one. The
/// rank map is injective by construction, which makes the order a
/// strict total order on the listed symbols.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
pub struct Precedence {
    rank: BTreeMap<Symbol, usize>,
    arity: BTreeMap<Symbol, usize>,
}

#[cfg(feature = "serialize")]
impl<'de> serde::Deserialize<'de> for Precedence {
    /// Deserialization routes through the same invariants as
    /// [`Precedence::new`]: rank and arity domains must match
    /// exactly and ranks must be injective — a malformed map never
    /// becomes a "total" order (PR #305 round 1).
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        struct Raw {
            rank: BTreeMap<Symbol, usize>,
            arity: BTreeMap<Symbol, usize>,
        }
        let raw = Raw::deserialize(deserializer)?;
        if raw.rank.len() != raw.arity.len()
            || !raw.rank.keys().zip(raw.arity.keys()).all(|(l, r)| l == r)
        {
            return Err(serde::de::Error::custom(
                "precedence rank and arity domains differ",
            ));
        }
        let mut seen = BTreeSet::new();
        for rank in raw.rank.values() {
            if !seen.insert(rank) {
                return Err(serde::de::Error::custom("duplicate precedence rank"));
            }
        }
        Ok(Self {
            rank: raw.rank,
            arity: raw.arity,
        })
    }
}

impl Precedence {
    /// Build a precedence from `(symbol, arity)` pairs ordered highest
    /// first.
    ///
    /// Rejects a symbol that appears more than once: a total order
    /// assigns exactly one rank to each symbol, and silently keeping
    /// the last would hide an inconsistent input. Coverage and arity
    /// agreement with a concrete system are checked by
    /// [`prove_terminating`].
    pub fn new(ordering: Vec<(Symbol, usize)>) -> RewriteResult<Self> {
        let mut rank = BTreeMap::new();
        let mut arity = BTreeMap::new();
        for (position, (symbol, declared_arity)) in ordering.into_iter().enumerate() {
            if rank.contains_key(&symbol) {
                return Err(RewriteError::InvalidRule {
                    message: alloc::format!("duplicate precedence symbol `{}`", symbol.as_str()),
                });
            }
            rank.insert(symbol.clone(), position);
            arity.insert(symbol, declared_arity);
        }
        Ok(Self { rank, arity })
    }

    /// Rank of `symbol`, if listed.
    fn rank_of(&self, symbol: &Symbol) -> Option<usize> {
        self.rank.get(symbol).copied()
    }

    /// Declared arity of `symbol`, if listed.
    fn arity_of(&self, symbol: &Symbol) -> Option<usize> {
        self.arity.get(symbol).copied()
    }
}

/// Per-system counts carried by a [`TerminationProof`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct TerminationReport {
    /// Number of rules examined before the proof completed.
    pub rules_checked: usize,
    /// Number of rules proved strictly decreasing.
    pub decreasing_rules: usize,
}

/// Outcome of attempting an LPO termination proof.
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum TerminationProof {
    /// Every rule strictly decreases, so the system terminates.
    ProvedTerminating {
        /// Per-system counts for the certificate.
        report: TerminationReport,
    },
    /// The rule at this index is not strictly decreasing, so this
    /// precedence yields no proof. The first offending rule is
    /// reported; no certificate is ever returned for a failing rule.
    NotDecreasing {
        /// Index of the first rule that failed to decrease.
        rule_index: usize,
    },
}

/// Prove termination of `system` under `precedence`.
///
/// Validates that the precedence covers every function symbol used by
/// the system exactly once with a matching arity, then checks each
/// rule in order. Returns [`TerminationProof::ProvedTerminating`] only
/// when every rule strictly decreases; otherwise returns
/// [`TerminationProof::NotDecreasing`] with the first offending rule
/// index. Budget exhaustion returns
/// [`RewriteError::RelationLimitExceeded`] without a partial proof.
pub fn prove_terminating(
    system: &TermSystem,
    precedence: &Precedence,
    resources: &mut RelationResources,
) -> RewriteResult<TerminationProof> {
    let mut validated: BTreeSet<Term> = BTreeSet::new();
    for rule in system.rules() {
        for side in [rule.lhs(), rule.rhs()] {
            if !validated.contains(side) {
                validate_input(side, precedence, resources)?;
                // Key materialization for the validation memo.
                resources.record_term(term_nodes(side), term_depth(side))?;
                validated.insert(side.clone());
            }
        }
    }
    validate_precedence(system, precedence, resources)?;

    let mut cache = LpoCache::new();
    let rules = system.rules();
    let mut decreasing_rules = 0usize;
    for (rule_index, rule) in rules.iter().enumerate() {
        if compare_pair(rule.lhs(), rule.rhs(), precedence, resources, &mut cache)? {
            decreasing_rules += 1;
            continue;
        }
        return Ok(TerminationProof::NotDecreasing { rule_index });
    }

    Ok(TerminationProof::ProvedTerminating {
        report: TerminationReport {
            rules_checked: rules.len(),
            decreasing_rules,
        },
    })
}

/// Bare LPO comparison `s >_lpo t` under `precedence`.
///
/// This is the comparison without rule-level semantics: it does not
/// check that `t` is a reduct or that any system is covered. Symbols
/// absent from `precedence` are a typed error, so callers — such as
/// the completion procedure — should validate coverage first (for
/// example through [`prove_terminating`]) or treat the error as fatal.
///
/// The comparison memoizes internally for the duration of the call;
/// one distinct term pair charges one operation, and each memo key is
/// term-bounds validated before it is cached.
pub fn lpo_greater(
    left: &Term,
    right: &Term,
    precedence: &Precedence,
    resources: &mut RelationResources,
) -> RewriteResult<bool> {
    validate_input(left, precedence, resources)?;
    if right != left {
        validate_input(right, precedence, resources)?;
    }
    let mut cache = LpoCache::new();
    compare_pair(left, right, precedence, resources, &mut cache)
}

/// Memo of `(left, right) -> left >_lpo right` for one query.
type LpoCache = BTreeMap<(Term, Term), bool>;

/// Memoized wrapper: charge and validate a distinct pair before it is
/// computed and cached.
fn compare_pair(
    left: &Term,
    right: &Term,
    precedence: &Precedence,
    resources: &mut RelationResources,
    cache: &mut LpoCache,
) -> RewriteResult<bool> {
    let key = (left.clone(), right.clone());
    if let Some(result) = cache.get(&key) {
        return Ok(*result);
    }
    resources.record_operations(1)?;
    // Materialize the memo key only after both terms clear the term
    // bounds; an oversized pair fails closed before caching.
    resources.record_term(term_nodes(left), term_depth(left))?;
    resources.record_term(term_nodes(right), term_depth(right))?;
    let result = compare_uncached(left, right, precedence, resources, cache)?;
    cache.insert(key, result);
    Ok(result)
}

/// Recursive comparison implementing the three LPO cases. Every
/// recursive call goes through [`compare_pair`], so recursion is
/// bounded by the (already validated) term depth.
fn compare_uncached(
    left: &Term,
    right: &Term,
    precedence: &Precedence,
    resources: &mut RelationResources,
    cache: &mut LpoCache,
) -> RewriteResult<bool> {
    if left == right {
        return Ok(false);
    }

    // A variable head never dominates.
    let (left_symbol, left_args) = match left {
        Term::Var(_) => return Ok(false),
        Term::Sym(symbol, args) => (symbol, args),
    };

    // Case 1: some immediate argument is `right` itself or already
    // dominates it. The recursion is the path descent that yields the
    // subterm property and makes the order transitive.
    for argument in left_args {
        if argument == right {
            return Ok(true);
        }
        if compare_pair(argument, right, precedence, resources, cache)? {
            return Ok(true);
        }
    }

    let (right_symbol, right_args) = match right {
        // `right` is a variable not appearing in `left`: incomparable.
        Term::Var(_) => return Ok(false),
        Term::Sym(symbol, args) => (symbol, args),
    };

    let left_rank = rank_of(precedence, left_symbol)?;
    let right_rank = rank_of(precedence, right_symbol)?;

    if left_rank < right_rank {
        // Case 2: the head symbol outranks (smaller rank is higher),
        // and `left` must be greater than every argument of `right`.
        for right_arg in right_args {
            if !compare_pair(left, right_arg, precedence, resources, cache)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    if left_rank > right_rank {
        return Ok(false);
    }

    // Case 3: same head symbol. The argument lists share an equal
    // prefix, the first differing pair must compare, and `left` must
    // be greater than every trailing `right` argument.
    if left_args.len() != right_args.len() {
        return Ok(false);
    }
    for index in 0..left_args.len() {
        if left_args[index] == right_args[index] {
            continue;
        }
        if !compare_pair(
            &left_args[index],
            &right_args[index],
            precedence,
            resources,
            cache,
        )? {
            return Ok(false);
        }
        for right_arg in &right_args[index + 1..] {
            if !compare_pair(left, right_arg, precedence, resources, cache)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }

    // Equal argument lists imply `left == right`, handled above.
    Ok(false)
}

/// Iteratively validate one input term BEFORE any recursive helper
/// touches it: enforce node/depth bounds and precedence coverage
/// (every symbol ranked, used at its declared arity). The recursive
/// comparison and shape helpers are only sound on terms that have
/// passed this check — a deeply nested input must fail here with a
/// typed limit, never overflow the stack (PR #305 round 1).
fn validate_input(
    term: &Term,
    precedence: &Precedence,
    resources: &mut RelationResources,
) -> RewriteResult<()> {
    let mut stack = alloc::vec::Vec::from([(term, 1usize)]);
    let mut nodes = 0usize;
    let mut max_depth = 0usize;
    while let Some((current, depth)) = stack.pop() {
        resources.record_operations(1)?;
        nodes = nodes.saturating_add(1);
        max_depth = max_depth.max(depth);
        if let Term::Sym(symbol, args) = current {
            let declared =
                precedence
                    .arity_of(symbol)
                    .ok_or_else(|| RewriteError::InvalidRule {
                        message: alloc::format!(
                            "precedence does not cover symbol `{}`",
                            symbol.as_str()
                        ),
                    })?;
            if declared != args.len() {
                return Err(RewriteError::InvalidRule {
                    message: alloc::format!(
                        "precedence ranks symbol `{}` at arity {} but the term uses arity {}",
                        symbol.as_str(),
                        declared,
                        args.len()
                    ),
                });
            }
            for arg in args {
                stack.push((arg, depth.saturating_add(1)));
            }
        }
    }
    resources.record_term(nodes, max_depth)?;
    Ok(())
}

/// Rank of a symbol, or a typed error when the precedence omits it.
fn rank_of(precedence: &Precedence, symbol: &Symbol) -> RewriteResult<usize> {
    precedence
        .rank_of(symbol)
        .ok_or_else(|| RewriteError::InvalidRule {
            message: alloc::format!("precedence does not rank symbol `{}`", symbol.as_str()),
        })
}

/// Check that `precedence` covers every function symbol used by the
/// system exactly once with the arity the system uses.
fn validate_precedence(
    system: &TermSystem,
    precedence: &Precedence,
    resources: &mut RelationResources,
) -> RewriteResult<()> {
    let mut used: BTreeMap<Symbol, usize> = BTreeMap::new();
    for rule in system.rules() {
        resources.record_term(term_nodes(rule.lhs()), term_depth(rule.lhs()))?;
        resources.record_term(term_nodes(rule.rhs()), term_depth(rule.rhs()))?;
        collect_symbols(rule.lhs(), &mut used)?;
        collect_symbols(rule.rhs(), &mut used)?;
    }

    for (symbol, arity) in &used {
        match precedence.arity_of(symbol) {
            None => {
                return Err(RewriteError::InvalidRule {
                    message: alloc::format!(
                        "precedence does not cover symbol `{}`",
                        symbol.as_str()
                    ),
                });
            }
            Some(declared) if declared != *arity => {
                return Err(RewriteError::InvalidRule {
                    message: alloc::format!(
                        "precedence ranks symbol `{}` at arity {} but the system uses arity {}",
                        symbol.as_str(),
                        declared,
                        arity
                    ),
                });
            }
            Some(_) => {}
        }
    }
    Ok(())
}

/// Record every function symbol in `term` with its arity, rejecting a
/// symbol used at two different arities.
fn collect_symbols(term: &Term, used: &mut BTreeMap<Symbol, usize>) -> RewriteResult<()> {
    match term {
        Term::Var(_) => Ok(()),
        Term::Sym(symbol, args) => {
            match used.get(symbol) {
                Some(existing) if *existing != args.len() => {
                    return Err(RewriteError::InvalidRule {
                        message: alloc::format!(
                            "symbol `{}` is used with arities {} and {}",
                            symbol.as_str(),
                            existing,
                            args.len()
                        ),
                    });
                }
                Some(_) => {}
                None => {
                    used.insert(symbol.clone(), args.len());
                }
            }
            for arg in args {
                collect_symbols(arg, used)?;
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relation::RelationLimits;
    use alloc::vec;

    fn x() -> Term {
        Term::var("x")
    }

    #[test]
    fn nested_dominance_descends_arguments() {
        // Ranked signatures fix one arity per symbol, so the two
        // comparisons live in separate fixtures (PR #305 round 1:
        // the original fixture used f and g at two arities and the
        // input validator correctly rejects it).
        let unary = Precedence::new(vec![(Symbol::new("f"), 1), (Symbol::new("g"), 1)])
            .expect("precedence");
        let mut resources = RelationResources::new(&RelationLimits::default());
        // f(g(x)) > g(f(x)): the head outranks and dominates the
        // single argument of the right-hand side.
        assert!(lpo_greater(
            &Term::sym("f", vec![Term::sym("g", vec![x()])]),
            &Term::sym("g", vec![Term::sym("f", vec![x()])]),
            &unary,
            &mut resources,
        )
        .expect("comparison"));
        // A term dominates a deeply nested argument through the path
        // descent: f(a, h(x)) > g(b, a) because f outranks g and
        // dominates both of g's arguments (b by precedence with no
        // arguments to check; a by descent to the equal argument).
        let binary = Precedence::new(vec![
            (Symbol::new("f"), 2),
            (Symbol::new("g"), 2),
            (Symbol::new("h"), 1),
            (Symbol::new("b"), 0),
            (Symbol::new("a"), 0),
        ])
        .expect("precedence");
        let mut resources = RelationResources::new(&RelationLimits::default());
        assert!(lpo_greater(
            &Term::sym("f", vec![Term::sym("a", vec![]), Term::sym("h", vec![x()])],),
            &Term::sym("g", vec![Term::sym("b", vec![]), Term::sym("a", vec![])]),
            &binary,
            &mut resources,
        )
        .expect("comparison"));
    }

    #[test]
    fn duplicate_symbols_are_rejected() {
        let result = Precedence::new(vec![
            (Symbol::new("f"), 1),
            (Symbol::new("g"), 1),
            (Symbol::new("f"), 1),
        ]);
        assert!(matches!(result, Err(RewriteError::InvalidRule { .. })));
    }

    #[test]
    fn same_symbol_arguments_compare_lexicographically() {
        let order = Precedence::new(vec![
            (Symbol::new("f"), 1),
            (Symbol::new("b"), 0),
            (Symbol::new("a"), 0),
        ])
        .expect("precedence");
        let mut resources = RelationResources::new(&RelationLimits::default());
        // f(b) > f(a) because b outranks a.
        assert!(lpo_greater(
            &Term::sym("f", vec![Term::constant("b")]),
            &Term::sym("f", vec![Term::constant("a")]),
            &order,
            &mut resources,
        )
        .expect("comparison"));
        // f(a) > f(b) is false.
        assert!(!lpo_greater(
            &Term::sym("f", vec![Term::constant("a")]),
            &Term::sym("f", vec![Term::constant("b")]),
            &order,
            &mut resources,
        )
        .expect("comparison"));
    }

    #[test]
    fn unknown_symbol_is_a_typed_error() {
        let order = Precedence::new(vec![(Symbol::new("f"), 1)]).expect("precedence");
        let mut resources = RelationResources::new(&RelationLimits::default());
        // The head `h` is absent from the precedence, so the comparison
        // cannot rank it and fails closed.
        let result = lpo_greater(
            &Term::sym("f", vec![x()]),
            &Term::sym("h", vec![x()]),
            &order,
            &mut resources,
        );
        assert!(matches!(result, Err(RewriteError::InvalidRule { .. })));
    }
}
