// SPDX-License-Identifier: MIT OR Apache-2.0

//! Public export and re-export reachability for catalog source generation.
//!
//! [`export_graph`] takes a [`ModuleGraph`] (Task 5B1) and the package root and
//! computes every Rust path that is externally reachable from the crate root,
//! while keeping enough source identity for later signature extraction (Task
//! 5B3) and trait-relationship analysis (Task 5B4).
//!
//! # Reachability model
//!
//! Reachability starts at the crate root. A module is reachable when it is
//! declared `pub` inside another reachable module, or when a reachable module
//! re-exports it (`pub use`). Public items declared in a reachable module are
//! exported at that module's path. Private modules are **not** exported merely
//! because they exist, but their public items can be exported through `pub use`.
//! Restricted visibility (`pub(crate)`, `pub(super)`, `pub(in path)`) is never
//! externally public.
//!
//! `use` paths are resolved locally and deterministically, including the
//! `crate`/`self`/`super` forms, rename aliases, glob (`*`) imports, and
//! multi-hop chains that thread through other local re-exports.
//!
//! # External re-exports
//!
//! A `pub use` whose first segment names an external crate (such as `std`,
//! `core`, `alloc`, or a dependency alias) cannot be traced to local source
//! without executing Cargo or rustc. Such imports are reported as sorted,
//! deduplicated, contextual [`ExportWarning`]s rather than fatal corruption,
//! and no export record is emitted for them.
//!
//! # `#[cfg]` deferral
//!
//! Configuration evaluation is deliberately deferred (as in Task 5B1). Two
//! declaration variants gated by mutually exclusive `cfg` may resolve to the
//! same exported path from different local sources; both are retained as
//! distinct [`ExportRecord`]s so neither is silently dropped, leaving
//! Task 5C1 free to attach gates later. Genuine name-resolution cycles are
//! reported as warnings instead of looping forever.
//!
//! Only declarations written directly in source are seen: macros and
//! `include!`-generated items are not expanded by this syntactic parser.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

use syn::{Item, UseTree, Visibility as SynVisibility};

use crate::{DiscoveryError, DiscoveryResult};

use super::modules::{ModuleGraph, ModuleVisibility};

/// Coarse classification of an exported local item.
///
/// This is intentionally less detailed than a normalized signature: Task 5B3
/// extracts signatures, while this enum only records enough to distinguish
/// kinds of exported names.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExportItemKind {
    /// `pub struct`.
    Struct,
    /// `pub enum`.
    Enum,
    /// `pub union`.
    Union,
    /// `pub fn`.
    Function,
    /// `pub const`.
    Constant,
    /// `pub static`.
    Static,
    /// `pub trait`.
    Trait,
    /// `pub type`.
    TypeAlias,
    /// A reachable module path.
    Module,
}

/// Where a publicly reachable export ultimately comes from.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExportSource {
    /// A local item declared in source.
    Local {
        /// Canonical module path where the item is declared, e.g.
        /// `crate::algebra::ga`.
        module: String,
        /// Item identifier exactly as written in source.
        ident: String,
        /// Coarse item kind.
        kind: ExportItemKind,
    },
    /// A reachable local module path; the exported name is the module itself.
    Module {
        /// Canonical module path of the exported module.
        module: String,
    },
}

/// A single publicly reachable export.
///
/// Because cfg evaluation is deferred, the same `path` may appear in more than
/// one record when mutually exclusive `cfg` variants resolve to different local
/// sources. Consumers that need a single source per path should group by
/// [`ExportRecord::path`] and defer disambiguation to Task 5C1.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExportRecord {
    /// Canonical exported path, e.g. `crate::module::Name`.
    pub path: String,
    /// Resolved source of the exported name.
    pub source: ExportSource,
}

