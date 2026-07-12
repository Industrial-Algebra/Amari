// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Syn-based Rust source parser for Amari API usage extraction.
//!
//! # Span handling
//!
//! `syn` provides byte-offset spans. We convert these to 1-based line/column
//! using a pre-computed line-offset table derived from the source text.
//! `proc_macro2` span start.column is 0-based — we add one for 1-based output.

use std::collections::BTreeSet;

use syn::{
    spanned::Spanned,
    visit::{self, Visit},
    AttrStyle, Attribute, ExprMacro, ExprPath, ItemExternCrate, ItemUse, Meta, PathSegment,
    StmtMacro, TraitBound, TypePath, UseTree,
};

use super::types::{RustCfgEvidence, RustUsage, RustUsageKind};
use crate::inspect::snapshot::SourceLocation;

use super::inspect::CrateAliasMap;

// ============================================================================
// Line offset table for byte → line/column conversion
// ============================================================================

#[derive(Clone, Debug)]
pub(crate) struct LineOffsets {
    offsets: Vec<usize>,
}

impl LineOffsets {
    pub fn from_source(source: &str) -> Self {
        let mut offsets = vec![0usize];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                offsets.push(i + 1);
            }
        }
        Self { offsets }
    }

    pub fn line_col(&self, byte_offset: usize) -> Option<(u32, u32)> {
        if byte_offset > self.offsets.last().copied().unwrap_or(0) + 8192 {
            return None;
        }
        match self.offsets.binary_search(&byte_offset) {
            Ok(line) => Some(((line + 1) as u32, 1u32)),
            Err(0) => None,
            Err(line) => {
                let line_idx = line - 1;
                let line_start = self.offsets[line_idx];
                let col = (byte_offset - line_start + 1) as u32;
                Some(((line_idx + 1) as u32, col))
            }
        }
    }
}

// ============================================================================
// Doc segment — individually anchored doc evidence
// ============================================================================

/// A single doc comment segment with its source location derived from span.
#[derive(Clone, Debug)]
pub(crate) struct DocSegment {
    /// The extracted doc text.
    pub text: String,
    /// Source location derived from the attribute's span.
    pub source: SourceLocation,
}

// ============================================================================
// Public parse entry point
// ============================================================================

#[derive(Debug)]
pub(super) struct ParsedRustFile {
    pub usages: Vec<RustUsage>,
    pub cfg_evidence: Vec<RustCfgEvidence>,
    /// Crate attributes with their span for source location resolution.
    pub crate_attrs: Vec<(String, proc_macro2::Span)>,
    /// Individually anchored doc comment segments from `///`, `//!`, and
    /// `#[doc = "..."]` attributes.
    pub doc_segments: Vec<DocSegment>,
}

/// Parse a Rust source file with syn and extract Amari evidence in one pass.
pub(super) fn parse_rust_source(
    source: &str,
    content_hash: &str,
    path: &str,
    aliases: &CrateAliasMap,
) -> Result<ParsedRustFile, SynParseError> {
    let file = match syn::parse_file(source) {
        Ok(f) => f,
        Err(e) => {
            let start = e.span().start();
            let reason = "parse error".to_string();
            return Err(SynParseError {
                reason,
                line: if start.line > 0 {
                    Some(start.line as u32)
                } else {
                    None
                },
                column: if start.line > 0 {
                    Some((start.column + 1) as u32)
                } else {
                    None
                },
                content_hash: content_hash.to_string(),
            });
        }
    };

    let mut visitor = AmariVisitor::new(source, content_hash, path, aliases);

    // Single pass: walk the whole file (also collects #[doc = "..."] segments
    // including /// and //! lowered to #[doc] by syn)
    visitor.visit_file(&file);

    sort_and_dedup_usages(&mut visitor.usages);
    sort_and_dedup_cfg(&mut visitor.cfg_evidence);

    Ok(ParsedRustFile {
        usages: visitor.usages,
        cfg_evidence: visitor.cfg_evidence,
        crate_attrs: visitor.crate_attrs,
        doc_segments: visitor.doc_segments,
    })
}

#[derive(Debug, Clone)]
pub(super) struct SynParseError {
    pub reason: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub content_hash: String,
}

// ============================================================================
// Dedup helpers
// ============================================================================

