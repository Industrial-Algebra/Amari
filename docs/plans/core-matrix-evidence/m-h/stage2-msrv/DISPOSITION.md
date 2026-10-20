# M-H Stage 2 — MSRV column Disposition

**Executed:** 2026-10-19, same revision; runner `../stage2-msrv.sh`
(`1.89.0`; §3.2 wrapper discipline; fresh target dir; `--locked`; the
matrix's 320 MSRV **library checks** only, per §4's "Library compile checks
across H"). Target dir 2.2 GiB.

## Result

- **320 invocations: 192 green, 128 RED** — failing set = rows 33–160
  exactly, zero deviations (same MATRIX-05 families; dependency activation,
  not compiler surface).
- Per-row wall: min 0.3 s / median 0.4 s / max 3.7 s / mean 0.43 s.
- Every green row is an MSRV-clean library configuration; dev-dependency
  MSRV support remains a separate row per the matrix's own scoping (not
  implied by these lib checks).

Retained artifacts: `results.tsv` (every invocation), `failures.txt`
(every failure), `samples/` (one log per failure family + first green row
per command), `summary.txt`.
Historical full log/test sets were never committed; see `ARCHIVE-NOTE.md`
for the regeneration recipe.
