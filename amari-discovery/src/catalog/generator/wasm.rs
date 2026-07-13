// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic WASM/TypeScript surface parsing and catalog generation.
//!
//! This module parses wasm-bindgen generated `.d.ts` declaration files into a
//! deterministic serializable model that the Amari discovery engine can index.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::CapabilityId;
use crate::{DiscoveryError, DiscoveryResult};

// ---------------------------------------------------------------------------
// Serializable surface model
// ---------------------------------------------------------------------------

/// A deterministic snapshot of a wasm-bindgen generated declaration surface.
///
/// Every field is sorted and deduplicated so that serialization produces a
/// stable witness suitable for drift detection.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WasmSurface {
    /// Schema version for the WASM surface format.
    pub schema_version: u32,
    /// SHA-256 hex of the normalized declaration surface.
    ///
    /// wasm-bindgen's low-level `InitOutput` names contain build-specific
    /// crate disambiguator hashes. Those volatile hashes are normalized
    /// before this identity is computed, while public declaration changes
    /// still change the value.
    pub source_hash: String,
    /// Human-readable scope note.
    pub description: String,
    /// Sorted class records.
    pub classes: Vec<WasmClass>,
    /// Sorted top-level function records.
    pub functions: Vec<WasmFunction>,
    /// Sorted enum records.
    pub enums: Vec<WasmEnum>,
    /// Sorted interface records.
    pub interfaces: Vec<WasmInterface>,
    /// Sorted type-alias records.
    pub type_aliases: Vec<WasmTypeAlias>,
    /// Sorted, deduplicated non-fatal parser warnings.
    pub warnings: Vec<WasmSurfaceWarning>,
    /// Optional validated capability-ID mappings.
    pub capability_mappings: Vec<WasmCapabilityMapping>,
}

impl WasmSurface {
    /// Schema version for the current surface format.
    pub const SCHEMA_VERSION: u32 = 1;
}

/// A parsed wasm-bindgen class declaration.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WasmClass {
    /// Class name.
    pub name: String,
    /// JSDoc comment block, if any.
    pub doc: Option<String>,
    /// Whether the constructor is private.
    pub private_constructor: bool,
    /// Explicit constructor signature, if present.
    pub constructor_signature: Option<String>,
    /// Sorted instance methods (non-static, non-constructor, non-free).
    pub methods: Vec<WasmMethod>,
    /// Sorted static methods.
    pub static_methods: Vec<WasmMethod>,
    /// Sorted readonly properties / getters.
    pub getters: Vec<WasmGetter>,
    /// Whether the class has a `free()` method.
    pub has_free: bool,
    /// Whether the class has `[Symbol.dispose]()`.
    pub has_dispose: bool,
}

/// A parsed method declaration.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WasmMethod {
    /// Method name.
    pub name: String,
    /// JSDoc comment block, if any.
    pub doc: Option<String>,
    /// Normalized TypeScript signature (name, parens, return type).
    pub signature: String,
    /// Whether this is a static method.
    pub is_static: bool,
}

/// A parsed readonly property / getter.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WasmGetter {
    /// Property name.
    pub name: String,
    /// TypeScript type annotation.
    pub type_annotation: String,
}

/// A parsed top-level function declaration.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WasmFunction {
    /// Function name.
    pub name: String,
    /// JSDoc comment block, if any.
    pub doc: Option<String>,
    /// Normalized TypeScript signature.
    pub signature: String,
}

/// A parsed enum declaration.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WasmEnum {
    /// Enum name.
    pub name: String,
    /// JSDoc comment block, if any.
    pub doc: Option<String>,
    /// Sorted variant records.
    pub variants: Vec<WasmEnumVariant>,
}

/// A parsed enum variant with its numeric value.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WasmEnumVariant {
    /// Variant name.
    pub name: String,
    /// JSDoc comment block, if any.
    pub doc: Option<String>,
    /// Numeric value.
    pub value: i64,
}

/// A parsed interface declaration.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WasmInterface {
    /// Interface name.
    pub name: String,
    /// JSDoc comment block, if any.
    pub doc: Option<String>,
    /// Sorted member signatures.
    pub members: Vec<WasmInterfaceMember>,
}

/// A parsed interface member.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WasmInterfaceMember {
    /// Member name.
    pub name: String,
    /// TypeScript type signature including `readonly` prefix if applicable.
    pub type_signature: String,
}

/// A parsed `export type Alias = Target` declaration.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WasmTypeAlias {
    /// Alias name.
    pub name: String,
    /// Target type expression.
    pub target: String,
}

/// A non-fatal warning encountered during parsing.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct WasmSurfaceWarning {
    /// Machine-readable warning kind.
    pub kind: String,
    /// Human-readable description.
    pub message: String,
    /// Line number in the source `.d.ts`, when available.
    pub line: Option<usize>,
}

/// A validated mapping from a WASM class/method to a shared `CapabilityId`.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct WasmCapabilityMapping {
    /// `Class.method` or `Class` qualified WASM export path.
    pub wasm_path: String,
    /// Validated Amari capability ID.
    pub capability_id: CapabilityId,
}

// ---------------------------------------------------------------------------
// wasm-bindgen volatility normalization
// ---------------------------------------------------------------------------

/// Replaces build-specific 16-hex crate disambiguators in generated names.
///
/// wasm-bindgen exposes low-level `InitOutput` members containing fragments
/// such as `wasm_bindgen_4b172f83b73aa3ee_`. The hexadecimal component is a
/// compiler/build identity, not part of Amari's API, and changes across clean
/// build environments. Only underscore-delimited 16-hex components are
/// replaced, leaving ordinary public names and type signatures unchanged.
fn normalize_disambiguators(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut normalized = Vec::with_capacity(value.len());
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let candidate_end = cursor.saturating_add(17);
        let is_disambiguator = bytes[cursor] == b'_'
            && candidate_end < bytes.len()
            && bytes[cursor + 1..candidate_end]
                .iter()
                .all(u8::is_ascii_hexdigit)
            && bytes[candidate_end] == b'_';
        if is_disambiguator {
            normalized.extend_from_slice(b"_HASH_");
            cursor = candidate_end + 1;
        } else {
            normalized.push(bytes[cursor]);
            cursor += 1;
        }
    }

    // The transformation copies complete original UTF-8 byte sequences and
    // substitutes ASCII only at an ASCII-delimited match, so failure is not
    // expected. Preserve the original defensively rather than panic.
    String::from_utf8(normalized).unwrap_or_else(|_| value.to_owned())
}

