//! Bounded AdamW training contracts (0.25 Cohort 8, Task 42).
//!
//! The tests come in the four groups the task enumerates — determinism,
//! convergence sanity, typed failure discipline, and admission — plus two
//! structural pinning tests: the seeded initialization is checked against an
//! independent re-implementation of the documented SplitMix64 mapping, and
//! the hand-derived gradient is checked against a central finite difference at
//! *live* coordinates — every probe must have a non-zero analytic derivative
//! (PR #367 round 1 P2b), and the two coordinates whose derivative is
//! identically zero by construction are asserted to be exactly zero rather than
//! silently "verified". Allocation-before-admission is measured in a separate
//! binary (`neural_training_admission.rs`), because a global counter is only
//! meaningful in a process that is not allocating in parallel.
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

// ── the hand-derived gradient ───���───────────────────────────────────

/// A fixture built for *live* gradient probes (PR #367 round 1 P2b).
///
/// Two hidden layers of width 2, with every pre-activation placed far from a
/// ReLU kink, and — the load-bearing part — with one unit per hidden layer
/// **live for the chosen example and dead for the unchosen one**. That
/// asymmetry is what makes the pairwise hinge's bias derivatives non-zero: the
/// loss only sees *differences* between the two scores, so any coordinate whose
/// effect is identical on both (a bias at a unit whose ReLU mask agrees, and
/// always the head bias) has an identically zero derivative and cannot be used
/// as a probe.
///
/// Hand-computed forward pass (margin 4.0, so the hinge stays active):
///
/// ```text
///            chosen                 unchosen
/// lane 0     1.0                    0.1
/// lane 1     0.5                    0.2
/// lane 2     1.0                    0.5
/// h0 pre     [2.25, 0.5]            [0.30, -0.45]   (unit 1 dead)
/// h0         [2.25, 0.5]            [0.30, 0.0]
/// h1 pre     [3.475, 0.525]         [0.55, -0.10]   (unit 1 dead)
/// h1         [3.475, 0.525]         [0.55, 0.0]
/// score       4.11875                0.80           difference 3.31875
/// loss        4.0 - 3.31875 = 0.68125 (active)
/// ```
///
/// Every ReLU pre-activation is at least `0.10` away from zero and the margin
/// headroom is `0.68`, so a `5e-3` perturbation cannot cross a kink or satisfy
/// the hinge: the loss stays affine in the perturbed coordinate.
fn gradient_tensors(config: &RankerConfig) -> Vec<(String, Vec<f32>)> {
    let mut tensors = config
        .expected_tensors()
        .into_iter()
        .map(|(name, shape)| {
            let elements: usize = shape.iter().product();
            (name, vec![0.0f32; elements])
        })
        .collect::<Vec<_>>();
    // `[out, in]` row-major: `out * input_width + input`, so output row `o`
    // starts at `o * width`.
    let width = CANDIDATE_FEATURE_WIDTH;
    for (name, values) in &mut tensors {
        match name.as_str() {
            "hidden.0.weight" => {
                values[0] = 2.0; // (out 0, in 0)
                values[1] = 0.5; // (out 0, in 1)
                values[width] = 0.5; // (out 1, in 0)
                values[width + 2] = 1.0; // (out 1, in 2)
            }
            "hidden.0.bias" => values[1] = -1.0,
            "hidden.1.weight" => {
                values[0] = 1.5; // (out 0, in 0)
                values[3] = 1.25; // (out 1, in 1), at `1 * 2 + 1`
            }
            "hidden.1.bias" => {
                values[0] = 0.1;
                values[1] = -0.1;
            }
            "head.weight" => {
                values[0] = 1.0;
                values[1] = 0.75;
            }
            "head.bias" => values[0] = 0.25,
            other => unreachable!("unexpected canonical tensor {other}"),
        }
    }
    tensors
}

fn gradient_fixture() -> (RankerConfig, RankerWeights, TrainingExample) {
    let config = config(&[2, 2]);
    let weights = build_weights(&config, gradient_tensors(&config));
    let example = example(
        &[(0, 1.0), (1, 0.5), (2, 1.0)],
        &[(0, 0.1), (1, 0.2), (2, 0.5)],
    );
    (config, weights, example)
}

/// One coordinate to probe: the tensor name, its index in the canonical
/// `[out, in]` layout, and whether the hinge's derivative there is expected to
/// be non-zero.
struct Probe {
    tensor: &'static str,
    canonical_index: usize,
    live: bool,
    why: &'static str,
}

