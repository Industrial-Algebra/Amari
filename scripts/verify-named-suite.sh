#!/usr/bin/env bash
#
# verify-named-suite.sh — executable vacuity assertion for a named cargo-test
# filter (CORE-D07 matrix §3, "named-suite filter integrity"; M-REP-B).
#
# A filtered invocation such as `cargo test -p amari-core geometric_axioms`
# exits 0 and prints "0 passed" when the filter matches nothing: an evidence-
# integrity defect, not a green row. This helper turns such an invocation into
# a real gate. It:
#
#   1. lists the tests the filter selects (`-- --list`);
#   2. requires the list to contain EXACTLY the approved inventory names;
#   3. runs the filtered tests and parses the `test result:` lines;
#   4. requires `passed >= 1` (non-vacuous) and `ignored == 0` (the approved
#      inventories contain no ignored/skipped tests).
#
# Usage:
#   scripts/verify-named-suite.sh [--features <FEATURES>] <PACKAGE> <FILTER> \
#       <EXPECTED_NAME> [<EXPECTED_NAME>...]
#
# Example:
#   scripts/verify-named-suite.sh amari-core geometric_axioms \
#       geometric_axioms::associativity geometric_axioms::distributivity
#
# Exit status: 0 only when the filter is non-vacuous and the inventory is
# complete; nonzero otherwise.

set -uo pipefail

FEATURES=""
if [ "${1:-}" = "--features" ]; then
    if [ "$#" -lt 3 ]; then
        echo "verify-named-suite: --features requires a value and a suite" >&2
        exit 2
    fi
    FEATURES="$2"
    shift 2
fi

if [ "$#" -lt 3 ]; then
    echo "verify-named-suite: usage: [--features <FEATURES>] <PACKAGE> <FILTER> <EXPECTED_NAME>..." >&2
    exit 2
fi

PACKAGE="$1"
FILTER="$2"
shift 2

EXPECTED_COUNT="$#"

cargo_args=(test --locked --quiet --package "$PACKAGE")
if [ -n "$FEATURES" ]; then
    cargo_args+=(--features "$FEATURES")
fi

SUITE="$PACKAGE :: $FILTER"
if [ -n "$FEATURES" ]; then
    SUITE="$SUITE (features: $FEATURES)"
fi

# 1. List the selected tests. A compile failure here must fail the gate.
LIST_OUT=$(cargo "${cargo_args[@]}" "$FILTER" -- --list 2>&1)
LIST_STATUS=$?
if [ "$LIST_STATUS" -ne 0 ]; then
    echo "verify-named-suite: FAIL — could not list $SUITE (exit $LIST_STATUS)" >&2
    printf '%s\n' "$LIST_OUT" >&2
    exit 1
fi

# 2. Exact inventory comparison (count + membership).
SELECTED_COUNT=$(printf '%s\n' "$LIST_OUT" | grep -c ': test$' || true)
if [ -z "$SELECTED_COUNT" ]; then
    SELECTED_COUNT=0
fi
if [ "$SELECTED_COUNT" -eq 0 ]; then
    echo "verify-named-suite: FAIL — VACUOUS: $SUITE selected 0 tests" >&2
    printf '%s\n' "$LIST_OUT" >&2
    exit 1
fi
if [ "$SELECTED_COUNT" -ne "$EXPECTED_COUNT" ]; then
    echo "verify-named-suite: FAIL — $SUITE selected $SELECTED_COUNT test(s), approved inventory has $EXPECTED_COUNT" >&2
    printf '%s\n' "$LIST_OUT" | grep ': test$' | sed 's/^/    selected: /' >&2
    exit 1
fi
for name in "$@"; do
    if ! printf '%s\n' "$LIST_OUT" | grep -qF "$name: test"; then
        echo "verify-named-suite: FAIL — expected test '$name' not selected by $SUITE" >&2
        printf '%s\n' "$LIST_OUT" | grep ': test$' | sed 's/^/    selected: /' >&2
        exit 1
    fi
done

# 3. Run the filtered tests and parse the `test result:` lines.
RUN_OUT=$(cargo "${cargo_args[@]}" "$FILTER" 2>&1)
RUN_STATUS=$?
PASSED=$(printf '%s\n' "$RUN_OUT" | sed -n 's/.*test result: [A-Za-z]*\. \([0-9][0-9]*\) passed.*/\1/p' | awk '{s+=$1} END {print s+0}')
IGNORED=$(printf '%s\n' "$RUN_OUT" | sed -n 's/.* \([0-9][0-9]*\) ignored.*/\1/p' | awk '{s+=$1} END {print s+0}')

if [ "$RUN_STATUS" -ne 0 ]; then
    echo "verify-named-suite: FAIL — $SUITE run exited $RUN_STATUS" >&2
    printf '%s\n' "$RUN_OUT" >&2
    exit 1
fi
if [ "$PASSED" -lt 1 ]; then
    echo "verify-named-suite: FAIL — VACUOUS: $SUITE reported $PASSED passed test(s)" >&2
    printf '%s\n' "$RUN_OUT" >&2
    exit 1
fi
if [ "$IGNORED" -ne 0 ]; then
    echo "verify-named-suite: FAIL — $SUITE reported $IGNORED ignored test(s); approved inventory declares none" >&2
    exit 1
fi

echo "verify-named-suite: OK — $SUITE: $PASSED passed, $IGNORED ignored, inventory of $EXPECTED_COUNT complete"
exit 0
