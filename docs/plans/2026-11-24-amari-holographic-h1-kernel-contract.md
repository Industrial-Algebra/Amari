# Amari holographic H1 — verified local Clifford kernel and 0.25.0 migrations

**Status:** Contract draft for independent review; the three programme decisions below are settled, not approval of an implementation. **Date:** 2026-11-24. **Baseline:** [Phase A audit and operator decisions](../audits/2026-11-18-amari-holographic-audit.md), audited tree `10f60c19`. Correct mathematics takes precedence over compatibility. Checked APIs must return a typed failure, never panic or return an invalid success.

## 1. Context, findings and decision record

The recommended `ProductCliffordAlgebra<K>` is a direct product of K independent Euclidean `Cl3` factors. It is not `Cl(3K)` and does not mix factors. The local coefficient order is `[1,e1,e2,e3,e12,e13,e23,I]`, **not** raw core mask order; its mask map is `[0,1,2,4,3,5,6,7]`. Preserve this public layout, including the order of the 32 blocks in JS/GPU arrays. The defect is in the operation, not this choice of coordinates.

| ID | Observed at baseline | Required repair |
|---|---|---|
| C1 | `algebra/cl3.rs:158–237`: 16/64 basis products wrong, 142/512 associativity failures; same wrong signs in serial and Rayon product. `(e1 e3)e2=+I` but `e1(e3 e2)=−I`. | U1, U6 |
| C2 | `cl3.rs:278–296`, `product_clifford.rs:190–197`: `1+e1` admitted as a unit; for `2+e1` the purported inverse yields product `1+.8e1`. These fail even on the correct scalar/e1 subtable. | U2, U6 |
| C3 | `cl3.rs:607–628`: final-output grade filters yield `1∧e1=0` and `e1·I=0`. | U3 |
| C4 | `cl3.rs:399–404`, `clifford.rs:325–335`, trait `algebra/mod.rs:435`: right dual, multiplication by I and Hodge star conflated; null blades silently annihilated. | U4 |
| C5 | `cl3.rs:440–452`: cyclic generator substitution sends `e12` to `e13` instead of `e23`. Invertibility of a slot shuffle was an insufficient test. | U5 |

**Operator decisions, 2026-11-24:** (1) Keep a verified local `Cl3` multiplication table; no generic core adapter on the binding hot path. (2) H1 **and** WASM fusion key migration, GPU fallback/WGSL parity and stored-binding migration disclosure are in 0.25.0. (3) Contract H1 and H3 in parallel; H3 is next because of the #189 additive-trace discrepancy. H2/H4/H5 queue behind. H3 need not land for H1 to expose honest checked key construction.

## 2. Bounded units and acceptance evidence

**Unit-level TDD:** for **each** U below, add its named regression to a durable CI target, run it against the old behavior (or capture a genuine missing-API failure), record the red result, implement, rerun, then refactor with it green. Include actual head, command, feature, runner, selection count and exit status. Never update expected values from the production table. Exact basis fixtures use equality on **all** coefficients; cosine is not an equality assertion.

### U1 — One local product and a genuinely independent oracle (C1)

Regenerate the signed 8×8 output-slot table from the Euclidean relations `ei²=1`, `ei ej=−ej ei` for `i≠j`; correct the printed table as well. Route `Cl3::{geometric_product,bind}`, `Mul`, and serial/Rayon product-algebra components through that **one** local kernel. Output remains 8 scalars per factor; do not convert each product through a heap-allocated generic multivector. Generate the WGSL equivalent from this source where practicable; a second handwritten Rust formula is not permitted.

**RED → gate:** retain the [signed-word oracle](../audits/holographic-repro/src/bin/clifford.rs) independently in the test suite: concatenate sorted words, count cross-word inversions, cancel repeated generators. It must not read the shipping table or call its sign helper. Cross-check it with core's mask-indexed `Multivector<3,0,0>`. Exercise **all 64 ordered pairs** and **all 512 triples** (both bracketings against the word result, not just each other); exercise all pair results in distinct direct-product blocks on serial and Rayon paths and verify untouched blocks. Add one mixed sum to test bilinear accumulation. Exact expected example: `(e1 e3)e2 = e1(e3 e2) = −I`. CI must fail on a sign mutation, not merely print mismatch counts.

### U2 — Checked two-sided inverse, including products (C2)

