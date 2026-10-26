# P-WASM Compile Closure — EXECUTED GREEN (2026-10-23)

Matrix §5, claim (i) only: **COMPILE SUPPORT** for the full portable
closure on `wasm32-unknown-unknown` — library CODEGEN (`build --lib`;
check is metadata-only per the P-EMBED lesson). 11/11 rows exit 0:
default, minimal, std, high-precision, wasm-alias (≡ high-precision),
phantom, gf2, serialize, formal-compat (creusot-contracts annotations
compile), combo-std-phantom-gf2, combo-portable-full (the maximal
portable set: everything except native-precision, which is
native-only by design). develop `957f223` (post-#342 — every row
builds through the new three-arm `math_dispatch`; wasm is hosted+std
→ the inherent arm, no libm seam on this target), stable pinned
`88d9e12ae…`, `--locked`.

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
