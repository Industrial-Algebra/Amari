# M-REP-C evidence — specification audit (2026-10-15)

Approved matrix: `docs/plans/2026-10-09-amari-core-verification-matrix.md`
(§8.1 D-B; §10 M-REP split; MATRIX-02 / MATRIX-03). Scope: the verified/phantom
module only — `amari-core/src/verified.rs`,
`amari-core/src/verified_contracts.rs`, `amari-core/src/verified_laws.rs` (now
removed). **No main-API changes** (`rotor.rs`, `exact_square.rs`, `lib.rs` core
types untouched). Base: `develop` `b3f2e0b`; worktree `core-mrep-c`; lock
`--locked` (no dependency changes); target `target/mrepc`. The dispatch's
`generate-lockfile` instruction was satisfied by the pre-existing, unchanged
lock: every gate ran `--locked` and resolved with no lock modification.

Operator decisions implemented (1–5):

| # | Decision | Result |
|---|---|---|
| 1 | Finiteness enforced at construction | `VerifiedMultivector::new`/`scalar`, `Vector::new`, and `VerifiedRotor::new` reject `NaN`/±infinity with a clear `Err`; `basis_vector` is finite by construction and routed through `new`; `KVector::from_multivector` consumes an already-validated multivector. |
| 2 | Finiteness preconditions on the equality contracts | `verified_scalar_mult`, `verified_addition`, `verify_scalar_identity`, `verify_zero_addition` carry `#[requires(...is_finite())]` and runtime `debug_assert!` preconditions. |
| 3 | Remove `verified_laws.rs` | `git rm` with full disclosure; disposed as **REMOVED-STUB**. |
| 4 | Add `verified_self_dot` (MATRIX-02 successor) | Added as a method on `Vector<T, D>` with `#[requires non-empty]`, `#[ensures(result >= T::zero())]`; rustdoc states the finite-input soundness scope. |
| 5 | Keep the `1e-4` rotor tolerance, documented | `VerifiedRotor` rustdoc now documents `1e-4` as the finite/even-grade/approximately-unit type-invariant tolerance. |

## Files changed

| Artifact | Change |
|---|---|
| `amari-core/src/verified.rs` | Module-level `all_finite` predicate; finiteness at all coefficient-taking constructors; `VerifiedMultivector::scalar` made fallible; `VerifiedRotor` finiteness check + type-invariant tolerance docs + dated NaN correction; `Vector::new` finiteness; `Vector::verified_self_dot`; M-REP-C batteries. |
| `amari-core/src/verified_contracts.rs` | `coefficients_are_finite` predicate; finiteness `requires` + `debug_assert!` on the four equality contracts; fallible-`scalar` call sites; `verified_norm` doc cross-reference. |
| `amari-core/src/verified_laws.rs` | **Deleted** (`git rm`), evidence `../verified-laws-removal.txt`. |
| `docs/technical/MATHEMATICAL_RIGOR.md` | Non-finite caveat rewritten to the construction-time policy + dated correction. |
| `docs/technical/TEST_ENFORCEMENT.md` | M-REP-C status note with the test-count deltas. |
| `docs/plans/2026-10-09-amari-core-verification-matrix.md` | MATRIX-02/MATRIX-03 closed; §9 refresh block. |
| `docs/audits/README.md` | `verified_laws.rs` row marked REMOVED. |
| `docs/1.0-audit.md` | Historical `verified_laws.rs` mentions annotated (history preserved). |
| `docs/plans/2026-09-19-amari-core-remediation-decisions.md` | Prerequisite line annotated (history preserved). |

## Finiteness batteries

**Construction battery** (`verified::tests`, default features):
`mrepc_multivector_new_rejects_nan`,
`..._rejects_positive_infinity`, `..._rejects_negative_infinity`,
`..._rejects_mixed_nonfinite`, `..._accepts_finite`,
`mrepc_scalar_rejects_nonfinite`, `mrepc_scalar_accepts_finite`,
`mrepc_basis_vector_accepts_finite`, `mrepc_vector_new_rejects_nonfinite`,
`mrepc_vector_new_accepts_finite`,
`mrepc_rotor_new_rejects_nonfinite_coefficients` (the last constructs a
`NaN`-scalar `VerifiedMultivector` by struct literal to bypass `new`, proving
the `VerifiedRotor` defense-in-depth check catches the former M-REP-A gap).

