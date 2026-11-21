// SPDX-License-Identifier: MIT OR Apache-2.0

//! Safe-tensor ranker checkpoints: weights, optimizer state, and the metadata
//! that binds them (Task 43).
//!
//! A checkpoint is **two artifacts**, because the two seams that own the two
//! halves of a resumable ranker are two different, already-ratified
//! conventions:
//!
//! - **weights** — Katakuri's S5 validated-checkpoint seam
//!   ([`katakuri::checkpoint::load_validated`]): a safetensors file whose
//!   `__metadata__` carries the `ia-mlp` spec, its canonical schema hash, the
//!   data-section hash, and Amari's own binding metadata;
//! - **optimizer state** — Katakuri's ratified S6 convention
//!   ([`katakuri::optimizer`]): a moments-only safetensors file with
//!   `.adamw.m` / `.adamw.v` per parameter, an `optimizer_state_version`,
//!   `optimizer`, and `step` in `__metadata__`, all-or-nothing completeness,
//!   and lexicographic canonical order.
//!
//! Keeping them separate is the conventions' own design: S6's
//! "moments-only" shape is exactly the artifact whose parameters live in the
//! model checkpoint, and S5's loader is exactly the seam that validates those
//! parameters. This module writes both artifacts and loads both through those
//! seams — it never re-implements safe opening, byte capping, metadata
//! conventions, hashing, or the nonfinite scan.
//!
//! ## Validation order (the whole point of the type)
//!
//! [`RankerCheckpoint::load`] validates *everything* before any model is
//! built:
//!
//! 1. **Trust policy** — the caller's [`TrustPolicy`] must enable
//!    `regular_file_only`, `verify_schema_hash`, `verify_model_hash`, and
//!    `reject_nonfinite`; a lax policy is
//!    [`CheckpointError::UnsafePolicy`], never silently upgraded. The byte
//!    cap is clamped down to this module's per-artifact ceiling, so a caller
//!    can tighten admission but never widen it.
//! 2. **Weights seam** — regular-file check (`O_NOFOLLOW` on Unix),
//!    byte cap, safetensors parse, `ia.*` metadata convention, `ia.spec`
//!    parse, tensor-set/shape/dtype agreement, declared hashes, then the
//!    nonfinite scan.
//! 3. **Architecture** — the file's spec, canonical JSON, must equal the
//!    caller's expected [`RankerConfig`]; Amari's framed ranker-spec digest
//!    must agree too.
//! 4. **Binding metadata** — the checkpoint schema version, the frozen
//!    feature-schema version and digest ([`FeatureSchemaV1`]), the declared
//!    weight digest, and the training step.
//! 5. **Optimizer seam** — the S6 pipeline: policy checks, moments
//!    all-or-nothing, dtype/shape agreement against the same spec, hashes,
//!    nonfinite scan.
//! 6. **Pair consistency** — the two artifacts must describe the same run:
//!    the weights artifact must declare the optimizer artifact's canonical
//!    content digest (`amari.optimizer-digest`) and the declaration must
//!    match the loaded moments, the two artifacts must record equal step
//!    counters, and the step counter must be inside
//!    [`MAX_CHECKPOINT_STEPS`].
//!
//! Only then is a [`RankerWeights`] built and an [`AdamWState`]
//! reconstructed. [`RankerCheckpoint::ranker`] — the one call that touches
//! the tensor backend — is a *separate* step the caller makes afterwards.
//!
//! ## The pair is bound by content, not by a step counter (F2)
//!
//! Equal step counters are necessary and not sufficient: two runs of the same
//! architecture over the same corpus converge on the same step count, so
//! `weights` from run A combined with `optimizer` from run B used to load and
//! resume from moments that never belonged to those parameters — silently
//! diverging from the run the pair claims to continue.
//!
//! The pair is therefore bound by **content**. [`RankerCheckpoint::save`]
//! always records the companion optimizer artifact's canonical content digest
//! — SHA-256 over its data section, which is the concatenation of the S6
//! moments in lexicographic canonical order — under
//! [`META_OPTIMIZER_DIGEST`], and [`RankerCheckpoint::load`] *requires* that
//! key and re-derives the digest from the loaded state. A mixed pair is
//! [`CheckpointError::InconsistentCheckpoint`]; a pre-binding artifact is
//! [`CheckpointError::InvalidMetadata`] (no compatibility shim: this format is
//! experimental and unreleased).
//!
//! The digest lives in the **weights** artifact because that is the artifact
//! whose validated seam returns its metadata map ([`ValidatedCheckpoint`]).
//! S6's validated seam returns the moments and the step, not `__metadata__`,
//! so a digest written into the optimizer artifact could be neither required
//! nor verified without re-opening that file — which would re-implement safe
//! opening and byte capping, exactly what this module refuses to do. Writing
//! the binding where it can be enforced is the whole point of the binding.
//! Single-artifact (weights-only) loads are unaffected: the key is metadata,
//! not a tensor, so the S5 seam's layout and hash checks see the same bytes
//! they always did.
//!
//! ## What is trusted after validation
//!
//! Post-validation the data is trusted as **data**, not as a claim: the
//! weights are trusted because every element was shape-checked, finiteness-
//! scanned, and re-digested against the writer's declaration; the moments are
//! trusted because S6 proved them complete and shape-correct before any of
//! them was read. What is *not* trusted is the file's own prose: no
//! rejection quotes a path, a metadata value, or a tensor name from the file.
//! Errors are [`CheckpointError`] — a rule name, a count, or a ceiling — so a
//! hostile artifact cannot make a diagnostic allocate (the bounded-diagnostics
//! doctrine, applied to the checkpoint surface).
//!
//! ## Resumption is a whole-epoch operation (PR #375 round 1)
//!
//! [`train_with_state`](crate::neural::train_with_state) restarts the example
//! order at index 0 of every epoch, so a state captured *mid-epoch* would
//! continue at the wrong example and diverge from the run it claims to
//! continue — silently. A checkpoint therefore **is** an epoch boundary, and
//! the type cannot represent anything else:
//!
//! - **the step that matters is the visit count**, not the optimizer's S6
//!   `step`: a visit whose hinge is already satisfied applies no update, so
//!   `S6 step % examples` is not the position in the epoch. The artifact
//!   records the run's visit count ([`RankerCheckpoint::steps_run`]) beside
//!   the S6 counter, and both are cross-checked (`optimizer step <=
//!   steps_run`);
//! - **at construction/save** ([`RankerCheckpoint::new`],
//!   [`RankerCheckpoint::from_outcome`]) a state whose visit count is not a
//!   whole number of epochs over the corpus it ran on is
//!   [`CheckpointError::NonEpochBoundary`]. Failing here is the friendly gate:
//!   an unresumable artifact is never written;
//! - **at load**, the recorded bookkeeping is re-validated (files travel) and
//!   the boundary is re-checked against the corpus the caller intends to
//!   resume on — the same typed error, naming both numbers;
//! - **the corpus is part of the checkpoint's identity**
//!   ([`CheckpointError::CorpusMismatch`]): continuing a run over a
//!   differently sized corpus is a different computation, not a resumption.
//!
//! Neither check bounds any work: a modulo and a comparison over two `u64`s,
//! so an artifact may record an arbitrarily large visit count without costing
//! anything (the corpus length is still capped at
//! [`TrainConfig::MAX_TRAIN_EXAMPLES`], because no larger corpus can be
//! trained on).
//!
//! ## Determinism
//!
//! Two saves of the same `(spec, weights, optimizer)` produce byte-identical
//! artifacts:
//!
//! - weight tensors are written in Katakuri's canonical parameter order, each
//!   in its canonical `[out, in]` row-major layout, little-endian f32;
//! - moment tensors are written in S6's lexicographic canonical order;
//! - `__metadata__` keys are written in sorted order, values are JSON-escaped
//!   with one fixed rule, and the two declared hashes are computed over the
//!   bytes the same function emits;
//! - no timestamp, path, host, or process value ever enters an artifact.
//!
//! A load→save round trip is therefore the identity on bytes, and a resumed
//! training run produces the same artifact as the uninterrupted run it
//! continues (the tests pin both).
//!
//! ## The transpose at rest
//!
//! The ranker computes with `[in, out]` weight buffers (borsalino-tensor's
//! `matmul` layout) and the trainer keeps its moments in that same layout;
//! the checkpoint format uses Katakuri's canonical `[out, in]`. The two
//! directions are exact permutations — never arithmetic — and happen once per
//! save or load.
//!
//! ## Temporary dependency status
//!
//! The optimizer-state convention is on Katakuri's `develop` and unreleased,
//! so `amari-rewrite` carries a **git** dependency on it, pinned by revision
//! (see `Cargo.toml`). This is the Cohort 8 precedent, and it is temporary:
//! Amari returns to a version dependency before the 0.25.0 release, because
//! git dependencies block `cargo publish`. The weights seam itself is
//! unchanged Katakuri 0.2.0 surface.

