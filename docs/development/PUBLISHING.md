# Publishing Guide

This document describes how to publish the Amari mathematical computing library to both npm and crates.io.

## Prerequisites

### GitHub Secrets
The following secrets must be configured in the GitHub repository:

- `CARGO_REGISTRY_TOKEN` - crates.io API token for publishing Rust crates

npm publishing uses **trusted publishing (OIDC)** — no npm token secret is
required once the package is linked on npmjs.com (see below). The legacy
`NPM_TOKEN` secret (classic token) is unused; npm revoked classic tokens in
the granular-token transition.

### npm Trusted Publishing (one-time setup)

On npmjs.com, for `@justinelliottcobb/amari-wasm`: **Settings → Trusted
Publishers → GitHub Actions** with:

- Repository owner: `Industrial-Algebra`
- Repository: `Amari`
- Workflow filename: `publish.yml` (exact, case-sensitive, include `.yml`)
- Environment: leave blank

npm does not validate the configuration when saving, so double-check each
field. Trusted publishing requires GitHub-hosted runners, Node >= 22.14.0,
and npm CLI >= 11.5.1; provenance attestations are generated automatically.

Fallback if OIDC is unavailable: create a **granular access token**
(read+write on the package, 2FA-bypass enabled) and store it as the
`NPM_TOKEN` secret. Granular tokens expire after at most 90 days and must be
rotated.

### Local Setup
For manual publishing:
- Rust toolchain with `wasm32-unknown-unknown` target
- `wasm-pack` installed
- Node.js 20+ with npm

## Automated Publishing (Recommended)

### Release via Script
```bash
# Publish version 0.1.0 to both npm and crates.io
./release.sh 0.1.0

# Publish only to npm
./release.sh 0.1.1 true false

# Publish only to crates.io
./release.sh 0.1.2 false true
```

The script will:
1. ✅ Update all version numbers
2. ✅ Run tests and build checks
3. ✅ Commit and tag the release
4. ✅ Push to GitHub (triggers publishing workflow)

### Manual Workflow Dispatch
You can also trigger publishing via GitHub Actions:

1. Go to **Actions** → **Publish to npm and crates.io**
2. Click **Run workflow**
3. Enter version and publishing options
4. Click **Run workflow**

## Publishing Process

### 1. Crates.io Publishing
The workflow publishes crates in dependency order:
1. `amari-core` - Core geometric algebra
2. `amari-tropical` - Tropical algebra
3. `amari-dual` - Dual number AD
4. `amari-info-geom` - Information geometry
5. `amari-fusion` - Fusion system
6. `amari-automata` - Cellular automata
7. `amari-gpu` - GPU acceleration
8. `amari` - Main umbrella crate

### 2. npm Publishing
1. Builds WASM packages for multiple targets:
   - Web (`pkg/`)
   - Node.js (`pkg-node/`)
   - Bundler (`pkg-bundler/`)
2. Publishes `@amari/core` to npm

### 3. Examples Update
After successful npm publishing:
1. Creates new branch `update-examples-with-published-library`
2. Updates examples suite to use published `@amari/core`
3. Creates PR to integrate real functionality

## Package Information

### npm Package
- **Name**: `@amari/core`
- **Scope**: `@amari`
- **Access**: Public
- **Targets**: Web, Node.js, Bundler

### Crates.io Packages
- `amari` - Main library
- `amari-core` - Core mathematical structures
- `amari-tropical` - Tropical algebra
- `amari-dual` - Automatic differentiation
- `amari-info-geom` - Information geometry
- `amari-fusion` - Fusion system
- `amari-automata` - Cellular automata
- `amari-gpu` - GPU acceleration
- `amari-wasm` - WebAssembly bindings

## Manual Publishing

If you need to publish manually:

### Crates.io
```bash
# Publish individual crate
cd amari-core
cargo publish

# Or use the dependency order script
./scripts/publish-crates.sh
```

### npm
```bash
cd amari-wasm
npm run build  # Build WASM
npm publish    # Publish to npm
```

## Troubleshooting

### Common Issues

**Crates.io publish fails**
- Check that all dependencies are published first
- Verify `Cargo.toml` has required metadata
- Ensure version numbers are updated

**npm publish fails**
- Verify the trusted publisher configuration on npmjs.com matches the
  repository and `publish.yml` exactly (all fields case-sensitive)
- Check that the workflow job has `id-token: write` permission and runs on a
  GitHub-hosted runner (trusted publishing does not support self-hosted)
- Check that WASM build completed successfully
- Ensure package version is incremented

**Examples update fails**
- Check that `@amari/core` package is available on npm
- Verify GitHub token has repository write permissions

### Version Management
- All Rust crates use workspace versioning
- WASM package version is managed separately in `package.json`
- Release script keeps everything in sync

## Next Steps

After publishing:
1. 📦 Packages are available on npm and crates.io
2. 🔄 Examples suite PR is created automatically
3. ✅ Merge the examples PR to complete the integration
4. 🚀 Examples now demonstrate real Amari functionality!

## Monitoring

- **GitHub Actions**: Monitor publishing progress
- **npm**: Check package at https://npmjs.com/package/@amari/core
- **crates.io**: Check crates at https://crates.io/search?q=amari