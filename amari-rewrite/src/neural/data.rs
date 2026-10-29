// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pairwise inverse-ranking training data derived from replayed traces.
//!
//! A dataset is a list of pairwise examples — a chosen candidate and a
//! valid-but-unchosen candidate from the same expansion round — featurized
//! through Task 40's frozen [`FeatureEncoder`]. It is Task 42's training
//! input: this module derives and emits records, and does no training.
//!
//! ## Label authority
//!
//! [`LabelAuthority::ProofPositive`] is reserved for examples derived from
//! *exact* replay-validated traces ([`GuidanceMode::CompleteWithinLimits`]).
//! An approximate trace ([`GuidanceMode::HeuristicPruning`]) is rejected
//! unless the dataset configuration explicitly admits it, and its examples
//! are always labeled [`LabelAuthority::HeuristicGuidance`] — approximate
//! evidence can never become proof data, whatever the configuration.
//!
//! ## Negatives are re-derived, never invented
//!
//! At each replayed state the symbolic engine
//! ([`crate::inverse::symbolic_predecessors`]) is re-run — it is the only
//! transition generator. The chosen transition is located among the
//! re-derived candidates by provenance, and every other candidate is a
//! negative. If the chosen transition is absent from the re-derivation the
//! trace is rejected with [`NeuralError::TraceDrift`]: a trace/engine drift
//! never fabricates a negative set. Duplicate candidates collapse by
//! canonical digest.
//!
//! The order of those two steps is load-bearing. Two distinct rules can
//! derive the *same* state (for example `f(X) -> X` and `f(a) -> a` from
//! `a`), so provenance — not the state digest — is the only thing that
//! identifies the transition the search actually took. The chosen transition
//! is therefore located in the **full** re-derived candidate set, before any
//! duplicate collapses; deduplication then applies to the negatives, with the
//! chosen state admitted once. The same full set is what the search saw, so
//! the feature context's candidate-count lanes (the branch factor) are
//! reconstructed from it *before* deduplication: the example records the
//! branching the engine faced, not the size of the surviving negative list.
//!
//! ## Determinism
//!
//! The dataset is a pure function of `(trace set, config, FeatureSchemaV1
//! digest)`. Examples are ordered by a content key built from the same
//! provenance/context fields Task 40 ranks by, so no permutation of the
//! input trace set can change the output. A seed is used *only* for the
//! documented ceiling sub-sampling, and it is recorded in the manifest.
//!
//! ## Bounded admission
//!
//! Admission runs before verification and before any digest, over the
//! **whole** certificate: the trace count, the step counts of *both*
//! derivation halves (forward and backward), both certified query terms,
//! every step (including both meeting terms, the unifier, and the
//! existential vector), every residual constraint side, and every
//! substitution range together with its transitive resolution chain *and*
//! the expanded structure resolving it composes. Every embedded term's node
//! count and depth are checked against [`RelationLimits`], and every
//! identifier — symbol and variable name alike — against
//! [`FeatureEncoder::MAX_IDENTIFIER_BYTES`], all with a bounded iterative
//! walk, *before* [`ReplayCertificate::verify`]
//! replays (which hashes and clones terms, and resolves substitution chains
//! recursively). No unbounded allocation or recursion can be reached from
//! an untrusted trace, and nothing is hashed or cloned before it is
//! admitted.

use alloc::collections::{BTreeMap, BTreeSet};

use crate::inverse::{
    query_hash, ForwardStep, GuidanceMode, ReplayCertificate, ReplayDerivation, ReplayQuery,
    SymbolicPredecessor,
};
use crate::neural::encode::{CandidateInput, FeatureEncoder, FeatureVector, SearchContext};
use crate::neural::model::{canonical_digest, content_key};
use crate::neural::{FeatureSchemaV1, NeuralError};
use crate::relation::digest::encode_term;
use crate::relation::{
    ConstraintSet, LogicVar, RelationLimits, RuleId, Sha256Digest, TermConstraint,
};
use crate::trs::{Substitution, Term, TermSystem, Variable};
use crate::Path;

/// Schema version of the dataset manifest, bumped only by a deliberate
/// incompatible change to the manifest layout.
pub const DATASET_MANIFEST_SCHEMA_VERSION: u32 = 1;

/// The label authority of one pairwise example.
///
/// [`Self::ProofPositive`] is minted only from exact replay-validated
/// traces; [`Self::HeuristicGuidance`] marks examples derived from
/// approximate traces, which can never become proof data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LabelAuthority {
    /// Derived from an exact replay-validated trace.
    ProofPositive,
    /// Derived from an approximate trace (explicitly admitted).
    HeuristicGuidance,
}

/// Bounded dataset configuration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DatasetConfig {
    max_examples: usize,
    max_traces: usize,
    max_steps_per_trace: usize,
    allow_approximate: bool,
    seed: Option<u64>,
    relation_limits: RelationLimits,
}

impl DatasetConfig {
    /// Hard ceiling on examples in one dataset (the neural corpus cap).
    pub const MAX_EXAMPLES: usize = 65_536;
    /// Hard ceiling on source traces in one dataset.
    pub const MAX_TRACES: usize = 1_024;
    /// Hard ceiling on derivation steps admitted per trace (matches the
    /// inverse search depth ceiling).
    pub const MAX_STEPS_PER_TRACE: usize = 64;

    /// Validate a dataset configuration. Zero or above-ceiling values are
    /// typed errors; ceilings are associated constants and cannot be raised.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        max_examples: usize,
        max_traces: usize,
        max_steps_per_trace: usize,
        allow_approximate: bool,
        seed: Option<u64>,
        relation_limits: RelationLimits,
    ) -> Result<Self, NeuralError> {
        let config = Self {
            max_examples,
            max_traces,
            max_steps_per_trace,
            allow_approximate,
            seed,
            relation_limits,
        };
        config.validate()?;
        Ok(config)
    }

    /// Re-run the ceiling checks. `new` calls this, and so does every
    /// consumer handed a config whose provenance is not `new`.
    pub fn validate(&self) -> Result<(), NeuralError> {
        self.check("examples", self.max_examples, Self::MAX_EXAMPLES)?;
        self.check("traces", self.max_traces, Self::MAX_TRACES)?;
        self.check(
            "trace steps",
            self.max_steps_per_trace,
            Self::MAX_STEPS_PER_TRACE,
        )?;
        Ok(())
    }

    fn check(
        &self,
        resource: &'static str,
        value: usize,
        ceiling: usize,
    ) -> Result<(), NeuralError> {
        if value == 0 || value > ceiling {
            return Err(NeuralError::LimitExceeded {
                resource,
                value,
                limit: ceiling,
            });
        }
        Ok(())
    }

    /// Example ceiling.
    pub fn max_examples(&self) -> usize {
        self.max_examples
    }

    /// Trace ceiling.
    pub fn max_traces(&self) -> usize {
        self.max_traces
    }

    /// Per-trace step ceiling.
    pub fn max_steps_per_trace(&self) -> usize {
        self.max_steps_per_trace
    }

    /// Whether approximate traces are admitted (always labeled
    /// [`LabelAuthority::HeuristicGuidance`]).
    pub fn allow_approximate(&self) -> bool {
        self.allow_approximate
    }

    /// The sub-sampling seed (used only when a ceiling truncates).
    pub fn seed(&self) -> Option<u64> {
        self.seed
    }

    /// The relation limits applied during admission and encoding.
    pub fn relation_limits(&self) -> RelationLimits {
        self.relation_limits
    }
}

