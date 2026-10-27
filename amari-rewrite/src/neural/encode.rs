// SPDX-License-Identifier: MIT OR Apache-2.0

//! Frozen candidate feature schema V1 and its structural encoder.
//!
//! The encoder turns one inverse candidate — a predecessor term, its
//! residual constraint set, and the provenance of the backward step that
//! produced it — into a fixed-width `f32` vector. Four properties are
//! contractual and each has a test:
//!
//! - **Frozen.** [`FeatureSchemaV1::LANES`] names every lane, in order,
//!   and [`FeatureSchemaV1::digest`] hashes that table. Renaming,
//!   reordering, adding, or removing a lane changes the digest, so a
//!   schema change can never land silently — and Task 43's checkpoint
//!   comparisons carry the digest alongside the weights.
//! - **Deterministic.** Every lane is built from counts, integer
//!   bit-lengths, and IEEE-754 multiply/divide. There is no transcendental
//!   function, no hash iteration, and no floating-point accumulation, so a
//!   lane value is bit-identical on every platform and every run.
//! - **Total.** Every term encodes: unknown or unresolvable positions stop
//!   the descent instead of failing, empty constraint sets encode as zeros,
//!   and every lane is clamped into its documented range before
//!   [`FeatureVector::new`] validates it.
//! - **Bounded and metered.** The input sizes are enforced against the
//!   caller-tightenable [`RelationLimits`] (term nodes, term depth,
//!   constraints) *and* charged against its operation budget, because the
//!   encoder is reachable on unvalidated search output. Constraint sides
//!   are terms and owe the same per-term node and depth ceilings as the
//!   state term — otherwise a constraint could smuggle an over-large term
//!   past admission (PR #353 round 1 P2a). The recorded step position is
//!   caller-supplied too, so walking it is metered as well.
//!
//! Encoding is also the module's admission gate: [`MlpRanker::rank`] and
//! [`MlpRanker::score_candidate`] run it before deriving the canonical
//! digest, because the digest clones the term and `Term::clone` recurses.
//!
//! ## The lane table (64 lanes, every range `[0, 1]`)
//!
//! Groups are contiguous and documented in [`FeatureSchemaV1::LANES`]:
//! `term.*` shape, `term.arity_*` histogram, `vars.*`/`symbols.*`
//! variable structure, `vars.depth_*` depth histogram, `constraints.*`
//! residual structure, `constraints.*_ratio` constraint classes,
//! `search.*` provenance/search context, and `position.*` step position.
//!
//! "Node depth" is 1-based across the module (a leaf has depth 1), which is
//! the same convention as [`crate::inverse`]'s term depth.

use crate::inverse::BackwardProvenance;
use crate::neural::NeuralError;
use crate::relation::{
    ConstraintSet, RelationLimits, RelationResources, Sha256Digest, TermConstraint,
};
use crate::trs::Term;

/// Width of the frozen candidate feature vector.
///
/// This is the `neural` row of the design's resource-authority table
/// (width 64) and the ranker specification's required `input_width`.
pub const CANDIDATE_FEATURE_WIDTH: usize = 64;

/// One frozen schema lane: its stable name and its formula.
///
/// Both halves are part of the schema digest, so the constant table below is
/// the schema's authority rather than a comment beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lane {
    /// The stable lane name (the identifier tests and checkpoints key on).
    pub name: &'static str,
    /// The lane's formula and range, in arithmetic.
    pub formula: &'static str,
}

impl Lane {
    /// A lane definition.
    pub const fn new(name: &'static str, formula: &'static str) -> Self {
        Self { name, formula }
    }
}

/// The frozen, versioned candidate feature schema.
///
/// A zero-sized marker: the schema is its lane table, not an instance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FeatureSchemaV1;

impl FeatureSchemaV1 {
    /// Schema version, bumped only by a deliberate incompatible change.
    pub const VERSION: u32 = 1;

    /// Vector width: one lane per entry of [`Self::LANES`].
    pub const WIDTH: usize = CANDIDATE_FEATURE_WIDTH;

    /// Digest frame for [`Self::digest`].
    pub const FRAME: &'static str = "amari.neural.feature-schema/v1";

    /// The pinned digest of the lane table below.
    ///
    /// Test-pinned: the constant is compared against the value derived
    /// from [`Self::LANES`], so a lane change fails the suite instead of
    /// silently invalidating every trained checkpoint.
    pub const PINNED_DIGEST: &'static str =
        "a872798187791e156eafd325ab9b7ea127f9ec049686c2c8740b663630650de0";

