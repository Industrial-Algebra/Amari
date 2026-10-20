// SPDX-License-Identifier: MIT OR Apache-2.0

//! Inverse (bidirectional) rule synthesis from observed steps (Task 37).
//!
//! Relations are usually observed in one direction only: a caller has
//! `(source, target)` pairs produced by a forward rewrite and wants
//! the reverse direction too. [`synthesize_inverse`] answers that
//! without ever guessing:
//!
//! 1. Forward rules are inferred with the Task 36 negative-example
//!    specialization
//!    ([`specialize_rules`](crate::synthesis::specialize_rules)).
//!    Returned rules replay every deduped positive example and
//!    reproduce no supplied negative; no other forward rule is
//!    invented.
//! 2. Each validated forward rule `lhs -> rhs` yields one
//!    [`InverseClause`]: its compiled backward contract
//!    ([`BackwardClause`]) plus a [`ResidualSchema`] over the
//!    variables the forward step erases. A reverse *rule* `rhs -> lhs`
//!    is derived only when the swap is itself a checked rule — that
//!    is, when every variable of the forward left-hand side also
//!    occurs on the forward right-hand side. An erasing rule has no
//!    checked reverse: `rhs -> lhs` would introduce exactly the
//!    variables the forward step erased, so the clause carries the
//!    residual schema those variables need instead of an unchecked
//!    rule (Cohort 2 already records erased bindings under
//!    [`LogicVar::new(0, i)`](LogicVar::new) keys in
//!    [`RewriteResidual`](crate::reversible::RewriteResidual)).
//!
//! # Residual evidence
//!
//! A residual example `(source, target)` is a forward step the caller
//! observed. For an erasing clause it is the evidence that the schema
//! would reconstruct that source from that target:
//!
//! 1. The rule's right-hand side must match `target`, recovering the
//!    non-erased bindings — the reverse instance class.
//! 2. The rule's left-hand side must then match `source` with those
//!    bindings fixed. The binding of each erased variable is the
//!    residual instance the schema has to carry.
//! 3. The reconstructed left-hand side, compared node by node under
//!    those bindings, must equal `source`. Nothing is materialized.
//!
//! A residual example that matches no promoted clause's reverse
//! target is ignored: it may describe a step no inferred rule
//! explains, and refusing the whole run for it would reject valid
//! evidence. A clause that erases variables and matches no residual
//! example is underdetermined, never guessed.
//!
//! # Outcomes
//!
//! Promotion requires three gates, checked in this order, each with
//! canonical (sorted, deduplicated) indices:
//!
//! 1. forward replay covers every deduped positive example;
//! 2. every supplied negative example is rejected by every clause;
//! 3. residual round-trip validates on the residual examples (for a
//!    lossless clause: the checked reverse rule reproduces the source
//!    of every positive it covers).
//!
//! [`InverseSynthesisOutcome`] then reports:
//!
//! - [`InverseSynthesisOutcome::Candidate`] when all gates pass;
//! - [`InverseSynthesisOutcome::Refuted`] with a concrete
//!   [`InverseRefutation`] witness when a gate fails — the evidence
//!   disproves promotion;
//! - [`InverseSynthesisOutcome::Unknown`] with an
//!   [`InverseUnknownReason`] when the evidence does not determine an
//!   answer: underdetermined examples, an erasing clause with no
//!   residual evidence, or residual evidence that gives two different
//!   reconstructions for one reverse instance class. Ambiguity and
//!   underdetermination are never resolved by picking a branch.
//!
//! # Determinism
//!
//! All three input lists are canonicalized by sorting and
//! deduplicating borrowed references, the inferred rule set is
//! already in canonical partition order, and clauses follow the rule
//! order. Permuting any input list therefore leaves the outcome
//! byte-identical.
//!
//! # Limits and metering
//!
//! Raw inputs are validated while still borrowed, iteratively, before
//! any clone, sort, comparison, or variable collection runs, so
//! adversarially deep input is a typed
//! [`RewriteError::RelationLimitExceeded`] instead of a stack abort.
//! [`InverseSynthesisConfig`] carries one [`SpecializationConfig`]
//! (whose [`RelationLimits`] are revalidated at construction, on
//! deserialization, and at every entry point) plus the residual
//! ceiling.
//!
//! One [`RelationResources`] budget covers the whole run, including
//! the nested forward inference: [`specialize_rules_with`](self)
//! charges the shared budget instead of opening a private one, so no
//! nested stage is ever funded by a silently separate allowance.
//! Operations are charged BEFORE the work they meter: per visited
//! node of every walk (admission, variable collection, equality,
//! matching, reconstruction), per binding entry visited (misses
//! included), per compared instance key, per sort/dedup entry, per
//! clone, and per rule or clause retained. There is no fixed cap
//! pretending to be a bound; every violation is typed.

