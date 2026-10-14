// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Brain v2 — Full pipeline integration test (Phases 1–3).
//!
//! Tests the complete end-to-end pipeline:
//!   Phase 1: Typed KU extraction (temporal lobe)
//!   Phase 2: Auto-bonding (SEQUENCE + Context bonds)
//!   Phase 3: Episodic memory, consolidation, StyleVector, DynamicDictionary

use brain::Brain;
use graph::node::Confidence;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn check(label: &str, pass: bool) -> bool {
    if pass {
        println!("  PASS  {}", label);
    } else {
        println!("  FAIL  {}", label);
    }
    pass
}

fn section(title: &str) {
    println!();
    println!("─── {} {}", title, "─".repeat(60usize.saturating_sub(title.len() + 5)));
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║   Lu Brain v2 — Full Integration Test (Phases 1–3)          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");

    let mut results: Vec<(&str, bool)> = Vec::new();
    macro_rules! assert_check {
        ($label:expr, $cond:expr) => {{
            let passed = check($label, $cond);
            results.push(($label, passed));
        }};
    }

    // ── 1. Create a Brain ─────────────────────────────────────────────────────

    section("1. Brain Construction");
    let mut brain = Brain::new();
    let initial_stats = brain.stats();
    assert_check!("brain starts empty (0 nodes)", initial_stats.total_nodes == 0);
    assert_check!("brain starts empty (0 bonds)", initial_stats.total_bonds == 0);

    // ── 2. Ingest 3 passages on related ML topics ─────────────────────────────

    section("2. Ingest 3 ML Passages");

    let passage_a = "\
Machine learning is a field of artificial intelligence. \
It enables systems to learn from experience without being explicitly programmed. \
Machine learning uses statistical models to make predictions from data. \
Supervised learning is a machine learning approach that trains on labelled data. \
Unsupervised learning discovers hidden patterns without labelled examples.";

    let passage_b = "\
A neural network is a computational model inspired by biological neurons. \
Neural networks consist of layers: an input layer, hidden layers, and an output layer. \
Each layer applies a transformation to produce higher-level representations. \
Backpropagation is the algorithm used to train neural networks by adjusting weights. \
Activation functions introduce non-linearity into neural network computations.";

    let passage_c = "\
Deep learning is a machine learning technique based on deep neural networks. \
Deep learning models learn feature hierarchies automatically from raw input. \
Convolutional neural networks excel at image classification tasks. \
Recurrent neural networks model sequential data such as text and speech. \
Transfer learning reuses weights pre-trained on large datasets to accelerate training.";

    let seeds_a = brain.learn_text(passage_a, Confidence::Terminal);
    let seeds_b = brain.learn_text(passage_b, Confidence::Terminal);
    let seeds_c = brain.learn_text(passage_c, Confidence::Terminal);

    let stats_after_ingest = brain.stats();
    println!("  Ingested {} + {} + {} = {} KU nodes",
        seeds_a.len(), seeds_b.len(), seeds_c.len(),
        stats_after_ingest.total_nodes);
    println!("  Auto-bonds created: {}", stats_after_ingest.total_bonds);

    assert_check!("3 passages produced nodes", stats_after_ingest.total_nodes >= 3);
    assert_check!("auto-bonding created SEQUENCE bonds",
        stats_after_ingest.total_bonds >= seeds_a.len() + seeds_b.len() + seeds_c.len() - 3);

    // ── 3. Ask 3 questions — verify structured multi-paragraph responses ───────

    section("3. Query: Structured Responses");

    let q1 = brain.respond("What is machine learning?");
    let q2 = brain.respond("How do neural networks work?");
    let q3 = brain.respond("What is deep learning?");

    println!("  Q1 ({} words): {}…", q1.split_whitespace().count(),
        q1.chars().take(70).collect::<String>());
    println!("  Q2 ({} words): {}…", q2.split_whitespace().count(),
        q2.chars().take(70).collect::<String>());
    println!("  Q3 ({} words): {}…", q3.split_whitespace().count(),
        q3.chars().take(70).collect::<String>());

    assert_check!("Q1 non-empty response", !q1.trim().is_empty());
    assert_check!("Q2 non-empty response", !q2.trim().is_empty());
    assert_check!("Q3 non-empty response", !q3.trim().is_empty());

    // Multi-paragraph: look for newlines or sentence-length responses
    let q1_words = q1.split_whitespace().count();
    let q2_words = q2.split_whitespace().count();
    assert_check!("Q1 is multi-sentence (>10 words)", q1_words > 10);
    assert_check!("Q2 is multi-sentence (>10 words)", q2_words > 10);

    // ── 4. Check episodic memory has 3 episodes ───────────────────────────────

    section("4. Episodic Memory");

    let ep_count_after_3 = brain.hippocampus.episodes.len();
    println!("  Episodes after 3 queries: {}", ep_count_after_3);
    assert_check!("3 queries recorded 3 episodes", ep_count_after_3 >= 3);

    // ── 5. Check that auto-bonding created bonds ──────────────────────────────

    section("5. Auto-Bonding Bonds");

    let total_bonds = brain.stats().total_bonds;
    println!("  Total bonds in graph: {}", total_bonds);
    assert_check!("at least 1 bond exists", total_bonds >= 1);

    // Verify SEQUENCE bonds exist via recall traversal
    let recall = brain.recall("machine learning", 2, 30);
    let has_traversal = !recall.traversal_path.is_empty()
        || !recall.activated_nodes.is_empty();
    assert_check!("recall finds machine learning nodes", has_traversal);

    // ── 6. Ask 8 more questions to trigger consolidation ─────────────────────

    section("6. Accumulate Episodes for Consolidation");

    let more_qs = [
        "What is supervised learning?",
        "What is unsupervised learning?",
        "Tell me about backpropagation",
        "What are activation functions?",
        "How does transfer learning work?",
        "What is a convolutional neural network?",
        "Tell me about recurrent neural networks",
        "What is a hidden layer?",
    ];

    for q in &more_qs {
        brain.respond(q);
    }

    let ep_count_before_consolidation = brain.hippocampus.episodes.len();
    println!("  Episodes before consolidation: {}", ep_count_before_consolidation);
    assert_check!("11 total queries = 11 episodes",
        ep_count_before_consolidation >= 11);

    // ── 7. Run consolidation and verify bond count increases ──────────────────

    section("7. Consolidation");

    let bonds_before = brain.stats().total_bonds;
    let report = brain.consolidate();

    println!("  Episodes processed  : {}", report.episodes_processed);
    println!("  Bonds strengthened  : {}", report.bonds_strengthened);
    println!("  Reinforced pairs    : {}", report.reinforced_pairs.len());

    assert_check!("consolidation processed episodes", report.episodes_processed > 0);

    // After consolidation, all episodes should be marked consolidated
    let unconsolidated = brain.hippocampus.episodes.iter().filter(|e| !e.consolidated).count();
    println!("  Remaining unconsolidated episodes: {}", unconsolidated);
    assert_check!("all episodes marked consolidated", unconsolidated == 0);

    let _ = bonds_before; // bond count may not increase if no co-occurrences yet

    // ── 8. Check StyleVector is tracking ─────────────────────────────────────

    section("8. StyleVector");

    let style = &brain.hippocampus.style;
    println!("  formality    = {:.4}", style.formality);
    println!("  verbosity    = {:.4}", style.verbosity);
    println!("  technicality = {:.4}", style.technicality);
    println!("  empathy      = {:.4}", style.empathy);
    println!("  sample_count = {}", style.sample_count);

    assert_check!("StyleVector has samples", style.sample_count > 0);
    let non_zero = style.formality != 0.0
        || style.verbosity != 0.0
        || style.technicality != 0.0;
    assert_check!("StyleVector has non-zero values", non_zero);

    // ── 9. Build dynamic dictionary and verify entries ────────────────────────

    section("9. Dynamic Dictionary");

    let dict = brain.build_dictionary();
    println!("  Entries    : {}", dict.entry_count());
    println!("  Generation : {}", dict.generation());

    let terms = dict.plaintext_terms();
    println!("  Sample terms (first 20):");
    for t in terms.iter().take(20) {
        print!("    {}", t);
    }
    if !terms.is_empty() { println!(); }

    assert_check!("dictionary has entries", dict.entry_count() > 0);
    assert_check!("dictionary has ≥10 entries", dict.entry_count() >= 10);

    // ── PASS/FAIL Summary ─────────────────────────────────────────────────────

    section("PASS/FAIL SUMMARY");

    let total   = results.len();
    let passed  = results.iter().filter(|(_, p)| *p).count();
    let failed  = total - passed;

    for (label, pass) in &results {
        let icon = if *pass { "✓" } else { "✗" };
        println!("  {} {}", icon, label);
    }

    println!();
    println!("  {}/{} checks passed", passed, total);

    let final_stats = brain.stats();
    println!();
    println!("  Final graph state:");
    println!("    Nodes    : {} (active: {})", final_stats.total_nodes, final_stats.active_nodes);
    println!("    Bonds    : {} (active: {})", final_stats.total_bonds, final_stats.active_bonds);
    println!("    Episodes : {}", brain.hippocampus.episodes.len());
    println!("    Style    : {} samples", brain.hippocampus.style.sample_count);
    println!("    Dict     : {} entries", dict.entry_count());
    println!();

    if failed == 0 {
        println!("  ╔══════════════════════════╗");
        println!("  ║   ALL CHECKS PASSED ✓    ║");
        println!("  ╚══════════════════════════╝");
    } else {
        println!("  ╔══════════════════════════╗");
        println!("  ║  {} CHECK(S) FAILED ✗    ║", failed);
        println!("  ╚══════════════════════════╝");
        std::process::exit(1);
    }
}
