#!/usr/bin/env bash
# P-EMBED supplemental re-verification (standalone; R1 fix — the sed-derived
# copy still pointed at the nostd tree and overwrote its evidence).
set -uo pipefail
EVID_ROOT="docs/plans/core-matrix-evidence/m-h"
OUT_ROOT="docs/plans/core-matrix-evidence/pembed"
case "${1:?stable|nightly|msrv}" in
  stable)  T_NAME=stable;  T_HASH=88d9e12ae178fab0fb5cc050a94da85685d449ea; MODE=stable;  DIR=target/mh-s3-stable ;;
  nightly) T_NAME=nightly; T_HASH=787af2b8c80638c51a4fc8e44f84e6891f243ec7; MODE=nightly; DIR=target/mh-s3-nightly ;;
  msrv)    T_NAME=1.89.0;  T_HASH=29483883eed69d5fb4db01964cdf2af4d86e9cb2; MODE=msrv;    DIR=target/mh-s3-msrv ;;
esac
# R2 fix: mh-lib writes its probe archives (toolchain dumps, effective-cfg
# files) to $EVID — route them under OUR output tree so nothing lands in
# the shared m-h evidence and OUT_ROOT_OVERRIDE is honored everywhere.
EVID="${OUT_ROOT_OVERRIDE:-$OUT_ROOT}/probe"
mkdir -p "$EVID"
D07_BASELINE_TARGET_DIR="$PWD/$DIR"
source "$EVID_ROOT/mh-lib.sh"
set +e
OUT="${OUT_ROOT_OVERRIDE:-$OUT_ROOT}/supplemental"
mkdir -p "$OUT/logs"
mh_assert_toolchain
if ! rustc +"$T_NAME" --target "$HOST" -C target-cpu=x86-64 -C target-feature=-avx2 --print cfg > "$OUT_ROOT/cfg-$MODE-direct.txt" 2>"$OUT_ROOT/cfg-$MODE-direct.err"; then
  echo "direct cfg acquisition FAILURE ($MODE) — aborting" >&2; exit 7
fi
[ -s "$OUT_ROOT/cfg-$MODE-direct.txt" ] || { echo "direct cfg EMPTY ($MODE)" >&2; exit 7; }
rm -f "$OUT_ROOT/cfg-$MODE-direct.err"
mh_assert_baseline_cfg "$OUT_ROOT/cfg-$MODE-direct.txt"
mh_effective_cfg_probe empty --no-default-features

printf 'quadrant\trow_label\tfeat_args\tcmd\twall_s\texit\n' > "$OUT/results-$MODE.tsv"
SINGLETONS="std gf2 creusot-contracts phantom-types formal-verification high-precision native-precision serde rayon serialize parallel full-precision wasm-precision"
run_row() {
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
  printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$MODE" "$LABEL" "$FARGS" "$CMD" "$(echo "$(date +%s.%N) $START" | awk '{printf "%.1f",$1-$2}')" "$RC" >> "$OUT/results-$MODE.tsv"
  [ "$RC" -ne 0 ] && echo "FAIL [$MODE] $LABEL $CMD" >> "$OUT/failures.txt"
  return $RC
}
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
  done
fi
TOTAL=$(tail -n +2 "$OUT/results-$MODE.tsv" | wc -l)
FAILED=$(awk -F'\t' -v m="$MODE" '$1==m && $6!="0"' "$OUT/results-$MODE.tsv" | wc -l)
echo "QUADRANT=$MODE TOTAL=$TOTAL FAILED=$FAILED" >> "$OUT/summary.txt"
cat "$OUT/summary.txt"
[ "$FAILED" -eq 0 ] || exit 1