    /// Every lane, in index order: its stable name and its formula.
    ///
    /// The lane table *is* the schema. The digest covers names and
    /// formulas alike, so a scale change (not just a rename) is a
    /// deliberate, visible schema change.
    pub const LANES: [Lane; CANDIDATE_FEATURE_WIDTH] = [
        // Term shape (0-7).
        Lane::new("term.nodes", "bit_length(node count) / 16"),
        Lane::new("term.depth", "depth / 64"),
        Lane::new("term.leaf_ratio", "leaves / nodes"),
        Lane::new("term.max_arity", "min(max arity, 16) / 16"),
        Lane::new("term.mean_symbol_arity", "(arity sum / symbol nodes) / 16"),
        Lane::new("term.left_spine", "leftmost root-to-leaf length / 64"),
        Lane::new(
            "term.branching_ratio",
            "nodes with arity >= 2 / symbol nodes",
        ),
        Lane::new("term.balance", "depth / nodes"),
        // Symbol arity histogram (8-15).
        Lane::new("term.arity_0_ratio", "arity-0 symbol nodes / symbol nodes"),
        Lane::new("term.arity_1_ratio", "arity-1 symbol nodes / symbol nodes"),
        Lane::new("term.arity_2_ratio", "arity-2 symbol nodes / symbol nodes"),
        Lane::new("term.arity_3_ratio", "arity-3 symbol nodes / symbol nodes"),
        Lane::new("term.arity_4_ratio", "arity-4 symbol nodes / symbol nodes"),
        Lane::new("term.arity_5_ratio", "arity-5 symbol nodes / symbol nodes"),
        Lane::new("term.arity_6_ratio", "arity-6 symbol nodes / symbol nodes"),
        Lane::new(
            "term.arity_7_plus_ratio",
            "arity >= 7 symbol nodes / symbol nodes",
        ),
        // Variable structure (16-23).
        Lane::new("vars.distinct_ratio", "distinct variables / nodes"),
        Lane::new("vars.occurrence_ratio", "variable occurrences / nodes"),
        Lane::new(
            "vars.repeat_ratio",
            "(occurrences - distinct) / occurrences",
        ),
        Lane::new("vars.ground", "1 when no variable occurs"),
        Lane::new("vars.mean_depth", "mean variable depth / 64"),
        Lane::new("vars.max_depth", "max variable depth / 64"),
        Lane::new("symbols.distinct_ratio", "distinct symbols / symbol nodes"),
        Lane::new(
            "symbols.name_length_mean",
            "mean symbol name length / 32 (clamped)",
        ),
        // Variable depth histogram (24-31).
        Lane::new("vars.depth_0_ratio", "variables at depth 0 / occurrences"),
        Lane::new("vars.depth_1_ratio", "variables at depth 1 / occurrences"),
        Lane::new("vars.depth_2_ratio", "variables at depth 2 / occurrences"),
        Lane::new(
            "vars.depth_3_4_ratio",
            "variables at depth 3-4 / occurrences",
        ),
        Lane::new(
            "vars.depth_5_8_ratio",
            "variables at depth 5-8 / occurrences",
        ),
        Lane::new(
            "vars.depth_9_16_ratio",
            "variables at depth 9-16 / occurrences",
        ),
        Lane::new(
            "vars.depth_17_32_ratio",
            "variables at depth 17-32 / occurrences",
        ),
        Lane::new(
            "vars.depth_33_64_ratio",
            "variables at depth 33-64 / occurrences",
        ),
        // Residual constraints (32-39).
        Lane::new("constraints.count", "bit_length(constraint count) / 16"),
        Lane::new("constraints.density", "constraint count / 4096"),
        Lane::new("constraints.equal_ratio", "equal constraints / constraints"),
        Lane::new("constraints.not_equal_ratio", "disequalities / constraints"),
        Lane::new(
            "constraints.node_ratio",
            "constraint side nodes / all nodes",
        ),
        Lane::new("constraints.side_depth_max", "max side depth / 64"),
        Lane::new("constraints.symbol_ratio", "side symbol nodes / side nodes"),
        Lane::new("constraints.var_ratio", "side variable nodes / side nodes"),
        // Constraint classes (40-47).
        Lane::new(
            "constraints.equal_ground_ratio",
            "ground equalities / constraints",
        ),
        Lane::new(
            "constraints.equal_shared_ratio",
            "variable-sharing equalities / constraints",
        ),
        Lane::new(
            "constraints.equal_disjoint_ratio",
            "variable-disjoint equalities / constraints",
        ),
        Lane::new(
            "constraints.not_equal_ground_ratio",
            "ground disequalities / constraints",
        ),
        Lane::new(
            "constraints.not_equal_shared_ratio",
            "variable-sharing disequalities / constraints",
        ),
        Lane::new(
            "constraints.not_equal_disjoint_ratio",
            "variable-disjoint disequalities / constraints",
        ),
        Lane::new(
            "constraints.var_share",
            "side variable nodes / all variable nodes",
        ),
        Lane::new(
            "constraints.term_var_overlap",
            "shared variable names / distinct state variables",
        ),
        // Provenance and search context (48-55).
        Lane::new("search.depth", "backward search depth / 64"),
        Lane::new("search.branch_factor", "bit_length(branch factor) / 17"),
        Lane::new("search.cycle", "1 when the successor was already seen"),
        Lane::new("search.term_growth", "(clamped node growth + 4096) / 8192"),
        Lane::new(
            "search.constraint_growth",
            "clamped constraint growth / 4096",
        ),
        Lane::new("search.existentials", "existential count / 4096"),
        Lane::new("search.rule_priority", "bit_length(rule priority) / 17"),
        Lane::new("search.scope", "1 when the freshening scope is non-zero"),
        // Position of the rewritten step (56-63).
        Lane::new("position.depth", "recorded path length / 64"),
        Lane::new("position.last_index", "min(last index, 16) / 16"),
        Lane::new("position.root", "1 for the root path"),
        Lane::new(
            "position.subterm_nodes",
            "resolved subterm nodes / term nodes",
        ),
        Lane::new("position.subterm_depth", "resolved subterm depth / 64"),
        Lane::new(
            "position.subterm_arity",
            "min(resolved subterm arity, 16) / 16",
        ),
        Lane::new("position.mean_index", "mean path index / 16"),
        Lane::new(
            "position.leftmost",
            "1 when every recorded step takes child 0",
        ),
    ];

