#!/bin/bash
set -e

# Script to verify all workspace crates are included in CI/CD workflows
# This prevents the common issue where adding a crate to workspace doesn't automatically add it to workflows

echo "🔍 Verifying workspace crates are in CI/CD workflows..."
echo ""

# Extract workspace members from Cargo.toml
WORKSPACE_MEMBERS=$(grep -A 1 "^\[workspace\]" Cargo.toml | grep "members" | sed 's/members = \[//' | sed 's/\]//' | tr ',' '\n' | sed 's/"//g' | sed 's/ //g' | grep -v "^$")

# Extract crates from publish.yml TIERS (one quoted space-separated tier per line)
PUBLISH_CRATES=$(awk '/TIERS=\(/{flag=1; next} flag && /^\s*\)/{flag=0} flag {print}' .github/workflows/publish.yml | grep -oE '"[^"]+"' | tr -d '"' | tr ' ' '\n' | grep -v "^$" | sort -u)

# Exclude crates that are intentionally not published to crates.io
# amari-wasm: Published to npm instead
EXCLUDED_CRATES="amari-wasm"

# Also check main amari crate
PUBLISH_CRATES=$(echo "$PUBLISH_CRATES" | grep -v '^amari$')
WORKSPACE_MEMBERS=$(echo "$WORKSPACE_MEMBERS" | grep -v "^amari$")

# Filter out excluded crates from workspace members for comparison
for excluded in $EXCLUDED_CRATES; do
    WORKSPACE_MEMBERS=$(echo "$WORKSPACE_MEMBERS" | grep -v "^$excluded$")
done

echo "📦 Workspace members:"
echo "$WORKSPACE_MEMBERS" | sort
echo ""

echo "🚀 Crates in publish.yml:"
echo "$PUBLISH_CRATES" | sort
echo ""

# Find missing crates
MISSING_FROM_PUBLISH=""
for crate in $WORKSPACE_MEMBERS; do
    if ! echo "$PUBLISH_CRATES" | grep -q "^$crate$"; then
        MISSING_FROM_PUBLISH="$MISSING_FROM_PUBLISH $crate"
    fi
done

# Find extra crates (in workflow but not in workspace)
EXTRA_IN_PUBLISH=""
for crate in $PUBLISH_CRATES; do
    if ! echo "$WORKSPACE_MEMBERS" | grep -q "^$crate$"; then
        EXTRA_IN_PUBLISH="$EXTRA_IN_PUBLISH $crate"
    fi
done

# Report results
if [ -z "$MISSING_FROM_PUBLISH" ] && [ -z "$EXTRA_IN_PUBLISH" ]; then
    echo "✅ All workspace crates are properly configured in publish workflow!"
    exit 0
else
    echo "❌ Workflow configuration issues found!"
    echo ""

    if [ -n "$MISSING_FROM_PUBLISH" ]; then
        echo "⚠️  Crates in workspace but NOT in publish.yml:"
        for crate in $MISSING_FROM_PUBLISH; do
            echo "   - $crate"
        done
        echo ""
        echo "Add these to the CRATES array in .github/workflows/publish.yml"
        echo ""
    fi

    if [ -n "$EXTRA_IN_PUBLISH" ]; then
        echo "⚠️  Crates in publish.yml but NOT in workspace:"
        for crate in $EXTRA_IN_PUBLISH; do
            echo "   - $crate"
        done
        echo ""
        echo "Remove these from .github/workflows/publish.yml or add to workspace"
        echo ""
    fi

    exit 1
fi
