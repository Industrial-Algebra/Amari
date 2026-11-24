//! Geometric inverse guidance contracts (0.25 Cohort 9, Task 46).
//!
//! One test per enumerated obligation of the task:
//!
//! - **golden coefficients** — the frozen eight-blade `InverseStateEmbeddingV1`
//!   descriptor is pinned literally: component values, the component table's
//!   digest, the version, and the embedding hash. A change to any formula is a
//!   visible golden failure rather than a silent re-embedding;
//! - **finite normalization boundaries** — every component is provably finite
//!   on every input, including degenerate states (a lone variable, no symbol
//!   nodes), saturating counts (`usize::MAX` existentials, `u64::MAX` search
//!   depth), and the hard ceiling boundaries themselves;
//! - **collision-with-distinct-state identity** — two states whose descriptors
//!   collide (equal coefficient vectors AND equal embedding hashes) stay two
//!   distinct graph nodes, because node identity is the canonical state digest
//!   and never embedding equality;
//! - **state/transition parity** — a transition-derived embedding (from an
//!   engine candidate) is bit-identical to the state-derived embedding with the
//!   same search depth and existential count;
//! - **distance/novelty/branching components** — the transparent geometric
//!   score's lanes are individually observable and its total is exactly the
//!   documented weighted sum;
//! - **partial graph** — a graph built from a subset of the reachable states
//!   ranks against what it knows, and an empty graph reports no nearest node;
//! - **cycles** — cyclic and self-loop transitions, and a long back-edge, are
//!   ordinary edges when they are witnessed: no recursion, no growth, no
//!   duplicate edges, and an unwitnessed self-loop is a typed rejection;
//! - **limits** — graph size, edge count, descriptor computation, and the
//!   strategy's candidate admission are all bounded before allocation, and a
//!   caller may tighten a ceiling but never widen it. The descriptor's
//!   identifier ceiling is checked on the borrowed name before anything is
//!   collected (Task 40 doctrine), so a shared 16 MiB prefix cannot turn
//!   sort/dedup into unbounded comparison work;
//! - **witnessed edges** — `connect` records an edge only against a
//!   [`TransitionWitness`], and a witness can only be minted by the engine-side
//!   validating constructor [`TransitionWitness::validate`], so an empty system
//!   can record neither `a -> b` nor an invented `a -> a` (PR #382 rounds 1-2,
//!   F2/F4);
//! - **replay identity** — the strategy hash covers the ranking-relevant graph
//!   snapshot (stored descriptors included), so two graphs over the same states
//!   that rank differently cannot share it (PR #382 round 1 F1);
//! - **determinism and direction** — a round's ranking is a function of content
//!   alone (permutation-invariant), it is a permutation of the engine's
//!   candidates (never a generation and never a drop), and the pinned score
//!   direction (preferred = LOWER) holds end to end.

#![cfg(feature = "network")]

use std::collections::BTreeMap;

use amari_rewrite::inverse::{
    score_candidate, symbolic_predecessors, validate_ranking, BackwardExplorer,
    BackwardSearchOutcome, GuidanceCandidate, GuidanceFailureKind, GuidanceMode, GuidanceRequest,
    GuidanceStrategy, GuidedSearchOutcome, InverseSearchConfig, SearchMode, SymbolicPredecessor,
    SymbolicScore, SymbolicState,
};
use amari_rewrite::network::{
    EmbeddingComponent, GeometricScore, GeometricScoreTable, GeometricStrategy,
    GeometricStrategyConfig, GraphNodeInput, InverseGraphConfig, InverseSearchGraph,
    InverseStateEmbeddingV1, NetworkError, TransitionWitness, MAX_GEOMETRIC_CANDIDATES,
    MAX_GRAPH_EDGES, MAX_GRAPH_NODES, MAX_IDENTIFIER_BYTES,
};
use amari_rewrite::relation::{ConstraintSet, RelationLimits, TermConstraint};
use amari_rewrite::trs::{Rule, Term, TermSystem};

/// The eight component names, in blade order. Pinned by
/// [`embedding_schema_is_versioned_and_frozen`].
const COMPONENT_NAMES: [&str; 8] = [
    "nodes.log",
    "depth.max",
    "vars.ratio",
    "symbols.arity_diversity",
    "constraints.density",
    "existentials.ratio",
    "depth.search",
    "content.sketch",
];

fn parse(text: &str) -> Term {
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
                    args.push(parse(&inner[start..index]));
                    start = index + 1;
                }
                _ => {}
            }
        }
        args.push(parse(&inner[start..]));
        Term::sym(head, args)
    } else if text.chars().next().is_some_and(char::is_uppercase) || text.starts_with('?') {
        Term::var(text)
    } else {
        Term::constant(text)
    }
}

fn system(rules: Vec<Rule>) -> TermSystem {
    TermSystem::new(rules)
}

fn limits() -> RelationLimits {
    RelationLimits::default()
}

/// `b => x`, `f(c) => x`: backward from `x` has exactly two valid candidates,
/// one of one node (a constant) and one of two nodes.
fn two_candidate_system() -> TermSystem {
    system(vec![
        Rule::new(parse("b"), parse("x")).unwrap(),
        Rule::new(parse("f(c)"), parse("x")).unwrap(),
    ])
}

/// `a => b`, `b => c`, `c => a`: a cyclic system, so a backward search that
/// exhausts its limits re-encounters states.
fn cycle_system() -> TermSystem {
    system(vec![
        Rule::new(parse("a"), parse("b")).unwrap(),
        Rule::new(parse("b"), parse("c")).unwrap(),
        Rule::new(parse("c"), parse("a")).unwrap(),
    ])
}

/// A small search configuration: the closure exhausts well inside these
/// limits, so the run is certified rather than truncated.
fn small_search_config() -> InverseSearchConfig {
    InverseSearchConfig::new(
        8,
        1_024,
        4_096,
        4_096,
        64,
        4_096,
        65_536,
        100_000,
        1 << 20,
        1 << 20,
    )
    .unwrap()
}

/// One expansion round's engine candidates, their canonical states, and their
/// transparent symbolic scores — in the engine's own order.
struct Round {
    parent: SymbolicState,
    states: Vec<SymbolicState>,
    predecessors: Vec<SymbolicPredecessor>,
    scores: Vec<SymbolicScore>,
}

