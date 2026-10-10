# CORE-D07 Validation and Review Checkpoint Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task, only after explicit checkpoint authorization and workflow reconciliation. This is a verification/document-review checkpoint, not a library implementation contract.

**Goal:** Validate the corrected baseline-ISA command recipe and complete independent review of the cumulative CORE-D07 draft before requesting concrete matrix approval.

**Architecture:** Separate planning, executable verification, independent review, matrix approval and remediation authorization. Use pinned source/lock and installed research compilers for a bounded two-stage recipe-conformance check: compile/document first, inspect actual flags, then execute tests. Preserve baseline failures; this is not the full host matrix.

**Tech Stack:** Bash, Python 3 standard library for loading the canonical Markdown recipe, installed rustup stable/nightly, Cargo, existing core tests/Clippy/rustdoc, Pi workflow monitor, ia-moment REVIEW class.

---

## Status, dependencies and scope

**Status:** DRAFT — authoring authorized; executable checks and review dispatch not yet authorized.

**Unit:** CORE-D07 recipe validation + report-only matrix review round 2.
**Depends on:** merged PR #276; worktree-root/phase reconciliation; explicit checkpoint approval.
**Followed by:** concrete CORE-D07 decision, then separately authored/approved W00 contract. Matrix approval does not authorize W00/W01 implementation.

Read the entire cumulative matrix, historical remediation decisions/work-packages/arithmetic plan, core source/manifest/tests, root manifest/toolchain, relevant workflows, and installed Cargo/monitor documentation. Do not modify those source/tooling files.

Permitted documentation changes:

- `docs/plans/2026-10-09-amari-core-verification-matrix.md`.
- This checkpoint document.
- Evidence under `docs/plans/core-matrix-evidence/recipe-validation/`.
- A subsequent review contract under `docs/plans/`.

After verification approval only, Cargo may generate artifacts under `target/d07-baseline/stable/` and `target/d07-baseline/nightly/`. These are explicit build-artifact exceptions, not permission for other file writes. Harness-managed state writes do not authorize agent edits to `.pi/`.

**Out of bounds:** source/test/manifest/dependency edits, scratch source files anywhere, lock regeneration, fetches/toolchain/target installation, proof execution, new AVX2 execution, full H execution, workflow changes/dispatch, commits/pushes/PRs, merge/tag/publication, monitor edits/resets/waivers. Do not create a shell/Python helper file or substitute Bash writes to escape monitoring.

## Task 1: Approve and reconcile the verification boundary

**TDD scenario:** Existing code — verification planning only; no source-writing RED/GREEN repair cycle.

**Step 1:** Request explicit authorization for this checkpoint's two-stage commands and report-only REVIEW round 2, presenting its scope and known-red nightly Clippy control. Stop on unclear/cancelled approval.

**Step 2:** Reconfirm actual parent Pi process cwd, session-header cwd, branch, source and lock. Required values: `.worktrees/core-matrix`; `chore/core-verification-matrix`; HEAD `ba1b7719f609e8f0f127d620ba6936cfffb61413`; lock SHA-256 `acb76177dfb6ad26413de8a56c8ac9a6f8c6f2ec866516e6f95ceb828b6159f5`.

```bash
pwd
readlink "/proc/$PPID/cwd"
git branch --show-current
git rev-parse HEAD
git status --short --branch
sha256sum Cargo.lock docs/plans/core-matrix-evidence/Cargo.lock.snapshot
sha256sum docs/plans/2026-10-09-amari-core-verification-matrix.md \
    docs/plans/2026-10-09-amari-core-matrix-validation-checkpoint.md
```

Read the first session-header line at `$PI_SESSION_FILE` through the read tool. A child cwd is not the parent check.

**Step 3:** Explicitly enter the bounded verification task using verification-before-completion, then read `.pi/superpowers-state.json` to confirm `currentPhase: verify`. Completing plan authoring is not D07 approval; production execution remains unauthorized.

Do not use `plan_tracker init`: it advances this installed monitor to `execute`. Updates do not change phase. Do not repeatedly reload mapped skills to reset state. Confirm a first-write branch reminder; any phase/TDD violation stops the task.

## Task 2: Load the exact recipe and pin compiler provenance

**TDD scenario:** Existing code — conformance checks before library execution.

**Step 1:** Inspect matrix section 3.2's canonical `baseline_cargo` block. It must pin rustc/rustdoc paths, remove encoded flags/external wrappers, force x86-64 baseline flags, preserve rustdoc `-D warnings`, and isolate artifacts. The observed user Cargo config selects `sccache`; this must be overridden.

