// SPDX-License-Identifier: MIT OR Apache-2.0

//! Exported declarative and procedural macro cataloguing for catalog source
//! generation.
//!
//! [`macro_catalog`] takes a [`ModuleGraph`] (Task 5B1) and a
//! [`WorkspaceInventory`] (Task 5A2) to extract every exported declarative
//! and procedural macro in a target source tree. Declarative macros with
//! `#[macro_export]` are recorded at the crate root regardless of their
//! declaration module. Private `macro_rules!` declarations are excluded.
//! Procedural macros (`#[proc_macro]`, `#[proc_macro_attribute]`, and
//! `#[proc_macro_derive]`) are recorded only when the target is a proc-macro
//! library (crate types include `proc-macro`). Local `pub use` re-exports of
//! known macros are resolved; external re-exports become sorted, deduplicated
//! warnings.
//!
//! # Declaration model
//!
//! - **Declarative macros**: `#[macro_export] macro_rules!` declarations are
//!   indexed at the canonical path `crate::<name>`. The normalized declaration
//!   token surface (matchers and transcribers, whitespace-collapsed) is
//!   preserved without expansion. Private `macro_rules!` (no `#[macro_export]`)
//!   is excluded.
//!
//! - **Procedural macros**: Only extracted when the package target's
//!   `crate_types` includes `proc-macro`. `#[proc_macro]` and
//!   `#[proc_macro_attribute]` record the function name as the export.
//!   `#[proc_macro_derive(Name, attributes(…))]` records the derive `Name`
//!   (not the function ident) as the export, with sorted, deduplicated helper
//!   attributes. The normalized function signature (vis + fn sig, no body) is
//!   stored alongside.
//!
//! - **Re-exports**: `pub use` paths that resolve to a known exported macro
//!   are recorded as alias records whose `source` points at the original
//!   declaration, with the alias path pinned to the re-export site. External
//!   re-exports (whose first segment is an external crate) produce
//!   [`MacroWarning`]s rather than fabricated local definitions.
//!
//! # `#[cfg]` deferral
//!
//! Configuration evaluation is deferred. Two `#[macro_export]` variants of
//! the same name gated by mutually exclusive `#[cfg]` annotations are retained
//! as distinct records with different source files so Task 5C1 can attach
//! gates later.
//!
//! # Limitations
//!
//! - Macro invocations are never expanded; only declarations are recorded.
//! - Macros defined inside function bodies are not detected by this syntactic
//!   parser (syn ignores them).
//! - Generated WASM/TypeScript types (e.g. from `wasm-bindgen`) are not
//!   parsed — those are Task 5C3.
//! - `macro_rules!` that uses a `2015`-style `#[macro_use]` import from an
//!   external crate is not traced; such imports are reported as external
//!   re-export warnings.
//! - `pub use` paths are offline-resolution only: without running Cargo or
//!   rustc we cannot distinguish type/value/macro namespaces. Warnings about
//!   unresolved re-exports are therefore *potential* macro-namespace
//!   re-exports; they are suppressed only when the path conclusively resolves
//!   to a known non-macro local item (struct, enum, fn, etc.).

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use quote::ToTokens;
use syn::{Item, ItemFn, Meta, UseTree, Visibility};

use crate::{DiscoveryError, DiscoveryResult};

use super::inventory::{TargetKind, WorkspaceInventory};
use super::modules::ModuleGraph;

// ---------------------------------------------------------------------------
// Public API types
// ---------------------------------------------------------------------------

/// Kinds of macros that can be catalogued.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MacroKind {
    /// `#[macro_export] macro_rules!` declarative macro.
    Declarative,
    /// `#[proc_macro]` function-like procedural macro.
    ProcMacro,
    /// `#[proc_macro_attribute]` attribute procedural macro.
    ProcMacroAttribute,
    /// `#[proc_macro_derive(Name, attributes(…))]` derive procedural macro.
    ProcMacroDerive,
}