impl Default for DatasetConfig {
    /// Defaults sit at the fixed ceilings, rejecting approximate traces.
    fn default() -> Self {
        Self {
            max_examples: Self::MAX_EXAMPLES,
            max_traces: Self::MAX_TRACES,
            max_steps_per_trace: Self::MAX_STEPS_PER_TRACE,
            allow_approximate: false,
            seed: None,
            relation_limits: RelationLimits::default(),
        }
    }
}

/// One pairwise example: a chosen candidate and a valid-but-unchosen
/// candidate from the same expansion round, featurized through the frozen
/// schema.
#[derive(Clone, Debug, PartialEq)]
pub struct PairwiseExample {
    chosen: FeatureVector,
    unchosen: FeatureVector,
    authority: LabelAuthority,
    target_distance: u64,
    key: Vec<u8>,
    chosen_digest: Sha256Digest,
    unchosen_digest: Sha256Digest,
}

impl PairwiseExample {
    /// Frozen features of the chosen candidate.
    pub fn chosen(&self) -> &FeatureVector {
        &self.chosen
    }

    /// Frozen features of the unchosen candidate.
    pub fn unchosen(&self) -> &FeatureVector {
        &self.unchosen
    }

    /// The label authority of this example.
    pub fn authority(&self) -> LabelAuthority {
        self.authority
    }

    /// Distance, in backward steps, from the trace root to the state where
    /// the choice occurred (the round depth, 1-based).
    pub fn target_distance(&self) -> u64 {
        self.target_distance
    }

    /// Canonical identity of the chosen candidate.
    pub fn chosen_digest(&self) -> Sha256Digest {
        self.chosen_digest
    }

    /// Canonical identity of the unchosen candidate.
    pub fn unchosen_digest(&self) -> Sha256Digest {
        self.unchosen_digest
    }
}

/// The reproducibility manifest: everything Task 42 needs to audit a
/// training run's provenance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatasetManifest {
    schema_version: u32,
    feature_schema_digest: Sha256Digest,
    proof_positive: usize,
    heuristic_guidance: usize,
    seed: Option<u64>,
    trace_digests: Vec<Sha256Digest>,
}

impl DatasetManifest {
    /// The manifest schema version.
    pub fn schema_version(&self) -> u32 {
        self.schema_version
    }

    /// The frozen feature-schema digest the examples were featurized under.
    pub fn feature_schema_digest(&self) -> Sha256Digest {
        self.feature_schema_digest
    }

    /// Count of [`LabelAuthority::ProofPositive`] examples.
    pub fn proof_positive(&self) -> usize {
        self.proof_positive
    }

    /// Count of [`LabelAuthority::HeuristicGuidance`] examples.
    pub fn heuristic_guidance(&self) -> usize {
        self.heuristic_guidance
    }

    /// The sub-sampling seed, recorded only when a ceiling truncated the
    /// dataset.
    pub fn seed(&self) -> Option<u64> {
        self.seed
    }

    /// Per-source-trace digests, in canonical (sorted) order.
    pub fn trace_digests(&self) -> &[Sha256Digest] {
        &self.trace_digests
    }
}

/// A derived dataset: canonical examples plus their reproducibility manifest.
#[derive(Clone, Debug, PartialEq)]
pub struct Dataset {
    examples: Vec<PairwiseExample>,
    manifest: DatasetManifest,
}

impl Dataset {
    /// The examples, in canonical content-key order.
    pub fn examples(&self) -> &[PairwiseExample] {
        &self.examples
    }

    /// The reproducibility manifest.
    pub fn manifest(&self) -> &DatasetManifest {
        &self.manifest
    }

    /// Number of examples.
    pub fn len(&self) -> usize {
        self.examples.len()
    }

    /// True when there are no examples.
    pub fn is_empty(&self) -> bool {
        self.examples.is_empty()
    }
}

/// The dataset builder: a config plus a feature encoder and the frozen
/// schema digest it binds every example to.
#[derive(Clone, Copy, Debug)]
pub struct DatasetBuilder {
    config: DatasetConfig,
    encoder: FeatureEncoder,
    schema_digest: Sha256Digest,
}

impl DatasetBuilder {
    /// Bind a builder to a validated configuration.
    pub fn new(config: DatasetConfig) -> Result<Self, NeuralError> {
        config.validate()?;
        let encoder = FeatureEncoder::new(config.relation_limits());
        let schema_digest = FeatureSchemaV1.digest();
        Ok(Self {
            config,
            encoder,
            schema_digest,
        })
    }

    /// The bound configuration.
    pub fn config(&self) -> &DatasetConfig {
        &self.config
    }

    /// The feature encoder (admission-gated to the configured limits).
    pub fn encoder(&self) -> &FeatureEncoder {
        &self.encoder
    }

    /// The frozen feature-schema digest the examples are featurized under.
    pub fn feature_schema_digest(&self) -> Sha256Digest {
        self.schema_digest
    }

    /// Derive a dataset from replay-validated traces against `system`.
    ///
    /// # Errors
    ///
    /// [`NeuralError::LimitExceeded`] when a ceiling is exceeded (traces,
    /// steps, or — with no seed configured — examples); [`NeuralError::EmptyTrace`]
    /// for a trace with no derivation steps; [`NeuralError::ApproximateTraceRejected`]
    /// for an approximate trace under a configuration that does not admit it;
    /// [`NeuralError::TraceVerificationFailed`] when a trace does not replay;
    /// [`NeuralError::TraceDrift`] when the chosen transition is absent from
    /// the re-derived candidates.
    pub fn build(
        &self,
        system: &TermSystem,
        traces: &[ReplayCertificate],
    ) -> Result<Dataset, NeuralError> {
        if traces.len() > self.config.max_traces() {
            return Err(NeuralError::LimitExceeded {
                resource: "traces",
                value: traces.len(),
                limit: self.config.max_traces(),
            });
        }

        let mut examples: Vec<PairwiseExample> = Vec::new();
        let mut trace_digests: Vec<Sha256Digest> = Vec::with_capacity(traces.len());

        for certificate in traces {
            // Admission precedes verification: every embedded term,
            // constraint side, substitution range, and identifier of the
            // whole certificate is bounded before `verify` hashes or clones
            // anything.
            admit_certificate(
                certificate,
                &self.config.relation_limits(),
                self.config.max_steps_per_trace(),
            )?;

            let (root, steps) = trace_steps(certificate);
            if steps.is_empty() {
                return Err(NeuralError::EmptyTrace);
            }
            if steps.len() > self.config.max_steps_per_trace() {
                return Err(NeuralError::LimitExceeded {
                    resource: "trace steps",
                    value: steps.len(),
                    limit: self.config.max_steps_per_trace(),
                });
            }

            let authority = match &certificate.guidance {
                GuidanceMode::CompleteWithinLimits => LabelAuthority::ProofPositive,
                GuidanceMode::HeuristicPruning { .. } => {
                    if !self.config.allow_approximate() {
                        return Err(NeuralError::ApproximateTraceRejected);
                    }
                    LabelAuthority::HeuristicGuidance
                }
            };

            certificate
                .verify(system)
                .map_err(|_| NeuralError::TraceVerificationFailed)?;

            self.derive_backward(system, root, steps, authority, &mut examples)?;
            trace_digests.push(trace_digest(certificate));
        }

        examples.sort_by(|left, right| left.key.cmp(&right.key));

        let mut sub_sampled = false;
        if examples.len() > self.config.max_examples() {
            match self.config.seed() {
                Some(seed) => {
                    examples = sub_sample(examples, self.config.max_examples(), seed);
                    sub_sampled = true;
                }
                None => {
                    return Err(NeuralError::LimitExceeded {
                        resource: "examples",
                        value: examples.len(),
                        limit: self.config.max_examples(),
                    });
                }
            }
        }

        let proof_positive = examples
            .iter()
            .filter(|example| example.authority == LabelAuthority::ProofPositive)
            .count();
        let heuristic_guidance = examples.len() - proof_positive;
        trace_digests.sort();
        let manifest = DatasetManifest {
            schema_version: DATASET_MANIFEST_SCHEMA_VERSION,
            feature_schema_digest: self.schema_digest,
            proof_positive,
            heuristic_guidance,
            seed: if sub_sampled {
                self.config.seed()
            } else {
                None
            },
            trace_digests,
        };

        Ok(Dataset { examples, manifest })
    }