use std::collections::BTreeMap;
use std::path::Path;

use katakuri::checkpoint::{
    load_validated, TrustPolicy, ValidatedCheckpoint, META_MODEL_HASH, META_SCHEMA_HASH, META_SPEC,
};
use katakuri::digest::artifact_sha256;
use katakuri::error::KatakuriError;
use katakuri::optimizer::{
    load_optimizer_state, moment_name, MomentKind, OptimizerStateError, ValidatedOptimizerState,
    META_OPTIMIZER, META_OPTIMIZER_STATE_VERSION, META_STEP, OPTIMIZER_ADAMW,
    OPTIMIZER_STATE_VERSION,
};

use crate::neural::encode::FeatureSchemaV1;
use crate::neural::train::{
    parameter_base, AdamWState, TrainConfig, TrainingMoments, TrainingOutcome,
};
use crate::neural::{MlpRanker, NeuralError, RankerConfig, RankerWeights};
use crate::relation::Sha256Digest;

/// Version of the *Amari* checkpoint layout (the binding metadata and the
/// two-artifact arrangement). Bumped only by an incompatible change.
pub const CHECKPOINT_SCHEMA_VERSION: u32 = 1;

/// `__metadata__` key carrying [`CHECKPOINT_SCHEMA_VERSION`].
pub const META_CHECKPOINT_VERSION: &str = "amari.checkpoint-version";

/// `__metadata__` key carrying [`FeatureSchemaV1::VERSION`].
pub const META_FEATURE_SCHEMA_VERSION: &str = "amari.feature-schema-version";

/// `__metadata__` key carrying [`FeatureSchemaV1::digest`] as lowercase hex.
pub const META_FEATURE_SCHEMA_HASH: &str = "amari.feature-schema-hash";

/// `__metadata__` key carrying [`RankerConfig::digest`] as lowercase hex.
pub const META_RANKER_SPEC_HASH: &str = "amari.ranker-spec-hash";

/// `__metadata__` key carrying the training step (the optimizer's S6 step
/// counter, spelled identically in both artifacts).
pub const META_TRAINING_STEP: &str = "amari.training-step";

/// `__metadata__` key carrying [`RankerWeights::digest`] as lowercase hex.
pub const META_WEIGHTS_HASH: &str = "amari.weights-hash";

/// `__metadata__` key carrying the companion **optimizer artifact's** canonical
/// content digest as lowercase hex (SHA-256 over its data section: the S6
/// moments concatenated in lexicographic canonical order).
///
/// This is the pair's content binding. Step counters can agree across runs;
/// content digests do not. See the module documentation for why the digest is
/// recorded here, in the weights artifact, rather than in the optimizer
/// artifact.
pub const META_OPTIMIZER_DIGEST: &str = "amari.optimizer-digest";

/// `__metadata__` key carrying the run's **visit count** — the training loop's
/// step counter, which is what indexes the example order (the S6 `step` skips
/// no-op visits and therefore cannot express epoch position).
pub const META_TRAINING_STEPS: &str = "amari.training-steps";

/// `__metadata__` key carrying the number of examples the run was trained
/// over: the epoch length, and part of the checkpoint's identity.
pub const META_TRAINING_EXAMPLES: &str = "amari.training-examples";

/// Largest step counter a checkpoint may declare.
///
/// The step is untrusted metadata, and the loader replays the bias-correction
/// recurrence once per step (S6 stores the step, not derived powers). Bounding
/// it keeps that replay bounded: 2^24 steps is 256 epochs over the largest
/// admissible corpus (65,536 examples), which is already far past any run the
/// CPU reference would finish.
pub const MAX_CHECKPOINT_STEPS: u64 = 16_777_216;

/// Header headroom granted on top of the largest legal data section.
///
/// The header holds the canonical spec (`<= 64 KiB` by
/// [`RankerConfig::MAX_SPEC_JSON_BYTES`]), a handful of tensor entries, and a
/// fixed set of metadata keys; 1 MiB is more than an order of magnitude of
/// slack while still being a fixed ceiling.
const HEADER_HEADROOM_BYTES: u64 = 1_048_576;

/// Byte ceiling on a weights artifact: the largest admissible data section
/// (16,777,216 f32) plus header headroom.
pub const MAX_WEIGHTS_CHECKPOINT_BYTES: u64 =
    RankerConfig::MAX_TENSOR_ELEMENTS * 4 + HEADER_HEADROOM_BYTES;

/// Byte ceiling on an optimizer-state artifact: two moments per parameter,
/// plus header headroom.
pub const MAX_OPTIMIZER_CHECKPOINT_BYTES: u64 =
    2 * (RankerConfig::MAX_TENSOR_ELEMENTS * 4) + HEADER_HEADROOM_BYTES;

/// A fully-enabled trust policy for checkpoint loading, capped at the largest
/// artifact this module can write.
///
/// Passing a laxer policy to [`RankerCheckpoint::load`] is a typed error, not
/// a silent upgrade: the four safety flags are the contract, and a caller can
/// only tighten the byte cap.
pub fn checkpoint_policy() -> TrustPolicy {
    TrustPolicy {
        regular_file_only: true,
        byte_cap: MAX_OPTIMIZER_CHECKPOINT_BYTES,
        verify_schema_hash: true,
        verify_model_hash: true,
        reject_nonfinite: true,
    }
}

