# H2 implementation and migration evidence

## Implementation review R1 — two P2 findings on `1e1f7d02`

**F5, trait and method rustdoc:** binding distributes over `superpose` (bilinearity), not nonlinear attention, voting or cleanup. `rg -in 'distribut|commutes with' amari-holographic` checked the crate source, README and tests; neither of the two former promises remains. The additive coefficient test still runs; no nonlinear distribution assertion was introduced.

**D2, direct NaN calls:** workspace search for `bundle_from_norms|hard_bundle|soft_cleanup` found no external Rust callers. Removed the hidden public trait methods; serial and Rayon callers now use private reducers. Validation runs before singleton return or cleanup. No migration to another unchecked method.

RED on the reviewed head plus regression tests, before changing the implementation, default features: `c test -p amari-holographic --test h2_r1_direct`, exit 101, **2 ran, 2 failed** (`direct_weighted_nan`, `direct_hard_nan`). Public checked controls rejected the singleton; the direct calls `MAPAlgebra::bundle_from_norms(&items, &[1.], 1.)` and `MAPAlgebra::hard_bundle(&items)` both produced this verbatim diagnostic:

```text
called `Result::unwrap_err()` on an `Ok` value: MAPAlgebra { components: [NaN] }
```

`c test -p amari-holographic --doc bundle_all` on the same pre-fix implementation, two direct-call compile-fail controls: both failed with `Test compiled successfully, but it's marked \`compile_fail\`.` (exit 101). GREEN after the change: same doctest command, **2 passed**; same integration binary, **1 passed** (`public_bundle_paths_reject_nan_before_singleton_shortcut`, checks `NonFinite` through whole-list, soft-majority and hard-majority APIs).

Combined R1 working-tree rerun of the U6 command matrix below: `211` default / `213` all-feature holographic tests passed, 17 ignored in each; H1 5; aggregate rewrite 560; fusion 43/7; WASM host 140 + both Node scripts; GPU three gates including lavapipe 101/25,856 actual binding readback; discovery 18 + drift 1 + WASM catalog 41; fmt, scoped warning-denied Clippy, docs all exit 0. Fresh exact bindgen pins, web regeneration then structural regeneration: 11,774 items, hash `a1814d049d0a548b36c2d930ed521856f18cd466ffbdbd4fe253f9119314f2ef`. Raw combined logs `/tmp/h2-final-QQq2FR`. Independent R2 pending.

## U6 — consumers and combined verification

`c test -p amari-fusion --features holographic --test h2_binding_domain` prior to consumer implementation (`fc7b3f1a`), confirmed in an isolated exact-parent `35db4b73` replay: exit 101, 0 ran, `error[E0432]: unresolved imports \`amari_fusion::unbind_general\`, \`amari_fusion::TypedKey\``. GREEN: `fusion_distinguishes_typed_keys_from_general_inverse` 1 passed, exit 0. Fusion reexports the checked operations. WASM still admits mixed certified keys at 256 finite coefficients; `randomVersor(2)` was exercised through store and tracked self-score. Singular suffix, wrong length and NaN errors do not trap or change itemCount. H3 evidence remains distinct from score. The public GPU pair bundle is CPU-backed at β=1; its adapter-required test compares coefficients and rejects a nonfinite suffix. Only the binding shader has a readback parity claim. No `tracedecay` runtime validation or release decision.

### Exact unit replays

Focused commands in the unit sections passed at `a2933e78` (7), `fc7b3f1a` (3), `cdd302df` (3), `9c089872` (5, all features), `35db4b73` (2 integration + 2 lib). U3's four original runtime REDs also reproduced on its actual `fc7b3f1a` parent. No rewritten expectation is part of the shape/name migration. In particular, `encode.rs` uses trait `permute`; no rewrite source edits after the coordinated D3 commit.

### Combined tree, separate from focused unit GREENs

Ran on `35db4b73` plus the U6 changes. `Cargo.lock` untracked, fresh `c generate-lockfile` confirmed bindgen `0.2.130` / test `0.3.80`. Under `RUSTUP_TOOLCHAIN=1.100.0` and the four compiler/target variables unset, ran `bash scripts/generate-discovery-wasm-surface.sh` **first** (web output byte-identical; schema gate 1 passed), then `c run -p amari-discovery --example generate_catalog -- .`: 11,786 items, hash `f626e1ea1f62b4446045291442f3a629fba069a442f0cb730b5662c689e92c7c`. No new discovery test binary.