/// Source identity of a single macro declaration.
///
/// `module` is the canonical module path where the macro was declared,
/// `source_path` is the package-relative file that hosts the declaration
/// (distinguishing cfg-deferred variants in different source files),
/// and `ordinal` is the 0-based file-order declaration index within that file.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MacroSource {
    /// Canonical module path, e.g. `crate` for `#[macro_export]`.
    pub module: String,
    /// Package-relative source file path, e.g. `src/lib.rs`.
    pub source_path: String,
    /// Declaration ordinal within `source_path` (0-based file order).
    pub ordinal: u64,
}

/// One macro catalog record.
///
/// Because cfg evaluation is deferred and because re-export aliases produce
/// multiple records sharing a declaration source, more than one record may
/// share the same `name` or `source`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacroRecord {
    /// Exported crate-qualified path, e.g. `crate::geo`.
    pub path: String,
    /// Exported macro name.
    pub name: String,
    /// Macro kind.
    pub kind: MacroKind,
    /// Local source identity of the backing declaration.
    pub source: MacroSource,
    /// Normalized declaration signature.
    pub signature: String,
    /// For proc-macro-derive, sorted deduplicated helper attributes.
    pub helpers: Vec<String>,
}

/// Why a macro declaration or re-export triggered a warning.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MacroWarningReason {
    /// A `pub use` whose first segment is an external crate.
    ExternalReexport {
        /// The unresolved external path, e.g. `serde::Serialize`.
        target: String,
    },
    /// A proc-macro attribute was malformed (e.g. `#[proc_macro_derive]` with
    /// no derive name).
    MalformedProcMacro {
        /// Human-readable detail about what went wrong.
        detail: String,
    },
    /// A local `pub use` that did not resolve to any known macro.
    UnresolvedReexport {
        /// Human-readable detail about the failed resolution.
        detail: String,
    },
}

/// A warning about a macro declaration or re-export that could not be resolved.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MacroWarning {
    /// Canonical module path where the issue was detected.
    pub declared_in: String,
    /// Would-be exported path at this site.
    pub path: String,
    /// Why this is a warning.
    pub reason: MacroWarningReason,
}

/// Complete macro index for one target source tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacroCatalog {
    /// Every macro record, sorted by `(path, name, source)` with duplicates
    /// removed.
    pub records: Vec<MacroRecord>,
    /// Sorted, deduplicated context warnings.
    pub warnings: Vec<MacroWarning>,
}

impl MacroCatalog {
    /// Finds a record by exported name.
    pub fn find(&self, name: &str) -> Option<&MacroRecord> {
        self.records.iter().find(|r| r.name == name)
    }
}

