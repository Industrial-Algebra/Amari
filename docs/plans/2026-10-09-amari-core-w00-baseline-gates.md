# CORE-W00 — Baseline Gates Repair Contract

> **REQUIRED SUB-SKILL:** executing-plans, task-by-task, after explicit approval of this contract.

**Goal:** Make warning-denied Clippy, no-default compilation, and feature-off test suites green on the audited default matrix row, without changing any API, algorithm, accumulation order, or numerical result.

**Architecture:** Four surgical fixes at already-audited sites. Loop desugaring that preserves iteration order and arithmetic exactly; explicit `alloc` imports; one integration-test feature gate; precise import gating replacing a blanket allow. Every fix is driven red-first by a reproduced failing gate.

**Tech Stack:** Rust (edition 2021, workspace amari-core), validated D07 baseline recipe (reviewed round-3), stable 1.98.0 + nightly 1.100.0-nightly installed toolchains.

---

## Preconditions (verified 2026-10-09)

- `origin/develop` = `5360d79`; **amari-core is byte-identical** to audit baseline `ba1b771` (`git diff --stat ba1b771..origin/develop -- amari-core` is empty). All cited line numbers are valid on both.
- Default suite reference: 284 unit/integration + 3 doctests = 287, identical names on both compilers (revalidation-1 evidence).
- Baseline recipe validated: `docs/plans/2026-10-09-amari-core-verification-matrix.md` §3.2 (matrix SHA-256 `cb56119f…`), including explicit-empty wrapper overrides and a fresh `D07_BASELINE_TARGET_DIR`.
- Audit anchors: CORE-01–15 findings untouched by this contract; W00 scope is exactly the four fixes below.

## Scope

**Worktree/branch:** new worktree from fresh `origin/develop` (`5360d79`), branch `fix/core-w00-baseline-gates`, PR → `develop` (squash or merge commit permitted for fix PRs).

**File whitelist** — no other file may be modified:

1. `amari-core/src/verified.rs` (three loop sites)
2. `amari-core/src/generic.rs` (imports)
3. `amari-core/src/rotor.rs` (import)
4. `amari-core/tests/gf2_tests.rs` (module gate)
5. `amari-core/src/precision.rs` (import gating only)

Expand the list only for a defect actually reproduced in this worktree's red gates; any expansion stops the contract for re-approval.

**Forbidden:** API/signature/behavior changes; iteration-order, accumulation-order, or arithmetic-expression changes beyond substituting an iterated value for the identical indexed read; new `#[allow]`/`#[expect]` attributes or lint configuration; toolchain/workflow/dependency/lockfile changes; edits to other crates; merges.

## Fixes

### F1 — three nightly `needless_range_loop` errors (verified.rs)

Red: nightly warning-denied Clippy exits 101 with exactly these unique errors — `j` at **159** (`geometric_product`, indexes `other.coefficients[j]` at 164), `j` at **339** (wedge, indexes `other.multivector.coefficients[j]` at 356), `i` at **387** (wedge, indexes `self.multivector.coefficients[i]` at 405).

Fix pattern (identical semantics, order, and arithmetic):

```rust
for (j, &other_j) in other.coefficients.iter().enumerate() {
    // all existing bit/index math on j unchanged; read other_j where
    // other.coefficients[j] was read
```

Outer loops and every `continue`/swap-count computation stay untouched. Green: stable **and** nightly `clippy --all-targets -- -D warnings` exit 0 with zero diagnostics (not just these three gone).

### F2 — no-default imports (generic.rs, rotor.rs)

Red: `check --no-default-features --lib` fails — `vec!` macro not found `generic.rs:79`, `Vec` not found `generic.rs:76` and `rotor.rs:215` (lib.rs is `no_std` + `extern crate alloc`).

Fix: add `use alloc::vec::Vec;` (rotor.rs) and `use alloc::vec::{self, Vec};` or equivalent (generic.rs) — unconditional, valid in std builds too (`alloc::vec::Vec` is the same type). Green: no-default lib check exits 0 on stable; std build unchanged.

### F3 — GF(2) test gating (tests/gf2_tests.rs)

Red: `test --no-default-features --features std` fails compiling `gf2_tests.rs:3` (`use amari_core::gf2::*` — module is `#[cfg(feature = "gf2")]`, lib.rs:34–35).

Fix: add `#![cfg(feature = "gf2")]` as the first line after the module doc comment. Green: std-only suite compiles and runs (empty GF(2) suite recorded explicitly); `--features gf2` runs the full GF(2) suite; default suite counts unchanged.

### F4 — precision import gating (precision.rs)

Red: reproduce and record the audit's warning-denied minimal-configuration failure(s) involving precision imports (the blanket `#[allow(unused_imports)]` at line 8 currently masks them) before editing.

Fix: gate each import (`num_traits::{…}`, fmt, consts) by the features that actually use it; remove the blanket allow only if all configurations below verify clean. Green: warning-denied checks pass on default, `--no-default-features`, `--no-default-features --features std`, and `--no-default-features --features high-precision` (others unchanged: no-new-code rule).

## Verification matrix (all with validated recipe, fresh target dirs, `--locked`)

| Gate | Command scope | Required |
|---|---|---|
| G1 | nightly + stable default Clippy `-D warnings --all-targets` | 0 / 0, zero diagnostics |
| G2 | stable `check --no-default-features --lib` | 0 |
| G3 | stable `test --no-default-features --features std` | 0 (empty gf2 suite recorded) |
| G4 | stable `test --features gf2` gf2 suite | full pass |
| G5 | F4 minimal configs, warning-denied | 0 |
| G6 | default `test` both compilers | 287 each, names identical to revalidation-1 inventory |
| G7 | default `check --all-targets`, `doc --no-deps` (`-D warnings`) both compilers | 0 |

Run order: reproduce all applicable reds first and archive them; apply fixes; then G1→G7. Any unexplained deviation, new diagnostic, or behavioral failure stops the contract.

## Process and dispatch

1. Maintainer approves this contract (ask_user).
2. Create worktree/branch per scope; archive red-gate logs under `docs/plans/core-matrix-evidence/w00/` in that branch.
3. Generation dispatch: named `deepseek` agent (`deepseek-flash`), this contract verbatim as task, worktree cwd — per standing ia-moment generation-class routing.
4. Orchestrator independently verifies G1–G7 with the validated recipe (fresh `target/d07-baseline/w00/<T>`).
5. Review: named `reviewer` (`openai-codex / gpt-6-astra`), report-only, cumulative, max 6 rounds, convergence = zero P1/P2.
6. PR `fix/core-w00-baseline-gates` → `develop`; no self-merge; standard CI applies.

**Not authorized by this contract:** CORE-01–15 mathematical remediation (W01+), full H, portability/AVX2/proof rows, any public API decision, or release actions. W01's finite-coefficient arithmetic contract is drafted separately after W00 integrates.