    /// The schema version.
    pub fn version(&self) -> u32 {
        Self::VERSION
    }

    /// The vector width.
    pub fn width(&self) -> usize {
        Self::WIDTH
    }

    /// The lane at `index`.
    pub fn lane(index: usize) -> Option<Lane> {
        Self::LANES.get(index).copied()
    }

    /// The index of a lane, if the name is part of the frozen schema.
    pub fn lane_index(name: &str) -> Option<usize> {
        Self::LANES.iter().position(|lane| lane.name == name)
    }

    /// The content digest of the lane table: frame, version, width, and
    /// every lane's index, name, and formula.
    ///
    /// A rename and a scale change are equally visible here: the digest is
    /// over the documented *formulas*, not just their labels.
    pub fn digest(&self) -> Sha256Digest {
        let mut payload = Vec::with_capacity(1024);
        payload.extend_from_slice(&Self::VERSION.to_le_bytes());
        payload.extend_from_slice(&(Self::WIDTH as u32).to_le_bytes());
        for (index, lane) in Self::LANES.iter().enumerate() {
            payload.extend_from_slice(&(index as u32).to_le_bytes());
            for text in [lane.name, lane.formula] {
                payload.extend_from_slice(&(text.len() as u32).to_le_bytes());
                payload.extend_from_slice(text.as_bytes());
            }
        }
        Sha256Digest::framed(Self::FRAME, &payload)
    }
}

/// A validated candidate feature vector.
///
/// Construction is the only way in: every lane must be finite and inside
/// `[0, 1]`, which is what makes "no non-finite feature can reach the
/// tensor backend" a property of the type rather than of its users.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeatureVector {
    lanes: [f32; CANDIDATE_FEATURE_WIDTH],
}

impl FeatureVector {
    /// Validate a lane array.
    ///
    /// # Errors
    ///
    /// [`NeuralError::InvalidFeatures`] when a lane is non-finite or
    /// outside the documented `[0, 1]` range.
    pub fn new(lanes: [f32; CANDIDATE_FEATURE_WIDTH]) -> Result<Self, NeuralError> {
        for lane in &lanes {
            if !lane.is_finite() {
                return Err(NeuralError::InvalidFeatures {
                    message: "feature lane is not finite",
                });
            }
            if !(0.0..=1.0).contains(lane) {
                return Err(NeuralError::InvalidFeatures {
                    message: "feature lane outside the documented [0, 1] range",
                });
            }
        }
        Ok(Self { lanes })
    }