/// Extracts exported macros from a target source tree.
///
/// `graph` must have been built with [`module_graph`](super::modules::module_graph)
/// for the same `package_root` and `source_path`. `inventory` must have been
/// built with [`inventory_workspace`](super::inventory::inventory_workspace).
/// The function re-reads each file-backed module source to collect macro
/// declarations and `use` statements; inline modules are indexed from their
/// host file. It performs no Cargo, rustc, or network access.
///
/// # Errors
///
/// Returns [`DiscoveryError::CatalogCorruption`] when a recorded source file
/// cannot be read or parsed, or when the requested package is not found in the
/// inventory.
pub fn macro_catalog(
    graph: &ModuleGraph,
    inventory: &WorkspaceInventory,
    package_name: &str,
    package_root: &Path,
) -> DiscoveryResult<MacroCatalog> {
    let canonical_root = fs::canonicalize(package_root).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "cannot resolve package root {}: {error}",
            package_root.display()
        ))
    })?;

    let package = inventory.package(package_name).ok_or_else(|| {
        DiscoveryError::CatalogCorruption(format!(
            "package `{package_name}` not found in inventory"
        ))
    })?;

    let is_proc_macro_target = package
        .targets
        .iter()
        .any(|t| t.kind == TargetKind::Library && t.crate_types.contains(&"proc-macro".to_owned()));

    // Phase 1: index all macro declarations (macro_export and proc_macro fns).
    let mut index = BTreeMap::new(); // (module, name) -> Vec<(source_path, ordinal, MacroRecord)>
    let mut warnings = BTreeSet::new();

    // Build local-module-name scope from the module graph for path classification.
    let local_module_names: BTreeSet<String> = graph
        .modules
        .iter()
        .map(|mr| mr.path.rsplit("::").next().unwrap_or(&mr.path).to_owned())
        .collect();

    // Build per-(file,module) item-name sets for conclusive non-macro suppression.
    // Keyed by (source_path, canonical_module), each entry is the set of declared
    // item names (structs, enums, fns, consts, traits, types, statics, unions).
    let mut local_items: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();

    for module_record in &graph.modules {
        let Some(source_rel) = &module_record.source_path else {
            continue;
        };
        let file = canonical_root.join(source_rel);
        if !file.is_file() {
            continue;
        }
        let source = fs::read_to_string(&file).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!(
                "cannot read module {}: {error}",
                file.display()
            ))
        })?;
        let ast = syn::parse_file(&source).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!("cannot parse {}: {error}", file.display()))
        })?;

        let canonical = &module_record.path;
        index_macros_in_file(
            canonical,
            source_rel.as_str(),
            &ast.items,
            is_proc_macro_target,
            &mut index,
            &mut warnings,
        );
        index_item_names(canonical, source_rel.as_str(), &ast.items, &mut local_items);
    }

    // Phase 2: collect declared macro records.
    let mut records: Vec<MacroRecord> = Vec::new();
    for decls in index.values() {
        for (_src, _ordinal, record) in decls {
            records.push(record.clone());
        }
    }

    // Phase 3: resolve re-exports.
    let known: BTreeSet<String> = records.iter().map(|r| r.name.clone()).collect();
    for module_record in &graph.modules {
        let Some(source_rel) = &module_record.source_path else {
            continue;
        };
        let file = canonical_root.join(source_rel);
        if !file.is_file() {
            continue;
        }
        let source = fs::read_to_string(&file).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!(
                "cannot read module {}: {error}",
                file.display()
            ))
        })?;
        let ast = syn::parse_file(&source).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!("cannot parse {}: {error}", file.display()))
        })?;

        resolve_reexports(
            &ast.items,
            &module_record.path,
            &known,
            &index,
            &local_module_names,
            &local_items,
            graph,
            &mut records,
            &mut warnings,
        );
    }

    // Deterministic ordering and deduplication.
    let mut seen: BTreeSet<(String, String, MacroKind, MacroSource)> = BTreeSet::new();
    records.retain(|record| {
        let key = (
            record.path.clone(),
            record.name.clone(),
            record.kind,
            record.source.clone(),
        );
        seen.insert(key)
    });
    records.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.kind.cmp(&right.kind))
            .then_with(|| left.source.cmp(&right.source))
    });

    Ok(MacroCatalog {
        records,
        warnings: warnings.into_iter().collect(),
    })
}

// ---------------------------------------------------------------------------
// Phase 1: macro declaration indexing
// ---------------------------------------------------------------------------

/// Indexes macro declarations from a file's items, recursing into inline
/// modules at arbitrary depth with a single shared file-level ordinal.
fn index_macros_in_file(
    canonical: &str,
    source_rel: &str,
    items: &[Item],
    is_proc_macro_target: bool,
    index: &mut BTreeMap<String, Vec<(String, u64, MacroRecord)>>,
    warnings: &mut BTreeSet<MacroWarning>,
) {
    let mut ordinal: u64 = 0;
    index_items_recursive(
        canonical,
        source_rel,
        items,
        is_proc_macro_target,
        index,
        warnings,
        &mut ordinal,
    );
}

