#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Verify crates.io packages are published after workspace dependencies."""

import json
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
workflow = (ROOT / ".github/workflows/publish.yml").read_text()
match = re.search(r"CRATES=\(\n(?P<body>.*?)\n\s*\)", workflow, re.DOTALL)
if match is None:
    raise AssertionError("publish workflow CRATES array not found")

published = re.findall(r'^\s*"([^"]+)"\s*$', match.group("body"), re.MULTILINE)
if len(published) != len(set(published)):
    raise AssertionError("publish workflow contains duplicate crates")

metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT
    )
)
packages = {package["name"]: package for package in metadata["packages"]}
positions = {name: index for index, name in enumerate(published)}
errors = []

for name in published:
    if name not in packages:
        errors.append(f"{name}: not a workspace package")
        continue
    for dependency in packages[name]["dependencies"]:
        dependency_name = dependency["name"]
        if dependency_name not in positions:
            continue
        if positions[dependency_name] >= positions[name]:
            errors.append(f"{name}: dependency {dependency_name} must be published first")

if positions.get("amari-discovery", -1) >= positions.get("amari", -1):
    errors.append("amari-discovery must be published before amari")

if errors:
    raise AssertionError("invalid publish order:\n- " + "\n- ".join(errors))

print("publish order is dependency-safe")
