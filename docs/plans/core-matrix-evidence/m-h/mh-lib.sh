#!/usr/bin/env bash
# Shared M-H runner library (post-R1 remediation): exact toolchain pinning,
# fail-closed effective-cfg preflight, identity archival.
# Consumers set: T_NAME (rustup selector), T_HASH (expected commit-hash),
#                MODE (label), EVID (evidence root), D07_BASELINE_TARGET_DIR.
set -euo pipefail

mh_assert_toolchain() {
  local actual
  actual=$(rustc +"$T_NAME" -vV | awk '/^commit-hash:/ {print $2}')
  if [ "$actual" != "$T_HASH" ]; then
    echo "TOOLCHAIN PIN MISMATCH ($MODE): +$T_NAME is $actual, expected $T_HASH" >&2
    return 5
  fi
  rustc +"$T_NAME" -vV > "$EVID/toolchain-$MODE.txt"
}

# Fail-closed: exit nonzero if any AVX/FMA token IS present (exact quoted tokens).
mh_assert_baseline_cfg() {
  local cfg_file="$1"
  for tok in 'target_feature="avx"' 'target_feature="avx2"' 'target_feature="fma"'; do
    if grep -qF "$tok" "$cfg_file"; then
      echo "cfg contamination ($MODE): $tok present in $cfg_file" >&2
      return 6
    fi
  done
}

# Effective Cargo cfg probe through the wrapper (matrix §3.2), archived.
mh_effective_cfg_probe() {
  local row_label="$1"; shift
  local out="$EVID/effective-cfg-$MODE-$row_label.txt"
  baseline_cargo rustc --locked -p amari-core --lib --target "$HOST" "$@" -- --print cfg > "$out" 2>/dev/null
  mh_assert_baseline_cfg "$out"
}

HOST=x86_64-unknown-linux-gnu

baseline_cargo() {
    local compiler documenter
    compiler=$(rustup which --toolchain "$T_NAME" rustc) || return
    documenter=$(rustup which --toolchain "$T_NAME" rustdoc) || return
    env -u CARGO_ENCODED_RUSTFLAGS -u CARGO_ENCODED_RUSTDOCFLAGS \
        -u CARGO_BUILD_TARGET -u CARGO_BUILD_RUSTC -u CARGO_BUILD_RUSTDOC \
        -u CARGO_BUILD_RUSTC_WRAPPER -u CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER \
        RUSTC_WRAPPER='' RUSTC_WORKSPACE_WRAPPER='' \
        RUSTC="$compiler" RUSTDOC="$documenter" \
        RUSTFLAGS='-C target-cpu=x86-64 -C target-feature=-avx2' \
        RUSTDOCFLAGS='-D warnings -C target-cpu=x86-64 -C target-feature=-avx2' \
        CARGO_TARGET_DIR="$D07_BASELINE_TARGET_DIR" \
        cargo +"$T_NAME" "$@"
}