/// Recursive helper that walks items and inline modules sharing a single
/// `ordinal` counter.
fn index_items_recursive(
    canonical: &str,
    source_rel: &str,
    items: &[Item],
    is_proc_macro_target: bool,
    index: &mut BTreeMap<String, Vec<(String, u64, MacroRecord)>>,
    warnings: &mut BTreeSet<MacroWarning>,
    ordinal: &mut u64,
) {
    for item in items {
        match item {
            Item::Macro(item_macro) => {
                let Some(ident) = &item_macro.ident else {
                    *ordinal += 1;
                    continue;
                };
                let name = ident.to_string();
                if has_attr(&item_macro.attrs, "macro_export") {
                    let path = format!("crate::{name}");
                    let signature = normalize(item_macro.to_token_stream());
                    let source = MacroSource {
                        module: "crate".to_owned(),
                        source_path: source_rel.to_owned(),
                        ordinal: *ordinal,
                    };
                    let record = MacroRecord {
                        path,
                        name: name.clone(),
                        kind: MacroKind::Declarative,
                        source,
                        signature,
                        helpers: Vec::new(),
                    };
                    let key = format!("crate::{name}");
                    index
                        .entry(key)
                        .or_default()
                        .push((source_rel.to_owned(), *ordinal, record));
                }
                // Private macro_rules! still consumes an ordinal slot.
                *ordinal += 1;
            }

            Item::Fn(item_fn) if is_proc_macro_target => {
                let start_ordinal = *ordinal;
                index_proc_macro_fns(
                    canonical,
                    source_rel,
                    item_fn,
                    start_ordinal,
                    index,
                    warnings,
                );
                *ordinal += 1;
            }

            Item::Mod(item_mod) => {
                if let Some((_, content)) = &item_mod.content {
                    let child_canonical = format!("{canonical}::{}", item_mod.ident);
                    index_items_recursive(
                        &child_canonical,
                        source_rel,
                        content,
                        is_proc_macro_target,
                        index,
                        warnings,
                        ordinal,
                    );
                }
                // Non-inline modules and inline mod decls each consume a slot.
                *ordinal += 1;
            }

            _ => {
                *ordinal += 1;
            }
        }
    }
}

/// Indexes all proc-macro records from a single function, handling multiple
/// `#[proc_macro_derive]` attributes on the same function (Issue 4).
fn index_proc_macro_fns(
    canonical: &str,
    source_rel: &str,
    item_fn: &ItemFn,
    ordinal: u64,
    index: &mut BTreeMap<String, Vec<(String, u64, MacroRecord)>>,
    warnings: &mut BTreeSet<MacroWarning>,
) {
    let fn_name = item_fn.sig.ident.to_string();
    let is_public = is_public(&item_fn.vis);

    if has_attr(&item_fn.attrs, "proc_macro") {
        if is_public {
            let record = macro_record_for_fn(
                canonical,
                source_rel,
                ordinal,
                &fn_name,
                MacroKind::ProcMacro,
                item_fn,
                &[],
            );
            index.entry(format!("crate::{fn_name}")).or_default().push((
                source_rel.to_owned(),
                ordinal,
                record,
            ));
        }
        return;
    }

    if has_attr(&item_fn.attrs, "proc_macro_attribute") {
        if is_public {
            let record = macro_record_for_fn(
                canonical,
                source_rel,
                ordinal,
                &fn_name,
                MacroKind::ProcMacroAttribute,
                item_fn,
                &[],
            );
            index.entry(format!("crate::{fn_name}")).or_default().push((
                source_rel.to_owned(),
                ordinal,
                record,
            ));
        }
        return;
    }

    // Handle zero, one, or many #[proc_macro_derive] attributes.
    let derive_results = extract_derive_names_and_helpers(&item_fn.attrs);
    for result in derive_results {
        match result {
            Ok((derive_name, helpers)) => {
                if is_public {
                    let record = macro_record_for_fn(
                        canonical,
                        source_rel,
                        ordinal,
                        &derive_name,
                        MacroKind::ProcMacroDerive,
                        item_fn,
                        &helpers,
                    );
                    index
                        .entry(format!("crate::{derive_name}"))
                        .or_default()
                        .push((source_rel.to_owned(), ordinal, record));
                } else {
                    warnings.insert(MacroWarning {
                        declared_in: canonical.to_owned(),
                        path: format!("crate::{fn_name}"),
                        reason: MacroWarningReason::MalformedProcMacro {
                            detail: format!(
                                "#[proc_macro_derive({derive_name})] on non-pub fn {fn_name} \
                                 is not visible — derive records are only emitted for `pub fn`"
                            ),
                        },
                    });
                }
            }
            Err(detail) => {
                warnings.insert(MacroWarning {
                    declared_in: canonical.to_owned(),
                    path: format!("crate::{fn_name}"),
                    reason: MacroWarningReason::MalformedProcMacro { detail },
                });
            }
        }
    }
}

