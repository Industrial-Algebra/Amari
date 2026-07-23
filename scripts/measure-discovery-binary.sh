#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Build and record the release-mode amari discovery binary measurement.

set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
BINARY="$ROOT/target/release/amari"
REPORT="$ROOT/amari-discovery/benchmarks.md"
START='<!-- discovery-binary-measurement:start -->'
END='<!-- discovery-binary-measurement:end -->'

cd "$ROOT"
cargo build --release -p amari-discovery --bin amari

test -f "$BINARY"
test -f "$REPORT"

bytes=$(wc -c < "$BINARY" | tr -d '[:space:]')
if command -v sha256sum >/dev/null 2>&1; then
    sha256=$(sha256sum "$BINARY" | awk '{print $1}')
elif command -v shasum >/dev/null 2>&1; then
    sha256=$(shasum -a 256 "$BINARY" | awk '{print $1}')
else
    echo "error: sha256sum or shasum is required" >&2
    exit 1
fi
rustc_version=$(rustc --version)
host=$(rustc -vV | awk '/^host: / {print $2}')
measured_utc=$(date -u +%Y-%m-%d)
mebibytes=$(python3 - "$bytes" <<'PY'
import sys
print(f"{int(sys.argv[1]) / (1024 * 1024):.2f}")
PY
)

python3 - "$REPORT" "$START" "$END" "$measured_utc" "$rustc_version" "$host" "$bytes" "$mebibytes" "$sha256" <<'PY'
from pathlib import Path
import sys

report_path = Path(sys.argv[1])
start, end = sys.argv[2], sys.argv[3]
measured_utc, rustc_version, host = sys.argv[4], sys.argv[5], sys.argv[6]
bytes_value, mebibytes, sha256 = sys.argv[7], sys.argv[8], sys.argv[9]
text = report_path.read_text()
if text.count(start) != 1 or text.count(end) != 1 or text.index(start) >= text.index(end):
    raise SystemExit("binary measurement markers are missing, duplicated, or out of order")
managed = "\n".join(
    [
        start,
        "",
        f"- Measured UTC: `{measured_utc}`",
        "- Build: `cargo build --release -p amari-discovery --bin amari`",
        f"- Toolchain: `{rustc_version}`",
        f"- Host target: `{host}`",
        "- Profile: `release`",
        "- Binary: `target/release/amari`",
        f"- Size: `{bytes_value}` bytes (`{mebibytes}` MiB)",
        f"- SHA-256: `{sha256}`",
        "",
        end,
    ]
)
prefix, remainder = text.split(start, 1)
_, suffix = remainder.split(end, 1)
report_path.write_text(prefix + managed + suffix)
PY

cat <<EOF
amari discovery release binary
  path: $BINARY
  host: $host
  size: $bytes bytes ($mebibytes MiB)
  sha256: $sha256
  report: $REPORT
EOF
