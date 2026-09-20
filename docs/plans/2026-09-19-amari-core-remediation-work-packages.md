# amari-core Audit Remediation Work Packages — Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill for approved package plans. This portfolio is not a single large implementation task; decision-blocked packages require a detailed plan before coding.

**Goal:** Account for every finding from the first core audit and deliver bounded, independently verified corrections before the 1.0 readiness freeze.

**Architecture:** Repair shared arithmetic and numerical contracts first, then domain-specific algorithms and invariant-bearing APIs; migrate consumers to the correct semantics. Performance and structural cleanup follow correctness, with explicit package dependencies and a separate closure record for each finding.

**Tech Stack:** Existing `amari-core` const-generic algebra, `CoreError`/`CoreResult`, Cargo tests, independent word-product/reference oracles, compile-fail tests, and Criterion. New dependencies or verification tooling require a package-specific decision.

---

## Governing policy and current state

[Programme](2026-09-19-amari-1.0-audit-remediation-programme.md) · [Decisions](2026-09-19-amari-core-remediation-decisions.md) · [Original report](../audits/2026-09-19-core-foundations.md)

**Correct mathematics wins over compatibility.** Breaking changes are explicitly acceptable; downstream consumers will be updated. Do not build a compatibility layer around an invalid mathematical contract or treat a downstream assertion of buggy behavior as a requirement.

All packages below are **planned, unimplemented**. Owners, PRs, release assignments, and closure evidence are **unassigned**. “Decision-blocked” means the mathematical/API design is unresolved, not that breakage needs permission. The baseline is `165d7cc`; revalidate against current `origin/develop` before execution.

## Complete finding-to-package map

Every `finding_NN_*` test below is in [the immutable first-pass fixture](../audits/core-foundations-repro/tests/foundations.rs). Preserve that evidence; copy/adapt selected regressions into the production crate as fixes land.

| Finding | Priority | Primary package | Required counterexample coverage |
|---|---|---|---|
| CORE-01: false general inverse | P1 | CORE-W03 | `finding_01_inverse_of_invertible_mixed_grade_element`, `finding_01_inverse_rejects_zero_divisor` |
| CORE-02: implicit pruning breaks algebra | P1 | CORE-W01 | `finding_02_geometric_product_preserves_small_coefficients`, `finding_02_geometric_product_associativity_across_scales`; extend through projected products and duality |
| CORE-03: nilpotent bivector exponential | P1 | CORE-W04 | `finding_03_exponential_of_nilpotent_bivector` |
| CORE-04: nonsimple bivector exponential | P1 | CORE-W04 | `finding_04_exponential_of_nonsimple_bivector` |
| CORE-05: null magnitude used for grade dispatch | P1 | CORE-W04, with W02 policy | `finding_05_exponential_of_null_vector_is_not_identity` |
| CORE-06: unconverged exponential fallback | P2 | CORE-W04 | `finding_06_scalar_exponential_converges` |
| CORE-07: null elements compare equal to zero | P1 | CORE-W02 | `finding_07_approximate_equality_distinguishes_null_vector_from_zero` |
| CORE-08: degenerate Hodge contract mismatch | P2 | CORE-W05 | `finding_08_degenerate_hodge_matches_documented_formal_complement`, or approved replacement if the contract changes |
| CORE-09: wrongly graded typed dual | P1 | CORE-W06, with W05 | `finding_09_vector_dual_preserves_declared_wrapper_grade`, or compile-fail/correctly typed replacement |
| CORE-10: eight-coefficient SLERP | P1 | CORE-W07a | `finding_10_slerp_supports_two_dimensional_rotors`, `finding_10_slerp_preserves_four_dimensional_endpoint`, or enforced-domain alternatives |
| CORE-11: rotor logarithm/power | P1 | CORE-W07b | `finding_11_boost_power_one_preserves_rotor`, `finding_11_small_rotation_power_one_preserves_rotor`, `finding_11_negative_identity_power_one_preserves_double_cover` |
| CORE-12: negative-unit vector mapping | P1 | CORE-W07c | `finding_12_rotor_from_negative_unit_vector_to_itself`, `finding_12_rotor_between_negative_unit_vectors_maps_direction` |
| CORE-13: reflection convention mismatch | P1 contract | CORE-W07d | `finding_13_reflection_in_hyperplane_negates_normal`, or correctly named separate line/hyperplane APIs |
| CORE-14: unchecked raw rotor invariant | P2 | CORE-W06 | `finding_14_raw_rotor_constructor_preserves_even_grade`, replaced by checked rejection or compile-fail when appropriate |
| CORE-15: 3D component layout in generic APIs | P2 | CORE-W06 | `finding_15_bivector_setter_handles_all_four_dimensional_components`, or constrained-3D plus general-access alternatives |

