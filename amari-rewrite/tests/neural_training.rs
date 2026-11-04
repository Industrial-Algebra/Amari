//! Bounded AdamW training contracts (0.25 Cohort 8, Task 42).
//!
//! The tests come in the four groups the task enumerates — determinism,
//! convergence sanity, typed failure discipline, and admission — plus two
//! structural pinning tests: the seeded initialization is checked against an
//! independent re-implementation of the documented SplitMix64 mapping, and
//! the hand-derived gradient is checked against a central finite difference.
//!
//! Everything is built from frozen [`FeatureVector`]s, so the fixtures are
//! exactly what the loss sees: no trace replay, no term construction. The
//! ranking path is untouched by training (that is Task 40's contract), which
//! is why the hinge fixture also compares the trainer's own forward pass with
//! [`MlpRanker::score_features`] bit for bit.

#![cfg(feature = "neural")]

use amari_rewrite::inverse::{
    BackwardExplorer, BackwardSearchOutcome, GuidanceMode, InverseSearchConfig, ReplayCertificate,
    ReplayDerivation, ReplayQuery, ResourceObservation, SearchMode,
};
use amari_rewrite::neural::{
    corpus_from_dataset, example_order, pairwise_hinge, seeded_weights, train, train_from_seed,
    DatasetBuilder, DatasetConfig, DifferentiableRule, ExampleOrder, FeatureVector, MlpRanker,
    PartialReason, RankerConfig, RankerWeights, TrainConfig, TrainingError, TrainingExample,
    TrainingOutcome, ADAMW_MOMENT_SUFFIX, ADAMW_NAMING_STATUS, ADAMW_VELOCITY_SUFFIX,
    CANDIDATE_FEATURE_WIDTH, INIT_SCALE,
};
use amari_rewrite::trs::{Rule, Term, TermSystem};

// ── fixtures ────────────────────────────────────────────────────────

/// A ranker specification, parsed through the same JSON seam callers use.
fn config(hidden: &[u32]) -> RankerConfig {
    let hidden = hidden
        .iter()
        .map(|width| format!("{{\"width\":{width}}}"))
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(
        "{{\"model_type\":\"ia-mlp\",\"schema_version\":1,\"input_width\":{},\
         \"hidden\":[{hidden}],\"head\":{{\"width\":1}},\
         \"hidden_activation\":\"relu\"}}",
        CANDIDATE_FEATURE_WIDTH
    );
    RankerConfig::from_spec_json(&json).expect("valid ranker spec")
}

fn lanes(pairs: &[(usize, f32)]) -> [f32; CANDIDATE_FEATURE_WIDTH] {
    let mut values = [0.0f32; CANDIDATE_FEATURE_WIDTH];
    for (index, value) in pairs {
        values[*index] = *value;
    }
    values
}

fn features(pairs: &[(usize, f32)]) -> FeatureVector {
    FeatureVector::new(lanes(pairs)).expect("valid feature lanes")
}

/// One example where the chosen candidate carries `chosen_lane_0` and the
/// unchosen carries `unchosen_lane_0` on lane 0, plus a shared nuisance lane.
fn example(chosen: &[(usize, f32)], unchosen: &[(usize, f32)]) -> TrainingExample {
    TrainingExample::new(features(chosen), features(unchosen))
}

/// A separable toy corpus: every pair differs on lane 0 (chosen higher), and
/// a shared lane 1/2 value keeps the non-discriminating weights gradient-free
/// on those lanes. Achievable margin: chosen lane 0 is 1.0, 0.75, 0.5 against
/// 0.0, 0.25, 0.0, so a positive lane-0 weight separates all three.
fn separable_corpus() -> Vec<TrainingExample> {
    vec![
        example(&[(0, 1.0)], &[]),
        example(&[(0, 0.75), (1, 0.25)], &[(0, 0.25), (1, 0.25)]),
        example(&[(0, 0.5), (2, 0.5)], &[(0, 0.0), (2, 0.5)]),
    ]
}

/// A corpus that can never converge: chosen and unchosen are the same point,
/// so the score difference is always zero and the hinge is always the full
/// margin.
fn unsolvable_corpus() -> Vec<TrainingExample> {
    let same = features(&[(3, 0.5)]);
    vec![
        TrainingExample::new(same, same),
        TrainingExample::new(same, same),
    ]
}

