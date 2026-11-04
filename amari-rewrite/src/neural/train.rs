// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded, deterministic AdamW training for the candidate ranker.
//!
//! Task 42 closes the loop Task 40 opened: the ranker's forward pass is
//! differentiable by hand, and this module descends its parameters over the
//! pairwise examples Task 41 derives from replay-validated traces. The
//! structure mirrors the two halves it trains:
//!
//! - **The objective is a hinge**, `max(0, margin - (s_chosen - s_unchosen))`,
//!   evaluated over Task 41 [`Dataset`] examples (or any hand-built
//!   [`TrainingExample`], which is how the toy convergence fixtures are
//!   assembled). There is no softmax, no cross-entropy, no log-sum-exp.
//! - **The arithmetic is the forward pass's own**, re-derived per step: the
//!   hand backprop below walks the same `Linear -> ReLU -> Linear -> scalar`
//!   graph with the same serial row-major accumulation order
//!   borsalino-tensor's CPU reference uses (`k` ascending, one `f32`
//!   accumulator per output element), so the gradient of the loss is the
//!   gradient of the score the ranker computes.
//!
//! ## No transcendental functions, anywhere
//!
//! The forward pass is ReLU-only precisely so scoring is bit-identical on
//! every platform ([`crate::neural::RankerConfig`] explains why a libm
//! `tanh` would break that). A trainer that reintroduced `exp`, `log`, or
//! `powf` would break it just as thoroughly — through the *parameters* this
//! time, since one ulp of drift in an update changes every later score. The
//! loop therefore computes the hinge with subtraction and comparison, bias
//! correction with an incrementally-multiplied `beta^t` accumulator, and
//! normalizes with `f32::sqrt`, which IEEE-754 requires to be correctly
//! rounded (unlike the transcendentals, whose last bit is
//! implementation-defined). Nothing in this module calls a libm function.
//!
//! ## No wall clock (D12)
//!
//! `deadline`-truncated training is not replayable: the parameters depend on
//! how fast the machine was, so a re-run cannot reproduce them. This module
//! simply does not offer a time budget. The only stopping conditions are
//! [`TrainConfig::max_steps`] and convergence, and both are part of the
//! configuration, so a run is a pure function of
//! `(architecture, initial weights, config, examples)`.
//!
//! ## Determinism
//!
//! Every source of order is fixed:
//!
//! - **Initialization** is [`seeded_weights`]: SplitMix64 draws, consumed in
//!   Katakuri's canonical tensor order, mapped to `f32` by the documented
//!   integer-to-fraction rule below. Same seed, same bits, on any platform.
//! - **Example order** is [`example_order`]: the dataset's own canonical
//!   content-key order, or a Fisher-Yates permutation over a SplitMix64
//!   stream salted away from the initialization stream. Both are pure
//!   functions of `(seed, len)`.
//! - **Update order** is the parameter order (hidden layers, then head; each
//!   layer's weight buffer before its bias buffer), and within a buffer the
//!   row-major element order.
//! - **Gradient accumulation order** is serial: the chosen example's
//!   contribution first (negated), then the unchosen example's. No reduction
//!   is reordered, tree-shaped, or parallel.
//!
//! ## Typed outcomes, and what `Partial` is not
//!
//! [`TrainingOutcome::Completed`] carries the converged parameters and the
//! optimizer state; [`TrainingOutcome::Partial`] carries a snapshot taken
//! when the step budget ran out. A `Partial` result is **not replayable** in
//! the evidentiary sense ([`TrainingOutcome::is_replayable`] is `false`): it
//! certifies nothing about convergence, so a consumer must not treat its
//! parameters as a trained ranker. The run is still reproducible — the
//! budget is part of the config — but reproducibility is not the same claim
//! as convergence.
//!
//! ## Non-finite values are typed errors, not outcomes
//!
//! Every score, loss, gradient element, moment, and updated parameter is
//! checked on every step; the first non-finite value stops the run with
//! [`TrainingError::NonFinite`] carrying the step index (0-based). It is an
//! error rather than a `Partial` reason because the state at that point
//! cannot be used at all: `NaN` propagates silently through every later
//! score. Nothing panics, and no `NaN` is ever returned as a parameter.
//!
//! ## Allocation discipline
//!
//! One run allocates: one parameter buffer set (copied from the initial
//! weights), one gradient buffer set, two forward caches, one backward
//! scratch set, one AdamW state, and one example-order vector — each sized
//! from the *validated* architecture/config/dataset, once. Each step
//! overwrites those buffers in place; only the outcome's `RankerWeights` and
//! exported entries are built at the end. Admission runs before any of it:
//! the step ceiling, the example ceiling, and the architecture's parameter
//! ceiling are all checked on scalars, so a rejected configuration costs a
//! comparison rather than an allocation.
//!
//! ## Provisional optimizer-state naming (S6)
//!
//! [`AdamWState::optimizer_state_entries`] is the *only* place the state's
//! tensor names are spelled. It exports `<param>.adamw.m` and
//! `<param>.adamw.v` for each parameter, where `<param>` is the canonical
//! Katakuri tensor name (`hidden.0.weight`, `head.bias`, ...) and the values
//! are in that tensor's canonical `[out, in]` row-major layout. The
//! convention is **provisional**: Katakuri's S6 work defines the ratified
//! optimizer-state convention in parallel, and Task 43 reconciles this
//! function with it. Nothing else in the crate may depend on the suffixes.

use thiserror::Error;

use crate::neural::{
    Dataset, DatasetConfig, FeatureVector, PairwiseExample, RankerConfig, RankerWeights,
    CANDIDATE_FEATURE_WIDTH,
};

/// Default hinge margin (`1.0`): a chosen score must beat its unchosen
/// counterpart by a full unit before the example contributes no loss.
///
/// One is the natural unit for scores that are sums of `[0, 1]` features
/// times bounded weights, and it is large enough that a converged ranker's
/// ordering is not a rounding artifact.
pub const DEFAULT_MARGIN: f32 = 1.0;

/// Provisional suffix of the first-moment export (`S6-provisional`).
pub const ADAMW_MOMENT_SUFFIX: &str = ".adamw.m";

/// Provisional suffix of the second-moment export (`S6-provisional`).
pub const ADAMW_VELOCITY_SUFFIX: &str = ".adamw.v";

/// Export naming status, so a consumer can see it without reading Task 43.
pub const ADAMW_NAMING_STATUS: &str =
    "provisional S6 convention (`<param>.adamw.m` / `<param>.adamw.v`); Task 43 reconciles";

/// Magnitude of the seeded initialization: weights land in
/// `[-INIT_SCALE, INIT_SCALE)`.
///
/// Small enough that a fresh ranker's hidden pre-activations stay inside the
/// ReLU's live range (a dead unit learns nothing), large enough that
/// immediately-symmetric units diverge. It is a documented constant rather
/// than a heuristic: changing it changes every seeded initialization, so it
/// is part of the reproducibility contract.
pub const INIT_SCALE: f32 = 0.125;