/// Why an expected export could not be resolved to local source.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ExportWarningReason {
    /// A `pub use` whose first segment is an external crate that cannot be
    /// inspected offline. `target` is the full external path as written.
    ExternalReexport {
        /// The unresolved external path, e.g. `std::collections::HashMap`.
        target: String,
    },
    /// A local `pub use` path that did not resolve to any declaration.
    Unresolved {
        /// Human-readable detail about the failed resolution.
        detail: String,
    },
}

/// A warning about a re-export that could not be traced to local source.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ExportWarning {
    /// Canonical module path where the `pub use` was declared.
    pub declared_in: String,
    /// Would-be exported path (`<declaring module>::<export name>`).
    pub path: String,
    /// Why local resolution failed.
    pub reason: ExportWarningReason,
}

/// Resolved public exports and warnings for one target source tree.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportGraph {
    /// Every publicly reachable export, sorted by [`ExportRecord`] order
    /// (path then source) with identical `(path, source)` pairs deduplicated.
    pub exports: Vec<ExportRecord>,
    /// Sorted, deduplicated resolution warnings.
    pub warnings: Vec<ExportWarning>,
}

impl ExportGraph {
    /// Returns every export whose exported path equals `path`.
    pub fn exports_at(&self, path: &str) -> Vec<&ExportRecord> {
        self.exports
            .iter()
            .filter(|record| record.path == path)
            .collect()
    }
}

/// Computes publicly reachable exports from a module graph and its package root.
///
/// `graph` must have been built with [`module_graph`](super::modules::module_graph)
/// for the same `package_root`. The function re-reads each file-backed module
/// source to collect item and `use` declarations; inline modules are indexed
/// from their host file. It performs no Cargo, rustc, or network access.
///
/// # Errors
///
/// Returns [`DiscoveryError::CatalogCorruption`] when a recorded source file
/// cannot be read or parsed.
pub fn export_graph(graph: &ModuleGraph, package_root: &Path) -> DiscoveryResult<ExportGraph> {
    let canonical_root = fs::canonicalize(package_root).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "cannot resolve package root {}: {error}",
            package_root.display()
        ))
    })?;
    let namespaces = index_namespaces(graph, &canonical_root)?;
    let resolver = Resolver::new(&namespaces);
    let mut collector = Collector::new();
    collector.expand(
        "crate",
        "crate",
        &resolver,
        &mut BTreeSet::from(["crate".to_owned()]),
    );
    Ok(collector.finish())
}

// ---------------------------------------------------------------------------
// Source indexing: per-canonical-module items, imports, child modules.
// ---------------------------------------------------------------------------

/// A name-binding entry contributed by a single `use` tree leaf.
#[derive(Clone, Debug)]
struct ImportEntry {
    /// Local name bound by the import (`None` only for globs).
    local_name: Option<String>,
    /// Full path segments as written, including the bound leaf segment for
    /// non-glob entries. For globs this is the target module path.
    path_segments: Vec<String>,
    /// Whether this entry is a glob (`*`).
    glob: bool,
}

/// Indexed namespace contents for one canonical module.
#[derive(Clone, Debug, Default)]
struct Namespace {
    /// Declared items by name, with kind and whether they are `pub`.
    items: BTreeMap<String, (ExportItemKind, bool)>,
    /// All `use` declarations expanded into binding entries, with `pub` flag.
    imports: Vec<(bool, ImportEntry)>,
}

/// Parent links and public-child edges keyed by canonical path.
#[derive(Clone, Debug, Default)]
struct Structure {
    /// Canonical path of the parent module, if any.
    parent: Option<String>,
    /// Child module names mapped to canonical child paths.
    children: BTreeMap<String, String>,
    /// Child names that are declared `pub` (reachable through this parent).
    public_children: BTreeSet<String>,
}

impl Structure {
    fn is_public_child(&self, name: &str) -> bool {
        self.public_children.contains(name)
    }
}

