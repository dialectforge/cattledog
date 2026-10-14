// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Lu AI Desktop — Web UI
//!
//! A modern web-based interface for Lu AI, served via HTTP.
//! Open http://localhost:3335 in your browser.
//!
//! Usage: lu_ai_desktop [brain_storage_path]

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use brain::Brain;
use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server};

const INDEX_HTML: &str = include_str!("index.html");
const PORT: u16 = 3335;

fn json_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap()
}

fn html_header() -> Header {
    Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap()
}

fn cors_headers() -> Vec<Header> {
    vec![
        Header::from_bytes(&b"Access-Control-Allow-Origin"[..], &b"*"[..]).unwrap(),
        Header::from_bytes(
            &b"Access-Control-Allow-Methods"[..],
            &b"GET, POST, PUT, DELETE, OPTIONS"[..],
        )
        .unwrap(),
        Header::from_bytes(
            &b"Access-Control-Allow-Headers"[..],
            &b"Content-Type"[..],
        )
        .unwrap(),
    ]
}

fn respond_json(request: Request, data: Value) {
    let body = data.to_string();
    let mut response = Response::from_string(body).with_header(json_header());
    for h in cors_headers() {
        response = response.with_header(h);
    }
    let _ = request.respond(response);
}

fn respond_error(request: Request, status: u16, msg: &str) {
    let body = json!({"error": msg}).to_string();
    let mut response = Response::from_string(body)
        .with_header(json_header())
        .with_status_code(status);
    for h in cors_headers() {
        response = response.with_header(h);
    }
    let _ = request.respond(response);
}

fn read_body(request: &mut Request) -> Option<Value> {
    let mut body = String::new();
    request.as_reader().read_to_string(&mut body).ok()?;
    serde_json::from_str(&body).ok()
}

fn resolve_storage_path() -> PathBuf {
    if let Some(arg) = std::env::args().nth(1) {
        return PathBuf::from(arg);
    }
    if let Ok(env_path) = std::env::var("LU_AI_BRAIN") {
        return PathBuf::from(env_path);
    }
    let project_path = PathBuf::from(r"brain");
    if project_path.parent().map_or(false, |p| p.exists()) {
        return project_path;
    }
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ai_factory")
        .join("brain")
}

fn main() {
    let storage_path = resolve_storage_path();
    eprintln!("[desktop] Loading brain from: {:?}", storage_path);

    let brain_file = storage_path.join("brain_state.json");
    let brain = match Brain::load(&storage_path) {
        Ok(b) => {
            let stats = b.stats();
            eprintln!(
                "[desktop] Brain loaded: {} nodes, {} bonds",
                stats.active_nodes, stats.active_bonds
            );
            b
        }
        Err(e) => {
            if brain_file.exists() {
                eprintln!(
                    "FATAL: Brain file exists at {:?} but failed to load: {}",
                    brain_file, e
                );
                std::process::exit(1);
            }
            eprintln!("[desktop] No existing brain found. Creating new brain.");
            let mut b = Brain::new(256);
            b.set_storage(storage_path.clone());
            b
        }
    };

    let brain = Arc::new(Mutex::new(brain));

    let server = Server::http(format!("0.0.0.0:{}", PORT)).unwrap_or_else(|e| {
        eprintln!("[desktop] Failed to start server: {}", e);
        std::process::exit(1);
    });

    eprintln!("[desktop] ╔════════════════════════════════════════╗");
    eprintln!("[desktop] ║  Lu AI Desktop running at:              ║");
    eprintln!("[desktop] ║  http://localhost:{}                  ║", PORT);
    eprintln!("[desktop] ╚════════════════════════════════════════╝");

    let server = Arc::new(server);
    let num_workers = 4;

    for _ in 0..num_workers {
        let server = Arc::clone(&server);
        let brain = Arc::clone(&brain);

        thread::spawn(move || {
            loop {
                let request = match server.recv() {
                    Ok(r) => r,
                    Err(_) => break,
                };
                handle_request(request, &brain);
            }
        });
    }

    // Block main thread
    loop {
        thread::park();
    }
}