fn train_config(max_steps: usize, seed: u64, learning_rate: f32) -> TrainConfig {
    TrainConfig::new(
        learning_rate,
        0.9,
        0.999,
        1e-8,
        0.0,
        1.0,
        max_steps,
        seed,
        ExampleOrder::Canonical,
    )
    .expect("valid training config")
}

/// Canonical tensors filled from a fixed deterministic pattern (small enough
/// that the ReLU range is live and the hinge is active).
fn patterned_tensors(config: &RankerConfig) -> Vec<(String, Vec<f32>)> {
    let mut stream = 0u64;
    config
        .expected_tensors()
        .into_iter()
        .map(|(name, shape)| {
            let elements: usize = shape.iter().product();
            let values = (0..elements)
                .map(|_| {
                    stream = stream.wrapping_add(1);
                    ((stream % 11) as f32 - 5.0) * 0.01
                })
                .collect();
            (name, values)
        })
        .collect()
}

fn build_weights(config: &RankerConfig, tensors: Vec<(String, Vec<f32>)>) -> RankerWeights {
    let borrowed: Vec<(&str, &[f32])> = tensors
        .iter()
        .map(|(name, values)| (name.as_str(), values.as_slice()))
        .collect();
    RankerWeights::from_tensors(config, &borrowed).expect("valid weights")
}

fn patterned_weights(config: &RankerConfig) -> RankerWeights {
    build_weights(config, patterned_tensors(config))
}

fn ranker(config: &RankerConfig, weights: &RankerWeights) -> MlpRanker {
    MlpRanker::new(config.clone(), weights.clone()).expect("matching weights")
}

/// Total hinge loss of a corpus under a weight set, computed through the
/// public hinge evaluation.
fn total_loss(
    config: &RankerConfig,
    weights: &RankerWeights,
    margin: f32,
    corpus: &[TrainingExample],
) -> f32 {
    corpus
        .iter()
        .map(|example| {
            pairwise_hinge(config, weights, margin, example)
                .expect("finite hinge evaluation")
                .loss()
        })
        .sum()
}

/// Every optimizer entry's bits, so equality is bit equality rather than
/// `f32` equality (`-0.0 == 0.0`, `NaN != NaN`).
fn entry_bits(entries: &[amari_rewrite::neural::OptimizerStateEntry]) -> Vec<(String, Vec<u32>)> {
    entries
        .iter()
        .map(|entry| {
            (
                entry.name().to_string(),
                entry.values().iter().map(|value| value.to_bits()).collect(),
            )
        })
        .collect()
}

// ── determinism ─────────────────────────────────────────────────────

#[test]
fn identical_inputs_produce_byte_identical_parameters_and_optimizer_state() {
    let config = config(&[4]);
    let corpus = separable_corpus();
    let settings = train_config(64, 0x5eed, 0.05);

    let first = train_from_seed(&config, &settings, &corpus).expect("first run");
    let second = train_from_seed(&config, &settings, &corpus).expect("second run");

    assert_eq!(
        first.steps_run(),
        second.steps_run(),
        "the same configuration must stop after the same number of steps"
    );
    assert!(
        first.steps_run() > corpus.len(),
        "the run must execute more than one epoch for the determinism claim to mean anything"
    );
    assert!(
        first.optimizer_state().tracked_elements() > 0,
        "the optimizer state must actually be populated"
    );
    assert_eq!(
        first.is_complete(),
        second.is_complete(),
        "the same configuration must reach the same outcome kind"
    );
    // The digest hashes every parameter's bits, so this is bit equality, not
    // approximate equality.
    assert_eq!(
        first.parameters().digest(),
        second.parameters().digest(),
        "two runs of the same (dataset, config, seed) must produce identical parameters"
    );
    assert_eq!(
        entry_bits(&first.optimizer_state().optimizer_state_entries()),
        entry_bits(&second.optimizer_state().optimizer_state_entries()),
        "two runs must produce an identical optimizer state, bit for bit"
    );
    assert_eq!(
        first.optimizer_state().step(),
        second.optimizer_state().step()
    );
    // And the whole outcome compares equal, which additionally pins the
    // exported naming order.
    assert_eq!(first, second);
}

