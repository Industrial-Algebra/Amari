// SPDX-License-Identifier: MIT OR Apache-2.0

//! The frozen `InverseStateEmbeddingV1` descriptor and the bounded geometric
//! inverse search graph that stores symbolic states as nodes and the symbolic
//! engine's valid inverse transitions as edges.
//!
//! ## Heuristic, non-isometric, and fixed
//!
//! `Cl(3,0,0)` has eight blades, so a state embeds as one coefficient per
//! blade. [`InverseStateEmbeddingV1`] fixes those eight components — log node
//! count, term depth, variable ratio, `(symbol, arity)` diversity, constraint
//! density, existential ratio, search/provenance depth, and a signed
//! SHA-256 content sketch — and nothing about it is learned, tuned, or
//! fitted. It is **a heuristic, non-isometric embedding**: the coefficients
//! are normalized *descriptors*, not coordinates, so a smaller geometric
//! distance is evidence that two states look alike, never a proof that they
//! are semantically close, structurally equivalent, or adjacent in the
//! rewrite relation. No distance on this surface is a correctness claim.
//!
//! Two consequences are contractual:
//!
//! - **The descriptor is total.** Each component has a hard-ceiling
//!   normalizer whose denominator is at least one, so every component is
//!   finite on every admitted input — a lone variable (no symbol node at all),
//!   a saturating existential count, a search depth of `u64::MAX` — and stays
//!   inside its documented range. Nothing here can produce a NaN or an
//!   infinity, and nothing divides by a count that can be zero.
//! - **Collisions are expected.** The content sketch is one byte and the
//!   other seven components quantize shape, so two distinct states routinely
//!   share all eight coefficients (and therefore the embedding hash). That is
//!   why node identity is the canonical state digest and *never* embedding
//!   equality: a colliding embedding must not merge two symbolic states, and
//!   it does not.
//!
//! ## The graph consumes; it never generates
//!
//! [`InverseSearchGraph`] is a bounded witness structure. It is built from
//! states the caller already holds and from transitions the symbolic engine
//! already validated ([`GuidanceRequest`](crate::inverse::GuidanceRequest)
//! rounds, or [`TransitionWitness`] tokens minted at the engine boundary); it
//! has no access to a term system and no method that returns a term, a state, or
//! a candidate. Geometry here orders the engine's candidates — it can never add,
//! validate, or invalidate one, and a graph edge is a record of a valid
//! transition, not a proposal.
//!
//! Edge admission is **enforced**, not promised: [`InverseSearchGraph::connect`]
//! takes a [`TransitionWitness`], and a witness names exactly the parent and
//! candidate state digests plus the transition provenance of one
//! engine-validated transition. A pair the witness does not name is a typed
//! [`NetworkError::UnwitnessedTransition`], so no caller can record an edge the
//! engine never produced. A recorded edge still creates no search candidate:
//! it is evidence about a transition the engine already derived.
//!
//! ## Witnesses are minted at the engine, never here
//!
//! This module has no access to a term system, so it is not the witness
//! authority either. [`TransitionWitness`] has private fields and no public
//! constructor: the one minting path is [`TransitionWitness::validate`], which
//! lives on the engine-owning inverse side, re-derives the parent's
//! predecessor round from the [`crate::trs::TermSystem`], and mints only when
//! the engine derives the supplied candidate's transition. A witness therefore
//! asserts "the engine validated this parent to candidate transition", and that
//! assertion is enforced by construction: an unrelated parent or a substituted
//! candidate state cannot mint one, so it is not expressible on this surface.
//! Both edge-creation paths ([`InverseSearchGraph::connect`] and
//! [`InverseSearchGraph::observe_round`]) consume only the validated token.
//! ## Cycles, partial graphs, and depth
//!
//! A backward search re-encounters states, so cycles are ordinary: an edge may
//! point backwards, and a self-loop is a legal edge when it is *witnessed* (the
//! engine excludes syntactically equal predecessors, but an alpha-variant
//! transition can still name the same canonical state). Every operation here
//! is iterative — a path walk, a nearest-node scan, a descriptor walk — so a
//! cycle changes nothing but the edge count, and a *partial* graph (only the
//! states reached so far) answers against what it knows and reports "no
//! nearest node" when it knows nothing.
//!
//! ## Bounded admission before allocation
//!
//! [`InverseSearchGraph::new`] allocates at most the config's node and edge
//! ceilings. A node insertion checks the node ceiling *before* the descriptor
//! walk, the walk checks term nodes, term depth, constraints, identifier bytes,
//! and the operation budget *before* it pushes a frontier frame or collects a
//! name, and an edge checks its witness and the edge ceiling before it is
//! recorded. The walk's only pre-inspection allocation is its traversal stack,
//! which starts with the bounded root frame — a constant-size vector, never
//! proportional to an identifier. **No identifier-sized allocation precedes a
//! rejection**: a name's bytes are measured on the borrowed `str` before the
//! name is pushed, so an over-long identifier is refused without having been
//! copied, sorted, or hashed. A rejected insertion leaves the graph exactly as it was (except
//! that node admission, which precedes edge admission in
//! [`InverseSearchGraph::connect`], has already happened — an admitted node
//! stays admitted).

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::vec::Vec;

