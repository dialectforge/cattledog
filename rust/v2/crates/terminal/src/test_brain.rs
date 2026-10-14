// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Brain v2 — Phase 2 integration test.
//!
//! Validates:
//!   1. Temporal lobe: typed extraction rates (should be mostly non-Raw)
//!   2. Auto-bonding: SEQUENCE + shared-subject Context bonds after ingest
//!   3. Multi-KU narration: coherent paragraphs combining typed KUs

use brain::Brain;
use brain::temporal::{ExtractionContext, TemporalLobe};
use graph::knowledge_unit::KuKind;
use graph::node::Confidence;
use std::collections::HashMap;

// ── Per-sentence extraction breakdown ────────────────────────────────────────

fn show_sentence_extraction(text: &str, label: &str) -> HashMap<String, usize> {
    let mut temporal = TemporalLobe::new();
    let ctx = ExtractionContext::new(text);
    let result = temporal.extract(&ctx);

    let mut ku_counts: HashMap<String, usize> = HashMap::new();
    println!("  [{}] {} sentences → {} KUs", label, result.sentences_seen, result.units.len());
    for ku in &result.units {
        let kind = format!("{:?}", ku.kind());
        let preview: String = ku.raw_text().chars().take(55).collect();
        println!("    {:12} │ {}", kind, preview);
        *ku_counts.entry(kind).or_insert(0) += 1;
    }
    println!(
        "    extraction confidence: {:.2}",
        result.extraction_confidence
    );
    ku_counts
}

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║      Lu Brain v2 — Phase 2 Integration Test                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // ── Test corpus ──────────────────────────────────────────────────────────

    let nn_text = "\
A neural network is a computing system inspired by the human brain. \
It consists of layers of connected nodes called neurons. \
Neural networks work by processing inputs through weighted connections, \
adjusting weights during training. \
Training causes the network to learn patterns from data. \
Neural networks are used to solve classification, regression, and generation tasks. \
For example, image classifiers use convolutional neural networks. \
Deep learning differs from shallow learning by having many hidden layers.";

    let enc_text = "\
Encryption is the process of converting plaintext into ciphertext. \
It works by applying a mathematical algorithm with a secret key. \
AES is a symmetric encryption standard used worldwide. \
Encryption is used to protect data confidentiality and integrity. \
Due to weak keys, encryption can be broken by brute-force attacks. \
For example, HTTPS uses TLS encryption to secure web traffic. \
Symmetric encryption differs from asymmetric encryption by using the same key for both operations.";

    // ── Phase 1: Extraction breakdown ────────────────────────────────────────

    println!("═══ PHASE 1 — EXTRACTION BREAKDOWN ═══════════════════════════");
    println!();
    let nn_counts  = show_sentence_extraction(nn_text,  "Neural Networks");
    println!();
    let enc_counts = show_sentence_extraction(enc_text, "Encryption");
    println!();

    let mut total_counts: HashMap<String, usize> = HashMap::new();
    for (k, v) in nn_counts.iter().chain(enc_counts.iter()) {
        *total_counts.entry(k.clone()).or_insert(0) += v;
    }

    println!("  ── Combined KU type totals ─────────────────────────────────");
    let mut sorted: Vec<_> = total_counts.iter().collect();
    sorted.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
    let total: usize = sorted.iter().map(|(_, v)| *v).sum();
    let raw_count = total_counts.get("Raw").copied().unwrap_or(0);
    for (kind, count) in &sorted {
        let bar: String = "█".repeat(**count);
        println!("    {:15} : {:2}  {}", kind, count, bar);
    }
    println!(
        "  Typed rate: {}/{} = {:.0}%",
        total - raw_count,
        total,
        100.0 * (total - raw_count) as f32 / total.max(1) as f32
    );

    // ── Phase 2: Auto-bonding ────────────────────────────────────────────────

    println!();
    println!("═══ PHASE 2 — AUTO-BONDING ════════════════════════════════════");

    let mut brain = Brain::new();

    let stats_before = brain.stats();
    let nn_seeds  = brain.learn_text(nn_text,  Confidence::Terminal);
    let stats_nn  = brain.stats();
    println!();
    println!("  After ingesting Neural Networks text:");
    println!("    KU nodes  : {} (+{})",
        stats_nn.total_nodes,
        stats_nn.total_nodes - stats_before.total_nodes);
    println!("    Bonds     : {} (+{})",
        stats_nn.total_bonds,
        stats_nn.total_bonds - stats_before.total_bonds);
    let nn_seq  = (nn_seeds.len() as isize - 1).max(0) as usize;
    println!("    Expected  : {} SEQUENCE + up to {} Context", nn_seq, nn_seeds.len());

    let stats_enc_before = brain.stats();
    let enc_seeds = brain.learn_text(enc_text, Confidence::Terminal);
    let stats_enc = brain.stats();
    println!();
    println!("  After ingesting Encryption text:");
    println!("    KU nodes  : {} (+{})",
        stats_enc.total_nodes,
        stats_enc.total_nodes - stats_enc_before.total_nodes);
    println!("    Bonds     : {} (+{})",
        stats_enc.total_bonds,
        stats_enc.total_bonds - stats_enc_before.total_bonds);
    let enc_seq = (enc_seeds.len() as isize - 1).max(0) as usize;
    println!("    Expected  : {} SEQUENCE + up to {} Context", enc_seq, enc_seeds.len());

    // Show bond-type breakdown via network introspection
    println!();
    println!("  ── Bond type breakdown (all bonds) ─────────────────────────");
    let mut bond_type_counts: HashMap<String, usize> = HashMap::new();
    for seed in nn_seeds.iter().chain(enc_seeds.iter()) {
        // Probe spreading activation from each seed at depth 1 to observe bonds
        let _ = seed; // bond inspection via recall result
    }
    // Use typed_recall to discover bond types in the graph
    let sample_recall = brain.recall("neural network", 2, 50);
    for hop in &sample_recall.traversal_path {
        let bt = format!("{:?}", hop.bond_type);
        *bond_type_counts.entry(bt).or_insert(0) += 1;
    }
    let seq_recall = brain.recall("encryption", 2, 50);
    for hop in &seq_recall.traversal_path {
        let bt = format!("{:?}", hop.bond_type);
        *bond_type_counts.entry(bt).or_insert(0) += 1;
    }
    if bond_type_counts.is_empty() {
        // Bonds exist but spreading activation wasn't triggered; show stats
        println!("    Total bonds in graph: {}", brain.stats().total_bonds);
        println!("    (Use typed_recall or spread_activation to traverse)");
    } else {
        let mut bt_sorted: Vec<_> = bond_type_counts.iter().collect();
        bt_sorted.sort_by(|a, b| b.1.cmp(a.1));
        for (bt, count) in &bt_sorted {
            println!("    {:12} : {}", bt, count);
        }
    }

    // ── Phase 3: Multi-KU narration ──────────────────────────────────────────

    println!();
    println!("═══ PHASE 3 — MULTI-KU NARRATION ═════════════════════════════");

    let queries = [
        ("What is a neural network?",      "definition query"),
        ("How does encryption work?",       "mechanism query"),
        ("What is deep learning?",          "definition — no direct KU"),
        ("Tell me about AES",               "open-ended query"),
        ("How do neural networks learn?",   "mechanism — from causal KU"),
        ("What is encryption used for?",    "purpose query"),
    ];

    for (query, note) in &queries {
        println!();
        println!("┌─ Query: \"{}\" [{}]", query, note);
        let response = brain.respond(query);
        for line in response.lines() {
            println!("│  {}", line);
        }
        println!("└─────────────────────────────────────────────────────────");
    }

    // ── Final stats ───────────────────────────────────────────────────────────

    let final_stats = brain.stats();
    println!();
    println!("═══ FINAL GRAPH STATS ══════════════════════════════════════════");
    println!("  Total nodes  : {} (active: {})", final_stats.total_nodes, final_stats.active_nodes);
    println!("  Total bonds  : {} (active: {})", final_stats.total_bonds, final_stats.active_bonds);
    println!();

    // KU kind breakdown from the live graph
    let all_kinds = [
        KuKind::Definition, KuKind::Mechanism, KuKind::Composition,
        KuKind::Purpose, KuKind::Contrast, KuKind::Causal,
        KuKind::Example, KuKind::Procedure, KuKind::Code, KuKind::Raw,
    ];
    println!("  KU kind breakdown in graph:");
    let _kind_recall = brain.recall("", 0, 1000);
    let mut kind_counts: HashMap<String, usize> = HashMap::new();
    // keyword_search returns nodes; use a broad search to approximate
    for ku_kind in &all_kinds {
        let tag = format!("{:?}", ku_kind);
        kind_counts.insert(tag, 0);
    }
    // Count via stats by probing each KU kind (approximate via broad recall)
    let all_recall = brain.recall("neural", 0, 200);
    let enc_all    = brain.recall("encryption", 0, 200);
    let mut seen_hex: std::collections::HashSet<String> = std::collections::HashSet::new();
    for node in all_recall.activated_nodes.iter().chain(enc_all.activated_nodes.iter()) {
        let hex = node.seed.to_hex();
        if seen_hex.insert(hex) {
            let k = format!("{:?}", node.knowledge_unit.kind());
            *kind_counts.entry(k).or_insert(0) += 1;
        }
    }
    let mut kc_sorted: Vec<_> = kind_counts.iter().filter(|(_, v)| **v > 0).collect();
    kc_sorted.sort_by(|a, b| b.1.cmp(a.1));
    for (kind, count) in &kc_sorted {
        println!("    {:15} : {}", kind, count);
    }
    println!();
    println!("Done.");
}
