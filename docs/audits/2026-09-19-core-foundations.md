# amari-core audit — batch 1: original foundations

- **Date:** 2026-09-19
- **Auditor:** coding-agent source review plus independently implemented executable oracle
- **Baseline:** `165d7ccbd6d1aa899babb2d0ae392fa15272e556` (`origin/develop` when fetched)
- **Package version:** 0.24.1 (concurrent 0.25 development)
- **Disposition:** **Changes required. This is not an approval of amari-core.**

No production source was changed. Scope is the current versions of the original September 9, 2025 files: `basis.rs`, `cayley.rs`, `lib.rs`, `rotor.rs`, manifest and basic example. Supporting product, rotor and earlier audit integration tests were read in full. [Ledger](README.md) identifies everything not yet reviewed. GPU/WASM implementations are excluded by maintainer direction.

The previous [0.19 audit](../1.0-audit.md) is useful historical evidence, not a correctness certification. It explicitly deferred wrapper-field privacy, fallible constructors and benchmarks. This pass additionally demonstrates mathematical defects beyond that audit's tests.

## Executive result

- Existing default suite: **284 unit/integration tests + 3 doctests passed**, none ignored.
- Independent basis-word oracle: **4,589 blade pairs across all 35 signatures with P+Q+R ≤ 4 passed**. The oracle also checks inner/outer products, contractions, reversion and nondegenerate double-Hodge identities on those basis inputs. This is bounded exhaustive testing, not a proof for arbitrary dimension or floating-point coefficients.
- New law/contract suite: **21 expected failing tests**, grouped into **15 findings** below. Tests assert the mathematically expected or documented behavior, not current incorrect behavior. Some share underlying causes and must not be counted as independent root defects.
- The multiplication table's sign/metric rules pass these checks; major errors occur in higher-level operations and coefficient-threshold policies.
- Default formatting and warning-denied rustdoc pass. Default warning-denied Clippy, no-default compilation and std-only full tests fail for pre-existing reasons described below.
- Dedicated phantom/formal modules, unsafe allocation/SIMD, precision and GF(2) source audits are **still pending**.

All source locations below refer to `amari-core/` at the pinned commit. Reproductions are in [the standalone fixture](core-foundations-repro/tests/foundations.rs); `finding_NN` maps to `CORE-NN`.

Severity: P1 = incorrect results on supported mathematical inputs or broken core invariants; P2 = domain/API validation or documentation contract requiring clarification. No P0 claim is made.

## Confirmed findings

### CORE-01 — P1: reverse/scalar-norm is not a general multivector inverse

**Location:** `src/lib.rs:513–522`.

`inverse()` computes `reverse(A) / <A reverse(A)>₀` without checking whether `A reverse(A)` is scalar. That identity is valid for appropriate blades/versors, not arbitrary multivectors.

- In Cl(3,0,0), `A = 2 + e1` is invertible with inverse `(2 - e1)/3`. Current output is `(2 + e1)/5`; multiplying it by A yields `1 + 0.8 e1`, not 1.
- `A = 1 + e1` is a zero divisor: `(1 + e1)(1 - e1) = 0`. Current `inverse()` nevertheless returns `Some((1 + e1)/2)`.

**Follow-up:** retain a rigorously checked fast path for the restricted domain and provide a correct general inversion path (or explicitly restrict/name the API). Verify both left and right residuals using coefficient comparisons and distinguish singularity from numerical failure. This is an API/numerical-design decision, not permission to silently return `None` for all mixed-grade inputs.

### CORE-02 — P1: absolute coefficient pruning changes the algebra

**Location:** `src/lib.rs:295–310`; related `455–456`, `660–661`.

Every geometric product discards input coefficients with magnitude below `1e-14`, before considering the other operand. This is not ordinary floating-point rounding:

- Scalar multivectors `1e-15 * 1e15` produce **0**, expected **1**.
- For scalars `a=b=1e-8`, `c=1e16`, `(a*b)*c = 0`, while `a*(b*c) = 1`.

The same absolute-zero policy affects grade selection and higher products, while Hodge dual explicitly prunes small coefficients. Small terms cannot be dropped based only on their own scale inside fundamental algebra operations.

**Follow-up:** use exact structural zero skipping in arithmetic; make approximate pruning an explicit caller-controlled operation with an error policy. Test identity, bilinearity and associativity across scales, not just O(1) coefficients.

