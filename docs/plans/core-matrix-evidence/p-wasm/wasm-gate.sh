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
# Input resolution (R2): CLOSURE_SRC wins; else an existing file under
# OUT_ROOT (override runs may supply reviewer-controlled input — NEVER
# clobbered); else the repo-committed copy. Validation happens AFTER
# resolution, so a poisoned override input genuinely fails closed.
if [ -z "${CLOSURE_SRC:-}" ] && [ ! -f "$OUT_ROOT/closure.txt" ]; then
  cp docs/plans/core-matrix-evidence/p-wasm/closure.txt "$OUT_ROOT/closure.txt"
fi
CLOSURE_SRC="${CLOSURE_SRC:-$OUT_ROOT/closure.txt}"
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
# picks. R2: the space is 8-DIMENSIONAL — creusot-contracts is an
# implicit CLI-addressable feature (no dep: prefix), so C-without-fv
# states are real capabilities: 160 canonical sets (not 96). Computed
# by compute-closure.py from the manifest (drift-checked).
# FAIL-CLOSED (R2): validate the closure input AND the expected
# inventory before any build — missing/truncated input must not yield
# rows=0 failed=0 success.
if [ ! -s "$CLOSURE_SRC" ]; then
  echo 'FATAL: closure input missing or empty: '"$CLOSURE_SRC" >&2; exit 7
fi
CLOSURE_N=$(grep -c '' "$CLOSURE_SRC")
if [ "$CLOSURE_N" -ne 160 ]; then
  echo "FATAL: closure inventory is $CLOSURE_N rows, expected 160" >&2; exit 7
fi
while IFS=$'\t' read -r IDX FEATS; do
  if [ "$FEATS" = '(minimal)' ]; then
    row "set-$IDX-minimal" '--no-default-features'
  else
    row "set-$IDX" "--no-default-features --features $FEATS"
  fi
done < <(cat "$CLOSURE_SRC") 
TOTAL=$(tail -n +2 "$OUT_ROOT/results.tsv" | wc -l)
FAILED=$(tail -n +2 "$OUT_ROOT/results.tsv" | awk -F'\t' '$3!="0"' | wc -l)
# inventory cross-check: rows run MUST equal the validated closure size
if [ "$TOTAL" -ne 160 ]; then
  echo "FATAL: ran $TOTAL rows, expected 160" >&2; exit 7
fi
echo "P-WASM COMPILE CLOSURE: rows=$TOTAL failed=$FAILED" | tee "$OUT_ROOT/summary.txt"
[ "$FAILED" -eq 0 ] || exit 1