use alloc::{collections::BTreeSet, string::String, vec::Vec};

use crate::{
    relation::{BackwardClause, LogicVar, RelationLimits, RelationResources},
    synthesis::refinement::{
        admit_term, applied_shape, canonical_examples, charged_equal, collect_variables, covers,
        covers_pair, match_pattern, one_level_equals, specialize_rules_with, Bindings,
        InconclusiveReason, SpecializationConfig, SpecializationOutcome,
    },
    trs::{Rule, Term, Variable},
    RewriteError, RewriteResult,
};

/// Caller-tightenable inverse-synthesis limits.
///
/// The associated constants are the hard ceilings; [`Self::new`] only
/// accepts values inside `(0, ceiling]`. Term size, depth, and
/// operation ceilings come from the enclosed
/// [`SpecializationConfig`], which is revalidated (including its
/// [`RelationLimits`]) at construction, on deserialization, and at
/// every entry point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serialize", derive(serde::Serialize))]
pub struct InverseSynthesisConfig {
    specialization: SpecializationConfig,
    max_residuals: usize,
}

#[cfg(feature = "serialize")]
impl<'de> serde::Deserialize<'de> for InverseSynthesisConfig {
    /// Deserialization routes through the checked constructor, which
    /// also revalidates the enclosed [`SpecializationConfig`] and its
    /// [`RelationLimits`]: a raw `{"max_residuals":0,...}` or a
    /// zero/above-ceiling nested limit never becomes a usable config.
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Raw {
            specialization: SpecializationConfig,
            max_residuals: usize,
        }
        let raw = <Raw as serde::Deserialize>::deserialize(deserializer)?;
        Self::new(raw.specialization, raw.max_residuals).map_err(serde::de::Error::custom)
    }
}

impl InverseSynthesisConfig {
    /// Hard ceiling for the total number of deduped residual examples.
    pub const MAX_RESIDUALS: usize = 4_096;

    /// Construct tightened limits. Zero or above-ceiling values are
    /// rejected: limits bound work, they do not disable it.
    pub fn new(specialization: SpecializationConfig, max_residuals: usize) -> RewriteResult<Self> {
        let config = Self {
            specialization,
            max_residuals,
        };
        config.revalidate()?;
        Ok(config)
    }

    /// Revalidate every invariant, including the enclosed
    /// specialization config and its relation limits.
    ///
    /// `RelationLimits` deserializes through its derived impl, which
    /// bypasses [`RelationLimits::new`]; re-running the checked
    /// `SpecializationConfig` constructor here closes that route for a
    /// config received from anywhere (Task 32/36 precedent).
    fn revalidate(&self) -> RewriteResult<()> {
        let specialization = &self.specialization;
        let _checked = SpecializationConfig::new(
            specialization.max_examples(),
            specialization.max_candidates(),
            specialization.max_partitions(),
            *specialization.limits(),
        )?;
        if self.max_residuals == 0 || self.max_residuals > Self::MAX_RESIDUALS {
            return Err(RewriteError::InvalidLimit {
                resource: "residuals",
                value: self.max_residuals,
                ceiling: Self::MAX_RESIDUALS,
            });
        }
        Ok(())
    }