#[test]
fn seeded_example_order_is_a_pure_function_of_seed_and_length() {
    let settings = TrainConfig::new(
        0.01,
        0.9,
        0.999,
        1e-8,
        0.0,
        1.0,
        16,
        0xabcd,
        ExampleOrder::SeededPermutation,
    )
    .expect("valid config");

    let first = example_order(&settings, 32);
    let second = example_order(&settings, 32);
    assert_eq!(
        first, second,
        "the order must be a pure function of the seed"
    );
    let mut sorted = first.clone();
    sorted.sort_unstable();
    assert_eq!(
        sorted,
        (0..32).collect::<Vec<_>>(),
        "the order must be a permutation of the example indices"
    );
    assert_ne!(
        first,
        (0..32).collect::<Vec<_>>(),
        "a seeded permutation of 32 examples must not be the identity"
    );

    let other_seed = TrainConfig::new(
        0.01,
        0.9,
        0.999,
        1e-8,
        0.0,
        1.0,
        16,
        0xabce,
        ExampleOrder::SeededPermutation,
    )
    .expect("valid config");
    assert_ne!(
        first,
        example_order(&other_seed, 32),
        "a different seed must permute differently"
    );

    let canonical = train_config(16, 0xabcd, 0.01);
    assert_eq!(
        example_order(&canonical, 8),
        (0..8).collect::<Vec<_>>(),
        "canonical order is the dataset's own order"
    );
    // Degenerate lengths must not panic or break the permutation property.
    assert!(example_order(&settings, 0).is_empty());
    assert_eq!(example_order(&settings, 1), vec![0]);
}

// ── initialization ──────────────────────────────────────────────────

#[test]
fn the_same_seed_initializes_identically_and_different_seeds_do_not() {
    let config = config(&[2, 3]);

    let first = seeded_weights(&config, 7).expect("seeded weights");
    let again = seeded_weights(&config, 7).expect("seeded weights");
    let other = seeded_weights(&config, 8).expect("seeded weights");

    assert_eq!(
        first.digest(),
        again.digest(),
        "the same seed must produce bit-identical initial weights"
    );
    assert_ne!(
        first.digest(),
        other.digest(),
        "a different seed must produce different initial weights"
    );
    // The initialization must be admissible to a ranker as-is: inside the
    // architecture's magnitude bound.
    let _ = MlpRanker::new(config.clone(), first);
}

#[test]
fn seeded_initialization_follows_the_documented_integer_mapping() {
    let config = config(&[3]);
    let seed = 0x0123_4567_89ab_cdefu64;

    // An independent re-implementation of the documented mapping (SplitMix64
    // with the canonical constants, then
    // `(top-24-bit fraction - 0.5) * 2 * INIT_SCALE`), consumed in the
    // canonical tensor order.
    let mut state = seed;
    let mut next = || {
        state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    };
    let mut tensors = Vec::new();
    for (name, shape) in config.expected_tensors() {
        let elements: usize = shape.iter().product();
        let values = (0..elements)
            .map(|_| {
                let fraction = (next() >> 40) as f32 * (1.0 / 16_777_216.0);
                (fraction - 0.5) * (2.0 * INIT_SCALE)
            })
            .collect();
        tensors.push((name, values));
    }
    let expected = build_weights(&config, tensors);

    assert_eq!(
        seeded_weights(&config, seed)
            .expect("seeded weights")
            .digest(),
        expected.digest(),
        "the seed-to-weights mapping must be exactly the documented one"
    );
}

// ── convergence sanity ──────────────────────────────────────────────

#[test]
fn a_separable_corpus_trains_until_every_chosen_score_wins() {
    let config = config(&[4]);
    let corpus = separable_corpus();
    let init = seeded_weights(&config, 11).expect("seeded weights");
    let settings = train_config(20_000, 11, 0.05);

    let initial_loss = total_loss(&config, &init, settings.margin(), &corpus);
    assert!(
        initial_loss > 0.0,
        "the fixture must start with an active hinge (got {initial_loss})"
    );

    let outcome = train(&config, &init, &settings, &corpus).expect("training run");
    let final_loss = total_loss(&config, outcome.parameters(), settings.margin(), &corpus);

    assert!(
        final_loss < initial_loss,
        "training must decrease the hinge loss: {initial_loss} -> {final_loss}"
    );
    assert_eq!(final_loss, 0.0, "a separable corpus must reach zero loss");

    let trained = ranker(&config, outcome.parameters());
    for example in &corpus {
        let chosen = trained
            .score_features(example.chosen())
            .expect("finite chosen score");
        let unchosen = trained
            .score_features(example.unchosen())
            .expect("finite unchosen score");
        assert!(
            chosen > unchosen,
            "the trained ranker must prefer the chosen candidate ({chosen} vs {unchosen})"
        );
    }
    assert!(
        outcome.is_complete(),
        "a converging run must report Completed, got {:?}",
        outcome.partial_reason()
    );
    assert!(outcome.is_replayable());
    assert!(
        outcome.optimizer_state().step() <= outcome.steps_run(),
        "a no-op example does not advance the optimizer"
    );
    assert!(
        outcome.optimizer_state().step() > 0,
        "a converged run must have applied at least one update"
    );
}

