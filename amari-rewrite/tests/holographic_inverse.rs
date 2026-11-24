//! Holographic trace recall contracts (0.25 Cohort 9, Task 47).
//!
//! One test per enumerated obligation of the task:
//!
//! - **frozen encoder vectors** — the deterministic `holographic-trace-v1`
//!   encoder is pinned literally: token vector digests for several terms, the
//!   first components of one token, the trace vector digests, the encoder
//!   config digest, and the schema string. A change to any formula is a
//!   visible golden failure rather than a silent re-embedding;
//! - **additive superposition** — a trace vector is exactly the
//!   [`amari_holographic::BindingAlgebra::superpose`] accumulation of its
//!   permuted tokens, coefficient-for-coefficient and bit-for-bit, and it
//!   *accumulates* magnitude. Bundling the same trace with itself returns the
//!   trace (an attention-weighted average), which is pinned as the reason the
//!   encoder composes with `superpose` and not with `bundle`;
//! - **exact repeated recall order** — the same query against the same corpus
//!   returns the same trace identities in the same order with bit-identical
//!   scores, and the order is a function of content alone (admission order
//!   permutes nothing);
//! - **seed / dimension / trace identity** — the encoder config digest covers
//!   the seed, the dimension, the schema, and the limits; the trace identity is
//!   a content hash that is independent of the seed and the dimension;
//! - **empty / malformed / oversized corpora** — typed rejections, never a
//!   silently empty or silently partial answer;
//! - **bounded admission** — every ceiling is checked before anything
//!   proportional to the admitted set is allocated, a caller may tighten a
//!   ceiling and never widen it, and a rejected admission leaves the corpus
//!   exactly as it was;
//! - **replay-validated provenance only** — a [`RecallTrace`] can only be minted
//!   by validating a [`ReplayCertificate`] against a term system, so the recall
//!   memory consumes validated inverse-search output and never generates a
//!   trace; a tampered certificate is a typed rejection;
//! - **transparent similarity evidence, pinned direction** — a match exposes the
//!   documented decomposition (`combined`, `goal_alignment`, `path_alignment`),
//!   each lane recomputes from the published vectors, and the direction
//!   convention (preferred = HIGHER) holds end to end: the identical trace
//!   ranks first, and the returned order is the descending similarity order
//!   (Cohort-8 lesson: a direction is stated *and* pinned, never implied);
//! - **pure module** — recall reads only the caller-provided corpus: two corpora
//!   never see each other's traces, and nothing is recalled that was not
//!   admitted;
//! - **default isolation** — a default-feature build exposes no holographic
//!   surface at all (this file compiles under every feature combination, the
//!   default-isolation module itself is compiled only in the exact default-like
//!   build, and the crate-root `compile_fail` doctest asserts the module path is
//!   absent from every non-`holographic-guidance` build).

#[cfg(feature = "holographic-guidance")]
mod guidance {
    use amari_holographic::{BindingAlgebra, FHRRAlgebra};
    use amari_rewrite::holographic::{
        HolographicEncoder, HolographicError, RecallConfig, RecallTrace, TraceCorpus,
        MAX_CORPUS_TRACES, MAX_RECALL_RESULTS, MAX_TRACE_DIMENSION, MAX_TRACE_TERMS,
        SCORE_DIRECTION,
    };
    use amari_rewrite::inverse::{
        BackwardExplorer, BackwardSearchOutcome, GuidanceMode, InverseSearchConfig,
        ReplayCertificate, ReplayDerivation, ReplayQuery, ResourceObservation, SearchMode,
    };
    use amari_rewrite::relation::RelationLimits;
    use amari_rewrite::trs::{Rule, Term, TermSystem};

    /// The frozen encoder seed these goldens were computed with.
    const SEED: u64 = 0x5441_534B_3437_0001;

    /// The frozen encoder dimension these goldens were computed with.
    const DIMENSION: usize = 64;

    /// Golden: framed digest of the `(seed, dimension, schema, limits)` config.
    const ENCODER_DIGEST: &str = "2278d319c0363cef701a5746d118d58f5973628d4d2054753930cfb075fc9dca";

    /// Golden: token vector digests for `a`, `b`, `p`, and the variable `X`.
    const TOKEN_A: &str = "64d2a07c0b64d531a5e423334b1c7264faf57acdd847ec548ecdc7427ca6916c";
    const TOKEN_B: &str = "442a46a8dbc79605c0ab3f14a83a0f338b3fb27e06fbc37897bf0a78917c7f04";
    const TOKEN_P: &str = "7d24bfd96e3f91bf082461c159ecabd78bce8c104555bee1842c566ecaf24362";
    const TOKEN_X: &str = "aa5e2f34dc32fe0a0c42877af9cc790ee1fb15e39305a7cb706fea985c871357";

    /// Golden: the same token at another dimension, and its config digest.
    const TOKEN_A_D128: &str = "5d920a85013284695f972ad6b31b6e2ef746f8e235dcd86aecefd31edbd9aca3";
    const ENCODER_DIGEST_D128: &str =
        "f1542eb2e08101996187f57a44adaee1ab6d3472939b0cdfb3a30086f394a1a4";