    /// The enclosed forward-specialization configuration.
    pub fn specialization(&self) -> &SpecializationConfig {
        &self.specialization
    }

    /// Maximum total number of deduped residual examples.
    pub fn max_residuals(&self) -> usize {
        self.max_residuals
    }

    /// The enclosed relation limits.
    pub fn limits(&self) -> &RelationLimits {
        self.specialization.limits()
    }
}

impl Default for InverseSynthesisConfig {
    /// The default profile is the fixed ceiling profile.
    fn default() -> Self {
        Self {
            specialization: SpecializationConfig::default(),
            max_residuals: Self::MAX_RESIDUALS,
        }
    }
}

/// The residual schema derived for one promoted clause.
///
/// Keys are the Cohort 2 residual binding keys over the clause's
/// sorted erased variable names, exactly as
/// [`RewriteResidual`](crate::reversible::RewriteResidual) records
/// them: `LogicVar::new(0, 0)`, `LogicVar::new(0, 1)`, and so on in
/// name order. An empty schema means the reverse direction is a
/// checked rule and needs no residual authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResidualSchema {
    bindings: Vec<(LogicVar, Variable)>,
}

impl ResidualSchema {
    /// Derive the schema for an already sorted, deduplicated erased
    /// variable list, charging per key before the key is built and
    /// per variable before its name is cloned.
    fn derive(erased: &[Variable], resources: &mut RelationResources) -> RewriteResult<Self> {
        resources.record_operations(erased.len())?;
        let mut bindings = Vec::with_capacity(erased.len());
        for (index, variable) in erased.iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| RewriteError::RelationLimitExceeded {
                resource: "residual schema",
                limit: u32::MAX as usize,
            })?;
            resources.record_operations(1)?;
            bindings.push((LogicVar::new(0, index), variable.clone()));
        }
        Ok(Self { bindings })
    }

    /// The schema entries as `(key, erased variable)` pairs in key
    /// order.
    pub fn bindings(&self) -> &[(LogicVar, Variable)] {
        &self.bindings
    }

    /// Number of erased variables the reverse direction must recover.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// True exactly when the reverse direction erases nothing.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// The erased variable names in key order.
    pub fn variables(&self) -> impl Iterator<Item = &Variable> {
        self.bindings.iter().map(|(_, variable)| variable)
    }
}

/// One promoted inverse clause.
///
/// [`Self::forward`] is the checked forward rule, [`Self::backward`]
/// its compiled backward contract, and [`Self::residual_schema`] the
/// residual authority the reverse direction needs.
/// [`Self::reverse_rule`] is `Some` exactly when the reverse
/// direction is itself a checked rule; for an erasing clause it is
/// `None` and the schema carries the erased variables instead of an
/// unchecked `rhs -> lhs` that would invent them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InverseClause {
    forward: Rule,
    backward: BackwardClause,
    residual: ResidualSchema,
    reverse: Option<Rule>,
}

impl InverseClause {
    /// The checked forward rule the clause was derived from.
    pub fn forward(&self) -> &Rule {
        &self.forward
    }

    /// The compiled backward contract for the forward rule.
    pub fn backward(&self) -> &BackwardClause {
        &self.backward
    }

    /// The residual schema the reverse direction needs.
    pub fn residual_schema(&self) -> &ResidualSchema {
        &self.residual
    }

    /// The checked reverse rule, present exactly when the forward
    /// rule erased nothing.
    pub fn reverse_rule(&self) -> Option<&Rule> {
        self.reverse.as_ref()
    }

    /// True when the reverse direction is a checked rule.
    pub fn is_reversible(&self) -> bool {
        self.reverse.is_some()
    }
}

/// Validation evidence for a promoted candidate.
///
/// Every count describes work that actually ran; the indices used by
/// [`InverseRefutation`] are into the corresponding canonical
/// (sorted, deduplicated) input list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InverseEvidence {
    positives: usize,
    negatives: usize,
    residuals: usize,
    forward_replays: usize,
    negative_rejections: usize,
    residual_roundtrips: usize,
    reversible_clauses: usize,
    residual_clauses: usize,
}

