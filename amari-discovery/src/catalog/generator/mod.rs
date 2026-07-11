// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic source-workspace catalog generation.

pub mod cfg;
mod exports;
pub mod inventory;
mod modules;
mod signatures;
mod traits;

pub use cfg::{
    cfg_gates, evaluate_expr, feature_default_closure, CfgExpr, CfgGate, CfgGateRecord, CfgStatus,
    CfgSurfaceKind,
};
pub use exports::{
    export_graph, ExportGraph, ExportItemKind, ExportRecord, ExportSource, ExportWarning,
    ExportWarningReason,
};
pub use inventory::{
    inventory_workspace, DependencyInventoryRecord, DependencyKind, FeatureInventoryRecord,
    PackageInventoryRecord, TargetInventoryRecord, TargetKind, WorkspaceInventory,
};
pub use modules::{module_graph, ModuleGraph, ModuleKind, ModuleRecord, ModuleVisibility};
pub use signatures::{
    signature_catalog, AggregateShape, AssociatedItem, AssociatedKind, FieldLabel, FieldShape,
    SignatureCatalog, SignatureKind, SignatureRecord, SignatureSource, VariantData, VariantField,
    VariantShape,
};
pub use traits::{
    trait_relationships, RelationshipEndpoint, SuperTraitConstraint, TraitAssociatedItem,
    TraitCatalog, TraitDefinition, TraitImplementation, TraitItemStatus,
};