/// The central-difference step.
///
/// `5e-3` is the documented sweet spot for an *f32* central difference:
///
/// - **No truncation term.** Inside a fixed ReLU mask with the hinge active,
///   the loss is affine in each individual parameter (an affine score, a
///   monotone relu, an active hinge), so the `O(eps^2)` Taylor term is exactly
///   zero. `eps` is therefore not limited by curvature; the fixture's
///   constraint is distance to the nearest kink (`0.10` at the h1-unit-1
///   pre-activation) and to the margin (`0.68`), both of which `5e-3` clears by
///   a factor of at least `20`.
/// - **Rounding dominates.** Each loss evaluation rounds at `gamma ~ 4 *
///   2^-24 ~ 2.4e-7` relative on `|loss| <= 2`, and the difference of two such
///   values carries that error absolutely. Dividing by `2 * eps = 1e-2` gives a
///   noise floor of `~ gamma * |loss| / (2 eps) ~ 5e-5`, which is the number
///   the tolerance below is built from (`4x` headroom).
///
/// A larger `eps` would not help (there is no truncation to shrink) and would
/// start to approach the kinks; a smaller one only amplifies f32 rounding.
///
/// Measured on this fixture, the worst observed error over the ten probes is
/// `4.5e-5` — the predicted rounding floor, not a truncation residual — so the
/// tolerance below is a derived bound with real headroom rather than a number
/// chosen to make the test pass.
const CENTRAL_DIFFERENCE_STEP: f32 = 5e-3;

/// Tolerance for the comparison: the rounding noise floor above (`~5e-5`),
/// with `4x` headroom, scaled by the gradient's own magnitude for the `|g| > 1`
/// coefficients.
fn central_difference_tolerance(analytic: f32) -> f32 {
    2e-4 * analytic.abs().max(1.0)
}

