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

**Third incident, R5 verification (2026-10-20):** the R5 remediation's
"positive scratch test" ran the REAL script against the REAL evidence
tree — the script hardcoded EVID, so the env override was inert. The
overwritten stable/nightly/msrv timings/logs/cfg were caught as R6's
P3 residual, restored again from df42927 (byte-verified), and
run-pilot.sh now honors EVID_OVERRIDE so verification can run against
scratch copies without editing. The effective-cfg-*-pilot-empty.txt
files in this tree date from the R3 re-run (the original run predates
the probe); they document the R3-fixed script's behavior, not the
original 2026-10-18 execution.
