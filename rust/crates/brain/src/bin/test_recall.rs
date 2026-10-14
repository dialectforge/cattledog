// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Test binary for Response Assembler v2.
//! Run: cargo run --release --bin test_recall

use std::path::PathBuf;

fn main() {
    let brain_path = PathBuf::from(r"brain");
    
    eprintln!("Loading brain...");
    let mut brain = brain::Brain::load(&brain_path)
        .expect("Failed to load brain");
    eprintln!("Brain loaded. Nodes: {}", brain.stats().active_nodes);
    
    let queries = vec![
        // Should lead with definitions
        "what is gravity",
        "what is a noun",
        "dog",
        "fish",
        // Should filter fragments and dedup well
        "energy",
        "big bang",
        "planet earth",
        // Process and Why should compose well
        "how does photosynthesis work",
        "why is the sky blue",
        // Think + Explore
        "what would happen if the sun disappeared",
        "explore energy",
        "tell me everything about water",
    ];
    
    for q in queries {
        println!("\n=== '{}' ===", q);
        let response = brain.respond(q, 3, 5);
        let text = if response.text.len() > 350 {
            format!("{}...", &response.text[..350])
        } else {
            response.text.clone()
        };
        println!("{}", text);
    }
}