    /// Derive examples from one trace's backward steps, re-deriving the
    /// engine's candidates at every state and subtracting the chosen
    /// transition.
    fn derive_backward(
        &self,
        system: &TermSystem,
        root: &Term,
        steps: &[SymbolicPredecessor],
        authority: LabelAuthority,
        out: &mut Vec<PairwiseExample>,
    ) -> Result<(), NeuralError> {
        let limits = self.config.relation_limits();
        for (index, chosen) in steps.iter().enumerate() {
            let state_term: &Term = if index == 0 {
                root
            } else {
                &steps[index - 1].term
            };

            let candidates = crate::inverse::symbolic_predecessors(
                system,
                state_term,
                &limits,
                chosen.provenance.scope,
            )
            .map_err(map_derive_error)?;

            // The branch factor is the branching the search actually faced,
            // so it is read from the full candidate set — duplicates
            // included — before any collapse (PR #357 round 1 P1c).
            let branch_factor = candidates.len() as u64;

            // Locate the chosen transition by provenance in the full set,
            // BEFORE digest deduplication: two distinct rules can derive the
            // same state, and the state digest cannot tell those transitions
            // apart. Collapsing first would discard the very representative
            // the trace recorded and turn a valid certificate into a
            // spurious TraceDrift (PR #357 round 1 P1b).
            //
            // Matching is modulo alpha-equivalence: alpha-equivalent rules
            // share one stable `RuleId`, and their freshened candidates share
            // one provenance while differing only in erased variable names.
            // The silent pick of the first match is sound because those
            // candidates encode to identical features and search context;
            // the `alpha_equivalent_candidates_are_feature_and_context_equivalent`
            // contract pins that equivalence (PR #357 round 2 P3).
            let Some(chosen_index) = candidates
                .iter()
                .position(|candidate| candidate.provenance == chosen.provenance)
            else {
                return Err(NeuralError::TraceDrift);
            };

            // Admission before identity: the chosen candidate is encoded
            // (bounded, iterative) before its digest clones the term.
            let chosen_candidate = &candidates[chosen_index];
            let chosen_input = CandidateInput::new(
                &chosen_candidate.term,
                &chosen_candidate.constraints,
                &chosen_candidate.provenance,
                self.context(system, state_term, chosen_candidate, index, branch_factor),
            );
            let chosen_features = self.encoder.encode(&chosen_input)?;
            let chosen_digest = canonical_digest(&chosen_input);

            // Deduplicate the negatives by canonical state digest. The
            // chosen state is admitted once: a candidate that reproduces it
            // from a different rule is the same state, not a distinct
            // valid-but-unchosen candidate.
            let mut seen: Vec<Sha256Digest> = Vec::with_capacity(candidates.len());
            seen.push(chosen_digest);

            for (candidate_index, negative) in candidates.iter().enumerate() {
                if candidate_index == chosen_index {
                    continue;
                }
                let negative_input = CandidateInput::new(
                    &negative.term,
                    &negative.constraints,
                    &negative.provenance,
                    self.context(system, state_term, negative, index, branch_factor),
                );
                let negative_digest = canonical_digest(&negative_input);
                if seen.contains(&negative_digest) {
                    continue;
                }
                seen.push(negative_digest);
                let negative_features = self.encoder.encode(&negative_input)?;
                let key = example_key(
                    authority,
                    (index + 1) as u64,
                    &chosen_input,
                    chosen_digest,
                    &negative_input,
                    negative_digest,
                );
                out.push(PairwiseExample {
                    chosen: chosen_features,
                    unchosen: negative_features,
                    authority,
                    target_distance: (index + 1) as u64,
                    key,
                    chosen_digest,
                    unchosen_digest: negative_digest,
                });
            }
        }
        Ok(())
    }

    /// Reconstruct the per-candidate search context the guidance layer
    /// computed, except `cycle` (unobservable from a trace alone, so
    /// uniformly `false`). `branch_factor` is the size of the full
    /// re-derived candidate set, counted before negative deduplication.
    fn context(
        &self,
        system: &TermSystem,
        parent: &Term,
        candidate: &SymbolicPredecessor,
        index: usize,
        branch_factor: u64,
    ) -> SearchContext {
        let parent_nodes = term_nodes(parent) as i64;
        let child_nodes = term_nodes(&candidate.term) as i64;
        let rule_priority = system
            .rules()
            .iter()
            .position(|rule| RuleId::from_rule(rule) == candidate.provenance.rule_id)
            .unwrap_or(usize::MAX) as u64;
        SearchContext::new(
            (index + 1) as u64,
            branch_factor,
            false,
            child_nodes - parent_nodes,
            0,
            candidate.existentials.len(),
            rule_priority,
        )
    }
}

/// Extract the trace root and its backward steps from a certificate.
fn trace_steps(certificate: &ReplayCertificate) -> (&Term, &[SymbolicPredecessor]) {
    match (&certificate.query, &certificate.derivation) {
        (ReplayQuery::Backward { target, .. }, ReplayDerivation::Backward(derivation)) => {
            (target, &derivation.steps)
        }
        (ReplayQuery::Bidirectional { goal, .. }, ReplayDerivation::Bidirectional(derivation)) => {
            (goal, &derivation.backward_steps)
        }
        // Unreachable after `verify`; the empty slice forces `EmptyTrace`.
        _ => (
            match &certificate.query {
                ReplayQuery::Backward { target, .. } => target,
                ReplayQuery::Bidirectional { goal, .. } => goal,
            },
            &[],
        ),
    }
}

