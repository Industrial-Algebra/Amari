// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded conservative TypeScript and generated declaration scanners.

pub mod types;
pub use types::{
    TypeScriptCapabilityEvidence, TypeScriptDeclarationExport, TypeScriptExportKind,
    TypeScriptFileContext, TypeScriptFileRole, TypeScriptImport, TypeScriptImportKind,
    TypeScriptInspection, TypeScriptInspectionWarning, TypeScriptRuntimeEvidence,
    TypeScriptRuntimeSignal, TypeScriptVocabularyEvidence,
};

use std::cell::Cell;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::time::Instant;

use sha2::{Digest, Sha256};
use walkdir::{DirEntry, WalkDir};

use super::types::NpmInspection;
use crate::catalog::generator::wasm::{parse_wasm_surface, WasmSurface};
use crate::error::{DiscoveryError, DiscoveryResult};
use crate::inspect::snapshot::{InspectionLimit, SnapshotState, SourceLocation};
use crate::inspect::{
    bounded_read, is_env_secret_name, is_skipped_dir_name, nofollow_open_readonly, BoundedOutcome,
    InspectionLimits, NofollowResult,
};
use crate::{CapabilityId, Catalog};

const AMARI_PACKAGE: &str = "@justinelliottcobb/amari-wasm";
const DECLARATION_PATH: &str = "node_modules/@justinelliottcobb/amari-wasm/amari_wasm.d.ts";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CandidateKind {
    Source,
    Declaration,
}

#[derive(Debug)]
struct Candidate {
    relative: String,
    absolute: PathBuf,
    kind: CandidateKind,
}

