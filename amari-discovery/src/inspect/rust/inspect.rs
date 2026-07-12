// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Rust source inspection — orchestration and public entry point.
//!
//! Walks a project root for `.rs` files and `README.md`, parses them
//! with `syn`, and extracts Amari API usage evidence, cfg/attribute
//! signals, file classification, and curated vocabulary.
//!
//! # Safety
//!
//! - Read-only: no file writes, no Cargo/rustc/build/network execution.
//! - No symlink following — reuses Task7 nofollow helpers.
//! - Bounded reads with per-file and aggregate limits.
//! - Warnings never expose source text or absolute paths.

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use sha2::{Digest, Sha256};

use crate::error::{DiscoveryError, DiscoveryResult};
use crate::inspect::cargo::CargoInspection;
use crate::inspect::rust::types::{
    RustCfgEvidence, RustCrateAttribute, RustFileKind, RustInspectionWarning, RustSourceInspection,
    RustUsage, VocabularyEvidence,
};
use crate::inspect::snapshot::{InspectionLimit, SnapshotState, SourceLocation};
use crate::inspect::{
    bounded_read, nofollow_open_readonly, BoundedOutcome, InspectionLimits, NofollowResult,
};

use super::parser::{parse_rust_source, LineOffsets};
use super::vocabulary::{extract_comment_segments, scan_vocabulary};

// ============================================================================
// CrateAliasMap — per-package crate name resolution
// ============================================================================

/// Maps Rust source crate identifiers to canonical Cargo package names
/// for a single Cargo package.
#[derive(Clone, Debug)]
pub(super) struct CrateAliasMap {
    pub(super) alias_to_pkg: BTreeMap<String, String>,
}

impl CrateAliasMap {
    fn from_package_deps(deps: &[crate::inspect::AmariDependencyEvidence]) -> Self {
        let mut alias_to_pkg = BTreeMap::new();
        for dep in deps {
            let normalized_alias = dep.alias.replace('-', "_");
            alias_to_pkg.insert(normalized_alias, dep.package_name.clone());
        }
        Self { alias_to_pkg }
    }

    pub(super) fn contains_crate(&self, segment: &str) -> bool {
        self.alias_to_pkg.contains_key(segment)
    }

    pub(super) fn resolve(&self, segment: &str) -> Option<String> {
        self.alias_to_pkg.get(segment).cloned()
    }
}

// ============================================================================
// Package-scoped alias map builder
// ============================================================================

fn build_package_aliases(cargo: &CargoInspection) -> BTreeMap<String, CrateAliasMap> {
    let mut maps = BTreeMap::new();

    maps.insert(
        String::new(),
        CrateAliasMap::from_package_deps(&cargo.root_package.dependencies),
    );

    for member in &cargo.workspace_members {
        let dir = member
            .manifest_path
            .trim_end_matches("/Cargo.toml")
            .trim_end_matches("\\Cargo.toml")
            .to_string();
        maps.insert(dir, CrateAliasMap::from_package_deps(&member.dependencies));
    }

    maps
}

/// Resolve a file's relative path to the package directory it belongs to,
/// using longest component-boundary prefix match over member directories
/// (normalized strings).
///
/// Returns `""` for the root package, or the member directory string.
fn resolve_package_dir<'a>(rel_path: &str, member_dirs: &'a [String]) -> &'a str {
    let mut best: Option<&str> = None;
    let mut best_len = 0;

    for dir in member_dirs {
        if dir.is_empty() {
            continue;
        }

        // Component-boundary prefix: either exact match or the member
        // directory followed by `/`, without allocating a temporary path.
        let is_member_path = rel_path == dir
            || rel_path
                .strip_prefix(dir)
                .is_some_and(|suffix| suffix.starts_with('/'));

        if is_member_path && dir.len() > best_len {
            best = Some(dir.as_str());
            best_len = dir.len();
        }
    }

    best.unwrap_or("")
}

// ============================================================================
// RustFileKind classification
// ============================================================================