/// Admit every field of one certificate against the configured limits.
///
/// Admission is the *whole* certificate, not just the reconstructed root:
/// both query terms, both derivation halves, every embedded term (constraint
/// sides and substitution ranges included), every position, and every
/// identifier. It is a bounded, iterative walk that never clones or hashes
/// the content it is judging, so nothing unadmitted reaches
/// [`ReplayCertificate::verify`] (PR #357 round 1 P1a).
fn admit_certificate(
    certificate: &ReplayCertificate,
    limits: &RelationLimits,
    max_steps: usize,
) -> Result<(), NeuralError> {
    check_query_bounded(&certificate.query, limits)?;
    // Both derivation halves are counted before their steps are walked, so
    // the step ceiling bounds admission work itself as well as replay
    // (PR #357 round 2 P1b).
    let (backward_steps, forward_steps) = match &certificate.derivation {
        ReplayDerivation::Backward(derivation) => (derivation.steps.len(), 0),
        ReplayDerivation::Bidirectional(derivation) => (
            derivation.backward_steps.len(),
            derivation.forward_steps.len(),
        ),
    };
    check_step_count(backward_steps, max_steps, "trace steps")?;
    check_step_count(forward_steps, max_steps, "forward steps")?;
    match &certificate.derivation {
        ReplayDerivation::Backward(derivation) => check_steps_bounded(&derivation.steps, limits),
        ReplayDerivation::Bidirectional(derivation) => {
            for step in &derivation.forward_steps {
                check_path_bounded(&step.position, limits)?;
            }
            check_steps_bounded(&derivation.backward_steps, limits)?;
            check_term_bounded(&derivation.meeting_forward, limits)?;
            check_term_bounded(&derivation.meeting_backward, limits)?;
            check_substitution_bounded(&derivation.unifier, limits)
        }
    }
}

/// Admit one derivation half's step count before its steps are walked.
fn check_step_count(count: usize, limit: usize, resource: &'static str) -> Result<(), NeuralError> {
    if count > limit {
        return Err(NeuralError::LimitExceeded {
            resource,
            value: count,
            limit,
        });
    }
    Ok(())
}

/// Admit both terms of the certified statement.
fn check_query_bounded(query: &ReplayQuery, limits: &RelationLimits) -> Result<(), NeuralError> {
    let (source, goal) = match query {
        ReplayQuery::Backward { target, goal } => (target, goal),
        ReplayQuery::Bidirectional { source, goal } => (source, goal),
    };
    check_term_bounded(source, limits)?;
    check_term_bounded(goal, limits)
}

/// Admit one derivation half: every step's term, position, residual
/// constraints, and unifying substitution.
fn check_steps_bounded(
    steps: &[SymbolicPredecessor],
    limits: &RelationLimits,
) -> Result<(), NeuralError> {
    for step in steps {
        check_term_bounded(&step.term, limits)?;
        check_path_bounded(&step.provenance.position, limits)?;
        check_constraints_bounded(&step.constraints, limits)?;
        check_existentials_bounded(&step.existentials, limits)?;
        check_substitution_bounded(&step.substitution, limits)?;
    }
    Ok(())
}

/// Admit a step's existential vector.
///
/// Existentials are a per-step auxiliary set exactly like the residual
/// constraints, and the per-step constraint ceiling (`RelationLimits`) is the
/// ceiling that fits: real steps carry a handful (one per erased LHS
/// variable), while replay ignores the vector and the trace digest would hash
/// it whole. A million-entry vector is therefore an unbounded payload that
/// reached both, so it is rejected at admission with the resource named
/// (PR #357 round 3 P2b).
fn check_existentials_bounded(
    existentials: &[LogicVar],
    limits: &RelationLimits,
) -> Result<(), NeuralError> {
    if existentials.len() > limits.max_constraints() {
        return Err(NeuralError::LimitExceeded {
            resource: "existentials",
            value: existentials.len(),
            limit: limits.max_constraints(),
        });
    }
    Ok(())
}

/// Admit a residual constraint set: the count against the constraint
/// ceiling, and both sides of every constraint as terms.
fn check_constraints_bounded(
    constraints: &ConstraintSet,
    limits: &RelationLimits,
) -> Result<(), NeuralError> {
    if constraints.len() > limits.max_constraints() {
        return Err(NeuralError::LimitExceeded {
            resource: "constraints",
            value: constraints.len(),
            limit: limits.max_constraints(),
        });
    }
    for constraint in constraints.constraints() {
        let (left, right) = match constraint {
            TermConstraint::Equal(left, right) | TermConstraint::NotEqual(left, right) => {
                (left, right)
            }
        };
        check_term_bounded(left, limits)?;
        check_term_bounded(right, limits)?;
    }
    Ok(())
}

/// Admit a substitution: every bound name and every range term, the entry
/// count, the longest transitive chain of variable-to-variable bindings
/// reachable through it, and the *expanded* structure the resolution
/// produces. `BTreeMap` iteration order makes every walk deterministic.
///
/// The entry ceiling reuses the term-node ceiling: a substitution is a map of
/// bounded ranges, so its cardinality is held to the same admitted size.
///
/// The chain bound is load-bearing: replay resolves a bound variable by
/// recursion (`Substitution::apply`), so a certificate can smuggle unbounded
/// stack depth through the unifier with individually tiny ranges
/// (`V0 -> V1 -> ... -> Vn -> a`). Counting entries alone does not bound that
/// depth, and walking it recursively to measure it would reproduce the bug,
/// so the chain is measured iteratively and memoized.
///
/// The chain bound is still not sufficient on its own: many short chains
/// compose. [`check_substitution_expansion_bounded`] bounds the composed
/// result — the depth and node count of the term each variable actually
/// resolves to — which is the structure replay recurses over.
fn check_substitution_bounded(
    substitution: &Substitution,
    limits: &RelationLimits,
) -> Result<(), NeuralError> {
    if substitution.len() > limits.max_term_nodes() {
        return Err(NeuralError::LimitExceeded {
            resource: "substitution entries",
            value: substitution.len(),
            limit: limits.max_term_nodes(),
        });
    }
    for (variable, term) in substitution.iter() {
        check_identifier(variable.as_str())?;
        check_term_bounded(term, limits)?;
    }
    check_substitution_chain_depth(substitution, limits)?;
    check_substitution_expansion_bounded(substitution, limits)
}

