// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Brain quality auditor — loads the migrated v2 brain and reports quality stats.
//!
//! Usage: cargo run --bin audit_brain
//! (reads %APPDATA%/lu-desktop/brain.json)

use brain::Brain;
use graph::knowledge_unit::KuKind;
use std::collections::HashMap;
use std::path::PathBuf;

fn main() {
    let appdata = std::env::var("APPDATA").expect("APPDATA env var not set");
    let path = PathBuf::from(format!("{}/lu-desktop/brain.json", appdata));

    println!("Loading brain from: {}", path.display());
    println!("File size: {} MB", std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0) / 1_048_576);

    let mut brain = match Brain::load(&path) {
        Ok(b) => {
            println!("Brain loaded successfully.\n");
            b
        }
        Err(e) => {
            eprintln!("ERROR loading brain: {}", e);
            std::process::exit(1);
        }
    };

    // ── Node counts ──────────────────────────────────────────────────────────────

    let all_count   = brain.network.node_count();
    let active_count = brain.network.active_node_count();
    let bond_count  = brain.network.bond_count();

    println!("=== NODE COUNT ===");
    println!("  Total nodes (incl. inactive): {}", all_count);
    println!("  Active nodes:                 {}", active_count);
    println!("  Inactive/revoked nodes:       {}", all_count.saturating_sub(active_count));

    // ── Counts by KU type ────────────────────────────────────────────────────────

    let nodes = brain.network.active_nodes();

    println!("\n=== COUNTS BY KU TYPE ===");
    let mut counts: HashMap<KuKind, usize> = HashMap::new();
    for node in &nodes {
        *counts.entry(node.knowledge_unit.kind()).or_insert(0) += 1;
    }
    let ordered = [
        KuKind::Definition, KuKind::Mechanism, KuKind::Composition,
        KuKind::Purpose,    KuKind::Contrast,  KuKind::Causal,
        KuKind::Example,    KuKind::Procedure, KuKind::Code,
        KuKind::Raw,
    ];
    for kind in &ordered {
        let n = *counts.get(kind).unwrap_or(&0);
        let pct = n as f64 / active_count.max(1) as f64 * 100.0;
        println!("  {:12?}  {:6}  ({:.1}%)", kind, n, pct);
    }

    // ── Sample 20 random nodes ───────────────────────────────────────────────────

    println!("\n=== SAMPLE 20 NODES (evenly spaced) ===");
    let step = (active_count / 20).max(1);
    let mut sampled = 0usize;
    for (i, node) in nodes.iter().enumerate() {
        if i % step == 0 && sampled < 20 {
            let raw   = node.knowledge_unit.raw_text();
            let preview: String = raw.chars().take(100).collect();
            println!(
                "  [{:5}] {:12?} | {} | {}",
                i,
                node.knowledge_unit.kind(),
                node.label(),
                preview
            );
            sampled += 1;
        }
    }

    // ── Garbage detection ────────────────────────────────────────────────────────

    println!("\n=== GARBAGE DETECTION ===");

    let is_html_like = |s: &str| -> bool {
        s.contains("</") || (s.contains('<') && s.contains('>'))
    };

    let is_just_numbers = |s: &str| -> bool {
        let trimmed = s.trim();
        !trimmed.is_empty()
            && trimmed.chars().all(|c| {
                c.is_ascii_digit()
                    || c.is_whitespace()
                    || c == '.'
                    || c == '-'
                    || c == ','
                    || c == '+'
                    || c == '/'
                    || c == '%'
            })
    };

    let mut empty_count      = 0usize;
    let mut too_short_count  = 0usize;
    let mut html_count       = 0usize;
    let mut numbers_count    = 0usize;
    let mut label_freq: HashMap<String, usize> = HashMap::new();

    let mut html_examples:       Vec<String> = Vec::new();
    let mut too_short_examples:  Vec<String> = Vec::new();
    let mut numbers_examples:    Vec<String> = Vec::new();

    for node in &nodes {
        let raw   = node.knowledge_unit.raw_text();
        let label = node.label();
        *label_freq.entry(label).or_insert(0) += 1;

        let trimmed = raw.trim();
        if trimmed.is_empty() {
            empty_count += 1;
        } else if trimmed.len() < 10 {
            too_short_count += 1;
            if too_short_examples.len() < 5 {
                too_short_examples.push(format!("[{:?}] {:?}", node.knowledge_unit.kind(), trimmed));
            }
        }

        if is_html_like(raw) {
            html_count += 1;
            if html_examples.len() < 5 {
                let preview: String = raw.chars().take(120).collect();
                html_examples.push(format!("[{:?}] {}", node.knowledge_unit.kind(), preview));
            }
        }

        if is_just_numbers(raw) {
            numbers_count += 1;
            if numbers_examples.len() < 5 {
                numbers_examples.push(format!("[{:?}] {:?}", node.knowledge_unit.kind(), raw.trim()));
            }
        }
    }

    let dup_label_count: usize = label_freq.values().filter(|&&c| c > 1).count();
    let dup_extra_nodes: usize = label_freq.values().filter(|&&c| c > 1).map(|&c| c - 1).sum();

    println!("  Empty content (raw_text blank):  {}", empty_count);
    println!("  Too short (< 10 chars):          {}", too_short_count);
    println!("  HTML/XML-like content:           {}", html_count);
    println!("  Just numbers:                    {}", numbers_count);
    println!("  Duplicate labels:                {} label(s) × {} extra node(s)", dup_label_count, dup_extra_nodes);

    if !too_short_examples.is_empty() {
        println!("\n  Too-short examples:");
        for ex in &too_short_examples { println!("    {}", ex); }
    }
    if !html_examples.is_empty() {
        println!("\n  HTML/XML examples:");
        for ex in &html_examples { println!("    {}", ex); }
    }
    if !numbers_examples.is_empty() {
        println!("\n  Numbers-only examples:");
        for ex in &numbers_examples { println!("    {}", ex); }
    }

    // Top duplicate labels
    let mut dup_vec: Vec<(String, usize)> = label_freq
        .into_iter()
        .filter(|(_, c)| *c > 1)
        .collect();
    dup_vec.sort_by(|a, b| b.1.cmp(&a.1));
    if !dup_vec.is_empty() {
        println!("\n  Top 10 duplicate labels:");
        for (label, count) in dup_vec.iter().take(10) {
            println!("    x{}  {}", count, label);
        }
    }

    // ── Bond count ───────────────────────────────────────────────────────────────

    println!("\n=== BOND COUNT ===");
    println!("  Total bonds: {}", bond_count);
    if bond_count == 0 {
        println!("  (Confirmed: 0 bonds — migration did not create any bonds.)");
    } else {
        println!("  (Non-zero! Unexpected bonds exist in the migrated brain.)");
    }

    // ── rebuild_keyword_index analysis ──────────────────────────────────────────

    println!("\n=== REBUILD_KEYWORD_INDEX ===");
    println!("  rebuild_keyword_index() was ALREADY called by Brain::load().");
    println!("  It rebuilds two search indices from node raw_text:");
    println!("    keyword_index:  word  → [node_hex, ...]  (for keyword search)");
    println!("    compound_index: phrase → [node_hex, ...]  (for 2-gram/3-gram phrases)");
    println!("  It does NOT create bonds. It only builds lookup indices.");
    println!("  Keyword index words:   {}", brain.network.keyword_index_size());
    println!("  Compound index phrases:{}", brain.network.compound_index_size());

    // ── Summary ──────────────────────────────────────────────────────────────────

    let total_garbage = empty_count + too_short_count + html_count + numbers_count + dup_extra_nodes;
    let garbage_pct   = total_garbage as f64 / active_count.max(1) as f64 * 100.0;

    println!("\n=== SUMMARY ===");
    println!("  Active nodes:      {}", active_count);
    println!("  Bonds:             {}", bond_count);
    println!("  Suspect/garbage:   {} nodes ({:.1}%)", total_garbage, garbage_pct);
    println!();
    println!("  Note: counts overlap — an HTML+short node is counted in both buckets.");
    println!("  Recommendation: clean first (remove garbage), then create bonds via");
    println!("  the rumination engine's co-activation pass or a dedicated re-bond pass.");
    println!("  Bonding garbage nodes wastes graph capacity and pollutes search results.");
}
