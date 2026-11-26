# H3 implementation evidence and migrations

Cargo prefix `c`: `env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_TARGET -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER cargo +1.100.0`.

## U1 — additive trace

Independent RED reproduction on detached `48123e8a` plus the six `u1_` tests (no production overlay): `c test -p amari-holographic --test h3_memory u1_`, 6 selected; exit 101. Verbatim excerpts:
```text
all-batch partition: coefficient 1: expected 1, got 0.21364785360223157
repeated pair doubles: coefficient 2: expected 2, got 1
test result: FAILED. 1 passed; 5 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
GREEN on the edited unit tree at parent `48123e8a`: `c test -p amari-holographic --test h3_memory u1_` and `c test -p amari-holographic --features parallel --test h3_memory u1_`, each exit 0:
```text
test u1_empty_batch_and_merge_are_noops ... ok
test u1_fhrr_trace_preserves_real_and_imaginary_components ... ok
test u1_store_sequential_trace_is_additive_at_legacy_soft_beta ... ok
test u1_repeated_pair_doubles_its_contribution ... ok
test u1_store_sequential_trace_is_additive_at_hard_beta ... ok
test u1_batch_and_merge_partitions_match_sequential_trace ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## U2 — checked, transactional operations

At `19a867ce`, `c test -p amari-holographic --test h3_atomic` (default features), 2 selected, exit 101:
```text
batch accepted a zero divisor
accepted non-bipolar key
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
GREEN: edited tree at same parent, `c test -p amari-holographic --lib --test h3_atomic u2_`, exit 0:
```text
test memory::holographic_memory::tests::u2_checked_count_before_commit ... ok
test algebra::cl3::tests::u2_unscaled_certificate ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 154 filtered out; finished in 0.00s
test u2_invalid_key_classes_and_imaginary_values ... ok
test u2_transactional_bad_prefix_and_mixed_tracking ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## U3 — query evidence

Parent `a4d97096`, `c test -p amari-holographic --test h3_membership` (defaults). RED 2 intended, 0 ran, compile exit 101:
```text
error[E0432]: unresolved import `amari_holographic::memory::MembershipEvidence`
error[E0599]: no method named `membership` found for struct `HolographicMemory<A>` in the current scope
error[E0599]: no method named `nearest_key` found for struct `HolographicMemory<A>` in the current scope
```
GREEN: identical command, exit 0:
```text
test u3_full_exact_evidence_and_duplicate_indices ... ok
test u3_walsh_crosstalk_without_age_gradient ... ok
test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## U4 — sided recovery

RED at `2507e6bc` plus two factorizer regressions; `c test -p amari-holographic --test h3_factor`, defaults, exit 101 (2 ran). Local and core e12 slots differ in index but both return the wrong sign:
```text
  left: [0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0]
 right: [0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0]
  left: [0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0]
 right: [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]
test result: FAILED. 0 passed; 2 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```
GREEN at parent `24e79494` with U4 edits, same command; exit 0:
```text
test h3_sided_local ... ok
test h3_sided_core_wrapper ... ok
test h3_impossible_and_singular ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## U5 — cleanup and quantitative acceptance

