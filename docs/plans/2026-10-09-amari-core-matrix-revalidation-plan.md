# CORE-D07 Recipe Correction and Revalidation Implementation Plan

> **REQUIRED SUB-SKILL:** Use the executing-plans skill to implement this plan task-by-task after explicit approval. This is a documentation/recipe-conformance contract, not permission to implement W00/W01.

**Goal:** Resolve the two verified round-2 P2 findings with a narrowly corrected command recipe, actionable diagnostic gates and fresh bounded evidence.

**Architecture:** Preserve the failed run and reviewed inputs. Explicitly disable external wrappers through inherited environment overrides while requiring Clippy's own driver to remain active. Run one compiler and one command group at a time, inspect diagnostics and actual command chains before advancing, and execute tests only after pre-runtime controls pass.

**Tech Stack:** Bash, existing installed stable/nightly, Cargo/Clippy/rustdoc, immutable diagnostic logs, read/write tools for documentation/evidence, ia-moment REVIEW-class round 3.

---

## Authorization status

**DRAFT; CATALOGUE PREPARATION AUTHORIZED.** After drafting, the maintainer selected **“Accept scoped baseline policy”** through ask_user: prepare the frozen-warning catalogue under docs/plans; matching recorded registry-warning fingerprints may be non-gating in this checkpoint only. Exact catalogue disposition, canonical recipe edits, revalidation and review dispatch remain unapproved. Both R2 findings remain open. Source, lock, original matrix/checkpoint and failed artifacts remain unchanged.

Three separate approvals remain:

1. **Diagnostic disposition:** the scoped baseline policy is accepted; exact catalogue hash and the six recorded Cargo profile-hint notices still require disposition. A warning policy is not inferred from Cargo's exit status.
2. **Bounded contract execution:** approve documentation correction, fresh artifacts, listed commands and round-3 review. Do not begin before approval 1 is settled.
3. **Later CORE-D07 / W00 / W01 decisions:** not included here, regardless of recipe result.

If the operator chooses to keep all dependency warnings blocking, stop this checkpoint. Any dependency repair needs its own contract; do not suppress warnings, change dependencies or reinterpret that choice.

## Unit / dependencies / exact scope

**Unit:** CORE-D07-R2 remediation, documentation and bounded recipe verification only.
**Depends on:** [R2 report](core-matrix-evidence/recipe-validation/review-round-2-report.md), [verified triage](core-matrix-evidence/recipe-validation/review-round-2-triage.md), explicit approvals above.
**Followed by:** cumulative review round 3, concrete D07 approval, then separately bounded W00.

After contract approval, permitted changes:

- Modify `docs/plans/2026-10-09-amari-core-verification-matrix.md`: canonical wrapper, explicit artifact-directory requirement, current failed/pending status and corrected inspection sequence only. Do not alter feature/compiler/target rows, owner proposals or approval fences.
- Keep `docs/plans/2026-10-09-amari-core-matrix-validation-checkpoint.md` unchanged as the frozen failed recipe. Add a supersession reference in the matrix, not edits to the historical checkpoint.
- Modify this plan only before its execution snapshot is frozen; record any subsequent change as a new approval boundary.
- Create evidence only under `docs/plans/core-matrix-evidence/recipe-validation/revalidation-1/`.
- Create a later round-3 review contract under `docs/plans/` when that dispatch is separately within the approved scope.
- Cargo-generated artifacts only under `target/d07-baseline/revalidation-1/{stable,nightly}/`.

Do not alter production source/tests/manifests, Cargo/user configuration, dependency resolution, workflows, previous logs/inspection markers/reports, or harness state manually. No helper source files, fetch/install/network, target/proof/AVX2/full-H execution, Git mutations, publishing or implementation dispatch. Do not route source writes through Bash. This work has no commit/PR step: those actions are not authorized.

## Proposed diagnostic disposition — approval required

**Owner:** operator/core maintainer until expressly delegated.

For this recipe-only checkpoint, propose treating only **previously observed, registry-owned dependency-warning fingerprints from the frozen default-core logs** as recorded, non-gating baseline evidence. This does not assert dependencies are warning-free/correct and does not grant an exception for full H, CI/release, source changes or other compilers/configurations.

