#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0

"""Verify or run one exhaustive amari-discovery integration-test shard."""

from __future__ import annotations

import subprocess
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TEST_DIR = ROOT / "amari-discovery/tests"

SHARDS: dict[str, tuple[str, ...]] = {
    "catalog": (
        "catalog_cfg",
        "catalog_exports",
        "catalog_generation",
        "catalog_integrity",
        "catalog_macros",
        "catalog_modules",
        "catalog_package_links",
        "catalog_packages",
        "catalog_signatures",
        "catalog_traits",
        "catalog_wasm",
        "ndjson",
        "protocol",
        "schema_contract",
    ),
    "inspection": (
        "cargo_inspection",
        "cargo_platform_inspection",
        "inspection_malformed",
        "inspection_paths",
        "inspection_privacy",
        "inspection_safety",
        "npm_inspection",
        "npm_packages",
        "rust_inspection",
        "rust_source_inspection",
        "ts_source_inspection",
    ),
    "planner": (
        "agent_contract",
        "ai_contract",
        "cli_capabilities",
        "cli_discover",
        "cli_plan_replay",
        "cli_probes",
        "cli_recommend_rust",
        "cli_recommend_ts",
        "plan_normalization",
        "planner_graph",
        "planner_ranking",
        "planner_recall",
        "probe_cgt",
        "probe_core",
        "probe_dual",
        "probe_engine",
        "probe_holographic",
        "probe_network",
        "probe_optimization",
        "probe_rewrite_infer",
        "probe_rewrite_normalize",
        "probe_rewrite_predecessors",
        "probe_surreal",
        "probe_tropical",
        "probe_worker_protocol",
        "shell",
        "shell_agent_contract",
    ),
}


def verify() -> int:
    actual = {path.stem for path in TEST_DIR.glob("*.rs")}
    assigned = [test for tests in SHARDS.values() for test in tests]
    counts = Counter(assigned)
    duplicates = sorted(test for test, count in counts.items() if count != 1)
    missing = sorted(actual - counts.keys())
    stale = sorted(counts.keys() - actual)
    if duplicates or missing or stale:
        if duplicates:
            print(f"ERROR: duplicate shard assignments: {', '.join(duplicates)}", file=sys.stderr)
        if missing:
            print(f"ERROR: unassigned discovery tests: {', '.join(missing)}", file=sys.stderr)
        if stale:
            print(f"ERROR: stale discovery tests: {', '.join(stale)}", file=sys.stderr)
        return 1
    print(f"Verified {len(actual)} discovery test targets across {len(SHARDS)} unique shards.")
    return 0


def run(shard: str) -> int:
    if verify() != 0:
        return 1
    tests = SHARDS.get(shard)
    if tests is None:
        print(f"ERROR: unknown discovery test shard `{shard}`", file=sys.stderr)
        return 2
    command = ["cargo", "test", "--package", "amari-discovery"]
    if shard == "catalog":
        command.extend(("--lib", "--bins"))
    if shard == "planner":
        command.extend(("--features", "ai"))
    for test in tests:
        command.extend(("--test", test))
    print(f"Running discovery {shard} shard ({len(tests)} integration targets)...", flush=True)
    return subprocess.run(command, cwd=ROOT, check=False).returncode


def main(arguments: list[str]) -> int:
    if arguments == ["--verify"]:
        return verify()
    if len(arguments) == 1:
        return run(arguments[0])
    print("usage: run-discovery-test-shard.py [--verify|catalog|inspection|planner]", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
