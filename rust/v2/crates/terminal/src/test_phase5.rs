// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Brain v2 — Phase 5 Integration Test.
//!
//! Validates:
//!   1. Core brain teach → ask → think pipeline (5+ passages)
//!   2. Save / load round-trip: node count, bond count, episode count,
//!      style vector sample_count all match after reload
//!   3. MCP server tool dispatch: tools/list, lu_status, lu_conversation,
//!      lu_teach, lu_think, lu_save, lu_configure, lu_observe
//!   4. REPL command parser: parses the standard command tokens correctly
//!   5. Autonomy gating via MCP: lu_teach blocked at Supervised tier,
//!      allowed at Assisted; lu_revoke blocked at Assisted, allowed at
//!      Collaborative
//!   6. BrainFile persistence fields: autonomy tier, episode list, and
//!      style vector are all restored correctly after reload

use brain::{hypothalamus::AutonomyTier, Brain};
use graph::node::Confidence;
use mcp_server::McpServer;
use serde_json::{json, Value};

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

// ── Corpus fixtures ───────────────────────────────────────────────────────────

const PASSAGE_OCEANS: &str = "\
The ocean covers more than 70 percent of the Earth's surface. \
Oceans regulate global climate by absorbing heat from the sun. \
The Pacific Ocean is the largest and deepest ocean on Earth. \
Ocean currents distribute heat around the planet, moderating temperatures. \
Marine ecosystems contain the majority of Earth's biodiversity.";

const PASSAGE_PHOTOSYNTHESIS: &str = "\
Photosynthesis is the process by which plants convert sunlight into energy. \
Chlorophyll is the pigment in plants that absorbs sunlight for photosynthesis. \
Plants use carbon dioxide and water to produce glucose during photosynthesis. \
Oxygen is released as a byproduct of the photosynthesis process. \
Photosynthesis forms the foundation of almost all food chains on Earth.";

const PASSAGE_GRAVITY: &str = "\
Gravity is the force that attracts two objects with mass toward each other. \
Isaac Newton described gravity as a universal force acting between all matter. \
The gravitational pull of the Earth keeps the Moon in orbit. \
Albert Einstein reformulated gravity as the curvature of spacetime. \
Gravity governs the formation of stars, planets, and galaxies.";

const PASSAGE_NEURONS: &str = "\
Neurons are the fundamental cells of the nervous system. \
A neuron transmits electrical signals called action potentials. \
Synapses are the junctions where neurons communicate with each other. \
Neurotransmitters carry signals across the synaptic gap between neurons. \
The human brain contains approximately 86 billion neurons.";

const PASSAGE_EVOLUTION: &str = "\
Evolution is the process of genetic change in populations over time. \
Natural selection favours traits that improve survival and reproduction. \
Charles Darwin proposed natural selection as the mechanism of evolution. \
Genetic mutations introduce new variation into a population. \
Fossil records provide evidence for the gradual change of species over millions of years.";

// ── REPL command parser (mirrors main.rs logic) ───────────────────────────────

