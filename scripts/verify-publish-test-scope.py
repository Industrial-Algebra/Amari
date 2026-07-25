#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Verify that publish validation never executes legacy GPU runtime tests."""

from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
workflow = (ROOT / ".github/workflows/publish.yml").read_text()
match = re.search(
    r"(?ms)^  validate:\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\n)", workflow
)
if match is None:
    raise AssertionError("publish workflow validate job not found")

validate = match.group("body")
commands = [
    line.strip()
    for line in validate.splitlines()
    if line.strip().startswith("cargo test ")
]
workspace = [line for line in commands if line.startswith("cargo test --workspace")]
expected_workspace = ["cargo test --workspace --exclude amari-gpu"]
if workspace != expected_workspace:
    raise AssertionError(
        f"expected one GPU-excluded workspace test, found: {workspace!r}"
    )

for required in (
    "cargo test -p amari-discovery --all-features",
    "cargo test -p amari-holographic --all-features",
):
    if required not in commands:
        raise AssertionError(f"missing required publish test: {required}")

if "timeout-minutes:" not in validate:
    raise AssertionError("validate job must have timeout-minutes")
if "--test-threads" in validate:
    raise AssertionError("publish tests must not use global serialization")
if "./run_all_tests.sh" in validate:
    raise AssertionError("run_all_tests.sh reintroduces amari-gpu runtime tests")
if "cargo clippy --all-targets --all-features -- -D warnings" not in validate:
    raise AssertionError("warning-denied all-feature clippy gate is missing")

print("publish test scope excludes amari-gpu runtime execution")