Structural observations are tracked separately, not lost because they lack a `finding_NN` runtime test:

| Observation / baseline failure | Package | Required evidence |
|---|---|---|
| PERF-01: rebuilt Cayley table | CORE-W08 | Sparse/dense benchmarks, allocation/resource measurements, no sign/metric regression |
| TYPE/API-01: public fields, panic/silent invalid-index behavior | CORE-W06 | Constructor/mutation/conversion/serialization audit, compile-fail and runtime boundary tests, consumer migration |
| PERF/API-02: alignment claim, allocation, mixed concerns | CORE-W08 plus pending unsafe audit | Actual buffer-alignment/safety evidence; corrected-kernel benchmarks; justified refactor boundaries |
| DOC/TEST-01: weak assertions, assertion-free tests, inaccurate docs/coverage claims, headers | CORE-W09 | Exact/reference assertions, correct mathematical comments, documented public contracts, licensing-preserving header review |
| Default warning-denied Clippy fails at `verified.rs:159,339,387` | CORE-W00 | Green same command with arithmetic/order preserved; no blanket lint suppression |
| No-default imports missing in `generic.rs` and `rotor.rs` | CORE-W00 | Green no-default library check, then approved minimal-target matrix |
| `gf2_tests.rs` unconditionally imports disabled feature | CORE-W00 | Green std-only suite and explicit coverage of enabled `gf2` |
| Precision unused imports with default features off | CORE-W00 | Warning-denied minimal configuration checks |

## Dependency graph

```text
W00 baseline gates ──→ W01 arithmetic ──→ W02 metric/error policy
                         │                  ├──→ W03 inverse
                         ├──────────────────┴──→ W04 exponential
                         └──→ W05 duality (D04)

pending phantom/formal audit + D05 ──→ W06 type/domain boundaries
W02 + W04 + W06 + D03 ──→ W07a/b/c rotor operations
D04 + W06 ──→ W07d reflection
corrected kernels + unsafe audit + D06 ──→ W08 performance/refactor
all affected contracts ──→ W09 documentation/test-evidence closure
all packages + remaining core audit ──→ current-baseline core re-audit
```

Dependencies can be narrowed when a concrete subplan proves independence. They are not an excuse to postpone a self-contained correctness fix. W00/W09 research may proceed while mathematical decisions are discussed; shared-file edits must still be coordinated.

## CORE-W00 — Restore trustworthy baseline gates

**Status:** bounded scope identified; execution/feature-matrix approval pending CORE-D07.

**Suggested branch:** `fix/core-audit-baseline-gates`.

**Files:** `amari-core/src/generic.rs`, `rotor.rs`, `precision.rs`, `verified.rs`; `amari-core/tests/gf2_tests.rs`; possibly `amari-core/Cargo.toml` for explicit required-feature declarations. Do not change CI to hide a failure.

**TDD scenario:** reproduce each existing compile/lint failure before a behavior-preserving correction.

1. Capture default, std-only and no-default failures on the chosen implementation baseline, including full diagnostics and active compiler.
2. Repair missing `alloc` imports and correctly gate GF(2)-specific tests; run both disabled and enabled feature cases so “gated” does not mean “never tested.”
3. Correct precision import gates and the three Clippy loops without changing accumulation order or indexing semantics. Review the touched verified code; lint cleanup is not proof certification.
4. Run the original default suite and independent blade oracle; run all commands in the D07-approved baseline matrix with warnings denied where applicable.
5. Submit a separate PR with old/new gate evidence. Establish the next baseline; do not claim the full optional/formal matrix is green from these fixes alone.

