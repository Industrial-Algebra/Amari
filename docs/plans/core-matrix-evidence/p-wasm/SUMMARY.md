# P-WASM Compile Closure — EXECUTED GREEN (2026-10-23)

Matrix §5, claim (i) only: **COMPILE SUPPORT** for the full portable
closure on `wasm32-unknown-unknown` — library CODEGEN (`build --lib`;
check is metadata-only per the P-EMBED lesson). **96/96 canonical
activation sets** (R1 remediation: the first draft ran 11
representative rows — §5's "including" list was exemplary, not the
boundary). The closure is computed from the manifest implication
edges over the 7-dimension portable feature space
{std, phantom-types, high-precision, gf2, serialize, parallel,
formal-verification}: 128 subsets dedup under closure to 96 canonical
sets (the 32 formal-verification-without-phantom subsets collapse
into their closures); aliases are not separate dimensions
(wasm-precision ≡ high-precision); native-precision/full-precision
excluded (rug native-only by design). Every row incl. minimal,
serialize+parallel rows, and formal-compat. develop `957f223`
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

`wasm-gate.sh` (committed; TOTAL override honors evidence AND build
artifacts), pinned toolchain in toolchain.txt, `--locked` against the
worktree lock. 11 logs retained (small enough not to need slimming).
