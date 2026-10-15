#!/usr/bin/env bash
# SPDX-License-Identifier: MIT OR Apache-2.0
#
# mrep-b-negative-batteries.sh — adversarial negative controls for the M-REP-B
# suite-integrity gates. Each mock reproduces a FALSE GREEN that review round 1
# demonstrated at HEAD 601bb3b:
#
#   1. prefixed rename  — exact-inventory matching must reject a renamed test
#                          (F1, scripts/verify-named-suite.sh);
#   2. duplicate args   — duplicate approved names must be a usage error
#                          (F1, scripts/verify-named-suite.sh);
#   3. cargo failure    — the coverage gather must block when cargo emits 300
#                          matching records and then exits 101 (F2, hook +
#                          workflow).
#
# A fourth source guard asserts the status-preserving gather is present in both
# artifacts, so the behavioral battery cannot silently drift from them.
#
# Exit status: 0 only when all batteries and the guard pass.

set -uo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HELPER="$REPO_ROOT/scripts/verify-named-suite.sh"
HOOK="$REPO_ROOT/.githooks/pre-commit"
WORKFLOW="$REPO_ROOT/.github/workflows/mathematical-correctness.yml"

FAILURES=0

# Run a command expected to exit nonzero; record pass/fail.
expect_nonzero() {
    local name="$1"
    shift
    local rc=0
    "$@" >/dev/null 2>&1 || rc=$?
    if [ "$rc" -eq 0 ]; then
        echo "BATTERY FAIL: $name exited 0 (expected nonzero)"
        FAILURES=$((FAILURES + 1))
    else
        echo "BATTERY OK:   $name exited $rc (nonzero)"
    fi
}

# --- Battery 1: a prefixed rename must fail exact-inventory matching -------
battery_rename() {
    cargo() {
        if [[ " $* " == *" --list "* ]]; then
            echo 'renamed::geometric_axioms::associativity: test'
        else
            echo 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'
        fi
    }
    export -f cargo
    bash "$HELPER" dummy geometric_axioms geometric_axioms::associativity
}

# --- Battery 2: duplicate approved names must be a usage error -------------
battery_duplicate() {
    cargo() {
        if [[ " $* " == *" --list "* ]]; then
            echo 'suite::a: test'
        else
            echo 'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out'
        fi
    }
    export -f cargo
    bash "$HELPER" dummy suite suite::a suite::a
}

# --- Battery 3: cargo emits 300 records then exits 101 -> gate must block ---
# Verbatim reproduction of the coverage-floor gather + floor as it now stands
# in .githooks/pre-commit and .github/workflows/mathematical-correctness.yml.
battery_cargo_failure() {
    cargo() {
        local i
        for ((i = 0; i < 300; i++)); do
            echo "test_$i: test"
        done
        return 101
    }
    export -f cargo
    local COVERAGE_LIST COVERAGE_STATUS TOTAL_TESTS
    COVERAGE_LIST=$(cargo test --workspace --lib --bins -- --list 2>/dev/null) || COVERAGE_STATUS=$?
    COVERAGE_STATUS=${COVERAGE_STATUS:-0}
    if [ "$COVERAGE_STATUS" -ne 0 ]; then
        echo "  (gather blocked: cargo exit $COVERAGE_STATUS)" >&2
        return 1
    fi
    TOTAL_TESTS=$(printf '%s\n' "$COVERAGE_LIST" | grep -c ": test$" || true)
    if [ -z "$TOTAL_TESTS" ]; then
        TOTAL_TESTS=0
    fi
    if [ "$TOTAL_TESTS" -lt 300 ]; then
        echo "  (floor blocked: $TOTAL_TESTS < 300)" >&2
        return 1
    fi
    echo "  FALSE GREEN: $TOTAL_TESTS records accepted despite cargo failure" >&2
    return 0
}

# --- Source guard: the real artifacts carry the status check ---------------
guard_source() {
    local failed=0
    if ! grep -Fq 'COVERAGE_LIST=$(cargo test --workspace --lib --bins -- --list 2>/dev/null) || COVERAGE_STATUS=$?' "$HOOK"; then
        echo "GUARD FAIL: hook lacks the status-preserving coverage gather"
        failed=1
    fi
    if ! grep -Fq 'TOTAL_TESTS_LIST=$(cargo test --workspace --lib --bins -- --list 2>/dev/null) || TOTAL_TESTS_STATUS=$?' "$WORKFLOW"; then
        echo "GUARD FAIL: workflow lacks the status-preserving total gather"
        failed=1
    fi
    if ! grep -Fq 'FORMAL_FEATURE_TESTS_LIST=$(cargo test --workspace --features formal-verification -- --list 2>/dev/null) || FORMAL_FEATURE_TESTS_STATUS=$?' "$WORKFLOW"; then
        echo "GUARD FAIL: workflow lacks the status-preserving formal gather"
        failed=1
    fi
    if ! grep -Fq 'SELECTED_SORTED=$(printf' "$HELPER"; then
        echo "GUARD FAIL: helper lacks exact sorted-inventory comparison"
        failed=1
    fi
    if ! grep -Fq 'duplicate approved name' "$HELPER"; then
        echo "GUARD FAIL: helper lacks duplicate-approved-name rejection"
        failed=1
    fi
    if [ "$failed" -eq 0 ]; then
        echo "GUARD OK:     hook + workflow + helper carry the hardened gates"
    else
        FAILURES=$((FAILURES + 1))
    fi
}

expect_nonzero "rename-prefix mock" battery_rename
expect_nonzero "duplicate-args mock" battery_duplicate
expect_nonzero "cargo-failure mock (300 records, exit 101)" battery_cargo_failure
guard_source

echo ""
if [ "$FAILURES" -eq 0 ]; then
    echo "ALL BATTERIES PASSED (3 behavioral + 1 source guard)"
    exit 0
fi
echo "NEGATIVE BATTERIES FAILED: $FAILURES"
exit 1