    /// Golden: trace identities. `full` is goal `a` with steps `b, a`, `short`
    /// is goal `a` with step `a`, `other` is goal `p` with step `p`.
    const TRACE_FULL: &str = "59f5045101b14a0bf77c98f74fc4f2c7458ac54ba52efbc94c5d782139f732ad";
    const TRACE_SHORT: &str = "c0e3859401d40d3364d876f6e155942c22fd0c9710b853c41ee5f84d8070da03";
    const TRACE_OTHER: &str = "d68f7834cc32e9e086b2f8cf8364ec2deecba25061f10038fd8c6e6d81d683b2";

    /// Golden: trace vector digests under [`SEED`].
    const VECTOR_FULL: &str = "3c5de83acdf21b8fa5a2fff5226df22cb6b690c9526a2aded859e56c5a6db5d8";
    const VECTOR_SHORT: &str = "0e47f54eb230542f234624619f5cc9b06621d926deb6d781aa15526698f7b0e5";
    const VECTOR_OTHER: &str = "e3ab7bcad3c83c5d01e15c442c4f60319f542bd92bf49eee28037edd19e1c4fe";

    /// Golden: the same content under a different seed shares no vector.
    const VECTOR_FULL_SEED2: &str =
        "f8644aab6ed9184ce801c1506aa620b3320f31e53c30133a3e163ca05c7d4ad7";

    /// Golden: similarity of `full` (the query) against each corpus trace, as
    /// `(combined, goal_alignment, path_alignment)`.
    const SCORE_FULL: (f64, f64, f64) = (1.0, 1.0, 1.0);
    const SCORE_SHORT: (f64, f64, f64) = (0.437_408_882_639_853_2, 1.0, -0.053_300_179_088_902_6);
    const SCORE_OTHER: (f64, f64, f64) = (0.0, -0.25, 0.266_500_895_444_513_05);

    fn constant(name: &str) -> Term {
        Term::constant(name)
    }

    /// `a => b`, `b => c`: backward from `c` to `a` derives the step terms
    /// `[b, a]` in derivation order (goal side first).
    fn chain_system() -> TermSystem {
        TermSystem::new(vec![
            Rule::new(constant("a"), constant("b")).unwrap(),
            Rule::new(constant("b"), constant("c")).unwrap(),
        ])
    }

    /// `p => q`: a disjoint system, so `other` shares nothing with `full`.
    fn disjoint_system() -> TermSystem {
        TermSystem::new(vec![Rule::new(constant("p"), constant("q")).unwrap()])
    }

    fn nested(term: Term, levels: usize) -> Term {
        (0..levels).fold(term, |inner, _| Term::sym("h", [inner]))
    }

    /// `h(h(h(h(a)))) => c`: the step term has five nodes and depth five, so a
    /// tightened walk ceiling rejects it at admission.
    fn nested_system() -> TermSystem {
        TermSystem::new(vec![
            Rule::new(nested(constant("a"), 4), constant("c")).unwrap()
        ])
    }

    fn certificate(system: &TermSystem, target: &Term, goal: &Term) -> ReplayCertificate {
        let outcome = BackwardExplorer::new(system, InverseSearchConfig::default())
            .search(target, goal, SearchMode::BreadthFirst)
            .expect("search must run");
        let BackwardSearchOutcome::Witness(derivation) = outcome else {
            panic!("the baseline system must yield a witness");
        };
        ReplayCertificate::issue(
            system,
            ReplayQuery::Backward {
                target: target.clone(),
                goal: goal.clone(),
            },
            ReplayDerivation::Backward(derivation),
            InverseSearchConfig::default(),
            GuidanceMode::CompleteWithinLimits,
            ResourceObservation {
                states: 3,
                transitions: 2,
                retained_bytes: 256,
                operations: 32,
            },
        )
    }

    fn validated(system: &TermSystem, target: &Term, goal: &Term) -> RecallTrace {
        RecallTrace::validate(system, &certificate(system, target, goal))
            .expect("a replayed certificate must validate")
    }

    /// Goal `a` with steps `b, a` (from `c`).
    fn full_trace() -> RecallTrace {
        validated(&chain_system(), &constant("c"), &constant("a"))
    }

    /// Goal `a` with step `a` (from `b`).
    fn short_trace() -> RecallTrace {
        validated(&chain_system(), &constant("b"), &constant("a"))
    }

    /// Goal `p` with step `p` (from `q`), in a disjoint system.
    fn other_trace() -> RecallTrace {
        validated(&disjoint_system(), &constant("q"), &constant("p"))
    }

    fn encoder() -> HolographicEncoder<DIMENSION> {
        HolographicEncoder::new(SEED, RelationLimits::default()).expect("the encoder is bounded")
    }

    fn corpus(traces: &[&RecallTrace]) -> TraceCorpus<DIMENSION> {
        let mut corpus = TraceCorpus::new(
            encoder(),
            RecallConfig::new(8, MAX_TRACE_TERMS, 8).expect("bounded config"),
        );
        for trace in traces {
            corpus.admit(trace).expect("admission must accept");
        }
        corpus
    }

    fn digest_hex(digest: &amari_rewrite::relation::Sha256Digest) -> String {
        digest.to_hex()
    }

    fn close(left: f64, right: f64) -> bool {
        (left - right).abs() <= 1e-12
    }

    // ---------------------------------------------------------------- goldens