/// Salt that separates the example-order stream from the initialization
/// stream (both are SplitMix64), so `seeded_weights(arch, seed)` and
/// [`example_order`] with the same seed draw different numbers.
pub const ORDER_STREAM_SALT: u64 = 0xa5a5_5a5a_1234_5678;

/// How a training run orders the dataset's examples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExampleOrder {
    /// The dataset's canonical content-key order (Task 41 sorts it, so this
    /// is a function of content alone).
    Canonical,
    /// A Fisher-Yates permutation over the salted SplitMix64 stream. Still a
    /// pure function of `(seed, len)`; documented in [`example_order`].
    SeededPermutation,
}

/// A validated training configuration.
///
/// Construction is the validation: every field is checked finite and in
/// range, the step budget is checked against [`Self::MAX_STEPS`], and the
/// type has no other constructor, so holding a `TrainConfig` means the
/// optimizer's constants are usable.
///
/// A *step* consumes exactly one pairwise example and applies one update.
/// An *epoch* is one pass over the dataset in [`example_order`]; the ranker
/// reports [`TrainingOutcome::Completed`] when a whole epoch contributes no
/// hinge loss. There is no mini-batch policy: the CPU reference scores one
/// row per step and batching would be a throughput decision, not a numeric
/// one (Task 40 established that batching cannot change a row's bits).
#[derive(Clone, Debug, PartialEq)]
pub struct TrainConfig {
    learning_rate: f32,
    beta1: f32,
    beta2: f32,
    epsilon: f32,
    weight_decay: f32,
    margin: f32,
    max_steps: usize,
    seed: u64,
    order: ExampleOrder,
}

impl TrainConfig {
    /// Hard ceiling on a run's step budget.
    ///
    /// Matches [`DatasetConfig::MAX_EXAMPLES`]: one epoch over the largest
    /// admissible corpus. The budget bounds the run, so a caller asking for
    /// more has described a different computation, not a longer one.
    pub const MAX_STEPS: usize = DatasetConfig::MAX_EXAMPLES;

    /// Hard ceiling on examples in one run: the neural corpus cap.
    pub const MAX_TRAIN_EXAMPLES: usize = DatasetConfig::MAX_EXAMPLES;

    /// Validate a configuration.
    ///
    /// # Errors
    ///
    /// [`TrainingError::InvalidConfig`] naming the first unusable field
    /// (non-finite or out of range), and [`TrainingError::LimitExceeded`]
    /// when `max_steps` exceeds [`Self::MAX_STEPS`]. The ceilings are
    /// associated constants, so no configuration can raise them.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        learning_rate: f32,
        beta1: f32,
        beta2: f32,
        epsilon: f32,
        weight_decay: f32,
        margin: f32,
        max_steps: usize,
        seed: u64,
        order: ExampleOrder,
    ) -> Result<Self, TrainingError> {
        let config = Self {
            learning_rate,
            beta1,
            beta2,
            epsilon,
            weight_decay,
            margin,
            max_steps,
            seed,
            order,
        };
        config.validate()?;
        Ok(config)
    }

    /// Re-run every check. `new` calls this, and so does every consumer
    /// handed a configuration whose provenance is not `new`.
    ///
    /// # Errors
    ///
    /// As [`Self::new`].
    pub fn validate(&self) -> Result<(), TrainingError> {
        if !self.learning_rate.is_finite() || self.learning_rate <= 0.0 {
            return Err(invalid(
                "learning_rate",
                "must be finite and greater than zero",
            ));
        }
        if !self.beta1.is_finite() || !(0.0..1.0).contains(&self.beta1) {
            return Err(invalid("beta1", "must be finite and inside [0, 1)"));
        }
        if !self.beta2.is_finite() || !(0.0..1.0).contains(&self.beta2) {
            return Err(invalid("beta2", "must be finite and inside [0, 1)"));
        }
        if !self.epsilon.is_finite() || self.epsilon <= 0.0 {
            return Err(invalid("epsilon", "must be finite and greater than zero"));
        }
        if !self.weight_decay.is_finite() || self.weight_decay < 0.0 {
            return Err(invalid("weight_decay", "must be finite and non-negative"));
        }
        if !self.margin.is_finite() || self.margin <= 0.0 {
            return Err(invalid("margin", "must be finite and greater than zero"));
        }
        if self.max_steps == 0 || self.max_steps > Self::MAX_STEPS {
            return Err(TrainingError::LimitExceeded {
                resource: "training steps",
                value: self.max_steps,
                limit: Self::MAX_STEPS,
            });
        }
        Ok(())
    }

    /// A usable default: `lr = 1e-2`, `betas = (0.9, 0.999)`,
    /// `epsilon = 1e-8`, no weight decay, margin [`DEFAULT_MARGIN`], the
    /// full step budget, seed `0`, canonical order.
    ///
    /// The step budget is the ceiling rather than a small number because a
    /// default that silently truncates would make
    /// [`TrainingOutcome::Partial`] the common case.
    pub fn defaults() -> Self {
        Self {
            learning_rate: 1e-2,
            beta1: 0.9,
            beta2: 0.999,
            epsilon: 1e-8,
            weight_decay: 0.0,
            margin: DEFAULT_MARGIN,
            max_steps: Self::MAX_STEPS,
            seed: 0,
            order: ExampleOrder::Canonical,
        }
    }

    /// The AdamW step size.
    pub fn learning_rate(&self) -> f32 {
        self.learning_rate
    }

    /// First-moment decay.
    pub fn beta1(&self) -> f32 {
        self.beta1
    }

    /// Second-moment decay.
    pub fn beta2(&self) -> f32 {
        self.beta2
    }

    /// Denominator stabilizer.
    pub fn epsilon(&self) -> f32 {
        self.epsilon
    }

    /// Decoupled weight-decay coefficient.
    pub fn weight_decay(&self) -> f32 {
        self.weight_decay
    }

    /// Hinge margin.
    pub fn margin(&self) -> f32 {
        self.margin
    }

    /// Step budget.
    pub fn max_steps(&self) -> usize {
        self.max_steps
    }

    /// Run seed (example order; pass the same value to [`seeded_weights`] for
    /// a fully seeded run).
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Example-order policy.
    pub fn order(&self) -> ExampleOrder {
        self.order
    }
}

impl Default for TrainConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

