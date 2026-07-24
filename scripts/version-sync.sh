#!/bin/bash

# Version Synchronization Script for Amari
# This script ensures all packages (Rust workspace + WASM package) stay synchronized

set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Version authorities and active JavaScript package metadata.
CARGO_TOML="Cargo.toml"
PACKAGE_JSON="amari-wasm/package.json"
NPM_PACKAGE_FILES=(
    "amari-wasm/package.json"
    "amari-wasm/examples/package.json"
    "typescript/package.json"
    "examples/typescript/package.json"
    "examples/web/interactive-demos/package.json"
    "examples-suite/package.json"
    "examples-suite/package-lock.json"
)
CATALOG_FILES=(
    "amari-discovery/catalog/probes.toml"
    "amari-discovery/catalog/semantic/core.toml"
)

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Function to validate semantic version format
validate_version() {
    local version=$1
    if [[ ! $version =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9-]+)?(\+[a-zA-Z0-9-]+)?$ ]]; then
        print_error "Invalid version format: $version"
        print_error "Expected format: MAJOR.MINOR.PATCH (e.g., 1.2.3, 1.2.3-alpha, 1.2.3+build)"
        return 1
    fi
    return 0
}

# Function to get current workspace version from Cargo.toml
get_current_version() {
    if [[ ! -f "$CARGO_TOML" ]]; then
        print_error "Cargo.toml not found in current directory"
        return 1
    fi

    grep -E "^version = " "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/'
}

# Function to get current package.json version
get_package_json_version() {
    if [[ ! -f "$PACKAGE_JSON" ]]; then
        print_error "package.json not found at $PACKAGE_JSON"
        return 1
    fi

    # Look for the main version field (not in scripts section)
    grep -E '^\s*"version":' "$PACKAGE_JSON" | head -1 | sed 's/.*"version": "\(.*\)".*/\1/'
}

# Function to update workspace version in Cargo.toml
update_cargo_version() {
    local new_version=$1

    print_status "Updating Cargo.toml workspace version to $new_version..."

    # Update workspace.package version
    sed -i.bak "s/^version = \".*\"/version = \"$new_version\"/" "$CARGO_TOML"

    # Update all workspace.dependencies versions
    sed -i.bak "s/version = \"[0-9]\+\.[0-9]\+\.[0-9]\+\"/version = \"$new_version\"/g" "$CARGO_TOML"

    # Remove backup file
    rm -f "$CARGO_TOML.bak"

    print_success "Updated Cargo.toml versions"
}

# Update every path dependency version in every workspace manifest.
update_path_dependency_versions() {
    local new_version=$1

    while IFS= read -r -d '' manifest; do
        sed -E -i.bak "/^[[:space:]]*amari(-[[:alnum:]-]+)?[[:space:]]*=.*path[[:space:]]*=/s/version = \"[0-9]+\.[0-9]+\.[0-9]+\"/version = \"$new_version\"/g" "$manifest"
        rm -f "$manifest.bak"
    done < <(find . -name Cargo.toml -not -path './target/*' -not -path './.worktrees/*' -print0)
}

# Update active JavaScript package versions without touching external packages.
update_npm_versions() {
    local new_version=$1

    print_status "Updating JavaScript package metadata to $new_version..."
    python3 - "$new_version" "${NPM_PACKAGE_FILES[@]}" <<'PY'
import json
import pathlib
import re
import sys

new_version = sys.argv[1]
for raw_path in sys.argv[2:]:
    path = pathlib.Path(raw_path)
    if not path.is_file():
        continue
    data = json.loads(path.read_text())
    if "version" in data:
        data["version"] = new_version
    root_package = data.get("packages", {}).get("")
    if isinstance(root_package, dict) and "version" in root_package:
        root_package["version"] = new_version
    for section in ("dependencies", "devDependencies", "peerDependencies", "optionalDependencies"):
        dependencies = data.get(section)
        if not isinstance(dependencies, dict):
            continue
        requirement = dependencies.get("@justinelliottcobb/amari-wasm")
        if not isinstance(requirement, str) or requirement == "latest":
            continue
        match = re.fullmatch(r"([~^]?)[0-9]+\.[0-9]+\.[0-9]+", requirement)
        if match:
            dependencies["@justinelliottcobb/amari-wasm"] = f"{match.group(1)}{new_version}"
    path.write_text(json.dumps(data, indent=2) + "\n")
PY
    print_success "Updated JavaScript package metadata"
}

