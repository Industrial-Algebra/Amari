# Amari holographic H2 — binding domains and numerical policy

**Draft for operator decision and independent review; not implementation or release authorization.** Base: `develop` at `2507e6bc` (#387). H2 concerns what a binding operation promises on a given input, not how an association is stored or evidence of membership is obtained.

## 1. Context, findings and decision record

Sources: [audit and register](../audits/2026-11-18-amari-holographic-audit.md), [replay crate](../audits/holographic-repro/), [H1](2026-11-24-amari-holographic-h1-kernel-contract.md), [H3](2026-11-24-amari-holographic-h3-memory-contract.md). The following are fresh local observations on the merge, not pre-H1 output. `c` below means `env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_TARGET -u RUSTC_WRAPPER -u RUSTC_WORKSPACE_WRAPPER cargo +1.100.0`.

| Finding | Executed counterexample on `2507e6bc` | Unit |
|---|---|---|
| C6 (P1) | Full wrapper, negative basis vector against itself: −1; null basis vector against itself: 0. Both coefficient vectors have Euclidean length 1. | U2 |
| C7 (P2) | `CliffordAlgebra::<0,0,1>::random_versor(1).inverse()` returned an error. Every vector in this signature squares to zero, so trying more draws cannot establish invertibility. | U1 |
| C8 (P2) | At scalar `1e−200` / `1e200`, `Cl3` and one-factor Product both report norm `0` / `inf`, normalization error / successful zero, self-similarity `0` / `NaN`. Full Euclidean wrapper gives correct norm and unit normalization at both scales but the same bad self-similarities. **Unlike normalization**, H1 `Cl3::checked_inverse` succeeds with `1e200` / `1e−200` respectively. | U2 |
| C9 (P1) | Two Product identities, β=0, one Rayon thread: sequential scalar 1, parallel 0.5. Source `product_clifford.rs::par_bundle_all` injects `product_zero` and uses `a.bundle(&b,beta).unwrap_or(a)`; the latter is an error-discard path, not an observed worker error in this fixture. | U4 |
| C10 (P2) | `ProductCliffordAlgebra::<0>::identity()` constructs; `permute(0)` then panics. Shuffling scalar factors `[2,3]` with `[0,2]` produces `[2,0]`, not a permutation or an error. | U3 |
| F1 (P1) | FHRR bins `[2,3i]` and MAP coordinates `[2,−3]` both unbind their binding of `[1,1]` to `[4,9]`; both accept zero inverse. Component reciprocal yields the FHRR identity here. Dense complex control: IDFT(bind) vs direct circular convolution max error `1.90e−15`; reciprocal recovery `9.42e−16`; adjoint recovery error `20.396`. The defect is the inverse claim, not frequency-bin multiplication. | U1 |
| F2 (P2) | FHRR and MAP `[1e200]` normalize to zero, NaN normalizes to `Ok` containing NaN. Global normalization of four unit bins gives magnitude 0.5 per bin and recovery 0.25; per-bin `normalize_unitary` on `[0,3+4i]` returns `[0,0.6+0.8i]` (not unitary). | U1/U2 |
| F3 (P2) | FHRR and MAP at `D=0` panic on `permute(1)`. For four unit spectral bins, permutation leaves the IDFT impulse at index 0, not 1. | U3 |
| F4 (P2) | MAP `bundle([2],[0],1)` gives .9426807891; list of those two gives .7615941560. `bundle([2],[2],0)` gives 0. Hard tie on `[1],[-1]` gives +1 on **both** paths; a tie policy is needed, not a claim that these two hard outputs differ. | U4 |
| F5 (P2) | Binding by −1 before vs after hard MAP cleanup of a tied vote gives opposite signs. `logsumexp(0,0,0)=0`, but at β=`1e−6` it is `693147.1805599453`, not near zero. | U5 |

**Replay method.** Used a scratch copy of `docs/audits/holographic-repro` with path dependencies pointing at the merge, preserving the historical fixtures. Ran `RAYON_NUM_THREADS=1 c run --release --manifest-path <scratch>/Cargo.toml` on its Clifford probe (as `src/main.rs`): use named `hodge_dual` instead of removed `dual`, print the null error and rejected `1+e1` rather than unwrapping, and print checked inverse, full normalization and one-factor Product norm/normalization/similarity at each scale. Exit 0; panic in the K=0 control was caught. H1 controls: zero mismatches for 64 products and 512 associativity triples; `2+e1` times reported inverse now equals 1; null Hodge dual returns `DegeneratePseudoscalar`. Ran `c test --release --manifest-path <scratch>/Cargo.toml --test spectral -- --nocapture --test-threads=1` (original `oracle`, `dense_oracle`, `surface`, `shift_bundle`: 4 passed, exit 0; these characterize defects). Additional scratch `projection_probe`, one passed, printed the F2 projection and F4 hard tie above. The replay manifest enables holographic `full`.

**Settled constraints.** H1's verified local kernel, two-sided unscaled `1e−12` Product factor inverse certificate, named duals, signed generator permutation and checked WASM keys stand. Do not replace them with norm tests. H3's checked retrieval-key classes, exact tracked-key membership / explicit `Unknown`, separately labelled nearest similarity, additive trace, and frozen coefficient-wise recovery certificate stand. A more general algebraic inverse in H2 would not by itself widen H3 membership. Values need not be keys. Neither H2 implementation nor H2 inclusion in 0.25.0 is decided here.

**Operator decision record — RESOLVED 2026-11-25 (round-1 remediation; per-row detail below).** No option was a default approval; each row records the operator's selection. Public signatures, errors, tolerances and migration follow from these selections and the frozen certificates in the Numerical Acceptance Law section.

| Decision | Operator selection |
|---|---|
| D1 | **Typed keys + named general inverse.** Typed validated key classes (bipolar MAP, unitary FHRR, H1-certified Product keys) for binding/unbinding/membership; a separately named checked-inverse unbind on general elements; every admitted key certified `k·k⁻¹ = 1` both sides; adjoint/correlation exposed as correlation, never as unbind. |
| D2 | **Per-operation typed policy.** Every public operation: empty/zero-norm/NaN/±∞/unrepresentable each get a defined result or a typed error at a named boundary — no universal sentinel; K=0/D=0 rejected at construction; shape invariants at construction and deserialization. |
| D3 | **Remove + two named operations.** Unqualified similarity removed on non-Euclidean signatures (typed error); separately named normalized metric pairing (signed, documented) and positive coefficient cosine (Euclidean-only). Generic trait callers get the typed error. |
| D4 | **One global bundling semantics.** Bundle is one defined global operation: associative list semantics, deterministic tie rule, thread-count-independent results, error propagation (never `unwrap_or`), empty list = identity with a defined result; the binary pair form is documented as fold order. |
| D5 | **Both, separately named.** log-sum-exp kept on its true mathematical domain (diverging zero-temperature limit documented; β=0 per D2); log-mean-exp exposed separately with the finite mean limit; no continuity claim spanning both. |
| D6 | **Rename to bin reindex + checked maps.** Spectral bin reindexing under an accurate name with checked bijective maps and D=0 rejection; any temporal delay would be a distinct phase-ramp operation; trait `permute` documented as bin reindexing; amari-rewrite's positional consumption migrates with a name-level note, zero behavior change (stop-and-coordinate if behavior would change). |

| Decision | Alternatives and consequence |
|---|---|
| D1: keys and sampling | Typed validated unitary/bipolar keys with distinct general elements, checked inverses on general elements, or separately named surfaces for both. Specify representability, near-unit membership tolerance vs exact tracked-key equality, reciprocal/certificate limits, projection of zero bins, and fallible vs signature-restricted versor generation. Adjoint/correlation must not masquerade as general unbinding. |
| D2: admissible inputs | For each public operation specify empty dimension/list, zero norm, subnormals, unrepresentable result, NaN/±∞ and β (`<0`, zero, finite positive, ±∞, NaN). Support with a defined result, or reject explicitly at a reachable boundary; do not choose a single sentinel for all. State shape invariant at construction/deserialization vs checked operation, including K=0 and D=0. |
| D3: signed/null similarity | Remove unqualified similarity on these signatures, or separate explicitly named normalized metric pairing from positive coefficient cosine. Specify trait behavior for generic callers; no metric quantity labelled cosine, ranking evidence or probability. |
| D4: list/binary semantics | Specify ordered binary fold vs global list weights (different even at β=0); or publish distinct names for both. Define MAP cleanup timing, singleton/empty behavior, stable hard tie (including reversed order), reproducibility/tolerance across thread counts and whether pair/list must agree. Distinct algorithms require distinct promises. |
| D5: logarithmic vocabulary | Keep actual log-sum-exp on its mathematical domain; offer normalized log-mean-exp with a finite mean limit instead; or expose both separately. Decide β=0 handling and deprecation/name mapping. The diverging and finite limits cannot share one continuity claim. |
| D6: permutation surface | Keep spectral bin reindexing under an accurate name; if temporal delay is exposed, make it a distinct phase-ramp operation. Specify sign/direction, trait `permute` semantics and migration of positional encodings; distinguish bijection from any retained gather. |

## 2. Bounded units and acceptance evidence

### U1 — Invertible keys, correlation, projection and versor generation (F1, F2, C7)

Scope: `algebra/{mod,fhrr,map,clifford}.rs`; preserve H1 Product keys. Under D1 separate pointwise multiplication (general), adjoint and inverse. For each admitted key check `k·inv(k)` and `inv(k)·k` against identity, then recover a nontrivial value on the documented side. Assert every component, including imaginary bins; neither real projection nor cosine certifies an inverse. Global L2 normalization is not per-bin projection and projection changing a key is not a lossless migration.

**Red → gate:** use F1 examples above, a dense complex DFT/direct convolution oracle, unit phasors and bipolar positives; reject or correctly invert nonunit nonzero keys according to D1. Reject an all-zero or single zero bin/coordinate where an inverse is requested, nonfinite data, unrepresentable reciprocals; verify the `(3,4)` projection and zero-bin outcome. For random versors test zero factors (empty product), pure null positive factor count, a mixed-signature null vector such as `e+ + e−`, and bounded deterministic draws on a supported signature. No unbounded retry or `Ok` nonunit: every claimed versor has a certified inverse; otherwise report unsupported/failure. Regression on H1's `1+e1` and `2+e1` distinguishes singularity from non-unit norm.

### U2 — Robust norms and named similarities (C6, C8, F2)

Scope: `algebra/{cl3,product_clifford,clifford,fhrr,map,mod}.rs`. Separate positive coefficient geometry from a signed metric pairing. Define FHRR coefficient cosine on the **full** real/imag representation when offered. Normalize and compare on scaled coefficients; do not square the extremes first or form the product of two huge norms. Do not clamp NaN or a null metric square into apparent agreement.

**Red → gate:** independent reference `hypot`/scaled coefficients on scalar `±1e±200`, vectors proportional to `(3,4)`, opposite and orthogonal pairs. Check representable norm, actual normalized components and signed cosine, not just finiteness. Negative and null unit generators have coefficient cosine 1 where offered, signed pairings −1 and 0 where offered. Exercise zero, smallest subnormal, nonfinite and finite inputs with unrepresentable norm according to D2; distinguish a representable unit direction from an unrepresentable magnitude. Tests cover all five wrappers and compare serial/parallel paths where implemented. Quantitative bounds are FROZEN in the Numerical Acceptance Law (scale-aware relative `1e−12` on representable scales; absolute-cap `1e−12` at zero targets; typed rejection of subnormal/overflow per D2).

### U3 — Shape, bijections and positional meaning (C10, F3)

Scope: `product_clifford.rs`, `fhrr.rs`, `map.rs`, trait docs. State shape at all public entry points, not just a checked constructor next to an unchecked identity. A shuffle called a permutation must validate range **and** uniqueness. Do not change H1's signed exterior action inside a factor.

**Red → gate:** exercise `K,D ∈ {0,1,4}`, zero and negative shifts and `i32::MIN/MAX` without panic; check identity, inverse/composition and hand-indexed orientation. Check `[0,2]` and `[1,1]` at K=2, plus valid swap. For spectral bin shift test a nonconstant complex spectrum. For any claimed time delay use direct DFT/IDFT and an impulse: delay by +1 under forward negative-exponent DFT multiplies bin `k` by `exp(−2πik/D)`. Rotating bins leaves an all-ones spectrum unchanged. Preserve rewrite goldens or stop per §3; never relabel their bin rotation as temporal delay.

### U4 — Weighted bundling, ties and parallel errors (C9, F4)

Scope: Product Rayon reducer, trait default fold, MAP pair/majority paths; apply D4 to each backend advertised to share the operation. Write down list weights, where cleanup occurs and whether order matters. Zero is not a binary average identity. Do not discard `Result` (including a failure in a non-first partition).

**Red → gate:** serial and Rayon `[1,1]` at zero; ordered scalars `[0,0,4]` distinguish fold (2) from uniform list mean (4/3) before any cleanup. Test empty/singleton, unequal weights at finite β, ties and swapped inputs at hard β, `[2],[0]` pair/list, and MAP zero-temperature-parameter cleanup. Compare actual coefficients against an independent formula for D4 under Rayon pools of 1, 2 and 4 threads and multiple partitions. Exercise an error after a valid prefix through the public list path; require the defined error, never a partial aggregate. No scheduling-dependent tie or swallowed invalid β even for short lists.

### U5 — Algebraic laws and temperature vocabulary (F5)

Scope: `algebra/mod.rs` contracts, helpers, affected examples/docs. Binding is distributive over *linear superposition* in the algebraic model, not generally over projection, voting or norm-based attention. Distinguish any statistical dissimilarity assertion from an identity.

**Red → gate:** coefficient tests of additive distributivity and the explicit MAP `−1` hard-tie counterexample; retain the latter as a negative law test. Test equal inputs `a=b=t`: log-sum-exp is `t+ln(2)/β`, log-mean-exp is `t` for β>0. Unequal inputs distinguish finite β from both the mean and the hard max. Check β approaching zero, β exactly zero, large finite β, nonfinite and disallowed β per D2/D5; do not smoke-test only `is_finite`. Update rustdoc equations, examples and deprecations together.

### U6 — Consumer gates and migration evidence

Apply §3 on the combined tree. **Red → gate:** for each changed public behavior demonstrate the old call or stored fixture and its approved successor (compile-fail for removed surface, runtime assertions for changed semantics). Keep H1 regression controls passing. No edits to `amari-rewrite` or its expected outputs to make this gate pass.

## Numerical Acceptance Law (round-1 P2#2 remediation — frozen, in-contract)

Per-operation certificates. H1's unscaled two-sided `1e−12` identity certificate and H3's elementwise `atol=1e−10, rtol=1e−8` recovery certificate are unchanged law; the following govern H2's units:

- **U1 keys and inverses:** every admitted key satisfies the H1 unscaled identity certificate (max absolute coefficient of both `k·inv(k)−1` and `inv(k)·k−1` ≤ `1e−12`); bipolar MAP keys are EXACT ±1 equality; FHRR unitary keys certify per-bin complex components. Non-unit nonzero keys unbind via the named general inverse under the same certificate or fail typed; `Ok` nonunit is forbidden.
- **U2 norms, normalization, similarity:** scale-aware RELATIVE accuracy — for representable scales the computed norm has relative error ≤ `1e−12` (stable accumulation; `hypot`-style) and normalization preserves direction with each coefficient within `1e−12` relative of the scaled input. Mandatory control: at norm `1e−200` the computed norm is nonzero and within `1e−12` relative of `1e−200` (a zero result FAILS); at `1e200` likewise; subnormal/overflow/unrepresentable results reject typed per D2, never silently zero. Similarity is Euclidean-only (D3) with `1e−12` relative accuracy on representable inputs.
- **U4 bundling:** sequential and parallel (Rayon) results are BITWISE identical on fixtures (deterministic tie rule; exactly-representable fixture values; the SAME accumulation order on both paths), and a worker error produces the SAME propagated error on both paths. The β=0 fixture: list bundling of `[1,1]` is `1` on both paths; the historical `0.5` divergence is the mandatory RED.
- **U5 logarithmic helpers (R3 P1 corrected):** `log_mean_exp(a,b,β) = (1/β)·ln((e^{βa}+e^{βb})/2)`; its β→0 limit is the ARITHMETIC MEAN `(a+b)/2` — NOT `ln(mean)`. Control fixture: `log_mean_exp(2,2,β) = 2` EXACTLY for every β>0 (`(1/β)ln(e^{2β}) = 2`); at β=0 the defined value is `(a+b)/2` (continuous); `log_mean_exp(0,0,1e−6) = 0`. `log_sum_exp(a,b,β) = (1/β)·ln(e^{βa}+e^{βb})` on β>0 (diverging β→0 limit documented; β=0 is a typed domain error — the historical β≤0→mean branch is the removed fake continuity); control fixture `log_sum_exp(0,0,1e−6) = 693147.1805599453` (= ln2/β) within `1e−9` relative; β=+∞ → `max(a,b)` for both. β<0 or NaN anywhere → typed error.
- **Zero-target rule (R3 P2 resolved):** wherever a certificate is stated in relative terms, the true/reference value 0 makes relative error undefined — the certificate at zero targets is the ABSOLUTE cap ≤ `1e−12` (norm of an exact zero vector; zero coefficients; zero-bin components). H3's recovery-certificate zero-target rule (`atol` only) is unchanged.

No implementation may introduce other tolerances, scaling substitutions, or zero-target policies for these operations.

## 3. Consumer impact and migration disclosure

| Consumer | Standing acceptance / migration |
|---|---|
| `amari-rewrite/src/holographic` | `encode.rs:77` consumes `BindingAlgebra`/`FHRRAlgebra`; positional `permute`, additive `superpose`, vector digests and `recall.rs` similarity are observable. Run `c check -p amari-rewrite --features holographic-guidance` and `c test -p amari-rewrite --features holographic-guidance` including Task-47-era goldens; baseline **527 passed, 0 ignored**, both commands exit 0. Run `c test -p amari-holographic` and `c test -p amari-holographic --all-features` (each **169 passed, 21 ignored**, exit 0 here). These are standing gates, not proof of H2 correctness. If encodings/rankings change, stop and coordinate with rewrite owner; no rewriting goldens or silently reusing a digest namespace. |
| `amari-wasm/src/fusion.rs` | Preserve `checked_holographic_key` at `store`, `retrieve`, `retrieveConfidence`: exactly 256 finite coefficients, all 32 factors H1-invertible; values need not be invertible. Build web target; run `node scripts/test-h1-wasm.mjs <built-web-dir>` plus changed domain/error cases. JS errors must not trap or mutate the count. Inventory exposed random/score helpers as well as ingress. |
| `amari-gpu/src/holographic.rs` | Public `batch_bundle` currently delegates pair bundling to CPU at β=1, not an arbitrary-list shader. Compare nontrivial pair output and errors to approved rule. Preserve H1 `CI=true` lavapipe gate (`c test -p amari-gpu --features holographic --test h1_shader_parity -- --ignored --nocapture`): actual Vulkan readback at 101 records, not just fallback/compile. Do not claim shader bundling parity unless that path is executed. |
| `amari-fusion` | Feature `holographic` reexports types in `src/lib.rs`. Compile/test downstream-style trait calls through reexports, `c test -p amari-fusion --features holographic`; disclose changed signatures/errors and do not maintain an unchecked back door. |
| `amari-discovery/src/probes/holographic.rs` | Seeded `MAP256` tracked recall and additive superposition are distinct probes. Exercise each public probe, compare coefficients and error/wire semantics. A score is not H3 membership. If surface changes, regenerate WASM first (`bash scripts/generate-discovery-wasm-surface.sh`), then structural catalog (`c run -p amari-discovery --example generate_catalog -- .`); verify drift on the merged tree. |
| External and persisted consumers | `tracedecay` was recorded against `^0.23.0`; no runtime validation claimed. Provide old → new Rust/JS examples for key admission vs general binding, metric pairing vs retrieval score, shuffle vs delay, list cleanup and β helpers. Version changed traces, scores, positional digests and cached bundles by convention; rebuild from original ordered operands/keys where semantics change. No generic conversion from a compressed aggregate or invented unitary key. Keep H1's product migration independent. |


### API surface named by the D-decisions (round-2 remediation)

| Decision | Public names (Rust) | Typed errors |
|---|---|---|
| D1 | `TypedKey` validation constructors per backend (bipolar MAP, unitary FHRR, H1-certified Product keys) — `bind_key`/`unbind_key` on typed keys; `unbind_general` (named checked-inverse unbind on general elements); `correlation` (adjoint, never an unbind) | `KeyDomain` (nonunit/non-bipolar/non-representable), H1 `Inverse` errors propagate |
| D2 | per-operation `Result` returns; construction-time `try_new`/deserialization validation for K>0, D>0, shape | `EmptyDimension`, `InvalidShape`, `Unrepresentable`, `NonFinite` |
| D3 | `coefficient_cosine` (Euclidean-only), `normalized_metric_pairing` (signed, documented); unqualified `similarity` removed on non-Euclidean | `UnsupportedSignature` |
| D4 | `bundle` (global list semantics, deterministic ties, thread-count independent); pair form documented as fold order | worker errors propagate; no `unwrap_or` |
| D5 | `log_sum_exp` (true domain, diverging limit documented), `log_mean_exp` (finite mean limit) | `Beta` domain errors per D2 |
| D6 | `rotate_bins`/`permute_bins` (checked bijective maps), phase-ramp delay as a distinct future op if ever needed | `NonBijectiveMap`, `EmptyDimension` |


### Frozen signatures, domains, formulas and tie rules (R3 P2 remediation)

| Surface | Signature and frozen semantics |
|---|---|
| `log_sum_exp` | `pub fn log_sum_exp(a: f64, b: f64, beta: f64) -> AlgebraResult<f64>` — `(1/β)ln(e^{βa}+e^{βb})`, stable max-shift; domain: finite `a,b`, β∈(0,+∞]; β≤0, NaN → `Beta`/`NonFinite` typed error; β=+∞ → `max(a,b)` |
| `log_mean_exp` | `pub fn log_mean_exp(a: f64, b: f64, beta: f64) -> AlgebraResult<f64>` — `(1/β)ln((e^{βa}+e^{βb})/2)`, stable max-shift; domain: finite `a,b`, β∈[0,+∞]; β=0 → `(a+b)/2`; β<0, NaN → typed error; β=+∞ → `max(a,b)` |
| `bundle` (pair) | `fn bundle(&self, other: &Self, beta: f64) -> AlgebraResult<Self>` — the two-element case of the global formula below, documented as fold order |
| `bundle_all` (global) | `fn bundle_all(items: &[Self], beta: f64) -> AlgebraResult<Self>` — **a function of the whole list, not a pair-fold** (R4 P2 reconciled): weights `w_i ∝ exp(β(‖x_i‖ − max_j‖x_j‖))` over FULL-REPRESENTATION norms in ONE pass, index-ordered stable accumulation; equal norms ⇒ equal weights; ALL norms < `1e−10` ⇒ uniform weights (bundling zero-target rule); MAP `soft_sign(β)` cleanup after weighting in the soft branch; β=+∞ ⇒ component-wise `majority` with first-index tie-breaking; **singleton ⇒ the element unchanged (the identity property)**; empty ⇒ `Ok(zero)` as convention — zero is NOT a pair identity; β<0/NaN ⇒ typed error. Rayon accumulates in the SAME index order — bitwise-identical to sequential. **Explicit non-promises (documented, with the reviewer's control as the reason): partition-invariance and associativity are NOT claimed — at β=0 the list `[0,0,4]` gives left fold `2`, right fold `1`, global `4/3` (a weighted mean is provably non-associative); no pair-fold composition is offered or implied.** D4's original option text said "associative list semantics" — that word was an over-promise this contract corrects; the operator's selection (ONE global semantics, deterministic, thread-independent, singleton identity) stands. |
| F5 distribution non-law (R4 P2 reconciled) | Binding is NOT claimed to distribute over nonlinear cleanup anywhere. The audit's original witness (scalar −1 flip-flopping a tied hard vote) is RETIRED: under first-index ties, scalar negation commutes with hard cleanup on tied votes (`[1],[−1]` yields `−1` on both bind-before and bind-after paths), so that fixture no longer witnesses anything. The U5 negative gate becomes: any test asserting `cleanup(k⊙vᵢ) = k⊙cleanup(vᵢ)` in general MUST NOT be added; a distribution-assertion test fails review by construction. Migration disclosure: consumers that relied on the old tied-vote sign asymmetry (none known) see changed behavior. |
| `similarity` (trait) | `fn similarity(&self, other: &Self) -> AlgebraResult<f64>` — becomes fallible: Euclidean backends compute `coefficient_cosine` (relative `1e−12`); non-Euclidean signatures return `UnsupportedSignature` (D3 removal); `normalized_metric_pairing` is the separate named signed operation |
| MAP cleanup timing | pair and list paths apply cleanup at the SAME stage (after weighting, before return); the hard branch's majority vote is itself the cleanup — no second pass |

## 4. Non-goals and test-exclusion/deferral register

| Boundary | Disposition |
|---|---|
| H1, core and rewrite | No product table/inverse-certificate relaxation, core algebra rewrite, new shader optimization, or rewrite patch. Report upstream conflicts to the owning session. |
| H3 memory | No storage/merge redesign, probability calibration, membership widening, cleanup iteration or resonator changes; coordinate the approved key/score seam before integration. |
| H4/H5, B1 | Optics, general serde/shape hardening and feature/no-std repair remain separate. H2 must still close construction/ingress bypasses for any invariant it claims. Known B1 no-default failure is not a passing test; ignored suites are not evidence. |
| Release | No schedule commitment, dependency bumps, lockfile tracking or publication. H2 cannot inherit H1's 0.25.0 approval. |

## 5. Review loop

Separate independent **contract** and **implementation** loops, at most **12 rounds each**. Log exact reviewed SHA, findings/severity, unit, dispositions and operator decisions. Contract sign-off requires D1–D6 resolved, exact admissibility/error tables, formulas and tolerances fixed, and executable distinguishing fixtures for every option selected. Closure requires zero unresolved P1/P2 and operator approval; escalate at cap, do not downgrade a failing gate. Independent mathematical review reruns the counterexamples against the reviewed tree.

On implementation authorization only: U1 → U6, TDD per unit, one local commit per unit. Record parent plus regression RED, exact-commit GREEN, and final combined rerun with SHA, command, features, named tests, selected counts, exit and diagnostic/coefficients. Distinguish historical output from reproduction. Run the §3 gates, workspace fmt, scoped docs and all-target/all-feature Clippy `-D warnings` on affected crates. No panic, inverse, dispatch, or stored-data compatibility claim from diff inspection alone. Operator alone merges; release is a separate decision.
