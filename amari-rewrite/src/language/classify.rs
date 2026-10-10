// SPDX-License-Identifier: MIT OR Apache-2.0

//! TRS classification for regular preimage scope (ADR 0001).
//!
//! The taxonomy is exhaustive and never heuristic: every admitted
//! system lands in exactly one class, and each class has a fixed
//! capability per preimage operation from the closure matrix
//! (`docs/research/rewrite-preimage-closure-matrix.md`). The ADR 0001
//! input contract is enforced BEFORE classification: every rule must
//! satisfy `Var(rhs) ⊆ Var(lhs)` (including rules admitted through
//! `Rule::new_unchecked`), and [`validate_alphabet`] checks the common
//! ranked alphabet of a system/language pair.

use alloc::collections::BTreeSet;
use alloc::format;

use crate::error::{RewriteError, RewriteResult};
use crate::language::TreeAutomaton;
use crate::trs::{Term, TermSystem, Variable};

/// The exhaustive TRS taxonomy for preimage exactness (ADR 0001).
///
/// Precedence is monotone: `Ground` ⊂ `LinearVariableDisjoint` ⊂
/// `LeftLinearShared` ⊂ all validated systems; anything else is
/// `NonLeftLinear`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum TrsClass {
    /// Every rule side is ground. Includes the empty system.
    Ground,
    /// Every left side is linear and every right side is ground.
    /// Under the enforced `Var(rhs) ⊆ Var(lhs)` boundary this is the
    /// literature's linear variable-disjoint class.
    LinearVariableDisjoint,
    /// Every left side is linear and at least one right side is
    /// non-ground (variables shared between sides).
    LeftLinearShared,
    /// Some left side repeats a variable.
    NonLeftLinear,
}

/// A preimage operation a certificate can cover.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum PreimageOperation {
    /// One-step preimage under the application relation (ADR 0001).
    OneStep,
    /// Reflexive `<= n` horizon union.
    FiniteHorizon(u32),
    /// Unbounded saturation.
    Saturation,
}

/// The approved construction behind an exact certificate (ADR 0001).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum PreimageConstruction {
    /// Horizon zero: the result is the input language unchanged.
    Identity,
    /// The left-linear one-step automaton construction.
    LeftLinearOneStep,
    /// Iteration of the one-step construction plus finite unions.
    FiniteHorizonIteration(u32),
    /// GTT-closure saturation (linear variable-disjoint systems).
    GttSaturation,
    /// Task 27: the canonical finite automaton of replayed finite
    /// witnesses — a sound under-approximation issued with `Partial`
    /// authority for cells the ADR approves no exact construction for
    /// (the upper bound is always absent).
    WitnessedLowerBound,
}

/// Whether a class admits an exact construction for an operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub enum PreimageCapability {
    /// An exact construction is approved.
    Exact(PreimageConstruction),
    /// No exact construction is approved: approximation and partial
    /// authority only (the matrix's NO cells).
    ApproximationOnly,
}

impl TrsClass {
    /// The approved capability of this class for an operation.
    /// Horizon zero is exact for every class (the language itself).
    pub fn capability(self, operation: PreimageOperation) -> PreimageCapability {
        if let PreimageOperation::FiniteHorizon(0) = operation {
            return PreimageCapability::Exact(PreimageConstruction::Identity);
        }
        match self {
            TrsClass::Ground | TrsClass::LinearVariableDisjoint => {
                PreimageCapability::Exact(match operation {
                    PreimageOperation::OneStep => PreimageConstruction::LeftLinearOneStep,
                    PreimageOperation::FiniteHorizon(n) => {
                        PreimageConstruction::FiniteHorizonIteration(n)
                    }
                    PreimageOperation::Saturation => PreimageConstruction::GttSaturation,
                })
            }
            TrsClass::LeftLinearShared => match operation {
                PreimageOperation::OneStep => {
                    PreimageCapability::Exact(PreimageConstruction::LeftLinearOneStep)
                }
                PreimageOperation::FiniteHorizon(n) => {
                    PreimageCapability::Exact(PreimageConstruction::FiniteHorizonIteration(n))
                }
                PreimageOperation::Saturation => PreimageCapability::ApproximationOnly,
            },
            TrsClass::NonLeftLinear => PreimageCapability::ApproximationOnly,
        }
    }
}

/// The classification of a validated term rewriting system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize, serde::Deserialize))]
pub struct Classification {
    class: TrsClass,
    rule_count: usize,
}

