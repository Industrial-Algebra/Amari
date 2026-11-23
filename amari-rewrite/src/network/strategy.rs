// SPDX-License-Identifier: MIT OR Apache-2.0

//! The transparent geometric candidate score and the ranking adapter that
//! implements the guidance seam over an already-valid inverse expansion round.
//!
//! ## What this module may do
//!
//! [`GeometricStrategy`] is a *ranking* bridge, exactly like the neural
//! adapter: the symbolic engine builds the round's candidates, the strategy
//! orders them, and [`crate::inverse::validate_ranking`] (applied by the
//! search, not here) requires the result to be a permutation. The strategy
//! never generates, validates, invalidates, or drops a candidate — pruning
//! authority stays with the guidance mode — and it holds the graph as an
//! immutable snapshot, so scoring a round cannot mutate the evidence a later
//! round is scored against.
//!
//! The graph it scores against is built by the caller from the engine's own
//! rounds ([`InverseSearchGraph::observe_round`]) or from explicit valid
//! `from -> to` links. Nothing in this module can add a node or an edge, so no
//! score can be improved by inventing a state.
//!
//! ## The score is transparent
//!
//! [`GeometricScore`] exposes seven lanes, every one of them in `[0, 1]` and
//! every one of them a documented function of the descriptor, the graph, or
//! the engine's own symbolic score:
//!
//! | lane | formula |
//! |---|---|
//! | `distance` | nearest-node descriptor distance / [`MAX_EMBEDDING_DISTANCE`] (clamped; `1` when the graph is empty) |
//! | `revisit` | `1` when the candidate's canonical digest is already a node, else `0` |
//! | `branching` | `min(branch factor, 256) / 256` |
//! | `depth` | descriptor component 1 (term depth / 64) |
//! | `constraints` | descriptor component 4 (constraint density) |
//! | `existentials` | descriptor component 5 (existential ratio) |
//! | `cycle` | `1` when the round flagged the candidate a cycle, else `0` |
//!
//! and the total is exactly `sum(WEIGHTS[i] * lanes[i])`, in lane order. The
//! weights are frozen constants ([`GeometricScoreTable::WEIGHTS`]) whose digest
//! is pinned, so the score is a published formula rather than a learned or
//! opaque number. `novelty` is the complement of `revisit` — one observation,
//! two readings — and is exposed for evidence.
//!
//! **Score direction: preferred is LOWER**, the same convention as the
//! symbolic score and the neural ranker (cohort 8 closeout F1): the ranking is
//! ascending by total, so the first index a caller sees is the candidate this
//! score prefers. The end-to-end direction is pinned by a test.
//!
//! No lane, weight, or total is a correctness claim. A lower total says "this
//! candidate looks more like the states this graph already holds"; it does not
//! say the candidate is closer to a solution, nor that a graph edge exists in
//! the rewrite relation beyond the transition it was built from.
//!
//! ## Determinism
//!
//! A round's ranking is a function of `(graph, config, candidates)` alone:
//!
//! - each candidate's descriptor is a pure function of its content (see
//!   [`InverseStateEmbeddingV1`]);
//! - the nearest-node scan is over the graph's node order and settles ties by
//!   the lowest index;
//! - the sort is `(total ascending, content key ascending)`, where the content
//!   key covers the canonical state digest, the transition's provenance, and
//!   the engine's search context. Ties are therefore settled by content and
//!   never by request order, so permuting the request permutes the indices but
//!   not the state order.
//!
//! ## Bounded admission
//!
//! The round's candidate count is admitted against
//! [`GeometricStrategyConfig::max_candidates`] *before* any descriptor is
//! computed, and the ceiling can only be lowered. A rejected round reports
//! `scored: 0`, because nothing was scored. A descriptor failure mid-round is
//! a typed, metered [`GuidanceFailure`] — never a silent fallback to the
//! engine's order, which would be a different search wearing the same name.

use alloc::vec::Vec;

use crate::inverse::{
    GuidanceCandidate, GuidanceFailure, GuidanceFailureKind, GuidanceRequest, GuidanceStrategy,
};
use crate::network::config::{GeometricStrategyConfig, NetworkError, MAX_EMBEDDING_DISTANCE};
use crate::network::graph::{InverseSearchGraph, InverseStateEmbeddingV1};
use crate::relation::Sha256Digest;

/// One lane of the geometric score: its stable name and its formula.
///
/// Both halves are part of [`GeometricScoreTable::digest`], so the lane table
/// is the score's authority. A changed formula is a changed score — and a
/// golden-test failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScoreLane {
    /// The stable lane name.
    pub name: &'static str,
    /// The lane's formula and range, in arithmetic.
    pub formula: &'static str,
}