**Exit:** the selected baseline commands genuinely pass, no global exclusions/suppression, and unresolved optional/target gates are explicitly assigned under D07.

## CORE-W01 — Preserve coefficients in fundamental arithmetic

**Status:** [detailed plan drafted](2026-09-19-amari-core-arithmetic-remediation-plan.md); execution not authorized.

**Depends on:** W00 for merge-gate readiness; rebaseline original oracle.

**Suggested branch:** `fix/core-exact-arithmetic`.

**Files:** `amari-core/src/lib.rs`; new `amari-core/tests/arithmetic_regressions.rs` and `basis_word_oracle.rs`; `CHANGELOG.md` when implementation is approved.

Remove implicit approximate pruning from geometric product, inner/outer products, contractions, Hodge coefficient transport and highest-grade detection. Separate structural zero from approximate queries. Preserve the current metric/sign/dual convention and do not broaden this task into inverse/exponential algorithms.

**Exit:** both CORE-02 tests plus dyadic small-coefficient identity, projected-product, contraction and dual-linearity tests pass; the complete small-signature oracle remains green. The public approximate-zero/`Zero` trait contract is tracked in W02, not silently redesigned here.

## CORE-W02 — Define metric, error and normalization semantics

**Status:** decision-blocked on CORE-D01; draft detailed plan after resolution.

**Depends on:** W01.

**Suggested branch family:** `fix/core-numeric-contracts`.

**Files:** `amari-core/src/lib.rs`, `error.rs`; dedicated `amari-core/tests/numeric_contracts.rs`; actual consumers discovered in the impact inventory.

**Work units:**

1. Record distinct meanings for signed metric quadratic form, coefficient error/norm, approximate equality, exact/approximate zero and normalization.
2. Add null-difference regressions, signed-square preservation, finite/non-finite and extreme-scale cases before changing behavior.
3. Correct equality/error measurement without replacing the signed metric used by algebra and geometry. Resolve what `num_traits::Zero::is_zero` promises.
4. Make vector/versor/general-multivector normalization domains explicit; an absolute scalar magnitude is not a proof of rotor membership.
5. Update consumers whose comparisons or stopping conditions rely on a null metric difference. Investigate failed downstream tests for old mathematical mistakes.

**Exit:** CORE-07 closes; dispatch/convergence clients have a sound approved error measure; no assertion claims a positive-definite/submultiplicative norm where the chosen form lacks that property.

## CORE-W03 — Correct general multivector inversion

**Status:** decision-blocked on CORE-D02 and its D01 numerical policy.

**Depends on:** W01/W02.

**Suggested branch family:** `fix/core-general-inverse`.

**Files:** existing inverse in `amari-core/src/lib.rs`, `error.rs`; algorithm module path chosen by the approved design; `amari-core/tests/inverse_regressions.rs`.

**Work units:**

1. Promote both CORE-01 counterexamples and add two-sided inverse/reference cases across signatures, zero divisors, null elements, invertible elements containing null terms and mixed grades.
2. Approve supported dimension/resource limits, error semantics, a justified restricted fast path and the general algorithm. Do not call reverse/scalar-norm a general inverse.
3. Implement with independent left/right coefficient residual checks and a numerical-failure policy; test ill-conditioned inputs separately from provable singularity.
4. Migrate consumers to the correct fallible API where needed. Explicitly distinguish a supported singular input from an unsupported dimension.

**Exit:** `2+e1` in Cl(3,0,0) inverts to `(2-e1)/3`; `1+e1` is rejected as noninvertible; all promised domains meet their approved residual/error bounds and resource limits. No blanket rejection of all mixed grades to make the tests pass.

## CORE-W04 — Correct exponential dispatch and algorithms

**Status:** decision-blocked on CORE-D02 for the general numerical algorithm/domain; D01 for error/dispatch policy.

**Depends on:** W01/W02.

**Suggested PRs:** scalar-square/nilpotent correction, nonsimple/general exponential, integration with rotor construction.

