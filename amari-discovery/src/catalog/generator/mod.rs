// SPDX-License-Identifier: MIT OR Apache-2.0

//! Deterministic source-workspace catalog generation.

mod inventory;

pub use inventory::{
    inventory_workspace, DependencyInventoryRecord, DependencyKind, FeatureInventoryRecord,
    PackageInventoryRecord, TargetInventoryRecord, TargetKind, WorkspaceInventory,
};
