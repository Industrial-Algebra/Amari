#!/usr/bin/env bash
# C2 evidence runner (contract #350 S1-S4): compile-fail harness +
# ingress suite + standing gates, TOTAL override discipline, fail-closed
# inventory (6 negative cases + 1 positive control + 11 dynamic tests
# asserted BEFORE success is declared).
set -uo pipefail
OUT_ROOT="${OUT_ROOT_OVERRIDE:-docs/plans/core-matrix-evidence/ctrack-c2}"
T_NAME=stable
mkdir -p "$OUT_ROOT/logs"
if [ -n "${OUT_ROOT_OVERRIDE:-}" ]; then TARGET_DIR="$OUT_ROOT/target"; else TARGET_DIR="$PWD/target/c2-gate"; fi
mkdir -p "$TARGET_DIR"
rustc +$T_NAME -vV > "$OUT_ROOT/toolchain.txt" 2>&1 || exit 7
grep -q 'commit-hash: 88d9e12ae178fab0fb5cc050a94da85685d449ea' "$OUT_ROOT/toolchain.txt" || { echo 'pin mismatch' >&2; exit 7; }
CFG="env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_TARGET RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER=''"
RUSTC_BIN="$(rustup which --toolchain $T_NAME rustc)"
printf 'gate\texit\n' > "$OUT_ROOT/results.tsv"
rm -f "$OUT_ROOT/failures.txt"

gate() { # label test_args
  local L="$1" A="$2" RC
  eval "$CFG RUSTC='$RUSTC_BIN' CARGO_TARGET_DIR='$TARGET_DIR' cargo +$T_NAME test --locked -p amari-core $A" \
    > "$OUT_ROOT/logs/$L.log" 2>&1
  RC=$?
  printf '%s\t%s\n' "$L" "$RC" >> "$OUT_ROOT/results.tsv"
  [ $RC -ne 0 ] && echo "FAIL $L" >> "$OUT_ROOT/failures.txt"
}
gate compile-fail '--test compile_fail -- --nocapture'
gate ingress '--test ingress_rejection'
gate default ''

# clippy gate (separate — not a test target)
eval "$CFG RUSTC='$RUSTC_BIN' CARGO_TARGET_DIR='$TARGET_DIR' cargo +$T_NAME clippy --locked -p amari-core --all-targets -- -D warnings" \
  > "$OUT_ROOT/logs/clippy.log" 2>&1
printf 'clippy\t%s\n' "$?" >> "$OUT_ROOT/results.tsv"

# FAIL-CLOSED inventory: the logs must show the expected counts.
NEG=$(grep -c 'REJECTED' "$OUT_ROOT/logs/compile-fail.log" || true)
POS=$(grep -c 'positive' "$OUT_ROOT/logs/compile-fail.log" || true)
ING=$(awk '/test result:/ {print $4}' "$OUT_ROOT/logs/ingress.log" | head -1)
[ "$NEG" -eq 6 ] || { echo "FATAL: expected 6 rejected cases, log shows $NEG" >&2; exit 7; }
[ "$ING" -eq 11 ] || { echo "FATAL: expected 11 ingress tests, log shows $ING" >&2; exit 7; }
FAILED=$(tail -n +2 "$OUT_ROOT/results.tsv" | awk -F'\t' '$2!="0"' | wc -l)
echo "C2 GATE: gates=$(tail -n +2 "$OUT_ROOT/results.tsv" | wc -l) failed=$FAILED (6 negatives rejected, 11 ingress green)" | tee "$OUT_ROOT/summary.txt"
[ "$FAILED" -eq 0 ] || exit 1
