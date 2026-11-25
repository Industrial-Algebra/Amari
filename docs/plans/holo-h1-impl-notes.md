# H1 implementation — PR handoff

**Target:** develop, 0.25.0. The CPU and Rayon bind kernels and generated WGSL use `Cl3::PRODUCT_TABLE`. Full-word, grade and two-sided residual oracles test the arithmetic independently. WASM flat-key ingress validates the 32 factors before mutating memory. H3 memory-model and H2 numeric-policy changes are not part of this patch.

**Migration:** Raw `[1,e1,e2,e3,e12,e13,e23,I]` slots and their 32-block ABI are unchanged. Derived coefficients change: `e13 * e2` formerly produced `+I`; the correct result is `−I`. Stop readers of old computed bindings, allocate a new versioned cache namespace, regenerate derived keys from original operands, replay associations to rebuild traces/indexes, then switch readers. There is no lossless conversion from a compressed trace alone. Choose among `right_clifford_dual`, `hodge_dual` and `formal_complement` by semantics, not a blanket rename. JS clients must handle errors for singular/nonfinite keys; zero values are allowed. No claim of external `tracedecay` runtime coverage.

**Evidence (Rust 1.100.0, x86_64 Linux, four Cargo overrides unset):**

| Unit | Failing baseline → passing control |
|---|---|
| U1 | 1 arithmetic test failed (101) → 1 passed: 64 ordered pairs, both bracketings of 512 triples, direct-product blocks including Rayon. |
| U2 | 1 inverse test failed (101) → 2 passed (public inverse and frozen candidate certificate). Max unscaled coefficient errors in both products must be ≤1e−12. |
| U3 | 1 grade test failed (101) → 1 passed; all ordered homogeneous pairs and a mixed case. |
| U4 | Missing dual API (101) → 1 test passed; `cargo check -p amari-rewrite --features holographic-guidance` passed at the trait change. |
| U5 | Missing checked generator map (101) → 1 test passed; cyclic action, invalid maps, extreme shift, signed product preservation. |
| U6 | Device test failed on an actual wrong shader coefficient at batch 101 (101); Node failed mixed-key recall (1). Corrected shader passed on Mesa llvmpipe Vulkan: CPU dispatch 4 records/1024 coefficients, device readback 101 records/25856 coefficients, basis exact and mixed tolerance 1e−6. Built web bindings passed Node mixed/unit recall and rejection/atomicity checks. |

**Final gates:** holographic default and all features 169 passing each; rewrite `holographic-guidance` check and test (527 passing); native wasm/fusion tests 182 passing; scoped all-target/all-feature `clippy -- -D warnings`, workspace fmt and scoped docs passed. Fresh lock resolution, wasm generation (byte-identical surface), then structural regeneration, structural drift gate 1/1 and `catalog_wasm` 41/41. No rewrite files changed. The discovery holographic adapter consumes MAP, not the changed geometric dual API; fusion reexports compile. No new no-std claim (B1 remains deferred).

**CI:** `Holographic Shader Parity` provisions lavapipe, requires enumeration, then tests the selected Vulkan backend. The generic job's no-device skip is not evidence here. Operator: mark this job required in branch protection when it appears. CI and independent PR review follow handoff; nothing had been pushed at implementation handoff.

## Evidence appendix — H1 implementation review R1

**Provenance.** The RED observations below are excerpts from the retained implementation-run output, not reconstructions. A RED revision denotes **HEAD plus the new regression**, not a test already present at that commit. Historical GREENs were run on the edited tree immediately before committing. To verify the attribution, during R1 we also checked out **each exact U1–U5 GREEN commit** in a separate detached worktree and reran its GREEN command below; the named tests, counts and exit codes agreed. No history was rewritten. The final-tree replay is separately identified below.

Commands start at the workspace root unless noted. `c` expands to the following command, equivalent to the historical `/tmp/h1cargo` wrapper. All Cargo tests use the debug test profile, default features unless flags are shown. Counts are executed Rust tests, not the number of coefficient comparisons inside them.

```sh
c() { env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_TARGET -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER cargo +1.100.0 "$@"; }
```

### U1–U5: unit history and exact-commit GREEN replay