/// Typed, sanitized training failures.
///
/// Every variant carries a name or an index, never a caller-supplied string
/// and never a backend error: the trainer computes in this module, so there
/// is no backend error to leak.
#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum TrainingError {
    /// A configuration field is non-finite or out of range.
    #[error("invalid training configuration: {field} {message}")]
    InvalidConfig {
        /// The rejected field.
        field: &'static str,
        /// The violated requirement.
        message: &'static str,
    },
    /// A fixed or caller-tightened ceiling rejected the input.
    #[error("training limit exceeded: {resource} (value {value}, ceiling {limit})")]
    LimitExceeded {
        /// The rejected resource.
        resource: &'static str,
        /// The offending value.
        value: usize,
        /// The applied ceiling.
        limit: usize,
    },
    /// A score, loss, gradient, moment, or parameter was not finite.
    ///
    /// `step` is the 0-based index of the step during which it was detected.
    #[error("training produced a non-finite value at step {step}")]
    NonFinite {
        /// The step index (0-based).
        step: usize,
    },
    /// An updated parameter left the architecture's admissible magnitude.
    ///
    /// Reachable only by a diverging configuration (a step size large enough
    /// to run the weights past [`RankerConfig::max_weight_magnitude`]): the
    /// resulting weights would be unrepresentable as a [`RankerWeights`].
    #[error("training left a parameter outside the architecture bound at step {step}")]
    ParameterOutOfBounds {
        /// The step index (0-based).
        step: usize,
    },
    /// The training corpus is empty.
    #[error("training corpus is empty")]
    EmptyDataset,
    /// The initial weights do not match the training architecture.
    #[error("initial weights do not match the training architecture")]
    MismatchedWeights,
}

fn invalid(field: &'static str, message: &'static str) -> TrainingError {
    TrainingError::InvalidConfig { field, message }
}

/// One pairwise training example: the features the ranker should prefer, and
/// the features it should not.
///
/// This is the trainer's input unit. [`Self::from_pairwise`] is the bridge
/// from Task 41's [`Dataset`]; [`Self::new`] exists so fixtures can build a
/// separable corpus without replaying traces (the two examples are just two
/// frozen feature vectors, which is all the loss ever looks at).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrainingExample {
    chosen: FeatureVector,
    unchosen: FeatureVector,
}

impl TrainingExample {
    /// Build an example from two already-validated feature vectors.
    pub fn new(chosen: FeatureVector, unchosen: FeatureVector) -> Self {
        Self { chosen, unchosen }
    }

    /// Borrow one dataset example's frozen features.
    pub fn from_pairwise(example: &PairwiseExample) -> Self {
        Self {
            chosen: *example.chosen(),
            unchosen: *example.unchosen(),
        }
    }

    /// The preferred candidate's features.
    pub fn chosen(&self) -> &FeatureVector {
        &self.chosen
    }

    /// The dispreferred candidate's features.
    pub fn unchosen(&self) -> &FeatureVector {
        &self.unchosen
    }
}

/// Convert a Task 41 dataset into the trainer's example list.
///
/// Cheap and total: [`FeatureVector`] is 64 `f32`s and `Copy`, and the
/// dataset's size is already bounded by [`DatasetConfig::MAX_EXAMPLES`], so
/// this copies at most 32 MiB of lanes.
pub fn corpus_from_dataset(dataset: &Dataset) -> Vec<TrainingExample> {
    dataset
        .examples()
        .iter()
        .map(TrainingExample::from_pairwise)
        .collect()
}

/// One parameter tensor's gradients, in the ranker's internal `[in, out]`
/// row-major layout.
///
/// The layout matches `RankerWeights`' internal `weight_t` buffers, so an
/// update is elementwise. (The *exported* optimizer state is transposed back
/// to Katakuri's canonical `[out, in]`; see
/// [`AdamWState::optimizer_state_entries`].)
#[derive(Clone, Debug, PartialEq)]
pub struct LayerGradients {
    weight_t: Vec<f32>,
    bias: Vec<f32>,
    in_width: usize,
    out_width: usize,
}

impl LayerGradients {
    /// `dLoss/dWeight` in `[in, out]` row-major order.
    pub fn weights(&self) -> &[f32] {
        &self.weight_t
    }

    /// `dLoss/dBias` in output order.
    pub fn biases(&self) -> &[f32] {
        &self.bias
    }

    /// The `(in, out)` widths this gradient describes.
    pub fn shape(&self) -> (usize, usize) {
        (self.in_width, self.out_width)
    }
}

/// The hand-derived gradient of the pairwise hinge over the whole ranker.
///
/// This is the [`crate::neural::DifferentiableRule::Gradient`] the ranker
/// advertises: per-layer weight and bias gradients, hidden layers in order
/// and the head last — exactly [`RankerConfig::expected_tensors`]' order with
/// the bias tensor after its weight tensor.
#[derive(Clone, Debug, PartialEq)]
pub struct RankerGradients {
    layers: Vec<LayerGradients>,
    head: LayerGradients,
}

impl RankerGradients {
    /// Hidden-layer gradients, in layer order.
    pub fn layers(&self) -> &[LayerGradients] {
        &self.layers
    }

    /// The head's gradient.
    pub fn head(&self) -> &LayerGradients {
        &self.head
    }

    /// Total gradient elements (equal to the parameter count).
    pub fn element_count(&self) -> usize {
        let layers: usize = self
            .layers
            .iter()
            .map(|layer| layer.weight_t.len() + layer.bias.len())
            .sum();
        layers + self.head.weight_t.len() + self.head.bias.len()
    }
}

/// One hinge evaluation: the loss and its gradient.
#[derive(Clone, Debug, PartialEq)]
pub struct HingeEvaluation {
    loss: f32,
    gradients: RankerGradients,
}

impl HingeEvaluation {
    /// `max(0, margin - (s_chosen - s_unchosen))`.
    pub fn loss(&self) -> f32 {
        self.loss
    }

    /// `dLoss/dParameters`.
    pub fn gradients(&self) -> &RankerGradients {
        &self.gradients
    }
}

/// Evaluate the pairwise hinge and its hand-derived gradient for one example.
///
/// The gradient is exact for the forward pass this module implements
/// (which is bit-identical to [`crate::neural::MlpRanker::score_features`]
/// on the same weights — asserted by the training tests).
///
/// # Errors
///
/// [`TrainingError::InvalidConfig`] when `margin` is not finite and
/// positive, [`TrainingError::MismatchedWeights`] when `weights` do not
/// match `arch`, and [`TrainingError::NonFinite`] (step `0`) when the
/// forward or backward pass overflows. A satisfied example (loss exactly
/// zero) returns a zero gradient: the hinge's subgradient is zero there, and
/// the trainer treats it as a no-op.
pub fn pairwise_hinge(
    arch: &RankerConfig,
    weights: &RankerWeights,
    margin: f32,
    example: &TrainingExample,
) -> Result<HingeEvaluation, TrainingError> {
    if !margin.is_finite() || margin <= 0.0 {
        return Err(invalid("margin", "must be finite and greater than zero"));
    }
    let parameters = parameters_from(weights, arch)?;
    let mut chosen_cache = Cache::new(&parameters);
    let mut unchosen_cache = Cache::new(&parameters);
    let mut gradients = parameters.gradient_buffers();
    let mut buffers = BackwardBuffers::new(&parameters);

    let chosen = forward(&parameters, example.chosen().lanes(), &mut chosen_cache);
    let unchosen = forward(&parameters, example.unchosen().lanes(), &mut unchosen_cache);
    for score in [chosen, unchosen] {
        if !score.is_finite() {
            return Err(TrainingError::NonFinite { step: 0 });
        }
    }
    let difference = chosen - unchosen;
    if !difference.is_finite() {
        return Err(TrainingError::NonFinite { step: 0 });
    }
    if difference >= margin {
        // A satisfied example has a zero subgradient: the evaluation is a
        // zero loss paired with a zero gradient, exactly what the trainer
        // treats as a no-op.
        return Ok(HingeEvaluation {
            loss: 0.0,
            gradients: gradients.into_gradients(),
        });
    }
    let loss = margin - difference;
    // Accumulation order: the chosen side (negated) first, then the
    // unchosen side, matching the trainer's per-step order.
    backward(
        &parameters,
        &chosen_cache,
        -1.0,
        &mut gradients,
        &mut buffers,
    );
    backward(
        &parameters,
        &unchosen_cache,
        1.0,
        &mut gradients,
        &mut buffers,
    );
    for value in gradients.elements() {
        if !value.is_finite() {
            return Err(TrainingError::NonFinite { step: 0 });
        }
    }
    Ok(HingeEvaluation {
        loss,
        gradients: gradients.into_gradients(),
    })
}

