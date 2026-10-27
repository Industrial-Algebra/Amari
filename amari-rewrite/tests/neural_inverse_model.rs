//! Frozen feature schema, structural encoder, and bounded MLP ranker contract
//! tests (0.25 Cohort 8, Task 40).
//!
//! Every expectation here is hand-computed from the documented schema: the
//! encoder lanes are pinned by name, the schema digest is pinned by value,
//! and the forward pass is checked against scalar arithmetic on a two-unit
//! hidden layer, so the MatMul + Add + ReLU composition is not
//! self-referential.

#![cfg(feature = "neural")]

use amari_rewrite::inverse::{BackwardProvenance, SymbolicState};
use amari_rewrite::neural::{
    CandidateInput, DifferentiableRule, FeatureEncoder, FeatureSchemaV1, FeatureVector, MlpRanker,
    NeuralError, RankedCandidate, RankerConfig, RankerWeights, SearchContext,
    CANDIDATE_FEATURE_WIDTH, MAX_RANKED_CANDIDATES,
};
use amari_rewrite::relation::{
    ConstraintSet, RelationLimits, RuleId, Sha256Digest, TermConstraint,
};
use amari_rewrite::trs::{Rule, Term};
use amari_rewrite::Path;

// ── fixtures ────────────────────────────────────────────────────────

fn rule_id() -> RuleId {
    let rule = Rule::new(
        Term::sym("f", [Term::var("x"), Term::constant("a")]),
        Term::constant("a"),
    )
    .expect("checked rule");
    RuleId::from_rule(&rule)
}

/// Repeated occurrences of `f` and `g`, so the arity/spine lanes are
/// non-degenerate.
fn term6() -> Term {
    // nodes: f | g | a | b | x | x = 6, depth 3, 4 leaves
    Term::sym(
        "f",
        [
            Term::sym("g", [Term::constant("a"), Term::constant("b")]),
            Term::var("x"),
            Term::var("x"),
        ],
    )
}

fn step_provenance(term: &Term, position: Path) -> BackwardProvenance {
    BackwardProvenance {
        rule_id: rule_id(),
        position,
        scope: 0,
        target_hash: Sha256Digest::canonical_term("amari.relation.term/v1", term),
        predecessor_hash: Sha256Digest::canonical_term("amari.relation.term/v1", term),
    }
}

fn context() -> SearchContext {
    // depth 2, branch factor 3, cyclic, term growth +1, no constraint
    // growth, one existential, rule priority 0.
    SearchContext::new(2, 3, true, 1, 0, 1, 0)
}

fn candidate<'a>(
    term: &'a Term,
    constraints: &'a ConstraintSet,
    provenance: &'a BackwardProvenance,
) -> CandidateInput<'a> {
    CandidateInput::new(term, constraints, provenance, context())
}

fn feature(features: &FeatureVector, name: &str) -> f32 {
    let index = FeatureSchemaV1::lane_index(name).expect("documented lane name");
    features.lanes()[index]
}

fn assert_feature(features: &FeatureVector, name: &str, expected: f32) {
    let actual = feature(features, name);
    assert!(
        (actual - expected).abs() < 1e-6,
        "lane {name}: expected {expected}, got {actual}"
    );
}

