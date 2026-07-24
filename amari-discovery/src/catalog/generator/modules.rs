// SPDX-License-Identifier: MIT OR Apache-2.0

//! Recursive local Rust module graph for catalog source generation.
//!
//! [`module_graph`] walks a single Cargo target's source root and records every
//! declared local module — file modules, `mod.rs`-style modules, `#[path]`
//! overrides, and inline modules — together with the visibility, parent, and
//! source-path metadata needed by later export-resolution tasks.
//!
//! Resolution follows the directory-based Cargo/Rust module conventions shared
//! by the 2018 and later editions (and the `mod.rs` form used by 2015 crates):
//! a module declared in a file resolves its file submodules from a directory
//! owned by that file. `#[cfg]` interpretation is deliberately deferred to a
//! later task; every `mod` declaration is treated as active and its source file
//! is required to exist. The walker is read-only, deterministic, offline, and
//! rejects any resolved source path or symlink that escapes the package root.
//!
//! Only declarations written directly in source are seen: macros and
//! `include!`-generated `mod` declarations are not expanded by this syntactic
//! parser, so any module introduced by those mechanisms is invisible to the
//! graph.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

use syn::{Item, ItemMod, Visibility as SynVisibility};

use crate::{DiscoveryError, DiscoveryResult};

/// How a module enters the local graph.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ModuleKind {
    /// The crate root (`lib.rs`, `main.rs`, or any Cargo target source root).
    Crate,
    /// An external file module: `mod foo;`, `mod.rs`, or a `#[path]` file.
    File,
    /// An inline module: `mod foo { ... }`.
    Inline,
}

/// Recorded visibility of a module declaration.
///
/// `Restricted` covers every path-restricted form (`pub(crate)`, `pub(super)`,
/// `pub(in path)`, and `pub(self)`), distinguishing them from fully `Public`
/// declarations so that later export resolution can decide reachability.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ModuleVisibility {
    /// `pub` with no restriction.
    Public,
    /// Any restricted visibility such as `pub(crate)` or `pub(super)`.
    Restricted,
    /// Inherited (private) visibility.
    Private,
}

/// A single module within a target's local source graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleRecord {
    /// Canonical module path from the crate root, e.g. `crate::algebra::ga`.
    pub path: String,
    /// How the module was declared or located.
    pub kind: ModuleKind,
    /// Visibility of the module declaration.
    pub visibility: ModuleVisibility,
    /// Package-root-relative source path using `/` separators for file-backed
    /// modules; `None` for inline modules.
    pub source_path: Option<String>,
    /// Canonical path of the parent module, or `None` for the crate root.
    pub parent: Option<String>,
    /// Canonical paths of directly declared child modules, sorted and
    /// deduplicated.
    ///
    /// Because cfg evaluation is deferred, more than one declaration variant
    /// may share this record's canonical path (see [`ModuleGraph`]). When that
    /// happens every variant carries the *unioned* logical child set for the
    /// path, so neither variant silently loses a child path; Task 5C1 will later
    /// attach gates to distinguish which children belong to which variant.
    pub children: Vec<String>,
}

/// The complete local module graph for one target source root.
///
/// Canonical module paths need **not** be unique. While cfg evaluation is
/// deferred, valid alternate declarations such as a `#[cfg(unix)]` /
/// `#[cfg(windows)]` pair may both resolve to the same canonical path and are
/// retained as distinct [`ModuleRecord`] entries (differing only in
/// `source_path` and nested children). Use [`ModuleGraph::find_all`] to
/// enumerate every variant sharing a path, or [`ModuleGraph::find`] for the
/// first deterministic match.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleGraph {
    /// Every recorded module, sorted by canonical path.
    pub modules: Vec<ModuleRecord>,
}

impl ModuleGraph {
    /// Returns the crate-root module record.
    pub fn root(&self) -> Option<&ModuleRecord> {
        self.modules
            .iter()
            .find(|record| record.kind == ModuleKind::Crate)
    }

    /// Finds the first module whose canonical path matches.
    ///
    /// Because cfg evaluation is deferred (see [`ModuleGraph`]), more than one
    /// declaration variant may share a canonical path. `find` returns the first
    /// such match in [`ModuleGraph::modules`] order (source-declaration order
    /// among equal paths) for convenience and backward compatibility. To
    /// enumerate every variant, use [`ModuleGraph::find_all`].
    pub fn find(&self, path: &str) -> Option<&ModuleRecord> {
        self.modules.iter().find(|record| record.path == path)
    }

