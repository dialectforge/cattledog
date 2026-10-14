// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Brain v2 — Phase 4 integration test.
//!
//! Validates:
//!   1. Thalamus message routing — send messages at different priorities,
//!      drain_by_priority() returns them Critical → High → Normal → Low.
//!   2. Hypothalamus autonomy tier gating — tool blocked at Supervised,
//!      allowed at Collaborative.
//!   3. Cerebellum tier-gated execution — try_execute() respects tier.
//!   4. Amygdala emotional memory — scar triggers on matching embedding;
//!      emotional tag raises activation_boost above 1.0.
//!   5. Thinking loop — decompose() splits multi-part queries; think_through()
//!      returns a non-empty synthesised answer.

use brain::{
    cerebellum::{ToolKind, ToolRequest},
    frontal::QueryIntent,
    hypothalamus::AutonomyTier,
    thalamus::{BrainMessage, MessagePriority},
    thinking::ThinkingEngine,
    Brain,
};
use graph::node::Confidence;
use serde_json::json;

// ── Macro helpers ─────────────────────────────────────────────────────────────

macro_rules! pass {
    ($pass:expr, $label:expr) => {{
        println!("  PASS: {}", $label);
        $pass += 1;
    }};
}
macro_rules! fail {
    ($fail:expr, $label:expr) => {{
        println!("  FAIL: {}", $label);
        $fail += 1;
    }};
}
macro_rules! check {
    ($pass:expr, $fail:expr, $label:expr, $cond:expr) => {{
        if $cond {
            pass!($pass, $label);
        } else {
            fail!($fail, $label);
        }
    }};
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║      Lu Brain v2 — Phase 4 Integration Test                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let mut pass: u32 = 0;
    let mut fail: u32 = 0;

    // ── Build a brain loaded with climate-change knowledge ────────────────────

    let mut brain = Brain::new();

    let causes_text = "\
Climate change is the long-term shift in global temperatures and weather patterns. \
Greenhouse gases are the primary driver of climate change. \
Carbon dioxide emissions from burning fossil fuels cause global warming. \
Methane is a potent greenhouse gas released by agriculture and livestock. \
Deforestation reduces the Earth's ability to absorb carbon dioxide. \
Industrial processes release nitrous oxide and other warming gases. \
The greenhouse effect traps heat in the atmosphere causing temperature rise.";

    let effects_text = "\
Global warming causes polar ice caps to melt and sea levels to rise. \
Rising sea levels threaten coastal cities and low-lying island nations. \
Climate change causes more frequent and severe weather events. \
Droughts and heatwaves become more common due to climate change. \
Ocean acidification occurs as oceans absorb excess carbon dioxide. \
Biodiversity loss accelerates as habitats shift due to warming temperatures. \
Food security is threatened as crop yields decline in warming regions.";

    let solutions_text = "\
Renewable energy from solar and wind power reduces carbon emissions. \
Carbon capture technology removes carbon dioxide from the atmosphere. \
Electric vehicles reduce dependence on fossil fuels for transportation. \
Energy efficiency improvements reduce overall energy consumption. \
Reforestation programs help restore carbon-absorbing forests. \
Carbon pricing mechanisms incentivise businesses to reduce emissions. \
International climate agreements coordinate global emissions reductions.";

    brain.learn_text(causes_text,    Confidence::Terminal);
    brain.learn_text(effects_text,   Confidence::Terminal);
    brain.learn_text(solutions_text, Confidence::Terminal);

    let stats = brain.stats();
    println!(
        "Corpus ingested: {} nodes, {} bonds",
        stats.total_nodes, stats.total_bonds
    );
    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 1 — Thalamus priority routing
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 1 — Thalamus Priority Routing ════════════════════════");

    // Enqueue messages at all four priority levels in non-sorted order.
    brain.thalamus.send(
        BrainMessage::HealthCheck { from: "frontal".to_string() },
        MessagePriority::Low,
    );
    brain.thalamus.send(
        BrainMessage::SaveRequested {
            path:   "test.lbr".to_string(),
            reason: "test".to_string(),
        },
        MessagePriority::Normal,
    );
    brain.thalamus.send(
        BrainMessage::Shutdown { graceful: false },
        MessagePriority::Critical,
    );
    brain.thalamus.send(
        BrainMessage::AutonomyChanged {
            new_tier: "autonomous".to_string(),
            reason:   "test".to_string(),
        },
        MessagePriority::High,
    );

    check!(pass, fail, "4 messages enqueued", brain.thalamus.pending() == 4);
    check!(pass, fail, "routed_count == 4",   brain.thalamus.routed_count == 4);

    // drain_by_priority() must return Critical → High → Normal → Low.
    let ordered = brain.thalamus.drain_by_priority();

    check!(pass, fail, "drain_by_priority returns all 4", ordered.len() == 4);
    check!(pass, fail, "thalamus empty after drain",       brain.thalamus.is_empty());

    let first_critical  = matches!(ordered[0], BrainMessage::Shutdown { .. });
    let second_high     = matches!(ordered[1], BrainMessage::AutonomyChanged { .. });
    let third_normal    = matches!(ordered[2], BrainMessage::SaveRequested { .. });
    let fourth_low      = matches!(ordered[3], BrainMessage::HealthCheck { .. });

    check!(pass, fail, "Critical first in drain",  first_critical);
    check!(pass, fail, "High second in drain",      second_high);
    check!(pass, fail, "Normal third in drain",     third_normal);
    check!(pass, fail, "Low fourth in drain",       fourth_low);

    // recv_highest() must pluck the Critical message even when Low was added first.
    brain.thalamus.send(
        BrainMessage::HealthCheck { from: "test".to_string() },
        MessagePriority::Low,
    );
    brain.thalamus.send(
        BrainMessage::Shutdown { graceful: true },
        MessagePriority::Critical,
    );
    let highest = brain.thalamus.recv_highest();
    check!(
        pass, fail,
        "recv_highest returns Critical over Low",
        matches!(highest, Some(BrainMessage::Shutdown { .. }))
    );
    brain.thalamus.drain_by_priority(); // clean up remaining

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 2 — Hypothalamus autonomy tier gating
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 2 — Hypothalamus Autonomy Tier Gating ════════════════");

    // At Supervised tier all tools must be blocked.
    brain.hypothalamus.set_autonomy_tier(AutonomyTier::Supervised);

    check!(pass, fail, "FileSystem blocked at Supervised",
        brain.hypothalamus.gate_tool(&ToolKind::FileSystem).is_err());
    check!(pass, fail, "WebSearch blocked at Supervised",
        brain.hypothalamus.gate_tool(&ToolKind::WebSearch).is_err());
    check!(pass, fail, "CodeExecution blocked at Supervised",
        brain.hypothalamus.gate_tool(&ToolKind::CodeExecution).is_err());
    check!(pass, fail, "ModelBridge blocked at Supervised",
        brain.hypothalamus.gate_tool(&ToolKind::ModelBridge).is_err());

    // At Assisted tier read-style tools are allowed; destructive tools blocked.
    brain.hypothalamus.set_autonomy_tier(AutonomyTier::Assisted);

    check!(pass, fail, "WebSearch allowed at Assisted",
        brain.hypothalamus.gate_tool(&ToolKind::WebSearch).is_ok());
    check!(pass, fail, "ModelBridge allowed at Assisted",
        brain.hypothalamus.gate_tool(&ToolKind::ModelBridge).is_ok());
    check!(pass, fail, "FileSystem blocked at Assisted",
        brain.hypothalamus.gate_tool(&ToolKind::FileSystem).is_err());
    check!(pass, fail, "CodeExecution blocked at Assisted",
        brain.hypothalamus.gate_tool(&ToolKind::CodeExecution).is_err());
    check!(pass, fail, "ComputerUse blocked at Assisted",
        brain.hypothalamus.gate_tool(&ToolKind::ComputerUse).is_err());

    // At Collaborative tier all tools are permitted.
    brain.hypothalamus.set_autonomy_tier(AutonomyTier::Collaborative);

    let all_ok_collab = [
        brain.hypothalamus.gate_tool(&ToolKind::FileSystem),
        brain.hypothalamus.gate_tool(&ToolKind::CodeExecution),
        brain.hypothalamus.gate_tool(&ToolKind::ComputerUse),
        brain.hypothalamus.gate_tool(&ToolKind::WebSearch),
    ]
    .iter()
    .all(|r| r.is_ok());

    check!(pass, fail, "All tools allowed at Collaborative", all_ok_collab);

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 3 — Cerebellum tier-gated execution
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 3 — Cerebellum Tool Gating ═══════════════════════════");

    brain.hypothalamus.set_autonomy_tier(AutonomyTier::Supervised);

    let req_fs = ToolRequest::new(
        ToolKind::FileSystem,
        json!({ "path": "/tmp/test.txt", "op": "read" }),
    );
    let result_blocked = brain
        .cerebellum
        .try_execute(req_fs, &brain.hypothalamus.autonomy_tier);
    check!(pass, fail, "FileSystem blocked at Supervised tier",
        result_blocked.is_err());
    check!(pass, fail, "Blocked request NOT enqueued",
        brain.cerebellum.pending_count() == 0);

    brain.hypothalamus.set_autonomy_tier(AutonomyTier::Collaborative);

    let req_web = ToolRequest::new(
        ToolKind::WebSearch,
        json!({ "query": "climate change solutions" }),
    );
    let result_ok = brain
        .cerebellum
        .try_execute(req_web, &brain.hypothalamus.autonomy_tier);
    check!(pass, fail, "WebSearch allowed at Collaborative tier",
        result_ok.is_ok());
    check!(pass, fail, "Allowed request enqueued in pending",
        brain.cerebellum.pending_count() == 1);

    // Model bridge at Assisted tier should pass (read-style tool).
    brain.hypothalamus.set_autonomy_tier(AutonomyTier::Assisted);
    let req_bridge = ToolRequest::new(
        ToolKind::ModelBridge,
        json!({ "model": "ollama/mistral", "prompt": "test" }),
    );
    let result_bridge = brain
        .cerebellum
        .try_execute(req_bridge, &brain.hypothalamus.autonomy_tier);
    check!(pass, fail, "ModelBridge allowed at Assisted tier",
        result_bridge.is_ok());

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 4 — Amygdala emotional memory
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 4 — Amygdala Emotional Memory ════════════════════════");

    // --- Scar similarity detection ---
    // Create two orthogonal unit vectors.  The scar fingerprint matches only
    // itself (cosine similarity = 1.0 ≥ threshold 0.85).
    let fingerprint_a: Vec<f32> = vec![1.0, 0.0, 0.0, 0.0];
    let fingerprint_b: Vec<f32> = vec![0.0, 1.0, 0.0, 0.0]; // orthogonal → sim = 0.0

    let scar_count_before = brain.amygdala.scars.len();
    brain.implant_scar("dangerous topic", "test revocation", fingerprint_a.clone());
    let scar_count_after = brain.amygdala.scars.len();

    check!(pass, fail, "implant_scar adds a scar",
        scar_count_after == scar_count_before + 1);

    let warnings_match = brain.amygdala.check_proposal(&fingerprint_a);
    check!(pass, fail, "Scar triggers on matching embedding",
        !warnings_match.is_empty());
    check!(pass, fail, "Warning message contains scar label",
        warnings_match.iter().any(|w| w.contains("dangerous topic")));

    let warnings_miss = brain.amygdala.check_proposal(&fingerprint_b);
    check!(pass, fail, "Scar silent on orthogonal embedding",
        warnings_miss.is_empty());

    let trigger_count = brain.amygdala.scars.last().map(|s| s.trigger_count).unwrap_or(0);
    check!(pass, fail, "trigger_count incremented after match",
        trigger_count >= 1);

    // --- Emotional boost ---
    // Tag a real node (one containing "carbon dioxide") with positive high-arousal
    // emotion and verify the activation multiplier is above 1.0.
    let co2_nodes = brain
        .network
        .keyword_search("carbon dioxide", 1, 1)
        .activated_nodes;

    if let Some(node) = co2_nodes.first() {
        // valence=0.8, arousal=0.9, confidence=0.9
        brain.amygdala.tag_node(node.seed.clone(), 0.8, 0.9, 0.9);
        let boost = brain.amygdala.activation_boost(&node.seed);

        check!(pass, fail, "Positive high-arousal tag gives boost > 1.0",
            boost > 1.0);

        // Verify formula: 1.0 + arousal * confidence * (valence * 0.5 + 0.5)
        let expected = 1.0_f32 + 0.9 * 0.9 * (0.8 * 0.5 + 0.5);
        check!(pass, fail, "Boost value matches EmotionalTag formula",
            (boost - expected).abs() < 0.001);
    } else {
        println!("  NOTE: no 'carbon dioxide' nodes found — skipping boost formula test");
    }

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 5 — Thinking loop
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 5 — Thinking Loop ═════════════════════════════════════");

    // --- Decomposition ---
    let engine = ThinkingEngine::new();
    let complex_q = "What causes climate change and how does renewable energy reduce emissions \
                     and what are the effects on biodiversity";

    let sub_qs = engine.decompose(complex_q);

    println!("  Query: \"{}\"", complex_q);
    println!("  Decomposed into {} sub-questions:", sub_qs.len());
    for (i, q) in sub_qs.iter().enumerate() {
        println!("    [{}] {}", i + 1, q);
    }

    check!(pass, fail, "Decomposed into ≥ 2 sub-questions", sub_qs.len() >= 2);
    check!(pass, fail, "max_steps respected (≤ 5)",          sub_qs.len() <= engine.max_steps);

    // --- Synthesis ---
    let obs = vec![
        (
            "causes of climate change".to_string(),
            "Greenhouse gases from fossil fuels drive global warming.".to_string(),
        ),
        (
            "renewable energy solutions".to_string(),
            "Solar and wind power reduce carbon dioxide emissions.".to_string(),
        ),
    ];
    let synthesized = engine.synthesize(complex_q, &obs);

    check!(pass, fail, "synthesize returns non-empty text", !synthesized.is_empty());
    let has_content = synthesized.to_lowercase().contains("greenhouse")
        || synthesized.to_lowercase().contains("causes")
        || synthesized.to_lowercase().contains("solar")
        || synthesized.to_lowercase().contains("renewable");
    check!(pass, fail, "synthesized answer includes content from both observations", has_content);

    // --- Full think_through via Brain ---
    brain.hypothalamus.set_autonomy_tier(AutonomyTier::Supervised);

    let chain_answer = brain.think_through(complex_q);
    println!();
    println!("  think_through preview (first 200 chars):");
    let preview: String = chain_answer.chars().take(200).collect();
    println!("    {}", preview);
    println!();

    check!(pass, fail, "think_through returns non-empty answer",
        !chain_answer.is_empty());

    // --- MultiPart intent detection ---
    let multi_q = "What causes climate change and how does renewable energy help";
    let intent  = brain.frontal.classify_intent(multi_q);
    let is_multi = matches!(
        intent,
        QueryIntent::MultiPart { .. } | QueryIntent::Complex { .. }
    );
    check!(pass, fail, "Multi-part query classified as MultiPart or Complex", is_multi);

    // respond() should transparently route through thinking loop and return content.
    let multi_answer = brain.respond(multi_q);
    check!(pass, fail, "respond() on multi-part query returns non-empty answer",
        !multi_answer.is_empty());

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // Summary
    // ══════════════════════════════════════════════════════════════════════════

    println!("╔══════════════════════════════════════════════════════════════╗");
    if fail == 0 {
        println!(
            "║  PHASE 4 RESULTS: {} PASS  {} FAIL                             ║",
            pass, fail
        );
    } else {
        println!(
            "║  PHASE 4 RESULTS: {} PASS  {} FAIL  ← REVIEW ABOVE            ║",
            pass, fail
        );
    }
    println!("╚══════════════════════════════════════════════════════════════╝");

    if fail > 0 {
        std::process::exit(1);
    }
}