impl Round {
    fn of(system: &TermSystem, target: &Term) -> Self {
        let predecessors =
            symbolic_predecessors(system, target, &limits(), 0).expect("the engine round");
        let parent = SymbolicState::new(target.clone(), ConstraintSet::new());
        let states: Vec<SymbolicState> = predecessors
            .iter()
            .map(|predecessor| {
                SymbolicState::new(predecessor.term.clone(), predecessor.constraints.clone())
            })
            .collect();
        let branch_factor = predecessors.len() as u64;
        let scores: Vec<SymbolicScore> = predecessors
            .iter()
            .zip(&states)
            .map(|(predecessor, state)| {
                score_candidate(&parent, state, predecessor, 1, branch_factor, false, 0)
            })
            .collect();
        Self {
            parent,
            states,
            predecessors,
            scores,
        }
    }

    fn candidates(&self) -> Vec<GuidanceCandidate<'_>> {
        self.states
            .iter()
            .zip(&self.predecessors)
            .zip(&self.scores)
            .map(|((state, predecessor), score)| GuidanceCandidate {
                state,
                predecessor,
                score,
            })
            .collect()
    }

    fn request<'a>(
        &'a self,
        candidates: &'a [GuidanceCandidate<'a>],
        mode: &'a GuidanceMode,
    ) -> GuidanceRequest<'a> {
        GuidanceRequest {
            parent: &self.parent,
            depth: 0,
            mode,
            candidates,
        }
    }

    /// One `(witness, candidate node)` pair per candidate, each witness minted
    /// by the engine-side validating boundary that re-derives this parent's
    /// round. This is the only way a caller can reach the graph's edge paths.
    fn witness_steps<'a>(
        &'a self,
        system: &TermSystem,
        limits: &RelationLimits,
    ) -> Vec<(TransitionWitness, GraphNodeInput<'a>)> {
        self.states
            .iter()
            .zip(&self.predecessors)
            .zip(&self.scores)
            .map(|((state, predecessor), score)| {
                let candidate = GuidanceCandidate {
                    state,
                    predecessor,
                    score,
                };
                let witness = TransitionWitness::validate(system, &self.parent, &candidate, limits)
                    .expect("the engine validates its own round");
                (witness, GraphNodeInput::of_candidate(&candidate))
            })
            .collect()
    }
}

/// A right-nested chain of symbol applications: `f(f(...f(c)))`, `depth`
/// nodes deep.
fn chain(depth: usize) -> Term {
    let mut term = Term::constant("c");
    for _ in 1..depth {
        term = Term::sym("f", [term]);
    }
    term
}

/// A complete binary tree of `levels` levels: `2^levels - 1` nodes.
fn balanced(levels: usize) -> Term {
    if levels <= 1 {
        return Term::constant("c");
    }
    let child = balanced(levels - 1);
    Term::sym("f", [child.clone(), child])
}

fn state(term: Term) -> SymbolicState {
    SymbolicState::new(term, ConstraintSet::new())
}

/// The greedy first pair of two distinct states whose descriptors collide —
/// equal coefficient vectors *and* equal embedding hashes, distinct canonical
/// digests. The descriptor's content sketch is one byte, so collisions are
/// expected (and are exactly what the digest-keyed node identity is for).
fn colliding_states() -> (SymbolicState, SymbolicState, InverseStateEmbeddingV1) {
    let mut seen: BTreeMap<[u64; 8], (amari_rewrite::relation::Sha256Digest, SymbolicState)> =
        BTreeMap::new();
    for index in 0..8_192u64 {
        let candidate = state(Term::sym("f", [Term::constant(format!("c{index}"))]));
        let embedding =
            InverseStateEmbeddingV1::of_state(&candidate, 1, 0, &limits()).expect("admitted");
        let key = embedding
            .coefficients()
            .map(|coefficient| coefficient.to_bits());
        match seen.get(&key) {
            Some((digest, first)) if *digest != candidate.canonical_digest() => {
                return (first.clone(), candidate, embedding);
            }
            Some(_) => {}
            None => {
                seen.insert(key, (candidate.canonical_digest(), candidate));
            }
        }
    }
    panic!("no colliding pair found among 8192 single-symbol states");
}

/// The frozen descriptor: golden coefficients for all eight components, the
/// pinned component-table digest, the version, and the embedding hash.
///
/// The state is `f(g(c), X, f(a))` with one `X = a` constraint, two
/// existentials, and search depth 3, so every component is non-degenerate:
/// six nodes, depth three, one distinct variable, five distinct
/// `(symbol, arity)` pairs, one constraint, two existentials, and a nonzero
/// content sketch.
#[test]
fn golden_embedding_coefficients_are_pinned() {
    let mut constraints = ConstraintSet::new();
    constraints.insert(TermConstraint::Equal(parse("X"), parse("a")));
    let golden_state = SymbolicState::new(parse("f(g(c), X, f(a))"), constraints);

    let embedding = InverseStateEmbeddingV1::of_state(&golden_state, 3, 2, &limits())
        .expect("a well-formed state is admitted");

    // Golden vector: log node count 3/32, depth 3/64, var ratio 1/6,
    // arity diversity 5/5, constraint density 1/7, existential ratio 2/8,
    // search depth 3/64, and the signed content sketch of the canonical
    // digest's first byte.
    const GOLDEN: [f64; 8] = [
        0.09375,             // bit_length(6) / 32
        0.046875,            // 3 / 64
        0.16666666666666666, // 1 distinct variable / 6 nodes
        1.0,                 // 5 distinct (symbol, arity) pairs / 5 symbols
        0.14285714285714285, // 1 / (6 + 1)
        0.25,                // 2 / (6 + 2)
        0.046875,            // 3 / 64
        0.2078431372549019,  // first digest byte 154 / 127.5 - 1
    ];
    const GOLDEN_HASH: &str = "7a9e88425c8403d2741ceb80fb3b5962a35fb7353db45f8c95669d04dcaee055";
    assert_eq!(embedding.coefficients(), GOLDEN);
    assert_eq!(
        InverseStateEmbeddingV1::VERSION,
        1,
        "the descriptor version is frozen"
    );
    assert_eq!(
        InverseStateEmbeddingV1::schema_digest().to_hex(),
        InverseStateEmbeddingV1::PINNED_DIGEST,
        "the component table's digest is pinned"
    );
    assert_eq!(
        embedding.hash().to_hex(),
        GOLDEN_HASH,
        "the embedding hash is golden"
    );
    // The `Cl(3,0,0)` multivector the graph stores carries exactly the eight
    // coefficients, in blade order.
    assert_eq!(
        embedding.to_multivector().as_slice(),
        GOLDEN.as_slice(),
        "the stored multivector is the descriptor"
    );
}