fn spec_json(input_width: u32, hidden: &[u32], activation: &str) -> String {
    let hidden = hidden
        .iter()
        .map(|width| format!("{{\"width\":{width}}}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"model_type\":\"ia-mlp\",\"schema_version\":1,\"input_width\":{input_width},\
         \"hidden\":[{hidden}],\"head\":{{\"width\":1}},\
         \"hidden_activation\":\"{activation}\"}}"
    )
}

fn config(hidden: &[u32]) -> RankerConfig {
    RankerConfig::from_spec_json(&spec_json(RankerConfig::FEATURE_WIDTH, hidden, "relu"))
        .expect("valid ranker spec")
}

/// Zero-filled tensors in canonical parameter order.
fn zero_tensors(config: &RankerConfig) -> Vec<(String, Vec<f32>)> {
    config
        .expected_tensors()
        .into_iter()
        .map(|(name, shape)| {
            let elements: usize = shape.iter().product();
            (name, vec![0.0f32; elements])
        })
        .collect()
}

fn borrow_tensors(tensors: &[(String, Vec<f32>)]) -> Vec<(&str, &[f32])> {
    tensors
        .iter()
        .map(|(name, values)| (name.as_str(), values.as_slice()))
        .collect()
}

fn weights(config: &RankerConfig, tensors: Vec<(String, Vec<f32>)>) -> RankerWeights {
    RankerWeights::from_tensors(config, &borrow_tensors(&tensors)).expect("valid weights")
}

fn ranker(config: &RankerConfig, tensors: Vec<(String, Vec<f32>)>) -> MlpRanker {
    MlpRanker::new(config.clone(), weights(config, tensors)).expect("matching weights")
}

/// A ranker whose score is a constant: every weight and bias is zero, so
/// every candidate scores `0.0` and only canonical order can break the tie.
fn constant_ranker(hidden: &[u32]) -> MlpRanker {
    let config = config(hidden);
    let tensors = zero_tensors(&config);
    ranker(&config, tensors)
}

/// The hand-computed golden ranker: hidden width 2, `h = relu([x0, -x0])`,
/// `score = h0 + 2*h1 + 0.5`.
fn golden_ranker() -> MlpRanker {
    let config = config(&[2]);
    let mut tensors = zero_tensors(&config);
    let width = RankerConfig::FEATURE_WIDTH as usize;
    for (name, values) in &mut tensors {
        match name.as_str() {
            "hidden.0.weight" => {
                // [out, in] row-major.
                values[0] = 1.0;
                values[width] = -1.0;
            }
            "head.weight" => {
                values[0] = 1.0;
                values[1] = 2.0;
            }
            "head.bias" => values[0] = 0.5,
            _ => {}
        }
    }
    ranker(&config, tensors)
}

/// `f(f(a,b),g(c,d),e)` — exactly 8 nodes, so `term.nodes` (the bit-length
/// bucket) is `4 / 16 = 0.25`.
fn term8() -> Term {
    Term::sym(
        "f",
        [
            Term::sym("f", [Term::constant("a"), Term::constant("b")]),
            Term::sym("g", [Term::constant("c"), Term::constant("d")]),
            Term::constant("e"),
        ],
    )
}

fn three_candidates() -> (Term, Term, Term) {
    (
        Term::sym("f", [Term::constant("a"), Term::constant("b")]),
        Term::sym("g", [Term::constant("c"), Term::constant("d")]),
        Term::sym("h", [Term::constant("e"), Term::constant("f")]),
    )
}

// ── frozen schema ──────────────────────────────────────────────────

#[test]
fn feature_schema_is_frozen_with_a_pinned_digest() {
    let schema = FeatureSchemaV1;
    assert_eq!(schema.version(), 1);
    assert_eq!(schema.width(), CANDIDATE_FEATURE_WIDTH);
    assert_eq!(CANDIDATE_FEATURE_WIDTH, 64);
    assert_eq!(FeatureSchemaV1::LANES.len(), CANDIDATE_FEATURE_WIDTH);
    // Any lane rename, reorder, addition, or removal changes the digest, so
    // a schema change cannot land silently: this value must be bumped
    // deliberately (and every checkpoint comparison in Task 43 with it).
    assert_eq!(
        schema.digest().to_hex(),
        FeatureSchemaV1::PINNED_DIGEST,
        "the frozen schema digest moved: bump the schema (and the pinned \
         value) deliberately, never to make a test pass"
    );
}

#[test]
fn feature_schema_lane_names_are_unique_and_indexable() {
    let mut seen = std::collections::BTreeSet::new();
    for (index, lane) in FeatureSchemaV1::LANES.iter().enumerate() {
        assert!(seen.insert(lane.name), "duplicate lane name {}", lane.name);
        assert_eq!(FeatureSchemaV1::lane_index(lane.name), Some(index));
        assert_eq!(FeatureSchemaV1::lane(index), Some(*lane));
        assert!(
            !lane.formula.is_empty(),
            "lane {} has no formula",
            lane.name
        );
    }
    assert_eq!(FeatureSchemaV1::lane_index("term.depth"), Some(1));
    assert_eq!(FeatureSchemaV1::lane_index("term.depth"), Some(1));
    assert_eq!(FeatureSchemaV1::lane_index("position.leftmost"), Some(63));
    assert_eq!(FeatureSchemaV1::lane_index("term.nonexistent"), None);
}

// ── encoder: term, constraint, provenance ──────────────────────────

#[test]
fn encoding_covers_term_shape_lanes() {
    let term = term6();
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&term, Path::from([1, 0]));
    let features = FeatureEncoder::default()
        .encode(&candidate(&term, &constraints, &provenance))
        .expect("total encoding");

    // 6 nodes -> bit length 3; depth 3; 4 leaves (a, b, x, x).
    assert_feature(&features, "term.nodes", 3.0 / 16.0);
    assert_feature(&features, "term.depth", 3.0 / 64.0);
    assert_feature(&features, "term.leaf_ratio", 4.0 / 6.0);
    // max arity 3, symbol nodes f/g/a/b = 4, arity sum 3 + 2 = 5.
    assert_feature(&features, "term.max_arity", 3.0 / 16.0);
    assert_feature(&features, "term.mean_symbol_arity", 1.25 / 16.0);
    // leftmost path f -> g -> a is 3 nodes.
    assert_feature(&features, "term.left_spine", 3.0 / 64.0);
    assert_feature(&features, "term.branching_ratio", 2.0 / 4.0);
    assert_feature(&features, "term.balance", 3.0 / 6.0);
    // Arity histogram over the symbol nodes.
    assert_feature(&features, "term.arity_0_ratio", 2.0 / 4.0);
    assert_feature(&features, "term.arity_1_ratio", 0.0);
    assert_feature(&features, "term.arity_2_ratio", 1.0 / 4.0);
    assert_feature(&features, "term.arity_3_ratio", 1.0 / 4.0);
    assert_feature(&features, "term.arity_7_plus_ratio", 0.0);
}

#[test]
fn encoding_covers_variable_lanes() {
    let term = term6();
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&term, Path::from([1]));
    let features = FeatureEncoder::default()
        .encode(&candidate(&term, &constraints, &provenance))
        .expect("total encoding");

    // One distinct variable, two occurrences, both at node depth 2.
    assert_feature(&features, "vars.distinct_ratio", 1.0 / 6.0);
    assert_feature(&features, "vars.occurrence_ratio", 2.0 / 6.0);
    assert_feature(&features, "vars.repeat_ratio", 0.5);
    assert_feature(&features, "vars.ground", 0.0);
    assert_feature(&features, "vars.mean_depth", 2.0 / 64.0);
    assert_feature(&features, "vars.max_depth", 2.0 / 64.0);
    // Four distinct single-character symbols.
    assert_feature(&features, "symbols.distinct_ratio", 1.0);
    assert_feature(&features, "symbols.name_length_mean", 1.0 / 32.0);
    assert_feature(&features, "vars.depth_2_ratio", 1.0);
    assert_feature(&features, "vars.depth_0_ratio", 0.0);

    let ground = Term::sym("f", [Term::constant("a")]);
    let ground_provenance = step_provenance(&ground, Path::root());
    let ground_features = FeatureEncoder::default()
        .encode(&candidate(&ground, &constraints, &ground_provenance))
        .expect("total encoding");
    assert_feature(&ground_features, "vars.ground", 1.0);
    assert_feature(&ground_features, "vars.repeat_ratio", 0.0);
}