fn sort_and_dedup_usages(usages: &mut Vec<RustUsage>) {
    usages.sort_by(|a, b| {
        a.crate_name
            .cmp(&b.crate_name)
            .then(a.alias.cmp(&b.alias))
            .then(a.path_segments.cmp(&b.path_segments))
            .then(a.kind.cmp(&b.kind))
            .then(a.source.path.cmp(&b.source.path))
            .then(a.source.line.cmp(&b.source.line))
            .then(a.source.column.cmp(&b.source.column))
    });
    usages.dedup_by(|a, b| {
        a.crate_name == b.crate_name
            && a.alias == b.alias
            && a.path_segments == b.path_segments
            && a.kind == b.kind
            && a.source.path == b.source.path
            && a.source.line == b.source.line
            && a.source.column == b.source.column
    });
}

fn sort_and_dedup_cfg(evidence: &mut Vec<RustCfgEvidence>) {
    evidence.sort_by(|a, b| {
        a.path
            .cmp(&b.path)
            .then(a.cfg_predicate.cmp(&b.cfg_predicate))
            .then(a.is_cfg_attr.cmp(&b.is_cfg_attr))
            .then(
                a.source
                    .as_ref()
                    .map(|s| (&s.path[..], s.line, s.column))
                    .cmp(&b.source.as_ref().map(|s| (&s.path[..], s.line, s.column))),
            )
    });
    evidence.dedup_by(|a, b| {
        a.path == b.path
            && a.cfg_predicate == b.cfg_predicate
            && a.is_cfg_attr == b.is_cfg_attr
            && a.source == b.source
    });
}

// ============================================================================
// Amari path matching helpers
// ============================================================================

type CfgKey = (String, String, bool, Option<u32>, Option<u32>);

fn is_amari_crate(segment: &str, aliases: &CrateAliasMap) -> bool {
    aliases.contains_crate(segment)
}

fn resolve_crate_name(segment: &str, aliases: &CrateAliasMap) -> Option<String> {
    aliases.resolve(segment)
}

// ============================================================================
// Amari visitor — single-pass AST traversal
// ============================================================================

struct AmariVisitor<'a> {
    content_hash: String,
    path: String,
    aliases: &'a CrateAliasMap,
    usages: Vec<RustUsage>,
    cfg_evidence: Vec<RustCfgEvidence>,
    seen_cfg: BTreeSet<CfgKey>,
    crate_attrs: Vec<(String, proc_macro2::Span)>,
    doc_segments: Vec<DocSegment>,
}

impl<'a> AmariVisitor<'a> {
    fn new(_source: &str, content_hash: &str, path: &str, aliases: &'a CrateAliasMap) -> Self {
        Self {
            content_hash: content_hash.to_string(),
            path: path.to_string(),
            aliases,
            usages: Vec::new(),
            cfg_evidence: Vec::new(),
            seen_cfg: BTreeSet::new(),
            crate_attrs: Vec::new(),
            doc_segments: Vec::new(),
        }
    }

    fn span_to_source(&self, span: proc_macro2::Span) -> Option<SourceLocation> {
        let start = span.start();
        if start.line == 0 {
            return None;
        }
        Some(SourceLocation {
            path: self.path.clone(),
            line: Some(start.line as u32),
            column: Some((start.column + 1) as u32),
            content_hash: self.content_hash.clone(),
        })
    }

    fn file_source(&self) -> SourceLocation {
        SourceLocation {
            path: self.path.clone(),
            line: None,
            column: None,
            content_hash: self.content_hash.clone(),
        }
    }

    fn record_usage(
        &mut self,
        crate_name: String,
        alias: String,
        path_segments: Vec<String>,
        kind: RustUsageKind,
        span: proc_macro2::Span,
    ) {
        let source = self
            .span_to_source(span)
            .unwrap_or_else(|| self.file_source());
        self.usages.push(RustUsage {
            crate_name,
            alias,
            path_segments,
            kind,
            source,
        });
    }

    fn check_path(
        &mut self,
        segments: &syn::punctuated::Punctuated<PathSegment, syn::Token![::]>,
        span: proc_macro2::Span,
        kind: RustUsageKind,
    ) {
        let first = match segments.first() {
            Some(s) => s.ident.to_string(),
            None => return,
        };
        if !is_amari_crate(&first, self.aliases) {
            return;
        }
        let crate_name = match resolve_crate_name(&first, self.aliases) {
            Some(n) => n,
            None => return,
        };
        let path_segments: Vec<String> = segments
            .iter()
            .skip(1)
            .map(|s| s.ident.to_string())
            .collect();

        self.record_usage(crate_name, first, path_segments, kind, span);
    }

