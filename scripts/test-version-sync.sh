#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0

set -euo pipefail

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
fixture=$(mktemp -d)
trap 'rm -rf "$fixture"' EXIT

mkdir -p \
  "$fixture/scripts" \
  "$fixture/member" \
  "$fixture/amari-wasm/examples" \
  "$fixture/amari-discovery/catalog/semantic" \
  "$fixture/typescript" \
  "$fixture/examples/typescript" \
  "$fixture/examples/web/interactive-demos" \
  "$fixture/examples-suite"
cp "$repo_root/scripts/version-sync.sh" "$fixture/scripts/version-sync.sh"
cp "$repo_root/scripts/bump-version.sh" "$fixture/scripts/bump-version.sh"

cat > "$fixture/Cargo.toml" <<'TOML'
[workspace]
members = ["member"]

[workspace.package]
version = "0.23.0"

[workspace.dependencies]
amari-core = { path = "member", version = "0.23.0" }
serde = { version = "1.0.219", features = ["derive"] }
TOML

cat > "$fixture/member/Cargo.toml" <<'TOML'
[package]
name = "member"
version.workspace = true
edition = "2021"

[dependencies]
amari-core = { path = "../member", version = "0.23.0" }
fixture-shared = { path = "../vendor", version = "9.8.7" }
serde = { version = "1.0.219" }
TOML
mkdir -p "$fixture/member/src"
printf '%s\n' '' > "$fixture/member/src/lib.rs"

write_package() {
  local path=$1
  local name=$2
  cat > "$path" <<JSON
{
  "name": "$name",
  "version": "0.23.0",
  "dependencies": {
    "@justinelliottcobb/amari-wasm": "^0.23.0",
    "unrelated": "^9.8.7"
  }
}
JSON
}

write_package "$fixture/amari-wasm/package.json" "@justinelliottcobb/amari-wasm"
write_package "$fixture/amari-wasm/examples/package.json" "amari-wasm-examples"
write_package "$fixture/typescript/package.json" "amari-typescript"
write_package "$fixture/examples/typescript/package.json" "amari-typescript-example"
write_package "$fixture/examples/web/interactive-demos/package.json" "amari-web-example"
write_package "$fixture/examples-suite/package.json" "amari-examples-suite"
cat > "$fixture/examples-suite/package-lock.json" <<'JSON'
{
  "name": "amari-examples-suite",
  "version": "0.23.0",
  "lockfileVersion": 3,
  "packages": {
    "": {
      "name": "amari-examples-suite",
      "version": "0.23.0"
    },
    "node_modules/unrelated": {
      "version": "9.8.7"
    }
  }
}
JSON

printf 'catalog_version = "0.23.0"\n' > "$fixture/amari-discovery/catalog/probes.toml"
printf 'catalog_version = "0.23.0"\n' > "$fixture/amari-discovery/catalog/semantic/core.toml"

(
  cd "$fixture"
  ./scripts/version-sync.sh set 0.24.0 >/dev/null
)

grep -q 'version = "0.24.0"' "$fixture/Cargo.toml"
grep -q 'path = "../member", version = "0.24.0"' "$fixture/member/Cargo.toml"
grep -q 'fixture-shared = { path = "../vendor", version = "9.8.7" }' "$fixture/member/Cargo.toml"
grep -q 'serde = { version = "1.0.219" }' "$fixture/member/Cargo.toml"
grep -q 'catalog_version = "0.24.0"' "$fixture/amari-discovery/catalog/probes.toml"

python3 - "$fixture" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
for relative in (
    "amari-wasm/package.json",
    "amari-wasm/examples/package.json",
    "typescript/package.json",
    "examples/typescript/package.json",
    "examples/web/interactive-demos/package.json",
    "examples-suite/package.json",
):
    data = json.loads((root / relative).read_text())
    assert data["version"] == "0.24.0", (relative, data["version"])
    assert data["dependencies"]["@justinelliottcobb/amari-wasm"] == "^0.24.0"
    assert data["dependencies"]["unrelated"] == "^9.8.7"

lock = json.loads((root / "examples-suite/package-lock.json").read_text())
assert lock["version"] == "0.24.0"
assert lock["packages"][""]["version"] == "0.24.0"
assert lock["packages"]["node_modules/unrelated"]["version"] == "9.8.7"
PY

# The legacy entry point must delegate to the same exhaustive authority.
(
  cd "$fixture"
  ./scripts/bump-version.sh 0.25.0 >/dev/null
  ./scripts/version-sync.sh verify 0.25.0 >/dev/null
)

# Verification must inspect nested manifests, not just count root matches.
sed -i 's/version = "0.25.0"/version = "0.23.0"/' "$fixture/member/Cargo.toml"
if (cd "$fixture" && ./scripts/version-sync.sh verify 0.25.0 >/dev/null 2>&1); then
  echo "version verification accepted a stale nested path dependency" >&2
  exit 1
fi

printf 'version-sync fixture passed\n'