**`verified_self_dot` battery** (`verified::tests`):
`mrepc_self_dot_known_values`, `mrepc_self_dot_zero_vector_is_zero`,
`mrepc_self_dot_single_coefficient`, `mrepc_self_dot_is_nonnegative`.

**Contract-precondition battery** (`verified::tests::contract_battery`,
`--features formal-verification`):
`mrepc_contract_scalar_mult_finite_exact`,
`mrepc_contract_addition_finite_exact`,
`mrepc_contract_scalar_identity_finite_exact`, and
`mrepc_contract_scalar_mult_nonfinite_scalar_panics` (`#[should_panic]`,
proving the runtime `debug_assert!` guard for the raw scalar argument).

## `verified_laws.rs` removal evidence (MATRIX-03, REMOVED-STUB)

Full extraction: `../verified-laws-removal.txt`. Summary:

- **Never compiled.** No `mod verified_laws` declaration/inclusion exists in
  any Rust source; additionally the whole file was
  `#![cfg(feature = "formal-verification")]`.
- **Bodies were stubs.** `associativity_property` returned a hardcoded `true`
  with the comment "This would be verified in a real proof"; `signature_law`,
  `anticommutativity_law`, `idempotence_law`, `orthogonality_law`,
  `norm_preservation`, `composition_law`, `quaternion_squares`,
  `hamilton_identity` were trait declarations only; the `impl` methods
  (`scalar_commutativity`, `multiplicative_identity`, `additive_identity`,
  `grade_involution_law`, `reversion_antiautomorphism`) had empty bodies.
- **Operator direction recorded.** This surface is a **candidate for
  replacement by Karpal proof/verify in a future pre-1.0
  certification/verification pass** (not part of this package).
- **Disposition.** M03 closes as **REMOVED-STUB**. Historical audit records
  (`docs/1.0-audit.md`, `docs/audits/README.md`, the matrix refresh note) were
  annotated rather than erased.

## Test-count deltas (disclosed)

| Gate | Before | After | Delta |
|---|---|---|---|
| `cargo test -p amari-core` (default) | 495 / 18 suites | **510 / 18 suites** | **+15** |
| `cargo test -p amari-core --features formal-verification` | 499 / 18 suites | **518 / 18 suites** | **+19** |

The +15 default tests are the 11 construction tests + 4 `verified_self_dot`
tests. The formal run adds the 4 `contract_tests` (pre-existing) plus the 4
new `contract_battery` tests, i.e. +19 over the M-REP-B formal figure. Both new
suites are real instance tests; the `#[should_panic]` test asserts a runtime
guard, not a placeholder.

Named-suite inventories are **unchanged**: `geometric_axioms` **6**,
`verified_contracts` (amari-core, formal) **4** — the batteries deliberately
live in `verified::tests` so they do not alter either approved inventory.

## Verification — pinned recipe

`RUSTC`/`RUSTDOC` = stable via `rustup which`, `RUSTC_WRAPPER=''`,
`RUSTC_WORKSPACE_WRAPPER=''`,
`RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2'`,
`CARGO_TARGET_DIR=$PWD/target/mrepc`, `--locked`, `cargo +stable`; the
named-suite helper was run with `RUSTUP_TOOLCHAIN=stable` (it invokes plain
`cargo`). Full env: `recipe-env.txt`.