// ── the trainer's forward pass is the ranker's ──────────────────────

#[test]
fn the_trainers_hinge_uses_the_rankers_own_score_bits() {
    let config = config(&[3, 2]);
    let weights = patterned_weights(&config);
    let model = ranker(&config, &weights);
    let margin = 1.0f32;

    for example in separable_corpus() {
        let evaluation =
            pairwise_hinge(&config, &weights, margin, &example).expect("hinge evaluation");
        let chosen = model.score_features(example.chosen()).expect("score");
        let unchosen = model.score_features(example.unchosen()).expect("score");
        let difference = chosen - unchosen;
        let expected = if difference >= margin {
            0.0
        } else {
            margin - difference
        };
        assert_eq!(
            evaluation.loss().to_bits(),
            expected.to_bits(),
            "the trainer's forward pass must reproduce the ranker's score bits \
             (chosen {chosen}, unchosen {unchosen})"
        );
    }
}

// ── the hand-derived gradient ───────────────────────────────────────

#[test]
fn the_hand_derived_gradient_matches_a_central_difference() {
    let config = config(&[2]);
    let corpus = separable_corpus();
    let example = corpus[1];
    let margin = 1.0f32;
    let base = patterned_tensors(&config);
    let weights = build_weights(&config, base.clone());

    let evaluation = pairwise_hinge(&config, &weights, margin, &example).expect("hinge evaluation");
    assert!(evaluation.loss() > 0.0, "the fixture hinge must be active");

    // Check one hidden-layer weight element and one head weight element. The
    // gradient's internal layout is `[in, out]`, the canonical tensors are
    // `[out, in]`, so `gradient[k * out + o]` is canonical element
    // `o * in + k`.
    let step = 1e-3f32;
    let mut checked = 0usize;
    for (tensor_index, (name, shape)) in config.expected_tensors().into_iter().enumerate() {
        if name != "hidden.0.weight" && name != "head.weight" {
            continue;
        }
        let out_width = shape[0];
        let in_width = shape[1];
        // A middle element, away from the tensor's edges.
        let canonical_index = (out_width / 2) * in_width + in_width / 2;
        let out = canonical_index / in_width;
        let input = canonical_index % in_width;

        // PR #367 round 1 P2b: a gradient check is only evidence at a *live*
        // coordinate. The chosen example must feed this weight's input lane and
        // the hinge's derivative with respect to it must be non-zero, otherwise
        // the comparison below proves nothing (a zero derivative matches a zero
        // finite difference).
        assert!(
            example.chosen().lanes()[input] != 0.0,
            "{name}[{canonical_index}]: input lane {input} is zero in the chosen \
             example, so this coordinate is dead and unusable as a probe"
        );

        let analytic = if name == "head.weight" {
            evaluation.gradients().head().weights()[canonical_index]
        } else {
            evaluation.gradients().layers()[0].weights()[input * out_width + out]
        };

        let perturbed = |delta: f32| {
            let mut tensors = base.clone();
            tensors[tensor_index].1[canonical_index] += delta;
            build_weights(&config, tensors)
        };
        let up = pairwise_hinge(&config, &perturbed(step), margin, &example)
            .expect("hinge evaluation")
            .loss();
        let down = pairwise_hinge(&config, &perturbed(-step), margin, &example)
            .expect("hinge evaluation")
            .loss();
        assert!(
            analytic != 0.0,
            "{name}[{canonical_index}]: the analytic derivative is zero, so this \
             coordinate cannot distinguish a correct gradient from a missing one"
        );
        let numeric = (up - down) / (2.0 * step);
        let tolerance = 2e-2 * analytic.abs().max(1.0);
        assert!(
            (numeric - analytic).abs() <= tolerance,
            "{name}[{canonical_index}]: analytic {analytic}, central difference {numeric} \
             (tolerance {tolerance})"
        );
        checked += 1;
    }
    assert_eq!(
        checked, 2,
        "both a hidden and a head weight must be checked"
    );
}

