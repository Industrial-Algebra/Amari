# GitHub Actions CI Setup

## Overview

This repository uses both **local pre-commit hooks** and **GitHub Actions** to
run its test, lint, and documentation suites. This dual approach provides:

- **Fast local feedback** via pre-commit hooks (~30 seconds)
- **Comprehensive cloud test execution** via GitHub Actions (~5-8 minutes)
- **Parallel test execution** to minimize CI time
- **Cost optimization** for private repositories

> **Scope note (M-REP-A, 2026-10-15).** These workflows *execute tests*; they do
> not discharge mathematical proofs. The `formal-verification` feature currently
> enables contract annotations and additional test suites (annotation
> compatibility), not a prover. Discharged obligations would require the C3
> toolchain-conformance and C4 proof gates described in the approved matrix
> (`docs/plans/2026-10-09-amari-core-verification-matrix.md`), which are not run
> here. See `docs/technical/MATHEMATICAL_RIGOR.md` for what ordinary tests do
> and do not establish.

## Workflows

### 1. `mathematical-correctness.yml`
**Purpose**: Comprehensive test/lint/doc run matching the pre-commit hook
**Runtime**: ~5-7 minutes
**Triggers**: Push to main/develop, all PRs

Runs the complete test suite sequentially:
- Code formatting (rustfmt)
- Linting (clippy)
- Core algebra test suites
- `formal-verification` feature test suites (annotation compatibility)
- Documentation builds

### 2. `parallel-verification.yml`
**Purpose**: Optimized parallel test execution
**Runtime**: ~3-5 minutes (parallel execution)
**Triggers**: Push to main/develop, all PRs

Parallel jobs:
- **Code Quality**: Format + Clippy (5 min)
- **Core Algebra**: Tests core crates in parallel (8 min)
- **Advanced Features**: Tests complex crates in parallel (10 min)
- **GPU Acceleration**: GPU tests with CI fallbacks (8 min)
- **Formal Verification**: `formal-verification` feature tests (8 min)
- **Documentation**: Doc builds (5 min)
- **Integration**: Final integration check (10 min)

### 3. `status-badges.yml`
**Purpose**: Generate dynamic status badges
**Runtime**: ~2 minutes
**Triggers**: Push to main, weekly schedule

Creates badges showing:
- Total test count (e.g., "579 passing")
- Test functions in annotation-marker files (e.g., "140 test functions in
  annotation-marker files") — a *source-inventory* count of `#[test]`
  functions in files that mention the `formal_verification` marker, not a count
  of proved statements
- Mathematical-correctness test-suite status

## Cost Analysis

### Free Tier Usage (Private Repos)
- **Parallel workflow**: ~5 minutes per run
- **Monthly estimate**: ~150-200 commits × 5 min = 750-1000 minutes
- **Free tier**: 2,000 minutes/month
- **Conclusion**: Well within free limits

### Public Repos
- **Completely free** - unlimited GitHub Actions minutes

### Optimization Strategies

1. **Conditional execution**:
   ```yaml
   if: contains(github.event.head_commit.message, '[skip ci]') == false
   ```

2. **Path-based triggers**:
   ```yaml
   paths:
     - 'amari-*/**'
     - 'Cargo.toml'
     - 'Cargo.lock'
   ```

3. **Matrix parallelization**:
   ```yaml
   strategy:
     matrix:
       crate: [amari-core, amari-tropical, amari-dual]
   ```

## Recommended Setup

### For Most Projects
Use **both workflows**:
- `parallel-verification.yml` for regular commits (faster feedback)
- `mathematical-correctness.yml` for release branches (comprehensive test run)

### For High-Frequency Development
Use **parallel-verification.yml** only with path triggers:
```yaml
on:
  push:
    paths:
      - 'amari-*/**'
      - '!**.md'  # Skip docs-only changes
```

### For Release Candidates
Add release-specific workflow:
```yaml
on:
  push:
    tags:
      - 'v*'
```

## Local vs. Cloud Testing

| Aspect | Pre-commit Hook | GitHub Actions |
|--------|----------------|----------------|
| **Speed** | 30 seconds | 5-8 minutes |
| **Coverage** | Full test suite | Full + parallel |
| **Environment** | Developer machine | Clean Ubuntu |
| **GPU Tests** | May work locally | Graceful fallback |
| **Cost** | Free | Free/cheap |
| **Timing** | Before commit | After push |

## Status Integration

Add to your README.md:
```markdown
![Mathematical Correctness](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/username/gist-id/raw/math-correctness.json)
![Tests](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/username/gist-id/raw/test-count.json)
![Annotation count](https://img.shields.io/endpoint?url=https://gist.githubusercontent.com/username/gist-id/raw/verification.json)
```

## What this setup does (and does not) establish

The combination of pre-commit hooks + GitHub Actions ensures:

✅ **Regression guard**: the pre-commit hook blocks commits whose test, lint, or
   documentation checks fail
✅ **Transparent test execution**: GitHub Actions runs the suites in a clean
   environment
✅ **Parallel efficiency**: matrix jobs minimize CI time
✅ **Cost effectiveness**: optimized for free tier usage

It does **not** establish mathematical correctness as a theorem. A green run
means the executed test instances matched their assertions on that machine,
toolchain, and feature set — not that a universally quantified algebraic law
holds. Floating-point extremes break even associativity: for scalar `f64`
`a = b = 1e308`, `c = 0`, `(a*b)*c` is `NaN` while `a*(b*c)` is `0`. Universal
statements are proof obligations (C3/C4) and are out of scope of this pipeline.