use amari_core::Multivector;
use amari_network::GeometricNetwork;

use crate::inverse::{GuidanceCandidate, SymbolicPredecessor, SymbolicState};
use crate::network::config::{InverseGraphConfig, NetworkError, MAX_IDENTIFIER_BYTES};
use crate::relation::{RelationLimits, RelationResources, Sha256Digest};
use crate::trs::Term;

/// Width of the frozen descriptor: one coefficient per `Cl(3,0,0)` blade.
pub const EMBEDDING_WIDTH: usize = 8;

/// One frozen descriptor component: its stable name and its formula.
///
/// Both halves are part of [`InverseStateEmbeddingV1::schema_digest`], so the
/// constant table is the descriptor's authority rather than a comment beside
/// it. A rename, a reorder, or a rescaled formula is a deliberate, visible
/// change — and a golden-test failure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmbeddingComponent {
    /// The stable component name.
    pub name: &'static str,
    /// The component's formula and range, in arithmetic.
    pub formula: &'static str,
}

impl EmbeddingComponent {
    /// A component definition.
    pub const fn new(name: &'static str, formula: &'static str) -> Self {
        Self { name, formula }
    }
}

/// The frozen, versioned eight-blade inverse state descriptor.
///
/// See the module documentation for the non-isometric, colliding,
/// digest-identified contract. Coefficients are `f64` and comparable with
/// `==` (they are the results of fixed integer-to-float conversions and
/// divisions), but the type deliberately does not implement `Eq`: an
/// embedding is evidence, not identity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InverseStateEmbeddingV1 {
    coefficients: [f64; EMBEDDING_WIDTH],
}

impl InverseStateEmbeddingV1 {
    /// The descriptor version, bumped only by a deliberate incompatible change.
    pub const VERSION: u32 = 1;

    /// The coefficient count.
    pub const WIDTH: usize = EMBEDDING_WIDTH;

    /// Digest frame for [`Self::schema_digest`] and [`Self::hash`].
    pub const FRAME: &'static str = "amari.network.inverse-embedding/v1";

    /// The pinned digest of the component table below.
    ///
    /// Test-pinned: the constant is compared against the value derived from
    /// [`Self::COMPONENTS`], so a component change fails the suite instead of
    /// silently re-embedding every stored node.
    pub const PINNED_DIGEST: &'static str =
        "6e243df2c9f6b5c92896af9c70732a048ddd90164bb39df7679a44951f06b2bc";

    /// The normalization ceiling shared with the depth components: a term
    /// depth or search depth at or above this saturates at `1.0`.
    pub const DEPTH_CEILING: usize = RelationLimits::MAX_TERM_DEPTH;

    /// The scale of the log node-count bucket: `bit_length(nodes) / 32`.
    ///
    /// `bit_length` is exact integer arithmetic, so the component needs no
    /// logarithm and is identical on every platform. The ceiling profile's
    /// 4 096 nodes occupy 13 bits, so the component stays inside `[0, 1]`.
    pub const NODE_LOG_SCALE: f64 = 32.0;

