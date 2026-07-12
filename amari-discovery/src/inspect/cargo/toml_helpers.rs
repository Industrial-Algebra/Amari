// SPDX-License-Identifier: MIT OR Apache-2.0

//! Internal TOML value extraction helpers.
//!
//! These helpers operate on already-parsed `toml::Value` tables.
//! Benign `Option::unwrap_or_default` is permitted when operating on
//! already-parsed table values (e.g. defaulting missing string arrays to
//! `Vec::new()`). The strict prohibition is against swallowing file-read
//! errors, TOML parse failures, or UTF-8 conversion errors with default
//! values — those are the real safety concerns, and callers must handle
//! them explicitly.

/// Extracts a `String` value from a `toml::Value` table at the given key.
pub(super) fn toml_string(table: &toml::value::Table, key: &str) -> Option<String> {
    table.get(key).and_then(|v| v.as_str()).map(String::from)
}

/// Extracts a `bool` value from a `toml::Value` table.
pub(super) fn toml_bool(table: &toml::value::Table, key: &str) -> Option<bool> {
    table.get(key).and_then(|v| v.as_bool())
}

/// Extracts an array of strings from a `toml::Value` table without default.
pub(super) fn toml_strings_opt(table: &toml::value::Table, key: &str) -> Option<Vec<String>> {
    table.get(key).and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect()
    })
}

/// Extracts a sorted, deduplicated array of strings.
pub(super) fn toml_strings_sorted(table: &toml::value::Table, key: &str) -> Vec<String> {
    let mut v = toml_strings_opt(table, key).unwrap_or_default();
    v.sort();
    v.dedup();
    v
}

/// Parses a dependency value into its string form.
///
/// Handles:
/// - `"0.23.0"` → a plain version string
/// - `{ version = "0.23.0", features = ["std"] }` → inline table
/// - `{ workspace = true, features = ["extra"] }` → workspace inheritance
pub(super) fn parse_dep_value(value: &toml::Value) -> Option<toml::value::Table> {
    // String form: "0.23.0"
    if let Some(version) = value.as_str() {
        let mut table = toml::value::Table::new();
        table.insert(
            "version".to_string(),
            toml::Value::String(version.to_string()),
        );
        return Some(table);
    }
    // Inline table form
    value.as_table().cloned()
}

// ============================================================================
// TOML error extraction (stable, no source snippets)
// ============================================================================

/// Extract a stable line and column from a `toml::de::Error`.
///
/// Uses `toml::de::Error::span()` when available to compute line and column
/// from bounded source bytes. The source bytes are read only for span
/// computation and are never persisted or leaked.
pub(super) fn toml_line_col_from_source(
    err: &toml::de::Error,
    source: &[u8],
) -> (Option<usize>, Option<usize>) {
    // Prefer structured span when available
    if let Some(span) = err.span() {
        let line = count_newlines_up_to(source, span.start) + 1;
        let col = column_from_line_start(source, span.start);
        return (Some(line), Some(col));
    }
    // Fallback: conservative parse of Display (will be removed once
    // all toml versions we target have span support)
    toml_line_col_fallback(err)
}

/// Fallback: extract line/col from the Display format.
fn toml_line_col_fallback(err: &toml::de::Error) -> (Option<usize>, Option<usize>) {
    let msg = err.to_string();
    let line = msg
        .split(" at line ")
        .nth(1)
        .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|s| s.parse::<usize>().ok());
    let col = msg
        .split(" column ")
        .nth(1)
        .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|s| s.parse::<usize>().ok());
    (line, col)
}

/// Count newlines in `source[..pos]`.
fn count_newlines_up_to(source: &[u8], pos: usize) -> usize {
    source[..pos.min(source.len())]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
}

/// Compute 1-based column for `pos` from the last newline before it.
fn column_from_line_start(source: &[u8], pos: usize) -> usize {
    let slice = &source[..pos.min(source.len())];
    match slice.iter().rposition(|&b| b == b'\n') {
        Some(last_nl) => pos.saturating_sub(last_nl),
        None => pos.saturating_add(1),
    }
}

/// Build a stable typed malformed reason from a TOML error (never
/// contains source snippets).
pub(super) fn toml_malformed_reason(err: &toml::de::Error) -> String {
    let msg = err.to_string();
    if msg.contains("missing field") {
        "missing required field".to_string()
    } else if msg.contains("invalid type") {
        "invalid type for field".to_string()
    } else if msg.contains("duplicate") {
        "duplicate key".to_string()
    } else if msg.contains("expected") {
        "unexpected TOML syntax".to_string()
    } else if msg.contains("newline") || msg.contains("EOF") {
        "unterminated string or table".to_string()
    } else {
        "invalid TOML".to_string()
    }
}

/// Extract line and column from a formatted manifest error string.
///
/// The error string produced by [`super::manifest::parse_manifest`] uses
/// the format `"<reason> at <path> line Some(N) col Some(M)"`.
/// This helper extracts the line and column numbers for use in
/// [`CargoInspectionWarning::MalformedManifest`].
pub(super) fn toml_line_col_from_manifest_path(error_msg: &str) -> (Option<usize>, Option<usize>) {
    let line = error_msg
        .split(" line ")
        .nth(1)
        .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|s| s.parse::<usize>().ok());
    let col = error_msg
        .split(" col ")
        .nth(1)
        .and_then(|s| s.split(|c: char| !c.is_ascii_digit()).next())
        .and_then(|s| s.parse::<usize>().ok());
    (line, col)
}
