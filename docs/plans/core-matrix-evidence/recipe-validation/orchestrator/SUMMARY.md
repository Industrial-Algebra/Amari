# CORE-D07 Recipe Checkpoint — FAILED CONFORMANCE / PAUSED

## Authorization and immutable snapshot

On 2026-10-09 the maintainer explicitly selected “Authorize this checkpoint”: bounded two-stage default-core recipe validation plus report-only review, not full H or remediation implementation. The worktree branch was separately confirmed as `chore/core-verification-matrix`.

- Actual Pi process/tool/session-header cwd: Amari `.worktrees/core-matrix`.
- Source: `ba1b7719f609e8f0f127d620ba6936cfffb61413`.
- Matrix SHA-256: `40c43115dd04bc0569270ab9c1715e6912b311dc4aff1d49a5aee341db38c938`.
- Checkpoint SHA-256: `c59543bbf2199cc7ea4a411f289da22f86f8a8a4927e4eed18c23941fe85b510`.
- Root lock SHA-256: `acb76177dfb6ad26413de8a56c8ac9a6f8c6f2ec866516e6f95ceb828b6159f5`; unchanged before/after both phases.
- Explicit skill transitions: executing-plans for the authorized checkpoint, then verification-before-completion; monitor read confirmed `verify: active`. No production implementation took place. Monitor phase labels are not evidence that library remediation or the full matrix passed.

The two plan documents were frozen between phases and still contain their pre-execution drafting/status statements. This record supplies the actual bounded execution results; it does not silently rewrite or approve the matrix.

## Commands and exits

The common setup and phase A/B Bash blocks were extracted verbatim from the pinned checkpoint Markdown and executed in separate Bash subshells. No helper source file was created. All Cargo invocations used the matrix's canonical baseline wrapper, `--locked --offline`, explicit host target, default core features, and deliberately contaminated inherited compiler/documenter/wrapper/encoded-ISA/target environment values.

| Command | stable | nightly |
|---|---:|---:|
| Direct baseline cfg + assertions | 0 | 0 |
| Effective Cargo cfg + assertions | 0 | 0 |
| Default all-target check | 0 | 0 |
| Default warning-denied rustdoc | 0 | 0 |
| Default tests/doctests | 0 | 0 |
| Default all-target Clippy with `-D warnings` | 0 | 101 |

Raw exits/logs are in each compiler's `status.tsv` and command `.log` files. Phase-A inspection records were written after reading the full direct/effective cfg and provenance outputs and relevant target rustc/rustdoc invocations. They attest phase A only; they are not blanket Clippy conformance approval.

### Runtime inventory

Each compiler executed:

- Unit: 202.
- Integration targets: 17 + 15 + 6 + 17 + 27 = 82.
- Total unit/integration: **284**.
- Doctests: **3**.
- Failed/ignored/measured/filtered: **0** in each of seven result summaries.

All 287 successful test-name entries were extracted from each complete raw test log and compared; the sorted inventories match. The stable execution section was additionally read in full. The raw logs preserve every name. These tests are ordinary cases, not closure of historical CORE findings or proof obligations.

## What conformed

Both direct/effective cfg outputs select x86_64 Linux, with AVX/AVX2/FMA absent. Relevant phase-A target invocations pin the selected rustc/rustdoc executable and use `-C target-cpu=x86-64 -C target-feature=-avx2`; rustdoc retains `-D warnings`. No external wrapper was present in those inspected phase-A core target commands.

See `stable/inspection.md` and `nightly/inspection.md` for exact log lines. This finding remains limited to the inspected paths; it does not establish Clippy isolation or all feature/target paths.

## Concrete blocker: Clippy restores external sccache

Actual verbose core invocations:

- `stable/clippy.log:762,1023`.
- `nightly/clippy.log:6985,7485` (failure commands also repeated after the final diagnostics).

They run:

```text
sccache <selected-toolchain>/bin/clippy-driver <selected-toolchain>/bin/rustc ...
```

This contradicts the recipe's external-wrapper-disabled claim. The user Cargo config contains `build.rustc-wrapper = "sccache"`. The canonical wrapper removes environment wrapper variables and supplies empty outer `cargo --config` wrapper settings; this was effective in inspected direct Cargo check/doc paths but **not** the observed Clippy path. The precise subcommand/config propagation mechanism still needs independent scrutiny; no untested correction has been applied.

Installed Cargo documentation says an empty `RUSTC_WRAPPER` overrides configuration and disables its wrapper; unsetting it is not the same override. That provides a candidate to investigate, not a verified repaired recipe. Preserve Clippy's own driver: suppressing lint execution to make the command green would be invalid.

The checkpoint is **FAILED CONFORMANCE**, regardless of stable Clippy exit 0 or phase-B script exit 0. The script's status/expected-lint checks were not themselves sufficient to establish absence of external wrappers.

## Additional diagnostic inventory

Complete-log scanning found no new core error kind: nightly renders the three known `needless_range_loop` diagnostics at `verified.rs:159,339,387` plus lib/lib-test failure summaries. Stable has no error lines.

However `-vv` also exposes registry-dependency warnings:

- Stable: **94 warning blocks** across 14 registry package/version paths.
- Nightly: **732 warning blocks** across 24 registry package/version paths.
- Non-summary warning blocks without an attributable registry path: **0** in either Clippy log.

Stable warning owners/counts: bit-set-0.8.0=1, bit-vec-0.8.0=10, cast-0.3.0=1, ciborium-0.2.2=2, ciborium-ll-0.2.2=3, criterion-0.8.2=4, itertools-0.13.0=6, plotters-0.3.7=4, plotters-backend-0.3.7=1, proptest-1.11.0=15, rayon-1.12.0=44, same-file-1.0.6=1, tinytemplate-1.2.1=1, wait-timeout-0.2.1=1.

Nightly warning owners/counts: approx-0.5.1=4, bit-set-0.8.0=1, bit-vec-0.8.0=10, bytemuck-1.25.2=24, cast-0.3.0=208, ciborium-0.2.2=2, ciborium-ll-0.2.2=6, clap_builder-4.6.7=1, criterion-0.8.2=4, criterion-plot-0.8.2=2, crossbeam-epoch-0.9.20=1, itertools-0.13.0=6, memchr-2.8.3=2, num-traits-0.2.19=365, oorandom-11.1.5=2, plotters-0.3.7=4, plotters-backend-0.3.7=1, proptest-1.11.0=31, rayon-1.12.0=48, regex-automata-0.4.18=3, same-file-1.0.6=1, tinytemplate-1.2.1=2, wait-timeout-0.2.1=3, walkdir-2.5.0=1.

These are observed warning-block counts, not independent defects or newly introduced dependency regressions. No warning exception or dependency fix has been inferred. The checkpoint's additional-diagnostic stopping rule needs a precise disposition before execution resumes.

## Stop and limits

Execution stopped after inspection, before report-only review round 2. No retry, new reviewer dispatch, source/dependency/configuration fix, tool install, fetch, lock regeneration, commit, PR, merge, or release action followed the failure. Tracked changes outside docs/plans remain empty; only the approved plan/evidence/build artifacts and harness-managed state were involved.

Next action requires explicit scope: independently diagnose the failed conformance read-only, or approve a new documentation correction/revalidation checkpoint. Preserve the failed run; do not overwrite it or call it green.

Full H, aliases, exact dated/MSRV/CI compilers, target builds/runtimes, new AVX2 paths, safety tooling, actual proof translation/discharge and W00/W01 implementation remain unexecuted/unapproved by this checkpoint. Concrete CORE-D07 approval is still pending.
