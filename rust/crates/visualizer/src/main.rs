// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Lu Ai Brain Visualizer
//!
//! A lightweight HTTP server that serves an interactive force-directed
//! graph of the brain's knowledge network. Open http://localhost:3333
//! in your browser to see the brain map.
//!
//! Usage: lu_ai_viz [brain_storage_path]
//! Default path: ./BrainStorage

use std::sync::Arc;
use std::path::PathBuf;
use tiny_http::{Server, Response, Header};

// The HTML page is embedded at compile time
const INDEX_HTML: &str = include_str!("index.html");

fn main() {
    // Parse args: optional brain storage path
    let args: Vec<String> = std::env::args().collect();
    let storage_path = if args.len() > 1 {
        args[1].clone()
    } else {
        // Default: look for BrainStorage relative to exe
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        // Try common locations — same priority as MCP server
        let candidates = vec![
            PathBuf::from(r"brain"),
            exe_dir.join("brain"),
            PathBuf::from("BrainStorage"),
        ];

        candidates.into_iter()
            .find(|p| p.exists())
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                eprintln!("[viz] Warning: No BrainStorage found. Using current dir.");
                "BrainStorage".to_string()
            })
    };

    eprintln!("[viz] Loading brain from: {}", storage_path);

    // Load the brain
    let storage_pathbuf = PathBuf::from(&storage_path);
    let brain = match brain::Brain::load(&storage_pathbuf) {
        Ok(b) => {
            let stats = b.stats();
            eprintln!(
                "[viz] Brain loaded: {} nodes, {} bonds",
                stats.active_nodes, stats.active_bonds
            );
            b
        }
        Err(e) => {
            eprintln!("[viz] Failed to load brain: {}", e);
            eprintln!("[viz] Usage: lu_ai_viz [path/to/BrainStorage]");
            std::process::exit(1);
        }
    };

    // Pre-compute the graph JSON (it's static — brain is read-only here)
    eprintln!("[viz] Building graph data...");
    let graph_json = brain.viz_graph_json();
    eprintln!("[viz] Graph JSON: {} bytes", graph_json.len());

    let graph_json = Arc::new(graph_json);

    // Start HTTP server
    let port = 3334;
    let server = Server::http(format!("0.0.0.0:{}", port)).unwrap_or_else(|e| {
        eprintln!("[viz] Failed to start server: {}", e);
        std::process::exit(1);
    });

    eprintln!("[viz] ╔════════════════════════════════════════╗");
    eprintln!("[viz] ║  Brain Map running at:                 ║");
    eprintln!("[viz] ║  http://localhost:{}                  ║", port);
    eprintln!("[viz] ╚════════════════════════════════════════╝");
    eprintln!("[viz] Press Ctrl+C to stop.");

    // Handle requests
    for request in server.incoming_requests() {
        let url = request.url().to_string();
        let graph = Arc::clone(&graph_json);

        match url.as_str() {
            "/" | "/index.html" => {
                let header = Header::from_bytes(
                    &b"Content-Type"[..],
                    &b"text/html; charset=utf-8"[..],
                ).unwrap();
                let response = Response::from_string(INDEX_HTML)
                    .with_header(header);
                let _ = request.respond(response);
            }
            "/api/graph" => {
                let header = Header::from_bytes(
                    &b"Content-Type"[..],
                    &b"application/json"[..],
                ).unwrap();
                let response = Response::from_string(graph.as_str())
                    .with_header(header);
                let _ = request.respond(response);
            }
            "/ws" => {
                // WebSocket upgrade — for now, just return 501
                // Live updates will be added in the next iteration
                let response = Response::from_string("WebSocket not yet implemented")
                    .with_status_code(501);
                let _ = request.respond(response);
            }
            _ => {
                let response = Response::from_string("Not Found")
                    .with_status_code(404);
                let _ = request.respond(response);
            }
        }
    }
}