    /// Returns every declaration variant with the given canonical path, in
    /// [`ModuleGraph::modules`] order.
    ///
    /// cfg evaluation is deferred, so valid alternate declarations such as
    /// ```ignore
    /// #[cfg(unix)]
    /// #[path = "sys/unix.rs"]
    /// mod sys;
    /// #[cfg(windows)]
    /// #[path = "sys/windows.rs"]
    /// mod sys;
    /// ```
    /// both resolve to the canonical path `crate::sys` and are retained as
    /// distinct records until Task 5C1 attaches gates. `find_all` exposes every
    /// variant so downstream tasks do not silently lose declaration data.
    pub fn find_all(&self, path: &str) -> Vec<&ModuleRecord> {
        self.modules
            .iter()
            .filter(|record| record.path == path)
            .collect()
    }
}

/// Builds the complete local module graph rooted at a target source file.
///
/// `package_root` is the package directory used as the containment boundary,
/// and `source_path` is the target's crate root relative to it (for example
/// `src/lib.rs`, `src/main.rs`, or `src/bin/tool/main.rs`). Every resolved
/// source file is canonicalized and must remain within `package_root`.
///
/// The graph records private and restricted modules as well as public ones,
/// because private modules may still be re-export sources, but it does not yet
/// decide which items are publicly reachable.
///
/// # Errors
///
/// Returns [`DiscoveryError::CatalogCorruption`] when the target source or any
/// declared module file is missing, ambiguous, malformed, escapes the package
/// root, or participates in a `#[path]` cycle.
pub fn module_graph(package_root: &Path, source_path: &str) -> DiscoveryResult<ModuleGraph> {
    let canonical_root = fs::canonicalize(package_root).map_err(|error| {
        DiscoveryError::CatalogCorruption(format!(
            "cannot resolve package root {}: {error}",
            package_root.display()
        ))
    })?;
    let mut builder = GraphBuilder {
        package_root: canonical_root,
        nodes: Vec::new(),
    };
    builder.build(source_path)?;
    Ok(builder.finish())
}

struct GraphBuilder {
    package_root: PathBuf,
    nodes: Vec<ModuleRecord>,
}