    /// Every component, in blade order: its stable name and its formula.
    pub const COMPONENTS: [EmbeddingComponent; EMBEDDING_WIDTH] = [
        EmbeddingComponent::new("nodes.log", "bit_length(node count) / 32"),
        EmbeddingComponent::new(
            "depth.max",
            "min(max node depth, 64) / 64 (root depth is 1)",
        ),
        EmbeddingComponent::new(
            "vars.ratio",
            "distinct variables / nodes (0 when the term has no nodes)",
        ),
        EmbeddingComponent::new(
            "symbols.arity_diversity",
            "distinct (symbol name, arity) pairs / symbol nodes (0 when none)",
        ),
        EmbeddingComponent::new("constraints.density", "constraints / (nodes + constraints)"),
        EmbeddingComponent::new(
            "existentials.ratio",
            "existentials / (nodes + existentials)",
        ),
        EmbeddingComponent::new(
            "depth.search",
            "min(search depth, 64) / 64 (the engine's provenance depth)",
        ),
        EmbeddingComponent::new(
            "content.sketch",
            "first digest byte / 127.5 - 1 (signed, [-1, 1])",
        ),
    ];

    /// The digest of the frozen component table (version, names, formulas).
    pub fn schema_digest() -> Sha256Digest {
        let mut encoding = Vec::with_capacity(768);
        encoding.extend_from_slice(&Self::VERSION.to_le_bytes());
        for component in Self::COMPONENTS {
            push_str(&mut encoding, component.name);
            push_str(&mut encoding, component.formula);
        }
        Sha256Digest::framed(Self::FRAME, &encoding)
    }

    /// Embed a state.
    ///
    /// `search_depth` is the backward depth the engine recorded for this state
    /// (the provenance depth), and `existentials` is the number of existential
    /// logic variables the step introduced. Both are the caller's own
    /// observations of an already-valid state; nothing here searches.
    ///
    /// # Errors
    ///
    /// [`NetworkError::LimitExceeded`] when the term exceeds the relation
    /// limits (nodes, depth) or the constraint set exceeds its ceiling, and
    /// [`NetworkError::BudgetExhausted`] when the operation budget runs out.
    /// The walk is iterative and metered, and it checks each ceiling before it
    /// grows a frontier.
    pub fn of_state(
        state: &SymbolicState,
        search_depth: u64,
        existentials: usize,
        limits: &RelationLimits,
    ) -> Result<Self, NetworkError> {
        let constraints = state.constraints().len();
        if constraints > limits.max_constraints() {
            return Err(NetworkError::LimitExceeded {
                resource: "constraints",
                value: constraints,
                limit: limits.max_constraints(),
            });
        }
        let shape = shape(state.term(), limits, constraints)?;
        let nodes = shape.nodes as f64;
        let components = [
            (u64::BITS - (shape.nodes as u64).leading_zeros()) as f64 / Self::NODE_LOG_SCALE,
            (shape.depth.min(Self::DEPTH_CEILING) as f64) / Self::DEPTH_CEILING as f64,
            ratio(shape.distinct_vars as f64, nodes),
            ratio(shape.distinct_pairs as f64, shape.symbol_nodes as f64),
            ratio(constraints as f64, nodes + constraints as f64),
            ratio(existentials as f64, nodes + existentials as f64),
            (search_depth.min(Self::DEPTH_CEILING as u64) as f64) / Self::DEPTH_CEILING as f64,
            f64::from(state.canonical_digest().as_bytes()[0]) / 127.5 - 1.0,
        ];
        Ok(Self {
            coefficients: components,
        })
    }

    /// Embed one engine candidate: its state, its symbolic score's depth, and
    /// its predecessor's existential count.
    ///
    /// This is the transition-derived path; it is bit-identical to
    /// [`Self::of_state`] with the same three inputs, because the descriptor is
    /// a function of content and not of how the content was reached.
    ///
    /// # Errors
    ///
    /// As [`Self::of_state`].
    pub fn of_candidate(
        candidate: &GuidanceCandidate<'_>,
        limits: &RelationLimits,
    ) -> Result<Self, NetworkError> {
        Self::of_state(
            candidate.state,
            candidate.score.depth,
            candidate.predecessor.existentials.len(),
            limits,
        )
    }