    /// The lanes, in schema order.
    pub fn lanes(&self) -> &[f32; CANDIDATE_FEATURE_WIDTH] {
        &self.lanes
    }

    /// One lane by index.
    pub fn lane(&self, index: usize) -> Option<f32> {
        self.lanes.get(index).copied()
    }

    /// The vector width.
    pub fn len(&self) -> usize {
        CANDIDATE_FEATURE_WIDTH
    }

    /// Always false: a feature vector has a fixed, non-zero width.
    pub fn is_empty(&self) -> bool {
        false
    }
}

/// Search context the guidance layer already computes for a candidate.
///
/// These are the quantities [`crate::inverse::SymbolicScore`] ranks by;
/// they are supplied rather than recomputed so encoding cannot disagree
/// with the symbolic scorer. Every field is clamped by the encoder, so an
/// arbitrary caller value can only saturate a lane, never corrupt one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SearchContext {
    /// Backward search depth of the candidate.
    pub depth: u64,
    /// Number of valid candidates in the expansion round.
    pub branch_factor: u64,
    /// Whether the candidate's canonical state was already seen.
    pub cycle: bool,
    /// Node-count change relative to the parent state.
    pub term_growth: i64,
    /// Constraint-count change relative to the parent state.
    pub constraint_growth: u64,
    /// Existential logic variables introduced by the step.
    pub existentials: usize,
    /// Stable rule priority (the rule's index in the system).
    pub rule_priority: u64,
}

impl SearchContext {
    /// Construct a search context.
    pub fn new(
        depth: u64,
        branch_factor: u64,
        cycle: bool,
        term_growth: i64,
        constraint_growth: u64,
        existentials: usize,
        rule_priority: u64,
    ) -> Self {
        Self {
            depth,
            branch_factor,
            cycle,
            term_growth,
            constraint_growth,
            existentials,
            rule_priority,
        }
    }
}

/// Everything the frozen schema encodes for one candidate.
///
/// Borrowed on purpose: encoding must not clone a term or a constraint set
/// per candidate. The canonical identity of the candidate is derived from
/// the same inputs by the ranker
/// ([`crate::inverse::SymbolicState::canonical_digest`]).
#[derive(Clone, Copy, Debug)]
pub struct CandidateInput<'a> {
    /// The predecessor term.
    pub term: &'a Term,
    /// Residual constraints on the predecessor.
    pub constraints: &'a ConstraintSet,
    /// Provenance of the backward step that produced it.
    pub provenance: &'a BackwardProvenance,
    /// Search context supplied by the guidance layer.
    pub context: SearchContext,
}

impl<'a> CandidateInput<'a> {
    /// Borrow one candidate for encoding.
    pub fn new(
        term: &'a Term,
        constraints: &'a ConstraintSet,
        provenance: &'a BackwardProvenance,
        context: SearchContext,
    ) -> Self {
        Self {
            term,
            constraints,
            provenance,
            context,
        }
    }
}

/// The structural encoder: frozen schema V1 under caller-tightened limits.
#[derive(Clone, Copy, Debug)]
pub struct FeatureEncoder {
    limits: RelationLimits,
}

impl FeatureEncoder {
    /// Encode under caller-tightened relation limits.
    pub fn new(limits: RelationLimits) -> Self {
        Self { limits }
    }

    /// The limits this encoder enforces.
    pub fn limits(&self) -> &RelationLimits {
        &self.limits
    }