impl Classification {
    /// The assigned class.
    pub fn class(&self) -> TrsClass {
        self.class
    }

    /// The number of rules classified.
    pub fn rule_count(&self) -> usize {
        self.rule_count
    }

    /// The approved capability for an operation.
    pub fn capability(&self, operation: PreimageOperation) -> PreimageCapability {
        self.class.capability(operation)
    }
}

/// Classify a system under the ADR 0001 input contract.
///
/// Every rule is validated FIRST: a right-side variable not present on
/// the left (admissible only through `Rule::new_unchecked`) is a hard
/// [`RewriteError::InvalidRule`] naming the rule index.
pub fn classify_system(system: &TermSystem) -> RewriteResult<Classification> {
    let rules = system.rules();
    let mut all_ground = true;
    let mut all_left_linear = true;
    let mut all_right_ground = true;
    for (index, rule) in rules.iter().enumerate() {
        let left_variables = rule.lhs().variables();
        for variable in rule.rhs().variables() {
            if !left_variables.contains(&variable) {
                return Err(RewriteError::InvalidRule {
                    message: format!(
                        "rule {index}: right-side variable `{}` is not present on the left side \
                         (ADR 0001 input contract)",
                        variable.as_str()
                    ),
                });
            }
        }
        if !left_variables.is_empty() || !rule.rhs().variables().is_empty() {
            all_ground = false;
        }
        if !is_linear(rule.lhs()) {
            all_left_linear = false;
        }
        if !rule.rhs().variables().is_empty() {
            all_right_ground = false;
        }
    }
    let class = if all_ground {
        TrsClass::Ground
    } else if all_left_linear && all_right_ground {
        TrsClass::LinearVariableDisjoint
    } else if all_left_linear {
        TrsClass::LeftLinearShared
    } else {
        TrsClass::NonLeftLinear
    };
    Ok(Classification {
        class,
        rule_count: rules.len(),
    })
}

/// Validate the ADR 0001 common-alphabet clause: every symbol (name
/// and arity) used by any rule side must belong to the language
/// automaton's ranked alphabet. Failure is
/// [`RewriteError::UnsupportedPreimage`] naming the rule and symbol.
pub fn validate_alphabet(system: &TermSystem, language: &TreeAutomaton) -> RewriteResult<()> {
    let mut ranked: BTreeSet<(&str, usize)> = BTreeSet::new();
    for symbol in language.alphabet() {
        ranked.insert((symbol.symbol().as_str(), usize::from(symbol.arity())));
    }
    for (index, rule) in system.rules().iter().enumerate() {
        validate_side(&ranked, index, rule.lhs())?;
        validate_side(&ranked, index, rule.rhs())?;
    }
    Ok(())
}

fn validate_side(
    ranked: &BTreeSet<(&str, usize)>,
    rule_index: usize,
    term: &Term,
) -> RewriteResult<()> {
    // Iterative (explicit worklist): verification and issuance must
    // never depend on call-stack depth for unmeasured terms.
    let mut stack = alloc::vec::Vec::from([term]);
    while let Some(node) = stack.pop() {
        if let Term::Sym(symbol, arguments) = node {
            let arity = arguments.len();
            if !ranked.contains(&(symbol.as_str(), arity)) {
                return Err(RewriteError::UnsupportedPreimage {
                    message: format!(
                        "rule {rule_index}: symbol `{}` with arity {arity} is not in the language \
                         alphabet (ADR 0001 input contract)",
                        symbol.as_str()
                    ),
                });
            }
            stack.extend(arguments.iter());
        }
    }
    Ok(())
}

fn is_linear(term: &Term) -> bool {
    let mut counts: alloc::collections::BTreeMap<Variable, usize> =
        alloc::collections::BTreeMap::new();
    count_variables(term, &mut counts);
    counts.values().all(|count| *count <= 1)
}

fn count_variables(term: &Term, counts: &mut alloc::collections::BTreeMap<Variable, usize>) {
    // Iterative (explicit worklist): classification runs during
    // verification of arbitrary certificates, so term walks must
    // never depend on call-stack depth.
    let mut stack = alloc::vec::Vec::from([term]);
    while let Some(node) = stack.pop() {
        match node {
            Term::Var(variable) => *counts.entry(variable.clone()).or_insert(0) += 1,
            Term::Sym(_, arguments) => stack.extend(arguments.iter()),
        }
    }
}