fn is_source_name(name: &str) -> bool {
    !name.ends_with(".d.ts")
        && [".ts", ".tsx", ".mts", ".cts", ".js", ".jsx", ".mjs", ".cjs"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
}

fn should_descend(entry: &DirEntry, max_depth: u64, depth_pruned: &Cell<bool>) -> bool {
    if !entry.file_type().is_dir() || entry.depth() == 0 {
        return true;
    }
    let Some(name) = entry.file_name().to_str() else {
        return false;
    };
    if is_skipped_dir_name(name) || is_env_secret_name(name) {
        return false;
    }
    if entry.depth() as u64 >= max_depth {
        depth_pruned.set(true);
        return false;
    }
    true
}

fn normalized_relative(path: &Path, root: &Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in path.strip_prefix(root).ok()?.components() {
        match component {
            Component::Normal(value) => parts.push(value.to_str()?),
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn collect_candidates(
    root: &Path,
    npm: &NpmInspection,
    limits: &InspectionLimits,
    warnings: &mut Vec<TypeScriptInspectionWarning>,
) -> (Vec<Candidate>, bool) {
    let depth_pruned = Cell::new(false);
    let mut candidates = Vec::new();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|entry| should_descend(entry, limits.max_traversal_depth, &depth_pruned));
    for result in walker {
        let Ok(entry) = result else {
            continue;
        };
        if entry.depth() == 0 || entry.file_type().is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str() else {
            warnings.push(TypeScriptInspectionWarning::NonUtf8Path {
                path_hint: "non-UTF-8 TypeScript candidate".to_string(),
            });
            continue;
        };
        if !is_source_name(name) || is_env_secret_name(name) {
            continue;
        }
        let Some(relative) = normalized_relative(entry.path(), root) else {
            warnings.push(TypeScriptInspectionWarning::NonUtf8Path {
                path_hint: "non-UTF-8 TypeScript candidate".to_string(),
            });
            continue;
        };
        candidates.push(Candidate {
            relative,
            absolute: entry.path().to_path_buf(),
            kind: CandidateKind::Source,
        });
    }
    let has_amari = npm
        .package
        .dependencies
        .iter()
        .any(|dependency| dependency.package_name == AMARI_PACKAGE);
    if has_amari {
        if DECLARATION_PATH.split('/').count() as u64 > limits.max_traversal_depth {
            depth_pruned.set(true);
        } else {
            candidates.push(Candidate {
                relative: DECLARATION_PATH.to_string(),
                absolute: root.join(DECLARATION_PATH),
                kind: CandidateKind::Declaration,
            });
        }
    }
    candidates.sort_by(|a, b| a.relative.cmp(&b.relative));
    candidates.dedup_by(|a, b| a.relative == b.relative);
    (candidates, depth_pruned.get())
}

#[derive(Debug)]
enum OpenOutcome {
    Opened(std::fs::File),
    Missing,
    Symlink,
    Unsupported,
}

#[cfg(unix)]
fn open_declaration(root: &Path) -> std::io::Result<OpenOutcome> {
    use rustix::fs::{open, openat, Mode, OFlags};

    let mut directory = open(
        root,
        OFlags::DIRECTORY | OFlags::NONBLOCK | OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    )?;
    for component in ["node_modules", "@justinelliottcobb", "amari-wasm"] {
        directory = match openat(
            &directory,
            component,
            OFlags::DIRECTORY
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK
                | OFlags::RDONLY
                | OFlags::CLOEXEC,
            Mode::empty(),
        ) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT) => return Ok(OpenOutcome::Missing),
            Err(rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR) => {
                return Ok(OpenOutcome::Symlink)
            }
            Err(error) => return Err(error.into()),
        };
    }
    let fd = match openat(
        &directory,
        "amari_wasm.d.ts",
        OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::RDONLY | OFlags::CLOEXEC,
        Mode::empty(),
    ) {
        Ok(fd) => fd,
        Err(rustix::io::Errno::NOENT) => return Ok(OpenOutcome::Missing),
        Err(rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR) => {
            return Ok(OpenOutcome::Symlink)
        }
        Err(error) => return Err(error.into()),
    };
    let file: std::fs::File = fd.into();
    if !file.metadata()?.is_file() {
        return Ok(OpenOutcome::Unsupported);
    }
    Ok(OpenOutcome::Opened(file))
}

#[cfg(not(unix))]
fn open_declaration(root: &Path) -> std::io::Result<OpenOutcome> {
    let mut current = root.to_path_buf();
    for component in ["node_modules", "@justinelliottcobb", "amari-wasm"] {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => return Ok(OpenOutcome::Symlink),
            Ok(metadata) if !metadata.is_dir() => return Ok(OpenOutcome::Unsupported),
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(OpenOutcome::Missing)
            }
            Err(error) => return Err(error),
        }
    }
    match nofollow_open_readonly(&current.join("amari_wasm.d.ts")) {
        Ok(NofollowResult::Opened(file)) => Ok(OpenOutcome::Opened(file)),
        Ok(NofollowResult::SymlinkOrRace) => Ok(OpenOutcome::Symlink),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(OpenOutcome::Missing),
        Err(error) => Err(error),
    }
}

fn open_candidate(root: &Path, candidate: &Candidate) -> std::io::Result<OpenOutcome> {
    if candidate.kind == CandidateKind::Declaration {
        return open_declaration(root);
    }
    match nofollow_open_readonly(&candidate.absolute) {
        Ok(NofollowResult::Opened(file)) => Ok(OpenOutcome::Opened(file)),
        Ok(NofollowResult::SymlinkOrRace) => Ok(OpenOutcome::Symlink),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(OpenOutcome::Missing),
        Err(error) => Err(error),
    }
}

#[derive(Default)]
struct Evidence {
    imports: Vec<TypeScriptImport>,
    declaration_exports: Vec<TypeScriptDeclarationExport>,
    runtime_signals: Vec<TypeScriptRuntimeEvidence>,
    file_contexts: Vec<TypeScriptFileContext>,
    vocabulary: Vec<TypeScriptVocabularyEvidence>,
    capabilities: Vec<TypeScriptCapabilityEvidence>,
    warnings: Vec<TypeScriptInspectionWarning>,
    entries: Vec<(String, Vec<u8>)>,
    input_files: Vec<SourceLocation>,
    total_bytes: u64,
}

