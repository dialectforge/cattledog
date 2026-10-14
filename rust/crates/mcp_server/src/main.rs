#![recursion_limit = "512"]
// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Lu Ai MCP Server
//!
//! stdio JSON-RPC server implementing the Model Context Protocol.
//! Claude Desktop spawns this as a subprocess and calls brain
//! operations as MCP tools. This is how the brain gets trained
//! at scale — Claude generates structured knowledge, feeds it
//! through MCP, and the brain ingests it.
//!
//! Protocol: JSON-RPC 2.0 over stdio
//! Transport: stdin/stdout (stderr for logging)
//!
//! # Architecture
//!
//! Brain lives in a dedicated worker thread.  The main stdin loop
//! dispatches every tools/call to that thread via an mpsc channel
//! and waits for the response.  This keeps the loop responsive
//! (even when a slow brain operation is running) and prevents any
//! future rumination thread from starving MCP tool calls.

mod tools;
mod protocol;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use brain::Brain;

/// Message sent from the main thread to the brain worker.
struct BrainRequest {
    id:     serde_json::Value,
    params: serde_json::Value,
    /// One-shot channel: worker sends the JSON-RPC response back here.
    reply:  mpsc::SyncSender<String>,
}

fn main() {
    eprintln!("[lu_ai_mcp] Starting MCP server...");

    // ─── Resolve storage path ─────────────────────────────────────────────────
    // Priority: command-line arg > LU_AI_BRAIN env var > default.
    let storage_path: PathBuf = if let Some(arg) = std::env::args().nth(1) {
        PathBuf::from(arg)
    } else if let Ok(env_path) = std::env::var("LU_AI_BRAIN") {
        PathBuf::from(env_path)
    } else {
        let project_path = PathBuf::from(r"brain");
        if project_path.parent().map_or(false, |p| p.exists()) {
            project_path
        } else {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("lu_ai")
                .join("brain")
        }
    };

    // ─── Load brain (refuse to overwrite an existing but broken file) ─────────
    let brain_file = storage_path.join("brain_state.json");
    let mut brain = match Brain::load(&storage_path) {
        Ok(b) => {
            eprintln!("[lu_ai_mcp] Brain loaded from {:?}", storage_path);
            b
        }
        Err(e) => {
            if brain_file.exists() {
                eprintln!("[lu_ai_mcp] FATAL: Brain file exists at {:?} but failed to load: {}", brain_file, e);
                eprintln!("[lu_ai_mcp] Refusing to create a new brain over an existing one.");
                eprintln!("[lu_ai_mcp] Fix the brain file or delete it manually to start fresh.");
                std::process::exit(1);
            }
            eprintln!("[lu_ai_mcp] First run. Creating new brain.");
            let mut b = Brain::new(256);
            b.set_storage(storage_path.clone());
            b
        }
    };
    brain.set_storage(storage_path);

    // Ensure Lu always knows about its creator (idempotent).
    let seeded = brain.seed_creator_knowledge();
    if seeded > 0 {
        eprintln!("[lu_ai_mcp] Seeded {} creator knowledge facts.", seeded);
        let _ = brain.save();
    }

    // ─── Brain worker thread ──────────────────────────────────────────────────
    // The worker owns Brain exclusively.  All tools/call requests are serialised
    // through this thread so rumination (when added) never starves tool calls —
    // the worker can be extended to use try_lock or yield points as needed.
    // Bounded capacity 1 provides natural back-pressure.
    let (req_tx, req_rx) = mpsc::sync_channel::<BrainRequest>(1);

    std::thread::Builder::new()
        .name("brain-worker".into())
        .spawn(move || {
            eprintln!("[lu_ai_mcp] Brain worker started.");
            while let Ok(req) = req_rx.recv() {
                let response = tools::handle_call(&req.id, &req.params, &mut brain);
                let _ = req.reply.send(response);
                // After the response is sent, drain any auto-resolve work queued
                // by recall(). This runs between requests so it never blocks the
                // caller that triggered the recall.
                if let Some(result) = brain.drain_auto_resolve() {
                    eprintln!("[auto_resolve] {}", result.summary);
                    let _ = brain.save();
                }
            }
            eprintln!("[lu_ai_mcp] Brain worker shutting down.");
        })
        .expect("failed to spawn brain-worker thread");

    // ─── stdio read loop ──────────────────────────────────────────────────────
    let stdin  = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l)  => l,
            Err(e) => {
                eprintln!("[lu_ai_mcp] stdin error: {}", e);
                break;
            }
        };

        let line = line.trim().to_string();
        if line.is_empty() { continue; }

        let request: serde_json::Value = match serde_json::from_str(&line) {
            Ok(v)  => v,
            Err(e) => {
                let err = protocol::error_response(
                    serde_json::Value::Null,
                    -32700,
                    &format!("Parse error: {}", e),
                );
                let _ = writeln!(stdout, "{}", err);
                let _ = stdout.flush();
                continue;
            }
        };

        let id     = request.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

        eprintln!("[lu_ai_mcp] method={}", method);

        let response = match method {
            // Lightweight — handled inline, no brain access needed.
            "initialize"                => protocol::handle_initialize(&id),
            "notifications/initialized" => continue, // no response
            "tools/list"                => tools::handle_list(&id),

            // Brain operations — dispatched to the worker thread.
            "tools/call" => {
                let params = request.get("params").cloned()
                    .unwrap_or(serde_json::Value::Object(Default::default()));

                let (reply_tx, reply_rx) = mpsc::sync_channel::<String>(1);

                if req_tx.send(BrainRequest { id: id.clone(), params, reply: reply_tx }).is_err() {
                    eprintln!("[lu_ai_mcp] Brain worker is gone. Shutting down.");
                    break;
                }

                match reply_rx.recv() {
                    Ok(resp) => resp,
                    Err(_) => {
                        eprintln!("[lu_ai_mcp] Brain worker dropped reply channel.");
                        protocol::error_response(id, -32603, "Brain worker unavailable")
                    }
                }
            }

            _ => protocol::error_response(id, -32601, &format!("Unknown method: {}", method)),
        };

        let _ = writeln!(stdout, "{}", response);
        let _ = stdout.flush();
    }

    eprintln!("[lu_ai_mcp] Server shutting down.");
}
