// SPDX-License-Identifier: MIT OR Apache-2.0

//! Command-line parsing and dispatch for the `amari` binary.

use std::{io, path::PathBuf};

use clap::{Parser, Subcommand, ValueEnum};

use crate::{render, Capabilities, DiscoveryError, DiscoveryResult};

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
            } => {
                let _ = (path, goal, goal_file);
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
        command => Err(DiscoveryError::NotImplemented(format!(
            "{} is not implemented in this build",
            command.unavailable_name()
        ))),
    }
}