impl GraphBuilder {
    fn build(&mut self, source_path: &str) -> DiscoveryResult<()> {
        let relative = Path::new(source_path);
        if relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(DiscoveryError::CatalogCorruption(format!(
                "target source path is absolute or escapes its package: {source_path}"
            )));
        }
        let root_file = self.contained_file(&self.package_root.join(relative))?;
        let root_rel = self.relative_to_root(&root_file)?;
        let owning_dir = root_file
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.package_root.clone());
        self.nodes.push(ModuleRecord {
            path: "crate".to_owned(),
            kind: ModuleKind::Crate,
            visibility: ModuleVisibility::Public,
            source_path: Some(root_rel),
            parent: None,
            children: Vec::new(),
        });

        let mut visiting = Vec::new();
        self.expand_file(&root_file, &owning_dir, "crate", &mut visiting)?;
        Ok(())
    }

    fn expand_file(
        &mut self,
        file: &Path,
        owning_dir: &Path,
        parent_path: &str,
        visiting: &mut Vec<PathBuf>,
    ) -> DiscoveryResult<()> {
        if visiting.iter().any(|visited| visited == file) {
            return Err(DiscoveryError::CatalogCorruption(format!(
                "module cycle detected at {}",
                self.relative_for_error(file)
            )));
        }
        let source = fs::read_to_string(file).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!(
                "cannot read module {}: {error}",
                self.relative_for_error(file)
            ))
        })?;
        let file_ast = syn::parse_file(&source).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!(
                "cannot parse {}: {error}",
                self.relative_for_error(file)
            ))
        })?;

        visiting.push(file.to_path_buf());
        let declaring_rel = self.relative_for_error(file);
        for item in &file_ast.items {
            if let Item::Mod(item_mod) = item {
                self.declare_module(item_mod, owning_dir, parent_path, &declaring_rel, visiting)?;
            }
        }
        visiting.pop();
        Ok(())
    }

    fn declare_module(
        &mut self,
        item: &ItemMod,
        owning_dir: &Path,
        parent_path: &str,
        declaring_rel: &str,
        visiting: &mut Vec<PathBuf>,
    ) -> DiscoveryResult<()> {
        let name = item.ident.to_string();
        let module_path = format!("{parent_path}::{name}");
        let visibility = module_visibility(&item.vis);
        let path_attr = path_attribute(&item.attrs);

        if let Some((_, items)) = &item.content {
            let inline_owning_dir = match &path_attr {
                Some(relative) => owning_dir.join(relative),
                None => owning_dir.to_path_buf(),
            };
            self.nodes.push(ModuleRecord {
                path: module_path.clone(),
                kind: ModuleKind::Inline,
                visibility,
                source_path: None,
                parent: Some(parent_path.to_owned()),
                children: Vec::new(),
            });
            for inner in items {
                if let Item::Mod(inner_mod) = inner {
                    self.declare_module(
                        inner_mod,
                        &inline_owning_dir,
                        &module_path,
                        declaring_rel,
                        visiting,
                    )?;
                }
            }
        } else {
            let (file, child_owning_dir) = self.resolve_external_module(
                &name,
                owning_dir,
                path_attr.as_deref(),
                declaring_rel,
            )?;
            let source_rel = self.relative_to_root(&file)?;
            self.nodes.push(ModuleRecord {
                path: module_path.clone(),
                kind: ModuleKind::File,
                visibility,
                source_path: Some(source_rel),
                parent: Some(parent_path.to_owned()),
                children: Vec::new(),
            });
            self.expand_file(&file, &child_owning_dir, &module_path, visiting)?;
        }
        Ok(())
    }

    /// Resolves an external `mod name;` declaration to its canonical source file
    /// and the directory that owns its submodules. Returns `(file, owning_dir)`.
    fn resolve_external_module(
        &self,
        name: &str,
        owning_dir: &Path,
        path_attr: Option<&str>,
        declaring_rel: &str,
    ) -> DiscoveryResult<(PathBuf, PathBuf)> {
        let (candidate, is_mod_rs) = match path_attr {
            Some(relative) => {
                let base = owning_dir.join(relative);
                if base.is_file() {
                    let is_mod = base.file_name().is_some_and(|stem| stem == "mod.rs");
                    (base, is_mod)
                } else if base.is_dir() {
                    (base.join("mod.rs"), true)
                } else {
                    return Err(DiscoveryError::CatalogCorruption(format!(
                        "module `{name}` declared in {declaring_rel} has no source at path `{relative}`"
                    )));
                }
            }
            None => {
                let file_candidate = owning_dir.join(format!("{name}.rs"));
                let mod_candidate = owning_dir.join(name).join("mod.rs");
                match (file_candidate.is_file(), mod_candidate.is_file()) {
                    (true, true) => {
                        return Err(DiscoveryError::CatalogCorruption(format!(
                            "module `{name}` declared in {declaring_rel} is ambiguous: source exists at both {} and {}",
                            self.relative_for_error(&file_candidate),
                            self.relative_for_error(&mod_candidate)
                        )));
                    }
                    (true, false) => (file_candidate, false),
                    (false, true) => (mod_candidate, true),
                    (false, false) => {
                        return Err(DiscoveryError::CatalogCorruption(format!(
                            "module `{name}` declared in {declaring_rel} has no source file"
                        )));
                    }
                }
            }
        };

        let canonical = self.contained_file(&candidate)?;
        let child_owning_dir = module_owning_directory(&canonical, is_mod_rs);
        Ok((canonical, child_owning_dir))
    }

    fn contained_file(&self, candidate: &Path) -> DiscoveryResult<PathBuf> {
        let resolved = fs::canonicalize(candidate).map_err(|error| {
            DiscoveryError::CatalogCorruption(format!(
                "cannot resolve source {}: {error}",
                candidate.display()
            ))
        })?;
        if !resolved.starts_with(&self.package_root) {
            return Err(DiscoveryError::CatalogCorruption(format!(
                "module source {} escapes package root {}",
                resolved.display(),
                self.package_root.display()
            )));
        }
        if !resolved.is_file() {
            return Err(DiscoveryError::CatalogCorruption(format!(
                "module source {} is not a file",
                resolved.display()
            )));
        }
        Ok(resolved)
    }

    fn relative_to_root(&self, canonical: &Path) -> DiscoveryResult<String> {
        let relative = canonical.strip_prefix(&self.package_root).map_err(|_| {
            DiscoveryError::CatalogCorruption(format!(
                "module source {} escapes package root {}",
                canonical.display(),
                self.package_root.display()
            ))
        })?;
        Ok(self.normalize(relative))
    }

    /// Best-effort package-relative path for error context, falling back to the
    /// raw display when the path is outside the package root or unresolved.
    fn relative_for_error(&self, path: &Path) -> String {
        match fs::canonicalize(path) {
            Ok(canonical) if canonical.starts_with(&self.package_root) => {
                match canonical.strip_prefix(&self.package_root) {
                    Ok(relative) => self.normalize(relative),
                    Err(_) => path.display().to_string(),
                }
            }
            _ => path.display().to_string(),
        }
    }

    fn normalize(&self, relative: &Path) -> String {
        relative
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/")
    }

    fn finish(&mut self) -> ModuleGraph {
        self.nodes.sort_by(|left, right| left.path.cmp(&right.path));

        // Canonical module paths need not be unique while cfg is deferred (see
        // `ModuleGraph`), so build a deduplicated logical child set per parent
        // and copy it into every variant sharing that path. The map is read with
        // `get` rather than drained with `remove`: a later variant with the same
        // canonical path must still receive the unioned logical children
        // instead of an empty list, since each variant owns a distinct source
        // file whose declared children are all retained for Task 5C1.
        let mut children_by_parent: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for node in &self.nodes {
            if let Some(parent) = &node.parent {
                children_by_parent
                    .entry(parent.clone())
                    .or_default()
                    .insert(node.path.clone());
            }
        }
        for node in self.nodes.iter_mut() {
            let mut children: Vec<String> = children_by_parent
                .get(&node.path)
                .map(|set| set.iter().cloned().collect())
                .unwrap_or_default();
            children.sort();
            node.children = children;
        }
        ModuleGraph {
            modules: std::mem::take(&mut self.nodes),
        }
    }
}