/// Classify an `.rs` file path relative to a package directory.
/// `rel_path` is the project-relative path. Classification uses
/// package-relative path while the output retains the project-relative path.
fn classify_file(rel_path: &str, pkg_dir: &str, package_name: &str) -> RustFileKind {
    // Compute package-relative path by stripping the package directory prefix
    let pkg_rel = if pkg_dir.is_empty() {
        rel_path.to_string()
    } else {
        let boundary = format!("{pkg_dir}/");
        rel_path
            .strip_prefix(&boundary)
            .unwrap_or(rel_path)
            .to_string()
    };

    let p = Path::new(&pkg_rel);

    if p.file_name().is_some_and(|n| n == "build.rs")
        && p.parent().map_or(true, |par| par.as_os_str().is_empty())
    {
        return RustFileKind::BuildScript {
            package: package_name.to_string(),
            path: rel_path.to_string(),
        };
    }

    if pkg_rel == "src/lib.rs" {
        return RustFileKind::Library {
            package: package_name.to_string(),
            path: rel_path.to_string(),
        };
    }

    if pkg_rel == "src/main.rs" {
        return RustFileKind::Binary {
            package: package_name.to_string(),
            path: rel_path.to_string(),
        };
    }

    if pkg_rel.starts_with("src/bin/") && pkg_rel.ends_with(".rs") {
        return RustFileKind::Binary {
            package: package_name.to_string(),
            path: rel_path.to_string(),
        };
    }

    if pkg_rel.starts_with("tests/") {
        return RustFileKind::Test {
            package: package_name.to_string(),
            path: rel_path.to_string(),
        };
    }

    if pkg_rel.starts_with("examples/") {
        return RustFileKind::Example {
            package: package_name.to_string(),
            path: rel_path.to_string(),
        };
    }

    if let Some(rest) = pkg_rel.strip_prefix("benches/") {
        // Conventional Cargo bench ROOTS only: `benches/<name>.rs` or
        // `benches/<name>/main.rs`. Nested helper modules (e.g.
        // `benches/<name>/helper.rs`) are NOT bench roots and fall through
        // to `Other` so they are never fabricated as benchmarks.
        let is_root = if rest.ends_with(".rs") && !rest.contains('/') {
            true
        } else if let Some((dir, file)) = rest.rsplit_once('/') {
            file == "main.rs" && !dir.is_empty() && !dir.contains('/')
        } else {
            false
        };
        if is_root {
            return RustFileKind::Bench {
                package: package_name.to_string(),
                path: rel_path.to_string(),
            };
        }
    }

    RustFileKind::Other {
        package: package_name.to_string(),
        path: rel_path.to_string(),
    }
}

// ============================================================================
// Main inspection function
// ============================================================================

