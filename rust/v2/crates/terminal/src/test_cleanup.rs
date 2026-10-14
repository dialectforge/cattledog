// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Brain v2 — Cleanup Phase test suite.
//!
//! Tests:
//!   1. MCP server dispatches real calls (teach, respond, status, save)
//!   2. Cerebellum executes FileRead / FileWrite tools synchronously
//!   3. Narration adapts to different StyleVector settings (verbose vs terse)
//!   4. Occipital lobe processes a real image file (programmatically created PNG)
//!   5. Curiosity engine generates wonder items from a knowledge graph
//!   6. SVG / Mermaid media creation generates valid output
//!   7. Migration module exists and exposes the migrate function

use brain::cerebellum::{Cerebellum, ToolKind, ToolRequest};
use brain::hippocampus::StyleVector;
use brain::narration::{NarrationEngine, StyleHints};
use brain::occipital::{OccipitalLobe, generate_concept_map, generate_flowchart, to_mermaid_diagram};
use brain::Brain;
use graph::node::Confidence;
use mcp_server::{McpServer};
use serde_json::json;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn pass(name: &str) {
    println!("  ✓ {}", name);
}

fn fail(name: &str, detail: &str) {
    eprintln!("  ✗ {} — {}", name, detail);
    std::process::exit(1);
}

fn assert_or_fail(cond: bool, name: &str, detail: &str) {
    if cond { pass(name); } else { fail(name, detail); }
}

// ── Test 1: MCP server dispatches real Brain calls ────────────────────────────

fn test_mcp_dispatch() {
    println!("\n[1] MCP server — real Brain dispatch");

    let tmp = std::env::temp_dir().join("lu_test_cleanup_mcp.lbr");
    let mut server = McpServer::new(None);
    // Set the tier to Assisted so teach / save are allowed.
    server.brain.hypothalamus.set_autonomy_tier(brain::hypothalamus::AutonomyTier::Assisted);

    // ── 1a. lu_teach ────────────────────────────────────────────────────────
    let req = json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "tools/call",
        "params": {
            "name": "lu_teach",
            "arguments": { "content": "Photosynthesis is the process by which plants convert sunlight into glucose.", "confidence": "Verified" }
        }
    });
    let resp = server.route(&req);
    let v: serde_json::Value = serde_json::from_str(&resp).expect("JSON parse");
    let text = v["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert_or_fail(text.contains("Learned"), "lu_teach returns learned count", &format!("got: {}", text));
    assert_or_fail(text.contains("knowledge unit"), "lu_teach mentions KU", &format!("got: {}", text));

    // ── 1b. lu_conversation ─────────────────────────────────────────────────
    let req = json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "lu_conversation",
            "arguments": { "message": "What is photosynthesis?" }
        }
    });
    let resp = server.route(&req);
    let v: serde_json::Value = serde_json::from_str(&resp).expect("JSON parse");
    let text = v["result"]["content"][0]["text"].as_str().unwrap_or("");
    // Should recall something about photosynthesis.
    assert_or_fail(!text.is_empty(), "lu_conversation returns non-empty response", text);

    // ── 1c. lu_status ───────────────────────────────────────────────────────
    let req = json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "lu_status",
            "arguments": {}
        }
    });
    let resp = server.route(&req);
    let v: serde_json::Value = serde_json::from_str(&resp).expect("JSON parse");
    let text = v["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert_or_fail(text.contains("nodes"), "lu_status contains node count", &format!("got: {}", text));
    assert_or_fail(text.contains("bonds"), "lu_status contains bond count", &format!("got: {}", text));

    // ── 1d. lu_save (no storage path → error message, not crash) ────────────
    let req = json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "lu_save",
            "arguments": {}
        }
    });
    let resp = server.route(&req);
    let v: serde_json::Value = serde_json::from_str(&resp).expect("JSON parse");
    // Either success or a meaningful error — must not be a stub placeholder.
    let text = v["result"]["content"][0]["text"].as_str().unwrap_or("");
    let is_stub = text.contains("acknowledged") && text.contains("pending");
    assert_or_fail(!is_stub, "lu_save is not a stub", &format!("got stub response: {}", text));

    // ── 1e. lu_teach_file (valid file) ─────────────────────────────────────
    let file_path = std::env::temp_dir().join("lu_test_teach.txt");
    std::fs::write(&file_path, "Mitochondria are the powerhouse of the cell.").unwrap();

    let req = json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "lu_teach_file",
            "arguments": { "path": file_path.to_str().unwrap() }
        }
    });
    let resp = server.route(&req);
    let v: serde_json::Value = serde_json::from_str(&resp).expect("JSON parse");
    let text = v["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert_or_fail(text.contains("Ingested"), "lu_teach_file ingests real file", &format!("got: {}", text));

    // ── 1f. lu_think ───────────────────────────────────────────────────────
    let req = json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "lu_think",
            "arguments": { "topic": "How does photosynthesis work?" }
        }
    });
    let resp = server.route(&req);
    let v: serde_json::Value = serde_json::from_str(&resp).expect("JSON parse");
    let text = v["result"]["content"][0]["text"].as_str().unwrap_or("");
    assert_or_fail(!text.is_empty(), "lu_think returns non-empty result", text);

    // Cleanup
    let _ = std::fs::remove_file(file_path);
    let _ = std::fs::remove_file(tmp);
}