    /// The eight coefficients, in blade order.
    pub fn coefficients(&self) -> [f64; EMBEDDING_WIDTH] {
        self.coefficients
    }

    /// One component by blade index.
    pub fn component(&self, index: usize) -> Option<f64> {
        self.coefficients.get(index).copied()
    }

    /// The embedding's identity: the version and the eight coefficient bit
    /// patterns.
    ///
    /// It deliberately does **not** cover the source state's digest: an
    /// embedding hash is an embedding identity, so two colliding descriptors
    /// hash equally while their states stay distinguishable by their canonical
    /// digests.
    pub fn hash(&self) -> Sha256Digest {
        let mut payload = Vec::with_capacity(4 + EMBEDDING_WIDTH * 8);
        payload.extend_from_slice(&Self::VERSION.to_le_bytes());
        for coefficient in self.coefficients {
            payload.extend_from_slice(&coefficient.to_bits().to_le_bytes());
        }
        Sha256Digest::framed(Self::FRAME, &payload)
    }

    /// The coefficient norm of the coefficient-wise difference: the same
    /// `Cl(3,0,0)` metric [`GeometricNetwork::geometric_distance`] reports,
    /// computed without materializing two multivectors.
    ///
    /// Scaling by the largest magnitude keeps every representable component
    /// difference representable, exactly as `Multivector::coefficient_norm`
    /// does. The result is a heuristic distance (see the module docs), always
    /// finite for finite inputs.
    pub fn distance(&self, other: &Self) -> f64 {
        let mut difference = [0.0f64; EMBEDDING_WIDTH];
        for (index, value) in difference.iter_mut().enumerate() {
            *value = self.coefficients[index] - other.coefficients[index];
        }
        scaled_norm(&difference)
    }

    /// The descriptor as the `Cl(3,0,0)` multivector the graph stores.
    pub fn to_multivector(&self) -> Multivector<3, 0, 0> {
        Multivector::from_coefficients(self.coefficients.to_vec())
    }
}

/// Shape statistics of one state term, gathering exactly what the descriptor
/// needs and nothing else.
#[derive(Clone, Copy, Debug, Default)]
struct TermShape {
    nodes: usize,
    depth: usize,
    symbol_nodes: usize,
    distinct_vars: usize,
    distinct_pairs: usize,
}

/// Walk a term iteratively, metered, and bounded before every push.
fn shape(
    term: &Term,
    limits: &RelationLimits,
    constraints: usize,
) -> Result<TermShape, NetworkError> {
    let max_nodes = limits.max_term_nodes();
    let max_depth = limits.max_term_depth();
    let max_operations = limits.max_operations();
    let mut resources = RelationResources::new(limits);
    let mut stats = TermShape::default();
    let mut variables: Vec<&str> = Vec::new();
    let mut pairs: Vec<(&str, usize)> = Vec::new();
    let mut stack: Vec<(&Term, usize)> = Vec::from([(term, 1)]);
    while let Some((node, depth)) = stack.pop() {
        resources
            .record_operations(1)
            .map_err(|_| NetworkError::BudgetExhausted {
                resource: "descriptor operations",
                limit: max_operations,
            })?;
        stats.nodes += 1;
        if stats.nodes > max_nodes {
            return Err(NetworkError::LimitExceeded {
                resource: "term nodes",
                value: stats.nodes,
                limit: max_nodes,
            });
        }
        if depth > max_depth {
            return Err(NetworkError::LimitExceeded {
                resource: "term depth",
                value: depth,
                limit: max_depth,
            });
        }
        stats.depth = stats.depth.max(depth);
        match node {
            Term::Var(variable) => {
                // Task 40's identifier-byte doctrine, applied before the name
                // is collected: the sort below never sees a name longer than
                // the ceiling, so a shared 16 MiB prefix cannot turn dedup into
                // unbounded comparison work. The name is borrowed either way.
                check_identifier(variable.as_str())?;
                variables.push(variable.as_str());
            }
            Term::Sym(symbol, args) => {
                check_identifier(symbol.as_str())?;
                stats.symbol_nodes += 1;
                pairs.push((symbol.as_str(), args.len()));
                // The frontier is bounded before it is materialized.
                let frontier = stats.nodes + stack.len() + args.len();
                if frontier > max_nodes {
                    return Err(NetworkError::LimitExceeded {
                        resource: "term nodes",
                        value: frontier,
                        limit: max_nodes,
                    });
                }
                for arg in args.iter().rev() {
                    stack.push((arg, depth + 1));
                }
            }
        }
    }
    resources
        .record_operations(constraints)
        .map_err(|_| NetworkError::BudgetExhausted {
            resource: "descriptor operations",
            limit: max_operations,
        })?;
    variables.sort_unstable();
    variables.dedup();
    pairs.sort_unstable();
    pairs.dedup();
    stats.distinct_vars = variables.len();
    stats.distinct_pairs = pairs.len();
    Ok(stats)
}

