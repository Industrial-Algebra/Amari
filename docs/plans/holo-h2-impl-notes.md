# H2 implementation and migration evidence

## U2 — stable geometry and coordinated D3 rewrite migration

Parent `a2933e78`. RED `c test -p amari-holographic --test h2_geometry` default, 1 selected, exit 101: `tiny scalar norm = 0`. Final working tree, same command: `u2_small_nonzero_norm`, `domains`, `scales_and_directions` all passed (3, exit 0).

Rewrite paused per operator ([coordination handoff](2026-11-25-holographic-rewrite-coordination-handoff.md)). Only `recall.rs`'s lane function/three field assignments and success extraction in six existing `holographic_inverse.rs` assertions migrate. Undefined lanes propagate `HolographicError::Composition`; no substituted scores or skipped candidates. The six assertions' numbers and bits were not changed. `holographic_remediation.rs` remains byte-identical. During migration `c test -p amari-rewrite --features holographic-guidance --test holographic_remediation similarity_lanes_are_clamped_to_the_documented_interval -- --exact` selected 1, exit 101:

```text
assertion `left == right` failed: an identical trace ranks at the top of the interval
  left: 0.9999999999999998
 right: 1.0
```

Operator-approved structural identity (candidate for promotion to contract text): **cosine(x,x) is exactly 1.0** for bitwise-identical full representations on the admitted finite, nonzero Euclidean domain. Not a tolerance or ranking policy. Zero/nonfinite remain errors; signed/null signatures return `UnsupportedSignature`. Use `normalized_metric_pairing` for the signed operation; handle fallible norm and cosine at the call site. The ordinary error already met the general `1e−12` law; the identity restores the stricter pre-H2 rewrite result. `c test -p amari-rewrite --features holographic-guidance`, all test binaries, 560 passed, 0 failed/ignored, exit 0.

`c` means `env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_TARGET -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER cargo +1.100.0` from the H2 worktree root.

## U1 — binding keys and general division

At parent `c0e66483` plus the initial three tests, **before** implementation: `c test -p amari-holographic --test h2_keys` (defaults), exit 101. Verbatim runtime RED excerpts:

```text
assertion failed: F::new([0., 3.], [0., 4.]).normalize_unitary().is_err()
  left: [12.0, -36.0]
 right: [3.0, -4.0]
  left: [0.0, 8.0, 9.0, 0.0]
 right: [0.0, 2.0, 1.0, 0.0]
test result: FAILED. 0 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Same command after implementation, same parent plus U1 changes: 7 passed, exit 0 (`general_key_does_not_rebind`, `nonunit_phases_are_not_inverted_by_correlation`, `projection_rejects_a_zero_bin`, `dense_complex_convolution`, `typed_admission_is_not_general_invertibility`, `singular_and_mixed_clifford_keys`, `sampler_reports_domain`). `c test -p amari-holographic --all-features`: 195 passed, 0 failed, 17 ignored, exit 0.

**Migration:** `TypedKey::try_new(element)?` admits a retrieval key; call `bind_key` / `unbind_key` on the validated key. For general algebraic division call `unbind_general(&key,&bound)?`. Use `correlation` when the original adjoint/rebinding was intended instead. A reciprocal is not admission to H3 membership. Full-wrapper `CliffordAlgebra::random_versor` is now fallible, including for positive factor counts in purely null signatures. Rebuild derived values from original operands if previous code stored the result of nonunit rebinding as a recovered value; there is no blanket reinterpretation of old results.

## U4 — whole-list bundling

Parent `cdd302df`, `c test -p amari-holographic --all-features --test h2_bundle`: three runtime failures (exit 101), recorded before implementation. `map_pair_list`: `left: [0.9426807890983486]`, `right: [0.7615941559557649]`; `whole_list_not_fold`: `left: 2.0`, `right: 1.3333333333333333`; `rayon_identity_pair`: `left: 0.5`, `right: 1.0`. GREEN same command: 5 passed, including `boundaries_and_ties` and `pool_sizes_match_ordered_formula_and_suffix_errors` (lengths 3/17/64, pools 1/2/4), exit 0. Rayon computes norms in parallel, then consumes all results in input order, including errors; no chunk-level bundle is combined with another.

## U3 — shape and rotations

Pre-change scratch at `a2933e78`, `c test -p amari-holographic --test h2_shape`, four regressions, exit 101: `[1,1] dropped a component instead of rejecting the map`; `attempt to calculate the remainder with a divisor of zero` on MAP, FHRR and Product. At `fc7b3f1a` plus U3, same command: `bin_orientation_and_extreme_shifts`, `checked_bijections`, `empty_construction` all passed, exit 0. Checked constructors reject empty shapes. Infallible const-sized construction requires a positive dimension at monomorphization. No serde impl on these three wrappers. FHRR `rotate_bins` is reindexing, not a time delay; the rewrite trait call retains identical orientation.
