// SPDX-License-Identifier: MIT OR Apache-2.0

//! Dynamic descriptions of the installed discovery binary.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{CatalogIdentity, Compatibility, DiscoveryError, Envelope, ReplayMetadata, SCHEMA_V1};

const BOOTSTRAP_CATALOG_HASH: &str =
    "c24c47368b1c638d66f464909d02ae8146194e82b0dc5340369d5875359f8c18";

/// Embedded catalog availability reported by the running binary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CatalogStatus {
    /// Embedded catalog version or `bootstrap` before catalog integration.
    pub version: String,
    /// Deterministic identity hash for the reported catalog state.
    pub hash: String,
    /// Whether a validated embedded capability catalog is available.
    pub available: bool,
}

/// Runtime state for an inspector or probe descriptor.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RuntimeCapabilityState {
    /// Stable runtime component identifier.
    pub id: String,
    /// Whether this binary recognizes the component descriptor.
    pub known: bool,
    /// Whether it is compatible with the current environment or project context.
    pub available: bool,
    /// Whether executable implementation code is compiled into this binary.
    pub executable: bool,
    /// Explanation when a state needs qualification.
    pub reason: Option<String>,
}

/// Compile-time state of an optional Cargo feature.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct FeatureGate {
    /// Cargo feature name.
    pub name: String,
    /// Whether the running binary was compiled with the feature.
    pub compiled: bool,
}

/// Provider-neutral AI adapter state.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AiAdapterStatus {
    /// Whether the provider-neutral contract is compiled.
    pub contract_compiled: bool,
    /// Whether a concrete provider is configured.
    pub provider_configured: bool,
    /// Whether an adapter may execute in this process.
    pub executable: bool,
}

/// Operating-system and architecture information.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct PlatformInfo {
    /// Operating-system identifier.
    pub os: String,
    /// CPU architecture identifier.
    pub arch: String,
    /// Platform family such as `unix` or `windows`.
    pub family: String,
    /// Full compilation target triple when this record describes a target.
    pub triple: Option<String>,
    /// How the platform information was obtained.
    pub source: String,
}

/// Default resource ceilings enforced by future inspectors and probes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum files considered by one project inspection.
    pub max_inspection_files: u64,
    /// Maximum aggregate bytes read by one project inspection.
    pub max_inspection_bytes: u64,
    /// Maximum recursive traversal depth.
    pub max_traversal_depth: u64,
    /// Maximum bytes accepted as one probe request.
    pub max_probe_input_bytes: u64,
    /// Maximum bytes emitted by one probe response.
    pub max_probe_output_bytes: u64,
    /// Default probe wall-clock limit in milliseconds.
    pub probe_timeout_millis: u64,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_inspection_files: 10_000,
            max_inspection_bytes: 16 * 1024 * 1024,
            max_traversal_depth: 32,
            max_probe_input_bytes: 1024 * 1024,
            max_probe_output_bytes: 1024 * 1024,
            probe_timeout_millis: 5_000,
        }
    }
}

/// A truthful description of functionality in the installed `amari` binary.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Capabilities {
    /// Installed binary name.
    pub binary: String,
    /// `amari-discovery` package version.
    pub tool_version: String,
    /// Supported machine protocol schemas.
    pub protocol_versions: Vec<String>,
    /// Embedded catalog status.
    pub catalog: CatalogStatus,
    /// Currently supported output modes.
    pub output_modes: Vec<String>,
    /// Default bounded-operation limits.
    pub resource_limits: ResourceLimits,
    /// Runtime host information.
    pub host: PlatformInfo,
    /// Compilation target information.
    pub target: PlatformInfo,
    /// Known project inspector states.
    pub project_inspectors: Vec<RuntimeCapabilityState>,
    /// Known probe states from the embedded catalog.
    pub known_probes: Vec<RuntimeCapabilityState>,
    /// Optional compile-time feature states.
    pub feature_gates: Vec<FeatureGate>,
    /// Optional AI adapter state.
    pub ai_adapter: AiAdapterStatus,
    /// Stable process exit codes keyed by error kind.
    pub exit_codes: BTreeMap<String, u8>,
}