/// Reject an identifier longer than [`MAX_IDENTIFIER_BYTES`].
///
/// The length is measured on the borrowed `str`, before the descriptor
/// collects, sorts, dedups, or hashes the name. Task 40's doctrine: a name that
/// cannot be admitted cannot make the surface do work proportional to it.
fn check_identifier(name: &str) -> Result<(), NetworkError> {
    if name.len() > MAX_IDENTIFIER_BYTES {
        return Err(NetworkError::LimitExceeded {
            resource: "identifier bytes",
            value: name.len(),
            limit: MAX_IDENTIFIER_BYTES,
        });
    }
    Ok(())
}

/// `part / whole`, with the empty-set case defined as zero.
///
/// The caller passes `whole` as an `f64` sum, so a saturating count cannot
/// overflow the denominator and the division can never be `0/0`.
fn ratio(part: f64, whole: f64) -> f64 {
    if whole > 0.0 {
        part / whole
    } else {
        0.0
    }
}

/// Scale-then-square-then-root norm, mirroring `coefficient_norm`'s
/// underflow/overflow discipline. `sqrt` is correctly rounded in IEEE-754, so
/// the result is platform-independent.
fn scaled_norm(values: &[f64; EMBEDDING_WIDTH]) -> f64 {
    if values.iter().any(|value| value.is_nan()) {
        return f64::NAN;
    }
    let max = values
        .iter()
        .fold(0.0f64, |largest, value| largest.max(value.abs()));
    if max == 0.0 || !max.is_finite() {
        return max;
    }
    let mut sum = 0.0f64;
    for value in values {
        let scaled = value / max;
        sum += scaled * scaled;
    }
    max * sum.sqrt()
}

/// Length-prefixed string, so component names and formulas cannot run together.
fn push_str(encoding: &mut Vec<u8>, text: &str) {
    let length = u32::try_from(text.len()).unwrap_or(u32::MAX);
    encoding.extend_from_slice(&length.to_le_bytes());
    encoding.extend_from_slice(text.as_bytes());
}

/// One node offered to the graph: an already-valid state plus the two
/// observations the descriptor needs.
///
/// The caller supplies them; the graph never derives a state, a depth, or an
/// existential count from anything but its own input.
#[derive(Clone, Copy, Debug)]
pub struct GraphNodeInput<'a> {
    /// The symbolic state (an engine product, never a graph product).
    pub state: &'a SymbolicState,
    /// The backward depth the engine recorded for this state.
    pub search_depth: u64,
    /// Existential logic variables the step introduced.
    pub existentials: usize,
}

impl<'a> GraphNodeInput<'a> {
    /// Offer a state with its engine-recorded depth and existential count.
    pub fn new(state: &'a SymbolicState, search_depth: u64, existentials: usize) -> Self {
        Self {
            state,
            search_depth,
            existentials,
        }
    }

    /// Offer one engine candidate.
    pub fn of_candidate(candidate: &GuidanceCandidate<'a>) -> Self {
        Self {
            state: candidate.state,
            search_depth: candidate.score.depth,
            existentials: candidate.predecessor.existentials.len(),
        }
    }
}

/// One admitted node: its identity, its descriptor, and its recorded depth.
#[derive(Clone, Debug)]
struct GraphNode {
    digest: Sha256Digest,
    embedding: InverseStateEmbeddingV1,
    depth: u64,
}

