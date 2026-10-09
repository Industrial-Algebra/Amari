# CORE-W01 — Finite-Coefficient Arithmetic Contract (refreshed against post-W00 baseline)

> **REQUIRED SUB-SKILL:** executing-plans, task-by-task, after explicit approval of this contract.

**Goal:** Close CORE-02: remove the implicit `1e-14` approximate pruning from fundamental `Multivector` arithmetic and structural grade selection — exact-zero skips only — preserving metric/sign rules and accumulation order. This is the first mathematical remediation of the audited programme.

**Specification:** the merged, audited plan [`2026-09-19-amari-core-arithmetic-remediation-plan.md`](2026-09-19-amari-core-arithmetic-remediation-plan.md) (Tasks 1–6, including the verbatim six-test regression fixture and the private `is_exact_zero()` helper) remains the controlling spec. This contract binds it to the current baseline and process; where they conflict on process detail, this contract governs, on mathematics the plan governs.

## Baseline (verified 2026-10-09, this worktree)

- Branch `fix/core-w01-finite-arithmetic` from `origin/develop` = `bfe5c08` (PR #280 / W00 merged). **W00 prerequisite satisfied:** baseline gates green (PR CI 16 passed; warning-denied Clippy 0 on stable and nightly).
- `amari-core/src/lib.rs` untouched by W00; all plan anchor sites verified current: geometric-product skips **300/305**, grade predicate **274**, projected-arithmetic `is_zero()` **326, 347, 586, 591, 618, 623**, Hodge skip **660**.
- Oracle source present at `docs/audits/core-foundations-repro/tests/foundations.rs` (landed via PR #276) — Task 1 promotes from it directly; no fixture checkout needed.
- Out-of-scope `1e-14` sites confirmed present and **left alone**: `456` (public `is_zero()`), `505/517/539/542/568` (inverse/normalization/exponential — W02+ decisions).

## Scope

**In:** exactly the plan's file set — `amari-core/src/lib.rs` (the enumerated predicate changes + private helper + doc updates), new `amari-core/tests/basis_word_oracle.rs` (oracle only) and `amari-core/tests/arithmetic_regressions.rs` (the plan's fixture verbatim), `CHANGELOG.md` unreleased section. Consumer checks per plan Task 5 (read/type-resolve; separate migration PR if scope exceeds).

**Out (stop-and-report, not decide):** public `is_zero()`/`Zero` semantics, inverse/exp/normalization thresholds, Hodge metric convention (CORE-08), rotor/typed wrappers, generic/verified/GF(2) paths, SIMD/layout, any new public API or epsilon parameter. No repository-wide `1e-14` sweep.

**Behavior policy:** result changes from previously pruned coefficients are expected corrections. Consumers relying on silent truncation get investigated and documented, not silently preserved.

## Execution process

1. **Red first:** establish the worktree lock (`cargo generate-lockfile` if absent; the develop lockfile is untracked), archive its hash, `--locked` thereafter. Promote the oracle (Task 1); add `arithmetic_regressions.rs` verbatim (Task 2 file); run and archive all six failures + oracle pass (evidence under `docs/plans/core-matrix-evidence/w01/red/`).
2. **Staged fixes per plan Tasks 2→4**, each stage dispatched to the **deepseek** generation agent (verbatim plan task + this contract; no cargo/git authority), each verified by the orchestrator before the next: gp skips → `is_exact_zero()` in the four projected methods → Hodge/grade predicates. Diff discipline: predicate swaps only; tables, signs, loop order, accumulation untouched.
3. **Green gates** (validated D07 `baseline_cargo` recipe, fresh `target/d07-baseline/w01/<T>`, `--locked`): default tests both compilers (287 baseline + new oracle/regression tests; zero failures); **release-mode** `arithmetic_regressions` + `basis_word_oracle` per plan Task 6; warning-denied Clippy stable+nightly; no-default lib check + std-only tests + minimal warning-denied configs (W00 parity); `rustfmt --check`; `doc -D warnings`.
4. **Task 5 consumer pass + docs/changelog**, then **reviewer loop** (`openai-codex/gpt-6-astra`, report-only, max 6 rounds, convergence = 0 P1/P2), then commit + PR to `develop` (no self-merge).
5. CORE-02 closes only after the reviewed PR merges and integrated-baseline evidence is recorded; the five other fixture-verified findings and CORE-01…15 remain open.

## Authorization boundary

This draft is **not** implementation authority. Approving it authorizes the red/green execution, dispatches, review and PR described above — nothing in W02+, no D07 full matrix, no release actions.
