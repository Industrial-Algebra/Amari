// SPDX-License-Identifier: MIT OR Apache-2.0

//! Human and machine renderers for typed discovery responses.

use std::io::Write;

use serde::Serialize;

use crate::{
    commands::discover::{ExampleResult, GraphResult, SearchResults},
    Capabilities, CapabilityRecord, DiscoveryOutcome, DiscoveryResult, Envelope, PlanStep,
    ProjectKind, ProjectSnapshot, Recommendation,
};

pub(crate) fn write_json<T: Serialize>(
    writer: &mut impl Write,
    envelope: &Envelope<T>,
) -> DiscoveryResult<()> {
    serde_json::to_writer(&mut *writer, envelope)?;
    writeln!(writer)?;
    Ok(())
}

pub(crate) fn write_capabilities_human(
    writer: &mut impl Write,
    envelope: &Envelope<Capabilities>,
) -> DiscoveryResult<()> {
    let capabilities = &envelope.data;
    let catalog_availability = if capabilities.catalog.available {
        "available"
    } else {
        "unavailable"
    };

    writeln!(writer, "Amari Discovery {}", capabilities.tool_version)?;
    writeln!(
        writer,
        "Protocol: {}",
        capabilities.protocol_versions.join(", ")
    )?;
    writeln!(
        writer,
        "Catalog: {} ({catalog_availability})",
        capabilities.catalog.version
    )?;
    writeln!(writer, "Project inspectors:")?;
    for inspector in &capabilities.project_inspectors {
        let state = if inspector.executable {
            "executable"
        } else if inspector.available {
            "available"
        } else if inspector.known {
            "known, unavailable"
        } else {
            "unknown"
        };
        writeln!(writer, "  {}: {state}", inspector.id)?;
    }
    writeln!(writer, "Output: {}", capabilities.output_modes.join(", "))?;
    writeln!(
        writer,
        "AI adapter: {}",
        if capabilities.ai_adapter.executable {
            "executable"
        } else if capabilities.ai_adapter.contract_compiled {
            "contract only"
        } else {
            "not compiled"
        }
    )?;
    writeln!(writer, "Resource limits:")?;
    let rl = &capabilities.resource_limits;
    writeln!(
        writer,
        "  max_inspection_files: {}",
        rl.max_inspection_files
    )?;
    writeln!(
        writer,
        "  max_inspection_bytes: {}",
        rl.max_inspection_bytes
    )?;
    writeln!(writer, "  max_traversal_depth: {}", rl.max_traversal_depth)?;
    writeln!(writer, "  max_per_file_bytes: {}", rl.max_per_file_bytes)?;
    writeln!(
        writer,
        "  max_inspection_wall_millis: {}",
        rl.max_inspection_wall_millis
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Project inspection
// ---------------------------------------------------------------------------

pub(crate) fn write_inspection_human(
    writer: &mut impl Write,
    envelope: &Envelope<ProjectSnapshot>,
) -> DiscoveryResult<()> {
    let snapshot = &envelope.data;
    let kind = match snapshot.project_kind {
        ProjectKind::RustCargo => "Rust/Cargo project",
        ProjectKind::NpmTypeScript => "npm/TypeScript project",
        ProjectKind::Mixed => "mixed Rust/Cargo and npm/TypeScript project",
        ProjectKind::Unknown => "unknown project",
    };
    writeln!(writer, "{kind}")?;
    writeln!(writer, "  Project hash: {}", snapshot.project_hash)?;
    writeln!(writer, "  Files: {}", snapshot.file_count)?;
    writeln!(writer, "  Bytes: {}", snapshot.total_bytes)?;
    writeln!(
        writer,
        "  Compatibility: {}",
        envelope.provenance.compatibility.status
    )?;

    if let Some(cargo) = &snapshot.cargo {
        let packages = std::iter::once(&cargo.root_package).chain(cargo.workspace_members.iter());
        let dependency_count: usize = packages.map(|package| package.dependencies.len()).sum();
        writeln!(writer, "  Amari dependencies: {dependency_count}")?;
    }
    if let Some(rust) = &snapshot.rust {
        writeln!(writer, "  API usages: {}", rust.usages.len())?;
        writeln!(writer, "  Domain vocabulary: {}", rust.vocabulary.len())?;
    }
    if let Some(platform) = &snapshot.platform {
        writeln!(writer, "  Benchmarks: {}", platform.benchmarks.len())?;
        writeln!(writer, "  WASM targets: {}", platform.wasm_targets.len())?;
        writeln!(
            writer,
            "  Native requirements: {}",
            platform.native_requirements.len()
        )?;
        writeln!(
            writer,
            "  no_std packages: {}",
            platform.no_std_evidence.packages.len()
        )?;
    }
    if let Some(npm) = &snapshot.npm {
        writeln!(
            writer,
            "  Amari npm dependencies: {}",
            npm.package.dependencies.len()
        )?;
    }
    if let Some(typescript) = &snapshot.typescript {
        writeln!(writer, "  JS/TS imports: {}", typescript.imports.len())?;
        writeln!(
            writer,
            "  Declaration exports: {}",
            typescript.declaration_exports.len()
        )?;
        writeln!(
            writer,
            "  WASM capabilities: {}",
            typescript.capabilities.len()
        )?;
        writeln!(
            writer,
            "  Runtime signals: {}",
            typescript.runtime_signals.len()
        )?;
        writeln!(
            writer,
            "  Domain vocabulary: {}",
            typescript.vocabulary.len()
        )?;
    }
    writeln!(writer, "  Warnings: {}", envelope.warnings.len())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Recommend
// ---------------------------------------------------------------------------

pub(crate) fn write_recommendation_human(
    writer: &mut impl Write,
    envelope: &Envelope<DiscoveryOutcome<Recommendation>>,
) -> DiscoveryResult<()> {
    match &envelope.data {
        DiscoveryOutcome::Recommended(recommendation) => {
            writeln!(
                writer,
                "Recommendation for: {}",
                recommendation.goal.statement
            )?;
            writeln!(
                writer,
                "Preferred: {}",
                recommendation.preferred.capability_id
            )?;
            writeln!(writer, "Plan hash: {}", recommendation.preferred.plan_hash)?;
            if let Some(score) = recommendation
                .scores
                .iter()
                .find(|score| score.capability_id == recommendation.preferred.capability_id)
            {
                writeln!(writer, "Confidence: {:.12}", score.confidence)?;
                writeln!(writer, "Scores:")?;
                writeln!(
                    writer,
                    "  applicability: {:.12}",
                    score.components.applicability
                )?;
                writeln!(writer, "  evidence: {:.12}", score.components.evidence)?;
                writeln!(writer, "  effort: {:.12}", score.components.effort)?;
                writeln!(writer, "  maturity: {:.12}", score.components.maturity)?;
                writeln!(writer, "  runtime: {:.12}", score.components.runtime)?;
                writeln!(writer, "  platform: {:.12}", score.components.platform)?;
                writeln!(
                    writer,
                    "  verification: {:.12}",
                    score.components.verification
                )?;
                writeln!(writer, "  risk: {:.12}", score.components.risk)?;
            }
            writeln!(writer, "Evidence:")?;
            for evidence in &recommendation.evidence {
                writeln!(writer, "  {}", evidence.summary)?;
            }
            writeln!(writer, "Missing information:")?;
            for missing in &recommendation.missing_information {
                writeln!(writer, "  {missing}")?;
            }
            writeln!(writer, "Suggested probes:")?;
            for probe_id in &recommendation.suggested_probes {
                writeln!(writer, "  {probe_id}")?;
            }
            writeln!(writer, "Suggested tests:")?;
            for step in &recommendation.suggested_tests {
                if let PlanStep::Test {
                    package, target, ..
                } = step
                {
                    let target = match target {
                        crate::PlanTestTarget::AllTargets => "all targets",
                        crate::PlanTestTarget::NpmPackage => "npm package tests",
                    };
                    writeln!(writer, "  {package}: {target}")?;
                }
            }
            if !recommendation.alternatives.is_empty() {
                writeln!(writer, "Alternatives:")?;
                for alternative in &recommendation.alternatives {
                    writeln!(writer, "  {}", alternative.capability_id)?;
                }
            }
        }
        DiscoveryOutcome::NoApplicableCapability { evidence } => {
            writeln!(writer, "No applicable Amari capability.")?;
            for item in evidence {
                writeln!(writer, "  {}", item.summary)?;
            }
        }
        DiscoveryOutcome::InsufficientEvidence { missing } => {
            writeln!(writer, "Insufficient evidence for a recommendation.")?;
            for item in missing {
                writeln!(writer, "  {item}")?;
            }
        }
        DiscoveryOutcome::Blocked { reasons } => {
            writeln!(writer, "Recommendation blocked.")?;
            for reason in reasons {
                writeln!(writer, "  {reason}")?;
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Discover: search
// ---------------------------------------------------------------------------

pub(crate) fn write_search_human(
    writer: &mut impl Write,
    envelope: &Envelope<SearchResults>,
) -> DiscoveryResult<()> {
    let results = &envelope.data;
    if results.results.is_empty() {
        writeln!(writer, "No capabilities matched '{}'.", results.query)?;
        return Ok(());
    }
    writeln!(writer, "Results for '{}':", results.query)?;
    for result in &results.results {
        writeln!(writer, "  {}  {}", result.id, result.name)?;
        writeln!(writer, "    {}", result.description)?;
        if !result.aliases.is_empty() {
            writeln!(writer, "    aliases: {}", result.aliases.join(", "))?;
        }
        if !result.concepts.is_empty() {
            writeln!(writer, "    concepts: {}", result.concepts.join(", "))?;
        }
        writeln!(
            writer,
            "    stability: {:?}  cost: {:?}",
            result.stability, result.cost
        )?;
    }
    writeln!(writer, "{} result(s)", results.results.len())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Discover: detail
// ---------------------------------------------------------------------------

pub(crate) fn write_detail_human(
    writer: &mut impl Write,
    envelope: &Envelope<CapabilityRecord>,
) -> DiscoveryResult<()> {
    let cap = &envelope.data;
    writeln!(writer, "{}", cap.name)?;
    writeln!(writer, "  ID: {}", cap.id)?;
    writeln!(writer, "  {}", cap.description)?;
    if !cap.aliases.is_empty() {
        writeln!(writer, "  Aliases: {}", cap.aliases.join(", "))?;
    }
    if !cap.concepts.is_empty() {
        writeln!(writer, "  Concepts: {}", cap.concepts.join(", "))?;
    }
    writeln!(writer, "  Stability: {:?}", cap.stability)?;
    writeln!(writer, "  Cost: {:?}", cap.cost)?;
    if !cap.feature_refs.is_empty() {
        writeln!(writer, "  Features: {}", cap.feature_refs.join(", "))?;
    }
    if !cap.crate_refs.is_empty() {
        writeln!(writer, "  Crates: {}", cap.crate_refs.join(", "))?;
    }
    if !cap.symbol_refs.is_empty() {
        writeln!(writer, "  Symbols:")?;
        for symbol in &cap.symbol_refs {
            writeln!(writer, "    {symbol}")?;
        }
    }
    if !cap.example_refs.is_empty() {
        writeln!(writer, "  Examples: {}", cap.example_refs.join(", "))?;
    }
    if !cap.probe_refs.is_empty() {
        writeln!(writer, "  Probes:")?;
        for probe in &cap.probe_refs {
            writeln!(writer, "    {probe}")?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Discover: graph
// ---------------------------------------------------------------------------

pub(crate) fn write_graph_human(
    writer: &mut impl Write,
    envelope: &Envelope<GraphResult>,
) -> DiscoveryResult<()> {
    let graph = &envelope.data;
    writeln!(writer, "{}  {}", graph.capability_id, graph.capability_name)?;
    if graph.relations.is_empty() {
        writeln!(writer, "  No relationships.")?;
    } else {
        writeln!(writer, "  Relationships:")?;
        for rel in &graph.relations {
            let direction = if rel.to == graph.capability_id {
                "inbound"
            } else {
                "outbound"
            };
            writeln!(
                writer,
                "    {} --{}--> {}  [{direction}]",
                rel.from, rel.kind, rel.to
            )?;
        }
    }
    writeln!(writer, "{} relation(s)", graph.relations.len())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Discover: example
// ---------------------------------------------------------------------------

pub(crate) fn write_example_human(
    writer: &mut impl Write,
    envelope: &Envelope<ExampleResult>,
) -> DiscoveryResult<()> {
    let ex = &envelope.data;
    writeln!(writer, "{}  {}", ex.capability_id, ex.capability_name)?;
    writeln!(writer, "  Examples:")?;
    for example in &ex.examples {
        writeln!(
            writer,
            "    {}:{}  {}",
            example.crate_name, example.example_name, example.path
        )?;
    }
    writeln!(writer, "{} example(s)", ex.examples.len())?;
    Ok(())
}
