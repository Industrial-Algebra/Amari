// Copyright (C) 2026 Industrial Algebra
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Bounded curated vocabulary extraction from source-anchored doc segments,
//! lexical comments, and README text.
//!
//! # Source anchoring
//!
//! Each doc segment carries a 0-based byte offset from the original source.
//! Comment segments also carry byte offsets. README text is scanned directly
//! from its source bytes. Every vocabulary evidence carries a 1-based
//! line/column SourceLocation derived from the original byte offset.

use crate::inspect::snapshot::SourceLocation;

use super::parser::DocSegment;
use super::types::VocabularyEvidence;

// ============================================================================
// Vocabulary allowlist
// ============================================================================

struct VocabTerm {
    normalized: &'static str,
    patterns: &'static [&'static str],
}

const VOCABULARY_TERMS: &[VocabTerm] = &[
    VocabTerm {
        normalized: "tropical_algebra",
        patterns: &[
            "tropical algebra",
            "max-plus",
            "maxplus",
            "min-plus",
            "minplus",
            "tropical semiring",
        ],
    },
    VocabTerm {
        normalized: "shortest_path",
        patterns: &["shortest path", "shortest-path", "dijkstra", "viterbi"],
    },
    VocabTerm {
        normalized: "geometric_algebra",
        patterns: &[
            "geometric algebra",
            "clifford algebra",
            "clifford",
            "multivector",
            "bivector",
            "trivector",
            "rotor",
            "geometric product",
        ],
    },
    VocabTerm {
        normalized: "autodiff",
        patterns: &[
            "autodiff",
            "automatic differentiation",
            "dual number",
            "dual numbers",
            "forward mode",
            "forward-mode",
            "reverse mode",
            "reverse-mode",
        ],
    },
    VocabTerm {
        normalized: "wasm",
        patterns: &[
            "wasm",
            "webassembly",
            "web assembly",
            "wasm-bindgen",
            "wasm-pack",
            "wasm target",
        ],
    },
    VocabTerm {
        normalized: "no_std",
        patterns: &[
            "no_std",
            "no-std",
            "#![no_std]",
            "#! [no_std]",
            "no std",
            "embedded",
            "bare-metal",
            "bare metal",
        ],
    },
    VocabTerm {
        normalized: "ffi",
        patterns: &["ffi", "foreign function interface", "c ffi", "c abi"],
    },
    VocabTerm {
        normalized: "native_linker",
        patterns: &[
            "native",
            "linker",
            "native linker",
            "native-link",
            "system linker",
            "linking",
        ],
    },
    VocabTerm {
        normalized: "blas",
        patterns: &[
            "blas",
            "lapack",
            "atlas",
            "openblas",
            "intel mkl",
            "accelerate framework",
        ],
    },
    VocabTerm {
        normalized: "gpu",
        patterns: &[
            "gpu",
            "cuda",
            "opencl",
            "vulkan",
            "compute shader",
            "wgpu",
            "webgpu",
            "gpgpu",
        ],
    },
    VocabTerm {
        normalized: "network_optimization",
        patterns: &[
            "network optimization",
            "network",
            "graph optimization",
            "propagation",
        ],
    },
    VocabTerm {
        normalized: "dynamical_systems",
        patterns: &[
            "dynamical system",
            "ode",
            "ordinary differential equation",
            "stability",
            "bifurcation",
            "chaos",
        ],
    },
    VocabTerm {
        normalized: "topology",
        patterns: &[
            "topology",
            "algebraic topology",
            "homology",
            "persistent homology",
            "morse theory",
            "simplicial",
        ],
    },
    VocabTerm {
        normalized: "information_geometry",
        patterns: &[
            "information geometry",
            "fisher information",
            "dually flat",
            "alpha connection",
        ],
    },
    VocabTerm {
        normalized: "holographic",
        patterns: &[
            "holographic",
            "vector symbolic architecture",
            "vsa",
            "hyperdimensional computing",
            "hd computing",
        ],
    },
    VocabTerm {
        normalized: "game_theory",
        patterns: &[
            "game theory",
            "combinatorial game",
            "nimber",
            "surreal number",
            "cgt",
        ],
    },
];