### CORE-03 — P1: exponential deletes nonzero nilpotent bivectors

**Location:** `src/lib.rs:537–544`.

In Cl(1,0,1), `B=e12` is nonzero and `B²=0`. The series terminates exactly: `exp(B)=1+B`. Current code sees a zero squared norm and returns **1**.

This loses the generator of parabolic/projective transformations; it is not a harmless small-angle approximation.

**Follow-up:** distinguish a zero element from a nonzero square-zero generator and implement the nilpotent limit. Share this correction with rotor construction tests rather than patching rotor outputs separately.

### CORE-04 — P1: the bivector exponential assumes every square is scalar

**Location:** `src/lib.rs:535–552`; propagation through `src/rotor.rs:19–38`.

In Cl(4,0,0), `B=e12+e34` is a valid, non-simple bivector. Its square is `-2+2e1234`, but `.scalar_part()` discards the grade-four term.

Because e12 and e34 commute,

`exp(B) = cos²(1) + sin(1)cos(1)(e12+e34) + sin²(1)e1234`.

Current output has scalar `0.15594369476537437` instead of `0.2919265817264289` and no grade-four component. Scalar normalization afterward cannot turn this into the correct rotor.

**Follow-up:** only use the scalar-square closed form after checking its precondition. Otherwise use a verified general exponential/decomposition, or expose an explicitly restricted simple-plane type. Test the full `R reverse(R)=1` multivector identity and grade-preserving action, not just `norm(R)=1`.

### CORE-05 — P1: indefinite magnitude is used as a grade-membership test

**Location:** `src/lib.rs:529–533`; related convergence check `568`.

For `v=e1+e2` in Cl(1,1,0), `v²=0` but v is not a bivector or zero. `(v - grade2(v)).norm()` is zero, so `exp()` takes the pure-bivector branch and returns identity instead of the exact `1+v`.

The general-multivector fallback exists in the implementation, but its dispatch excludes valid null inputs incorrectly. The fallback's stopping criterion also uses this indefinite quantity.

**Follow-up:** use coefficient/grade-support checks for structural membership and a positive-definite coefficient error measure for convergence. If general exponential inputs are not to be supported, narrow the public contract explicitly; do not classify them by indefinite length.

### CORE-06 — P2: general exponential silently truncates after degree 19

**Location:** `src/lib.rs:558–574`.

The implemented general-multivector fallback returns without a convergence status after `for n in 1..20`.

`Multivector::<3,0,0>::scalar(20).exp()` returns `228152458.75893366`, versus `exp(20)=485165195.4097903` (about 53% low). This is finite, moderate input; there is no overflow involved.

**Follow-up:** decide the supported general-exponential domain (public documentation currently emphasizes bivectors). For a general API, use scaling/error control and report failure to converge instead of returning an unqualified approximation.

### CORE-07 — P1: approximate equality identifies nonzero null elements with zero

**Location:** `src/lib.rs:460–481`, `498–500`.

`approx_eq()` tests the indefinite magnitude of the difference. In Cl(1,1,0), `e1+e2` compares equal to zero at epsilon `1e-12`. This can mask incorrect results in downstream algorithms and tests.

The magnitude documentation also asserts definiteness and submultiplicativity without qualification. Definiteness fails on nonzero null elements. Even in Euclidean Cl(3,0,0), setting `A=1+e1` gives `|A²|=2√2 > |A|²=2`, so the claimed constant-one submultiplicativity does not hold for this coefficient norm on arbitrary multivectors.

**Follow-up:** distinguish metric quadratic form, coefficient norm/error and versor normalization. Preserve signed metric semantics where needed; do not “fix” this by globally replacing the metric. Approximate equality must compare coefficients with an explicit tolerance policy.

### CORE-08 — P2: degenerate Hodge behavior contradicts its documented convention

**Location:** `src/lib.rs:647–649`, `679–699`.

Docs promise a permutation-only formal complement for R>0. Implementation instead multiplies by the metric and annihilates blades containing null generators.

In Cl(2,0,1), `star(e3)` is **zero**, whereas the promised formal complement is **e12**.

**Follow-up:** choose and name the intended operation (metric-induced degenerate map versus Poincaré/formal complement). Both have mathematical uses; they are not interchangeable. The reproduction checks the current documented promise, not a claim that an invertible metric Hodge star exists for a degenerate metric.

### CORE-09 — P1: a library method constructs a wrongly graded wrapper

