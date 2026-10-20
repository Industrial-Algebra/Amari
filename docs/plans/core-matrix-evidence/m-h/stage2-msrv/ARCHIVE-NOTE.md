# Archive note (2026-10-19)

The full per-invocation log set (logs/) and per-row test-name files
(tests/) were NOT committed: 3,200+ files whose RED content is three
error families repeated per row and whose green content is uniform
shape. Retained instead: complete results.tsv (every invocation, wall,
exit), complete failures.txt (every failure), samples/ (one log per
failure family + first green row per command), tests-manifest.tsv
(per-row count + sha256 of the sorted name list — F2 nonemptiness
auditable) for the test-executing quadrants. The full sets are
deterministically reproducible: runner (../stage*.sh or pilot/run-pilot.sh),
revision (develop 6463630 tree), pinned toolchains, regenerated lock.
