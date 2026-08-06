#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Verify crates.io packages publish in dependency-safe tiers.

The publish workflow groups crates into TIERS: crates within one tier must
not depend on any published crate in the same tier, and every published
internal dependency must live in a strictly earlier tier. Tier boundaries are
the only points where the workflow waits for crates.io indexing.
"""

import json
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[1]
workflow = (ROOT / ".github/workflows/publish.yml").read_text()
match = re.search(r"TIERS=\(\n(?P<body>.*?)\n\s*\)", workflow, re.DOTALL)
if match is None:
    raise AssertionError("publish workflow TIERS array not found")

tier_lines = re.findall(r'^\s*"([^"]+)"\s*$', match.group("body"), re.MULTILINE)
if not tier_lines:
    raise AssertionError("publish workflow TIERS array is empty")

tiers = [line.split() for line in tier_lines]
published = [crate for tier in tiers for crate in tier]
if len(published) != len(set(published)):
    raise AssertionError("publish workflow contains duplicate crates across tiers")

metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT
    )
)
packages = {package["name"]: package for package in metadata["packages"]}
tier_of = {}
for index, tier in enumerate(tiers):
    for name in tier:
        tier_of[name] = index
positions = {name: index for index, name in enumerate(published)}
errors = []

for name in published:
    if name not in packages:
        errors.append(f"{name}: not a workspace package")
        continue
    for dependency in packages[name]["dependencies"]:
        dependency_name = dependency["name"]
        if dependency_name not in tier_of or dependency_name == name:
            continue
        if tier_of[dependency_name] >= tier_of[name]:
            errors.append(
                f"{name} (tier {tier_of[name]}): dependency {dependency_name} "
                f"must be in a strictly earlier tier (found tier "
                f"{tier_of[dependency_name]})"
            )

if positions.get("amari-discovery", -1) >= positions.get("amari", -1):
    errors.append("amari-discovery must be published before amari")

if errors:
    raise AssertionError("invalid publish tiers:\n- " + "\n- ".join(errors))

print(f"publish tiers are dependency-safe ({len(tiers)} tiers, {len(published)} crates)")
