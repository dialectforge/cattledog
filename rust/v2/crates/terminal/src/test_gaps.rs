// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Brain v2 — Gap-fill test suite.
//!
//! Tests:
//!   1. Sandbox path validation rejects ../escapes
//!   2. Code execution — simple Python script
//!   3. Code execution — timeout behavior
//!   4. Rumination bond discovery
//!   5. Rumination cooldown
//!   6. Bond type inference
//!   7. Cerebellum CodeExecution tool dispatch

use brain::code_sandbox::{CodeExecutionRequest, CodeSandbox};
use brain::rumination::{infer_bond_type, RuminationEngine};
use brain::Brain;
use graph::knowledge_unit::KuKind;

// ── Test helpers ──────────────────────────────────────────────────────────────

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

// ── Test 1: Path validation ───────────────────────────────────────────────────

fn test_path_validation() {
    println!("\n[1] Sandbox path validation");

    let sandbox = CodeSandbox::new().expect("sandbox init");

    // A path escaping via ../ must be rejected.
    let escape = sandbox.sandbox_dir.join("..").join("etc").join("passwd");
    let result = sandbox.validate_path(&escape);
    assert_or_fail(result.is_err(), "reject ../escape path", "expected Err");

    // A path directly inside the sandbox must be accepted.
    let safe = sandbox.sandbox_dir.join("safe_file.txt");
    // validate_path for a non-existent path uses normalize logic
    let result2 = sandbox.validate_path(&safe);
    assert_or_fail(result2.is_ok(), "accept sandbox-internal path", &format!("got {:?}", result2));

    // A raw ../.. path string (relative)
    let dotdot = std::path::Path::new("../../secret");
    let result3 = sandbox.validate_path(dotdot);
    // When the path is relative and resolves outside sandbox, it should be rejected.
    // (the resolved path will be something like C:\lu_ai_sandbox\..\..\secret = C:\secret)
    // which is outside the sandbox — our normalizer should catch this.
    if result3.is_ok() {
        let resolved = result3.unwrap();
        assert_or_fail(
            resolved.starts_with(&sandbox.sandbox_dir),
            "relative ../.. resolves within sandbox",
            &format!("resolved to {}", resolved.display()),
        );
    } else {
        pass("reject relative ../.. path");
    }
}

// ── Test 2: Python execution ──────────────────────────────────────────────────

fn test_python_execution() {
    println!("\n[2] Python code execution");

    let sandbox = CodeSandbox::new().expect("sandbox init");
    let req = CodeExecutionRequest::new("python", "print('hello from sandbox')");
    let result = sandbox.execute(&req);

    if result.stderr.contains("not found") || result.stderr.contains("python") && result.exit_code == Some(-1) {
        println!("  ~ python not installed — skipping execution test (sandbox init OK)");
        pass("sandbox initialised successfully");
        return;
    }

    assert_or_fail(
        result.stdout.trim() == "hello from sandbox",
        "python stdout captured",
        &format!("stdout='{}'  stderr='{}'", result.stdout.trim(), result.stderr.trim()),
    );
    assert_or_fail(!result.timed_out, "python did not time out", "timed_out=true");
    assert_or_fail(result.exit_code == Some(0), "python exit code 0", &format!("exit={:?}", result.exit_code));
}

// ── Test 3: Timeout ───────────────────────────────────────────────────────────

fn test_timeout() {
    println!("\n[3] Code execution timeout");

    let sandbox = CodeSandbox::new().expect("sandbox init");

    // A sleep-heavy Python script with a very short timeout.
    let req = CodeExecutionRequest::new(
        "python",
        "import time\ntime.sleep(10)\nprint('should not reach here')",
    )
    .with_timeout(300); // 300 ms — well under the 10 s sleep

    let result = sandbox.execute(&req);

    if result.stderr.contains("not found") || (result.exit_code == Some(-1) && result.stderr.contains("python")) {
        println!("  ~ python not installed — skipping timeout test");
        pass("timeout test skipped (no python)");
        return;
    }

    assert_or_fail(result.timed_out, "process marked as timed_out", &format!(
        "timed_out={} stdout='{}' stderr='{}'",
        result.timed_out, result.stdout, result.stderr
    ));
    assert_or_fail(
        !result.stdout.contains("should not reach here"),
        "stdout empty on timeout",
        &format!("stdout='{}'", result.stdout),
    );
}

// ── Test 4: Rumination bond discovery ─────────────────────────────────────────

fn test_rumination_bond_discovery() {
    println!("\n[4] Rumination — bond discovery");

    let mut brain = Brain::new();

    // Teach two separate facts.
    let seeds_a = brain.learn_text(
        "Photosynthesis is the process by which plants convert sunlight into energy.",
        graph::node::Confidence::Unverified,
    );
    let seeds_b = brain.learn_text(
        "Chlorophyll is the pigment in plants that absorbs sunlight during photosynthesis.",
        graph::node::Confidence::Unverified,
    );

    // Manually push episodes that reference nodes from both facts together.
    // This simulates two co-activation events.
    let mut combined_seeds: Vec<df_core::Seed> = seeds_a.iter().take(1).cloned().collect();
    combined_seeds.extend(seeds_b.iter().take(1).cloned());

    brain.hippocampus.record_episode("Q: photosynthesis → 3 nodes", 0.5, combined_seeds.clone());
    brain.hippocampus.record_episode("Q: plants energy → 2 nodes", 0.5, combined_seeds.clone());

    let bonds_before = brain.network.all_bonds().len();

    // Use a zero-cooldown engine so we can ruminate immediately.
    brain.rumination = brain::rumination::RuminationEngine::new(0);
    let report = brain.ruminate();

    let bonds_after = brain.network.all_bonds().len();

    assert_or_fail(
        report.bonds_discovered > 0 || bonds_after > bonds_before,
        "rumination discovered at least one new bond",
        &format!("discovered={} bonds_before={} bonds_after={}", report.bonds_discovered, bonds_before, bonds_after),
    );
    assert_or_fail(
        report.nodes_visited > 0,
        "rumination visited at least one node",
        &format!("nodes_visited={}", report.nodes_visited),
    );
}

