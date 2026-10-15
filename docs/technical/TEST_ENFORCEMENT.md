# Test Enforcement

> **Status (M-REP-A, 2026-10-15): honest rewrite.** This page previously
> described test enforcement as validating mathematical properties and formal
> guarantees. Test enforcement *runs and gates tests*; it does not prove
> theorems. This page now describes the enforcement mechanics and what they do
> and do not establish. See `docs/technical/MATHEMATICAL_RIGOR.md` for the
> epistemic framing and the approved C3/C4 proof gates.

## Overview

The Amari workspace enforces a strict pre-commit and CI pipeline so that a
change cannot land while the formatting, lint, test, or documentation checks
fail. This is important for a library that other crates build on top of, but it
is a *regression guard*, not a source of mathematical proof.

## 🎯 Why test enforcement?

Mathematical libraries need strong regression protection because:
- **Cascading errors**: a single wrong result can invalidate downstream work.
- **Trust**: users depend on the library's documented behavior.
- **Regression prevention**: changes in one domain should not break another.
- **Annotation compatibility**: the `formal-verification` feature suites must
  keep compiling and executing.

Universal algebraic laws are proof obligations (C3/C4, not done here), not
something test enforcement establishes. See `MATHEMATICAL_RIGOR.md`.

## 🔧 Setup

Run once after cloning:

```bash
./setup-githooks.sh
```

This configures git to use the custom pre-commit hooks that enforce:

### ✅ Pre-commit checks

1. **Code formatting** (`cargo fmt --check`)
   - Consistent code style across the workspace

2. **Clippy lints** (`cargo clippy --workspace --all-targets --all-features`)
   - Common bugs and Rust best practices
   - All warnings must be resolved (treated as errors)

3. **Core test suites** (`cargo test --workspace`)
   - All exercised operations must pass their assertions
   - Includes large edge-case suites
   - Validates exercised properties and edge cases

4. **`formal-verification` feature suites**
   (`cargo test --workspace --features formal-verification`)
   - Annotation-compatibility tests compile and execute
   - Contract annotations and phantom type constraints compile
   - (This is annotation compatibility, not discharged proof.)

5. **Documentation build** (`cargo doc --workspace --no-deps`)
   - All documentation compiles
   - Doc examples are tested

## 🚦 Test enforcement workflow

### Normal commit flow
```bash
git add .
git commit -m "feat: implement new mathematical operation"
# Pre-commit hooks automatically run:
# ✅ Code formatting check
# ✅ Clippy lints
# ✅ Core test suites (all crates)
# ✅ formal-verification feature suites
# ✅ Documentation build
# Commit proceeds if all checks pass
```

### Failed check workflow
```bash
git commit -m "feat: buggy implementation"
# ❌ Checks failed!
# Fix the issues, then retry
git add .
git commit -m "feat: correct implementation"
# ✅ All checks pass, commit proceeds
```

### Emergency bypass (use sparingly)
```bash
# Only for urgent fixes when checks are temporarily broken
git commit --no-verify -m "emergency: critical security fix"
```

## 📊 Test coverage standards

The enforcement system gates on the test suites and reports coverage counts:

### Test count targets
- **amari-core**: 140+ tests covering geometric algebra operations
- **amari-tropical**: 53+ tests covering tropical algebra
- **Each crate**: minimum 300% increase from baseline

Counts are inventory/coverage numbers: how many test functions exist and how
much code they exercise. They are not a measure of proved mathematics.

### Test categories required
- **Constructor tests**: creation methods exercised
- **Property tests**: algebraic properties exercised on sampled/edge inputs
- **Edge-case tests**: infinity, zero, boundary conditions
- **Operation tests**: arithmetic and algebraic operations
- **Integration tests**: cross-crate compatibility
- **Annotation-compatibility tests**: phantom-type/contract code compiles and
  runs

## 🔬 Formal-annotation compatibility

The `formal-verification` feature suites specifically check that:

### Mathematical properties are exercised
- **Associativity**: `(a ⊕ b) ⊕ c = a ⊕ (b ⊕ c)` on the tested inputs
- **Commutativity**: `a ⊕ b = b ⊕ a` on the tested inputs
- **Identity elements**: `a ⊕ 0 = a` on the tested inputs
- **Distributivity**: `a ⊗ (b ⊕ c) = (a ⊗ b) ⊕ (a ⊗ c)` on the tested inputs

These are instance checks. In particular, IEEE-754 arithmetic is not
associative at extremes: for scalar `f64` `a = b = 1e308`, `c = 0`,
`(a*b)*c` is `NaN` while `a*(b*c)` is `0`.

### Type safety
- **Phantom type constraints**: compile-time type checks enforced by the
  compiler
- **Dimensional consistency**: matrix/tensor dimension types checked
- **Semiring separation**: MaxPlus/MinPlus type separation maintained

### Annotation compatibility
- **Semiring annotations**: tropical contract annotations compile
- **Geometric algebra annotations**: Clifford-algebra contract annotations
  compile
- **Information geometry annotations**: manifold contract annotations compile

A green `formal-verification` run means the annotations compile and the
compatibility suites pass; it does not mean any contract was translated to a
prover or discharged.

## 📈 Benefits

### Regression protection
- **Zero observed regressions**: blocked commits cannot break exercised paths
- **Continuous execution**: the suites run on every commit
- **Early detection**: errors are caught before they reach users

### Development quality
- **Consistent code style**: uniform formatting across all crates
- **Best practices**: clippy enforces Rust best practices
- **Documentation quality**: all docs must build

### Research reliability
- **Reproducible execution**: the same checks run locally and in CI
- **Trustworthy foundation**: exercised behavior is protected
- **Type-level safety**: phantom types give compile-time type constraints

## 🛠️ Troubleshooting

### Common issues

#### Test failures
```bash
# See which specific tests failed
cargo test --workspace

# Run a specific test for debugging
cargo test test_name -- --nocapture

# Run the formal-verification feature suites
cargo test --workspace --features formal-verification
```

#### Formatting issues
```bash
# Fix formatting automatically
cargo fmt

# Check formatting
cargo fmt --check
```

#### Clippy warnings
```bash
# See all warnings
cargo clippy --workspace --all-targets --all-features

# Auto-fix some issues
cargo clippy --workspace --all-targets --all-features --fix
```

#### Documentation errors
```bash
# Build docs to see errors
cargo doc --workspace --no-deps

# Include private items for debugging
cargo doc --workspace --no-deps --document-private-items
```

### Disable temporarily
If you need to temporarily disable enforcement (use sparingly):

```bash
# Disable for a single commit
git commit --no-verify -m "emergency fix"

# Disable hooks entirely (not recommended)
git config core.hooksPath ""

# Re-enable hooks
git config core.hooksPath .githooks
```

## 📋 Maintenance

### Adding new tests
When adding new mathematical operations:

1. **Implement comprehensive tests first**
2. **Follow the proven test patterns** from existing crates
3. **Include edge cases and property instances**
4. **Add annotation-compatibility tests if applicable**
5. **Ensure all checks pass before committing**

### Updating enforcement
To modify the pre-commit checks:

1. **Edit `.githooks/pre-commit`**
2. **Test changes thoroughly**
3. **Update this documentation**
4. **Communicate changes to the team**

## 🎯 Success metrics

The enforcement system ensures:
- **100% check-pass rate** on commits that are allowed to land
- **No observed regressions** across releases
- **Broad coverage** of exercised operations
- **Annotation compatibility** for `formal-verification` builds
- **Consistent code quality** across contributors

This is a regression guard, not a proof. Universal mathematical statements are
tracked as C3/C4 obligations in the approved verification matrix.

---

*This enforcement system keeps the exercised behavior of Amari stable; it does
not by itself establish mathematical truth.*