update_catalog_versions() {
    local new_version=$1

    for catalog in "${CATALOG_FILES[@]}"; do
        [[ -f "$catalog" ]] || continue
        sed -E -i.bak "s/^catalog_version = \"[^\"]+\"/catalog_version = \"$new_version\"/" "$catalog"
        rm -f "$catalog.bak"
    done
}

# Function to verify all versions are synchronized
verify_versions() {
    local expected_version=$1

    print_status "Verifying version synchronization..."

    # Check Cargo.toml workspace version
    local cargo_version=$(grep -E "^version = " "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/')
    if [[ "$cargo_version" != "$expected_version" ]]; then
        print_error "Cargo.toml workspace version mismatch: expected $expected_version, found $cargo_version"
        return 1
    fi

    # Check every path dependency in every manifest.
    local stale_manifest_lines
    stale_manifest_lines=$(find . -name Cargo.toml -not -path './target/*' -not -path './.worktrees/*' -print0 \
        | xargs -0 grep -H -E '^[[:space:]]*amari(-[[:alnum:]-]+)?[[:space:]]*=.*path[[:space:]]*=' \
        | grep 'version = ' \
        | grep -v "version = \"$expected_version\"" || true)
    if [[ -n "$stale_manifest_lines" ]]; then
        print_error "Stale path dependency versions found:"
        printf '%s\n' "$stale_manifest_lines" >&2
        return 1
    fi

    # Check active JavaScript package metadata and internal requirements.
    if ! python3 - "$expected_version" "${NPM_PACKAGE_FILES[@]}" <<'PY'
import json
import pathlib
import re
import sys

expected = sys.argv[1]
errors = []
for raw_path in sys.argv[2:]:
    path = pathlib.Path(raw_path)
    if not path.is_file():
        continue
    data = json.loads(path.read_text())
    if "version" in data and data["version"] != expected:
        errors.append(f"{path}: top-level version {data['version']}")
    root_package = data.get("packages", {}).get("")
    if isinstance(root_package, dict) and "version" in root_package and root_package["version"] != expected:
        errors.append(f"{path}: root lock package version {root_package['version']}")
    for section in ("dependencies", "devDependencies", "peerDependencies", "optionalDependencies"):
        dependencies = data.get(section)
        if not isinstance(dependencies, dict):
            continue
        requirement = dependencies.get("@justinelliottcobb/amari-wasm")
        if not isinstance(requirement, str) or requirement == "latest":
            continue
        match = re.fullmatch(r"[~^]?([0-9]+\.[0-9]+\.[0-9]+)", requirement)
        if match and match.group(1) != expected:
            errors.append(f"{path}: {section} amari-wasm requirement {requirement}")
if errors:
    print("\n".join(errors), file=sys.stderr)
    raise SystemExit(1)
PY
    then
        print_error "JavaScript package versions are not synchronized"
        return 1
    fi

    for catalog in "${CATALOG_FILES[@]}"; do
        [[ -f "$catalog" ]] || continue
        if ! grep -q "^catalog_version = \"$expected_version\"$" "$catalog"; then
            print_error "$catalog catalog_version is not $expected_version"
            return 1
        fi
    done

    print_success "All versions synchronized to $expected_version"
    return 0
}

# Function to show current version status
show_status() {
    print_status "Current version status:"

    if [[ -f "$CARGO_TOML" ]]; then
        local cargo_version=$(grep -E "^version = " "$CARGO_TOML" | head -1 | sed 's/version = "\(.*\)"/\1/' || echo "NOT_FOUND")
        echo "  Cargo.toml workspace: $cargo_version"
    else
        echo "  Cargo.toml: NOT_FOUND"
    fi

    if [[ -f "$PACKAGE_JSON" ]]; then
        local package_version=$(get_package_json_version || echo "NOT_FOUND")
        echo "  package.json: $package_version"
    else
        echo "  package.json: NOT_FOUND"
    fi

    # Show workspace dependencies versions
    echo "  Workspace dependencies:"
    grep -E "version = \"[0-9]" "$CARGO_TOML" | head -5 | sed 's/^/    /'
    if [[ $(grep -c "version = \"[0-9]" "$CARGO_TOML") -gt 5 ]]; then
        echo "    ... ($(grep -c "version = \"[0-9]" "$CARGO_TOML") total)"
    fi
}

