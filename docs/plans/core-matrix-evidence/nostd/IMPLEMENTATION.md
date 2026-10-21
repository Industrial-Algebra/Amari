# No-std Host Repair — Implementation Record

**Branch:** `fix/nostd-host-repair` from develop `2156514`; contract PR #336
(converged R2). Lock: the M-H `executed-Cargo.lock` (no change — S4 dropped).

## The repair (three files, five lines)

1. `gf2/grassmannian.rs`: `use alloc::vec;` (S1, as contracted).
2. `verified.rs`: `use alloc::vec; use alloc::vec::Vec;`;
   `std::marker::PhantomData` → `core::marker`;
   `std::f64::consts::FRAC_1_SQRT_2` → `core::f64::consts` (S2).
3. `verified_contracts.rs`: `use alloc::vec::Vec;` — ONE site beyond the
   contract's census (the formal-only module; the contract's completeness
   gate explicitly reserved the re-run as the authority for such sites).

RED confirmed first (row 33 lib check: 28 errors pre-repair on this exact
tree), then GREEN per-family, then the full gauntlet.

## Re-verification (F1: incremental on retained M-H target dirs; pinned
toolchains, fail-closed cfg preflight, `--locked`; runner `reverify.sh`)

| Gate | Invocations | Result |
|---|---|---|
| The 128 RED rows, stable ×4 cmds | 512 | 0 failed |
| The 128 RED rows, nightly ×4 | 512 | 0 failed |
| The 128 RED rows, MSRV lib | 128 | 0 failed |
| **Full 320-row drift sweep, stable** | 1,280 | **0 failed** |
| **Full 320-row drift sweep, nightly** | 1,280 | **0 failed** |
| **Full 320-row drift sweep, MSRV** | 320 | **0 failed** |
| Supplemental singletons+aliases, 3 quadrants | 166 | 0 failed |
| Catalog drift gate | — | green, **no delta** (expected post-S4-drop) |
| amari-core default full test row | — | 514/0 |
| clippy `-D warnings`, fmt, root 9/0, doc-tests 3/0 | — | green |

**Post-state: the entire M-H matrix (2,880 + 166 + 128-row focus) is green.**
MATRIX-05's host-scope obligation is discharged; the retained target dirs
remain available for future F1 re-runs.

## Residuals

- P-EMBED inherits the libm question (num-traits without std on bare-metal
  targets) — separate matrix item, per the contract's deferral.
- MSRV dev-dependency support unclaimed (lib checks only, per matrix scoping).