impl InverseEvidence {
    /// Number of deduped positive examples replayed forward.
    pub fn positives(&self) -> usize {
        self.positives
    }

    /// Number of deduped negative examples rejected.
    pub fn negatives(&self) -> usize {
        self.negatives
    }

    /// Number of deduped residual examples supplied.
    pub fn residuals(&self) -> usize {
        self.residuals
    }

    /// Number of `(positive, clause)` forward replay checks.
    pub fn forward_replays(&self) -> usize {
        self.forward_replays
    }

    /// Number of `(negative, clause)` rejection checks.
    pub fn negative_rejections(&self) -> usize {
        self.negative_rejections
    }

    /// Number of residual round-trip checks performed.
    pub fn residual_roundtrips(&self) -> usize {
        self.residual_roundtrips
    }

    /// Number of clauses whose reverse direction is a checked rule.
    pub fn reversible_clauses(&self) -> usize {
        self.reversible_clauses
    }

    /// Number of clauses that carry a non-empty residual schema.
    pub fn residual_clauses(&self) -> usize {
        self.residual_clauses
    }
}

/// A concrete witness that refutes promotion.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InverseWitness {
    refutation: InverseRefutation,
}

impl InverseWitness {
    /// What the witness shows.
    pub fn refutation(&self) -> &InverseRefutation {
        &self.refutation
    }
}

/// What a refuting witness shows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InverseRefutation {
    /// No promoted clause replays this positive example forward, so the
    /// candidate rule set does not explain evidence it must explain.
    ForwardReplay {
        /// Canonical index of the unexplained positive example.
        positive: usize,
    },
    /// A promoted clause reproduces this negative example.
    NegativeAccepted {
        /// Canonical index of the accepted negative example.
        negative: usize,
        /// Index of the clause that reproduces it.
        clause: usize,
    },
    /// Residual evidence does not reconstruct the claimed source from
    /// its target under this clause.
    ResidualRoundTrip {
        /// Canonical index of the refuting residual example.
        residual: usize,
        /// Index of the clause it fails against.
        clause: usize,
    },
    /// The checked reverse of a lossless clause does not reproduce the
    /// source of a positive example the forward rule covers.
    ReverseReplay {
        /// Canonical index of the unrefuted positive example.
        positive: usize,
        /// Index of the clause whose reverse fails.
        clause: usize,
    },
}

/// Why no bidirectional system was promoted.
///
/// Every variant reports that the evidence does not determine an
/// answer. None of them asserts that no bidirectional system exists,
/// and none of them is resolved by choosing a branch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InverseUnknownReason {
    /// The positive and negative examples do not determine a checked
    /// forward rule set.
    Underdetermined {
        /// The specialization outcome that stopped the search.
        detail: InconclusiveReason,
    },
    /// An erasing clause has no residual evidence, so the reconstruction
    /// its schema needs is not determined.
    ResidualUnderdetermined {
        /// Index of the unvalidated clause.
        clause: usize,
    },
    /// Residual evidence gives two different reconstructions for one
    /// reverse instance class; promoting either would be a guess.
    Ambiguous {
        /// Index of the conflicted clause.
        clause: usize,
    },
}

/// The typed result of inverse synthesis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InverseSynthesisOutcome {
    /// Validated forward rules with their inverse clauses, in canonical
    /// order.
    Candidate {
        /// The checked forward rules.
        rules: Vec<Rule>,
        /// One derived clause per rule, in the same order.
        clauses: Vec<InverseClause>,
        /// Replay, rejection, and round-trip evidence.
        evidence: InverseEvidence,
    },
    /// A concrete witness disproved promotion.
    Refuted {
        /// What the witness shows.
        witness: InverseWitness,
    },
    /// The evidence does not determine a candidate.
    Unknown {
        /// Why the search stopped.
        reason: InverseUnknownReason,
    },
}