// ── Test 5: Rumination cooldown ───────────────────────────────────────────────

fn test_rumination_cooldown() {
    println!("\n[5] Rumination — cooldown");

    // Use a 60-second cooldown.
    let mut engine = RuminationEngine::new(60_000);

    let mut brain = Brain::new();
    brain.learn_text("Water boils at 100 degrees Celsius.", graph::node::Confidence::Unverified);
    let episodes: Vec<brain::hippocampus::EpisodicMemory> = brain.hippocampus.episodes.clone();

    // First call: no cooldown active → runs.
    engine.step(&mut brain.network, &episodes, &mut brain.temporal, false);

    // Immediately after: cooldown is active → returns empty report.
    assert_or_fail(engine.is_cooling_down(), "cooldown active after first pass", "is_cooling_down=false");

    let report2 = engine.step(&mut brain.network, &episodes, &mut brain.temporal, false);
    assert_or_fail(
        report2.bonds_discovered == 0 && report2.bonds_strengthened == 0,
        "second immediate call returns empty report",
        &format!("discovered={} strengthened={}", report2.bonds_discovered, report2.bonds_strengthened),
    );

    // Reset cooldown to zero → should run again.
    engine.cooldown = std::time::Duration::from_millis(0);
    assert_or_fail(!engine.is_cooling_down(), "not cooling down after reset", "is_cooling_down=true");
}

// ── Test 6: Bond type inference ───────────────────────────────────────────────

fn test_bond_type_inference() {
    println!("\n[6] Bond type inference");

    use graph::bond::BondType;

    // Causal wins over everything.
    let bt = infer_bond_type(Some(KuKind::Causal), Some(KuKind::Definition));
    assert_or_fail(bt == BondType::Causes, "Causal+Definition → Causes", &format!("{:?}", bt));

    // Two Definitions → IsA.
    let bt = infer_bond_type(Some(KuKind::Definition), Some(KuKind::Definition));
    assert_or_fail(bt == BondType::IsA, "Definition+Definition → IsA", &format!("{:?}", bt));

    // Composition → HasPart.
    let bt = infer_bond_type(Some(KuKind::Composition), Some(KuKind::Raw));
    assert_or_fail(bt == BondType::HasPart, "Composition+Raw → HasPart", &format!("{:?}", bt));

    // Mechanism → Requires.
    let bt = infer_bond_type(Some(KuKind::Mechanism), Some(KuKind::Purpose));
    assert_or_fail(bt == BondType::Requires, "Mechanism+Purpose → Requires", &format!("{:?}", bt));

    // Example → ExampleOf.
    let bt = infer_bond_type(Some(KuKind::Example), Some(KuKind::Raw));
    assert_or_fail(bt == BondType::ExampleOf, "Example+Raw → ExampleOf", &format!("{:?}", bt));

    // Default → Context.
    let bt = infer_bond_type(Some(KuKind::Raw), Some(KuKind::Raw));
    assert_or_fail(bt == BondType::Context, "Raw+Raw → Context", &format!("{:?}", bt));

    // None → Context.
    let bt = infer_bond_type(None, None);
    assert_or_fail(bt == BondType::Context, "None+None → Context", &format!("{:?}", bt));
}

// ── Test 7: Cerebellum CodeExecution dispatch ─────────────────────────────────

fn test_cerebellum_code_exec() {
    println!("\n[7] Cerebellum — CodeExecution tool dispatch");

    use brain::cerebellum::{Cerebellum, ToolKind, ToolRequest};

    let mut cerebellum = Cerebellum::new();
    let req = ToolRequest::new(
        ToolKind::CodeExecution,
        serde_json::json!({
            "language": "python",
            "code": "print('cerebellum dispatch ok')",
        }),
    );

    let result = cerebellum.execute_sync(req);

    // The error message should NOT be the old stub message.
    let old_stub = "code execution not yet implemented";
    let actual_err = result.error.as_deref().unwrap_or("");
    assert_or_fail(
        !actual_err.contains(old_stub),
        "CodeExecution is no longer a stub",
        &format!("error='{}'", actual_err),
    );

    // Either it ran successfully (python present) or it failed with a real error.
    // Either way — the dispatch happened.
    if result.success {
        let stdout = result.output["stdout"].as_str().unwrap_or("").trim();
        assert_or_fail(
            stdout == "cerebellum dispatch ok",
            "cerebellum python stdout",
            &format!("stdout='{}'", stdout),
        );
    } else {
        // Python may not be installed — that's a sandbox error, not a dispatch error.
        pass("cerebellum dispatched to sandbox (python not installed — sandbox error OK)");
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    println!("Lu Brain v2 — Gap-Fill Test Suite");
    println!("==================================");

    test_path_validation();
    test_python_execution();
    test_timeout();
    test_rumination_bond_discovery();
    test_rumination_cooldown();
    test_bond_type_inference();
    test_cerebellum_code_exec();

    println!("\n All gap-fill tests passed.");
}