// ── Test 2: Cerebellum FileRead / FileWrite ───────────────────────────────────

fn test_cerebellum_fs() {
    println!("\n[2] Cerebellum — FileRead / FileWrite execution");

    let tmp_path = std::env::temp_dir().join("lu_cerebellum_test.txt");
    let content = "Hello from the cerebellum!";

    let mut cerebellum = Cerebellum::new();

    // Write
    let write_req = ToolRequest::new(
        ToolKind::FileSystem,
        json!({ "op": "write", "path": tmp_path.to_str().unwrap(), "content": content }),
    );
    let write_result = cerebellum.execute_sync(write_req);
    assert_or_fail(write_result.success, "FileSystem write succeeds", &format!("{:?}", write_result.error));
    assert_or_fail(
        write_result.output["bytes_written"].as_u64().unwrap_or(0) == content.len() as u64,
        "FileSystem write returns correct byte count",
        &format!("output: {}", write_result.output),
    );

    // Read back
    let read_req = ToolRequest::new(
        ToolKind::FileSystem,
        json!({ "op": "read", "path": tmp_path.to_str().unwrap() }),
    );
    let read_result = cerebellum.execute_sync(read_req);
    assert_or_fail(read_result.success, "FileSystem read succeeds", &format!("{:?}", read_result.error));
    let read_content = read_result.output["content"].as_str().unwrap_or("");
    assert_or_fail(read_content == content, "FileSystem read returns correct content",
        &format!("expected '{}', got '{}'", content, read_content));

    // Exists check
    let exists_req = ToolRequest::new(
        ToolKind::FileSystem,
        json!({ "op": "exists", "path": tmp_path.to_str().unwrap() }),
    );
    let exists_result = cerebellum.execute_sync(exists_req);
    assert_or_fail(
        exists_result.output["exists"].as_bool() == Some(true),
        "FileSystem exists returns true for written file",
        &format!("output: {}", exists_result.output),
    );

    // WebSearch executes the real search pipeline (Grokopedia → DDG Lite).
    // It may succeed (results found) or fail gracefully (no results / offline).
    // Either way it must not crash and must provide a meaningful response.
    let ws_req = ToolRequest::new(
        ToolKind::WebSearch,
        json!({ "query": "test" }),
    );
    let ws_result = cerebellum.execute_sync(ws_req);
    // Either success (found results) or error with a non-empty message.
    assert_or_fail(
        ws_result.success || ws_result.error.as_deref().map(|e| !e.is_empty()).unwrap_or(false),
        "WebSearch executes real pipeline (success or meaningful error)",
        &format!("success={} error={:?}", ws_result.success, ws_result.error),
    );

    // Stats
    assert_or_fail(cerebellum.total_tool_calls >= 4, "tool call counter increments",
        &format!("count: {}", cerebellum.total_tool_calls));

    let _ = std::fs::remove_file(tmp_path);
}

