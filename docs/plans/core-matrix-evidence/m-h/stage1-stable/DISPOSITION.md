# M-H Stage 1 (stable full-H) — Disposition

**Executed:** 2026-10-18, worktree `chore/m-h-pilot` (source tree identical to
develop `6463630`), runner `../stage1-stable.sh` (§3.2 wrapper discipline,
fresh target dir, `--locked`, F2 per-row test-name capture,
failure-record-and-continue). Wall 27 m 8 s; target dir 83.8 GiB (both inside
the pilot estimates).

## Result

- **1,280 invocations** (320 rows × check/test/clippy/doc): **768 green, 512 RED**.
- **192/320 rows fully green** — every `std` row and the no-`std` legacy rows
  (1–32). Every passing row produced ≥1 test name; unique test-name union 523.
- **128/320 rows RED in exactly the three MATRIX-05 families** (rows 33–160:
  no-`std` × {`phantom-types`, `formal-verification`, `gf2`}), all four
  commands each, all `rc=101`. Full logs in `logs/`, failures in
  `failures.txt` (sha256 7489bdab…), per-command TSV in `results.tsv`.

## Diagnosis (verified against source, not inferred)

1. `amari-core/src/gf2/grassmannian.rs:48` — `vec!` used without an
   `alloc::vec` import under `no_std` (the matrix's documented "+gf2 (1
   error)", unchanged since 354945a). One-line bounded repair.
2. `amari-core/src/verified.rs` — 12+ `T: Float + Zero + One` bounds on
   methods reachable via `phantom-types`/`formal-verification` with `std`
   off; `num_traits::Float` has no `f32`/`f64` impls without num-traits'
   `std` (or its `libm` feature) → E0283/E0308/E0425 cascade (57–61 errors
   per row; grew from the documented 27/32 because M-REP-C added
   `all_finite`/`rotor_tolerance` Float bounds). Repair needs a design
   decision (std-gate the Float-bound surface vs num-traits `libm`
   activation) — its own bounded contract.

These are pre-existing, matrix-tracked defects (MATRIX-05: "remain owed to
W00-scope host repair + P-EMBED"), compiler-independent (dependency
activation), NOT M-H regressions: rows 1–32 prove the no-`std` pipeline
itself works; only the three gated modules break it.

## Gate amendment (operator-approved, ask_user 2026-10-18)

Option B's stage-2 gate ("on stable-green") is amended per option **C —
hybrid**: stage 2 (nightly full-H + MSRV column) proceeds on the same
revision with the 128 rows expected-RED in every quadrant; M-H closes with
complete 4-quadrant evidence including the RED families (the MATRIX-05
deliverable); the no-std host repair opens afterward as its own bounded
contract (gf2 one-liner + verified.rs design decision). Quadrant re-runs
after that repair land follow the F1 ledger rules (only invalidated rows).

## Retained artifacts

`results.tsv` (every invocation), `failures.txt` (every failure),
`samples/` (one log per failure family + first green row per command),
`summary.txt`. Test inventories: `tests2-manifest.tsv` +
`tests2-union.txt` (post-R1 full-doctest-identity recapture; the
original collapsed inventories were superseded). Historical full log/test
sets were never committed; see `ARCHIVE-NOTE.md` for the regeneration
recipe.