    /// Encode one candidate.
    ///
    /// # Errors
    ///
    /// [`NeuralError::LimitExceeded`] when the term, its depth, or the
    /// constraint count exceeds the applied limit;
    /// [`NeuralError::BudgetExhausted`] when the operation budget runs
    /// out while metering the traversal. Both are reachable on
    /// unvalidated search output, so neither is an assertion.
    pub fn encode(&self, candidate: &CandidateInput<'_>) -> Result<FeatureVector, NeuralError> {
        let mut resources = RelationResources::new(&self.limits);
        let mut term = self.term_stats(candidate.term, &mut resources)?;
        let constraints = self.constraint_stats(candidate.constraints, &mut resources)?;
        let position = self.position_stats(candidate.term, candidate.provenance, &mut resources)?;

        let mut lanes = [0.0f32; CANDIDATE_FEATURE_WIDTH];
        let distinct_vars = distinct(&mut term.var_names);
        let distinct_symbols = distinct(&mut term.symbol_names);
        let overlap = shared_names(&term.var_names, &constraints.var_names);
        let state_vars = union_count(&term.var_names, &constraints.var_names);

        // Term shape.
        lanes[0] = bit_bucket(term.nodes as u64, 16.0);
        lanes[1] = term.depth as f32 / 64.0;
        lanes[2] = ratio(term.leaves, term.nodes);
        lanes[3] = term.max_arity.min(16) as f32 / 16.0;
        lanes[4] = ratio(term.arity_sum, term.symbol_nodes) / 16.0;
        lanes[5] = term.left_spine as f32 / 64.0;
        lanes[6] = ratio(term.branching_nodes, term.symbol_nodes);
        lanes[7] = ratio(term.depth, term.nodes);

        // Symbol arity histogram.
        for bucket in 0..8 {
            lanes[8 + bucket] = ratio(term.arity_hist[bucket], term.symbol_nodes);
        }

        // Variable structure.
        lanes[16] = ratio(distinct_vars, term.nodes);
        lanes[17] = ratio(term.var_occurrences, term.nodes);
        lanes[18] = if term.var_occurrences == 0 {
            0.0
        } else {
            (term.var_occurrences - distinct_vars) as f32 / term.var_occurrences as f32
        };
        lanes[19] = if term.var_occurrences == 0 { 1.0 } else { 0.0 };
        lanes[20] = ratio(term.var_depth_sum, term.var_occurrences) / 64.0;
        lanes[21] = term.var_max_depth as f32 / 64.0;
        lanes[22] = ratio(distinct_symbols, term.symbol_nodes);
        lanes[23] = ratio(term.name_len_sum, term.symbol_nodes) / 32.0;

        // Variable depth histogram.
        for bucket in 0..8 {
            lanes[24 + bucket] = ratio(term.var_depth_hist[bucket], term.var_occurrences);
        }

        // Residual constraints.
        lanes[32] = bit_bucket(constraints.count as u64, 16.0);
        lanes[33] = constraints.count as f32 / 4096.0;
        lanes[34] = ratio(constraints.equal, constraints.count);
        lanes[35] = ratio(constraints.not_equal, constraints.count);
        lanes[36] = ratio(constraints.side_nodes, term.nodes + constraints.side_nodes);
        lanes[37] = constraints.side_depth_max as f32 / 64.0;
        lanes[38] = ratio(constraints.side_symbol_nodes, constraints.side_nodes);
        lanes[39] = ratio(constraints.side_var_occurrences, constraints.side_nodes);

        // Constraint classes.
        lanes[40] = ratio(constraints.equal_ground, constraints.count);
        lanes[41] = ratio(constraints.equal_shared, constraints.count);
        lanes[42] = ratio(constraints.equal_disjoint, constraints.count);
        lanes[43] = ratio(constraints.not_equal_ground, constraints.count);
        lanes[44] = ratio(constraints.not_equal_shared, constraints.count);
        lanes[45] = ratio(constraints.not_equal_disjoint, constraints.count);
        lanes[46] = ratio(
            constraints.side_var_occurrences,
            term.var_occurrences + constraints.side_var_occurrences,
        );
        lanes[47] = ratio(overlap, state_vars);

        // Provenance and search context.
        let context = candidate.context;
        lanes[48] = context.depth as f32 / 64.0;
        lanes[49] = bit_bucket(context.branch_factor, 17.0);
        lanes[50] = if context.cycle { 1.0 } else { 0.0 };
        lanes[51] = (context.term_growth.clamp(-4096, 4096) as f32 + 4096.0) / 8192.0;
        lanes[52] = context.constraint_growth as f32 / 4096.0;
        lanes[53] = context.existentials as f32 / 4096.0;
        lanes[54] = bit_bucket(context.rule_priority, 17.0);
        lanes[55] = if candidate.provenance.scope == 0 {
            0.0
        } else {
            1.0
        };

        // Position of the rewritten step.
        let path = candidate.provenance.position.as_slice();
        lanes[56] = path.len() as f32 / 64.0;
        lanes[57] = path.last().copied().unwrap_or(0).min(16) as f32 / 16.0;
        lanes[58] = if candidate.provenance.position.is_root() {
            1.0
        } else {
            0.0
        };
        lanes[59] = ratio(position.nodes, term.nodes);
        lanes[60] = position.depth as f32 / 64.0;
        lanes[61] = position.arity.min(16) as f32 / 16.0;
        lanes[62] = mean_index(path) / 16.0;
        lanes[63] = if path.iter().all(|&index| index == 0) {
            1.0
        } else {
            0.0
        };

        // Clamping is the totality guarantee: a wide symbol or a caller
        // context value far past its ceiling saturates a lane instead of
        // failing the encode, and no lane can leave [0, 1].
        for lane in &mut lanes {
            *lane = lane.clamp(0.0, 1.0);
        }
        FeatureVector::new(lanes)
    }