    fn process_item_use(&mut self, item_use: &ItemUse) {
        let span = item_use.span();
        self.collect_from_use_tree(&item_use.tree, &mut Vec::new(), span);
    }

    fn collect_from_use_tree(
        &mut self,
        tree: &UseTree,
        prefix: &mut Vec<String>,
        span: proc_macro2::Span,
    ) {
        match tree {
            UseTree::Path(use_path) => {
                prefix.push(use_path.ident.to_string());
                self.collect_from_use_tree(&use_path.tree, prefix, span);
                prefix.pop();
            }
            UseTree::Name(use_name) => {
                let final_ident = use_name.ident.to_string();
                self.record_use_leaf(
                    prefix,
                    Some(&final_ident),
                    use_name.ident.span(),
                    RustUsageKind::Use,
                );
            }
            UseTree::Rename(use_rename) => {
                let final_ident = use_rename.ident.to_string();
                self.record_use_leaf(
                    prefix,
                    Some(&final_ident),
                    use_rename.ident.span(),
                    RustUsageKind::Use,
                );
            }
            UseTree::Glob(_) => {
                self.record_use_leaf(prefix, None, span, RustUsageKind::Use);
            }
            UseTree::Group(use_group) => {
                for item in &use_group.items {
                    self.collect_from_use_tree(item, prefix, span);
                }
            }
        }
    }

    fn record_use_leaf(
        &mut self,
        prefix: &[String],
        leaf_ident: Option<&str>,
        span: proc_macro2::Span,
        kind: RustUsageKind,
    ) {
        if prefix.is_empty() {
            if let Some(ident) = leaf_ident {
                if is_amari_crate(ident, self.aliases) {
                    if let Some(crate_name) = resolve_crate_name(ident, self.aliases) {
                        self.record_usage(crate_name, ident.to_string(), vec![], kind, span);
                    }
                }
            }
            return;
        }

        let first = &prefix[0];
        if !is_amari_crate(first, self.aliases) {
            return;
        }
        let crate_name = match resolve_crate_name(first, self.aliases) {
            Some(n) => n,
            None => return,
        };

        let mut path_segments: Vec<String> = prefix[1..].to_vec();
        if let Some(leaf) = leaf_ident {
            path_segments.push(leaf.to_string());
        }

        self.record_usage(crate_name, first.clone(), path_segments, kind, span);
    }

    fn collect_inner_attr(&mut self, attr: &Attribute) {
        if !matches!(attr.style, AttrStyle::Inner(_)) {
            return;
        }
        if attr.path().is_ident("doc") {
            return;
        }
        let normalized = normalize_attr(attr);
        if !normalized.is_empty() {
            self.crate_attrs.push((normalized, attr.span()));
        }
    }

    fn collect_cfg(&mut self, attr: &Attribute) {
        if attr.path().is_ident("cfg") {
            if let Meta::List(list) = &attr.meta {
                let pred = list.tokens.to_string();
                let normalized = normalize_cfg_predicate(&pred);
                if !normalized.is_empty() {
                    let source = self.span_to_source(attr.span());
                    let key = (
                        self.path.clone(),
                        normalized.clone(),
                        false,
                        source.as_ref().and_then(|s| s.line),
                        source.as_ref().and_then(|s| s.column),
                    );
                    if self.seen_cfg.insert(key) {
                        self.cfg_evidence.push(RustCfgEvidence {
                            path: self.path.clone(),
                            cfg_predicate: normalized,
                            is_cfg_attr: false,
                            source,
                        });
                    }
                }
            }
        } else if attr.path().is_ident("cfg_attr") {
            if let Meta::List(list) = &attr.meta {
                let tokens_str = list.tokens.to_string();
                if let Some(pred) = extract_cfg_attr_predicate(&tokens_str) {
                    let source = self.span_to_source(attr.span());
                    let key = (
                        self.path.clone(),
                        pred.clone(),
                        true,
                        source.as_ref().and_then(|s| s.line),
                        source.as_ref().and_then(|s| s.column),
                    );
                    if self.seen_cfg.insert(key) {
                        self.cfg_evidence.push(RustCfgEvidence {
                            path: self.path.clone(),
                            cfg_predicate: pred,
                            is_cfg_attr: true,
                            source,
                        });
                    }
                }
            }
        }
    }