/// Inspect Rust source files in a project for Amari API usage.
pub fn inspect_rust_sources(
    root: &Path,
    cargo: &CargoInspection,
    limits: &InspectionLimits,
) -> DiscoveryResult<RustSourceInspection> {
    if !root.is_dir() {
        return Err(DiscoveryError::InspectionFailure(format!(
            "project root is not a directory: {}",
            root.display()
        )));
    }

    let root_canon = root.canonicalize().map_err(|err| {
        DiscoveryError::InspectionFailure(format!("cannot canonicalize project root: {}", err))
    })?;

    let aliases_by_pkg = build_package_aliases(cargo);

    // Build member directories as normalized strings (not PathBuf)
    let member_dirs: Vec<String> = cargo
        .workspace_members
        .iter()
        .map(|m| {
            m.manifest_path
                .trim_end_matches("/Cargo.toml")
                .trim_end_matches("\\Cargo.toml")
                .to_string()
        })
        .collect();

    let start = Instant::now();
    let mut warnings = Vec::new();
    let mut provenance_entries: Vec<(String, Vec<u8>)> = Vec::new();

    let mut usages: Vec<RustUsage> = Vec::new();
    let mut file_kinds: Vec<RustFileKind> = Vec::new();
    let mut crate_attrs: Vec<RustCrateAttribute> = Vec::new();
    let mut cfg_evidence: Vec<RustCfgEvidence> = Vec::new();
    let mut vocabulary: Vec<VocabularyEvidence> = Vec::new();

    let mut file_count: u64 = 0;
    let mut total_bytes: u64 = 0;
    let mut candidate_count: u64 = 0;
    let mut stop_reason: Option<InspectionLimit> = None;
    let mut saw_depth_prune = false;

    let max_depth_usize = usize::try_from(limits.max_traversal_depth).map_err(|_| {
        DiscoveryError::InspectionFailure(
            "max_traversal_depth exceeds platform address size".into(),
        )
    })?;

    let walker = walkdir::WalkDir::new(&root_canon)
        .follow_links(false)
        .sort_by_file_name()
        .max_depth(max_depth_usize)
        .into_iter()
        .filter_entry(|e| {
            if e.depth() == 0 {
                return true;
            }
            if let Some(name) = e.file_name().to_str() {
                !crate::inspect::is_skipped_dir_name(name)
                    && !crate::inspect::is_env_secret_name(name)
            } else {
                !e.file_type().is_dir()
            }
        });

    for entry in walker {
        // Wall-clock check before each entry
        let elapsed = start.elapsed().as_millis() as u64;
        if elapsed >= limits.max_inspection_wall_millis {
            stop_reason = Some(InspectionLimit::WallClock {
                max_millis: limits.max_inspection_wall_millis,
                observed_millis: elapsed,
            });
            break;
        }

        let entry = entry
            .map_err(|_| DiscoveryError::InspectionFailure("cannot read directory entry".into()))?;

        // Depth pruning detection
        if entry.depth() >= max_depth_usize && entry.file_type().is_dir() {
            saw_depth_prune = true;
            continue;
        }

        // --- SYMLINK CHECK FIRST (before is_file) ---
        // Only candidate .rs / README symlinks produce warnings;
        // unrelated symlinks are silently skipped.
        let ft = entry.file_type();
        if ft.is_symlink() {
            let file_name_os = entry.file_name();
            let is_candidate_symlink = file_name_os
                .to_str()
                .is_some_and(|n| n.ends_with(".rs") || n.eq_ignore_ascii_case("readme.md"));
            // Also check non-UTF8 .rs via OsStr extension
            let is_non_utf8_rs_candidate = file_name_os.to_str().is_none() && {
                // Check if the non-UTF8 name ends with ".rs"
                let name_bytes = file_name_os.as_encoded_bytes();
                name_bytes.len() >= 3 && &name_bytes[name_bytes.len() - 3..] == b".rs"
            };
            if is_candidate_symlink || is_non_utf8_rs_candidate {
                match normalize_rel_path(entry.path(), &root_canon) {
                    Ok(rel) => {
                        warnings.push(RustInspectionWarning::SymlinkedFile { path: rel });
                    }
                    Err(_) => {
                        warnings.push(RustInspectionWarning::NonUtf8Path {
                            path_hint: "symlink path unavailable".into(),
                        });
                    }
                }
            }
            continue;
        }

        if !ft.is_file() {
            continue;
        }

        // Only candidate .rs and README.md — identify BEFORE normalization
        // Use OsStr extension check for non-UTF8 .rs files;
        // non-UTF8 non-.rs files are silently skipped without counting.
        let file_name_os = entry.file_name();
        let file_name = file_name_os.to_str();
        let is_rust = file_name.is_some_and(|n| n.ends_with(".rs")) || {
            // Non-UTF8: check if raw bytes end with ".rs"
            let name_bytes = file_name_os.as_encoded_bytes();
            name_bytes.len() >= 3 && &name_bytes[name_bytes.len() - 3..] == b".rs"
        };
        let is_readme = file_name.is_some_and(|n| n.eq_ignore_ascii_case("readme.md"));

        if !is_rust && !is_readme {
            continue;
        }

        // Increment considered count BEFORE normalization
        // (non-UTF8 .rs files consume considered slots)
        candidate_count = candidate_count.saturating_add(1);

        // File-count limit: trigger on considered > max (consistent with Task7)
        if candidate_count > limits.max_inspection_files {
            stop_reason = Some(InspectionLimit::FileCount {
                max: limits.max_inspection_files,
                observed: candidate_count,
            });
            break;
        }

        // Normalized relative path with non-UTF-8 rejection
        let rel_path = match normalize_rel_path(entry.path(), &root_canon) {
            Ok(p) => p,
            Err(_) => {
                // Non-UTF8 path component: warn only for .rs candidates
                if is_rust {
                    warnings.push(RustInspectionWarning::NonUtf8Path {
                        path_hint: "non-UTF-8 path component".into(),
                    });
                }
                continue;
            }
        };

        // Env secret file check (only for UTF-8 names)
        if file_name.is_some_and(crate::inspect::is_env_secret_name) {
            continue;
        }

        // Open with nofollow
        let mut file = match nofollow_open_readonly(entry.path()) {
            Ok(NofollowResult::Opened(f)) => f,
            Ok(NofollowResult::SymlinkOrRace) => {
                warnings.push(RustInspectionWarning::SymlinkedFile { path: rel_path });
                continue;
            }
            Err(_) => {
                warnings.push(RustInspectionWarning::ReadFailure {
                    path: rel_path.clone(),
                    reason: "I/O error opening file".into(),
                });
                continue;
            }
        };

        // Per-file size pre-check using opened-file metadata (not walker entry)
        let metadata = match file.metadata() {
            Ok(m) => m,
            Err(_) => {
                warnings.push(RustInspectionWarning::ReadFailure {
                    path: rel_path.clone(),
                    reason: "cannot stat opened file".into(),
                });
                continue;
            }
        };
        let file_size = metadata.len();
        if file_size > limits.max_per_file_bytes {
            warnings.push(RustInspectionWarning::OversizedFile {
                path: rel_path.clone(),
                size: file_size,
                limit: limits.max_per_file_bytes,
            });
            continue;
        }

        // Bounded read (authoritative)
        let remaining = limits.max_inspection_bytes.saturating_sub(total_bytes);
        let content = match bounded_read(&mut file, limits.max_per_file_bytes, remaining) {
            Ok(BoundedOutcome::Accepted(bytes)) => bytes,
            Ok(BoundedOutcome::PerFileExceeded) => {
                warnings.push(RustInspectionWarning::OversizedFile {
                    path: rel_path,
                    size: file_size,
                    limit: limits.max_per_file_bytes,
                });
                continue;
            }
            Err(_) => {
                warnings.push(RustInspectionWarning::ReadFailure {
                    path: rel_path.clone(),
                    reason: "I/O error reading file".into(),
                });
                continue;
            }
        };

        // Checked aggregate byte arithmetic
        let content_len = content.len() as u64;
        let new_total = total_bytes
            .checked_add(content_len)
            .ok_or_else(|| DiscoveryError::InspectionFailure("byte total overflow".into()))?;
        if new_total > limits.max_inspection_bytes {
            stop_reason = Some(InspectionLimit::TotalBytes {
                max: limits.max_inspection_bytes,
                observed: total_bytes,
            });
            break;
        }

        let content_hash = hex::encode(Sha256::digest(&content));

        // Track provenance for ALL accepted files (including invalid UTF-8)
        provenance_entries.push((rel_path.clone(), content.clone()));
        file_count = file_count.saturating_add(1);
        total_bytes = new_total;

        // Determine which package this file belongs to (BEFORE UTF-8 parsing,
        // so invalid UTF-8 .rs still gets its package-scoped RustFileKind)
        let pkg_dir = resolve_package_dir(&rel_path, &member_dirs);
        let pkg_name = if pkg_dir.is_empty() {
            cargo.root_package.name.as_str()
        } else {
            cargo
                .workspace_members
                .iter()
                .find(|m| {
                    let d = m
                        .manifest_path
                        .trim_end_matches("/Cargo.toml")
                        .trim_end_matches("\\Cargo.toml");
                    d == pkg_dir
                })
                .map(|m| m.name.as_str())
                .ok_or_else(|| {
                    DiscoveryError::InspectionFailure(format!(
                        "no member found for package directory '{pkg_dir}'"
                    ))
                })?
        };

        // Classify .rs file kind BEFORE UTF-8 validation
        // (invalid UTF-8 .rs still receives its package-scoped RustFileKind)
        if is_rust {
            file_kinds.push(classify_file(&rel_path, pkg_dir, pkg_name));
        }

        // Validate UTF-8
        let source = match std::str::from_utf8(&content) {
            Ok(s) => s,
            Err(_) => {
                // Invalid UTF-8: included in input_files/input_hash/count/bytes,
                // has its RustFileKind, emit InvalidUtf8Source warning, skip parsing
                warnings.push(RustInspectionWarning::InvalidUtf8Source { path: rel_path });
                continue;
            }
        };

        // Get the package's alias map — no fallback from known member to root
        let aliases = aliases_by_pkg.get(pkg_dir).ok_or_else(|| {
            DiscoveryError::InspectionFailure(format!(
                "no alias map for package directory '{pkg_dir}'"
            ))
        })?;

        if is_rust {
            match parse_rust_source(source, &content_hash, &rel_path, aliases) {
                Ok(parsed) => {
                    usages.extend(parsed.usages);
                    cfg_evidence.extend(parsed.cfg_evidence);

                    // Crate attributes with source locations
                    let line_offsets = LineOffsets::from_source(source);
                    for (attr_name, attr_span) in &parsed.crate_attrs {
                        let source_loc =
                            find_attr_location_from_span(*attr_span, &content_hash, &rel_path);
                        crate_attrs.push(RustCrateAttribute {
                            path: rel_path.clone(),
                            attribute: attr_name.clone(),
                            source: source_loc,
                        });
                    }

                    // Vocabulary from individually anchored doc segments + lexical comments
                    let comment_segments = extract_comment_segments(source);
                    let vocab = scan_vocabulary(
                        &parsed.doc_segments,
                        &comment_segments,
                        None, // no README for .rs files
                        &rel_path,
                        &content_hash,
                        &line_offsets,
                    );
                    vocabulary.extend(vocab);
                }
                Err(err) => {
                    warnings.push(RustInspectionWarning::MalformedSource {
                        path: rel_path.clone(),
                        reason: err.reason,
                        line: err.line,
                        column: err.column,
                        content_hash: err.content_hash,
                    });
                }
            }
        } else if is_readme {
            let line_offsets = LineOffsets::from_source(source);
            let vocab = scan_vocabulary(
                &[],          // no doc segments
                &[],          // no comment segments
                Some(source), // scan README text directly
                &rel_path,
                &content_hash,
                &line_offsets,
            );
            vocabulary.extend(vocab);
        }
    }

    // Sort and dedup vocabulary by path + term + source BEFORE truncation
    // so pre-cap unique total is correctly reported
    vocabulary.sort_by(|a, b| {
        a.path.cmp(&b.path).then(a.term.cmp(&b.term)).then(
            a.source
                .as_ref()
                .map(|s| (s.line, s.column))
                .cmp(&b.source.as_ref().map(|s| (s.line, s.column))),
        )
    });
    vocabulary.dedup_by(|a, b| {
        a.path == b.path
            && a.term == b.term
            && a.source.as_ref().map(|s| (s.line, s.column))
                == b.source.as_ref().map(|s| (s.line, s.column))
    });

    // Vocabulary truncation AFTER dedup — report unique pre-cap total
    const MAX_VOCAB_TOTAL: usize = 200;
    if vocabulary.len() > MAX_VOCAB_TOTAL {
        warnings.push(RustInspectionWarning::VocabularyTruncated {
            total: vocabulary.len(),
            cap: MAX_VOCAB_TOTAL,
        });
        vocabulary.truncate(MAX_VOCAB_TOTAL);
    }

    // Sort and dedup crate attributes by path + attribute + source
    crate_attrs.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.attribute.cmp(&b.attribute))
            .then(
                a.source
                    .as_ref()
                    .map(|s| (s.line, s.column))
                    .cmp(&b.source.as_ref().map(|s| (s.line, s.column))),
            )
    });
    crate_attrs.dedup_by(|a, b| {
        a.path == b.path
            && a.attribute == b.attribute
            && a.source.as_ref().map(|s| (s.line, s.column))
                == b.source.as_ref().map(|s| (s.line, s.column))
    });

    // Sort file kinds
    file_kinds.sort_by(|a, b| {
        let pa = file_kind_path(a);
        let pb = file_kind_path(b);
        pa.cmp(pb)
    });

    // Add limit-exceeded warning if needed
    if let Some(ref limit) = stop_reason {
        warnings.push(RustInspectionWarning::LimitExceeded {
            limit: limit.clone(),
        });
    }

    // Compute input hash (over accepted files, including invalid UTF-8)
    let mut sorted_entries: Vec<&(String, Vec<u8>)> = provenance_entries.iter().collect();
    sorted_entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (path, content) in &sorted_entries {
        hasher.update((path.len() as u32).to_le_bytes());
        hasher.update(path.as_bytes());
        hasher.update((content.len() as u64).to_le_bytes());
        hasher.update(content);
    }
    let input_hash = hex::encode(hasher.finalize());

    // Build input file list (one SourceLocation per accepted file, including invalid UTF-8)
    let input_files: Vec<SourceLocation> = sorted_entries
        .iter()
        .map(|(path, content)| SourceLocation {
            path: (*path).clone(),
            line: None,
            column: None,
            content_hash: hex::encode(Sha256::digest(content)),
        })
        .collect();

    let final_state = if let Some(limit) = stop_reason {
        SnapshotState::LimitExceeded { limit }
    } else if saw_depth_prune {
        SnapshotState::LimitExceeded {
            limit: InspectionLimit::TraversalDepth {
                max: limits.max_traversal_depth,
            },
        }
    } else {
        SnapshotState::Complete
    };

    Ok(RustSourceInspection {
        usages,
        file_kinds,
        crate_attributes: crate_attrs,
        cfg_evidence,
        vocabulary,
        warnings,
        input_hash,
        state: final_state,
        inspected_file_count: file_count,
        total_bytes,
        input_files,
    })
}