/// Every component is finite on every input: degenerate states, saturating
/// counts, and each hard ceiling boundary.
#[test]
fn embedding_components_are_finite_at_every_boundary() {
    let limits = limits();

    // Degenerate: a lone variable. One node, no symbol node at all, so every
    // symbol-relative ratio is defined as zero instead of 0/0.
    let lone_variable =
        InverseStateEmbeddingV1::of_state(&state(Term::var("X")), 0, 0, &limits).expect("admitted");
    assert!(lone_variable.coefficients().iter().all(|c| c.is_finite()));
    assert_eq!(
        lone_variable.component(2).unwrap(),
        1.0,
        "one variable, one node"
    );
    assert_eq!(
        lone_variable.component(3).unwrap(),
        0.0,
        "no symbol node means zero arity diversity, never NaN"
    );

    // Saturating counts: the deepest admissible search depth and the largest
    // possible existential count both saturate at their ceilings instead of
    // overflowing a `usize` sum.
    let saturated =
        InverseStateEmbeddingV1::of_state(&state(Term::var("X")), u64::MAX, usize::MAX, &limits)
            .expect("admitted");
    assert!(saturated.coefficients().iter().all(|c| c.is_finite()));
    assert_eq!(
        saturated.component(6).unwrap(),
        1.0,
        "search depth saturates"
    );
    // `usize::MAX` existentials against one node saturates the ratio at its
    // upper boundary in `f64` — finite, and still inside `[0, 1]`.
    assert_eq!(saturated.component(5).unwrap(), 1.0);

    // Ceiling boundaries: depth exactly at the ceiling, and the largest term
    // the relation limits admit (a complete binary tree of 4095 nodes).
    let deepest = InverseStateEmbeddingV1::of_state(
        &state(chain(RelationLimits::MAX_TERM_DEPTH)),
        0,
        0,
        &limits,
    )
    .expect("admitted at the depth ceiling");
    assert_eq!(deepest.component(1).unwrap(), 1.0);

    let largest = InverseStateEmbeddingV1::of_state(&state(balanced(12)), 0, 0, &limits)
        .expect("admitted under the node ceiling");
    assert_eq!(
        largest.component(0).unwrap(),
        12.0 / 32.0,
        "4095 nodes occupy 12 bits, and 12 / 32 is the pinned bucket"
    );

    // Every component of every admitted descriptor is inside its documented
    // range: `[0, 1]` except the signed sketch's `[-1, 1]`.
    for descriptor in [&lone_variable, &saturated, &deepest, &largest] {
        for index in 0..7 {
            let value = descriptor.component(index).expect("a component");
            assert!((0.0..=1.0).contains(&value), "component {index} = {value}");
        }
        let sketch = descriptor.component(7).expect("the sketch");
        assert!((-1.0..=1.0).contains(&sketch), "sketch = {sketch}");
    }

    // An over-large term is rejected by admission, not encoded.
    let error = InverseStateEmbeddingV1::of_state(
        &state(chain(RelationLimits::MAX_TERM_DEPTH + 1)),
        0,
        0,
        &limits,
    )
    .expect_err("a term past the depth ceiling is not admitted");
    assert!(matches!(
        error,
        NetworkError::LimitExceeded {
            resource: "term depth",
            ..
        }
    ));
}

/// Two states whose descriptors collide stay two distinct nodes: node identity
/// is the canonical state digest and never embedding equality.
#[test]
fn colliding_embeddings_keep_distinct_node_identity() {
    let (first, second, embedding) = colliding_states();
    assert_ne!(
        first.canonical_digest(),
        second.canonical_digest(),
        "the colliding pair is two distinct states"
    );
    let same = InverseStateEmbeddingV1::of_state(&second, 1, 0, &limits()).expect("admitted");
    assert_eq!(
        embedding.coefficients(),
        same.coefficients(),
        "the pair collides in every coefficient"
    );
    assert_eq!(
        embedding.hash(),
        same.hash(),
        "and therefore in the embedding hash"
    );

    let mut graph = InverseSearchGraph::new(InverseGraphConfig::default());
    let first_index = graph
        .insert(GraphNodeInput::new(&first, 1, 0))
        .expect("admitted");
    let second_index = graph
        .insert(GraphNodeInput::new(&second, 1, 0))
        .expect("admitted");
    assert_ne!(
        first_index, second_index,
        "a colliding embedding never merges two symbolic states"
    );
    assert_eq!(graph.num_nodes(), 2);
    assert_eq!(
        graph.node_embedding(first_index).unwrap().coefficients(),
        graph.node_embedding(second_index).unwrap().coefficients(),
        "the two nodes really do share a descriptor"
    );
    assert_eq!(
        graph.node_digest(first_index),
        Some(first.canonical_digest())
    );
    assert_eq!(
        graph.node_digest(second_index),
        Some(second.canonical_digest())
    );
    assert_eq!(
        graph.index_of(&second.canonical_digest()),
        Some(second_index)
    );
}

/// A transition-derived embedding is bit-identical to the state-derived
/// embedding: the descriptor is a function of content, not of how the state was
/// reached, and the graph's node descriptor agrees with both.
#[test]
fn state_and_transition_embeddings_agree() {
    let system = two_candidate_system();
    let round = Round::of(&system, &parse("x"));
    let candidates = round.candidates();
    assert_eq!(candidates.len(), 2);

    let mut graph = InverseSearchGraph::new(InverseGraphConfig::default());
    for candidate in &candidates {
        let from_transition =
            InverseStateEmbeddingV1::of_candidate(candidate, &limits()).expect("admitted");
        let from_state = InverseStateEmbeddingV1::of_state(
            candidate.state,
            candidate.score.depth,
            candidate.predecessor.existentials.len(),
            &limits(),
        )
        .expect("admitted");
        assert_eq!(from_transition, from_state, "parity in every coefficient");
        assert_eq!(
            from_transition.hash(),
            from_state.hash(),
            "parity in the embedding hash"
        );

        let index = graph
            .insert(GraphNodeInput::of_candidate(candidate))
            .expect("admitted");
        assert_eq!(
            graph.node_embedding(index),
            Some(&from_transition),
            "the node descriptor is the candidate descriptor"
        );
        assert_eq!(
            graph.node_digest(index),
            Some(candidate.state.canonical_digest())
        );
    }
}

