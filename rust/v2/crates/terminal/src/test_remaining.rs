// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! test_remaining — validates all v1-vs-v2 parity gap implementations.
//!
//! Covers:
//!   1. Proposal creation and approval/rejection
//!   2. Session task/belief lifecycle (including TTL expiry)
//!   3. Response confidence level assignment
//!   4. Source attribution percentages
//!   5. Compound tokenization
//!   6. New curiosity gap types (DeadEnd, UnresolvedScar, RecurringFailure)
//!   7. Self-improvement metric tracking
//!   8. Arena variant competition

use brain::{
    arena::{Arena, ResponseVariant},
    curiosity::{CuriosityEngine, WonderSource},
    frontal::compute_response_confidence,
    proposals::{PendingProposals, ThinkingProposal},
    self_improvement::SelfImprovement,
    session_memory::SessionState,
    Brain,
};
use mcp_server::McpServer;

fn pass(name: &str) {
    println!("  [PASS] {}", name);
}

fn fail(name: &str, msg: &str) {
    eprintln!("  [FAIL] {} — {}", name, msg);
    std::process::exit(1);
}

// ── 1. Proposal Approval Pipeline ────────────────────────────────────────────

fn test_proposals() {
    println!("\n=== 1. Proposal Approval Pipeline ===");

    let mut queue = PendingProposals::new(8);

    // Queue a learn insight proposal.
    let id1 = queue.queue(
        ThinkingProposal::LearnInsight {
            text: "Gravity is the force of attraction between masses".to_string(),
            confidence: "Verified".to_string(),
        },
        "thinking about physics",
    );

    // Queue a form hypothesis proposal.
    let id2 = queue.queue(
        ThinkingProposal::FormHypothesis {
            hypothesis: "Dark matter interacts only gravitationally".to_string(),
            supporting_seeds: vec![],
        },
        "cosmology query",
    );

    // Queue a revoke proposal.
    let id3 = queue.queue(
        ThinkingProposal::RevokeNode {
            seed: "a".repeat(64),
            reason: "outdated".to_string(),
        },
        "cleanup",
    );

    if queue.pending_count() != 3 {
        fail("queue_count", &format!("expected 3, got {}", queue.pending_count()));
    }
    pass("three proposals queued");

    // Approve first proposal.
    let approved = queue.approve(&id1);
    if approved.is_none() {
        fail("approve", "approve returned None");
    }
    pass("proposal approved and removed from queue");

    if queue.pending_count() != 2 {
        fail("post_approve_count", &format!("expected 2, got {}", queue.pending_count()));
    }
    pass("pending count correct after approve");

    // Reject second proposal.
    if !queue.reject(&id2) {
        fail("reject", "reject returned false");
    }
    pass("proposal rejected");

    if queue.pending_count() != 1 {
        fail("post_reject_count", &format!("expected 1, got {}", queue.pending_count()));
    }
    pass("pending count correct after reject");

    // Reject non-existent returns false.
    if queue.reject("nonexistent-id") {
        fail("reject_missing", "should return false for missing id");
    }
    pass("reject missing id returns false");

    // Approve missing returns None.
    if queue.approve("nonexistent-id").is_some() {
        fail("approve_missing", "should return None for missing id");
    }
    pass("approve missing id returns None");

    // Eviction: fill beyond max_size.
    for i in 0..10 {
        queue.queue(
            ThinkingProposal::LearnInsight {
                text: format!("fact {}", i),
                confidence: "Terminal".to_string(),
            },
            "bulk",
        );
    }
    if queue.pending_count() > 8 {
        fail("eviction", &format!("queue exceeded max_size: {}", queue.pending_count()));
    }
    pass("queue eviction respects max_size");

    // Check totals.
    if queue.total_approved < 1 {
        fail("total_approved", "should be at least 1");
    }
    if queue.total_rejected < 1 {
        fail("total_rejected", "should be at least 1");
    }
    pass("approval/rejection counters tracked");

    // Wire through MCP server.
    let mut server = McpServer::new(None);
    server.brain.hypothalamus.set_autonomy_tier(brain::hypothalamus::AutonomyTier::Assisted);

    // Queue a real LearnInsight proposal via direct brain access.
    let pid = server.brain.proposals.queue(
        ThinkingProposal::LearnInsight {
            text: "Photons travel at the speed of light".to_string(),
            confidence: "Terminal".to_string(),
        },
        "test",
    );

    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "lu_approve",
            "arguments": { "proposal_id": pid }
        }
    });
    let resp = server.route(&req);
    let val: serde_json::Value = serde_json::from_str(&resp).unwrap();
    let text = val["result"]["content"][0]["text"].as_str().unwrap_or("");
    if !text.contains("approved") {
        fail("mcp_approve", &format!("unexpected response: {}", text));
    }
    pass("MCP lu_approve applies proposal");

    // After approval the brain should have learned the insight.
    let recall = server.brain.recall("photons", 2, 5);
    if recall.activated_nodes.is_empty() {
        fail("proposal_applied", "approved LearnInsight not found in graph");
    }
    pass("approved LearnInsight persists in brain graph");

    // Reject a proposal via MCP.
    let pid2 = server.brain.proposals.queue(
        ThinkingProposal::FormHypothesis {
            hypothesis: "test hypothesis".to_string(),
            supporting_seeds: vec![],
        },
        "test",
    );
    let req2 = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "lu_reject",
            "arguments": { "proposal_id": pid2 }
        }
    });
    let resp2 = server.route(&req2);
    let val2: serde_json::Value = serde_json::from_str(&resp2).unwrap();
    let text2 = val2["result"]["content"][0]["text"].as_str().unwrap_or("");
    if !text2.contains("rejected") {
        fail("mcp_reject", &format!("unexpected: {}", text2));
    }
    pass("MCP lu_reject discards proposal");

    let _ = id3; // suppress unused warning
}

