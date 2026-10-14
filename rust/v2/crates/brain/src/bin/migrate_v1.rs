// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! One-shot migration binary: reads v1 brain data and produces a v2 .lbr file.

use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: migrate_v1 <v1_brain_dir> <output.lbr>");
        std::process::exit(1);
    }

    let v1_path = PathBuf::from(&args[1]);
    let output_path = PathBuf::from(&args[2]);

    // Ensure output directory exists.
    if let Some(parent) = output_path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create output directory");
    }

    println!("Creating fresh v2 brain...");
    let mut brain = brain::Brain::new();
    brain.set_storage(output_path.clone());

    println!("Migrating from v1 data at: {}", v1_path.display());
    println!("(This may take a minute for large brain files...)");

    match brain.migrate(&v1_path) {
        Ok(stats) => {
            println!("\n=== Migration Stats ===");
            println!("Total v1 nodes migrated: {}", stats.total_v1_nodes);
            println!("Successfully typed:      {}", stats.successfully_typed);
            println!("Remained raw:            {}", stats.remained_raw);
            println!("Typing rate:             {:.1}%", stats.typing_rate() * 100.0);
            println!("Bonds migrated:          {}", stats.bonds_migrated);
            println!("Bonds skipped:           {}", stats.bonds_skipped);

            println!("\nSaving v2 brain to: {}", output_path.display());
            brain.save().expect("Failed to save v2 brain file");

            // Report file size.
            if let Ok(meta) = std::fs::metadata(&output_path) {
                let size_mb = meta.len() as f64 / (1024.0 * 1024.0);
                println!("Output file size: {:.2} MB", size_mb);
            }

            println!("\nMigration complete!");
        }
        Err(e) => {
            eprintln!("Migration failed: {}", e);
            std::process::exit(1);
        }
    }
}