const MAX_VOCAB_PER_FILE: usize = 20;

// ============================================================================
// Annotated segment for source-anchored vocabulary
// ============================================================================

#[derive(Clone, Debug)]
pub(crate) struct AnnotatedSegment {
    pub text: String,
    pub byte_start: usize,
}

// ============================================================================
// Comment extraction (lexical, bounded, hardened)
// ============================================================================

pub(crate) fn extract_comment_segments(source: &str) -> Vec<AnnotatedSegment> {
    let bytes = source.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    let mut segments = Vec::new();

    while i < len {
        match bytes[i] {
            b'/' if i + 1 < len => match bytes[i + 1] {
                b'/' => {
                    i += 2;
                    let start = i;
                    while i < len && bytes[i] != b'\n' {
                        i += 1;
                    }
                    let comment_text = &source[start..i];
                    segments.push(AnnotatedSegment {
                        text: comment_text.to_string(),
                        byte_start: start,
                    });
                }
                b'*' => {
                    let comment_start = i;
                    i += 2;
                    let mut depth: u32 = 1;
                    let text_start = i;
                    let mut text_end = i;
                    while i + 1 < len && depth > 0 {
                        if bytes[i] == b'/' && bytes[i + 1] == b'*' {
                            depth += 1;
                            i += 2;
                        } else if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                            depth -= 1;
                            if depth == 0 {
                                text_end = i;
                            }
                            i += 2;
                        } else {
                            i += 1;
                        }
                    }
                    if text_end > text_start {
                        let comment_text = &source[text_start..text_end];
                        segments.push(AnnotatedSegment {
                            text: comment_text.to_string(),
                            byte_start: comment_start,
                        });
                    }
                }
                _ => {
                    i += 1;
                }
            },
            b'"' => {
                i += 1;
                while i < len {
                    match bytes[i] {
                        b'\\' => {
                            i += 2;
                        }
                        b'"' => {
                            i += 1;
                            break;
                        }
                        b'\n' => {
                            i += 1;
                            break;
                        }
                        _ => {
                            i += 1;
                        }
                    }
                }
            }
            b'r' if i + 1 < len && (bytes[i + 1] == b'"' || bytes[i + 1] == b'#') => {
                i += 1;
                let mut hash_count = 0u32;
                while i < len && bytes[i] == b'#' {
                    hash_count += 1;
                    i += 1;
                }
                if i < len && bytes[i] == b'"' {
                    i += 1;
                    loop {
                        if i >= len {
                            break;
                        }
                        if bytes[i] == b'"' {
                            i += 1;
                            let mut closing = 0u32;
                            while i < len && bytes[i] == b'#' && closing < hash_count {
                                closing += 1;
                                i += 1;
                            }
                            if closing == hash_count {
                                break;
                            }
                        } else {
                            i += 1;
                        }
                    }
                }
            }
            b'b' if i + 1 < len => match bytes[i + 1] {
                b'"' => {
                    i += 2;
                    while i < len {
                        match bytes[i] {
                            b'\\' => {
                                i += 2;
                            }
                            b'"' => {
                                i += 1;
                                break;
                            }
                            b'\n' => {
                                i += 1;
                                break;
                            }
                            _ => {
                                i += 1;
                            }
                        }
                    }
                }
                b'r' if i + 2 < len && (bytes[i + 2] == b'"' || bytes[i + 2] == b'#') => {
                    i += 2;
                    let mut hash_count = 0u32;
                    while i < len && bytes[i] == b'#' {
                        hash_count += 1;
                        i += 1;
                    }
                    if i < len && bytes[i] == b'"' {
                        i += 1;
                        loop {
                            if i >= len {
                                break;
                            }
                            if bytes[i] == b'"' {
                                i += 1;
                                let mut closing = 0u32;
                                while i < len && bytes[i] == b'#' && closing < hash_count {
                                    closing += 1;
                                    i += 1;
                                }
                                if closing == hash_count {
                                    break;
                                }
                            } else {
                                i += 1;
                            }
                        }
                    }
                }
                b'\'' => {
                    i += 2;
                    while i < len {
                        match bytes[i] {
                            b'\\' => {
                                i += 2;
                            }
                            b'\'' => {
                                i += 1;
                                break;
                            }
                            _ => {
                                i += 1;
                            }
                        }
                    }
                }
                _ => {
                    i += 1;
                }
            },
            b'\'' => {
                i += 1;
                while i < len {
                    match bytes[i] {
                        b'\\' => {
                            i += 2;
                        }
                        b'\'' => {
                            i += 1;
                            break;
                        }
                        b'\n' => {
                            i += 1;
                            break;
                        }
                        _ => {
                            i += 1;
                        }
                    }
                }
            }
            _ => {
                i += 1;
            }
        }
    }

    segments
}

