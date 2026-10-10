# CORE-D07 Review Round 2 — Returned Report

Record: report-only, received from moment “CORE-D07 R2 read-only diagnosis”.
Reviewer: named `reviewer`, `openai-codex / gpt-6-astra`.
Session: `/home/lucien/.pi/agent/sessions/--home-lucien-working-industrial-algebra-Amari-.worktrees-core-matrix--/2026-10-09T02-59-00-842Z_0ad5fb2f-0933b9c2-1bbf4230-27b2.jsonl`.

The report below is the reviewer's returned evidence/classification, not an orchestrator assertion of executable reproduction or repair. Triage is recorded separately. No correction/revalidation was authorized by this review.

---

## Round 2 verdict: NOT CONVERGED

**0 P1 / 2 P2 / 0 P3.** Failed recipe conformance remains unresolved. No commands were rerun, no files were authored, and nothing was posted to GitHub.

### Context confirmed

- Tool cwd, parent Pi process cwd, and session-header cwd: `/home/lucien/working/industrial-algebra/Amari/.worktrees/core-matrix`.
- Provider/model: `openai-codex / gpt-6-astra`.
- Branch: `chore/core-verification-matrix`.
- HEAD: `ba1b7719f609e8f0f127d620ba6936cfffb61413`.
- Matrix, checkpoint, and lock hashes exactly match the review contract; archived lock matches the root lock.
- Loaded requesting-code-review once. Actual state: `review: active`, TDD idle, verification false, no waiver. No phase/TDD warning or first-write reminder occurred.
- Tracked diff is empty. Relevant source/manifests/workflows also match the historical audit baseline.

## Findings

### P2 — External-wrapper isolation fails across the Clippy boundary

**Location:** `docs/plans/2026-10-09-amari-core-verification-matrix.md:119–130`.

**Read-only inspection:** Search `recipe-validation/orchestrator/{stable,nightly}/clippy.log` for `sccache .*clippy-driver`; inspect stable lines **762,1023**, nightly **6985,7485**, and `~/.cargo/config.toml:2`.

**Observed:** Core library and library-test commands invoke:

```text
sccache <selected-toolchain>/bin/clippy-driver <selected-toolchain>/bin/rustc …
```

**Required:** Clippy’s driver must execute without an external wrapper. Stable exit 0 does not establish that property.

**Diagnosis:** Unsetting `RUSTC_WRAPPER` removes the environment override; it does not disable the user-configured wrapper. Installed Cargo documentation explicitly distinguishes an empty value from an unset variable. The outer `--config` settings demonstrably fail to isolate the observed Clippy path. External-subcommand dispatch and a subsequent Cargo configuration boundary explain the evidence, but the exact installed Clippy forwarding implementation was not available for source-level confirmation.

**Suggested correction:** In the canonical environment, replace the two `-u RUSTC_*WRAPPER` entries with explicit `RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER=''`, retaining removal of `CARGO_BUILD_*WRAPPER` and the other controls. Treat this as an **untested candidate**, not a repaired recipe. Verify that Clippy still installs and executes its own driver.

### P2 — The additional-diagnostic stop rule lacks an actionable inspection boundary and disposition

**Location:** `docs/plans/2026-10-09-amari-core-matrix-validation-checkpoint.md:212–238`.

**Read-only inspection:** Compare the phase-B loop and stopping rule with `orchestrator/stable/check.log:278–279`, `stable/clippy.log:27–29`, both `status.tsv` files, and `SUMMARY.md`’s diagnostic inventory.

**Observed:** A dependency warning already appears during phase A, is replayed beside `Fresh plotters-backend` during Clippy, and both compilers subsequently complete phase B. The script checks exits and presence of known nightly errors; complete diagnostic inspection is deferred until afterward. Thus “new diagnostic: STOP” neither defines comparison scope nor reliably interrupts the command sequence.

**Required:** Before resuming, specify what constitutes a newly encountered diagnostic, who dispositions it, and where execution pauses. “No new core error kind” is narrower than the checkpoint’s current rule.

**Suggested correction:** Require diagnostic review between bounded command groups, before runtime execution or progression to the next compiler. Obtain explicit disposition of the frozen dependency-warning inventory. Preserve warning-denied core gates and stop on unexpected core diagnostics, dependency errors, or unclassified warning changes; do not silently redefine the rule as “ignore dependencies.”

## Evidence classification

**Verified by inspection—not executable reproduction:**

- Direct/effective cfg records exclude AVX, AVX2 and FMA.
- Recorded core rustc/rustdoc commands retain pinned paths, explicit host target and baseline flags.
- Documentation and doctest rustdoc invocations retain `-D warnings`.
- Each compiler’s result summaries report **284 unit/integration tests + 3 doctests**, none ignored.
- Nightly diagnostics identify exactly the three recorded loop locations; their warning-denied failure demonstrates real lint execution.