/// A typed training outcome.
///
/// [`Self::Completed`] is convergence evidence; [`Self::Partial`] is a
/// budget-truncated snapshot and is explicitly **not replayable**
/// ([`Self::is_replayable`] is `false`).
#[derive(Clone, Debug, PartialEq)]
pub enum TrainingOutcome {
    /// A full epoch contributed no hinge loss: the run converged.
    Completed {
        /// Steps executed (one per example visit, including no-op steps).
        steps_run: usize,
        /// The converged parameters.
        parameters: RankerWeights,
        /// The optimizer state at the stopping point.
        optimizer: AdamWState,
    },
    /// The step budget ran out before convergence.
    ///
    /// **Non-replayable**: this outcome carries no convergence evidence, so
    /// its parameters must not be treated as a trained ranker. The run is
    /// still reproducible (the budget is part of the configuration), but
    /// reproducibility is not the claim a `Partial` result makes.
    Partial {
        /// Why the run stopped.
        reason: PartialReason,
        /// Steps executed.
        steps_run: usize,
        /// The snapshot's parameters.
        parameters: RankerWeights,
        /// The snapshot's optimizer state.
        optimizer: AdamWState,
    },
}

impl TrainingOutcome {
    /// Steps executed.
    pub fn steps_run(&self) -> usize {
        match self {
            Self::Completed { steps_run, .. } | Self::Partial { steps_run, .. } => *steps_run,
        }
    }

    /// The parameters at the stopping point.
    pub fn parameters(&self) -> &RankerWeights {
        match self {
            Self::Completed { parameters, .. } | Self::Partial { parameters, .. } => parameters,
        }
    }

    /// The optimizer state at the stopping point.
    pub fn optimizer_state(&self) -> &AdamWState {
        match self {
            Self::Completed { optimizer, .. } | Self::Partial { optimizer, .. } => optimizer,
        }
    }

    /// Whether the run converged.
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Completed { .. })
    }

    /// Whether this outcome is a replayable training result.
    ///
    /// Only [`Self::Completed`] is: a `Partial` outcome was truncated by its
    /// step budget and certifies nothing, so it is marked non-replayable
    /// here rather than left to a caller's convention.
    pub fn is_replayable(&self) -> bool {
        self.is_complete()
    }

    /// The partial reason, if the run was truncated.
    pub fn partial_reason(&self) -> Option<PartialReason> {
        match self {
            Self::Completed { .. } => None,
            Self::Partial { reason, .. } => Some(*reason),
        }
    }
}

/// Why a run stopped short of convergence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartialReason {
    /// [`TrainConfig::max_steps`] was reached.
    StepBudgetExhausted,
}

/// One exported optimizer-state tensor.
#[derive(Clone, Debug, PartialEq)]
pub struct OptimizerStateEntry {
    name: String,
    values: Vec<f32>,
}

impl OptimizerStateEntry {
    /// The provisional tensor name (`<param>.adamw.m` / `<param>.adamw.v`).
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The tensor's values, in the canonical `[out, in]` row-major layout
    /// (or `[out]` for a bias).
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

/// One layer's AdamW moment buffers.
#[derive(Clone, Debug, PartialEq)]
struct MomentBuffers {
    weight_m: Vec<f32>,
    weight_v: Vec<f32>,
    bias_m: Vec<f32>,
    bias_v: Vec<f32>,
}

impl MomentBuffers {
    fn zeros(weights: usize, biases: usize) -> Self {
        Self {
            weight_m: vec![0.0f32; weights],
            weight_v: vec![0.0f32; weights],
            bias_m: vec![0.0f32; biases],
            bias_v: vec![0.0f32; biases],
        }
    }
}

/// `(in_width, out_width)` of one parameter layer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct LayerShape {
    in_width: usize,
    out_width: usize,
}

/// The AdamW optimizer state: first and second moments per parameter, the
/// step counter, and the bias-correction accumulators.
///
/// The bias corrections are *accumulated* (`beta^t` by repeated
/// multiplication) rather than computed with `powf`: the exponent is the only
/// transcendental the update would otherwise need, and this module does not
/// use any (see the module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct AdamWState {
    moments: Vec<MomentBuffers>,
    shapes: Vec<LayerShape>,
    step: usize,
    beta1_power: f32,
    beta2_power: f32,
}

impl AdamWState {
    /// Zeroed state for an architecture: every moment starts at `0.0`, the
    /// step counter at `0`.
    fn zeros(arch: &RankerConfig) -> Self {
        let shapes = layer_shapes(arch);
        let moments = shapes
            .iter()
            .map(|shape| MomentBuffers::zeros(shape.in_width * shape.out_width, shape.out_width))
            .collect();
        Self {
            moments,
            shapes,
            step: 0,
            beta1_power: 1.0,
            beta2_power: 1.0,
        }
    }

    /// Steps completed (each successful update is one step; a no-op example
    /// that already satisfied the margin does not advance the optimizer, and
    /// therefore does not advance this counter).
    pub fn step(&self) -> usize {
        self.step
    }

    /// Total moment elements tracked (`2 x` the parameter count).
    pub fn tracked_elements(&self) -> usize {
        self.moments
            .iter()
            .map(|moment| {
                moment.weight_m.len()
                    + moment.weight_v.len()
                    + moment.bias_m.len()
                    + moment.bias_v.len()
            })
            .sum()
    }

