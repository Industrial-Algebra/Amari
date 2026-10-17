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

---

# M-REP-C review-round-1 remediation (2026-10-16)

Base: `8bc3011` (M-REP-C closure) on PR #311
(`fix/core-mrep-c-specification-audit` → `develop`). Round-scoped fix of all
four P1 review findings plus the P3 path scrub and a proactive panic sweep.
Theme: **construction-time finiteness cannot protect operation results**; the
operations themselves now carry their own domain handling. Evidence for this
round: `r1/`.

## Findings fixed

| # | Finding | Remediation | Battery |
|---|---|---|---|
| F1 | `Vector::verified_self_dot` `result >= 0` contract violated by the `[MAX]+[MAX]=inf`, then `(+inf)+(-inf)=NaN` coefficient chain; doc claimed "finite ⇒ finite sum" (false: `[MAX]` self-dot is `+inf`) | `verified_self_dot` now returns `Result<T, &'static str>`, rejecting non-finite coefficients; contract `Ok(r) ==> r >= 0` (sound: finite `c ⇒ c² >= 0`, possibly `+inf`; a sum of nonnegatives is never `NaN`); doc corrected | `mrepc_self_dot_rejects_overflow_nan_chain` → `Err`; `mrepc_self_dot_single_max_overflows_to_positive_infinity` → `Ok(+inf)`, `+inf >= 0`; zero vector → `Ok(0)` |
| F2 | `VerifiedMultivector::scalar` unconditional `ensures(result.is_ok())` contradicts `Err`-on-`NaN` | Contract re-encoded success-conditionally (`Ok` ⇒ `is_scalar` ∧ `coefficients[0] == value` ∧ `len == BASIS_SIZE`; `Err` ⇒ `!value.is_finite()`) | existing `mrepc_scalar_rejects_nonfinite` / `mrepc_scalar_accepts_finite` |
| F3 | `verified_scalar_mult` satisfied every `requires` for `2.0 × scalar(MAX)` yet panicked on `new(...).unwrap()` when the product overflowed to `inf` | `verified_scalar_mult` returns `Result<Self, &'static str>`, routing the product through `VerifiedMultivector::new` (no unwrap); contract success-conditional; `requires` inputs/scalar finite; `debug_assert` mirrors; call sites migrated | `mrepc_contract_scalar_mult_overflow_rejected` → `Err`; `mrepc_contract_scalar_mult_finite_exact` → exact `6.0` |
| F4 | `VerifiedRotor::compose` divided by a zero norm in `Cl(1,1,0)` (`[s,0,0,s] ∘ [s,0,0,-s] == 0`), manufacturing `NaN` coefficients that `apply` then propagated | `compose` returns `Result<Self, &'static str>`; zero/non-finite denominator ⇒ `Err("composition is degenerate")`; normalized coefficients re-checked by `VerifiedMultivector::new`; exact-unit-norm `ensures` removed (dated note) | `mrepc_compose_degenerate_product_is_error` → `Err`; `mrepc_compose_ordinary_product_is_ok`; `mrepc_apply_on_valid_rotor_is_finite` (the apply-on-degenerate path is unreachable) |
| F5 | 6 host paths across 4 evidence files under `mrep-c/` | Scrubbed to `$AMARI` / `$HOME` placeholders (0 host paths remain) | a local-home-prefix grep of this directory returns empty |

**Proactive sweep (same class).** Runtime `unwrap`/`expect`/`panic` paths
reachable from finite constructed inputs: **6 → 0** (see
`r1/unwrap-sweep.txt`). The remaining `.unwrap()` occurrences are
specification-annotation text (dropped by the dummy Creusot proc-macro under
the pinned recipe) and test-only (`(A)`/`(T)` classes). The `geometric_product`
`T::from(sign).unwrap()` was replaced by a total `match`; the `VerifiedRotor`
tolerance now reads through the private `rotor_tolerance::<T>()` helper. As
"siblings with the same shape", `verified_addition`, `verify_scalar_identity`,
`verify_zero_addition`, and `verify_basis_vector_square` also became fallible
so their `Ok` coefficient equalities cannot be falsified by `NaN` and no
constant-`scalar`/`basis_vector` unwrap remains.

## Test-count deltas (disclosed)

