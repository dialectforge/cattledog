// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! migrate_bonds — re-migrates v1 bonds into an already-migrated v2 brain.
//!
//! Use this after the initial node migration when bonds were skipped due to the
//! old `extract_seed_hex_from_bond` bug (tried to read string fields where the
//! actual format stores `{ "bytes": [u8; 32] }` objects).
//!
//! ## Usage
//!
//!   migrate_bonds [v1_brain_state.json] [v2_brain.json]
//!
//! Defaults:
//!   v1: %APPDATA%/lu-desktop/brain_state.json   (or pass a direct path)
//!   v2: %APPDATA%/lu-desktop/brain.json
//!
//! ## Strategy
//!
//! 1. Load v2 brain (already has nodes from prior migration).
//! 2. Build a label → Vec<Seed> index over all active v2 nodes.
//! 3. Parse v1 brain_state.json:
//!      a. Nodes section → v1_hex → v2_seed via label lookup.
//!      b. Bonds section → for each active bond, extract source/target hex using
//!         the fixed bytes-array decoder, resolve through the v1→v2 map, and
//!         call `brain.associate()`.
//! 4. Rebuild keyword index and save.

use brain::migration::{extract_seed_hex_from_bond, map_v1_bond_label};
use brain::Brain;
use df_core::{Seed, SeedGated};
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    println!("=== Lu Brain v2 — Migrate Bonds ===\n");

    let args: Vec<String> = std::env::args().collect();

    let v1_path = if args.len() > 1 {
        PathBuf::from(&args[1])
    } else {
        let appdata = std::env::var("APPDATA")
            .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
        PathBuf::from(&appdata).join("lu-desktop").join("brain_state.json")
    };

    let v2_path = if args.len() > 2 {
        PathBuf::from(&args[2])
    } else {
        let appdata = std::env::var("APPDATA")
            .unwrap_or_else(|_| std::env::var("HOME").unwrap_or_else(|_| ".".to_string()));
        PathBuf::from(&appdata).join("lu-desktop").join("brain.json")
    };

    println!("V1 source : {}", v1_path.display());
    println!("V2 brain  : {}\n", v2_path.display());

    // ── Validate paths ────────────────────────────────────────────────────────
    if !v1_path.exists() {
        eprintln!("V1 brain not found: {}", v1_path.display());
        std::process::exit(1);
    }
    if !v2_path.exists() {
        eprintln!("V2 brain not found: {}", v2_path.display());
        std::process::exit(1);
    }

    // ── Step 1: Load v2 brain ─────────────────────────────────────────────────
    println!("Loading v2 brain...");
    let mut brain = match Brain::load(&v2_path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("FAILED to load v2 brain: {}", e);
            std::process::exit(1);
        }
    };
    brain.set_storage(v2_path.clone());

    let initial = brain.network.stats();
    println!(
        "V2: {} active nodes, {} total bonds\n",
        initial.active_nodes, initial.total_bonds
    );

    // ── Step 2: Build label → v2 Seed index ──────────────────────────────────
    println!("Building label index from v2 nodes...");
    // Map normalised raw_text → first matching v2 Seed.
    // (Multiple nodes may share a label; we keep the first encountered.)
    let mut label_to_v2: HashMap<String, Seed> = HashMap::new();
    for node in brain.network.active_nodes() {
        let key = node.knowledge_unit.raw_text().trim().to_string();
        label_to_v2.entry(key).or_insert_with(|| node.seed().clone());
    }
    println!("  Indexed {} unique labels\n", label_to_v2.len());

    // ── Step 3: Parse v1 JSON ─────────────────────────────────────────────────
    println!("Reading v1 brain_state.json (may take a moment for large files)...");
    let json_str = match std::fs::read_to_string(&v1_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("FAILED to read v1 file: {}", e);
            std::process::exit(1);
        }
    };

    let json: serde_json::Value = match serde_json::from_str(&json_str) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("FAILED to parse v1 JSON: {}", e);
            std::process::exit(1);
        }
    };
    println!("  Parsed OK\n");

    // ── Step 3a: Build v1_hex → v2_seed map via label matching ───────────────
    println!("Building v1 → v2 node map...");
    let nodes_map = match json
        .get("network")
        .and_then(|n| n.get("nodes"))
        .and_then(|n| n.as_object())
    {
        Some(m) => m,
        None => {
            eprintln!("V1 JSON missing network.nodes");
            std::process::exit(1);
        }
    };

    let mut node_map: HashMap<String, Seed> = HashMap::new();
    let mut unmatched_nodes = 0usize;

    for (v1_hex, node_val) in nodes_map {
        let active = node_val["active"].as_bool().unwrap_or(false);
        if !active {
            continue;
        }
        let label = node_val["label"].as_str().unwrap_or("").trim().to_string();
        if label.is_empty() {
            continue;
        }
        if let Some(v2_seed) = label_to_v2.get(&label) {
            node_map.insert(v1_hex.clone(), v2_seed.clone());
        } else {
            unmatched_nodes += 1;
        }
    }

    println!(
        "  Mapped {} v1 nodes  ({} unmatched — not yet in v2)\n",
        node_map.len(),
        unmatched_nodes
    );

    // ── Step 3b: Migrate bonds ────────────────────────────────────────────────
    println!("Migrating bonds...");
    let bonds_map = match json
        .get("network")
        .and_then(|n| n.get("bonds"))
        .and_then(|b| b.as_object())
    {
        Some(m) => m,
        None => {
            println!("  V1 JSON has no network.bonds section — nothing to migrate.\n");
            return;
        }
    };

    let total_v1_bonds = bonds_map.len();
    let mut migrated = 0usize;
    let mut skipped_inactive = 0usize;
    let mut skipped_no_hex = 0usize;
    let mut skipped_no_node = 0usize;

    for (_, bond_val) in bonds_map {
        let active = bond_val["active"].as_bool().unwrap_or(true);
        if !active {
            skipped_inactive += 1;
            continue;
        }

        let src_hex = extract_seed_hex_from_bond(bond_val, "source");
        let tgt_hex = extract_seed_hex_from_bond(bond_val, "target");

        let (src_hex, tgt_hex) = match (src_hex, tgt_hex) {
            (Some(s), Some(t)) => (s, t),
            _ => {
                skipped_no_hex += 1;
                continue;
            }
        };

        let src_v2 = node_map.get(&src_hex);
        let tgt_v2 = node_map.get(&tgt_hex);

        match (src_v2, tgt_v2) {
            (Some(src), Some(tgt)) => {
                let label = bond_val["label"].as_str().unwrap_or("");
                let bond_type = map_v1_bond_label(label);
                let weight = bond_val["weight"].as_f64().unwrap_or(1.0) as f32;
                let src = src.clone();
                let tgt = tgt.clone();
                let _ = brain.associate(&src, &tgt, bond_type, weight.clamp(0.1, 5.0));
                migrated += 1;
            }
            _ => {
                skipped_no_node += 1;
            }
        }
    }

    println!("  Total v1 bonds        : {}", total_v1_bonds);
    println!("  Migrated              : {}", migrated);
    println!("  Skipped (inactive)    : {}", skipped_inactive);
    println!("  Skipped (no hex)      : {}", skipped_no_hex);
    println!("  Skipped (node missing): {}", skipped_no_node);
    println!();

    // ── Step 4: Rebuild index + save ─────────────────────────────────────────
    println!("Rebuilding keyword index...");
    brain.network.rebuild_keyword_index();

    println!("Saving v2 brain...");
    match brain.save() {
        Ok(_) => println!("  Saved: {}\n", v2_path.display()),
        Err(e) => {
            eprintln!("  SAVE FAILED: {}", e);
            std::process::exit(1);
        }
    }

    // ── Summary ───────────────────────────────────────────────────────────────
    let final_stats = brain.network.stats();
    println!("=== Summary ===");
    println!("  Bonds before          : {}", initial.total_bonds);
    println!("  Bonds after           : {}", final_stats.total_bonds);
    println!("  Net new bonds         : {}", final_stats.total_bonds.saturating_sub(initial.total_bonds));
    println!("  Active bonds          : {}", final_stats.active_bonds);

    // Sample 5 migrated bonds
    let sample: Vec<_> = brain
        .network
        .all_bonds()
        .into_iter()
        .filter(|b| b.active)
        .take(5)
        .collect();

    if !sample.is_empty() {
        println!("\n  Sample bonds:");
        for bond in &sample {
            let src = brain
                .network
                .get_node(&bond.source_seed)
                .map(|n| n.label())
                .unwrap_or_else(|| "?".to_string());
            let tgt = brain
                .network
                .get_node(&bond.target_seed)
                .map(|n| n.label())
                .unwrap_or_else(|| "?".to_string());
            let src_s: String = src.chars().take(45).collect();
            let tgt_s: String = tgt.chars().take(45).collect();
            println!(
                "    [{:?}] {:?} → {:?}  (w={:.2})",
                bond.bond_type, src_s, tgt_s, bond.weight
            );
        }
    }

    println!("\nDone.");
}