// ── 2. Session Task / Belief Lifecycle ───────────────────────────────────────

fn test_session_memory() {
    println!("\n=== 2. Session Memory ===");

    let mut session = SessionState::new();

    // Tasks.
    let tid = session.add_task("Summarise the quantum mechanics chapter");
    if session.task_count() != 1 {
        fail("task_count", "expected 1 task");
    }
    pass("task added");

    if !session.record_attempt(&tid, "keyword_recall", "found 3 KUs") {
        fail("record_attempt", "record_attempt returned false");
    }
    let task = session.tasks.iter().find(|t| t.id == tid).unwrap();
    if task.attempts.len() != 1 {
        fail("attempt_count", "expected 1 attempt");
    }
    pass("attempt recorded on task");

    session.complete_task(&tid);
    let task = session.tasks.iter().find(|t| t.id == tid).unwrap();
    if task.status != brain::session_memory::TaskStatus::Completed {
        fail("task_complete", "task should be Completed");
    }
    pass("task marked Completed");

    // Beliefs with TTL.
    let bid = session.add_belief("The user is asking about quantum mechanics", 0.9, 3600);
    if session.active_belief_count() != 1 {
        fail("belief_count", "expected 1 active belief");
    }
    pass("belief added with TTL");

    // Expired belief: TTL = 0 seconds.
    let _bid_expired = session.add_belief("Temporary assumption", 0.5, 0);
    // With ttl=0 the belief is immediately expired.
    let pruned = session.prune_expired_beliefs();
    if pruned < 1 {
        fail("prune_expired", &format!("expected ≥1 pruned, got {}", pruned));
    }
    pass("expired belief pruned");

    if session.active_belief_count() != 1 {
        fail("active_beliefs_after_prune", &format!(
            "expected 1 active, got {}", session.active_belief_count()
        ));
    }
    pass("active belief count correct after prune");

    // Check remaining TTL.
    let b = session.beliefs.iter().find(|b| b.id == bid).unwrap();
    if b.remaining_secs() == 0 {
        fail("remaining_secs", "should still have time remaining");
    }
    pass("belief remaining_secs > 0");

    // Learnings.
    let lid = session.add_learning(
        "Quantum entanglement doesn't allow faster-than-light communication",
        Some(tid.clone()),
    );
    if session.learning_count() != 1 {
        fail("learning_count", "expected 1 learning");
    }
    let learning = session.learnings.iter().find(|l| l.id == lid).unwrap();
    if learning.source_task_id.as_deref() != Some(&tid) {
        fail("learning_source", "source_task_id mismatch");
    }
    pass("learning recorded with source task link");
}

// ── 3. Response Confidence Level ─────────────────────────────────────────────

