# CORE-W01 — Red/Blue Evidence Summary

Worktree `fix/core-w01-finite-arithmetic` from `origin/develop` = `bfe5c08` (post-W00). Branch lock `ba6b36cf91a1b906e098598eac9ab9fbf6e7e945a96a3295571416826d96ca46` (deterministic resolution matching W00's; archived `red/lock.sha256`; `--locked` thereafter; unchanged through every gate). All commands via the validated D07 baseline recipe (explicit-empty wrappers, pinned stable 1.98.0 / nightly 1.100.0-nightly, host target, baseline ISA flags); zero sccache in all logs.

## Red (archived in `red/`, pre-fix)

- `basis_word_oracle` promoted from `docs/audits/core-foundations-repro/tests/foundations.rs` (license header, `coefficients`/`blade`/`word_product`/`check_signature` + oracle test; one mechanical lint adaptation for stable Clippy: `% 2 == 0` → `.is_multiple_of(2)` at line 110 — semantics identical for these non-negative operands; the immutable audit fixture is untouched). **1 passed** (4,589 blade pairs, 35 signatures).
- `arithmetic_regressions.rs` = the plan's six-test fixture verbatim (rustfmt reflow only). **All six failed** as audited (`regressions-red.log`).

## Staged production changes (`amari-core/src/lib.rs` only; deepseek dispatches, orchestrator-verified between stages)

1. `geometric_product_scalar`: two `abs() < 1e-14` skips → `== 0.0` (gp tests 2/2 green, rest red — `stage1-*.log`).
2. Private `is_exact_zero()` helper + six predicate swaps in `inner_product`, `outer_product`, `left_contraction`, `right_contraction` (projected tests green, structural red — `stage2-*.log`).
3. `hodge_dual` skip → `== 0.0` (metric/permutation convention untouched) and `grade()` predicate → `is_exact_zero()` (all six green, oracle green — `stage3-*.log`).

Untouched by design: public `is_zero()` (~461), `num_traits::Zero` impl, and the W02+ `1e-14` sites (456/511/523/545/548/574). Doc comments updated on the corrected operations; CHANGELOG `[Unreleased]` entry added (CORE-02).

## Green (in `green/`, `status.tsv`; stable unless prefixed N=nightly)

| Gate | Result |
|---|---|
| default test, both compilers | **294 each** (287 baseline + 6 regressions + 1 oracle), 0 failed/ignored |
| release-mode `arithmetic_regressions` + `basis_word_oracle` | 0 / 0 |
| warning-denied Clippy: default both compilers; std-only; no-default lib (executed after review R1); no-default+high-precision lib | all 0, zero core diagnostics |
| no-default lib check; std-only tests (empty GF(2) suite as in W00) | 0 |
| all-targets check + `doc -D warnings`, both compilers | 0 |
| workspace type-check (`--workspace`, host) | 0 (compilation only — no behavioral claim) |
| original fixture `finding_02` filter (immutable snapshot lock, executed after review R1) | **2/2 passed** — both decimal-scale CORE-02 cases |
| full workspace default test run | 636 passed; 16 failed — all 16 are `amari-discovery` `probes::supervisor` tests, reproduced **identically on the pre-W01 baseline** in this environment (see `S-baseline-discovery-supervisor-comparison.log`; the same checks are green in GitHub CI, cf. PR #280) — pre-existing local-environment failures, excluded from W01 acceptance on that recorded basis |
| `rustfmt --check` on lib.rs + both test files | clean |

## Review-R1 corrections (all three P2s remediated)

1. Missing no-default Clippy gate executed and archived (`S-nodefault-lib-clippy`, exit 0).
2. Plan-Task-6 original decimal-scale reproduction executed and archived (`S-original-fixture-finding02`, 2/2 on the snapshot lock `19d17cb9…`, lock file removed after the run).
3. Consumer claim narrowed: workspace **type-check** success only; behavioral evidence = the full workspace test run above plus a bounded call-site inspection — tolerance patterns found near consumer call sites are the consumers' own explicit epsilons (e.g. `amari-measure` convergence checks), not reliance on core-internal pruning; a per-consumer behavioral audit remains CI integration scope. No in-repo consumer was found relying on silent pruning; absence of such reliance is inspection-supported, not proven exhaustive.

## Deviations from the plan/contract (disclosed)

1. Oracle promotion needed the fixture's `coefficients`/`blade` helpers (first extraction missed them; compile-caught, fixed) and the one Clippy lint adaptation above.
2. The plan's Task 6 command list was executed through the validated D07 recipe (per this contract's process section) rather than bare cargo; release-mode regressions/oracle included as required.
3. Monitor notes: this session's write-tool paths resolve against the core-matrix root; cross-worktree writes to core-w01 triggered disclosed plan-phase warnings before executing-plans was intentionally loaded (phase now `execute`). No guard was disabled or edited.

## Limits

Closes CORE-02 only upon reviewed merge + integrated-baseline recording. The fixture's other five findings and CORE-01…15 (inverse, exponential, approx-eq, Hodge convention, rotor invariants, …) remain open; public approximate-query policy (D01/W02) undecided; no full-matrix/portability/proof claims.