| Unit | Historical RED: revision, command, selection, exit, diagnostic | GREEN: unit commit, command, selection and exit (historical tree and R1 rerun) |
|---|---|---|
| U1 | `fbd74ed6` · `c test -q -p amari-holographic --all-features --test h1_kernel` · `u1_signed_words`, 1 failed, **101**. Actual e23 slot `-1.0`; expected `1.0` (other seven coefficients zero). | `252a3429` · same command · `u1_signed_words`, 1 passed, **0**. Checks both bracketings against an independent word result and the serial/Rayon factor paths. |
| U2 | `252a3429` · `c test -q -p amari-holographic --all-features --test h1_kernel u2_` · `u2_inverses`, 1 failed, **101**: `assertion failed: (one + v).inverse().is_none()` (`v=e1`). This RED is **not** a separate observed failure of the private residual-certificate test. | `589451d6` · `c test -p amari-holographic u2_` · default features; `u2_inverses` and `algebra::cl3::tests::u2_unscaled_certificate`, 2 passed, **0**. The latter rejects the frozen scaled-residual counterexample. |
| U3 | `589451d6` · `c test -p amari-holographic --test h1_kernel u3_` · `u3_grade_products`, 1 failed, **101**. First pair tests `1 ∧ 1`: actual coefficients `[0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]`; expected scalar 1. | `ac68f9e3` · same command · `u3_grade_products`, 1 passed, **0**. |
| U4 | `ac68f9e3` · `c test -p amari-holographic --test h1_kernel u4_` · filter targets `u4_duals` (1 intended, **0 ran**), **101**: ``error[E0599]: no method named `formal_complement` found for struct `Cl3` in the current scope``. `hodge_dual` and `right_clifford_dual` were also missing. | `abd8eca1` · same command · `u4_duals`, 1 passed, **0**. Historical compatibility check on this tree: `c check -p amari-rewrite --features holographic-guidance`, **0** (compile only). |
| U5 | `abd8eca1` · `c test -p amari-holographic --test h1_kernel u5_` · filter targets `u5_permutations` (1 intended, **0 ran**), **101**: ``error[E0599]: no method named `permute_generators` found for struct `Cl3` in the current scope``. | `db1f2bee` · same command · `u5_permutations`, 1 passed, **0**. |

### U6: two consumer gates, with distinct failure signals

Both historical RED probes ran while HEAD was **`252a3429`** (the U1 CPU product was already corrected), with the new probes present but before the consumer migrations. They were **not** first run against `db1f2bee`. The GREEN source tree was committed as **`cdad1d6d`**, and both paths were executed again on that exact commit during R1.

**GPU.** Historical RED, GREEN and R1 replay used the following on x86_64 Linux. Default GPU crate features plus `holographic`; `--ignored` deliberately selects the dedicated hardware test, rather than consulting the generic `CI=true` skip.

```sh
export CI=true
export LD_LIBRARY_PATH=/nix/store/xin0b9mlvl6w1qqhvr2nfdcv5qns1b13-vulkan-loader-1.4.350.0/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}
export VK_ICD_FILENAMES=/run/opengl-driver/share/vulkan/icd.d/lvp_icd.x86_64.json
c test -p amari-gpu --features holographic --test h1_shader_parity -- --ignored --nocapture
```

- **RED:** `h1_shader_parity`, 1 selected, **101**. CPU dispatch passed (`4 records / 1024 coefficients; shader=false`); after actual Vulkan dispatch: `batch=101 coeff=126 actual=-1 expected=1`.
- **GREEN and R1 replay:** `h1_shader_parity`, 1 passed, **0** each. Selected adapter `llvmpipe (LLVM 21.1.8, 256 bits)`, Mesa 26.1.4, backend Vulkan. Output: `readback OK: 4 records / 1024 coefficients; shader=false` and `readback OK: 101 records / 25856 coefficients; shader=true`. Basis coefficients exact; mixed tolerance `1e-6`.

**WASM.** Independent of the GPU environment. Node v24.18.0; wasm-pack dev profile, web target, crate defaults. Historical `OUT=/tmp/h1-web`; R1 used a newly built package at `OUT=/tmp/h1-r1-final.F7PS2s/web`:

```sh
(cd amari-wasm && env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_TARGET -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER RUSTUP_TOOLCHAIN=1.100.0 wasm-pack build --dev --target web --out-dir "$OUT")
node scripts/test-h1-wasm.mjs "$OUT"
```

- **RED:** build **0**, one Node script, **1**. The first mixed-key recall failed `assert.ok(Math.abs(x-value[i]/Math.sqrt(32))<1e-12)`. Later rejection cases were not reached in this RED.
- **GREEN and R1 replay:** build **0**, one script, **0** each: `H1 WASM fusion: mixed/unit recall and key error cases passed`. For each of two keys the script checks all 256 normalized output coefficients, a singular factor, wrong length and NaN at `store`, `retrieve` and `retrieveConfidence`, unchanged count after rejection, and acceptance of a zero value.

### Final implementation commit `cdad1d6d` — fresh R1 rerun

| Command (in addition to both U6 commands above) | Selection and result |
|---|---|
| `c test -p amari-holographic --all-features --test h1_kernel` | `u1_signed_words`, `u2_inverses`, `u3_grade_products`, `u4_duals`, `u5_permutations`: **5 passed**, exit **0**. |
| `c test -p amari-holographic --lib u2_unscaled_certificate` | `algebra::cl3::tests::u2_unscaled_certificate`: **1 passed**, exit **0**. |
| `c check -p amari-rewrite --features holographic-guidance` | Compile-only coordination gate, exit **0**. |
| `c test -p amari-rewrite --features holographic-guidance` | Whole package with guidance enabled, **527 passed, 0 failed, 0 ignored**, exit **0**; not 527 H1-specific tests. |

The 169 default / 169 all-feature package totals above are from the historical combined-tree verification, not new full-package runs for this appendix.