fn index_namespaces(
    graph: &ModuleGraph,
    package_root: &Path,
) -> DiscoveryResult<BTreeMap<String, (Namespace, Structure)>> {
    let mut map: BTreeMap<String, (Namespace, Structure)> = BTreeMap::new();

    // Ensure an entry exists for every module (including inline-only modules).
    for record in &graph.modules {
        map.entry(record.path.clone()).or_default();
    }

    // Populate structural parent/child/visibility edges from the graph.
    for record in &graph.modules {
        if let Some((_, structure)) = map.get_mut(&record.path) {
            if structure.parent.is_none() {
                structure.parent = record.parent.clone();
            }
        }
    }
    for record in &graph.modules {
        for child_path in &record.children {
            let child_name = child_path
                .rsplit("::")
                .next()
                .unwrap_or(child_path)
                .to_owned();
            if let Some((_, structure)) = map.get_mut(&record.path) {
                structure
                    .children
                    .insert(child_name.clone(), child_path.clone());
                // cfg-deferred: a child path is reachable if any variant is `pub`.
                let any_public = graph
                    .find_all(child_path)
                    .iter()
                    .any(|variant| variant.visibility == ModuleVisibility::Public);
                if any_public {
                    structure.public_children.insert(child_name);
                }
            }
        }
    }

    // Index item and use declarations from each file-backed module source.
    for record in &graph.modules {
        let Some(source_rel) = &record.source_path else {
            continue;
        };
        let file = package_root.join(source_rel);
        let source = fs::read_to_string(&file).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!(
                "cannot read module {}: {error}",
                file.display()
            ))
        })?;
        let ast = syn::parse_file(&source).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!("cannot parse {}: {error}", file.display()))
        })?;
        index_items(&ast.items, &record.path, &mut map);
    }

    Ok(map)
}

/// Recursively indexes items and inline-module contents under `canonical`.
///
/// Item and `use` declarations are inserted in one scoped mutable borrow, then
/// inline modules are recursed in a second pass so the borrow is released
/// before descending into child namespaces.
fn index_items(
    items: &[Item],
    canonical: &str,
    map: &mut BTreeMap<String, (Namespace, Structure)>,
) {
    if let Some((namespace, _)) = map.get_mut(canonical) {
        for item in items {
            index_named_item(namespace, item);
        }
    }
    // Inline modules carry their own items; external modules are indexed
    // separately via their own source file.
    for item in items {
        if let Item::Mod(item_mod) = item {
            if let Some((_, content)) = &item_mod.content {
                let child_canonical = format!("{canonical}::{}", item_mod.ident);
                map.entry(child_canonical.clone()).or_default();
                index_items(content, &child_canonical, map);
            }
        }
    }
}

/// Inserts a single named item or `use` declaration into `namespace`.
fn index_named_item(namespace: &mut Namespace, item: &Item) {
    match item {
        Item::Struct(item_struct) => {
            insert_item(
                namespace,
                &item_struct.ident,
                ExportItemKind::Struct,
                &item_struct.vis,
            );
        }
        Item::Enum(item_enum) => {
            insert_item(
                namespace,
                &item_enum.ident,
                ExportItemKind::Enum,
                &item_enum.vis,
            );
        }
        Item::Union(item_union) => {
            insert_item(
                namespace,
                &item_union.ident,
                ExportItemKind::Union,
                &item_union.vis,
            );
        }
        Item::Fn(item_fn) => {
            insert_item(
                namespace,
                &item_fn.sig.ident,
                ExportItemKind::Function,
                &item_fn.vis,
            );
        }
        Item::Const(item_const) => {
            insert_item(
                namespace,
                &item_const.ident,
                ExportItemKind::Constant,
                &item_const.vis,
            );
        }
        Item::Static(item_static) => {
            insert_item(
                namespace,
                &item_static.ident,
                ExportItemKind::Static,
                &item_static.vis,
            );
        }
        Item::Trait(item_trait) => {
            insert_item(
                namespace,
                &item_trait.ident,
                ExportItemKind::Trait,
                &item_trait.vis,
            );
        }
        Item::Type(item_type) => {
            insert_item(
                namespace,
                &item_type.ident,
                ExportItemKind::TypeAlias,
                &item_type.vis,
            );
        }
        Item::Use(item_use) => {
            let is_pub = is_public(&item_use.vis);
            for entry in expand_use_tree(&item_use.tree) {
                namespace.imports.push((is_pub, entry));
            }
        }
        _ => {}
    }
}