**Step 2:** Use installed `stable` and `nightly` only for this research checkpoint. Expected rustc identities: stable `1.98.0 (88d9e12ae 2026-08-18)`; nightly `1.100.0-nightly (787af2b8c 2026-08-25)`. Record full Cargo/Clippy/rustdoc versions. Stop on mismatch; do not substitute another compiler or claim these aliases completed proposed exact compiler/MSRV/CI-nightly rows.

**Step 3:** Use the common setup below at the start of **each** Task 3 invocation. Copy this block and the chosen phase block into one Bash command enclosed in `( ... )`. They are inline verification commands, not a new helper file. Repeat setup in phase B; shell variables/functions do not survive separate tool calls.

Default label is `orchestrator`. An independently approved reviewer repeats both phases with `RUN_LABEL=review-round-2` before setup; later approved rounds use their actual number. Never overwrite an earlier run.

```bash
set -euo pipefail
RUN_LABEL=${RUN_LABEL:-orchestrator}
[[ "$RUN_LABEL" =~ ^(orchestrator|review-round-[2-6])$ ]]
ROOT="$PWD"
MATRIX=docs/plans/2026-10-09-amari-core-verification-matrix.md
CHECKPOINT=docs/plans/2026-10-09-amari-core-matrix-validation-checkpoint.md
EVIDENCE="$ROOT/docs/plans/core-matrix-evidence/recipe-validation/$RUN_LABEL"
LOCK_HASH=acb76177dfb6ad26413de8a56c8ac9a6f8c6f2ec866516e6f95ceb828b6159f5
SOURCE=ba1b7719f609e8f0f127d620ba6936cfffb61413
test "$(git rev-parse HEAD)" = "$SOURCE"
test "$(git branch --show-current)" = chore/core-verification-matrix
git diff --quiet -- amari-core Cargo.toml rust-toolchain.toml .github/workflows
printf '%s  Cargo.lock\n' "$LOCK_HASH" | sha256sum --check --status

WRAPPER=$(python3 - "$MATRIX" <<'PY'
import pathlib
import re
import sys
text = pathlib.Path(sys.argv[1]).read_text()
blocks = re.findall(r"```bash\n(.*?)\n```", text, flags=re.S)
selected = [b for b in blocks if b.startswith(
    "HOST=x86_64-unknown-linux-gnu\nbaseline_cargo() {")]
assert len(selected) == 1, "expected one canonical baseline wrapper"
print(selected[0])
PY
)
eval "$WRAPPER"

# Deliberately contaminated inherited values; only inside this subshell.
export CARGO_ENCODED_RUSTFLAGS=$'-C\x1ftarget-cpu=native\x1f-C\x1ftarget-feature=+avx2'
export CARGO_ENCODED_RUSTDOCFLAGS=$'-C\x1ftarget-cpu=native\x1f-C\x1ftarget-feature=+avx2'
export RUSTC=/__d07_invalid_compiler__ RUSTDOC=/__d07_invalid_documenter__
export CARGO_BUILD_RUSTC=/__d07_invalid_compiler__
export CARGO_BUILD_RUSTDOC=/__d07_invalid_documenter__
export RUSTC_WRAPPER=/__d07_invalid_wrapper__
export RUSTC_WORKSPACE_WRAPPER=/__d07_invalid_wrapper__
export CARGO_BUILD_RUSTC_WRAPPER=/__d07_invalid_wrapper__
export CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER=/__d07_invalid_wrapper__
export CARGO_BUILD_TARGET=thumbv7em-none-eabihf

assert_version() {
    local expected
    case "$T" in
        stable) expected='rustc 1.98.0 (88d9e12ae 2026-08-18)' ;;
        nightly) expected='rustc 1.100.0-nightly (787af2b8c 2026-08-25)' ;;
        *) return 1 ;;
    esac
    test "$(rustc +"$T" --version)" = "$expected"
}
assert_baseline_cfg() {
    local file=$1
    grep -Fxq 'target_arch="x86_64"' "$file"
    grep -Fxq 'target_os="linux"' "$file"
    if grep -Eq '^target_feature="(avx|avx2|fma)"$' "$file"; then
        printf 'STOP: forbidden target feature in %s\n' "$file" >&2
        return 1
    fi
}
runlog() {
    local name=$1 rc
    shift
    if "$@" >"$OUT/$name.log" 2>&1; then rc=0; else rc=$?; fi
    printf '%s\t%s\n' "$name" "$rc" >>"$OUT/status.tsv"
    printf '%s: exit %s (%s)\n' "$name" "$rc" "$OUT/$name.log"
    return "$rc"
}
```

## Task 3: Compile/document, inspect, then execute separately

**TDD scenario:** Existing code — positive conformance checks plus known-red Clippy control, with no repairs.

### Step 1: Phase A — no core library execution