#[cfg(unix)]
fn detect_host() -> PlatformInfo {
    let host = rustix::system::uname();
    PlatformInfo {
        os: host.sysname().to_string_lossy().to_ascii_lowercase(),
        arch: host.machine().to_string_lossy().into_owned(),
        family: "unix".into(),
        triple: None,
        source: "uname".into(),
    }
}

#[cfg(windows)]
fn detect_host() -> PlatformInfo {
    PlatformInfo {
        os: "windows".into(),
        arch: std::env::var("PROCESSOR_ARCHITECTURE").unwrap_or_else(|_| "unknown".into()),
        family: "windows".into(),
        triple: None,
        source: "windows-environment".into(),
    }
}

#[cfg(not(any(unix, windows)))]
fn detect_host() -> PlatformInfo {
    PlatformInfo {
        os: "unknown".into(),
        arch: "unknown".into(),
        family: "unknown".into(),
        triple: None,
        source: "unavailable".into(),
    }
}

fn compilation_target() -> PlatformInfo {
    PlatformInfo {
        os: std::env::consts::OS.to_owned(),
        arch: std::env::consts::ARCH.to_owned(),
        family: std::env::consts::FAMILY.to_owned(),
        triple: Some(env!("AMARI_DISCOVERY_TARGET").to_owned()),
        source: "cargo-target".into(),
    }
}

impl Capabilities {
    /// Detects capabilities available in the running binary.
    pub fn current() -> Self {
        let unavailable_reason = Some("implementation is not available in bootstrap mode".into());

        Self {
            binary: "amari".into(),
            tool_version: env!("CARGO_PKG_VERSION").into(),
            protocol_versions: vec![SCHEMA_V1.into()],
            catalog: CatalogStatus {
                version: "bootstrap".into(),
                hash: BOOTSTRAP_CATALOG_HASH.into(),
                available: false,
            },
            output_modes: vec!["human".into(), "json".into()],
            resource_limits: ResourceLimits::default(),
            host: detect_host(),
            target: compilation_target(),
            project_inspectors: vec![
                RuntimeCapabilityState {
                    id: "rust-cargo".into(),
                    known: true,
                    available: false,
                    executable: false,
                    reason: unavailable_reason.clone(),
                },
                RuntimeCapabilityState {
                    id: "npm-typescript".into(),
                    known: true,
                    available: false,
                    executable: false,
                    reason: unavailable_reason,
                },
            ],
            known_probes: Vec::new(),
            feature_gates: vec![
                FeatureGate {
                    name: "standard-probes".into(),
                    compiled: cfg!(feature = "standard-probes"),
                },
                FeatureGate {
                    name: "ai".into(),
                    compiled: cfg!(feature = "ai"),
                },
            ],
            ai_adapter: AiAdapterStatus {
                contract_compiled: cfg!(feature = "ai"),
                provider_configured: false,
                executable: false,
            },
            exit_codes: DiscoveryError::exit_codes()
                .iter()
                .map(|(kind, code)| ((*kind).to_owned(), *code))
                .collect(),
        }
    }

    /// Wraps current capabilities in the shared versioned response envelope.
    pub fn envelope() -> Envelope<Self> {
        let capabilities = Self::current();
        let catalog = CatalogIdentity {
            version: capabilities.catalog.version.clone(),
            hash: capabilities.catalog.hash.clone(),
        };
        Envelope::new(
            capabilities,
            catalog,
            Compatibility {
                status: "bootstrap".into(),
                reasons: vec!["embedded capability catalog is not available yet".into()],
            },
            ReplayMetadata {
                replayable: false,
                required_hashes: vec![],
                reasons: vec!["bootstrap capabilities are environment-specific".into()],
            },
        )
    }
}
