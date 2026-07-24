#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# generate-discovery-wasm-surface.sh — Deterministic WASM surface catalog generation.
#
# Builds amari-wasm with wasm-pack, parses the authoritative .d.ts, and writes
# catalog/generated-wasm.json atomically.  The script is idempotent and
# produces identical output when the generated WASM surface is unchanged.
#
# Prerequisites: rustup, wasm-pack, wasm32-unknown-unknown target.
#
# Usage: ./scripts/generate-discovery-wasm-surface.sh

set -euo pipefail

# ----- canonicalize the repo root -----
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO_ROOT"

echo "==> Generating Amari WASM discovery surface"

# ----- verify prerequisites -----
if ! command -v wasm-pack &>/dev/null; then
    echo "ERROR: wasm-pack is not installed. Install it from https://rustwasm.github.io/wasm-pack/installer/"
    exit 1
fi

if ! rustup target list --installed | grep -q wasm32-unknown-unknown; then
    echo "==> Installing wasm32-unknown-unknown target"
    rustup target add wasm32-unknown-unknown
fi

# ----- build WASM with wasm-pack -----
echo "==> Building amari-wasm with wasm-pack"
TEMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TEMP_DIR"' EXIT

cd "$REPO_ROOT/amari-wasm"
wasm-pack build --dev --target web --out-dir "$TEMP_DIR" --no-pack

# ----- parse authoritative .d.ts and write catalog -----
DTS_FILE="$TEMP_DIR/amari_wasm.d.ts"
if [[ ! -f "$DTS_FILE" ]]; then
    echo "ERROR: wasm-pack did not produce $DTS_FILE"
    exit 1
fi

OUTPUT="$REPO_ROOT/amari-discovery/catalog/generated-wasm.json"
TEMP_OUTPUT="$OUTPUT.tmp.$$"

echo "==> Parsing $DTS_FILE"
cargo run -p amari-discovery --example generate_wasm_surface -- "$DTS_FILE" > "$TEMP_OUTPUT"

# ----- atomic write -----
mv "$TEMP_OUTPUT" "$OUTPUT"
echo "==> Wrote $OUTPUT ($(wc -c < "$OUTPUT") bytes)"

# ----- verify the output parses -----
echo "==> Verifying generated catalog"
cargo test -p amari-discovery --test catalog_wasm -- generated_has_valid_schema_version 2>&1 | tail -5

echo "==> Done. Commit catalog/generated-wasm.json when the surface changes."
