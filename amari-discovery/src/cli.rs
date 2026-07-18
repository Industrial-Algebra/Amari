// SPDX-License-Identifier: MIT OR Apache-2.0

//! Command-line parsing and dispatch for the `amari` binary.

use std::{io, path::PathBuf};

use clap::{Parser, Subcommand, ValueEnum};

use crate::inspect::{inspect_project_envelope, InspectionLimits};
use crate::{commands, render, Capabilities, Catalog, DiscoveryError, DiscoveryResult, GoalSpec};

#[derive(Debug, Parser)]
#[command(
    name = "amari",
    version,
    about = "Discover and plan integrations with the Amari mathematical ecosystem"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    #[arg(long, global = true, help = "Emit a versioned JSON response")]
    json: bool,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Describe functionality compiled into this binary.
    Capabilities,
    /// Search and inspect the embedded capability catalog.
    Discover {
        #[command(subcommand)]
        command: DiscoverCommand,
    },
    /// Inspect an existing project without modifying it.
    Inspect {
        /// Project path; defaults to the current directory.
        path: Option<PathBuf>,
    },
    /// Recommend Amari capabilities for a project and goal.
    Recommend {
        /// Project path; defaults to the current directory.
        path: Option<PathBuf>,
        /// Inline goal description.
        #[arg(
            long,
            conflicts_with = "goal_file",
            required_unless_present = "goal_file"
        )]
        goal: Option<String>,
        /// Path to a typed goal JSON document.
        #[arg(long, conflicts_with = "goal")]
        goal_file: Option<PathBuf>,
        /// Path to a bounded JSON array of saved probe results.
        #[arg(long, value_name = "FILE")]
        probe_results: Option<PathBuf>,
    },
    /// Normalize a saved recommendation candidate into a replayable plan.
    Plan {
        /// Candidate identifier from the saved recommendation.
        candidate_id: String,
        /// Saved recommendation artifact.
        #[arg(long)]
        recommendation: PathBuf,
        /// Current project path used for replay validation.
        #[arg(long)]
        project: PathBuf,
    },
    /// List, describe, or run registered bounded probes.
    Probe {
        #[command(subcommand)]
        command: ProbeCommand,
    },
    /// Start the human shell over the same typed command handlers.
    Shell {
        /// Default project path for shell commands.
        #[arg(long)]
        project: Option<PathBuf>,
    },
    /// Emit a versioned machine protocol schema.
    Schema {
        /// Schema family; omit to list available schemas.
        #[arg(value_enum)]
        kind: Option<SchemaKind>,
    },
}

#[derive(Debug, Subcommand)]
enum DiscoverCommand {
    /// Search capability names, aliases, concepts, and symbols.
    Search {
        /// Search query.
        query: String,
    },
    /// Show a complete capability or symbol record.
    Detail {
        /// Capability ID or symbol name.
        identifier: String,
    },
    /// Show relationships around a capability or symbol.
    Graph {
        /// Capability ID or symbol name.
        identifier: String,
    },
    /// Show a relevant checked-in example.
    Example {
        /// Capability ID or symbol name.
        identifier: String,
    },
}

#[derive(Debug, Subcommand)]
enum ProbeCommand {
    /// List known probes and executable state.
    List,
    /// Describe a known probe contract.
    Describe {
        /// Stable probe ID.
        probe_id: String,
    },
    /// Run or dry-run a registered probe.
    Run {
        /// Stable probe ID.
        probe_id: String,
        /// Typed probe input JSON.
        #[arg(long, conflicts_with = "plan", required_unless_present = "plan")]
        input: Option<PathBuf>,
        /// Saved plan containing a typed probe request.
        #[arg(long, conflicts_with = "input")]
        plan: Option<PathBuf>,
        /// Validate and report execution without running the probe.
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SchemaKind {
    Request,
    Response,
    Goal,
    Plan,
    Probe,
}

impl Command {
    fn unavailable_name(&self) -> &'static str {
        match self {
            Self::Capabilities => "capabilities",
            Self::Discover { command } => command.unavailable_name(),
            Self::Inspect { path } => {
                let _ = path;
                "inspect"
            }
            Self::Recommend {
                path,
                goal,
                goal_file,
                probe_results,
            } => {
                let _ = (path, goal, goal_file, probe_results);
                "recommend"
            }
            Self::Plan {
                candidate_id,
                recommendation,
                project,
            } => {
                let _ = (candidate_id, recommendation, project);
                "plan"
            }
            Self::Probe { command } => command.unavailable_name(),
            Self::Shell { project } => {
                let _ = project;
                "shell"
            }
            Self::Schema { kind } => {
                let _ = kind;
                "schema"
            }
        }
    }
}

impl DiscoverCommand {
    fn unavailable_name(&self) -> &'static str {
        match self {
            Self::Search { query } => {
                let _ = query;
                "discover search"
            }
            Self::Detail { identifier } => {
                let _ = identifier;
                "discover detail"
            }
            Self::Graph { identifier } => {
                let _ = identifier;
                "discover graph"
            }
            Self::Example { identifier } => {
                let _ = identifier;
                "discover example"
            }
        }
    }
}