    /// The encoder's token phase derivation, its vector digest, and its config
    /// digest are frozen. Renaming a schema, reordering a coefficient, changing
    /// a bit order, or touching the seed schedule fails here.
    #[test]
    fn frozen_encoder_vectors_are_golden() {
        let encoder = encoder();
        assert_eq!(encoder.dimension(), DIMENSION);
        assert_eq!(encoder.seed(), SEED);
        assert_eq!(
            digest_hex(&encoder.digest()),
            ENCODER_DIGEST,
            "the encoder config digest is pinned"
        );

        // Tokens are inspected through replay-validated traces only: `full`
        // carries `a` and `b`, `other` carries `p`, and a zero-step witness to
        // the variable `X` carries the variable token.
        let full = full_trace();
        let other = other_trace();
        let variable = validated(&chain_system(), &constant("a"), &Term::var("X"));
        assert!(variable.steps().is_empty());
        for (trace, index, expected) in [
            (&full, 0, TOKEN_A),
            (&full, 1, TOKEN_B),
            (&other, 0, TOKEN_P),
            (&variable, 0, TOKEN_X),
        ] {
            let token = encoder
                .trace_token(trace, index)
                .expect("a validated, bounded term");
            assert_eq!(encoder.dimension(), token.dimension());
            assert_eq!(digest_hex(&encoder.vector_digest(&token)), expected);
        }

        // Literal components, not only digests: the token of `a` starts with
        // these eight signs (LSB-first bit order within each digest byte).
        let token_a = encoder.trace_token(&full, 0).expect("bounded");
        let components: Vec<f64> = (0..8).map(|i| token_a.real(i).expect("in range")).collect();
        assert_eq!(components, vec![-1.0, 1.0, 1.0, 1.0, 1.0, -1.0, 1.0, 1.0]);
        // Bipolar by construction: every component is a sign, the imaginary
        // part is identically zero, and the norm is exactly `sqrt(D)`.
        for index in 0..DIMENSION {
            let real = token_a.real(index).expect("in range");
            assert!(real == 1.0 || real == -1.0, "component {index} is a sign");
            assert_eq!(token_a.imag(index), Some(0.0));
        }
        assert_eq!(token_a.norm(), (DIMENSION as f64).sqrt());

        // The dimension is part of the frozen derivation: a different `D` is a
        // different vector, not a prefix of this one.
        let wide =
            HolographicEncoder::<128>::new(SEED, RelationLimits::default()).expect("bounded");
        assert_eq!(
            digest_hex(&wide.vector_digest(&wide.trace_token(&full, 0).expect("bounded"))),
            TOKEN_A_D128
        );
        assert_eq!(digest_hex(&wide.digest()), ENCODER_DIGEST_D128);

        // The other two trace vectors are pinned too, so every corpus entry the
        // contract tests use has a frozen encoding.
        assert_eq!(
            digest_hex(&encoder.encode(&short_trace()).expect("bounded").digest),
            VECTOR_SHORT
        );
        assert_eq!(
            digest_hex(&encoder.encode(&other_trace()).expect("bounded").digest),
            VECTOR_OTHER
        );
        assert_eq!(digest_hex(&encoder.digest()).len(), 64);
        assert_eq!(SCORE_DIRECTION, "higher-is-better");
    }

    /// A trace vector is exactly the additive superposition of its permuted
    /// tokens — goal at position 0, steps at positions 1.. — accumulated with
    /// the canonical [`BindingAlgebra::superpose`], never with `bundle`.
    #[test]
    fn traces_compose_by_additive_superposition() {
        let encoder = encoder();
        let trace = full_trace();
        assert_eq!(trace.goal(), &constant("a"));
        assert_eq!(trace.steps(), &[constant("b"), constant("a")]);

        let encoding = encoder.encode(&trace).expect("a bounded trace");
        assert_eq!(
            digest_hex(&encoding.digest),
            VECTOR_FULL,
            "the frozen trace vector is pinned"
        );
        assert_eq!(
            digest_hex(&encoder.vector_digest(&encoding.vector)),
            VECTOR_FULL
        );

        // Rebuild the same vector lane by lane: token at position `i`,
        // permuted by `i`, superposed in order. The tokens come from the
        // validated trace itself (index 0 is the goal, then the steps).
        let token_sources = [0usize, 1, 0];
        let mut rebuilt = <FHRRAlgebra<DIMENSION> as BindingAlgebra>::zero();
        for (position, index) in token_sources.iter().enumerate() {
            let step = BindingAlgebra::permute(
                &encoder.trace_token(&trace, *index).expect("bounded"),
                position as i32,
            );
            rebuilt = <FHRRAlgebra<DIMENSION> as BindingAlgebra>::superpose(&rebuilt, &step)
                .expect("same dimension");
        }
        for index in 0..DIMENSION {
            assert_eq!(
                encoding.vector.real(index).unwrap().to_bits(),
                rebuilt.real(index).unwrap().to_bits(),
                "component {index} is bit-identical to the lane-by-lane rebuild"
            );
        }
        assert_eq!(encoding.vector.imag(0), Some(0.0));

        // Superposition ACCUMULATES: adding a trace to itself doubles every
        // coefficient and the norm, so earlier evidence never decays.
        let doubled = <FHRRAlgebra<DIMENSION> as BindingAlgebra>::superpose(
            &encoding.vector,
            &encoding.vector,
        )
        .expect("same dimension");
        for index in 0..DIMENSION {
            assert_eq!(
                doubled.real(index).unwrap(),
                2.0 * encoding.vector.real(index).unwrap()
            );
        }
        assert_eq!(doubled.norm(), 2.0 * encoding.vector.norm());

        // Bundling is the wrong algebra here, and this is why: an attention
        // weighted bundle of a trace with itself IS the trace (bounded
        // magnitude, earlier contributions attenuated), while superposition
        // doubles it. The encoder must compose with `superpose`.
        let bundled = <FHRRAlgebra<DIMENSION> as BindingAlgebra>::bundle(
            &encoding.vector,
            &encoding.vector,
            0.5,
        )
        .expect("same dimension");
        assert_eq!(
            digest_hex(&encoder.vector_digest(&bundled)),
            VECTOR_FULL,
            "bundle averages back to the input"
        );
        assert_ne!(
            digest_hex(&encoder.vector_digest(&doubled)),
            digest_hex(&encoder.vector_digest(&bundled))
        );
        assert!(bundled.norm() < doubled.norm());

        // Position binding is real: moving a step's position changes the
        // vector, so trace order is part of the encoding.
        let mut reordered = <FHRRAlgebra<DIMENSION> as BindingAlgebra>::zero();
        reordered = <FHRRAlgebra<DIMENSION> as BindingAlgebra>::superpose(
            &reordered,
            &encoder.trace_token(&trace, 0).expect("bounded"),
        )
        .unwrap();
        for (position, index) in [(0usize, 0usize), (1, 1)] {
            let step = BindingAlgebra::permute(
                &encoder.trace_token(&trace, index).expect("bounded"),
                (position + 1) as i32,
            );
            reordered =
                <FHRRAlgebra<DIMENSION> as BindingAlgebra>::superpose(&reordered, &step).unwrap();
        }
        assert_ne!(
            digest_hex(&encoder.vector_digest(&reordered)),
            VECTOR_FULL,
            "a permuted trace is a different vector"
        );
    }