/// One ranker's resumable state: architecture, validated weights, optimizer
/// state, and the binding metadata.
///
/// The fields are only reachable through [`RankerCheckpoint::new`] (which
/// cross-checks the components) or [`RankerCheckpoint::load`] (which validates
/// two artifacts), so holding one means the three pieces describe the same
/// run. Nothing here is a tensor backend: [`Self::ranker`] is the separate
/// construction step.
#[derive(Clone, Debug, PartialEq)]
pub struct RankerCheckpoint {
    config: RankerConfig,
    weights: RankerWeights,
    optimizer: AdamWState,
    /// The training loop's step counter (example visits) at the checkpoint.
    steps_run: u64,
    /// Number of examples the run was trained over (the epoch length).
    examples: u64,
}

impl RankerCheckpoint {
    /// Bind already-validated components into an epoch-boundary checkpoint.
    ///
    /// `steps_run` is the training loop's step counter (one per example
    /// visited, no-ops included) and `examples` the number of examples the run
    /// was trained over. Both are part of the resumability contract, so a
    /// mid-epoch state cannot be represented: it would resume at the wrong
    /// example (see the module documentation).
    ///
    /// The S6 `step` metadata stays the **optimizer's** counter
    /// ([`AdamWState::step`]), derived rather than passed so it cannot disagree
    /// with the state; `steps_run` is recorded beside it and must be at least
    /// as large.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::WeightsRejected`] with [`WeightsRejection::Layout`]
    /// when the weights were built for a different architecture,
    /// [`CheckpointError::OptimizerRejected`] with
    /// [`OptimizerRejection::Moments`] when the optimizer state was,
    /// [`CheckpointError::NonEpochBoundary`] when `steps_run` is not a whole
    /// number of epochs over `examples`, and
    /// [`CheckpointError::LimitExceeded`] when `examples` is not a trainable
    /// corpus length.
    pub fn new(
        config: RankerConfig,
        weights: RankerWeights,
        optimizer: AdamWState,
        steps_run: u64,
        examples: u64,
    ) -> Result<Self, CheckpointError> {
        check_weights(&config, &weights)?;
        check_optimizer(&config, &optimizer)?;
        admit_examples(examples)?;
        admit_alignment(steps_run, examples)?;
        admit_counter_crossing(steps_run, &optimizer)?;
        Ok(Self {
            config,
            weights,
            optimizer,
            steps_run,
            examples,
        })
    }

    /// Build a checkpoint from a training run's outcome and its corpus length.
    ///
    /// The visit count comes from [`TrainingOutcome::steps_run`], so a caller
    /// cannot pair a run's state with another run's step count.
    ///
    /// # Errors
    ///
    /// As [`Self::new`] — in particular
    /// [`CheckpointError::NonEpochBoundary`] for a budget-truncated run that
    /// stopped mid-epoch (pick a step budget that is a whole number of epochs).
    pub fn from_outcome(
        config: RankerConfig,
        outcome: &TrainingOutcome,
        examples: u64,
    ) -> Result<Self, CheckpointError> {
        Self::new(
            config,
            outcome.parameters().clone(),
            outcome.optimizer_state().clone(),
            u64::try_from(outcome.steps_run()).unwrap_or(u64::MAX),
            examples,
        )
    }

    /// The bound architecture.
    pub fn config(&self) -> &RankerConfig {
        &self.config
    }

    /// The validated weights.
    pub fn weights(&self) -> &RankerWeights {
        &self.weights
    }

    /// The optimizer state (moments, step counter, bias corrections).
    pub fn optimizer_state(&self) -> &AdamWState {
        &self.optimizer
    }

    /// The training step this checkpoint belongs to (the optimizer's S6 step
    /// counter: updates applied, no-op visits excluded).
    pub fn training_step(&self) -> u64 {
        u64::try_from(self.optimizer.step()).unwrap_or(u64::MAX)
    }

    /// The number of examples the run had visited at this checkpoint (the
    /// counter that indexes the example order, so it is a whole number of
    /// epochs by construction).
    pub fn steps_run(&self) -> u64 {
        self.steps_run
    }

    /// The number of examples the run was trained over (the epoch length).
    pub fn training_examples(&self) -> u64 {
        self.examples
    }

    /// Build the ranker from the validated weights.
    ///
    /// This is the only call on this type that reaches the tensor backend, and
    /// it is deliberately separate from [`Self::load`]: every file-derived
    /// check has already passed by the time a caller can ask for a model.
    ///
    /// # Errors
    ///
    /// [`NeuralError::InvalidWeights`] when the weight layer shapes disagree
    /// with the bound configuration (unreachable through
    /// [`Self::new`]/[`Self::load`], which check exactly that).
    pub fn ranker(&self) -> Result<MlpRanker, NeuralError> {
        MlpRanker::new(self.config.clone(), self.weights.clone())
    }

    /// Write the two artifacts.
    ///
    /// `weights_path` receives the safetensors weight checkpoint,
    /// `optimizer_path` the S6 optimizer-state checkpoint. Both writes are
    /// plain file writes of the canonical byte string: the caller owns the
    /// destination policy (this module never follows an untrusted *target*
    /// path; reading is the untrusted direction and is policy-checked).
    ///
    /// Both artifacts are built before either is written, because the weights
    /// artifact records the optimizer artifact's content digest: a build
    /// failure therefore leaves no half-pair on disk.
    ///
    /// # Errors
    ///
    /// [`CheckpointError::WeightsRejected`] /
    /// [`CheckpointError::OptimizerRejected`] when the in-memory components
    /// disagree with the architecture, and [`CheckpointError::WriteFailed`]
    /// when a file cannot be written.
    pub fn save(&self, weights_path: &Path, optimizer_path: &Path) -> Result<(), CheckpointError> {
        let (optimizer, optimizer_digest) = self.optimizer_artifact()?;
        let weights = self.weights_artifact(&optimizer_digest)?;
        write_artifact(weights_path, &weights, "weights")?;
        write_artifact(optimizer_path, &optimizer, "optimizer")
    }