#[test]
fn encoding_covers_constraint_lanes() {
    let term = term6();
    let mut constraints = ConstraintSet::new();
    constraints.insert(TermConstraint::Equal(Term::var("x"), Term::constant("a")));
    constraints.insert(TermConstraint::NotEqual(
        Term::sym("f", [Term::constant("a")]),
        Term::var("b"),
    ));
    let provenance = step_provenance(&term, Path::from([1]));
    let features = FeatureEncoder::default()
        .encode(&candidate(&term, &constraints, &provenance))
        .expect("total encoding");

    // Two constraints -> bit length 2.
    assert_feature(&features, "constraints.count", 2.0 / 16.0);
    assert_feature(&features, "constraints.density", 2.0 / 4096.0);
    assert_feature(&features, "constraints.equal_ratio", 0.5);
    assert_feature(&features, "constraints.not_equal_ratio", 0.5);
    // Side nodes: x, a | f(a), b = 5; state nodes 6 + 5 = 11.
    assert_feature(&features, "constraints.node_ratio", 5.0 / 11.0);
    assert_feature(&features, "constraints.side_depth_max", 2.0 / 64.0);
    // Symbol nodes a | f, a = 3 of 5; variable nodes x | b = 2 of 5.
    assert_feature(&features, "constraints.symbol_ratio", 3.0 / 5.0);
    assert_feature(&features, "constraints.var_ratio", 2.0 / 5.0);
    // Both constraints are one-sided-variable ("disjoint") classes.
    assert_feature(&features, "constraints.equal_ground_ratio", 0.0);
    assert_feature(&features, "constraints.equal_shared_ratio", 0.0);
    assert_feature(&features, "constraints.equal_disjoint_ratio", 0.5);
    assert_feature(&features, "constraints.not_equal_disjoint_ratio", 0.5);
    // 2 constraint variable occurrences of 4 total; one shared name (x).
    assert_feature(&features, "constraints.var_share", 0.5);
    assert_feature(&features, "constraints.term_var_overlap", 0.5);
}

#[test]
fn encoding_covers_provenance_and_position_lanes() {
    let term = term6();
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&term, Path::from([1, 0]));
    let features = FeatureEncoder::default()
        .encode(&candidate(&term, &constraints, &provenance))
        .expect("total encoding");

    // Search context: depth 2, branch 3, cycle, growth +1, 1 existential.
    assert_feature(&features, "search.depth", 2.0 / 64.0);
    assert_feature(&features, "search.branch_factor", 2.0 / 17.0);
    assert_feature(&features, "search.cycle", 1.0);
    assert_feature(&features, "search.term_growth", 4097.0 / 8192.0);
    assert_feature(&features, "search.constraint_growth", 0.0);
    assert_feature(&features, "search.existentials", 1.0 / 4096.0);
    assert_feature(&features, "search.rule_priority", 0.0);
    assert_feature(&features, "search.scope", 0.0);
    // Position [1, 0]: depth 2, last index 0, mean index 0.5, not root,
    // not leftmost; the deepest resolvable node is the variable `x`.
    assert_feature(&features, "position.depth", 2.0 / 64.0);
    assert_feature(&features, "position.last_index", 0.0);
    assert_feature(&features, "position.root", 0.0);
    assert_feature(&features, "position.subterm_nodes", 1.0 / 6.0);
    assert_feature(&features, "position.subterm_depth", 1.0 / 64.0);
    assert_feature(&features, "position.subterm_arity", 0.0);
    assert_feature(&features, "position.mean_index", 0.5 / 16.0);
    assert_feature(&features, "position.leftmost", 0.0);

    let root = step_provenance(&term, Path::root());
    let root_features = FeatureEncoder::default()
        .encode(&candidate(&term, &constraints, &root))
        .expect("total encoding");
    assert_feature(&root_features, "position.root", 1.0);
    assert_feature(&root_features, "position.subterm_nodes", 1.0);
    assert_feature(&root_features, "position.subterm_depth", 3.0 / 64.0);
    assert_feature(&root_features, "position.subterm_arity", 3.0 / 16.0);

    let leftmost = step_provenance(&term, Path::from([0, 0]));
    let leftmost_features = FeatureEncoder::default()
        .encode(&candidate(&term, &constraints, &leftmost))
        .expect("total encoding");
    assert_feature(&leftmost_features, "position.leftmost", 1.0);
}

#[test]
fn every_lane_stays_within_the_documented_unit_range() {
    let term = term6();
    let mut constraints = ConstraintSet::new();
    constraints.insert(TermConstraint::Equal(Term::var("x"), Term::var("y")));
    constraints.insert(TermConstraint::NotEqual(
        Term::constant("a"),
        Term::constant("b"),
    ));
    let provenance = step_provenance(&term, Path::from([0, 1]));
    let features = FeatureEncoder::default()
        .encode(&candidate(&term, &constraints, &provenance))
        .expect("total encoding");

    for (index, value) in features.lanes().iter().enumerate() {
        assert!(
            value.is_finite() && (0.0..=1.0).contains(value),
            "lane {} ({}) out of range: {value}",
            index,
            FeatureSchemaV1::LANES[index].name
        );
    }
}

#[test]
fn a_wide_symbol_saturates_lanes_instead_of_failing() {
    // Arity 64 is legal at the term level and far past every arity lane's
    // scale, so the lanes must saturate rather than leave [0, 1].
    let wide = Term::sym(
        "f",
        (0..64).map(|index| Term::constant(format!("c{index}"))),
    );
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&wide, Path::root());
    let features = FeatureEncoder::default()
        .encode(&candidate(&wide, &constraints, &provenance))
        .expect("a wide term still encodes");
    // max_arity saturates at its 16 scale; the mean arity (64 / 65 symbol
    // nodes) is reported exactly, not saturated.
    assert_feature(&features, "term.max_arity", 1.0);
    assert_feature(&features, "term.mean_symbol_arity", (64.0 / 65.0) / 16.0);
    assert_feature(&features, "term.arity_7_plus_ratio", 1.0 / 65.0);
    assert_feature(&features, "term.arity_0_ratio", 64.0 / 65.0);
    for lane in features.lanes() {
        assert!(lane.is_finite() && (0.0..=1.0).contains(lane));
    }
}