    /// Encoder identity is content-hashed over `(schema, seed, dimension,
    /// limits)`, and trace identity is content-hashed over the trace alone.
    #[test]
    fn encoder_and_trace_identity_are_content_hashed() {
        let base = encoder();
        // Same config, same digest.
        assert_eq!(base.digest(), encoder().digest());
        // A different seed is a different encoder.
        let reseeded =
            HolographicEncoder::<DIMENSION>::new(SEED ^ 1, RelationLimits::default()).unwrap();
        assert_ne!(base.digest(), reseeded.digest());
        // A different dimension is a different encoder.
        let wide = HolographicEncoder::<128>::new(SEED, RelationLimits::default()).unwrap();
        assert_ne!(base.digest(), wide.digest());
        // A different limit set is a different encoder.
        let tight = HolographicEncoder::<DIMENSION>::new(
            SEED,
            RelationLimits::new(64, 8, 16, 1_024).expect("tight but valid"),
        )
        .unwrap();
        assert_ne!(base.digest(), tight.digest());

        // Trace identity is the trace's content, and does not depend on the
        // encoder at all: the same replayed content hashed under a different
        // seed keeps its identity while its vector changes.
        let trace = full_trace();
        assert_eq!(digest_hex(&trace.trace_hash()), TRACE_FULL);
        assert_eq!(digest_hex(&short_trace().trace_hash()), TRACE_SHORT);
        assert_eq!(digest_hex(&other_trace().trace_hash()), TRACE_OTHER);
        assert_eq!(
            digest_hex(&reseeded.encode(&trace).expect("bounded").digest),
            VECTOR_FULL_SEED2
        );
        assert_eq!(
            trace.trace_hash(),
            reseeded.encode(&trace).unwrap().trace_hash
        );

        // Re-validating the same content yields the same identity; the query
        // identity binds to the certified query, and the system identity to
        // the certified system.
        let again = full_trace();
        assert_eq!(again.trace_hash(), trace.trace_hash());
        assert_eq!(again.query_hash(), trace.query_hash());
        assert_eq!(again.system_hash(), trace.system_hash());
        assert_ne!(short_trace().query_hash(), trace.query_hash());

        // Different content, different identity.
        assert_ne!(trace.trace_hash(), short_trace().trace_hash());
        assert_ne!(short_trace().trace_hash(), other_trace().trace_hash());
        // The trace identity is a 32-byte digest rendered as 64 hex characters.
        assert_eq!(digest_hex(&trace.trace_hash()).len(), 64);
    }

    // ----------------------------------------------------------------- recall