    /// Load and validate a checkpoint pair against `expected` and `resume`.
    ///
    /// `resume` supplies the AdamW betas, which are a training-configuration
    /// choice and not checkpointed state: S6 records the step counter, so the
    /// bias-correction accumulators are re-derived from `(step, betas)`. A
    /// resumed run must therefore use the same betas it was trained with, for
    /// the same reason it must use the same learning rate. `examples` is the
    /// length of the corpus the caller intends to resume on: the checkpoint
    /// must belong to that corpus and stop on a whole-epoch boundary of it.
    ///
    /// # Errors
    ///
    /// [`CheckpointError`] — a typed, sanitized rejection from any stage of
    /// the ordered pipeline documented at the module level, plus
    /// [`CheckpointError::NonEpochBoundary`] and
    /// [`CheckpointError::CorpusMismatch`] for an artifact that cannot be
    /// resumed without silently diverging.
    pub fn load(
        weights_path: &Path,
        optimizer_path: &Path,
        policy: &TrustPolicy,
        expected: &RankerConfig,
        resume: &TrainConfig,
        examples: u64,
    ) -> Result<Self, CheckpointError> {
        let weights_policy = enforce_policy(policy, MAX_WEIGHTS_CHECKPOINT_BYTES)?;
        let optimizer_policy = enforce_policy(policy, MAX_OPTIMIZER_CHECKPOINT_BYTES)?;

        let loaded: ValidatedCheckpoint =
            load_validated(weights_path, &weights_policy).map_err(weights_rejection)?;
        if loaded.spec.canonical_json() != expected.spec().canonical_json() {
            return Err(CheckpointError::ArchitectureMismatch);
        }

        let metadata = &loaded.metadata;
        let version = metadata_u32(metadata, META_CHECKPOINT_VERSION)?;
        if version != CHECKPOINT_SCHEMA_VERSION {
            return Err(CheckpointError::UnsupportedCheckpointVersion { found: version });
        }
        let feature_version = metadata_u32(metadata, META_FEATURE_SCHEMA_VERSION)?;
        if feature_version != FeatureSchemaV1::VERSION
            || metadata_digest(metadata, META_FEATURE_SCHEMA_HASH)? != FeatureSchemaV1.digest()
        {
            return Err(CheckpointError::FeatureSchemaDrift);
        }
        if metadata_digest(metadata, META_RANKER_SPEC_HASH)? != expected.digest() {
            return Err(CheckpointError::ArchitectureMismatch);
        }
        let declared_weights = metadata_digest(metadata, META_WEIGHTS_HASH)?;
        // The pair's content binding: required, never optional. An artifact
        // written before the binding existed is rejected rather than accepted
        // without a content check (the format is experimental and unreleased).
        let declared_optimizer = metadata_digest(metadata, META_OPTIMIZER_DIGEST)?;
        let weights_step = metadata_u64(metadata, META_TRAINING_STEP)?;
        // Epoch bookkeeping, re-validated because the artifact is untrusted:
        // the recorded visit count must be a whole number of epochs over the
        // corpus it was trained on, and that corpus must be the one the caller
        // is resuming on.
        let steps_run = metadata_u64(metadata, META_TRAINING_STEPS)?;
        let recorded_examples = metadata_u64(metadata, META_TRAINING_EXAMPLES)?;
        admit_examples(recorded_examples)?;
        if recorded_examples != examples {
            return Err(CheckpointError::CorpusMismatch {
                recorded: recorded_examples,
                resume: examples,
            });
        }
        admit_examples(examples)?;
        admit_alignment(steps_run, examples)?;

        let mut tensors: Vec<(String, Vec<f32>)> = Vec::with_capacity(loaded.tensors.len());
        for tensor in &loaded.tensors {
            tensors.push((tensor.name.clone(), decode_f32(&tensor.data)?));
        }
        let borrowed: Vec<(&str, &[f32])> = tensors
            .iter()
            .map(|(name, values)| (name.as_str(), values.as_slice()))
            .collect();
        let weights = RankerWeights::from_tensors(expected, &borrowed)
            .map_err(CheckpointError::WeightValidation)?;
        if weights.digest() != declared_weights {
            return Err(CheckpointError::WeightsDigestMismatch);
        }

        let validated: ValidatedOptimizerState =
            load_optimizer_state(optimizer_path, &optimizer_policy, expected.spec())
                .map_err(optimizer_rejection)?;
        if validated.step > MAX_CHECKPOINT_STEPS {
            return Err(CheckpointError::LimitExceeded {
                resource: "checkpoint steps",
                value: usize::try_from(validated.step).unwrap_or(usize::MAX),
                limit: usize::try_from(MAX_CHECKPOINT_STEPS).unwrap_or(usize::MAX),
            });
        }
        if validated.step != weights_step {
            return Err(CheckpointError::InconsistentCheckpoint {
                detail: "the weights and optimizer artifacts record different training steps",
            });
        }
        // Content, not just counters: same-architecture, same-corpus runs agree
        // on the step count, so only the digest can refuse a mixed pair.
        if artifact_sha256(&validated.canonical_data_bytes()) != declared_optimizer.to_hex() {
            return Err(CheckpointError::InconsistentCheckpoint {
                detail: "the optimizer artifact is not the companion the weights artifact binds",
            });
        }
        let step = usize::try_from(validated.step).map_err(|_| CheckpointError::LimitExceeded {
            resource: "checkpoint steps",
            value: usize::MAX,
            limit: usize::try_from(MAX_CHECKPOINT_STEPS).unwrap_or(usize::MAX),
        })?;
        let moments = decode_moments(expected, &validated)?;
        let borrowed: Vec<(&str, &[f32])> = moments
            .iter()
            .map(|(name, values)| (name.as_str(), values.as_slice()))
            .collect();
        let optimizer =
            AdamWState::from_checkpoint(expected, step, resume, &borrowed).map_err(|_| {
                CheckpointError::OptimizerRejected {
                    kind: OptimizerRejection::Moments,
                }
            })?;
        admit_counter_crossing(steps_run, &optimizer)?;

        Ok(Self {
            config: expected.clone(),
            weights,
            optimizer,
            steps_run,
            examples,
        })
    }

    /// Build the weights artifact's bytes.
    ///
    /// `optimizer_digest` is the companion optimizer artifact's canonical
    /// content digest; it is written under [`META_OPTIMIZER_DIGEST`] and is
    /// therefore recorded by every save, not by a caller's choice.
    fn weights_artifact(&self, optimizer_digest: &str) -> Result<Vec<u8>, CheckpointError> {
        check_weights(&self.config, &self.weights)?;
        let plan = self.config.expected_tensors();
        let layers = self.weights.training_layers();
        let spec_json = self.config.spec().canonical_json();
        let mut metadata = BTreeMap::new();
        metadata.insert(META_SPEC.to_string(), spec_json.clone());
        metadata.insert(
            META_SCHEMA_HASH.to_string(),
            artifact_sha256(spec_json.as_bytes()),
        );
        metadata.insert(
            META_CHECKPOINT_VERSION.to_string(),
            CHECKPOINT_SCHEMA_VERSION.to_string(),
        );
        metadata.insert(
            META_FEATURE_SCHEMA_VERSION.to_string(),
            FeatureSchemaV1::VERSION.to_string(),
        );
        metadata.insert(
            META_FEATURE_SCHEMA_HASH.to_string(),
            FeatureSchemaV1.digest().to_hex(),
        );
        metadata.insert(
            META_RANKER_SPEC_HASH.to_string(),
            self.config.digest().to_hex(),
        );
        metadata.insert(
            META_TRAINING_STEP.to_string(),
            self.training_step().to_string(),
        );
        metadata.insert(META_TRAINING_STEPS.to_string(), self.steps_run.to_string());
        metadata.insert(
            META_TRAINING_EXAMPLES.to_string(),
            self.examples.to_string(),
        );
        metadata.insert(
            META_WEIGHTS_HASH.to_string(),
            self.weights.digest().to_hex(),
        );
        metadata.insert(
            META_OPTIMIZER_DIGEST.to_string(),
            optimizer_digest.to_string(),
        );

        let artifact = build_artifact(&plan, &metadata, MAX_WEIGHTS_CHECKPOINT_BYTES, |index| {
            let (weight_t, bias, in_width, out_width) = layers[index / 2];
            if index % 2 == 0 {
                canonical_layout(weight_t, in_width, out_width)
            } else {
                bias.to_vec()
            }
        });
        artifact
            .map(|(bytes, _)| bytes)
            .map_err(|failure| match failure {
                ArtifactFailure::PlanMismatch => CheckpointError::WeightsRejected {
                    kind: WeightsRejection::Layout,
                },
                ArtifactFailure::TooLarge { value, limit } => CheckpointError::LimitExceeded {
                    resource: "checkpoint bytes",
                    value,
                    limit,
                },
            })
    }

