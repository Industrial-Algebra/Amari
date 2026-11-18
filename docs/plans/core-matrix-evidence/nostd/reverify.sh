#!/usr/bin/env bash
# No-std host repair re-verification (F1): re-runs H rows INCREMENTALLY on
# the retained M-H target dirs. Rows selected via FIRST..LAST on the §3.1
# enumeration; commands per matrix §3.2. No directory wipe — deps stay cached.
set -uo pipefail
EVID_ROOT="docs/plans/core-matrix-evidence/m-h"
OUT_ROOT="docs/plans/core-matrix-evidence/nostd"
case "${1:?stable|nightly|msrv}" in
  stable)  T_NAME=stable;  T_HASH=88d9e12ae178fab0fb5cc050a94da85685d449ea; MODE=stable;  DIR=target/mh-s1-stable ;;
  nightly) T_NAME=nightly; T_HASH=787af2b8c80638c51a4fc8e44f84e6891f243ec7; MODE=nightly; DIR=target/mh-s2-nightly ;;
  msrv)    T_NAME=1.89.0;  T_HASH=29483883eed69d5fb4db01964cdf2af4d86e9cb2; MODE=msrv;    DIR=target/mh-s2-msrv ;;
esac
FIRST="${2:-33}"; LAST="${3:-160}"
EVID="$EVID_ROOT"
D07_BASELINE_TARGET_DIR="$PWD/$DIR"
source "$EVID_ROOT/mh-lib.sh"
set +e
OUT="$OUT_ROOT/reverify-$MODE"
mkdir -p "$OUT/logs"
mh_assert_toolchain
if ! rustc +"$T_NAME" --target "$HOST" -C target-cpu=x86-64 -C target-feature=-avx2 --print cfg > "$OUT_ROOT/cfg-$MODE-direct.txt" 2>"$OUT_ROOT/cfg-$MODE-direct.err"; then
  echo "direct cfg acquisition FAILURE ($MODE) — aborting" >&2; exit 7
fi
[ -s "$OUT_ROOT/cfg-$MODE-direct.txt" ] || { echo "direct cfg EMPTY ($MODE)" >&2; exit 7; }
rm -f "$OUT_ROOT/cfg-$MODE-direct.err"
mh_assert_baseline_cfg "$OUT_ROOT/cfg-$MODE-direct.txt"
mh_effective_cfg_probe empty --no-default-features

printf 'row\tcmd\twall_s\texit\n' > "$OUT/results.tsv"
declare -i FAILED=0 TOTAL=0
while IFS=$'\t' read -r ID FEATS; do
  case "$ID" in ''|*[!0-9]*) continue ;; esac
  [ "$ID" -lt "$FIRST" ] && continue
  [ "$ID" -gt "$LAST" ] && continue
  if [ -z "$FEATS" ]; then F_ARGS="--no-default-features"; else F_ARGS="--no-default-features --features $FEATS"; fi
  if [ "$MODE" = msrv ]; then CMDS="check"; EXTRA="--lib"
  else CMDS="check test clippy doc"; EXTRA=""; fi
  for CMD in $CMDS; do
    X="$EXTRA"; CA=""
    case "$CMD" in check|clippy) [ -z "$X" ] && X="--all-targets" ;; test) X="" ;; doc) X="--no-deps" ;; esac
    [ "$CMD" = clippy ] && CA="-- -D warnings"
    START=$(date +%s.%N)
    if [ "$CMD" = doc ]; then
      baseline_cargo doc --locked -p amari-core $F_ARGS --target "$HOST" --no-deps > "$OUT/logs/row$ID-$CMD.log" 2>&1
    elif [ -z "$X" ]; then
      baseline_cargo "$CMD" --locked -p amari-core $F_ARGS --target "$HOST" $CA > "$OUT/logs/row$ID-$CMD.log" 2>&1
    else
      baseline_cargo "$CMD" --locked -p amari-core $F_ARGS --target "$HOST" $X $CA > "$OUT/logs/row$ID-$CMD.log" 2>&1
    fi
    RC=$?
    printf '%s\t%s\t%s\t%s\n' "$ID" "$CMD" "$(echo "$(date +%s.%N) $START" | awk '{printf "%.1f",$1-$2}')" "$RC" >> "$OUT/results.tsv"
    TOTAL+=1
    [ "$RC" -ne 0 ] && { FAILED+=1; echo "FAIL row $ID $CMD rc=$RC" >> "$OUT/failures.txt"; }
  done
done < "$EVID_ROOT/pilot/matrix-rows.txt"
echo "TOTAL=$TOTAL FAILED=$FAILED" > "$OUT/summary.txt"
cat "$OUT/summary.txt"
[ "$FAILED" -eq 0 ] || exit 1