**Location:** `src/lib.rs:989–995`; related public fields `807`, `839`, `1001`.

`Vector::<4,0,0>::e1().hodge_dual()` returns a **Bivector** containing the grade-three blade e234. Correct Hodge grade is n−1, which equals 2 only in dimension 3. This happens without a caller forging/mutating anything.

Additionally, `Scalar`, `Vector` and `Bivector` expose `pub mv`, so callers can freely bypass every grade claim. The older audit already deferred that field-privacy issue; the method-created invalid value is additional evidence.

**Follow-up:** constrain genuinely 3D methods or return the appropriate grade/general multivector. Private representation plus checked construction must back any phantom/grade guarantee. Adding marker parameters while keeping unchecked mutable storage would not solve this.

### CORE-10 — P1: generic SLERP hard-codes eight coefficients

**Location:** `src/rotor.rs:146–205`.

- `Rotor::<2,0,0>::identity().slerp(&identity, 0.5)` panics at line 151 (length 4, index 4).
- For a Cl(4,0,0) rotor generated by e34 with angle 1, `identity.slerp(&r, 1.0)` returns identity rather than r: coefficient 12 is outside every interpolation loop.

**Follow-up:** specialize quaternion-style SLERP to its actual domain, or implement group-appropriate interpolation for supported signatures/dimensions. Merely changing eight to `BASIS_COUNT` does not establish that quaternion interpolation preserves higher-dimensional Spin-group constraints.

### CORE-11 — P1: rotor logarithm/power loses boosts, small rotations and the double cover

**Location:** `src/rotor.rs:237–274`.

Three independently executed `R.power(1) = R` checks fail:

- Cl(1,1,0), generator e12, parameter 1: scalar `cosh(0.5)=1.127625965206381` is clamped to 1, logarithm becomes zero, power becomes identity.
- Cl(3,0,0), rotation angle `1e-8`: scalar rounds to 1; `acos(1)=0` drops the still-resolvable bivector coefficient `-5e-9`.
- Cl(3,0,0), angle 2π: stored rotor is −1 (as the existing double-cover test requires), but `power(1)` returns +1.

The last pair represents the same vector rotation but different rotor elements; the API exposes the latter and claims algebraic exponentiation. The logarithm at −1 is branch-ambiguous, not identically zero.

**Follow-up:** domain-aware and numerically stable logarithms, explicit branch policy, and exact identity/integer-power laws. Multi-plane rotor logarithms require separate treatment as well; this pass does not certify them.

### CORE-12 — P1: from-vectors rotor formula assumes positive unit squares

**Location:** `src/rotor.rs:113–121`.

Cl(0,2,0) supplies two ordinary negative-unit basis vectors. `from_vectors(e1,e2)` returns a rotor whose action maps e1 to **−e2**, not e2. `from_vectors(e1,e1)` returns **None**, although identity is a solution.

The formula `normalize(1 + b*a)` uses `a*a=+1` without checking that assumption. Normalizing by absolute metric length does not change a negative square into a positive one.

**Follow-up:** a metric-aware construction with causal/domain checks, or a positive-definite specialization with explicit restrictions. Also document the antipodal case; the existing formula alone cannot resolve it uniquely.

### CORE-13 — P1 contract mismatch: documented hyperplane reflection implements line reflection

**Location:** `src/rotor.rs:280–287`; test `324–337`.

Docs specify reflection through a hyperplane defined by a unit normal n. In Euclidean space, that should negate the normal and preserve tangent vectors. Current `n*v*n` instead preserves n and negates perpendicular vectors.

For `v=n=e1`, actual is **+e1**, expected **−e1**. The existing unit test enshrines the opposite convention (reflection about the normal line).

**Follow-up:** maintainer must choose whether to correct the hyperplane behavior or rename/document the current line reflection and provide a separate hyperplane operation. For general signatures use the appropriate inverse/sign, and for arbitrary multivectors specify the outermorphism/grade-involution convention. Do not apply a global minus sign without defining the supported input grades.

### CORE-14 — P2: raw rotor construction cannot enforce rotor invariants

**Location:** `src/rotor.rs:19–38`, `55–59`, `97–100`.

`from_multivector_bivector()` relies on the caller to supply an actual bivector. Supplying e1 in Cl(3,0,0) produces a `Rotor` with vector coefficient approximately `-0.4194911955787122`, violating its own even-grade definition. Typed `Bivector` input is also forgeable through `pub mv`.

