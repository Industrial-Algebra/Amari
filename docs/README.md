# Amari Documentation

This directory contains comprehensive documentation for the Amari mathematical computing library.

## Documentation Structure

### 📋 Quick Links

- **[Main README](../README.md)** - Project overview and quick start
- **[Changelog](../CHANGELOG.md)** - Version history and changes
- **[Version Management](../VERSION_MANAGEMENT.md)** - Version synchronization procedures

### 📦 Releases

Current release process documentation:

- **[v0.9.8 Release Process](releases/RELEASE_PROCESS_v0.9.8.md)** - Current release procedures and lessons learned

### 🗺️ Roadmaps

Current planning and release-roadmap documentation:

- **[v0.21.0 Tropical / Dual Roadmap](roadmap/v0.21.0-tropical-dual-roadmap.md)** - Planning for the optimization-oriented `amari-tropical` / `amari-dual` cycle
- **[v0.21.0 Tropical / Dual Checklist](roadmap/v0.21.0-tropical-dual-checklist.md)** - Milestone tracker for the `0.21.0` extension cycle
- **[Tropical / Dual Optimization Design](roadmap/amari-tropical-dual-optimization_design.md)** - Design notes for compiler/interpreter optimization use cases
- **[Tropical Ordinal Substrate Design](roadmap/amari-tropical-ordinal-substrate_design.md)** - Design notes for the ordinal semiring substrate in `amari-tropical`
- **[V1 Readiness Plan](roadmap/V1_READINESS_PLAN.md)** - Existing broader readiness tracker

### 🏗️ Architecture

Technical architecture and design documentation:

- **[Multi-GPU Design](architecture/v0.9.6-multi-gpu-design.md)** - Multi-GPU architecture details

### 🔧 Development

Development guides and procedures:

- **[CI Setup](development/CI_SETUP.md)** - Continuous integration configuration
- **[Publishing Guide](development/PUBLISHING.md)** - Publishing to npm and crates.io

### 🧠 Claude Code

AI-assisted development context:

- **[Consolidated Context](claude-code/CONSOLIDATED_CONTEXT.md)** - Current project context for AI sessions

### 📚 Technical

Core methodology and design documentation:

- **[API Naming Convention](technical/API_NAMING_CONVENTION.md)** - API standardization guidelines
- **[Error Handling Design](technical/ERROR_HANDLING_DESIGN.md)** - Unified error handling approach
- **[Formal Verification](technical/FORMAL_VERIFICATION.md)** - Verification strategy and roadmap
- **[Mathematical Rigor](technical/MATHEMATICAL_RIGOR.md)** - Mathematical verification methodology
- **[Phantom Types Methodology](technical/PHANTOM_TYPES_METHODOLOGY.md)** - Step-by-step phantom types implementation
- **[Phantom Types Quick Reference](technical/PHANTOM_TYPES_QUICK_REFERENCE.md)** - Quick reference guide
- **[Test Enforcement](technical/TEST_ENFORCEMENT.md)** - Test coverage and enforcement standards

### 📚 Examples

Example code and learning resources:

- **[Examples README](../examples/README.md)** - Example code overview
- **[Learning Paths](../examples/LEARNING_PATHS.md)** - Structured learning paths
- **[Examples Documentation](../examples/DOCUMENTATION.md)** - Detailed example documentation

### 📦 Archive

Historical and outdated documentation (kept for reference):

- **[Deployment Strategy](archive/DEPLOYMENT_STRATEGY.md)** - Historical deployment planning (v0.3.x)
- **[DUAL GPU Integration Plan](archive/DUAL_GPU_INTEGRATION_PLAN.md)** - Historical planning document
- **[GPU Integration Plan v0.9.4](archive/GPU_INTEGRATION_PLAN_v0.9.4.md)** - Historical planning document
- **[GPU Optimization Achievement v0.9.5](archive/GPU_OPTIMIZATION_ACHIEVEMENT_v0.9.5.md)** - v0.9.5 milestone
- **[Integration Status](archive/INTEGRATION_STATUS.md)** - Historical integration matrix (v0.9.2)
- **[Phase 4 Planning](archive/PHASE4_PLANNING.md)** - Historical Phase 4 planning
- **[Phase 4A Analysis](archive/PHASE4A_ANALYSIS.md)** - Historical Phase 4A verification analysis
- **[Phase 4C v0.8.0 Planning](archive/phase4c_v0.8.0_planning.md)** - Historical v0.8.0 WASM planning
- **[Release Process](archive/RELEASE_PROCESS.md)** - Historical release process (pre-v0.9.8)
- **[Roadmap v0.9.5](archive/ROADMAP_v0.9.5.md)** - Historical roadmap
- **[Tropical GPU Integration Plan](archive/TROPICAL_GPU_INTEGRATION_PLAN.md)** - Historical planning document
- **[v0.9.5 Release Summary](archive/v0.9.5_RELEASE_SUMMARY.md)** - Historical release summary
- **[v0.9.6 Release Summary](archive/v0.9.6_RELEASE_SUMMARY.md)** - Historical release summary
- **[V0.9.0 WASM Precision Strategy](archive/V0_9_0_WASM_PRECISION_STRATEGY.md)** - Historical WASM strategy
- **[WASM/GPU Implementation Status](archive/WASM_GPU_IMPLEMENTATION_STATUS.md)** - Historical status document

## Getting Started

1. **New Users**: Start with the [Main README](../README.md)
2. **Contributors**: Check [Multi-GPU Design](architecture/v0.9.6-multi-gpu-design.md) and [Technical](technical/) documentation
3. **Developers**: Review the [v0.9.8 Release Process](releases/RELEASE_PROCESS_v0.9.8.md) before making releases
4. **Learners**: Follow the [Learning Paths](../examples/LEARNING_PATHS.md) for structured tutorials
5. **AI Sessions**: Reference [Consolidated Context](claude-code/CONSOLIDATED_CONTEXT.md) for current project state

## Crate-Specific Documentation

Individual crates have their own documentation:

- **[amari-network README](../amari-network/README.md)** - Geometric network analysis
- **[amari-network Mathematical Foundations](../amari-network/MATHEMATICAL_FOUNDATIONS.md)** - Mathematical background
- **[amari-optimization README](../amari-optimization/README.md)** - Optimization algorithms
- **[amari-wasm README](../amari-wasm/README.md)** - WebAssembly bindings
- **[amari-automata CONTEXT](../amari-automata/CONTEXT.md)** - Cellular automata context

## Contributing to Documentation

When adding new documentation:

1. **Place it appropriately**:
   - Release docs → `docs/releases/`
   - Architecture docs → `docs/architecture/`
   - Development guides → `docs/development/`
   - Outdated docs → `docs/archive/`

2. **Update this index** to include the new document

3. **Follow naming conventions**:
   - Use lowercase with hyphens: `multi-gpu-design.md`
   - Include version numbers where relevant: `v0.9.6-multi-gpu-design.md`
   - Use descriptive names: `RELEASE_PROCESS.md` not `process.md`

## Documentation Maintenance

- **Review quarterly** to identify outdated content
- **Archive** completed plans and historical documents
- **Update** integration status after major releases
- **Keep** this index current with all documentation files