    /// Repeated recall is bit-stable: same query, same corpus, same ranked
    /// identities, same scores *bit for bit* — and admission order cannot
    /// change the answer.
    #[test]
    fn recall_repeats_exactly_and_is_admission_order_invariant() {
        let full = full_trace();
        let short = short_trace();
        let other = other_trace();
        let forward = corpus(&[&full, &short, &other]);
        let reversed = corpus(&[&other, &short, &full]);

        let query = full_trace();
        let first = forward.recall(&query, 8).expect("recall must run");
        assert_eq!(first.considered, 3);
        assert_eq!(first.matches.len(), 3);

        let order: Vec<String> = first
            .matches
            .iter()
            .map(|entry| digest_hex(&entry.trace_hash))
            .collect();
        assert_eq!(order, vec![TRACE_FULL, TRACE_SHORT, TRACE_OTHER]);

        for _ in 0..3 {
            let again = forward.recall(&query, 8).expect("recall must run");
            assert_eq!(again.query_hash, first.query_hash);
            assert_eq!(again.encoder_digest, first.encoder_digest);
            for (left, right) in first.matches.iter().zip(again.matches.iter()) {
                assert_eq!(left.trace_hash, right.trace_hash);
                assert_eq!(
                    left.score.combined.to_bits(),
                    right.score.combined.to_bits()
                );
                assert_eq!(
                    left.score.goal_alignment.to_bits(),
                    right.score.goal_alignment.to_bits()
                );
                assert_eq!(
                    left.score.path_alignment.to_bits(),
                    right.score.path_alignment.to_bits()
                );
            }
        }

        // Content, not arrival order, decides: the reversed corpus answers
        // with the identical ranked list.
        let backwards = reversed.recall(&query, 8).expect("recall must run");
        let backwards_order: Vec<String> = backwards
            .matches
            .iter()
            .map(|entry| digest_hex(&entry.trace_hash))
            .collect();
        assert_eq!(backwards_order, order);
        for (left, right) in first.matches.iter().zip(backwards.matches.iter()) {
            assert_eq!(
                left.score.combined.to_bits(),
                right.score.combined.to_bits()
            );
        }

        // The limit truncates the tail only, keeping the ranked prefix.
        let limited = forward.recall(&query, 1).expect("recall must run");
        assert_eq!(limited.matches.len(), 1);
        assert_eq!(digest_hex(&limited.matches[0].trace_hash), TRACE_FULL);
    }

    /// The match exposes the documented similarity decomposition, and every
    /// lane recomputes from the published vectors.
    #[test]
    fn recall_exposes_transparent_similarity_evidence() {
        let encoder = encoder();
        let full = full_trace();
        let short = short_trace();
        let other = other_trace();
        let corpus = corpus(&[&full, &short, &other]);
        let query = full_trace();
        let outcome = corpus.recall(&query, 8).expect("recall must run");
        assert_eq!(outcome.query_hash, query.trace_hash());

        let encoded_query = encoder.encode(&query).expect("bounded");
        let mut pinned = BTreeMapLike::new();
        pinned.insert(TRACE_FULL, SCORE_FULL);
        pinned.insert(TRACE_SHORT, SCORE_SHORT);
        pinned.insert(TRACE_OTHER, SCORE_OTHER);

        for entry in &outcome.matches {
            let hex = digest_hex(&entry.trace_hash);
            let expected = pinned.get(&hex).expect("every match was pinned");
            assert!(
                close(entry.score.combined, expected.0),
                "combined lane for {hex}: {0} != {1}",
                entry.score.combined,
                expected.0
            );
            assert!(close(entry.score.goal_alignment, expected.1));
            assert!(close(entry.score.path_alignment, expected.2));

            // Each lane is exactly the canonical similarity of the published
            // parts, so the evidence is recomputable by any caller.
            let source = match hex.as_str() {
                TRACE_FULL => &full,
                TRACE_SHORT => &short,
                TRACE_OTHER => &other,
                _ => unreachable!("only admitted traces are matched"),
            };
            let encoded = encoder.encode(source).expect("bounded");
            assert_eq!(
                entry.score.combined.to_bits(),
                BindingAlgebra::similarity(&encoded_query.vector, &encoded.vector).to_bits()
            );
            assert_eq!(
                entry.score.goal_alignment.to_bits(),
                BindingAlgebra::similarity(&encoded_query.goal, &encoded.goal).to_bits()
            );
            assert_eq!(
                entry.score.path_alignment.to_bits(),
                BindingAlgebra::similarity(&encoded_query.path, &encoded.path).to_bits()
            );
        }

        // The lanes are evidence, not an additive score: the documented total
        // is `combined` alone, measured against the full vectors.
        let short_entry = outcome
            .matches
            .iter()
            .find(|entry| digest_hex(&entry.trace_hash) == TRACE_SHORT)
            .expect("the short trace is admitted");
        assert!(close(short_entry.score.combined, SCORE_SHORT.0));
        assert!(close(short_entry.score.goal_alignment, 1.0));
        assert!(short_entry.score.path_alignment < 0.0);
        assert!(
            !close(
                short_entry.score.combined,
                short_entry.score.goal_alignment + short_entry.score.path_alignment
            ),
            "the lanes are independent evidence, never summed into the total"
        );
    }

    /// The pinned direction convention is HIGHER-is-better, and it holds end to
    /// end: an exact-content trace is first with the maximal lane, and the
    /// returned order is the descending similarity order.
    #[test]
    fn recall_prefers_higher_similarity() {
        assert_eq!(SCORE_DIRECTION, "higher-is-better");
        let full = full_trace();
        let short = short_trace();
        let other = other_trace();
        let corpus = corpus(&[&other, &full, &short]);
        let query = full_trace();
        let outcome = corpus.recall(&query, 8).expect("recall must run");

        assert_eq!(
            digest_hex(&outcome.matches[0].trace_hash),
            TRACE_FULL,
            "the identical trace ranks first"
        );
        assert!(close(outcome.matches[0].score.combined, 1.0));
        assert!(close(outcome.matches[0].score.goal_alignment, 1.0));
        assert!(close(outcome.matches[0].score.path_alignment, 1.0));

        // Descending: no later match may outrank an earlier one, and the
        // order agrees with the raw (HIGHER-is-better) lane, not its negation.
        for pair in outcome.matches.windows(2) {
            assert!(
                pair[0].score.combined > pair[1].score.combined,
                "scores must decrease: {} then {}",
                pair[0].score.combined,
                pair[1].score.combined
            );
        }
        let mut sorted: Vec<f64> = outcome.matches.iter().map(|m| m.score.combined).collect();
        sorted.sort_by(|left, right| right.total_cmp(left));
        let returned: Vec<f64> = outcome.matches.iter().map(|m| m.score.combined).collect();
        assert_eq!(
            returned
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>(),
            sorted
                .iter()
                .map(|value| value.to_bits())
                .collect::<Vec<_>>()
        );
        // The last match is the least similar, and it is NOT the best by the
        // highest score (a LOWER-is-better reading would have inverted this).
        assert!(outcome.matches.last().unwrap().score.combined < 0.5);
    }

