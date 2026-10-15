# CORE-D07 Review Round 2 — Read-only Failed-Recipe Diagnosis

## Dispatch class / target / record

- **Class:** REVIEW; named `reviewer`, `openai-codex`, `gpt-6-astra` (match the prior matrix scrutiny tier; no downgrade or fallback).
- **Record:** report-only; no GitHub posting. No matrix PR exists. Return findings in the moment result; orchestrator persists the report before triage.
- **Round:** 2 of maximum 6 total rounds; full cumulative document review, with this round's executable reproduction explicitly forbidden.
- **Repo/cwd:** `/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix`.
- **Branch:** `chore/core-verification-matrix`.
- **Base/source HEAD:** `ba1b7719f609e8f0f127d620ba6936cfffb61413`; relevant library source unchanged. Target is uncommitted working-tree documentation/evidence, not `HEAD~1..HEAD` or merged PR #276.
- **Matrix:** `docs/plans/2026-10-09-amari-core-verification-matrix.md`, SHA-256 `40c43115dd04bc0569270ab9c1715e6912b311dc4aff1d49a5aee341db38c938`.
- **Checkpoint:** `docs/plans/2026-10-09-amari-core-matrix-validation-checkpoint.md`, SHA-256 `c59543bbf2199cc7ea4a411f289da22f86f8a8a4927e4eed18c23941fe85b510`.
- **Evidence:** `docs/plans/core-matrix-evidence/recipe-validation/orchestrator/`, beginning with `SUMMARY.md` and each compiler's provenance/cfg/inspection/status/command logs.
- **Root lock:** SHA-256 `acb76177dfb6ad26413de8a56c8ac9a6f8c6f2ec866516e6f95ceb828b6159f5`.

## Human authorization and workflow boundary

The maintainer authorized the bounded recipe checkpoint, which has now run and **FAILED CONFORMANCE**. The orchestrator stopped before review dispatch/reruns after unexpected warnings and actual external-wrapper evidence. The maintainer then separately authorized **independent read-only diagnosis**, explicitly excluding fixes, builds/tests/installs and revalidation. This contract governs that narrower authority.

Parent process/tool/session-header cwd and branch have been checked. Parent monitor explicitly entered `review`; `verification.verified` remains false and TDD idle with no source/test files. Its automatic phase label `verify: complete` is a navigation artifact, **not** a successful verification verdict. Actual conformance remains failed. Child cwd/phase never certify a parent root/phase.

Before review, confirm your actual tool/parent-process cwd, session-header cwd, branch and target hashes read-only. Load requesting-code-review once to establish your own review phase, then inspect actual state. Do not initialize plan_tracker, change branch/root, edit guard state, waive verification, or reload phases to erase history. A phase/TDD warning stops the task. If a first-write reminder occurs, report it; no agent-authored writes are authorized in this review.

## What was implemented / requirements comparison

Documentation only: complete extended matrix decision draft, proposed scheduling/owners/approval fences, sanitized command recipe and bounded validation checkpoint. No production library remediation, new dependency, workflow change or implementation dispatch occurred.

Compare the cumulative draft against the earlier CORE-D07 decisions/work-packages and explicit maintainer choices:

- Complete extended matrix definition before implementation.
- Retain portability promises (host baseline/AVX2, real WASM + Node, genuine alloc-based embedded library check), with precise scheduling still pending approval.
- Stage actual deductive proofs before 1.0; ordinary formal-feature compilation is not proof.
- Neither D07 nor W00/W01 implementation is approved; compatibility is not a constraint on future correctness repairs.

Review current document consistency and whether the failed recipe can support its claims. This is not production-readiness certification, mathematical closure of all findings, or a full feature/toolchain/target/proof re-run.

## Allowed read-only inspection

Read complete target documents and relevant evidence. Read historical audit/planning documents, root/core Cargo manifests, Cargo configuration, relevant workflow YAML and installed Cargo/Clippy/Pi monitor documentation/source as needed for command/config/phase claims. Do not extend this into the remaining production-source mathematical audit.

Allowed Bash operations: file discovery/search, line-number extraction, hashing, read-only git status/diff/show/rev-parse, and nonexecuting shell syntax scrutiny. You may analyze existing text/output; do not execute document scripts/snippets. Do not create a helper file, report file, log, marker or scratch file through any tool; return your report in the final response. Harness-managed session files are not permission for manual state writes.

**Forbidden:** Cargo/rustc/rustup/Clippy invocations (including fresh probes), tests/builds, execution of checkpoint Python/Bash blocks, environment experiments, installs/fetches/network queries, writes/edits/deletes, Git mutations, workflow/GitHub actions, source fixes, new AVX2 or proof runs, full H enumeration execution, further moment dispatch. Do not bypass this through Bash. No runtime result may be described as independently reproduced in this round.

## Prior round and dispositions

Round 1: **0 P1 / 2 P2 / 0 P3**, not converged.