    /// Export the state as named tensors.
    ///
    /// This is the **single** place the state's naming lives (module
    /// documentation: provisional S6 convention). For each parameter it emits
    /// `<param>.adamw.m` (first moment) and `<param>.adamw.v` (second
    /// moment), in [`RankerConfig::expected_tensors`] order, with the values
    /// transposed back to Katakuri's canonical `[out, in]` row-major layout
    /// so they line up with a checkpoint's parameter tensors (Task 43).
    ///
    /// The transpose is exact (a permutation, no arithmetic) and happens once
    /// per export, never per step.
    pub fn optimizer_state_entries(&self) -> Vec<OptimizerStateEntry> {
        let depth = self.shapes.len();
        let mut entries = Vec::with_capacity(depth * 4);
        for (index, (moment, shape)) in self.moments.iter().zip(&self.shapes).enumerate() {
            // The head is the final layer of the canonical order.
            let base = if index + 1 == depth {
                "head".to_string()
            } else {
                format!("hidden.{index}")
            };
            entries.push(OptimizerStateEntry {
                name: format!("{base}.weight{ADAMW_MOMENT_SUFFIX}"),
                values: transpose(&moment.weight_m, shape.in_width, shape.out_width),
            });
            entries.push(OptimizerStateEntry {
                name: format!("{base}.weight{ADAMW_VELOCITY_SUFFIX}"),
                values: transpose(&moment.weight_v, shape.in_width, shape.out_width),
            });
            entries.push(OptimizerStateEntry {
                name: format!("{base}.bias{ADAMW_MOMENT_SUFFIX}"),
                values: moment.bias_m.clone(),
            });
            entries.push(OptimizerStateEntry {
                name: format!("{base}.bias{ADAMW_VELOCITY_SUFFIX}"),
                values: moment.bias_v.clone(),
            });
        }
        entries
    }
}

/// SplitMix64: the documented initialization and permutation stream.
///
/// Integer-only, so two platforms cannot disagree about a draw. The
/// constants are SplitMix64's canonical ones (`0x9E3779B97F4A7C15` increment,
/// `0xBF58476D1CE4E5B9` and `0x94D049BB133111EB` mixers, shifts 30/27/31).
#[derive(Clone, Copy, Debug)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    const GOLDEN_GAMMA: u64 = 0x9E37_79B9_7F4A_7C15;
    const MIX_1: u64 = 0xBF58_476D_1CE4_E5B9;
    const MIX_2: u64 = 0x94D0_49BB_1331_11EB;

    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(Self::GOLDEN_GAMMA);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(Self::MIX_1);
        z = (z ^ (z >> 27)).wrapping_mul(Self::MIX_2);
        z ^ (z >> 31)
    }

    /// A uniform fraction in `[0, 1)` built from the top 24 bits of one
    /// draw.
    ///
    /// 24 bits is f32's significand width, so the mapping
    /// `(next_u64() >> 40) as f32 / 2^24` is exact: every draw lands on a
    /// representable f32 with no rounding, and the low bits (which a wider
    /// cast would have to round) are discarded rather than rounded.
    fn next_fraction(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 * (1.0 / 16_777_216.0)
    }
}

/// Seeded parameter initialization.
///
/// The exact seed-to-weights mapping, which is part of the reproducibility
/// contract:
///
/// 1. A SplitMix64 stream is opened on `seed` (the canonical mixer:
///    increment `0x9E3779B97F4A7C15`, mixers `0xBF58476D1CE4E5B9` and
///    `0x94D049BB133111EB`, shifts 30/27/31).
/// 2. Parameters are visited in Katakuri's canonical tensor order
///    ([`RankerConfig::expected_tensors`]: `hidden.0.weight`,
///    `hidden.0.bias`, ..., `head.weight`, `head.bias`), row-major within
///    each tensor — weights in their canonical `[out, in]` layout.
/// 3. Each element consumes exactly one draw `u = next_u64() >> 40`, becomes
///    the fraction `f = u as f32 / 2^24` in `[0, 1)`, and is set to
///    `(f - 0.5) * (2 * INIT_SCALE)`, i.e. a uniform value in
///    `[-INIT_SCALE, INIT_SCALE)`.
///
/// Biases are drawn from the same stream (rather than left at zero) so no
/// ReLU unit starts dead, which would make training depend on luck rather
/// than on the seed.
///
/// # Errors
///
/// [`TrainingError::MismatchedWeights`] if the architecture's tensor shapes
/// and the generated buffers disagree (unreachable for a validated spec;
/// `INIT_SCALE` is far inside every bound, so no generated value can be
/// rejected for magnitude).
pub fn seeded_weights(arch: &RankerConfig, seed: u64) -> Result<RankerWeights, TrainingError> {
    let mut stream = SplitMix64::new(seed);
    let expected = arch.expected_tensors();
    let mut tensors: Vec<(String, Vec<f32>)> = Vec::with_capacity(expected.len());
    for (name, shape) in expected {
        let elements: usize = shape.iter().product();
        let mut values = Vec::with_capacity(elements);
        for _ in 0..elements {
            let fraction = stream.next_fraction();
            values.push((fraction - 0.5) * (2.0 * INIT_SCALE));
        }
        tensors.push((name, values));
    }
    weights_from_tensors(arch, tensors)
}

/// The example visit order for a run.
///
/// [`ExampleOrder::Canonical`] is the identity permutation (the dataset is
/// already in canonical content-key order). [`ExampleOrder::SeededPermutation`]
/// is a descending Fisher-Yates shuffle over a SplitMix64 stream opened on
/// `seed ^ ORDER_STREAM_SALT`: for `i` from `len - 1` down to `1`, swap slot
/// `i` with slot `next_u64() % (i + 1)`.
///
/// The modulo is deliberately the documented rule (not rejection sampling):
/// the resulting bias is a property of the draw distribution, and the
/// permutation's *reproducibility* — which is the contract — does not depend
/// on the bias being zero. Changing either the salt or the rule changes every
/// seeded order, so both are pinned here.
///
/// Allocates one `len`-element vector; call it on an admitted example count.
pub fn example_order(config: &TrainConfig, len: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..len).collect();
    if config.order == ExampleOrder::SeededPermutation {
        let mut stream = SplitMix64::new(config.seed ^ ORDER_STREAM_SALT);
        for index in (1..len).rev() {
            let bound = index as u64 + 1;
            let swap = (stream.next_u64() % bound) as usize;
            order.swap(index, swap);
        }
    }
    order
}