/// The geometric score's lanes are individually transparent and its total is
/// exactly the documented weighted sum, in the documented lane order.
#[test]
fn distance_novelty_and_branching_components_are_transparent() {
    let system = two_candidate_system();
    let round = Round::of(&system, &parse("x"));
    let candidates = round.candidates();

    // A graph that already knows the first candidate's state: it is a revisit
    // at distance zero, while the second candidate is novel.
    let mut graph = InverseSearchGraph::new(InverseGraphConfig::default());
    graph
        .insert(GraphNodeInput::of_candidate(&candidates[0]))
        .expect("admitted");

    let revisit = GeometricScore::of(&candidates[0], &graph).expect("scored");
    assert_eq!(
        revisit.distance(),
        0.0,
        "an exact node match is distance zero"
    );
    assert_eq!(revisit.revisit(), 1.0, "the state is already a node");
    assert_eq!(revisit.novelty(), 0.0);
    assert_eq!(revisit.cycle(), 0.0);
    assert_eq!(revisit.constraints(), 0.0);
    assert_eq!(revisit.existentials(), 0.0);
    assert_eq!(
        revisit.branching(),
        2.0 / 256.0,
        "the round's branch factor, normalized by its hard ceiling"
    );
    assert_eq!(revisit.depth(), 1.0 / 64.0, "the constant term's depth");

    let novel = GeometricScore::of(&candidates[1], &graph).expect("scored");
    assert_eq!(novel.revisit(), 0.0);
    assert_eq!(novel.novelty(), 1.0);
    assert!(
        novel.distance() > 0.0,
        "a distinct state is at a positive distance from the known one"
    );
    assert_eq!(novel.depth(), 2.0 / 64.0, "the two-node term's depth");
    assert_eq!(novel.branching(), revisit.branching());

    // The documented formula, spelled out here in lane order.
    for score in [&revisit, &novel] {
        let expected = 0.25 * score.distance()
            + 0.20 * score.revisit()
            + 0.10 * score.branching()
            + 0.15 * score.depth()
            + 0.10 * score.constraints()
            + 0.10 * score.existentials()
            + 0.10 * score.cycle();
        assert_eq!(score.total(), expected, "the total is the weighted sum");
        assert!((0.0..=1.0).contains(&score.total()));
    }
    assert_eq!(
        GeometricScoreTable::WEIGHTS,
        [0.25, 0.20, 0.10, 0.15, 0.10, 0.10, 0.10],
        "the weights are frozen"
    );
    assert_eq!(
        GeometricScoreTable::digest().to_hex(),
        GeometricScoreTable::PINNED_DIGEST,
        "the score table's digest is pinned"
    );

    // An empty graph knows nothing: no nearest node, and every candidate is
    // maximally distant.
    let empty = InverseSearchGraph::new(InverseGraphConfig::default());
    assert_eq!(empty.num_nodes(), 0);
    let probe = InverseStateEmbeddingV1::of_state(&state(Term::constant("z")), 0, 0, &limits())
        .expect("admitted");
    assert_eq!(empty.distance_to_nearest(&probe), None);
    let unknown = GeometricScore::of(&candidates[1], &empty).expect("scored");
    assert_eq!(unknown.distance(), 1.0);
    assert_eq!(unknown.revisit(), 0.0);
}

/// A partial graph — a subset of the reachable states — and cyclic edges are
/// safe: no recursion, no unbounded growth, no duplicated edges.
#[test]
fn partial_graph_and_cycles_are_safe() {
    let mut graph = InverseSearchGraph::new(InverseGraphConfig::default());
    let nodes: Vec<SymbolicState> = (0..200u32)
        .map(|index| state(Term::constant(format!("c{index}"))))
        .collect();

    // A partial graph — only these 200 states are known — answers against what
    // it holds; the missing remainder of the state space is never needed.
    let indices: Vec<usize> = nodes
        .iter()
        .enumerate()
        .map(|(depth, node)| {
            graph
                .insert(GraphNodeInput::new(node, depth as u64, 0))
                .expect("admitted")
        })
        .collect();
    assert_eq!(graph.num_nodes(), 200);

    // A cyclic system's rounds witness a genuine three-cycle: `a -> c`,
    // `b -> a`, `c -> b`. The final edge is a long back-edge closing the
    // cycle, and every edge is a transition the engine actually produced.
    let system = cycle_system();
    let rounds: Vec<Round> = ["a", "b", "c"]
        .iter()
        .map(|target| Round::of(&system, &parse(target)))
        .collect();
    for round in &rounds {
        let steps = round.witness_steps(&system, &limits());
        graph
            .observe_round(GraphNodeInput::new(&round.parent, 0, 0), &steps)
            .expect("admitted");
    }
    assert_eq!(graph.num_nodes(), 203, "the cycle's three states joined");
    assert_eq!(graph.num_edges(), 3, "one witnessed edge per round");
    let a = graph
        .index_of(&state(parse("a")).canonical_digest())
        .expect("a is a node");
    let b = graph
        .index_of(&state(parse("b")).canonical_digest())
        .expect("b is a node");
    let c = graph
        .index_of(&state(parse("c")).canonical_digest())
        .expect("c is a node");
    assert!(graph.has_edge(a, c));
    assert!(graph.has_edge(c, b));
    assert!(graph.has_edge(b, a), "the back-edge closes the cycle");

    // Re-observing a round adds no duplicate edge, and an unwitnessed
    // self-loop is a typed rejection rather than an ordinary edge.
    let candidates = rounds[0].candidates();
    let steps = rounds[0].witness_steps(&system, &limits());
    graph
        .observe_round(GraphNodeInput::new(&rounds[0].parent, 0, 0), &steps)
        .expect("already observed");
    assert_eq!(graph.num_edges(), 3, "duplicate edges are not re-added");
    let witness =
        TransitionWitness::validate(&system, &rounds[0].parent, &candidates[0], &limits())
            .expect("the engine validates its own round");
    assert!(matches!(
        graph.connect(
            witness,
            GraphNodeInput::new(&rounds[0].parent, 0, 0),
            GraphNodeInput::new(&rounds[0].parent, 0, 0),
        ),
        Err(NetworkError::UnwitnessedTransition)
    ));
    assert_eq!(
        graph.num_edges(),
        3,
        "the unwitnessed self-loop added nothing"
    );

    // The cycle is walked as a lookup, not a recursion: re-inserting a node
    // and re-reading its index are constant-time and never grow the graph.
    for _ in 0..64 {
        assert_eq!(
            graph
                .insert(GraphNodeInput::new(&nodes[0], 0, 0))
                .expect("admitted"),
            indices[0]
        );
        assert_eq!(
            graph.index_of(&nodes[199].canonical_digest()),
            Some(indices[199])
        );
    }
    assert_eq!(graph.num_nodes(), 203);
    assert_eq!(graph.num_edges(), 3);
    assert!(graph
        .distance_to_nearest(graph.node_embedding(0).expect("a node"))
        .is_some());
}

