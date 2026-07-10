// SPDX-License-Identifier: MIT OR Apache-2.0

//! Human and machine renderers for typed discovery responses.

use std::io::Write;

use serde::Serialize;

use crate::{Capabilities, DiscoveryResult, Envelope};

pub(crate) fn write_json<T: Serialize>(
    writer: &mut impl Write,
    envelope: &Envelope<T>,
) -> DiscoveryResult<()> {
    serde_json::to_writer(&mut *writer, envelope)?;
    writeln!(writer)?;
    Ok(())
}

pub(crate) fn write_capabilities_human(
    writer: &mut impl Write,
    envelope: &Envelope<Capabilities>,
) -> DiscoveryResult<()> {
    let capabilities = &envelope.data;
    let catalog_availability = if capabilities.catalog.available {
        "available"
    } else {
        "unavailable"
    };

    writeln!(writer, "Amari Discovery {}", capabilities.tool_version)?;
    writeln!(
        writer,
        "Protocol: {}",
        capabilities.protocol_versions.join(", ")
    )?;
    writeln!(
        writer,
        "Catalog: {} ({catalog_availability})",
        capabilities.catalog.version
    )?;
    writeln!(writer, "Project inspectors:")?;
    for inspector in &capabilities.project_inspectors {
        let state = if inspector.executable {
            "executable"
        } else if inspector.available {
            "available"
        } else if inspector.known {
            "known, unavailable"
        } else {
            "unknown"
        };
        writeln!(writer, "  {}: {state}", inspector.id)?;
    }
    writeln!(writer, "Output: {}", capabilities.output_modes.join(", "))?;
    writeln!(
        writer,
        "AI adapter: {}",
        if capabilities.ai_adapter.executable {
            "executable"
        } else if capabilities.ai_adapter.contract_compiled {
            "contract only"
        } else {
            "not compiled"
        }
    )?;
    Ok(())
}