- Keep scoped core `-D warnings` unchanged. Any unexpected core warning/error blocks.
- Dependency errors always block.
- A new or unclassified warning fingerprint blocks, even from an already listed package.
- Normalize a dependency warning by compiler identity, package/version, warning header/code/message and first primary source location (registry prefix removed, relative file + line/column retained). Optional Rustc explanatory lint-note presence/absence is renderer metadata, not identity; record it without treating suppression of a repeated note as a new warning. Require that primary location to identify the registry package; do not borrow a package path from a later verbose command in the same text block.
- Count repeat/replayed blocks separately from unique fingerprints. A `Fresh` replay or changed multiplicity is recorded, not automatically a new regression; no missing diagnostic is represented as a tested absence without explaining command/lint caps.
- Cargo package-warning summaries are not separate defects. Classify summaries explicitly and retain them; unknown summaries stop for inspection. Known nightly build-failure summaries are accepted only with the precise expected core diagnostics/exit below.
- Retain every raw diagnostic and log; never filter the actual gate output to hide warnings.

The previous 94/732 warning-block counts remain historical reported counts, **not** a new allowlist, unique-defect count, or permission to accept every warning in those packages. Generate/review a normalized catalogue from the exact frozen logs before any new Cargo invocation, then obtain an explicit catalogue disposition. If attribution is ambiguous, classify it BLOCKED, not accepted.

Frozen input hashes (paths relative to `core-matrix-evidence/recipe-validation/orchestrator/`):

| Log | SHA-256 |
|---|---|
| stable/cargo-cfg.log | `4350820eb613bb7aaca64f907e5f2ae12f966ec7a919ea55356388b845bcda1c` |
| stable/check.log | `37238c168a7e037e988a8339dbe3a5374ab395176c6f38ef94716bfd6138b848` |
| stable/doc.log | `4802f35964f5b614d85144a4670e3a7f1a7d2c811830150f7d4775a742fa6fb3` |
| stable/clippy.log | `cec59b3b2e103a546bf4d839e5c02be0b27cdc806abdfb6f7c7499723fd6e98c` |
| stable/test.log | `9e3a0383161b64fd1cbb617ec354288a562e2cd6662db31e628a0feb0be81f2e` |
| nightly/cargo-cfg.log | `2d17c30ab46b33f29037ca303b237e34e7844b05a842219431bc174191f63ff8` |
| nightly/check.log | `6d96d239d8eb5346fc80511c9d5833b646fa91d119552a7c678e91389935562f` |
| nightly/doc.log | `991332c5c8522833909d6673fd687a986d57c5065350be2f59c28b9cc5b5b21c` |
| nightly/clippy.log | `071b52d58a580600c14da5097eda4bae30f782b1f6a97b1813d30b35eefb79b5` |
| nightly/test.log | `2be37b04a0b79bb59dea04bb78bbc25d6dd7d229df9f986e1a9a9896bb0fb9c5` |

## Task 1: Approve the diagnostic catalogue before execution

**TDD scenario:** Trivial documentation/data classification; no production change or executable regression test.

**Step 1:** Verify the ten raw-log hashes above using `sha256sum`; read complete diagnostic blocks, not merely summary counts. Read-only text analysis is allowed in the planning context. Do not create a parser/helper source file or run Cargo. Before authorized catalogue preparation creates its evidence root, perform the exclusive-destination checks:

```bash
test ! -e docs/plans/core-matrix-evidence/recipe-validation/revalidation-1
test ! -e target/d07-baseline/revalidation-1
test ! -L target
test ! -L target/d07-baseline
```

Create the evidence root exclusively (fail if it appeared between checking and creation), then record its ownership; do not create the target root yet. None of these creation steps is authorized by drafting permission.

**Step 2:** After catalogue-preparation authority is expressly included in approval, create `revalidation-1/diagnostic-catalogue.md`. Use the write tool for authorization/ownership records; the large catalogue may be generated mechanically with inline Python as documentation data solely inside this explicitly approved evidence root. No helper source file or out-of-scope write is permitted; require a nonexistent catalogue destination. List normalized warning fingerprints, exact source-log line references, duplicate counts and every unresolved attribution/summary. A table has columns: compiler hash, package/version, header/code/message, primary relative file:line:column, evidence references, proposed disposition. Do not present the catalogue as already created or approved by this draft.