Run common setup plus this block in one subshell. Capture raw outputs and exits, not success-masking pipelines. Stop on any failure. This phase builds dependencies/build scripts as ordinary Cargo does, but does not execute core test/example/SIMD functions.

```bash
for T in stable nightly; do
    assert_version
    OUT="$EVIDENCE/$T"
    test ! -e "$OUT"
    mkdir -p "$OUT"
    sha256sum Cargo.lock "$MATRIX" "$CHECKPOINT" >"$OUT/provenance.sha256"
    {
        git rev-parse HEAD
        rustc +"$T" --version --verbose
        cargo +"$T" --version
        cargo +"$T" clippy --version
        rustdoc +"$T" --version
        rustup which --toolchain "$T" rustc
        rustup which --toolchain "$T" rustdoc
    } >"$OUT/provenance.log" 2>&1

    rustc +"$T" --target "$HOST" -C target-cpu=x86-64 \
        -C target-feature=-avx2 --print cfg >"$OUT/direct-cfg.txt"
    assert_baseline_cfg "$OUT/direct-cfg.txt"
    if baseline_cargo rustc --locked --offline -vv -p amari-core --lib \
        --target "$HOST" -- --print cfg >"$OUT/cargo-cfg.txt" \
        2>"$OUT/cargo-cfg.log"; then cfg_rc=0; else cfg_rc=$?; fi
    printf 'cargo-cfg\t%s\n' "$cfg_rc" >>"$OUT/status.tsv"
    test "$cfg_rc" -eq 0
    assert_baseline_cfg "$OUT/cargo-cfg.txt"

    runlog check baseline_cargo check --locked --offline -vv \
        -p amari-core --target "$HOST" --all-targets
    runlog doc baseline_cargo doc --locked --offline -vv \
        -p amari-core --target "$HOST" --no-deps
    printf '%s  Cargo.lock\n' "$LOCK_HASH" | sha256sum --check --status
done
```

### Step 2: Mandatory inspection before phase B

Use the read tool to inspect each compiler's full cfg/provenance logs and relevant verbose rustc/rustdoc invocations. Verify pinned executable paths, target `x86_64-unknown-linux-gnu`, `target-cpu=x86-64`, `target-feature=-avx2`, documenter `-D warnings`, and absence of inherited native/AVX2 flags, invalid executables or external sccache. Distinguish host build-script/proc-macro commands from core target invocations. If relevant commands are missing/cached, record insufficient evidence and stop; do not infer flags from a successful exit.

For each compiler, only after this inspection passes, use the write tool to create `$EVIDENCE/<compiler>/inspection.md` with:

- exact line `Inspection verdict: PASS`;
- reviewed source, matrix/checkpoint/lock hashes;
- direct/effective cfg result and relevant log line references;
- observed rustc/rustdoc executable paths and flags;
- explicit statement that no core library tests have executed yet.

This record is an agent inspection result, not a formal proof or maintainer approval. A sentinel without actual inspection is invalid evidence. Do not modify either plan between phases: provenance hashes must continue to match.

### Step 3: Phase B — default tests and Clippy only

In a **new** subshell, repeat common setup verbatim, then run this block. It checks phase-A provenance and inspection records before any core runtime execution. First-run logs are not overwritten.

```bash
for T in stable nightly; do
    assert_version
    OUT="$EVIDENCE/$T"
    sha256sum --check --status "$OUT/provenance.sha256"
    grep -Fxq 'Inspection verdict: PASS' "$OUT/inspection.md"
    assert_baseline_cfg "$OUT/direct-cfg.txt"
    assert_baseline_cfg "$OUT/cargo-cfg.txt"
    test ! -e "$OUT/test.log"
    test ! -e "$OUT/clippy.log"

    runlog test baseline_cargo test --locked --offline -vv \
        -p amari-core --target "$HOST"
    if runlog clippy baseline_cargo clippy --locked --offline -vv \
        -p amari-core --target "$HOST" --all-targets -- -D warnings; then
        clippy_rc=0
    else
        clippy_rc=$?
    fi
    if test "$T" = stable; then
        test "$clippy_rc" -eq 0
    else
        test "$clippy_rc" -eq 101
        grep -Fq 'needless_range_loop' "$OUT/clippy.log"
        for line in 159 339 387; do
            grep -Fq "amari-core/src/verified.rs:$line:" "$OUT/clippy.log"
        done
    fi
    printf '%s  Cargo.lock\n' "$LOCK_HASH" | sha256sum --check --status
    git status --short --branch >"$OUT/final-status.txt"
done
```

### Step 4: Inspect results and record limits

