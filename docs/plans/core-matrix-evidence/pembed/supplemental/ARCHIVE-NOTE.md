# Dropped bulk per evidence-slimming doctrine (2026-10-23)

166 per-invocation logs dropped; full manifest in dropped-logs.sha256.
The claims live in results*.tsv (complete, every row + exit code),
summary.txt, and samples/ (representative first/canary/last logs).
Regeneration: reverify.sh/supplemental.sh (committed) under the pinned
toolchain (toolchain.txt / probe/) with the executed Cargo.lock —
the H-matrix lock applies (target dirs warm or cold; identical rows).