| Gate | Result | Log |
|---|---|---|
| `cargo +stable test -p amari-core --locked` | **510 passed / 18 suites**, exit 0, 0 failed, 0 ignored | `amari-core-default-test.log` |
| `cargo +stable test -p amari-core --features formal-verification --locked` | **518 passed / 18 suites**, exit 0, 0 failed, 0 ignored | `amari-core-formal-test.log` |
| `cargo +stable clippy -p amari-core --all-targets --locked -- -D warnings` | exit 0, 0 warnings | `amari-core-clippy-default.log` |
| `cargo +stable clippy -p amari-core --all-targets --features formal-verification --locked -- -D warnings` | exit 0, 0 warnings (contract module compiles under dummy Creusot) | `amari-core-clippy-formal.log` |
| `cargo +stable fmt --all --check` | exit 0 | `fmt-check.log` |
| `RUSTDOCFLAGS='-D warnings' cargo +stable doc -p amari-core --no-deps --locked` | exit 0 | `amari-core-rustdoc.log` |
| `cargo +stable check --workspace --locked` | exit 0 | `workspace-check.log` |
| `geometric_axioms` named-suite assertion | OK — 6 passed, 0 ignored, exact inventory | `named-suite-assertions.log` |
| `verified_contracts` (formal) named-suite assertion | OK — 4 passed, 0 ignored, exact inventory | `named-suite-assertions.log` |
| `verified_laws.rs` removal evidence | no module declaration; whole-file gate; stub bodies | `../verified-laws-removal.txt` |

## Ambiguities resolved

1. **Location of `verified_self_dot`.** The dispatch gives method syntax
   `pub fn verified_self_dot(&self) -> T` and ties it to MATRIX-02, whose
   unsound contract was on `Vector::dot` in `verified.rs`; the `#[requires
   non-empty]` precondition is meaningful for `Vector<T, D>` (a `Vector<T, 0>`
   can be empty) but vacuous for `VerifiedMultivector` (its `coefficients`
   length is always `2^(P+Q+R) >= 1`). It was therefore added to
   `Vector<T, D>` in `verified.rs` as the direct self-dot successor, with the
   removed-dot distinction documented. If the reviewer intended
   `VerifiedMultivector::verified_self_dot`, the move is mechanical.
2. **`VerifiedMultivector::scalar` became fallible.** To "reject with a clear
   `Err`" a coefficient-taking constructor must return `Result`; `scalar` now
   returns `Result<Self, &'static str>`. Blast radius is zero external
   consumers (only `amari-tropical` imports `{Dim, Signature}`, marker types
   only); all in-crate call sites were updated. Finite behavior is unchanged
   apart from `.unwrap()` at call sites.
3. **`from_vector`/`from_bivector`.** The verified module has no such
   constructors (they exist only on the main `Multivector`, which is
   out of scope). The constructor audit covers every constructor that actually
   exists: `VerifiedMultivector::{new, scalar, basis_vector}`,
   `KVector::from_multivector`, `VerifiedRotor::new`, `Vector::new`.
4. **`KVector::from_multivector` and `Vector::add`/`dot`/`geometric_product`
   can still overflow finite inputs to `inf`.** The dispatch scopes finiteness
   to construction and explicitly scopes `verified_self_dot` to finite
   inputs; no operation-level guard was added. This is stated in the rustdoc
   and in `MATHEMATICAL_RIGOR.md`.
5. **`verified_norm` nonnegativity was not re-added.** The dispatch names
   `verified_self_dot` as the sound successor, so `verified_norm` keeps only
   its non-emptiness precondition; its doc now records why (overflow can carry
   `inf`).
6. **`debug_assert!` vs `assert!`.** The dispatch says "runtime debug_assert
   the preconditions"; `debug_assert!` was used, so the contract battery's
   panic test is meaningful in debug/test builds (the gated profile).

## Residuals

- **No proof tool was invoked.** These are annotation/test-compatibility and
  runtime-rejection changes; C3/C4 remain open per the matrix.
- **Workspace `--all-features` Clippy remains BLOCKED in this environment**
  (pre-existing `z3-sys` cmake failure, disclosed by M-REP-B); the dispatch's
  scoped Clippy gates pass.
- **`verified_self_dot` soundness is finite-input-scoped.** Operations can
  overflow; a future arithmetic-model pass (C4) owns universal claims.
- **The phantom layer is not certified.** The operator's future direction —
  becoming a *certified* layer via a pre-1.0 certification/verification pass
  (candidate: Karpal proof/verify) — is recorded here and in the removal note,
  but is explicitly **not** part of M-REP-C.
