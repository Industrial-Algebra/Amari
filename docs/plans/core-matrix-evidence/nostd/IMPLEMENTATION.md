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

## R1 remediation (2026-10-21)

Reviewer P2: `supplemental-reverify.sh` derived FAILED via
`grep -c ... || echo 0` — grep prints 0 AND exits 1 on no match, so the
fallback appended a second line (`FAILED=$'0\n0'`), and the shared
failures.txt let earlier-mode failures contaminate later summaries.
Fixed in BOTH the nostd runner and the committed M-H stage3 script
(the same bug existed there): counts now derive from the authoritative
per-mode results TSV. Negative-proven with a poisoned singleton
(rc=1, FAILED=2 reported); positive-proven clean (rc=0, 30/0).

**Incident disclosure (the M-H R4-R6 lesson recurred):** the FIRST
poison attempt targeted a literal script line that exists only inside
the runner's loop — the sed no-op'd, so the "proof" vacuously passed;
the SECOND attempt redirected OUT_ROOT while OUT was hardcoded, so the
poison run wrote to the real evidence tree (interleaved rows, dirty
logs). Evidence restored from the pre-incident commit (30/68/68 rows,
0 failed, byte-clean); the runner now honors OUT_ROOT_OVERRIDE so
scratch runs cannot touch the evidence tree; the final positive and
negative proofs both ran isolated (scratch rc=0 with clean summary;
poisoned scratch rc=1).
