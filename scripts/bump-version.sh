#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
# Compatibility entry point. version-sync.sh is the sole version authority.

set -euo pipefail

if [[ $# -ne 1 ]]; then
    echo "Usage: $0 <new_version>" >&2
    exit 1
fi

script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
echo "NOTICE: bump-version.sh delegates to version-sync.sh set" >&2
exec "$script_dir/version-sync.sh" set "$1"