# Function to bump version automatically
bump_version() {
    local bump_type=$1
    local current_version=$(get_current_version)

    if [[ -z "$current_version" ]]; then
        print_error "Could not determine current version"
        return 1
    fi

    # Parse current version
    IFS='.' read -r major minor patch <<< "$current_version"

    case $bump_type in
        major)
            major=$((major + 1))
            minor=0
            patch=0
            ;;
        minor)
            minor=$((minor + 1))
            patch=0
            ;;
        patch)
            patch=$((patch + 1))
            ;;
        *)
            print_error "Invalid bump type: $bump_type (use: major, minor, patch)"
            return 1
            ;;
    esac

    local new_version="$major.$minor.$patch"
    echo "$new_version"
}

# Function to show usage
show_usage() {
    cat << EOF
Usage: $0 <command> [options]

Commands:
    status                  Show current version status
    set <version>          Set specific version (e.g., 1.0.0)
    bump <major|minor|patch>  Bump version automatically
    verify <version>       Verify all versions match expected version

Examples:
    $0 status              # Show current versions
    $0 set 1.0.0          # Set all versions to 1.0.0
    $0 bump minor         # Bump minor version (1.0.0 -> 1.1.0)
    $0 verify 1.0.0       # Check if all versions are 1.0.0

This script synchronizes versions between:
- Cargo.toml workspace.package.version
- every internal path dependency version in workspace manifests
- active JavaScript package and package-lock metadata
- amari-wasm dependency requirements in examples
- discovery semantic/probe catalog versions
EOF
}

# Main script logic
main() {
    if [[ $# -eq 0 ]]; then
        show_usage
        exit 1
    fi

    local command=$1

    case $command in
        status)
            show_status
            ;;
        set)
            if [[ $# -ne 2 ]]; then
                print_error "Usage: $0 set <version>"
                exit 1
            fi
            local new_version=$2
            if ! validate_version "$new_version"; then
                exit 1
            fi

            print_status "Setting all versions to $new_version"
            update_cargo_version "$new_version"
            update_path_dependency_versions "$new_version"
            update_npm_versions "$new_version"
            update_catalog_versions "$new_version"
            verify_versions "$new_version"
            ;;
        bump)
            if [[ $# -ne 2 ]]; then
                print_error "Usage: $0 bump <major|minor|patch>"
                exit 1
            fi
            local bump_type=$2
            local new_version=$(bump_version "$bump_type")
            if [[ $? -ne 0 ]]; then
                exit 1
            fi

            print_status "Bumping $bump_type version to $new_version"
            update_cargo_version "$new_version"
            update_path_dependency_versions "$new_version"
            update_npm_versions "$new_version"
            update_catalog_versions "$new_version"
            verify_versions "$new_version"
            ;;
        verify)
            if [[ $# -ne 2 ]]; then
                print_error "Usage: $0 verify <version>"
                exit 1
            fi
            local expected_version=$2
            if ! validate_version "$expected_version"; then
                exit 1
            fi
            verify_versions "$expected_version"
            ;;
        *)
            print_error "Unknown command: $command"
            show_usage
            exit 1
            ;;
    esac
}

# Run main function
main "$@"