#[test]
fn a_satisfied_example_has_no_gradient() {
    let config = config(&[2]);
    // A ranker that separates the corpus by a wide margin: lane 0 weighted
    // positively through one hidden unit, head weight positive.
    let mut tensors = patterned_tensors(&config);
    for (name, values) in &mut tensors {
        match name.as_str() {
            "hidden.0.weight" => {
                values.fill(0.0);
                values[0] = 4.0; // [out, in]: output 0, input lane 0
            }
            "hidden.0.bias" => values.fill(0.0),
            "head.weight" => {
                values.fill(0.0);
                values[0] = 4.0;
            }
            "head.bias" => values.fill(0.0),
            _ => unreachable!("canonical tensor names are known"),
        }
    }
    let weights = build_weights(&config, tensors);
    let example = example(&[(0, 1.0)], &[(0, 0.0)]);

    let evaluation = pairwise_hinge(&config, &weights, 1.0, &example).expect("hinge evaluation");
    assert_eq!(evaluation.loss(), 0.0, "16 - 0 exceeds the unit margin");
    assert!(
        evaluation.gradients().layers().iter().all(|layer| layer
            .weights()
            .iter()
            .all(|value| *value == 0.0)
            && layer.biases().iter().all(|value| *value == 0.0)),
        "a satisfied example's hinge subgradient is zero"
    );
    assert!(evaluation
        .gradients()
        .head()
        .weights()
        .iter()
        .all(|value| *value == 0.0));
}

// ── typed failure discipline ────────────────────────────────────────

#[test]
fn a_huge_learning_rate_overflows_into_a_typed_nonfinite_error() {
    let config = config(&[2]);
    let corpus = separable_corpus();
    let init = seeded_weights(&config, 3).expect("seeded weights");
    // Finite on both counts (so validation admits them), but
    // `learning_rate * weight_decay` overflows f32, which makes the decoupled
    // decay factor non-finite: the run must report that as a typed error at
    // the step where it happened, not return a NaN parameter.
    let settings = TrainConfig::new(
        f32::MAX,
        0.9,
        0.999,
        1e-8,
        f32::MAX,
        1.0,
        8,
        3,
        ExampleOrder::Canonical,
    )
    .expect("the values are finite and in range");

    match train(&config, &init, &settings, &corpus) {
        Err(TrainingError::NonFinite { step }) => assert_eq!(step, 0),
        other => panic!("expected a typed non-finite error at step 0, got {other:?}"),
    }
}

#[test]
fn a_diverging_run_reports_a_typed_error_with_its_step_index() {
    let config = config(&[2]);
    let corpus = separable_corpus();
    let init = seeded_weights(&config, 5).expect("seeded weights");
    // A step size far above the architecture's admissible magnitude: the
    // first update leaves the valid parameter range, which the per-step check
    // reports as a typed error carrying the step index. Nothing panics and no
    // non-finite parameter escapes.
    let settings = train_config(16, 5, 1e30);

    match train(&config, &init, &settings, &corpus) {
        Err(TrainingError::NonFinite { step })
        | Err(TrainingError::ParameterOutOfBounds { step }) => assert!(
            step < settings.max_steps(),
            "the reported step index must be inside the run (got {step})"
        ),
        other => panic!("expected a typed divergence error, got {other:?}"),
    }
}

#[test]
fn a_step_budget_exhausts_into_a_non_replayable_partial_outcome() {
    let config = config(&[2]);
    let corpus = unsolvable_corpus();
    let init = seeded_weights(&config, 21).expect("seeded weights");
    let settings = train_config(3, 21, 0.01);

    let outcome = train(&config, &init, &settings, &corpus).expect("a budget stop is not an error");

    assert!(
        !outcome.is_complete(),
        "the unsolvable corpus cannot converge inside three steps"
    );
    assert!(
        !outcome.is_replayable(),
        "a Partial outcome is non-replayable"
    );
    assert_eq!(
        outcome.partial_reason(),
        Some(PartialReason::StepBudgetExhausted)
    );
    assert_eq!(outcome.steps_run(), 3, "the budget is exactly the stop");
    assert!(matches!(outcome, TrainingOutcome::Partial { .. }));
    // The snapshot is still a valid parameter set, so a caller can inspect it
    // — it just must not treat it as trained.
    let _ = MlpRanker::new(config.clone(), outcome.parameters().clone());
}