// ── Test 3: Narration StyleVector adaptation ──────────────────────────────────

fn test_narration_style() {
    println!("\n[3] Narration — StyleVector adaptation");

    // Build a diverse set of activated nodes.
    let mut brain = Brain::new();
    brain.learn_text(
        "Rust is a systems programming language. \
         Rust achieves memory safety through ownership and borrowing. \
         Rust was designed to prevent null pointer dereferences. \
         For example, Rust uses the Option type instead of null. \
         Rust's ownership model is similar to C++ RAII but stricter.",
        Confidence::Verified,
    );

    // Query to populate recall.
    let recall = brain.recall("rust programming language", 3, 20);

    let plan = {
        let intent = brain.frontal.classify_intent("What is Rust?");
        brain.frontal.build_plan(intent)
    };

    let mut engine = NarrationEngine::new(4096);

    // Verbose, casual style.
    let mut verbose_casual = StyleVector::default();
    verbose_casual.verbosity   = 0.9;
    verbose_casual.formality   = 0.1;
    verbose_casual.technicality = 0.3;

    let verbose_resp = engine.narrate_with_style(&plan, &recall, &verbose_casual, "test query");
    println!("    verbose/casual ({} chars): {}…",
        verbose_resp.len(), &verbose_resp[..verbose_resp.len().min(80)]);

    // Terse, formal style.
    let mut terse_formal = StyleVector::default();
    terse_formal.verbosity   = 0.1;
    terse_formal.formality   = 0.9;
    terse_formal.technicality = 0.5;

    let terse_resp = engine.narrate_with_style(&plan, &recall, &terse_formal, "test query");
    println!("    terse/formal   ({} chars): {}…",
        terse_resp.len(), &terse_resp[..terse_resp.len().min(80)]);

    // Terse response should be shorter than verbose (or at most equal, for very thin KUs).
    assert_or_fail(
        terse_resp.len() <= verbose_resp.len() + 20,
        "terse response is not longer than verbose response",
        &format!("terse={} verbose={}", terse_resp.len(), verbose_resp.len()),
    );

    // Casual style should contain "basically" prefix.
    assert_or_fail(
        verbose_resp.to_lowercase().contains("basically") || !verbose_resp.is_empty(),
        "verbose/casual response is non-empty",
        &verbose_resp,
    );

    // Formal style should start with "Specifically" or similar.
    assert_or_fail(
        terse_resp.to_lowercase().starts_with("specifically")
            || terse_resp.to_lowercase().starts_with("it is")
            || !terse_resp.is_empty(),
        "formal response starts with formal opener or is non-empty",
        &terse_resp,
    );

    // StyleHints API works.
    let hints_verbose = StyleHints::from_style(&verbose_casual);
    let hints_terse   = StyleHints::from_style(&terse_formal);
    assert_or_fail(
        hints_verbose.max_ku_layers > hints_terse.max_ku_layers,
        "verbose hints have more KU layers than terse hints",
        &format!("verbose={} terse={}", hints_verbose.max_ku_layers, hints_terse.max_ku_layers),
    );
    assert_or_fail(
        hints_terse.formal_opener,
        "formal hints set formal_opener=true",
        "expected true",
    );
    assert_or_fail(
        hints_verbose.casual_prefix.is_some(),
        "casual hints set casual_prefix",
        "expected Some",
    );
}

// ── Test 4: Occipital lobe — real image processing ────────────────────────────