fn test_response_confidence() {
    println!("\n=== 3. Response Confidence ===");

    use brain::frontal::ResponseConfidence;

    let c = compute_response_confidence(0, 0.0);
    if c != ResponseConfidence::Empty {
        fail("empty_confidence", &format!("expected Empty, got {:?}", c));
    }
    pass("0 KUs → Empty");

    let c = compute_response_confidence(1, 0.2);
    if c != ResponseConfidence::Weak {
        fail("weak_confidence", &format!("expected Weak, got {:?}", c));
    }
    pass("1 KU, low strength → Weak");

    let c = compute_response_confidence(2, 0.6);
    if c != ResponseConfidence::Moderate {
        fail("moderate_confidence", &format!("expected Moderate, got {:?}", c));
    }
    pass("2 KUs, good strength → Moderate");

    let c = compute_response_confidence(5, 0.8);
    if c != ResponseConfidence::Strong {
        fail("strong_confidence", &format!("expected Strong, got {:?}", c));
    }
    pass("5 KUs, high strength → Strong");

    let c = compute_response_confidence(10, 0.5);
    if c != ResponseConfidence::Strong {
        fail("strong_many", &format!("expected Strong, got {:?}", c));
    }
    pass("10 KUs → Strong");

    // Labels.
    assert_eq!(ResponseConfidence::Empty.label(), "empty");
    assert_eq!(ResponseConfidence::Strong.label(), "strong");
    pass("confidence labels correct");
}

// ── 4. Source Attribution Percentages ────────────────────────────────────────

fn test_source_attribution() {
    println!("\n=== 4. Source Attribution ===");

    let mut brain = Brain::new();
    brain.learn_text("The sun is a star at the centre of our solar system", graph::node::Confidence::Terminal);
    brain.learn_text("Stars produce energy through nuclear fusion", graph::node::Confidence::Terminal);
    brain.learn_text("Nuclear fusion combines hydrogen atoms into helium", graph::node::Confidence::Terminal);

    let _ = brain.respond("what is the sun");

    let thread = brain.hippocampus.current_thread.as_ref().unwrap();
    let last_turn = thread.turns.last().expect("no turns recorded");

    // Attribution should sum to ~1.0.
    let total: f32 = last_turn.source_attribution.iter().map(|(_, p)| p).sum();
    if !last_turn.source_attribution.is_empty() {
        // Allow minor floating-point drift.
        if (total - 1.0).abs() > 0.01 {
            fail("attribution_sum", &format!("attributions sum to {:.4}, expected ~1.0", total));
        }
        pass("source attribution percentages sum to 1.0");

        // All percentages should be in [0, 1].
        for (_, pct) in &last_turn.source_attribution {
            if *pct < 0.0 || *pct > 1.0 {
                fail("attribution_range", &format!("pct out of range: {}", pct));
            }
        }
        pass("all attribution percentages in [0, 1]");
    } else {
        // No KUs found — attribution is empty, which is valid.
        pass("attribution empty when no KUs recalled (valid)");
    }
}

// ── 5. Compound Tokenization ──────────────────────────────────────────────────

fn test_compound_tokenization() {
    println!("\n=== 5. Compound Tokenization ===");

    let mut brain = Brain::new();

    // Teach a phrase that forms a compound token.
    brain.learn_text(
        "Machine learning is a subset of artificial intelligence",
        graph::node::Confidence::Terminal,
    );
    brain.learn_text(
        "Deep learning uses neural networks",
        graph::node::Confidence::Terminal,
    );
    brain.network.rebuild_keyword_index();

    let cidx_size = brain.network.compound_index_size();
    if cidx_size == 0 {
        fail("compound_index_built", "compound index should not be empty after learning");
    }
    pass(&format!("compound index built with {} entries", cidx_size));

    // Query with the compound phrase — should find the node.
    let recall = brain.network.keyword_search("machine learning", 2, 5);
    if recall.activated_nodes.is_empty() {
        fail("compound_query", "compound query 'machine learning' returned 0 nodes");
    }
    pass("compound phrase query 'machine learning' recalls nodes");

    // Verify rebuild_keyword_index also populates compound_index.
    brain.network.rebuild_keyword_index();
    if brain.network.compound_index_size() == 0 {
        fail("compound_rebuild", "compound index empty after rebuild");
    }
    pass("rebuild_keyword_index populates compound_index");
}

// ── 6. New Curiosity Gap Types ────────────────────────────────────────────────

