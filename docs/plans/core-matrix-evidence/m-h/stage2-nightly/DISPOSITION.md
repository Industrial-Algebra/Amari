# M-H Stage 2 — Nightly full-H Disposition

**Executed:** 2026-10-19 (same revision as stage 1: source tree identical to
develop `6463630`; branch `chore/m-h-pilot`), runner `../stage2-nightly.sh`
(pinned nightly `1.100.0-nightly (787af2b8c 2026-08-25)`, hash-verified in the
pilot; §3.2 wrapper discipline; fresh target dir; `--locked`; F2 per-row
test-name capture). Wall 28 m 35 s (incl. MSRV quadrant); target dir 76.6 GiB.

## Result

- **1,280 invocations (320 × 4): 768 green, 512 RED** — identical to stable.
- **Failing set = rows 33–160 exactly; zero deviations** in either direction
  (no row outside failed; no row inside passed). The compiler-independence
  prediction of the stage-1 diagnosis held exactly.
- Passing test rows: 192; empty test-name files among them: **0**; unique
  test-name union: **524 — identical to stable's union**, i.e. cross-compiler
  test-selection equivalence measured, not assumed. (The originally captured
  unions said 523: the R1 pipeline collapsed distinct same-file doctests;
  post-R1 recapture with full identity yields 524, still byte-identical
  across compilers. Superseded inventories retained, marked.)
- Per-command wall (median/max): check 0.8/7.1 s · test 2.7/7.9 s · clippy
  0.9/2.0 s · doc 0.3/3.6 s.

Retained artifacts: `results.tsv` (every invocation), `failures.txt`
(every failure), `samples/` (one log per failure family + first green row
per command), `summary.txt`. Test inventories: `tests2-manifest.tsv` + `tests2-union.txt`
(post-R1 full-doctest-identity recapture; the original collapsed
inventories were superseded).
Historical full log/test sets were never committed; see `ARCHIVE-NOTE.md`
for the regeneration recipe.
