#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Verify exhaustive discovery sharding and stable required-check contracts."""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CI = ROOT / ".github/workflows/ci.yml"
MATH = ROOT / ".github/workflows/mathematical-correctness.yml"
RUNNER = ROOT / "scripts/run-discovery-test-shard.py"


def require(text: str, needle: str, source: Path, errors: list[str]) -> None:
    if needle not in text:
        errors.append(f"{source.relative_to(ROOT)} must contain: {needle}")


def forbid(text: str, needle: str, source: Path, errors: list[str]) -> None:
    if needle in text:
        errors.append(f"{source.relative_to(ROOT)} must not contain: {needle}")


def main() -> int:
    errors: list[str] = []
    ci = CI.read_text(encoding="utf-8")
    math = MATH.read_text(encoding="utf-8")

    require(
        ci,
        "cargo test --workspace --exclude amari-discovery --features native-precision",
        CI,
        errors,
    )
    require(
        ci,
        "cargo test --workspace --exclude amari-discovery --features high-precision",
        CI,
        errors,
    )
    require(ci, "python3 scripts/verify-discovery-ci-sharding.py", CI, errors)
    for shard, name in (
        ("catalog", "Discovery Catalog Tests"),
        ("inspection", "Discovery Inspection Tests"),
        ("planner", "Discovery Planner and CLI Tests"),
    ):
        require(math, f"name: {name}", MATH, errors)
        require(
            math,
            f"python3 scripts/run-discovery-test-shard.py {shard}",
            MATH,
            errors,
        )
    require(math, "name: Discovery Integration Tests", MATH, errors)
    require(math, "name: Mathematical Correctness Check", MATH, errors)
    forbid(
        math,
        "cargo test --package amari-discovery --tests",
        MATH,
        errors,
    )

    if not RUNNER.is_file():
        errors.append("scripts/run-discovery-test-shard.py must exist")
    else:
        result = subprocess.run(
            [sys.executable, str(RUNNER), "--verify"],
            cwd=ROOT,
            check=False,
        )
        if result.returncode != 0:
            errors.append("discovery shard assignments must be exhaustive and unique")

    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print("Discovery CI sharding is exhaustive and required check names are stable.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