/// Ceilings bound work before allocation, and a caller may tighten them but
/// never widen them.
#[test]
fn limits_are_enforced_before_allocation() {
    // Configuration ceilings.
    assert!(matches!(
        InverseGraphConfig::new(0, 1, limits()),
        Err(NetworkError::LimitExceeded {
            resource: "graph nodes",
            ..
        })
    ));
    assert!(matches!(
        InverseGraphConfig::new(MAX_GRAPH_NODES + 1, 1, limits()),
        Err(NetworkError::LimitExceeded {
            resource: "graph nodes",
            ..
        })
    ));
    assert!(matches!(
        InverseGraphConfig::new(1, 0, limits()),
        Err(NetworkError::LimitExceeded {
            resource: "graph edges",
            ..
        })
    ));
    assert!(matches!(
        InverseGraphConfig::new(1, MAX_GRAPH_EDGES + 1, limits()),
        Err(NetworkError::LimitExceeded {
            resource: "graph edges",
            ..
        })
    ));
    assert!(GeometricStrategyConfig::new(0).is_err());
    assert!(GeometricStrategyConfig::new(MAX_GEOMETRIC_CANDIDATES + 1).is_err());
    assert_eq!(
        GeometricStrategyConfig::default().max_candidates(),
        MAX_GEOMETRIC_CANDIDATES
    );

    // A two-node graph rejects the third node without allocating a descriptor
    // or a node slot.
    let mut graph =
        InverseSearchGraph::new(InverseGraphConfig::new(2, 1, limits()).expect("admitted"));
    let a = state(Term::constant("a"));
    let b = state(Term::constant("b"));
    let c = state(Term::constant("c"));
    graph.insert(GraphNodeInput::new(&a, 0, 0)).expect("first");
    graph.insert(GraphNodeInput::new(&b, 0, 0)).expect("second");
    assert!(matches!(
        graph.insert(GraphNodeInput::new(&c, 0, 0)),
        Err(NetworkError::LimitExceeded {
            resource: "graph nodes",
            ..
        })
    ));
    assert_eq!(graph.num_nodes(), 2, "the rejected node was never added");

    // One witnessed edge is admitted; the second is rejected by the edge
    // ceiling and not recorded.
    let system = two_candidate_system();
    let round = Round::of(&system, &parse("x"));
    let candidates = round.candidates();
    let parent = GraphNodeInput::new(&round.parent, 0, 0);
    let mut edges =
        InverseSearchGraph::new(InverseGraphConfig::new(3, 1, limits()).expect("admitted"));
    let witness = |index: usize| {
        TransitionWitness::validate(&system, &round.parent, &candidates[index], &limits())
            .expect("the engine validates its own round")
    };
    edges
        .connect(
            witness(0),
            parent,
            GraphNodeInput::of_candidate(&candidates[0]),
        )
        .expect("first edge");
    assert!(matches!(
        edges.connect(
            witness(1),
            parent,
            GraphNodeInput::of_candidate(&candidates[1]),
        ),
        Err(NetworkError::LimitExceeded {
            resource: "graph edges",
            ..
        })
    ));
    assert_eq!(edges.num_edges(), 1);

    // The descriptor's computation is sized by the relation ceilings.
    assert!(matches!(
        InverseStateEmbeddingV1::of_state(
            &state(chain(RelationLimits::MAX_TERM_DEPTH + 1)),
            0,
            0,
            &limits()
        ),
        Err(NetworkError::LimitExceeded { .. })
    ));

    // Candidate admission precedes every descriptor: an over-full round is
    // rejected with nothing scored.
    let round = Round::of(&system, &parse("x"));
    let candidates = round.candidates();
    let mode = GuidanceMode::CompleteWithinLimits;
    let request = round.request(&candidates, &mode);
    let strategy = GeometricStrategy::new(
        InverseSearchGraph::new(InverseGraphConfig::default()),
        GeometricStrategyConfig::new(1).expect("admitted"),
    );
    let failure = strategy.rank(&request).expect_err("over-full round");
    assert_eq!(
        failure.kind,
        GuidanceFailureKind::LimitExceeded {
            resource: "geometric strategy candidates",
            value: 2,
            limit: 1,
        }
    );
    assert_eq!(failure.candidates, 2);
    assert_eq!(failure.scored, 0, "nothing was scored");
}

/// A round's ranking is a function of content alone, and it is always a
/// permutation of the engine's candidates — never a generation, never a drop.
#[test]
fn ranking_is_deterministic_and_a_permutation() {
    let system = two_candidate_system();
    let round = Round::of(&system, &parse("x"));
    let candidates = round.candidates();
    let mode = GuidanceMode::CompleteWithinLimits;
    let strategy = GeometricStrategy::new(
        InverseSearchGraph::new(InverseGraphConfig::default()),
        GeometricStrategyConfig::default(),
    );

    let request = round.request(&candidates, &mode);
    let forward = strategy.rank(&request).expect("ranked");
    assert_eq!(forward.len(), candidates.len());
    validate_ranking(&forward, candidates.len()).expect("a permutation");
    let mut sorted = forward.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec![0, 1], "no index is dropped or invented");

    // Determinism: the same request ranks identically, forever.
    for _ in 0..4 {
        assert_eq!(strategy.rank(&request).expect("ranked"), forward);
    }

    // Permutation invariance: reversing the request permutes the indices but
    // not the canonical state order.
    let mut reversed_candidates = candidates.clone();
    reversed_candidates.reverse();
    let reversed_request = round.request(&reversed_candidates, &mode);
    let backward = strategy.rank(&reversed_request).expect("ranked");
    let forward_digests: Vec<_> = forward
        .iter()
        .map(|index| round.states[*index].canonical_digest())
        .collect();
    let backward_digests: Vec<_> = backward
        .iter()
        .map(|index| round.states[candidates.len() - 1 - *index].canonical_digest())
        .collect();
    assert_eq!(forward_digests, backward_digests);

    // The pinned direction: the preferred candidate is the LOWEST total, and a
    // cycle-flagged round is strictly worse than the same round without it.
    let plain = GeometricScore::of(&candidates[0], strategy.graph()).expect("scored");
    let mut cycled_score = round.scores[0].clone();
    cycled_score.cycle = true;
    let cycled_candidate = GuidanceCandidate {
        state: &round.states[0],
        predecessor: &round.predecessors[0],
        score: &cycled_score,
    };
    let cycled = GeometricScore::of(&cycled_candidate, strategy.graph()).expect("scored");
    assert!(
        cycled.total() > plain.total() + 0.09,
        "a cycle flag costs its full lane weight"
    );

    // A one-candidate round is still a permutation, and an empty round is
    // empty: the strategy never fabricates work.
    let single = &candidates[..1];
    let single_request = round.request(single, &mode);
    assert_eq!(strategy.rank(&single_request).expect("ranked"), vec![0]);
    let none: [GuidanceCandidate<'_>; 0] = [];
    let empty_request = round.request(&none, &mode);
    assert!(strategy.rank(&empty_request).expect("ranked").is_empty());
}

