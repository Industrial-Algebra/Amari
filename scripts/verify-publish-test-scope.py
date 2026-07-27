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

timeout_lines = [
    line.strip()
    for line in validate.splitlines()
    if line.strip().startswith("timeout-minutes:")
]
if timeout_lines != ["timeout-minutes: 90"]:
    raise AssertionError(
        "validate job must use the empirically bounded 90-minute timeout; "
        f"found: {timeout_lines!r}"
    )
if "--test-threads" in validate:
    raise AssertionError("publish tests must not use global serialization")
if "./run_all_tests.sh" in validate:
    raise AssertionError("run_all_tests.sh reintroduces amari-gpu runtime tests")
clippy_commands = [
    line.strip()
    for line in validate.splitlines()
    if line.strip().startswith("cargo clippy ")
]
expected_clippy = [
    "cargo clippy --workspace --all-features -- -D warnings",
    (
        "cargo clippy --workspace --all-targets --all-features -- "
        "-D warnings -A clippy::needless_range_loop -A clippy::collapsible_match"
    ),
]
if clippy_commands != expected_clippy:
    raise AssertionError(
        "expected warning-denied normal targets plus the documented tagged-test "
        f"lint exception, found: {clippy_commands!r}"
    )

active_lines = [
    line.split("#", 1)[0].strip()
    for line in workflow.splitlines()
    if line.split("#", 1)[0].strip()
]
release_toolchain = "uses: dtolnay/rust-toolchain@1.97.1"
toolchain_uses = [
    line for line in active_lines if line.startswith("uses: dtolnay/rust-toolchain@")
]
if toolchain_uses != [release_toolchain] * 4:
    raise AssertionError(
        "all and only the four publish jobs must pin the verified Rust 1.97.1 "
        f"toolchain; found: {toolchain_uses!r}"
    )

workflow_preamble = workflow.split("\njobs:", 1)[0]
preamble_lines = [
    line.split("#", 1)[0].strip()
    for line in workflow_preamble.splitlines()
    if line.split("#", 1)[0].strip()
]
release_override = "RUSTUP_TOOLCHAIN: 1.97.1"
preamble_overrides = [
    line for line in preamble_lines if line.startswith("RUSTUP_TOOLCHAIN:")
]
all_overrides = [line for line in active_lines if line.startswith("RUSTUP_TOOLCHAIN:")]
if preamble_overrides != [release_override] or all_overrides != [release_override]:
    raise AssertionError(
        "publish workflow must have exactly one global rust-toolchain.toml "
        f"override, RUSTUP_TOOLCHAIN 1.97.1; found: {all_overrides!r}"
    )

print("publish test scope and effective release toolchain are pinned")