/// Returns the directory that owns a resolved module's submodules.
///
/// `mod.rs` files own their containing directory; other files own a sibling
/// directory named after the file stem (which need not exist yet).
fn module_owning_directory(file: &Path, is_mod_rs: bool) -> PathBuf {
    let parent = file
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    if is_mod_rs {
        parent
    } else {
        let stem = file
            .file_stem()
            .map(std::ffi::OsString::from)
            .unwrap_or_else(|| std::ffi::OsString::from("module"));
        parent.join(stem)
    }
}

fn module_visibility(visibility: &SynVisibility) -> ModuleVisibility {
    match visibility {
        SynVisibility::Public(_) => ModuleVisibility::Public,
        SynVisibility::Restricted(_) => ModuleVisibility::Restricted,
        SynVisibility::Inherited => ModuleVisibility::Private,
    }
}

/// Extracts the string value of a `#[path = "..."]` attribute, when present.
fn path_attribute(attrs: &[syn::Attribute]) -> Option<String> {
    use syn::{Expr, ExprLit, Lit, Meta};

    attrs.iter().find_map(|attr| {
        if !attr.path().is_ident("path") {
            return None;
        }
        let Meta::NameValue(name_value) = &attr.meta else {
            return None;
        };
        let Expr::Lit(ExprLit {
            lit: Lit::Str(literal),
            ..
        }) = &name_value.value
        else {
            return None;
        };
        Some(literal.value())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_mapping_covers_public_restricted_and_private() {
        assert_eq!(
            module_visibility(&syn::parse_quote!(pub)),
            ModuleVisibility::Public
        );
        assert_eq!(
            module_visibility(&syn::parse_quote!(pub(crate))),
            ModuleVisibility::Restricted
        );
        assert_eq!(
            module_visibility(&syn::parse_quote!(pub(self))),
            ModuleVisibility::Restricted
        );
        assert_eq!(
            module_visibility(&syn::parse_quote!()),
            ModuleVisibility::Private
        );
    }
}