fn insert_item(
    namespace: &mut Namespace,
    ident: &syn::Ident,
    kind: ExportItemKind,
    vis: &SynVisibility,
) {
    let name = ident.to_string();
    // cfg-deferred: retain the most permissive visibility seen for the name.
    let public = is_public(vis);
    namespace
        .items
        .entry(name)
        .and_modify(|(_, existing_public)| {
            *existing_public = *existing_public || public;
        })
        .or_insert((kind, public));
}

fn is_public(visibility: &SynVisibility) -> bool {
    matches!(visibility, SynVisibility::Public(_))
}

/// Expands a `use` tree into individual binding entries.
fn expand_use_tree(tree: &UseTree) -> Vec<ImportEntry> {
    let mut entries = Vec::new();
    flatten_use_tree(tree, Vec::new(), &mut entries);
    entries
}

fn flatten_use_tree(tree: &UseTree, prefix: Vec<String>, entries: &mut Vec<ImportEntry>) {
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
            entries.push(ImportEntry {
                local_name: Some(local),
                path_segments: segments,
                glob: false,
            });
        }
        UseTree::Rename(rename) => {
            let mut segments = prefix;
            segments.push(rename.ident.to_string());
            entries.push(ImportEntry {
                local_name: Some(rename.rename.to_string()),
                path_segments: segments,
                glob: false,
            });
        }
        UseTree::Glob(_) => {
            entries.push(ImportEntry {
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

// ---------------------------------------------------------------------------
// Path resolution over canonical module namespaces.
// ---------------------------------------------------------------------------

/// Outcome of resolving a `use` path against canonical module namespaces.
#[derive(Clone, Debug, Eq, PartialEq)]
enum Resolved {
    /// A declared item.
    Item {
        module: String,
        ident: String,
        kind: ExportItemKind,
    },
    /// A module canonical path.
    Module(String),
    /// An enum-variant re-export (`Enum::Variant`); the source is the enum.
    EnumVariant { module: String, enum_ident: String },
    /// The first segment names an external crate.
    External(String),
    /// The path did not resolve locally because a binding exists but could
    /// not be followed (cycle, ambiguity, or unresolvable target).
    Unresolved(String),
    /// The name is genuinely absent from the module namespace (no item, child
    /// module, or import binds it). A first segment with this outcome is an
    /// external crate reference.
    NotFound,
}

struct Resolver<'a> {
    namespaces: &'a BTreeMap<String, (Namespace, Structure)>,
}

impl<'a> Resolver<'a> {
    fn new(namespaces: &'a BTreeMap<String, (Namespace, Structure)>) -> Self {
        Self { namespaces }
    }

    fn namespace(&self, module: &str) -> Option<&'a (Namespace, Structure)> {
        self.namespaces.get(module)
    }

    fn parent_of(&self, module: &str) -> Option<&'a str> {
        self.namespaces
            .get(module)
            .and_then(|(_, structure)| structure.parent.as_deref())
    }

    /// Resolves a path starting from `current` module. The path's first segment
    /// may be `crate`, `self`, `super`, a local name, or an external crate.
    fn resolve(
        &self,
        current: &str,
        segments: &[String],
        visited: &mut BTreeSet<String>,
    ) -> Resolved {
        if segments.is_empty() {
            return Resolved::Module(current.to_owned());
        }
        let first = &segments[0];
        let rest = &segments[1..];

        match first.as_str() {
            "crate" | "$crate" => self.resolve_rest("crate", rest, visited),
            "self" => self.resolve_rest(current, rest, visited),
            "super" => match self.parent_of(current) {
                Some(parent) => self.resolve_rest(parent, rest, visited),
                None => Resolved::Unresolved(format!("`super` has no parent at `{current}`")),
            },
            _ => {
                let looked_up = self.lookup(current, first, visited);
                match &looked_up {
                    // A first segment genuinely absent from the local namespace
                    // is an external crate reference. A binding that exists but
                    // failed to resolve stays an unresolved failure rather than
                    // masquerading as external.
                    Resolved::NotFound => Resolved::External(segments.join("::")),
                    Resolved::External(_) | Resolved::Unresolved(_) => looked_up,
                    _ => self.descend(looked_up, rest, visited),
                }
            }
        }
    }

    /// Continues resolution from a known module with the remaining segments.
    fn resolve_rest(
        &self,
        module: &str,
        rest: &[String],
        visited: &mut BTreeSet<String>,
    ) -> Resolved {
        if rest.is_empty() {
            return Resolved::Module(module.to_owned());
        }
        let looked_up = self.lookup(module, &rest[0], visited);
        self.descend(looked_up, &rest[1..], visited)
    }

    /// Descends through remaining segments from an already-resolved target.
    fn descend(
        &self,
        target: Resolved,
        rest: &[String],
        visited: &mut BTreeSet<String>,
    ) -> Resolved {
        match target {
            Resolved::Item {
                kind: ExportItemKind::Enum,
                module,
                ident,
            } => {
                if rest.is_empty() {
                    Resolved::Item {
                        module,
                        ident,
                        kind: ExportItemKind::Enum,
                    }
                } else if rest.len() == 1 {
                    Resolved::EnumVariant {
                        module,
                        enum_ident: ident,
                    }
                } else {
                    Resolved::Unresolved("cannot descend into an enum variant".to_owned())
                }
            }
            Resolved::Item {
                kind,
                module,
                ident,
            } => {
                if rest.is_empty() {
                    Resolved::Item {
                        kind,
                        module,
                        ident,
                    }
                } else {
                    Resolved::Unresolved(format!(
                        "cannot descend into non-module item `{module}::{ident}`"
                    ))
                }
            }
            Resolved::Module(module) => self.resolve_rest(&module, rest, visited),
            Resolved::EnumVariant { .. } if rest.is_empty() => target,
            Resolved::EnumVariant { .. } => {
                Resolved::Unresolved("cannot descend into an enum variant".to_owned())
            }
            Resolved::External(target) => {
                if rest.is_empty() {
                    Resolved::External(target)
                } else {
                    Resolved::Unresolved("cannot navigate into an external path".to_owned())
                }
            }
            Resolved::Unresolved(_) => target,
            Resolved::NotFound => {
                Resolved::Unresolved("name not found during path navigation".to_string())
            }
        }
    }

    /// Looks up a single name within a module's namespace (item, child module,
    /// named import, or glob provider).
    fn lookup(&self, module: &str, name: &str, visited: &mut BTreeSet<String>) -> Resolved {
        let Some((namespace, structure)) = self.namespaces.get(module) else {
            return Resolved::Unresolved(format!("unknown module `{module}`"));
        };

        if let Some((kind, _)) = namespace.items.get(name) {
            return Resolved::Item {
                module: module.to_owned(),
                ident: name.to_owned(),
                kind: *kind,
            };
        }
        if let Some(child) = structure.children.get(name) {
            return Resolved::Module(child.clone());
        }

        // Named imports may rebind `name` to a path (possibly external). A
        // re-export that fails to resolve — most importantly a self-referential
        // `pub use name;` — is ignored as a candidate rather than treated as a
        // competing source, so it cannot manufacture false ambiguity.
        let mut cycle_key = String::from(module);
        cycle_key.push('\0');
        cycle_key.push_str(name);
        if !visited.insert(cycle_key.clone()) {
            return Resolved::Unresolved(format!("cyclic re-export at `{module}::{name}`"));
        }

        let mut distinct: Vec<Resolved> = Vec::new();
        let mut named_bound = false;
        for (_is_pub, entry) in &namespace.imports {
            if entry.glob {
                continue;
            }
            if entry.local_name.as_deref() == Some(name) {
                named_bound = true;
                match self.resolve(module, &entry.path_segments, visited) {
                    Resolved::Unresolved(_) | Resolved::NotFound => continue,
                    concrete => {
                        if !distinct.contains(&concrete) {
                            distinct.push(concrete);
                        }
                    }
                }
            }
        }
        match distinct.len() {
            0 => {}
            1 => {
                visited.remove(&cycle_key);
                // Guarded by `distinct.len() == 1`; never panics.
                return distinct.swap_remove(0);
            }
            _ => {
                visited.remove(&cycle_key);
                return Resolved::Unresolved(format!(
                    "ambiguous re-export source for `{module}::{name}`"
                ));
            }
        }

        // Glob imports may provide `name` from their target module.
        let mut glob_candidate: Option<Resolved> = None;
        for (_is_pub, entry) in &namespace.imports {
            if !entry.glob {
                continue;
            }
            let target = self.resolve(module, &entry.path_segments, visited);
            if let Resolved::Module(target_module) = target {
                if let Some(provider) = self.public_member(&target_module, name, visited) {
                    if let Some(existing) = &glob_candidate {
                        if existing != &provider {
                            visited.remove(&cycle_key);
                            return Resolved::Unresolved(format!(
                                "ambiguous glob source for `{module}::{name}`"
                            ));
                        }
                    } else {
                        glob_candidate = Some(provider);
                    }
                }
            }
        }
        visited.remove(&cycle_key);
        if let Some(provider) = glob_candidate {
            return provider;
        }
        // The name is bound by some import but could not be resolved.
        if named_bound {
            return Resolved::Unresolved(format!("`{name}` bound but unresolved in `{module}`"));
        }
        // No item, child, or import binds the name: it is genuinely absent.
        Resolved::NotFound
    }

    /// Returns the public item or public child module named `name` in `module`.
    fn public_member(
        &self,
        module: &str,
        name: &str,
        _visited: &mut BTreeSet<String>,
    ) -> Option<Resolved> {
        let (namespace, structure) = self.namespaces.get(module)?;
        if let Some((kind, public)) = namespace.items.get(name) {
            if *public {
                return Some(Resolved::Item {
                    module: module.to_owned(),
                    ident: name.to_owned(),
                    kind: *kind,
                });
            }
        }
        if structure.children.get(name).is_some_and(|child| {
            // cfg-deferred reachability is decided by the export walk, but a
            // module name brought in by a glob still needs to exist.
            self.namespaces.contains_key(child)
        }) {
            return Some(Resolved::Module(structure.children[name].clone()));
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Export collection: reachable-namespace depth-first walk.
// ---------------------------------------------------------------------------

struct Collector {
    exports: BTreeSet<ExportRecord>,
    warnings: BTreeSet<ExportWarning>,
}

impl Collector {
    fn new() -> Self {
        Self {
            exports: BTreeSet::new(),
            warnings: BTreeSet::new(),
        }
    }

    fn finish(self) -> ExportGraph {
        ExportGraph {
            exports: self.exports.into_iter().collect(),
            warnings: self.warnings.into_iter().collect(),
        }
    }

    fn expand(
        &mut self,
        namespace: &str,
        canonical: &str,
        resolver: &Resolver,
        visiting: &mut BTreeSet<String>,
    ) {
        let Some((namespace_data, structure)) = resolver.namespace(canonical) else {
            return;
        };

        // Public items declared directly in this reachable module.
        for (name, (kind, public)) in &namespace_data.items {
            if *public {
                self.exports.insert(ExportRecord {
                    path: child_path(namespace, name),
                    source: ExportSource::Local {
                        module: canonical.to_owned(),
                        ident: name.clone(),
                        kind: *kind,
                    },
                });
            }
        }

        // Public child modules make their descendants reachable.
        for (child_name, child_canonical) in &structure.children {
            if structure.is_public_child(child_name) {
                self.exports.insert(ExportRecord {
                    path: child_path(namespace, child_name),
                    source: ExportSource::Module {
                        module: child_canonical.clone(),
                    },
                });
                if visiting.insert(child_canonical.clone()) {
                    self.expand(
                        &child_path(namespace, child_name),
                        child_canonical,
                        resolver,
                        visiting,
                    );
                    visiting.remove(child_canonical);
                }
            }
        }

        // Public re-exports bring names (and possibly module paths) into scope.
        for (is_pub, entry) in &namespace_data.imports {
            if !is_pub {
                continue;
            }
            self.expand_import(namespace, canonical, entry, resolver, visiting);
        }
    }

    fn expand_import(
        &mut self,
        namespace: &str,
        canonical: &str,
        entry: &ImportEntry,
        resolver: &Resolver,
        visiting: &mut BTreeSet<String>,
    ) {
        let mut visited = BTreeSet::new();
        if entry.glob {
            let target = resolver.resolve(canonical, &entry.path_segments, &mut visited);
            match target {
                Resolved::Module(target_module) => {
                    self.expand_glob(namespace, &target_module, resolver, visiting);
                }
                Resolved::External(target) => {
                    self.warn_external(namespace, canonical, "*", &target)
                }
                Resolved::Unresolved(detail) => {
                    self.warn_unresolved(namespace, canonical, "*", &detail)
                }
                _ => self.warn_unresolved(namespace, canonical, "*", "glob target is not a module"),
            }
            return;
        }

        let export_name = entry.local_name.clone().unwrap_or_default();
        let target = resolver.resolve(canonical, &entry.path_segments, &mut visited);
        match target {
            Resolved::Item {
                module,
                ident,
                kind,
            } => {
                if source_is_public(resolver, &module, &ident) {
                    self.exports.insert(ExportRecord {
                        path: child_path(namespace, &export_name),
                        source: ExportSource::Local {
                            module,
                            ident,
                            kind,
                        },
                    });
                } else {
                    self.warn_unresolved(
                        namespace,
                        canonical,
                        &export_name,
                        "re-export source is not public",
                    );
                }
            }
            Resolved::Module(target_module) => {
                self.exports.insert(ExportRecord {
                    path: child_path(namespace, &export_name),
                    source: ExportSource::Module {
                        module: target_module.clone(),
                    },
                });
                if visiting.insert(target_module.clone()) {
                    self.expand(
                        &child_path(namespace, &export_name),
                        &target_module,
                        resolver,
                        visiting,
                    );
                    visiting.remove(&target_module);
                }
            }
            Resolved::EnumVariant { module, enum_ident } => {
                if source_is_public(resolver, &module, &enum_ident) {
                    self.exports.insert(ExportRecord {
                        path: child_path(namespace, &export_name),
                        source: ExportSource::Local {
                            module,
                            ident: enum_ident,
                            kind: ExportItemKind::Enum,
                        },
                    });
                } else {
                    self.warn_unresolved(
                        namespace,
                        canonical,
                        &export_name,
                        "re-export source is not public",
                    );
                }
            }
            Resolved::External(target) => {
                self.warn_external(namespace, canonical, &export_name, &target)
            }
            Resolved::Unresolved(detail) => {
                self.warn_unresolved(namespace, canonical, &export_name, &detail)
            }
            Resolved::NotFound => self.warn_unresolved(
                namespace,
                canonical,
                &export_name,
                "re-export path resolved to nothing",
            ),
        }
    }

    fn expand_glob(
        &mut self,
        namespace: &str,
        target_module: &str,
        resolver: &Resolver,
        visiting: &mut BTreeSet<String>,
    ) {
        // A glob exposes every public name reachable through the target
        // module: items declared there, public child modules, named/aliased
        // `pub use` re-exports, and names the target itself re-exports with
        // `pub use ...::*`. We process a worklist of glob-source modules,
        // following public glob chains, with a `processed` guard so cycles
        // terminate. Named re-exports are expanded under the glob's
        // destination prefix via [`Collector::expand_import`], which resolves
        // them back to their origin and reports external sources as warnings.
        let mut sources: BTreeSet<String> = BTreeSet::new();
        sources.insert(target_module.to_owned());
        let mut processed: BTreeSet<String> = BTreeSet::new();

        while let Some(source) = sources.iter().next().cloned() {
            sources.remove(&source);
            if !processed.insert(source.clone()) {
                continue;
            }
            let Some((target_namespace, target_structure)) = resolver.namespace(&source) else {
                continue;
            };

            for (name, (kind, public)) in &target_namespace.items {
                if *public {
                    self.exports.insert(ExportRecord {
                        path: child_path(namespace, name),
                        source: ExportSource::Local {
                            module: source.clone(),
                            ident: name.clone(),
                            kind: *kind,
                        },
                    });
                }
            }
            for (child_name, child_canonical) in &target_structure.children {
                // Globs only expose module names that are public in the target.
                if target_structure.public_children.contains(child_name) {
                    self.exports.insert(ExportRecord {
                        path: child_path(namespace, child_name),
                        source: ExportSource::Module {
                            module: child_canonical.clone(),
                        },
                    });
                    if visiting.insert(child_canonical.clone()) {
                        self.expand(
                            &child_path(namespace, child_name),
                            child_canonical,
                            resolver,
                            visiting,
                        );
                        visiting.remove(child_canonical);
                    }
                }
            }
            // Public re-exports declared in the source module are themselves
            // public names the glob must surface. Glob re-exports extend the
            // worklist so `pub use a::*` then `pub use b::*` chains flatten
            // transitively; named/aliased re-exports are expanded under the
            // glob's destination prefix, resolving to origin source and
            // reporting external targets as contextual warnings.
            let mut chain_visited = BTreeSet::new();
            for (is_pub, entry) in &target_namespace.imports {
                if !*is_pub {
                    continue;
                }
                if entry.glob {
                    if let Resolved::Module(chained) =
                        resolver.resolve(&source, &entry.path_segments, &mut chain_visited)
                    {
                        sources.insert(chained);
                    }
                } else {
                    self.expand_import(namespace, &source, entry, resolver, visiting);
                }
            }
        }
    }

    fn warn_external(&mut self, namespace: &str, canonical: &str, export_name: &str, target: &str) {
        self.warnings.insert(ExportWarning {
            declared_in: canonical.to_owned(),
            path: child_path(namespace, export_name),
            reason: ExportWarningReason::ExternalReexport {
                target: target.to_owned(),
            },
        });
    }

    fn warn_unresolved(
        &mut self,
        namespace: &str,
        canonical: &str,
        export_name: &str,
        detail: &str,
    ) {
        self.warnings.insert(ExportWarning {
            declared_in: canonical.to_owned(),
            path: child_path(namespace, export_name),
            reason: ExportWarningReason::Unresolved {
                detail: detail.to_owned(),
            },
        });
    }
}

fn source_is_public(resolver: &Resolver, module: &str, ident: &str) -> bool {
    resolver
        .namespace(module)
        .and_then(|(namespace, _)| namespace.items.get(ident))
        .is_some_and(|(_, public)| *public)
}

fn child_path(namespace: &str, name: &str) -> String {
    format!("{namespace}::{name}")
}

// Public API re-export surface is declared in `super::mod`.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_handles_grouped_renames_and_globs() {
        let tree: UseTree = syn::parse_quote!(a::b::{c, d::e as f, *});
        let entries = expand_use_tree(&tree);
        let summary: Vec<(Option<&str>, String, bool)> = entries
            .iter()
            .map(|e| (e.local_name.as_deref(), e.path_segments.join("::"), e.glob))
            .collect();
        assert!(summary.contains(&(Some("c"), "a::b::c".to_owned(), false)));
        assert!(summary.contains(&(Some("f"), "a::b::d::e".to_owned(), false)));
        assert!(summary.contains(&(None, "a::b".to_owned(), true)));
    }
}