    /// Visit one term, enforcing the node/depth ceilings and charging the
    /// operation budget once per node.
    fn term_stats(
        &self,
        term: &Term,
        resources: &mut RelationResources,
    ) -> Result<TermStats, NeuralError> {
        let mut stats = TermStats {
            left_spine: left_spine_length(term, self.limits.max_term_depth()),
            ..TermStats::default()
        };
        let mut stack: Vec<(&Term, usize)> = Vec::from([(term, 1)]);
        while let Some((node, depth)) = stack.pop() {
            charge(resources, self.limits.max_operations())?;
            stats.nodes += 1;
            if stats.nodes > self.limits.max_term_nodes() {
                return Err(NeuralError::LimitExceeded {
                    resource: "term nodes",
                    value: stats.nodes,
                    limit: self.limits.max_term_nodes(),
                });
            }
            if depth > self.limits.max_term_depth() {
                return Err(NeuralError::LimitExceeded {
                    resource: "term depth",
                    value: depth,
                    limit: self.limits.max_term_depth(),
                });
            }
            stats.depth = stats.depth.max(depth);
            match node {
                Term::Var(variable) => {
                    stats.leaves += 1;
                    stats.var_occurrences += 1;
                    stats.var_depth_sum += depth;
                    stats.var_max_depth = stats.var_max_depth.max(depth);
                    stats.var_depth_hist[depth_bucket(depth)] += 1;
                    stats.var_names.push(variable.as_str().into());
                }
                Term::Sym(symbol, args) => {
                    stats.symbol_nodes += 1;
                    stats.arity_sum += args.len();
                    stats.max_arity = stats.max_arity.max(args.len());
                    stats.arity_hist[args.len().min(7)] += 1;
                    if args.len() >= 2 {
                        stats.branching_nodes += 1;
                    }
                    if args.is_empty() {
                        stats.leaves += 1;
                    }
                    stats.name_len_sum += symbol.as_str().len();
                    stats.symbol_names.push(symbol.as_str().into());
                    for arg in args.iter().rev() {
                        stack.push((arg, depth + 1));
                    }
                }
            }
        }
        Ok(stats)
    }

    /// Visit every constraint side, enforcing the constraint-count ceiling
    /// and charging the same operation budget per visited side node.
    fn constraint_stats(
        &self,
        constraints: &ConstraintSet,
        resources: &mut RelationResources,
    ) -> Result<ConstraintStats, NeuralError> {
        let mut stats = ConstraintStats::default();
        if constraints.len() > self.limits.max_constraints() {
            return Err(NeuralError::LimitExceeded {
                resource: "constraints",
                value: constraints.len(),
                limit: self.limits.max_constraints(),
            });
        }
        stats.count = constraints.len();
        for constraint in constraints.constraints() {
            let (tag, left, right) = match constraint {
                TermConstraint::Equal(left, right) => (Class::Equal, left, right),
                TermConstraint::NotEqual(left, right) => (Class::NotEqual, left, right),
            };
            let left_side = self.side_stats(left, resources)?;
            let right_side = self.side_stats(right, resources)?;
            let shared = shares_name(&left_side.var_names, &right_side.var_names);
            let ground = left_side.var_names.is_empty() && right_side.var_names.is_empty();
            stats.side_nodes += left_side.nodes + right_side.nodes;
            stats.side_symbol_nodes += left_side.symbol_nodes + right_side.symbol_nodes;
            stats.side_var_occurrences += left_side.var_names.len() + right_side.var_names.len();
            stats.side_depth_max = stats
                .side_depth_max
                .max(left_side.depth.max(right_side.depth));
            stats.var_names.extend(left_side.var_names.iter().cloned());
            stats.var_names.extend(right_side.var_names.iter().cloned());
            match (tag, ConstraintClass::of(ground, shared)) {
                (Class::Equal, ConstraintClass::Ground) => stats.equal_ground += 1,
                (Class::Equal, ConstraintClass::Shared) => stats.equal_shared += 1,
                (Class::Equal, ConstraintClass::Disjoint) => stats.equal_disjoint += 1,
                (Class::NotEqual, ConstraintClass::Ground) => stats.not_equal_ground += 1,
                (Class::NotEqual, ConstraintClass::Shared) => {
                    stats.not_equal_shared += 1;
                }
                (Class::NotEqual, ConstraintClass::Disjoint) => {
                    stats.not_equal_disjoint += 1;
                }
            }
            match tag {
                Class::Equal => stats.equal += 1,
                Class::NotEqual => stats.not_equal += 1,
            }
        }
        Ok(stats)
    }