impl ScoreLane {
    /// A lane definition.
    pub const fn new(name: &'static str, formula: &'static str) -> Self {
        Self { name, formula }
    }
}

/// Number of score lanes.
pub const SCORE_LANES: usize = 7;

/// The frozen geometric score table: seven lanes and their weights.
///
/// A zero-sized marker: the table is its constants, not an instance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GeometricScoreTable;

impl GeometricScoreTable {
    /// The score table's version, bumped only by a deliberate change.
    pub const VERSION: u32 = 1;

    /// Digest frame for [`Self::digest`].
    pub const FRAME: &'static str = "amari.network.geometric-score/v1";

    /// The pinned digest of the lane and weight tables below.
    ///
    /// Test-pinned: compared against the value derived from [`Self::LANES`] and
    /// [`Self::WEIGHTS`], so a weight or formula change fails the suite.
    pub const PINNED_DIGEST: &'static str =
        "e9d99199529faea855aa90fd602a03d45faaab9c248045f14bc61717a4ea9329";

    /// Every lane, in order.
    pub const LANES: [ScoreLane; SCORE_LANES] = [
        ScoreLane::new(
            "distance",
            "nearest-node descriptor distance / 8 (clamped to [0, 1]; 1 when the graph is empty)",
        ),
        ScoreLane::new(
            "revisit",
            "1 when the candidate's canonical digest is already a node, else 0",
        ),
        ScoreLane::new("branching", "min(branch factor, 256) / 256"),
        ScoreLane::new("depth", "descriptor component 1: min(term depth, 64) / 64"),
        ScoreLane::new(
            "constraints",
            "descriptor component 4: constraints / (nodes + constraints)",
        ),
        ScoreLane::new(
            "existentials",
            "descriptor component 5: existentials / (nodes + existentials)",
        ),
        ScoreLane::new(
            "cycle",
            "1 when the round flagged the candidate a cycle, else 0",
        ),
    ];

    /// The frozen lane weights, in lane order. They sum to exactly `1.0`, so a
    /// total is always inside `[0, 1]`.
    pub const WEIGHTS: [f64; SCORE_LANES] = [0.25, 0.20, 0.10, 0.15, 0.10, 0.10, 0.10];

    /// The digest of the frozen lane and weight tables.
    pub fn digest() -> Sha256Digest {
        let mut encoding = Vec::with_capacity(1_024);
        encoding.extend_from_slice(&Self::VERSION.to_le_bytes());
        for lane in Self::LANES {
            push_str(&mut encoding, lane.name);
            push_str(&mut encoding, lane.formula);
        }
        for weight in Self::WEIGHTS {
            encoding.extend_from_slice(&weight.to_bits().to_le_bytes());
        }
        Sha256Digest::framed(Self::FRAME, &encoding)
    }
}

/// One candidate's transparent geometric evidence.
///
/// The total is stored once, computed by [`Self::total_of`] at construction, so
/// every reader sees the same number the ranking sorted by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeometricScore {
    lanes: [f64; SCORE_LANES],
    total: f64,
}

impl GeometricScore {
    /// Score one engine candidate against a graph.
    ///
    /// # Errors
    ///
    /// A descriptor rejection (see [`InverseStateEmbeddingV1::of_state`]).
    pub fn of(
        candidate: &GuidanceCandidate<'_>,
        graph: &InverseSearchGraph,
    ) -> Result<Self, NetworkError> {
        let embedding = InverseStateEmbeddingV1::of_candidate(candidate, graph.config().limits())?;
        let distance = match graph.distance_to_nearest(&embedding) {
            Some(distance) => (distance / MAX_EMBEDDING_DISTANCE).clamp(0.0, 1.0),
            // An empty graph is not a distance claim: it simply has no
            // evidence, which is the same position as maximally distant.
            None => 1.0,
        };
        let revisit = if graph.contains(&candidate.state.canonical_digest()) {
            1.0
        } else {
            0.0
        };
        let branching = (candidate.score.branch_factor.min(256) as f64) / 256.0;
        let depth = embedding.component(1).unwrap_or(0.0);
        let constraints = embedding.component(4).unwrap_or(0.0);
        let existentials = embedding.component(5).unwrap_or(0.0);
        let cycle = if candidate.score.cycle { 1.0 } else { 0.0 };
        let lanes = [
            distance,
            revisit,
            branching,
            depth,
            constraints,
            existentials,
            cycle,
        ];
        Ok(Self {
            lanes,
            total: Self::total_of(&lanes),
        })
    }