1. **P2 inherited ISA flags could reach deferred SIMD:** independently verified that native host CPU flags enable AVX/AVX2/FMA. Corrected documentation to sanitize flags/wrappers, assert effective cfg before execution and isolate artifacts. Newly run direct/Cargo cfg and phase-A rustc/rustdoc checks passed their ISA controls. The new Clippy wrapper evidence below is a legitimate new reproduction against the correction, not mere disagreement with prior triage.
2. **P2 overstated approval:** corrected “maintainer-selected” precise portability scheduling and purported approved W01 scope. Concrete row timing/ownership and refreshed W01 implementation scope are explicitly pending. Do not re-raise this as settled wording unless a changed line or new contrary evidence establishes a remaining error.

There is no PR thread for this report-only review. These dispositions and the frozen evidence are its record. Review the cumulative documents, not only the latest wrapper changes. Convergence requires a round with zero P1/P2; a later corrected recipe still needs separately authorized executable validation. At six rounds with unresolved findings, stop and escalate; never auto-merge.

## Primary observed failure — inspect, do not rerun

The canonical wrapper removes RUSTC_WRAPPER/CARGO_BUILD_RUSTC_WRAPPER and supplies empty outer `cargo --config` wrapper values. User `~/.cargo/config.toml` selects `build.rustc-wrapper = "sccache"`.

Phase-A observed direct compiler/documenter calls had pinned rustc/rustdoc paths, `--target x86_64-unknown-linux-gnu`, `-C target-cpu=x86-64 -C target-feature=-avx2`, rustdoc `-D warnings`; effective cfg has no AVX/AVX2/FMA. Default checks/docs/tests exited 0; each compiler executed 284 unit/integration + 3 doctests, none ignored. Stable Clippy exited 0; nightly exited 101 with the three known loop diagnostics at `verified.rs:159,339,387`.

**But actual Clippy core commands include external sccache:**

- `stable/clippy.log:762,1023`.
- `nightly/clippy.log:6985,7485`; also final failed command descriptions following diagnostics.

```text
sccache <toolchain>/bin/clippy-driver <toolchain>/bin/rustc ...
```

Installed Cargo reference `environment-variables.html:218–231` documents that empty wrapper environment values override configuration and disable their corresponding wrapper. Unsetting differs from that override. Diagnose the command/config boundary from available references/evidence; distinguish an established mechanism from an untested candidate fix. Preserve Clippy's own driver/lint execution. Do not approve a no-op Clippy gate.

Verbose logs also expose 94 stable / 732 nightly registry-dependency warning blocks, with owners/counts recorded in SUMMARY. No new core error kind was observed, but the checkpoint's “new diagnostic” stopping rule requires a precise disposition. Distinguish scoped core warning-denied gates, Cargo dependency lint caps, verbose replay, new regressions and blanket warning exemptions. Do not assume permission to suppress diagnostics, change dependencies, or weaken matrix gates.

## Claims under scrutiny — verbatim from the cumulative draft

1. “Wrappers are disabled for this evidence gate so they cannot inject unrecorded compiler options.” Assess against the actual Clippy commands, not exit codes.
2. “A nonempty explicit `RUSTFLAGS` overrides Cargo-config rustflags; removing the encoded variable prevents it taking precedence.” Check recorded cfg/flags and configuration precedence separately from wrapper isolation.
3. “Apply matching baseline flags to rustdoc; removing encoded rustdoc flags also preserves `-D warnings`.” Check phase-A and doctest evidence; do not extrapolate to unexecuted targets/proofs.
4. “At this baseline these produce **1,440 distinct named closed sets**. For source/implementation coverage they normalize to **320 capability configurations**”. Check documentary/manifest reasoning only. State what remains unverified without executing generators or extending the source audit.
5. “Role ownership here is a proposal requiring approval”. Check cumulative scope/scheduling and authorization statements for misleading inference, distinguishing frozen pre-execution drafting status from actual checkpoint authorization/results in SUMMARY.
6. “Ordinary `requires`/`ensures` remove contract expressions; a green build does not even establish that those expressions are meaningful for proof translation.” Scrutinize evidence boundaries; no new proof or full formal audit.

## Required output

- P1 blocking / P2 should-fix / P3 advisory findings with exact file:line, read-only reproduction/inspection command or input, observed vs. required, and a bounded suggested correction.
- Classify the external-wrapper failure and additional-diagnostic rule precisely. Do not count every dependency warning as an independently verified defect.
- Separate verified observations, supported root-cause conclusions, hypotheses requiring new executable controls, and scope-limited/unverified claims.
- Propose the smallest **future approval contract** for correction/revalidation: exact recipe change candidate, permitted artifacts, negative controls, actual Clippy driver/wrapper/cfg inspections, known-red disposition, no cache/log overwrite, no production edits/installation/unsafe execution. Do not implement or run it.
- Overall verdict and convergence status; this round cannot certify executable repair even if documentary findings were absent.
- Mandatory footer: **ambiguities I resolved and how**, including which prior artifacts were inspected vs. trusted and why each unexecuted claim remains unverified.

Finite report; do not paste megabytes of raw logs or include unrelated credentials/environment data. No praise padding; critique the artifact/claims, not the operator.
