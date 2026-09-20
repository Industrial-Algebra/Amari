# amari-core Exact Arithmetic Remediation — Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task after execution approval. Do not use the Mercury rapid-development workflow.

**Goal:** Resolve CORE-02 by removing implicit approximate pruning from fundamental `Multivector` arithmetic and structural grade selection, without changing the chosen algebra or adding an approximation policy.

**Architecture:** Preserve the existing Cayley sign/metric rules and accumulation order; skip only coefficients that are exactly zero. Use a private structural-zero helper in grade-based arithmetic, leaving public numerical-query design to CORE-D01/W02. Keep original audit evidence immutable and promote independent regression tests into the production crate.

**Tech Stack:** Existing Rust/Cargo and `amari-core`; integration tests using exact dyadic inputs and the already-written independent word-product oracle. No new dependencies or public types.

---

## Status, prerequisites and fences

**Package:** CORE-W01. **State:** detailed draft; not executed or implementation-authorized. **Owner/PR/release:** unassigned.

[Programme](2026-09-19-amari-1.0-audit-remediation-programme.md) · [Package dependencies](2026-09-19-amari-core-remediation-work-packages.md) · [Pending matrix decision](2026-09-19-amari-core-remediation-decisions.md#core-d07--verification-matrix-and-scope-boundaries)

- Mathematical correctness is paramount. Changes to results that previously depended on dropped coefficients are expected corrections, not compatibility regressions to preserve.
- Audit baseline: `165d7cc`. Reproduce on current `origin/develop`; coordinate with W00 and other editors of `amari-core/src/lib.rs`.
- W00 must restore the agreed baseline gates before this PR is called merge-ready. Approve D07's exact matrix/toolchain; do not waive a failing required check because it predates this change.
- **In scope:** finite-coefficient geometric/inner/outer products, contractions, Hodge coefficient transport, highest-nonzero-grade detection. Ordinary IEEE rounding/underflow/overflow still exists; this plan removes an artificial absolute cutoff, not floating-point nonassociativity in general.
- **Out of scope:** public approximate equality/zero/normalization contracts, inverse/exponential algorithms, changing metric/Hodge conventions, typed wrappers, generic/verified/GF(2) implementations, SIMD/cache/layout redesign, GPU/WASM audit.
- Public `is_zero()` currently documents approximate behavior; its inherent method and `num_traits::Zero` contract remain assigned to W02/D01. Fundamental arithmetic must stop using that approximate query as a structural predicate now.
- Do not replace **every** `1e-14` occurrence. Inverse thresholds, normalization and convergence require separate mathematical/numerical decisions.

## Task 1 — Establish the implementation baseline and oracle

**TDD scenario:** modifying tested code; reproduce the original failures and run existing tests first.

**Files:**
- Read: `docs/audits/2026-09-19-core-foundations.md`.
- Read: `docs/audits/core-foundations-repro/tests/foundations.rs`.
- Create: `amari-core/tests/basis_word_oracle.rs`.
- Do not edit: any file under `docs/audits/`.

**Steps:**

1. Fetch and create a fresh isolated worktree/branch from `origin/develop`, suggested `fix/core-exact-arithmetic`. Record HEAD and diff against the audit baseline for `amari-core`, workspace dependency manifests and relevant consumers. If the finding is already fixed, verify and link that fixing commit instead.
2. Record the approved compiler. Establish the worktree's ignored lock explicitly (`cargo generate-lockfile` if needed, or a known current resolution), then use `--locked` in subsequent verification. Do not copy a stale lock and silently claim an identical dependency baseline.
3. Run `cargo test --locked -p amari-core` and the agreed W00 matrix. Stop/report any new failure. Existing gate blockers must remain visible.
4. Reproduce the original CORE-02 tests with a temporary checkout/copy of the preserved audit fixture if audit documents have not yet landed on `develop`. Use its documented lock snapshot and path dependency to this implementation; do not rebase the source worktree onto an unreviewed documentation branch merely to obtain test data.
5. Promote **only the passing independent oracle** to `amari-core/tests/basis_word_oracle.rs`: copy the complete `coefficients`, `blade`, `word_product`, `check_signature` helpers and `oracle_all_basis_pairs_through_dimension_four` test from the pinned fixture. Imports needed: `use amari_core::{CayleyTable, Multivector};`. Retain the license header, independent-word explanation and all 35 signatures. Do not copy all intentionally failing probes into the production suite.
6. Run `cargo test --locked -p amari-core --test basis_word_oracle`. Expected: one test passes, covering 4,589 pairs. Commit this passing oracle-only checkpoint after normal verification; it does not fix CORE-02.

## Task 2 — Add exact small-coefficient regressions; fix geometric product

**TDD scenario:** RED → GREEN for geometric product; later tests remain red until their named tasks.

**Files:**
- Create: `amari-core/tests/arithmetic_regressions.rs` with the complete test fixture below.
- Modify: `amari-core/src/lib.rs`, `geometric_product_scalar` (baseline lines 295–310).

Use exact dyadic coefficients so zero results cannot be dismissed as tolerance artifacts. The new file is:

```rust
// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: Apache-2.0

use amari_core::Multivector;

type E3 = Multivector<3, 0, 0>;
const LARGE: f64 = (1_u64 << 50) as f64;
const SMALL: f64 = 1.0 / LARGE;

fn blade<const P: usize, const Q: usize, const R: usize>(
    index: usize,
    coefficient: f64,
) -> Multivector<P, Q, R> {
    let mut value = Multivector::zero();
    value.set(index, coefficient);
    value
}

#[test]
fn gp_small_finite_factor_is_not_zero() {
    let result = E3::scalar(SMALL).geometric_product(&E3::scalar(LARGE));
    assert_eq!(result.as_slice(), E3::scalar(1.0).as_slice());
    let vector = E3::basis_vector(0) * SMALL;
    assert_eq!(
        E3::scalar(1.0).geometric_product(&vector).as_slice(),
        vector.as_slice(),
    );
}

#[test]
fn gp_associativity_does_not_drop_a_small_intermediate() {
    let scale = (1_u64 << 25) as f64;
    let a = E3::scalar(1.0 / scale);
    let b = E3::scalar(1.0 / scale);
    let c = E3::scalar(LARGE);
    let left = a.geometric_product(&b).geometric_product(&c);
    let right = a.geometric_product(&b.geometric_product(&c));
    assert_eq!(left.as_slice(), E3::scalar(1.0).as_slice());
    assert_eq!(right.as_slice(), E3::scalar(1.0).as_slice());
}

#[test]
fn projected_outer_product_keeps_small_grade_components() {
    let a = E3::basis_vector(0) * SMALL;
    let b = E3::basis_vector(1) * LARGE;
    assert_eq!(a.outer_product(&b).as_slice(), blade::<3, 0, 0>(3, 1.0).as_slice());
    type Pga = Multivector<1, 0, 1>;
    let a = Pga::basis_vector(0) * SMALL;
    let b = Pga::basis_vector(1) * LARGE;
    assert_eq!(a.outer_product(&b).as_slice(), blade::<1, 0, 1>(3, 1.0).as_slice());
}

#[test]
fn projected_inner_and_contractions_keep_small_grade_components() {
    let a = E3::basis_vector(0) * SMALL;
    let same = E3::basis_vector(0) * LARGE;
    assert_eq!(a.inner_product(&same).as_slice(), E3::scalar(1.0).as_slice());
    let b = blade::<3, 0, 0>(3, LARGE);
    assert_eq!(a.left_contraction(&b).as_slice(), E3::basis_vector(1).as_slice());
    assert_eq!(b.right_contraction(&a).as_slice(), (-E3::basis_vector(1)).as_slice());
    type Mink = Multivector<1, 1, 0>;
    let negative = Mink::basis_vector(1) * SMALL;
    let scaled_negative = Mink::basis_vector(1) * LARGE;
    assert_eq!(negative.inner_product(&scaled_negative).scalar_part(), -1.0);
}

#[test]
fn structural_hodge_transport_is_linear_at_small_scale() {
    let small = E3::basis_vector(0) * SMALL;
    assert_eq!(small.hodge_dual().as_slice(), blade::<3, 0, 0>(6, SMALL).as_slice());
}

#[test]
fn structural_grade_reports_a_small_nonzero_component() {
    let value = E3::scalar(2.0) + E3::basis_vector(0) * SMALL;
    assert_eq!(value.grade(), 1);
    assert_eq!(E3::zero().grade(), 0);
}
```

**Steps:**

1. Run `cargo test --locked -p amari-core --test arithmetic_regressions`. At the audited implementation all six tests fail. If any already pass on the current baseline, record why; do not force an expected red count.
2. Isolate `cargo test --locked -p amari-core --test arithmetic_regressions gp_`; observe the two order-one geometric-product failures before editing production code.
3. In `geometric_product_scalar`, change only the two coefficient skip predicates:
   - `self.coefficients[i].abs() < 1e-14` → `self.coefficients[i] == 0.0`.
   - `rhs.coefficients[j].abs() < 1e-14` → `rhs.coefficients[j] == 0.0`.
   Preserve table generation, metric sign handling, loop order and accumulation. Do not bundle a cache or loop-vectorization change.
4. Re-run the `gp_` filter: expected both pass. Re-run the independent oracle: expected pass. Other regression filters may still fail; do not commit an incomplete red suite as a mergeable checkpoint.

## Task 3 — Separate structural zero in projected arithmetic

**TDD scenario:** RED → GREEN for previously skipped grade components.

**Files:** `amari-core/src/lib.rs`, private helper and inner/outer/contraction methods; the tests from Task 2.

**Steps:**

1. Run `cargo test --locked -p amari-core --test arithmetic_regressions projected_`. After Task 2, failures must still demonstrate grade-level pruning; investigate if not.
2. Add this **private** method in the existing `Multivector<P,Q,R>` implementation:

```rust
/// Whether every stored coefficient is exactly zero.
/// Internal structural predicate, not a tolerance-based numerical query.
fn is_exact_zero(&self) -> bool {
    self.coefficients.iter().all(|&coefficient| coefficient == 0.0)
}
```

3. Replace `mv_a.is_zero()` / `mv_b.is_zero()` in `inner_product`, `outer_product`, `left_contraction` and `right_contraction` with `is_exact_zero()`. At the pinned baseline these are the conditions at lines 326, 347, 586, 591, 618 and 623; use function names after rebasing, not blind line substitutions.
4. Do not change the inherent public `is_zero()` or the `num_traits::Zero` implementation in this task. They need D01's public contract decision. Do not add an epsilon parameter to fundamental products.
5. Re-run the `projected_` filter, then `gp_` and the independent oracle. Expected all selected tests pass. Check the negative-metric and null-basis wedge assertions, not only Euclidean output.

## Task 4 — Preserve coefficients in Hodge transport and grade detection

**TDD scenario:** RED → GREEN for two structural operations.

**Files:** `amari-core/src/lib.rs`, `hodge_dual` and `grade`; Task 2 tests.

**Steps:**

1. Run `cargo test --locked -p amari-core --test arithmetic_regressions structural_` and confirm both original failures.
2. Change the coefficient skip in `hodge_dual` (baseline line 660) from an absolute tolerance to `== 0.0`. **Do not change its metric/permutation convention**; CORE-08 is a different decision/work package.
3. Change `grade`'s `projection.is_zero()` check (baseline line 274) to the private `is_exact_zero()` predicate. Its documented contract is highest nonzero grade, not highest grade above a hidden tolerance.
4. Run the full `arithmetic_regressions` target: expected six tests pass. Run the full independent oracle: expected pass.
5. Inspect all internal uses of `is_zero()` in `lib.rs`. Any remaining use inside fundamental arithmetic must have an explicit reason or belong to the separately documented numerical-query API. Do not make this a repository-wide mechanical replacement.

## Task 5 — Check consumers and document the corrected contract

**TDD scenario:** existing behavior changes; validate assumptions rather than preserving incorrect expectations.

**Files:**
- Modify: `amari-core/src/lib.rs` docs for changed arithmetic/grade behavior.
- Modify: `CHANGELOG.md`, following its current unreleased-section convention; do not bump a release version during this task.
- Read/update only if required by verified impact: resolved base-Rust consumers; use a separate migration PR when scope exceeds this package.

**Steps:**

1. Document that algebraic operations do not apply an implicit approximate-zero filter; arithmetic still follows ordinary floating-point limitations. Document the distinction from the current approximate query without blessing its trait semantics as audited.
2. Search/type-resolve users of the affected arithmetic and grade APIs. Add targeted regression tests where hidden pruning was used as an undocumented sparsification/convergence mechanism. No exact list of affected crates is assumed from text search alone.
3. Investigate failures that reveal old downstream bugs. If a caller genuinely needs approximation, it must opt into a separately designed numerical operation, not regain silent truncation inside the product.
4. Add a changelog entry referencing CORE-02 and the corrected semantics; record affected consumers and follow-up numerical-policy questions.
5. Record any newly exposed issue under a new finding/linked package rather than silently expanding this PR into inverse, rotor, or wrapper redesign.

## Task 6 — Verify, review and close only this finding

**TDD scenario:** full post-change verification and independent mathematical/code review.

**Steps:**

1. Run the original CORE-02 reproduction filter against the candidate implementation. Both original decimal-scale cases must pass in addition to the exact dyadic production tests. The standalone fixture still contains unrelated unfixed findings; do not mark them fixed or demand a fully green fixture yet.
2. Format touched core files with `cargo fmt -p amari-core`, then run with the approved toolchain/dependency resolution:

```bash
cargo fmt -p amari-core --check
cargo test --locked -p amari-core
cargo test --locked --release -p amari-core --test arithmetic_regressions
cargo test --locked --release -p amari-core --test basis_word_oracle
cargo clippy --locked -p amari-core --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --locked -p amari-core --no-deps
cargo test --locked -p amari-core --no-default-features --features std
cargo check --locked -p amari-core --no-default-features --lib
```

3. Run the remaining D07-approved rows and type-resolved consumer tests, and satisfy **all current required PR checks**. The list above is not an exclusion of optional features, repository CI or backend/binding integration requirements. If W00 or external integration is still blocked, report it and leave the PR unmergeable rather than weakening the matrix.
4. Inspect the diff: no approximate cutoff remains in the targeted fundamental operations, no sign/metric rule or accumulation order changed, no unrelated implementation was folded in, and preserved audit evidence is untouched.
5. After green approved local checks, commit the complete correction and regressions on the isolated fix branch, e.g. `git commit -m "fix(core): preserve small coefficients in algebraic operations"`. Open a PR to `develop` only when authorized to do so; include RED/GREEN commands, baseline SHAs, migrations and known outstanding findings.
6. Obtain independent mathematical and Rust-code review. Close CORE-02 only after the reviewed PR is merged and its integrated-baseline evidence is recorded in the live remediation register. W02/W03/W04 and other findings remain open.

## Planning-time validation (2026-09-19)

The complete Task 2 test fixture was extracted unchanged from this Markdown and compiled from standard input into a temporary test executable, linked to the existing default-feature `amari-core` artifacts at `165d7cc`. Compiler: `rustc 1.100.0-nightly (787af2b8c 2026-08-25)`.

- Compilation: exit 0.
- Execution: exit 101; **all six tests fail** on the audited implementation, with the expected zeroed products/dual coefficient and incorrect highest grade.
- The exact dyadic Hodge coefficient is `8.881784197001252e-16`; comparisons are coefficientwise/exact, so a large epsilon cannot mask its loss.
- No production source or original audit fixture was changed. No GREEN implementation claim is made; the proposed repair still requires execution approval and verification.

## Stop conditions

Stop and report if: the baseline changed materially; a finite exact-reference test disagrees with the proposed mathematics; an unapproved numerical/API choice becomes necessary; a required check fails; the fix depends on a shared-file change not yet integrated; or consumer failures require a broader redesign. Correctness-first does not mean guessing an algorithm or hiding the next defect.
