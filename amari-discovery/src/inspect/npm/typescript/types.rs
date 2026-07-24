// SPDX-License-Identifier: MIT OR Apache-2.0

//! Public evidence types for bounded TypeScript source inspection.

use serde::{Deserialize, Serialize};

use crate::inspect::snapshot::{InspectionLimit, SnapshotState, SourceLocation};
use crate::protocol::CapabilityId;

/// Bounded TypeScript source and declaration inspection result.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TypeScriptInspection {
    /// Package-scoped imports of `@justinelliottcobb/amari-wasm`.
    pub imports: Vec<TypeScriptImport>,
    /// Exports parsed from the installed generated declaration surface.
    pub declaration_exports: Vec<TypeScriptDeclarationExport>,
    /// Fixed runtime and bundler signals.
    pub runtime_signals: Vec<TypeScriptRuntimeEvidence>,
    /// Classified accepted JavaScript/TypeScript source files.
    pub file_contexts: Vec<TypeScriptFileContext>,
    /// Fixed domain vocabulary found in accepted sources.
    pub vocabulary: Vec<TypeScriptVocabularyEvidence>,
    /// Shared semantic capabilities supported by the installed declaration surface.
    pub capabilities: Vec<TypeScriptCapabilityEvidence>,
    /// Non-fatal bounded inspection warnings.
    pub warnings: Vec<TypeScriptInspectionWarning>,
    /// Deterministic framed SHA-256 over accepted source/declaration inputs.
    pub input_hash: String,
    /// Complete or typed partial state.
    pub state: SnapshotState,
    /// Number of accepted TypeScript/declaration files.
    pub inspected_file_count: u64,
    /// Total accepted input bytes.
    pub total_bytes: u64,
    /// Content-addressed accepted input locations.
    pub input_files: Vec<SourceLocation>,
}

/// One static or dynamic Amari WASM import.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct TypeScriptImport {
    /// Canonical npm package specifier.
    pub package_name: String,
    /// Original exported name for named imports.
    pub imported_name: Option<String>,
    /// Local binding name, when the syntax establishes one.
    pub local_name: Option<String>,
    /// Conservative import syntax category.
    pub kind: TypeScriptImportKind,
    /// Whether this is an `import type` declaration.
    pub type_only: bool,
    /// Source location of the import statement.
    pub source: SourceLocation,
}

/// Supported TypeScript import syntax categories.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeScriptImportKind {
    /// `{ Export as Local }` or `{ Export }`.
    Named,
    /// `* as namespace`.
    Namespace,
    /// Default import binding.
    Default,
    /// Side-effect-only static import.
    SideEffect,
    /// `import("package")`.
    Dynamic,
    /// CommonJS `require("package")`.
    Require,
}

/// One public generated declaration export.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct TypeScriptDeclarationExport {
    /// Exported identifier.
    pub name: String,
    /// Declaration kind.
    pub kind: TypeScriptExportKind,
    /// Generated declaration provenance.
    pub source: SourceLocation,
}

/// Generated declaration export kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeScriptExportKind {
    /// Exported class.
    Class,
    /// Exported top-level function.
    Function,
    /// Exported enum.
    Enum,
    /// Exported interface.
    Interface,
    /// Exported type alias.
    TypeAlias,
}

/// One classified accepted JavaScript/TypeScript source file.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct TypeScriptFileContext {
    /// Conservative project role inferred from the normalized path.
    pub role: TypeScriptFileRole,
    /// Content-addressed source location.
    pub source: SourceLocation,
}

/// Conservative JavaScript/TypeScript file roles.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeScriptFileRole {
    /// Ordinary application/library source.
    Source,
    /// Test or specification source.
    Test,
    /// Example or demo source.
    Example,
    /// Bundler/tool configuration source.
    Config,
}

/// Fixed domain vocabulary evidence without retained source text.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct TypeScriptVocabularyEvidence {
    /// Canonical fixed vocabulary term.
    pub term: String,
    /// Content-addressed source location.
    pub source: SourceLocation,
}

/// One fixed runtime or bundler signal.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct TypeScriptRuntimeEvidence {
    /// Detected fixed signal.
    pub signal: TypeScriptRuntimeSignal,
    /// Content-addressed source location.
    pub source: SourceLocation,
}

/// Supported runtime and bundler signals.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TypeScriptRuntimeSignal {
    /// Vite configuration is present.
    Vite,
    /// Webpack configuration is present.
    Webpack,
    /// Browser globals are referenced.
    Browser,
    /// Node.js globals or imports are referenced.
    Node,
    /// The WebAssembly global is referenced.
    WebAssembly,
}

/// Mapping from an installed WASM declaration path to a shared capability ID.
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub struct TypeScriptCapabilityEvidence {
    /// Authoritative `Class.method` WASM export path.
    pub wasm_path: String,
    /// Shared semantic capability ID from the embedded catalog.
    pub capability_id: CapabilityId,
    /// Installed generated declaration provenance.
    pub source: SourceLocation,
}

/// Non-fatal TypeScript inspection warning.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum TypeScriptInspectionWarning {
    /// The expected installed generated declaration file was absent.
    MissingDeclarations {
        /// Fixed normalized project-relative path.
        path: String,
    },
    /// The declaration file contained no parseable generated exports.
    MalformedDeclarations {
        /// Fixed normalized relative path.
        path: String,
        /// Sanitized fixed reason.
        reason: String,
        /// SHA-256 of the accepted malformed bytes.
        content_hash: String,
    },
    /// A candidate was not valid UTF-8.
    InvalidUtf8Source {
        /// Normalized relative path.
        path: String,
        /// SHA-256 of the accepted invalid bytes.
        content_hash: String,
    },
    /// A candidate symlink or replacement race was rejected.
    SymlinkedFile {
        /// Normalized relative path.
        path: String,
    },
    /// A candidate could not be opened/read safely.
    ReadFailure {
        /// Normalized relative path.
        path: String,
    },
    /// A candidate exceeded the configured per-file bound.
    OversizedFile {
        /// Normalized relative path.
        path: String,
        /// Configured maximum bytes.
        limit: u64,
        /// Bounded overflow evidence (`limit + 1`).
        observed: u64,
    },
    /// A candidate path contained non-UTF-8 components.
    NonUtf8Path {
        /// Sanitized parent context.
        path_hint: String,
    },
    /// A bounded resource stopped inspection.
    LimitExceeded {
        /// Typed limit evidence.
        limit: InspectionLimit,
    },
}
