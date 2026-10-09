# CORE-W00 — Red/Blue Evidence Summary

> **In this worktree.** An earlier draft of this file was mistakenly written into the sibling core-matrix worktree (orchestrator relative-path error) and was therefore absent when review round 1 ran; it has been moved here and corrected per that review.

## Round-1 review corrections applied

1. Summary/evidence relocated into this worktree; `green/provenance.md` now records toolchains, the verbatim validated wrapper, exact per-gate commands and exits (`green/status.tsv`), lock hash continuity, and formatting status.
2. All green gates were re-executed with `-vv` after the formatting fix so logs embed the actual invocation chains (pinned toolchain executables, explicit host target); the first green logs are preserved under `green/initial/`.
3. `rustfmt` applied to `verified.rs` and `rotor.rs` (whitespace-only reflow at the edited sites); `rustfmt --check` exits 0 on all five whitelisted files.

Worktree `fix/core-w00-baseline-gates` from `origin/develop` (`5360d79`; amari-core byte-identical to audit baseline `ba1b771`). Branch lock `ba6b36cf91a1b906e098598eac9ab9fbf6e7e945a96a3295571416826d96ca46` (own resolution; the audited lock correctly refused `--locked` against the newer workspace). All commands via the validated D07 baseline recipe (explicit-empty wrappers, pinned compilers, host target, baseline ISA flags, fresh `target/d07-baseline/w00/<T>`); zero sccache occurrences in every red and green log.

## Red gates (pre-fix, archived in `red/`)

| Gate | Command scope | Result |
|---|---|---|
| R1 | nightly default Clippy `-D warnings --all-targets` | 101 — exactly the 3 known `needless_range_loop` errors (159:22, 339:22, 387:18) |
| R2 | stable `check --no-default-features --lib` | 101 — `vec!`/`Vec` not found (generic.rs 76/79, rotor.rs 215) |
| R3 | stable `test --no-default-features --features std` | 101 — unresolved import `amari_core::gf2` (gf2_tests.rs:3) |
| R4a | stable std-only warning-denied Clippy | 101 — unused imports `Formatter`/`Result` (precision.rs:12:32) |

## Green gates (post-fix, archived in `green/`)

| Gate | Scope | Result |
|---|---|---|
| G1 | default Clippy `-D warnings --all-targets`, stable + nightly | **0 / 0, zero diagnostics** |
| G2 | stable `check --no-default-features --lib` | 0 |
| G3 | stable `test --no-default-features --features std` | 0; gf2 suite correctly empty (0 tests), other suites pass |
| G4 | stable `test --no-default-features --features std,gf2` | 0; gf2 suite runs 6/6 — identical to its default-mode count |
| G5 | warning-denied Clippy: no-default lib, std-only all-targets, no-default+high-precision lib | 0 / 0 / 0 |
| G6 | default `test` both compilers | **287 each** (284 + 3 doctests); name inventories identical stable==nightly==pre-fix revalidation baseline |
| G7 | default `check --all-targets` + `doc --no-deps` (`-D warnings`) both compilers | 0 everywhere |

## Implementation diff (five whitelisted files, nothing else)

- `verified.rs`: exactly the three flagged loop headers → `iter().enumerate()`; the three indexed reads replaced by the iterated values; all index math, order, and arithmetic unchanged.
- `generic.rs`: `use alloc::vec;` + `use alloc::vec::Vec;`. Deviation note: the contract's preferred `use alloc::vec::{self, Vec};` does not bring the `vec!` macro into scope (implementer verified with standalone rustc; the used form matches lib.rs).
- `rotor.rs`: `use alloc::vec::Vec;`.
- `gf2_tests.rs`: `#![cfg(feature = "gf2")]` after the module doc comment.
- `precision.rs`: blanket `#[allow(unused_imports)]` and the unused `num_traits` import removed (only fully-qualified paths remain in use); `Formatter`/`Result` imports gated to `all(std/not-std, any(native, high, wasm)-precision)` matching the only `Display` impls that use them; `Debug`/`Display`/`consts` unconditional as before. No new allows.

HEAD `5360d79` (no commits yet), lock unchanged through all gates. Limits: default-core host scope only; not full H/portability/proofs; CORE-01–15 untouched.
