// SPDX-License-Identifier: MIT OR Apache-2.0

//! Command-line parsing and dispatch for the `amari` binary.

use std::{
    ffi::OsString,
    io::{self, Write},
    path::{Path, PathBuf},
};

use clap::{Parser, Subcommand, ValueEnum};

use crate::inspect::{inspect_project_envelope, InspectionLimits};
use crate::{
    commands, render, Capabilities, Catalog, CatalogIdentity, Compatibility, DiscoveryError,
    DiscoveryResult, Envelope, GoalSpec, ReplayMetadata,
};

#[derive(Debug, Parser)]
#[command(
    name = "amari",
    version,
    about = "Discover and plan integrations with the Amari mathematical ecosystem"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    #[arg(
        long,
        global = true,
        conflicts_with = "ndjson",
        help = "Emit a versioned JSON response"
    )]
    json: bool,

    #[arg(
        long,
        global = true,
        conflicts_with = "json",
        help = "Emit one versioned NDJSON response record"
    )]
    ndjson: bool,
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
    #[command(name = "__probe-worker", hide = true)]
    ProbeWorker,
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
            Self::ProbeWorker => "__probe-worker",
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
    let mode = output_mode(cli.json, cli.ndjson);
    match cli.command {
        Command::Shell { project } => {
            let stdin = io::stdin();
            let mut reader = stdin.lock();
            let mut stdout = io::stdout().lock();
            match mode {
                render::OutputMode::Human => {
                    crate::shell::run_human(&mut reader, &mut stdout, |arguments, writer| {
                        run_shell_command(
                            arguments,
                            project.as_deref(),
                            render::OutputMode::Human,
                            writer,
                        )
                    })
                }
                render::OutputMode::Json => {
                    crate::shell::run_json(&mut reader, &mut stdout, |arguments, writer| {
                        run_shell_command(
                            arguments,
                            project.as_deref(),
                            render::OutputMode::Json,
                            writer,
                        )
                    })
                }
                render::OutputMode::Ndjson => {
                    crate::shell::run_ndjson(&mut reader, &mut stdout, |arguments, writer| {
                        run_shell_command(
                            arguments,
                            project.as_deref(),
                            render::OutputMode::Ndjson,
                            writer,
                        )
                    })
                }
            }
        }
        Command::ProbeWorker => crate::probes::worker::run_stdio(),
        command => {
            let mut stdout = io::stdout().lock();
            dispatch_command(command, mode, &mut stdout)
        }
    }
}

fn output_mode(json: bool, ndjson: bool) -> render::OutputMode {
    if ndjson {
        render::OutputMode::Ndjson
    } else if json {
        render::OutputMode::Json
    } else {
        render::OutputMode::Human
    }
}

fn run_shell_command<W: Write>(
    mut arguments: Vec<OsString>,
    session_project: Option<&Path>,
    mode: render::OutputMode,
    writer: &mut W,
) -> DiscoveryResult<()> {
    apply_session_project(&mut arguments, session_project);
    let cli = Cli::try_parse_from(std::iter::once(OsString::from("amari")).chain(arguments))
        .map_err(|error| DiscoveryError::InvalidInput(error.to_string()))?;
    if cli.json || cli.ndjson {
        return Err(DiscoveryError::InvalidInput(
            "shell output mode is selected when the shell starts, not per command".to_owned(),
        ));
    }
    match cli.command {
        Command::Shell { .. } | Command::ProbeWorker => Err(DiscoveryError::InvalidInput(
            "nested shell and worker commands are unavailable inside the shell".to_owned(),
        )),
        command => dispatch_command(command, mode, writer),
    }
}

fn apply_session_project(arguments: &mut Vec<OsString>, session_project: Option<&Path>) {
    let Some(project) = session_project else {
        return;
    };
    match arguments.first().and_then(|argument| argument.to_str()) {
        Some("inspect") if arguments.len() == 1 => arguments.push(project.as_os_str().to_owned()),
        Some("recommend") => {
            let has_explicit_path = arguments
                .get(1)
                .and_then(|argument| argument.to_str())
                .is_some_and(|argument| !argument.starts_with('-'));
            if !has_explicit_path {
                arguments.insert(1, project.as_os_str().to_owned());
            }
        }
        Some("plan") => {
            let has_explicit_project = arguments.iter().any(|argument| {
                argument == "--project"
                    || argument
                        .to_str()
                        .is_some_and(|argument| argument.starts_with("--project="))
            });
            if !has_explicit_project {
                arguments.push(OsString::from("--project"));
                arguments.push(project.as_os_str().to_owned());
            }
        }
        _ => {}
    }
}