#[test]
fn encoding_is_alpha_invariant_and_ignores_caller_variable_names() {
    let left = Term::sym("f", [Term::var("x"), Term::var("x")]);
    let right = Term::sym("f", [Term::var("q"), Term::var("q")]);
    let no_constraints = ConstraintSet::new();
    let left_provenance = step_provenance(&left, Path::from([0]));
    let right_provenance = step_provenance(&right, Path::from([0]));
    let encoder = FeatureEncoder::default();
    let left_features = encoder
        .encode(&candidate(&left, &no_constraints, &left_provenance))
        .expect("total encoding");
    let right_features = encoder
        .encode(&candidate(&right, &no_constraints, &right_provenance))
        .expect("total encoding");
    assert_eq!(left_features, right_features);
}

#[test]
fn encoding_is_total_when_the_recorded_position_does_not_resolve() {
    // A constant has no children: the descent stops at the last resolvable
    // node instead of panicking or erroring.
    let term = Term::constant("a");
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&term, Path::from([7, 3, 1]));
    let features = FeatureEncoder::default()
        .encode(&candidate(&term, &constraints, &provenance))
        .expect("total encoding");
    assert_feature(&features, "position.depth", 3.0 / 64.0);
    assert_feature(&features, "position.subterm_nodes", 1.0);
    assert_feature(&features, "position.subterm_depth", 1.0 / 64.0);
    assert_feature(&features, "position.last_index", 1.0 / 16.0);
}

#[test]
fn feature_vector_rejects_nonfinite_and_out_of_range_lanes() {
    let mut lanes = [0.0f32; CANDIDATE_FEATURE_WIDTH];
    assert!(FeatureVector::new(lanes).is_ok());
    lanes[3] = f32::NAN;
    assert!(matches!(
        FeatureVector::new(lanes),
        Err(NeuralError::InvalidFeatures { .. })
    ));
    lanes[3] = f32::INFINITY;
    assert!(matches!(
        FeatureVector::new(lanes),
        Err(NeuralError::InvalidFeatures { .. })
    ));
    lanes[3] = 1.5;
    assert!(matches!(
        FeatureVector::new(lanes),
        Err(NeuralError::InvalidFeatures { .. })
    ));
    lanes[3] = -0.5;
    assert!(matches!(
        FeatureVector::new(lanes),
        Err(NeuralError::InvalidFeatures { .. })
    ));
}

// ── encoder bounds and metering ────────────────────────────────────

#[test]
fn encoder_enforces_term_node_ceiling() {
    let term = term8();
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&term, Path::root());
    let limits = RelationLimits::new(7, 64, 4_096, 1_000_000).expect("tightened limits");
    let encoder = FeatureEncoder::new(limits);
    match encoder.encode(&candidate(&term, &constraints, &provenance)) {
        Err(NeuralError::LimitExceeded {
            resource,
            value,
            limit,
        }) => {
            assert_eq!(resource, "term nodes");
            assert_eq!(value, 8);
            assert_eq!(limit, 7);
        }
        other => panic!("expected a typed node-limit error, got {other:?}"),
    }
}

#[test]
fn encoder_enforces_term_depth_ceiling() {
    // 65 nested symbols: depth 65, one node above the fixed depth ceiling.
    let mut term = Term::constant("a");
    for _ in 0..64 {
        term = Term::sym("f", [term]);
    }
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&term, Path::root());
    match FeatureEncoder::default().encode(&candidate(&term, &constraints, &provenance)) {
        Err(NeuralError::LimitExceeded {
            resource,
            value,
            limit,
        }) => {
            assert_eq!(resource, "term depth");
            assert_eq!(value, 65);
            assert_eq!(limit, RelationLimits::MAX_TERM_DEPTH);
        }
        other => panic!("expected a typed depth-limit error, got {other:?}"),
    }
}

#[test]
fn encoder_enforces_constraint_ceiling() {
    let term = term6();
    let mut constraints = ConstraintSet::new();
    constraints.insert(TermConstraint::Equal(Term::var("x"), Term::constant("a")));
    constraints.insert(TermConstraint::Equal(Term::var("x"), Term::constant("b")));
    let provenance = step_provenance(&term, Path::root());
    let limits = RelationLimits::new(4_096, 64, 1, 1_000_000).expect("tightened limits");
    let encoder = FeatureEncoder::new(limits);
    match encoder.encode(&candidate(&term, &constraints, &provenance)) {
        Err(NeuralError::LimitExceeded {
            resource,
            value,
            limit,
        }) => {
            assert_eq!(resource, "constraints");
            assert_eq!(value, 2);
            assert_eq!(limit, 1);
        }
        other => panic!("expected a typed constraint-limit error, got {other:?}"),
    }
}

#[test]
fn encoder_enforces_the_metered_operation_budget() {
    // The 7-node fixture cannot be encoded for fewer than 7 charges.
    let term = term6();
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&term, Path::root());
    let limits = RelationLimits::new(4_096, 64, 4_096, 4).expect("tightened limits");
    let encoder = FeatureEncoder::new(limits);
    match encoder.encode(&candidate(&term, &constraints, &provenance)) {
        Err(NeuralError::BudgetExhausted { resource, limit }) => {
            assert_eq!(resource, "operations");
            assert_eq!(limit, 4);
        }
        other => panic!("expected a typed budget error, got {other:?}"),
    }
}

#[test]
fn encoder_charges_the_recorded_path_length() {
    // The path is caller-supplied provenance, so walking it is metered
    // like every other traversal: 64 recorded steps cannot hide behind a
    // one-node term.
    let term = Term::constant("a");
    let constraints = ConstraintSet::new();
    let path = Path::from(vec![0usize; 64]);
    let provenance = step_provenance(&term, path);
    let limits = RelationLimits::new(4_096, 64, 4_096, 8).expect("tightened limits");
    let encoder = FeatureEncoder::new(limits);
    match encoder.encode(&candidate(&term, &constraints, &provenance)) {
        Err(NeuralError::BudgetExhausted { resource, limit }) => {
            assert_eq!(resource, "operations");
            assert_eq!(limit, 8);
        }
        other => panic!("expected a typed budget error, got {other:?}"),
    }
}