/// Bound the longest transitive variable chain through a substitution.
///
/// The walk is iterative and memoized, so admission cost stays linear in the
/// entry count however deep a hostile chain is. A range need not be a bare
/// variable to extend a chain: `V0 -> f(V1)` resolves `V0` to `f(V1)` and then
/// recurses into `V1`, so the graph edges are the *bound variables occurring
/// in* each range, not just bare-variable ranges. The depth is the number of
/// variable resolutions on the longest path, with a repeated variable ending
/// the path (replay leaves it in place at cycle detection).
///
/// The ceiling is the admitted term-depth ceiling: unification produces
/// idempotent substitutions (no chains at all), so a chain longer than any
/// admitted term walk cannot come from a real search and is rejected as
/// hostile. This bound names chain abuse precisely, but it does not bound the
/// recursion `Substitution::apply` performs during replay: short chains
/// compose (PR #357 round 3 P1). [`check_substitution_expansion_bounded`] is
/// the bound over the composed result.
fn check_substitution_chain_depth(
    substitution: &Substitution,
    limits: &RelationLimits,
) -> Result<(), NeuralError> {
    let ceiling = limits.max_term_depth();
    let mut resolved: BTreeMap<Variable, usize> = BTreeMap::new();
    for (start, range) in substitution.iter() {
        if resolved.contains_key(start) {
            continue;
        }
        let mut stack: Vec<(Variable, Vec<Variable>, usize)> = Vec::new();
        stack.push((start.clone(), bound_variables(substitution, range), 0));
        while !stack.is_empty() {
            let frame = stack.len() - 1;
            if stack[frame].2 < stack[frame].1.len() {
                let child = stack[frame].1[stack[frame].2].clone();
                stack[frame].2 += 1;
                if resolved.contains_key(&child) {
                    continue;
                }
                if stack.iter().any(|(open, _, _)| *open == child) {
                    // A cycle resolves to its fixed point in replay: the
                    // repeated variable is left in place, not followed again.
                    continue;
                }
                let child_range = substitution.get_var(&child).expect("bound variable");
                stack.push((child, bound_variables(substitution, child_range), 0));
                continue;
            }
            let tail = stack[frame]
                .1
                .iter()
                .map(|child| resolved.get(child).copied().unwrap_or(0))
                .max()
                .unwrap_or(0);
            let depth = tail.saturating_add(1);
            if depth > ceiling {
                return Err(NeuralError::LimitExceeded {
                    resource: "substitution chain depth",
                    value: depth,
                    limit: ceiling,
                });
            }
            let (node, _, _) = stack.pop().expect("non-empty stack");
            resolved.insert(node, depth);
        }
    }
    Ok(())
}

/// Bound the *expanded* structure a substitution resolves to.
///
/// The chain ceiling and the per-range ceilings are each necessary but
/// neither is sufficient. Sixty-four ranges of sixty-three `f` wrappers each
/// are within the per-range term depth, and their variable chain is exactly
/// the chain ceiling, yet resolving the first variable composes to a term of
/// depth ~3,970. Replay resolves ranges recursively
/// (`Substitution::apply`), so the composition — not the parts — is the
/// structure whose size has to be bounded before recursion (PR #357 round 3
/// P1).
///
/// The walk mirrors `apply` exactly: a bound variable occurrence is replaced
/// by its resolved range at the same depth, and a variable already being
/// resolved (a cycle) is left in place, matching the fixed-point rule in
/// `apply_guarded`. Two deliberate differences keep it safe as a judge:
/// it is iterative (one explicit frame stack, so hostile depth cannot recurse)
/// and it stops at the first ceiling breach, so its cost is bounded by the
/// node ceiling rather than by what the substitution claims. An expanding
/// variable occurrence is counted as one node in addition to its range, which
/// over-counts what resolution produces: the node bound is conservative.
///
/// Ceilings: the expanded term must satisfy the admitted term ceilings
/// (nodes and depth), because that term is what replay substitutes into the
/// meeting terms and what the encoder later walks. A substitution whose
/// resolution exceeds them cannot come from a real search, whose resolved
/// ranges are subterms of admitted states.
fn check_substitution_expansion_bounded(
    substitution: &Substitution,
    limits: &RelationLimits,
) -> Result<(), NeuralError> {
    let max_nodes = limits.max_term_nodes();
    let max_depth = limits.max_term_depth();
    for (_, range) in substitution.iter() {
        let mut visiting: BTreeSet<Variable> = BTreeSet::new();
        let mut nodes = 0usize;
        let mut stack: Vec<Frame<'_>> = Vec::from([Frame::Enter(range, 1)]);
        while let Some(frame) = stack.pop() {
            let (node, depth) = match frame {
                // The expansion of a variable is complete once its frames
                // have popped, so it leaves the cycle set here.
                Frame::Exit(variable) => {
                    visiting.remove(&variable);
                    continue;
                }
                Frame::Enter(node, depth) => (node, depth),
            };
            nodes += 1;
            if nodes > max_nodes {
                return Err(NeuralError::LimitExceeded {
                    resource: "expanded substitution nodes",
                    value: nodes,
                    limit: max_nodes,
                });
            }
            if depth > max_depth {
                return Err(NeuralError::LimitExceeded {
                    resource: "expanded substitution depth",
                    value: depth,
                    limit: max_depth,
                });
            }
            match node {
                Term::Var(variable) => {
                    if visiting.contains(variable) {
                        // A cycle resolves to its fixed point: the variable
                        // is left in place, exactly as `apply` does.
                        continue;
                    }
                    if let Some(bound) = substitution.get_var(variable) {
                        visiting.insert(variable.clone());
                        stack.push(Frame::Exit(variable.clone()));
                        stack.push(Frame::Enter(bound, depth));
                    }
                }
                Term::Sym(_, args) => {
                    for arg in args.iter().rev() {
                        stack.push(Frame::Enter(arg, depth + 1));
                    }
                }
            }
        }
    }
    Ok(())
}

/// One frame of the expansion walk: a node at its depth, or the end of one
/// variable's expansion (which releases it from the cycle set).
enum Frame<'a> {
    Enter(&'a Term, usize),
    Exit(Variable),
}

/// The bound variables occurring in a substitution range: the edges of the
/// resolution graph. Names that are not bound by this substitution resolve to
/// themselves and end a path.
fn bound_variables(substitution: &Substitution, range: &Term) -> Vec<Variable> {
    range
        .variables()
        .into_iter()
        .filter(|variable| substitution.get_var(variable).is_some())
        .collect()
}

/// Admit a rewrite position. A path through a term of depth `d` has at most
/// `d - 1` indices, so a path longer than `max_term_depth` names a subterm no
/// admitted state could contain.
fn check_path_bounded(position: &Path, limits: &RelationLimits) -> Result<(), NeuralError> {
    let depth = position.as_slice().len();
    if depth > limits.max_term_depth() {
        return Err(NeuralError::LimitExceeded {
            resource: "term depth",
            value: depth,
            limit: limits.max_term_depth(),
        });
    }
    Ok(())
}

/// Reject an identifier longer than [`FeatureEncoder::MAX_IDENTIFIER_BYTES`].
///
/// Task 40's doctrine applies here unchanged: a name is checked on the
/// borrowed `str`, before admission hashes it (the certificate's query and
/// trace digests encode every name) or copies it.
fn check_identifier(name: &str) -> Result<(), NeuralError> {
    if name.len() > FeatureEncoder::MAX_IDENTIFIER_BYTES {
        return Err(NeuralError::LimitExceeded {
            resource: "identifier bytes",
            value: name.len(),
            limit: FeatureEncoder::MAX_IDENTIFIER_BYTES,
        });
    }
    Ok(())
}