/// Train the ranker from given initial weights.
///
/// One step consumes one example; an epoch is one pass in
/// [`example_order`]. The run stops with [`TrainingOutcome::Completed`] when
/// an entire epoch contributes zero hinge loss, and with
/// [`TrainingOutcome::Partial`] when [`TrainConfig::max_steps`] runs out
/// first.
///
/// # Errors
///
/// [`TrainingError::InvalidConfig`] for an unusable configuration,
/// [`TrainingError::LimitExceeded`] when the step budget or example count
/// exceeds its ceiling (checked before any allocation),
/// [`TrainingError::EmptyDataset`] for no examples,
/// [`TrainingError::MismatchedWeights`] when `init` was built for a
/// different architecture, [`TrainingError::NonFinite`] with the step index
/// when any score, loss, gradient, moment, or parameter stops being finite,
/// and [`TrainingError::ParameterOutOfBounds`] when an update runs a
/// parameter past the architecture's magnitude bound.
pub fn train(
    arch: &RankerConfig,
    init: &RankerWeights,
    config: &TrainConfig,
    examples: &[TrainingExample],
) -> Result<TrainingOutcome, TrainingError> {
    // Admission first: scalars only, no allocation.
    config.validate()?;
    if examples.is_empty() {
        return Err(TrainingError::EmptyDataset);
    }
    if examples.len() > TrainConfig::MAX_TRAIN_EXAMPLES {
        return Err(TrainingError::LimitExceeded {
            resource: "training examples",
            value: examples.len(),
            limit: TrainConfig::MAX_TRAIN_EXAMPLES,
        });
    }
    // Parameter counts are bounded by the architecture spec, which
    // `RankerConfig::from_spec` already validated, and `init`'s shapes are
    // checked against it here before any buffer is sized.
    let mut parameters = parameters_from(init, arch)?;
    let bound = arch.max_weight_magnitude();
    let order = example_order(config, examples.len());
    let mut state = AdamWState::zeros(arch);
    let mut workspace = Workspace::new(&parameters);

    let mut steps = 0usize;
    loop {
        let mut converged = true;
        for &index in &order {
            if steps >= config.max_steps {
                return finish(
                    arch,
                    parameters,
                    state,
                    steps,
                    Some(PartialReason::StepBudgetExhausted),
                );
            }
            let loss = run_step(
                &mut parameters,
                &mut state,
                &mut workspace,
                config,
                &examples[index],
                steps,
                bound,
            )?;
            if loss > 0.0 {
                converged = false;
            }
            steps += 1;
        }
        if converged {
            return finish(arch, parameters, state, steps, None);
        }
    }
}

/// Train from a seeded initialization.
///
/// Equivalent to `train(arch, &seeded_weights(arch, config.seed())?, config,
/// examples)`; the convenience exists so a fully seeded run never has two
/// seeds to keep in sync.
///
/// # Errors
///
/// As [`train`] and [`seeded_weights`].
pub fn train_from_seed(
    arch: &RankerConfig,
    config: &TrainConfig,
    examples: &[TrainingExample],
) -> Result<TrainingOutcome, TrainingError> {
    let init = seeded_weights(arch, config.seed())?;
    train(arch, &init, config, examples)
}

// ── internals ───────────────────────────────────────────────────────

/// One layer's trainable parameters, in the internal `[in, out]` layout.
#[derive(Clone, Debug, PartialEq)]
struct LayerParameters {
    weight_t: Vec<f32>,
    bias: Vec<f32>,
    in_width: usize,
    out_width: usize,
}

/// The whole ranker's trainable parameters (and, as gradient buffers, the
/// negative space for them).
#[derive(Clone, Debug, PartialEq)]
struct Parameters {
    layers: Vec<LayerParameters>,
    head: LayerParameters,
}

impl Parameters {
    fn zeroed_like(&self) -> Self {
        let layers = self
            .layers
            .iter()
            .map(|layer| LayerParameters {
                weight_t: vec![0.0f32; layer.weight_t.len()],
                bias: vec![0.0f32; layer.bias.len()],
                in_width: layer.in_width,
                out_width: layer.out_width,
            })
            .collect();
        Self {
            layers,
            head: LayerParameters {
                weight_t: vec![0.0f32; self.head.weight_t.len()],
                bias: vec![0.0f32; self.head.bias.len()],
                in_width: self.head.in_width,
                out_width: self.head.out_width,
            },
        }
    }

    /// Gradient buffers with the same shapes (already callable on the
    /// gradient set; kept for symmetry with `zeroed_like`).
    fn gradient_buffers(&self) -> Self {
        self.zeroed_like()
    }

    fn zero(&mut self) {
        for layer in &mut self.layers {
            layer.weight_t.fill(0.0);
            layer.bias.fill(0.0);
        }
        self.head.weight_t.fill(0.0);
        self.head.bias.fill(0.0);
    }

    /// Every element, in the canonical update order (layers, weight then
    /// bias; head last).
    fn elements(&self) -> impl Iterator<Item = &f32> + '_ {
        self.layers
            .iter()
            .flat_map(|layer| layer.weight_t.iter().chain(layer.bias.iter()))
            .chain(self.head.weight_t.iter().chain(self.head.bias.iter()))
    }

    /// Convert to the public gradient type.
    fn into_gradients(self) -> RankerGradients {
        let layers = self
            .layers
            .into_iter()
            .map(|layer| LayerGradients {
                weight_t: layer.weight_t,
                bias: layer.bias,
                in_width: layer.in_width,
                out_width: layer.out_width,
            })
            .collect();
        RankerGradients {
            layers,
            head: LayerGradients {
                weight_t: self.head.weight_t,
                bias: self.head.bias,
                in_width: self.head.in_width,
                out_width: self.head.out_width,
            },
        }
    }

    /// Convert to Katakuri's canonical `[out, in]` tensors: the inverse of
    /// the transpose `RankerWeights::from_tensors` applies at construction.
    fn canonical_tensors(
        self,
        arch: &RankerConfig,
    ) -> Result<Vec<(String, Vec<f32>)>, TrainingError> {
        let names = arch.expected_tensors();
        let mut values: Vec<Vec<f32>> = Vec::with_capacity(names.len());
        for layer in &self.layers {
            values.push(transpose(&layer.weight_t, layer.in_width, layer.out_width));
            values.push(layer.bias.clone());
        }
        values.push(transpose(
            &self.head.weight_t,
            self.head.in_width,
            self.head.out_width,
        ));
        values.push(self.head.bias.clone());
        if values.len() != names.len() {
            return Err(TrainingError::MismatchedWeights);
        }
        let mut tensors = Vec::with_capacity(names.len());
        for ((name, shape), values) in names.into_iter().zip(values) {
            let elements: usize = shape.iter().product();
            if elements != values.len() {
                return Err(TrainingError::MismatchedWeights);
            }
            tensors.push((name, values));
        }
        Ok(tensors)
    }
}

/// Forward cache: the input activations of every layer and the hidden
/// pre-activations (needed for the ReLU derivative).
struct Cache {
    /// `activations[0]` is the feature row; `activations[i + 1]` is the
    /// ReLU output of hidden layer `i`.
    activations: Vec<Vec<f32>>,
    /// Hidden layer `i`'s pre-activation (`z = x W + b`).
    pre_activations: Vec<Vec<f32>>,
}

impl Cache {
    fn new(parameters: &Parameters) -> Self {
        let mut activations = Vec::with_capacity(parameters.layers.len() + 1);
        activations.push(vec![0.0f32; CANDIDATE_FEATURE_WIDTH]);
        for layer in &parameters.layers {
            activations.push(vec![0.0f32; layer.out_width]);
        }
        let pre_activations = parameters
            .layers
            .iter()
            .map(|layer| vec![0.0f32; layer.out_width])
            .collect();
        Self {
            activations,
            pre_activations,
        }
    }
}