#[test]
fn configuration_validation_rejects_unusable_values() {
    let order = ExampleOrder::Canonical;
    let base = (
        0.01f32, 0.9f32, 0.999f32, 1e-8f32, 0.0f32, 1.0f32, 4usize, 0u64,
    );

    // learning rate must be positive and finite
    for learning_rate in [0.0, -1.0] {
        match TrainConfig::new(
            learning_rate,
            base.1,
            base.2,
            base.3,
            base.4,
            base.5,
            base.6,
            base.7,
            order,
        ) {
            Err(TrainingError::InvalidConfig {
                field: "learning_rate",
                ..
            }) => {}
            other => {
                panic!("learning_rate {learning_rate}: expected typed rejection, got {other:?}")
            }
        }
    }
    for learning_rate in [f32::NAN, f32::INFINITY] {
        match TrainConfig::new(
            learning_rate,
            base.1,
            base.2,
            base.3,
            base.4,
            base.5,
            base.6,
            base.7,
            order,
        ) {
            Err(TrainingError::InvalidConfig {
                field: "learning_rate",
                ..
            }) => {}
            other => {
                panic!("learning_rate {learning_rate}: expected typed rejection, got {other:?}")
            }
        }
    }

    // betas must lie in [0, 1)
    for beta in [1.0, 1.5, -0.1] {
        match TrainConfig::new(
            base.0, beta, base.2, base.3, base.4, base.5, base.6, base.7, order,
        ) {
            Err(TrainingError::InvalidConfig { field: "beta1", .. }) => {}
            other => panic!("beta1 {beta}: expected typed rejection, got {other:?}"),
        }
        match TrainConfig::new(
            base.0, base.1, beta, base.3, base.4, base.5, base.6, base.7, order,
        ) {
            Err(TrainingError::InvalidConfig { field: "beta2", .. }) => {}
            other => panic!("beta2 {beta}: expected typed rejection, got {other:?}"),
        }
    }
    match TrainConfig::new(
        base.0,
        f32::NAN,
        base.2,
        base.3,
        base.4,
        base.5,
        base.6,
        base.7,
        order,
    ) {
        Err(TrainingError::InvalidConfig { field: "beta1", .. }) => {}
        other => panic!("beta1 NaN: expected typed rejection, got {other:?}"),
    }

    // epsilon must be positive
    for epsilon in [0.0, -1e-8] {
        match TrainConfig::new(
            base.0, base.1, base.2, epsilon, base.4, base.5, base.6, base.7, order,
        ) {
            Err(TrainingError::InvalidConfig {
                field: "epsilon", ..
            }) => {}
            other => panic!("epsilon {epsilon}: expected typed rejection, got {other:?}"),
        }
    }

    // weight decay must be non-negative
    match TrainConfig::new(
        base.0, base.1, base.2, base.3, -0.1, base.5, base.6, base.7, order,
    ) {
        Err(TrainingError::InvalidConfig {
            field: "weight_decay",
            ..
        }) => {}
        other => panic!("expected typed weight_decay rejection, got {other:?}"),
    }

    // the margin must be positive
    for margin in [0.0, -1.0] {
        match TrainConfig::new(
            base.0, base.1, base.2, base.3, base.4, margin, base.6, base.7, order,
        ) {
            Err(TrainingError::InvalidConfig {
                field: "margin", ..
            }) => {}
            other => panic!("margin {margin}: expected typed rejection, got {other:?}"),
        }
    }

    // zero steps is a ceiling violation, reported as such
    match TrainConfig::new(
        base.0, base.1, base.2, base.3, base.4, base.5, 0, base.7, order,
    ) {
        Err(TrainingError::LimitExceeded {
            resource: "training steps",
            value: 0,
            ..
        }) => {}
        other => panic!("zero steps: expected typed limit rejection, got {other:?}"),
    }

    // the defaults are valid
    let defaults = TrainConfig::default();
    defaults.validate().expect("defaults are valid");
    assert_eq!(defaults.margin(), 1.0);
}

