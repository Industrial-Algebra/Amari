// SPDX-License-Identifier: MIT OR Apache-2.0
//! Generates `catalog/generated.json` from the workspace root.
//!
//! Usage: cargo run -p amari-discovery --example generate_catalog <workspace-root>
//!
//! The example canonicalizes the supplied workspace root, requires that an
//! `amari-discovery` package exists, writes only the fixed output path
//! `<root>/amari-discovery/catalog/generated.json`, and performs no other
//! filesystem writes. The output is deterministic: running the example twice
//! on the same workspace produces identical output.
//!
//! The parent directory `amari-discovery/catalog/` must already exist;
//! this example does not create arbitrary directories.

use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process;

use amari_discovery::generate_workspace_catalog;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <workspace-root>", args[0]);
        process::exit(1);
    }

    let root = &args[1];
    let root_path = Path::new(root);

    let catalog = generate_workspace_catalog(root_path).unwrap_or_else(|error| {
        eprintln!("Error generating catalog: {error}");
        process::exit(1);
    });

    // Write canonical JSON to the fixed output path.
    // Parent must already exist; do not create arbitrary directories.
    let output_dir = root_path.join("amari-discovery/catalog");
    if !output_dir.is_dir() {
        eprintln!(
            "Error: output directory {} must already exist",
            output_dir.display()
        );
        process::exit(1);
    }

    let output_path = output_dir.join("generated.json");
    let tmp_path = output_dir.join("generated.json.tmp");

    let json_bytes = {
        let mut bytes = serde_json::to_vec_pretty(&catalog).unwrap_or_else(|error| {
            eprintln!("Error serializing catalog: {error}");
            process::exit(1);
        });
        bytes.push(b'\n');
        bytes
    };

    // Atomic write with tmp cleanup on any error.
    let write_result = (|| -> Result<(), String> {
        let mut file = fs::File::create(&tmp_path)
            .map_err(|e| format!("Error creating temp file {}: {e}", tmp_path.display()))?;
        file.write_all(&json_bytes)
            .map_err(|e| format!("Error writing temp file: {e}"))?;
        file.sync_all()
            .map_err(|e| format!("Error syncing temp file: {e}"))?;
        drop(file);
        fs::rename(&tmp_path, &output_path).map_err(|e| {
            format!(
                "Error renaming {} to {}: {e}",
                tmp_path.display(),
                output_path.display()
            )
        })?;
        Ok(())
    })();

    if let Err(msg) = write_result {
        // Best-effort cleanup of the temp file.
        let _ = fs::remove_file(&tmp_path);
        eprintln!("{msg}");
        process::exit(1);
    }

    let crate_count = catalog.crates.len();
    let item_count: usize = catalog.crates.iter().map(|c| c.items.len()).sum();
    let edge_count = catalog.dependency_edges.len();
    let hash = catalog.content_hash.as_deref().unwrap_or("none");
    eprintln!(
        "Generated catalog: {crate_count} crates, {item_count} items, {edge_count} dependency edges, hash {hash}"
    );
}