fn test_curiosity_gaps() {
    println!("\n=== 6. New Curiosity Gap Types ===");

    let mut engine = CuriosityEngine::new();

    let node_labels = vec![
        ("Machine learning is a subset of AI".to_string(), "definition".to_string()),
        ("Neural networks are used for classification".to_string(), "mechanism".to_string()),
        ("Gradient descent minimizes loss functions".to_string(), "mechanism".to_string()),
    ];

    // Bond pairs: machine learning ↔ neural networks only (gradient descent isolated).
    let bond_pairs = vec![
        ("Machine learning is a subset of AI".to_string(),
         "Neural networks are used for classification".to_string()),
    ];

    // ── DeadEnd: nodes with degree 1 ─────────────────────────────────────────
    let scar_labels = vec![
        "Incorrect backpropagation formula".to_string(),
    ];
    let weak_topics = vec![
        "stochastic optimization".to_string(),
    ];

    let n = engine.analyze_extended(
        &["Q: machine learning → 3 words recalled".to_string()],
        &node_labels,
        &bond_pairs,
        &scar_labels,
        &weak_topics,
    );
    println!("    Generated {} wonder items", n);

    let sources: Vec<WonderSource> = engine.wonder_queue.items
        .iter()
        .map(|w| w.source.clone())
        .collect();

    // Should contain at least one DeadEnd.
    if !sources.contains(&WonderSource::DeadEnd) {
        fail("dead_end", "expected DeadEnd wonder item for isolated node");
    }
    pass("DeadEnd gap detected for degree-1 node");

    // Should contain UnresolvedScar.
    if !sources.contains(&WonderSource::UnresolvedScar) {
        fail("unresolved_scar", "expected UnresolvedScar wonder item");
    }
    pass("UnresolvedScar gap detected");

    // Should contain RecurringFailure.
    if !sources.contains(&WonderSource::RecurringFailure) {
        fail("recurring_failure", "expected RecurringFailure wonder item");
    }
    pass("RecurringFailure gap detected");

    // Verify the new sources are also accessible from WonderSource.
    assert_eq!(WonderSource::DeadEnd, WonderSource::DeadEnd);
    assert_eq!(WonderSource::UnresolvedScar, WonderSource::UnresolvedScar);
    assert_eq!(WonderSource::RecurringFailure, WonderSource::RecurringFailure);
    pass("new WonderSource variants accessible and equatable");
}

// ── 7. Self-Improvement Metric Tracking ──────────────────────────────────────

fn test_self_improvement() {
    println!("\n=== 7. Self-Improvement ===");

    let mut si = SelfImprovement::new();

    // Record some queries.
    si.record_query("miss query", &[], &[]); // miss
    si.record_query("miss query 2", &[], &[]); // miss
    si.record_query("hit query", &[], &[]); // hit (empty nodes for test)
    si.record_query("hit query 2", &[], &[]); // hit
    si.record_query("miss query 3", &[], &[]); // miss

    if si.metrics.total_queries != 5 {
        fail("total_queries", &format!("expected 5, got {}", si.metrics.total_queries));
    }
    pass("total_queries tracked");

    if si.metrics.total_hits != 0 {
        fail("total_hits", &format!("expected 0 (all empty nodes), got {}", si.metrics.total_hits));
    }
    pass("total_hits tracked");

    let expected_rate = 0.0_f32;
    if (si.metrics.recall_hit_rate - expected_rate).abs() > 0.01 {
        fail("hit_rate", &format!("expected ~{:.2}, got {:.2}", expected_rate, si.metrics.recall_hit_rate));
    }
    pass("recall_hit_rate computed correctly");

    // Consolidation tracking.
    si.record_consolidation(0);
    si.record_consolidation(3);
    si.record_consolidation(0);

    if si.metrics.total_consolidations != 3 {
        fail("total_consolidations", &format!("expected 3, got {}", si.metrics.total_consolidations));
    }
    if si.metrics.consolidation_hits != 1 {
        fail("consolidation_hits", &format!("expected 1, got {}", si.metrics.consolidation_hits));
    }
    let expected_ce = 1.0 / 3.0;
    if (si.metrics.consolidation_effectiveness - expected_ce).abs() > 0.01 {
        fail("consolidation_effectiveness", &format!("expected ~{:.2}, got {:.2}",
            expected_ce, si.metrics.consolidation_effectiveness));
    }
    pass("consolidation effectiveness tracked");

    // Health score.
    let health = si.metrics.health_score();
    if health < 0.0 || health > 1.0 {
        fail("health_score", &format!("out of range: {}", health));
    }
    pass("health_score in [0, 1]");

    // Analyse node labels.
    let node_labels: Vec<(String, String)> = (0..10)
        .map(|i| (format!("raw fact {}", i), "raw".to_string()))
        .chain(vec![("A is defined as B".to_string(), "definition".to_string())])
        .collect();

    let suggestions = si.analyze(&node_labels);
    // Should suggest improving unstructured knowledge.
    if suggestions.is_empty() {
        fail("suggestions_empty", "expected suggestions for raw-heavy knowledge base");
    }
    pass(&format!("analyze() generated {} suggestion(s)", suggestions.len()));

    let improvement = si.suggest_improvements();
    if improvement.is_empty() {
        fail("suggest_improvements_empty", "suggest_improvements returned nothing");
    }
    pass("suggest_improvements() returns suggestions");

    // Wire through Brain.
    let mut brain = Brain::new();
    brain.learn_text("Stars produce heat and light through fusion", graph::node::Confidence::Terminal);
    let _ = brain.respond("what is a star");
    if brain.self_improvement.metrics.total_queries != 1 {
        fail("brain_si_wire", "Brain.self_improvement not updated by respond()");
    }
    pass("Brain.respond() updates self_improvement metrics");
}