/// Bounded, iterative admission walk over a term: node count, depth, and
/// every identifier are checked against the limits before anything clones or
/// recurses over the term.
fn check_term_bounded(term: &Term, limits: &RelationLimits) -> Result<(), NeuralError> {
    let max_nodes = limits.max_term_nodes();
    let max_depth = limits.max_term_depth();
    let mut stack: Vec<(&Term, usize)> = Vec::from([(term, 1)]);
    let mut nodes = 0usize;
    while let Some((node, depth)) = stack.pop() {
        nodes += 1;
        if nodes > max_nodes {
            return Err(NeuralError::LimitExceeded {
                resource: "term nodes",
                value: nodes,
                limit: max_nodes,
            });
        }
        if depth > max_depth {
            return Err(NeuralError::LimitExceeded {
                resource: "term depth",
                value: depth,
                limit: max_depth,
            });
        }
        match node {
            Term::Var(variable) => check_identifier(variable.as_str())?,
            Term::Sym(symbol, args) => {
                check_identifier(symbol.as_str())?;
                if args.len() > max_nodes.saturating_sub(nodes) {
                    return Err(NeuralError::LimitExceeded {
                        resource: "term nodes",
                        value: nodes.saturating_add(args.len()),
                        limit: max_nodes,
                    });
                }
                for arg in args.iter().rev() {
                    stack.push((arg, depth + 1));
                }
            }
        }
    }
    Ok(())
}

/// Iterative node count (terms are already bounded by admission).
fn term_nodes(term: &Term) -> usize {
    let mut count = 0usize;
    let mut stack: Vec<&Term> = Vec::from([term]);
    while let Some(node) = stack.pop() {
        count += 1;
        if let Term::Sym(_, args) = node {
            for arg in args.iter().rev() {
                stack.push(arg);
            }
        }
    }
    count
}

/// A sanitized re-derivation rejection: relation limits name the resource,
/// anything else is replaced by a stage-named error.
fn map_derive_error(error: crate::RewriteError) -> NeuralError {
    match error {
        crate::RewriteError::RelationLimitExceeded { resource, limit } => {
            NeuralError::LimitExceeded {
                resource,
                value: limit,
                limit,
            }
        }
        _ => NeuralError::TraceRederivation,
    }
}

/// The deterministic content key that orders examples.
fn example_key(
    authority: LabelAuthority,
    target_distance: u64,
    chosen: &CandidateInput<'_>,
    chosen_digest: Sha256Digest,
    unchosen: &CandidateInput<'_>,
    unchosen_digest: Sha256Digest,
) -> Vec<u8> {
    let mut key = Vec::with_capacity(320);
    key.push(match authority {
        LabelAuthority::ProofPositive => 0x00,
        LabelAuthority::HeuristicGuidance => 0x01,
    });
    key.extend_from_slice(&target_distance.to_le_bytes());
    key.extend_from_slice(&content_key(chosen, chosen_digest));
    key.extend_from_slice(&content_key(unchosen, unchosen_digest));
    key
}

/// Frame for one backward step's complete identity (see [`encode_step`]).
const STEP_FRAME: &str = "amari.neural.step/v1";

/// Frame for a bidirectional derivation's meeting pair and unifier.
const MEETING_FRAME: &str = "amari.neural.meeting/v1";

/// Ceiling on the entry arrangements enumerated while canonicalizing a
/// substitution whose entries have indistinguishable shapes (mirrors the
/// state-digest cap in `crate::inverse`).
const MAX_SUBSTITUTION_ARRANGEMENTS: u64 = 720;

/// Ceiling on the bytes materialized while resolving equal-shape substitution
/// ties on the whole step payload: the candidates are encoded one at a time and
/// the enumeration stops once this budget is spent, falling back to the
/// shape-sorted baseline arrangement.
///
/// Admission bounds each field of a step, not the product of arrangements by
/// the payload they are compared over: a maximally admitted step (thousands of
/// residuals over thousand-node terms) times 720 arrangements could otherwise
/// materialize gigabytes from one untrusted trace. A realistic step encodes in
/// kilobytes, so a full enumeration of the 720-arrangement cap fits easily;
/// above the budget the baseline order is used — still deterministic and still
/// sharing-preserving, only possibly missing an alpha-equivalent dedup, which
/// can never become a false equality (two steps are equal only when their
/// encodings are identical).
const MAX_TIE_RESOLUTION_BYTES: u64 = 4 * 1024 * 1024;

/// Canonical per-trace digest: the authority tag, the *whole* certified
/// statement (the query, both terms), and the derivation — every forward
/// step, every backward step (including its term, substitution,
/// constraints, existentials, and provenance), both meeting terms, and the
/// unifier.
///
/// The query belongs in the identity: two traces over the same backward
/// chain but different certified goals are different claims about the
/// system, and a digest that ignored the goal would let either stand in for
/// the other (PR #357 round 1 P2). The forward half is covered for the same
/// reason a bidirectional derivation is not just its backward steps. Step
/// payloads are covered because the documented identity scope is the full
/// derivation, not only the transition provenance: two certificates that
/// replay to the same statement through different recorded substitutions,
/// constraints, or existentials are different training evidence (PR #357
/// round 2 P2). Every field here has already been admitted, so hashing
/// cannot be reached with unbounded content.
///
/// Variable identity is scoped per step: each step is encoded under ONE
/// renaming environment shared by its term, substitution, residuals, and
/// existentials, so which variable of the term a residual, a range, or an
/// existential mentions — and whether two ranges mention the same variable —
/// is part of the identity (PR #357 round 4, extended in round 5). The
/// bidirectional tail is one scope for the same reason.
///
/// The canonical encoding of a step is the **minimum over equal-shape tie
/// arrangements of the entire framed payload** — not of the substitution
/// segment alone. A tie in the substitution encoding does not imply a tie in
/// the environment it commits, and that environment governs the residuals and
/// existentials that follow, so the choice is resolved on the whole payload
/// (PR #357 round 5 P2b).
fn trace_digest(certificate: &ReplayCertificate) -> Sha256Digest {
    let mut payload = Vec::new();
    match certificate.guidance {
        GuidanceMode::CompleteWithinLimits => payload.push(0x00),
        GuidanceMode::HeuristicPruning { .. } => payload.push(0x01),
    }
    payload.extend_from_slice(query_hash(&certificate.query).as_bytes());
    match &certificate.derivation {
        ReplayDerivation::Backward(derivation) => {
            payload.push(0x00);
            for step in &derivation.steps {
                encode_step(&mut payload, step);
            }
        }
        ReplayDerivation::Bidirectional(derivation) => {
            payload.push(0x01);
            for step in &derivation.forward_steps {
                encode_forward_step(&mut payload, step);
            }
            for step in &derivation.backward_steps {
                encode_step(&mut payload, step);
            }
            // One identity scope for the meeting pair and the unifier: a
            // unifier range may mention a meeting-term variable, so the two
            // terms and the substitution are encoded under a shared renaming
            // environment (PR #357 round 4, same family as the per-step env).
            // The unifier ends the payload, so the whole-payload tie rule
            // degenerates to the smallest unifier encoding here.
            let mut prefix = Vec::new();
            let mut variables = Vec::new();
            encode_term(&derivation.meeting_forward, &mut variables, &mut prefix);
            encode_term(&derivation.meeting_backward, &mut variables, &mut prefix);
            let candidates = substitution_candidates(&derivation.unifier, &variables);
            let digest = framed_minimum(MEETING_FRAME, &prefix, candidates, |_, _| {});
            payload.extend_from_slice(digest.as_bytes());
        }
    }
    Sha256Digest::framed("amari.neural.trace/v1", &payload)
}

