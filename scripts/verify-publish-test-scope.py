#!/usr/bin/env python3
# SPDX-License-Identifier: MIT OR Apache-2.0
"""Pin the publish workflow's validation policy (post-0.24.1 velocity redesign).

Policy:
- The validate job runs on the dedicated self-hosted release runner as a fast
  smoke gate. The full test/clippy/docs matrix is owned by PR CI
  (ci.yml + mathematical-correctness.yml), which every tagged commit has
  already passed; publish-time duplication cost ~55-60 minutes per release.
- Validate must not contain cargo test or cargo clippy invocations, must not
  use run_all_tests.sh, and must not serialize tests globally.
- crates.io publishing must not use fixed indexing sleeps (the 0.24.1 release
  spent ~20 minutes in sleep 45 calls); it must poll the sparse index at tier
  boundaries instead.
- All four publish jobs pin the verified Rust 1.97.1 toolchain through
  dtolnay/rust-toolchain@1.97.1 plus the single global RUSTUP_TOOLCHAIN
  override.
"""

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

runs_on_lines = [
    line.strip() for line in validate.splitlines() if line.strip().startswith("runs-on:")
]
if runs_on_lines != ["runs-on: [self-hosted, release]"]:
    raise AssertionError(
        "validate job must run on the dedicated self-hosted release runner; "
        f"found: {runs_on_lines!r}"
    )

timeout_lines = [
    line.strip()
    for line in validate.splitlines()
    if line.strip().startswith("timeout-minutes:")
]
if timeout_lines != ["timeout-minutes: 30"]:
    raise AssertionError(
        "validate smoke gate must keep the bounded 30-minute timeout; "
        f"found: {timeout_lines!r}"
    )

active_validate = [
    line.split("#", 1)[0]
    for line in validate.splitlines()
    if line.split("#", 1)[0].strip()
]
if any("cargo test" in line for line in active_validate):
    raise AssertionError(
        "publish-time cargo test reintroduces the duplicated matrix; "
        "the full test matrix is enforced by PR CI on the tagged commits"
    )
if any("cargo clippy" in line for line in active_validate):
    raise AssertionError(
        "publish-time cargo clippy reintroduces the duplicated matrix; "
        "warning-denied clippy is enforced by PR CI on the tagged commits"
    )
if "--test-threads" in validate:
    raise AssertionError("publish validation must not use global serialization")
if "./run_all_tests.sh" in validate:
    raise AssertionError("run_all_tests.sh reintroduces amari-gpu runtime tests")

required_smoke_steps = [
    "cargo fmt --all -- --check",
    "scripts/version-sync.sh verify",
    "scripts/verify-release-metadata.py",
    "generate_catalog",
    "scripts/generate-discovery-wasm-surface.sh",
    "scripts/verify-publish-order.py",
    "scripts/verify-workflow-crates.sh",
    "scripts/verify-amari-binary-owner.py",
    "cargo check --workspace --all-features",
]
for required in required_smoke_steps:
    if required not in validate:
        raise AssertionError(f"missing required smoke step: {required}")

publish_match = re.search(
    r"(?ms)^  publish-crates:\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\n)", workflow
)
if publish_match is None:
    raise AssertionError("publish workflow publish-crates job not found")
publish = publish_match.group("body")

if "TIERS=(" not in publish:
    raise AssertionError("publish-crates must publish in dependency tiers (TIERS)")
if "CRATES=(" in publish:
    raise AssertionError("stale flat CRATES array found; use tiered TIERS")
if re.search(r"sleep\s+45", publish):
    raise AssertionError(
        "fixed indexing sleeps are forbidden; poll the sparse index instead"
    )
for required in ("sparse_index_url", "wait_for_index", "index.crates.io"):
    if required not in publish:
        raise AssertionError(f"publish-crates must poll the sparse index ({required})")

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

npm_match = re.search(
    r"(?ms)^  publish-npm:\n(?P<body>.*?)(?=^  [a-zA-Z0-9_-]+:\n|\Z)", workflow
)
if npm_match is None:
    raise AssertionError("publish workflow publish-npm job not found")
publish_npm = npm_match.group("body")
publish_npm_active = "\n".join(
    line.split("#", 1)[0] for line in publish_npm.splitlines()
)
if "id-token: write" not in publish_npm_active:
    raise AssertionError(
        "publish-npm must grant id-token: write for trusted publishing (OIDC)"
    )
if "NODE_AUTH_TOKEN" in publish_npm_active or "secrets.NPM_TOKEN" in publish_npm_active:
    raise AssertionError(
        "publish-npm must not use the dead NPM_TOKEN secret; trusted "
        "publishing (OIDC) replaced classic npm tokens"
    )
if "node-version: '24'" not in publish_npm_active:
    raise AssertionError(
        "publish-npm must use Node 24 (trusted publishing requires Node "
        ">= 22.14.0 with npm CLI >= 11.5.1)"
    )
if "npm whoami" in publish_npm_active:
    raise AssertionError(
        "publish-npm must not run npm whoami; OIDC identity exists only "
        "during npm publish"
    )

print("publish workflow policy is pinned (self-hosted smoke, tiered polling)")
