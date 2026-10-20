# Archive note (2026-10-19)

The full per-invocation log set (logs/) and per-row test-name files
(tests/) were NOT committed: 3,200+ files whose RED content is three
error families repeated per row and whose green content is uniform
shape. Retained instead: complete results.tsv (every invocation, wall,
exit), complete failures.txt (every failure), samples/ (one log per
failure family + first green row per command), tests2-manifest.tsv
(per-row count + sha256 of the sorted full-identity name list — F2
nonemptiness auditable), tests2-union.txt (the 524-name union, R1
recapture), tests-manifest.superseded.tsv and
tests-union.superseded.txt (the original collapsed inventories, kept for
the record), tests2-samples/ (two full per-row inventories). The full sets are
deterministically reproducible from the committed resolution evidence:
the exact executed lock (`../executed-Cargo.lock`, sha256 in
`../executed-Cargo.lock.sha256` — restore as the workspace `Cargo.lock`),
the exact toolchain identities (`../toolchain-pins.txt`, asserted by the
runners), the runner script, and the revision (develop 6463630 tree).