    /// Build the optimizer-state artifact's bytes, plus its canonical content
    /// digest (SHA-256 over the data section, which is what the weights
    /// artifact binds).
    fn optimizer_artifact(&self) -> Result<(Vec<u8>, String), CheckpointError> {
        check_optimizer(&self.config, &self.optimizer)?;
        let moments = self.optimizer.training_moments();
        let depth = moments.len();
        let mut slots: Vec<MomentSlot> = Vec::with_capacity(depth * 4);
        for (index, layer) in moments.iter().enumerate() {
            let base = parameter_base(index, depth);
            let weight = format!("{base}.weight");
            let bias = format!("{base}.bias");
            slots.push(MomentSlot {
                name: moment_name(&weight, MomentKind::First),
                shape: vec![layer.out_width, layer.in_width],
                layer: index,
                kind: SlotKind::WeightFirst,
            });
            slots.push(MomentSlot {
                name: moment_name(&weight, MomentKind::Second),
                shape: vec![layer.out_width, layer.in_width],
                layer: index,
                kind: SlotKind::WeightSecond,
            });
            slots.push(MomentSlot {
                name: moment_name(&bias, MomentKind::First),
                shape: vec![layer.out_width],
                layer: index,
                kind: SlotKind::BiasFirst,
            });
            slots.push(MomentSlot {
                name: moment_name(&bias, MomentKind::Second),
                shape: vec![layer.out_width],
                layer: index,
                kind: SlotKind::BiasSecond,
            });
        }
        // S6 canonical order is lexicographic over the moment names.
        slots.sort_by(|left, right| left.name.cmp(&right.name));
        let plan: Vec<(String, Vec<usize>)> = slots
            .iter()
            .map(|slot| (slot.name.clone(), slot.shape.clone()))
            .collect();

        let spec_json = self.config.spec().canonical_json();
        let mut metadata = BTreeMap::new();
        metadata.insert(
            META_OPTIMIZER_STATE_VERSION.to_string(),
            OPTIMIZER_STATE_VERSION.to_string(),
        );
        metadata.insert(META_OPTIMIZER.to_string(), OPTIMIZER_ADAMW.to_string());
        metadata.insert(META_STEP.to_string(), self.training_step().to_string());
        metadata.insert(META_SPEC.to_string(), spec_json.clone());
        metadata.insert(
            META_SCHEMA_HASH.to_string(),
            artifact_sha256(spec_json.as_bytes()),
        );

        let artifact = build_artifact(&plan, &metadata, MAX_OPTIMIZER_CHECKPOINT_BYTES, |index| {
            moment_buffer(&moments[slots[index].layer], slots[index].kind)
        });
        artifact.map_err(|failure| match failure {
            ArtifactFailure::PlanMismatch => CheckpointError::OptimizerRejected {
                kind: OptimizerRejection::Moments,
            },
            ArtifactFailure::TooLarge { value, limit } => CheckpointError::LimitExceeded {
                resource: "checkpoint bytes",
                value,
                limit,
            },
        })
    }
}

/// Typed, sanitized checkpoint failures.
///
/// Every variant names a rule, a count, or a ceiling. Nothing here carries a
/// path, an artifact's own text, or a backend error value: the file is
/// untrusted, and a diagnostic must not become an allocation channel or a
/// path oracle (the bounded-diagnostics doctrine).
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CheckpointError {
    /// The weights artifact failed Katakuri's validated-checkpoint seam.
    #[error("ranker weights checkpoint rejected at the {kind:?} check")]
    WeightsRejected {
        /// Which stage or rule fired.
        kind: WeightsRejection,
    },
    /// The optimizer-state artifact failed Katakuri's S6 seam.
    #[error("optimizer-state checkpoint rejected at the {kind:?} check")]
    OptimizerRejected {
        /// Which stage or rule fired.
        kind: OptimizerRejection,
    },
    /// The weights passed the seam but failed Amari's own validation (shape,
    /// coverage, finiteness, or the architecture's magnitude bound).
    #[error("checkpoint weights failed ranker validation: {0}")]
    WeightValidation(NeuralError),
    /// The declared canonical weight digest does not match the tensors.
    #[error("checkpoint weights do not match their declared digest")]
    WeightsDigestMismatch,
    /// The trust policy leaves a safety check disabled.
    #[error("unsafe checkpoint trust policy: {flag} must be enabled")]
    UnsafePolicy {
        /// The flag the caller left off.
        flag: &'static str,
    },
    /// A fixed or caller-tightened ceiling rejected the artifact.
    #[error("checkpoint limit exceeded: {resource} (value {value}, ceiling {limit})")]
    LimitExceeded {
        /// The rejected resource.
        resource: &'static str,
        /// The offending value.
        value: usize,
        /// The applied ceiling.
        limit: usize,
    },
    /// The artifact declares a checkpoint layout version this crate does not
    /// implement. Forward compatibility is a new version, never a guess.
    #[error("unsupported checkpoint version {found} (this crate implements 1)")]
    UnsupportedCheckpointVersion {
        /// The version found in metadata.
        found: u32,
    },
    /// The artifact was produced under a different frozen feature schema, so
    /// its features and weights no longer have the same meaning.
    #[error("checkpoint was produced under a different feature schema")]
    FeatureSchemaDrift,
    /// The artifact's architecture is not the caller's expected architecture.
    #[error("checkpoint architecture does not match the expected configuration")]
    ArchitectureMismatch,
    /// A required Amari metadata key is absent or does not parse.
    #[error("checkpoint metadata '{key}' is absent or malformed")]
    InvalidMetadata {
        /// The offending key (a constant from this module, never file text).
        key: &'static str,
    },
    /// The two artifacts do not belong to the same run.
    #[error("checkpoint artifacts are inconsistent: {detail}")]
    InconsistentCheckpoint {
        /// What disagreed.
        detail: &'static str,
    },
    /// The checkpoint does not sit on a whole-epoch boundary for the corpus it
    /// would be resumed against, so resuming it would silently diverge from
    /// the run it claims to continue.
    #[error(
        "checkpoint stops at step {step}, which is not a whole number of epochs over \
         {examples} examples"
    )]
    NonEpochBoundary {
        /// The run's visit count at the checkpoint.
        step: u64,
        /// The epoch length (corpus size) that count is measured against.
        examples: u64,
    },
    /// The checkpoint was produced over a differently sized corpus than the
    /// one it would be resumed on. Continuing over another corpus is a
    /// different computation, never a resumption.
    #[error("checkpoint was trained over {recorded} examples, the resume corpus has {resume}")]
    CorpusMismatch {
        /// The corpus length recorded in the artifact.
        recorded: u64,
        /// The corpus length the caller intends to resume on.
        resume: u64,
    },
    /// An F32 tensor span was not a whole number of elements. Unreachable
    /// after the seam's span validation; reported rather than assumed.
    #[error("checkpoint tensor data is not a whole number of F32 elements")]
    MalformedTensorData,
    /// Writing an artifact failed. The operating-system detail is reduced to
    /// its kind so no path is carried.
    #[error("checkpoint write failed during {operation} ({kind:?})")]
    WriteFailed {
        /// The file being written (`weights` or `optimizer`).
        operation: &'static str,
        /// The reduced I/O failure kind.
        kind: std::io::ErrorKind,
    },
}