/// One backward step's identity, encoded under ONE variable-renaming
/// environment.
///
/// A step is a single identity scope, not a bag of independently hashed
/// fields: the term it derived, its unifying substitution, its residual
/// constraints, and its existentials are encoded under a common `variables`
/// environment, so a variable keeps one index across all of them. Encoding
/// the fields separately made two kinds of identity invisible: which variable
/// of the term a residual names (`(f(a,U,V), U != b)` vs `(f(a,U,V), V != b)`,
/// PR #357 round 4 P2a), and whether two substitution ranges name the same
/// variable (`{U1 -> XX, U2 -> XX}` vs `{U1 -> XX, U2 -> YY}`, round 4 P2b).
///
/// The environment is built by canonical traversal — the term's variables
/// first (preorder), then the substitution's, then the residuals', then the
/// existential references — never from hash-map iteration order, and never
/// from variable names: names only ever select an index. Alpha-equivalent
/// steps therefore encode identically.
fn encode_step(payload: &mut Vec<u8>, step: &SymbolicPredecessor) {
    let mut prefix = Vec::new();
    prefix.extend_from_slice(step.provenance.rule_id.digest().as_bytes());
    prefix.extend_from_slice(step.provenance.target_hash.as_bytes());
    prefix.extend_from_slice(step.provenance.predecessor_hash.as_bytes());
    prefix.extend_from_slice(&step.provenance.scope.to_le_bytes());
    encode_path(&mut prefix, &step.provenance.position);
    // The step's own payload, not just the transition identity: the term it
    // derived, its unifying substitution, its residual constraints, and its
    // existentials — all under one shared renaming environment.
    let mut variables: Vec<String> = Vec::new();
    encode_term(&step.term, &mut variables, &mut prefix);
    // Equal-shape ties in the substitution are resolved on the ENTIRE framed
    // payload: the candidate environments are carried into the residual and
    // existential encoding, and the arrangement whose whole step encoding is
    // smallest wins (PR #357 round 5 P2b).
    let candidates = substitution_candidates(&step.substitution, &variables);
    let digest = framed_minimum(STEP_FRAME, &prefix, candidates, |out, vars| {
        encode_constraints(out, &step.constraints, vars);
        encode_existentials(out, &step.existentials, vars);
    });
    payload.extend_from_slice(digest.as_bytes());
}

/// Existential logic variables, encoded as variable *references* through the
/// shared environment rather than as their `(scope, index)` names.
///
/// An existential is a reference to a variable in the step's scope — the very
/// variable the term writes as `?s.i` — so its identity is the index the
/// shared environment assigns that variable. Encoding the raw pair left the
/// association between the recorded existential and the term's variable
/// positions invisible: swapping two variable arguments of `f(a,?1.1,?1.2)`
/// yields an alpha-equivalent term (identical term encoding) and an unchanged
/// existential list, so "`?1.1` is the existential of `f(a,?1.1,?1.2)`" and
/// "`?1.1` is the existential of `f(a,?1.2,?1.1)`" hashed the same, though the
/// existential names a different argument (PR #357 round 5 P2a).
///
/// The vector order is the freshener's allocation order (sorted LHS variable
/// names) and is encoded as recorded; alpha-equivalent steps list it in the
/// same order, so the order costs no alpha-invariance.
fn encode_existentials(
    payload: &mut Vec<u8>,
    existentials: &[LogicVar],
    variables: &mut Vec<String>,
) {
    use alloc::string::ToString;
    payload.extend_from_slice(&(existentials.len() as u32).to_le_bytes());
    for existential in existentials {
        // The display form is exactly how the freshener names this variable
        // inside terms, so the lookup resolves to the term's index.
        encode_term(&Term::var(existential.to_string()), variables, payload);
    }
}

/// Forward-step audit: `ForwardStep` carries no terms of its own, so its
/// complete identity is the rule, the position, and both engine term hashes.
fn encode_forward_step(payload: &mut Vec<u8>, step: &ForwardStep) {
    payload.extend_from_slice(step.rule_id.digest().as_bytes());
    payload.extend_from_slice(step.source_hash.as_bytes());
    payload.extend_from_slice(step.result_hash.as_bytes());
    encode_path(payload, &step.position);
}

fn encode_constraints(
    payload: &mut Vec<u8>,
    constraints: &ConstraintSet,
    variables: &mut Vec<String>,
) {
    // One encoding over the WHOLE set (order-preserving, as recorded), under
    // the STEP's renaming environment — the term's and the substitution's
    // environment, not a fresh one. Per-constraint digests restart renumbering
    // at zero, which made `{X != a, X != b}` and `{X != a, Y != b}` collide
    // (PR #357 round 3 P2a); a set-scoped but term-independent environment
    // left the term/residual association invisible (round 4 P2a).
    payload.extend_from_slice(&(constraints.len() as u32).to_le_bytes());
    for constraint in constraints.constraints() {
        let (tag, left, right) = match constraint {
            TermConstraint::Equal(left, right) => (0x10u8, left, right),
            TermConstraint::NotEqual(left, right) => (0x11u8, left, right),
        };
        payload.push(tag);
        encode_term(left, variables, payload);
        encode_term(right, variables, payload);
    }
}

fn encode_path(payload: &mut Vec<u8>, path: &Path) {
    let position = path.as_slice();
    payload.extend_from_slice(&(position.len() as u32).to_le_bytes());
    for index in position {
        payload.extend_from_slice(&(*index as u64).to_le_bytes());
    }
}