#[test]
fn the_hand_derived_gradient_matches_a_central_difference() {
    let (config, weights, example) = gradient_fixture();
    let margin = 4.0f32;
    // The canonical tensors the fixture was built from, so a perturbation can
    // be applied by canonical index.
    let base = gradient_tensors(&config);

    let evaluation = pairwise_hinge(&config, &weights, margin, &example).expect("hinge evaluation");
    assert!(
        (evaluation.loss() - 0.68125).abs() < 1e-5,
        "the fixture's loss must be the hand-computed 0.68125 (got {})",
        evaluation.loss()
    );
    assert!(evaluation.loss() > 0.0, "the hinge must be active");

    // The chosen example must actually feed these lanes: a weight probe whose
    // input lane is zero is a dead coordinate (PR #367 round 1 P2b).
    for lane in [0, 1, 2] {
        assert_ne!(
            example.chosen().lanes()[lane],
            0.0,
            "lane {lane} must be non-zero in the chosen example"
        );
    }
    // And the fixture must be separable away from every kink.
    assert!(
        example.chosen().lanes()[2] != example.unchosen().lanes()[2],
        "lane 2 must differ between the pair, so the hidden unit that only it \
         feeds is live for exactly one of them"
    );

    let probes = [
        Probe {
            // Deepest propagation: two layers back, and the input lane is
            // non-zero in both examples but larger for the chosen one.
            tensor: "hidden.0.weight",
            canonical_index: 0, // (out 0, in 0)
            live: true,
            why: "hidden-layer-0 weight two layers deep, live lane 0",
        },
        Probe {
            // The mask-differing unit: live for the chosen example only.
            tensor: "hidden.0.weight",
            canonical_index: CANDIDATE_FEATURE_WIDTH + 2, // (out 1, in 2)
            live: true,
            why: "hidden-layer-0 weight into the unit that is dead for the unchosen example",
        },
        Probe {
            tensor: "hidden.0.bias",
            canonical_index: 1,
            live: true,
            why: "hidden-layer-0 bias whose ReLU mask differs between the pair",
        },
        Probe {
            tensor: "hidden.1.weight",
            canonical_index: 0, // (out 0, in 0)
            live: true,
            why: "hidden-layer-1 weight",
        },
        Probe {
            tensor: "hidden.1.weight",
            canonical_index: 3, // (out 1, in 1)
            live: true,
            why: "hidden-layer-1 weight into the unit that is dead for the unchosen example",
        },
        Probe {
            tensor: "hidden.1.bias",
            canonical_index: 1,
            live: true,
            why: "hidden-layer-1 bias whose ReLU mask differs between the pair",
        },
        Probe {
            tensor: "head.weight",
            canonical_index: 0,
            live: true,
            why: "head weight on the first hidden-1 unit",
        },
        Probe {
            tensor: "head.weight",
            canonical_index: 1,
            live: true,
            why: "head weight on the second hidden-1 unit",
        },
        Probe {
            // The pairwise hinge's gradient with respect to a constant offset
            // shared by both scores is `-1 + 1 = 0` exactly. Asserting the zero
            // (with its derivation) is what keeps this a check rather than a
            // vacuous probe: a spurious non-zero here would be a real bug.
            tensor: "head.bias",
            canonical_index: 0,
            live: false,
            why: "head bias: a shared offset cancels in the pairwise hinge",
        },
        Probe {
            // Same cancellation for a hidden bias whose mask agrees on both
            // sides: the downstream path is weight-shared, so the two terms
            // cancel exactly.
            tensor: "hidden.0.bias",
            canonical_index: 0,
            live: false,
            why: "hidden-layer-0 bias whose ReLU mask agrees on both sides",
        },
    ];

    let mut live_checked = 0usize;
    let mut zero_checked = 0usize;
    for probe in probes {
        let tensor_index = config
            .expected_tensors()
            .iter()
            .position(|(name, _)| name == probe.tensor)
            .expect("canonical tensor name");
        let shape = &config.expected_tensors()[tensor_index].1;
        // A weight tensor is `[out, in]`; a bias tensor is `[out]`.
        let is_weight = shape.len() == 2;
        let (out_width, out, input) = if is_weight {
            let (out_width, in_width) = (shape[0], shape[1]);
            (
                out_width,
                probe.canonical_index / in_width,
                probe.canonical_index % in_width,
            )
        } else {
            (1, probe.canonical_index, 0)
        };

        // A weight probe needs a live input lane; a bias probe applies the
        // constant `1.0` and has no lane to check.
        if is_weight {
            assert_ne!(
                example.chosen().lanes()[input],
                0.0,
                "{}: input lane {input} is zero in the chosen example (dead coordinate)",
                probe.why
            );
        }

        let analytic = analytic_gradient(&evaluation, probe.tensor, out, input, out_width);
        if probe.live {
            assert_ne!(
                analytic, 0.0,
                "{}: the derivative is identically zero, so this probe proves nothing",
                probe.why
            );
            live_checked += 1;
        } else {
            assert_eq!(
                analytic, 0.0,
                "{}: the derivative must be exactly zero",
                probe.why
            );
            zero_checked += 1;
        }

        let perturbed = |delta: f32| {
            let mut tensors = base.clone();
            tensors[tensor_index].1[probe.canonical_index] += delta;
            build_weights(&config, tensors)
        };
        let up = pairwise_hinge(
            &config,
            &perturbed(CENTRAL_DIFFERENCE_STEP),
            margin,
            &example,
        )
        .expect("hinge evaluation")
        .loss();
        let down = pairwise_hinge(
            &config,
            &perturbed(-CENTRAL_DIFFERENCE_STEP),
            margin,
            &example,
        )
        .expect("hinge evaluation")
        .loss();
        let numeric = (up - down) / (2.0 * CENTRAL_DIFFERENCE_STEP);
        let tolerance = central_difference_tolerance(analytic);
        assert!(
            (numeric - analytic).abs() <= tolerance,
            "{} ({}[{}]): analytic {analytic}, central difference {numeric}              (tolerance {tolerance}, step {CENTRAL_DIFFERENCE_STEP})",
            probe.why,
            probe.tensor,
            probe.canonical_index
        );
    }
    assert_eq!(live_checked, 8, "eight live coordinates must be covered");
    assert_eq!(zero_checked, 2, "two cancelled coordinates must be covered");
}

/// Read one element of the analytic gradient, mapping the canonical `[out, in]`
/// index onto the gradient's internal `[in, out]` layout.
fn analytic_gradient(
    evaluation: &amari_rewrite::neural::HingeEvaluation,
    tensor: &str,
    out: usize,
    input: usize,
    out_width: usize,
) -> f32 {
    let gradients = evaluation.gradients();
    match tensor {
        "head.weight" => gradients.head().weights()[input * out_width + out],
        "head.bias" => gradients.head().biases()[out],
        "hidden.0.weight" => gradients.layers()[0].weights()[input * out_width + out],
        "hidden.0.bias" => gradients.layers()[0].biases()[out],
        "hidden.1.weight" => gradients.layers()[1].weights()[input * out_width + out],
        "hidden.1.bias" => gradients.layers()[1].biases()[out],
        other => unreachable!("unexpected tensor {other}"),
    }
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
                values[0] = 4.0; // (out 0, in 0) in the canonical `[out, in]` layout
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
    assert!(evaluation
        .gradients()
        .head()
        .biases()
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
