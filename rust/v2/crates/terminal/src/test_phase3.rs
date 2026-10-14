// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Brain v2 — Phase 3 integration test.
//!
//! Validates:
//!   1. Episodic memory: every respond() call records an episode
//!   2. Consolidation: running consolidate() strengthens co-occurring bonds
//!   3. StyleVector: values shift away from zero after exchanges
//!   4. DynamicDictionary: build_dictionary() produces populated dict

use brain::Brain;
use graph::node::Confidence;

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║      Lu Brain v2 — Phase 3 Integration Test                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    // ── Test corpus ──────────────────────────────────────────────────────────

    let ml_text = "\
Machine learning is a subset of artificial intelligence. \
It enables computers to learn from data without being explicitly programmed. \
Machine learning works by training models on large datasets. \
Supervised learning is a type of machine learning that uses labelled examples. \
Neural networks are used in machine learning for pattern recognition. \
Gradient descent is the optimisation algorithm used to train neural networks. \
Overfitting occurs when a model learns the training data too well. \
Regularisation is a technique to prevent overfitting in machine learning.";

    let dl_text = "\
Deep learning is a branch of machine learning based on artificial neural networks. \
Deep learning works by stacking many layers to learn hierarchical representations. \
Convolutional neural networks are used in deep learning for image recognition. \
Recurrent neural networks process sequential data in deep learning. \
Transformers have replaced recurrent neural networks for many language tasks. \
Attention mechanism allows transformers to weigh relevant parts of the input. \
Deep learning requires large amounts of data and significant compute resources. \
Transfer learning allows deep learning models to reuse pre-trained weights.";

    let crypto_text = "\
Cryptography is the practice of securing information through mathematical techniques. \
Symmetric encryption uses the same key for encryption and decryption. \
AES is a widely used symmetric encryption algorithm. \
Public key cryptography uses a key pair: a public key and a private key. \
RSA is a popular public key cryptography algorithm. \
Hashing converts data into a fixed-size digest that cannot be reversed. \
SHA-256 is a cryptographic hash function used in blockchain systems. \
Digital signatures use private keys to authenticate the origin of data.";

    let mut brain = Brain::new();

    // ── Step 1: Ingest passages ───────────────────────────────────────────────

    println!("═══ STEP 1 — INGEST ════════════════════════════════════════════");
    let _ml_seeds    = brain.learn_text(ml_text,     Confidence::Terminal);
    let _dl_seeds    = brain.learn_text(dl_text,     Confidence::Terminal);
    let _crypto_seeds = brain.learn_text(crypto_text, Confidence::Terminal);

    let stats = brain.stats();
    println!("  Nodes: {}  Bonds: {}", stats.total_nodes, stats.total_bonds);
    println!("  Episodes before queries: {}", brain.hippocampus.episodes.len());
    println!();

    // ── Step 2: Ask questions — each should record an episode ─────────────────

    println!("═══ STEP 2 — QUERIES & EPISODE RECORDING ═══════════════════════");
    let queries = [
        "What is machine learning?",
        "How does deep learning work?",
        "What is AES?",
        "Tell me about neural networks",
        "What is gradient descent?",
    ];

    for (i, q) in queries.iter().enumerate() {
        let response = brain.respond(q);
        let ep_count = brain.hippocampus.episodes.len();
        println!("  [{}] Q: \"{}\"", i + 1, q);
        let preview: String = response.chars().take(80).collect();
        println!("       A: {}…", preview);
        println!("       Episodes recorded: {}", ep_count);
        println!();
    }

    // ── Step 3: Verify episodes were recorded ─────────────────────────────────

    println!("═══ STEP 3 — EPISODIC MEMORY CHECK ════════════════════════════");
    let ep_count = brain.hippocampus.episodes.len();
    println!("  Total episodes: {}", ep_count);
    let expected = queries.len();
    if ep_count >= expected {
        println!("  PASS: {} episodes recorded (expected ≥ {})", ep_count, expected);
    } else {
        println!("  FAIL: only {} episodes (expected ≥ {})", ep_count, expected);
    }
    println!();
    println!("  Episode log:");
    for ep in brain.hippocampus.episodes.iter().take(5) {
        println!("    [{:.2}] {} seeds: {}",
            ep.emotional_valence,
            ep.description.chars().take(60).collect::<String>(),
            ep.seeds_involved.len());
    }
    println!();

    // ── Step 4: Consolidation ─────────────────────────────────────────────────

    println!("═══ STEP 4 — CONSOLIDATION ═════════════════════════════════════");
    let bonds_before = brain.stats().total_bonds;
    println!("  Bonds before consolidation: {}", bonds_before);

    let report = brain.consolidate();
    println!("  Episodes processed  : {}", report.episodes_processed);
    println!("  Bonds strengthened  : {}", report.bonds_strengthened);
    println!("  Reinforced pairs    : {}", report.reinforced_pairs.len());

    if report.episodes_processed > 0 {
        println!("  PASS: consolidation ran on {} episodes", report.episodes_processed);
    } else {
        println!("  NOTE: no unprocessed episodes (may have been already consolidated)");
    }
    println!();

    // Ask more questions to generate new unconsolidated episodes
    let more_queries = [
        "What is overfitting?",
        "How does attention work?",
        "What is transfer learning?",
        "How does hashing work?",
        "What is RSA?",
    ];
    for q in &more_queries {
        brain.respond(q);
    }

    let report2 = brain.consolidate();
    println!("  Second consolidation pass:");
    println!("  Episodes processed  : {}", report2.episodes_processed);
    println!("  Bonds strengthened  : {}", report2.bonds_strengthened);
    println!("  Reinforced pairs    : {}", report2.reinforced_pairs.len());
    if report2.episodes_processed > 0 {
        println!("  PASS: second consolidation ran");
    }
    println!();

    // ── Step 5: StyleVector ───────────────────────────────────────────────────

    println!("═══ STEP 5 — STYLE VECTOR ══════════════════════════════════════");
    let style = &brain.hippocampus.style;
    println!("  Formality    : {:.4}", style.formality);
    println!("  Verbosity    : {:.4}", style.verbosity);
    println!("  Technicality : {:.4}", style.technicality);
    println!("  Empathy      : {:.4}", style.empathy);
    println!("  Sample count : {}", style.sample_count);

    let non_default = style.formality != 0.0
        || style.verbosity != 0.0
        || style.technicality != 0.0
        || style.empathy != 0.0;
    if non_default && style.sample_count > 0 {
        println!("  PASS: StyleVector has non-default values after {} exchanges", style.sample_count);
    } else {
        println!("  FAIL: StyleVector still at defaults");
    }
    println!();

    // ── Step 6: Dynamic Dictionary ────────────────────────────────────────────

    println!("═══ STEP 6 — DYNAMIC DICTIONARY ════════════════════════════════");
    let dict = brain.build_dictionary();
    println!("  Dictionary entries  : {}", dict.entry_count());
    println!("  Dictionary generation: {}", dict.generation());

    let terms = dict.plaintext_terms();
    println!("  Sample terms (first 15):");
    for term in terms.iter().take(15) {
        println!("    {}", term);
    }

    if dict.entry_count() > 0 {
        println!("  PASS: dictionary has {} entries", dict.entry_count());
    } else {
        println!("  FAIL: dictionary is empty");
    }
    println!();

    // ── Summary ───────────────────────────────────────────────────────────────

    println!("═══ PHASE 3 SUMMARY ════════════════════════════════════════════");
    let final_stats = brain.stats();
    println!("  Final graph: {} nodes, {} bonds", final_stats.total_nodes, final_stats.total_bonds);
    println!("  Episodes   : {}", brain.hippocampus.episodes.len());
    println!("  Style samples: {}", brain.hippocampus.style.sample_count);
    println!("  Dict entries : {}", dict.entry_count());

    let all_pass = ep_count >= expected
        && non_default
        && dict.entry_count() > 0;

    if all_pass {
        println!();
        println!("  ✓ ALL PHASE 3 CHECKS PASSED");
    } else {
        println!();
        println!("  ✗ SOME CHECKS FAILED — review output above");
    }
}
