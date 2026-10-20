#!/usr/bin/env bash
# M-H stage 3 (post-R1): the §3.1/§4 supplemental rows the closure owed —
# 13 singletons alone and +std; named alias rows (default, all-features,
# full-precision, wasm-precision) with full H commands; resolved-activation
# comparison for alias rows; MSRV lib checks for everything.
# Usage: stage3-supplemental.sh <stable|nightly|msrv>
set -uo pipefail
EVID="docs/plans/core-matrix-evidence/m-h"
case "${1:?}" in
  stable)  T_NAME=stable;  T_HASH=88d9e12ae178fab0fb5cc050a94da85685d449ea; MODE=stable ;;
  nightly) T_NAME=nightly; T_HASH=787af2b8c80638c51a4fc8e44f84e6891f243ec7; MODE=nightly ;;
  msrv)    T_NAME=1.89.0;  T_HASH=29483883eed69d5fb4db01964cdf2af4d86e9cb2; MODE=msrv ;;
esac
D07_BASELINE_TARGET_DIR="$PWD/target/mh-s3-$MODE"
rm -rf "$D07_BASELINE_TARGET_DIR"; mkdir -p "$D07_BASELINE_TARGET_DIR"
source "$EVID/mh-lib.sh"
set +e   # record-and-continue semantics for H rows (mh-lib sets -e for preflight)
OUT="$EVID/stage3-supplemental"
mkdir -p "$OUT/logs"
mh_assert_toolchain

# Unconditional effective-cfg preflight (R2 fix): runs in EVERY mode.
if ! rustc +"$T_NAME" --target "$HOST" -C target-cpu=x86-64 -C target-feature=-avx2 --print cfg > "$EVID/cfg-$MODE-direct.txt" 2>"$EVID/cfg-$MODE-direct.err"; then
  echo "direct cfg acquisition FAILURE ($MODE) — aborting" >&2
  exit 7
fi
[ -s "$EVID/cfg-$MODE-direct.txt" ] || { echo "direct cfg EMPTY ($MODE) — aborting" >&2; exit 7; }
rm -f "$EVID/cfg-$MODE-direct.err"
mh_assert_baseline_cfg "$EVID/cfg-$MODE-direct.txt"
mh_effective_cfg_probe empty --no-default-features

SINGLETONS="std gf2 creusot-contracts phantom-types formal-verification high-precision native-precision serde rayon serialize parallel full-precision wasm-precision"
printf 'quadrant\trow_label\tfeat_args\tcmd\twall_s\texit\n' > "$OUT/results-$MODE.tsv"

run_row() { # label, feat_args, cmd, extra
  local LABEL="$1" FARGS="$2" CMD="$3" EXTRA="$4" START END RC CLIPPY_ARGS=""
  [ "$CMD" = clippy ] && CLIPPY_ARGS="-- -D warnings"
  START=$(date +%s.%N)
  if [ "$CMD" = doc ]; then
    baseline_cargo doc --locked -p amari-core $FARGS --target "$HOST" --no-deps > "$OUT/logs/$MODE-$LABEL-$CMD.log" 2>&1
  elif [ -z "$EXTRA" ]; then
    baseline_cargo "$CMD" --locked -p amari-core $FARGS --target "$HOST" $CLIPPY_ARGS > "$OUT/logs/$MODE-$LABEL-$CMD.log" 2>&1
  else
    baseline_cargo "$CMD" --locked -p amari-core $FARGS --target "$HOST" $EXTRA $CLIPPY_ARGS > "$OUT/logs/$MODE-$LABEL-$CMD.log" 2>&1
  fi
  RC=$?
  END=$(date +%s.%N)
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$MODE" "$LABEL" "$FARGS" "$CMD" "$(echo "$END $START" | awk '{printf "%.1f",$1-$2}')" "$RC" >> "$OUT/results-$MODE.tsv"
  [ "$RC" -ne 0 ] && echo "FAIL [$MODE] $LABEL $CMD" >> "$OUT/failures.txt"
  return $RC
}

# --- singleton rows: alone and +std ---
for F in $SINGLETONS; do
  if [ "$MODE" = msrv ]; then
    run_row "sing-$F" "--no-default-features --features $F" check "--lib"
    run_row "sing-std-$F" "--no-default-features --features std,$F" check "--lib"
  else
    run_row "sing-$F" "--no-default-features --features $F" check "--all-targets"
    run_row "sing-$F" "--no-default-features --features $F" test ""
    run_row "sing-std-$F" "--no-default-features --features std,$F" check "--all-targets"
    run_row "sing-std-$F" "--no-default-features --features std,$F" test ""
  fi
done

# --- alias rows (no --no-default-features for default/all-features rows) ---
if [ "$MODE" = msrv ]; then
  run_row "alias-default" "" check "--lib"
  run_row "alias-all-features" "--all-features" check "--lib"
  run_row "alias-full-precision" "--no-default-features --features full-precision" check "--lib"
  run_row "alias-wasm-precision" "--no-default-features --features wasm-precision" check "--lib"
else
  for LABEL_FARGS in "alias-default|" "alias-all-features|--all-features" \
                     "alias-full-precision|--no-default-features --features full-precision" \
                     "alias-wasm-precision|--no-default-features --features wasm-precision"; do
    LABEL="${LABEL_FARGS%%|*}"; FARGS="${LABEL_FARGS#*|}"
    for CMD in check test clippy doc; do
      EXTRA=""; case "$CMD" in check|clippy) EXTRA="--all-targets" ;; doc) EXTRA="--no-deps" ;; esac
      run_row "$LABEL" "$FARGS" "$CMD" "$EXTRA"
    done
    # resolved-activation evidence for the alias row
    baseline_cargo tree --locked -p amari-core $FARGS -f "{p} {f}" --depth 0 > "$OUT/logs/$MODE-$LABEL-features.txt" 2>&1
  done
  # activation comparison anchors: the canonical equivalents
  baseline_cargo tree --locked -p amari-core --no-default-features --features std,phantom-types,high-precision,gf2 -f "{p} {f}" --depth 0 > "$OUT/logs/$MODE-canonical-default-equiv-features.txt" 2>&1
  baseline_cargo tree --locked -p amari-core --no-default-features --features high-precision,native-precision -f "{p} {f}" --depth 0 > "$OUT/logs/$MODE-canonical-fullp-equiv-features.txt" 2>&1
  baseline_cargo tree --locked -p amari-core --no-default-features --features high-precision -f "{p} {f}" --depth 0 > "$OUT/logs/$MODE-canonical-wasmp-equiv-features.txt" 2>&1
fi

FAILED=$(grep -c "^FAIL \[$MODE\]" "$OUT/failures.txt" 2>/dev/null || echo 0)
TOTAL=$(tail -n +2 "$OUT/results-$MODE.tsv" | wc -l)
echo "QUADRANT=$MODE TOTAL=$TOTAL FAILED=$FAILED" >> "$OUT/summary.txt"
cat "$OUT/summary.txt"
[ "$FAILED" -eq 0 ] || exit 1