/// The descriptor's identity surface is versioned and frozen: the component
/// table's names, its digest, and the strategy's replay hash all track the
/// descriptor's version.
#[test]
fn embedding_schema_is_versioned_and_frozen() {
    assert_eq!(InverseStateEmbeddingV1::WIDTH, 8);
    assert_eq!(InverseStateEmbeddingV1::VERSION, 1);
    let names: Vec<&str> = InverseStateEmbeddingV1::COMPONENTS
        .iter()
        .map(|EmbeddingComponent { name, .. }| *name)
        .collect();
    assert_eq!(names, COMPONENT_NAMES);

    // Two descriptors that differ in one component hash differently, and the
    // same descriptor hashes identically every time.
    let shallow = InverseStateEmbeddingV1::of_state(&state(Term::constant("a")), 1, 0, &limits())
        .expect("admitted");
    let deep = InverseStateEmbeddingV1::of_state(
        &state(Term::sym("f", [Term::constant("a")])),
        1,
        0,
        &limits(),
    )
    .expect("admitted");
    assert_ne!(shallow.hash(), deep.hash());
    assert_eq!(
        shallow.hash(),
        InverseStateEmbeddingV1::of_state(&state(Term::constant("a")), 1, 0, &limits())
            .expect("admitted")
            .hash()
    );

    // The strategy's replay identity covers the descriptor schema, the score
    // table, the graph's content, and its admission ceilings.
    let empty_graph = InverseSearchGraph::new(InverseGraphConfig::default());
    let a = GeometricStrategy::new(empty_graph, GeometricStrategyConfig::default());
    let mut populated = InverseSearchGraph::new(InverseGraphConfig::default());
    populated
        .insert(GraphNodeInput::new(&state(Term::constant("a")), 0, 0))
        .expect("admitted");
    let b = GeometricStrategy::new(populated, GeometricStrategyConfig::default());
    assert_ne!(
        a.replay_hash(),
        b.replay_hash(),
        "a changed graph is a changed strategy"
    );
    let tightened = GeometricStrategy::new(
        InverseSearchGraph::new(InverseGraphConfig::default()),
        GeometricStrategyConfig::new(2).expect("admitted"),
    );
    assert_ne!(
        a.replay_hash(),
        tightened.replay_hash(),
        "a changed ceiling is a changed strategy"
    );
    assert_eq!(
        GuidanceStrategy::strategy_hash(&a),
        a.replay_hash(),
        "the guidance seam reports the same identity"
    );
}

/// Ordering only, end to end: a geometric strategy in complete mode changes the
/// visit order and nothing else, so an exhausted closure keeps its certified
/// evidence hash — and the graph the strategy scores against was built from one
/// engine round, consuming transitions the engine had already validated.
#[test]
fn complete_mode_keeps_the_evidence_hash_with_a_geometric_strategy() {
    let system = cycle_system();
    let target = parse("a");
    let goal = parse("z");

    let baseline = BackwardExplorer::new(&system, small_search_config())
        .search(&target, &goal, SearchMode::BreadthFirst)
        .expect("the engine runs");
    let BackwardSearchOutcome::Exhausted(plain) = &baseline else {
        panic!("baseline must exhaust: {baseline:?}");
    };

    // The graph learns one already-valid round: `a`'s predecessor `c`, with the
    // edge `a -> c`. Nothing here can invent a state or a transition.
    let round = Round::of(&system, &target);
    let candidates = round.candidates();
    let mode = GuidanceMode::CompleteWithinLimits;
    let mut graph = InverseSearchGraph::new(InverseGraphConfig::default());
    let steps = round.witness_steps(&system, &limits());
    let parent = graph
        .observe_round(GraphNodeInput::new(&round.parent, 0, 0), &steps)
        .expect("admitted");
    assert_eq!(graph.num_nodes(), 2);
    assert_eq!(graph.num_edges(), candidates.len());
    assert_eq!(graph.node_depth(parent), Some(0));
    assert!(graph.has_edge(parent, graph.num_nodes() - 1));
    assert_eq!(graph.network().num_nodes(), graph.num_nodes());

    let strategy = GeometricStrategy::new(graph, GeometricStrategyConfig::default());
    let guided = BackwardExplorer::new(&system, small_search_config())
        .search_with_strategy(
            &target,
            &goal,
            SearchMode::BreadthFirst,
            &mode,
            Some(&strategy),
        )
        .expect("the engine runs");
    let GuidedSearchOutcome::Completed(BackwardSearchOutcome::Exhausted(geometric)) = guided else {
        panic!("geometric complete mode must still exhaust: {guided:?}");
    };
    // Identical evidence hash == identical explored set == identical replay
    // identity: ordering changed nothing about authority.
    assert_eq!(plain.evidence_hash(), geometric.evidence_hash());
    assert_eq!(plain.authority(), geometric.authority());
}