/// A proof that the symbolic engine validated one inverse transition.
///
/// The graph records an edge only against a witness. A witness names the
/// transition it came from — the parent's canonical state digest, the
/// candidate's canonical state digest, and the transition's provenance digest —
/// and asserts exactly one thing: **the engine validated this parent to
/// candidate transition**. That assertion is enforced by construction, not by
/// convention:
///
/// - the fields are private and there is no public setter, so a minted witness
///   cannot be re-bound to a different parent, state, or provenance;
/// - there is no public constructor from a caller-assembled record. The one
///   minting path is [`TransitionWitness::validate`], which lives on the
///   engine-owning inverse side, re-runs the engine for the supplied parent,
///   and mints only when the engine derives that candidate's transition. A
///   genuine predecessor for `x` therefore cannot mint a witness for an
///   unrelated parent `a`, and a substituted candidate state cannot be passed
///   off as a predecessor — neither is expressible.
///
/// [`InverseSearchGraph::connect`] then refuses any endpoint pair the witness
/// does not name, so "valid inverse transitions only" is enforced by the API
/// rather than promised in a comment.
///
/// The type holds digests alone: no term, no path, no diagnostic, so a witness
/// is not a content channel and cannot become one. Minting one never derives a
/// transition — it copies an engine validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransitionWitness {
    from: Sha256Digest,
    to: Sha256Digest,
    provenance: Sha256Digest,
}

impl TransitionWitness {
    /// Mint the token for a transition the engine has already validated.
    ///
    /// Crate-internal by design: the only caller is
    /// [`TransitionWitness::validate`], the public engine-side constructor that
    /// re-checks the transition. Keeping this private to the crate is what
    /// makes "caller-assembled record, therefore witness" inexpressible.
    pub(crate) fn mint(from: Sha256Digest, to: Sha256Digest, provenance: Sha256Digest) -> Self {
        Self {
            from,
            to,
            provenance,
        }
    }

    /// The witness's source state digest.
    pub fn from_digest(&self) -> Sha256Digest {
        self.from
    }

    /// The witness's target state digest.
    pub fn to_digest(&self) -> Sha256Digest {
        self.to
    }

    /// The witness's transition provenance digest.
    pub fn provenance_digest(&self) -> Sha256Digest {
        self.provenance
    }
}

/// Digest of one transition's provenance: rule identity, replaced position,
/// freshening scope, and the authority hashes, exactly as the engine recorded
/// them.
///
/// Crate-internal so the engine-side minting boundary
/// ([`TransitionWitness::validate`]) can bind the provenance it validated.
pub(crate) fn provenance_digest(predecessor: &SymbolicPredecessor) -> Sha256Digest {
    let provenance = &predecessor.provenance;
    let mut payload = Vec::with_capacity(160);
    payload.extend_from_slice(provenance.rule_id.digest().as_bytes());
    let position = provenance.position.as_slice();
    let position_len = u32::try_from(position.len()).unwrap_or(u32::MAX);
    payload.extend_from_slice(&position_len.to_le_bytes());
    for index in position {
        payload.extend_from_slice(&(*index as u64).to_le_bytes());
    }
    payload.extend_from_slice(&provenance.scope.to_le_bytes());
    payload.extend_from_slice(provenance.target_hash.as_bytes());
    payload.extend_from_slice(provenance.predecessor_hash.as_bytes());
    Sha256Digest::framed("amari.network.inverse-transition/v1", &payload)
}

/// A bounded geometric inverse search graph: symbolic states as nodes, the
/// engine's valid transitions as edges, keyed by canonical state digest.
///
/// See the module documentation: the graph consumes valid transitions and
/// never generates one, collisions never merge nodes, and every ceiling is
/// enforced before allocation.
#[derive(Clone, Debug)]
pub struct InverseSearchGraph {
    network: GeometricNetwork<3, 0, 0>,
    config: InverseGraphConfig,
    nodes: Vec<GraphNode>,
    index: BTreeMap<Sha256Digest, usize>,
    edges: BTreeSet<(usize, usize)>,
}