    fn collect_doc_attr(&mut self, attr: &Attribute) {
        if !attr.path().is_ident("doc") {
            return;
        }
        if let Meta::NameValue(nv) = &attr.meta {
            if let syn::Expr::Lit(expr_lit) = &nv.value {
                if let syn::Lit::Str(lit_str) = &expr_lit.lit {
                    // Use span-derived source location (no source.find fallback)
                    let source = self
                        .span_to_source(attr.span())
                        .unwrap_or_else(|| self.file_source());
                    self.doc_segments.push(DocSegment {
                        text: lit_str.value(),
                        source,
                    });
                }
            }
        }
    }
}

// ============================================================================
// Visit implementation — single traversal
// ============================================================================

impl<'a> Visit<'_> for AmariVisitor<'a> {
    fn visit_item_use(&mut self, item_use: &ItemUse) {
        self.process_item_use(item_use);
        visit::visit_item_use(self, item_use);
    }

    fn visit_item_extern_crate(&mut self, item: &ItemExternCrate) {
        let ident = item.ident.to_string();
        if is_amari_crate(&ident, self.aliases) {
            if let Some(crate_name) = resolve_crate_name(&ident, self.aliases) {
                let source = self
                    .span_to_source(item.ident.span())
                    .unwrap_or_else(|| self.file_source());
                self.usages.push(RustUsage {
                    crate_name,
                    alias: ident,
                    path_segments: vec![],
                    kind: RustUsageKind::ExternCrate,
                    source,
                });
            }
        }
        visit::visit_item_extern_crate(self, item);
    }

    fn visit_expr_path(&mut self, expr: &ExprPath) {
        self.check_path(
            &expr.path.segments,
            expr.path.span(),
            RustUsageKind::PathExpression,
        );
        visit::visit_expr_path(self, expr);
    }

    fn visit_type_path(&mut self, ty: &TypePath) {
        self.check_path(&ty.path.segments, ty.path.span(), RustUsageKind::PathType);
        visit::visit_type_path(self, ty);
    }

    fn visit_trait_bound(&mut self, tb: &TraitBound) {
        self.check_path(&tb.path.segments, tb.path.span(), RustUsageKind::PathTrait);
        visit::visit_trait_bound(self, tb);
    }

    fn visit_expr_macro(&mut self, mac: &ExprMacro) {
        self.check_path(
            &mac.mac.path.segments,
            mac.mac.path.span(),
            RustUsageKind::PathMacro,
        );
        visit::visit_expr_macro(self, mac);
    }

    fn visit_stmt_macro(&mut self, mac: &StmtMacro) {
        self.check_path(
            &mac.mac.path.segments,
            mac.mac.path.span(),
            RustUsageKind::PathMacro,
        );
        visit::visit_stmt_macro(self, mac);
    }

    fn visit_attribute(&mut self, attr: &Attribute) {
        self.collect_cfg(attr);
        self.collect_doc_attr(attr);
        self.collect_inner_attr(attr);
        visit::visit_attribute(self, attr);
    }

    fn visit_file(&mut self, file: &syn::File) {
        for attr in &file.attrs {
            self.visit_attribute(attr);
        }
        for item in &file.items {
            self.visit_item(item);
        }
    }
}

// ============================================================================
// Attribute normalization
// ============================================================================

fn normalize_attr(attr: &Attribute) -> String {
    use quote::ToTokens;
    let path = attr.path();
    if let Some(ident) = path.get_ident() {
        let base = ident.to_string();
        match &attr.meta {
            Meta::Path(_) => base,
            Meta::List(list) => {
                let inner = normalize_cfg_predicate(&list.tokens.to_string());
                format!("{base}({inner})")
            }
            Meta::NameValue(nv) => {
                let val = nv.value.to_token_stream().to_string();
                format!("{base} = {val}")
            }
        }
    } else {
        let segs: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        if segs.is_empty() {
            return String::new();
        }
        let base = segs.join("::");
        match &attr.meta {
            Meta::Path(_) => base,
            Meta::List(list) => {
                let inner = normalize_cfg_predicate(&list.tokens.to_string());
                format!("{base}({inner})")
            }
            Meta::NameValue(nv) => {
                let val = nv.value.to_token_stream().to_string();
                format!("{base} = {val}")
            }
        }
    }
}