Expected default check/test/doc exits are 0 on both compilers; default inventory is **284 unit/integration + 3 doctests**, none ignored. Inspect executed names/counts, not merely a zero exit. Stable Clippy is expected 0. Nightly Clippy is expected 101 with exactly the three known loop diagnostics at `verified.rs:159,339,387`; inspect the complete log to exclude additional errors. Presence checks in the script do not prove there were no other diagnostics.

Unexpected success/failure, compiler/config/source change, missing cached artifact, new diagnostic or guard violation: STOP and report. No fetch/install/re-resolution, source repair, broader retry or unapproved compiler substitution. Known nightly red is baseline evidence, **not** a green matrix row. Cargo timeout is not permission to change gates.

After both phases, archive a Markdown summary under the evidence root recording raw exits, cfg/flag inspection, exact test inventory, unchanged lock/source, and outstanding failures. Recipe conformance does not certify full H, aliases, exact compiler rows, MSRV, targets, unsafe paths, actual proofs or remediation readiness.

## Task 4: Independent cumulative review round 2

**TDD scenario:** Documentation scrutiny — falsifiable claims checked against source/evidence; no library edits.

**Step 1:** Freeze current source/document/lock hashes and write a review contract under `docs/plans/`, covering the entire matrix, this checkpoint, and newly recorded evidence, not only the latest edits. The contract must specify which commands the reviewer independently reproduces and which artifacts are inspected; never represent inspection as reproduction.

**Step 2:** Explicitly establish a supported review context with requesting-code-review and verify its actual root/phase. Child phase/cwd do not change the parent. Subsequent document corrections require an intentionally scoped planning context, not manual state reset or backward skill reloads to erase history.

**Step 3:** Use named `reviewer`, provider `openai-codex`, model `gpt-6-astra`, cwd `.worktrees/core-matrix`, record **report-only**. This matches prior scrutiny tier and exceeds the floor. No generation-tier/Anthropic fallback. Reviewer reproduction is limited to approved commands/artifact paths above; use `RUN_LABEL=review-round-2` if repeating both phases. No source helpers, installations, deferred unsafe/target/proof tests or full H runs.

Require scrutiny of these verbatim matrix claims:

1. “At this baseline these produce **1,440 distinct named closed sets**. For source/implementation coverage they normalize to **320 capability configurations**”. Check closure/domain reasoning, not just multiplication.
2. “A nonempty explicit `RUSTFLAGS` overrides Cargo-config rustflags; removing the encoded variable prevents it taking precedence.” Attack effective compiler/documenter/ISA isolation and archived evidence.
3. “Current native-precision documentation explicitly excludes WASM; preserve that restriction.” Check support promises/std-dependency blockers.
4. “Ordinary `requires`/`ensures` remove contract expressions; a green build does not even establish that those expressions are meaningful for proof translation.” Check proof/tooling boundaries.
5. “Role ownership here is a proposal requiring approval”. Check all schedule/scope language for inferred approval.

Output: P1/P2/P3 findings with file:line, exact reproduction, observed/expected, proposed fix, plus ambiguity footer. State **round 2**, breaker **6 total rounds**; prior round 0 P1 / 2 P2 / 0 P3. Prior verified corrections: inherited ISA contamination and overstated scheduling/W01 approval. Do not re-raise settled findings without new evidence; newly reproduced recipe behavior is legitimate evidence. Read the full report/footer.

## Task 5: Triage and request the concrete matrix decision

**TDD scenario:** Documentation fixes — verify findings before adopting corrections.

**Step 1:** Verify each finding; persist fixed/rejected-with-reason/deferred dispositions under `docs/plans/` with exact evidence. Restrict document corrections to the allowed plans; code/tooling/workflow/support changes need separate authorization.

**Step 2:** Repeat cumulative review after P1/P2 corrections, refreshing document hashes. Stop at six total rounds if findings remain; escalate. Convergence requires a round with zero P1/P2; record P3 without treating it as blocking.

**Step 3:** Present exact proposed compiler/configuration/target rows, scheduling, owners, known failures, unexecuted gates and review verdict. Ask for explicit CORE-D07 approval. Neither checkpoint approval nor convergence implies it.

**Step 4:** Only after D07 approval, author and obtain separate approval for bounded W00 against fresh develop. W01 needs rebaseline after W00 and its own refreshed contract approval. This checkpoint ends before generation dispatch or production implementation.

## Planning handoff

The maintainer confirmed `chore/core-verification-matrix` for documentation-only planning. Both documents remain uncommitted drafts. No executable command above or new review dispatch has run during authoring. Do not mark overall matrix planning/verification complete or offer W00/W01 execution choices merely because the drafts exist.

Next decision: authorize **only this bounded two-stage recipe validation + report-only review**, or retain the draft for more planning. Neither option approves production implementation, merges, or release actions.