// ── ranker configuration and tensor shapes ─────────────────────────

#[test]
fn ranker_config_reports_the_shipped_tensor_shapes() {
    let config = config(&[RankerConfig::MAX_HIDDEN_WIDTH]);
    assert_eq!(config.spec().input_width, 64);
    assert_eq!(config.hidden_widths(), vec![256]);
    assert_eq!(
        config.expected_tensors(),
        vec![
            ("hidden.0.weight".to_string(), vec![256, 64]),
            ("hidden.0.bias".to_string(), vec![256]),
            ("head.weight".to_string(), vec![1, 256]),
            ("head.bias".to_string(), vec![1]),
        ]
    );
    assert_eq!(config.tensor_elements(), 256 * 64 + 256 + 256 + 1);
    assert!(config.tensor_elements() <= RankerConfig::MAX_TENSOR_ELEMENTS);
    assert_eq!(
        config.digest().to_hex(),
        RankerConfig::from_spec_json(&spec_json(64, &[256], "relu"))
            .expect("same spec")
            .digest()
            .to_hex()
    );

    // The shipped shapes execute: a zero weight set scores every candidate
    // 0.0 across a real 64 -> 256 -> 1 forward pass.
    let ranker = ranker(&config, zero_tensors(&config));
    let term = term8();
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&term, Path::root());
    let scored = ranker
        .score_candidate(
            &FeatureEncoder::default(),
            &candidate(&term, &constraints, &provenance),
        )
        .expect("the shipped architecture scores");
    assert_eq!(scored.score(), 0.0);
}

#[test]
fn ranker_config_rejects_a_non_schema_input_width() {
    let error = RankerConfig::from_spec_json(&spec_json(32, &[256], "relu"))
        .expect_err("input width must match the frozen schema");
    assert!(matches!(error, NeuralError::InvalidConfig { .. }));
    assert!(
        error.to_string().contains("input_width"),
        "unexpected message: {error}"
    );
}

#[test]
fn ranker_config_rejects_an_oversized_hidden_layer() {
    let error = RankerConfig::from_spec_json(&spec_json(64, &[512], "relu"))
        .expect_err("hidden width is ceilinged");
    assert!(matches!(error, NeuralError::InvalidConfig { .. }));
    assert!(
        error.to_string().contains("hidden"),
        "unexpected message: {error}"
    );
}

#[test]
fn ranker_config_rejects_a_tensor_budget_overflow() {
    let error = RankerConfig::from_spec_json(&spec_json(64, &[16_384, 16_384], "relu"))
        .expect_err("tensor element budget is ceilinged");
    assert!(matches!(error, NeuralError::InvalidConfig { .. }));
    assert!(
        error.to_string().contains("tensor elements"),
        "unexpected message: {error}"
    );
}

#[test]
fn ranker_config_rejects_unsupported_activations() {
    for activation in ["tanh", "gelu_erf", "gelu_tanh"] {
        let error = RankerConfig::from_spec_json(&spec_json(64, &[256], activation))
            .expect_err("only ReLU is implemented Amari-side");
        assert!(matches!(error, NeuralError::InvalidConfig { .. }));
        assert!(
            error.to_string().contains("hidden_activation"),
            "unexpected message: {error}"
        );
    }
}

#[test]
fn ranker_config_sanitizes_malformed_specs() {
    let error = RankerConfig::from_spec_json("{\"model_type\":\"ia-mlp\"}")
        .expect_err("an incomplete spec is rejected");
    assert!(matches!(error, NeuralError::InvalidConfig { .. }));
}

// ── weights: the validated construction seam ──────────────────────

#[test]
fn ranker_weights_reject_malformed_tensor_sets() {
    let config = config(&[2]);
    let width = RankerConfig::FEATURE_WIDTH as usize;

    let mut missing = zero_tensors(&config);
    missing.pop();
    assert!(matches!(
        RankerWeights::from_tensors(&config, &borrow_tensors(&missing)),
        Err(NeuralError::InvalidWeights { .. })
    ));

    let mut extra = zero_tensors(&config);
    extra.push(("hidden.9.weight".to_string(), vec![0.0; 2 * width]));
    assert!(matches!(
        RankerWeights::from_tensors(&config, &borrow_tensors(&extra)),
        Err(NeuralError::InvalidWeights { .. })
    ));

    let mut duplicate = zero_tensors(&config);
    let clone = duplicate[0].clone();
    duplicate.push(clone);
    assert!(matches!(
        RankerWeights::from_tensors(&config, &borrow_tensors(&duplicate)),
        Err(NeuralError::InvalidWeights { .. })
    ));

    let mut short = zero_tensors(&config);
    short[0].1.pop();
    assert!(matches!(
        RankerWeights::from_tensors(&config, &borrow_tensors(&short)),
        Err(NeuralError::InvalidWeights { .. })
    ));
}

#[test]
fn ranker_weights_reject_nonfinite_and_unbounded_values() {
    let config = config(&[2]);
    for poison in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut tensors = zero_tensors(&config);
        tensors[0].1[0] = poison;
        assert!(
            matches!(
                RankerWeights::from_tensors(&config, &borrow_tensors(&tensors)),
                Err(NeuralError::InvalidWeights { .. })
            ),
            "a {poison} weight must be rejected"
        );
    }
    let mut tensors = zero_tensors(&config);
    tensors[0].1[0] = 2.0 * RankerWeights::MAX_ABSOLUTE_WEIGHT;
    assert!(matches!(
        RankerWeights::from_tensors(&config, &borrow_tensors(&tensors)),
        Err(NeuralError::InvalidWeights { .. })
    ));
    tensors[0].1[0] = RankerWeights::MAX_ABSOLUTE_WEIGHT;
    assert!(RankerWeights::from_tensors(&config, &borrow_tensors(&tensors)).is_ok());
}