fn dispatch_command<W: Write>(
    command: Command,
    mode: render::OutputMode,
    writer: &mut W,
) -> DiscoveryResult<()> {
    match command {
        Command::Capabilities => {
            let envelope = Capabilities::envelope()?;
            render::write_envelope(writer, &envelope, mode, render::write_capabilities_human)
        }
        Command::Discover { command } => {
            let catalog = Catalog::embedded()?;
            run_discover(&catalog, command, mode, writer)
        }
        Command::Inspect { path } => run_inspect(path, mode, writer),
        Command::Recommend {
            path,
            goal,
            goal_file,
            probe_results,
        } => run_recommend(path, goal, goal_file, probe_results, mode, writer),
        Command::Plan {
            candidate_id,
            recommendation,
            project,
        } => run_plan(candidate_id, recommendation, project, mode, writer),
        Command::Probe { command } => run_probe(command, mode, writer),
        Command::Schema { kind } => run_schema(kind, mode, writer),
        command => Err(DiscoveryError::NotImplemented(format!(
            "{} is not implemented in this build",
            command.unavailable_name()
        ))),
    }
}

/// Renders a process-level error according to the selected machine mode.
///
/// JSON mode emits one structured object on stderr. Human mode emits the
/// stable error kind and message. Rendering failures are intentionally ignored
/// because the original process error determines the exit status.
pub fn report_error(error: &DiscoveryError) {
    let machine =
        std::env::args_os().any(|argument| argument == "--json" || argument == "--ndjson");
    let mut stderr = io::stderr().lock();
    if machine {
        let payload = serde_json::json!({
            "kind": error.kind(),
            "message": error.to_string(),
            "details": { "exit_code": error.exit_code() }
        });
        let _ = serde_json::to_writer(&mut stderr, &payload);
        let _ = writeln!(stderr);
    } else {
        let _ = writeln!(stderr, "{}: {error}", error.kind());
    }
}

fn run_schema<W: Write>(
    kind: Option<SchemaKind>,
    mode: render::OutputMode,
    writer: &mut W,
) -> DiscoveryResult<()> {
    let catalog = Catalog::embedded()?;
    match kind {
        Some(kind) => {
            let kind = match kind {
                SchemaKind::Request => crate::SchemaKind::Request,
                SchemaKind::Response => crate::SchemaKind::Response,
                SchemaKind::Goal => crate::SchemaKind::Goal,
                SchemaKind::Plan => crate::SchemaKind::Plan,
                SchemaKind::Probe => crate::SchemaKind::Probe,
            };
            let envelope = schema_envelope(&catalog, crate::protocol_schema(kind)?);
            render::write_envelope(writer, &envelope, mode, render::write_schema_human)
        }
        None => {
            let envelope = schema_envelope(&catalog, crate::protocol_schema_catalog()?);
            render::write_envelope(writer, &envelope, mode, render::write_schema_catalog_human)
        }
    }
}

fn schema_envelope<T>(catalog: &Catalog, data: T) -> Envelope<T> {
    Envelope::new(
        data,
        CatalogIdentity {
            version: catalog.version().to_owned(),
            hash: catalog.content_hash().to_owned(),
        },
        Compatibility {
            status: "compatible".to_owned(),
            reasons: Vec::new(),
        },
        ReplayMetadata {
            replayable: false,
            required_hashes: Vec::new(),
            reasons: vec!["schema discovery is catalog-independent".to_owned()],
        },
    )
}

