// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Test: load brain from JSON, save as bincode, reload from bincode, verify.

use std::path::PathBuf;

fn main() {
    let brain_path = PathBuf::from(r"brain");
    
    // Step 1: Load (will use JSON since no .bin exists yet)
    println!("=== Step 1: Loading brain ===");
    let mut brain = brain::Brain::load(&brain_path)
        .expect("Failed to load brain");
    let stats = brain.stats();
    println!("  Nodes: {}, Bonds: {}", stats.active_nodes, stats.active_bonds);
    
    // Step 2: Save (writes both .bin and .json)
    println!("\n=== Step 2: Saving brain (bincode + JSON) ===");
    brain.save().expect("Failed to save brain");
    
    // Check file sizes
    let bin_path = brain_path.join("brain_state.bin");
    let json_path = brain_path.join("brain_state.json");
    let bin_size = std::fs::metadata(&bin_path).map(|m| m.len()).unwrap_or(0);
    let json_size = std::fs::metadata(&json_path).map(|m| m.len()).unwrap_or(0);
    println!("  JSON: {:.1} MB", json_size as f64 / 1_048_576.0);
    println!("  Bincode: {:.1} MB", bin_size as f64 / 1_048_576.0);
    println!("  Compression: {:.0}% smaller", (1.0 - bin_size as f64 / json_size as f64) * 100.0);
    
    // Step 3: Reload from bincode
    println!("\n=== Step 3: Reloading from bincode ===");
    let mut brain2 = brain::Brain::load(&brain_path)
        .expect("Failed to reload brain from bincode");
    let stats2 = brain2.stats();
    println!("  Nodes: {}, Bonds: {}", stats2.active_nodes, stats2.active_bonds);
    
    // Step 4: Verify by recalling
    println!("\n=== Step 4: Verify recall works ===");
    let queries = vec!["big", "dog", "happy", "big bang"];
    for q in queries {
        let result = brain2.recall(q, 3, 3);
        println!("\n  Query: '{}'", q);
        for node in &result.activated_nodes {
            println!("    [{:.1}%] {}", node.contribution * 100.0, 
                &node.label[..node.label.len().min(70)]);
        }
    }
    
    // Verify counts match
    assert_eq!(stats.active_nodes, stats2.active_nodes, "Node count mismatch!");
    assert_eq!(stats.active_bonds, stats2.active_bonds, "Bond count mismatch!");
    println!("\n=== ALL CHECKS PASSED ===");
}
