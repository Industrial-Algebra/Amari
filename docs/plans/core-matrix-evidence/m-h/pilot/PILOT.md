# M-H Pilot — Wall-Clock and Storage Estimate

**Date:** 2026-10-18 · **Host:** 24-core x86-64, 89 GB RAM, NVMe (3.6 TB free) ·
**Tree:** develop `6463630` (#319 merged) · **Worktree:** `.worktrees/core-matrix`
on `chore/m-h-pilot`

Purpose (matrix §4, M-H cost and scoping): record the pilot-derived
wall-clock/storage estimate **before** the operator chooses the execution
option (A monolithic / B staged / C predicate-pruned). This pilot measured
representative rows under the exact §3.2 `baseline_cargo` wrapper discipline
(baseline ISA `-C target-cpu=x86-64 -C target-feature=-avx2`, explicit-empty
wrappers, pinned `RUSTC`/`RUSTDOC`, per-compiler artifact directory); cfg
assertions archived (`cfg-{stable,nightly,msrv}.txt`, AVX/AVX2/FMA absent);
nightly hash verified against the researched pin
(`787af2b8c80638c51a4fc8e44f84e6891f243ec7`, `1.100.0-nightly 2026-08-25`).

## Method

- The 320-row canonical family archived verbatim from the §3.1 generator:
  `matrix-rows.txt`.
- Four representative rows spanning the cost classes: `EMPTY` (no features),
  `FULL` (`std,formal-verification,gf2,high-precision,native-precision,rayon,serde`),
  `MID1` (`std,phantom-types,high-precision`), `MID2`
  (`std,gf2,creusot-contracts,native-precision,rayon`).
- Per compiler: fresh target dir; all four H commands per row
  (check/test/clippy/doc — MSRV mode: `check --lib` only, per the 320-row
  MSRV library-check column). `--locked` throughout; the worktree lock was
  regenerated once under the baseline env (untracked lock, repo policy).
- Copy-executable harness: `run-pilot.sh`; raw timings `timings-*.tsv`;
  command logs `log-*.txt` (test execution sanity: **1,908 tests, 0 failed
  on stable AND nightly across the 4 rows; MSRV 0 errors**).

## Measured per-row wall-clock (all four commands, seconds)

| Row | stable 1.98.0 | nightly pinned | MSRV 1.89.0 (check only) |
|---|---|---|---|
| EMPTY (cold dir: dep compile) | 15.8 | 16.4 | 2.3 |
| FULL (new-dep closure) | 17.9 | 18.8 | 4.5 |
| MID1 (warm steady state) | 5.6 | 4.8 | 0.4 |
| MID2 (warm steady state) | 5.4 | 4.9 | 0.4 |

Steady-state row cost is dominated by amari-core's own rebuild ×4 commands
plus test execution; dev-dependency compilation (criterion et al.) is paid
once per compiler (visible in the EMPTY cold row).

## Extrapolation to full H (320 rows)

Wall-clock per compiler = cold start + 319 × row cost, bracketed
[MID steady, FULL] (conservative upper bracket treats every row as a
new-dep-closure row; no cross-row parallelism assumed — matrix discipline
is sequential rows, cargo's internal 24-core parallelism only):

- **stable:** 15.8 + 319×[5.4 .. 17.9] s = **30 .. 96 min**
- **nightly:** 16.4 + 319×[4.8 .. 18.8] s = **27 .. 100 min**
- **MSRV (320 lib checks):** 2.3 + 319×[0.4 .. 4.5] s = **4 .. 24 min**
- **Total (sequential compilers): ≈ 1 h .. 3.7 h.** Running the three
  compiler quadrants concurrently (independent target dirs) bounds it by
  the nightly quadrant: **≈ 0.5 .. 1.7 h.**

Storage: test artifacts dominate and are per-feature-fingerprint (no reuse
across rows): ~350–560 MB per row observed on the stable/nightly dirs
(some GC shrinkage observed on `doc`; not relied on). Full-H per-compiler
target dir ≈ 320 × 400 MB ≈ **~130 GB** (MSRV column ~3 GB). **Peak across
all quadrants ≈ ~260 GB** — fits the 3.6 TB volume with wide margin; an
explicit per-quadrant size checkpoint is still recommended so the estimate
can be re-based if growth is superlinear.

## Caveats

1. Four rows sample the cost classes but not every dep-fingerprint corner
   (e.g. serde+rayon+phantom+contracts simultaneously); the FULL bracket
   bounds new-dep compile cost.
2. Timings are from one host, one disk state, sequential; CI/other hosts
   will differ. This estimate is for *local execution planning*.
3. Doctest execution is included in `test` rows (matrix §3.2: full
   `cargo test` includes unit/integration/doctests and builds examples).
4. The named-suite filter-integrity rule (F2) and per-row test-name
   manifests add harness bookkeeping cost not included here (scripted,
   amortized; expect minutes, not hours).
5. `--all-targets` covers benches compile-only; benchmark *timings* are
   out of scope for H.

## Recommendation

**Option B (staged), as the matrix's own default proposal.** The measured
cost makes even (A) monolithic tractable (~1–3.7 h, ~260 GB peak), so B is
not chosen for cost but for evidence quality: stable-green first, then
nightly + MSRV quadrants pinned to that exact revision, per-quadrant
archives, and re-running only quadrants invalidated by develop movement
per the F1 ledger rules. Option (C) predicate-pruned would save the least
(its equivalence-class justification map is itself work) and carries the
serde/rayon dependency-distinctness trap the matrix already flagged.

## Operator decision

Recorded separately (ask_user, 2026-10-18): option **B** approved as
recommended — staged: stable full-H (320×4) first; on stable-green,
nightly full-H (320×4) + MSRV (320 lib checks) on the same revision;
per-quadrant evidence archives under `core-matrix-evidence/m-h/`;
quadrant re-runs only per F1 ledger rules.
