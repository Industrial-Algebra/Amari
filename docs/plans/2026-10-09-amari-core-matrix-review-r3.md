# CORE-D07 Cumulative Review — Round 3 Contract

**Dispatch class:** REVIEW (report-only, read-only). Named agent `reviewer`, provider `openai-codex`, model `gpt-6-astra`, worktree cwd, `fork=false`. Round 3 of maximum 6. Prior: R1 0 P1 / 2 P2 / 0 P3; R2 0 P1 / 2 P2 / 0 P3 (both P2 verified and now claimed remediated).

## Scope — cumulative

The entire CORE-D07 record, not only the latest edits:

1. `docs/plans/2026-10-09-amari-core-verification-matrix.md` — canonical matrix, SHA-256 `cb56119f53c25adefedca079c7c3698512b08c05e4fd7086b84401901a74f0f1`. §3.2 wrapper corrected to explicit-empty `RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER=''` with mandatory never-reused `D07_BASELINE_TARGET_DIR`.
2. `docs/plans/2026-10-09-amari-core-matrix-validation-checkpoint.md` — frozen failed recipe, SHA-256 `c59543bbf2199cc7ea4a411f289da22f86f8a8a4927e4eed18c23941fe85b510` (archived copy must match).
3. Failed orchestrator run under `core-matrix-evidence/recipe-validation/orchestrator/` — must remain unchanged (spot-verify at least stable/check.log, stable/clippy.log, nightly/clippy.log against the hashes recorded in the revalidation plan and R2 report).
4. Round-2 report/triage under `recipe-validation/review-round-2-{report,triage}.md`.
5. New revalidation evidence under `recipe-validation/revalidation-1/` — SUMMARY SHA-256 `2ee149c5928a9a29903a5eb1661f44e58c397f7edc3c5d41f7add007223f1c7c`, `diagnostic-catalogue.md` SHA-256 `64c6185bd65192e878cbf2bb49992d42b01b7d0d5b6a65a618e705bf459be9dc`, per-compiler logs/status/versions, `inputs/` archives, APPROVALS.
6. Root `Cargo.lock` SHA-256 `acb76177dfb6ad26413de8a56c8ac9a6f8c6f2ec866516e6f95ceb828b6159f5`; HEAD `ba1b7719f609e8f0f127d620ba6936cfffb61413`; branch `chore/core-verification-matrix`.

## Claims to scrutinize

- **R2-F1 closure (wrapper isolation):** the corrected wrapper's mechanism claim (empty env values override Cargo config in every child Cargo process, including cargo-clippy's inner re-exec; unset does not; CLI `--config` does not survive it); zero `sccache` occurrences in all ten revalidation logs; genuine `bin/clippy-driver` chains; pinned rustc/rustdoc; explicit host target; baseline ISA flags; rustdoc `-D warnings`; contaminated negative controls demonstrably overridden.
- **R2-F2 closure (diagnostic boundary):** inspection genuinely occurred between command groups and before runtime (evidence: stable pre-test diagnostic verdict before stable tests; nightly before nightly tests; Clippy before tests on both); all warning fingerprints match frozen identity sets; only the six recorded `rustix@1.1.5` notices; no dependency errors; nightly exactly the three known unique loop errors (159/339/387) plus ordinary lib/lib-test summaries, exit 101.
- **Honesty of limits:** no full-H/portability/proof/safety claim; no CORE-D07 or W00/W01 authorization inferred; catalogue demoted to reference-only per maintainer direction; failed run preserved, not overwritten.
- **Approval-scope honesty:** the maintainer authorized the minimal correction + rerun; APPROVALS records must not overstate this into broader authority. Round-3 dispatch itself was separately approved ("Lets be thorough and dispatch the review").

## Constraints on the reviewer

Read-only: no Cargo/rustc/rustup/clippy invocations, no builds/tests/probes, no file writes, no network, no Git mutations, no GitHub posts, no harness/monitor changes. Inspection of recorded evidence and local installed references only. Do not represent inspection as reproduction. Runtime results remain log evidence certified by the orchestrator, not independently rerun.

## Required return

P1/P2/P3 findings with exact file/line evidence; explicit verdict on whether R2-F1 and R2-F2 are closed; observed-versus-hypothetical separation for any mechanism claims; a bounded future-contract proposal only if findings remain; convergence verdict (zero P1/P2 converges); and a mandatory footer **"Ambiguities I resolved and how."**