**Files:** `amari-core/src/lib.rs`, `rotor.rs`, `error.rs`; `amari-core/tests/exponential_regressions.rs`; algorithm extraction only after design approval.

**Work units:**

1. Port CORE-03–06 and establish independent references: square-zero `1+B`, scalar `exp(x)`, and commuting e12/e34 factorization.
2. Check grade membership by coefficients, and scalar-square membership by the full square. Never use indefinite magnitude to prove either condition.
3. Correct the scalar-square branches, including the nonzero nilpotent limit, with numerically stable small-generator behavior.
4. Implement the approved general algorithm with convergence/error reporting and resource bounds, or an explicitly approved enforced domain restriction. Never silently return degree-19 truncation as converged.
5. Verify rotor construction from genuine bivectors using the **full** `R reverse(R)=1` identity and grade-preserving action, including nonsimple generators where supported.

**Exit:** all four finding families close through mathematically correct outputs or an explicitly approved, enforced domain contract; no identity substitution for failed convergence.

## CORE-W05 — Resolve duality contracts

**Status:** decision-blocked on CORE-D04; coordinate type returns with W06.

**Depends on:** W01; W06 for final typed API.

**Suggested branch:** `fix/core-duality-contracts`.

**Files:** `amari-core/src/lib.rs`; `amari-core/tests/duality_regressions.rs`; relevant documentation and consumer call sites.

Separate metric-induced Hodge operations from metric-free complements if both are supported. Fix documentation and implementation to one explicit convention per API. Add basis-complement orientation, linearity, supported double-dual identities, negative metric factors and degenerate/null-input tests. Do not require inversion of a null pseudoscalar.

**Exit:** CORE-08 closes under an approved convention; CORE-09's grade return is correct in combination with W06; callers cannot mistake a degenerate metric map for an invertible complement.

## CORE-W06 — Make type and dimension guarantees real

**Status:** decision-blocked on CORE-D05 and the pending `verified*`/phantom audit.

**Depends on:** approved mathematical domains from D01/D03/D04; W05 coordination.

**Suggested PRs:** grade representation/constructors; dimension-aware components; checked rotor boundaries; consumer migration.

**Files:** `amari-core/src/lib.rs`, `basis.rs`, `rotor.rs`, `error.rs`; `verified.rs` only after its audit/design; `amari-core/tests/type_invariants.rs`, `dimension_contracts.rs`; compile-fail fixture location/tooling chosen in D05; downstream resolved call sites.

**Work units:**

1. Inventory every public construction, conversion, mutable access and optional serialization path for grade/signature/rotor invariants. Resolve overlap with the existing verified hierarchy before adding new marker types.
2. Introduce the approved private representations and checked constructors. Grade projection must be explicitly named as lossy if it is offered; it is not validation.
3. Constrain inherently 3D conveniences or provide dimension-general operations with specified component order. Validate both indices in table/basis APIs, not merely their flattened sum.
4. Reject invalid raw rotor input or make it unrepresentable. Remove silent identity fallback and post-hoc grade projection as invariant enforcement.
5. Update all affected base consumers. Record separate backend/binding coordination without attempting their excluded implementation audits.

**Exit:** CORE-09/14/15 and TYPE/API-01 close. Invalid grade/dimension construction fails at compile time where promised, otherwise returns a precise documented error. Tests cover invalid inputs and all supported mutation/conversion paths; phantom markers are backed by real invariants.

## CORE-W07 — Correct rotor-group and reflection operations

**Status:** decision-blocked on CORE-D03/D04, with representation coordination under D05.

**Depends on:** W02/W04/W06 for shared semantics; narrow independent subplans when possible.

**Files:** `amari-core/src/rotor.rs`, `error.rs`; separate `rotor_interpolation_regressions.rs`, `rotor_log_power_regressions.rs`, `rotor_mapping_regressions.rs`, `reflection_regressions.rs` under `amari-core/tests/`; consumer migrations.