/// Which stage or rule rejected a weights artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeightsRejection {
    /// The file could not be opened or read.
    Io,
    /// `regular_file_only` refused it: a symlink, FIFO, device, or directory.
    NotRegularFile,
    /// The file exceeded the byte cap before any parse work.
    TooLarge,
    /// The safetensors header or layout failed to parse.
    Parse,
    /// The `ia.*` metadata convention was violated.
    Metadata,
    /// `ia.spec` is not a valid `ia-mlp` specification.
    Spec,
    /// The tensor set disagrees with the spec (missing, unexpected, wrong
    /// shape, or wrong dtype).
    Layout,
    /// The declared schema hash does not match the spec.
    SchemaHash,
    /// The declared model hash does not match the data section.
    ModelHash,
    /// `reject_nonfinite` found NaN or infinity.
    Nonfinite,
    /// Any other seam failure.
    Other,
}

/// Which stage or rule rejected an optimizer-state artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptimizerRejection {
    /// The file could not be opened or read.
    Io,
    /// `regular_file_only` refused it.
    NotRegularFile,
    /// The file exceeded the byte cap.
    TooLarge,
    /// The safetensors header or layout failed to parse.
    Parse,
    /// The `ia.*` metadata convention was violated, or `ia.spec` is
    /// malformed.
    Metadata,
    /// A parameter is missing one of its two moments (all-or-nothing).
    Moments,
    /// A tensor outside the derived moment set.
    UnexpectedTensor,
    /// Parameter tensors are present but not complete.
    PartialParameters,
    /// A carried parameter tensor disagrees with the spec.
    ParameterShape,
    /// A moment's shape disagrees with its parameter's.
    MomentShape,
    /// A moment's dtype disagrees with its parameter's.
    MomentDtype,
    /// A required S6 metadata key is absent.
    MissingMetadata,
    /// An S6 metadata value does not parse.
    MalformedMetadata,
    /// `optimizer_state_version` names a convention this crate does not
    /// implement.
    UnsupportedVersion,
    /// `optimizer` names an optimizer this convention does not cover.
    UnsupportedOptimizer,
    /// The artifact belongs to a different model.
    SpecMismatch,
    /// `reject_nonfinite` found NaN or infinity.
    Nonfinite,
    /// Any other seam failure.
    Other,
}

/// Map a S5 seam failure to the sanitized weights vocabulary.
fn weights_rejection(error: KatakuriError) -> CheckpointError {
    CheckpointError::WeightsRejected {
        kind: s5_kind(&error),
    }
}

/// The S5 stage a seam failure reached, in the weights vocabulary.
fn s5_kind(error: &KatakuriError) -> WeightsRejection {
    match error {
        KatakuriError::CheckpointIo { .. } => WeightsRejection::Io,
        KatakuriError::CheckpointNotRegularFile { .. } => WeightsRejection::NotRegularFile,
        KatakuriError::CheckpointTooLarge { .. } => WeightsRejection::TooLarge,
        KatakuriError::ProvenanceMalformed { .. } | KatakuriError::UnknownIaMetadata { .. } => {
            WeightsRejection::Metadata
        }
        KatakuriError::MlpSpecParse { .. } => WeightsRejection::Spec,
        KatakuriError::WeightLayoutMismatch { .. } => WeightsRejection::Layout,
        KatakuriError::SchemaHashMismatch { .. } => WeightsRejection::SchemaHash,
        KatakuriError::ModelHashMismatch { .. } => WeightsRejection::ModelHash,
        KatakuriError::NonfiniteTensorValue { .. } => WeightsRejection::Nonfinite,
        KatakuriError::TruncatedFile { .. }
        | KatakuriError::HeaderLengthExceedsFile { .. }
        | KatakuriError::HeaderTooLarge { .. }
        | KatakuriError::HeaderParse { .. }
        | KatakuriError::HeaderNotAnObject
        | KatakuriError::InvalidMetadata { .. }
        | KatakuriError::InvalidDtype { .. }
        | KatakuriError::InvalidTensorInfo { .. }
        | KatakuriError::OffsetOutOfBounds { .. }
        | KatakuriError::SizeMismatch { .. }
        | KatakuriError::SizeOverflow { .. }
        | KatakuriError::OverlappingTensors { .. }
        | KatakuriError::TensorNotFound { .. } => WeightsRejection::Parse,
        _ => WeightsRejection::Other,
    }
}

/// The S5 stage a seam failure reached, in the optimizer vocabulary.
///
/// The same pipeline, the same rules; only the reporting vocabulary differs,
/// so a failure on the optimizer artifact is never reported as a weights
/// failure.
fn s5_optimizer_kind(error: &KatakuriError) -> OptimizerRejection {
    match s5_kind(error) {
        WeightsRejection::Io => OptimizerRejection::Io,
        WeightsRejection::NotRegularFile => OptimizerRejection::NotRegularFile,
        WeightsRejection::TooLarge => OptimizerRejection::TooLarge,
        WeightsRejection::Parse => OptimizerRejection::Parse,
        WeightsRejection::Metadata | WeightsRejection::Spec => OptimizerRejection::Metadata,
        WeightsRejection::Nonfinite => OptimizerRejection::Nonfinite,
        // A weights-layout or hash failure on a moments-only artifact can
        // only be a shape-level disagreement with the S6 moment set.
        WeightsRejection::Layout | WeightsRejection::SchemaHash | WeightsRejection::ModelHash => {
            OptimizerRejection::MomentShape
        }
        WeightsRejection::Other => OptimizerRejection::Other,
    }
}