`c test -p amari-holographic --test h3_cleanup` at parent `e1353799`, default features, RED compile exit 101, 3 intended / 0 ran:
```text
error[E0432]: unresolved import `amari_holographic::memory::WeightedStatus`
error[E0599]: no method named `cleanup_nearest` found for struct `Resonator<A>` in the current scope
```
GREEN (same parent, U5 edits) `c test -p amari-holographic --test h3_cleanup --test h3_factor`, exit 0:
```text
test u5_original_observation_certificate ... ok
test u5_orthogonal_and_invalid ... ok
test u5_real_update_and_hard_selection ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
test h3_sided_local ... ok
test h3_sided_core_wrapper ... ok
test h3_impossible_and_singular ... ok
test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

## U6 — migration and historical RED

Independent consumer probes were prepared against the old surface, before migrating each consumer. The discovery test ran on `2507e6bc`, default crate features plus `standard-probes`: `c test -p amari-discovery --features standard-probes --test h3_recall`, one selected (`h3_recall_v2_evidence_and_raw_coefficients`), exit 101:

```text
called `Result::unwrap()` on an `Err` value: InvalidInput("unknown probe `amari-probe:holographic:recall:v2`")
```

`node scripts/test-h3-wasm.mjs /tmp/h1-r1-final.F7PS2s/web`, one built web-module JS scenario, exit 1. Used the retained dev-profile web build from H1's `cdad1d6d` exact-commit replay (same implementation as merge `2507e6bc`), **not** a new H3 build. Verbatim excerpt:

```text
AssertionError [ERR_ASSERTION]: raw additive recall
+ actual - expected

  [
    0,
+   0.125,
+   0.125,
-   1,
-   1,
```

`c test -p amari-gpu --features holographic --test h3_memory` at HEAD `2507e6bc` with U1 in-progress but before the GPU error migration. `h3_gpu_memory_batch_is_atomic`, one selected, exit 101:

```text
singular final key must reject entire batch
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

**Stored data.** Keep the original trace and its convention metadata read-only. Create a new namespace labelled with *both* the H1 product convention and H3 additive-memory convention. Revalidate original ordered keys against the checked key class, reject invalid records explicitly, and replay every original `(key,value)` into an empty trace with the intended tracking mode. Check the resulting count and keys against the record log and compare raw coefficients to an independent sum of bindings before switching readers. Old attention traces cannot be rescaled into additive traces. If original pairs are lost, rebuild from upstream sources or retire the trace; no lossless conversion is known. Do not use nearest-key similarity as membership, probability or provenance. Partial-coordinate query semantics are deferred to H5, not simulated by ignoring masks.

U6 partial GREEN, working tree at `0a1b6291`: `c test -p amari-gpu --features holographic --test h3_memory`, `h3_gpu_memory_batch_is_atomic`, 1 passed, exit 0. New `wasm-pack build --dev --target web --out-dir /tmp/h3-evidence/web` in `amari-wasm` with sanitized environment and `RUSTUP_TOOLCHAIN=1.100.0`, exit 0. `node scripts/test-h1-wasm.mjs /tmp/h3-evidence/web` and `node scripts/test-h3-wasm.mjs /tmp/h3-evidence/web`, one scenario each, exit 0. Output: `H3 Node: additive recall, typed evidence, rejection and both cleanup modes passed`. This was the first consumer GREEN; the final integrated run follows.

**Discovery v1 → v2.** Change probe ID from `amari-probe:holographic:recall:v1` to `:v2`, and both schema IDs from `holographic-recall/{input,output}/v1` to `/v2`. The request shape (ordered seed pairs and query seed) is unchanged. Remove `confidence`; read `membership_evidence` (`{"kind":"present","indices":[...]}`, `{"kind":"absent"}` or `{"kind":"unknown"}`) and the separate nullable `nearest_key_similarity`. The seeded probe always tracks; untracked Rust/JS clients must not coerce `unknown` into absence. Use `raw_coefficients` instead of a cleaned value, and exact indices instead of inferred contribution weights. `capacity` is nominal. The 3-entry v2 cooperative-budget fixture charges 13,056 operations and 10 iterations. In the Rust/JS cleanup API replace `converged` with the nearest-selection outcome or weighted termination status plus the original-observation certificate.

## Final combined verification

Implementation lineage: U1 `19a867ce`, U2 `a4d97096`, U3 `24e79494`, U4 `e1353799`, U5 `0a1b6291`, U6 this commit. The following local executions were on the final U6 code tree at parent `0a1b6291` before the commit, not remote CI. Commands use `c` above; unspecified feature sets are defaults. **Every command below exited 0.**

| Command | Observed selection/result |
|---|---|
| `c test -p amari-holographic` and `c test -p amari-holographic --all-features` | Each 182 passed, 0 failed, 17 ignored. Includes all five H3 Rust integration suites. |
| `c test -p amari-holographic --all-features --test h1_kernel` | Five passed; `u1_signed_words` includes 64 pairs and 512 triples. |
| `c check -p amari-rewrite --features holographic-guidance`; `c test -p amari-rewrite --features holographic-guidance` | Check passed; 527 tests passed, none failed or ignored. No rewrite edits. |
| `c test -p amari-wasm --lib`; `c test -p amari-wasm`; `c test -p amari-fusion --features holographic` | WASM host 140 passed on each invocation; fusion 42 passed, 7 ignored. |
| `c test -p amari-gpu --features holographic --test h3_memory`; `c test -p amari-gpu --features holographic --test h1_shader_parity -- --ignored --nocapture` | One each: `h3_gpu_memory_batch_is_atomic` and `h1_shader_parity`. `CI=true`, Vulkan loader and lavapipe ICD selected. CPU readback 4 records / 1,024 coefficients; WGSL 101 / 25,856. |
| In `amari-wasm`: `wasm-pack build --dev --target web --out-dir /tmp/h3-final-Je8Ek9/web-pkg`; from root: `node scripts/test-h1-wasm.mjs /tmp/h3-final-Je8Ek9/web-pkg`; `node scripts/test-h3-wasm.mjs /tmp/h3-final-Je8Ek9/web-pkg` | One built web module; both scripts passed. Actual JS checked raw recall, membership, scores, singular rejection, nearest and weighted outputs. |
| `c test -p amari-discovery --features standard-probes --test h3_recall --test probe_holographic --test wire_schema_registry` | 18 passed, including `h3_recall_v2_evidence_and_raw_coefficients` (present/absent queries, independent raw MAP oracle, old confidence absent). |
| Fresh untracked resolution: `c generate-lockfile`; `bash scripts/generate-discovery-wasm-surface.sh`; `c run -p amari-discovery --example generate_catalog -- .` | Bindgen 0.2.130; repeated web-surface generation byte-identical. Structural catalog: 28 crates, 11,672 items, 108 edges, hash `97998b5e35abf3c34c71ea55af8e30ced701abcff4746a55d304f313e2d38daf`. |
| `c test -p amari-discovery generated_equals_checked_in`; `c test -p amari-discovery --test catalog_wasm` | 1 structural drift check passed; 41 WASM catalog tests passed. |
| `c fmt --all -- --check`; `c clippy $SCOPE --all-targets --all-features -- -D warnings`; `c doc $SCOPE --all-features --no-deps` | Formatting, Clippy and docs passed. `$SCOPE` = `-p amari-holographic -p amari-gpu -p amari-wasm -p amari-fusion -p amari-discovery`. Cargo manifest and existing rustdoc link warnings are separate from warning-denied Clippy. |

Ignored tests are not counted as passes. No external consumers, release, or remote CI are claimed verified.

## Implementation review R1 — committed regression gates

Parent `7f9ec2b7`; tests and evidence only. Both P2s concerned missing coverage, not reproduced production defects. The six new `r1_` fixtures passed on their first run against the reviewed implementation. To verify sensitivity, the following **negative controls** ran in disposable detached worktrees at the same parent plus the new tests, with isolated build directories. No mutated production source was staged.

| Default-feature command (`c` as above) | Deliberate change and observed RED |
|---|---|
| `c test -p amari-holographic --test h3_atomic r1_` | Commit trace after each batch prefix; advance merge count before addition. `r1_batch_superpose_rollback` and `r1_merge_superpose_rollback`: **2 failed**, exit 101. The marked backend reports wrapped `NumericalError` for `7 + 13` (prefix already bound and accumulated) or `3 + 13` (compatible tracked merge). Tests compare trace bits, count and ordered matching indices for every admissible key; a valid retrieval must still return 3. |
| `c test -p amari-holographic --test h3_cleanup r1_` | Use final β instead of initial β for budget one; keep first codebook position on a tie. Single-update, reversed nearest tie and finite-to-hard tie tests **failed**, 1 unaffected pass, exit 101. |
| `c test -p amari-holographic --test h3_cleanup r1_linear_schedule_three_updates` | Change the interpolation divisor from `N−1` to `N` only. **1 failed**, exit 101: `assertion failed: (actual - expected).abs() < 1e-12`. This distinguishes the middle weight, not just the final β. |

**Unmodified production tree, GREEN:** `c test -p amari-holographic --test h3_atomic --test h3_cleanup r1_` selected 2 atomic and 4 cleanup tests, all passed, exit 0. Cleanup tests: `r1_linear_schedule_three_updates`, `r1_single_update_endpoint`, `r1_nearest_tie_is_order_independent`, `r1_finite_then_hard_tie`. The one-update budget uses the initial endpoint when endpoints differ; coincident hard endpoints select the exact word with `Updated { iterations: 1 }`.

`c test -p amari-holographic` and `c test -p amari-holographic --all-features` each reported **188 passed, 0 failed, 17 ignored** (formerly 182 passed), exit 0. `c fmt --all -- --check` and `c clippy -p amari-holographic --all-targets --all-features -- -D warnings` both exited 0.