    /// Count one constraint side's nodes, depth, symbols, and variables.
    ///
    /// A constraint side is a term, so it owes the same node and depth
    /// ceilings as the state term: an unbounded side would let a
    /// constraint smuggle an over-large term past admission (PR #353 round
    /// 1 P2a).
    fn side_stats(
        &self,
        side: &Term,
        resources: &mut RelationResources,
    ) -> Result<SideStats, NeuralError> {
        let mut stats = SideStats::default();
        let mut stack: Vec<(&Term, usize)> = Vec::from([(side, 1)]);
        while let Some((node, depth)) = stack.pop() {
            charge(resources, self.limits.max_operations())?;
            stats.nodes += 1;
            if stats.nodes > self.limits.max_term_nodes() {
                return Err(NeuralError::LimitExceeded {
                    resource: "term nodes",
                    value: stats.nodes,
                    limit: self.limits.max_term_nodes(),
                });
            }
            if depth > self.limits.max_term_depth() {
                return Err(NeuralError::LimitExceeded {
                    resource: "term depth",
                    value: depth,
                    limit: self.limits.max_term_depth(),
                });
            }
            stats.depth = stats.depth.max(depth);
            match node {
                Term::Var(variable) => stats.var_names.push(variable.as_str().into()),
                Term::Sym(_, args) => {
                    stats.symbol_nodes += 1;
                    for arg in args.iter().rev() {
                        stack.push((arg, depth + 1));
                    }
                }
            }
        }
        Ok(stats)
    }

    /// Statistics of the subterm the recorded position resolves to, and of
    /// the path itself. An unresolvable step stops the descent at the last
    /// node that did resolve — never an error, never a panic.
    fn position_stats(
        &self,
        term: &Term,
        provenance: &BackwardProvenance,
        resources: &mut RelationResources,
    ) -> Result<PositionStats, NeuralError> {
        let path = provenance.position.as_slice();
        charge_many(resources, path.len(), self.limits.max_operations())?;
        let mut node = term;
        for &index in path {
            let next = match node {
                Term::Var(_) => None,
                Term::Sym(_, args) => args.get(index),
            };
            match next {
                Some(child) => node = child,
                None => break,
            }
        }
        let arity = match node {
            Term::Var(_) => 0,
            Term::Sym(_, args) => args.len(),
        };
        let mut stats = PositionStats {
            arity,
            ..PositionStats::default()
        };
        let mut stack: Vec<(&Term, usize)> = Vec::from([(node, 1)]);
        while let Some((current, depth)) = stack.pop() {
            charge(resources, self.limits.max_operations())?;
            stats.nodes += 1;
            if depth > self.limits.max_term_depth() {
                return Err(NeuralError::LimitExceeded {
                    resource: "term depth",
                    value: depth,
                    limit: self.limits.max_term_depth(),
                });
            }
            stats.depth = stats.depth.max(depth);
            if let Term::Sym(_, args) = current {
                for arg in args.iter().rev() {
                    stack.push((arg, depth + 1));
                }
            }
        }
        Ok(stats)
    }
}

impl Default for FeatureEncoder {
    /// The fixed-ceiling profile: [`RelationLimits::default`].
    fn default() -> Self {
        Self::new(RelationLimits::default())
    }
}

/// Variable structure of one term.
#[derive(Clone, Debug, Default)]
struct TermStats {
    nodes: usize,
    depth: usize,
    leaves: usize,
    max_arity: usize,
    arity_hist: [usize; 8],
    symbol_nodes: usize,
    branching_nodes: usize,
    left_spine: usize,
    arity_sum: usize,
    var_occurrences: usize,
    var_depth_sum: usize,
    var_max_depth: usize,
    var_depth_hist: [usize; 8],
    name_len_sum: usize,
    var_names: Vec<String>,
    symbol_names: Vec<String>,
}

/// Residual-constraint structure of one candidate.
#[derive(Clone, Debug, Default)]
struct ConstraintStats {
    count: usize,
    equal: usize,
    not_equal: usize,
    side_nodes: usize,
    side_symbol_nodes: usize,
    side_var_occurrences: usize,
    side_depth_max: usize,
    equal_ground: usize,
    equal_shared: usize,
    equal_disjoint: usize,
    not_equal_ground: usize,
    not_equal_shared: usize,
    not_equal_disjoint: usize,
    var_names: Vec<String>,
}

