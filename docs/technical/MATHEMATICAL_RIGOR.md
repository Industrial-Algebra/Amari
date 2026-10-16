# Mathematical Rigor: Test and Annotation Methodology

> **Status (M-REP-A, 2026-10-15): honest rewrite.** This page previously
> described the test/CI pipeline as establishing *verified mathematical
> guarantees*. That was an overstatement. The pipeline executes tests and
> compiles annotation attributes; it does **not** discharge proofs. This page
> now distinguishes (a) what ordinary tests establish, (b) what annotations
> express, and (c) what would constitute proof. See
> `docs/plans/2026-10-09-amari-core-verification-matrix.md` for the approved
> verification matrix and the C3/C4 proof gates.

## Philosophy

> "A mathematical library lives and dies on its correctness."

This repository takes mathematical correctness seriously: the test suites
exercise the algebra across many inputs and edge cases, and CI is configured to
fail on regressions. Rigor here means *executed evidence plus explicit
contracts* — not a claim of proof.

## Test layers

### 1. 🛡️ Pre-commit hook (local)
**Purpose**: Immediate feedback to prevent commits that fail checks
**Runtime**: ~30 seconds
**Coverage**: Branch-dependent — formatting + Clippy always (workspace);
the full test/formal/doctest/doc battery ONLY on the full branch; the
targeted branch (single changed crate) runs that crate's tests/docs with
doctest failures tolerated and NO formal-feature suite. See the complete
source-mapped table in `TEST_ENFORCEMENT.md`; coverage is NOT identical
to CI.

```bash
# Runs on each commit attempt (unless bypassed with `git commit --no-verify`);
# the FULL-vs-TARGETED branch is chosen per the TEST_ENFORCEMENT.md table.
.githooks/pre-commit
```

**Blocks commits whose (in-scope) checks fail:**
- Code formatting (rustfmt) — always, workspace-wide
- Linting violations (clippy -D warnings) — always, workspace-wide
- Failing tests — in the branch's scope only (targeted: one crate;
  full: workspace; tolerances per the TEST_ENFORCEMENT.md table)
- Documentation build failures — workspace (full branch) or the changed
  crate (targeted branch)

### 2. 🔬 GitHub Actions CI (cloud)
**Purpose**: Run the test/lint/doc suites in a clean environment
**Runtime**: ~10-15 minutes
**Triggers**: Every push, every PR

**Steps:**
1. **Code quality**: format + clippy with zero warnings tolerated
2. **Core algebra test suites**: library and binary tests
3. **Formal-annotation compatibility**: `formal-verification` feature test suites
4. **Integration test suites**: cross-crate interaction
5. **Documentation tests**: doc examples must compile and pass
6. **Geometric algebra axiom test suites**: explicit algebraic property tests,
   non-vacuity asserted against an approved inventory
7. **Phantom type type-safety tests**: compile-time constraint (type) tests,
   gated per crate (`formal-verification` for `amari-core`, `contracts` for
   `amari-tropical`)
8. **Test-count reporting**: coverage/inventory counts

### 3. 📊 Continuous monitoring (counts)
- Minimum 300+ `#[test]` functions (currently 579+)
- The coverage floor is a real gate: the gather captures cargo's exit
  status separately and a FAILED gather exits 1 in the gather itself;
  a well-formed low count is then blocked by `[ "$TOTAL_TESTS" -lt 300 ]`
  (M-REP-B, hardened R1)
- Test-function counts in annotation-marker files are source-inventory counts
- All phantom type constraints that the compiler can check are enforced at
  compile time

## What ordinary tests establish

Ordinary tests are **executed instances**. Each test runs a concrete operation
on concrete inputs and asserts a concrete result. A passing suite therefore
establishes that:

- those specific inputs produced the asserted outputs on that machine, Rust
  toolchain, and feature set;
- no regression was observed in the exercised code paths.

It does **not** establish a universally quantified algebraic law. Universal
statements require a proof (or exhaustive enumeration over a finite domain),
not a finite sample of executions.

## What annotations express

- **Contract annotations** such as
  `#[cfg_attr(feature = "formal-verification", requires(...), ensures(...))]`
  express *intended specifications*. In ordinary builds the `cfg_attr`
  predicate is false and the attribute expands to nothing. Even when the
  `formal-verification` feature is enabled, the attribute is a specification
  marker: a green build does not establish that the expression is meaningful
  to a prover, nor that any obligation is translated or discharged.
- **Phantom type parameters** (`PhantomData<Signature<P, Q, R>>`,
  `PhantomData<Grade<K>>`, …) express and let the compiler *check* type-level
  constraints such as signature and grade. That is type-level safety — the
  compiler rejects mismatched types — but it is not a proof of runtime
  mathematical results.
- Annotation compatibility is what the `formal-verification` feature currently
  tests. It is not a prover run.

## What would constitute proof

Machine-checked proof of the core obligations is **not done** in this
repository. It would require, per the approved matrix:

- **C3 — toolchain conformance**: a pinned, provenance-recorded proof toolchain
  (for example Creusot 0.8.0 with matched contracts and Why3/SMT solvers).
- **C4 — translation and discharged obligations**: translating the core
  functions/contracts and discharging the resulting verification conditions.

These tools are not installed here. The approved matrix retains C3/C4 as a
pre-1.0 default (with an alternative to descope the `formal-verification`
feature to annotation compatibility only). That decision belongs to the matrix,
not to this page.

## Honest caveats

