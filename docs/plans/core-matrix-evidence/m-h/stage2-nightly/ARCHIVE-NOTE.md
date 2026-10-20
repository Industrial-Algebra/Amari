# Archive note (2026-10-19)

The full per-invocation log set (logs/) and per-row test-name files
(tests/) were NOT committed: 3,200+ files whose RED content is three
error families repeated per row and whose green content is uniform
shape. Retained instead: complete results.tsv (every invocation, wall,
exit), complete failures.txt (every failure), samples/ (one log per
failure family + first green row per command), tests-manifest.tsv
(per-row count + sha256 of the sorted name list — F2 nonemptiness
auditable), tests-union.txt (the 523-name union). The full sets are
deterministically reproducible from the committed resolution evidence:
the exact executed lock (`../executed-Cargo.lock`, sha256 in
`../executed-Cargo.lock.sha256` — restore as the workspace `Cargo.lock`),
the exact toolchain identities (`../toolchain-pins.txt`, asserted by the
runners), the runner script, and the revision (develop 6463630 tree).
