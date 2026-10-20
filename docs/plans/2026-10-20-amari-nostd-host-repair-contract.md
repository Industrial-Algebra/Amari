# No-std Host Repair Contract (MATRIX-05 closure; M-H option-C follow-on)

**Branch:** `chore/nostd-repair-contract` from develop `954de7a` · 2026-10-20
**Owner:** matrix-tracked "W00-scope host repair" (MATRIX-05); P-EMBED board
validation stays OUT of scope — this is the host repair only.
**Goal:** turn the 128 M-H RED rows (no-`std` × {`phantom-types`,
`formal-verification`, `gf2`}) green; no behavior change in `std` builds.

## Measured surface (census on `954de7a`)

**S1 — gf2 import (1 line).** `amari-core/src/gf2/grassmannian.rs:48` uses
`vec!` with only `alloc::vec::Vec` imported (line 6). Every sibling module
(`matrix.rs:6`, `vector.rs:4`, `clifford.rs:7`) carries `use alloc::vec;`.
Fix: add the same import. This is the entire gf2-alone failure (1 error per
H-row log; M-H stage-1/2 samples archived).

**S2 — verified.rs no-std repairs (bounded, ~40 sites + 2 path fixes).**
`verified.rs` is reachable under `phantom-types` (lib.rs:71 unconditional
declaration) and compiles nothing without repairs:

- 33 `vec!`/`format!` macro sites, **zero** `alloc` imports today.
- `verified.rs:25` `use std::marker::PhantomData` → `core::marker`.
- `verified.rs:1132` `std::f64::consts::FRAC_1_SQRT_2` → `core::f64::consts`.
- 15 `T: Float + Zero + One` bounds whose method bodies call `.abs()` (8×),
  `.is_finite()` (7×), `.sqrt()` (5×), `.is_nan()`, `.max()`.

**Upstream fact (decisive):** in `num-traits 0.2.19` (`src/float.rs`),
`abs`/`sqrt`/`floor`/… exist ONLY under `cfg(feature = "std")` OR
`cfg(all(not(std), feature = "libm"))` (via the `libm` crate). There is NO
third implementation. So no-`std` `Float` requires `num-traits/libm`.

**S3 — error.rs paths (5 `std::` uses).** Reachable from `gf2`
(`matrix.rs:5` uses `crate::error`). Fix: `core`/`alloc` paths
(`std::fmt` → `core::fmt`, etc.); no semantic change (std re-exports core
for these items).

**S4 — feature wiring (Cargo.toml, 1 edit).**
`phantom-types = []` → `["num-traits/libm"]`. `formal-verification`
already implies `phantom-types` (inherits). `libm 0.2.16` is ALREADY in
the workspace lock (no new resolution; MSRV-compatible — verify in-gate).
In `std` builds `num-traits`' default features keep the std impls; the
libm cfg is inert there (additive, harmless — no unification trap).

**S5 — census discrepancy to resolve in-implementation:** archived M-H
row-33 logs attribute errors to a macro `float_impl_std`; no such
identifier exists in-tree at `954de7a` (rg-verified). The live recompile
is the authoritative site list; the archived samples are the cross-check.
If the macro surfaces (e.g. inside `#[cfg(test)]` code), fix or gate it in
the same bounded pass; do not expand scope beyond the three families.

## R1 correction (2026-10-20) — premises revised after review round 1

The reviewer's findings, all verified empirically (reproduction + scratch
patch on this tree):

1. **error.rs S3 DROPPED**: all 5 `std::` uses sit under
   `#[cfg(feature = "std")]` (`std::error::Error` impls at :375/:395/:416/:434)
   — already correctly gated; they never contributed to the RED rows.
