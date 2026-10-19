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
   among passing rows; stable/nightly test-name unions identical at 523 —
   measured cross-compiler selection equivalence).
2. **192/320 rows fully green in all quadrants**: every `std` row and the
   no-`std` legacy rows. The no-`std` pipeline itself is proven working
   (rows 1–32).
3. **128 rows RED in exactly the three MATRIX-05 families** (no-`std` ×
   {`phantom-types`, `formal-verification`, `gf2`}), compiler-independent,
   exhaustively logged: `grassmannian.rs:48` `vec!` without alloc import
   (1-line repair); `verified.rs` 12+ `T: Float` bounds without num-traits
   `std`/`libm` (design-decision repair). This IS the MATRIX-05 evidence
   deliverable, now at full-matrix resolution.
4. **Pilot estimate validated**: actual wall inside every bracket (27–29 m
   vs 30–96 m stable estimate — brackets conservative); storage 83.8/76.6
   GiB vs ~130 GiB/quad estimate. Target dirs retained (not GC'd) for F1
   incremental re-runs; disk headroom ample (3.6 T free).

## Follow-on (operator-approved as part of option C)

The no-`std` host repair opens as its own bounded contract (matrix-tracked
owner: W00-scope host repair + P-EMBED): gf2 one-liner + the verified.rs
Float-surface design decision (std-gate the Float-bound methods vs
num-traits `libm` activation). After it lands, F1 ledger rules govern
quadrant re-runs: only the 128 invalidated rows, incremental target dirs.

## Residuals

- Rows 33–160 remain RED until that repair (by design of option C).
- MSRV dev-dependency support unclaimed (lib checks only, per matrix scoping).
- Named-suite F2 inventories belong to the CI rows, not re-verified here.
- cfg/alias/singleton/`default`/`all-features` cross-checks beyond the four
  canonical H commands are P-HOST/P-AVX2-adjacent rows, not M-H scope.
