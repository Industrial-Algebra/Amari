# M-REP-D — unsafe/SIMD Audit Contract

**Date drafted:** 2026-10-17
**Branch:** `chore/mrep-d-contract` (from develop `dedd1f6` + #314)
**Matrix mandate:** M-REP-D completion is the **eligibility prerequisite for
P-AVX2 execution** (verification matrix §5); the operator-approved doctrine
governing it: *no safety certification follows from green ordinary tests;
allocation/alignment/intrinsics preconditions reviewed first; Miri/sanitizer
evidence archived separately; Miri cannot be assumed to validate unsupported
SIMD code.*

---

## 1. Measured surface (audit-first; develop `dedd1f6`)

Exhaustive `unsafe`/intrinsics census over workspace `src/`, with
false-positives dispositioned:

### 1.1 Genuine unsafe execution sites (the audit surface — 5 files, 3 crates)

| # | Site | Kind | Preliminary reading |
|---|---|---|---|
| S1 | `amari-core/src/simd.rs:18` — `geometric_product_3d_avx2` | `unsafe {}` with `_mm256_loadu_pd` ×4 (8×f64 = the full `Multivector<3,0,0>`), FMA-class arithmetic | In-bounds **iff** the type guarantees exactly 8 coefficients (2^3); `loadu` = alignment-free by design. Requires: type-invariant proof (construction paths), arithmetic equivalence battery |
| S2 | `amari-core/src/simd.rs:100` — `batch_geometric_product_avx2` | safe wrapper over S1 | `num_pairs = len/8` **silently truncates partial tails**; no length-multiple assertion; `.to_vec()` per pair (perf smell, not safety) |
| S3 | `amari-core/src/simd.rs:131` — `select_geometric_product_impl` | runtime `is_x86_feature_detected!` + compile-time cfg | Correct hybrid pattern; must verify the no-AVX2 build compiles the scalar path and the AVX2 build gates on runtime detection |
| S4 | `amari-core/src/aligned_alloc.rs` — `alloc` / `NonNull::new_unchecked` / `dealloc` | manual allocation with layout | **ZERO users** (`pub(crate)` module, declared `lib.rs:25`, never referenced). Candidate REMOVAL; if retained, needs layout/Drop invariants argued + sanitizer evidence |
| S5 | `amari-calculus/src/manifold/metric.rs` — 4× `transmute_copy` | array reinterpret between dimension layouts | Size relation compile-checked by `transmute_copy`'s size assert; validity trivial (plain arrays) — but each site's size/validity invariant must be documented at the site |
| S6 | `amari-calculus/src/manifold/connection.rs:27,87` — 2× `ptr::read` "clone hack" | bitwise copy with original retained | **Sound today**: `MetricTensor` = `[[ScalarField; DIM]; DIM]`, `ScalarField` = `fn(&[f64]) -> f64 + usize` — no heap, no Drop ⇒ no double-drop. Fragile: the SAFETY comment admits "hack"; adding any owning field makes it UB. Replace with proper `Clone` |

### 1.2 Audited, not unsafe code (excluded from the surface, recorded here)

- `amari-discovery` `unsafe_trait: bool` catalog-metadata fields (5+2+1
  occurrences) — data *about* trait unsafety, not unsafe code.
- `amari-discovery/src/inspect/rust/parser.rs:735,740` — `#![forbid(unsafe_code)]`
  inside a **test fixture string**.
- `amari-gpu/src/enumerative.rs` — the 5 grep hits are the `subset_len`
  **field name** (substring of `set_len`); host code contains no unsafe;
  compute is WGSL-side. GPU crate excluded from this audit.

### 1.3 Dead-but-public SIMD infrastructure

- `amari-core/src/simd.rs:150` `AlignedBuffer<N>` (`#[repr(C, align(32))]`) —
  safe code, **zero users**; overlaps S4's purpose. Candidate REMOVAL or
  adoption by the SIMD paths (see §3 D6).

## 2. Audit protocol — per-site dispositions

Every site in §1.1 receives exactly one disposition, with evidence:

- **VERIFIED-SOUND** — a written safety argument covering in-bounds,
  alignment, validity, and lifetime, plus a test that would catch its
  violation where feasible.
- **REPAIRED** — minimal change preserving semantics (e.g. S6 proper Clone;
  S2 length contract).
- **REPLACED** — safe alternative with identical semantics.
- **REMOVED** — dead code with zero users (S4; possibly `AlignedBuffer`),
  operator-approved.
- **DOCUMENTED** — contract tightening at the API boundary (S2 truncation
  semantics; S1 coefficient-count invariant).

No site may close on "tests pass" alone — the matrix doctrine. Where a
safety argument depends on a type invariant (S1's 8-coefficient guarantee),
the invariant is proven from construction paths or the site is repaired to
check it.

## 3. Decision points for the operator

- **D1 (S4):** remove `aligned_alloc` module (zero users) — recommended —
  or retain + full invariants/sanitizer work.
- **D2 (S6):** replace `ptr::read` hacks with `impl Clone` for
  `MetricTensor`/connection (semantics-preserving; removes the fragility) —
  recommended.
- **D3 (S2):** `batch_geometric_product_avx2` truncation — assert
  length-multiple (fail fast) vs document — recommendation: **assert**;
  silent partial results are a correctness hazard.
- **D4 (S5):** keep `transmute_copy` with documented size/validity
  invariants at each site, vs rewrite as safe index copies — recommendation:
  keep + document (rewrites churn proof-carrying code for zero gain).
- **D5 (batteries):** AVX2-vs-scalar equivalence methodology — bounded-ULP
  comparison **plus** the W03 exact-Fraction oracle adjudicating both paths
  (accumulation order differs ⇒ bit-identity is NOT promised; the oracle
  decides the true value and both paths must sit within the established
  tolerance of it). Adversarial sets: zeros, subnormal-spread, ±MAX mixes,
  cancellation-heavy products, all basis-vector pairs, randomized dense
  draws (seeded, ≥10⁴ cases).
- **D6 (S3/§1.3):** leave dispatcher and `AlignedBuffer` as-is (documented)
  vs adopt aligned buffers in the SIMD hot path — recommendation: **leave**;
  `loadu` needs no alignment; perf claims wait for M-H data.

## 4. Verification battery (acceptance criteria)

1. **Safety review document** — per-site dispositions with arguments,
   archived at `docs/plans/core-matrix-evidence/mrep-d/`.
2. **Equivalence battery** — S1/S2 vs scalar vs exact-Fraction oracle over
   the D5 sets; tolerances recorded; failures are findings, not tuning
   targets.
3. **Miri** — run the crate's test suite under Miri on the AVX2-enabled
   build where intrinsics are supported; unsupported-intrinsic cases
   documented as such (per matrix: no assumption either way).
4. **Sanitizers** — ASan/LSan on the alloc-bearing build **if** S4 is
   retained; if removed, recorded not-applicable.
5. **Both-feature matrix** — clippy `-D warnings` and tests for
   `RUSTFLAGS='-C target-feature=+avx2'` AND the existing `-avx2` pinned
   recipe; MSRV row on both.
6. **Runtime SIMD inventory** — a test that reports which implementation
   `select_geometric_product_impl` returns under each build, so CI evidence
   distinguishes compiled-in vs dispatched paths (P-AVX2's "runtime SIMD
   inventory" requirement).
7. Named suites and catalog unaffected (no public API change unless D2/D3
   choose repair; if changed, catalog regen per protocol).

## 5. P-AVX2 eligibility statement (this package's exit)

On completion: the §5 P-AVX2 row becomes executable — representative
configurations compiled and run under explicit `+avx2`, inventory +
independent arithmetic checks recorded. M-REP-D does **not** itself assert
performance claims (M-H territory), and does not certify safety beyond the
argued dispositions above.

## 6. Out of scope

amari-gpu (WGSL compute — separate surface, separate audit if ever needed);
discovery metadata; any new SIMD paths (this is an audit of existing ones);
performance optimization.

---

*Inventory commands (reproducible):* `grep -rn 'unsafe' --include='*.rs' */src/`,
`grep -rln 'std::arch|target_feature(enable|_mm256|_mm_|is_x86_feature_detected|__m256|__m128'`,
`grep -rn 'transmute|from_raw_parts|set_len|get_unchecked|assume_aligned'`
(comment lines excluded; false-positive dispositions in §1.2).
