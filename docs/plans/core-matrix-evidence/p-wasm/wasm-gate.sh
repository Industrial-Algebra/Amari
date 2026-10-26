#!/usr/bin/env bash
# P-WASM compile-closure gate (matrix §5, claim (i)): wasm32-unknown-unknown,
# the full PORTABLE closure (native-precision absent — rug is native-only),
# library codegen (build --lib; check is metadata-only per the P-EMBED lesson).
# TOTAL override discipline (pembed R3–R5): evidence AND build artifacts
# route under OUT_ROOT_OVERRIDE.
set -uo pipefail
OUT_ROOT="${OUT_ROOT_OVERRIDE:-docs/plans/core-matrix-evidence/p-wasm}"
T_NAME=stable
mkdir -p "$OUT_ROOT/logs"
cp docs/plans/core-matrix-evidence/p-wasm/closure.txt "$OUT_ROOT/closure.txt" 2>/dev/null || true
if [ -n "${OUT_ROOT_OVERRIDE:-}" ]; then TARGET_DIR="$OUT_ROOT/target"; else TARGET_DIR="$PWD/target/wasm-gate"; fi
mkdir -p "$TARGET_DIR"
rustc +$T_NAME -vV > "$OUT_ROOT/toolchain.txt" 2>&1 || exit 7
grep -q 'commit-hash: 88d9e12ae178fab0fb5cc050a94da85685d449ea' "$OUT_ROOT/toolchain.txt" || { echo 'pin mismatch' >&2; exit 7; }
printf 'config\tfeat_args\texit\n' > "$OUT_ROOT/results.tsv"
rm -f "$OUT_ROOT/failures.txt"
row() { # label feat_args
  local L="$1" F="$2" RC
  env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_BUILD_TARGET RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER='' \
    RUSTC="$(rustup which --toolchain $T_NAME rustc)" \
    CARGO_TARGET_DIR="$TARGET_DIR" \
    cargo +$T_NAME build --locked --lib -p amari-core --target wasm32-unknown-unknown $F \
    > "$OUT_ROOT/logs/$L.log" 2>&1
  RC=$?
  printf '%s\t%s\t%s\n' "$L" "$F" "$RC" >> "$OUT_ROOT/results.tsv"
  [ $RC -ne 0 ] && echo "FAIL $L" >> "$OUT_ROOT/failures.txt"
}
# R1 remediation: the EXHAUSTIVE portable closure, not representative
# picks — 96 canonical activation sets computed from the manifest
# implication edges (closure.txt; M-H closure-map discipline). Aliases
# are not separate dimensions; native-precision/full-precision excluded
# (rug is native-only by design).
while IFS=$'\t' read -r IDX FEATS; do
  if [ "$FEATS" = '(minimal)' ]; then
    row "set-$IDX-minimal" '--no-default-features'
  else
    row "set-$IDX" "--no-default-features --features $FEATS"
  fi
done < <(cat "${CLOSURE_SRC:-$OUT_ROOT/closure.txt}") 
TOTAL=$(tail -n +2 "$OUT_ROOT/results.tsv" | wc -l)
FAILED=$(tail -n +2 "$OUT_ROOT/results.tsv" | awk -F'\t' '$3!="0"' | wc -l)
echo "P-WASM COMPILE CLOSURE: rows=$TOTAL failed=$FAILED" | tee "$OUT_ROOT/summary.txt"
[ "$FAILED" -eq 0 ] || exit 1
