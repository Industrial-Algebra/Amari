#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Private deterministic fixture for probe-supervisor process tests."""

from __future__ import annotations

import json
import os
import sys


def main() -> int:
    mode = sys.argv[1] if len(sys.argv) > 1 else ""
    if mode == "context":
        print(
            json.dumps(
                {
                    "argv": sys.argv[1:],
                    "cwd": os.getcwd(),
                    "environment": dict(sorted(os.environ.items())),
                },
                sort_keys=True,
            )
        )
        return 0
    print(f"unknown fixture mode: {mode}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