// ============================================================================
// ASCII case-insensitive matching
// ============================================================================

fn find_ascii_case_insensitive(text: &str, pattern: &str) -> Option<usize> {
    let text_bytes = text.as_bytes();
    let pattern_bytes = pattern.as_bytes();
    let text_len = text_bytes.len();
    let pat_len = pattern_bytes.len();

    if pat_len == 0 || pat_len > text_len {
        return None;
    }

    for &b in pattern_bytes {
        if !b.is_ascii() {
            return None;
        }
    }

    'outer: for start in 0..=text_len.saturating_sub(pat_len) {
        for j in 0..pat_len {
            let tc = text_bytes[start + j];
            let pc = pattern_bytes[j];
            if !tc.is_ascii() {
                continue 'outer;
            }
            if !tc.eq_ignore_ascii_case(&pc) {
                continue 'outer;
            }
        }
        return Some(start);
    }

    None
}

// ============================================================================
// Vocabulary scanning with individually anchored segments
// ============================================================================

/// Scan individually anchored doc segments, comment segments, and optional
/// README source for vocabulary terms.
///
/// Doc segments carry span-derived source locations directly.
/// Comment segments and README text use byte offsets + line_offsets table.
/// Each evidence record carries a 1-based line/column.
/// Distinct occurrences (same term, different locations) in the same file
/// are preserved as separate evidence entries.
pub(crate) fn scan_vocabulary(
    doc_segments: &[DocSegment],
    comment_segments: &[AnnotatedSegment],
    readme_source: Option<&str>,
    path: &str,
    content_hash: &str,
    line_offsets: &super::parser::LineOffsets,
) -> Vec<VocabularyEvidence> {
    let mut evidence: Vec<VocabularyEvidence> = Vec::new();

    let max_per_file = MAX_VOCAB_PER_FILE;

    // Track per-file seen (term, line, column) for dedup
    let mut seen: Vec<(&'static str, Option<u32>, Option<u32>)> = Vec::new();

    // Scan doc segments with span-derived source locations
    for seg in doc_segments {
        if evidence.len() >= max_per_file {
            break;
        }
        for term in VOCABULARY_TERMS {
            if evidence.len() >= max_per_file {
                break;
            }
            for pattern in term.patterns {
                if evidence.len() >= max_per_file {
                    break;
                }
                if let Some(rel_pos) = find_ascii_case_insensitive(&seg.text, pattern) {
                    let loc = Some(SourceLocation {
                        path: seg.source.path.clone(),
                        line: seg.source.line,
                        column: seg.source.column.map(|c| c + rel_pos as u32),
                        content_hash: seg.source.content_hash.clone(),
                    });
                    push_vocab_evidence(
                        term.normalized,
                        loc,
                        &mut evidence,
                        &mut seen,
                        max_per_file,
                        path,
                    );
                    break;
                }
            }
        }
    }

    // Scan comment segments (byte offset based)
    for seg in comment_segments {
        scan_text_for_vocab(
            &seg.text,
            seg.byte_start,
            &mut evidence,
            &mut seen,
            max_per_file,
            path,
            content_hash,
            line_offsets,
        );
    }

    // Scan README text directly
    if let Some(readme) = readme_source {
        scan_text_for_vocab(
            readme,
            0,
            &mut evidence,
            &mut seen,
            max_per_file,
            path,
            content_hash,
            line_offsets,
        );
    }

    evidence.sort_by(|a, b| a.term.cmp(&b.term));
    evidence
}

/// Push a vocabulary evidence entry if not already seen (by term, line, column).
fn push_vocab_evidence(
    term: &'static str,
    loc: Option<SourceLocation>,
    evidence: &mut Vec<VocabularyEvidence>,
    seen: &mut Vec<(&'static str, Option<u32>, Option<u32>)>,
    max_per_file: usize,
    path: &str,
) {
    if evidence.len() >= max_per_file {
        return;
    }
    let key = (
        term,
        loc.as_ref().and_then(|s| s.line),
        loc.as_ref().and_then(|s| s.column),
    );
    if !seen.contains(&key) {
        seen.push(key);
        evidence.push(VocabularyEvidence {
            path: path.to_string(),
            term: term.to_string(),
            source: loc,
        });
    }
}

/// Scan text (with byte offset) for vocabulary terms.
#[allow(clippy::too_many_arguments)]
fn scan_text_for_vocab(
    text: &str,
    byte_offset: usize,
    evidence: &mut Vec<VocabularyEvidence>,
    seen: &mut Vec<(&'static str, Option<u32>, Option<u32>)>,
    max_per_file: usize,
    path: &str,
    content_hash: &str,
    line_offsets: &super::parser::LineOffsets,
) {
    if evidence.len() >= max_per_file {
        return;
    }
    for term in VOCABULARY_TERMS {
        if evidence.len() >= max_per_file {
            break;
        }
        for pattern in term.patterns {
            if evidence.len() >= max_per_file {
                break;
            }
            if let Some(rel_pos) = find_ascii_case_insensitive(text, pattern) {
                let abs_offset = byte_offset + rel_pos;
                let loc = line_offsets
                    .line_col(abs_offset)
                    .map(|(line, column)| SourceLocation {
                        path: path.to_string(),
                        line: Some(line),
                        column: Some(column),
                        content_hash: content_hash.to_string(),
                    });
                push_vocab_evidence(term.normalized, loc, evidence, seen, max_per_file, path);
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Comment extraction tests ----

    #[test]
    fn test_comment_extraction_basic() {
        let source = "// line comment\nlet x = 1; /* block comment */";
        let segs = extract_comment_segments(source);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.trim()).collect();
        assert!(texts.iter().any(|t| t.contains("line comment")));
        assert!(texts.iter().any(|t| t.contains("block comment")));
    }

    #[test]
    fn test_comment_string_literal_not_comment() {
        let source = "let s = \"// not a comment\"; // real comment";
        let segs = extract_comment_segments(source);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.trim()).collect();
        assert!(!texts.iter().any(|t| t.contains("not a comment")));
        assert!(texts.iter().any(|t| t.contains("real comment")));
    }

    #[test]
    fn test_comment_raw_string_not_comment() {
        let source = "// top comment\nlet s = r#\"// not a comment either\"#; // bottom";
        let segs = extract_comment_segments(source);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.trim()).collect();
        assert!(texts.iter().any(|t| t.contains("top comment")));
        assert!(!texts.iter().any(|t| t.contains("not a comment either")));
        assert!(texts.iter().any(|t| t.contains("bottom")));
    }

    #[test]
    fn test_comment_byte_string_not_comment() {
        let source = "// first\nlet b = b\"// inside byte string\"; // second";
        let segs = extract_comment_segments(source);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.trim()).collect();
        assert!(texts.iter().any(|t| t.contains("first")));
        assert!(!texts.iter().any(|t| t.contains("inside byte string")));
        assert!(texts.iter().any(|t| t.contains("second")));
    }

    #[test]
    fn test_comment_char_literal_not_comment() {
        let source = "let c = '\"'; // real";
        let segs = extract_comment_segments(source);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.trim()).collect();
        assert!(texts.iter().any(|t| t.contains("real")));
    }

    #[test]
    fn test_comment_byte_char_literal_not_comment() {
        let source = "let b = b'/'; // real";
        let segs = extract_comment_segments(source);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.trim()).collect();
        assert!(texts.iter().any(|t| t.contains("real")));
    }

    #[test]
    fn test_nested_block_comments() {
        let source = "/* outer /* inner */ outer2 */ // line";
        let segs = extract_comment_segments(source);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.trim()).collect();
        assert!(texts.iter().any(|t| t.contains("outer2")));
        assert!(texts.iter().any(|t| t.contains("line")));
    }

    #[test]
    fn test_comment_raw_byte_string_not_comment() {
        let source = "let s = br\"// in raw byte\"; // real";
        let segs = extract_comment_segments(source);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.trim()).collect();
        assert!(!texts.iter().any(|t| t.contains("in raw byte")));
        assert!(texts.iter().any(|t| t.contains("real")));
    }

    // ---- Raw string edge cases ----

    #[test]
    fn test_raw_string_no_hashes() {
        let source = "let s = r\"// not comment\"; // real";
        let segs = extract_comment_segments(source);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.trim()).collect();
        assert!(!texts.iter().any(|t| t.contains("not comment")));
        assert!(texts.iter().any(|t| t.contains("real")));
    }

    #[test]
    fn test_raw_string_single_hash() {
        let source = "let s = r#\"// not comment\"#; // real";
        let segs = extract_comment_segments(source);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.trim()).collect();
        assert!(!texts.iter().any(|t| t.contains("not comment")));
        assert!(texts.iter().any(|t| t.contains("real")));
    }

    #[test]
    fn test_raw_byte_string_no_hashes() {
        let source = "let s = br\"// not comment\"; // real";
        let segs = extract_comment_segments(source);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.trim()).collect();
        assert!(!texts.iter().any(|t| t.contains("not comment")));
        assert!(texts.iter().any(|t| t.contains("real")));
    }

    #[test]
    fn test_raw_byte_string_double_hash() {
        let source = "let s = br##\"// not comment\"##; // real";
        let segs = extract_comment_segments(source);
        let texts: Vec<&str> = segs.iter().map(|s| s.text.trim()).collect();
        assert!(!texts.iter().any(|t| t.contains("not comment")));
        assert!(texts.iter().any(|t| t.contains("real")));
    }

    // ---- Vocabulary scanning ----

    #[test]
    fn test_vocabulary_from_doc_segments() {
        let doc_segs = vec![DocSegment {
            text: "tropical algebra for shortest path".into(),
            source: SourceLocation {
                path: "test.rs".into(),
                line: Some(1),
                column: Some(4),
                content_hash: "hash123".into(),
            },
        }];
        let source = "// tropical algebra for shortest path";
        let line_offsets = super::super::parser::LineOffsets::from_source(source);
        let evidence = scan_vocabulary(&doc_segs, &[], None, "test.rs", "hash123", &line_offsets);
        let terms: Vec<&str> = evidence.iter().map(|e| e.term.as_str()).collect();
        assert!(terms.contains(&"tropical_algebra"));
        assert!(terms.contains(&"shortest_path"));
    }

    #[test]
    fn test_string_with_vocabulary_not_evidence() {
        let source = "// real comment about gpu\nlet s = \"tropical algebra // BLAS\";";
        let line_offsets = super::super::parser::LineOffsets::from_source(source);
        let comment_segs = extract_comment_segments(source);
        let evidence = scan_vocabulary(&[], &comment_segs, None, "test.rs", "hash", &line_offsets);
        let terms: Vec<&str> = evidence.iter().map(|e| e.term.as_str()).collect();
        assert!(terms.contains(&"gpu"));
        assert!(!terms.contains(&"tropical_algebra"));
        assert!(!terms.contains(&"blas"));
    }

    #[test]
    fn test_vocabulary_geometric_algebra() {
        let text = "Clifford algebra and multivector operations with geometric product.";
        let line_offsets = super::super::parser::LineOffsets::from_source(text);
        let evidence = scan_vocabulary(&[], &[], Some(text), "test.rs", "hash", &line_offsets);
        let terms: Vec<&str> = evidence.iter().map(|e| e.term.as_str()).collect();
        assert!(terms.contains(&"geometric_algebra"));
    }

    #[test]
    fn test_vocabulary_wasm_no_std() {
        let text = "WASM and WebAssembly target with no_std embedded support.";
        let line_offsets = super::super::parser::LineOffsets::from_source(text);
        let evidence = scan_vocabulary(&[], &[], Some(text), "test.rs", "hash", &line_offsets);
        let terms: Vec<&str> = evidence.iter().map(|e| e.term.as_str()).collect();
        assert!(terms.contains(&"wasm"));
        assert!(terms.contains(&"no_std"));
    }

    #[test]
    fn test_vocabulary_ffi_blas() {
        let text = "FFI native linker with BLAS and OpenBLAS integration.";
        let line_offsets = super::super::parser::LineOffsets::from_source(text);
        let evidence = scan_vocabulary(&[], &[], Some(text), "test.rs", "hash", &line_offsets);
        let terms: Vec<&str> = evidence.iter().map(|e| e.term.as_str()).collect();
        assert!(terms.contains(&"ffi"));
        assert!(terms.contains(&"native_linker"));
        assert!(terms.contains(&"blas"));
    }

    #[test]
    fn test_vocabulary_has_source_locations() {
        let doc_segs = vec![DocSegment {
            text: "tropical algebra for shortest path".into(),
            source: SourceLocation {
                path: "test.rs".into(),
                line: Some(1),
                column: Some(4),
                content_hash: "hash".into(),
            },
        }];
        let source = "// tropical algebra for shortest path";
        let line_offsets = super::super::parser::LineOffsets::from_source(source);
        let evidence = scan_vocabulary(&doc_segs, &[], None, "test.rs", "hash", &line_offsets);
        for ev in &evidence {
            assert!(ev.source.is_some());
            if let Some(ref src) = ev.source {
                assert!(src.line.is_some());
                assert!(src.column.is_some());
            }
        }
        assert!(!evidence.is_empty());
    }

    #[test]
    fn test_ascii_case_insensitive_match() {
        assert_eq!(find_ascii_case_insensitive("WASM target", "wasm"), Some(0));
        assert_eq!(
            find_ascii_case_insensitive("WebAssembly", "webassembly"),
            Some(0)
        );
        assert_eq!(find_ascii_case_insensitive("GPU compute", "gpu"), Some(0));
    }

    #[test]
    fn test_readme_direct_scan() {
        let readme = "# Project\n\nSupports WASM and no_std targets with GPU acceleration.";
        let line_offsets = super::super::parser::LineOffsets::from_source(readme);
        let evidence = scan_vocabulary(&[], &[], Some(readme), "README.md", "hash", &line_offsets);
        let terms: Vec<&str> = evidence.iter().map(|e| e.term.as_str()).collect();
        assert!(terms.contains(&"wasm"));
        assert!(terms.contains(&"no_std"));
        assert!(terms.contains(&"gpu"));
        for ev in &evidence {
            assert!(ev.source.is_some());
        }
    }
}