**Dependency warnings:** The summary reports **94 stable / 732 nightly diagnostic blocks**. Simple warning-line counts are larger because they include package/build summaries. I inspected representative registry-owned warnings, their phase-A counterparts, `Fresh` replay, and dependency commands carrying `--cap-lints warn`. These are not 826 independently established defects, nor evidence that scoped core `-D warnings` was disabled. Their complete owner attribution/counting remains inherited evidence, not independently reconstructed here.

**Unsupported conclusions:** No evidence establishes injected ISA options by sccache, newly introduced dependency regressions, successful wrapper repair, or full-matrix/proof readiness.

## Claims verdict

1. **Wrappers disabled:** **Falsified** by actual Clippy commands.
2. **RUSTFLAGS precedence:** **Supported** by installed Cargo’s explicit precedence order and recorded cfg/commands; separate from wrapper isolation.
3. **Rustdoc flags preserved:** **Supported for inspected default documentation/doctest paths only.**
4. **1,440 / 320 configurations:** **Manifest arithmetic supported.** Named closure factors are `2 × 2 × 5 × 8 × 3 × 3 = 1,440`; alias normalization gives `2 × 2 × 5 × 4 × 2 × 2 = 320`. The archived cfg inventory supports the proposed alias equivalence. No generator, complete dependency-activation map, or full H execution was independently verified.
5. **Ownership remains proposed:** **Supported.** Scheduling and W01 authorization fences remain explicit; no new evidence reopens the settled approval finding.
6. **Ordinary contracts erase expressions:** **Supported directly** by installed `creusot-contracts-proc-0.8.0`: non-Creusot cfg selects dummy implementations; `requires`/`ensures` discard their attribute argument. This establishes neither translation nor proof validity.

## Smallest future approval contract

Authorize separately, without implying D07 or implementation approval:

1. **Documentation correction only:** Apply the explicit-empty wrapper candidate; define diagnostic dispositions and inspection pauses; link current failed status while preserving the frozen run.
2. **Fresh artifacts:** Use a newly named evidence directory and fresh compiler-specific target directories, e.g. `recipe-validation/revalidation-1/` and `target/d07-baseline/revalidation-1/{stable,nightly}/`. Refuse pre-existing destinations. Never clean, overwrite, or reuse the failed run’s caches/logs.
3. **Same bounded scope:** Same source/lock, installed stable/nightly, default core features, host target, locked/offline operation. No production/configuration/dependency edits, installations, network, full H, proofs or AVX2 execution.
4. **Negative controls:** Retain deliberately invalid inherited executables/wrappers/target and encoded native/AVX2 flags; preserve existing user sccache configuration as the configuration-level challenge.
5. **Before runtime:** Inspect fresh direct/effective cfg, actual compiler/documenter commands, and actual Clippy command chains. Require the selected `clippy-driver`, no external wrapper, baseline flags, and real diagnostics—not merely command success.
6. **Known-red contract:** Stable Clippy 0; nightly 101 with precisely the known three loop diagnostics and ordinary failure summaries. Nightly success is a failed control, not permission to declare green.
7. **Then bounded tests:** Only after inspections and diagnostic dispositions pass, execute the existing default tests/doctests and inspect their commands/counts. Stop on any unexplained deviation; no automatic retries.
8. Archive results and seek cumulative round 3 scrutiny. Maximum remains six rounds.

## Ambiguities I resolved and how

- **Frozen drafting status versus current authority:** Treated the plans as immutable pre-execution snapshots; `SUMMARY.md` and the round-2 contract supply subsequent authorization/failure status. Did not re-raise stale wording as renewed approval overstatement.
- **Parent versus child state:** Confirmed this reviewer’s actual process/session root and phase. The orchestrator’s earlier phase/history remains contract-supplied evidence, not certified by my state.
- **Historical evidence:** Read the complete matrix/checkpoint/summary and historical decisions, work packages and arithmetic plan; inspected manifests, recorded provenance/cfg/status/commands, selected workflow references, archived cfg inventory and installed Cargo/contract-macro references. Trusted the supplied round-1 dispositions and aggregate warning attribution rather than rerunning them.
- **Wrapper mechanism:** Distinguished observed external-wrapper failure and documented unset/empty semantics from the supported—but not source-traced—subcommand propagation explanation and untested correction.
- **“New diagnostic”:** Interpreted it conservatively as requiring disposition, not proof of a new regression or automatic permission to suppress dependency output.
- **Unexecuted claims:** Runtime results remain log evidence; closure enumeration/dependency equivalence, target portability, exact dated/MSRV compilers, safety and proofs remain unverified because this round expressly prohibited execution and broader auditing.

Completed independent read-only diagnosis; **conformance remains failed and round 2 does not converge**.
