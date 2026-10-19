#!/usr/bin/env bash
# M-H stage 1 (option B): stable full-H — 320 rows x 4 commands.
# §3.2 baseline_cargo discipline; per-row test-name capture (F2);
# failure recording with continue-then-report (no masking).
set -uo pipefail
HOST=x86_64-unknown-linux-gnu
T=1.89.0
EVID="docs/plans/core-matrix-evidence/m-h"
OUT="$EVID/stage2-msrv"
rm -rf "$OUT"; mkdir -p "$OUT/tests" "$OUT/logs"

D07_BASELINE_TARGET_DIR="$PWD/target/mh-s2-msrv"
rm -rf "$D07_BASELINE_TARGET_DIR"; mkdir -p "$D07_BASELINE_TARGET_DIR"

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

SMOKE="${SMOKE_N:-0}"
declare -i FAILED=0 TOTAL=0
printf 'row\tcmd\twall_s\texit\n' > "$OUT/results.tsv"

while IFS=$'\t' read -r ID FEATS; do
  [ "$ID" = "1" ] && [[ "$FEATS" == \#* ]] && continue
  case "$ID" in ''|*[!0-9]*) continue ;; esac
  if [ "$SMOKE" -gt 0 ] && [ "$ID" -gt "$SMOKE" ]; then break; fi
  if [ -z "$FEATS" ]; then F_ARGS="--no-default-features"
  else F_ARGS="--no-default-features --features $FEATS"; fi

  for CMD in check; do
    EXTRA=""; CLIPPY_ARGS=""
    case "$CMD" in
      check)  EXTRA="--lib" ;;
      test)   EXTRA="" ;;
      clippy) EXTRA="--all-targets"; CLIPPY_ARGS="-- -D warnings" ;;
      doc)    EXTRA="--no-deps" ;;
    esac
    START=$(date +%s.%N)
    if [ "$CMD" = doc ]; then
      baseline_cargo doc --locked -p amari-core $F_ARGS --target "$HOST" $EXTRA \
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
    W=$(echo "$END $START" | awk '{printf "%.1f", $1-$2}')
    printf '%s\t%s\t%s\t%s\n' "$ID" "$CMD" "$W" "$RC" >> "$OUT/results.tsv"
    TOTAL+=1
    if [ "$RC" -ne 0 ]; then
      FAILED+=1
      echo "FAIL row $ID $CMD rc=$RC" >> "$OUT/failures.txt"
    fi
    if [ "$CMD" = test ]; then
      grep -E '^test .+ \.\.\. ' "$OUT/logs/row$ID-test.log" \
        | awk '{print $2}' | sort > "$OUT/tests/row$ID.txt"
      # doctest sections reuse "test <path>" lines; keep all
    fi
  done
done < "$EVID/pilot/matrix-rows.txt"

echo "TOTAL=$TOTAL FAILED=$FAILED" > "$OUT/summary.txt"
du -sk "$D07_BASELINE_TARGET_DIR" | awk '{print "target_dir_kb", $1}' >> "$OUT/summary.txt"
[ "$FAILED" -eq 0 ] || exit 1
