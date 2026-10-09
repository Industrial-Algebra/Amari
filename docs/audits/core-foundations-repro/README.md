# Reproduce the first amari-core audit

Baseline: `165d7ccbd6d1aa899babb2d0ae392fa15272e556`. See [report](../2026-09-19-core-foundations.md).

This is a **standalone, unpublished workspace**. It is deliberately not a member of the production workspace: the 21 failing law/contract probes document unfixed defects and API-contract gaps. They are not ignored tests, production fixes, or tests that assert buggy output is correct. The one independent-oracle test passes. Review proposed convention/API changes before promoting a reproduction into the production regression suite.

From the Amari checkout containing the audit files:

```bash
# Restore the small, recorded dependency resolution. Cargo.lock itself is ignored.
cp docs/audits/core-foundations-repro/Cargo.lock.snapshot \
   docs/audits/core-foundations-repro/Cargo.lock

# Expected: 1 passing test, 21 filtered out.
CARGO_TARGET_DIR="$PWD/target" cargo test --locked \
  --manifest-path docs/audits/core-foundations-repro/Cargo.toml oracle_

# Expected at the pinned baseline: 1 passed, 21 failed; exit 101.
CARGO_TARGET_DIR="$PWD/target" cargo test --locked \
  --manifest-path docs/audits/core-foundations-repro/Cargo.toml

# Same observed result with optimizations.
CARGO_TARGET_DIR="$PWD/target" cargo test --locked --release \
  --manifest-path docs/audits/core-foundations-repro/Cargo.toml

# Isolate one report finding, for example the inverse contract:
CARGO_TARGET_DIR="$PWD/target" cargo test --locked \
  --manifest-path docs/audits/core-foundations-repro/Cargo.toml finding_01
```

The original run used `--offline` with cached dependencies; omit it when dependencies need downloading. The path dependency always uses the **current checkout's** amari-core, so check `git rev-parse HEAD` before interpreting a change in results. The report and saved output refer to the pinned baseline, not future fixes.

The oracle concatenates explicit generator words, sorts using adjacent transpositions, cancels pairs using the metric, and reconstructs the surviving blade. It does not copy the implementation's XOR and bit-pair sign algorithm. Across all P,Q,R with P+Q+R ≤ 4, it enumerates `Σ C(n+2,2) 4^n = 4,589` ordered basis pairs. Numerical comparisons use coefficientwise absolute error, not amari-core's audited metric `norm()` or `approx_eq()`.

For convention-dependent findings, the expected result comes from the existing documented contract (not an assertion that one convention is the only possible design). The raw-rotor-input test intentionally violates a prose precondition to demonstrate that the type boundary does not enforce it.
