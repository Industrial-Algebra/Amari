# P-WASM Compile Closure — EXECUTED GREEN (2026-10-23)

Matrix §5, claim (i) only: **COMPILE SUPPORT** for the full portable
closure on `wasm32-unknown-unknown` — library CODEGEN (`build --lib`;
check is metadata-only per the P-EMBED lesson). **160/160 canonical
capability sets** (R1: the first draft ran 11 representative rows;
R2: the 7-dim/96-set remediation still missed the 64
creusot-contracts-without-formal-verification states — the manifest
has no `dep:` prefix there, so cargo exposes an implicit CLI-addressable
feature activating the annotations crate alone). The space is
8-dimensional: {std, phantom-types, high-precision, gf2, serialize,
parallel, formal-verification, creusot-contracts}; closure edge
formal-verification → {creusot-contracts, phantom-types}; 2^8 = 256
subsets dedup to 160 canonical closed sets (compute-closure.py,
manifest-drift-checked). serde/rayon are normalized aliases of
serialize/parallel; wasm-precision ≡ high-precision;
native-precision/full-precision excluded (rug native-only). Rows
include minimal, creusot-contracts-only (builds green — the state R1
missed), serialize+parallel, formal-compat, and the maximal set. develop `957f223`
(post-#342): every row builds through the new three-arm
`math_dispatch` — **std rows take the inherent arm; std-off rows take
the libm arm** (design-intended; R1 IR-confirmed; the libm seam
exists on this target for std-off configs and libm is pure Rust, so
it builds — the original SUMMARY's "no libm seam on this target" was
wrong and is withdrawn). Stable pinned `88d9e12ae…`, `--locked`.

## Claim (ii) — RUNTIME COVERAGE (unchanged, not re-claimed)

The verified runtime surface remains the existing `amari-wasm`
binding configuration (wasm-pack test --node; CI rows every PR).
Runtime validation of OTHER closures is explicitly UNVERIFIED.

## D-A-bis default record

**NOT now** (proposed default, now executed as such): compile closure
+ binding runtime as the verified surface; extension to
closure-to-behavior fixtures only if a consumer requires it. This is
an ADDITION decision, deferred — not a narrowing of any recorded
promise.

## Regeneration

`wasm-gate.sh` + `compute-closure.py` (committed; TOTAL override honors
evidence AND build artifacts; **fail-closed input validation** —
truncated/empty/missing closure input exits 7 with ZERO cargo
invocations, proven by sentinel-cargo negative tests; the expected
160-row inventory is asserted both before the run and from the
executed rows). Pinned toolchain in toolchain.txt, `--locked`.
Artifacts: results.tsv (160 rows), closure.txt, 4 sample logs
(minimal / creusot-contracts-only / maximal / first),
dropped-logs.sha256 manifest, ARCHIVE-NOTE.md.
