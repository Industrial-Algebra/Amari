# Archive note (2026-10-20)

Supplemental stage created lean by design (post-R1): committed are the
per-quadrant results TSVs (every invocation), the failure inventory, the
resolved-activation comparison files (features/),
sample logs for each failure family and one green alias test, and the runner.
Full logs regenerate deterministically: runner + executed-Cargo.lock +
toolchain-pins.txt + revision. Expected-RED: the three MATRIX-05 singleton
families (gf2 / phantom-types / formal-verification alone, no std) fail in
every quadrant — the +std variants and all four alias rows are green.
