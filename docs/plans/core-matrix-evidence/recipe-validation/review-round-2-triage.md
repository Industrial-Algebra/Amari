# CORE-D07 Round 2 — Orchestrator Triage

## Loop status

Round 2: **0 P1 / 2 P2 / 0 P3**, not converged. Two rounds run; maximum six total. No findings closed, correction applied, or revalidation executed in this triage. Independent review was read-only; runtime evidence remains the frozen orchestrator checkpoint, not an independent rerun.

Returned report: [review-round-2-report.md](review-round-2-report.md).
Failed-run record: [orchestrator/SUMMARY.md](orchestrator/SUMMARY.md).

Matrix/checkpoint/lock hashes remain the review-contract values; tracked non-plan changes remain empty. Any future revision must retain the frozen evidence and disclose the new snapshot.

## R2-F1 — External wrapper across Clippy — VERIFIED, OPEN

The existing canonical recipe removes `RUSTC_WRAPPER` and `RUSTC_WORKSPACE_WRAPPER` instead of setting explicit empty overrides. The user Cargo config selects `sccache`. Actual core Clippy command chains show external sccache on both compilers (`stable/clippy.log:762,1023`, `nightly/clippy.log:6985,7485`). Installed Cargo reference states that an empty wrapper environment variable overrides configuration and disables that wrapper.

Disposition: **Verified; correction/revalidation approval pending.** The observation and unset/empty semantics are established. Exact installed external-subcommand forwarding internals were not traced at source level, and explicit-empty behavior on this Clippy path remains an untested correction candidate. Do not claim wrapper repair from documentation alone.

Candidate: retain existing sanitization/pinned executables/baseline flags, but set `RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER=''` explicitly while still removing `CARGO_BUILD_*WRAPPER`. Require actual Clippy command-chain inspection and the known nightly lint failure to prove Clippy's own driver still runs. No CLI/configuration/source edits or new test run authorized yet.

This is new executable evidence against the round-1 correction; it legitimately reopens its conformance claim. It is not disagreement with the prior finding or renewed scope-approval overstatement.

## R2-F2 — Diagnostic stopping boundary — VERIFIED, OPEN

Read the phase-B script and stopping rule at checkpoint lines 200–238: tests precede Clippy, both compiler iterations run in one group, and full diagnostic inspection is deferred until afterward. Presence checks confirm known error strings but neither classify all diagnostics nor stop at an unclassified dependency warning.

Read `stable/check.log:278–279`: a registry-owned `plotters-backend-0.3.7` lifetime warning occurs during phase A. Read `stable/clippy.log:27–29`: the same warning is replayed beside `Fresh plotters-backend`. The recorded status files confirm both compiler groups progressed through tests/Clippy before full inspection.

Disposition: **Verified; diagnostic policy and execution-boundary approval pending.** The existing rule is broader than its implemented inspection sequence and does not define “new” or how cache replay affects comparison. “No new core error kind” is not an adequate substitute.

Required future contract:

- Run/inspect bounded command groups per compiler, with cfg/flags, Clippy driver/wrapper and diagnostic checks before any runtime execution or advancement to the next compiler.
- Preserve core `-D warnings`; dependency errors or unexpected core diagnostics block.
- Obtain explicit maintainer disposition of existing dependency warnings. The 94/732 counts are observed diagnostic blocks, not unique defects or an exhaustive future allowlist. Define normalized package/version + warning/primary-location comparison and retain raw output; replay/count differences are not automatically new regressions.
- Unclassified changes stop for review; no automatic suppression, blanket dependency exemption or ignored warning delta.
- Keep stable Clippy exit 0 and nightly's three known loop diagnostics/exit 101 as conformance controls, not a full-matrix-green claim.

No dependency warning disposition has been inferred or approved by this triage.

## Next permission boundary

A new documentation-only planning task may propose the two corrections and an exact fresh-artifact revalidation contract. It requires explicit authorization; current permission covers report persistence/read-only triage only. Production source/config/dependencies, runtime revalidation, installation, full H, unsafe/SIMD/proof runs, commits and merges remain out of bounds.

Any approved later validation must use fresh named evidence/target directories and preserve the failed run without cleaning/reusing/overwriting its caches or logs. Its authorization is separate from CORE-D07 approval and from W00/W01 implementation authority.