This reproduction intentionally violates the argument's prose precondition: it demonstrates a **validation/type-boundary gap**, not a wrong exponential on valid bivector input. Normalization of a scalar norm does not establish membership in the rotor group, and `apply_to_vector` projects away unwanted grades rather than reporting a violated invariant.

**Follow-up:** checked constructors, private representations, and explicit failure instead of silent identity fallback. Review the dedicated `VerifiedRotor` separately in the next cohort; its name is not evidence it closes this gap.

### CORE-15 — P2: generic component APIs silently retain a 3D-only layout

**Location:** `src/lib.rs:210–219`, `1057–1063`, `1100–1109`; related 3D methods `src/rotor.rs:126–142`, `213–223`.

`set_bivector_component` uses `[3,5,6]` for every dimension. In Cl(4,0,0), which has six bivector components, setting component 3 to 1 silently does nothing. Smaller dimensions can instead index outside the allocation. Bivector `get`/indexing has the same fixed layout; matrix and axis-angle helpers likewise assume 3D.

**Follow-up:** explicitly named/dimension-constrained 3D conveniences plus a checked dimension-general component API. Specify blade-index versus component-index ordering; do not let invalid requests silently become zero.

## Rust/type/API and performance observations

These are source-established findings/recommendations, not benchmark claims or extra mathematically independent failures.

### PERF-01 — P2: Cayley “caching” rebuilds the full table for every product

`src/cayley.rs:1`, `32–48`; `src/lib.rs:295–296`.

`get()` calls `generate()` every time and returns an owned table, despite the module's caching description. For n=P+Q+R it builds 4ⁿ entries, each with an O(n²) sign calculation. On this 64-bit layout `(f64,usize)` is 16 bytes: n=10 requires a 16 MiB table for each call, even scalar×scalar. Inner/outer/contraction code repeats this construction for active grade pairs.

**Proposal, not an approved redesign:** benchmark reusable immutable tables versus on-demand blade products, with separate dense/sparse and dimension regimes. Bound resource growth and account for `no_std` before choosing a cache. No speedup factor is claimed from static analysis.

### TYPE/API-01 — P2: signature separation works; grade and dimension guarantees do not

The same-signature const-generic operands are a useful compile-time boundary. However, public wrapper storage allows `Vector { mv: scalar(1) }`, etc. `basis.rs:60–71` and `lib.rs:125–143` expose panic-based constructors without fallible alternatives; the older audit deferred this migration.

`basis.rs:15–30` does not validate name indices/dimensions. `cayley.rs:53–54` indexes a flattened array without independently bounding i and j (e.g. j equal to the basis count aliases the next row when the combined index is in range). `lib.rs:154–164` silently converts invalid reads/writes to zero/no-op. This mixture of panic, silent fallback and partial validation makes mistakes hard to diagnose.

Use private newtypes/grade markers for actual invariants and checked raw-input boundaries, not phantom types as decoration. Choose a compatible migration rather than changing all public APIs during an audit.

### PERF/API-02 — P2: allocation and layout claims need evidence

- `src/lib.rs:83–87`: `repr(align(32))` aligns the `Box`-holding struct, **not the heap coefficient buffer**. It is not an AVX2-alignment guarantee for `as_slice().as_ptr()`. SIMD safety itself remains pending the dedicated unsafe audit.
- `src/lib.rs:318–358`, `440–452`, `580–637`: grade decomposition/projection repeatedly allocates full-size dense buffers; `vector_part` and `bivector_type` also project twice through wrapper conversions.
- `src/lib.rs:460–461`: scalar norm is computed via a complete geometric product/table rebuild, though its scalar contraction can be computed directly from diagonal blade weights.
- `lib.rs` is 1,246 lines mixing representation, kernels, numeric policy and wrappers. Extracting those concepts may help, but only after the mathematical contracts and regression gates are established.

### DOC/TEST-01 — P2: tests and documentation overstate coverage