/// Synthesize inverse clauses from positive, negative, and residual
/// examples.
///
/// Positive examples are `(source, target)` forward steps; negative
/// examples are `(source, target)` pairs the forward rule set must not
/// reproduce; residual examples are `(source, target)` steps used to
/// validate the residual schema of erasing clauses.
///
/// Returns [`InverseSynthesisOutcome::Candidate`] only when every
/// deduped positive replays forward, every negative is rejected, and
/// residual round-trip validates; otherwise the typed
/// [`InverseSynthesisOutcome::Refuted`] or
/// [`InverseSynthesisOutcome::Unknown`] outcome is returned. Indices
/// in witnesses and reasons refer to canonical (sorted, deduplicated)
/// input order, so they do not depend on caller order.
///
/// # Errors
///
/// - [`RewriteError::InvalidRule`] when there are no positive
///   examples, or when one example pair is supplied as both positive
///   and negative.
/// - [`RewriteError::InvalidLimit`] when the configuration (including
///   the enclosed specialization config and relation limits) violates
///   its invariants.
/// - [`RewriteError::RelationLimitExceeded`] when an example,
///   residual, candidate, partition, term node/depth, or operation
///   ceiling is exceeded.
pub fn synthesize_inverse(
    positives: &[(Term, Term)],
    negatives: &[(Term, Term)],
    residuals: &[(Term, Term)],
    config: &InverseSynthesisConfig,
) -> RewriteResult<InverseSynthesisOutcome> {
    // A config is a public value: revalidate it at the entry point.
    config.revalidate()?;
    if positives.is_empty() {
        return Err(RewriteError::InvalidRule {
            message: String::from("cannot synthesize an inverse without positive examples"),
        });
    }

    let limits = *config.limits();
    let mut resources = RelationResources::new(&limits);

    // 1. Raw admission over the BORROWED inputs, iteratively. Nothing
    // below may sort, dedup, compare, or clone an unvalidated term: a
    // deep term must fail typed here instead of aborting in a
    // recursive clone or comparison.
    admit_examples(positives, &limits, &mut resources)?;
    admit_examples(negatives, &limits, &mut resources)?;
    admit_examples(residuals, &limits, &mut resources)?;

    // 2. Canonicalize borrowed references (charged comparisons).
    let positive_examples = canonical_examples(positives, &mut resources)?;
    let negative_examples = canonical_examples(negatives, &mut resources)?;
    let residual_examples = canonical_examples(residuals, &mut resources)?;
    if residual_examples.len() > config.max_residuals() {
        return Err(RewriteError::RelationLimitExceeded {
            resource: "residuals",
            limit: config.max_residuals(),
        });
    }

    // 3. Checked forward inference, charged to this run's single
    // budget so nested work is never funded separately.
    let inferred = specialize_rules_with(
        positives,
        negatives,
        config.specialization(),
        &mut resources,
    )?;
    let rules = match inferred {
        SpecializationOutcome::Refined { rules, coverage: _ } => rules,
        SpecializationOutcome::Inconclusive { reason } => {
            return Ok(InverseSynthesisOutcome::Unknown {
                reason: InverseUnknownReason::Underdetermined { detail: reason },
            });
        }
    };

    // 4. Derive one clause per validated forward rule.
    resources.record_operations(rules.len())?;
    let mut clauses = Vec::with_capacity(rules.len());
    for rule in &rules {
        clauses.push(derive_clause(rule, &limits, &mut resources)?);
    }

    // 5. Gate 1: forward replay covers every deduped positive.
    let mut forward_replays = 0usize;
    for (positive, example) in positive_examples.iter().copied().enumerate() {
        let mut replayed = false;
        for clause in &clauses {
            forward_replays = forward_replays.saturating_add(1);
            if covers(clause.forward(), example, &limits, &mut resources)? {
                replayed = true;
                break;
            }
        }
        if !replayed {
            return Ok(InverseSynthesisOutcome::Refuted {
                witness: InverseWitness {
                    refutation: InverseRefutation::ForwardReplay { positive },
                },
            });
        }
    }

    // 6. Gate 2: no clause reproduces a negative example.
    let mut negative_rejections = 0usize;
    for (negative, example) in negative_examples.iter().copied().enumerate() {
        for (clause_index, clause) in clauses.iter().enumerate() {
            negative_rejections = negative_rejections.saturating_add(1);
            if covers(clause.forward(), example, &limits, &mut resources)? {
                return Ok(InverseSynthesisOutcome::Refuted {
                    witness: InverseWitness {
                        refutation: InverseRefutation::NegativeAccepted {
                            negative,
                            clause: clause_index,
                        },
                    },
                });
            }
        }
    }

    // 7. Gate 3: residual round-trip validates every clause.
    let mut residual_roundtrips = 0usize;
    let mut reversible_clauses = 0usize;
    let mut residual_clauses = 0usize;
    for (clause_index, clause) in clauses.iter().enumerate() {
        let Some(reverse) = clause.reverse_rule() else {
            residual_clauses = residual_clauses.saturating_add(1);
            let verdict = validate_residual_evidence(
                clause,
                &residual_examples,
                &limits,
                &mut resources,
                &mut residual_roundtrips,
            )?;
            match verdict {
                ResidualVerdict::Validated => {}
                ResidualVerdict::Refuted(residual) => {
                    return Ok(InverseSynthesisOutcome::Refuted {
                        witness: InverseWitness {
                            refutation: InverseRefutation::ResidualRoundTrip {
                                residual,
                                clause: clause_index,
                            },
                        },
                    });
                }
                ResidualVerdict::Underdetermined => {
                    return Ok(InverseSynthesisOutcome::Unknown {
                        reason: InverseUnknownReason::ResidualUnderdetermined {
                            clause: clause_index,
                        },
                    });
                }
                ResidualVerdict::Ambiguous => {
                    return Ok(InverseSynthesisOutcome::Unknown {
                        reason: InverseUnknownReason::Ambiguous {
                            clause: clause_index,
                        },
                    });
                }
            }
            continue;
        };

        reversible_clauses = reversible_clauses.saturating_add(1);
        for (positive, example) in positive_examples.iter().copied().enumerate() {
            // The reverse direction is only evidence for steps this
            // rule actually fires on.
            if !covers(clause.forward(), example, &limits, &mut resources)? {
                continue;
            }
            residual_roundtrips = residual_roundtrips.saturating_add(1);
            if !covers_pair(reverse, &example.1, &example.0, &limits, &mut resources)? {
                return Ok(InverseSynthesisOutcome::Refuted {
                    witness: InverseWitness {
                        refutation: InverseRefutation::ReverseReplay {
                            positive,
                            clause: clause_index,
                        },
                    },
                });
            }
        }
    }

    let evidence = InverseEvidence {
        positives: positive_examples.len(),
        negatives: negative_examples.len(),
        residuals: residual_examples.len(),
        forward_replays,
        negative_rejections,
        residual_roundtrips,
        reversible_clauses,
        residual_clauses,
    };
    Ok(InverseSynthesisOutcome::Candidate {
        rules,
        clauses,
        evidence,
    })
}

