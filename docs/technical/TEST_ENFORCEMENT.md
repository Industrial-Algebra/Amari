# Test Enforcement

> **Status (M-REP-A, 2026-10-15): honest rewrite.** This page previously
> described test enforcement as validating mathematical properties and formal
> guarantees. Test enforcement *runs and gates tests*; it does not prove
> theorems. This page now describes the enforcement mechanics and what they do
> and do not establish. See `docs/technical/MATHEMATICAL_RIGOR.md` for the
> epistemic framing and the approved C3/C4 proof gates.

## Overview

The Amari workspace runs a pre-commit hook plus CI checks. The table below is
**transcribed by script** from `.githooks/pre-commit` (292 lines) — mechanically
extracted, not hand-summarised. The extractor and its complete output are
archived in `docs/plans/core-matrix-evidence/mrep-a/hook-extraction.txt`; every
row cites the exact script line range, and every command is verbatim.

| # | Check | Command (verbatim) | Branch condition | Failure handling | Script lines |
|---|---|---|---|---|---|
| 1 | Formatting | `cargo fmt --check` (workspace) | TOP-LEVEL (both branches) | BLOCKS (`exit 1`) | 69-73 |
| 2 | Clippy | `cargo clippy --workspace --all-targets --all-features -- -D warnings` | TOP-LEVEL (both branches) | BLOCKS (`exit 1`) | 77-81 |
| 3 | Workspace tests | `cargo test --workspace --quiet` | FULL (`RUN_FULL_TESTS=true`) | BLOCKS (`exit 1`) | 96-100 |
| 4 | Formal-feature suite | `cargo test --workspace --features formal-verification --quiet` | FULL | BLOCKS (`exit 1`) | 104-108 |
| 5 | Integration tests | `cargo test --workspace --tests --quiet` | FULL | BLOCKS (`exit 1`) | 112-116 |
| 6 | Doctests | `cargo test --workspace --doc --quiet` | FULL | BLOCKS (`exit 1`) | 120-124 |
| 7 | `geometric_axioms` filter | `cargo test --package amari-core geometric_axioms --quiet` | FULL | BLOCKS (`exit 1`); selected-test count not asserted — M-REP-B | 128-132 |
| 8 | `semiring_axioms` filter | `cargo test --package amari-tropical semiring_axioms --quiet` | FULL | BLOCKS (`exit 1`) | 136-140 |
| 9 | Phantom contracts, core | `cargo test --package amari-core verified_contracts --quiet` | FULL | BLOCKS (`exit 1`) | 144-148 |
| 10 | Phantom contracts, tropical | `cargo test --package amari-tropical verified_contracts --quiet 2>/dev/null` | FULL | TOLERATED (warning echo only; no `exit 1`) | 149-151 |
| 11 | Phantom contracts, network | `cargo test --package amari-network verified_contracts --quiet 2>/dev/null` | FULL | TOLERATED (warning echo only; no `exit 1`) | 152-154 |
| 12 | Coverage gather | `TOTAL_TESTS=$(cargo test --workspace --lib --bins -- --list 2>/dev/null \| grep -c "test " \|\| echo "0")` | FULL | gather only; `2>/dev/null` + `\|\| echo "0"` yield count 0 on failure; the L159 gate decides | 157-158 |
| 13 | Coverage floor | `[ "$TOTAL_TESTS" -lt 300 ]` | FULL | BLOCKS below 300 (`exit 1`) | 159-163 |
| 14 | Workspace docs | `cargo doc --workspace --no-deps --quiet` | FULL | BLOCKS (`exit 1`) | 167-171 |
| 15 | Crate tests | `cargo test --package "$CHANGED_CRATE" --quiet` | TARGETED (`CHANGED_CRATE` set) | BLOCKS (`exit 1`) | 183-187 |
| 16 | Crate doctests | `cargo test --package "$CHANGED_CRATE" --doc --quiet 2>/dev/null` | TARGETED | TOLERATED (`if`-failure prints warning; `else` prints success) | 192-196 |
| 17 | Crate docs | `cargo doc --package "$CHANGED_CRATE" --no-deps --quiet` | TARGETED | BLOCKS (`exit 1`) | 200-204 |
| 18 | WASM gate determination | `WASM_CRATES=(...)`; `WASM_AFFECTED=true` if `RUN_FULL_TESTS=true`, else loop `CHANGED_CRATE` over `WASM_CRATES` | FULL: always; TARGETED: `CHANGED_CRATE` is one of `WASM_CRATES` | selection only (no command) | 215-228 |
| 19 | WASM target lookup | `rustup target list --installed \| grep -q wasm32-unknown-unknown` | WASM-conditional (`WASM_AFFECTED=true`) | condition only; gates the install step | 234-237 |
| 20 | WASM target install | `rustup target add wasm32-unknown-unknown` | WASM-conditional, only when the lookup fails | BLOCKS: unguarded, no failure branch; a non-zero exit aborts under `set -e` (L7), with no custom message | 236 (with 7) |
| 21 | WASM `amari-wasm`, full | `cargo check --package amari-wasm --target wasm32-unknown-unknown --quiet` | WASM-conditional + FULL | BLOCKS (`exit 1`) | 242-246 |
| 22 | WASM per-crate loop, full | `cargo check --package "$crate" --target wasm32-unknown-unknown --quiet 2>/dev/null` for `$crate` in `WASM_CRATES` **excluding `amari-wasm` and `amari`** | WASM-conditional + FULL + loop | TOLERATED (warning echo only; no `exit 1`) | 248-255 |
| 23 | WASM `amari-wasm`, targeted | `cargo check --package amari-wasm --target wasm32-unknown-unknown --quiet` | WASM-conditional + TARGETED + `CHANGED_CRATE=amari-wasm` | BLOCKS (`exit 1`) | 258-263 |
| 24 | WASM changed crate, targeted | `cargo check --package "$CHANGED_CRATE" --target wasm32-unknown-unknown --quiet 2>/dev/null` | WASM-conditional + TARGETED + `CHANGED_CRATE != amari-wasm` **and** `!= amari` | TOLERATED (warning echo only; no `exit 1`) | 264-269 |
| 25 | `amari` (root) special case | **no WASM `cargo check` is issued** | WASM-conditional | full: excluded by the loop condition (L250); targeted: the `elif [ "$CHANGED_CRATE" != "amari" ]` guard (L264) skips the check. The WASM banner and target-install step still run | 250, 264 |