#[test]
fn ranker_weights_digest_is_stable_and_content_sensitive() {
    let config = config(&[2]);
    let tensors = zero_tensors(&config);
    let first = weights(&config, tensors.clone());
    let second = weights(&config, tensors.clone());
    assert_eq!(first.digest(), second.digest());
    assert_eq!(first.element_count(), 2 * 64 + 2 + 2 + 1);

    let mut changed = tensors;
    changed[0].1[0] = 0.25;
    let third = weights(&config, changed);
    assert_ne!(first.digest(), third.digest());
}

#[test]
fn ranker_rejects_a_config_its_weights_were_not_built_for() {
    let weights_config = config(&[2]);
    let weights = weights(&weights_config, zero_tensors(&weights_config));
    let other_config = config(&[3]);
    let error = MlpRanker::new(other_config, weights)
        .expect_err("weight shapes must match the configuration");
    assert!(matches!(error, NeuralError::InvalidWeights { .. }));
}

// ── forward pass, determinism, ties, ordering ─────────────────────

#[test]
fn ranker_forward_matches_hand_computed_scalar_arithmetic() {
    let ranker = golden_ranker();
    let term = term8();
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&term, Path::root());
    let encoder = FeatureEncoder::default();
    let features = encoder
        .encode(&candidate(&term, &constraints, &provenance))
        .expect("total encoding");
    assert_feature(&features, "term.nodes", 0.25);

    // h = relu([x0, -x0]) = [0.25, 0]; score = 0.25 + 2 * 0 + 0.5.
    let score = ranker
        .score_features(&features)
        .expect("finite feature vector");
    assert_eq!(score.to_bits(), 0.75f32.to_bits());
}

#[test]
fn ranker_scores_are_bit_deterministic_and_batch_invariant() {
    let ranker = golden_ranker();
    let encoder = FeatureEncoder::default();
    let (left, middle, right) = three_candidates();
    let constraints = ConstraintSet::new();
    let left_provenance = step_provenance(&left, Path::from([0]));
    let middle_provenance = step_provenance(&middle, Path::from([1]));
    let right_provenance = step_provenance(&right, Path::from([0, 1]));

    let single = ranker
        .score_candidate(&encoder, &candidate(&left, &constraints, &left_provenance))
        .expect("single score");

    let forward = ranker
        .rank(
            &encoder,
            &[
                candidate(&left, &constraints, &left_provenance),
                candidate(&middle, &constraints, &middle_provenance),
            ],
        )
        .expect("batch scores");
    let reused = forward
        .iter()
        .find(|ranked| ranked.canonical_digest() == single.canonical_digest())
        .expect("the single candidate appears in the batch");
    assert_eq!(reused.score().to_bits(), single.score().to_bits());
    assert_eq!(reused.features(), single.features());

    // Repeating the identical batch is bit-identical, order included.
    let repeat = ranker
        .rank(
            &encoder,
            &[
                candidate(&left, &constraints, &left_provenance),
                candidate(&middle, &constraints, &middle_provenance),
            ],
        )
        .expect("batch scores");
    assert_eq!(
        forward
            .iter()
            .map(RankedCandidate::score)
            .map(f32::to_bits)
            .collect::<Vec<_>>(),
        repeat
            .iter()
            .map(RankedCandidate::score)
            .map(f32::to_bits)
            .collect::<Vec<_>>()
    );

    // Candidate order in the request never changes the canonical result
    // order, because the batch is canonicalized before scoring.
    let reversed = ranker
        .rank(
            &encoder,
            &[
                candidate(&middle, &constraints, &middle_provenance),
                candidate(&right, &constraints, &right_provenance),
                candidate(&left, &constraints, &left_provenance),
            ],
        )
        .expect("batch scores");
    let shuffled = ranker
        .rank(
            &encoder,
            &[
                candidate(&right, &constraints, &right_provenance),
                candidate(&left, &constraints, &left_provenance),
                candidate(&middle, &constraints, &middle_provenance),
            ],
        )
        .expect("batch scores");
    assert_eq!(
        reversed
            .iter()
            .map(RankedCandidate::canonical_digest)
            .collect::<Vec<_>>(),
        shuffled
            .iter()
            .map(RankedCandidate::canonical_digest)
            .collect::<Vec<_>>()
    );
}

#[test]
fn ranker_breaks_ties_by_canonical_candidate_order() {
    // A constant scorer: every candidate scores exactly 0.0, so only the
    // canonical digest can order the result, and it must order it the same
    // way regardless of the request order.
    let ranker = constant_ranker(&[2]);
    let encoder = FeatureEncoder::default();
    let (left, middle, right) = three_candidates();
    let constraints = ConstraintSet::new();
    let left_provenance = step_provenance(&left, Path::from([0]));
    let middle_provenance = step_provenance(&middle, Path::from([0]));
    let right_provenance = step_provenance(&right, Path::from([0]));

    let forward = ranker
        .rank(
            &encoder,
            &[
                candidate(&left, &constraints, &left_provenance),
                candidate(&middle, &constraints, &middle_provenance),
                candidate(&right, &constraints, &right_provenance),
            ],
        )
        .expect("batch scores");
    let reversed = ranker
        .rank(
            &encoder,
            &[
                candidate(&right, &constraints, &right_provenance),
                candidate(&middle, &constraints, &middle_provenance),
                candidate(&left, &constraints, &left_provenance),
            ],
        )
        .expect("batch scores");

    assert!(forward.iter().all(|ranked| ranked.score() == 0.0));
    let mut sorted: Vec<Sha256Digest> = forward
        .iter()
        .map(RankedCandidate::canonical_digest)
        .collect();
    let observed = sorted.clone();
    sorted.sort();
    assert_eq!(observed, sorted, "ties must resolve by canonical digest");
    assert_eq!(
        observed,
        reversed
            .iter()
            .map(RankedCandidate::canonical_digest)
            .collect::<Vec<_>>()
    );
    // The digest is the joint alpha-canonical state identity, not a hash of
    // the request position.
    let expected: Vec<Sha256Digest> = [left.clone(), middle.clone(), right.clone()]
        .iter()
        .map(|term| SymbolicState::new(term.clone(), constraints.clone()).canonical_digest())
        .collect();
    let mut expected_sorted = expected;
    expected_sorted.sort();
    assert_eq!(observed, expected_sorted);
}