fn normalize_cfg_predicate(raw: &str) -> String {
    let collapsed: String = raw.split_whitespace().collect::<Vec<&str>>().join(" ");
    collapsed.trim().to_string()
}

fn extract_cfg_attr_predicate(tokens: &str) -> Option<String> {
    let mut depth = 0i32;
    let mut comma_pos = None;
    for (i, ch) in tokens.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                comma_pos = Some(i);
                break;
            }
            _ => {}
        }
    }

    let pred_str = if let Some(pos) = comma_pos {
        &tokens[..pos]
    } else {
        tokens
    };

    let normalized = normalize_cfg_predicate(pred_str);
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

// ============================================================================
// Unit tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn test_aliases() -> CrateAliasMap {
        let mut map = BTreeMap::new();
        map.insert("amari".to_string(), "amari".to_string());
        map.insert("amari_core".to_string(), "amari-core".to_string());
        map.insert("amari_tropical".to_string(), "amari-tropical".to_string());
        map.insert("amari_dual".to_string(), "amari-dual".to_string());
        CrateAliasMap { alias_to_pkg: map }
    }

    // ---- LineOffsets ----

    #[test]
    fn line_offsets_basic() {
        let src = "line1\nline2\nline3";
        let lo = LineOffsets::from_source(src);
        assert_eq!(lo.line_col(0), Some((1, 1)));
        assert_eq!(lo.line_col(6), Some((2, 1)));
        assert_eq!(lo.line_col(12), Some((3, 1)));
        assert!(lo.line_col(100).is_some());
    }

    #[test]
    fn line_offsets_mid_line() {
        let src = "abc\ndef";
        let lo = LineOffsets::from_source(src);
        assert_eq!(lo.line_col(1), Some((1, 2)));
        assert_eq!(lo.line_col(5), Some((2, 2)));
    }

    // ---- Basic use imports ----

    #[test]
    fn test_parse_simple_use() {
        let source = "use amari::tropical::TropicalNumber;";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        assert_eq!(result.usages.len(), 1);
        assert_eq!(result.usages[0].crate_name, "amari");
        assert_eq!(result.usages[0].alias, "amari");
        assert_eq!(
            result.usages[0].path_segments,
            vec!["tropical", "TropicalNumber"]
        );
        assert_eq!(result.usages[0].kind, RustUsageKind::Use);
    }

    #[test]
    fn test_parse_direct_crate_use() {
        let source = "use amari_core::Multivector;";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        assert_eq!(result.usages.len(), 1);
        assert_eq!(result.usages[0].crate_name, "amari-core");
    }

    #[test]
    fn test_parse_glob_use() {
        let source = "use amari::tropical::*;";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        assert_eq!(result.usages.len(), 1);
    }

    #[test]
    fn test_parse_grouped_use() {
        let source = "use amari::{tropical::TropicalNumber, dual::DualNumber};";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        assert_eq!(result.usages.len(), 2);
    }

    #[test]
    fn test_parse_renamed_use() {
        let source = "use amari::dual::DualNumber as Dual;";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        assert_eq!(result.usages.len(), 1);
    }

    #[test]
    fn test_parse_bare_use_crate() {
        let source = "use amari_tropical;";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        assert_eq!(result.usages.len(), 1);
    }

    // ---- Extern crate ----

    #[test]
    fn test_parse_extern_crate() {
        let source = "extern crate amari_core;";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        assert_eq!(result.usages.len(), 1);
        assert_eq!(result.usages[0].kind, RustUsageKind::ExternCrate);
    }

    // ---- Crate attributes ----

    #[test]
    fn test_parse_no_std_crate_attr() {
        let source = "#![no_std]\n#![forbid(unsafe_code)]\nfn main() {}";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        let names: Vec<&str> = result.crate_attrs.iter().map(|(s, _)| s.as_str()).collect();
        assert!(names.contains(&"no_std"));
        assert!(names.contains(&"forbid(unsafe_code)"));
    }

    // ---- Cfg evidence ----

    #[test]
    fn test_parse_cfg() {
        let source = "#[cfg(feature = \"gpu\")]\nuse amari_core::Multivector;";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        assert!(!result.cfg_evidence.is_empty());
        assert!(result.cfg_evidence[0].cfg_predicate.contains("gpu"));
        assert!(!result.cfg_evidence[0].is_cfg_attr);
    }

    #[test]
    fn test_parse_cfg_attr() {
        let source = "#[cfg_attr(feature = \"nightly\", doc = \"nightly only\")]\nfn foo() {}";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        assert!(!result.cfg_evidence.is_empty());
        assert!(result.cfg_evidence[0].is_cfg_attr);
        assert!(result.cfg_evidence[0].cfg_predicate.contains("nightly"));
    }

    // ---- Malformed ----

    #[test]
    fn test_parse_malformed() {
        let source = "use amari::tropical::TropicalNumber";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.reason.contains("parse error"));
        assert!(err.line.is_some());
    }

    // ---- Non-amari ignored ----

    #[test]
    fn test_parse_non_amari_ignored() {
        let source = "use std::collections::HashMap;\nuse serde::Serialize;";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        assert!(result.usages.is_empty());
    }

    // ---- Path expressions and types ----

    #[test]
    fn test_parse_path_expression() {
        let source = "fn main() {\n    let x = amari_tropical::TropicalNumber::new(1.0);\n}";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        let path_exps: Vec<_> = result
            .usages
            .iter()
            .filter(|u| u.kind == RustUsageKind::PathExpression)
            .collect();
        assert!(!path_exps.is_empty());
    }

    #[test]
    fn test_parse_type_path() {
        let source = "fn foo(x: amari::tropical::TropicalNumber) {}";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        let type_paths: Vec<_> = result
            .usages
            .iter()
            .filter(|u| u.kind == RustUsageKind::PathType)
            .collect();
        assert!(!type_paths.is_empty());
    }

    // ---- PathTrait ----

    #[test]
    fn test_parse_trait_bound() {
        let source = "fn foo<T: amari_core::SomeTrait>() {}";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        let trait_bounds: Vec<_> = result
            .usages
            .iter()
            .filter(|u| u.kind == RustUsageKind::PathTrait)
            .collect();
        assert!(!trait_bounds.is_empty());
    }

    // ---- PathMacro ----

    #[test]
    fn test_parse_macro_path() {
        let source = "fn main() { amari_core::some_macro!(); }";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        let macro_paths: Vec<_> = result
            .usages
            .iter()
            .filter(|u| u.kind == RustUsageKind::PathMacro)
            .collect();
        assert!(!macro_paths.is_empty());
    }

    // ---- Doc segments ----

    #[test]
    fn test_parse_doc_segments() {
        let source = "/// First doc comment\nfn foo() {}\n/// Second doc\nfn bar() {}";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        assert!(
            result.doc_segments.len() >= 2,
            "should have at least 2 doc segments, got {}",
            result.doc_segments.len()
        );

        let texts: Vec<&str> = result
            .doc_segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(texts.iter().any(|t| t.contains("First doc")));
        assert!(texts.iter().any(|t| t.contains("Second doc")));
    }

    #[test]
    fn test_parse_doc_attribute_segment() {
        let source = "#[doc = \"explicit doc attribute\"]\nfn foo() {}";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        let texts: Vec<&str> = result
            .doc_segments
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        assert!(texts.iter().any(|t| t.contains("explicit doc")));
    }

    // ---- 1-based column ----

    #[test]
    fn test_usage_has_1based_column() {
        let source = "use amari_core::Multivector;";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        assert_eq!(result.usages.len(), 1);
        let col = result.usages[0].source.column;
        assert!(col.is_some());
        assert!(col.unwrap() >= 1);
    }

    // ---- Cfg dedup by location ----

    #[test]
    fn test_cfg_dedup() {
        let source = "#[cfg(feature = \"gpu\")]\n#[cfg(feature = \"gpu\")]\nfn foo() {}";
        let aliases = test_aliases();
        let result = parse_rust_source(source, "hash", "test.rs", &aliases).unwrap();
        let gpu_cfgs: Vec<_> = result
            .cfg_evidence
            .iter()
            .filter(|c| c.cfg_predicate.contains("gpu"))
            .collect();
        assert_eq!(gpu_cfgs.len(), 2, "distinct locations should be preserved");
    }
}