/// Map a S6 seam failure to the sanitized optimizer vocabulary.
///
/// The convention's own variants are already typed and bounded; a wrapped
/// [`KatakuriError`] is re-classified rather than re-wrapped, because its
/// `Display` carries a path.
fn optimizer_rejection(error: OptimizerStateError) -> CheckpointError {
    let kind = match error {
        OptimizerStateError::Checkpoint(inner) => s5_optimizer_kind(&inner),
        OptimizerStateError::MissingMoment { .. } => OptimizerRejection::Moments,
        OptimizerStateError::UnexpectedTensor { .. } => OptimizerRejection::UnexpectedTensor,
        OptimizerStateError::PartialParameters { .. } => OptimizerRejection::PartialParameters,
        OptimizerStateError::ParameterMismatch { .. } => OptimizerRejection::ParameterShape,
        OptimizerStateError::MomentShapeMismatch { .. } => OptimizerRejection::MomentShape,
        OptimizerStateError::MomentDtypeMismatch { .. } => OptimizerRejection::MomentDtype,
        OptimizerStateError::MissingMetadata { .. } => OptimizerRejection::MissingMetadata,
        OptimizerStateError::MalformedMetadata { .. } => OptimizerRejection::MalformedMetadata,
        OptimizerStateError::UnsupportedStateVersion { .. } => {
            OptimizerRejection::UnsupportedVersion
        }
        OptimizerStateError::UnsupportedOptimizer { .. } => {
            OptimizerRejection::UnsupportedOptimizer
        }
        OptimizerStateError::SpecMismatch { .. } => OptimizerRejection::SpecMismatch,
        OptimizerStateError::MalformedSpecMetadata { .. } => OptimizerRejection::Metadata,
    };
    CheckpointError::OptimizerRejected { kind }
}

/// Re-check the caller's policy and clamp its byte cap to `cap`.
///
/// The four safety flags are the checkpoint contract: a caller may tighten the
/// cap but cannot turn off the regular-file check, either declared hash check,
/// or the nonfinite scan.
fn enforce_policy(policy: &TrustPolicy, cap: u64) -> Result<TrustPolicy, CheckpointError> {
    for (flag, enabled) in [
        ("regular_file_only", policy.regular_file_only),
        ("verify_schema_hash", policy.verify_schema_hash),
        ("verify_model_hash", policy.verify_model_hash),
        ("reject_nonfinite", policy.reject_nonfinite),
    ] {
        if !enabled {
            return Err(CheckpointError::UnsafePolicy { flag });
        }
    }
    Ok(TrustPolicy {
        byte_cap: policy.byte_cap.min(cap),
        ..policy.clone()
    })
}

/// Admit a corpus length: no larger corpus can be trained on, and an empty one
/// has no epoch to be aligned to (and would divide by zero).
fn admit_examples(examples: u64) -> Result<(), CheckpointError> {
    if examples == 0
        || examples > u64::try_from(TrainConfig::MAX_TRAIN_EXAMPLES).unwrap_or(u64::MAX)
    {
        return Err(CheckpointError::LimitExceeded {
            resource: "checkpoint corpus examples",
            value: usize::try_from(examples).unwrap_or(usize::MAX),
            limit: TrainConfig::MAX_TRAIN_EXAMPLES,
        });
    }
    Ok(())
}

/// Admit a visit count as a whole number of epochs over `examples`.
///
/// O(1) whatever the count: a modulo and a comparison, so an artifact cannot
/// make the loader do work proportional to a number it wrote.
fn admit_alignment(steps_run: u64, examples: u64) -> Result<(), CheckpointError> {
    if examples == 0 || !steps_run.is_multiple_of(examples) {
        return Err(CheckpointError::NonEpochBoundary {
            step: steps_run,
            examples,
        });
    }
    Ok(())
}

/// The optimizer cannot have applied more updates than the run visited
/// examples: one update is the most a visit can do.
fn admit_counter_crossing(steps_run: u64, optimizer: &AdamWState) -> Result<(), CheckpointError> {
    if u64::try_from(optimizer.step()).unwrap_or(u64::MAX) > steps_run {
        return Err(CheckpointError::InconsistentCheckpoint {
            detail: "the optimizer advanced more often than the run visited examples",
        });
    }
    Ok(())
}

/// One artifact tensor's placement in the canonical writer order.
struct MomentSlot {
    name: String,
    shape: Vec<usize>,
    layer: usize,
    kind: SlotKind,
}

/// Which of a layer's four moment buffers a slot names.
#[derive(Clone, Copy)]
enum SlotKind {
    WeightFirst,
    WeightSecond,
    BiasFirst,
    BiasSecond,
}

/// The canonical `[out, in]` bytes of one moment buffer.
fn moment_buffer(moments: &TrainingMoments<'_>, kind: SlotKind) -> Vec<f32> {
    match kind {
        SlotKind::WeightFirst => {
            canonical_layout(moments.weight_m, moments.in_width, moments.out_width)
        }
        SlotKind::WeightSecond => {
            canonical_layout(moments.weight_v, moments.in_width, moments.out_width)
        }
        SlotKind::BiasFirst => moments.bias_m.to_vec(),
        SlotKind::BiasSecond => moments.bias_v.to_vec(),
    }
}

/// Reinterpret `[in, out]` row-major as `[out, in]` row-major. Exact: a
/// permutation of elements, never arithmetic.
fn canonical_layout(values: &[f32], in_width: usize, out_width: usize) -> Vec<f32> {
    let mut canonical = vec![0.0f32; values.len()];
    for input in 0..in_width {
        for output in 0..out_width {
            canonical[output * in_width + input] = values[input * out_width + output];
        }
    }
    canonical
}

/// The architecture's `(in_width, out_width)` per layer, head last.
fn config_layer_shapes(config: &RankerConfig) -> Vec<(usize, usize)> {
    let mut shapes = Vec::with_capacity(config.spec().hidden.len() + 1);
    let mut in_width = config.spec().input_width as usize;
    for layer in &config.spec().hidden {
        let out_width = layer.width as usize;
        shapes.push((in_width, out_width));
        in_width = out_width;
    }
    shapes.push((in_width, config.spec().head.width as usize));
    shapes
}

/// Check that a weight set's layer shapes are the architecture's.
fn check_weights(config: &RankerConfig, weights: &RankerWeights) -> Result<(), CheckpointError> {
    let expected = config_layer_shapes(config);
    let layers = weights.training_layers();
    if layers.len() != expected.len() {
        return Err(CheckpointError::WeightsRejected {
            kind: WeightsRejection::Layout,
        });
    }
    for ((weight_t, bias, in_width, out_width), (expected_in, expected_out)) in
        layers.into_iter().zip(&expected)
    {
        if in_width != *expected_in
            || out_width != *expected_out
            || weight_t.len() != in_width * out_width
            || bias.len() != out_width
        {
            return Err(CheckpointError::WeightsRejected {
                kind: WeightsRejection::Layout,
            });
        }
    }
    Ok(())
}

/// Check that an optimizer state's layer shapes are the architecture's.
fn check_optimizer(config: &RankerConfig, optimizer: &AdamWState) -> Result<(), CheckpointError> {
    let expected = config_layer_shapes(config);
    let moments = optimizer.training_moments();
    if moments.len() != expected.len() {
        return Err(CheckpointError::OptimizerRejected {
            kind: OptimizerRejection::Moments,
        });
    }
    for (layer, (expected_in, expected_out)) in moments.iter().zip(&expected) {
        if layer.in_width != *expected_in
            || layer.out_width != *expected_out
            || layer.weight_m.len() != layer.in_width * layer.out_width
            || layer.weight_v.len() != layer.in_width * layer.out_width
            || layer.bias_m.len() != layer.out_width
            || layer.bias_v.len() != layer.out_width
        {
            return Err(CheckpointError::OptimizerRejected {
                kind: OptimizerRejection::Moments,
            });
        }
    }
    Ok(())
}