fn macro_record_for_fn(
    canonical: &str,
    source_rel: &str,
    ordinal: u64,
    name: &str,
    kind: MacroKind,
    item_fn: &ItemFn,
    helpers: &[String],
) -> MacroRecord {
    let path = format!("crate::{name}");
    let signature = fn_sig_head(&item_fn.vis, &item_fn.sig);
    let source = MacroSource {
        module: canonical.to_owned(),
        source_path: source_rel.to_owned(),
        ordinal,
    };
    MacroRecord {
        path,
        name: name.to_owned(),
        kind,
        source,
        signature,
        helpers: helpers.to_vec(),
    }
}

// ---------------------------------------------------------------------------
// Phase 3: re-export resolution
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn resolve_reexports(
    items: &[Item],
    canonical: &str,
    known: &BTreeSet<String>,
    index: &BTreeMap<String, Vec<(String, u64, MacroRecord)>>,
    local_module_names: &BTreeSet<String>,
    local_items: &BTreeMap<(String, String), BTreeSet<String>>,
    graph: &ModuleGraph,
    records: &mut Vec<MacroRecord>,
    warnings: &mut BTreeSet<MacroWarning>,
) {
    for item in items {
        if let Item::Use(item_use) = item {
            if !is_public(&item_use.vis) {
                continue;
            }
            for entry in expand_use_tree(&item_use.tree) {
                if entry.glob {
                    continue;
                }
                // `as _` discards the import — no record, no warning (Bug 2).
                if entry.local_name.as_deref() == Some("_") {
                    continue;
                }
                let target_segments = &entry.path_segments;
                let alias_name = entry.local_name.as_deref();

                // Check if this path conclusively resolves to a known non-macro
                // local item — if so, suppress entirely (Issue 5, Bug 3/4).
                // Must run BEFORE the external-re-export check because
                // single-segment paths like `pub use ANSWER` are local, not
                // external. The canonical resolver returns `None` for genuinely
                // external paths (e.g. `serde::Serialize` where `serde` is not
                // a child module), so they fall through correctly.
                let conclusive_non_macro = is_conclusively_non_macro_local_item(
                    canonical,
                    target_segments,
                    alias_name,
                    graph,
                    local_items,
                );
                if conclusive_non_macro {
                    continue;
                }

                // Determine if this is an external re-export.
                // It is external only when the first segment is NOT crate/self/super,
                // NOT a known macro, and NOT a local module name.
                let first_is_external = !target_segments.is_empty()
                    && !matches!(
                        target_segments[0].as_str(),
                        "crate" | "$crate" | "self" | "super"
                    )
                    && known.get(&target_segments[0]).is_none()
                    && !local_module_names.contains(&target_segments[0]);

                if first_is_external {
                    let target_str = target_segments.join("::");
                    let export_path = match alias_name {
                        Some(name) => format!("{canonical}::{name}"),
                        None => format!("{canonical}::{target_str}"),
                    };
                    warnings.insert(MacroWarning {
                        declared_in: canonical.to_owned(),
                        path: export_path,
                        reason: MacroWarningReason::ExternalReexport { target: target_str },
                    });
                    continue;
                }

                // Try to resolve local re-export.
                let target_name = if target_segments.len() == 1 {
                    Some(target_segments[0].as_str())
                } else if target_segments.len() == 2
                    && matches!(target_segments[0].as_str(), "crate" | "$crate")
                {
                    Some(target_segments[1].as_str())
                } else {
                    None
                };

                if let Some(resolved_name) = target_name {
                    if known.contains(resolved_name) {
                        let export_name = alias_name.unwrap_or(resolved_name);
                        let key = format!("crate::{resolved_name}");
                        if let Some(decls) = index.get(&key) {
                            for (_src_path, _ordinal, orig_record) in decls {
                                let mut re_record = orig_record.clone();
                                re_record.path = format!("{canonical}::{export_name}");
                                re_record.name = export_name.to_owned();
                                records.push(re_record);
                            }
                        }
                    } else {
                        let export_name = alias_name.unwrap_or(resolved_name);
                        warnings.insert(MacroWarning {
                            declared_in: canonical.to_owned(),
                            path: format!("{canonical}::{export_name}"),
                            reason: MacroWarningReason::UnresolvedReexport {
                                detail: format!(
                                    "`{resolved_name}` does not name a known exported macro"
                                ),
                            },
                        });
                    }
                } else {
                    let export_name =
                        alias_name.unwrap_or(&target_segments[target_segments.len() - 1]);
                    let last = target_segments.last().map(String::as_str).unwrap_or("");
                    if known.contains(last) {
                        let key = format!("crate::{last}");
                        if let Some(decls) = index.get(&key) {
                            for (_, _, orig_record) in decls {
                                let mut re_record = orig_record.clone();
                                re_record.path = format!("{canonical}::{export_name}");
                                re_record.name = export_name.to_owned();
                                records.push(re_record);
                            }
                        }
                    } else {
                        warnings.insert(MacroWarning {
                            declared_in: canonical.to_owned(),
                            path: format!("{canonical}::{export_name}"),
                            reason: MacroWarningReason::UnresolvedReexport {
                                detail: format!(
                                    "could not resolve `{}` to a known exported macro",
                                    target_segments.join("::")
                                ),
                            },
                        });
                    }
                }
            }
        }
        // Recurse into inline modules.
        if let Item::Mod(item_mod) = item {
            if let Some((_, content)) = &item_mod.content {
                let child_canonical = format!("{canonical}::{}", item_mod.ident);
                resolve_reexports(
                    content,
                    &child_canonical,
                    known,
                    index,
                    local_module_names,
                    local_items,
                    graph,
                    records,
                    warnings,
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Attribute helpers
// ---------------------------------------------------------------------------

/// Checks whether `attrs` contains an attribute with the given name.
fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|seg| seg.ident == name)
    })
}

/// Extracts every `#[proc_macro_derive]` attribute from `attrs`, returning a
/// result per attribute.
///
/// Each `Ok` contains `(derive_name, sorted_deduped_helpers)`. Each `Err`
/// describes a malformed attribute. The caller emits one record per `Ok` and
/// one warning per `Err`.
fn extract_derive_names_and_helpers(
    attrs: &[syn::Attribute],
) -> Vec<Result<(String, Vec<String>), String>> {
    let mut results = Vec::new();
    for attr in attrs {
        if !attr
            .path()
            .segments
            .last()
            .is_some_and(|seg| seg.ident == "proc_macro_derive")
        {
            continue;
        }
        match &attr.meta {
            Meta::List(list) => {
                let s = normalize(list.tokens.clone());
                match parse_derive_args_from_normalized(&s) {
                    Ok((derive_name, helpers)) => {
                        let mut sorted = helpers;
                        sorted.sort();
                        sorted.dedup();
                        results.push(Ok((derive_name, sorted)));
                    }
                    Err(detail) => results.push(Err(detail)),
                }
            }
            _ => results.push(Err("proc_macro_derive requires list form".to_string())),
        }
    }
    results
}

/// Parses a normalized `proc_macro_derive` argument string into `(name, helpers)`.
///
/// The input is a whitespace-normalized token string, e.g.:
/// `"Name , attributes ( helper_a , helper_b )"`
fn parse_derive_args_from_normalized(s: &str) -> Result<(String, Vec<String>), String> {
    // Extract the derive name: first non-comma token/group.
    // Strategy: find the first token before a `,`.
    let name_end = s.find(',').unwrap_or(s.len());
    let name_part = s[..name_end].trim().to_string();
    if name_part.is_empty() {
        return Err("no derive name found".to_string());
    }
    let derive_name = name_part
        .split_whitespace()
        .next()
        .unwrap_or(&name_part)
        .to_string();

    // Extract helper attributes: look for `attributes (`.
    let rest = &s[name_end..];
    let attrs_start = match rest.find("attributes (") {
        Some(pos) => name_end + pos + "attributes (".len(),
        None => return Ok((derive_name, Vec::new())),
    };
    let inner_str = &s[attrs_start..];
    // Find the matching closing paren.
    let mut depth = 1u32;
    let mut end = 0;
    for (i, ch) in inner_str.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }
    if end == 0 {
        return Err("unterminated attributes(...)".to_string());
    }
    let inner = &inner_str[..end];
    let helpers: Vec<String> = inner
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    // Validate each helper looks like a valid ident.
    for h in &helpers {
        if h.is_empty() || !h.chars().all(|c| c.is_alphanumeric() || c == '_') {
            return Err(format!("invalid helper attribute name: `{h}`"));
        }
    }
    Ok((derive_name, helpers))
}

