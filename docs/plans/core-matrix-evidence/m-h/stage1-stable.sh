#!/usr/bin/env bash
# M-H stage 1 (option B): stable full-H — 320 rows x 4 commands.
# Post-R1 remediation: exact toolchain pin + fail-closed effective-cfg
# preflight (mh-lib.sh); the historical execution used the identical
# baseline_cargo body (see DISPOSITION.md history note).
set -uo pipefail
EVID="docs/plans/core-matrix-evidence/m-h"
T_NAME=stable
T_HASH=88d9e12ae178fab0fb5cc050a94da85685d449ea
MODE=stable
source "$EVID/mh-lib.sh"
OUT="$EVID/stage1-stable"
mkdir -p "$OUT/tests" "$OUT/logs"
D07_BASELINE_TARGET_DIR="$PWD/target/mh-s1-stable"

mh_assert_toolchain

# Unconditional effective-cfg preflight (R2 fix): runs in EVERY mode.
rustc +"$T_NAME" --target "$HOST" -C target-cpu=x86-64 -C target-feature=-avx2 --print cfg > "$EVID/cfg-$MODE-direct.txt"
mh_assert_baseline_cfg "$EVID/cfg-$MODE-direct.txt"
mh_effective_cfg_probe empty --no-default-features
rustc +"$T_NAME" --target "$HOST" -C target-cpu=x86-64 -C target-feature=-avx2 --print cfg > "$EVID/cfg-stable-direct.txt"
mh_assert_baseline_cfg "$EVID/cfg-stable-direct.txt"

run_one() {
  local ID="$1" FEATS="$2" CMD="$3" EXTRA="$4"
  local F_ARGS
  if [ -z "$FEATS" ]; then F_ARGS="--no-default-features"
  else F_ARGS="--no-default-features --features $FEATS"; fi
  local CLIPPY_ARGS=""
  [ "$CMD" = clippy ] && CLIPPY_ARGS="-- -D warnings"
  local START END RC
  START=$(date +%s.%N)
  if [ "$CMD" = doc ]; then
    baseline_cargo doc --locked -p amari-core $F_ARGS --target "$HOST" --no-deps \
      > "$OUT/logs/row$ID-$CMD.log" 2>&1
  elif [ -z "$EXTRA" ]; then
    baseline_cargo "$CMD" --locked -p amari-core $F_ARGS --target "$HOST" $CLIPPY_ARGS \
      > "$OUT/logs/row$ID-$CMD.log" 2>&1
  else
    baseline_cargo "$CMD" --locked -p amari-core $F_ARGS --target "$HOST" $EXTRA $CLIPPY_ARGS \
      > "$OUT/logs/row$ID-$CMD.log" 2>&1
  fi
  RC=$?
  END=$(date +%s.%N)
  printf '%s\t%s\t%s\t%s\n' "$ID" "$CMD" "$(echo "$END $START" | awk '{printf "%.1f", $1-$2}')" "$RC" >> "$OUT/results.tsv"
  return $RC
}

if [ "${PREFLIGHT_ONLY:-0}" = 1 ]; then
  mh_effective_cfg_probe empty --no-default-features
  mh_effective_cfg_probe full --no-default-features --features std,formal-verification,gf2,high-precision,native-precision,rayon,serde
  echo "PREFLIGHT OK ($MODE)"; exit 0
fi

declare -i FAILED=0 TOTAL=0
printf 'row\tcmd\twall_s\texit\n' > "$OUT/results.tsv"
while IFS=$'\t' read -r ID FEATS; do
  case "$ID" in ''|*[!0-9]*) continue ;; esac
  for CMD in check test clippy doc; do
    EXTRA=""; case "$CMD" in check|clippy) EXTRA="--all-targets" ;; test) EXTRA="" ;; doc) EXTRA="--no-deps" ;; esac
    run_one "$ID" "$FEATS" "$CMD" "$EXTRA" || { FAILED+=1; echo "FAIL row $ID $CMD" >> "$OUT/failures.txt"; }
    TOTAL+=1
    if [ "$CMD" = test ]; then
      sed -nE 's/^test (.+) \.\.\. .*/\1/p' "$OUT/logs/row$ID-test.log" | sort > "$OUT/tests/row$ID.txt"
    fi
  done
done < "$EVID/pilot/matrix-rows.txt"
echo "TOTAL=$TOTAL FAILED=$FAILED" > "$OUT/summary.txt"
du -sk "$D07_BASELINE_TARGET_DIR" | awk '{print "target_dir_kb", $1}' >> "$OUT/summary.txt"
[ "$FAILED" -eq 0 ] || exit 1
