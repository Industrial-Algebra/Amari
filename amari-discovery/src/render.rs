// SPDX-License-Identifier: MIT OR Apache-2.0

//! Human and machine renderers for typed discovery responses.

use std::io::Write;

use serde::Serialize;

use crate::{
    commands::{
        discover::{ExampleResult, GraphResult, SearchResults},
        probe::{ProbeDescription, ProbeDryRun, ProbeList, ProbeRunResult},
    },
    CandidatePlan, Capabilities, CapabilityRecord, DiscoveryOutcome, DiscoveryResult, Envelope,
    NdjsonWriter, PlanStep, ProbeBackend, ProbeIsolation, ProjectKind, ProjectSnapshot,
    ProtocolSchema, Recommendation, SchemaCatalog,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OutputMode {
    Human,
    Json,
    Ndjson,
}

pub(crate) fn write_json<T: Serialize>(
    writer: &mut impl Write,
    envelope: &Envelope<T>,
) -> DiscoveryResult<()> {
    serde_json::to_writer(&mut *writer, envelope)?;
    writeln!(writer)?;
    Ok(())
}

pub(crate) fn write_envelope<T, W, F>(
    writer: &mut W,
    envelope: &Envelope<T>,
    mode: OutputMode,
    human: F,
) -> DiscoveryResult<()>
where
    T: Serialize,
    W: Write,
    F: FnOnce(&mut W, &Envelope<T>) -> DiscoveryResult<()>,
{
    match mode {
        OutputMode::Human => human(writer, envelope),
        OutputMode::Json => write_json(writer, envelope),
        OutputMode::Ndjson => NdjsonWriter::new(writer)?.write(envelope),
    }
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
// Schemas
// ---------------------------------------------------------------------------

pub(crate) fn write_schema_catalog_human(
    writer: &mut impl Write,
    envelope: &Envelope<SchemaCatalog>,
) -> DiscoveryResult<()> {
    writeln!(writer, "Available schemas:")?;
    for schema in &envelope.data.schemas {
        writeln!(writer, "  {}: {}", schema.kind, schema.id)?;
    }
    Ok(())
}

pub(crate) fn write_schema_human(
    writer: &mut impl Write,
    envelope: &Envelope<ProtocolSchema>,
) -> DiscoveryResult<()> {
    writeln!(writer, "Schema: {}", envelope.data.kind)?;
    writeln!(writer, "ID: {}", envelope.data.id)?;
    writeln!(writer, "Protocol: {}", envelope.data.protocol_version)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Probes
// ---------------------------------------------------------------------------

pub(crate) fn write_probe_list_human(
    writer: &mut impl Write,
    envelope: &Envelope<ProbeList>,
) -> DiscoveryResult<()> {
    writeln!(writer, "Registered probes:")?;
    for probe in &envelope.data.probes {
        let state = if probe.executable {
            "executable"
        } else if probe.available {
            "available, not executable"
        } else {
            "unavailable"
        };
        writeln!(writer, "  {}: {state}", probe.id)?;
    }
    Ok(())
}

pub(crate) fn write_probe_description_human(
    writer: &mut impl Write,
    envelope: &Envelope<ProbeDescription>,
) -> DiscoveryResult<()> {
    let description = &envelope.data;
    writeln!(writer, "Probe: {}", description.descriptor.id)?;
    writeln!(
        writer,
        "Capability: {}",
        description.descriptor.capability_id
    )?;
    writeln!(
        writer,
        "Input schema: {}",
        description.descriptor.input_schema
    )?;
    writeln!(
        writer,
        "Output schema: {}",
        description.descriptor.output_schema
    )?;
    writeln!(writer, "Executable: {}", yes_no(description.executable))?;
    writeln!(writer, "Process isolation: yes")?;
    writeln!(writer, "Hard timeout: {}", yes_no(description.hard_timeout))?;
    writeln!(
        writer,
        "Crash isolation: {}",
        yes_no(description.crash_isolation)
    )?;
    Ok(())
}

pub(crate) fn write_probe_dry_run_human(
    writer: &mut impl Write,
    envelope: &Envelope<ProbeDryRun>,
) -> DiscoveryResult<()> {
    let dry_run = &envelope.data;
    writeln!(writer, "Probe dry-run: {}", dry_run.probe_id)?;
    writeln!(writer, "Compatible: {}", yes_no(dry_run.compatible))?;
    writeln!(writer, "Would execute: no")?;
    writeln!(writer, "Plan hash: {}", dry_run.plan_hash)?;
    writeln!(writer, "Planned isolation: process")?;
    Ok(())
}

pub(crate) fn write_probe_run_human(
    writer: &mut impl Write,
    envelope: &Envelope<ProbeRunResult>,
) -> DiscoveryResult<()> {
    let run = &envelope.data;
    let result = &run.result;
    writeln!(writer, "Probe: {}", result.probe_id)?;
    writeln!(writer, "Input schema: {}", run.input_schema)?;
    writeln!(writer, "Output schema: {}", run.output_schema)?;
    writeln!(writer, "Backend: {}", backend_name(result.backend))?;
    writeln!(writer, "Isolation: {}", isolation_name(run.isolation))?;
    writeln!(writer, "Deterministic: {}", yes_no(run.deterministic))?;
    writeln!(writer, "Hard timeout: {}", yes_no(run.hard_timeout))?;
    writeln!(writer, "Crash isolation: {}", yes_no(run.crash_isolation))?;
    writeln!(writer, "Timeout (millis): {}", run.timeout_millis)?;
    writeln!(writer, "Duration (micros): {}", result.duration_micros)?;
    writeln!(writer, "Resources:")?;
    writeln!(writer, "  operations: {}", result.resources.operations)?;
    writeln!(writer, "  nodes: {}", result.resources.nodes)?;
    writeln!(writer, "  iterations: {}", result.resources.iterations)?;
    writeln!(writer, "  bytes: {}", result.resources.bytes)?;
    writeln!(writer, "Tool version: {}", envelope.provenance.tool_version)?;
    writeln!(
        writer,
        "Catalog version: {}",
        envelope.provenance.catalog.version
    )?;
    writeln!(writer, "Catalog hash: {}", result.catalog_hash)?;
    writeln!(
        writer,
        "Project hash: {}",
        result.project_hash.as_deref().unwrap_or("none")
    )?;
    writeln!(writer, "Input hash: {}", result.input_hash)?;
    match result.seed {
        Some(seed) => writeln!(writer, "Seed: {seed}")?,
        None => writeln!(writer, "Seed: none")?,
    }
    write_string_items(
        writer,
        "Validated assumptions",
        &result.validated_assumptions,
    )?;
    write_string_items(writer, "Refuted assumptions", &result.refuted_assumptions)?;
    write_string_items(writer, "Warnings", &result.warnings)?;
    writeln!(writer, "Result: {}", serde_json::to_string(&result.output)?)?;
    Ok(())
}

fn write_string_items(
    writer: &mut impl Write,
    label: &str,
    items: &[String],
) -> DiscoveryResult<()> {
    writeln!(writer, "{label}:")?;
    if items.is_empty() {
        writeln!(writer, "  none")?;
    } else {
        for item in items {
            writeln!(writer, "  {item}")?;
        }
    }
    Ok(())
}

const fn backend_name(backend: ProbeBackend) -> &'static str {
    match backend {
        ProbeBackend::Cpu => "cpu",
    }
}

fn isolation_name(isolation: ProbeIsolation) -> &'static str {
    match isolation {
        ProbeIsolation::Cooperative => "cooperative",
        ProbeIsolation::Process => "process",
    }
}

const fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
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
// Plan replay
// ---------------------------------------------------------------------------

pub(crate) fn write_plan_human(
    writer: &mut impl Write,
    envelope: &Envelope<CandidatePlan>,
) -> DiscoveryResult<()> {
    let plan = &envelope.data;
    writeln!(writer, "Plan for: {}", plan.capability_id)?;
    writeln!(writer, "Plan hash: {}", plan.plan_hash)?;
    writeln!(writer, "Steps:")?;
    for step in &plan.steps {
        match step {
            PlanStep::Dependency {
                package, version, ..
            } => writeln!(writer, "  dependency: {package} = {version}")?,
            PlanStep::Feature {
                package, feature, ..
            } => writeln!(writer, "  feature: {package}/{feature}")?,
            PlanStep::Symbol { path, .. } => writeln!(writer, "  symbol: {path}")?,
            PlanStep::Example {
                package, example, ..
            } => writeln!(writer, "  example: {package}/{example}")?,
            PlanStep::Probe { probe_id, .. } => writeln!(writer, "  probe: {probe_id}")?,
            PlanStep::Test {
                package, target, ..
            } => {
                let target = match target {
                    crate::PlanTestTarget::AllTargets => "all targets",
                    crate::PlanTestTarget::NpmPackage => "npm package tests",
                };
                writeln!(writer, "  test: {package} ({target})")?;
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
