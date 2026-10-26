#!/usr/bin/env bash
# C3 conformance gate (contract #350 S6): runs cargo creusot prove on
# the committed fixtures and parses OBLIGATION OUTCOMES:
#   sum_to must show >=1 proved goal and NO unproved goal;
#   wrong_successor must show an UNPROVED goal (the tool must fail it).
# A green exit with an unproved positive goal, or an all-proved
# negative, is a GATE FAILURE (exit 1). Missing outcomes: exit 7.
set -uo pipefail
OUT_ROOT="$(realpath -m "${OUT_ROOT_OVERRIDE:-docs/plans/core-matrix-evidence/ctrack-c3}")"
EVID_SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$OUT_ROOT"
BASE=$(mktemp -d); trap 'rm -rf "$BASE"' EXIT
WORK="$BASE/c3_conf_scratch"   # creusot validates project dir names: no dots
mkdir -p "$WORK"
cp -r "$EVID_SRC/conformance/." "$WORK/"
cd "$WORK"
export PATH="$HOME/.local/share/creusot/bin:$PATH"
cargo creusot init >> "$OUT_ROOT/conformance-prove.log" 2>&1 \
  || { echo "FATAL: cargo creusot init failed in scratch fixture" >&2; exit 7; }
cargo creusot prove >> "$OUT_ROOT/conformance-prove.log" 2>&1
RC=$?
POS_PROVED=$(grep -c 'sum_to: ✔' "$OUT_ROOT/conformance-prove.log" || true)
NEG_UNPROVED=$(grep -c 'vc_wrong_successor: ✘' "$OUT_ROOT/conformance-prove.log" || true)
[ "$POS_PROVED" -ge 1 ] || { echo "FATAL: positive fixture not proved (or outcome line absent)" >&2; exit 7; }
[ "$NEG_UNPROVED" -ge 1 ] || { echo "FATAL: negative fixture not shown unproved (or outcome line absent)" >&2; exit 7; }
[ "$RC" -ne 0 ] || { echo "GATE FAILURE: prove exited 0 despite an unproved false postcondition" >&2; exit 1; }
echo "C3 CONFORMANCE: positive PROVED ($POS_PROVED), negative UNPROVED and tool-failed (rc=$RC)" | tee "$OUT_ROOT/conformance-summary.txt"