impl InverseSearchGraph {
    /// Open an empty graph with the configured ceilings.
    ///
    /// Allocation is bounded by the config: at most `max_nodes` node slots and
    /// `max_edges` edge slots, whatever the search later offers.
    pub fn new(config: InverseGraphConfig) -> Self {
        Self {
            network: GeometricNetwork::with_capacity(config.max_nodes(), config.max_edges()),
            config,
            nodes: Vec::with_capacity(config.max_nodes()),
            index: BTreeMap::new(),
            edges: BTreeSet::new(),
        }
    }

    /// The graph's validated ceilings.
    pub fn config(&self) -> &InverseGraphConfig {
        &self.config
    }

    /// The underlying geometric network (positions are descriptors; edges are
    /// recorded transitions weighted by the endpoint distance).
    pub fn network(&self) -> &GeometricNetwork<3, 0, 0> {
        &self.network
    }

    /// Admitted nodes.
    pub fn num_nodes(&self) -> usize {
        self.nodes.len()
    }

    /// Recorded transitions.
    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    /// The node index for a canonical state digest, if the state was admitted.
    pub fn index_of(&self, digest: &Sha256Digest) -> Option<usize> {
        self.index.get(digest).copied()
    }

    /// Whether the state with this digest is a node.
    pub fn contains(&self, digest: &Sha256Digest) -> bool {
        self.index.contains_key(digest)
    }

    /// The canonical state digest of a node.
    pub fn node_digest(&self, index: usize) -> Option<Sha256Digest> {
        self.nodes.get(index).map(|node| node.digest)
    }

    /// The descriptor of a node.
    pub fn node_embedding(&self, index: usize) -> Option<&InverseStateEmbeddingV1> {
        self.nodes.get(index).map(|node| &node.embedding)
    }

    /// The engine-recorded depth of a node.
    pub fn node_depth(&self, index: usize) -> Option<u64> {
        self.nodes.get(index).map(|node| node.depth)
    }

    /// Whether `from -> to` is a recorded transition (including a self-loop).
    pub fn has_edge(&self, from: usize, to: usize) -> bool {
        self.edges.contains(&(from, to))
    }

    /// Every recorded transition, in ascending `(from, to)` index order.
    pub fn edges(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.edges.iter().copied()
    }

    /// Admit one state, returning its index.
    ///
    /// A node already present (by canonical digest) is returned unchanged: a
    /// repeated state is one node, and a colliding *embedding* is irrelevant to
    /// that decision.
    ///
    /// # Errors
    ///
    /// [`NetworkError::LimitExceeded`] when the node ceiling is reached (before
    /// the descriptor is computed), or any descriptor rejection.
    pub fn insert(&mut self, node: GraphNodeInput<'_>) -> Result<usize, NetworkError> {
        let digest = node.state.canonical_digest();
        if let Some(index) = self.index.get(&digest) {
            return Ok(*index);
        }
        if self.nodes.len() >= self.config.max_nodes() {
            return Err(NetworkError::LimitExceeded {
                resource: "graph nodes",
                value: self.nodes.len() + 1,
                limit: self.config.max_nodes(),
            });
        }
        // Admission precedes the descriptor walk: an over-full graph never
        // pays for an encoding it cannot store.
        let embedding = InverseStateEmbeddingV1::of_state(
            node.state,
            node.search_depth,
            node.existentials,
            self.config.limits(),
        )?;
        let index = self.nodes.len();
        self.network.add_node(embedding.to_multivector());
        self.nodes.push(GraphNode {
            digest,
            embedding,
            depth: node.search_depth,
        });
        self.index.insert(digest, index);
        Ok(index)
    }