fn run_probe<W: Write>(
    command: ProbeCommand,
    mode: render::OutputMode,
    writer: &mut W,
) -> DiscoveryResult<()> {
    let catalog = Catalog::embedded()?;
    match command {
        ProbeCommand::List => {
            let envelope = commands::probe::list_envelope(&catalog)?;
            render::write_envelope(writer, &envelope, mode, render::write_probe_list_human)
        }
        ProbeCommand::Describe { probe_id } => {
            let probe_id = probe_id.parse()?;
            let envelope = commands::probe::describe_envelope(&catalog, &probe_id)?;
            render::write_envelope(
                writer,
                &envelope,
                mode,
                render::write_probe_description_human,
            )
        }
        ProbeCommand::Run {
            probe_id,
            input,
            plan,
            dry_run,
        } => {
            let probe_id = probe_id.parse()?;
            match (input, plan, dry_run) {
                (Some(_), None, true) => Err(DiscoveryError::InvalidInput(
                    "probe dry-run requires --plan and never accepts --input".to_owned(),
                )),
                (None, Some(_), false) => Err(DiscoveryError::InvalidInput(
                    "probe plan execution is disabled; provide explicit typed input with --input"
                        .to_owned(),
                )),
                (Some(path), None, false) => {
                    let envelope = commands::probe::run_input_envelope(&catalog, &probe_id, &path)?;
                    render::write_envelope(writer, &envelope, mode, render::write_probe_run_human)
                }
                (None, Some(path), true) => {
                    let envelope =
                        commands::probe::dry_run_plan_envelope(&catalog, &probe_id, &path)?;
                    render::write_envelope(
                        writer,
                        &envelope,
                        mode,
                        render::write_probe_dry_run_human,
                    )
                }
                _ => Err(DiscoveryError::InvalidInput(
                    "probe run requires exactly one of --input or --plan".to_owned(),
                )),
            }
        }
    }
}

/// Runs bounded project inspection and renders the shared typed snapshot.
fn run_inspect<W: Write>(
    path: Option<PathBuf>,
    mode: render::OutputMode,
    writer: &mut W,
) -> DiscoveryResult<()> {
    let root = match path {
        Some(path) => path,
        None => std::env::current_dir()?,
    };
    let envelope = inspect_project_envelope(&root, &InspectionLimits::default())?;
    render::write_envelope(writer, &envelope, mode, render::write_inspection_human)
}

/// Runs deterministic recommendation for a supported project.
fn run_recommend<W: Write>(
    path: Option<PathBuf>,
    goal: Option<String>,
    goal_file: Option<PathBuf>,
    probe_results: Option<PathBuf>,
    mode: render::OutputMode,
    writer: &mut W,
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
    render::write_envelope(writer, &envelope, mode, render::write_recommendation_human)
}

/// Replays one candidate from a saved recommendation artifact.
fn run_plan<W: Write>(
    candidate_id: String,
    recommendation: PathBuf,
    project: PathBuf,
    mode: render::OutputMode,
    writer: &mut W,
) -> DiscoveryResult<()> {
    let artifact = commands::plan::read_recommendation(&recommendation)?;
    let catalog = Catalog::embedded()?;
    let envelope = commands::plan::replay_plan_envelope(
        &catalog,
        &project,
        &candidate_id,
        artifact,
        &InspectionLimits::default(),
    )?;
    render::write_envelope(writer, &envelope, mode, render::write_plan_human)
}

/// Dispatches a discover subcommand against the embedded catalog and renders output.
fn run_discover<W: Write>(
    catalog: &Catalog,
    command: DiscoverCommand,
    mode: render::OutputMode,
    writer: &mut W,
) -> DiscoveryResult<()> {
    match command {
        DiscoverCommand::Search { query } => {
            let envelope = commands::discover::search_envelope(catalog, &query);
            render::write_envelope(writer, &envelope, mode, render::write_search_human)
        }
        DiscoverCommand::Detail { identifier } => {
            let envelope = commands::discover::detail_envelope(catalog, &identifier)?;
            render::write_envelope(writer, &envelope, mode, render::write_detail_human)
        }
        DiscoverCommand::Graph { identifier } => {
            let envelope = commands::discover::graph_envelope(catalog, &identifier)?;
            render::write_envelope(writer, &envelope, mode, render::write_graph_human)
        }
        DiscoverCommand::Example { identifier } => {
            let envelope = commands::discover::example_envelope(catalog, &identifier)?;
            render::write_envelope(writer, &envelope, mode, render::write_example_human)
        }
    }
}
