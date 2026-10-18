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
| S1 | `amari-core/src/simd.rs:18` — `geometric_product_3d_avx2` | `unsafe {}` with `_mm256_loadu_pd` ×4 (8×f64 = the full `Multivector<3,0,0>`), **10× `_mm256_fmadd_pd`** | **Feature precondition is {AVX2, FMA}, not AVX2 alone** (R1 finding: `rustc -C target-feature=+avx2 --print cfg` shows `target_feature="fma"` absent; `fmadd` is an FMA intrinsic). Three consequences: (a) the compile-time cfg gate must be `all(target_feature = "avx2", target_feature = "fma")` — under P-AVX2's exact flag the current avx2-only gate either fails to compile the `fmadd` calls or mis-states the contract; (b) the std dispatcher must detect **both** features before returning the AVX2 fn (an AVX2-without-FMA CPU must get the scalar path); (c) the no_std compile-time path takes the same corrected gate. In-bounds **iff** the type guarantees exactly 8 coefficients (2^3, verified: `from_coefficients` asserts `len == BASIS_COUNT`); `loadu` = alignment-free by design. Disposition: REPAIRED (gate + dispatcher) with equivalence battery |
| S2 | `amari-core/src/simd.rs:100` — `batch_geometric_product_avx2` | safe wrapper over S1 | `num_pairs = len/8` **silently truncates partial tails**; no length-multiple assertion; `.to_vec()` per pair (perf smell, not safety) |
| S3 | `amari-core/src/simd.rs:131` — `select_geometric_product_impl` | runtime `is_x86_feature_detected!` + compile-time cfg | Correct hybrid pattern; must verify the no-AVX2 build compiles the scalar path and the AVX2 build gates on runtime detection |
| S4 | `amari-core/src/aligned_alloc.rs` — `alloc` / `NonNull::new_unchecked` / `dealloc` | manual allocation with layout | **ZERO users** (`pub(crate)` module, declared `lib.rs:25`, never referenced). Candidate REMOVAL; if retained, needs layout/Drop invariants argued + sanitizer evidence |
| S5 | `amari-calculus/src/manifold/metric.rs` — 4× `transmute_copy` | array reinterpret between dimension layouts (e.g. `[[ScalarField;2];2]` → `[[ScalarField;DIM];DIM]` under a `DIM==2` guard) | **Correction (R1):** `transmute_copy`'s size check is a **runtime panic**, not compile-time, and the operation **duplicates ownership bitwise** — the source is dropped at scope end while the copy lives on. Soundness therefore requires the SAME field-closure argument as S6: `ScalarField` = `fn(&[f64])->f64 + usize`, no heap, **no drop glue** — verified. Retention requires, per site: the DIM guard, a matching-layout argument (src/dst element counts equal by construction in each branch), and the no-owning-state/no-Drop argument written at the site. Disposition: DOCUMENTED with these arguments, or REPLACED by safe construction if the operator prefers |
| S6 | `amari-calculus/src/manifold/connection.rs:27,87` — 2× `ptr::read` "clone hack" | bitwise copy with original retained | **Sound today**: `MetricTensor` = `[[ScalarField; DIM]; DIM]`, `ScalarField` = `fn(&[f64]) -> f64 + usize` — no heap, no Drop ⇒ no double-drop. Fragile: the SAFETY comment admits "hack"; adding any owning field makes it UB. Replace with proper `Clone` |

| S7 | root `amari` crate `src/deterministic/` — `pub unsafe trait Deterministic` (`scalar.rs:45`) + 4 `unsafe impl`s (`DetF32`, `DetVector2`, `DetRotor2`, one more per census) | marker-trait unsafety — **attestation, not execution** | The trait's `# Safety` contract demands bit-exact reproducibility across platforms/compilers/opt-levels; each `unsafe impl` ATTESTS that guarantee. Audit question: is the attestation justified for each type's full operation surface (e.g. `DetF32` arithmetic inheriting f32 non-associativity is fine for determinism-per-identical-inputs, but cross-opt-level bit-exactness of `from_bits`/ops must hold)? Disposition: VERIFIED-SOUND with per-type argument or trait-scope REPAIR | (R1: this crate was entirely missing from my census — root `src/`, not `*/src/`) |
| S8 | `amari-rewrite/tests/language_metered.rs:367-377` — test-local `MaxSingleAlloc` `unsafe impl GlobalAlloc` + delegating `alloc`/`dealloc` | test-only allocator proxy over `System` | Delegation preserves `GlobalAlloc`'s contract (same layout forwarded both directions; `fetch_max`/atomics are safe); test scope means no production exposure. Disposition: VERIFIED-SOUND with the delegation argument written at the site | (R1: tests/ were outside my census scope) |

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
- **D5 (batteries) — frozen acceptance policy (R1):**
  **Tier 1, SIMD-vs-scalar (both under +avx2,+fma):** where both results are
  finite, require `|simd − scalar| ≤ max(8 ulp of scalar, 1e-13 · |scalar|)`
  (8 ulp headroom for FMA reassociation; tighten if achieved). Where either
  is nonfinite, require **identical finiteness class** (both-NaN, or both
  ±inf with matching sign when the exact value overflows) — bit-identity of
  NaN payloads NOT required.
  **Tier 2, scalar-vs-oracle:** only where scalar is finite — the established
  W03 exact-Fraction regime (Fraction-rounded-once comparison within the
  documented tolerance). Where scalar is nonfinite and the exact value is
  finite and representable-class (e.g. `MAX·2 + MAX·(−2)`: exact 0, float
  NaN via inf−inf), record as **EXPECTED-DIVERGENT** in the adversarial set —
  an inherent floating-point behavior shared by both paths, not a SIMD
  finding; escalation to P1 only when SIMD and scalar *differ* in class.
  Adversarial sets: zeros, subnormal-spread, ±MAX mixes, cancellation-heavy
  products, all 64 basis-pair products, seeded randomized dense draws (≥10⁴).
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

*Inventory commands (corrected R1 — extended regex, root + member src/ AND
tests/benches/examples/build.rs):*
`rg -n -E '\bunsafe\b' src/ */src/ */tests/ */benches/ */examples/`
`rg -ln -E 'std::arch|target_feature\(enable|_mm256|_mm_|is_x86_feature_detected|__m256|__m128' src/ */src/`
`rg -n -E 'transmute|from_raw_parts|\.set_len|get_unchecked|assume_aligned' src/ */src/ */tests/`
(comment lines excluded; false-positive dispositions in §1.2). Coverage
statement: the census universe is every `.rs` file compiled by the workspace
member graph plus the root crate, production and test targets alike.
