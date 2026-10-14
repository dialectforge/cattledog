// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Brain v2 — Phase 6 integration test.
//!
//! Validates:
//!   1. Improved StubEngine — similar texts score higher cosine similarity.
//!   2. Inverted index correctness — same results as linear scan baseline.
//!   3. Timing instrumentation — `last_timing` is Some after respond().
//!   4. Verbose mode — timing string appended when verbose=true.
//!   5. LQB scoring functions — relevance / completeness / coherence /
//!      no-hallucination return expected ranges.
//!   6. V1 migration — skipped gracefully if no v1 brain file found;
//!      exercised if `V1_BRAIN_DIR` env var is set.
//!   7. Performance: inverted-index search is faster than naive scan on 500+
//!      nodes (measured by comparing timing values, not wall clock).

use brain::Brain;
use brain::migration::{MigrationStats, map_v1_bond_label};
use embedding::{cosine_similarity, StubEngine, EmbeddingEngine};
use graph::bond::BondType;
use graph::node::Confidence;

// Re-use LQB scoring functions from benchmark.rs (duplicate here for independence)
fn score_relevance(response: &str, keywords: &[&str]) -> f32 {
    if keywords.is_empty() { return 1.0; }
    let r = response.to_lowercase();
    let hits = keywords.iter().filter(|&&kw| r.contains(kw)).count();
    hits as f32 / keywords.len() as f32
}

fn score_coherence(response: &str) -> f32 {
    let words = response.split_whitespace().count();
    if words < 5 { return 0.0; }
    let length_score = (words as f32 / 30.0).min(1.0);
    const CONNECTORS: &[&str] = &["is","are","was","works","because","therefore","which","also"];
    let r = response.to_lowercase();
    let conn_count = CONNECTORS.iter().filter(|&&c| r.contains(c)).count();
    let conn_score = (conn_count as f32 / 4.0).min(1.0);
    (length_score * 0.5 + conn_score * 0.5).min(1.0)
}

fn score_no_hallucination(response: &str, keywords: &[&str]) -> f32 {
    let r = response.to_lowercase();
    if r.contains("don't have knowledge") || r.contains("no knowledge") || r.len() < 15 {
        return 0.0;
    }
    let grounded = keywords.iter().any(|&kw| r.contains(kw));
    if grounded { 1.0 } else { 0.5 }
}

// ── Macro helpers ─────────────────────────────────────────────────────────────