fn normalize_volatile_wasm_bindgen_names(
    classes: &mut [WasmClass],
    functions: &mut [WasmFunction],
    interfaces: &mut [WasmInterface],
    type_aliases: &mut [WasmTypeAlias],
) {
    for class in classes.iter_mut() {
        class.name = normalize_disambiguators(&class.name);
        class.constructor_signature = class
            .constructor_signature
            .as_deref()
            .map(normalize_disambiguators);
        for method in class.methods.iter_mut().chain(&mut class.static_methods) {
            method.name = normalize_disambiguators(&method.name);
            method.signature = normalize_disambiguators(&method.signature);
        }
        for getter in &mut class.getters {
            getter.name = normalize_disambiguators(&getter.name);
            getter.type_annotation = normalize_disambiguators(&getter.type_annotation);
        }
        class.methods.sort();
        class.static_methods.sort();
        class.getters.sort();
    }
    for function in functions.iter_mut() {
        function.name = normalize_disambiguators(&function.name);
        function.signature = normalize_disambiguators(&function.signature);
    }
    for interface in interfaces.iter_mut() {
        interface.name = normalize_disambiguators(&interface.name);
        for member in &mut interface.members {
            member.name = normalize_disambiguators(&member.name);
            member.type_signature = normalize_disambiguators(&member.type_signature);
        }
        interface.members.sort();
        interface.members.dedup();
    }
    for alias in type_aliases.iter_mut() {
        alias.name = normalize_disambiguators(&alias.name);
        alias.target = normalize_disambiguators(&alias.target);
    }
    classes.sort();
    functions.sort();
    interfaces.sort();
    type_aliases.sort();
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parses a wasm-bindgen generated `.d.ts` source into a deterministic
/// [`WasmSurface`].
///
/// The parser handles the standard wasm-bindgen declaration patterns:
/// exported classes with constructors, instance/static methods, getters,
/// `free()` / `[Symbol.dispose]()`, top-level functions, enums,
/// `export type` aliases, and interfaces. Unsupported declaration shapes
/// are reported as sorted, deduplicated [`WasmSurfaceWarning`] records
/// rather than causing a parse failure.
///
/// # Errors
///
/// Returns [`DiscoveryError::InvalidInput`] when the source is empty or
/// contains no recognizable declarations.
pub fn parse_wasm_surface(source: &str) -> DiscoveryResult<WasmSurface> {
    if source.trim().is_empty() {
        return Err(DiscoveryError::InvalidInput("empty .d.ts source".into()));
    }

    let mut parser = WasmSurfaceParser::new(source);
    let (mut classes, mut functions, enums, mut interfaces, mut type_aliases, warnings) =
        parser.parse();

    normalize_volatile_wasm_bindgen_names(
        &mut classes,
        &mut functions,
        &mut interfaces,
        &mut type_aliases,
    );

    // Hash the normalized parsed surface rather than raw `.d.ts` bytes.
    // wasm-bindgen embeds build-specific crate-disambiguator hashes in
    // low-level `InitOutput` member names; hashing raw source made clean CI
    // builds drift despite an unchanged public API.
    let source_hash = {
        let canonical = serde_json::to_vec(&(
            &classes,
            &functions,
            &enums,
            &interfaces,
            &type_aliases,
            &warnings,
        ))?;
        let mut hasher = Sha256::new();
        hasher.update(canonical);
        hex::encode(hasher.finalize())
    };

    if classes.is_empty()
        && functions.is_empty()
        && enums.is_empty()
        && interfaces.is_empty()
        && type_aliases.is_empty()
    {
        return Err(DiscoveryError::InvalidInput(
            "no recognizable declarations found in .d.ts source".into(),
        ));
    }

    Ok(WasmSurface {
        schema_version: WasmSurface::SCHEMA_VERSION,
        source_hash,
        description: format!(
            "{} classes, {} functions, {} enums, {} interfaces, {} type aliases",
            classes.len(),
            functions.len(),
            enums.len(),
            interfaces.len(),
            type_aliases.len(),
        ),
        classes,
        functions,
        enums,
        interfaces,
        type_aliases,
        warnings,
        capability_mappings: Vec::new(),
    })
}

/// Validates capability mappings against a known set of valid capability IDs
/// and returns a new surface with only the valid mappings retained.
///
/// Invalid mappings are reported as warnings in the returned surface.
pub fn validate_capability_mappings(
    mut surface: WasmSurface,
    valid_ids: &[CapabilityId],
) -> WasmSurface {
    let valid_set: std::collections::HashSet<_> = valid_ids.iter().collect();
    let (valid, invalid): (Vec<_>, Vec<_>) = surface
        .capability_mappings
        .into_iter()
        .partition(|mapping| valid_set.contains(&&mapping.capability_id));

    for mapping in &invalid {
        surface.warnings.push(WasmSurfaceWarning {
            kind: "invalid_capability_mapping".into(),
            message: format!(
                "WASM path `{}` maps to unknown capability `{}`",
                mapping.wasm_path, mapping.capability_id
            ),
            line: None,
        });
    }

    surface.capability_mappings = valid;
    surface.warnings.sort_by(|a, b| a.message.cmp(&b.message));
    surface.warnings.dedup_by(|a, b| a.message == b.message);
    surface
}

// ---------------------------------------------------------------------------
// Capability mapping enrichment
// ---------------------------------------------------------------------------

/// The set of canonical capability IDs that the built-in mapper recognises.
///
/// Each hardcoded ID is parsed and validated; an invalid ID returns an error
/// rather than panicking.
fn canonical_mapping_ids() -> DiscoveryResult<Vec<CapabilityId>> {
    Ok(vec![
        // -- products --
        "amari:amari-core:product:geometric-product".parse()?,
        // -- rotors --
        "amari:amari-core:rotor:rotation".parse()?,
    ])
}

/// Fast-path multivector class names whose `geometricProduct` maps to the
/// geometric-product capability.
const FAST_PATH_MULTIVECTOR_NAMES: &[&str] = &[
    "WasmMultivector030",
    "WasmMultivector110",
    "WasmMultivector200",
    "WasmMultivector210",
    "WasmMultivector300",
    "WasmMultivector310",
    "WasmMultivector410",
    "WasmMultivector500",
];

/// Fast-path rotor class names whose `apply` method maps to the rotation
/// capability.
const FAST_PATH_ROTOR_NAMES: &[&str] = &[
    "WasmRotor030",
    "WasmRotor110",
    "WasmRotor200",
    "WasmRotor210",
    "WasmRotor300",
    "WasmRotor310",
    "WasmRotor410",
    "WasmRotor500",
];

/// Builds deterministic, sorted, deduplicated capability mappings from a
/// parsed [`WasmSurface`].
///
/// Mappings are derived from actual class/method exports proven present in
/// the surface — the function only emits a mapping when both the class *and*
/// the expected method appear in `surface`.  Unmapped exports are left alone.
///
/// # Mapping rules
///
/// | WASM path | Capability ID |
/// |---|---|
/// | `WasmGenericMultivector.geometricProduct`<br>`WasmMultivector{N}.geometricProduct` (fast-path) | `amari:amari-core:product:geometric-product` |
/// | `WasmGenericRotor.apply`<br>`WasmRotor{N}.apply` (fast-path) | `amari:amari-core:rotor:rotation` |
///
/// # Errors
///
/// Returns [`DiscoveryError::InvalidId`] if any hardcoded canonical
/// [`CapabilityId`] fails to parse.
pub fn default_capability_mappings(
    surface: &WasmSurface,
) -> DiscoveryResult<Vec<WasmCapabilityMapping>> {
    let ids = canonical_mapping_ids()?;
    let gp_id = &ids[0];
    let rot_id = &ids[1];

    let mut mappings = Vec::new();

    for cls in &surface.classes {
        // --- geometricProduct → geometric-product ---
        let is_multivector = cls.name == "WasmGenericMultivector"
            || FAST_PATH_MULTIVECTOR_NAMES.contains(&cls.name.as_str());
        if is_multivector && cls.methods.iter().any(|m| m.name == "geometricProduct") {
            mappings.push(WasmCapabilityMapping {
                wasm_path: format!("{}.geometricProduct", cls.name),
                capability_id: gp_id.clone(),
            });
        }

        // --- apply → rotation ---
        let is_rotor =
            cls.name == "WasmGenericRotor" || FAST_PATH_ROTOR_NAMES.contains(&cls.name.as_str());
        if is_rotor && cls.methods.iter().any(|m| m.name == "apply") {
            mappings.push(WasmCapabilityMapping {
                wasm_path: format!("{}.apply", cls.name),
                capability_id: rot_id.clone(),
            });
        }
    }

    mappings.sort();
    mappings.dedup();
    Ok(mappings)
}

// ---------------------------------------------------------------------------
// Parser internals
// ---------------------------------------------------------------------------

/// Offset into the source string.
type Pos = usize;

struct WasmSurfaceParser {
    source: String,
    pos: Pos,
    len: usize,
    line: usize,
    warnings: Vec<WasmSurfaceWarning>,
}

impl WasmSurfaceParser {
    fn new(source: &str) -> Self {
        Self {
            source: source.to_owned(),
            pos: 0,
            len: source.len(),
            line: 1,
            warnings: Vec::new(),
        }
    }

    fn warn(&mut self, kind: &str, message: String) {
        self.warnings.push(WasmSurfaceWarning {
            kind: kind.into(),
            message,
            line: Some(self.line),
        });
    }

    // -- character/position helpers --

    fn current(&self) -> Option<char> {
        self.source[self.pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let c = self.current()?;
        let char_len = c.len_utf8();
        if c == '\n' {
            self.line += 1;
        }
        self.pos += char_len;
        Some(c)
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            // skip whitespace
            while self.pos < self.len {
                let c = self.source.as_bytes()[self.pos];
                if !c.is_ascii_whitespace() {
                    break;
                }
                self.advance();
            }

            // skip line comments
            if self.pos + 1 < self.len
                && self.source.as_bytes()[self.pos] == b'/'
                && self.source.as_bytes()[self.pos + 1] == b'/'
            {
                while self.pos < self.len {
                    let c = self.source.as_bytes()[self.pos];
                    self.advance();
                    if c == b'\n' {
                        break;
                    }
                }
                continue;
            }

            // skip block comments (but NOT JSDoc /** ... */)
            if self.pos + 2 < self.len
                && self.source.as_bytes()[self.pos] == b'/'
                && self.source.as_bytes()[self.pos + 1] == b'*'
                && self.source.as_bytes()[self.pos + 2] != b'*'
            {
                self.advance(); // /
                self.advance(); // *
                let mut depth = 1u32;
                while self.pos < self.len && depth > 0 {
                    if self.pos + 1 < self.len
                        && self.source.as_bytes()[self.pos] == b'*'
                        && self.source.as_bytes()[self.pos + 1] == b'/'
                    {
                        self.advance(); // *
                        self.advance(); // /
                        depth -= 1;
                    } else if self.pos + 1 < self.len
                        && self.source.as_bytes()[self.pos] == b'/'
                        && self.source.as_bytes()[self.pos + 1] == b'*'
                    {
                        self.advance(); // /
                        self.advance(); // *
                        depth += 1;
                    } else {
                        self.advance();
                    }
                }
                continue;
            }

            break;
        }
    }

    /// Read a JSDoc block comment if present. Returns the comment text (without
    /// delimiters) and leaves the cursor past it.
    fn read_jsdoc(&mut self) -> Option<String> {
        let saved_pos = self.pos;
        let saved_line = self.line;
        self.skip_whitespace_and_comments();

        // Check for /** ... */
        if self.pos + 3 < self.len
            && self.source.as_bytes()[self.pos] == b'/'
            && self.source.as_bytes()[self.pos + 1] == b'*'
            && self.source.as_bytes()[self.pos + 2] == b'*'
        {
            self.advance(); // /
            self.advance(); // *
            self.advance(); // *
                            // Handle empty JSDoc: /**/ — after consuming / * * we are at the
                            // closing `/` of the `*/` pair, so the doc is empty.  Distinguish
                            // from a nested `/**` that would start with `*`.
            if self.pos < self.len
                && self.source.as_bytes()[self.pos] == b'/'
                && (self.pos + 1 >= self.len || self.source.as_bytes()[self.pos + 1] != b'*')
            {
                self.advance(); // closing /
                return Some(String::new());
            }
            let start = self.pos;
            let mut depth = 1u32;
            while self.pos < self.len && depth > 0 {
                if self.pos + 1 < self.len
                    && self.source.as_bytes()[self.pos] == b'*'
                    && self.source.as_bytes()[self.pos + 1] == b'/'
                {
                    self.advance(); // *
                    self.advance(); // /
                    depth -= 1;
                } else if self.pos + 1 < self.len
                    && self.source.as_bytes()[self.pos] == b'/'
                    && self.source.as_bytes()[self.pos + 1] == b'*'
                {
                    self.advance();
                    self.advance();
                    depth += 1;
                } else {
                    self.advance();
                }
            }
            let end = self.pos.saturating_sub(2);
            let raw = self.source[start..end].trim().to_owned();
            // Strip leading * from each line
            let cleaned: String = raw
                .lines()
                .map(|line| line.trim().strip_prefix('*').unwrap_or(line).trim())
                .collect::<Vec<_>>()
                .join("\n");
            return Some(cleaned.trim().to_owned());
        }

        self.pos = saved_pos;
        self.line = saved_line;
        None
    }

    /// Parse a numeric enum initializer token into `i64`.
    ///
    /// Supports decimal (with optional `+`/`-` sign), hex (`0x`/`0X`),
    /// binary (`0b`/`0B`), and octal (`0o`/`0O`).  Returns `None` for
    /// unsupported tokens (floating-point, suffixed, etc.).
    fn parse_enum_initializer(token: &str) -> Option<i64> {
        let token = token.trim();
        if token.is_empty() {
            return None;
        }
        // Hex
        if let Some(hex) = token
            .strip_prefix("0x")
            .or_else(|| token.strip_prefix("0X"))
        {
            return i64::from_str_radix(hex, 16).ok();
        }
        // Binary
        if let Some(bin) = token
            .strip_prefix("0b")
            .or_else(|| token.strip_prefix("0B"))
        {
            return i64::from_str_radix(bin, 2).ok();
        }
        // Octal
        if let Some(oct) = token
            .strip_prefix("0o")
            .or_else(|| token.strip_prefix("0O"))
        {
            return i64::from_str_radix(oct, 8).ok();
        }
        // Decimal (with optional sign)
        // Must consist entirely of decimal digits optionally preceded by + or -
        let digits = if let Some(rest) = token.strip_prefix(&['+', '-'][..]) {
            rest
        } else {
            token
        };
        if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        token.parse::<i64>().ok()
    }

    /// Consume balanced `foo<bar<baz>>` angle-bracket text.
    fn skip_balanced_angles(&mut self) {
        if self.current() != Some('<') {
            return;
        }
        self.advance(); // <
        let mut depth = 1u32;
        while self.pos < self.len && depth > 0 {
            match self.current() {
                Some('<') => {
                    self.advance();
                    depth += 1;
                }
                Some('>') => {
                    self.advance();
                    depth -= 1;
                }
                Some('{') => {
                    self.advance();
                    self.skip_balanced_braces();
                }
                Some('[') => {
                    self.advance();
                    self.skip_balanced_brackets();
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Consume balanced `{...}` text.
    fn skip_balanced_braces(&mut self) {
        let mut depth = 1u32;
        while self.pos < self.len && depth > 0 {
            match self.current() {
                Some('{') => {
                    self.advance();
                    depth += 1;
                }
                Some('}') => {
                    self.advance();
                    depth -= 1;
                }
                Some('<') => {
                    self.advance();
                    self.skip_balanced_angles();
                }
                Some('[') => {
                    self.advance();
                    self.skip_balanced_brackets();
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn skip_balanced_brackets(&mut self) {
        let mut depth = 1u32;
        while self.pos < self.len && depth > 0 {
            match self.current() {
                Some('[') => {
                    self.advance();
                    depth += 1;
                }
                Some(']') => {
                    self.advance();
                    depth -= 1;
                }
                Some('<') => {
                    self.advance();
                    self.skip_balanced_angles();
                }
                Some('{') => {
                    self.advance();
                    self.skip_balanced_braces();
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    fn skip_balanced_parens(&mut self) {
        let mut depth = 1u32;
        while self.pos < self.len && depth > 0 {
            match self.current() {
                Some('(') => {
                    self.advance();
                    depth += 1;
                }
                Some(')') => {
                    self.advance();
                    depth -= 1;
                }
                Some('<') => {
                    self.advance();
                    self.skip_balanced_angles();
                }
                Some('{') => {
                    self.advance();
                    self.skip_balanced_braces();
                }
                Some('[') => {
                    self.advance();
                    self.skip_balanced_brackets();
                }
                _ => {
                    self.advance();
                }
            }
        }
    }

    /// Read a well-formed TS type expression after the current cursor position
    /// up to `;`, `{`, `)`, `]`, `>` or whitespace/comma at depth 0.
    fn read_type_expression(&mut self) -> Option<String> {
        let start = self.pos;
        let mut depth = 0u32;
        while self.pos < self.len {
            let c = self.source.as_bytes()[self.pos];
            match c {
                b'(' | b'<' | b'{' | b'[' => {
                    depth += 1;
                    self.advance();
                }
                b')' | b'}' | b']' => {
                    if depth == 0 {
                        break;
                    }
                    depth = depth.saturating_sub(1);
                    self.advance();
                }
                b'>' => {
                    // `>` at depth>0 closes a generic; at depth 0 it's `=>`
                    if depth > 0 {
                        depth = depth.saturating_sub(1);
                    }
                    self.advance();
                }
                b';' | b',' => {
                    if depth == 0 {
                        break;
                    }
                    self.advance();
                }
                b'\n' | b'\r' => {
                    if depth == 0 {
                        break;
                    }
                    self.advance();
                }
                _ => {
                    self.advance();
                }
            }
        }
        if self.pos > start {
            Some(self.source[start..self.pos].trim().to_owned())
        } else {
            None
        }
    }

    // -- top-level parsing --

    // -- top-level parsing --

    #[allow(clippy::type_complexity)]
    fn parse(
        &mut self,
    ) -> (
        Vec<WasmClass>,
        Vec<WasmFunction>,
        Vec<WasmEnum>,
        Vec<WasmInterface>,
        Vec<WasmTypeAlias>,
        Vec<WasmSurfaceWarning>,
    ) {
        let mut classes: Vec<WasmClass> = Vec::new();
        let mut functions: Vec<WasmFunction> = Vec::new();
        let mut enums: Vec<WasmEnum> = Vec::new();
        let mut interfaces: Vec<WasmInterface> = Vec::new();
        let mut type_aliases: Vec<WasmTypeAlias> = Vec::new();

        self.skip_whitespace_and_comments();

        while self.pos < self.len {
            self.skip_whitespace_and_comments();

            if self.pos >= self.len {
                break;
            }

            let doc = self.read_jsdoc();
            self.skip_whitespace_and_comments();

            if self.pos >= self.len {
                break;
            }

            // Check for "export" prefix
            if !self.try_consume_keyword("export") {
                // skip non-export lines
                let saved_pos = self.pos;
                self.warn("skipped", "unexpected non-export line".into());
                // advance to end of line or next export
                while self.pos < self.len {
                    if self.try_consume_keyword("export") {
                        break;
                    }
                    if self.source.as_bytes()[self.pos] == b'\n' {
                        self.advance();
                        break;
                    }
                    self.advance();
                }
                if self.pos == saved_pos {
                    self.advance(); // prevent infinite loop
                }
                continue;
            }

            self.skip_whitespace_and_comments();

            // "default" export (skip entirely)
            if self.try_consume_keyword("default") {
                self.skip_whitespace_and_comments();
                // Consume optional "function" keyword
                let _is_func = self.try_consume_keyword("function");
                // skip to the end of this declaration
                let mut brace_depth = 0u32;
                while self.pos < self.len {
                    let c = self.source.as_bytes()[self.pos];
                    if c == b'{' {
                        brace_depth += 1;
                        self.advance();
                    } else if c == b'}' {
                        if brace_depth == 0 {
                            self.advance();
                        } else {
                            brace_depth -= 1;
                            self.advance();
                        }
                    } else if c == b';' && brace_depth == 0 {
                        self.advance();
                        break;
                    } else {
                        self.advance();
                    }
                }
                continue;
            }

            // "declare" keyword
            let _declare = self.try_consume_keyword("declare");
            self.skip_whitespace_and_comments();

            // "abstract" keyword
            let _abstract = self.try_consume_keyword("abstract");
            self.skip_whitespace_and_comments();

            if self.pos >= self.len {
                break;
            }

            // Now check the declaration kind
            if self.try_consume_keyword("class") {
                if let Some(cls) = self.parse_class(doc) {
                    classes.push(cls);
                }
            } else if self.try_consume_keyword("function") {
                if let Some(func) = self.parse_top_level_function(doc) {
                    functions.push(func);
                }
            } else if self.try_consume_keyword("enum") {
                if let Some(enum_) = self.parse_enum(doc) {
                    enums.push(enum_);
                }
            } else if self.try_consume_keyword("interface") {
                if let Some(iface) = self.parse_interface(doc) {
                    interfaces.push(iface);
                }
            } else if self.try_consume_keyword("type") {
                if let Some(alias) = self.parse_type_alias() {
                    type_aliases.push(alias);
                }
            } else if self.try_consume_keyword("{") {
                // "export { Foo as Bar }" re-export aliases
                self.parse_export_aliases(&mut type_aliases);
            } else {
                // Unknown export — skip to next export or line
                self.warn(
                    "unsupported_export",
                    "unsupported export declaration shape".into(),
                );
                while self.pos < self.len {
                    if self.try_consume_keyword("export") {
                        break;
                    }
                    if self.source.as_bytes()[self.pos] == b'\n' {
                        self.advance();
                        break;
                    }
                    self.advance();
                }
            }

            self.skip_whitespace_and_comments();
        }

        // Sort everything
        classes.sort_by(|a, b| a.name.cmp(&b.name));
        for cls in &mut classes {
            cls.methods.sort();
            cls.static_methods.sort();
            cls.getters.sort();
        }
        functions.sort();
        enums.sort();
        for e in &mut enums {
            e.variants.sort();
        }
        interfaces.sort();
        for i in &mut interfaces {
            i.members.sort();
        }
        type_aliases.sort();
        self.warnings.sort_by(|a, b| a.message.cmp(&b.message));
        self.warnings.dedup_by(|a, b| a.message == b.message);

        (
            classes,
            functions,
            enums,
            interfaces,
            type_aliases,
            self.warnings.clone(),
        )
    }

    fn try_consume_keyword(&mut self, keyword: &str) -> bool {
        let remaining = &self.source[self.pos..];
        if let Some(after) = remaining.strip_prefix(keyword) {
            // must be followed by non-identifier char (or EOF)
            if after.is_empty()
                || !after
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
            {
                for _ in 0..keyword.len() {
                    self.advance();
                }
                return true;
            }
        }
        false
    }

    fn read_identifier(&mut self) -> Option<String> {
        self.skip_whitespace_and_comments();
        let start = self.pos;
        while self.pos < self.len {
            let c = self.source.as_bytes()[self.pos];
            if c.is_ascii_alphanumeric() || c == b'_' {
                self.advance();
            } else {
                break;
            }
        }
        if self.pos > start {
            Some(self.source[start..self.pos].to_owned())
        } else {
            None
        }
    }

    // -- class parsing --

    fn parse_class(&mut self, class_doc: Option<String>) -> Option<WasmClass> {
        self.skip_whitespace_and_comments();
        let name = self.read_identifier()?;

        self.skip_whitespace_and_comments();

        // Must find `{`
        if self.current() != Some('{') {
            self.warn("malformed_class", format!("class {name}: expected '{{'"));
            return None;
        }
        self.advance(); // {

        let mut methods: Vec<WasmMethod> = Vec::new();
        let mut static_methods: Vec<WasmMethod> = Vec::new();
        let mut getters: Vec<WasmGetter> = Vec::new();
        let mut has_free = false;
        let mut has_dispose = false;
        let mut private_constructor = false;
        let mut constructor_signature: Option<String> = None;

        loop {
            self.skip_whitespace_and_comments();

            if self.pos >= self.len {
                break;
            }

            if self.current() == Some('}') {
                self.advance(); // }
                self.skip_whitespace_and_comments();
                // optional trailing ;
                if self.current() == Some(';') {
                    self.advance();
                }
                break;
            }

            let member_doc = self.read_jsdoc();
            self.skip_whitespace_and_comments();

            let mut is_static = false;
            if self.try_consume_keyword("static") {
                is_static = true;
                self.skip_whitespace_and_comments();
            }

            let mut is_readonly = false;
            if self.try_consume_keyword("readonly") {
                is_readonly = true;
                self.skip_whitespace_and_comments();
            }

            // Check for constructor
            if self.try_consume_keyword("private") {
                self.skip_whitespace_and_comments();
                if self.try_consume_keyword("constructor") {
                    private_constructor = true;
                    constructor_signature = self.read_constructor_signature();
                } else {
                    // private something else — skip
                    self.warn(
                        "private_member",
                        format!("class {name}: skipping private member"),
                    );
                    self.skip_to_semicolon_or_brace();
                }
                continue;
            }

            // Skip `set` accessor (e.g. `set label(value: string | null)`)
            if self.try_consume_keyword("set") {
                self.skip_to_semicolon_or_brace();
                continue;
            }

            if self.try_consume_keyword("constructor") {
                constructor_signature = self.read_constructor_signature();
                continue;
            }

            // Check for getter (`get ` or readonly + name without parens, where
            // `get` is followed by an identifier that has `:` not `(` after it)
            let mut is_getter_keyword = false;
            if self.peek_keyword("get") {
                // Look ahead: is this `get name:` (getter) or `get(...):` (method)?
                let saved = self.pos;
                let saved_line = self.line;
                // skip "get"
                for _ in 0..3 {
                    self.advance();
                }
                self.skip_whitespace_and_comments();
                let next_ident = self.read_identifier();
                self.skip_whitespace_and_comments();
                // If next token is `:` or `(` (for `get label()` syntax), it's a getter
                // If next is `(` after the identifier without `:`, it's a method named get
                let looks_like_getter = match self.current() {
                    Some(':') => true,
                    Some('(') => {
                        // Peek further: `get label(): type` vs `get(x: number): type`
                        // If there's an identifier before `(`, it's a getter with `()`
                        next_ident.is_some()
                    }
                    _ => false,
                };
                // Restore position
                self.pos = saved;
                self.line = saved_line;
                is_getter_keyword = looks_like_getter;
            }

            if is_readonly || is_getter_keyword {
                if !is_readonly {
                    self.try_consume_keyword("get");
                }
                if let Some(getter) = self.read_getter(&name) {
                    getters.push(getter);
                    continue;
                }
            }

            // Method or special member
            // Check for [Symbol.dispose] before trying read_identifier
            if self.current() == Some('[') {
                self.advance(); // [
                                // consume through ]
                while self.pos < self.len && self.current() != Some(']') {
                    self.advance();
                }
                if self.current() == Some(']') {
                    self.advance();
                    self.skip_whitespace_and_comments();
                    if self.current() == Some('(') {
                        self.advance();
                        self.skip_balanced_parens();
                        if self.current() == Some(')') {
                            self.advance();
                        }
                        self.skip_whitespace_and_comments();
                        if self.current() == Some(':') {
                            self.advance();
                            self.read_type_expression();
                        }
                        self.skip_whitespace_and_comments();
                        if self.current() == Some(';') {
                            self.advance();
                        }
                        has_dispose = true;
                    }
                }
                continue;
            }

            // Read identifier for method name
            let ident = self.read_identifier();

            if let Some(method_name) = ident {
                // `free(): void;`
                if method_name == "free" {
                    self.skip_whitespace_and_comments();
                    if self.current() == Some('(') {
                        self.advance();
                        self.skip_balanced_parens();
                        if self.current() == Some(')') {
                            self.advance();
                        }
                        self.skip_whitespace_and_comments();
                        if self.current() == Some(':') {
                            self.advance();
                            self.read_type_expression();
                        }
                        self.skip_whitespace_and_comments();
                        if self.current() == Some(';') {
                            self.advance();
                        }
                        has_free = true;
                    }
                    continue;
                }

                let signature = self.read_method_from_name(method_name.as_str(), is_static);
                if let Some(sig) = signature {
                    let method = WasmMethod {
                        name: method_name,
                        doc: member_doc,
                        signature: sig,
                        is_static,
                    };
                    if is_static {
                        static_methods.push(method);
                    } else {
                        methods.push(method);
                    }
                }
                continue;
            }

            // Unknown member — skip line
            self.warn(
                "unknown_member",
                format!("class {name}: skipping unknown member"),
            );
            self.skip_to_semicolon_or_brace();
        }

        Some(WasmClass {
            name,
            doc: class_doc,
            private_constructor,
            constructor_signature,
            methods,
            static_methods,
            getters,
            has_free,
            has_dispose,
        })
    }

    fn read_constructor_signature(&mut self) -> Option<String> {
        self.skip_whitespace_and_comments();
        if self.current() == Some('(') {
            let start = self.pos;
            self.advance(); // (
            self.skip_balanced_parens();
            if self.current() == Some(')') {
                self.advance();
            }
            self.skip_whitespace_and_comments();
            if self.current() == Some(';') {
                self.advance();
            }
            if self.pos > start {
                return Some(
                    self.source[start..self.pos]
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" "),
                );
            }
        }
        None
    }

    fn read_getter(&mut self, class_name: &str) -> Option<WasmGetter> {
        let name = self.read_identifier()?;

        self.skip_whitespace_and_comments();
        // Handle `get name(): type` syntax (optional parens)
        if self.current() == Some('(') {
            self.advance();
            self.skip_balanced_parens();
            if self.current() == Some(')') {
                self.advance();
            }
            self.skip_whitespace_and_comments();
        }
        if self.current() == Some(':') {
            self.advance();
            let type_annotation = self.read_type_expression().unwrap_or_default();
            self.skip_whitespace_and_comments();
            if self.current() == Some(';') {
                self.advance();
            }
            Some(WasmGetter {
                name,
                type_annotation,
            })
        } else {
            self.warn(
                "malformed_getter",
                format!("class {class_name}: malformed getter {name}"),
            );
            self.skip_to_semicolon_or_brace();
            None
        }
    }

    fn peek_keyword(&self, keyword: &str) -> bool {
        let remaining = &self.source[self.pos..];
        remaining.starts_with(keyword)
            && (remaining[keyword.len()..]
                .chars()
                .next()
                .map_or(true, |c| !c.is_ascii_alphanumeric() && c != '_'))
    }

    fn read_method_from_name(&mut self, _name: &str, _is_static: bool) -> Option<String> {
        // We already consumed the name; now read params, return type, semicolon
        let start = self.pos;

        self.skip_whitespace_and_comments();
        if self.current() == Some('?') {
            self.advance();
        }

        self.skip_whitespace_and_comments();
        if self.current() == Some('(') {
            self.advance(); // (
            self.skip_balanced_parens();
            if self.current() == Some(')') {
                self.advance();
            }
        }

        self.skip_whitespace_and_comments();
        if self.current() == Some(':') {
            self.advance();
            self.read_type_expression();
        }

        // read semicolon
        self.skip_whitespace_and_comments();
        if self.current() == Some(';') {
            self.advance();
        }

        if self.pos > start {
            let raw = self.source[start..self.pos].trim().to_owned();
            let cleaned = raw
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .replace(" ,", ",")
                .replace("( ", "(")
                .replace(" )", ")")
                .replace(" :", ":")
                .trim_end_matches(';')
                .to_owned();
            Some(cleaned)
        } else {
            None
        }
    }

    fn skip_to_semicolon_or_brace(&mut self) {
        while self.pos < self.len {
            let c = self.source.as_bytes()[self.pos];
            if c == b';' || c == b'{' || c == b'}' {
                if c != b'{' && c != b'}' {
                    self.advance();
                }
                break;
            }
            self.advance();
        }
    }

    // -- top-level function --

    fn parse_top_level_function(&mut self, doc: Option<String>) -> Option<WasmFunction> {
        let name = self.read_identifier()?;
        self.skip_whitespace_and_comments();

        if self.current() != Some('(') {
            self.warn(
                "malformed_function",
                format!("function {name}: expected '('"),
            );
            return None;
        }

        let start = self.pos;
        self.advance(); // (
        self.skip_balanced_parens();
        if self.current() == Some(')') {
            self.advance();
        }

        self.skip_whitespace_and_comments();
        if self.current() == Some(':') {
            self.advance();
            self.read_type_expression();
        }

        self.skip_whitespace_and_comments();
        if self.current() == Some(';') {
            self.advance();
        }

        let raw = self.source[start..self.pos].trim().to_owned();
        let cleaned = raw
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace(" ,", ",")
            .replace("( ", "(")
            .replace(" )", ")")
            .replace(" :", ":")
            .replace(" ;", "");

        let signature = format!("{name}{cleaned}");
        Some(WasmFunction {
            name,
            doc,
            signature,
        })
    }

    // -- enum --

    fn parse_enum(&mut self, doc: Option<String>) -> Option<WasmEnum> {
        let name = self.read_identifier()?;
        self.skip_whitespace_and_comments();
        if self.current() != Some('{') {
            self.warn("malformed_enum", format!("enum {name}: expected '{{'"));
            return None;
        }
        self.advance(); // {

        let mut variants: Vec<WasmEnumVariant> = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.len || self.current() == Some('}') {
                break;
            }
            let var_doc = self.read_jsdoc();
            self.skip_whitespace_and_comments();
            let var_name = self.read_identifier();
            if let Some(vn) = var_name {
                self.skip_whitespace_and_comments();
                let mut value: i64 = 0;
                if self.current() == Some('=') {
                    self.advance();
                    self.skip_whitespace_and_comments();
                    let val_start = self.pos;
                    // Read the full initializer token: digits, hex/binary/octal
                    // prefixes, signs, until comma, brace, or whitespace.
                    while self.pos < self.len {
                        let c = self.source.as_bytes()[self.pos];
                        if c == b',' || c == b'}' || c == b';' || c.is_ascii_whitespace() {
                            break;
                        }
                        self.advance();
                    }
                    let token = self.source[val_start..self.pos].trim();
                    if !token.is_empty() {
                        if let Some(parsed) = Self::parse_enum_initializer(token) {
                            value = parsed;
                        } else {
                            self.warn(
                                "skipped_enum_variant",
                                format!(
                                    "enum {}: variant `{vn}` has unsupported initializer `{token}` — skipping variant",
                                    name
                                ),
                            );
                            // skip this variant's comma and continue
                            self.skip_whitespace_and_comments();
                            if self.current() == Some(',') {
                                self.advance();
                            }
                            continue;
                        }
                    }
                }
                variants.push(WasmEnumVariant {
                    name: vn,
                    doc: var_doc,
                    value,
                });
                self.skip_whitespace_and_comments();
                if self.current() == Some(',') {
                    self.advance();
                }
            } else {
                break;
            }
        }
        if self.current() == Some('}') {
            self.advance();
            self.skip_whitespace_and_comments();
            if self.current() == Some(';') {
                self.advance();
            }
        }

        Some(WasmEnum {
            name,
            doc,
            variants,
        })
    }

    // -- interface --

    fn parse_interface(&mut self, doc: Option<String>) -> Option<WasmInterface> {
        let name = self.read_identifier()?;
        self.skip_whitespace_and_comments();
        if self.current() != Some('{') {
            self.warn(
                "malformed_interface",
                format!("interface {name}: expected '{{'"),
            );
            return None;
        }
        self.advance(); // {

        let mut members: Vec<WasmInterfaceMember> = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.len || self.current() == Some('}') {
                break;
            }

            let is_readonly = self.try_consume_keyword("readonly");
            self.skip_whitespace_and_comments();

            let member_name = self.read_identifier();
            if let Some(mn) = member_name {
                self.skip_whitespace_and_comments();
                // skip ( ) for function-type members
                if self.current() == Some('(') {
                    self.advance();
                    self.skip_balanced_parens();
                    if self.current() == Some(')') {
                        self.advance();
                    }
                }
                if self.current() == Some(':') || self.current() == Some('?') {
                    if self.current() == Some('?') {
                        self.advance();
                    }
                    if self.current() == Some(':') {
                        self.advance();
                    }
                    let type_expr = self.read_type_expression().unwrap_or_default();
                    self.skip_whitespace_and_comments();
                    if self.current() == Some(';') {
                        self.advance();
                    }
                    let prefix = if is_readonly { "readonly " } else { "" };
                    let ts = format!("{prefix}{mn}: {type_expr}");
                    members.push(WasmInterfaceMember {
                        name: mn,
                        type_signature: ts,
                    });
                } else {
                    self.skip_to_semicolon_or_brace();
                }
            } else {
                break;
            }
        }
        if self.current() == Some('}') {
            self.advance();
            self.skip_whitespace_and_comments();
            if self.current() == Some(';') {
                self.advance();
            }
        }

        Some(WasmInterface { name, doc, members })
    }

    // -- type alias --

    fn parse_type_alias(&mut self) -> Option<WasmTypeAlias> {
        let name = self.read_identifier()?;
        self.skip_whitespace_and_comments();
        if self.current() != Some('=') {
            self.warn(
                "malformed_type_alias",
                format!("type alias {name}: expected '='"),
            );
            return None;
        }
        self.advance();
        self.skip_whitespace_and_comments();

        let target = self.read_type_expression().unwrap_or_default();
        self.skip_whitespace_and_comments();
        if self.current() == Some(';') {
            self.advance();
        }

        Some(WasmTypeAlias { name, target })
    }

    /// Parse `export { A, B as C }` re-export aliases.
    fn parse_export_aliases(&mut self, aliases: &mut Vec<WasmTypeAlias>) {
        // "{" already consumed
        loop {
            self.skip_whitespace_and_comments();
            if self.pos >= self.len {
                break;
            }
            if self.current() == Some('}') {
                self.advance();
                self.skip_whitespace_and_comments();
                if self.current() == Some(';') {
                    self.advance();
                }
                break;
            }

            let Some(source_name) = self.read_identifier() else {
                if self.current() == Some(',') {
                    self.advance();
                }
                continue;
            };

            self.skip_whitespace_and_comments();

            if self.try_consume_keyword("as") {
                self.skip_whitespace_and_comments();
                if let Some(alias) = self.read_identifier() {
                    aliases.push(WasmTypeAlias {
                        name: alias,
                        target: source_name,
                    });
                }
            }

            self.skip_whitespace_and_comments();
            if self.current() == Some(',') {
                self.advance();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_source_returns_error() {
        let err = parse_wasm_surface("").unwrap_err();
        assert!(err.to_string().contains("empty"));
    }

    #[test]
    fn parse_no_exports_returns_error() {
        let err = parse_wasm_surface("/* nothing */\n").unwrap_err();
        assert!(err.to_string().contains("no recognizable"));
    }

    #[test]
    fn parse_single_class_with_method() {
        let src = "
/** My class docs. */
export class Foo {
  free(): void;
  /**
   * Do the thing.
   */
  bar(x: number): string;
}
";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.classes.len(), 1);
        let cls = &surface.classes[0];
        assert_eq!(cls.name, "Foo");
        assert!(cls.doc.as_ref().unwrap().contains("My class docs"));
        assert_eq!(cls.methods.len(), 1);
        assert_eq!(cls.methods[0].name, "bar");
        assert_eq!(cls.methods[0].signature, "(x: number): string");
        assert!(cls.has_free);
    }

    #[test]
    fn parse_static_method() {
        let src = "
export class Foo {
  static create(x: number): Foo;
}
";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.classes[0].static_methods.len(), 1);
        assert_eq!(surface.classes[0].static_methods[0].name, "create");
        assert!(surface.classes[0].static_methods[0].is_static);
    }

    #[test]
    fn parse_private_constructor() {
        let src = "
export class Foo {
  private constructor();
  static make(): Foo;
}
";
        let surface = parse_wasm_surface(src).unwrap();
        assert!(surface.classes[0].private_constructor);
        assert_eq!(surface.classes[0].static_methods.len(), 1);
    }

    #[test]
    fn parse_getters() {
        let src = "
export class Foo {
  readonly dim: number;
  readonly basisCount: number;
}
";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.classes[0].getters.len(), 2);
        let names: Vec<_> = surface.classes[0]
            .getters
            .iter()
            .map(|g| g.name.as_str())
            .collect();
        assert!(names.contains(&"dim"));
        assert!(names.contains(&"basisCount"));
    }

    #[test]
    fn parse_top_level_function() {
        let src = "
export function init(): void;
export function add(a: number, b: number): number;
";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.functions.len(), 2);
        assert_eq!(surface.functions[0].name, "add");
        assert_eq!(surface.functions[1].name, "init");
    }

    #[test]
    fn parse_enum() {
        let src = "
export enum Color {
  Red = 0,
  Green = 1,
  Blue = 2,
}
";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.enums.len(), 1);
        assert_eq!(surface.enums[0].name, "Color");
        assert_eq!(surface.enums[0].variants.len(), 3);
    }

    #[test]
    fn parse_interface() {
        let src = "
export interface Point {
  readonly x: number;
  y: number;
}
";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.interfaces.len(), 1);
        assert_eq!(surface.interfaces[0].name, "Point");
        assert_eq!(surface.interfaces[0].members.len(), 2);
    }

    #[test]
    fn parse_type_alias() {
        let src = "
export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;
";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.type_aliases.len(), 1);
        assert_eq!(surface.type_aliases[0].name, "InitInput");
        assert!(surface.type_aliases[0]
            .target
            .contains("WebAssembly.Module"));
    }

    #[test]
    fn parse_export_alias() {
        let src = "
export { Foo, Bar as Baz };
";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.type_aliases.len(), 1);
        assert_eq!(surface.type_aliases[0].name, "Baz");
        assert_eq!(surface.type_aliases[0].target, "Bar");
    }

    #[test]
    fn parse_generic_type_in_signature() {
        let src = "
export class Generic {
  transform(input: Array<WasmFoo>): WasmFoo;
}
";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(
            surface.classes[0].methods[0].signature,
            "(input: Array<WasmFoo>): WasmFoo"
        );
    }

    #[test]
    fn parse_dispose_symbol() {
        let src = "
export class Foo {
  [Symbol.dispose](): void;
}
";
        let surface = parse_wasm_surface(src).unwrap();
        assert!(surface.classes[0].has_dispose);
    }

    #[test]
    fn validate_mappings_keeps_valid_removes_invalid() {
        let src = "
export class Foo {
  bar(): number;
}
";
        let mut surface = parse_wasm_surface(src).unwrap();
        let valid_id: CapabilityId = "amari:amari-core:product:geometric-product"
            .parse()
            .unwrap();
        surface.capability_mappings = vec![
            WasmCapabilityMapping {
                wasm_path: "Foo.bar".into(),
                capability_id: valid_id.clone(),
            },
            WasmCapabilityMapping {
                wasm_path: "Bogus.method".into(),
                capability_id: "amari:fake:module:symbol".parse().unwrap(),
            },
        ];

        let validated = validate_capability_mappings(surface, &[valid_id]);
        assert_eq!(validated.capability_mappings.len(), 1);
        assert_eq!(validated.capability_mappings[0].wasm_path, "Foo.bar");
        assert!(validated
            .warnings
            .iter()
            .any(|w| w.kind == "invalid_capability_mapping"));
    }

    #[test]
    fn parse_real_dts_fragment_geometric_product() {
        let src = "
export class WasmMultivector300 {
    free(): void;
    geometricProduct(other: WasmMultivector300): WasmMultivector300;
    add(other: WasmMultivector300): WasmMultivector300;
}
";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.classes.len(), 1);
        let cls = &surface.classes[0];
        assert_eq!(cls.name, "WasmMultivector300");
        assert!(cls.has_free);
        let method_names: Vec<_> = cls.methods.iter().map(|m| m.name.as_str()).collect();
        assert!(method_names.contains(&"geometricProduct"));
        assert!(method_names.contains(&"add"));
    }

    #[test]
    fn parse_interface_with_function_type_members() {
        let src = "
export interface TestOutput {
    readonly memory: WebAssembly.Memory;
    readonly foo_free: (a: number, b: number) => void;
    readonly bar_free: (a: number, b: number) => void;
}
";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.interfaces.len(), 1);
        assert_eq!(surface.interfaces[0].members.len(), 3);
        assert_eq!(
            surface.warnings.len(),
            0,
            "should have no warnings: {:?}",
            surface.warnings
        );
    }

    // -- default_capability_mappings --

    #[test]
    fn default_mappings_empty_surface_returns_empty() {
        let src = "export class Unrelated { bar(): number; }";
        let surface = parse_wasm_surface(src).unwrap();
        let mappings = default_capability_mappings(&surface).unwrap();
        assert!(mappings.is_empty());
    }

    #[test]
    fn default_mappings_recognises_fast_path_multivector() {
        let src = "export class WasmMultivector300 {
            geometricProduct(other: WasmMultivector300): WasmMultivector300;
        }";
        let surface = parse_wasm_surface(src).unwrap();
        let mappings = default_capability_mappings(&surface).unwrap();
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].wasm_path, "WasmMultivector300.geometricProduct");
        assert_eq!(
            mappings[0].capability_id.to_string(),
            "amari:amari-core:product:geometric-product"
        );
    }

    #[test]
    fn default_mappings_recognises_generic_multivector() {
        let src = "export class WasmGenericMultivector {
            geometricProduct(other: WasmGenericMultivector): WasmGenericMultivector;
        }";
        let surface = parse_wasm_surface(src).unwrap();
        let mappings = default_capability_mappings(&surface).unwrap();
        assert_eq!(mappings.len(), 1);
        assert_eq!(
            mappings[0].wasm_path,
            "WasmGenericMultivector.geometricProduct"
        );
    }

    #[test]
    fn default_mappings_recognises_fast_path_rotor() {
        let src = "export class WasmRotor300 {
            private constructor();
            apply(mv: WasmMultivector300): WasmMultivector300;
        }";
        let surface = parse_wasm_surface(src).unwrap();
        let mappings = default_capability_mappings(&surface).unwrap();
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].wasm_path, "WasmRotor300.apply");
        assert_eq!(
            mappings[0].capability_id.to_string(),
            "amari:amari-core:rotor:rotation"
        );
    }

    #[test]
    fn default_mappings_recognises_generic_rotor() {
        let src = "export class WasmGenericRotor {
            private constructor();
            apply(mv: WasmGenericMultivector): WasmGenericMultivector;
        }";
        let surface = parse_wasm_surface(src).unwrap();
        let mappings = default_capability_mappings(&surface).unwrap();
        assert_eq!(mappings.len(), 1);
        assert_eq!(mappings[0].wasm_path, "WasmGenericRotor.apply");
    }

    #[test]
    fn default_mappings_skips_class_without_method() {
        // WasmMultivector300 without geometricProduct should NOT be mapped
        let src = "export class WasmMultivector300 {
            add(other: WasmMultivector300): WasmMultivector300;
        }";
        let surface = parse_wasm_surface(src).unwrap();
        let mappings = default_capability_mappings(&surface).unwrap();
        assert!(mappings.is_empty());
    }

    #[test]
    fn default_mappings_is_sorted_and_deduped() {
        let src = "
export class WasmMultivector030 {
    geometricProduct(other: WasmMultivector030): WasmMultivector030;
}
export class WasmRotor030 {
    apply(mv: WasmMultivector030): WasmMultivector030;
}
export class WasmMultivector500 {
    geometricProduct(other: WasmMultivector500): WasmMultivector500;
}
";
        let surface = parse_wasm_surface(src).unwrap();
        let mappings = default_capability_mappings(&surface).unwrap();
        assert_eq!(mappings.len(), 3);
        // Must be sorted by wasm_path
        let paths: Vec<_> = mappings.iter().map(|m| m.wasm_path.as_str()).collect();
        let mut sorted = paths.clone();
        sorted.sort();
        assert_eq!(paths, sorted);
    }

    #[test]
    fn canonical_ids_parse_without_panic() {
        // Verify every hardcoded canonical ID is valid.
        // canonical_mapping_ids() returns a Result; this test unwraps to
        // confirm all canonical IDs parse at unit-test time.
        let ids: Vec<CapabilityId> = canonical_mapping_ids().unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(
            ids[0].to_string(),
            "amari:amari-core:product:geometric-product"
        );
        assert_eq!(ids[1].to_string(), "amari:amari-core:rotor:rotation");
    }

    // -- 5C3 parser robustness: non-decimal enum initializers --

    #[test]
    fn enum_hex_initializer_no_spurious_variants() {
        let src = "
export enum Flags {
    A = 0xFF,
    B = 0x10,
    C = 2,
}";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.enums.len(), 1);
        let e = &surface.enums[0];
        assert_eq!(
            e.variants.len(),
            3,
            "must have exactly 3 variants, not split hex"
        );
        let names: Vec<_> = e.variants.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, vec!["A", "B", "C"]);
        assert_eq!(e.variants[0].value, 0xFF);
        assert_eq!(e.variants[1].value, 0x10);
        assert_eq!(e.variants[2].value, 2);
    }

    #[test]
    fn enum_binary_initializer_no_spurious_variants() {
        let src = "
export enum Bits {
    Hi = 0b1000,
    Lo = 0b0001,
}";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.enums.len(), 1);
        let e = &surface.enums[0];
        assert_eq!(
            e.variants.len(),
            2,
            "must have exactly 2 variants, not split binary"
        );
        let names: Vec<_> = e.variants.iter().map(|v| v.name.as_str()).collect();
        assert_eq!(names, vec!["Hi", "Lo"]);
        assert_eq!(e.variants[0].value, 0b1000);
        assert_eq!(e.variants[1].value, 0b0001);
    }

    #[test]
    fn enum_octal_initializer_parsed() {
        let src = "
export enum OctalFlags {
    Perm = 0o755,
    Read = 0o444,
}";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.enums.len(), 1);
        let e = &surface.enums[0];
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[0].value, 0o755);
        assert_eq!(e.variants[1].value, 0o444);
    }

    #[test]
    fn enum_negative_decimal_initializer_parsed() {
        let src = "
export enum Signed {
    Minus = -1,
    Zero = 0,
    Plus = 1,
}";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.enums.len(), 1);
        let e = &surface.enums[0];
        assert_eq!(e.variants.len(), 3);
        // Sorted alphabetically: Minus, Plus, Zero
        assert_eq!(e.variants[0].name, "Minus");
        assert_eq!(e.variants[0].value, -1);
        assert_eq!(e.variants[1].name, "Plus");
        assert_eq!(e.variants[1].value, 1);
        assert_eq!(e.variants[2].name, "Zero");
        assert_eq!(e.variants[2].value, 0);
    }

    #[test]
    fn enum_leading_decimal_initializer_parsed() {
        // Leading sign followed by digits only
        let src = "
export enum Leading {
    A = +5,
    B = -3,
}";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.enums.len(), 1);
        let e = &surface.enums[0];
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[0].value, 5);
        assert_eq!(e.variants[1].value, -3);
    }

    #[test]
    fn enum_malformed_suffix_warns_and_skips_variant() {
        // e.g. `A = 42L` — not valid TS, but parser should survive
        let src = "
export enum BadSuffix {
    A = 42L,
    B = 0,
}";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.enums.len(), 1);
        let e = &surface.enums[0];
        // Variant A is skipped because 42L cannot be parsed as i64.
        // Variant B is parsed normally.
        assert_eq!(
            e.variants.len(),
            1,
            "variant A with malformed suffix must be skipped"
        );
        assert_eq!(e.variants[0].name, "B");
        assert_eq!(e.variants[0].value, 0);
        assert!(
            surface
                .warnings
                .iter()
                .any(|w| w.kind == "skipped_enum_variant"),
            "must warn about skipped variant: {:?}",
            surface.warnings
        );
    }

    #[test]
    fn enum_unsupported_initializer_warns_and_skips() {
        // Floating point literals are not valid i64 — skip with warning
        let src = "
export enum Floats {
    A = 1.5,
    B = 2,
}";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.enums.len(), 1);
        let e = &surface.enums[0];
        assert_eq!(
            e.variants.len(),
            1,
            "variant with float initializer must be skipped"
        );
        assert_eq!(e.variants[0].name, "B");
        assert_eq!(e.variants[0].value, 2);
        assert!(
            surface
                .warnings
                .iter()
                .any(|w| w.kind == "skipped_enum_variant"),
            "must warn about skipped float variant"
        );
    }

    #[test]
    fn enum_unsupported_initializer_skips_only_that_variant() {
        // Variant with unsupported initializer is skipped; later valid variants survive
        let src = "
export enum Mixed {
    Good = 1,
    Bad = 1.5,
    Better = 3,
}";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.enums.len(), 1);
        let e = &surface.enums[0];
        assert_eq!(e.variants.len(), 2);
        assert_eq!(e.variants[0].name, "Better");
        assert_eq!(e.variants[0].value, 3);
        assert_eq!(e.variants[1].name, "Good");
        assert_eq!(e.variants[1].value, 1);
        assert!(
            surface
                .warnings
                .iter()
                .any(|w| w.kind == "skipped_enum_variant"),
            "must warn about skipped variant"
        );
    }

    // -- 5C3 parser robustness: empty JSDoc --

    #[test]
    fn empty_jsdoc_before_class_parses_normally() {
        let src = "/**/
export class EmptyDoc {
    free(): void;
}";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.classes.len(), 1);
        let cls = &surface.classes[0];
        assert_eq!(cls.name, "EmptyDoc");
        // Empty JSDoc should yield None (no content) or Some("")
        assert!(
            cls.doc.as_deref().map_or(true, |d| d.is_empty()),
            "empty JSDoc must produce empty/None doc, got {:?}",
            cls.doc
        );
    }

    #[test]
    fn triple_star_empty_jsdoc_parses_normally() {
        let src = "/***/
export class TripleDoc {
    free(): void;
}";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.classes.len(), 1);
        let cls = &surface.classes[0];
        assert_eq!(cls.name, "TripleDoc");
        assert!(
            cls.doc.as_deref().map_or(true, |d| d.is_empty()),
            "empty triple-star JSDoc must produce empty/None doc, got {:?}",
            cls.doc
        );
    }

    #[test]
    fn ordinary_jsdoc_still_works() {
        let src = "/** @param x - the value */
export function testFn(x: number): void;";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.functions.len(), 1);
        let f = &surface.functions[0];
        assert_eq!(f.name, "testFn");
        assert!(f.doc.as_ref().unwrap().contains("@param x"));
    }

    #[test]
    fn multiline_jsdoc_still_works() {
        let src = "/**
 * Multi-line
 * documentation
 */
export class MultiDoc {
    free(): void;
}";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.classes.len(), 1);
        let cls = &surface.classes[0];
        assert_eq!(cls.name, "MultiDoc");
        assert!(cls.doc.as_ref().unwrap().contains("Multi-line"));
    }

    #[test]
    fn class_without_jsdoc_still_has_none_doc() {
        let src = "export class NoDoc {
    free(): void;
}";
        let surface = parse_wasm_surface(src).unwrap();
        assert_eq!(surface.classes.len(), 1);
        assert!(surface.classes[0].doc.is_none());
    }

    #[test]
    fn wasm_bindgen_disambiguator_hashes_do_not_change_the_surface() {
        let first = r#"
export class StableApi {
    free(): void;
}
export interface InitOutput {
    readonly wasm_bindgen_4b172f83b73aa3ee___convert__closures_____invoke___bool__true_: (a: number) => number;
    readonly core_e772e18dc9b1936e___result__Result: number;
}
"#;
        let second = r#"
export class StableApi {
    free(): void;
}
export interface InitOutput {
    readonly wasm_bindgen_abc868e3374577bb___convert__closures_____invoke___bool__true_: (a: number) => number;
    readonly core_4721bad40cb17f09___result__Result: number;
}
"#;

        let first = parse_wasm_surface(first).unwrap();
        let second = parse_wasm_surface(second).unwrap();

        assert_eq!(first, second);
        let serialized = serde_json::to_string(&first).unwrap();
        assert!(!serialized.contains("4b172f83b73aa3ee"));
        assert!(!serialized.contains("abc868e3374577bb"));
    }

    #[test]
    fn public_wasm_signature_change_changes_the_surface_hash() {
        let first =
            parse_wasm_surface("export function stableApi(value: number): number;").unwrap();
        let second =
            parse_wasm_surface("export function stableApi(value: string): number;").unwrap();

        assert_ne!(first.source_hash, second.source_hash);
    }
}