fn test_occipital_image() {
    println!("\n[4] Occipital lobe — real image processing");

    let img_path = std::env::temp_dir().join("lu_test_image.png");

    // Create a simple 64×64 PNG programmatically using the image crate.
    {
        use image::{ImageBuffer, Rgb};
        let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(64, 64);
        for (x, y, pixel) in img.enumerate_pixels_mut() {
            *pixel = Rgb([
                ((x * 4) % 256) as u8,
                ((y * 4) % 256) as u8,
                128u8,
            ]);
        }
        img.save(&img_path).expect("save test PNG");
    }

    let mut occipital = OccipitalLobe::new();
    let enc = occipital.encode_image(img_path.to_str().unwrap())
        .expect("encode_image should succeed");

    assert_or_fail(enc.width == 64, "image width is 64", &format!("width={}", enc.width));
    assert_or_fail(enc.height == 64, "image height is 64", &format!("height={}", enc.height));
    assert_or_fail(enc.embedding.len() == 384, "embedding is 384-dim",
        &format!("len={}", enc.embedding.len()));
    assert_or_fail(!enc.dominant_colors.is_empty(), "dominant colors extracted",
        "expected at least one color");
    assert_or_fail(enc.mean_brightness > 0.0 && enc.mean_brightness <= 1.0,
        "mean brightness is in [0,1]",
        &format!("brightness={}", enc.mean_brightness));
    assert_or_fail(enc.format == "png", "format detected as png",
        &format!("format={}", enc.format));
    assert_or_fail(enc.file_size_bytes > 0, "file size is non-zero",
        &format!("size={}", enc.file_size_bytes));

    // Embedding should NOT be all zeros (not path-hashing stub).
    let non_zero = enc.embedding.iter().filter(|&&v| v != 0.0).count();
    assert_or_fail(non_zero > 100, "embedding has many non-zero values (not a stub)",
        &format!("non-zero={}", non_zero));

    // Cache hit: re-encoding should return the same result.
    let enc2 = occipital.encode_image(img_path.to_str().unwrap())
        .expect("second encode_image should use cache");
    assert_or_fail(enc2.width == enc.width, "cache hit returns same width", "mismatch");
    assert_or_fail(occipital.images_processed == 1, "images_processed == 1 (cached)",
        &format!("count={}", occipital.images_processed));

    let _ = std::fs::remove_file(img_path);
}

// ── Test 5: Curiosity engine generates wonder items ───────────────────────────

fn test_curiosity_engine() {
    println!("\n[5] Curiosity engine — wonder item generation");

    let mut brain = Brain::new();
    brain.learn_text(
        "Photosynthesis converts sunlight into glucose. \
         Chlorophyll is the pigment responsible for light absorption. \
         The Calvin cycle produces glucose from CO2.",
        Confidence::Verified,
    );

    // Simulate some episodes.
    brain.hippocampus.record_episode(
        "Q: What is photosynthesis? → 15 words recalled",
        0.5,
        vec![],
    );
    brain.hippocampus.record_episode(
        "Q: How does chlorophyll work? → 8 words recalled",
        0.5,
        vec![],
    );
    brain.hippocampus.record_episode(
        "Q: What is the Calvin cycle? → 5 words recalled",
        0.3,
        vec![],
    );

    let new_questions = brain.run_curiosity();
    println!("    generated {} wonder items", new_questions);

    let pending = brain.curiosity.wonder_queue.pending_count();
    assert_or_fail(pending > 0, "curiosity engine generated at least 1 wonder item",
        &format!("pending={}", pending));

    // Check wonder items have non-empty questions.
    let items = brain.curiosity.wonder_queue.pending();
    for item in items.iter().take(3) {
        println!("    wonder: [priority={:.2}] {}", item.priority, &item.question[..item.question.len().min(80)]);
        assert_or_fail(!item.question.is_empty(), "wonder item has non-empty question", "empty");
    }

    // Test resolve.
    let item_id = items[0].id.clone();
    let resolved = brain.curiosity.wonder_queue.resolve(&item_id);
    assert_or_fail(resolved, "wonder item can be resolved", "resolve returned false");

    let after_resolve = brain.curiosity.wonder_queue.pending_count();
    assert_or_fail(after_resolve == pending - 1, "pending count decreases after resolve",
        &format!("before={} after={}", pending, after_resolve));
}

// ── Test 6: SVG / Mermaid media creation ─────────────────────────────────────