Expose failure in the inherent and trait paths, including unbind and any product with one bad factor. `reverse(x)/s` is a fast path only when `x reverse(x)` really is a finite nonzero scalar and the quotient is representable. Otherwise use a general checked inverse (a cold-path adapter to core via the explicit slot map is eligible). Reject nonfinite inputs/intermediates or uncertifiable/unrepresentable results; do not use small coefficient norm as a synonym for singularity. **Frozen acceptance certificate (R1 P2#1 resolved):** acceptance requires the UNSCALED identity error — max absolute coefficient of both `xy−1` and `yx−1` — to be ≤ `1e−12` on certified fixtures. A normwise scale-aware residual (‖xy−1‖/‖x‖‖y‖) may be REPORTED as conditioning metadata but can NEVER be an acceptance criterion. **Mandatory negative fixture:** `a = 1−2⁻⁴⁰`, `b = 2³⁹`, `x = 1+a·e1`, candidate `y = b(1−e1)`: both products equal `0.5−0.5e1` (order-one identity error) while the scale-aware residual is `≈2.27e−13`; this candidate MUST be rejected. Document the numerical success criterion, not an exact real invertibility decision for all f64 inputs.

**RED → gate:** exact zero divisor `(1+e1)(1−e1)=0`; inverse must fail. For `x=2+e1`, expected inverse `(2−e1)/3`; assert both `xy` and `yx` against identity, all coefficients within `1e−12` on this well-conditioned case. Cover scalar, bivector, mixed-grade, one singular factor among good factors, NaN/∞ and a small invertible scalar (e.g. `1e−20`). Pin success for the ordinary fixtures; reject on failure rather than substituting zero/identity. Require finite two-sided residuals under the frozen unscaled identity cap above; ill-conditioned solves that cannot meet it report failure with conditioning metadata rather than passing a scaled bound.

### U3 — Grade-selected wedge and contraction (C3)

For homogeneous grades `r,s` select the `r+s` part of their geometric product for wedge (zero if >3), and the `abs(r−s)` part for core-style inner product, **including scalars**. Sum over pairs of homogeneous parts for general inputs. This is not exclusively left contraction. Test all 64 pairs against independent word/grade selection and `amari-core/src/lib.rs` inner/outer products, plus mixed-grade bilinearity. **RED → gate:** `1∧e1=e1`, `e1·I=e23`, repeated-vector wedge zero; agreement on vector/vector inputs alone does not pass.

### U4 — Separate named duals (C4)

On `Cl3` and general `CliffordAlgebra<P,Q,R>` expose distinct operations through the geometric trait: **right Clifford dual** `x I⁻¹`; **metric Hodge** `reverse(x) I`, in core's convention; **formal complement** maps each ordered blade to its oriented missing factors so that `blade ∧ complement(blade)=I`, independent of metric, extended linearly. Do not call these aliases of a generic `dual`. If a compatibility shim is proposed, it must explicitly name its destination and never return a metric result on a degenerate signature. Right dual and Hodge return typed error for `R>0` even on scalar input; formal complement remains available. Reuse core Hodge/formal-complement behavior in the generic wrapper.

**RED → gate:** independently tabulate all eight local blades, compare named maps and the generic wrapper after mask conversion; in Euclidean 3D `right_dual(1)=−I`, `hodge(1)=I`, `hodge(e12)=e3`. Include a negative-signature case distinguishing Hodge from formal complement (e.g. in `Cl(0,1,0)` Hodge of `e1` is −1, complement +1), and a null generator with two typed errors and nonzero formal complement. Retain reversal and conjugation controls on all blades.

### U5 — Checked generator cycle and its exterior extension (C5)

For the cycle `e1→e2→e3→e1`, send every ordered blade to the product of the mapped vectors **with sorting sign**: `e12→e23`, `e13→−e12`, `e23→−e13`, `I→I`. Apply shifts modulo 3, including negative shifts and `i32::MIN`, without negation overflow. Check any user-supplied generator/index map before using it (out of range, repeats → error); a fixed cycle needs no fallible runtime lookup. **RED → gate:** basis table, inverse cycle, period 3, signed generator products and preservation of all 64 products. Test a product-algebra factor through its public permutation path for positive K. Not an assertion that arbitrary coefficient shuffles are algebra automorphisms.

### U6 — Consumer migrations and executable parity (release-blocking)

* **WASM:** `amari-wasm/src/fusion.rs` flat keys at `store`, `retrieve`, `retrieveConfidence` share checked conversion: 256 finite coefficients, ABI order unchanged, every key factor invertible under U2. Values are not required to be invertible. Surface failure via JS error, not a wasm trap or successful zero; validate before mutation. Execute built bindings in Node: valid mixed key `(2+e1)` repeated in all factors, known value and expected recall (allow for the existing explicitly normalized retrieval); singular factor, wrong length and nonfinite key. Verify count unchanged on rejected stores, and successful unit-key baseline. Update TS examples/signatures if changed.
* **GPU:** `amari-gpu/src/holographic.rs::{batch_bind_cpu,batch_bind_gpu,get_bind_shader,batch_unbind_cpu}`. Run a small batch through actual CPU dispatch and a batch **above the GPU cutoff** through compiled WGSL with readback. Record adapter/backend and evidence of path taken. Across the 32 blocks/batch cover every ordered pair plus mixed and distinct-block inputs; compare each path to the word oracle, not just to each other. Basis results are exactly representable in f32. Declare tolerance for non-exact f32 fixtures. Exercise checked CPU unbind on valid and singular keys. A no-adapter early return, WGSL parse-only test or fallback does **not** satisfy shader parity. A software WebGPU adapter executing the actual shader is acceptable; otherwise bring evidence from a capable runner before sign-off. **Nominated CI mechanism (R1 P3#4 resolved):** shader parity runs as its own required CI job that installs `mesa-vulkan-drivers` (lavapipe software Vulkan) and REQUIRES an adapter to enumerate — no adapter is a job FAILURE, not a skip; the job must not consult the existing `CI=true` availability gate (`amari-gpu/tests/common/mod.rs::direct_gpu_runtime_available`, observed `false` under `CI=true`), which is precisely the skip this contract prohibits. Adapter/backend identity is recorded in job output as evidence of path taken.

## 3. Consumer impact and migration disclosure

Audit `amari-fusion` reexports, WASM fusion and GPU public tests/examples for ambiguous duals and raw key constructors. Compile migrated callers on the combined tree. `tracedecay` consumed 0.23 per the census; disclose changes publicly rather than claiming that client has been tested. Changelog and crate migration notes must distinguish unchanged **raw slot layout** from changed **derived coefficients**. Version/cache-namespace computed bindings. Recompute traces, indexes and saved derived keys from original operands under the new kernel, including CPU/GPU crossovers. Do not promise an inverse migration from a lossy trace alone, or silently mix old and new artifacts. Provide one old/new signed-blade example and a clean-rebuild recipe. Regenerate discovery WASM surface then structural catalog on the merged tree where affected; run both drift gates.

## 4. Non-goals and test-exclusion/deferral register

| Item | Disposition/owner and when |
|---|---|
| C6–C10, F1–F5 | H2 owns mixed-signature cosine, general key-domain policy, numeric reductions, Rayon bundling and K=0/component-shuffle policy. H1 cannot use these deferrals to bypass U2 or the consumer key checks. |
| Memory model, optics, no-default-features (B1) | H3, H4/H5 and H5 respectively. Pre-existing no-std build failure is disclosed, not a passing configuration. No unrelated GPU rewrite/wgpu upgrade. |
| External consumer runtime / GPU availability | Publish migration note; external app execution not claimed. WGSL execution and Node are **not excluded**; blocked runner → report blocked, no release acceptance. |

Run default and all-feature `amari-holographic` tests, scoped consumer tests, wasm build/runtime, fmt, warning-denied scoped clippy, docs and required CI. Use Rust 1.100.0 with the four Cargo overrides unset as in the audit. New named test filters must select nonzero expected counts. Record any new exclusion with reason, owner, milestone and explicit operator approval.

## 5. Review loop

Independent contract review first; reproduce the audit cases and challenge the oracle independence, inverse criterion, dual conventions and backend observability. Record new operator decisions separately from the three already made. Only after zero outstanding P1/P2 and operator contract approval may a bounded implementation PR begin. Separate implementation loop: execute gates at the presented head, triage findings by unit, rerun affected tests and cross-unit migrations. Each loop caps at **12 rounds**; unresolved at cap → stop and return to operator, not automatic acceptance. Operator alone merges; this contract does not authorize release actions.
