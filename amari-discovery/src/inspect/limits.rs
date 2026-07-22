// SPDX-License-Identifier: MIT OR Apache-2.0

//! Resource limits for project inspection.

use serde::{Deserialize, Serialize};

use crate::capabilities::ResourceLimits;

/// Extended resource limits for filesystem project inspection.
///
/// `InspectionLimits` derives its defaults from [`ResourceLimits`] so
/// that every default value is surfaced in `amari capabilities`.
///
/// # Examples
///
/// ```
/// use amari_discovery::inspect::InspectionLimits;
/// use amari_discovery::ResourceLimits;
///
/// let limits = InspectionLimits::default();
/// let rl = ResourceLimits::default();
/// assert_eq!(limits.max_inspection_files, rl.max_inspection_files);
/// assert_eq!(limits.max_per_file_bytes, rl.max_per_file_bytes);
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct InspectionLimits {
    /// Maximum files considered by one project inspection
    /// (regular non-secret candidates, not only accepted files).
    pub max_inspection_files: u64,
    /// Maximum aggregate bytes accepted by one project inspection.
    pub max_inspection_bytes: u64,
    /// Maximum recursive traversal depth.
    pub max_traversal_depth: u64,
    /// Maximum bytes accepted from a single file.
    pub max_per_file_bytes: u64,
    /// Maximum wall-clock time for inspection in milliseconds.
    pub max_inspection_wall_millis: u64,
}

impl Default for InspectionLimits {
    fn default() -> Self {
        let rl = ResourceLimits::default();
        Self {
            max_inspection_files: rl.max_inspection_files,
            max_inspection_bytes: rl.max_inspection_bytes,
            max_traversal_depth: rl.max_traversal_depth,
            max_per_file_bytes: rl.max_per_file_bytes,
            max_inspection_wall_millis: rl.max_inspection_wall_millis,
        }
    }
}