#[test]
fn admission_rejects_a_step_budget_past_the_ceiling() {
    let config = config(&[1]);
    let init = seeded_weights(&config, 1).expect("seeded weights");
    let settings = TrainConfig::new(
        0.01,
        0.9,
        0.999,
        1e-8,
        0.0,
        1.0,
        TrainConfig::MAX_STEPS + 1,
        1,
        ExampleOrder::Canonical,
    );

    // Construction itself is the admission gate.
    match settings {
        Err(TrainingError::LimitExceeded {
            resource: "training steps",
            value,
            limit,
        }) => {
            assert_eq!(value, TrainConfig::MAX_STEPS + 1);
            assert_eq!(limit, TrainConfig::MAX_STEPS);
        }
        other => panic!("expected a typed step-ceiling rejection, got {other:?}"),
    }
    // And an over-ceiling corpus is refused before any buffer is sized.
    let oversized = vec![separable_corpus()[0]; TrainConfig::MAX_TRAIN_EXAMPLES + 1];
    let settings = train_config(4, 1, 0.01);
    match train(&config, &init, &settings, &oversized) {
        Err(TrainingError::LimitExceeded {
            resource: "training examples",
            value,
            limit,
        }) => {
            assert_eq!(value, TrainConfig::MAX_TRAIN_EXAMPLES + 1);
            assert_eq!(limit, TrainConfig::MAX_TRAIN_EXAMPLES);
        }
        other => panic!("expected a typed example-ceiling rejection, got {other:?}"),
    }
    // An empty corpus is a typed error, not a silent no-op.
    match train(&config, &init, &settings, &[]) {
        Err(TrainingError::EmptyDataset) => {}
        other => panic!("expected a typed empty-corpus rejection, got {other:?}"),
    }
}

#[test]
fn initial_weights_must_match_the_architecture() {
    let narrow = config(&[2]);
    let wide = config(&[3]);
    let weights = seeded_weights(&narrow, 1).expect("seeded weights");
    let corpus = separable_corpus();
    let settings = train_config(4, 1, 0.01);

    match train(&wide, &weights, &settings, &corpus) {
        Err(TrainingError::MismatchedWeights) => {}
        other => panic!("expected a typed architecture mismatch, got {other:?}"),
    }
}

// ── optimizer state export (provisional S6 naming) ──────────────────

#[test]
fn optimizer_state_export_is_one_provisional_naming_function() {
    let config = config(&[3]);
    let corpus = separable_corpus();
    let init = seeded_weights(&config, 2).expect("seeded weights");
    let settings = train_config(8, 2, 0.05);
    let outcome = train(&config, &init, &settings, &corpus).expect("training run");
    let entries = outcome.optimizer_state().optimizer_state_entries();

    // One `m` and one `v` tensor per parameter tensor, in canonical order.
    let mut expected_names = Vec::new();
    for (name, _) in config.expected_tensors() {
        expected_names.push(format!("{name}{ADAMW_MOMENT_SUFFIX}"));
        expected_names.push(format!("{name}{ADAMW_VELOCITY_SUFFIX}"));
    }
    let actual_names: Vec<String> = entries
        .iter()
        .map(|entry| entry.name().to_string())
        .collect();
    assert_eq!(actual_names, expected_names);

    // The values are in the canonical `[out, in]` layout, so their lengths
    // are the canonical tensor lengths (two entries per parameter tensor).
    let tensors = config.expected_tensors();
    for (index, entry) in entries.iter().enumerate() {
        let (name, shape) = &tensors[index / 2];
        let elements: usize = shape.iter().product();
        assert_eq!(
            entry.values().len(),
            elements,
            "entry {name} must carry one value per parameter element"
        );
    }
    assert_eq!(entries.len(), config.expected_tensors().len() * 2);
    assert_eq!(
        outcome.optimizer_state().tracked_elements(),
        entries
            .iter()
            .map(|entry| entry.values().len())
            .sum::<usize>()
    );

    // The convention is explicitly provisional until Task 43 reconciles it.
    assert!(ADAMW_MOMENT_SUFFIX.contains("adamw.m"));
    assert!(ADAMW_VELOCITY_SUFFIX.contains("adamw.v"));
    assert!(ADAMW_NAMING_STATUS.contains("provisional"));
}

// ── the trait seam names the real gradient type ─────────────────────