#[test]
fn ranker_rejects_a_batch_above_the_ceiling() {
    let ranker = constant_ranker(&[2]);
    let encoder = FeatureEncoder::default();
    let term = Term::constant("a");
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&term, Path::root());
    let batch = vec![candidate(&term, &constraints, &provenance); MAX_RANKED_CANDIDATES + 1];
    match ranker.rank(&encoder, &batch) {
        Err(NeuralError::LimitExceeded {
            resource,
            value,
            limit,
        }) => {
            assert_eq!(resource, "ranked candidates");
            assert_eq!(value, MAX_RANKED_CANDIDATES + 1);
            assert_eq!(limit, MAX_RANKED_CANDIDATES);
        }
        other => panic!("expected a typed batch-limit error, got {other:?}"),
    }
}

#[test]
fn ranker_rejects_an_oversized_term_before_canonicalizing_it() {
    // PR #353 round 1 P1a: the canonical digest is computed from a CLONED
    // term, and `Term::clone` recurses. Admission therefore has to happen
    // first: a 100,000-deep term must come back as a typed limit error
    // instead of aborting the process in `clone`.
    let mut deep = Term::constant("a");
    for _ in 0..100_000 {
        deep = Term::sym("f", [deep]);
    }
    let ranker = constant_ranker(&[2]);
    let encoder = FeatureEncoder::default();
    let constraints = ConstraintSet::new();
    let shallow = Term::constant("a");
    let provenance = step_provenance(&shallow, Path::root());

    let outcome = ranker.rank(&encoder, &[candidate(&deep, &constraints, &provenance)]);

    // The deep-drop discipline: dropping a 100,000-deep term recurses too,
    // so the fixture is leaked before any assertion can panic (an unwind
    // would drop it and abort the process, hiding the real failure).
    let observed = match &outcome {
        Err(NeuralError::LimitExceeded {
            resource,
            value,
            limit,
        }) => Some((*resource, *value, *limit)),
        Err(_) => None,
        Ok(_) => None,
    };
    std::mem::forget(deep);

    let (resource, value, limit) = observed.expect("expected a typed admission error, no abort");
    assert_eq!(resource, "term depth");
    assert_eq!(value, RelationLimits::MAX_TERM_DEPTH + 1);
    assert_eq!(limit, RelationLimits::MAX_TERM_DEPTH);
}

#[test]
fn ranker_derives_a_weight_bound_from_the_architecture() {
    // PR #353 round 1 P1b: a fixed 1e6 weight ceiling cannot keep eight
    // 256-wide layers finite (each layer multiplies the activation by
    // 256 x 1e6). The bound has to come from the architecture.
    let deep = config(&[256; 8]);
    let shipped = config(&[256]);
    assert!(
        deep.max_weight_magnitude() < RankerWeights::MAX_ABSOLUTE_WEIGHT,
        "an eight-layer ranker must derive a bound below the absolute ceiling"
    );
    assert_eq!(
        shipped.max_weight_magnitude(),
        RankerWeights::MAX_ABSOLUTE_WEIGHT,
        "the shipped architecture is bounded by the absolute ceiling"
    );

    // The derived bound is achievable: weights at exactly that magnitude
    // score finitely through eight layers.
    let bound = deep.max_weight_magnitude();
    let mut tensors = zero_tensors(&deep);
    for (_, values) in &mut tensors {
        values.fill(bound);
    }
    let ranker = ranker(&deep, tensors);
    let features = FeatureVector::new([1.0f32; CANDIDATE_FEATURE_WIDTH]).expect("in range");
    let score = ranker
        .score_features(&features)
        .expect("the derived bound keeps the forward pass finite");
    assert!(score.is_finite());

    // And the reproduction: 1e6 weights on that architecture must either
    // be rejected at validation or score finitely - never a backend
    // rejection mid-forward.
    let mut oversized = zero_tensors(&deep);
    for (_, values) in &mut oversized {
        values.fill(RankerWeights::MAX_ABSOLUTE_WEIGHT);
    }
    match RankerWeights::from_tensors(&deep, &borrow_tensors(&oversized)) {
        Err(NeuralError::InvalidWeights { message }) => {
            assert!(
                message.contains("magnitude"),
                "the rejection must name the magnitude: {message}"
            );
        }
        Ok(weights) => {
            let ranker = MlpRanker::new(deep.clone(), weights).expect("shapes agree");
            let score = ranker
                .score_features(&features)
                .expect("admitted weights must score finitely");
            assert!(score.is_finite());
        }
        Err(other) => panic!("expected a magnitude rejection or admission, got {other:?}"),
    }
}

#[test]
fn encoder_enforces_the_node_ceiling_on_constraint_sides() {
    // PR #353 round 1 P2a: a constraint side is a term, so it owes the same
    // node ceiling as the state term.
    let term = Term::constant("a");
    let mut constraints = ConstraintSet::new();
    constraints.insert(TermConstraint::Equal(
        Term::sym("f", [Term::constant("b"), Term::constant("c")]),
        Term::constant("d"),
    ));
    let provenance = step_provenance(&term, Path::root());
    let limits = RelationLimits::new(1, 64, 4_096, 1_000_000).expect("tightened limits");
    let encoder = FeatureEncoder::new(limits);
    match encoder.encode(&candidate(&term, &constraints, &provenance)) {
        Err(NeuralError::LimitExceeded {
            resource,
            value,
            limit,
        }) => {
            assert_eq!(resource, "term nodes");
            assert_eq!(limit, 1);
            assert!(value > limit, "the side node count must exceed the limit");
        }
        other => panic!("expected a typed side node-limit error, got {other:?}"),
    }
}

