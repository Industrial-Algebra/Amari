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
//! All three input lists are canonicalized into structural order and
//! deduplicated from borrowed references, the inferred rule set is
//! already in canonical partition order, and clauses follow the rule
//! order. The canonicalization charge is order independent (every
//! unordered example pair is ranked once), so permuting any input list
//! leaves the outcome byte-identical at ANY operation budget —
//! including one tight enough to exhaust, which is then the same typed
//! error for every order (PR #338 closeout P1/P2).
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
    /// A limits value can no longer arrive unvalidated — [`RelationLimits`]
    /// validates its own wire form (PR #353 round 4 P2b) — but re-running
    /// the checked `SpecializationConfig` constructor here keeps this
    /// config's authority a function of its own checked fields, whatever
    /// produced them (Task 32/36 precedent).
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
    /// Derive the schema from the compiled contract's sorted erased
    /// names, charging two units per entry (the key and the cloned
    /// name) BEFORE the entry is built.
    fn derive(erased: &[String], resources: &mut RelationResources) -> RewriteResult<Self> {
        let mut bindings = Vec::with_capacity(erased.len());
        for (index, name) in erased.iter().enumerate() {
            let index = u32::try_from(index).map_err(|_| RewriteError::RelationLimitExceeded {
                resource: "residual schema",
                limit: u32::MAX as usize,
            })?;
            resources.record_operations(2)?;
            bindings.push((LogicVar::new(0, index), Variable::new(name.clone())));
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

    // 7. Gate 3a: every lossless clause replays its checked reverse
    // over each positive the forward rule covers. Every clause visit is
    // charged BEFORE the clause is inspected, so an erasing clause's
    // dismissal is metered too (PR #335 round 3 P2).
    let mut residual_roundtrips = 0usize;
    let mut reversible_clauses = 0usize;
    let mut residual_clauses = 0usize;
    for (clause_index, clause) in clauses.iter().enumerate() {
        resources.record_operations(1)?;
        let Some(reverse) = clause.reverse_rule() else {
            residual_clauses = residual_clauses.saturating_add(1);
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

    // 8. Gate 3b: one global pass validates the residual schema of
    // every erasing clause. A residual is obligated only to the
    // clauses whose reverse target it matches, so an overlapping
    // right-hand side never refutes another clause's evidence.
    match validate_residual_evidence(
        &clauses,
        &residual_examples,
        &limits,
        &mut resources,
        &mut residual_roundtrips,
    )? {
        ResidualVerdict::Validated => {}
        ResidualVerdict::Refuted { residual, clause } => {
            return Ok(InverseSynthesisOutcome::Refuted {
                witness: InverseWitness {
                    refutation: InverseRefutation::ResidualRoundTrip { residual, clause },
                },
            });
        }
        ResidualVerdict::Underdetermined { clause } => {
            return Ok(InverseSynthesisOutcome::Unknown {
                reason: InverseUnknownReason::ResidualUnderdetermined { clause },
            });
        }
        ResidualVerdict::Ambiguous { clause } => {
            return Ok(InverseSynthesisOutcome::Unknown {
                reason: InverseUnknownReason::Ambiguous { clause },
            });
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
/// The backward contract compilation is metered internally: variable
/// collection, the quadratic RHS-subset and erased-set scans, the rule
/// identity digest, and the two-sided clone all charge the shared
/// budget per visited node and per visited entry (PR #335 round 1
/// P2a). The residual schema is derived from the contract's own erased
/// set, so clause, schema, and digest can never disagree.
fn derive_clause(
    rule: &Rule,
    limits: &RelationLimits,
    resources: &mut RelationResources,
) -> RewriteResult<InverseClause> {
    // Enforce the per-term ceilings before any of the rule's work runs.
    let (lhs_nodes, _lhs_depth) = admit_term(rule.lhs(), limits, resources)?;
    let (rhs_nodes, _rhs_depth) = admit_term(rule.rhs(), limits, resources)?;

    let backward = BackwardClause::compile_metered(rule, resources)?;
    let residual = ResidualSchema::derive(backward.erased_variables(), resources)?;

    // A reverse rule exists only when the swap is checked: an erasing
    // forward rule would make `rhs -> lhs` invent its erased
    // variables. Never synthesize that unchecked rule.
    //
    // The swap is checked BY CONSTRUCTION here: the metered compilation
    // proved RHS ⊆ LHS and the schema is empty, which is exactly
    // LHS ⊆ RHS, so the checked constructor would only rescan both
    // sides — an unmetered variable walk and sort we do not need. The
    // unchecked constructor is reached solely on that proved invariant
    // (PR #335 round 2: no unmetered scan on this path).
    let reverse = if residual.is_empty() {
        resources.record_operations(lhs_nodes.saturating_add(rhs_nodes))?;
        Some(Rule::new_unchecked(rule.rhs().clone(), rule.lhs().clone()))
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

/// The residual gate's verdict after the global residual pass.
enum ResidualVerdict {
    Validated,
    Refuted { residual: usize, clause: usize },
    Underdetermined { clause: usize },
    Ambiguous { clause: usize },
}

/// Residual evidence collected for one erasing clause.
struct ClauseResiduals {
    key_variables: Vec<Variable>,
    instances: Vec<ResidualInstance>,
    ambiguous: bool,
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

/// Validate the residual schema of every erasing clause in one global
/// pass.
///
/// Obligation is scoped per residual: a residual is offered only to
/// those clauses whose reverse target (the rule's right-hand side) it
/// matches, and it is accepted by the first such clause that also
/// reconstructs its claimed source. Only a residual that EVERY
/// target-compatible clause rejects is a refutation, and a residual
/// that no erasing clause targets is ignored by contract (PR #335
/// round 1 P1: two clauses sharing a right-hand side used to refute
/// each other's evidence).
///
/// Every walk charges the shared budget per visited node and per
/// visited entry: schema-entry collection, the reverse-instance key
/// selection scan, the target and source matches, the prospective
/// reconstruction shape, the node-by-node comparison, and every
/// captured binding and key comparison (PR #335 round 1 P2b).
fn validate_residual_evidence(
    clauses: &[InverseClause],
    residuals: &[&(Term, Term)],
    limits: &RelationLimits,
    resources: &mut RelationResources,
    roundtrips: &mut usize,
) -> RewriteResult<ResidualVerdict> {
    // Per-clause reverse instance keys. The charge covers the capacity
    // of the evidence vector and one visit per clause; the loop below
    // adds the per-node and per-entry work.
    resources.record_operations(clauses.len())?;
    let mut evidence: Vec<Option<ClauseResiduals>> = Vec::with_capacity(clauses.len());
    let mut erasing_clauses = 0usize;
    for clause in clauses {
        if clause.residual_schema().is_empty() {
            evidence.push(None);
            continue;
        }
        erasing_clauses = erasing_clauses.saturating_add(1);
        let mut lhs_variables = BTreeSet::new();
        collect_variables(clause.forward().lhs(), &mut lhs_variables, resources)?;
        let schema = clause.residual_schema();
        // Charge the schema-entry collection BEFORE it allocates.
        resources.record_operations(schema.len())?;
        let erased: Vec<&Variable> = schema.variables().collect();
        let mut key_variables: Vec<Variable> = Vec::new();
        for variable in &lhs_variables {
            // Metered selection scan: every visited erased entry is
            // charged BEFORE it is compared.
            let mut erased_here = false;
            for candidate in &erased {
                resources.record_operations(1)?;
                if *candidate == variable {
                    erased_here = true;
                    break;
                }
            }
            if !erased_here {
                resources.record_operations(1)?;
                key_variables.push(variable.clone());
            }
        }
        evidence.push(Some(ClauseResiduals {
            key_variables,
            instances: Vec::new(),
            ambiguous: false,
        }));
    }

    // Bypass: with no erasing clause, no residual can be obligated, so
    // the traversal below would only visit clauses that are not
    // candidates. It is skipped entirely; the scan that established
    // this fact was charged one unit per clause above, so the bypass
    // condition itself is metered (PR #335 round 2 P2b).
    if erasing_clauses == 0 {
        return Ok(ResidualVerdict::Validated);
    }

    for (residual_index, example) in residuals.iter().copied().enumerate() {
        // Charge the attempt itself: an unrelated residual still costs
        // a visit before it can be dismissed.
        resources.record_operations(1)?;
        let mut accepted = false;
        let mut rejected_by: Option<usize> = None;
        for (clause_index, clause) in clauses.iter().enumerate() {
            // Charge every visited clause BEFORE inspecting it.
            resources.record_operations(1)?;
            let Some(meter) = evidence[clause_index].as_mut() else {
                continue;
            };
            // Compatibility: the rule RHS must match the target,
            // recovering the non-erased bindings.
            let mut bindings = Bindings::new();
            if !match_pattern(clause.forward().rhs(), &example.1, &mut bindings, resources)? {
                continue;
            }
            // The claimed source must be an instance of the rule LHS
            // with those bindings fixed, and the reconstruction must
            // compare equal to it. Nothing is materialized.
            let mut reconstructs =
                match_pattern(clause.forward().lhs(), &example.0, &mut bindings, resources)?;
            if reconstructs {
                let _shape = applied_shape(clause.forward().lhs(), &bindings, limits, resources)?;
                reconstructs =
                    one_level_equals(clause.forward().lhs(), &bindings, &example.0, resources)?;
            }
            if !reconstructs {
                if rejected_by.is_none() {
                    rejected_by = Some(clause_index);
                }
                continue;
            }

            *roundtrips = roundtrips.saturating_add(1);
            let instance = capture_instance(
                &meter.key_variables,
                clause.residual_schema(),
                &bindings,
                limits,
                resources,
            )?;
            match find_instance(&meter.instances, &instance, resources)? {
                Some(existing) => {
                    if !existing.same_value(&instance, resources)? {
                        meter.ambiguous = true;
                    }
                }
                None => meter.instances.push(instance),
            }
            accepted = true;
            break;
        }
        if !accepted {
            if let Some(clause) = rejected_by {
                return Ok(ResidualVerdict::Refuted {
                    residual: residual_index,
                    clause,
                });
            }
            // No erasing clause targets this residual: ignored.
        }
    }

    // Final evidence scans: one visit per clause each, charged before
    // the scan runs.
    // Conflicting reconstructions are reported before missing
    // evidence: both are Unknown, and clause order is canonical.
    resources.record_operations(evidence.len())?;
    for (clause_index, meter) in evidence.iter().enumerate() {
        if let Some(meter) = meter {
            if meter.ambiguous {
                return Ok(ResidualVerdict::Ambiguous {
                    clause: clause_index,
                });
            }
        }
    }
    resources.record_operations(evidence.len())?;
    for (clause_index, meter) in evidence.iter().enumerate() {
        if let Some(meter) = meter {
            if meter.instances.is_empty() {
                return Ok(ResidualVerdict::Underdetermined {
                    clause: clause_index,
                });
            }
        }
    }
    Ok(ResidualVerdict::Validated)
}

/// Capture one residual instance, charging per clone before it is
/// allocated.
fn capture_instance(
    key_variables: &[Variable],
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
        key.push((variable.clone(), term.clone()));
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
