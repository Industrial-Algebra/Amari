#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Private deterministic fixture for probe-supervisor process tests."""

from __future__ import annotations

import json
import os
import struct
import sys
import time


def write_frame(value: object) -> None:
    body = json.dumps(value, sort_keys=True, separators=(",", ":")).encode()
    sys.stdout.buffer.write(struct.pack(">I", len(body)))
    sys.stdout.buffer.write(body)
    sys.stdout.buffer.flush()


def success_response() -> dict[str, object]:
    return {
        "provenance": {
            "tool_version": "fixture",
            "catalog": {"version": "fixture", "hash": "fixture-catalog"},
            "compatibility": {"status": "compatible", "reasons": []},
            "replay": {"replayable": True, "required_hashes": [], "reasons": []},
            "project_hash": None,
            "input_hash": "fixture-input",
            "seed": None,
        },
        "execution": {
            "probe_id": "amari-probe:tropical:viterbi:v1",
            "input_schema": "fixture/input/v1",
            "output_schema": "fixture/output/v1",
            "backend": "cpu",
            "isolation": "cooperative",
            "deterministic": True,
            "resources": {"operations": 1, "nodes": 1, "iterations": 1, "bytes": 1},
            "output": {"fixture": True},
        },
    }


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
    if mode == "valid":
        write_frame(success_response())
        return 0
    if mode == "simultaneous":
        sys.stderr.buffer.write(b"d" * (256 * 1024))
        sys.stderr.buffer.flush()
        write_frame(success_response())
        return 0
    if mode == "flood-stdout":
        while True:
            sys.stdout.buffer.write(b"o" * 8192)
            sys.stdout.buffer.flush()
    if mode == "flood-stderr":
        while True:
            sys.stderr.buffer.write(b"e" * 8192)
            sys.stderr.buffer.flush()
    if mode == "slow":
        pid_file = sys.argv[2]
        orphan_marker = sys.argv[3]
        with open(pid_file, "w", encoding="ascii") as handle:
            handle.write(str(os.getpid()))
            handle.flush()
        time.sleep(5.0)
        with open(orphan_marker, "w", encoding="ascii") as handle:
            handle.write("worker-survived-timeout")
        return 0
    print(f"unknown fixture mode: {mode}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