Source semantics for the WASM rows: `amari-wasm` is the only crate whose WASM
check **blocks** (rows 21, 23); every other checked crate is **TOLERATED**
(rows 22, 24); the root `amari` crate is never checked (row 25).

**Branch selection precedence** (script lines 34-62, in condition order):
`[full-test]` in `.git/COMMIT_EDITMSG` (read via
`grep -q '\[full-test\]' .git/COMMIT_EDITMSG 2>/dev/null`, L33-39), OR
workspace-level changed files (`Cargo.toml`, `.githooks`, `.github`,
`scripts`; L30), OR no crate changes (`CRATE_COUNT=0`), OR more than one
changed crate (`CRATE_COUNT>1`) → **FULL**; otherwise exactly one changed
crate → **TARGETED** with `CHANGED_CRATE` set (L55-61). The targeted branch
prints a reminder that the full suite runs in CI/CD (L208).

**WASM gate** (script lines 215-228): `WASM_AFFECTED=true` whenever the FULL
branch runs; in the TARGETED branch it is true only when `CHANGED_CRATE` is
one of `WASM_CRATES` = `amari amari-core amari-tropical amari-dual
amari-network amari-relativistic amari-info-geom amari-enumerative
amari-fusion amari-automata amari-wasm` (L216). The WASM validation block
(target lookup/install and the checks in rows 19-25) runs only when
`WASM_AFFECTED=true`.

A green TARGETED hook therefore does not mean the full pipeline passed —
the authoritative gate is CI (see `CI_SETUP.md`). Either branch is a
regression guard and convenience, not a source of mathematical proof.

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

This configures git to use the custom pre-commit hooks. The list below is the
**FULL-branch** command set; the TARGETED branch runs only a subset
(formatting + Clippy always; the changed crate's tests and docs, with crate
doctest failures tolerated and no formal-feature suite). See the table above
for the per-check branch and failure handling.

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
# Pre-commit hooks run (branch-dependent; see the table above):
# ✅ Code formatting check             (both branches, workspace)
# ✅ Clippy lints                      (both branches, workspace)
# ✅ Core test suites                  (FULL: all crates; TARGETED: changed crate)
# ✅ formal-verification feature suites (FULL branch only)
# ✅ Documentation build               (FULL: workspace; TARGETED: changed crate)
# Commit proceeds if the branch's in-scope checks pass.
# The TARGETED branch tolerates crate doctest failures and skips the formal suite
# and the workspace-wide checks (CI runs them; see below).
```

### Failed check workflow
```bash
git commit -m "feat: buggy implementation"
# ❌ Checks failed!
# Fix the issues, then retry
git add .
git commit -m "feat: correct implementation"
# ✅ In-scope checks pass, commit proceeds
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
- **Blocked-failure commits**: both branches reject formatting/Clippy
  failures workspace-wide and test failures in their respective scope
  (targeted: the changed crate; full: the workspace)
- **Continuous execution**: the full suite runs on every push via CI
- **Early detection**: most errors are caught before they reach users —
  the targeted branch's tolerances (doctests, no formal suite, one
  crate's tests) remain until the full branch or CI runs

### Development quality
- **Consistent code style**: uniform formatting across all crates
- **Best practices**: clippy enforces Rust best practices
- **Documentation quality**: all docs must build

### Research reliability
- **Reproducible execution**: the hook's FULL-branch checks and CI share
  overlapping definitions; the TARGETED branch is a strict subset, and local
  coverage is NOT identical to CI (see the table above and `MATHEMATICAL_RIGOR.md`)
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
5. **Ensure the applicable in-scope checks pass before committing** (see the
   table above; the TARGETED branch is narrower than CI)

### Updating enforcement
To modify the pre-commit checks:

1. **Edit `.githooks/pre-commit`**
2. **Test changes thoroughly**
3. **Update this documentation**
4. **Communicate changes to the team**

## 🎯 Success metrics

The enforcement system provides:
- **In-scope check-pass on landable commits**: a commit only lands after the
  branch's in-scope checks pass; tolerated checks (per the table above) may
  fail with a warning, and the TARGETED branch defers the rest to CI
- **No observed regressions** across releases
- **Broad coverage** of exercised operations
- **Annotation compatibility** for `formal-verification` builds
- **Consistent code quality** across contributors

This is a regression guard, not a proof. Universal mathematical statements are
tracked as C3/C4 obligations in the approved verification matrix.

---

*This enforcement system keeps the exercised behavior of Amari stable; it does
not by itself establish mathematical truth.*