    // ------------------------------------------------------------ rejections

    /// An empty corpus is a typed rejection, not a silently empty answer.
    #[test]
    fn empty_corpus_is_a_typed_rejection() {
        let corpus = TraceCorpus::<DIMENSION>::new(
            encoder(),
            RecallConfig::new(8, MAX_TRACE_TERMS, 8).unwrap(),
        );
        assert!(corpus.is_empty());
        let error = corpus
            .recall(&full_trace(), 8)
            .expect_err("an empty corpus cannot answer");
        assert_eq!(error, HolographicError::EmptyCorpus);
    }

    /// A malformed corpus entry — no replay-validated step, or a duplicate
    /// identity — is a typed rejection.
    #[test]
    fn malformed_corpus_is_a_typed_rejection() {
        let mut corpus = corpus(&[&full_trace()]);

        // A duplicate identity is malformed: the same trace content cannot be
        // admitted twice under one identity.
        let duplicate = full_trace();
        let error = corpus
            .admit(&duplicate)
            .expect_err("duplicates are rejected");
        assert_eq!(
            error,
            HolographicError::MalformedTrace {
                reason: "duplicate trace identity"
            }
        );
        assert_eq!(corpus.len(), 1);

        // A zero-step trace is a query shape, not a corpus entry: it records no
        // replay-validated inverse step, so admitting it is malformed.
        let empty = validated(&chain_system(), &constant("a"), &constant("a"));
        assert!(empty.steps().is_empty());
        let error = corpus
            .admit(&empty)
            .expect_err("zero-step traces are rejected");
        assert_eq!(
            error,
            HolographicError::MalformedTrace {
                reason: "trace has no replay-validated step"
            }
        );
        assert_eq!(corpus.len(), 1);
    }

    /// Oversized requests are typed rejections at every ceiling: corpus size,
    /// trace length, dimension, and the recall limit.
    #[test]
    fn oversized_requests_are_typed_rejections() {
        // Dimension above the hard ceiling.
        let error =
            HolographicEncoder::<{ MAX_TRACE_DIMENSION + 1 }>::new(SEED, RelationLimits::default())
                .expect_err("an oversized dimension is rejected");
        assert_eq!(
            error,
            HolographicError::LimitExceeded {
                resource: "trace dimension",
                value: MAX_TRACE_DIMENSION + 1,
                limit: MAX_TRACE_DIMENSION,
            }
        );

        // Corpus size ceiling, tightened by the caller.
        let mut bounded = TraceCorpus::<DIMENSION>::new(
            encoder(),
            RecallConfig::new(1, MAX_TRACE_TERMS, 4).unwrap(),
        );
        bounded.admit(&full_trace()).expect("first admission fits");
        let error = bounded
            .admit(&short_trace())
            .expect_err("the ceiling rejects the second");
        assert_eq!(
            error,
            HolographicError::LimitExceeded {
                resource: "recall traces",
                value: 2,
                limit: 1,
            }
        );
        assert_eq!(bounded.len(), 1);

        // Trace length ceiling.
        let mut tight =
            TraceCorpus::<DIMENSION>::new(encoder(), RecallConfig::new(8, 2, 4).unwrap());
        let error = tight
            .admit(&full_trace())
            .expect_err("a three-term trace exceeds a two-term ceiling");
        assert_eq!(
            error,
            HolographicError::LimitExceeded {
                resource: "trace terms",
                value: 3,
                limit: 2,
            }
        );
        assert!(tight.is_empty());

        // Config and limit ceilings cannot be widened.
        assert_eq!(
            RecallConfig::new(0, MAX_TRACE_TERMS, 4).expect_err("zero traces is invalid"),
            HolographicError::LimitExceeded {
                resource: "recall traces",
                value: 0,
                limit: MAX_CORPUS_TRACES,
            }
        );
        assert_eq!(
            RecallConfig::new(MAX_CORPUS_TRACES + 1, MAX_TRACE_TERMS, 4)
                .expect_err("above the hard ceiling is invalid"),
            HolographicError::LimitExceeded {
                resource: "recall traces",
                value: MAX_CORPUS_TRACES + 1,
                limit: MAX_CORPUS_TRACES,
            }
        );
        assert_eq!(
            RecallConfig::new(8, MAX_TRACE_TERMS + 1, 4).expect_err("terms ceiling"),
            HolographicError::LimitExceeded {
                resource: "trace terms",
                value: MAX_TRACE_TERMS + 1,
                limit: MAX_TRACE_TERMS,
            }
        );
        assert_eq!(
            RecallConfig::new(8, MAX_TRACE_TERMS, MAX_RECALL_RESULTS + 1)
                .expect_err("results ceiling"),
            HolographicError::LimitExceeded {
                resource: "recall results",
                value: MAX_RECALL_RESULTS + 1,
                limit: MAX_RECALL_RESULTS,
            }
        );

        let corpus = corpus(&[&full_trace()]);
        assert_eq!(
            corpus
                .recall(&full_trace(), 0)
                .expect_err("zero results is invalid"),
            HolographicError::LimitExceeded {
                resource: "recall results",
                value: 0,
                limit: MAX_RECALL_RESULTS,
            }
        );
        assert_eq!(
            corpus
                .recall(&full_trace(), MAX_RECALL_RESULTS + 1)
                .expect_err("above the hard ceiling is invalid"),
            HolographicError::LimitExceeded {
                resource: "recall results",
                value: MAX_RECALL_RESULTS + 1,
                limit: MAX_RECALL_RESULTS,
            }
        );
        // The corpus config is the tighter authority: a request above it is
        // rejected, never silently clamped.
        let mut narrowed = TraceCorpus::<DIMENSION>::new(
            encoder(),
            RecallConfig::new(8, MAX_TRACE_TERMS, 2).unwrap(),
        );
        narrowed.admit(&full_trace()).unwrap();
        assert_eq!(
            narrowed
                .recall(&full_trace(), 3)
                .expect_err("cannot widen the configured result ceiling"),
            HolographicError::LimitExceeded {
                resource: "recall results",
                value: 3,
                limit: 2,
            }
        );
    }