2. **S4 libm wiring DROPPED from this contract (deferred to P-EMBED)**: in
   host `-p amari-core` rows, num-traits' DEFAULT features unify
   `num-traits/std` ON even under `--no-default-features`, so
   `num_traits::Float` impls EXIST and the 15 `T: Float` bounds resolve —
   reproduction shows the lib-check failure set is 7 `vec` + 5 `Vec` +
   1 `std` module (verified.rs:25) + 13 downstream E0308s that are all
   cascades of the unresolved `Vec`. **Proof**: adding `use alloc::vec;
   use alloc::vec::Vec;` + two core-path edits (+ the S1 gf2 import)
   compiles the phantom no-std lib row CLEAN (rc=0, only an
   unused-`format`-import warning — `format!` count in verified.rs is 0,
   so it is excluded). libm becomes genuinely required only where
   num-traits must build WITHOUT std — i.e. P-EMBED targets where the std
   crate does not exist. Recorded there, not here.
3. **S5 RESOLVED**: `float_impl_std` is num-traits 0.2.19's internal
   macro (its `cfg(std)` Float impl generator; present in the vendored
   source and in w01/w02 evidence logs). The archived M-H "originates in
   macro float_impl_std" notes were the std impls working correctly —
   not an in-tree identifier, not a defect.
4. **P3 bound recount**: 15 grep hits for `T: Float` = 12 method-level
   bounds + impl-level duplicates; corrected count in S2 below. The
   bounds themselves need NO changes.

The original D1(a) decision was taken on the false libm premise; the
re-confirmed decision below supersedes it.

## Revised surface (post-R1, empirically proven)

- **S1** `gf2/grassmannian.rs`: add `use alloc::vec;` (one line; siblings
  carry it).
- **S2** `verified.rs`: add `use alloc::vec; use alloc::vec::Vec;` (no
  `format` — zero uses); `use std::marker::PhantomData` → `core::marker`
  (:25); `std::f64::consts::FRAC_1_SQRT_2` → `core::f64::consts` (:1132).
  The 12 method-level `T: Float` bounds are untouched and resolve via
  unified `num-traits/std`. Test/doc targets in the RED rows may carry
  additional test-module import sites — the 128-row re-run is the
  authoritative completeness gate.

## Decision D1 — RE-CONFIRMED (operator, ask_user 2026-10-20): imports-only repair; libm wiring DEFERRED to P-EMBED where num-traits must build without std. Supersedes the original (a) taken on the corrected premise.

## Decision D1 (operator, ask_user) [historical]

- **(a) RECOMMENDED — libm wiring:** S1–S4 as above. Preserves the full
  verified surface for no-std consumers (P-EMBED explicitly wants "GF(2)
  and phantom combinations"). Cost: `libm` crate compiled into no-std
  builds (small, pure-Rust, rust-lang maintained, exactly designed for
  this; already in the lock).
- **(b) std-gating instead:** `#[cfg(feature = "std")]` on the 15
  Float-bound methods (plus their tests). No new dep; but no-std+phantom
  loses the verified constructors — contradicts P-EMBED's stated needs.

## Re-verification plan (F1 ledger: only invalidated rows)

The 128 rows, all three quadrants, using the M-H retained target dirs
(`target/mh-{s1-stable,s2-nightly,s2-msrv}`) and the pinned-runner
discipline (hash-asserted toolchains, fail-closed cfg preflight,
`--locked` — lock gains only the already-present `libm` edge):

1. stable: 128 rows × 4 commands (≈10–25 min warm);
2. nightly: same;
3. MSRV: 128 lib checks;
4. full 320-row sweep re-run once at the end (drift guard: the other 192
   rows must remain green — catches accidental regressions);
5. supplemental singleton rows re-run (the three RED singletons go green);
6. catalog regenerated from the merged superset (feature-table change →
   manifest delta; drift gate `generated_equals_checked_in` green);
7. `std`-build regression gates: amari-core default test row, clippy
   (default + `--all-targets` + feature rows), fmt, rustdoc; root
   workspace `cargo check --locked -p amari-core` default row.

**Expected post-state:** 2,880 + 166 M-H invocations all green; the
no-std pipeline (proven by rows 1–32) extended to all gated modules.

## Exclusions

- P-EMBED target (`thumbv7em-none-eabihf`) cross-compiles and board
  requirements — separate matrix item, unchanged.
- Any new no-std functionality; any behavioral change to `std` builds;
  num-traits version changes; MSRV movement.
- `deterministic` family, SIMD battery, calculus/rewrite crates: untouched
  (their rows already green; re-verified only via the 320-row drift sweep).
