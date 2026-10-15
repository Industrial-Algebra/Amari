# M-REP-B evidence — suite integrity (2026-10-15)

Approved matrix: `docs/plans/2026-10-09-amari-core-verification-matrix.md`
§3 "Named-suite filter integrity" (loop-2 F2) + §10 M-REP-B split (MATRIX-04).
Base: `develop` `e8559b7`; lock `eb2442dc`; worktree `core-mrep-b`; target
`target/mrepb`.

Per-site root-cause analysis and the nine-site disposition table are in
`investigation.md`. Red inventory: `red/filter-inventory.txt`,
`red/invocation-sites.txt`.

## Change summary

| Artifact | Change |
|---|---|
| `scripts/verify-named-suite.sh` | **New.** Shared executable vacuity + approved-inventory assertion for every surviving named filter. |
| `amari-core/tests/geometric_axioms.rs` | **New.** Six public-API instance tests under a `geometric_axioms` module; the M04/`MATRIX-04` missing suite. |
| `.github/workflows/mathematical-correctness.yml` | Both named-suite steps now call the helper with explicit feature flags and inventories; coverage gather repaired (`TOTAL_TESTS` and `FORMAL_FEATURE_TESTS`). |
| `.githooks/pre-commit` | Same helper-based named suites; network `verified_contracts` fake-green removed; coverage gather repaired (`: test$` pattern + single well-formed count). |
| `docs/technical/TEST_ENFORCEMENT.md` | Table rows 7–13 rewritten, row 11 marked REMOVED, line ranges re-mapped for the 315-line hook, M-REP-B status note. |
| `docs/technical/MATHEMATICAL_RIGOR.md` | Named-suite command reference now uses the helper; coverage-floor and phantom-suite notes updated. |

No production behaviour changed. The only Rust addition is the new test file;
`verified_contracts`/`semiring` modules were untouched (their tests already
existed; only the invocations were wrong).

## Named-suite dispositions (fixed-invocation unless noted)

| Suite | Package | Feature | Disposition |
|---|---|---|---|
| `geometric_axioms` | amari-core | default | **new-suite** + assertion (6 tests) |
| `semiring_contracts` | amari-tropical | `contracts` | **fixed-invocation** (filter was `semiring_axioms`) + assertion (2) |
| `verified_contracts` | amari-core | `formal-verification` | **fixed-invocation** + assertion (4) |
| `verified_contracts` | amari-tropical | `contracts` | **fixed-invocation** + assertion (5) |
| `verified_contracts` | amari-network | — | **removed** (module has no `#[test]` functions) |

## Verification — pinned recipe

`RUSTC`/`RUSTDOC` = stable via `rustup which`, `RUSTC_WRAPPER=''`,
`RUSTC_WORKSPACE_WRAPPER=''`,
`RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2'`,
`CARGO_TARGET_DIR=$PWD/target/mrepb`, `--locked`, `cargo +stable`
(the toolchain file pins nightly; stable Cargo used per M-REP-A).
Full env: `green/recipe-env.txt`.

| Gate | Result | Log |
|---|---|---|
| `cargo +stable test -p amari-core --locked` | **495 passed / 18 suites**, exit 0, 0 failed, 0 ignored (was 489/17) | `green/amari-core-default-test.log` |
| `cargo +stable test -p amari-core --features formal-verification --locked` | **499 passed / 18 suites**, exit 0, 0 failed, 0 ignored (was 493/17) | `green/amari-core-formal-test.log` |
| `cargo +stable test -p amari-tropical --locked` | 45 passed / 4 suites, exit 0 | `green/amari-tropical-default-test.log` |
| `cargo +stable test -p amari-tropical --features contracts --locked` | 50 passed / 4 suites, exit 0 | `green/amari-tropical-contracts-test.log` |
| `cargo +stable test -p amari-network --locked` | 17 passed / 4 suites, exit 0 | `green/amari-network-default-test.log` |
| Named-suite assertions (all four suites + negative controls) | all OK; vacuous filters exit 1 | `green/named-suite-assertions.log` |
| `cargo +stable clippy -p amari-core --all-targets --locked -- -D warnings` | exit 0, 0 warnings | `green/amari-core-clippy-default.log` |
| `cargo +stable clippy -p amari-core --all-targets --features formal-verification --locked -- -D warnings` | exit 0, 0 warnings | `green/amari-core-clippy-formal.log` |
| `cargo +stable fmt --all --check` | exit 0 | `green/fmt-check.log` |
| `RUSTDOCFLAGS='-D warnings' cargo +stable doc -p amari-core --no-deps --locked` | exit 0 | `green/amari-core-rustdoc.log` |
| `cargo +stable check --workspace --locked` | exit 0 | `green/workspace-check.log` |
| All `.github/workflows/*.yml` YAML valid | OK (7/7) | `green/yaml-validity.log` |
| `bash -n` hook + helper (shellcheck not installed) | OK | `green/shell-validation.log` |
| Coverage-floor repair demonstration | old `$'0\n0'` no-block → new `0` blocks; real `2075` passes | `green/coverage-floor-repair.log` |

### Test-count delta (disclosed)

The new integration target adds **+6 tests and +1 test binary** in every
configuration that builds `amari-core`, so default moves 489/17 → **495/18** and
formal 493/17 → **499/18**. These six are real, independently passing
instance tests (associativity, distributivity, multiplicative identity,
additive identity, scalar commutativity, basis signature), not placeholders.

## Residuals and limitations

- **Workspace `--all-features` Clippy is BLOCKED in this environment**, not by
  this change: the build of `z3-sys v0.11.0` panics in its `cmake` custom build
  step (`green/workspace-clippy-allfeatures.log`, exit 101). The dispatch's
  Clippy gate (`amari-core` default + formal) passes. This pre-existing
  environmental limitation also affected M-REP-A (whose matrix used the same
  scoped Clippy rows). No source in this change reaches `z3`.
- **`shellcheck` is not installed**; the hook and helper were validated with
  `bash -n` instead, and the helper's positive/negative behaviour was exercised
  directly (`green/named-suite-assertions.log`).
- The M-REP-A `hook-extraction.txt` remains the historical transcript of the
  292-line hook; `TEST_ENFORCEMENT.md` now documents the 315-line post-M-REP-B
  hook. Regenerating the full M-REP-A extractor output was not required for the
  changed rows, but the extractor itself is not committed, so the transcript is
  not reproducible from-tree (tracked as a residual, unchanged by M-REP-B).
- `verified_contracts` annotation soundness (NaN/inf contracts) and the
  phantom/unsafe audits remain **M-REP-C/D**, untouched here.
- **MATRIX-04's badge clause is not in this dispatch's enumerated scope.** The
  dispatch covers the nine named filters and the coverage floor. `test-status.yml`
  and `status-badges.yml` count test functions (an inventory, honestly relabelled
  by M-REP-A); they are not named filters and do not gate, so the F2 rule does not
  apply to them. The M-REP-A note that their *selection/count mechanics* differ is
  left as an open, flagged follow-up rather than silently changed here
  (`status-badges.yml`'s badge-creation steps are commented out; `test-status.yml`
  falls back to a source `#[test]` count).

## Out-of-scope confirmation

No API, algorithm, dependency, feature set, toolchain pin, or production
runtime behaviour was changed. The only new Rust file is a test; the only
workflow/hook changes are named-suite invocations, assertions, and the
coverage-gather mechanics.