impl ProbeCommand {
    fn unavailable_name(&self) -> &'static str {
        match self {
            Self::List => "probe list",
            Self::Describe { probe_id } => {
                let _ = probe_id;
                "probe describe"
            }
            Self::Run {
                probe_id,
                input,
                plan,
                dry_run,
            } => {
                let _ = (probe_id, input, plan, dry_run);
                "probe run"
            }
        }
    }
}

/// Parses process arguments, dispatches a typed command, and renders stdout.
///
/// # Errors
///
/// Returns a structured error when rendering fails or the selected command is
/// not executable in the current bootstrap implementation.
pub fn run() -> DiscoveryResult<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Capabilities => {
            let envelope = Capabilities::envelope()?;
            let mut stdout = io::stdout().lock();
            if cli.json {
                render::write_json(&mut stdout, &envelope)
            } else {
                render::write_capabilities_human(&mut stdout, &envelope)
            }
        }
        Command::Discover { command } => {
            let catalog = Catalog::embedded()?;
            run_discover(&catalog, command, cli.json)
        }
        Command::Inspect { path } => run_inspect(path, cli.json),
        Command::Recommend {
            path,
            goal,
            goal_file,
            probe_results,
        } => run_recommend(path, goal, goal_file, probe_results, cli.json),
        command => Err(DiscoveryError::NotImplemented(format!(
            "{} is not implemented in this build",
            command.unavailable_name()
        ))),
    }
}

/// Runs bounded project inspection and renders the shared typed snapshot.
fn run_inspect(path: Option<PathBuf>, json: bool) -> DiscoveryResult<()> {
    let root = match path {
        Some(path) => path,
        None => std::env::current_dir()?,
    };
    let envelope = inspect_project_envelope(&root, &InspectionLimits::default())?;
    let mut stdout = io::stdout().lock();
    if json {
        render::write_json(&mut stdout, &envelope)
    } else {
        render::write_inspection_human(&mut stdout, &envelope)
    }
}

/// Runs deterministic recommendation for a supported project.
fn run_recommend(
    path: Option<PathBuf>,
    goal: Option<String>,
    goal_file: Option<PathBuf>,
    probe_results: Option<PathBuf>,
    json: bool,
) -> DiscoveryResult<()> {
    let goal = match (goal, goal_file) {
        (Some(statement), None) => GoalSpec {
            statement,
            constraints: Vec::new(),
        },
        (None, Some(path)) => commands::recommend::read_goal_spec(&path)?,
        _ => {
            return Err(DiscoveryError::InvalidInput(
                "recommend requires exactly one of --goal or --goal-file".to_owned(),
            ));
        }
    };
    let root = match path {
        Some(path) => path,
        None => std::env::current_dir()?,
    };
    let saved_probes = match probe_results {
        Some(path) => commands::recommend::read_probe_results(&path)?,
        None => Vec::new(),
    };
    let catalog = Catalog::embedded()?;
    let envelope = commands::recommend::recommend_project_envelope(
        &catalog,
        &root,
        goal,
        saved_probes,
        &InspectionLimits::default(),
    )?;
    let mut stdout = io::stdout().lock();
    if json {
        render::write_json(&mut stdout, &envelope)
    } else {
        render::write_recommendation_human(&mut stdout, &envelope)
    }
}

/// Dispatches a discover subcommand against the embedded catalog and renders output.
fn run_discover(catalog: &Catalog, command: DiscoverCommand, json: bool) -> DiscoveryResult<()> {
    let mut stdout = io::stdout().lock();
    match command {
        DiscoverCommand::Search { query } => {
            let envelope = commands::discover::search_envelope(catalog, &query);
            if json {
                render::write_json(&mut stdout, &envelope)
            } else {
                render::write_search_human(&mut stdout, &envelope)
            }
        }
        DiscoverCommand::Detail { identifier } => {
            let envelope = commands::discover::detail_envelope(catalog, &identifier)?;
            if json {
                render::write_json(&mut stdout, &envelope)
            } else {
                render::write_detail_human(&mut stdout, &envelope)
            }
        }
        DiscoverCommand::Graph { identifier } => {
            let envelope = commands::discover::graph_envelope(catalog, &identifier)?;
            if json {
                render::write_json(&mut stdout, &envelope)
            } else {
                render::write_graph_human(&mut stdout, &envelope)
            }
        }
        DiscoverCommand::Example { identifier } => {
            let envelope = commands::discover::example_envelope(catalog, &identifier)?;
            if json {
                render::write_json(&mut stdout, &envelope)
            } else {
                render::write_example_human(&mut stdout, &envelope)
            }
        }
    }
}
