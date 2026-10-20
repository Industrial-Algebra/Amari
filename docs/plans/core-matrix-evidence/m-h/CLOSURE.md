# M-H — Closure Record (full-H execution milestone)

**Option B (staged), amended to C (hybrid) after stage 1** — both operator
decisions recorded via ask_user (2026-10-18). Revision: develop `6463630`
tree, branch `chore/m-h-pilot`, PR #330.

## Quadrant summary

| Quadrant | Invocations | Green | RED | Failing rows | Wall | Target dir |
|---|---|---|---|---|---|---|
| stable 1.98.0 (320×4) | 1,280 | 768 | 512 | 33–160 | 27 m 8 s | 83.8 GiB |
| pinned nightly (320×4) | 1,280 | 768 | 512 | 33–160 | (w/ MSRV) 28 m 35 s | 76.6 GiB |
| MSRV 1.89.0 (320 lib checks) | 320 | 192 | 128 | 33–160 | — | 2.2 GiB |
| **Union** | **2,880** | **1,728** | **1,152** | **33–160, zero deviations across all quadrants** | | |

## What M-H established

1. **The full-H table executed** on the current develop: every one of the
   320 canonical capability rows ran through check/test/clippy/doc on both
   ordinary compilers plus the MSRV library column — 2,880 invocations with
   per-row logs, timings, and test-name manifests (F2: 0 empty manifests
   among passing rows; stable/nightly test-name unions identical at 524 —
   measured cross-compiler selection equivalence; 523 under the original
   capture before the R1 doctest-identity recapture).
2. **192/320 rows fully green in all quadrants**: every `std` row and the
   no-`std` legacy rows. The no-`std` pipeline itself is proven working
   (rows 1–32).
3. **128 rows RED in exactly the three MATRIX-05 families** (no-`std` ×
   {`phantom-types`, `formal-verification`, `gf2`}), compiler-independent,
   exhaustively logged: `grassmannian.rs:48` `vec!` without alloc import
   (1-line repair); `verified.rs` 12+ `T: Float` bounds without num-traits
   `std`/`libm` (design-decision repair). This IS the MATRIX-05 evidence
   deliverable, now at full-matrix resolution.
4. **Pilot estimate validated as conservative**: actual stable wall
   (27 m 8 s) fell BELOW the 30–96 m bracket's lower bound; nightly+MSRV
   similarly. Storage 83.8/76.6 GiB vs ~130 GiB/quad estimate. Target dirs
   retained (not GC'd) for F1 incremental re-runs; disk headroom ample
   (3.6 T free).

## Follow-on (operator-approved as part of option C)

The no-`std` host repair opens as its own bounded contract (matrix-tracked
owner: W00-scope host repair + P-EMBED): gf2 one-liner + the verified.rs
Float-surface design decision (std-gate the Float-bound methods vs
num-traits `libm` activation). After it lands, F1 ledger rules govern
quadrant re-runs: only the 128 invalidated rows, incremental target dirs.

## Supplemental rows (executed post-R1; stage3-supplemental/)

Matrix §4 assigns the §3.1 supplemental rows to M-H; they were executed
after review round 1 flagged their absence (see Remediation below):
13 singletons alone and +std (check+test on both ordinary compilers;
lib checks on MSRV), and the four named alias rows (default,
all-features, full-precision, wasm-precision) through the full four
H commands on both compilers. Results: stable 68 invocations / 6 RED,
nightly 68 / 6, MSRV 30 / 3 — every RED is one of the three MATRIX-05
singleton families (`gf2`/`phantom-types`/`formal-verification` ALONE,
no `std`); all `+std` variants and all alias rows green on every
compiler. Resolved-activation comparisons archived
(`stage3-supplemental/features/`): each alias resolves to exactly its
canonical equivalent's feature set.

## R1 remediation (2026-10-20)

Review round 1 (7 findings: 6 P2, 1 P3) remediated: (1) the exact
executed `Cargo.lock` archived (`executed-Cargo.lock` + sha256 — the
deterministic-reproduction claim previously could not reconstruct the
resolution); (2) test inventories recaptured with FULL doctest identity
(the original `awk $2` pipeline collapsed distinct doctests sharing a
file path; unions 523 → 524, byte-identical across stable/nightly; DEPS-COMPARISON.md records
   alias≡canonical DEPENDENCY-activation identity for every alias with a
   canonical equivalent);
(3) runners now pin exact toolchains and assert commit-hashes
(`toolchain-pins.txt`); (4) fail-closed effective-cfg preflight added
(the original grep pattern was fail-open on `target_feature="avx"`;
pre- and post-fix probes archived); (5) stale artifact references in
dispositions corrected; (6) this closure no longer narrows matrix-owned
obligations — the supplemental rows are executed above; (7) pilot
arithmetic corrected (nightly FULL 18.8 s) and estimates described as
conservative where actuals fell below bracket floors.

## Residuals

- Rows 33–160 remain RED until the no-std repair (by design of option C).
- MSRV dev-dependency support unclaimed (lib checks only, per matrix scoping).
- Named-suite F2 inventories belong to the CI rows, not re-verified here.
