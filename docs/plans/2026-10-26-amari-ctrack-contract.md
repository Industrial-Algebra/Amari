# Amari C-Track Contract — C2 Static Rejection + C3 Toolchain Conformance

**Status:** DRAFT for operator decisions → contract review loop → approval
**Branch:** `chore/ctrack-contract` from develop `3d0593d` (post-M-TGT)
**Bounded scope:** C2 (type-boundary rejection) in full + C3 (Creusot
toolchain conformance) bootstrap/gate + **a C4 translation SMOKE**
(operator decision D4, amended from the draft's deferral): one
`cargo creusot` translation run over amari-core's formal feature,
proving nothing, establishing only that translation of the real
surface is nonempty and reproducible. Full C4 (discharged
obligations, replayable proofs, spec audit) remains its own follow-up
contract — the matrix §7 BLOCKED status is not lifted by a smoke.

## 1. Context

M-TGT is closed (P-EMBED #342, P-AVX2 #343, P-WASM #349 — all merged).
Per matrix §7/D-B, the remaining pre-1.0 gates are C2 (static rejection),
C3 (toolchain conformance), C4 (proofs). D-B's immediate contract repair
(the unsound `ensures(result >= T::zero())` removal) landed with M-REP-A.

## 2. Census (develop `3d0593d`, 2026-10-26)

**C2 surface — static.** `verified.rs` carries three phantom markers:
`Signature<P,Q,R>` (line 61), `Grade<K>` (64), `Dim<D>` (67).
`VerifiedMultivector<T,P,Q,R>` and `KVector<T,K,P,Q,R>` are branded by
them (PhantomData fields, lines 85/310); `VerifiedRotor<T,P,Q,R>` (505).
Cross-signature/grade/dim operations are therefore TYPE ERRORS at
compile time (R1 scratch-verified: all six S1 families fail with
E0308/E0277; positive control compiles). **Zero compile-fail tests
exist for the verified-core boundaries** — no harness, no negative
cases anywhere in amari-core. NOT zero workspace-wide (R1 census
correction): `amari-rewrite-macros` carries its own trybuild UI suite
(`tests/ui.rs`, 22 `compile_fail` cases — a different boundary: macro
diagnostics) and its dev-deps include `trybuild = "1"` (so D1's
"zero new dependencies" is precise only at the amari-core manifest
level; the crate already exists elsewhere in the workspace — decision
unchanged, context corrected).

**C2 surface — dynamic.** Runtime rejection exists and is partially
tested: `VerifiedMultivector::new` (length, finiteness),
`VerifiedRotor::new` (finite, even-grade, unit-norm `|n-1| <= 1e-4`
documented as the accepted set), `compose`/`apply_to_multivector`
(Result paths). Error-path coverage lives in
`inverse_exp_contracts.rs` and `rotor_error_privacy_s1.rs` (grep
`expect_err|is_err`), PLUS constructor-rejection unit tests inside
`verified.rs` itself (R1 census correction — the draft said only the
two integration files); no dedicated ingress-rejection suite.

**C3 surface.** Nothing installed: no `cargo-creusot` binary, no
`nightly-2025-11-13` toolchain. `verified_contracts.rs` is 250 lines,
**23 anchored `#[requires]`/`#[ensures]` obligations** (R1 count
correction; the draft's 25 was an unanchored grep including doc-text
mentions), compiled today only through the
dummy path (contracts/proc-macros 0.8.0 outside `cfg(creusot)` — a
green build proves NOTHING about translation, per matrix C1 note).

## 3. Work items

### C2 — static rejection made real

- **S1** Compile-fail harness per D1 (rustc-direct — operator
  decision). Negative cases ONLY for guarantees the design actually
  promises (scope per D2, operator-amended to ADD Vector-dim and
  cross-type mixing):
  1. cross-signature multiply/add (`VerifiedMultivector<f64,2,0,0>` ×
     `VerifiedMultivector<f64,0,2,0>`) — must be a type error;
  2. cross-signature rotor compose (`VerifiedRotor<f64,3,0,0>.compose`
     with `VerifiedRotor<f64,0,3,0>`);
  3. grade mismatch through `KVector<T,K,...>` branding;
  4. apply-to-multivector across signatures;
  5. cross-dimension `Vector` ops (`Vector<f64,2>.dot/.add` with
     `Vector<f64,3>`) — D2 amendment;
  6. cross-type mixing — a `Vector` passed where a
     `VerifiedMultivector` is expected, and KVector/VerifiedMultivector
     non-conversion paths (no `From`/`Into` bypass) — D2 amendment.
- **S2** Negative controls: each case must fail with an error pointing
  at the intended boundary (signature/grade/dim mismatch), not an
  unrelated compile error — the harness records the actual message and
  asserts the boundary name appears (exact-token, fail-closed grep
  discipline).
- **S2b Remedy direction (operator standing preference, 2026-10-26):
  richer substrate.** If any S1 case does NOT fail to compile today
  (e.g. cross-type mixing compiles via inference, or a From/Into path
  silently converts), the remedy is to ADD the static boundary —
  stronger typing, sealed traits, removing accidental conversion
  impls — not to narrow the promise. An Amari with more options and
  paths means MORE static guarantees where the type system can carry
  them, never fewer.
- **S3** Dynamic rejection suite, recorded SEPARATELY (matrix C2):
  dedicated `tests/ingress_rejection.rs` covering every documented
  `Err` string of the three ingress constructors plus compose/apply
  failure modes, including the unit-norm tolerance boundary
  (`1e-4 ± ε` acceptance band edges).
- **S4** Evidence: per-case messages + counts under the slim doctrine
  (TSV + samples + runner; TOTAL override discipline).

### C3 — toolchain conformance

- **S5** Bootstrap per D3: `rustup toolchain install
  nightly-2025-11-13` (pinned), upstream Creusot v0.8.0 via the
  official tag-specific procedure (release artifact install, NOT
  floating `cargo install --git`). Capture the full version tuple:
  release source hash, `cargo creusot version`, rustc commit, **the
  resolved prelude identity and configuration** (matrix §7/C3 names it
  explicitly; R1 P3 — the draft's tuple omitted it), Why3, why3find,
  solver versions and configuration — recorded as OBSERVED at
  bootstrap, never invented (matrix C3).
- **S6** Conformance gate, both directions:
  - **positive:** a nontrivial supported proof fixture (a small
    verified function with a real postcondition) that TRANSLATES and
    PROVES under the installed tuple — obligation outcome inspected,
    not process exit (parse the proof-report artifact);
  - **negative:** a deliberately FALSE postcondition that remains
    UNPROVED / fails the gate. The gate must distinguish "proved",
    "unproved", "timeout", "error" — a green exit on the false fixture
    is a gate failure.
- **S7** Evidence: bootstrap provenance file + both fixtures' outcome
  artifacts under the slim doctrine.

## 4. Operator decisions (ask_user before implementation)

All four decisions resolved by the operator 2026-10-26 (ask_user,
recorded verbatim in the session log):

- **D1 — rustc-direct harness** (as proposed: pinned-toolchain shell
  runner, exit status + message-token assertions; zero new
  dependencies — standing doctrine since the candle incident).
- **D2 — AMENDED (operator): Add Vector-dim + cross-type mixing.**
  Static promises now ALSO cover cross-dimension Vector ops and
  branded-type mixing / non-conversion paths; the KVector grade
  promise STAYS (grade-content of VerifiedMultivector remains
  runtime-only). Runtime scope unchanged (length, finiteness, norm,
  parity).
- **D3 — Install now** (as proposed: nightly-2025-11-13 + Creusot
  v0.8.0 official tag-specific procedure on this machine; tuple
  recorded as observed).
- **D4 — AMENDED (operator): Include C4 smoke.** Translation smoke
  folded into this contract (S8 below); full C4 stays a follow-up
  contract.

### C4 smoke (D4 amendment — translation only, NO proving claims)

- **S8** After C3's conformance gate passes: the translation smoke
  runs TWICE — the initial translation, then a **clean replay** (fresh
  target/output directory, same pinned tuple and lock) with a DEFINED
  comparison: the obligation inventory (per-item identifiers or
  counts + content hashes of generated artifacts) must be equal
  across both runs. One run demonstrates nonemptiness only (R1
  correction); reproducibility is an evidenced result, not a word.
  Over `--no-default-features --features std,formal-verification`
  (matrix §7's proposed command form, to be validated against actual
  v0.8.0 CLI semantics during S5). Deliverables: nonempty obligation
  set tied to source/lock/tool hashes + the two-run comparison;
  translation artifacts archived slim (hashes + counts + sample, not
  bulk). **Claims wording: "translation is nonempty and reproduces
  bit-comparably across a clean replay" — nothing is proved,
  discharged, or verified by this step.** Full C4 (obligation
  inventory, trusted assumptions, numeric model, discharged proofs,
  replay) is the follow-up contract.

## 5. Verification matrix (implementation PR)

| Gate | Command form | Expected |
|---|---|---|
| C2 static negatives | S1 harness, pinned stable | all fail with boundary tokens |
| C2 negative controls | S2 message assertions | exact-token matches, fail-closed |
| C2 dynamic suite | `cargo test -p amari-core --test ingress_rejection` | all green |
| Standing gates | default suite, clippy, fmt, doctests, catalog drift | unchanged-green, no drift |
| C3 positive fixture | S6 under installed tuple | proved, outcome-parsed |
| C3 negative fixture | S6 false postcondition | unproved, gate fails it |
| C4 smoke | S8 translation | nonempty, reproducible; NO proof claims |
| H spot rows | reverify.sh rows 1,160,320 × stable | green (no regression) |

## 6. Exclusions

- **C4 full proofs** — separate follow-up contract (S8 smoke only in
  scope here; the §7 BLOCKED status on discharged obligations stands).
- No new feature flags; no public API changes; no contract-text edits
  beyond what C2/C3 evidence requires.
- ARM/board runtime (standing residual); approximation accuracy
  certification (standing residual).

## 7. Evidence plan

`docs/plans/core-matrix-evidence/ctrack-c2/` and `ctrack-c3/`:
runners with TOTAL override + fail-closed inventory checks (P-WASM
R2/R3 lessons), complete TSVs, samples, manifests, bootstrap
provenance. Slim from the start — no bulk logs at any stage.