| Gate | Before R1 | After R1 | Delta |
|---|---|---|---|
| `cargo test -p amari-core` (default) | 510 / 18 suites | **515 / 18 suites** | **+5** |
| `cargo test -p amari-core --features formal-verification` | 518 / 18 suites | **525 / 18 suites** | **+7** |

The +5 default tests are the F1 battery (2) + the F4 compose/apply battery (3).
The formal run adds the 2 new `contract_battery` overflow tests
(`verified_scalar_mult` + `verified_addition`). Named-suite inventories are
**unchanged**: `geometric_axioms` **6**, `verified_contracts` (formal) **4**.

## Verification — pinned recipe (`r1/`)

Same pinned recipe as the closure round (`r1/recipe-env.txt`): stable
`RUSTC`/`RUSTDOC`, empty wrappers, `RUSTFLAGS='-C target-cpu=x86-64 -C
target-feature=-avx2'`, `CARGO_TARGET_DIR=$PWD/target/mrepc`, `--locked`,
`cargo +stable` (named-suite helper under `RUSTUP_TOOLCHAIN=stable`).

| Gate | Result | Log |
|---|---|---|
| `cargo +stable test -p amari-core --locked` | **515 passed / 18 suites**, exit 0, 0 failed, 0 ignored | `r1/amari-core-default-test.log` |
| `cargo +stable test -p amari-core --features formal-verification --locked` | **525 passed / 18 suites**, exit 0, 0 failed, 0 ignored | `r1/amari-core-formal-test.log` |
| `cargo +stable clippy -p amari-core --all-targets --locked -- -D warnings` | exit 0, 0 warnings | `r1/amari-core-clippy-default.log` |
| `cargo +stable clippy -p amari-core --all-targets --features formal-verification --locked -- -D warnings` | exit 0, 0 warnings | `r1/amari-core-clippy-formal.log` |
| `cargo +stable fmt --all --check` | exit 0 | `r1/fmt-check.log` |
| `RUSTDOCFLAGS='-D warnings' cargo +stable doc -p amari-core --no-deps --locked` | exit 0 | `r1/amari-core-rustdoc.log` |
| `cargo +stable check --workspace --locked` | exit 0 | `r1/workspace-check.log` |
| `geometric_axioms` named-suite assertion | OK — 6 passed, 0 ignored, exact inventory | `r1/named-suite-assertions.log` |
| `verified_contracts` (formal) named-suite assertion | OK — 4 passed, 0 ignored, exact inventory | `r1/named-suite-assertions.log` |
| operation-result unwrap/panic sweep | runtime 6 → 0; F-class list; method | `r1/unwrap-sweep.txt` |

## R1 ambiguities resolved

1. **"Sibling with the same shape" breadth.** F3 names `verified_scalar_mult`
   explicitly; the probe sweep named `add/sub/neg/geometric_product/compose/
   apply/inverse`. `verified_addition`, `verify_scalar_identity`,
   `verify_zero_addition`, and `verify_basis_vector_square` were treated as
   siblings — each can take finite inputs to a non-finite (or, for the last,
   out-of-range) result — and made fallible so no unchecked `.unwrap()` and no
   `NaN`-falsifiable `Ok` equality remains. `VerifiedMultivector::add` /
   `geometric_product` / `apply_to_multivector` stay infallible arithmetic
   (panic-free); their checked wrappers own the finiteness boundary.
2. **`compose` error wording.** Zero and non-finite denominators both map to
   the dispatch's exact `"composition is degenerate"`; a non-finite
   *normalized* coefficient is rejected by `VerifiedMultivector::new`'s own
   message (also an `Err`).
3. **`verified_self_dot` precondition.** The stale `requires(!data.is_empty())`
   was replaced by `requires(all_finite(&self.data))` (the encoded domain the
   dispatch asked for); an empty `Vector<T,0>` would today return `Ok(0)`.
4. **Exact-unit-norm contract.** The unsound `ensures(norm == 1)` was removed
   with a dated note rather than "repaired"; the actual postcondition (finite
   coefficients) is now the `Ok` ensures.

## R1 residuals

- No proof tool invoked; annotations remain expressed, not discharged (C3/C4).
- Infallible `add` / `geometric_product` / `apply` still may overflow finite
  inputs to `±inf`; callers needing the finite-coefficient invariant use the
  checked wrappers or `VerifiedMultivector::new`.
- Workspace `--all-features` Clippy remains blocked by the pre-existing
  `z3-sys` cmake failure disclosed by M-REP-B.
