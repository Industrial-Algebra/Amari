#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Verify the installed `amari` binary has exactly one workspace owner."""

import json
import subprocess

metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"]
    )
)
owners = [
    package["name"]
    for package in metadata["packages"]
    if any(
        target["name"] == "amari" and "bin" in target["kind"]
        for target in package["targets"]
    )
]

assert owners == ["amari-discovery"], owners