// ---------------------------------------------------------------------------
// Item-name indexing (Issue 5: conclusive non-macro suppression)
// ---------------------------------------------------------------------------

/// Recursively indexes item names from source items into `local_items` keyed
/// by `(source_path, canonical_module)`. This provides conclusive knowledge that
/// a name is a non-macro local item (struct, enum, fn, const, etc.).
fn index_item_names(
    canonical: &str,
    source_rel: &str,
    items: &[Item],
    local_items: &mut BTreeMap<(String, String), BTreeSet<String>>,
) {
    for item in items {
        let name = match item {
            Item::Struct(s) => Some(s.ident.to_string()),
            Item::Enum(e) => Some(e.ident.to_string()),
            Item::Union(u) => Some(u.ident.to_string()),
            Item::Fn(f) => Some(f.sig.ident.to_string()),
            Item::Const(c) => Some(c.ident.to_string()),
            Item::Static(s) => Some(s.ident.to_string()),
            Item::Trait(t) => Some(t.ident.to_string()),
            Item::Type(t) => Some(t.ident.to_string()),
            _ => None,
        };
        if let Some(name) = name {
            local_items
                .entry((source_rel.to_owned(), canonical.to_owned()))
                .or_default()
                .insert(name);
        }
        // Recurse into inline modules.
        if let Item::Mod(item_mod) = item {
            if let Some((_, content)) = &item_mod.content {
                let child_canonical = format!("{canonical}::{}", item_mod.ident);
                index_item_names(&child_canonical, source_rel, content, local_items);
            }
        }
    }
}