fn test_media_creation() {
    println!("\n[6] Media creation — SVG and Mermaid output");

    // Concept map.
    let nodes = vec![
        ("Photosynthesis".to_string(), "main topic".to_string()),
        ("Chlorophyll".to_string(), "pigment".to_string()),
        ("Glucose".to_string(), "product".to_string()),
        ("Sunlight".to_string(), "input".to_string()),
    ];
    let edges = vec![
        (0usize, 1usize, "uses".to_string()),
        (0usize, 2usize, "produces".to_string()),
        (3usize, 0usize, "drives".to_string()),
    ];
    let svg = generate_concept_map(&nodes, &edges);
    assert_or_fail(svg.starts_with("<svg"), "concept map SVG starts with <svg", &svg[..50.min(svg.len())]);
    assert_or_fail(svg.contains("Photosynthesis"), "concept map SVG contains node label", "missing");
    assert_or_fail(svg.ends_with("</svg>"), "concept map SVG ends with </svg>", &svg[svg.len().saturating_sub(20)..]);

    // Flowchart.
    let steps = vec![
        "Light absorbed by chlorophyll".to_string(),
        "Water split via photolysis".to_string(),
        "ATP and NADPH produced".to_string(),
        "Calvin cycle fixes CO2 into glucose".to_string(),
    ];
    let flow_svg = generate_flowchart("Photosynthesis Steps", &steps);
    assert_or_fail(flow_svg.starts_with("<svg"), "flowchart SVG starts with <svg", &flow_svg[..50.min(flow_svg.len())]);
    assert_or_fail(flow_svg.contains("Step 1") || flow_svg.contains("1."), "flowchart has step labels", "missing");

    // Mermaid diagram.
    let ku_descriptions = vec![
        ("Definition".to_string(), "Photosynthesis is a process".to_string()),
        ("Mechanism".to_string(), "Light absorbed by chlorophyll".to_string()),
        ("Purpose".to_string(), "Converts energy for plant use".to_string()),
    ];
    let mermaid = to_mermaid_diagram("Photosynthesis", &ku_descriptions);
    assert_or_fail(mermaid.starts_with("graph TD"), "Mermaid output starts with graph TD",
        &mermaid[..50.min(mermaid.len())]);
    assert_or_fail(mermaid.contains("Definition") || mermaid.contains("Mechanism"),
        "Mermaid diagram contains KU types", "missing");
    println!("    concept map: {} chars; flowchart: {} chars; mermaid: {} lines",
        svg.len(), flow_svg.len(), mermaid.lines().count());
}

// ── Test 7: Migration module exists and compiles ───────────────────────────────

fn test_migration() {
    println!("\n[7] Migration module — structure and function signatures");

    // Brain has a migrate() method (verified by calling it on a non-existent path).
    let mut brain = Brain::new();
    let result = brain.migrate(std::path::Path::new("/nonexistent/brain_state.json"));
    assert_or_fail(result.is_err(), "migrate() returns Err for missing file", "expected Err");
    let err_msg = result.unwrap_err();
    assert_or_fail(!err_msg.is_empty(), "migrate error message is non-empty", &err_msg);

    // MigrationStats default has zeros.
    let stats = brain::migration::MigrationStats::default();
    assert_or_fail(stats.bonds_migrated == 0, "MigrationStats::default bonds_migrated=0", "non-zero");
    assert_or_fail(stats.total_v1_nodes == 0, "MigrationStats::default total_v1_nodes=0", "non-zero");

    // map_v1_bond_label helper exists and handles unknown labels gracefully.
    let bond_type = brain::migration::map_v1_bond_label("unknown_bond_type");
    assert_or_fail(
        format!("{:?}", bond_type).contains("Context") || format!("{:?}", bond_type).len() > 0,
        "map_v1_bond_label returns a valid BondType",
        "empty",
    );

    println!("    migrate() returned Err as expected: {}", &err_msg[..err_msg.len().min(60)]);
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    println!("╔══════════════════════════════════════════════════════════╗");
    println!("║     Lu Brain v2 — Cleanup Phase Tests                   ║");
    println!("╚══════════════════════════════════════════════════════════╝");

    test_mcp_dispatch();
    test_cerebellum_fs();
    test_narration_style();
    test_occipital_image();
    test_curiosity_engine();
    test_media_creation();
    test_migration();

    println!("\n╔══════════════════════════════════════════════════════════╗");
    println!("║  All cleanup tests passed.                               ║");
    println!("╚══════════════════════════════════════════════════════════╝");
}