fn handle_request(mut request: Request, brain: &Arc<Mutex<Brain>>) {
    let url = request.url().to_string();
    let method = request.method().clone();

    // Handle CORS preflight
    if matches!(method, Method::Options) {
        let mut response = Response::from_string("");
        for h in cors_headers() {
            response = response.with_header(h);
        }
        let _ = request.respond(response);
        return;
    }

    match (method.clone(), url.as_str()) {
        // ─── Static Assets ──────────────────────────────
        (Method::Get, "/" | "/index.html") => {
            let mut response = Response::from_string(INDEX_HTML).with_header(html_header());
            for h in cors_headers() {
                response = response.with_header(h);
            }
            let _ = request.respond(response);
        }

        // ─── Chat ───────────────────────────────────────
        (Method::Post, "/api/chat") => {
            let body = match read_body(&mut request) {
                Some(b) => b,
                None => return respond_error(request, 400, "Invalid JSON"),
            };
            let message = body["message"].as_str().unwrap_or("").to_string();
            if message.is_empty() {
                return respond_error(request, 400, "Missing 'message' field");
            }
            let conversation_id = body["conversation_id"].as_str().map(|s| s.to_string());

            let mut brain = brain.lock().unwrap();

            // Save user message to conversation
            if let Some(ref cid) = conversation_id {
                brain.add_conversation_message(
                    cid,
                    brain::conversations::MessageRole::User,
                    &message,
                    vec![],
                    vec![],
                );
            }

            let response = brain.respond(&message, 3, 12);

            // Save assistant response to conversation
            if let Some(ref cid) = conversation_id {
                brain.add_conversation_message(
                    cid,
                    brain::conversations::MessageRole::Assistant,
                    &response.text,
                    vec![],
                    response.sources.clone(),
                );
            }

            respond_json(
                request,
                json!({
                    "text": response.text,
                    "atoms_used": response.atoms_used,
                    "traversal_hops": response.traversal_hops,
                    "coherence_score": response.coherence_score,
                    "sources": response.sources.iter().map(|(hex, label, pct)| {
                        json!({"seed": hex, "label": label, "contribution": pct})
                    }).collect::<Vec<_>>(),
                    "scar_warnings": response.scar_warnings,
                }),
            );

            // Auto-save after interaction
            let _ = brain.save();
        }

        // ─── Stats ──────────────────────────────────────
        (Method::Get, "/api/stats") => {
            let brain = brain.lock().unwrap();
            let stats = brain.stats();
            respond_json(
                request,
                json!({
                    "total_nodes": stats.total_nodes,
                    "active_nodes": stats.active_nodes,
                    "revoked_nodes": stats.revoked_nodes,
                    "total_bonds": stats.total_bonds,
                    "active_bonds": stats.active_bonds,
                    "dead_bonds": stats.dead_bonds,
                    "scars": stats.scars,
                }),
            );
        }

        // ─── Proposals ──────────────────────────────────
        (Method::Get, "/api/proposals") => {
            let brain = brain.lock().unwrap();
            let stats = brain.stats();
            respond_json(
                request,
                json!({
                    "proposals": [],
                    "note": "Use POST /api/ingest to create proposals, then approve/reject by ID",
                    "brain_nodes": stats.active_nodes,
                }),
            );
        }

        // ─── Approve Proposal ───────────────────────────
        (Method::Post, path) if path.starts_with("/api/approve/") => {
            let id = &path["/api/approve/".len()..];
            let mut brain = brain.lock().unwrap();
            match brain.approve_proposal(id) {
                Some(seed) => {
                    let _ = brain.save();
                    respond_json(
                        request,
                        json!({"status": "approved", "seed": seed.to_hex()}),
                    );
                }
                None => respond_error(request, 404, "Proposal not found"),
            }
        }

        // ─── Reject Proposal ────────────────────────────
        (Method::Post, path) if path.starts_with("/api/reject/") => {
            let id = &path["/api/reject/".len()..];
            let mut brain = brain.lock().unwrap();
            if brain.reject_proposal(id) {
                respond_json(request, json!({"status": "rejected"}));
            } else {
                respond_error(request, 404, "Proposal not found");
            }
        }

        // ─── Ingest ─────────────────────────────────────
        (Method::Post, "/api/ingest") => {
            let body = match read_body(&mut request) {
                Some(b) => b,
                None => return respond_error(request, 400, "Invalid JSON"),
            };

            let mut brain = brain.lock().unwrap();

            if let Some(text) = body["text"].as_str() {
                let source = body["source"].as_str().unwrap_or("web-ui");
                let report = brain.ingest_text(text, source);
                let _ = brain.save();
                respond_json(
                    request,
                    json!({
                        "source": report.source,
                        "total_sentences": report.total_sentences,
                        "proposals_ready": report.proposals_ready,
                        "proposals_blocked": report.proposals_blocked,
                        "duplicates_skipped": report.duplicates_skipped,
                        "proposals": report.proposals.iter().map(|p| {
                            json!({
                                "id": p.id,
                                "content": p.content,
                                "status": p.status,
                                "scar_conflicts": p.scar_conflicts,
                                "similar_existing": p.similar_existing.iter().map(|(hex, label, sim)| {
                                    json!({"seed": hex, "label": label, "similarity": sim})
                                }).collect::<Vec<_>>(),
                            })
                        }).collect::<Vec<_>>(),
                    }),
                );
            } else if let Some(path) = body["path"].as_str() {
                match brain.ingest_file(path) {
                    Ok(report) => {
                        let _ = brain.save();
                        respond_json(
                            request,
                            json!({
                                "source": report.source,
                                "total_sentences": report.total_sentences,
                                "proposals_ready": report.proposals_ready,
                                "proposals_blocked": report.proposals_blocked,
                                "duplicates_skipped": report.duplicates_skipped,
                                "proposals": report.proposals.iter().map(|p| {
                                    json!({
                                        "id": p.id,
                                        "content": p.content,
                                        "status": p.status,
                                    })
                                }).collect::<Vec<_>>(),
                            }),
                        );
                    }
                    Err(e) => respond_error(request, 500, &e),
                }
            } else {
                respond_error(request, 400, "Missing 'text' or 'path' field");
            }
        }

        // ─── Graph (for Brain Map) ──────────────────────
        (Method::Get, "/api/graph") => {
            let brain = brain.lock().unwrap();
            let graph_json = brain.viz_graph_json();
            let mut response = Response::from_string(graph_json).with_header(json_header());
            for h in cors_headers() {
                response = response.with_header(h);
            }
            let _ = request.respond(response);
        }

        // ─── Scars ──────────────────────────────────────
        (Method::Get, "/api/scars") => {
            let brain = brain.lock().unwrap();
            let scars = brain.scars();
            let scar_list: Vec<Value> = scars
                .iter()
                .map(|s| {
                    json!({
                        "seed": s.revoked_seed.to_hex(),
                        "label": s.label,
                        "reason": s.reason,
                        "revoked_at": s.revoked_at.to_rfc3339(),
                    })
                })
                .collect();
            respond_json(request, json!({"scars": scar_list}));
        }

        // ─── Learn (direct terminal-confidence learning) ─
        (Method::Post, "/api/learn") => {
            let body = match read_body(&mut request) {
                Some(b) => b,
                None => return respond_error(request, 400, "Invalid JSON"),
            };
            let content = match body["content"].as_str() {
                Some(c) if !c.is_empty() => c.to_string(),
                _ => return respond_error(request, 400, "Missing 'content' field"),
            };

            let mut brain = brain.lock().unwrap();
            let seed = brain.learn(
                &content,
                graph::Confidence::Terminal,
                "desktop-terminal",
            );
            let _ = brain.save();
            respond_json(
                request,
                json!({"status": "learned", "seed": seed.to_hex()}),
            );
        }

        // ─── Propose (gated learning) ───────────────────
        (Method::Post, "/api/propose") => {
            let body = match read_body(&mut request) {
                Some(b) => b,
                None => return respond_error(request, 400, "Invalid JSON"),
            };
            let content = match body["content"].as_str() {
                Some(c) if !c.is_empty() => c.to_string(),
                _ => return respond_error(request, 400, "Missing 'content' field"),
            };
            let source = body["source"].as_str().unwrap_or("desktop-ui").to_string();

            let mut brain = brain.lock().unwrap();
            let proposal = brain.propose(&content, &source);
            respond_json(
                request,
                json!({
                    "id": proposal.id,
                    "content": proposal.content,
                    "source": proposal.source,
                    "recommendation": proposal.recommendation,
                    "similar_existing": proposal.similar_existing.iter().map(|(hex, label, sim)| {
                        json!({"seed": hex, "label": label, "similarity": sim})
                    }).collect::<Vec<_>>(),
                    "scar_conflicts": proposal.scar_conflicts,
                }),
            );
        }

        // ─── Code Generate ──────────────────────────────
        (Method::Post, "/api/code/generate") => {
            let body = match read_body(&mut request) {
                Some(b) => b,
                None => return respond_error(request, 400, "Invalid JSON"),
            };
            let description = match body["description"].as_str() {
                Some(d) if !d.is_empty() => d.to_string(),
                _ => return respond_error(request, 400, "Missing 'description' field"),
            };
            let code = match body["code"].as_str() {
                Some(c) if !c.is_empty() => c.to_string(),
                _ => return respond_error(request, 400, "Missing 'code' field"),
            };
            let language = body["language"].as_str().unwrap_or("python").to_string();

            let mut brain = brain.lock().unwrap();
            let block = brain.code_generate(&description, &language, &code);
            respond_json(
                request,
                json!({
                    "language": block.language,
                    "code": block.code,
                    "description": block.description,
                    "auto_execute": block.auto_execute,
                    "sandbox_dir": brain.sandbox_dir().to_string_lossy(),
                }),
            );
        }

        // ─── Code Execute ───────────────────────────────
        (Method::Post, "/api/code/execute") => {
            let body = match read_body(&mut request) {
                Some(b) => b,
                None => return respond_error(request, 400, "Invalid JSON"),
            };
            let language = match body["language"].as_str() {
                Some(l) if !l.is_empty() => l.to_string(),
                _ => return respond_error(request, 400, "Missing 'language' field"),
            };
            let code = match body["code"].as_str() {
                Some(c) if !c.is_empty() => c.to_string(),
                _ => return respond_error(request, 400, "Missing 'code' field"),
            };
            let description = body["description"].as_str().unwrap_or("code execution").to_string();

            let block = brain::computer_use::CodeBlock {
                language,
                code,
                description,
                auto_execute: true,
            };

            let mut brain = brain.lock().unwrap();
            match brain.code_execute(&block) {
                Ok(result) => {
                    respond_json(
                        request,
                        json!({
                            "stdout": result.stdout,
                            "stderr": result.stderr,
                            "exit_code": result.exit_code,
                            "duration_ms": result.duration_ms,
                            "files_created": result.files_created,
                            "timed_out": result.timed_out,
                        }),
                    );
                }
                Err(e) => respond_error(request, 500, &e),
            }
        }

        // ─── Code History ───────────────────────────────
        (Method::Get, "/api/code/history") => {
            let brain = brain.lock().unwrap();
            let history = brain.code_execution_history();
            let entries: Vec<Value> = history.iter().map(|r| {
                json!({
                    "id": r.id,
                    "language": r.code.language,
                    "description": r.code.description,
                    "exit_code": r.result.exit_code,
                    "duration_ms": r.result.duration_ms,
                    "executed_at": r.executed_at.to_rfc3339(),
                    "stdout_preview": if r.result.stdout.len() > 200 {
                        format!("{}...", &r.result.stdout[..200])
                    } else {
                        r.result.stdout.clone()
                    },
                })
            }).collect();
            respond_json(request, json!({"history": entries, "total": entries.len()}));
        }

        // ─── File Read (sandboxed) ──────────────────────
        (Method::Post, "/api/files/read") => {
            let body = match read_body(&mut request) {
                Some(b) => b,
                None => return respond_error(request, 400, "Invalid JSON"),
            };
            let filename = match body["filename"].as_str() {
                Some(f) if !f.is_empty() => f.to_string(),
                _ => return respond_error(request, 400, "Missing 'filename' field"),
            };

            let brain = brain.lock().unwrap();
            match brain.sandbox_file_read(&filename) {
                Ok(content) => {
                    respond_json(
                        request,
                        json!({
                            "filename": filename,
                            "content": content,
                            "size_bytes": content.len(),
                        }),
                    );
                }
                Err(e) => respond_error(request, 400, &e),
            }
        }

        // ─── File Write (sandboxed) ─────────────────────
        (Method::Post, "/api/files/write") => {
            let body = match read_body(&mut request) {
                Some(b) => b,
                None => return respond_error(request, 400, "Invalid JSON"),
            };
            let filename = match body["filename"].as_str() {
                Some(f) if !f.is_empty() => f.to_string(),
                _ => return respond_error(request, 400, "Missing 'filename' field"),
            };
            let content = match body["content"].as_str() {
                Some(c) => c.to_string(),
                _ => return respond_error(request, 400, "Missing 'content' field"),
            };

            let brain = brain.lock().unwrap();
            match brain.sandbox_file_write(&filename, &content) {
                Ok(path) => {
                    respond_json(
                        request,
                        json!({
                            "filename": filename,
                            "path": path,
                            "size_bytes": content.len(),
                        }),
                    );
                }
                Err(e) => respond_error(request, 400, &e),
            }
        }

        // ─── File List (sandboxed) ──────────────────────
        (Method::Get, "/api/files/list") => {
            let brain = brain.lock().unwrap();
            match brain.sandbox_file_list(None) {
                Ok(entries) => {
                    respond_json(
                        request,
                        json!({
                            "sandbox_dir": brain.sandbox_dir().to_string_lossy(),
                            "entries": entries.iter().map(|e| {
                                json!({
                                    "name": e.name,
                                    "is_dir": e.is_dir,
                                    "size_bytes": e.size_bytes,
                                })
                            }).collect::<Vec<_>>(),
                            "total": entries.len(),
                        }),
                    );
                }
                Err(e) => respond_error(request, 500, &e),
            }
        }

        // ─── Conversations: List ────────────────────────
        (Method::Get, "/api/conversations") => {
            let brain = brain.lock().unwrap();
            let sessions = brain.list_conversations();
            let list: Vec<Value> = sessions.iter().map(|s| {
                json!({
                    "id": s.id,
                    "name": s.name,
                    "message_count": s.message_count,
                    "created_at": s.created_at.to_rfc3339(),
                    "updated_at": s.updated_at.to_rfc3339(),
                })
            }).collect();
            respond_json(request, json!({ "conversations": list }));
        }

        // ─── Conversations: Create ──────────────────────
        (Method::Post, "/api/conversations") => {
            let body = read_body(&mut request);
            let name = body.as_ref().and_then(|b| b["name"].as_str());
            let mut brain = brain.lock().unwrap();
            let id = brain.new_conversation(name);
            respond_json(request, json!({ "id": id, "status": "created" }));
        }

        // ─── Conversations: Get / Rename / Delete by ID ─
        (_, ref path) if path.starts_with("/api/conversations/") => {
            let id = &path["/api/conversations/".len()..];
            let id = id.trim_end_matches('/').to_string();

            match method {
                Method::Get => {
                    let brain = brain.lock().unwrap();
                    match brain.get_conversation(&id) {
                        Some(session) => {
                            let msgs: Vec<Value> = session.messages.iter().map(|m| {
                                json!({
                                    "id": m.id,
                                    "role": m.role,
                                    "content": m.content,
                                    "images": m.images,
                                    "sources": m.sources.iter().map(|(hex, label, pct)| {
                                        json!({"seed": hex, "label": label, "contribution": pct})
                                    }).collect::<Vec<_>>(),
                                    "timestamp": m.timestamp.to_rfc3339(),
                                })
                            }).collect();
                            respond_json(request, json!({
                                "id": session.id,
                                "name": session.name,
                                "messages": msgs,
                                "created_at": session.created_at.to_rfc3339(),
                                "updated_at": session.updated_at.to_rfc3339(),
                            }));
                        }
                        None => respond_error(request, 404, "Conversation not found"),
                    }
                }
                Method::Put => {
                    let body = match read_body(&mut request) {
                        Some(b) => b,
                        None => return respond_error(request, 400, "Invalid JSON"),
                    };
                    let name = match body["name"].as_str() {
                        Some(n) => n,
                        None => return respond_error(request, 400, "Missing 'name' field"),
                    };
                    let mut brain = brain.lock().unwrap();
                    if brain.rename_conversation(&id, name) {
                        respond_json(request, json!({ "status": "renamed" }));
                    } else {
                        respond_error(request, 404, "Conversation not found");
                    }
                }
                Method::Delete => {
                    let mut brain = brain.lock().unwrap();
                    if brain.delete_conversation(&id) {
                        respond_json(request, json!({ "status": "deleted" }));
                    } else {
                        respond_error(request, 404, "Conversation not found");
                    }
                }
                _ => respond_error(request, 405, "Method not allowed"),
            }
        }

        // ─── 404 ────────────────────────────────────────
        _ => {
            let response = Response::from_string("Not Found").with_status_code(404);
            let _ = request.respond(response);
        }
    }
}