    /// Admission is metered before allocation, and a rejected admission leaves
    /// the corpus exactly as it was.
    #[test]
    fn admission_is_bounded_before_allocation() {
        // A tightened term budget rejects an otherwise admissible trace, and
        // the corpus is unchanged: nothing was encoded, nothing was retained.
        let tight_limits = RelationLimits::new(4, 8, 16, 4_096).expect("valid limits");
        let mut corpus = TraceCorpus::<DIMENSION>::new(
            HolographicEncoder::new(SEED, tight_limits).unwrap(),
            RecallConfig::default(),
        );
        let before = corpus.encoder_digest();
        let trace = validated(&nested_system(), &constant("c"), &nested(constant("a"), 4));
        let error = corpus
            .admit(&trace)
            .expect_err("a term-node ceiling rejects the trace");
        assert_eq!(
            error,
            HolographicError::LimitExceeded {
                resource: "trace term nodes",
                value: 5,
                limit: 4,
            }
        );
        assert!(corpus.is_empty());
        assert_eq!(corpus.encoder_digest(), before);

        // The same trace fits a corpus with the default budget.
        let mut roomy = TraceCorpus::<DIMENSION>::new(encoder(), RecallConfig::default());
        roomy.admit(&trace).expect("the default budget admits it");
        assert_eq!(roomy.len(), 1);

        // The default config never exceeds the hard ceilings.
        let config = RecallConfig::default();
        assert_eq!(config.max_traces(), MAX_CORPUS_TRACES);
        assert_eq!(config.max_terms(), MAX_TRACE_TERMS);
        assert_eq!(config.max_results(), MAX_RECALL_RESULTS);

        // A trace term deeper than the encoder's depth ceiling is rejected at
        // admission, before any hashing or encoding work, so a deep term never
        // reaches the derivation.
        let shallow = RelationLimits::new(64, 3, 16, 4_096).expect("valid limits");
        let mut bounded = TraceCorpus::<DIMENSION>::new(
            HolographicEncoder::new(SEED, shallow).unwrap(),
            RecallConfig::default(),
        );
        let error = bounded
            .admit(&trace)
            .expect_err("a depth ceiling rejects the trace");
        assert_eq!(
            error,
            HolographicError::LimitExceeded {
                resource: "trace term depth",
                value: 4,
                limit: 3,
            }
        );
        assert!(bounded.is_empty());
    }

    /// A trace can only be minted at the replay boundary: the module consumes
    /// validated inverse-search output and never generates one.
    #[test]
    fn traces_are_only_minted_from_replay_validated_certificates() {
        let system = chain_system();
        let target = constant("c");
        let goal = constant("a");
        let certificate = certificate(&system, &target, &goal);
        let trace = RecallTrace::validate(&system, &certificate).expect("genuine witness");
        assert_eq!(trace.query_hash(), certificate.query_hash);
        assert_eq!(trace.system_hash(), certificate.system_hash);

        // A tampered query breaks the replay: the certificate no longer
        // verifies, so no trace can be minted from it.
        let mut tampered = certificate.clone();
        tampered.query = ReplayQuery::Backward {
            target: target.clone(),
            goal: constant("b"),
        };
        let error = RecallTrace::validate(&system, &tampered)
            .expect_err("a tampered query cannot mint a trace");
        assert!(matches!(error, HolographicError::UnvalidatedProvenance(_)));

        // A certificate bound to another system does not verify here either.
        let other_system = disjoint_system();
        let error = RecallTrace::validate(&other_system, &certificate)
            .expect_err("a foreign certificate cannot mint a trace");
        assert!(matches!(error, HolographicError::UnvalidatedProvenance(_)));

        // And a certificate whose derivation was swapped for a fabricated
        // chain is rejected by the replay, not trusted as a record.
        let mut forged = certificate.clone();
        if let ReplayDerivation::Backward(derivation) = &mut forged.derivation {
            derivation.steps.clear();
        }
        let error = RecallTrace::validate(&system, &forged)
            .expect_err("an invented derivation cannot mint a trace");
        assert!(matches!(error, HolographicError::UnvalidatedProvenance(_)));
    }