/// Returns `true` when a `pub use` path conclusively resolves to a known
/// non-macro local item, so the macro warning can be suppressed (Issue 5, Bug 3/4).
///
/// Uses a small canonical path resolver that converts any local path
/// (`crate::a::b::Type`, `self::Type`, `super::super::Type`, bare `Type`,
/// `module::Type`, etc.) into a canonical `(module, item_name)` pair by
/// consulting the [`ModuleGraph`] for valid module descent and parent links.
/// Only exact canonical-module + item-name matches suppress; ambiguous or
/// unresolved paths remain warnings. This replaces the prior length-gated and
/// loose `ends_with` approach (Bug 4).
fn is_conclusively_non_macro_local_item(
    current_module: &str,
    target_segments: &[String],
    _alias_name: Option<&str>,
    graph: &ModuleGraph,
    local_items: &BTreeMap<(String, String), BTreeSet<String>>,
) -> bool {
    let resolved = resolve_canonical_item_path(current_module, target_segments, graph);
    if let Some((canonical_module, item_name)) = resolved {
        for ((_source_rel, module), names) in local_items {
            if module == &canonical_module && names.contains(&item_name) {
                return true;
            }
        }
    }
    false
}

/// Resolves a local-use path to a canonical `(module_path, item_name)` using
/// the module graph.
///
/// Rules:
/// - `crate` / `$crate` → reset base to `crate`.
/// - `self` → stay at current module.
/// - `super` → move to parent (via graph, falling back to segment trimming).
/// - Otherwise → descend into a child module whose canonical path is exactly
///   `{current_module}::{segment}`, validated by [`ModuleGraph::find`].
/// - The final segment is the item name; all preceding segments must form
///   valid module descents.
/// - Returns `None` when any descent fails (ambiguous/broken path → warning).
fn resolve_canonical_item_path(
    current_module: &str,
    segments: &[String],
    graph: &ModuleGraph,
) -> Option<(String, String)> {
    if segments.is_empty() {
        return None;
    }
    let mut module = current_module.to_string();
    let n = segments.len();
    for (i, seg) in segments.iter().enumerate() {
        let is_last = i == n - 1;
        match seg.as_str() {
            "crate" | "$crate" if !is_last => {
                module = "crate".to_string();
            }
            "self" if !is_last => {
                // stay at current module
            }
            "super" if !is_last => {
                // Move to parent. Prefer the graph, fall back to path trimming.
                if let Some(parent) = parent_module(&module, graph) {
                    module = parent;
                } else {
                    // Fallback: chop off the last `::segment`.
                    if let Some(pos) = module.rfind("::") {
                        module = module[..pos].to_string();
                    } else {
                        // Already at crate root; additional `super` stays there.
                    }
                }
            }
            seg if is_last => {
                // Final segment is the item name; module is fully resolved.
                return Some((module, seg.to_string()));
            }
            other => {
                // Descend into a child module. Must be an exact match in the graph.
                let child_path = format!("{module}::{other}");
                if graph.find(&child_path).is_some() {
                    module = child_path;
                } else {
                    // Unknown intermediate segment — path is ambiguous.
                    return None;
                }
            }
        }
    }
    // We exhausted all segments without finding a last-is-last.
    // This shouldn't happen because the loop has an is_last check, but if all
    // segments are path-control words, treat the last as an item at the
    // resolved module.
    None
}

