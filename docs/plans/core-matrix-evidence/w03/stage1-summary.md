# CORE-W03 stage 1 — exact dyadic square (evidence summary)

Worktree `fix/core-w03-inverse-exp`, base `origin/develop` = `9633b69`.
Scope: `amari-core` only. Contract authority:
`docs/plans/2026-10-10-amari-core-w03-inverse-exp-contract.md` (D02.3 and its
2026-10-12 erratum).

## Files

- `amari-core/src/error.rs` — new public `ExpError` and `NonFiniteStage`
  (std: `thiserror` derive + `Display`; `no_std`: manual `Display`, matching the
  existing `CoreError` pattern). No serde derives, matching `CoreError`.
- `amari-core/src/lib.rs` — `mod exact_square;`, `pub(crate) use
  exact_square::exact_bivector_square;`, and crate-root re-export of
  `ExpError`/`NonFiniteStage`.
- `amari-core/src/exact_square.rs` — new module: 66-word `u64` two's-complement
  signed accumulator, `decompose`, `eta_blade`, `reorder_sign`,
  `exact_bivector_square`, `ExactSquare` (nilpotence / simplicity / sign+angle),
  and the `E = 2q + r` extraction. Unit tests use `num-bigint` as oracle.
- `amari-core/Cargo.toml` — `num-bigint = "0.4"` dev-dependency (already in the
  lock file via other workspace crates).
- Evidence: `stage1-lib-test.log`, `stage1-check-all-targets.log`,
  `stage1-clippy-all-targets.log`, `stage1-fmt.log`.

Untouched: `amari-core/tests/inverse_exp_contracts.rs`, `exp()`, `inverse()`,
`exp_series()`, all consumers.

## Commands / results

Baseline recipe (as dispatched):
`env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_TARGET RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER='' RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2' CARGO_TARGET_DIR=$PWD/target/w03 cargo +stable …`

| Command | Result |
|---|---|
| `cargo test -p amari-core --lib` | **210 passed, 0 failed** (202 pre-existing + 8 new `exact_square::tests`); `stage1-lib-test.log` |
| `cargo check -p amari-core --lib` | pass |
| `cargo check -p amari-core --lib --no-default-features` | pass |
| `cargo clippy -p amari-core --lib -- -D warnings` | pass (0 warnings) |
| `cargo fmt -p amari-core -- --check` | clean (`stage1-fmt.log`, 0 bytes) |
| `cargo fmt -p amari-core` | applied |
| existing integration targets (arithmetic_regressions, audit_tests, basis_word_oracle, geometric_product, gf2_tests, macro_contracts, numerical_contracts, products, rotors) | 102 passed, 0 failed |
| `cargo check -p amari-core --all-targets` | **exit 101** — only `tests/inverse_exp_contracts.rs` (expected stage-1 red) |
| `cargo clippy -p amari-core --all-targets -- -D warnings` | **exit 101** — only `tests/inverse_exp_contracts.rs` (expected stage-1 red) |

## Deviation (disclosed)

`--all-targets` is not green by construction at stage 1: the red contract test
requires the `Result`-returning `exp()` that the dispatch explicitly forbids
changing until stage 3, so `tests/inverse_exp_contracts.rs` cannot compile yet.
Every error in the two `--all-targets` logs is confined to that file (19 ×
missing `.expect()` on `Multivector`, 1 × `.unwrap_or_else`, 1 × `E0308`
`Err/Ok` pattern). The lib, lib-test, and all other integration targets compile
and the lib tests are green. `stage1-clippy-all-targets.log` contains zero
`exact_square` diagnostics.