    /// Recall reads only the caller-provided corpus: two corpora over disjoint
    /// traces never see each other's evidence, and nothing appears that was not
    /// admitted.
    #[test]
    fn recall_consumes_only_the_caller_corpus() {
        let full = full_trace();
        let other = other_trace();
        let left = corpus(&[&full]);
        let right = corpus(&[&other]);

        let left_outcome = left.recall(&full_trace(), 8).expect("recall must run");
        let right_outcome = right.recall(&full_trace(), 8).expect("recall must run");

        assert_eq!(left_outcome.considered, 1);
        assert_eq!(right_outcome.considered, 1);
        assert_eq!(digest_hex(&left_outcome.matches[0].trace_hash), TRACE_FULL);
        assert_eq!(
            digest_hex(&right_outcome.matches[0].trace_hash),
            TRACE_OTHER
        );
        // The exact trace is not in `right`, so its best lane is strictly less
        // than the exact match's — evidence comes from the corpus alone.
        assert!(left_outcome.matches[0].score.combined > right_outcome.matches[0].score.combined);

        // Growing one corpus does not change the other's answer.
        let mut grown = corpus(&[&other]);
        grown.admit(&full_trace()).expect("admission");
        let untouched = left.recall(&full_trace(), 8).expect("recall must run");
        assert_eq!(untouched.considered, 1);
        assert_eq!(
            untouched.matches[0].score.combined.to_bits(),
            left_outcome.matches[0].score.combined.to_bits()
        );
    }

    /// A tiny ordered map, so the pinned score table needs no dependency.
    struct BTreeMapLike {
        entries: Vec<(&'static str, (f64, f64, f64))>,
    }

    impl BTreeMapLike {
        fn new() -> Self {
            Self {
                entries: Vec::new(),
            }
        }

        fn insert(&mut self, key: &'static str, value: (f64, f64, f64)) {
            self.entries.push((key, value));
        }

        fn get(&self, key: &str) -> Option<&(f64, f64, f64)> {
            self.entries
                .iter()
                .find(|(candidate, _)| *candidate == key)
                .map(|(_, value)| value)
        }
    }

    /// The encoder's limits are the caller-tightenable relation walk ceilings,
    /// and a tightened ceiling is what the corpus honours at admission.
    #[test]
    fn encoder_limits_are_the_relation_walk_ceilings() {
        let limits = RelationLimits::new(64, 8, 16, 4_096).expect("valid limits");
        let encoder = HolographicEncoder::<DIMENSION>::new(SEED, limits).unwrap();
        assert_eq!(encoder.limits().max_term_nodes(), 64);
        assert_eq!(encoder.limits().max_term_depth(), 8);
        // Clamping is defence in depth: a value built outside the validating
        // constructor cannot widen the hard ceiling.
        assert_eq!(encoder.limits().max_term_nodes(), 64);
        assert!(encoder.limits().max_term_nodes() <= RelationLimits::MAX_TERM_NODES);
    }
}

/// Default-feature builds expose no holographic surface.
///
/// The compile-time proof lives at the crate root: a `compile_fail` doctest
/// that names `amari_rewrite::holographic` is compiled only while
/// `holographic-guidance` is off, so a default build demonstrates that the
/// module path does not resolve. This module pins the other half — that the
/// configuration under test really is a plain `std` default build with no
/// research-guidance surface compiled at all (so the whole contract module
/// above is absent, not merely skipped).
///
/// The claim is about *one* configuration, so the module is compiled only in
/// that exact configuration: `std` on and `holographic-guidance`, `neural`,
/// `network`, `completion`, `macros`, `smt`, and `serialize` all off (the whole
/// non-default feature list from `Cargo.toml`). A build that turns any research
/// feature on is not a default build (its `holographic` surface question is not
/// this module's obligation) and would fail the count below for the honest
/// reason that it did compile a research surface. Gating here keeps the
/// assertion truthful in every feature combination the file is built under,
/// including `--all-features` and `--features macros`, instead of asserting a
/// default-build property from a non-default build. The crate-root
/// `compile_fail` doctest is deliberately not gated this way: it must keep
/// proving the absence of `amari_rewrite::holographic` in *every*
/// non-`holographic-guidance` build.
#[cfg(all(
    feature = "std",
    not(feature = "holographic-guidance"),
    not(feature = "neural"),
    not(feature = "network"),
    not(feature = "completion"),
    not(feature = "macros"),
    not(feature = "smt"),
    not(feature = "serialize")
))]
mod default_isolation {
    #[test]
    fn default_builds_expose_no_holographic_surface() {
        // Count the research surfaces this build compiled. `std` is the only
        // member of the default feature set, so every other count must be
        // zero and the std count must be one.
        let research_surfaces = u8::from(cfg!(feature = "neural"))
            + u8::from(cfg!(feature = "network"))
            + u8::from(cfg!(feature = "completion"))
            + u8::from(cfg!(feature = "macros"))
            + u8::from(cfg!(feature = "smt"))
            + u8::from(cfg!(feature = "serialize"))
            + u8::from(!cfg!(feature = "std"));
        assert_eq!(
            research_surfaces, 0,
            "a default build compiles no research surface"
        );
    }
}