/// Backward scratch: `dpre[i]` is the ReLU-masked delta of hidden layer `i`,
/// `dinput[i]` the delta carried into layer `i`'s input (`dinput[0]` is
/// unused but sized, so indices line up with the cache).
struct BackwardBuffers {
    dpre: Vec<Vec<f32>>,
    dinput: Vec<Vec<f32>>,
}

impl BackwardBuffers {
    fn new(parameters: &Parameters) -> Self {
        let mut dinput = Vec::with_capacity(parameters.layers.len() + 1);
        dinput.push(vec![0.0f32; CANDIDATE_FEATURE_WIDTH]);
        for layer in &parameters.layers {
            dinput.push(vec![0.0f32; layer.out_width]);
        }
        let dpre = parameters
            .layers
            .iter()
            .map(|layer| vec![0.0f32; layer.out_width])
            .collect();
        Self { dpre, dinput }
    }
}

/// Per-run scratch: two forward caches (chosen/unchosen), the gradient
/// buffers, and the backward scratch. Allocated once, overwritten in place.
struct Workspace {
    chosen: Cache,
    unchosen: Cache,
    gradients: Parameters,
    backward: BackwardBuffers,
}

impl Workspace {
    fn new(parameters: &Parameters) -> Self {
        Self {
            chosen: Cache::new(parameters),
            unchosen: Cache::new(parameters),
            gradients: parameters.gradient_buffers(),
            backward: BackwardBuffers::new(parameters),
        }
    }
}

/// The `(in, out)` widths of every layer, head last.
fn layer_shapes(arch: &RankerConfig) -> Vec<LayerShape> {
    let mut shapes = Vec::with_capacity(arch.spec().hidden.len() + 1);
    let mut in_width = arch.spec().input_width as usize;
    for layer in &arch.spec().hidden {
        let out_width = layer.width as usize;
        shapes.push(LayerShape {
            in_width,
            out_width,
        });
        in_width = out_width;
    }
    shapes.push(LayerShape {
        in_width,
        out_width: arch.spec().head.width as usize,
    });
    shapes
}

/// Copy a validated weight set into the trainer's buffers, checking the
/// layer shapes against the architecture.
fn parameters_from(init: &RankerWeights, arch: &RankerConfig) -> Result<Parameters, TrainingError> {
    let shapes = layer_shapes(arch);
    let borrowed = init.training_layers();
    if borrowed.len() != shapes.len() {
        return Err(TrainingError::MismatchedWeights);
    }
    let mut layers = Vec::with_capacity(shapes.len());
    for ((weight_t, bias, in_width, out_width), shape) in borrowed.into_iter().zip(&shapes) {
        if in_width != shape.in_width
            || out_width != shape.out_width
            || weight_t.len() != in_width * out_width
            || bias.len() != out_width
        {
            return Err(TrainingError::MismatchedWeights);
        }
        layers.push(LayerParameters {
            weight_t: weight_t.to_vec(),
            bias: bias.to_vec(),
            in_width,
            out_width,
        });
    }
    let head = layers.pop().ok_or(TrainingError::MismatchedWeights)?;
    Ok(Parameters { layers, head })
}

/// The forward pass for one feature row, filling `cache`.
///
/// The accumulation is borsalino-tensor's CPU `matmul` order exactly: one
/// `f32` accumulator per output element, contracted index ascending, then
/// the bias added. ReLU is the same comparison-and-select the ranker uses, so
/// this function and [`crate::neural::MlpRanker::score_features`] return the
/// same bits for the same weights.
fn forward(
    parameters: &Parameters,
    features: &[f32; CANDIDATE_FEATURE_WIDTH],
    cache: &mut Cache,
) -> f32 {
    cache.activations[0].copy_from_slice(features);
    for (index, layer) in parameters.layers.iter().enumerate() {
        let input = &cache.activations[index];
        let pre = &mut cache.pre_activations[index];
        for (output, pre_activation) in pre.iter_mut().enumerate() {
            let mut accumulation = 0.0f32;
            for (contracted, value) in input.iter().enumerate() {
                accumulation += value * layer.weight_t[contracted * layer.out_width + output];
            }
            *pre_activation = accumulation + layer.bias[output];
        }
        let activations = &mut cache.activations[index + 1];
        for (output, activation) in activations.iter_mut().enumerate() {
            let value = pre[output];
            *activation = if value > 0.0 { value } else { 0.0 };
        }
    }
    let input = &cache.activations[parameters.layers.len()];
    let head = &parameters.head;
    let mut accumulation = 0.0f32;
    for (contracted, value) in input.iter().enumerate() {
        accumulation += value * head.weight_t[contracted * head.out_width];
    }
    accumulation + head.bias[0]
}

/// Accumulate `dscore * dScore/dParameters` into `gradients`.
///
/// The contraction order is fixed and documented: for each layer (head
/// first, then hidden layers in reverse), each input index ascending, each
/// output index ascending. `dinput` (the delta carried to the previous layer)
/// accumulates over outputs ascending — the mirror of the forward pass's
/// ascending contraction over inputs, which is the only order that needs no
/// platform-specific reduction shape.
fn backward(
    parameters: &Parameters,
    cache: &Cache,
    dscore: f32,
    gradients: &mut Parameters,
    buffers: &mut BackwardBuffers,
) {
    let head = &parameters.head;
    let head_gradient = &mut gradients.head;
    let depth = parameters.layers.len();
    let input = &cache.activations[depth];
    head_gradient.bias[0] += dscore;
    let dinput = &mut buffers.dinput[depth];
    for (contracted, value) in input.iter().enumerate() {
        head_gradient.weight_t[contracted * head.out_width] += dscore * value;
        dinput[contracted] = dscore * head.weight_t[contracted * head.out_width];
    }

    for index in (0..depth).rev() {
        let layer = &parameters.layers[index];
        let pre = &cache.pre_activations[index];
        let dpre = &mut buffers.dpre[index];
        let upstream = &buffers.dinput[index + 1];
        for (output, delta) in dpre.iter_mut().enumerate() {
            *delta = if pre[output] > 0.0 {
                upstream[output]
            } else {
                0.0
            };
        }
        let gradient = &mut gradients.layers[index];
        let input = &cache.activations[index];
        let dinput = &mut buffers.dinput[index];
        for (contracted, value) in input.iter().enumerate() {
            let mut accumulation = 0.0f32;
            for (output, delta) in dpre.iter().enumerate() {
                let weight = layer.weight_t[contracted * layer.out_width + output];
                gradient.weight_t[contracted * layer.out_width + output] += value * delta;
                accumulation += weight * delta;
            }
            dinput[contracted] = accumulation;
        }
        for (output, delta) in dpre.iter().enumerate() {
            gradient.bias[output] += delta;
        }
    }
}

