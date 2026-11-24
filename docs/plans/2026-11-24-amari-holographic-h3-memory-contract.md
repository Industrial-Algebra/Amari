# Amari holographic H3 — additive memory, query evidence and resonator correctness

**Status:** Contract draft for independent review, not implementation approval. **Date:** 2026-11-24. **Scope:** `amari-holographic/src/memory/{mod,error,holographic_memory,resonator}.rs`, affected configuration and consumers. See [accepted audit, reproductions and operator decisions](../audits/2026-11-18-amari-holographic-audit.md) at baseline `10f60c19` and [the #189 accumulation/cleanup distinction](2026-06-21-additive-superpose-binding-algebra.md). H3 applies that distinction to the memory implementation; it does not claim #189 already migrated it. Breaking changes with disclosed migrations are preferable to successful-looking wrong answers.

## 1. Findings and decision record

| ID | Evidence at baseline | Unit |
|---|---|---|
| M1 | Three equally weighted values at β=0 receive weights `(¼,¼,½)` on sequential store, `(½,¼,¼)` when grouped. A hard winner can erase an item while the count advances. | U1 |
| M2 | Empty memory says present, confidence 1. At identical load, oldest/newest recall cosine `.000082/.965734` have identical reported confidence `.940894`. | U3 |
| M3 | A singular query becomes a valid-looking zero with confidence `.756883`. Store, batch and merge can swallow algebra failure and still update metadata. | U2 |
| M4 | Merge an untracked item into tracked memory: count 1, consistency false. All-false and all-true masks give the same nonzero recall. | U2 for tracking; mask disposition below |
| M5 | For `bound=e1·e2`, left book `[e1,−e1]`, right `[e2]`, the recovered factors multiply to `−bound`. | U4 |
| M6 | `converged=true` at reconstruction cosine −1; other fixture converges before first update at similarity 0; constant infinite β selects the wrong exact match. | U5 |

**Operator decisions, 2026-11-24:** (1) H1 retains a verified local `Cl3` table, gated by the independent 64-pair/512-triple oracle (no core adapter in the hot product). (2) H1 plus WASM fusion, GPU fallback/WGSL and stored-binding migrations **are in 0.25.0**. (3) H1/H3 **contracts** proceed in parallel; H3 is next because M1 conflicts with #189. H2/H4/H5 follow. No H3 release inclusion was decided. Stage H3 evidence on a known-law backend independently; final ProductClifford integration must run on the accepted H1 tree, not absorb H1 arithmetic fixes here.

## 2. Work units and executable acceptance conditions

**Per-unit TDD:** for each U below add named failing tests before changing its implementation, verify the intended red (not a setup/feature failure), implement and refactor only once green. Include positive and negative controls, actual executed test counts, head, command, backend and feature settings. Use coefficient/complex-component assertions or independent residuals; a cosine of 1 cannot detect a scale error. Preserve the [memory replay](../audits/holographic-repro/src/bin/memory.rs) as baseline evidence rather than editing its output into the new oracle.

### U1 — One additive trace model (M1)

Define raw trace `T = Σ kᵢ.bind(vᵢ)` with zero for the empty trace. Use #189 `superpose` and (if weighted) coefficient `scale`, **not** `bundle`, majority, normalize or temperature-dependent selection. `store_batch` and `merge` must mean concatenation of records; ordered keys and count advance together. A normalized display/readout is not the raw trace. If attention memory is retained it requires a distinct name/API, not an undocumented beta/config switch changing storage semantics.

**Red → gate:** store `(1,e1),(1,e2),(1,e3)`: expect `e1+e2+e3` both sequentially and under every partition into batch/merge, for legacy β=0 and hard settings if accepted. Repeated pair doubles its contribution; empty batch/merge is a no-op. Verify real and imaginary components of an FHRR trace (`to_coefficients` alone loses imag). Exact dyadic fixtures; documented rounding tolerance, not a bitwise associativity promise for arbitrary f64. Run default and parallel. If introducing weights, pin zero, signed and nonfinite policies with an independent sum test.

### U2 — Fallibility, atomic state and tracking (M3; coupled part of M4)

Use `HolographicResult`/`memory/error.rs` through the public store, batch, merge and retrieval paths where failure is possible. Reject inadmissible retrieval keys and nonfinite inputs/results; preserve cause (singular vs invalid data, dimensions, incompatible tracking/configuration, arithmetic). Singular need not mean small norm. Never substitute zero, reuse stale trace, return `valid=true` after error, or discard the error at the consumer. Check before commit of trace, count and tracking; on **any** rejected batch/merge, all remain unchanged. Checked APIs must not panic. Specify empty-retrieval outcome separately from U3 membership.

**Domain seam with H2:** do not trust `inverse().is_ok()` as proof for every existing backend: at this baseline MAP and FHRR even admit zero. Admit a documented checked key class (e.g. bipolar MAP, unitary FHRR), verify actual two-sided identity in the full representation, or report unsupported. Do not use cosine or FHRR's real projection alone. Require `0`, NaN/∞ and actual zero divisors to fail, without mandating a wholesale H2 redesign. Use accepted H1 checked inverse for ProductClifford mixed keys. Pin the storage policy: if a key is accepted for a *retrievable association*, verify its domain at ingestion rather than only on a later query.

**Red → gate:** singular final key after a valid batch prefix; injected `superpose` failure after successful binds; failed merge; checked count overflow fixture; valid zero *value* with good key. Assert returned variant, unchanged state and subsequent successful retrieval. For tracking, reject incompatible merges transactionally or prove ordered keys/indices remain faithful; gate tracked+untracked in both directions, empty source, tracked+tracked and attribution indices. For `2+e1` and value `e2`, raw single-key recall is `e2` on H1. An unexpected error on a good fixture fails the test (no `if let Ok` skip).

### U3 — Membership and confidence need evidence for this query (M2)

Empty memory cannot report a key present. `D/ln D`, `√(D/n)` and any `near_capacity` threshold are **nominal load heuristics**, not calibrated confidence, measured interference or membership predicates. Proposed minimal tracked mode: compare query with actual stored keys using a documented comparison/domain and threshold; distinguish exact match, nearest-key similarity and fidelity of the retrieved value. A key match does not certify value recall. Untracked mode with insufficient evidence must say unknown/unavailable, not produce a fabricated score. If a numerical *probability* is retained, specify key/value distribution, independence, success metric/threshold, calibration dataset, seed protocol and held-out calibration/false-positive gates before approval. Otherwise expose labelled evidence without probability language. Remove or explicitly rename old fields/helpers rather than silently changing their meaning.

**Red → gate:** empty → not present; same count, present and orthogonal absent query produce distinguishable tracked evidence; untracked unknown never coerced to yes or to calibrated certainty; duplicates/ties and zero-noise positive tested. Replay eight orthogonal Walsh-sign keys and a common identity value: additive raw trace gives equal recall cosine `1/√8` for every stored key, not an age gradient. This is a deterministic cross-talk control, not a statistical capacity theorem. Test an ambiguous/noisy negative at the documented boundary. Check Rust, discovery and JS do not relabel nominal SNR as query evidence.

### U4 — Sided factor recovery (M5)

For `T=A·B`, estimate **B** by `A⁻¹·T` and **A** by `T·B⁻¹`. Ordinary one-key memory unbinding is correctly on the left; do not reverse it. Expose sided operations or spell out operands; validate inverses as in U2 and propagate nested cleanup failures. Do not assert that factors are unique when the codebooks permit multiple decompositions.

**Red → gate:** actual factorizer, left candidates `[e1,−e1]`, right `[e2]`, bound `e12`: expect `(e1,e2)` and all coefficients of `A·B−T` zero (not merely a reported similarity). Use the core-backed Euclidean wrapper as a clean noncommutative control, repeat on corrected `Cl3`; include singular candidate policy and an impossible reconstruction. An all-commutative test or singleton books on both sides cannot establish correctness here.

### U5 — Honest cleanup and stopping (M6; decision required)

**Open point R1: real weighted iteration (proposed default) vs explicitly named nearest-neighbor search.** The default computes normalized nonnegative softmax weights on finite scores at each step and forms `Σ weightᵢ·codewordᵢ` by `scale`+`superpose` on the full representation. For hard/infinite β branch directly to documented deterministic argmax; avoid `∞−∞`. Validate schedules, thresholds and budgets at ingress, define the one-step endpoint and tie rule. If reviewers instead recommend nearest-neighbor, remove annealing/iteration claims and unused temperature controls; document exact score and ties. No silent argmax marketed as a weighted update.

Define the target of each certificate: cleanup must distinguish nearest available candidate from acceptable evidence relative to the **original observation**; factorization must measure coefficient residual between the **returned** product and bound. Use finite absolute+relative tolerances and a defined zero-target policy, stated with test fixtures; high cosine alone cannot certify reconstruction. Record stalled/budget-exhausted/rejected separately from success. A previous score of zero is not a previous iterate. Do not claim guaranteed convergence to the true item from arbitrary inputs.

**Red → gate:** orthogonal singleton cannot report successful recovery merely by standing still; exact target passes; poor fixed point, failed nested step, NaN β, zero budget, hard ties and constant +∞ with exact non-last match all have specified outcomes. For default weighted mode check one actual update against a hand-calculated unequal-weight vector, verify schedule endpoints and that changing β changes the soft update. For the alternative verify exhaustive nearest-neighbor agreement and no annealing claims. Factorizer returning `−T` or `2T` must not report success for nonzero T. Freeze algorithm and quantitative stopping policy in contract review before implementing U5.

## 3. Consumers and migration (U6)

Migrate `amari-fusion` reexports/docs, `amari-gpu/src/holographic.rs::GpuHolographicMemory`, `amari-wasm/src/fusion.rs` memory/resonator wrappers and `amari-discovery/src/probes/holographic.rs` including wire schemas, budgets and fixtures. Inventory any remaining call sites with a workspace search. No silent validation-score→probability conversion to preserve an old signature; if discovery changes a published wire field's meaning/type, version the contract or introduce a new field/endpoint and document the old one. Make absence of evidence distinguishable from numeric zero in JSON/JS. Expose typed failures as JS errors, not numeric sentinels or traps.

**Red → gate:** consumer-level present/absent and singular fixtures (including a real built wasm module in Node), raw additive trace/recall and error propagation in GPU-wrapper and discovery tests, qualified examples that handle `Result`, updated schemas and regenerated catalogs (WASM then structural) on the combined tree. Migration note must explain old attention trace → replay original pairs to rebuild; no rescale of a compressed trace restores lost entries. Identify renamed result fields, membership tracking requirement, mask disposition, and migration from `converged: bool` to the approved status semantics. Version retained artifacts separately from H1's product convention. No claim of external runtime validation without evidence.

## 4. Non-goals and deferral register

| Item | Boundary and next owner |
|---|---|
| M4 coordinate-mask semantics | Tracking consistency is inseparable from U1/U2 and **in H3**. A generic mask over Euclidean blades, MAP components and complex frequency bins needs a separate coordinate/domain contract. Proposed **H5** owner for a checked partial-query design and mask oracles. In H3 remove the misleading entry point or return explicit unsupported rather than silently ignoring the mask. If review finds a trivially coupled sound mask contract, record evidence and amend before implementation. |
| H1/H2 arithmetic | No product-table, arbitrary-key inverse or generic similarity overhaul here. Only claim recall/evidence on supported key and score domains; use validated controls, reject unsupported operations. Reference H1 on the merged tree. |
| Optical H4/H5, no-std B1, full memory snapshots, GPU shader redesign, calibrated capacity research | Out of scope; H5 owns B1. No new serde/portability or calibrated probability guarantees. A separately named legacy attention model requires its own tests if retained. |
| External consumers/hardware | Disclose compatibility impact; external binaries are not claimed tested. No exclusion of in-workspace wrapper, Node or wire-contract tests. Any new deferral needs operator approval, reason, owner and milestone. |

## 5. Review and completion

Separate contract and implementation loops, **12 rounds maximum each**; record findings, decisions and dispositions against unit and exact revision. Independent reviewers must run the counterexamples, audit the update/error paths, validate sidedness and test actual consumer outputs. Closure requires **zero unresolved P1/P2** in the agreed scope and operator approval; stop and escalate at cap. Before contract sign-off choose the evidence/membership API, tracking incompatibility behavior and R1 cleanup/stopping policy (including numeric thresholds); record operator decisions, do not infer them from this draft. Default and all-feature suites, scoped consumers, fmt, docs and warning-denied clippy run with the audit's sanitized Cargo environment on 1.100.0. Known no-default-features failure B1 is not a passing gate. Implementation and release timing require separate authorization; operator alone merges.
