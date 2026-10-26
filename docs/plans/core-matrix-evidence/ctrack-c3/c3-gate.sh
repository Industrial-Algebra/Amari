#!/usr/bin/env bash
# C3 conformance gate (contract #350 S6), R2-hardened:
#   - FRESH log every run (no historical-outcome reads: the log is
#     truncated before prove; grep sees only this run's output);
#   - COMPLETE outcome sets: sum_to must have >=1 proved goal AND NO
#     unproved goal; wrong_successor must have an unproved goal;
#   - ERROR CLASSIFICATION: prove rc!=0 must be the EXPECTED 'unproved
#     file' failure — any other tool error is exit 7, not a pass;
#   - TWO clean runs; both coma manifests archived and compared.
set -uo pipefail
OUT_ROOT="$(realpath -m "${OUT_ROOT_OVERRIDE:-docs/plans/core-matrix-evidence/ctrack-c3}")"
EVID_SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
mkdir -p "$OUT_ROOT"
BASE=$(mktemp -d); trap 'rm -rf "$BASE"' EXIT

prove_once() { # workdir logfile
  local W="$1" LOG="$2"
  ( cd "$W" && export PATH="$HOME/.local/share/creusot/bin:$PATH"
    cargo creusot init > "$LOG" 2>&1 \
      || { echo "TOOL-ERROR: cargo creusot init" >&2; return 7; }
    cargo creusot prove >> "$LOG" 2>&1
    local RC=$?
    # R3 P2-2: the expected failure is EXACTLY rc 1 AND the marker AND
    # the terminal why3find-failure line — any other status (e.g. a
    # fatal 139 after valid-looking output) is a tool error.
    local TERMINAL_OK=0
    tail -1 "$LOG" | grep -q "Error: 'why3find prove' failed" && TERMINAL_OK=1
    if [ "$RC" -ne 1 ] || ! grep -q "Error: 1 unproved file" "$LOG" || [ "$TERMINAL_OK" -ne 1 ]; then
      echo "TOOL-ERROR: prove did not fail the expected way (rc=$RC, marker/terminal mismatch)" >&2
      return 7
    fi
    # complete outcome validation (fresh log only — grep cannot see history)
    local POS_OK POS_BAD NEG_BAD
    POS_OK=$(grep -c 'sum_to: ✔' "$LOG" || true)
    POS_BAD=$(grep -c 'sum_to: ✘' "$LOG" || true)
    NEG_BAD=$(grep -c 'vc_wrong_successor: ✘' "$LOG" || true)
    [ "$POS_OK" -ge 1 ] || { echo "FATAL: positive fixture not proved" >&2; return 7; }
    [ "$POS_BAD" -eq 0 ] || { echo "GATE FAILURE: positive fixture has unproved goals" >&2; return 1; }
    [ "$NEG_BAD" -ge 1 ] || { echo "FATAL: negative fixture not shown unproved" >&2; return 7; }
    [ $RC -ne 0 ] || { echo "GATE FAILURE: prove exited 0 despite the false postcondition" >&2; return 1; }
    return 0 )
}

# R2 finding: .coma artifacts embed ABSOLUTE scratch paths — raw hashes
# differ across directories (the earlier same-dir 'bit-identical' claim
# held only path-invariantly; PROVENANCE corrected). Reproducibility is
# compared on PATH-NORMALIZED content digests; raw manifests are still
# archived per run for inspection.
normalized_digest() { # workdir
  local W="$1"
  ( cd "$W" && find . -name '*.coma' | sort | while read -r F; do
      sed "s|$W||g" "$F" | sha256sum | awk -v f="$F" '{print $1"  "f}'
    done ) | sha256sum | awk '{print $1}'
}
for RUN in run1 run2; do
  W="$BASE/c3_conf_${RUN}"; mkdir -p "$W"
  cp -r "$EVID_SRC/conformance/." "$W/"
  [ -f "$EVID_SRC/conformance/Cargo.lock" ] && cp "$EVID_SRC/conformance/Cargo.lock" "$W/Cargo.lock"
  prove_once "$W" "$OUT_ROOT/conformance-prove-$RUN.log"; RC=$?
  [ $RC -eq 0 ] || exit $RC
  ( cd "$W" && find . -name '*.coma' -exec sha256sum {} \; | sort ) > "$OUT_ROOT/conformance-manifest-$RUN.txt"
  # R3 P2-3: nonempty translation inventory REQUIRED before comparison —
  # expected: both contracted functions' coma files (>= 2).
  N=$(wc -l < "$OUT_ROOT/conformance-manifest-$RUN.txt")
  [ "$N" -ge 2 ] || { echo "FATAL: run $RUN produced $N coma artifacts (expected >= 2) — empty inventories cannot establish reproducibility" >&2; exit 7; }
  echo "$(normalized_digest "$W")" > "$OUT_ROOT/conformance-digest-$RUN.txt"
done
if [ "$(cat "$OUT_ROOT/conformance-digest-run1.txt")" = "$(cat "$OUT_ROOT/conformance-digest-run2.txt")" ]; then
  echo "C3 CONFORMANCE: both runs green; PATH-NORMALIZED digests IDENTICAL across clean replays in distinct directories (raw .coma bytes embed scratch paths — nondeterminism source disclosed)" \
    | tee "$OUT_ROOT/conformance-summary.txt"
else
  echo "GATE FAILURE: path-normalized clean-replay digests differ" >&2; exit 1
fi