/// One training step: score both sides, and (when the hinge is active)
/// backprop and apply one AdamW update.
fn run_step(
    parameters: &mut Parameters,
    state: &mut AdamWState,
    workspace: &mut Workspace,
    config: &TrainConfig,
    example: &TrainingExample,
    step: usize,
    bound: f32,
) -> Result<f32, TrainingError> {
    let chosen = forward(parameters, example.chosen().lanes(), &mut workspace.chosen);
    if !chosen.is_finite() {
        return Err(TrainingError::NonFinite { step });
    }
    let unchosen = forward(
        parameters,
        example.unchosen().lanes(),
        &mut workspace.unchosen,
    );
    if !unchosen.is_finite() {
        return Err(TrainingError::NonFinite { step });
    }
    let difference = chosen - unchosen;
    if !difference.is_finite() {
        return Err(TrainingError::NonFinite { step });
    }
    if difference >= config.margin {
        // Satisfied: the hinge's gradient is zero, so the step is a no-op
        // and the optimizer does not advance.
        return Ok(0.0);
    }
    let loss = config.margin - difference;
    if !loss.is_finite() {
        return Err(TrainingError::NonFinite { step });
    }

    workspace.gradients.zero();
    backward(
        parameters,
        &workspace.chosen,
        -1.0,
        &mut workspace.gradients,
        &mut workspace.backward,
    );
    backward(
        parameters,
        &workspace.unchosen,
        1.0,
        &mut workspace.gradients,
        &mut workspace.backward,
    );
    if workspace
        .gradients
        .elements()
        .any(|value| !value.is_finite())
    {
        return Err(TrainingError::NonFinite { step });
    }

    apply_adamw(parameters, &workspace.gradients, state, config, step, bound)?;
    Ok(loss)
}

/// One AdamW update, in the canonical parameter order.
///
/// PyTorch's parameterization: decay the parameter first
/// (`p = p * (1 - lr * wd)`), then subtract the bias-corrected moment step.
/// Bias correction divides by the accumulated `1 - beta^t`; no `powf`.
fn apply_adamw(
    parameters: &mut Parameters,
    gradients: &Parameters,
    state: &mut AdamWState,
    config: &TrainConfig,
    step: usize,
    bound: f32,
) -> Result<(), TrainingError> {
    state.step += 1;
    state.beta1_power *= config.beta1;
    state.beta2_power *= config.beta2;
    let correction1 = 1.0 - state.beta1_power;
    let correction2 = 1.0 - state.beta2_power;
    if !correction1.is_finite()
        || correction1 <= 0.0
        || !correction2.is_finite()
        || correction2 <= 0.0
    {
        return Err(TrainingError::NonFinite { step });
    }
    let decay = 1.0 - config.learning_rate * config.weight_decay;

    let depth = parameters.layers.len();
    for index in 0..depth {
        let layer = &mut parameters.layers[index];
        let gradient = &gradients.layers[index];
        let moment = &mut state.moments[index];
        adamw_elementwise(
            &mut layer.weight_t,
            &gradient.weight_t,
            &mut moment.weight_m,
            &mut moment.weight_v,
            config,
            correction1,
            correction2,
            decay,
            step,
            bound,
        )?;
        adamw_elementwise(
            &mut layer.bias,
            &gradient.bias,
            &mut moment.bias_m,
            &mut moment.bias_v,
            config,
            correction1,
            correction2,
            decay,
            step,
            bound,
        )?;
    }
    let head = &mut parameters.head;
    let gradient = &gradients.head;
    let moment = &mut state.moments[depth];
    adamw_elementwise(
        &mut head.weight_t,
        &gradient.weight_t,
        &mut moment.weight_m,
        &mut moment.weight_v,
        config,
        correction1,
        correction2,
        decay,
        step,
        bound,
    )?;
    adamw_elementwise(
        &mut head.bias,
        &gradient.bias,
        &mut moment.bias_m,
        &mut moment.bias_v,
        config,
        correction1,
        correction2,
        decay,
        step,
        bound,
    )
}

#[allow(clippy::too_many_arguments)]
fn adamw_elementwise(
    parameters: &mut [f32],
    gradients: &[f32],
    first: &mut [f32],
    second: &mut [f32],
    config: &TrainConfig,
    correction1: f32,
    correction2: f32,
    decay: f32,
    step: usize,
    bound: f32,
) -> Result<(), TrainingError> {
    for (index, parameter) in parameters.iter_mut().enumerate() {
        let gradient = gradients[index];
        let moment = config.beta1 * first[index] + (1.0 - config.beta1) * gradient;
        let velocity = config.beta2 * second[index] + (1.0 - config.beta2) * gradient * gradient;
        if !moment.is_finite() || !velocity.is_finite() {
            return Err(TrainingError::NonFinite { step });
        }
        first[index] = moment;
        second[index] = velocity;
        let corrected_moment = moment / correction1;
        let corrected_velocity = velocity / correction2;
        let denominator = corrected_velocity.sqrt() + config.epsilon;
        if !corrected_moment.is_finite() || !denominator.is_finite() || denominator <= 0.0 {
            return Err(TrainingError::NonFinite { step });
        }
        let updated = *parameter * decay - config.learning_rate * (corrected_moment / denominator);
        if !updated.is_finite() {
            return Err(TrainingError::NonFinite { step });
        }
        if updated.abs() > bound {
            return Err(TrainingError::ParameterOutOfBounds { step });
        }
        *parameter = updated;
    }
    Ok(())
}

/// Build the outcome, converting the trainer's buffers to canonical tensors
/// once.
fn finish(
    arch: &RankerConfig,
    parameters: Parameters,
    optimizer: AdamWState,
    steps_run: usize,
    partial: Option<PartialReason>,
) -> Result<TrainingOutcome, TrainingError> {
    let tensors = parameters.canonical_tensors(arch)?;
    let parameters = weights_from_tensors(arch, tensors)?;
    Ok(match partial {
        None => TrainingOutcome::Completed {
            steps_run,
            parameters,
            optimizer,
        },
        Some(reason) => TrainingOutcome::Partial {
            reason,
            steps_run,
            parameters,
            optimizer,
        },
    })
}

/// Validate and build a weight set from owned canonical tensors.
fn weights_from_tensors(
    arch: &RankerConfig,
    tensors: Vec<(String, Vec<f32>)>,
) -> Result<RankerWeights, TrainingError> {
    let borrowed: Vec<(&str, &[f32])> = tensors
        .iter()
        .map(|(name, values)| (name.as_str(), values.as_slice()))
        .collect();
    // Sanitized: the trainer and the generator both check finiteness and
    // magnitude per step, so a rejection here means the tensors and the
    // architecture disagreed, and no backend detail is worth propagating.
    RankerWeights::from_tensors(arch, &borrowed).map_err(|_| TrainingError::MismatchedWeights)
}

/// Reinterpret `[in, out]` row-major as `[out, in]` row-major. Exact: a
/// permutation of elements, never an arithmetic step.
fn transpose(values: &[f32], in_width: usize, out_width: usize) -> Vec<f32> {
    let mut transposed = vec![0.0f32; values.len()];
    for input in 0..in_width {
        for output in 0..out_width {
            transposed[output * in_width + input] = values[input * out_width + output];
        }
    }
    transposed
}