// ============================================================================
// Helpers
// ============================================================================

/// Normalize a relative path from an entry path and root, rejecting non-UTF-8.
fn normalize_rel_path(entry_path: &Path, root: &Path) -> Result<String, ()> {
    let rel = entry_path.strip_prefix(root).map_err(|_| ())?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            std::path::Component::Normal(os_str) => match os_str.to_str() {
                Some(s) => parts.push(s),
                None => return Err(()),
            },
            _ => return Err(()),
        }
    }
    if parts.is_empty() {
        Ok(".".into())
    } else {
        Ok(parts.join("/"))
    }
}

fn file_kind_path(kind: &RustFileKind) -> &str {
    match kind {
        RustFileKind::Library { path, .. }
        | RustFileKind::Binary { path, .. }
        | RustFileKind::Test { path, .. }
        | RustFileKind::Example { path, .. }
        | RustFileKind::Bench { path, .. }
        | RustFileKind::BuildScript { path, .. }
        | RustFileKind::Other { path, .. } => path,
    }
}

/// Try to find the 1-based line/column of a crate attribute from its span.
fn find_attr_location_from_span(
    span: proc_macro2::Span,
    content_hash: &str,
    path: &str,
) -> Option<SourceLocation> {
    let start = span.start();
    if start.line == 0 {
        return None;
    }
    Some(SourceLocation {
        path: path.to_string(),
        line: Some(start.line as u32),
        column: Some((start.column + 1) as u32),
        content_hash: content_hash.to_string(),
    })
}