/// Returns the canonical parent module of `module` by consulting the
/// [`ModuleGraph`], or `None` when the module is the crate root or unknown.
fn parent_module(module: &str, graph: &ModuleGraph) -> Option<String> {
    graph.find(module).and_then(|mr| mr.parent.clone())
}

// ---------------------------------------------------------------------------
// Helpers: signature, use-tree, visibility, normalization
// ---------------------------------------------------------------------------

/// Returns the normalized function head (visibility + signature, no body).
fn fn_sig_head(vis: &Visibility, sig: &syn::Signature) -> String {
    normalize(quote::quote!(#vis #sig))
}

/// Normalizes a token stream into a deterministic single-space-separated form.
fn normalize(stream: impl ToTokens) -> String {
    stream
        .to_token_stream()
        .to_string()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_public(visibility: &Visibility) -> bool {
    matches!(visibility, Visibility::Public(_))
}

/// Entry in an expanded use tree.
#[derive(Clone, Debug)]
struct UseEntry {
    local_name: Option<String>,
    path_segments: Vec<String>,
    glob: bool,
}

/// Expands a `use` tree into individual binding entries.
fn expand_use_tree(tree: &UseTree) -> Vec<UseEntry> {
    let mut entries = Vec::new();
    flatten_use_tree(tree, Vec::new(), &mut entries);
    entries
}

fn flatten_use_tree(tree: &UseTree, prefix: Vec<String>, entries: &mut Vec<UseEntry>) {
    match tree {
        UseTree::Path(path) => {
            let mut segments = prefix;
            segments.push(path.ident.to_string());
            flatten_use_tree(&path.tree, segments, entries);
        }
        UseTree::Name(name) => {
            let mut segments = prefix;
            let local = name.ident.to_string();
            segments.push(local.clone());
            entries.push(UseEntry {
                local_name: Some(local),
                path_segments: segments,
                glob: false,
            });
        }
        UseTree::Rename(rename) => {
            let mut segments = prefix;
            segments.push(rename.ident.to_string());
            entries.push(UseEntry {
                local_name: Some(rename.rename.to_string()),
                path_segments: segments,
                glob: false,
            });
        }
        UseTree::Glob(_) => {
            entries.push(UseEntry {
                local_name: None,
                path_segments: prefix,
                glob: true,
            });
        }
        UseTree::Group(group) => {
            for inner in &group.items {
                flatten_use_tree(inner, prefix.clone(), entries);
            }
        }
    }
}
