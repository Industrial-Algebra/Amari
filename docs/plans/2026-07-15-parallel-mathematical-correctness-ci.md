# Parallel Mathematical Correctness CI Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task.

**Goal:** Prevent PR #188's mathematical-correctness gate from exhausting one 20-minute job while preserving every existing verification command.

**Architecture:** Replace the serial job with independent quality, core/formal, workspace-integration, discovery-integration, and documentation jobs. Preserve `Mathematical Correctness Check` as an `always()` summary gate so branch-protection identity and failure propagation remain stable.

**Tech Stack:** GitHub Actions, Cargo, stable Rust, Python/PyYAML for local structural verification.

---

### Task 1: Prove the current workflow is serial

**Files:**
- Test: `.github/workflows/mathematical-correctness.yml`

1. Run a one-off YAML assertion requiring the planned job IDs and summary dependencies.
2. Confirm it fails because only `mathematical-correctness` exists.

### Task 2: Split verification while retaining the gate

**Files:**
- Modify: `.github/workflows/mathematical-correctness.yml`

1. Add a shared checkout/toolchain/cache setup to each verification job.
2. Move formatting and Clippy into `code-quality`.
3. Move library, binary, formal-verification, doctest, axiom, type-safety, and property-count checks into `core-mathematics`.
4. Run non-discovery integration tests in `workspace-integration` with `--exclude amari-discovery`.
5. Run `amari-discovery` integration tests independently in `discovery-integration`.
6. Move private-item documentation generation into `documentation`.
7. Add an `always()` summary job named `Mathematical Correctness Check` that fails unless every dependency succeeded.

### Task 3: Verify and publish

1. Re-run the YAML structural assertion; expect PASS.
2. Run `cargo fmt --all -- --check` and `git diff --check`.
3. Commit and push without rewriting published branch history.
4. Monitor the new PR #188 checks and inspect each job result.