    /// The documented weighted sum, in lane order.
    ///
    /// The addition order is part of the contract: callers and tests compute the
    /// same expression left to right, so the total is bit-identical everywhere.
    pub fn total_of(lanes: &[f64; SCORE_LANES]) -> f64 {
        let weights = GeometricScoreTable::WEIGHTS;
        weights[0] * lanes[0]
            + weights[1] * lanes[1]
            + weights[2] * lanes[2]
            + weights[3] * lanes[3]
            + weights[4] * lanes[4]
            + weights[5] * lanes[5]
            + weights[6] * lanes[6]
    }

    /// Every lane value, in lane order.
    pub fn lanes(&self) -> [f64; SCORE_LANES] {
        self.lanes
    }

    /// The distance lane: nearest-node descriptor distance, normalized.
    pub fn distance(&self) -> f64 {
        self.lanes[0]
    }

    /// The revisit lane: `1` when the state is already a graph node.
    pub fn revisit(&self) -> f64 {
        self.lanes[1]
    }

    /// The novelty evidence: `1 - revisit`.
    pub fn novelty(&self) -> f64 {
        1.0 - self.lanes[1]
    }

    /// The branching lane: the round's branch factor, normalized.
    pub fn branching(&self) -> f64 {
        self.lanes[2]
    }

    /// The depth lane: the descriptor's term-depth component.
    pub fn depth(&self) -> f64 {
        self.lanes[3]
    }

    /// The constraints lane: the descriptor's constraint density.
    pub fn constraints(&self) -> f64 {
        self.lanes[4]
    }

    /// The existentials lane: the descriptor's existential ratio.
    pub fn existentials(&self) -> f64 {
        self.lanes[5]
    }

    /// The cycle lane: the engine's cycle flag, normalized.
    pub fn cycle(&self) -> f64 {
        self.lanes[6]
    }

    /// The weighted total. Preferred is LOWER.
    pub fn total(&self) -> f64 {
        self.total
    }
}

/// The geometric ranking adapter: a fixed score table over one graph snapshot.
///
/// Holding a `GeometricStrategy` means "this graph, scored by this table, under
/// these ceilings", and [`Self::replay_hash`] is that identity — the value the
/// guidance seam folds into replay identity through
/// [`crate::inverse::scorer_hash_with`].
#[derive(Clone, Debug)]
pub struct GeometricStrategy {
    graph: InverseSearchGraph,
    config: GeometricStrategyConfig,
}

impl GeometricStrategy {
    /// Bind a graph snapshot to the ranking adapter.
    pub fn new(graph: InverseSearchGraph, config: GeometricStrategyConfig) -> Self {
        Self { graph, config }
    }

    /// The graph this strategy scores against.
    pub fn graph(&self) -> &InverseSearchGraph {
        &self.graph
    }

    /// The validated round ceiling.
    pub fn config(&self) -> &GeometricStrategyConfig {
        &self.config
    }

    /// One candidate's transparent geometric evidence.
    ///
    /// # Errors
    ///
    /// A descriptor rejection.
    pub fn score(&self, candidate: &GuidanceCandidate<'_>) -> Result<GeometricScore, NetworkError> {
        GeometricScore::of(candidate, &self.graph)
    }

    /// This strategy's replay identity: the descriptor schema, the score table,
    /// the graph's admitted state digests in node order, and every ceiling that
    /// can change a ranking.
    ///
    /// Two different graph snapshots never share it, and a changed descriptor
    /// schema, score table, node count, node set, or ceiling is a changed
    /// strategy — because each one can change the ranking.
    pub fn replay_hash(&self) -> Sha256Digest {
        let mut payload = Vec::with_capacity(160 + self.graph.num_nodes() * 32);
        payload.extend_from_slice(&InverseStateEmbeddingV1::VERSION.to_le_bytes());
        payload.extend_from_slice(InverseStateEmbeddingV1::schema_digest().as_bytes());
        payload.extend_from_slice(&GeometricScoreTable::VERSION.to_le_bytes());
        payload.extend_from_slice(GeometricScoreTable::digest().as_bytes());
        payload.extend_from_slice(&(self.graph.num_nodes() as u64).to_le_bytes());
        for index in 0..self.graph.num_nodes() {
            if let Some(digest) = self.graph.node_digest(index) {
                payload.extend_from_slice(digest.as_bytes());
            }
        }
        payload.extend_from_slice(&(self.config.max_candidates() as u64).to_le_bytes());
        let config = self.graph.config();
        payload.extend_from_slice(&(config.max_nodes() as u64).to_le_bytes());
        payload.extend_from_slice(&(config.max_edges() as u64).to_le_bytes());
        let limits = config.limits();
        for limit in [
            limits.max_term_nodes(),
            limits.max_term_depth(),
            limits.max_constraints(),
            limits.max_operations(),
        ] {
            payload.extend_from_slice(&(limit as u64).to_le_bytes());
        }
        Sha256Digest::framed("amari.network.geometric-strategy/v1", &payload)
    }
}