/// **F1 reproduction (PR #382 round 1).** Two graphs admit exactly the same
/// candidate states — identical canonical digests, identical node order — but
/// store different insertion metadata (search depth 64 / `usize::MAX`
/// existentials versus 1 / 0). The stored descriptors are ranking-relevant, so
/// the two rankings really do differ; the replay identity must not alias them.
#[test]
fn replay_hash_covers_ranking_relevant_graph_state() {
    let system = two_candidate_system();
    let round = Round::of(&system, &parse("x"));
    let candidates = round.candidates();
    let mode = GuidanceMode::CompleteWithinLimits;
    let request = round.request(&candidates, &mode);

    let build = |metadata: &[(u64, usize)]| {
        let mut graph = InverseSearchGraph::new(InverseGraphConfig::default());
        for (candidate, (depth, existentials)) in candidates.iter().zip(metadata) {
            graph
                .insert(GraphNodeInput::new(candidate.state, *depth, *existentials))
                .expect("admitted");
        }
        graph
    };

    let fresh =
        GeometricStrategy::new(build(&[(1, 0), (1, 0)]), GeometricStrategyConfig::default());
    let saturated = GeometricStrategy::new(
        build(&[(64, usize::MAX), (1, 0)]),
        GeometricStrategyConfig::default(),
    );

    // Identical candidate sets: the alias is not a different node set.
    let digests = |graph: &InverseSearchGraph| -> Vec<_> {
        (0..graph.num_nodes())
            .map(|index| graph.node_digest(index).expect("a node"))
            .collect()
    };
    assert_eq!(
        digests(fresh.graph()),
        digests(saturated.graph()),
        "the reproduction is two graphs over the same states"
    );
    assert_eq!(
        fresh.graph().num_nodes(),
        saturated.graph().num_nodes(),
        "nor is it a different node count"
    );

    // The stored metadata is ranking-relevant: the same round ranks in the
    // opposite order (the reviewer's [0, 1] versus [1, 0]).
    assert_eq!(fresh.rank(&request).expect("ranked"), vec![0, 1]);
    assert_eq!(saturated.rank(&request).expect("ranked"), vec![1, 0]);

    // Therefore a replay identity that only covered the state digests aliased
    // two different rankings. It must cover the ranking-relevant snapshot.
    assert_ne!(
        fresh.replay_hash(),
        saturated.replay_hash(),
        "rankings differ, so replay identities must differ"
    );
    // Determinism is untouched: identical inputs hash identically.
    assert_eq!(
        fresh.replay_hash(),
        GeometricStrategy::new(build(&[(1, 0), (1, 0)]), GeometricStrategyConfig::default())
            .replay_hash()
    );
}

/// **F2 remediation (PR #382 round 1).** An edge is admitted only against an
/// engine witness: the reproduction's empty system has no predecessor, so its
/// `a -> b` link can never be recorded — and a witness minted for a different
/// round is a typed rejection that admits nothing.
#[test]
fn unwitnessed_transitions_are_rejected() {
    let empty = system(Vec::new());
    let a = state(parse("a"));
    let b = state(parse("b"));
    assert!(
        symbolic_predecessors(&empty, a.term(), &limits(), 0)
            .expect("the empty round")
            .is_empty(),
        "an empty system has no predecessor, so no engine witness exists"
    );

    // A real round elsewhere mints a witness -- for `x -> b`, not for `a -> b`.
    let system = two_candidate_system();
    let round = Round::of(&system, &parse("x"));
    let candidates = round.candidates();
    let candidate = candidates
        .iter()
        .find(|candidate| candidate.state.canonical_digest() == b.canonical_digest())
        .expect("the round derives b");
    let witness = TransitionWitness::validate(&system, &round.parent, candidate, &limits())
        .expect("the engine validates its own round");
    assert_eq!(witness.from_digest(), round.parent.canonical_digest());
    assert_eq!(witness.to_digest(), b.canonical_digest());
    assert_eq!(
        witness.provenance_digest(),
        TransitionWitness::validate(&system, &round.parent, candidate, &limits())
            .expect("the engine validates its own round")
            .provenance_digest(),
        "the witness is a deterministic copy of the engine's validation"
    );

    // The reviewer's reproduction: `a -> b` has no engine witness, so the pair
    // the witness does not name is rejected and the graph stays empty.
    let mut graph = InverseSearchGraph::new(InverseGraphConfig::default());
    let outcome = graph.connect(
        witness,
        GraphNodeInput::new(&a, 0, 0),
        GraphNodeInput::new(&b, 0, 0),
    );
    assert!(
        matches!(outcome, Err(NetworkError::UnwitnessedTransition)),
        "an unwitnessed edge must be a typed rejection, got {outcome:?}"
    );
    assert_eq!(graph.num_nodes(), 0, "a rejected edge admits no endpoint");
    assert_eq!(graph.num_edges(), 0, "no edge is recorded");

    // The same holds for a self-loop: an unwitnessed `a -> a` is rejected.
    assert!(matches!(
        graph.connect(
            witness,
            GraphNodeInput::new(&a, 0, 0),
            GraphNodeInput::new(&a, 0, 0)
        ),
        Err(NetworkError::UnwitnessedTransition)
    ));
    assert_eq!(graph.num_edges(), 0, "no self-loop is recorded either");

    // The witnessed pair itself is admitted, and an existing edge is not
    // re-added. Reversing the witness's endpoints is again a rejection.
    let parent = GraphNodeInput::new(&round.parent, 0, 0);
    let target = GraphNodeInput::of_candidate(candidate);
    let (source, sink) = graph.connect(witness, parent, target).expect("witnessed");
    assert_eq!(graph.num_edges(), 1);
    assert!(graph.has_edge(source, sink));
    assert_eq!(
        graph.connect(witness, parent, target).expect("present"),
        (source, sink)
    );
    assert_eq!(graph.num_edges(), 1, "duplicate edges are not re-added");
    assert!(matches!(
        graph.connect(witness, target, parent),
        Err(NetworkError::UnwitnessedTransition)
    ));
    assert_eq!(graph.num_edges(), 1, "the reversed pair was never admitted");
}