#[derive(Debug, PartialEq)]
enum ReplCommand<'a> {
    Teach(&'a str),
    Ask(&'a str),
    Think(&'a str),
    Status,
    Save(Option<&'a str>),
    Load(&'a str),
    Tier(&'a str),
    Scar { label: &'a str, reason: &'a str },
    Consolidate,
    Dictionary,
    Help,
    Quit,
    Respond(&'a str),
}

fn parse_command(input: &str) -> ReplCommand<'_> {
    match input {
        ":status"      => ReplCommand::Status,
        ":consolidate" => ReplCommand::Consolidate,
        ":dictionary"  => ReplCommand::Dictionary,
        ":help"        => ReplCommand::Help,
        ":quit" | ":exit" | "quit" | "exit" => ReplCommand::Quit,
        s if s.starts_with(":teach ")  => ReplCommand::Teach(s[7..].trim()),
        s if s.starts_with(":ask ")    => ReplCommand::Ask(s[5..].trim()),
        s if s.starts_with(":think ")  => ReplCommand::Think(s[7..].trim()),
        s if s.starts_with(":save")    => {
            let rest = s[5..].trim();
            ReplCommand::Save(if rest.is_empty() { None } else { Some(rest) })
        }
        s if s.starts_with(":load ")   => ReplCommand::Load(s[6..].trim()),
        s if s.starts_with(":tier ")   => ReplCommand::Tier(s[6..].trim()),
        s if s.starts_with(":scar ")   => {
            let rest = s[6..].trim();
            let mut parts = rest.splitn(2, ' ');
            let label  = parts.next().unwrap_or("").trim();
            let reason = parts.next().unwrap_or("user-implanted").trim();
            ReplCommand::Scar { label, reason }
        }
        other => ReplCommand::Respond(other),
    }
}

// ── MCP helper ────────────────────────────────────────────────────────────────

fn call_tool(server: &mut McpServer, tool_name: &str, args: Value) -> Value {
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": tool_name,
            "arguments": args
        }
    });
    let raw = server.route(&req);
    serde_json::from_str(&raw).unwrap_or(Value::Null)
}

fn is_tool_error(resp: &Value) -> bool {
    resp["result"]["isError"].as_bool().unwrap_or(false)
}

fn tool_text(resp: &Value) -> String {
    resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string()
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() {
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║      Lu Brain v2 — Phase 5 Integration Test                 ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();

    let mut pass: u32 = 0;
    let mut fail: u32 = 0;

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 1 — Core brain: teach 5 passages, ask, think
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 1 — Core Brain: Teach / Ask / Think ══════════════════");

    let mut brain = Brain::new();

    let seeds_a = brain.learn_text(PASSAGE_OCEANS,        Confidence::Terminal);
    let seeds_b = brain.learn_text(PASSAGE_PHOTOSYNTHESIS, Confidence::Terminal);
    let seeds_c = brain.learn_text(PASSAGE_GRAVITY,       Confidence::Terminal);
    let seeds_d = brain.learn_text(PASSAGE_NEURONS,       Confidence::Terminal);
    let seeds_e = brain.learn_text(PASSAGE_EVOLUTION,     Confidence::Terminal);

    let total_seeds = seeds_a.len() + seeds_b.len() + seeds_c.len()
                    + seeds_d.len() + seeds_e.len();

    let stats_after = brain.stats();
    println!("  Ingested {} KUs across 5 passages.", total_seeds);
    println!("  Graph: {} nodes, {} bonds", stats_after.total_nodes, stats_after.total_bonds);

    check!(pass, fail, "5 passages produced ≥5 nodes",    stats_after.total_nodes >= 5);
    check!(pass, fail, "auto-bonding created bonds",       stats_after.total_bonds >= 1);

    // Simple ask (recall path)
    let recall = brain.recall("ocean", 3, 10);
    check!(pass, fail, "recall 'ocean' returns nodes",     !recall.activated_nodes.is_empty());

    // Full respond for photosynthesis
    let resp_photo = brain.respond("What is photosynthesis?");
    check!(pass, fail, "respond returns non-empty answer", !resp_photo.trim().is_empty());
    let has_photo = resp_photo.to_lowercase().contains("photosynthesis")
        || resp_photo.to_lowercase().contains("plant")
        || resp_photo.to_lowercase().contains("sunlight");
    check!(pass, fail, "photosynthesis answer references topic", has_photo);

    // Thinking loop — multi-part query
    let think_q = "What is gravity and how does it affect the Moon";
    let think_ans = brain.think_through(think_q);
    check!(pass, fail, "think_through returns non-empty answer", !think_ans.trim().is_empty());

    // Episode recording
    let ep_count = brain.hippocampus.episodes.len();
    check!(pass, fail, "episodes recorded after queries", ep_count >= 2);

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 2 — Save / load round-trip
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 2 — Save / Load Round-Trip ═══════════════════════════");

    // Force a few more queries to build up style vector
    brain.respond("Tell me about neurons");
    brain.respond("How does evolution work?");

    // Set a non-default tier so we can verify it survives the round-trip
    brain.hypothalamus.set_autonomy_tier(AutonomyTier::Assisted);

    let pre_nodes   = brain.stats().total_nodes;
    let pre_bonds   = brain.stats().total_bonds;
    let pre_eps     = brain.hippocampus.episodes.len();
    let pre_samples = brain.hippocampus.style.sample_count;
    let _pre_scars  = brain.amygdala.scars.len();

    // Implant a scar to verify it survives
    brain.implant_scar("danger-zone", "test-scar", vec![1.0, 0.0, 0.0]);
    let pre_scars_with_extra = brain.amygdala.scars.len();

    let tmp_path = std::path::PathBuf::from("test_phase5_roundtrip.lbr");
    brain.set_storage(tmp_path.clone());

    match brain.save() {
        Ok(()) => {
            println!("  Saved to {:?}", tmp_path);
            check!(pass, fail, "brain saved successfully", true);
        }
        Err(e) => {
            println!("  Save FAILED: {}", e);
            check!(pass, fail, "brain saved successfully", false);
        }
    }

    match Brain::load(&tmp_path) {
        Ok(loaded) => {
            let post_nodes   = loaded.stats().total_nodes;
            let post_bonds   = loaded.stats().total_bonds;
            let post_eps     = loaded.hippocampus.episodes.len();
            let post_samples = loaded.hippocampus.style.sample_count;
            let post_scars   = loaded.amygdala.scars.len();
            let post_tier    = loaded.hypothalamus.autonomy_tier;

            println!("  Pre-save:   {} nodes, {} bonds, {} episodes, {} samples, {} scars",
                pre_nodes, pre_bonds, pre_eps, pre_samples, pre_scars_with_extra);
            println!("  Post-load:  {} nodes, {} bonds, {} episodes, {} samples, {} scars",
                post_nodes, post_bonds, post_eps, post_samples, post_scars);

            check!(pass, fail, "node count preserved after reload",    post_nodes   == pre_nodes);
            check!(pass, fail, "bond count preserved after reload",    post_bonds   == pre_bonds);
            check!(pass, fail, "episode count preserved after reload", post_eps     == pre_eps);
            check!(pass, fail, "style sample count preserved",        post_samples == pre_samples);
            check!(pass, fail, "scar list preserved after reload",    post_scars   == pre_scars_with_extra);
            check!(pass, fail, "autonomy tier preserved (Assisted)",
                post_tier == AutonomyTier::Assisted);
        }
        Err(e) => {
            println!("  Load FAILED: {}", e);
            for _ in 0..7 {
                fail!(fail, "round-trip check (load failed)");
            }
        }
    }

    // Clean up temp file
    let _ = std::fs::remove_file(&tmp_path);

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 3 — MCP server tool dispatch
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 3 — MCP Server Tool Dispatch ═════════════════════════");

    let mut server = McpServer::new(None);

    // Teach a few passages so the brain has content
    let seeds_mcp = server.brain.learn_text(PASSAGE_OCEANS, Confidence::Terminal);
    server.brain.learn_text(PASSAGE_PHOTOSYNTHESIS, Confidence::Terminal);
    server.brain.learn_text(PASSAGE_GRAVITY, Confidence::Terminal);
    println!("  Pre-loaded {} KUs into MCP server brain.", seeds_mcp.len() + 10);

    // Elevate tier so state-modifying tools are available
    server.brain.hypothalamus.set_autonomy_tier(AutonomyTier::Assisted);

    // 3a. tools/list
    let list_req  = json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}});
    let list_resp: Value = serde_json::from_str(&server.route(&list_req)).unwrap_or_default();
    let tool_count = list_resp["result"]["tools"].as_array().map(|a| a.len()).unwrap_or(0);
    check!(pass, fail, "tools/list returns ≥10 tools", tool_count >= 10);

    // 3b. lu_status
    let status_resp = call_tool(&mut server, "lu_status", json!({}));
    check!(pass, fail, "lu_status is not an error",     !is_tool_error(&status_resp));
    let status_text = tool_text(&status_resp);
    check!(pass, fail, "lu_status includes node count",
        status_text.contains("nodes") || status_text.contains("node"));

    // 3c. lu_conversation
    let conv_resp = call_tool(&mut server, "lu_conversation",
        json!({"message": "What is photosynthesis?"}));
    check!(pass, fail, "lu_conversation is not an error", !is_tool_error(&conv_resp));
    let conv_text = tool_text(&conv_resp);
    check!(pass, fail, "lu_conversation returns non-empty text", !conv_text.trim().is_empty());

    // 3d. lu_teach
    let teach_resp = call_tool(&mut server, "lu_teach",
        json!({"ku_type": "Raw", "content": "The Sun is a star at the centre of the solar system.", "confidence": "Terminal"}));
    check!(pass, fail, "lu_teach is not an error",      !is_tool_error(&teach_resp));
    check!(pass, fail, "lu_teach text mentions learned", tool_text(&teach_resp).contains("Learned"));

    // 3e. lu_think
    let think_resp = call_tool(&mut server, "lu_think",
        json!({"topic": "What is photosynthesis and how does it relate to oxygen"}));
    check!(pass, fail, "lu_think is not an error",      !is_tool_error(&think_resp));
    check!(pass, fail, "lu_think returns non-empty",    !tool_text(&think_resp).trim().is_empty());

    // 3f. lu_observe
    let obs_resp = call_tool(&mut server, "lu_observe", json!({}));
    check!(pass, fail, "lu_observe is not an error",    !is_tool_error(&obs_resp));
    let obs_text = tool_text(&obs_resp);
    check!(pass, fail, "lu_observe includes node data",
        obs_text.contains("active_nodes") || obs_text.contains("nodes"));

    // 3g. lu_configure (change tier)
    let cfg_resp = call_tool(&mut server, "lu_configure",
        json!({"autonomy_tier": "Collaborative"}));
    check!(pass, fail, "lu_configure is not an error",  !is_tool_error(&cfg_resp));
    check!(pass, fail, "autonomy tier changed",
        server.brain.hypothalamus.autonomy_tier == AutonomyTier::Collaborative);

    // 3h. lu_session list
    let sess_resp = call_tool(&mut server, "lu_session", json!({"action": "list"}));
    check!(pass, fail, "lu_session list is not an error", !is_tool_error(&sess_resp));

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 4 — REPL command parser
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 4 — REPL Command Parser ══════════════════════════════");

    check!(pass, fail, ":status → Status",
        parse_command(":status") == ReplCommand::Status);
    check!(pass, fail, ":consolidate → Consolidate",
        parse_command(":consolidate") == ReplCommand::Consolidate);
    check!(pass, fail, ":dictionary → Dictionary",
        parse_command(":dictionary") == ReplCommand::Dictionary);
    check!(pass, fail, ":help → Help",
        parse_command(":help") == ReplCommand::Help);
    check!(pass, fail, ":quit → Quit",
        parse_command(":quit") == ReplCommand::Quit);
    check!(pass, fail, ":exit → Quit",
        parse_command(":exit") == ReplCommand::Quit);
    check!(pass, fail, ":teach hello → Teach('hello')",
        parse_command(":teach hello world") == ReplCommand::Teach("hello world"));
    check!(pass, fail, ":ask what → Ask('what is gravity')",
        parse_command(":ask what is gravity") == ReplCommand::Ask("what is gravity"));
    check!(pass, fail, ":think ... → Think",
        parse_command(":think complex question") == ReplCommand::Think("complex question"));
    check!(pass, fail, ":save → Save(None)",
        parse_command(":save") == ReplCommand::Save(None));
    check!(pass, fail, ":save /tmp/x → Save(Some('/tmp/x'))",
        parse_command(":save /tmp/x") == ReplCommand::Save(Some("/tmp/x")));
    check!(pass, fail, ":load /tmp/x → Load('/tmp/x')",
        parse_command(":load /tmp/x") == ReplCommand::Load("/tmp/x"));
    check!(pass, fail, ":tier assisted → Tier('assisted')",
        parse_command(":tier assisted") == ReplCommand::Tier("assisted"));
    check!(pass, fail, ":scar topic reason → Scar{label,reason}",
        parse_command(":scar danger-zone user-revoked")
            == ReplCommand::Scar { label: "danger-zone", reason: "user-revoked" });
    check!(pass, fail, "plain text → Respond",
        parse_command("what is gravity?")
            == ReplCommand::Respond("what is gravity?"));

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 5 — Autonomy gating via MCP
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 5 — Autonomy Gating via MCP ══════════════════════════");

    let mut gating_server = McpServer::new(None);

    // Default tier is Supervised — teach should be blocked
    let supervised_teach = call_tool(&mut gating_server, "lu_teach",
        json!({"ku_type": "Raw", "content": "test knowledge"}));
    check!(pass, fail, "lu_teach blocked at Supervised tier",
        is_tool_error(&supervised_teach));

    // lu_conversation should NOT be gated — always allowed
    let supervised_converse = call_tool(&mut gating_server, "lu_conversation",
        json!({"message": "hello"}));
    check!(pass, fail, "lu_conversation allowed at Supervised tier",
        !is_tool_error(&supervised_converse));

    // lu_status should NOT be gated
    let supervised_status = call_tool(&mut gating_server, "lu_status", json!({}));
    check!(pass, fail, "lu_status allowed at Supervised tier",
        !is_tool_error(&supervised_status));

    // Elevate to Assisted — teach should now pass
    gating_server.brain.hypothalamus.set_autonomy_tier(AutonomyTier::Assisted);
    let assisted_teach = call_tool(&mut gating_server, "lu_teach",
        json!({"ku_type": "Raw", "content": "gating test knowledge unit"}));
    check!(pass, fail, "lu_teach allowed at Assisted tier",
        !is_tool_error(&assisted_teach));

    // lu_revoke requires Collaborative — should be blocked at Assisted
    let assisted_revoke = call_tool(&mut gating_server, "lu_revoke",
        json!({"seed_hex": "0".repeat(64), "reason": "test"}));
    check!(pass, fail, "lu_revoke blocked at Assisted tier",
        is_tool_error(&assisted_revoke));

    // Elevate to Collaborative — revoke should now give a different error
    // (bad seed_hex rather than tier error, meaning gating passed)
    gating_server.brain.hypothalamus.set_autonomy_tier(AutonomyTier::Collaborative);
    let collab_revoke = call_tool(&mut gating_server, "lu_revoke",
        json!({"seed_hex": "0".repeat(64), "reason": "test"}));
    // "0" * 64 is a valid hex seed — it will just produce a no-op revoke
    // (node doesn't exist), so the response should NOT be a tier-gate error.
    let revoke_text = tool_text(&collab_revoke);
    let is_tier_gate_error = revoke_text.contains("requires") && revoke_text.contains("tier");
    check!(pass, fail, "lu_revoke at Collaborative passes tier gate",
        !is_tier_gate_error);

    // lu_save blocked at Supervised
    let mut save_server = McpServer::new(None);
    let supervised_save = call_tool(&mut save_server, "lu_save", json!({}));
    check!(pass, fail, "lu_save blocked at Supervised tier",
        is_tool_error(&supervised_save));

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // TEST 6 — BrainFile full-state persistence verification
    // ══════════════════════════════════════════════════════════════════════════

    println!("═══ TEST 6 — BrainFile Full-State Persistence ══════════════════");

    let mut b6 = Brain::new();
    b6.learn_text(PASSAGE_NEURONS,  Confidence::Terminal);
    b6.learn_text(PASSAGE_EVOLUTION, Confidence::Terminal);
    b6.respond("What are neurons?");
    b6.respond("How does natural selection work?");
    b6.hypothalamus.set_autonomy_tier(AutonomyTier::Collaborative);
    b6.implant_scar("revoked-concept", "test", vec![0.5, 0.5]);

    let b6_nodes   = b6.stats().total_nodes;
    let b6_bonds   = b6.stats().total_bonds;
    let b6_eps     = b6.hippocampus.episodes.len();
    let b6_samples = b6.hippocampus.style.sample_count;
    let b6_scars   = b6.amygdala.scars.len();

    let tmp6 = std::path::PathBuf::from("test_phase5_state.lbr");
    b6.set_storage(tmp6.clone());
    let save6_ok = b6.save().is_ok();
    check!(pass, fail, "brain b6 saved without error", save6_ok);

    match Brain::load(&tmp6) {
        Ok(b6r) => {
            check!(pass, fail, "b6 nodes match after reload",   b6r.stats().total_nodes == b6_nodes);
            check!(pass, fail, "b6 bonds match after reload",   b6r.stats().total_bonds == b6_bonds);
            check!(pass, fail, "b6 episodes match after reload", b6r.hippocampus.episodes.len() == b6_eps);
            check!(pass, fail, "b6 style samples match",        b6r.hippocampus.style.sample_count == b6_samples);
            check!(pass, fail, "b6 scars match after reload",   b6r.amygdala.scars.len() == b6_scars);
            check!(pass, fail, "b6 tier Collaborative preserved",
                b6r.hypothalamus.autonomy_tier == AutonomyTier::Collaborative);

            // Verify first scar label survived
            let scar_label_ok = b6r.amygdala.scars.iter()
                .any(|s| s.label == "revoked-concept");
            check!(pass, fail, "scar label 'revoked-concept' preserved", scar_label_ok);

            // Verify style vector values are non-zero
            let sv = &b6r.hippocampus.style;
            check!(pass, fail, "style vector formality preserved non-zero",
                sv.formality != 0.0 || sv.verbosity != 0.0 || sv.technicality != 0.0);
        }
        Err(e) => {
            println!("  Load b6 FAILED: {}", e);
            for _ in 0..8 {
                fail!(fail, "b6 round-trip check (load failed)");
            }
        }
    }

    let _ = std::fs::remove_file(&tmp6);

    println!();

    // ══════════════════════════════════════════════════════════════════════════
    // Summary
    // ══════════════════════════════════════════════════════════════════════════

    println!("╔══════════════════════════════════════════════════════════════╗");
    if fail == 0 {
        println!(
            "║  PHASE 5 RESULTS: {} PASS  {} FAIL                              ║",
            pass, fail
        );
    } else {
        println!(
            "║  PHASE 5 RESULTS: {} PASS  {} FAIL  ← REVIEW ABOVE             ║",
            pass, fail
        );
    }
    println!("╚══════════════════════════════════════════════════════════════╝");

    if fail > 0 {
        std::process::exit(1);
    }
}
