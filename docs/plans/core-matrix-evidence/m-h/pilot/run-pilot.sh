#!/usr/bin/env bash
# M-H pilot: wall-clock/storage estimate for the full-H execution options.
# Matrix §3.2 wrapper discipline verbatim; fresh per-compiler target dirs.
# Usage: run-pilot.sh <stable|nightly|msrv>   (from the worktree root)
set -euo pipefail

MODE="${1:?stable|nightly|msrv}"
HOST=x86_64-unknown-linux-gnu
EVID="docs/plans/core-matrix-evidence/m-h/pilot"

case "$MODE" in
  stable)  T=stable ;;
  nightly) T=nightly ;;
  msrv)    T=1.89.0 ;;
  *) echo "bad mode" >&2; exit 2 ;;
esac

# Researched-compiler verification (matrix §4: hash, not date alone).
ACTUAL_HASH=$(rustc +"$T" -vV | awk '/^commit-hash:/ {print $2}')
EXPECTED_HASH=""
[ "$T" = nightly ] && EXPECTED_HASH="787af2b8c80638c51a4fc8e44f84e6891f243ec7"
if [ -n "$EXPECTED_HASH" ] && [ "$ACTUAL_HASH" != "$EXPECTED_HASH" ]; then
  echo "PIN MISMATCH: nightly is $ACTUAL_HASH, researched $EXPECTED_HASH" >&2
  exit 3
fi

D07_BASELINE_TARGET_DIR="$PWD/target/mh-pilot-$MODE"
rm -rf "$D07_BASELINE_TARGET_DIR"
mkdir -p "$D07_BASELINE_TARGET_DIR"

baseline_cargo() {
    local compiler documenter
    compiler=$(rustup which --toolchain "$T" rustc) || return
    documenter=$(rustup which --toolchain "$T" rustdoc) || return
    env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_ENCODED_RUSTDOCFLAGS \
        -u CARGO_BUILD_TARGET -u CARGO_BUILD_RUSTC -u CARGO_BUILD_RUSTDOC \
        -u CARGO_BUILD_RUSTC_WRAPPER -u CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER \
        RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER='' \
        RUSTC="$compiler" RUSTDOC="$documenter" \
        RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2' \
        RUSTDOCFLAGS='-D warnings -C target-cpu=x86-64 -C target-feature=-avx2' \
        CARGO_TARGET_DIR="$D07_BASELINE_TARGET_DIR" \
        cargo +"$T" "$@"
}

dir_kb() { du -sk "$D07_BASELINE_TARGET_DIR" | awk '{print $1}'; }

# Pre-execution cfg assertion: AVX/AVX2/FMA absent.
rustc +"$T" --target "$HOST" -C target-cpu=x86-64 -C target-feature=-avx2 --print cfg \
  > "$EVID/cfg-$MODE.txt"
if grep -qE 'avx2|"fma"|avx ' "$EVID/cfg-$MODE.txt"; then
  echo "cfg contamination: AVX/AVX2/FMA present" >&2; exit 4
fi

# Representative rows (cost classes: empty/minimal-dep, maximal, two mids).
ROWS=(
  "EMPTY|"
  "FULL|std,formal-verification,gf2,high-precision,native-precision,rayon,serde"
  "MID1|std,phantom-types,high-precision"
  "MID2|std,gf2,creusot-contracts,native-precision,rayon"
)

TSV="$EVID/timings-$MODE.tsv"
echo -e "toolchain\trow\tcommand\twall_s\tdir_kb_before\tdir_kb_after" > "$TSV"

for entry in "${ROWS[@]}"; do
  NAME="${entry%%|*}"; FEATS="${entry#*|}"
  if [ -z "$FEATS" ]; then F_ARGS="--no-default-features"
  else F_ARGS="--no-default-features --features $FEATS"; fi

  if [ "$MODE" = msrv ]; then
    CMDS=("check")
    ARGS=("--lib")
  else
    CMDS=("check" "test" "clippy" "doc")
    ARGS=("--all-targets" "" "--all-targets" "--no-deps")
  fi

  for i in "${!CMDS[@]}"; do
    CMD="${CMDS[$i]}"; EXTRA="${ARGS[$i]}"
    CLIPPY_ARGS=""
    [ "$CMD" = clippy ] && CLIPPY_ARGS="-- -D warnings"
    BEFORE=$(dir_kb)
    START=$(date +%s.%N)
    if [ "$CMD" = doc ]; then
      baseline_cargo doc --locked -p amari-core $F_ARGS --target "$HOST" $EXTRA \
        >> "$EVID/log-$MODE.txt" 2>&1
    elif [ -z "$EXTRA" ]; then
      baseline_cargo "$CMD" --locked -p amari-core $F_ARGS --target "$HOST" $CLIPPY_ARGS \
        >> "$EVID/log-$MODE.txt" 2>&1
    else
      baseline_cargo "$CMD" --locked -p amari-core $F_ARGS --target "$HOST" $EXTRA $CLIPPY_ARGS \
        >> "$EVID/log-$MODE.txt" 2>&1
    fi
    END=$(date +%s.%N)
    AFTER=$(dir_kb)
    printf '%s\t%s\t%s\t%.1f\t%s\t%s\n' "$T" "$NAME" "$CMD" \
      "$(echo "$END $START" | awk '{print $1-$2}')" "$BEFORE" "$AFTER" >> "$TSV"
    echo "[$MODE] $NAME $CMD done"
  done
done

echo "PILOT $MODE COMPLETE -> $TSV"