impl GuidanceStrategy for GeometricStrategy {
    fn strategy_hash(&self) -> Sha256Digest {
        self.replay_hash()
    }

    fn rank(&self, request: &GuidanceRequest<'_>) -> Result<Vec<usize>, GuidanceFailure> {
        let candidates = request.candidates;
        let total = candidates.len();
        // Admission precedes everything: nothing proportional to a round that
        // was never admitted is allocated, and nothing is scored.
        if total > self.config.max_candidates() {
            return Err(GuidanceFailure::new(
                GuidanceFailureKind::LimitExceeded {
                    resource: "geometric strategy candidates",
                    value: total,
                    limit: self.config.max_candidates(),
                },
                total,
                0,
            ));
        }
        let mut scored: Vec<(usize, f64, Vec<u8>)> = Vec::with_capacity(total);
        for (index, candidate) in candidates.iter().enumerate() {
            let score = self
                .score(candidate)
                .map_err(|error| failure(&error, total, index))?;
            scored.push((index, score.total(), content_key(candidate)));
        }
        // Ascending score (preferred = LOWER), then the content key: a stable
        // sort, so two candidates with identical content keep the engine's
        // order (they are interchangeable — every input the score observed is
        // equal).
        scored.sort_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.2.cmp(&right.2))
        });
        Ok(scored.into_iter().map(|(index, _, _)| index).collect())
    }
}

/// The deterministic content key that settles every remaining tie.
///
/// The state digest identifies the canonical state, but two candidates for the
/// same state can differ in search context (depth, branch factor, growth,
/// existentials, rule priority) and in provenance (rule, position, scope,
/// target/predecessor hashes). Those differences are observable, so the total
/// order must cover them: the returned ranking is then a function of the
/// candidates' content alone, and permuting the request cannot change it.
fn content_key(candidate: &GuidanceCandidate<'_>) -> Vec<u8> {
    let mut key = Vec::with_capacity(160);
    key.extend_from_slice(candidate.state.canonical_digest().as_bytes());
    let provenance = &candidate.predecessor.provenance;
    key.extend_from_slice(provenance.rule_id.digest().as_bytes());
    let position = provenance.position.as_slice();
    let position_len = u32::try_from(position.len()).unwrap_or(u32::MAX);
    key.extend_from_slice(&position_len.to_le_bytes());
    for index in position {
        key.extend_from_slice(&(*index as u64).to_le_bytes());
    }
    key.extend_from_slice(&provenance.scope.to_le_bytes());
    key.extend_from_slice(provenance.target_hash.as_bytes());
    key.extend_from_slice(provenance.predecessor_hash.as_bytes());
    let score = candidate.score;
    key.extend_from_slice(&score.depth.to_le_bytes());
    key.extend_from_slice(&score.branch_factor.to_le_bytes());
    key.push(u8::from(score.cycle));
    key.extend_from_slice(&score.term_growth.to_le_bytes());
    key.extend_from_slice(&score.constraint_growth.to_le_bytes());
    key.extend_from_slice(&(candidate.predecessor.existentials.len() as u64).to_le_bytes());
    key.extend_from_slice(&score.rule_priority.to_le_bytes());
    key
}

/// Translate one network-surface failure into the guidance seam's typed,
/// sanitized, metered failure.
fn failure(error: &NetworkError, candidates: usize, scored: usize) -> GuidanceFailure {
    let kind = match error {
        NetworkError::LimitExceeded {
            resource,
            value,
            limit,
        } => GuidanceFailureKind::LimitExceeded {
            resource,
            value: *value,
            limit: *limit,
        },
        NetworkError::BudgetExhausted { resource, limit } => GuidanceFailureKind::BudgetExhausted {
            resource,
            limit: *limit,
        },
        // A missing node index cannot arise while scoring a round (every
        // distance is over admitted nodes). Reported as an unavailable surface
        // rather than mapped onto a scoring cause that did not happen.
        NetworkError::UnknownNode { .. } => GuidanceFailureKind::Unavailable {
            stage: "graph node",
        },
    };
    GuidanceFailure::new(kind, candidates, scored)
}

/// Length-prefixed string, so lane names and formulas cannot run together.
fn push_str(encoding: &mut Vec<u8>, text: &str) {
    let length = u32::try_from(text.len()).unwrap_or(u32::MAX);
    encoding.extend_from_slice(&length.to_le_bytes());
    encoding.extend_from_slice(text.as_bytes());
}