/// Decode one F32 tensor's little-endian bytes.
///
/// Bounded by construction: the seam has already proved each span equals its
/// declared shape's element count.
fn decode_f32(bytes: &[u8]) -> Result<Vec<f32>, CheckpointError> {
    let (chunks, remainder) = bytes.as_chunks::<4>();
    if !remainder.is_empty() {
        return Err(CheckpointError::MalformedTensorData);
    }
    let mut values = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        values.push(f32::from_le_bytes(*chunk));
    }
    Ok(values)
}

/// Decode every parameter's two moments, in parameter order.
fn decode_moments(
    expected: &RankerConfig,
    validated: &ValidatedOptimizerState,
) -> Result<Vec<(String, Vec<f32>)>, CheckpointError> {
    let params = expected.expected_tensors();
    let mut moments = Vec::with_capacity(params.len() * 2);
    for (param, _) in &params {
        // S6 proved all-or-nothing completeness; a miss here would be a
        // contradiction between the seam and this loop, not a file issue.
        let Some((first, second)) = validated.moments_for(param) else {
            return Err(CheckpointError::OptimizerRejected {
                kind: OptimizerRejection::Moments,
            });
        };
        moments.push((first.name.clone(), decode_f32(&first.data)?));
        moments.push((second.name.clone(), decode_f32(&second.data)?));
    }
    Ok(moments)
}

/// A required metadata value.
fn required_metadata<'a>(
    metadata: &'a BTreeMap<String, String>,
    key: &'static str,
) -> Result<&'a str, CheckpointError> {
    metadata
        .get(key)
        .map(String::as_str)
        .ok_or(CheckpointError::InvalidMetadata { key })
}

/// A required `u32` metadata value.
fn metadata_u32(
    metadata: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<u32, CheckpointError> {
    required_metadata(metadata, key)?
        .parse()
        .map_err(|_| CheckpointError::InvalidMetadata { key })
}

/// A required `u64` metadata value.
fn metadata_u64(
    metadata: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<u64, CheckpointError> {
    required_metadata(metadata, key)?
        .parse()
        .map_err(|_| CheckpointError::InvalidMetadata { key })
}

/// A required hex digest metadata value.
fn metadata_digest(
    metadata: &BTreeMap<String, String>,
    key: &'static str,
) -> Result<Sha256Digest, CheckpointError> {
    Sha256Digest::parse_hex(required_metadata(metadata, key)?)
        .map_err(|_| CheckpointError::InvalidMetadata { key })
}

/// Write one artifact, reduced to a bounded error.
fn write_artifact(
    path: &Path,
    bytes: &[u8],
    operation: &'static str,
) -> Result<(), CheckpointError> {
    std::fs::write(path, bytes).map_err(|error| CheckpointError::WriteFailed {
        operation,
        kind: error.kind(),
    })
}

/// Why serializing an artifact failed.
enum ArtifactFailure {
    /// A yielded tensor disagreed with its plan entry (unreachable for
    /// components that passed [`check_weights`] / [`check_optimizer`]).
    PlanMismatch,
    /// The serialized artifact exceeded its ceiling.
    TooLarge {
        /// The artifact's size in bytes.
        value: usize,
        /// The applied ceiling.
        limit: usize,
    },
}

/// Serialize a canonical safetensors artifact.
///
/// `plan` is the tensor order and shapes; `values` yields one tensor's f32
/// elements at a time, so the writer never holds more than one tensor's copy.
/// `ia.model-hash` is computed here, over the bytes this function emits, and
/// inserted into the metadata before the header is built. The returned string
/// is the artifact's **canonical content digest** — the same
/// `artifact_sha256` over the same data section, which is what the pair
/// binding records and what the loader re-derives from the validated state.
fn build_artifact<F>(
    plan: &[(String, Vec<usize>)],
    metadata: &BTreeMap<String, String>,
    cap: u64,
    mut values: F,
) -> Result<(Vec<u8>, String), ArtifactFailure>
where
    F: FnMut(usize) -> Vec<f32>,
{
    let mut data: Vec<u8> = Vec::new();
    let mut spans: Vec<(usize, usize)> = Vec::with_capacity(plan.len());
    for (index, (_, shape)) in plan.iter().enumerate() {
        let elements: usize = shape.iter().product();
        let tensor = values(index);
        if tensor.len() != elements {
            return Err(ArtifactFailure::PlanMismatch);
        }
        let start = data.len();
        for value in &tensor {
            data.extend_from_slice(&value.to_le_bytes());
        }
        spans.push((start, data.len()));
    }

    let mut metadata = metadata.clone();
    let content_digest = artifact_sha256(&data);
    metadata.insert(META_MODEL_HASH.to_string(), content_digest.clone());
    let header = build_header(plan, &spans, &metadata);

    let limit = usize::try_from(cap).unwrap_or(usize::MAX);
    let total = 8usize
        .checked_add(header.len())
        .and_then(|prefix| prefix.checked_add(data.len()))
        .ok_or(ArtifactFailure::TooLarge {
            value: usize::MAX,
            limit,
        })?;
    if total > limit {
        return Err(ArtifactFailure::TooLarge {
            value: total,
            limit,
        });
    }
    let mut artifact = Vec::with_capacity(total);
    artifact.extend_from_slice(&(header.len() as u64).to_le_bytes());
    artifact.extend_from_slice(header.as_bytes());
    artifact.extend_from_slice(&data);
    Ok((artifact, content_digest))
}

/// Build the deterministic safetensors header.
///
/// Metadata keys are written in sorted order (`BTreeMap`), tensors in the
/// plan's order, and every string is escaped by one fixed rule — so the same
/// inputs always produce the same bytes.
fn build_header(
    plan: &[(String, Vec<usize>)],
    spans: &[(usize, usize)],
    metadata: &BTreeMap<String, String>,
) -> String {
    let mut header = String::with_capacity(512 + 64 * metadata.len());
    header.push('{');
    for ((name, shape), (start, end)) in plan.iter().zip(spans) {
        push_json_string(&mut header, name);
        header.push_str(":{\"dtype\":\"F32\",\"shape\":[");
        for (index, dimension) in shape.iter().enumerate() {
            if index > 0 {
                header.push(',');
            }
            header.push_str(&dimension.to_string());
        }
        header.push_str("],\"data_offsets\":[");
        header.push_str(&start.to_string());
        header.push(',');
        header.push_str(&end.to_string());
        header.push_str("]},");
    }
    header.push_str("\"__metadata__\":{");
    for (index, (key, value)) in metadata.iter().enumerate() {
        if index > 0 {
            header.push(',');
        }
        push_json_string(&mut header, key);
        header.push(':');
        push_json_string(&mut header, value);
    }
    header.push_str("}}");
    header
}

/// Append `text` as a JSON string literal.
///
/// Hand-rolled so the artifact's bytes depend on this crate alone: a
/// serializer's map iteration or feature flags must never be able to change a
/// checkpoint's bytes.
fn push_json_string(out: &mut String, text: &str) {
    out.push('"');
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            control if control < ' ' => {
                out.push_str(&format!("\\u{:04x}", u32::from(control)));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}