- `src/lib.rs:1142` checks `abs(actual) - 1 < epsilon`, which also accepts zero and the wrong sign; it does not test distance from +1.
- `src/lib.rs:1177` compares norms of the two associativity results rather than coefficients. Distinct answers can have equal norms.
- `tests/products.rs:175–188` computes bivector products but contains no assertion.
- `tests/geometric_product.rs:134–145` has an incorrect anticommutation comment/TODO: disjoint grade-1 and grade-2 blades commute, since two swaps contribute +1. Current result is correct, but the comment should be replaced by an independent exact expectation.
- `tests/audit_tests.rs:110–167` describes a “complete 8×8” Cayley verification but checks only 11 selected pairs; the new oracle actually enumerates them and additional signatures.
- `tests/geometric_product.rs:221–236` uses a hard 100 ms wall-clock threshold and discards results without `black_box`; it is not a reliable performance benchmark.
- Crate-level docs omit a feature catalogue; many public wrapper methods lack docs, examples and error/panic contracts. None of the four audited production source files has an SPDX header. Do not silently change the existing `MIT OR Apache-2.0` licensing choice during cleanup.
- Default `cargo doc -D warnings` succeeding does not detect missing public docs unless the relevant lint is enabled, and is not documentation-completeness evidence.

## Verification evidence and limits

Toolchain: `rustc 1.100.0-nightly (787af2b8c 2026-08-25)`, host `x86_64-unknown-linux-gnu`; Cargo `1.100.0-nightly (e8cb624d5 2026-08-22)`.

Commands run from the audit worktree, with raw output retained in [core-foundations-evidence](core-foundations-evidence/):

| Command | Result |
|---|---|
| `cargo test --locked --offline -p amari-core` | PASS: 202 unit + 82 integration + 3 doc tests |
| `cargo fmt -p amari-core --check` | PASS |
| `RUSTDOCFLAGS='-D warnings' cargo doc --locked --offline -p amari-core --no-deps` | PASS |
| `cargo run --locked --offline -p amari-core --example basic` | PASS; printed 3D rotations/composition agree with expected output |
| `cargo clippy --locked --offline -p amari-core --all-targets -- -D warnings` | FAIL: three `needless_range_loop` diagnostics, `verified.rs:159,339,387` |
| `cargo check --locked --offline -p amari-core --no-default-features` | FAIL: missing `vec!`/`Vec` imports at `generic.rs:79,76`, missing `Vec` at `rotor.rs:215`; precision unused-import warning |
| `cargo test --locked --offline -p amari-core --no-default-features --features std` | FAIL: `tests/gf2_tests.rs:3` imports feature-gated module without gating the test; precision unused-import warning |
| Standalone fixture, debug and release | 1 oracle test passes; 21 law/contract probes fail as documented; no ignored tests |

`Cargo.lock` is ignored by this repository and absent in a fresh worktree. The initial `--locked` attempt failed before compiling. The primary checkout's ignored lock was copied (not modified there), but was stale for this baseline; Cargo still refused `--locked`. Resolving offline in the isolated worktree produced a usable lock, then subsequent root checks used `--locked`. Root lock SHA-256: `e7419fcc2b0eb45e7ad28fffed0f72ffd96d2da3012c5af4194c19e8f3369bee`. The standalone reproduction's small dependency lock snapshot is retained beside its manifest for repeatability; the root lock hash records the larger local workspace resolution, not a claim that it was version-controlled.

**Explicitly not verified in this batch:** all-feature/formal/Creusot proofs, native precision/rug, optional parallel/serialize configurations, full workspace regression tests, MSRV, other CPU architectures, no_std target cross-builds beyond the failed host check, unsafe/Miri/sanitizers, performance benchmarks, extreme-dimension resource limits and comprehensive NaN/Inf/overflow behavior. GPU and WASM execution are excluded, not silently skipped test successes.

## Proposed follow-up batches (not implemented or approved)

1. Correct inverse and coefficient-threshold semantics (CORE-01/02); add coefficientwise regression tests first.
2. Establish the norm/error/normalization contracts and correct exponential domains (CORE-03–07). Reuse independent scalar and commuting/nilpotent references.
3. Define supported rotor/reflection domains and branch behavior; repair interpolation, powers and negative-signature vector mapping (CORE-10–14).
4. Agree on grade-wrapper privacy, dimension-specific APIs, fallible construction and dual conventions (CORE-08/09/15). Dedicated phantom audit informs the design; do not pre-empt it.
5. Only then benchmark table reuse/on-demand kernels and allocation reduction; fix feature-matrix/test-contract gaps separately.

**Next audit source:** `src/unicode_ops.rs`, followed by the September 30 phantom/formal cohort. No claim is made that the crate-wide or repository-wide audit is complete.
