#!/usr/bin/env bash
# Post-R1: regenerate per-row test inventories preserving FULL doctest
# identity (name between 'test ' and ' ... '), from the retained warm
# target dirs. Green test rows only; RED rows produce no tests.
set -uo pipefail
EVID="docs/plans/core-matrix-evidence/m-h"
QUAD="$1"   # stage1-stable | stage2-nightly
case "$QUAD" in
  stage1-stable) T_NAME=stable; T_HASH=88d9e12ae178fab0fb5cc050a94da85685d449ea; MODE=stable; DIR=target/mh-s1-stable ;;
  stage2-nightly) T_NAME=nightly; T_HASH=787af2b8c80638c51a4fc8e44f84e6891f243ec7; MODE=nightly; DIR=target/mh-s2-nightly ;;
  *) echo "bad quad" >&2; exit 2 ;;
esac
D07_BASELINE_TARGET_DIR="$PWD/$DIR"
source "$EVID/mh-lib.sh"
mh_assert_toolchain
OUT="$EVID/$QUAD"
# green test rows = test invocations with exit 0
awk -F'\t' '$2=="test" && $4=="0" {print $1}' "$OUT/results.tsv" > "$OUT/green-test-rows.txt"
mkdir -p "$OUT/tests2"
> "$OUT/tests2-manifest.tsv"; printf 'row\ttest_count\tnames_sha256\n' >> "$OUT/tests2-manifest.tsv"
while read -r ID; do
  FEATS=$(awk -F'\t' -v id="$ID" '$1==id {print $2}' "$EVID/pilot/matrix-rows.txt")
  F_ARGS="--no-default-features"; [ -n "$FEATS" ] && F_ARGS="--no-default-features --features $FEATS"
  baseline_cargo test --locked -p amari-core $F_ARGS --target "$HOST" > "$OUT/tests2-row$ID.log" 2>&1
  RC=$?
  if [ "$RC" -ne 0 ]; then echo "RECAPTURE FAILED row $ID rc=$RC" >&2; exit 7; fi
  sed -nE 's/^test (.+) \.\.\. .*/\1/p' "$OUT/tests2-row$ID.log" | sort > "$OUT/tests2/row$ID.txt"
  CNT=$(wc -l < "$OUT/tests2/row$ID.txt" | tr -d ' ')
  SHA=$(sha256sum "$OUT/tests2/row$ID.txt" | cut -d' ' -f1)
  printf '%s\t%s\t%s\n' "$ID" "$CNT" "$SHA" >> "$OUT/tests2-manifest.tsv"
done < "$OUT/green-test-rows.txt"
cat "$OUT"/tests2/row*.txt | sort -u > "$OUT/tests2-union.txt"
echo "recaptured $(wc -l < "$OUT/green-test-rows.txt") rows; union $(wc -l < "$OUT/tests2-union.txt")"
rm -f "$OUT"/tests2-row*.log