| Subpackage | Scope | Acceptance beyond original reproductions |
|---|---|---|
| W07a | Interpolation domains, not just loop lengths | Correct endpoints, identity/self interpolation, double-cover policy, full rotor membership and grade-preserving action; enforce unsupported signatures/dimensions |
| W07b | Logarithm/power, boosts and branch behavior | `R^0=1`, `R^1=R`, integer powers agree with composition, inverse where supported, resolvable tiny rotations, circular/hyperbolic/parabolic and multi-plane cases per contract |
| W07c | Vector-to-vector construction | Correct negative-unit mapping, same-vector identity, causal-class/null/antipodal handling, norm preservation and explicit incompatible-input errors |
| W07d | Hyperplane versus line reflection | Normal/tangent action, involution, negative-signature normal inverse, documented arbitrary-grade outermorphism or enforced vector-only input |

Each subpackage gets its own ≤8-task executable plan, decision references and PR. Do not fold all rotor changes into one large replacement. Passing one Euclidean 3D example does not certify arbitrary Spin(P,Q,R).

**Exit:** CORE-10–13 close, including every CORE-11 branch/scale reproduction. Consumer-facing changes are documented and migrated; no route silently discards coefficients or returns identity for an unsupported transform.

## CORE-W08 — Measure and remove structural performance costs

**Status:** research/benchmark plan pending CORE-D06 and the unsafe/SIMD audit; not a correctness prerequisite except where resource safety blocks an API.

**Files:** `amari-core/src/cayley.rs`, `lib.rs`, `aligned_alloc.rs`, `simd.rs`; `amari-core/benches/performance_suite.rs`; proposed extraction paths only after approval.

1. Benchmark the corrected kernels and allocator behavior: sparse/dense inputs, repeated/changing signatures, several dimensions, single/batch calls, actual buffer alignment and memory ceilings.
2. Compare bounded table reuse and on-demand blade multiplication, including `no_std`/concurrency implications. Preserve correct accumulation semantics and avoid an unbounded global cache.
3. Reduce redundant grade projections/norm computations and allocations where measured. Keep representation/cache changes independently reviewable.
4. Run algebraic oracles, numerical regressions, applicable safety checks and benchmark comparisons on the same recorded machine/toolchain.

**Exit:** PERF-01/PERF/API-02 have measured dispositions; alignment and safety claims refer to the coefficient buffer, not just its owning struct; no invented speedup targets or thresholds from the stale v0.17 benchmark table.

## CORE-W09 — Repair assertions, documentation and readiness evidence

**Status:** partial tasks can proceed now; final contract docs depend on their owning packages.

**Files:** `amari-core/src/lib.rs`, `tests/geometric_product.rs`, `tests/products.rs`, `tests/audit_tests.rs`, `README.md`, public-module docs, benchmark suite; root readiness documentation in a separate documentation PR.

1. Replace norm-only/one-sided assertions and assertion-free tests with exact coefficient/reference expectations; document why disjoint grade-1/grade-2 blades commute.
2. Preserve/promote the complete basis oracle rather than calling selected pairs a complete table check. Record supported input domains and coverage gaps honestly.
3. Replace fragile unconsumed wall-clock performance tests with appropriate deterministic correctness tests and separate consumed Criterion measurements.
4. Document every changed public contract with runnable examples, valid domains, numerical policy and error behavior. Establish documentation lints deliberately; rustdoc without missing-docs lint is not completeness evidence.
5. Review missing headers consistently with the repository's existing license; adding headers is not permission to relicense the project.
6. Update the live closure records and later rebaseline `docs/roadmap/V1_READINESS_PLAN.md` against current evidence, not historic test counts.

**Exit:** DOC/TEST-01 closes; package docs describe the implemented mathematics and actual feature support. The old report remains historical and unchanged.

## Portfolio closure checklist

- [ ] All 15 finding rows have complete closure records, including all linked reproductions or approved replacements.
- [ ] Structural observations and baseline gate failures have explicit verified dispositions.
- [ ] The rest of amari-core has been audited; new findings are added, not folded into a claim that batch 1 covered them.
- [ ] Required feature/toolchain/target and consumer matrices are approved and green; exceptions are named and owned.
- [ ] Independent review has checked both mathematical reasoning and implementation; no P0/P1 or stable-contract blocker remains.
- [ ] A current integrated baseline is re-audited before the 1.0 freeze. Package completion is not release/publication approval.
