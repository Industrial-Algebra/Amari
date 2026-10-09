# CORE-W00 — Green-gate provenance

Recorded after review round 1 identified two evidence gaps (misplaced summary; missing execution metadata; formatting drift). All gates below were re-executed 2026-10-09 with `-vv` after the formatting fix; the first (non-`-vv`) green logs are preserved in `initial/`. Red logs in `../red/` predate the fix and were produced by the identical wrapper function with the exits recorded in `../SUMMARY.md`.

## Toolchains

- stable: `rustc 1.98.0 (88d9e12ae 2026-08-18)` — `/home/lucien/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin/rustc`
- nightly: `rustc 1.100.0-nightly (787af2b8c 2026-08-25)` — `/home/lucien/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/bin/rustc`

## Wrapper (validated D07 recipe, verbatim)

`baseline_cargo` resolves `rustc`/`rustdoc` via `rustup which --toolchain "$T"`, then runs cargo under
`env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_ENCODED_RUSTDOCFLAGS -u CARGO_BUILD_TARGET -u CARGO_BUILD_RUSTC -u CARGO_BUILD_RUSTDOC -u CARGO_BUILD_RUSTC_WRAPPER -u CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER='' RUSTC="$compiler" RUSTDOC="$documenter" RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2' RUSTDOCFLAGS='-D warnings -C target-cpu=x86-64 -C target-feature=-avx2' CARGO_TARGET_DIR="$D07_BASELINE_TARGET_DIR"`
with `cargo +"$T"`. This is the round-3-reviewed correction from `recipe-validation/revalidation-1`; empty env overrides defeat the user Cargo-config `sccache` wrapper in every child Cargo process. The `-vv` logs embed each `Running \`...\`` invocation and show pinned toolchain executables and explicit `--target x86_64-unknown-linux-gnu` (e.g. `G7-stable-check.log:90–91`, 9 core rustc invocations). `RUSTC_WRAPPER=''` is not echoed per rustc invocation by cargo; wrapper isolation is evidenced by zero `sccache` occurrences in all 14 green logs plus the recipe validated in revalidation-1.

## Exact commands and exits (`status.tsv`)

| Log | Command (after `baseline_cargo`) | Exit |
|---|---|---|
| G1-{stable,nightly}-default-clippy | `clippy --locked -vv -p amari-core --target x86_64-unknown-linux-gnu --all-targets -- -D warnings` | 0 / 0 |
| G2-no-default-check | `check --locked -vv -p amari-core --no-default-features --lib --target …` | 0 |
| G5a | `clippy --locked -vv -p amari-core --no-default-features --lib --target … -- -D warnings` | 0 |
| G5b | same with `--features std --all-targets` | 0 |
| G5c | same with `--features high-precision --lib` | 0 |
| G3-std-only-test | `test --locked -vv -p amari-core --no-default-features --features std --target …` | 0 |
| G4-minimal-gf2-test | `test --locked -vv -p amari-core --no-default-features --features std,gf2 --target …` | 0 |
| G6-{stable,nightly}-default-test | `test --locked -vv -p amari-core --target …` | 0 / 0 |
| G7-{stable,nightly}-check | `check --locked -vv -p amari-core --target … --all-targets` | 0 / 0 |
| G7-{stable,nightly}-doc | `doc --locked -vv -p amari-core --target … --no-deps` | 0 / 0 |

All runs from `CARGO_TARGET_DIR=target/d07-baseline/w00/<compiler>`. Lock hash before, between, and after all gates: `ba6b36cf91a1b906e098598eac9ab9fbf6e7e945a96a3295571416826d96ca46` (matches `../red/lock.sha256`).

## Formatting

`rustfmt --edition 2021` (stable toolchain) applied in-place to `verified.rs` and `rotor.rs` after round 1 (assignment reflow at the three edited sites; import placement in rotor.rs — whitespace only, no token changes); `rustfmt --check --edition 2021` exits 0 on all five whitelisted files.

## Counts and inventories (from the `-vv` logs)

- G1 nightly: zero `^(error|warning):` diagnostics.
- G3: gf2 suite `0 passed` (empty, gate active); other suites pass.
- G4: gf2 suite 6/6.
- G6: 202+17+15+6+17+27 = 284 + 3 doctests = 287 per compiler; sorted successful-name inventories identical stable==nightly==pre-fix revalidation-1 baseline.