    /// Record the witnessed transition `from -> to`, admitting both endpoints
    /// first.
    ///
    /// `witness` must name exactly these two endpoints: it is minted by the
    /// engine-side validating constructor ([`TransitionWitness::validate`]), so
    /// the pair an edge records is a transition the symbolic engine already
    /// validated. A pair the witness does not name is the typed
    /// [`NetworkError::UnwitnessedTransition`] rejection and admits nothing —
    /// the witness is checked before either endpoint.
    ///
    /// The edge weight is the geometric distance between the endpoints'
    /// descriptors — a heuristic edge measure, never an authority. A repeated
    /// pair is not re-added, and a witnessed self-loop or back-edge is an
    /// ordinary edge. A recorded edge never creates a search candidate: it is a
    /// record of a transition the engine produced, not a proposal.
    ///
    /// # Errors
    ///
    /// [`NetworkError::UnwitnessedTransition`] when the endpoints are not the
    /// pair the witness names, [`NetworkError::LimitExceeded`] when either
    /// endpoint is not admitted or the edge ceiling is reached. An endpoint
    /// admitted by this call stays admitted even when the edge that introduced
    /// it is then rejected.
    pub fn connect(
        &mut self,
        witness: TransitionWitness,
        from: GraphNodeInput<'_>,
        to: GraphNodeInput<'_>,
    ) -> Result<(usize, usize), NetworkError> {
        // The witness gates everything: an unwitnessed transition leaves the
        // graph exactly as it was, not even admitting an endpoint.
        if from.state.canonical_digest() != witness.from
            || to.state.canonical_digest() != witness.to
        {
            return Err(NetworkError::UnwitnessedTransition);
        }
        let source = self.insert(from)?;
        let target = self.insert(to)?;
        if self.edges.contains(&(source, target)) {
            return Ok((source, target));
        }
        if self.edges.len() >= self.config.max_edges() {
            return Err(NetworkError::LimitExceeded {
                resource: "graph edges",
                value: self.edges.len() + 1,
                limit: self.config.max_edges(),
            });
        }
        let weight = self.nodes[source]
            .embedding
            .distance(&self.nodes[target].embedding);
        let nodes = self.nodes.len();
        self.network
            .add_edge(source, target, weight)
            .map_err(|_| NetworkError::UnknownNode {
                index: source.max(target),
                nodes,
            })?;
        self.edges.insert((source, target));
        Ok((source, target))
    }

    /// Record one expansion round: the parent state and one validated transition
    /// to each of its engine-derived candidates.
    ///
    /// This is the only bulk entry point, and it consumes engine **witnesses**:
    /// the graph learns `parent -> candidate` for transitions the symbolic
    /// engine already validated, and nothing else. Each `(witness, candidate)`
    /// step is admitted through [`Self::connect`], so the witness names both
    /// endpoints and an unmatched pair is a typed
    /// [`NetworkError::UnwitnessedTransition`] — a caller-assembled
    /// [`GuidanceRequest`](crate::inverse::GuidanceRequest) alone cannot record
    /// an edge, because it cannot mint a witness.
    ///
    /// The parent is the state being expanded, so it carries no existential
    /// count (existentials belong to the *step* that produced a candidate). The
    /// parent node is therefore recorded with zero existentials, which affects
    /// only its descriptor's existential ratio — node identity is its canonical
    /// digest either way.
    ///
    /// # Errors
    ///
    /// As [`Self::connect`].
    pub fn observe_round(
        &mut self,
        parent: GraphNodeInput<'_>,
        steps: &[(TransitionWitness, GraphNodeInput<'_>)],
    ) -> Result<usize, NetworkError> {
        for &(witness, candidate) in steps {
            self.connect(witness, parent, candidate)?;
        }
        self.insert(parent)
    }

    /// The nearest admitted node to a descriptor, with its distance.
    ///
    /// Ties resolve to the lowest node index, so the answer is a function of
    /// content and admission order alone. `None` means the graph knows nothing
    /// (it has no nodes), which is *not* a distance claim.
    pub fn nearest(&self, embedding: &InverseStateEmbeddingV1) -> Option<(usize, f64)> {
        let mut best: Option<(usize, f64)> = None;
        for (index, node) in self.nodes.iter().enumerate() {
            let distance = node.embedding.distance(embedding);
            if !distance.is_finite() {
                continue;
            }
            match best {
                Some((_, nearest)) if distance.total_cmp(&nearest) != core::cmp::Ordering::Less => {
                }
                _ => best = Some((index, distance)),
            }
        }
        best
    }

    /// The distance to the nearest admitted node, or `None` for an empty graph.
    pub fn distance_to_nearest(&self, embedding: &InverseStateEmbeddingV1) -> Option<f64> {
        self.nearest(embedding).map(|(_, distance)| distance)
    }
}