**Step 3:** Ask the owner to approve the catalogue's exact hash and scope-limited disposition. Present it with the complete bounded execution contract in one focused checkpoint decision, explicitly including or excluding the six Cargo metadata notices. Record their response in `revalidation-1/APPROVALS.md`. An unresolved fingerprint or cancelled disposition blocks new Cargo commands. Catalogue-only approval does not imply approval of canonical edits or the command contract.

## Task 2: Preserve inputs and prepare the candidate after approval

**TDD scenario:** Modifying a tested recipe — frozen failed-conformance commands are the RED evidence; candidate remains untested until the controls below run.

**Step 1:** Reconfirm actual parent Pi/session/tool cwd, branch `chore/core-verification-matrix`, source `ba1b7719f609e8f0f127d620ba6936cfffb61413`, clean tracked non-plan state and root lock hash `acb76177dfb6ad26413de8a56c8ac9a6f8c6f2ec866516e6f95ceb828b6159f5`. Source/configuration/dependencies must not change to obtain conformance.

**Step 2:** Verify the catalogue task's ownership record and required approvals in the already created evidence root. Refuse an existing target root again immediately before verification preparation. Do not clean, overwrite or reuse the failed run. Append only named fresh evidence files to the owned root.

```bash
test -f docs/plans/core-matrix-evidence/recipe-validation/revalidation-1/APPROVALS.md
test ! -e target/d07-baseline/revalidation-1
test ! -L target
test ! -L target/d07-baseline
```

**Step 3:** Before editing the matrix, archive its current complete bytes and the historical checkpoint through read/write tools into:

- `revalidation-1/inputs/matrix-before.md`, expected SHA-256 `40c43115dd04bc0569270ab9c1715e6912b311dc4aff1d49a5aee341db38c938`.
- `revalidation-1/inputs/checkpoint-failed.md`, expected SHA-256 `c59543bbf2199cc7ea4a411f289da22f86f8a8a4927e4eed18c23941fe85b510`.

Verify copies before any correction. Failed raw logs/reports/markers stay untouched.

**Step 4:** Replace matrix section 3.2's wrapper with the exact candidate below only after contract approval. This adds explicit-empty overrides and an explicitly selected artifact directory; no algorithm/API/library change is involved. Required `D07_BASELINE_TARGET_DIR` avoids silently using the failed cache. Document its required assignment for future H rows; full H itself remains out of scope.

```bash
HOST=x86_64-unknown-linux-gnu
baseline_cargo() {
    local compiler documenter artifact_dir
    compiler=$(rustup which --toolchain "$T" rustc) || return
    documenter=$(rustup which --toolchain "$T" rustdoc) || return
    artifact_dir=$(realpath -m "${D07_BASELINE_TARGET_DIR:?set an approved compiler-specific artifact directory}") || return
    case "$artifact_dir" in
        "$PWD"/target/d07-baseline/*) ;;
        *) printf 'STOP: out-of-scope artifact directory\n' >&2; return 2 ;;
    esac
    env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_ENCODED_RUSTDOCFLAGS \
        -u CARGO_BUILD_TARGET -u CARGO_BUILD_RUSTC -u CARGO_BUILD_RUSTDOC \
        -u CARGO_BUILD_RUSTC_WRAPPER -u CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER \
        RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER='' \
        RUSTC="$compiler" RUSTDOC="$documenter" \
        RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2' \
        RUSTDOCFLAGS='-D warnings -C target-cpu=x86-64 -C target-feature=-avx2' \
        CARGO_TARGET_DIR="$artifact_dir" \
        cargo +"$T" --config 'build.rustc-wrapper=""' \
            --config 'build.rustc-workspace-wrapper=""' "$@"
}
```