/// Admit every example pair while still borrowed, charging one unit
/// per pair and per visited node of both sides.
fn admit_examples(
    examples: &[(Term, Term)],
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<()> {
    for example in examples {
        resources.record_operations(1)?;
        let _lhs_shape = admit_term(&example.0, limits, resources)?;
        let _rhs_shape = admit_term(&example.1, limits, resources)?;
    }
    Ok(())
}

/// Derive the clause for one checked forward rule.
///
/// The backward contract compilation rescans both sides, sorts and
/// deduplicates the erased set, and clones both terms; every one of
/// those units is charged before the compile runs.
fn derive_clause(
    rule: &Rule,
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<InverseClause> {
    let (lhs_nodes, _lhs_depth) = admit_term(rule.lhs(), limits, resources)?;
    let (rhs_nodes, _rhs_depth) = admit_term(rule.rhs(), limits, resources)?;
    resources.record_operations(lhs_nodes.saturating_add(rhs_nodes))?;
    let mut lhs_variables = BTreeSet::new();
    collect_variables(rule.lhs(), &mut lhs_variables, resources)?;
    let mut rhs_variables = BTreeSet::new();
    collect_variables(rule.rhs(), &mut rhs_variables, resources)?;
    resources.record_operations(lhs_variables.len().saturating_add(rhs_variables.len()))?;

    // Defensive re-verification of the checked-rule invariant.
    if let Some(missing) = rhs_variables
        .iter()
        .find(|variable| !lhs_variables.contains(*variable))
    {
        return Err(RewriteError::InvalidRule {
            message: alloc::format!(
                "forward rule rhs variable {} does not occur in \
                 the lhs",
                missing.as_str()
            ),
        });
    }

    // Erased variables in sorted (BTreeSet) order: this is the schema
    // order the residual keys are built from.
    resources.record_operations(lhs_variables.len())?;
    let mut erased: Vec<Variable> = Vec::new();
    for variable in &lhs_variables {
        if !rhs_variables.contains(variable) {
            erased.push(variable.clone());
        }
    }

    let backward = BackwardClause::compile(rule)?;
    let residual = ResidualSchema::derive(&erased, resources)?;

    // A reverse rule exists only when the swap is checked: an erasing
    // forward rule would make `rhs -> lhs` invent its erased
    // variables. Never synthesize that unchecked rule.
    let reverse = if erased.is_empty() {
        resources.record_operations(lhs_nodes.saturating_add(rhs_nodes))?;
        let reversed = Rule::new(rule.rhs().clone(), rule.lhs().clone()).ok();
        if reversed.is_none() {
            return Err(RewriteError::InvalidRule {
                message: String::from("lossless forward rule has no checked reverse"),
            });
        }
        reversed
    } else {
        None
    };

    // Retention copies the forward rule into the clause: charge both
    // sides per node BEFORE the clone is allocated.
    resources.record_operations(lhs_nodes.saturating_add(rhs_nodes))?;
    Ok(InverseClause {
        forward: rule.clone(),
        backward,
        residual,
        reverse,
    })
}

/// The outcome of validating one clause's residual evidence.
enum ResidualVerdict {
    Validated,
    Refuted(usize),
    Underdetermined,
    Ambiguous,
}

/// One validated residual instance: the reverse instance class (the
/// non-erased bindings) and the residual value (the erased bindings).
struct ResidualInstance {
    key: Vec<(Variable, Term)>,
    value: Vec<Term>,
}

impl ResidualInstance {
    /// Compare the residual values with charged structural equality.
    fn same_value(&self, probe: &Self, resources: &mut RelationResources) -> RewriteResult<bool> {
        resources.record_operations(1)?;
        if self.value.len() != probe.value.len() {
            return Ok(false);
        }
        for (left, right) in self.value.iter().zip(&probe.value) {
            if !charged_equal(left, right, resources)? {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Validate the residual schema of one erasing clause against the
/// residual examples.
///
/// Every walk is charged: the reverse-target match, the source match,
/// the prospective reconstruction shape, the node-by-node comparison,
/// and every captured binding and key comparison.
fn validate_residual_evidence(
    clause: &InverseClause,
    residuals: &[&(Term, Term)],
    limits: &RelationLimits,
    resources: &mut RelationResources,
    roundtrips: &mut usize,
) -> RewriteResult<ResidualVerdict> {
    let forward = clause.forward();
    let schema = clause.residual_schema();

    // Key variables: the LHS variables the schema does not erase, in
    // sorted order. They identify the reverse instance class.
    let mut lhs_variables = BTreeSet::new();
    collect_variables(forward.lhs(), &mut lhs_variables, resources)?;
    let erased: Vec<&Variable> = schema.variables().collect();
    resources.record_operations(lhs_variables.len().saturating_add(erased.len()))?;
    let mut key_variables: Vec<&Variable> = Vec::new();
    for variable in &lhs_variables {
        if !erased.contains(&variable) {
            key_variables.push(variable);
        }
    }

    let mut instances: Vec<ResidualInstance> = Vec::new();
    for (residual_index, example) in residuals.iter().copied().enumerate() {
        // Reverse-target match: the rule RHS must match the target,
        // recovering the non-erased bindings.
        let mut bindings = Bindings::new();
        if !match_pattern(forward.rhs(), &example.1, &mut bindings, resources)? {
            // This example describes a step no clause reverses.
            continue;
        }
        // The claimed source must be an instance of the rule LHS with
        // the recovered bindings fixed; a mismatch refutes the claim.
        if !match_pattern(forward.lhs(), &example.0, &mut bindings, resources)? {
            return Ok(ResidualVerdict::Refuted(residual_index));
        }
        *roundtrips = roundtrips.saturating_add(1);

        // Residual round trip: the reconstruction from the LHS template
        // under the recovered bindings must equal the claimed source.
        // The shape is accounted before anything could be materialized,
        // and nothing ever is.
        let _shape = applied_shape(forward.lhs(), &bindings, limits, resources)?;
        if !one_level_equals(forward.lhs(), &bindings, &example.0, resources)? {
            return Ok(ResidualVerdict::Refuted(residual_index));
        }

        let instance = capture_instance(&key_variables, schema, &bindings, limits, resources)?;
        match find_instance(&instances, &instance, resources)? {
            Some(existing) => {
                if !existing.same_value(&instance, resources)? {
                    return Ok(ResidualVerdict::Ambiguous);
                }
            }
            None => instances.push(instance),
        }
    }

    if instances.is_empty() {
        return Ok(ResidualVerdict::Underdetermined);
    }
    Ok(ResidualVerdict::Validated)
}

/// Capture one residual instance, charging per clone before it is
/// allocated.
fn capture_instance(
    key_variables: &[&Variable],
    schema: &ResidualSchema,
    bindings: &Bindings<'_>,
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<ResidualInstance> {
    resources.record_operations(key_variables.len().saturating_add(schema.len()))?;
    let mut key = Vec::with_capacity(key_variables.len());
    for variable in key_variables {
        let Some(term) = bindings.get(variable, resources)? else {
            return Err(RewriteError::InvalidRule {
                message: String::from("residual key variable is unbound"),
            });
        };
        let _shape = admit_term(term, limits, resources)?;
        resources.record_operations(1)?;
        key.push(((*variable).clone(), term.clone()));
    }
    let mut value = Vec::with_capacity(schema.len());
    for variable in schema.variables() {
        let Some(term) = bindings.get(variable, resources)? else {
            return Err(RewriteError::InvalidRule {
                message: String::from("residual schema variable is unbound"),
            });
        };
        let _shape = admit_term(term, limits, resources)?;
        resources.record_operations(1)?;
        value.push(term.clone());
    }
    Ok(ResidualInstance { key, value })
}

/// Find the instance with the same reverse instance class, charging
/// every visited entry and every compared key node.
fn find_instance<'a>(
    instances: &'a [ResidualInstance],
    probe: &ResidualInstance,
    resources: &mut RelationResources,
) -> RewriteResult<Option<&'a ResidualInstance>> {
    for instance in instances {
        resources.record_operations(1)?;
        if instance.key.len() != probe.key.len() {
            continue;
        }
        let mut equal = true;
        for ((left_variable, left_term), (right_variable, right_term)) in
            instance.key.iter().zip(&probe.key)
        {
            resources.record_operations(1)?;
            if left_variable != right_variable || !charged_equal(left_term, right_term, resources)?
            {
                equal = false;
                break;
            }
        }
        if equal {
            return Ok(Some(instance));
        }
    }
    Ok(None)
}