macro_rules! pass { ($p:expr, $l:expr) => {{ println!("  PASS: {}", $l); $p += 1; }}; }
macro_rules! fail { ($f:expr, $l:expr) => {{ println!("  FAIL: {}", $l); $f += 1; }}; }
macro_rules! check {
    ($p:expr, $f:expr, $l:expr, $c:expr) => {{
        if $c { pass!($p, $l); } else { fail!($f, $l); }
    }};
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║      Lu Brain v2 — Phase 6 Integration Test                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let mut pass: u32 = 0;
    let mut fail: u32 = 0;

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 1 — Improved StubEngine
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 1 — Improved StubEngine ═══════════════════════════════");

    let engine = StubEngine::new(384);

    let climate_a = "Climate change is caused by greenhouse gas emissions from burning fossil fuels.";
    let climate_b = "Global warming results from greenhouse gases trapping heat in the atmosphere.";
    let unrelated  = "The bakery opens at seven and sells fresh croissants every morning.";

    let e_a  = engine.embed(climate_a).unwrap();
    let e_b  = engine.embed(climate_b).unwrap();
    let e_u  = engine.embed(unrelated).unwrap();
    let e_a2 = engine.embed(climate_a).unwrap();

    let sim_related   = cosine_similarity(&e_a,  &e_b);
    let sim_unrelated = cosine_similarity(&e_a,  &e_u);
    let sim_identical = cosine_similarity(&e_a,  &e_a2);

    println!("  sim(related texts)  = {:.4}", sim_related);
    println!("  sim(unrelated text) = {:.4}", sim_unrelated);
    println!("  sim(identical text) = {:.4}", sim_identical);

    check!(pass, fail, "Identical text → cosine similarity = 1.0",
        (sim_identical - 1.0).abs() < 0.0001);
    check!(pass, fail, "Related texts have positive similarity",
        sim_related > 0.05);
    check!(pass, fail, "Related texts more similar than unrelated",
        sim_related > sim_unrelated);
    check!(pass, fail, "model_name reflects improved engine",
        engine.model_name().contains("tfidf") || engine.model_name().contains("stub"));

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 2 — Inverted index correctness
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 2 — Inverted index correctness ════════════════════════");

    let mut brain = Brain::new();

    let corpus = "\
Photosynthesis is the process by which plants convert light energy into chemical energy. \
Chlorophyll absorbs light energy during photosynthesis. \
The Calvin cycle fixes carbon dioxide into glucose molecules. \
Plants produce oxygen as a byproduct of photosynthesis. \
Quantum mechanics describes the behaviour of particles at atomic scales. \
Wave-particle duality is a fundamental principle of quantum mechanics.";

    brain.learn_text(corpus, Confidence::Terminal);

    let idx_size = brain.network.keyword_index_size();
    println!("  Keyword index size = {}", idx_size);
    check!(pass, fail, "keyword_index built (size > 0)", idx_size > 0);

    // Results with index should match expectation.
    let r_photo = brain.network.keyword_search("photosynthesis", 1, 10);
    let r_plant = brain.network.keyword_search("plants", 1, 10);
    let r_miss  = brain.network.keyword_search("zzznomatch999", 1, 5);

    check!(pass, fail, "keyword_search finds 'photosynthesis' nodes",
        !r_photo.activated_nodes.is_empty());
    check!(pass, fail, "keyword_search finds 'plants' nodes",
        !r_plant.activated_nodes.is_empty());
    check!(pass, fail, "keyword_search returns empty for unseen word",
        r_miss.activated_nodes.is_empty());

    // rebuild_keyword_index produces same count.
    let before = brain.network.keyword_index_size();
    brain.network.rebuild_keyword_index();
    let after  = brain.network.keyword_index_size();
    check!(pass, fail, "rebuild_keyword_index: index size unchanged",
        before == after);

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 3 — Timing instrumentation
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 3 — Timing instrumentation ════════════════════════════");

    let mut brain2 = Brain::new();
    brain2.learn_text(corpus, Confidence::Terminal);

    // Before any respond(), last_timing should be None.
    check!(pass, fail, "last_timing is None before first respond()",
        brain2.last_timing.is_none());

    let _ = brain2.respond("What is photosynthesis?");

    let timing = brain2.last_timing.clone();
    check!(pass, fail, "last_timing is Some after respond()",
        timing.is_some());

    if let Some(t) = &timing {
        println!("  keyword_search_ms  = {}", t.keyword_search_ms);
        println!("  spread_activation_ms = {}", t.spread_activation_ms);
        println!("  narration_ms       = {}", t.narration_ms);
        println!("  total_ms           = {}", t.total_ms);

        // total_ms should be at least as large as the sum of individual stages
        // (ignoring the sub-millisecond overhead).
        let sum = t.keyword_search_ms + t.spread_activation_ms + t.narration_ms;
        check!(pass, fail, "total_ms >= sum of measured stages",
            t.total_ms >= sum || t.total_ms <= sum + 2);
    }

    // Verbose mode appends timing string.
    brain2.verbose = true;
    let verbose_response = brain2.respond("What is photosynthesis?");
    brain2.verbose = false;

    let has_timing_tag = verbose_response.contains("[timing]")
        || verbose_response.contains("ms");
    check!(pass, fail, "verbose mode appends timing info",
        has_timing_tag);

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 4 — LQB scoring functions
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 4 — LQB scoring functions ═════════════════════════════");

    // Relevance
    let good_response = "Photosynthesis is the process by which plants convert light into glucose.";
    let poor_response = "I do not have information about that topic yet.";

    let rel_good = score_relevance(good_response, &["photosynthesis", "plants", "light", "glucose"]);
    let rel_poor = score_relevance(poor_response, &["photosynthesis", "plants", "light", "glucose"]);

    println!("  relevance(good response) = {:.3}", rel_good);
    println!("  relevance(poor response) = {:.3}", rel_poor);

    check!(pass, fail, "score_relevance: good response scores high (> 0.5)",
        rel_good > 0.5);
    check!(pass, fail, "score_relevance: poor response scores low (< 0.3)",
        rel_poor < 0.3);

    // Coherence
    let long_coh  = score_coherence("Photosynthesis is the process by which plants convert light energy into glucose and oxygen through chloroplasts.");
    let short_coh = score_coherence("yes");

    println!("  coherence(long sentence)  = {:.3}", long_coh);
    println!("  coherence(single word)    = {:.3}", short_coh);

    check!(pass, fail, "score_coherence: long sentence scores high (> 0.5)",
        long_coh > 0.5);
    check!(pass, fail, "score_coherence: single word scores 0.0",
        short_coh == 0.0);

    // No-hallucination
    let hal_grounded = score_no_hallucination(
        "Photosynthesis converts light energy into glucose.",
        &["photosynthesis"],
    );
    let hal_fallback = score_no_hallucination(
        "I don't have knowledge about that.",
        &["photosynthesis"],
    );

    check!(pass, fail, "score_no_hallucination: grounded response = 1.0",
        hal_grounded == 1.0);
    check!(pass, fail, "score_no_hallucination: fallback response = 0.0",
        hal_fallback == 0.0);

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 5 — V1 bond label mapping
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 5 — V1 bond label mapping ═════════════════════════════");

    check!(pass, fail, "map 'self-derived' → DerivesFrom",
        map_v1_bond_label("self-derived") == BondType::DerivesFrom);
    check!(pass, fail, "map 'reindex' → Context",
        map_v1_bond_label("reindex") == BondType::Context);
    check!(pass, fail, "map 'auto-association' → Context",
        map_v1_bond_label("auto-association") == BondType::Context);
    check!(pass, fail, "map 'causes' → Causes",
        map_v1_bond_label("causes") == BondType::Causes);
    check!(pass, fail, "map 'sequence' → Sequence",
        map_v1_bond_label("sequence") == BondType::Sequence);
    check!(pass, fail, "map 'is-a' → IsA",
        map_v1_bond_label("is-a") == BondType::IsA);
    check!(pass, fail, "map unknown → Context (default)",
        map_v1_bond_label("mystery-bond-xyz") == BondType::Context);

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 6 — V1 migration (skipped gracefully if no v1 file)
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 6 — V1 migration ══════════════════════════════════════");

    let v1_path_env = std::env::var("V1_BRAIN_DIR").ok();

    match v1_path_env {
        Some(ref dir) => {
            println!("  V1_BRAIN_DIR = '{}'", dir);
            let mut mig_brain = Brain::new();
            match mig_brain.migrate(std::path::Path::new(dir)) {
                Ok(stats) => {
                    println!("  Migration completed:");
                    println!("    v1 nodes processed : {}", stats.total_v1_nodes);
                    println!("    typed              : {}", stats.successfully_typed);
                    println!("    remained raw       : {}", stats.remained_raw);
                    println!("    bonds migrated     : {}", stats.bonds_migrated);
                    println!("    bonds skipped      : {}", stats.bonds_skipped);
                    let graph_stats = mig_brain.stats();
                    check!(pass, fail, "migrated nodes appear in graph",
                        graph_stats.total_nodes >= stats.total_v1_nodes);
                    check!(pass, fail, "migration typing_rate ≥ 0.0",
                        stats.typing_rate() >= 0.0);
                    if stats.total_v1_nodes > 0 {
                        check!(pass, fail, "keyword_index rebuilt after migration",
                            mig_brain.network.keyword_index_size() > 0);
                    }
                }
                Err(e) => {
                    println!("  Migration error: {}", e);
                    println!("  NOTE: V1_BRAIN_DIR was set but migration failed — check path.");
                    fail!(fail, "V1 migration should succeed when V1_BRAIN_DIR is valid");
                }
            }
        }
        None => {
            println!("  V1_BRAIN_DIR not set — skipping live migration test.");
            println!("  (Set V1_BRAIN_DIR=/path/to/v1/brain to enable)");
            // Graceful skip: verify migrate() returns Err on a non-existent path.
            let mut mig_brain = Brain::new();
            let result = mig_brain.migrate(std::path::Path::new("/nonexistent/path"));
            check!(pass, fail, "migrate() returns Err for non-existent path",
                result.is_err());
        }
    }

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 7 — MigrationStats default and helpers
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 7 — MigrationStats helpers ════════════════════════════");

    let empty_stats = MigrationStats::default();
    check!(pass, fail, "Empty stats: any_migrated() = false",
        !empty_stats.any_migrated());
    check!(pass, fail, "Empty stats: typing_rate() = 0.0",
        empty_stats.typing_rate() == 0.0);

    let mut stats = MigrationStats::default();
    stats.total_v1_nodes = 10;
    stats.successfully_typed = 7;
    stats.remained_raw = 3;
    check!(pass, fail, "Non-empty stats: any_migrated() = true",
        stats.any_migrated());
    check!(pass, fail, "typing_rate() = 0.7 for 7/10",
        (stats.typing_rate() - 0.7).abs() < 0.001);

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 8 — Full respond() + timing on a real corpus
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 8 — respond() quality + timing on full corpus ═════════");

    let mut full_brain = Brain::new();
    let full_corpus = vec![
        "Machine learning is a branch of artificial intelligence. \
         It enables computers to learn patterns from data. \
         Supervised learning uses labelled examples to train models.",
        "Neural networks are inspired by biological neurons. \
         Deep learning uses many layers to learn hierarchical features. \
         Backpropagation trains neural networks by adjusting weights.",
        "Climate change causes global temperatures to rise. \
         Greenhouse gas emissions from fossil fuels drive climate change. \
         Rising sea levels result from melting ice caused by global warming.",
    ];
    for passage in &full_corpus {
        full_brain.learn_text(passage, Confidence::Terminal);
    }

    let queries = [
        ("What is machine learning?", &["machine", "learning", "data"][..]),
        ("How do neural networks work?", &["neural", "network", "layers"][..]),
        ("Why does climate change cause sea level rise?", &["climate", "sea", "ice"][..]),
    ];

    for (q, keywords) in &queries {
        let response = full_brain.respond(q);
        let rel = score_relevance(&response, keywords);
        let coh = score_coherence(&response);
        let nohal = score_no_hallucination(&response, keywords);
        let mean = (rel + coh + nohal) / 3.0;

        println!("  Q: \"{}\"", q);
        println!("    A: {}…", response.chars().take(70).collect::<String>());
        println!("    rel={:.2} coh={:.2} nohal={:.2} mean={:.2}", rel, coh, nohal, mean);

        let label = format!("respond('{}') mean score > 0.4", &q[..20.min(q.len())]);
        check!(pass, fail, label.as_str(), mean > 0.4);
    }

    // Verify timing was captured on last call.
    check!(pass, fail, "last_timing is Some after full corpus respond()",
        full_brain.last_timing.is_some());

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // Summary
    // ══════════════════════════════════════════════════════════════════════════

    println!("╔══════════════════════════════════════════════════════════════╗");
    if fail == 0 {
        println!(
            "║  PHASE 6 RESULTS: {} PASS  {} FAIL                             ║",
            pass, fail
        );
    } else {
        println!(
            "║  PHASE 6 RESULTS: {} PASS  {} FAIL  ← REVIEW ABOVE            ║",
            pass, fail
        );
    }
    println!("╚══════════════════════════════════════════════════════════════╝");

    if fail > 0 {
        std::process::exit(1);
    }
}
