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
# Automatically runs on every commit attempt
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
6. **Geometric algebra axiom test suites**: explicit algebraic property tests
7. **Phantom type type-safety tests**: compile-time constraint (type) tests
8. **Test-count reporting**: coverage/inventory counts

### 3. 📊 Continuous monitoring (counts)
- Minimum 300+ `#[test]` functions (currently 579+)
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
- **Non-finite inputs are not universally rejected.** For example,
  `VerifiedRotor::new` computes a unit-norm check written as
  `(norm - 1).abs() > 1e-4`, which is `false` for `NaN`; a scalar `NaN` is
  consequently accepted as a "verified" rotor. Retained coefficient-equality
  annotations likewise fail on `NaN`/`inf`. Non-finite handling and the exact
  specification are deferred to M-REP-C.
- **Counts are not proofs.** Test-function and coverage counts measure how much
  code is exercised, not how much mathematics is proved.

## Failure handling

### Zero tolerance for green-check failures
- **Any failing check blocks the pipeline**
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
# Full check (matches CI exactly)
./.githooks/pre-commit

# Specific property test suites
cargo test geometric_axioms
cargo test semiring_axioms
cargo test verified_contracts
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
