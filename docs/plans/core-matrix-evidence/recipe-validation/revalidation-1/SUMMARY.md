# Revalidation 1 — Summary

**Verdict: RECIPE CONFORMANCE RESTORED for the default-core checkpoint scope.** Executed 2026-10-09 after the maintainer approved the minimal correction ("Great - lets do that"): explicit-empty wrapper overrides plus a bounded rerun. The frozen failed run remains archived and unchanged.

## Correction applied

`docs/plans/2026-10-09-amari-core-verification-matrix.md` §3.2 only: `RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER=''` (explicit empty, overriding user Cargo config in every child Cargo process, including cargo-clippy's inner re-exec) replaced the ineffective `-u RUSTC_*WRAPPER` + `--config build.rustc-wrapper=""` combination; `D07_BASELINE_TARGET_DIR` now requires an explicit never-reused artifact directory. Pre-edit matrix and checkpoint archived under `inputs/` (hashes verified). No production source, manifest, config, dependency, or workflow change; user sccache configuration untouched.

## Execution

Both installed compilers, default features, host target, `--locked --offline`, deliberately contaminated inherited environment (native/AVX2 encoded flags, invalid compiler/documenter/wrapper/target variables). Each command group inspected before runtime or compiler progression; Clippy before tests; stable accepted before nightly.

| Compiler | cargo-cfg | check | doc | clippy | test |
|---|---|---:|---:|---:|---|
| stable 1.98.0 | 0 | 0 | 0 | **0** | **0** |
| nightly 1.100.0-nightly | 0 | 0 | 0 | **101** | **0** |

## Conformance evidence

- **Wrapper isolation:** zero `sccache` occurrences in all ten logs; genuine `bin/clippy-driver` chains in both Clippy runs; pinned rustc/rustdoc paths; explicit `--target x86_64-unknown-linux-gnu`; baseline flags (`-C target-cpu=x86-64 -C target-feature=-avx2`) in all core invocations; rustdoc retains `-D warnings`.
- **Negative controls:** poisoned encoded flags/compiler/wrapper/target variables were all overridden; direct and effective cfg contain no AVX/AVX2/FMA on either compiler.
- **Known-red control:** nightly Clippy exit 101 with exactly the three unique `needless_range_loop` errors at `verified.rs:159,339,387` plus ordinary lib/lib-test compile summaries — lint execution proven, not silenced. Stable Clippy 0.
- **Diagnostics:** every warning fingerprint in all eight compile/doc/clippy logs matches the frozen identity sets (stable 87, nightly 207/513 per command); zero new fingerprints; only the six recorded `rustix@1.1.5` profile-hint notices reappeared. No dependency errors.
- **Tests:** 202+17+15+6+17+27 = 284 unit/integration + 3 doctests = 287 passed per compiler; 0 failed/ignored/measured/filtered; the 287 successful test names are identical across compilers.
- **Immutability:** root lock hash, HEAD `ba1b7719…`, frozen failed-run logs, and archived inputs all unchanged; tracked non-plan diff empty; fresh artifacts only under `target/d07-baseline/revalidation-1/{stable,nightly}/`.

## Limits

This validates the **recipe** for default-core host evidence only. It is not full H, not portability/AVX2/proof/safety evidence, not CORE-D07 approval, and not W00/W01 implementation authority. The three nightly loop errors remain unfixed library findings (CORE-W00 scope); dependency warnings remain recorded baseline, not resolved. Round-3 cumulative review not yet dispatched.