**Step 5:** Label the candidate unvalidated and link failed evidence/R2 findings in the matrix. Do not label wrapper isolation fixed yet. Freeze the corrected matrix and this plan's hashes, plus source/lock/candidate/catalogue/approval records in `revalidation-1/inputs/provenance.md`. Also create `revalidation-1/inputs/provenance.sha256` containing `sha256sum` output for the corrected matrix, this plan, Cargo.lock, diagnostic catalogue, APPROVALS and unchanged user Cargo config. Use the exact existing `.cargo/config.toml` (hash only, no credential/configuration dumps); absence/change stops the control. Do not edit frozen inputs between commands; any correction stops the run and needs a revised contract/snapshot.

## Task 3: Establish the approved verification context and common setup

**TDD scenario:** Existing code — conformance controls, not source implementation.

**Step 1:** Explicitly enter verification-before-completion for this approved task and inspect actual phase/root. Do not initialize plan_tracker (it advances to execute), manually reset state or reload mapped skills to erase history. A guard violation stops the task. Phase labels never certify D07 or library correctness.

**Step 2:** Record exact versions and confirm installed aliases match:

- stable rustc `1.98.0 (88d9e12ae 2026-08-18)`;
- nightly rustc `1.100.0-nightly (787af2b8c 2026-08-25)`.

No compiler installation/substitution. These are research aliases, not completed exact-date/MSRV/CI rows.

**Step 3:** For each command invocation, repeat the following setup in a subshell and load the single candidate Bash block from the frozen corrected matrix into that subshell. Use read-only Markdown extraction as in the previous checkpoint; assert exactly one `HOST=...\nbaseline_cargo() {` block, then evaluate only that reviewed block. No helper file is created. Set T to **stable first**; no nightly command until stable's entire group is inspected/dispositioned. Repeat with T=nightly later. Do not loop over compilers in one tool call.

```bash
set -euo pipefail
T=stable  # Change only to nightly after the stable checkpoint is accepted.
HOST=x86_64-unknown-linux-gnu
OUT="$PWD/docs/plans/core-matrix-evidence/recipe-validation/revalidation-1/$T"
D07_BASELINE_TARGET_DIR="$PWD/target/d07-baseline/revalidation-1/$T"
BASE="$PWD/docs/plans/core-matrix-evidence/recipe-validation/revalidation-1"
MATRIX=docs/plans/2026-10-09-amari-core-verification-matrix.md
test "$(git rev-parse HEAD)" = ba1b7719f609e8f0f127d620ba6936cfffb61413
test "$(git branch --show-current)" = chore/core-verification-matrix
git diff --quiet -- amari-core Cargo.toml rust-toolchain.toml .github/workflows
sha256sum --check --status "$BASE/inputs/provenance.sha256"
test -d "$OUT"
case "$T" in
    stable) expected='rustc 1.98.0 (88d9e12ae 2026-08-18)' ;;
    nightly) expected='rustc 1.100.0-nightly (787af2b8c 2026-08-25)' ;;
    *) exit 2 ;;
esac
test "$(rustc +"$T" --version)" = "$expected"
WRAPPER=$(python3 - "$MATRIX" <<'PY'
import pathlib
import re
import sys
blocks = re.findall(r"```bash\n(.*?)\n```", pathlib.Path(sys.argv[1]).read_text(), flags=re.S)
selected = [b for b in blocks if b.startswith(
    "HOST=x86_64-unknown-linux-gnu\nbaseline_cargo() {")]
assert len(selected) == 1, "expected one frozen candidate wrapper"
print(selected[0])
PY
)
eval "$WRAPPER"
# Read the owner approvals and catalogue before every command; file presence
# and SHA checks attest immutable input, not permission or diagnostic success.

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

runlog() {
    local name=$1 rc
    shift
    test ! -e "$OUT/$name.log" || return 2
    if "$@" >"$OUT/$name.log" 2>&1; then rc=0; else rc=$?; fi
    printf '%s\t%s\n' "$name" "$rc" >>"$OUT/status.tsv"
    printf '%s: exit %s (%s)\n' "$name" "$rc" "$OUT/$name.log"
    return "$rc"
}
```

Preserve the unchanged user sccache configuration as the configuration-level negative control. Environment contamination is confined to the subshell; no user configuration edit is permitted. The candidate must replace/remove it, not invoke invalid executables. Empty `RUSTC_WORKSPACE_WRAPPER` is a candidate input, not permission to suppress Clippy's own driver.

## Task 4: Run and inspect each non-runtime command separately