/// **F4 reproduction (PR #382 round 2).** Witness minting is the engine's, not
/// the caller's. The reviewer's three cases — a genuine predecessor paired with
/// an unrelated parent, a substituted candidate state posing as a self-loop, and
/// a fabricated round pushed through the bulk path — are all typed rejections,
/// and none of them admits a node or an edge.
///
/// The positive control matters as much as the negative ones: the same
/// engine-side boundary accepts the round it just derived, so this test cannot
/// be satisfied by a constructor that rejects everything.
#[test]
fn fabricated_transitions_cannot_mint_witnesses() {
    let limits = limits();
    let empty = system(Vec::new());
    let a = state(parse("a"));
    let b = state(parse("b"));
    let x = state(parse("x"));
    assert!(
        symbolic_predecessors(&empty, a.term(), &limits, 0)
            .expect("the empty round")
            .is_empty(),
        "an empty system derives no predecessor of `a`"
    );

    // A genuine engine round elsewhere: rule `b => x`.
    let other = system(vec![Rule::new(parse("b"), parse("x")).unwrap()]);
    let round = Round::of(&other, x.term());
    let candidates = round.candidates();
    let genuine = candidates
        .iter()
        .find(|candidate| candidate.state.canonical_digest() == b.canonical_digest())
        .expect("the round derives b");
    let genuine_witness = TransitionWitness::validate(&other, &round.parent, genuine, &limits)
        .expect("the engine validates the round it derived");
    assert_eq!(genuine_witness.from_digest(), x.canonical_digest());
    assert_eq!(genuine_witness.to_digest(), b.canonical_digest());

    // Case 1 — the reviewer's reproduction: a real `x -> b` record paired with
    // an unrelated parent `a`. The engine derives no predecessor of `a` under
    // the empty system, so no witness exists: the typed rejection, not an
    // `Ok((0, 1))` edge.
    let unrelated = TransitionWitness::validate(&empty, &a, genuine, &limits);
    assert!(
        matches!(unrelated, Err(NetworkError::UnwitnessedTransition)),
        "an unrelated parent must be a typed rejection, got {unrelated:?}"
    );

    // Case 2 — the candidate state substituted for the parent (`a` posing as
    // its own predecessor). The candidate state's digest is re-derived from the
    // predecessor, so this cannot mint either: no `Ok((0, 0))` self-loop.
    let substituted = GuidanceCandidate {
        state: &a,
        predecessor: genuine.predecessor,
        score: genuine.score,
    };
    let forged = TransitionWitness::validate(&other, &a, &substituted, &limits);
    assert!(
        matches!(forged, Err(NetworkError::UnwitnessedTransition)),
        "a substituted candidate state must be a typed rejection, got {forged:?}"
    );
    assert!(
        TransitionWitness::validate(&other, &round.parent, &substituted, &limits).is_err(),
        "nor may the parent's own round bridge the substitution"
    );

    // Case 3 — a fabricated round through the bulk path. `GuidanceRequest` is
    // publicly constructible, but it is not a witness: every entry of it is a
    // typed rejection at the minting boundary, so `observe_round` has no token
    // to consume and records nothing.
    let mode = GuidanceMode::CompleteWithinLimits;
    let request = GuidanceRequest {
        parent: &a,
        depth: 0,
        mode: &mode,
        candidates: &[substituted],
    };
    assert_eq!(
        request.candidates.len(),
        1,
        "the fabricated round has one entry"
    );
    for candidate in request.candidates {
        let minted = TransitionWitness::validate(&other, request.parent, candidate, &limits);
        assert!(
            matches!(minted, Err(NetworkError::UnwitnessedTransition)),
            "a fabricated round entry must be a typed rejection, got {minted:?}"
        );
    }
    let mut graph = InverseSearchGraph::new(InverseGraphConfig::default());
    let steps: Vec<(TransitionWitness, GraphNodeInput<'_>)> = Vec::new();
    graph
        .observe_round(GraphNodeInput::new(&a, 0, 0), &steps)
        .expect("an empty round records nothing");
    assert_eq!(
        graph.num_nodes(),
        1,
        "only the observed parent was admitted"
    );
    assert_eq!(graph.num_edges(), 0, "a fabricated round records no edge");
    assert!(
        !graph.has_edge(0, 0),
        "in particular, no invented self-loop"
    );

    // And the genuine round still reaches the graph unchanged: the boundary
    // admits what the engine derived and nothing else.
    let steps = round.witness_steps(&other, &limits);
    let mut graph = InverseSearchGraph::new(InverseGraphConfig::default());
    let parent = graph
        .observe_round(GraphNodeInput::new(&round.parent, 0, 0), &steps)
        .expect("the genuine round is admitted");
    assert_eq!(graph.num_edges(), candidates.len());
    assert_eq!(graph.node_digest(parent), Some(x.canonical_digest()));
    let (source, sink) = graph.edges().next().expect("one recorded transition");
    assert_eq!(graph.node_digest(source), Some(x.canonical_digest()));
    assert_eq!(graph.node_digest(sink), Some(b.canonical_digest()));
}

/// **F3 reproduction (PR #382 round 1).** Two variable names sharing a 16 MiB
/// prefix make the descriptor's sort/dedup scan unbounded bytes with no
/// admission and no charge. The identifier ceiling (Task 40 doctrine) is
/// applied to the borrowed name before anything is collected.
#[test]
fn oversized_identifiers_are_rejected_before_the_descriptor_sorts_them() {
    const IDENTIFIER_CEILING: usize = MAX_IDENTIFIER_BYTES;
    assert_eq!(MAX_IDENTIFIER_BYTES, 256, "Task 40's ceiling, unchanged");
    let tight = RelationLimits::new(3, 2, 1, 3).expect("admitted");
    let prefix = "x".repeat(16 * 1024 * 1024);
    let term = Term::sym(
        "f",
        [
            Term::var(format!("{prefix}a")),
            Term::var(format!("{prefix}b")),
        ],
    );
    let rejected = InverseStateEmbeddingV1::of_state(&state(term), 0, 0, &tight);
    assert!(
        matches!(
            rejected,
            Err(NetworkError::LimitExceeded {
                resource: "identifier bytes",
                value,
                limit,
            }) if limit == IDENTIFIER_CEILING && value > IDENTIFIER_CEILING
        ),
        "an unbounded identifier must be a typed rejection, got {rejected:?}"
    );

    // The ceiling is a boundary, not a heuristic: a name of exactly the
    // ceiling is admitted, one byte more is rejected.
    let admitted = Term::var("y".repeat(IDENTIFIER_CEILING));
    let admitted_term = Term::sym("f", [admitted.clone(), Term::var("Z")]);
    assert!(
        InverseStateEmbeddingV1::of_state(&state(admitted_term), 0, 0, &tight).is_ok(),
        "an identifier at the ceiling is admitted"
    );
    let over = Term::sym(
        "f",
        [
            Term::var("y".repeat(IDENTIFIER_CEILING + 1)),
            Term::var("Z"),
        ],
    );
    assert!(matches!(
        InverseStateEmbeddingV1::of_state(&state(over), 0, 0, &tight),
        Err(NetworkError::LimitExceeded {
            resource: "identifier bytes",
            ..
        })
    ));
    let symbol = Term::sym("s".repeat(IDENTIFIER_CEILING + 1), Vec::new());
    assert!(matches!(
        InverseStateEmbeddingV1::of_state(&state(symbol), 0, 0, &tight),
        Err(NetworkError::LimitExceeded {
            resource: "identifier bytes",
            ..
        })
    ));
}
