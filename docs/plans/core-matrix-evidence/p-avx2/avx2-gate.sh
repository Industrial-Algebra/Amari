#!/usr/bin/env bash
# P-AVX2 execution gate (matrix §5 row): host + explicit
# target-feature=+avx2,+fma, representative configurations.
# TOTAL override discipline from the start (pembed lessons R3–R5):
# OUT_ROOT_OVERRIDE routes evidence AND build artifacts away from the repo.
set -uo pipefail
OUT_ROOT="${OUT_ROOT_OVERRIDE:-docs/plans/core-matrix-evidence/p-avx2}"
T_NAME=stable
mkdir -p "$OUT_ROOT/logs"
if [ -n "${OUT_ROOT_OVERRIDE:-}" ]; then
  TARGET_DIR="$OUT_ROOT/target"
else
  TARGET_DIR="$PWD/target/avx2-gate"
fi
mkdir -p "$TARGET_DIR"

rustc +$T_NAME -vV > "$OUT_ROOT/toolchain.txt" 2>&1 || exit 7
grep -q 'commit-hash: 88d9e12ae178fab0fb5cc050a94da85685d449ea' "$OUT_ROOT/toolchain.txt" \
  || { echo 'toolchain pin mismatch' >&2; exit 7; }

AVX_FLAGS='-C target-feature=+avx2,+fma'
printf 'config\tcmd\texit\n' > "$OUT_ROOT/results.tsv"

row() { # label feat_args test_args
  local L="$1" F="$2" A="$3" RC
  env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_TARGET RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER='' \
    RUSTC="$(rustup which --toolchain $T_NAME rustc)" \
    RUSTFLAGS="-C target-cpu=x86-64 $AVX_FLAGS" \
    CARGO_TARGET_DIR="$TARGET_DIR" \
    cargo +$T_NAME test --locked -p amari-core $F $A \
    > "$OUT_ROOT/logs/$L.log" 2>&1
  RC=$?
  printf '%s\t%s\t%s\n' "$L" "$F $A" "$RC" >> "$OUT_ROOT/results.tsv"
  [ $RC -ne 0 ] && echo "FAIL $L" >> "$OUT_ROOT/failures.txt"
}
rm -f "$OUT_ROOT/failures.txt"
row default '' '--test simd_audit'          # THE battery: runtime inventory + bigint oracle
row default-full '' ''                       # full default suite under AVX2
row minimal '--no-default-features' '--test simd_audit'
row native '--features native-precision' '--test simd_audit'
row all '--all-features' '--test simd_audit'
# scalar-expectation comparison: battery vs baseline row (already in H evidence);
# record the impl-name inventory as selected at runtime
# Runtime inventory is ASSERTED in-test (simd_rows::runtime_inventory:
# dispatcher must select "avx2+fma" and the dispatched fn must match the
# direct kernel coefficient-for-coefficient) — captured stdout is empty,
# so the inventory evidence is the passing assertion itself.
: > "$OUT_ROOT/impl-inventory.txt"
printf 'dispatcher selection: avx2+fma (asserted by simd_rows::runtime_inventory; dispatch == direct kernel for all 8 coefficients)\n' >> "$OUT_ROOT/impl-inventory.txt"
TOTAL=$(tail -n +2 "$OUT_ROOT/results.tsv" | wc -l)
FAILED=$(tail -n +2 "$OUT_ROOT/results.tsv" | awk -F'\t' '$3!="0"' | wc -l)
echo "P-AVX2 GATE: rows=$TOTAL failed=$FAILED" | tee "$OUT_ROOT/summary.txt"
[ "$FAILED" -eq 0 ] || exit 1
