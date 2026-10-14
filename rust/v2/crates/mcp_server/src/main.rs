// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! MCP server v2 entry point — stdio JSON-RPC 2.0 server.
//!
//! Claude Desktop (or any MCP host) spawns this process and communicates
//! over stdin/stdout.  The brain lives in a dedicated worker thread;
//! the main loop dispatches tool calls through a bounded mpsc channel.
//!
//! ## Startup sequence
//! 1. Main thread enters the stdin loop immediately — `initialize` and
//!    `tools/list` are handled without touching the brain.
//! 2. A background thread loads the brain (potentially slow for large files).
//! 3. `tools/call` requests are forwarded to the worker with a 55-second
//!    deadline, leaving 5 s headroom for the 60 s MCP host timeout.

use mcp_server::{error_response, handle_initialize, handle_tools_list, tool_error, McpServer};
use serde_json::Value;
use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

/// Message from the stdin loop to the brain worker thread.
struct BrainRequest {
    id:     Value,
    params: Value,
    reply:  mpsc::Sender<String>,
}

fn main() {
    eprintln!("[lu-mcp-v2] Starting Lu Brain v2.0 MCP server");

    let brain_path = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from);

    // ── Brain worker thread ───────────────────────────────────────────────────
    // Loads the brain (potentially minutes for a 1.9 GB file) then processes
    // tool calls serially.  The main stdin loop can respond to `initialize`
    // and `tools/list` without waiting for this to finish.
    let (req_tx, req_rx) = mpsc::channel::<BrainRequest>();
    let brain_ready = Arc::new(AtomicBool::new(false));
    let brain_ready_worker = brain_ready.clone();

    let brain_path_worker = brain_path.clone();
    std::thread::spawn(move || {
        eprintln!("[lu-mcp-v2] Brain loading — stdin loop is live.");
        let mut server = McpServer::new(brain_path_worker);
        brain_ready_worker.store(true, Ordering::Release);
        eprintln!("[lu-mcp-v2] Brain ready — processing tool calls.");

        for req in req_rx {
            let response = server.handle_tool_call(&req.id, &req.params);
            let _ = req.reply.send(response);
        }
        eprintln!("[lu-mcp-v2] Brain worker exiting.");
    });

    // ── Stdin loop ────────────────────────────────────────────────────────────
    let stdin  = std::io::stdin();
    let stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) if !l.trim().is_empty() => l,
            Ok(_)  => continue,
            Err(e) => {
                eprintln!("[lu-mcp-v2] stdin error: {}", e);
                break;
            }
        };

        let request: Value = match serde_json::from_str(&line) {
            Ok(v)  => v,
            Err(e) => {
                let response = error_response(
                    Value::Null,
                    -32700,
                    &format!("Parse error: {}", e),
                );
                let mut out = stdout.lock();
                let _ = writeln!(out, "{}", response);
                continue;
            }
        };

        // JSON-RPC 2.0: notifications have no "id" field — do not send any response.
        if request.get("id").is_none() {
            continue;
        }

        let id     = &request["id"];
        let method = request["method"].as_str().unwrap_or("");
        let params = &request["params"];

        let response = match method {
            // These don't need the brain — respond immediately so Claude
            // Desktop sees the server as alive before Brain::load finishes.
            "initialize" => handle_initialize(id),
            "tools/list" => handle_tools_list(id),

            // All tool calls are forwarded to the brain worker.
            // If the brain isn't ready yet, return immediately so the caller
            // can retry rather than blocking for the full 55-second deadline.
            "tools/call" => {
                if !brain_ready.load(Ordering::Acquire) {
                    tool_error(
                        id,
                        "Brain is still loading — please retry in a few seconds.",
                    )
                } else {
                    let (reply_tx, reply_rx) = mpsc::channel();
                    let req = BrainRequest {
                        id:     id.clone(),
                        params: params.clone(),
                        reply:  reply_tx,
                    };
                    if req_tx.send(req).is_err() {
                        tool_error(id, "Brain worker has terminated unexpectedly")
                    } else {
                        match reply_rx.recv_timeout(Duration::from_secs(55)) {
                            Ok(resp) => resp,
                            Err(mpsc::RecvTimeoutError::Timeout) => tool_error(
                                id,
                                "Request timed out after 55 seconds — operation was too slow. \
                                 Please retry.",
                            ),
                            Err(mpsc::RecvTimeoutError::Disconnected) => tool_error(
                                id,
                                "Brain worker disconnected",
                            ),
                        }
                    }
                }
            }

            _ => error_response(id.clone(), -32601, "Method not found"),
        };

        let mut out = stdout.lock();
        if let Err(e) = writeln!(out, "{}", response) {
            eprintln!("[lu-mcp-v2] stdout write error: {}", e);
            break;
        }
    }

    eprintln!("[lu-mcp-v2] Shutting down");
}