/// Candidate encodings of a substitution under the caller's shared renaming
/// environment: one `(entry encoding, environment after it)` pair per
/// equal-shape tie arrangement, never empty.
///
/// Domains and ranges are encoded with `variables`, the environment that
/// already holds the step's term variables, so a range naming the same
/// variable as another range — or as the term — is identified with it, and a
/// domain is a variable index rather than a literal name (PR #357 round 4
/// P2b).
///
/// `BTreeMap` iteration order is by variable name, which is not
/// alpha-invariant, so entries are shape-keyed (variables erased), stably
/// sorted by that key, and each maximal run of equal keys — whose relative
/// order a sort cannot decide — is enumerated exhaustively (capped at
/// [`MAX_SUBSTITUTION_ARRANGEMENTS`] total). All arrangements are returned
/// rather than the one with the smallest entry encoding: two arrangements can
/// encode the same bytes yet resolve the environment differently, and it is
/// the caller — which knows what is encoded after the substitution — that
/// picks the canonical one. Beyond the cap only the shape-sorted baseline
/// order is returned: still deterministic and still sharing-preserving, only
/// possibly missing an alpha-equivalent dedup — a missed dedup can never
/// become a false equality, because two steps are equal only when their
/// encodings are identical.
fn substitution_candidates(
    substitution: &Substitution,
    variables: &[String],
) -> Vec<(Vec<u8>, Vec<String>)> {
    let mut decorated: Vec<(Vec<u8>, (&Variable, &Term))> = substitution
        .iter()
        .map(|entry| (substitution_entry_shape_key(entry.1), entry))
        .collect();
    decorated.sort_by(|left, right| left.0.cmp(&right.0));
    // Group maximal runs of equal shape keys.
    let mut runs: Vec<core::ops::Range<usize>> = Vec::new();
    let mut start = 0;
    for index in 1..=decorated.len() {
        if index == decorated.len() || decorated[index].0 != decorated[start].0 {
            runs.push(start..index);
            start = index;
        }
    }
    // Total arrangement count (product of run factorials), capped.
    let mut arrangements: u64 = 1;
    for run in &runs {
        let size = (run.end - run.start) as u64;
        let mut factorial: u64 = 1;
        for factor in 2..=size {
            factorial = factorial.saturating_mul(factor);
        }
        arrangements = arrangements.saturating_mul(factorial);
        if arrangements > MAX_SUBSTITUTION_ARRANGEMENTS {
            break;
        }
    }
    if arrangements <= MAX_SUBSTITUTION_ARRANGEMENTS && !runs.is_empty() {
        let run_permutations: Vec<Vec<Vec<usize>>> = runs
            .iter()
            .map(|run| permutations(run.end - run.start))
            .collect();
        let mut candidates = Vec::new();
        let mut odometer = vec![0usize; runs.len()];
        'arrangements: loop {
            let mut trial_variables = variables.to_vec();
            let mut trial = Vec::new();
            trial.extend_from_slice(&(decorated.len() as u32).to_le_bytes());
            for (slot, run) in runs.iter().enumerate() {
                for &local in &run_permutations[slot][odometer[slot]] {
                    let (_, (variable, term)) = &decorated[run.start + local];
                    encode_substitution_entry(&mut trial, &mut trial_variables, variable, term);
                }
            }
            candidates.push((trial, trial_variables));
            // Advance the odometer; overflow at slot 0 ends the enumeration.
            let mut slot = runs.len();
            while slot > 0 {
                slot -= 1;
                odometer[slot] += 1;
                if odometer[slot] < run_permutations[slot].len() {
                    break;
                }
                odometer[slot] = 0;
                if slot == 0 {
                    break 'arrangements;
                }
            }
        }
        candidates
    } else {
        // Baseline order: the shape-sorted one (the only candidate for an
        // empty substitution and above the enumeration cap).
        let mut trial_variables = variables.to_vec();
        let mut trial = Vec::new();
        trial.extend_from_slice(&(decorated.len() as u32).to_le_bytes());
        for (_, (variable, term)) in &decorated {
            encode_substitution_entry(&mut trial, &mut trial_variables, variable, term);
        }
        vec![(trial, trial_variables)]
    }
}

/// Digest of a framed payload, minimized over the substitution's equal-shape
/// tie arrangements.
///
/// The canonicalization criterion of this family: the arrangement chosen is
/// the one minimizing the **entire framed payload** — the caller's `prefix`
/// (provenance and term, identical across arrangements), the arrangement's
/// entry encoding, and the caller's `tail` encoded under that arrangement's
/// committed environment (residuals and existentials for a step, nothing for
/// the meeting tail). Minimizing the substitution segment alone was wrong: an
/// arrangement tie in the entries says nothing about the environment the two
/// arrangements commit, so `{AA -> a, ZZ -> a}` with the residual `AA != b`
/// committed one environment while its alpha-equivalent residual `ZZ != b`
/// committed another and the two alpha-equivalent steps hashed differently
/// (PR #357 round 5 P2b).
///
/// The enumeration is metered against [`MAX_TIE_RESOLUTION_BYTES`]; over
/// budget, the shape-sorted baseline arrangement (candidate zero) is used.
fn framed_minimum(
    frame: &str,
    prefix: &[u8],
    candidates: Vec<(Vec<u8>, Vec<String>)>,
    tail: impl Fn(&mut Vec<u8>, &mut Vec<String>),
) -> Sha256Digest {
    let encode_candidate = |(blob, resolved): &(Vec<u8>, Vec<String>)| {
        let mut encoding = Vec::with_capacity(prefix.len() + blob.len() + 32);
        encoding.extend_from_slice(prefix);
        encoding.extend_from_slice(&(blob.len() as u32).to_le_bytes());
        encoding.extend_from_slice(blob);
        let mut variables = resolved.clone();
        tail(&mut encoding, &mut variables);
        encoding
    };
    let mut best: Option<Vec<u8>> = None;
    let mut spent: u64 = 0;
    for candidate in &candidates {
        let encoding = encode_candidate(candidate);
        spent = spent.saturating_add(encoding.len() as u64);
        if spent > MAX_TIE_RESOLUTION_BYTES {
            best = None;
            break;
        }
        if best.as_ref().is_none_or(|current| encoding < *current) {
            best = Some(encoding);
        }
    }
    let encoding = match best {
        Some(encoding) => encoding,
        None => encode_candidate(&candidates[0]),
    };
    Sha256Digest::framed(frame, &encoding)
}

/// One substitution entry: the bound variable as a renaming index (not its
/// name) and the range under the same environment.
fn encode_substitution_entry(
    out: &mut Vec<u8>,
    variables: &mut Vec<String>,
    variable: &Variable,
    term: &Term,
) {
    out.push(0x20);
    encode_term(&Term::var(variable.as_str()), variables, out);
    encode_term(term, variables, out);
}

/// Wildcard shape key for a substitution entry: the range's structure with
/// every variable erased, so the key is invariant under alpha-renaming.
fn substitution_entry_shape_key(term: &Term) -> Vec<u8> {
    let mut key = Vec::new();
    encode_shape(term, &mut key);
    key
}

/// Variable-erased structural encoding (the same scheme as the state
/// digest's constraint shape keys).
fn encode_shape(term: &Term, out: &mut Vec<u8>) {
    use alloc::string::ToString;
    match term {
        Term::Var(_) => {
            out.push(0x00);
            out.extend_from_slice(&0u32.to_le_bytes());
        }
        Term::Sym(symbol, args) => {
            out.push(0x01);
            let name = symbol.to_string();
            out.extend_from_slice(&(args.len() as u32).to_le_bytes());
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            for arg in args {
                encode_shape(arg, out);
            }
        }
    }
}

/// All permutations of `0..size`, in lexicographic order.
fn permutations(size: usize) -> Vec<Vec<usize>> {
    let mut result = Vec::new();
    let mut current: Vec<usize> = (0..size).collect();
    loop {
        result.push(current.clone());
        let mut pivot = None;
        for index in (0..current.len().saturating_sub(1)).rev() {
            if current[index] < current[index + 1] {
                pivot = Some(index);
                break;
            }
        }
        let Some(pivot) = pivot else { break };
        let mut successor = current.len() - 1;
        while current[successor] <= current[pivot] {
            successor -= 1;
        }
        current.swap(pivot, successor);
        current[pivot + 1..].reverse();
    }
    result
}

/// Deterministic seeded sub-sampling: a partial Fisher–Yates shuffle over
/// the content-key-sorted list, keeping the first `max` and re-sorting.
fn sub_sample(mut examples: Vec<PairwiseExample>, max: usize, seed: u64) -> Vec<PairwiseExample> {
    let total = examples.len();
    let mut state = seed;
    for index in 0..max {
        let offset = (splitmix64(&mut state) % (total - index) as u64) as usize;
        examples.swap(index, index + offset);
    }
    examples.truncate(max);
    examples.sort_by(|left, right| left.key.cmp(&right.key));
    examples
}

/// SplitMix64: a tiny, deterministic, dependency-free PRNG for sub-sampling.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}
