#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Verify release metadata consistency for the tagged version.

Checks, against cargo metadata:
- every workspace package version equals the expected release version;
- every internal dependency requirement is either `*` (workspace
  inheritance), `=<version>`, or `^<version>` for the expected version.
"""

import json
import pathlib
import subprocess
import sys

if len(sys.argv) != 2:
    raise SystemExit("usage: verify-release-metadata.py <expected-version>")

expected = sys.argv[1]
ROOT = pathlib.Path(__file__).resolve().parents[1]
metadata = json.loads(
    subprocess.check_output(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=ROOT
    )
)
packages = metadata["packages"]
names = {package["name"] for package in packages}
errors = []

for package in packages:
    if package["version"] != expected:
        errors.append(
            f"{package['name']}: version {package['version']} != {expected}"
        )

allowed_reqs = {"*", f"={expected}", f"^{expected}"}
for package in packages:
    for dependency in package["dependencies"]:
        if dependency["name"] not in names:
            continue
        req = dependency["req"]
        if req not in allowed_reqs:
            errors.append(
                f"{package['name']}: internal dependency {dependency['name']} "
                f"has requirement {req!r} (allowed: {sorted(allowed_reqs)})"
            )

if errors:
    raise AssertionError("release metadata mismatch:\n- " + "\n- ".join(errors))

print(f"release metadata is consistent at {expected} ({len(packages)} packages)")