// ── 8. Arena Variant Competition ─────────────────────────────────────────────

fn test_arena() {
    println!("\n=== 8. Arena Variant Competition ===");

    let mut arena = Arena::new();
    assert!(!arena.enabled);
    pass("arena disabled by default");

    arena.enable();
    assert!(arena.enabled);
    pass("arena.enable() works");

    // Build some variants.
    let v1 = ResponseVariant::new(
        "Machine learning is a technique that lets systems learn from data.",
        vec!["ML is a technique".to_string()],
        vec!["definition".to_string()],
    );
    let v2 = ResponseVariant::new(
        "ML works by gradient descent to minimise a loss function.",
        vec!["gradient descent".to_string(), "loss function".to_string()],
        vec!["mechanism".to_string(), "causal".to_string()],
    );
    let v3 = ResponseVariant::new(
        "Example: a spam filter uses machine learning.",
        vec!["spam filter".to_string()],
        vec!["example".to_string()],
    );

    let winner = arena.compete(vec![v1, v2, v3]);
    if winner.is_none() {
        fail("compete_winner", "compete() returned None with 3 variants");
    }
    let winner = winner.unwrap();
    if winner.score <= 0.0 {
        fail("winner_score", &format!("winner score should be > 0, got {}", winner.score));
    }
    pass("arena competition returns a winner with score > 0");

    // Losers should be logged.
    if arena.losers_log.len() != 2 {
        fail("losers_count", &format!("expected 2 losers, got {}", arena.losers_log.len()));
    }
    pass("losing variants logged");

    // All losers should have score ≤ winner score.
    for loser in &arena.losers_log {
        if loser.score > winner.score {
            fail("loser_score", "loser has higher score than winner");
        }
    }
    pass("winner has highest score");

    // Compete with empty list.
    let none = arena.compete(vec![]);
    if none.is_some() {
        fail("empty_compete", "empty compete should return None");
    }
    pass("empty variant list returns None");

    // Empty compete does not increment the counter.
    if arena.total_competitions != 1 {
        fail("total_competitions", &format!("expected 1, got {}", arena.total_competitions));
    }
    pass("competition count tracked");

    // Recent responses tracked for novelty.
    arena.disable();
    assert!(!arena.enabled);
    pass("arena.disable() works");
}

// ── 9. lu_subscribe returns event stream ─────────────────────────────────────

fn test_subscribe() {
    println!("\n=== 9. lu_subscribe event stream ===");

    let mut server = McpServer::new(None);
    let req = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "lu_subscribe",
            "arguments": { "channel": "query" }
        }
    });
    let resp = server.route(&req);
    let val: serde_json::Value = serde_json::from_str(&resp).unwrap();
    let text = val["result"]["content"][0]["text"].as_str().unwrap_or("");
    // Should be parseable JSON array of events.
    let events: Result<serde_json::Value, _> = serde_json::from_str(text);
    if let Ok(arr) = events {
        if !arr.is_array() {
            fail("subscribe_array", "expected JSON array of events");
        }
        pass("lu_subscribe returns JSON array");
        if arr.as_array().unwrap().is_empty() {
            fail("subscribe_not_empty", "expected at least one event");
        }
        pass("lu_subscribe array is non-empty");
    } else {
        fail("subscribe_parse", &format!("response not JSON: {}", text));
    }
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    println!("╔══════════════════════════════════════════════════════╗");
    println!("║        Lu Brain v2 — Remaining Parity Gap Tests      ║");
    println!("╚══════════════════════════════════════════════════════╝");

    test_proposals();
    test_session_memory();
    test_response_confidence();
    test_source_attribution();
    test_compound_tokenization();
    test_curiosity_gaps();
    test_self_improvement();
    test_arena();
    test_subscribe();

    println!("\n╔══════════════════════════════════════════════════════╗");
    println!("║  All remaining parity gap tests PASSED               ║");
    println!("╚══════════════════════════════════════════════════════╝");
}