- **Floating-point non-associativity at extremes.** IEEE-754 arithmetic is not
  associative. For scalar `f64` `a = b = 1e308`, `c = 0`: `(a*b)*c` evaluates
  to `NaN` while `a*(b*c)` evaluates to `0`. Ordinary tests of associativity
  are therefore instance checks, not a universal guarantee.
- **Non-finite inputs are rejected at construction, not everywhere (M-REP-C,
  2026-10-15).** The phantom-module constructors that take coefficients
  (`VerifiedMultivector::new`, `VerifiedMultivector::scalar`, `Vector::new`,
  and `VerifiedRotor::new` via its inner multivector) reject `NaN` and
  ±infinity with a clear `Err`; the equality contracts in
  `verified_contracts.rs` carry matching finiteness preconditions and runtime
  `debug_assert!`s. (Before M-REP-C, `VerifiedRotor::new` accepted a `NaN`
  scalar because its unit-norm test `(norm - 1).abs() > 1e-4` is `false` for
  `NaN`.) **Operation results need their own domain handling (M-REP-C review
  round 1, 2026-10-16):** finite inputs can still overflow (`[MAX]+[MAX]` is
  `+inf`, and `(+inf)+(-inf)` is `NaN`), so `Vector::verified_self_dot`
  rejects non-finite coefficients and returns `Ok(r) ==> r >= 0`; the checked
  `verified_scalar_mult` / `verified_addition` / `verify_scalar_identity` /
  `verify_zero_addition` operations and `VerifiedRotor::compose` return
  `Result` and route their result through `VerifiedMultivector::new`, whose
  finiteness gate is the single checked boundary. The infallible arithmetic
  (`add`, `geometric_product`, `apply`) remains panic-free but can still carry
  `inf`. `verified_laws.rs` was removed as unreachable stub code
  (MATRIX-03); replacing that surface with a real proof/verification pass is a
  future pre-1.0 direction, not part of M-REP-C.
- **Counts are not proofs.** Test-function and coverage counts measure how much
  code is exercised, not how much mathematics is proved.

## Failure handling

### Zero tolerance for green-check failures
- **Any failing gating check in the CI pipeline blocks it** (the local hook's
  TARGETED-branch tolerances are enumerated in `TEST_ENFORCEMENT.md`)
- **No exception for "minor" errors**
- **Documentation examples must compile and pass**

### Failure investigation protocol
1. **Identify** which test or check failed.
2. **Analyze** whether it is a regression or a new edge case.
3. **Fix** the implementation or the assertion.
4. **Re-run** to ensure the fix does not break other properties.
5. **Add** a regression test.

## Performance vs. coverage

### Current choice: **comprehensive over fast**
- **CI runtime**: 10-15 minutes
- **Parallel optimization available but disabled**: prioritizing completeness
- **Cost**: ~15-20 commits fit in the free tier per day

### Future optimization path
1. Enable `parallel-verification.yml` for speed.
2. Keep `mathematical-correctness.yml` for releases.
3. Add matrix testing across Rust versions.
4. Add property-based testing.

## Repository protection

### Required status checks
- ✅ Mathematical-correctness test suites must pass
- ✅ Test suites must pass
- ✅ Documentation must build
- ✅ Zero clippy warnings tolerated

### Branch protection rules
```yaml
required_status_checks:
  strict: true
  contexts:
    - "Mathematical Correctness Check"
dismiss_stale_reviews: true
require_code_owner_reviews: true
required_approving_review_count: 1
```

## Contribution expectations

By contributing to this repository, you agree that:

1. **Mathematical correctness is non-negotiable.**
2. **Algebraic properties are tested explicitly** (as executed instances).
3. **Type-level safety is enforced where the type system can express it.**
4. **Documentation examples are tested** and must compile and pass.
5. **Performance optimizations cannot compromise the checks above.**

## Commands

### Local development
```bash
# Hook run; branch chosen by changed crates; coverage is NOT identical to CI.
# See the source-mapped table in TEST_ENFORCEMENT.md.
./.githooks/pre-commit

# Specific named property suites. Each invocation is wrapped by the
# approved-inventory + non-vacuity assertion (scripts/verify-named-suite.sh):
# a filter that selects zero tests (or misses an approved name) fails.
# See TEST_ENFORCEMENT.md for the per-suite inventories.
bash scripts/verify-named-suite.sh amari-core geometric_axioms \
  geometric_axioms::associativity geometric_axioms::distributivity \
  geometric_axioms::multiplicative_identity geometric_axioms::additive_identity \
  geometric_axioms::scalar_commutes geometric_axioms::basis_vector_signature
bash scripts/verify-named-suite.sh --features contracts amari-tropical semiring_contracts \
  verified_contracts::contract_tests::test_maxplus_semiring_contracts \
  verified_contracts::contract_tests::test_minplus_semiring_contracts
bash scripts/verify-named-suite.sh --features formal-verification amari-core verified_contracts \
  verified_contracts::contract_tests::test_signature_verification \
  verified_contracts::contract_tests::test_verified_addition_properties \
  verified_contracts::contract_tests::test_verified_identity_laws \
  verified_contracts::contract_tests::test_verified_scalar_multiplication
```

### Manual CI trigger
```bash
# Trigger the comprehensive test run
gh workflow run mathematical-correctness.yml

# Trigger the parallel test run (when enabled)
gh workflow run parallel-verification.yml
```

---

**Remember**: a green pipeline means the executed tests passed. Universal
mathematical claims are proof obligations, and those are tracked by the
verification matrix (C3/C4), not asserted by CI.