All commands below exited 0. Counts are totals across binaries, not the last binary printed.

| Command (from repo root) | Result |
|---|---|
| `c fmt --all -- --check` | clean |
| `c test -p amari-holographic` / same `--all-features` | 208 / 210 passed; 17 ignored each |
| `c test -p amari-holographic --all-features --test h1_kernel` | 5 passed; 64 pairs, 512 triples |
| `c check` and `c test -p amari-rewrite --features holographic-guidance` (same package/features for check) | 560 passed, none ignored; remediation golden byte-identical |
| `c test -p amari-fusion --features holographic`; `c test -p amari-wasm --lib` | 43 passed / 7 ignored; 140 passed |
| `c test -p amari-gpu --features holographic --lib test_batch_bundle_cpu -- --ignored --nocapture`; same crate/features `--test h3_memory`; `--test h1_shader_parity -- --ignored --nocapture` | 1 each; `CI=true`, Vulkan lvp ICD, llvmpipe (LLVM 21.1.8). Read back 4 records / 1,024 coefficients CPU, **101 / 25,856 shader** |
| `(cd amari-wasm && wasm-pack build --dev --target web --out-dir "$pkg")`; `node scripts/test-h1-wasm.mjs "$pkg"`; `node scripts/test-h3-wasm.mjs "$pkg"` | both exercised generated web bindings |
| `c test -p amari-discovery --features standard-probes --test h3_recall --test probe_holographic --test wire_schema_registry`; `c test -p amari-discovery generated_equals_checked_in`; `c test -p amari-discovery --test catalog_wasm` | 18; 1 drift; 41 WASM catalog |
| `c clippy $crates --all-targets --all-features -- -D warnings`; `c doc $crates --all-features --no-deps` | passed; Cargo manifest and rustdoc link warnings are separate, not a warning-free docs claim |

`$crates` selects `-p` for each of amari-holographic, amari-gpu, amari-wasm, amari-fusion and amari-discovery. Combined precommit logs `/tmp/h2-final-HoYeHL`. This is local implementation evidence, not PR CI or independent review.

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

## U5 — logarithmic helpers and the linear law

Parent `9c089872`, `c test -p amari-holographic --test h2_temperature`, default: RED exit 101, zero tests ran: `error[E0432]: unresolved imports \`amari_holographic::algebra::log_mean_exp\`, \`amari_holographic::algebra::log_sum_exp\``. GREEN, same command: `distinct_zero_limits` and `bind_distributes_over_addition`, 2 passed. `c test -p amari-holographic --lib algebra::temperature`: `large_range_and_invalid_inputs`, `finite_and_hard_limits`, 2 passed, exits 0. Equal-input mean, `ln(2)/β` offset, unequal-input mean and hard limits, typed rejection, and extreme opposite signs are tested numerically. No distributivity claim about nonlinear cleanup.

## U4 — whole-list bundling

Parent `cdd302df`, `c test -p amari-holographic --all-features --test h2_bundle`: three runtime failures (exit 101), recorded before implementation. `map_pair_list`: `left: [0.9426807890983486]`, `right: [0.7615941559557649]`; `whole_list_not_fold`: `left: 2.0`, `right: 1.3333333333333333`; `rayon_identity_pair`: `left: 0.5`, `right: 1.0`. GREEN same command: 5 passed, including `boundaries_and_ties` and `pool_sizes_match_ordered_formula_and_suffix_errors` (lengths 3/17/64, pools 1/2/4), exit 0. Rayon computes norms in parallel, then consumes all results in input order, including errors; no chunk-level bundle is combined with another.

## U3 — shape and rotations

Pre-change scratch at `a2933e78`, `c test -p amari-holographic --test h2_shape`, four regressions, exit 101: `[1,1] dropped a component instead of rejecting the map`; `attempt to calculate the remainder with a divisor of zero` on MAP, FHRR and Product. At `fc7b3f1a` plus U3, same command: `bin_orientation_and_extreme_shifts`, `checked_bijections`, `empty_construction` all passed, exit 0. Checked constructors reject empty shapes. Infallible const-sized construction requires a positive dimension at monomorphization. No serde impl on these three wrappers. FHRR `rotate_bins` is reindexing, not a time delay; the rewrite trait call retains identical orientation.