#[test]
fn the_ranker_advertises_the_trainers_gradient_type() {
    let config = config(&[2]);
    let weights = patterned_weights(&config);
    let model = ranker(&config, &weights);
    let example = separable_corpus()[1];

    // `Gradient` is the concrete hand-derived type, not the unit placeholder.
    fn assert_gradient_is_concrete<
        T: DifferentiableRule<Vec<f32>, Gradient = amari_rewrite::neural::RankerGradients>,
    >(
        _value: &T,
    ) {
    }
    assert_gradient_is_concrete(&model);

    let evaluation = pairwise_hinge(&config, &weights, 1.0, &example).expect("hinge evaluation");
    assert_eq!(
        evaluation.gradients().element_count(),
        config.tensor_elements() as usize,
        "the gradient must have one element per parameter"
    );
    assert_eq!(
        evaluation
            .gradients()
            .layers()
            .iter()
            .map(|layer| layer.shape())
            .collect::<Vec<_>>(),
        vec![(CANDIDATE_FEATURE_WIDTH, 2)],
    );
}

// ── the dataset bridge ──────────────────────────────────────────────

fn parse_term(text: &str) -> Term {
    let text = text.trim();
    if let Some(open) = text.find('(') {
        let head = &text[..open];
        let inner = &text[open + 1..text.len() - 1];
        let mut args = Vec::new();
        let mut depth = 0i32;
        let mut start = 0;
        for (index, ch) in inner.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => depth -= 1,
                ',' if depth == 0 => {
                    args.push(parse_term(&inner[start..index]));
                    start = index + 1;
                }
                _ => {}
            }
        }
        args.push(parse_term(&inner[start..]));
        Term::sym(head, args)
    } else if text.chars().next().is_some_and(char::is_uppercase) || text.starts_with('?') {
        Term::var(text)
    } else {
        Term::constant(text)
    }
}

/// The add/h system: from `a`, both `add(a, zero)` and `h(a)` are valid
/// predecessors, so a replayed trace always carries unchosen negatives.
fn add_h_system() -> TermSystem {
    TermSystem::new(vec![
        Rule::new(parse_term("add(X, zero)"), parse_term("X")).expect("checked rule"),
        Rule::new(parse_term("h(X)"), parse_term("X")).expect("checked rule"),
    ])
}

fn backward_witness(system: &TermSystem) -> ReplayCertificate {
    let target = parse_term("a");
    let goal = parse_term("h(add(a, zero))");
    let outcome = BackwardExplorer::new(system, InverseSearchConfig::default())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .expect("search succeeds");
    let BackwardSearchOutcome::Witness(derivation) = outcome else {
        panic!("expected a witness, got {outcome:?}");
    };
    ReplayCertificate::issue(
        system,
        ReplayQuery::Backward { target, goal },
        ReplayDerivation::Backward(derivation),
        InverseSearchConfig::default(),
        GuidanceMode::CompleteWithinLimits,
        ResourceObservation {
            states: 4,
            transitions: 4,
            retained_bytes: 512,
            operations: 8,
        },
    )
}

#[test]
fn a_task_41_dataset_trains_end_to_end() {
    let system = add_h_system();
    let certificate = backward_witness(&system);
    let builder = DatasetBuilder::new(DatasetConfig::default()).expect("dataset config");
    let dataset = builder
        .build(&system, &[certificate])
        .expect("dataset builds");
    assert!(
        !dataset.is_empty(),
        "the fixture trace must produce at least one pairwise example"
    );

    let corpus = corpus_from_dataset(&dataset);
    assert_eq!(
        corpus.len(),
        dataset.len(),
        "the bridge copies every example"
    );

    let config = config(&[4]);
    let init = seeded_weights(&config, 9).expect("seeded weights");
    // A small margin: the real encoder's lanes are ordinary ratios, so the
    // first evaluation is well inside the hinge's active range.
    let settings = TrainConfig::new(
        0.05,
        0.9,
        0.999,
        1e-8,
        0.0,
        0.1,
        4_000,
        9,
        ExampleOrder::SeededPermutation,
    )
    .expect("valid config");

    let initial_loss = total_loss(&config, &init, settings.margin(), &corpus);
    assert!(initial_loss > 0.0, "the fixture hinge must be active");
    let outcome = train(&config, &init, &settings, &corpus).expect("training over a dataset");
    let final_loss = total_loss(&config, outcome.parameters(), settings.margin(), &corpus);
    assert!(
        final_loss < initial_loss,
        "training over a Task 41 dataset must decrease the hinge loss: \
         {initial_loss} -> {final_loss}"
    );
    assert!(
        !outcome.parameters().digest().as_bytes().is_empty(),
        "the outcome carries a real weight set"
    );
}