impl Evidence {
    fn accept(&mut self, path: &str, bytes: Vec<u8>) -> SourceLocation {
        let source = SourceLocation {
            path: path.to_string(),
            line: None,
            column: None,
            content_hash: hex::encode(Sha256::digest(&bytes)),
        };
        self.total_bytes = self.total_bytes.saturating_add(bytes.len() as u64);
        self.input_files.push(source.clone());
        self.entries.push((path.to_string(), bytes));
        source
    }

    fn input_hash(&self) -> String {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut hasher = Sha256::new();
        for (path, bytes) in entries {
            hasher.update((path.len() as u32).to_le_bytes());
            hasher.update(path.as_bytes());
            hasher.update((bytes.len() as u64).to_le_bytes());
            hasher.update(bytes);
        }
        hex::encode(hasher.finalize())
    }
}

fn located(base: &SourceLocation, line: Option<u32>) -> SourceLocation {
    SourceLocation {
        path: base.path.clone(),
        line,
        column: None,
        content_hash: base.content_hash.clone(),
    }
}

fn line_at(source: &str, offset: usize) -> u32 {
    source.as_bytes()[..offset.min(source.len())]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        .saturating_add(1) as u32
}

fn mask(source: &str, strings: bool) -> String {
    #[derive(Clone, Copy, PartialEq)]
    enum State {
        Code,
        Line,
        Block,
        Single,
        Double,
        Template,
    }
    let bytes = source.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut state = State::Code;
    let mut i = 0;
    while i < bytes.len() {
        let byte = bytes[i];
        let next = bytes.get(i + 1).copied();
        match state {
            State::Code if byte == b'/' && next == Some(b'/') => {
                out.extend_from_slice(b"  ");
                state = State::Line;
                i += 2;
            }
            State::Code if byte == b'/' && next == Some(b'*') => {
                out.extend_from_slice(b"  ");
                state = State::Block;
                i += 2;
            }
            State::Code if matches!(byte, b'\'' | b'"' | b'`') => {
                state = match byte {
                    b'\'' => State::Single,
                    b'"' => State::Double,
                    _ => State::Template,
                };
                out.push(if strings { b' ' } else { byte });
                i += 1;
            }
            State::Line if byte == b'\n' => {
                out.push(byte);
                state = State::Code;
                i += 1;
            }
            State::Line | State::Block => {
                if state == State::Block && byte == b'*' && next == Some(b'/') {
                    out.extend_from_slice(b"  ");
                    state = State::Code;
                    i += 2;
                } else {
                    out.push(if byte == b'\n' { byte } else { b' ' });
                    i += 1;
                }
            }
            State::Single | State::Double | State::Template => {
                let close = match state {
                    State::Single => b'\'',
                    State::Double => b'"',
                    _ => b'`',
                };
                out.push(if strings && byte != b'\n' { b' ' } else { byte });
                if byte == b'\\' && next.is_some() {
                    let escaped = next.unwrap_or_default();
                    out.push(if strings && escaped != b'\n' {
                        b' '
                    } else {
                        escaped
                    });
                    i += 2;
                } else {
                    if byte == close {
                        state = State::Code;
                    }
                    i += 1;
                }
            }
            State::Code => {
                out.push(byte);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_default()
}

fn push_named(
    clause: &str,
    type_only: bool,
    source: &SourceLocation,
    out: &mut Vec<TypeScriptImport>,
) {
    let (Some(start), Some(end)) = (clause.find('{'), clause.rfind('}')) else {
        return;
    };
    if end <= start {
        return;
    }
    for item in clause[start + 1..end]
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let parts: Vec<_> = item.split_whitespace().collect();
        let imported = parts[0];
        let local = if parts.len() == 3 && parts[1] == "as" {
            parts[2]
        } else {
            imported
        };
        out.push(TypeScriptImport {
            package_name: AMARI_PACKAGE.to_string(),
            imported_name: Some(imported.to_string()),
            local_name: Some(local.to_string()),
            kind: TypeScriptImportKind::Named,
            type_only,
            source: source.clone(),
        });
    }
}

fn scan_imports(text: &str, base: &SourceLocation) -> Vec<TypeScriptImport> {
    let cleaned = mask(text, false);
    let mut out = Vec::new();
    let mut offset = 0;
    for chunk in cleaned.split_inclusive(';') {
        let normalized = chunk.split_whitespace().collect::<Vec<_>>().join(" ");
        let source = located(base, Some(line_at(&cleaned, offset)));
        let markers = [
            format!("from \"{AMARI_PACKAGE}\""),
            format!("from '{AMARI_PACKAGE}'"),
        ];
        if let Some(start) = normalized.find("import ") {
            let statement = &normalized[start..];
            if let Some(marker) = markers.iter().find_map(|marker| statement.find(marker)) {
                let mut clause = statement[7..marker].trim();
                let type_only = clause.starts_with("type ");
                if type_only {
                    clause = clause.trim_start_matches("type ").trim();
                }
                push_named(clause, type_only, &source, &mut out);
                let words: Vec<_> = clause.split_whitespace().collect();
                if let Some(namespace) = words
                    .windows(3)
                    .find(|w| w[0] == "*" && w[1] == "as")
                    .map(|w| w[2])
                {
                    out.push(TypeScriptImport {
                        package_name: AMARI_PACKAGE.to_string(),
                        imported_name: None,
                        local_name: Some(namespace.trim_end_matches(',').to_string()),
                        kind: TypeScriptImportKind::Namespace,
                        type_only,
                        source: source.clone(),
                    });
                }
                let first = clause.split(',').next().unwrap_or("").trim();
                if !first.is_empty() && !first.starts_with('{') && !first.starts_with('*') {
                    out.push(TypeScriptImport {
                        package_name: AMARI_PACKAGE.to_string(),
                        imported_name: Some("default".to_string()),
                        local_name: Some(first.to_string()),
                        kind: TypeScriptImportKind::Default,
                        type_only,
                        source: source.clone(),
                    });
                }
            }
        }
        let compact: String = normalized.chars().filter(|c| !c.is_whitespace()).collect();
        for (needle, kind) in [
            (
                format!("import(\"{AMARI_PACKAGE}\")"),
                TypeScriptImportKind::Dynamic,
            ),
            (
                format!("import('{AMARI_PACKAGE}')"),
                TypeScriptImportKind::Dynamic,
            ),
            (
                format!("require(\"{AMARI_PACKAGE}\")"),
                TypeScriptImportKind::Require,
            ),
            (
                format!("require('{AMARI_PACKAGE}')"),
                TypeScriptImportKind::Require,
            ),
        ] {
            if compact.contains(&needle) {
                out.push(TypeScriptImport {
                    package_name: AMARI_PACKAGE.to_string(),
                    imported_name: None,
                    local_name: None,
                    kind,
                    type_only: false,
                    source: source.clone(),
                });
            }
        }
        for needle in [
            format!("import\"{AMARI_PACKAGE}\""),
            format!("import'{AMARI_PACKAGE}'"),
        ] {
            if compact.starts_with(&needle) {
                out.push(TypeScriptImport {
                    package_name: AMARI_PACKAGE.to_string(),
                    imported_name: None,
                    local_name: None,
                    kind: TypeScriptImportKind::SideEffect,
                    type_only: false,
                    source: source.clone(),
                });
            }
        }
        offset += chunk.len();
    }
    out.sort();
    out.dedup();
    out
}

fn has_identifier(text: &str, identifier: &str) -> bool {
    text.split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .any(|token| token == identifier)
}

fn scan_runtime(path: &str, text: &str, base: &SourceLocation) -> Vec<TypeScriptRuntimeEvidence> {
    let mut found = Vec::new();
    let name = path.rsplit('/').next().unwrap_or(path);
    if name.starts_with("vite.config.") {
        found.push(TypeScriptRuntimeSignal::Vite);
    }
    if name.starts_with("webpack.config.") {
        found.push(TypeScriptRuntimeSignal::Webpack);
    }
    let code = mask(text, true);
    if ["window", "document", "navigator"]
        .iter()
        .any(|id| has_identifier(&code, id))
    {
        found.push(TypeScriptRuntimeSignal::Browser);
    }
    if ["process", "Buffer", "__dirname", "__filename"]
        .iter()
        .any(|id| has_identifier(&code, id))
    {
        found.push(TypeScriptRuntimeSignal::Node);
    }
    if has_identifier(&code, "WebAssembly") {
        found.push(TypeScriptRuntimeSignal::WebAssembly);
    }
    found.sort();
    found.dedup();
    found
        .into_iter()
        .map(|signal| TypeScriptRuntimeEvidence {
            signal,
            source: base.clone(),
        })
        .collect()
}

fn classify_file(path: &str) -> TypeScriptFileRole {
    let lower = path.to_ascii_lowercase();
    let name = lower.rsplit('/').next().unwrap_or(&lower);
    let segments: Vec<_> = lower.split('/').collect();
    if name.contains(".test.")
        || name.contains(".spec.")
        || segments
            .iter()
            .any(|segment| matches!(*segment, "test" | "tests" | "__tests__"))
    {
        TypeScriptFileRole::Test
    } else if segments
        .iter()
        .any(|segment| matches!(*segment, "example" | "examples" | "demo" | "demos"))
    {
        TypeScriptFileRole::Example
    } else if name.contains("config.") {
        TypeScriptFileRole::Config
    } else {
        TypeScriptFileRole::Source
    }
}

fn scan_vocabulary(text: &str, base: &SourceLocation) -> Vec<TypeScriptVocabularyEvidence> {
    const TERMS: &[(&str, &str)] = &[
        ("geometric algebra", "geometric algebra"),
        ("multivector", "multivector"),
        ("rotor", "rotor"),
        ("tropical", "tropical algebra"),
        ("dual number", "dual number"),
        ("surreal", "surreal number"),
        ("combinatorial game", "combinatorial game theory"),
        ("clifford", "Clifford algebra"),
    ];
    let lower = text.to_ascii_lowercase();
    let mut evidence = Vec::new();
    for (needle, canonical) in TERMS {
        if let Some(offset) = lower.find(needle) {
            evidence.push(TypeScriptVocabularyEvidence {
                term: (*canonical).to_string(),
                source: located(base, Some(line_at(text, offset))),
            });
        }
    }
    evidence
}

fn surface_has_path(surface: &WasmSurface, path: &str) -> bool {
    let Some((class_name, member)) = path.split_once('.') else {
        return false;
    };
    surface.classes.iter().any(|class| {
        class.name == class_name
            && (class.methods.iter().any(|method| method.name == member)
                || class
                    .static_methods
                    .iter()
                    .any(|method| method.name == member)
                || class.getters.iter().any(|getter| getter.name == member))
    })
}

fn scan_declarations(
    text: &str,
    source: &SourceLocation,
    catalog: &Catalog,
) -> DiscoveryResult<(
    Vec<TypeScriptDeclarationExport>,
    Vec<TypeScriptCapabilityEvidence>,
)> {
    let surface = parse_wasm_surface(text).map_err(|_| {
        DiscoveryError::InspectionFailure("generated declaration surface is malformed".to_string())
    })?;
    let mut exports = Vec::new();
    for class in &surface.classes {
        exports.push(TypeScriptDeclarationExport {
            name: class.name.clone(),
            kind: TypeScriptExportKind::Class,
            source: source.clone(),
        });
    }
    for function in &surface.functions {
        exports.push(TypeScriptDeclarationExport {
            name: function.name.clone(),
            kind: TypeScriptExportKind::Function,
            source: source.clone(),
        });
    }
    for value in &surface.enums {
        exports.push(TypeScriptDeclarationExport {
            name: value.name.clone(),
            kind: TypeScriptExportKind::Enum,
            source: source.clone(),
        });
    }
    for interface in &surface.interfaces {
        exports.push(TypeScriptDeclarationExport {
            name: interface.name.clone(),
            kind: TypeScriptExportKind::Interface,
            source: source.clone(),
        });
    }
    for alias in &surface.type_aliases {
        exports.push(TypeScriptDeclarationExport {
            name: alias.name.clone(),
            kind: TypeScriptExportKind::TypeAlias,
            source: source.clone(),
        });
    }
    exports.sort();
    exports.dedup();
    let mut capabilities = Vec::new();
    for mapping in catalog.wasm_capability_mappings() {
        if surface_has_path(&surface, &mapping.wasm_path) {
            capabilities.push(TypeScriptCapabilityEvidence {
                wasm_path: mapping.wasm_path.clone(),
                capability_id: CapabilityId::from_str(&mapping.capability_id)?,
                source: source.clone(),
            });
        }
    }
    capabilities.sort();
    capabilities.dedup();
    Ok((exports, capabilities))
}

fn elapsed_exceeded(started: Instant, limits: &InspectionLimits) -> bool {
    started.elapsed().as_millis() > u128::from(limits.max_inspection_wall_millis)
}

/// Inspects bounded TypeScript sources and the installed generated Amari WASM declaration file.
///
/// This scanner performs only read-only local file access. It never runs npm, Node.js,
/// bundlers, lifecycle scripts, project code, shell commands, or network requests.
///
/// # Errors
///
/// Returns an error when `root` is not a directory or validated catalog capability IDs
/// cannot be reconstructed. Missing, malformed, symlinked, and resource-limited inputs
/// are represented as typed warnings or partial states.
pub fn inspect_typescript_sources(
    root: impl AsRef<Path>,
    npm: &NpmInspection,
    catalog: &Catalog,
    limits: &InspectionLimits,
) -> DiscoveryResult<TypeScriptInspection> {
    let root = root.as_ref();
    if !root.is_dir() {
        return Err(DiscoveryError::InspectionFailure(
            "TypeScript inspection root is not a directory".to_string(),
        ));
    }
    let started = Instant::now();
    let mut evidence = Evidence::default();
    let (candidates, depth_pruned) = collect_candidates(root, npm, limits, &mut evidence.warnings);
    let mut state = if depth_pruned {
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::TraversalDepth {
                max: limits.max_traversal_depth,
            },
        }
    } else {
        SnapshotState::Complete
    };
    let mut considered = 0u64;

    for candidate in candidates {
        if elapsed_exceeded(started, limits) {
            state = SnapshotState::LimitExceeded {
                limit: InspectionLimit::WallClock {
                    max_millis: limits.max_inspection_wall_millis,
                    observed_millis: limits.max_inspection_wall_millis.saturating_add(1),
                },
            };
            break;
        }
        let opened = match open_candidate(root, &candidate) {
            Ok(value) => value,
            Err(_) => {
                evidence
                    .warnings
                    .push(TypeScriptInspectionWarning::ReadFailure {
                        path: candidate.relative,
                    });
                continue;
            }
        };
        let mut file = match opened {
            OpenOutcome::Opened(file) => file,
            OpenOutcome::Missing if candidate.kind == CandidateKind::Declaration => {
                evidence
                    .warnings
                    .push(TypeScriptInspectionWarning::MissingDeclarations {
                        path: candidate.relative,
                    });
                continue;
            }
            OpenOutcome::Missing => continue,
            OpenOutcome::Symlink => {
                evidence
                    .warnings
                    .push(TypeScriptInspectionWarning::SymlinkedFile {
                        path: candidate.relative,
                    });
                continue;
            }
            OpenOutcome::Unsupported => {
                evidence
                    .warnings
                    .push(TypeScriptInspectionWarning::ReadFailure {
                        path: candidate.relative,
                    });
                continue;
            }
        };
        considered = considered.saturating_add(1);
        if considered > limits.max_inspection_files {
            let limit = InspectionLimit::FileCount {
                max: limits.max_inspection_files,
                observed: considered,
            };
            evidence
                .warnings
                .push(TypeScriptInspectionWarning::LimitExceeded {
                    limit: limit.clone(),
                });
            state = SnapshotState::LimitExceeded { limit };
            break;
        }
        let remaining = limits
            .max_inspection_bytes
            .saturating_sub(evidence.total_bytes);
        let bytes = match bounded_read(&mut file, limits.max_per_file_bytes, remaining) {
            Ok(BoundedOutcome::Accepted(bytes)) if bytes.len() as u64 > remaining => {
                let limit = InspectionLimit::TotalBytes {
                    max: limits.max_inspection_bytes,
                    observed: limits.max_inspection_bytes.saturating_add(1),
                };
                evidence
                    .warnings
                    .push(TypeScriptInspectionWarning::LimitExceeded {
                        limit: limit.clone(),
                    });
                state = SnapshotState::LimitExceeded { limit };
                break;
            }
            Ok(BoundedOutcome::Accepted(bytes)) => bytes,
            Ok(BoundedOutcome::PerFileExceeded) => {
                evidence
                    .warnings
                    .push(TypeScriptInspectionWarning::OversizedFile {
                        path: candidate.relative,
                        limit: limits.max_per_file_bytes,
                        observed: limits.max_per_file_bytes.saturating_add(1),
                    });
                continue;
            }
            Err(_) => {
                evidence
                    .warnings
                    .push(TypeScriptInspectionWarning::ReadFailure {
                        path: candidate.relative,
                    });
                continue;
            }
        };
        let source = evidence.accept(&candidate.relative, bytes.clone());
        let text = match std::str::from_utf8(&bytes) {
            Ok(text) => text,
            Err(_) => {
                evidence
                    .warnings
                    .push(TypeScriptInspectionWarning::InvalidUtf8Source {
                        path: candidate.relative,
                        content_hash: source.content_hash,
                    });
                continue;
            }
        };
        match candidate.kind {
            CandidateKind::Source => {
                evidence.imports.extend(scan_imports(text, &source));
                evidence
                    .runtime_signals
                    .extend(scan_runtime(&candidate.relative, text, &source));
                evidence.file_contexts.push(TypeScriptFileContext {
                    role: classify_file(&candidate.relative),
                    source: source.clone(),
                });
                evidence.vocabulary.extend(scan_vocabulary(text, &source));
            }
            CandidateKind::Declaration => match scan_declarations(text, &source, catalog) {
                Ok((exports, capabilities)) => {
                    evidence.declaration_exports.extend(exports);
                    evidence.capabilities.extend(capabilities);
                }
                Err(_) => {
                    evidence
                        .warnings
                        .push(TypeScriptInspectionWarning::MalformedDeclarations {
                            path: candidate.relative,
                            reason: "no supported generated exports could be parsed".to_string(),
                            content_hash: source.content_hash,
                        })
                }
            },
        }
    }

    evidence.imports.sort();
    evidence.imports.dedup();
    evidence.declaration_exports.sort();
    evidence.declaration_exports.dedup();
    evidence.runtime_signals.sort();
    evidence.runtime_signals.dedup();
    evidence.file_contexts.sort();
    evidence.file_contexts.dedup();
    evidence.vocabulary.sort();
    evidence.vocabulary.dedup();
    evidence.capabilities.sort();
    evidence.capabilities.dedup();
    evidence.input_files.sort();
    evidence.input_files.dedup();
    let input_hash = evidence.input_hash();
    Ok(TypeScriptInspection {
        imports: evidence.imports,
        declaration_exports: evidence.declaration_exports,
        runtime_signals: evidence.runtime_signals,
        file_contexts: evidence.file_contexts,
        vocabulary: evidence.vocabulary,
        capabilities: evidence.capabilities,
        warnings: evidence.warnings,
        input_hash,
        state,
        inspected_file_count: evidence.input_files.len() as u64,
        total_bytes: evidence.total_bytes,
        input_files: evidence.input_files,
    })
}
