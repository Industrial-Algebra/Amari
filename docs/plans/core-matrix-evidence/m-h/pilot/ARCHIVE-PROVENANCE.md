# Pilot artifact provenance (R4 finding)

The R3 verification re-executed run-pilot.sh in all three modes, which
overwrote the archived 2026-10-18 timings/logs/cfg files (the nightly
FULL row became 18.9 s). The ORIGINAL 2026-10-18 artifacts — the actual
measurements behind PILOT.md's table — were restored from the pilot's
first commit (df42927); run-pilot.sh retains the post-R3 fixes. The
current timings-*.tsv files are byte-identical to the originals
(nightly FULL total 18.8 s, matching the table). Lesson recorded:
verification re-runs of archiving scripts must target scratch copies,
never the evidence tree.