/// One constraint side.
#[derive(Clone, Debug, Default)]
struct SideStats {
    nodes: usize,
    depth: usize,
    symbol_nodes: usize,
    var_names: Vec<String>,
}

/// The subterm resolved from the recorded step position.
#[derive(Clone, Copy, Debug, Default)]
struct PositionStats {
    nodes: usize,
    depth: usize,
    arity: usize,
}

/// The constraint's tag: equality or disequality.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Class {
    /// `Equal(left, right)`.
    Equal,
    /// `NotEqual(left, right)`.
    NotEqual,
}

/// The variable class of a constraint: where its variables live.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConstraintClass {
    /// No variable on either side.
    Ground,
    /// The sides share at least one variable name.
    Shared,
    /// At least one side has a variable; the sides share none.
    Disjoint,
}

impl ConstraintClass {
    /// Classify a constraint from its ground/shared flags.
    fn of(ground: bool, shared: bool) -> Self {
        if ground {
            Self::Ground
        } else if shared {
            Self::Shared
        } else {
            Self::Disjoint
        }
    }
}

/// Charge one metered operation.
fn charge(resources: &mut RelationResources, limit: usize) -> Result<(), NeuralError> {
    resources
        .record_operations(1)
        .map_err(|_| NeuralError::BudgetExhausted {
            resource: "operations",
            limit,
        })
}

/// Charge `count` metered operations at once.
fn charge_many(
    resources: &mut RelationResources,
    count: usize,
    limit: usize,
) -> Result<(), NeuralError> {
    resources
        .record_operations(count)
        .map_err(|_| NeuralError::BudgetExhausted {
            resource: "operations",
            limit,
        })
}

/// `part / whole` as an `f32`, with the empty-set case defined as zero.
fn ratio(part: usize, whole: usize) -> f32 {
    if whole == 0 {
        0.0
    } else {
        part as f32 / whole as f32
    }
}

/// Integer log2 bucket: `bit_length(value) / scale`.
///
/// The bucket is exact integer arithmetic, which is why the encoder needs no
/// logarithm — and why lane values are platform-independent.
fn bit_bucket(value: u64, scale: f32) -> f32 {
    let bits = u64::BITS - value.leading_zeros();
    bits as f32 / scale
}

/// Variable depth histogram bucket for a 1-based node depth.
fn depth_bucket(depth: usize) -> usize {
    match depth {
        0 | 1 => depth,
        2 => 2,
        3..=4 => 3,
        5..=8 => 4,
        9..=16 => 5,
        17..=32 => 6,
        _ => 7,
    }
}

/// Length of the leftmost root-to-leaf path, in nodes.
///
/// Stops one step past the depth ceiling: a spine longer than the ceiling
/// is rejected by the traversal anyway, so the walk never needs to follow
/// it (an unbounded walk here would be an unmetered cost on adversarial
/// input).
fn left_spine_length(term: &Term, max_depth: usize) -> usize {
    let mut length = 1;
    let mut node = term;
    while length <= max_depth {
        match node {
            Term::Sym(_, args) => match args.first() {
                Some(first) => {
                    length += 1;
                    node = first;
                }
                None => break,
            },
            Term::Var(_) => break,
        }
    }
    length
}

/// Mean of the recorded path indices, zero for the root path.
fn mean_index(path: &[usize]) -> f32 {
    if path.is_empty() {
        return 0.0;
    }
    let sum = path
        .iter()
        .fold(0usize, |total, &index| total.saturating_add(index));
    sum as f32 / path.len() as f32
}

/// Number of distinct names.
fn distinct(names: &mut Vec<String>) -> usize {
    names.sort_unstable();
    names.dedup();
    names.len()
}

/// Number of names present in both lists.
fn shared_names(left: &[String], right: &[String]) -> usize {
    let mut left = left.to_vec();
    let mut right = right.to_vec();
    left.sort_unstable();
    left.dedup();
    right.sort_unstable();
    right.dedup();
    left.iter()
        .filter(|name| right.binary_search(name).is_ok())
        .count()
}

/// Size of the union of two name lists.
fn union_count(left: &[String], right: &[String]) -> usize {
    let mut all: Vec<String> = left.iter().chain(right.iter()).cloned().collect();
    distinct(&mut all)
}

/// Whether the two lists share at least one name.
fn shares_name(left: &[String], right: &[String]) -> bool {
    shared_names(left, right) > 0
}