**TDD scenario:** Modifying a tested recipe — conformance must reject the old external-wrapper chain and accept only the observed corrected chain.

Prepare each compiler once before its first command with the block below. Use T=stable first; replace only with nightly after stable's complete inspection checkpoint has been accepted. Do not create/reinitialize OUT in the repeated common setup.

```bash
(
set -euo pipefail
T=stable
BASE="$PWD/docs/plans/core-matrix-evidence/recipe-validation/revalidation-1"
test ! -e "$BASE/$T"
test ! -e "$PWD/target/d07-baseline/revalidation-1/$T"
mkdir "$BASE/$T"
{
    git rev-parse HEAD
    rustc +"$T" --version --verbose
    cargo +"$T" --version
    cargo +"$T" clippy --version
    rustdoc +"$T" --version
    rustup which --toolchain "$T" rustc
    rustup which --toolchain "$T" rustdoc
} >"$BASE/$T/versions.log" 2>&1
)
```

Use fresh OUT directory for each compiler. Each step below is a separate tool invocation followed by full relevant output/diagnostic inspection and a written inspection record under OUT. Do not run the next step automatically. Compare normalized dependency warnings to the approved catalogue; unknowns block. Evidence records include source/input/catalogue hashes and exact log references; a marker without inspection is invalid.

**Step 1: Direct and effective cfg.** No core library execution.

```bash
test ! -e "$OUT/direct-cfg.txt"
test ! -e "$OUT/cargo-cfg.txt"
test ! -e "$OUT/cargo-cfg.log"
rustc +"$T" --target "$HOST" -C target-cpu=x86-64 \
    -C target-feature=-avx2 --print cfg >"$OUT/direct-cfg.txt"
if baseline_cargo rustc --locked --offline -vv -p amari-core --lib \
    --target "$HOST" -- --print cfg >"$OUT/cargo-cfg.txt" \
    2>"$OUT/cargo-cfg.log"; then rc=0; else rc=$?; fi
printf 'cargo-cfg\t%s\n' "$rc" >>"$OUT/status.tsv"
test "$rc" -eq 0
for p in "$OUT/direct-cfg.txt" "$OUT/cargo-cfg.txt"; do
    grep -Fxq 'target_arch="x86_64"' "$p"
    grep -Fxq 'target_os="linux"' "$p"
    if grep -Eq '^target_feature="(avx|avx2|fma)"$' "$p"; then exit 2; fi
done
```

Inspect compiler path/target/baseline flags, destination under the fresh target root, and all diagnostics before Step 2. Empty/cached cfg or missing relevant command is insufficient evidence: stop without cleaning/retrying.

**Step 2: All-target compilation, then inspect.**

```bash
runlog check baseline_cargo check --locked --offline -vv \
    -p amari-core --target "$HOST" --all-targets
```

Expected 0. Inspect actual core rustc invocations and classify every diagnostic before Step 3. No tests/examples/benchmarks execute.

**Step 3: Rustdoc, then inspect.**

```bash
runlog doc baseline_cargo doc --locked --offline -vv \
    -p amari-core --target "$HOST" --no-deps
```

Expected 0. Inspect pinned rustdoc, explicit target/baseline flags, `-D warnings`, no external wrapper and the fresh output path. Classify diagnostics before Step 4. Documentation generation is not proof or runtime evidence.

**Step 4: Clippy BEFORE tests, then inspect.**

```bash
if runlog clippy baseline_cargo clippy --locked --offline -vv \
    -p amari-core --target "$HOST" --all-targets -- -D warnings; then
    clippy_rc=0
else
    clippy_rc=$?
fi
case "$T" in
    stable) test "$clippy_rc" -eq 0 ;;
    nightly) test "$clippy_rc" -eq 101 ;;
    *) exit 2 ;;
esac
```

Required inspection, not just exit checks:

- Actual core lib and lib-test chains directly use selected `clippy-driver` and selected rustc with **no external sccache/other wrapper**.
- Baseline flags and explicit target retained; no native/+AVX2/FMA flags.
- All outputs under the fresh compiler target directory; no failed-run cache reuse.
- Stable has no unexpected core diagnostics. Nightly's unique core-error fingerprints are exactly the three known `needless_range_loop` errors at primary locations `verified.rs:159,339,387`, with ordinary corresponding build-failure summaries and no other error fingerprints. Record any repeated lib/lib-test rendering separately with its target provenance; do not assume raw diagnostic-block multiplicity is a unique-error count.
- Nightly success is a failed lint-execution control, not a green gate. Driver absence, missing diagnostic evidence, additional core diagnostic or unclassified dependency warning/error stops the run.

After all four inspections/dispositions pass, write `OUT/pre-runtime-inspection.md` with explicit accepted hashes, command chains, cfg, diagnostics and verdict. The nightly known-red control is recorded as red; acceptance is recipe conformance only.

## Task 5: Execute tests only after pre-runtime acceptance

**TDD scenario:** Existing code — unchanged default regression inventory; no repairs.

**Step 1:** Recheck actual root/phase, source/lock/frozen input/catalogue hashes and read the compiler's pre-runtime inspection record. No CLI command can replace this review. Stop if a prerequisite is pending or changed.

**Step 2:** Run only:

```bash
runlog test baseline_cargo test --locked --offline -vv \
    -p amari-core --target "$HOST"
```

**Step 3:** Inspect raw runtime/doctest commands, flags, fresh paths, names and every diagnostic before accepting the compiler or advancing. Expected 0; 202 unit + 82 integration = 284, plus 3 doctests; none failed/ignored/filtered. No new unsafe SIMD functions should be compiled/executed. Ordinary baseline std/dependency behavior is not SIMD safety certification.

**Step 4:** Record stable acceptance before beginning Task 4 for nightly. Record nightly separately. New/unclassified warning, dependency error, unexpected core failure/success, cache/path/config/source drift or guard warning stops immediately. No automatic retry, code repair, installation or broader permission.

## Task 6: Archive result and obtain cumulative round 3 review

**TDD scenario:** Documentation/evidence review; no source changes.

**Step 1:** Verify old inputs/logs remained byte-identical, new target/evidence roots are separate, tracked non-plan diff remains empty, and lock unchanged. Archive raw exits, all inspection decisions, normalized-warning comparisons/counts, test inventory and remaining baseline red. Do not call full H, portability, safety or proofs green.

**Step 2:** Author a cumulative round-3 review contract under docs/plans after revalidation, then obtain only the separately approved REVIEW-class dispatch: named reviewer, openai-codex/gpt-6-astra, worktree cwd, report-only, maximum six total rounds. Include both earlier rounds, their dispositions and newly validated evidence. Do not downgrade or permit unspecified reruns.

**Step 3:** Verify findings before fixes. R2-F1 closes only with actual no-external-wrapper Clippy chains and genuine lint execution; R2-F2 closes only with approved diagnostic disposition and demonstrated pauses/decisions before runtime/next compiler. Convergence requires a round with zero P1/P2. Break at six with unresolved findings.

## Handoff and limits

The recipe correction and revalidation remain **unexecuted**. Drafting and catalogue preparation were authorized; existing failed/reviewed recipes remain unchanged and the candidate is not applied or validated. Prepared catalogue: `core-matrix-evidence/recipe-validation/revalidation-1/diagnostic-catalogue.md`, SHA-256 `64c6185bd65192e878cbf2bb49992d42b01b7d0d5b6a65a618e705bf459be9dc`; 600 normalized source-attributed warning identities across 602 rendering variants, with six Cargo profile-hint notices separately pending. Next approval must expressly disposition that catalogue/notices AND authorize the complete bounded correction/revalidation/review contract. No execution is inferred from drafting or policy approval.

No W00/W01 execution choice is offered here. D07 still needs concrete rows/compiler/scheduling/owner approval after review. Every later implementation package requires its own bounded approved contract and fresh baseline.

## Execution record amendment (2026-10-09)

The maintainer directed the minimal path: one-line wrapper correction plus bounded rerun, with the catalogue demoted to reference-only comparison. Executed per that authorization; conformance restored for the default-core checkpoint scope. See `core-matrix-evidence/recipe-validation/revalidation-1/SUMMARY.md`. Tasks 1–6 above are superseded by that record for this run; the frozen failed run and both review rounds remain archived.