#[test]
fn ranker_ties_are_permutation_invariant_beyond_the_state_digest() {
    // PR #353 round 1 P2b: two candidates for the same state at different
    // search depths tie on both score and canonical digest, so the state
    // digest alone cannot order them - and request order must not either.
    let ranker = constant_ranker(&[2]);
    let encoder = FeatureEncoder::default();
    let term = Term::sym("f", [Term::constant("a")]);
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&term, Path::root());
    let at = |depth: u64| {
        CandidateInput::new(
            &term,
            &constraints,
            &provenance,
            SearchContext::new(depth, 3, false, 0, 0, 0, 0),
        )
    };
    let forward = ranker
        .rank(&encoder, &[at(3), at(1), at(2)])
        .expect("batch scores");
    let reversed = ranker
        .rank(&encoder, &[at(2), at(1), at(3)])
        .expect("batch scores");

    let depth_lane = FeatureSchemaV1::lane_index("search.depth").expect("documented lane");
    let depths = |ranked: &[RankedCandidate]| {
        ranked
            .iter()
            .map(|candidate| candidate.features().lanes()[depth_lane])
            .collect::<Vec<_>>()
    };
    assert!(forward.iter().all(|candidate| candidate.score() == 0.0));
    assert_eq!(depths(&forward), depths(&reversed));
    assert_eq!(
        depths(&forward),
        vec![1.0 / 64.0, 2.0 / 64.0, 3.0 / 64.0],
        "tied candidates order by their encoded content, not request order"
    );
}

#[test]
fn ranker_accepts_an_empty_batch() {
    let ranker = constant_ranker(&[2]);
    let encoder = FeatureEncoder::default();
    assert!(ranker
        .rank(&encoder, &[])
        .expect("an empty batch scores to nothing")
        .is_empty());
}

#[test]
fn ranker_keeps_scores_finite_at_the_weight_ceiling() {
    let config = config(&[2]);
    let mut tensors = zero_tensors(&config);
    for (name, values) in &mut tensors {
        if name.ends_with("weight") {
            values.fill(-RankerWeights::MAX_ABSOLUTE_WEIGHT);
        } else {
            values.fill(RankerWeights::MAX_ABSOLUTE_WEIGHT);
        }
    }
    let ranker = ranker(&config, tensors);
    let features = FeatureVector::new([1.0f32; CANDIDATE_FEATURE_WIDTH]).expect("in range");
    let score = ranker.score_features(&features).expect("finite score");
    assert!(score.is_finite(), "score must stay finite: {score}");
}

// ── backend error sanitization ────────────────────────────────────

#[test]
fn neural_errors_never_expose_backend_internals() {
    let config = config(&[2]);
    let mut tensors = zero_tensors(&config);
    tensors[0].1[0] = f32::NAN;
    let error = RankerWeights::from_tensors(&config, &borrow_tensors(&tensors))
        .expect_err("nonfinite weights are rejected");
    let rendered = error.to_string();
    for forbidden in [
        "TensorError",
        "borsalino",
        "Borsalino",
        "katakuri",
        "Katakuri",
    ] {
        assert!(
            !rendered.contains(forbidden),
            "error message leaks {forbidden}: {rendered}"
        );
    }

    let backend = NeuralError::BackendRejected {
        stage: "hidden matmul",
    };
    assert_eq!(
        backend.to_string(),
        "tensor backend rejected the hidden matmul step"
    );
}

// ── the preserved neural trait seam ───────────────────────────────

#[test]
fn differentiable_rule_trait_still_supports_arbitrary_states() {
    struct Doubling;

    impl DifferentiableRule<f64> for Doubling {
        type Parameters = ();
        type Gradient = ();
        type Error = NeuralError;

        fn forward(&self, state: &f64) -> Result<f64, Self::Error> {
            Ok(state * 2.0)
        }

        fn loss(&self, predicted: &f64, target: &f64) -> Result<f64, Self::Error> {
            Ok((predicted - target).abs())
        }
    }

    let rule = Doubling;
    assert_eq!(rule.forward(&2.0).expect("forward"), 4.0);
    assert_eq!(rule.loss(&4.0, &3.0).expect("loss"), 1.0);
}

#[test]
fn ranker_integrates_through_the_differentiable_rule_seam() {
    let ranker = golden_ranker();
    let term = term8();
    let constraints = ConstraintSet::new();
    let provenance = step_provenance(&term, Path::root());
    let features = FeatureEncoder::default()
        .encode(&candidate(&term, &constraints, &provenance))
        .expect("total encoding");

    let input = features.lanes().to_vec();
    let scored = <MlpRanker as DifferentiableRule<Vec<f32>>>::forward(&ranker, &input)
        .expect("forward scores the feature vector");
    assert_eq!(scored.len(), 1);
    assert_eq!(scored[0].to_bits(), 0.75f32.to_bits());

    let loss = <MlpRanker as DifferentiableRule<Vec<f32>>>::loss(&ranker, &scored, &vec![0.25])
        .expect("loss is finite");
    assert!((loss - 0.25).abs() < 1e-12);

    let nonfinite = vec![f32::NAN; CANDIDATE_FEATURE_WIDTH];
    assert!(matches!(
        <MlpRanker as DifferentiableRule<Vec<f32>>>::forward(&ranker, &nonfinite),
        Err(NeuralError::InvalidFeatures { .. })
    ));
    assert!(matches!(
        <MlpRanker as DifferentiableRule<Vec<f32>>>::forward(&ranker, &vec![0.0; 3]),
        Err(NeuralError::InvalidFeatures { .. })
    ));
    assert!(matches!(
        <MlpRanker as DifferentiableRule<Vec<f32>>>::loss(&ranker, &vec![1.0], &vec![1.0, 2.0]),
        Err(NeuralError::InvalidFeatures { .. })
    ));
}
