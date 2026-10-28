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
//! Admission runs before verification: the trace count, the per-trace step
//! count, and every recorded term's node count and depth are checked
//! against [`RelationLimits`] with a bounded iterative walk *before*
//! [`ReplayCertificate::verify`] replays (which clones terms). No unbounded
//! allocation or recursion can be reached from an untrusted trace.

use crate::inverse::{
    GuidanceMode, ReplayCertificate, ReplayDerivation, ReplayQuery, SymbolicPredecessor,
};
use crate::neural::encode::{CandidateInput, FeatureEncoder, FeatureVector, SearchContext};
use crate::neural::model::{canonical_digest, content_key};
use crate::neural::{FeatureSchemaV1, NeuralError};
use crate::relation::{RelationLimits, RuleId, Sha256Digest};
use crate::trs::{Term, TermSystem};

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
            // Admission precedes verification: cheap bounded checks run
            // before `verify` clones any term.
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
            check_term_bounded(root, &self.config.relation_limits())?;
            for step in steps {
                check_term_bounded(&step.term, &self.config.relation_limits())?;
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

            // Collapse duplicate candidates by canonical predecessor digest.
            let mut deduped: Vec<SymbolicPredecessor> = Vec::with_capacity(candidates.len());
            let mut seen: Vec<Sha256Digest> = Vec::with_capacity(candidates.len());
            for candidate in candidates {
                let digest =
                    Sha256Digest::canonical_term("amari.relation.term/v1", &candidate.term);
                if seen.contains(&digest) {
                    continue;
                }
                seen.push(digest);
                deduped.push(candidate);
            }

            let branch_factor = deduped.len() as u64;
            let Some(chosen_index) = deduped
                .iter()
                .position(|candidate| candidate.provenance == chosen.provenance)
            else {
                return Err(NeuralError::TraceDrift);
            };

            // Admission before identity: the chosen candidate is encoded
            // (bounded, iterative) before its digest clones the term.
            let chosen_candidate = &deduped[chosen_index];
            let chosen_input = CandidateInput::new(
                &chosen_candidate.term,
                &chosen_candidate.constraints,
                &chosen_candidate.provenance,
                self.context(system, state_term, chosen_candidate, index, branch_factor),
            );
            let chosen_features = self.encoder.encode(&chosen_input)?;
            let chosen_digest = canonical_digest(&chosen_input);

            for (candidate_index, negative) in deduped.iter().enumerate() {
                if candidate_index == chosen_index {
                    continue;
                }
                let negative_input = CandidateInput::new(
                    &negative.term,
                    &negative.constraints,
                    &negative.provenance,
                    self.context(system, state_term, negative, index, branch_factor),
                );
                let negative_features = self.encoder.encode(&negative_input)?;
                let negative_digest = canonical_digest(&negative_input);
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
    /// uniformly `false`).
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

/// Bounded, iterative admission walk over a term: node count and depth are
/// checked against the limits before anything clones or recurses over the
/// term.
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
        if let Term::Sym(_, args) = node {
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

/// Canonical per-trace digest: the authority tag, the derivation kind, and
/// every backward step's provenance, in trace order.
fn trace_digest(certificate: &ReplayCertificate) -> Sha256Digest {
    let mut payload = Vec::new();
    match certificate.guidance {
        GuidanceMode::CompleteWithinLimits => payload.push(0x00),
        GuidanceMode::HeuristicPruning { .. } => payload.push(0x01),
    }
    match &certificate.derivation {
        ReplayDerivation::Backward(derivation) => {
            payload.push(0x00);
            for step in &derivation.steps {
                encode_step(&mut payload, step);
            }
        }
        ReplayDerivation::Bidirectional(derivation) => {
            payload.push(0x01);
            for step in &derivation.backward_steps {
                encode_step(&mut payload, step);
            }
        }
    }
    Sha256Digest::framed("amari.neural.trace/v1", &payload)
}

fn encode_step(payload: &mut Vec<u8>, step: &SymbolicPredecessor) {
    payload.extend_from_slice(step.provenance.rule_id.digest().as_bytes());
    payload.extend_from_slice(step.provenance.target_hash.as_bytes());
    payload.extend_from_slice(step.provenance.predecessor_hash.as_bytes());
    payload.extend_from_slice(&step.provenance.scope.to_le_bytes());
    let position = step.provenance.position.as_slice();
    payload.extend_from_slice(&(position.len() as u32).to_le_bytes());
    for index in position {
        payload.extend_from_slice(&(*index as u64).to_le_bytes());
    }
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
