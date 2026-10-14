// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! MCP server v2 — ~12 tools exposed via stdio JSON-RPC 2.0.
//!
//! The v2 server is a clean redesign of the v1 server.  It exposes 13 typed
//! tools and 5 event subscription channels.  All tool dispatch goes through
//! a bounded worker queue so the stdin loop never blocks on slow operations.
//!
//! Tool → channel mapping:
//!   Conversation, Think                    → Query channel
//!   Teach, TeachFile, Approve, Reject      → Learn channel
//!   Revoke                                 → Safety channel
//!   Status, Observe, Session               → Observe channel
//!   Configure, Save                        → Admin channel

use brain::{
    hypothalamus::AutonomyTier,
    proposals::ThinkingProposal,
    Brain,
};
use df_core::Seed;
use graph::{bond::BondType, node::Confidence};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

// ── McpTool enum ──────────────────────────────────────────────────────────────

/// All v2 MCP tools.  Each variant corresponds to one tool in the MCP
/// `tools/list` response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpTool {
    /// Send a message to Lu and receive a structured response.
    Conversation,
    /// Teach Lu a typed piece of knowledge (with structured KU fields).
    Teach,
    /// Teach Lu from a file path (text, PDF, Markdown, or source code).
    TeachFile,
    /// Approve a pending knowledge proposal by ID.
    Approve,
    /// Reject a pending knowledge proposal by ID.
    Reject,
    /// Revoke existing knowledge by seed hex (creates an amygdala scar).
    Revoke,
    /// Configure brain parameters: autonomy tier, budget, storage path.
    Configure,
    /// Get brain health status, stats, and pending proposals.
    Status,
    /// Observe the brain's internal activity without interacting.
    Observe,
    /// Subscribe to a named event channel for streaming updates.
    Subscribe,
    /// Ask Lu to think deeply about a topic (multi-step reasoning).
    Think,
    /// Trigger an immediate save of the brain state to disk.
    Save,
    /// Manage conversation sessions: new, list, switch, summarise.
    Session,
    /// Run a full web research session and ingest results into the brain.
    WebResearch,
    /// Teach Lu an image directly from a file path — no proposal step.
    TeachImage,
    /// Report recall relevance quality metrics for recent queries (Phase 1).
    RecallQuality,
    /// Show the bond-type traversal chains that most often lead to high-quality recalls (Phase 2).
    HotPaths,
    /// Show the query patterns with the lowest EMA recall quality (Phase 2).
    WeakPatterns,
    /// Demote nodes that consistently appear in low-quality recalls (Phase 2).
    ApplyDemotions,
    /// Show current RecallParameters and the recent tuning adjustment history.
    TuneStatus,
}

impl McpTool {
    pub fn name(&self) -> &'static str {
        match self {
            McpTool::Conversation => "lu_conversation",
            McpTool::Teach        => "lu_teach",
            McpTool::TeachFile    => "lu_teach_file",
            McpTool::Approve      => "lu_approve",
            McpTool::Reject       => "lu_reject",
            McpTool::Revoke       => "lu_revoke",
            McpTool::Configure    => "lu_configure",
            McpTool::Status       => "lu_status",
            McpTool::Observe      => "lu_observe",
            McpTool::Subscribe    => "lu_subscribe",
            McpTool::Think        => "lu_think",
            McpTool::Save         => "lu_save",
            McpTool::Session      => "lu_session",
            McpTool::WebResearch    => "lu_web_research",
            McpTool::TeachImage     => "lu_teach_image",
            McpTool::RecallQuality  => "lu_recall_quality",
            McpTool::HotPaths       => "lu_hot_paths",
            McpTool::WeakPatterns   => "lu_weak_patterns",
            McpTool::ApplyDemotions => "lu_apply_demotions",
            McpTool::TuneStatus     => "lu_tune_status",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            McpTool::Conversation =>
                "Send a message to Lu and receive a response drawn from her knowledge graph.",
            McpTool::Teach =>
                "Teach Lu a new typed piece of knowledge with explicit semantic structure.",
            McpTool::TeachFile =>
                "Ingest a file into Lu's knowledge graph (text, PDF, Markdown, or code).",
            McpTool::Approve =>
                "Approve a pending knowledge proposal so it enters the long-term graph.",
            McpTool::Reject =>
                "Reject a pending proposal — it will not be learned.",
            McpTool::Revoke =>
                "Revoke a piece of knowledge by seed hex. Creates a permanent scar.",
            McpTool::Configure =>
                "Update brain configuration: autonomy tier, resource budgets, storage path.",
            McpTool::Status =>
                "Return brain health, node/bond counts, pending proposals, and scar list.",
            McpTool::Observe =>
                "Read recent brain activity without triggering a response.",
            McpTool::Subscribe =>
                "Subscribe to a brain event channel (query, learn, safety, observe, admin).",
            McpTool::Think =>
                "Ask Lu to reason step-by-step over a topic and return a structured report.",
            McpTool::Save =>
                "Flush the brain state to disk immediately.",
            McpTool::Session =>
                "Manage conversation sessions: list, new, switch, export, summarise.",
            McpTool::WebResearch =>
                "Run a web research session for a query (Grokopedia → DDG Lite → Wikipedia). Fetches pages, extracts text, and ingests into Lu's knowledge graph.",
            McpTool::TeachImage =>
                "Teach Lu an image directly from a file path. Encodes it via the occipital lobe into a 384-dim embedding, stores it as an Image node, and creates Depicts bonds to similar text nodes for cross-modal recall. Accepts PNG, JPEG, GIF, WebP.",
            McpTool::RecallQuality =>
                "Report recall relevance quality metrics: average relevance score, worst/best queries, and total queries logged.",
            McpTool::HotPaths =>
                "Show the bond-type traversal chains that most often lead to high-quality recalls, sorted by quality × frequency.",
            McpTool::WeakPatterns =>
                "Show query patterns with the lowest EMA recall quality. Identifies topics Lu handles poorly.",
            McpTool::ApplyDemotions =>
                "Demote nodes that consistently appear in low-quality recalls from Unverified to Contested confidence.",
            McpTool::TuneStatus =>
                "Show the current self-tuning RecallParameters (top_k, activation_threshold, bond_traversal_depth, type_weighting) and the recent P-controller adjustment history.",
        }
    }

    /// The event channel this tool primarily routes through.
    pub fn channel(&self) -> McpChannel {
        match self {
            McpTool::TuneStatus                         => McpChannel::Observe,
            McpTool::Conversation | McpTool::Think      => McpChannel::Query,
            McpTool::Teach | McpTool::TeachFile
            | McpTool::TeachImage
            | McpTool::Approve | McpTool::Reject        => McpChannel::Learn,
            McpTool::Revoke                             => McpChannel::Safety,
            McpTool::Status | McpTool::Observe
            | McpTool::Session                          => McpChannel::Observe,
            McpTool::Configure | McpTool::Save          => McpChannel::Admin,
            McpTool::Subscribe                          => McpChannel::Observe,
            McpTool::WebResearch                        => McpChannel::Learn,
            McpTool::RecallQuality | McpTool::HotPaths
            | McpTool::WeakPatterns                     => McpChannel::Observe,
            McpTool::ApplyDemotions                     => McpChannel::Admin,
        }
    }

    pub fn input_schema(&self) -> Value {
        match self {
            McpTool::Conversation => json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string", "description": "The message to send to Lu." },
                    "session_id": { "type": "string", "description": "Optional session ID." }
                },
                "required": ["message"]
            }),
            McpTool::Teach => json!({
                "type": "object",
                "properties": {
                    "ku_type": {
                        "type": "string",
                        "enum": ["Definition","Mechanism","Composition","Purpose",
                                 "Contrast","Causal","Example","Procedure","Code","Raw"],
                        "description": "The knowledge unit type."
                    },
                    "content": { "type": "string", "description": "The raw source text." },
                    "confidence": {
                        "type": "string",
                        "enum": ["Terminal","Verified","Unverified","Inferred"],
                        "default": "Terminal"
                    }
                },
                "required": ["ku_type", "content"]
            }),
            McpTool::TeachFile => json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute path to the file." }
                },
                "required": ["path"]
            }),
            McpTool::TeachImage => json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Absolute path to the image file (PNG, JPEG, GIF, or WebP)."
                    },
                    "caption": {
                        "type": "string",
                        "description": "Optional description of the image. Aids cross-modal recall."
                    }
                },
                "required": ["path"]
            }),
            McpTool::Approve | McpTool::Reject => json!({
                "type": "object",
                "properties": {
                    "proposal_id": { "type": "string" }
                },
                "required": ["proposal_id"]
            }),
            McpTool::Revoke => json!({
                "type": "object",
                "properties": {
                    "seed_hex": { "type": "string", "description": "64-char hex seed." },
                    "reason": { "type": "string" }
                },
                "required": ["seed_hex", "reason"]
            }),
            McpTool::Configure => json!({
                "type": "object",
                "properties": {
                    "autonomy_tier": {
                        "type": "string",
                        "enum": ["Supervised","Assisted","Collaborative","Autonomous"]
                    },
                    "storage_path": { "type": "string" },
                    "max_memory_mb": { "type": "integer" },
                    "feature_flag": {
                        "type": "string",
                        "description": "Name of the feature flag to set (e.g. 'web_research', 'rumination', 'arena_mode')."
                    },
                    "value": {
                        "type": "boolean",
                        "description": "Value to assign to feature_flag (true = enabled, false = disabled)."
                    }
                }
            }),
            McpTool::Think => json!({
                "type": "object",
                "properties": {
                    "topic": { "type": "string" },
                    "depth": { "type": "integer", "default": 3 }
                },
                "required": ["topic"]
            }),
            McpTool::Session => json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "enum": ["new","list","switch","export","summarise"]
                    },
                    "session_id": { "type": "string" }
                },
                "required": ["action"]
            }),
            McpTool::WebResearch => json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "The topic or question to research." }
                },
                "required": ["query"]
            }),
            McpTool::RecallQuality => json!({
                "type": "object",
                "properties": {
                    "last_n": {
                        "type": "integer",
                        "description": "Number of recent queries to include in the window average (default 20).",
                        "default": 20
                    }
                }
            }),
            McpTool::HotPaths => json!({
                "type": "object",
                "properties": {
                    "top_n": {
                        "type": "integer",
                        "description": "Number of top paths to return (default 10).",
                        "default": 10
                    }
                }
            }),
            McpTool::WeakPatterns => json!({
                "type": "object",
                "properties": {
                    "min_hits": {
                        "type": "integer",
                        "description": "Minimum number of times a pattern must have been seen (default 3).",
                        "default": 3
                    },
                    "top_n": {
                        "type": "integer",
                        "description": "Number of worst patterns to return (default 10).",
                        "default": 10
                    }
                }
            }),
            // Status, Observe, Subscribe, Save, ApplyDemotions take no required params
            _ => json!({ "type": "object", "properties": {} }),
        }
    }

    pub fn all() -> &'static [McpTool] {
        ALL_TOOLS
    }
}

static ALL_TOOLS: &[McpTool] = &[
    McpTool::Conversation,
    McpTool::Teach,
    McpTool::TeachFile,
    McpTool::TeachImage,
    McpTool::Approve,
    McpTool::Reject,
    McpTool::Revoke,
    McpTool::Configure,
    McpTool::Status,
    McpTool::Observe,
    McpTool::Subscribe,
    McpTool::Think,
    McpTool::Save,
    McpTool::Session,
    McpTool::WebResearch,
    McpTool::RecallQuality,
    McpTool::HotPaths,
    McpTool::WeakPatterns,
    McpTool::ApplyDemotions,
    McpTool::TuneStatus,
];

// ── McpChannel ────────────────────────────────────────────────────────────────

/// The 5 event channels.  Each channel can be independently subscribed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum McpChannel {
    /// Conversation and thinking results
    Query,
    /// Knowledge learning events (proposals, approvals, rejections)
    Learn,
    /// Revocations and scar alerts
    Safety,
    /// Passive observation and session events
    Observe,
    /// Configuration and administrative operations
    Admin,
}

impl McpChannel {
    pub fn name(&self) -> &'static str {
        match self {
            McpChannel::Query   => "query",
            McpChannel::Learn   => "learn",
            McpChannel::Safety  => "safety",
            McpChannel::Observe => "observe",
            McpChannel::Admin   => "admin",
        }
    }

    pub fn all() -> &'static [McpChannel] {
        &[
            McpChannel::Query,
            McpChannel::Learn,
            McpChannel::Safety,
            McpChannel::Observe,
            McpChannel::Admin,
        ]
    }
}

// ── JSON-RPC helpers ──────────────────────────────────────────────────────────

pub fn success_response(id: Value, result: Value) -> String {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    }))
    .unwrap_or_default()
}

pub fn error_response(id: Value, code: i32, message: &str) -> String {
    serde_json::to_string(&json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message }
    }))
    .unwrap_or_default()
}

pub fn tool_result(id: &Value, text: &str) -> String {
    success_response(
        id.clone(),
        json!({ "content": [{ "type": "text", "text": text }] }),
    )
}

pub fn tool_error(id: &Value, text: &str) -> String {
    success_response(
        id.clone(),
        json!({
            "content": [{ "type": "text", "text": text }],
            "isError": true
        }),
    )
}

pub fn handle_initialize(id: &Value) -> String {
    success_response(
        id.clone(),
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {}
            },
            "serverInfo": {
                "name": "lu-brain-v2",
                "version": "2.0.0"
            }
        }),
    )
}

pub fn handle_tools_list(id: &Value) -> String {
    let tools: Vec<Value> = McpTool::all()
        .iter()
        .map(|t| {
            json!({
                "name": t.name(),
                "description": t.description(),
                "inputSchema": t.input_schema()
            })
        })
        .collect();

    success_response(id.clone(), json!({ "tools": tools }))
}

// ── McpServer ─────────────────────────────────────────────────────────────────

/// Live MCP server — owns a `Brain` and dispatches tool calls against it.
pub struct McpServer {
    pub brain: Brain,
    pub brain_path: Option<std::path::PathBuf>,
    pub semantic_mem: Vec<(String, Vec<f32>)>,
}

impl McpServer {
    /// Create a new server.  If `brain_path` is provided and the file exists
    /// the brain is loaded from disk; otherwise a fresh brain is created.
    pub fn new(brain_path: Option<std::path::PathBuf>) -> Self {
        let mut brain = if let Some(ref p) = brain_path {
            if p.exists() {
                Brain::load(p).unwrap_or_else(|_| Brain::new())
            } else {
                Brain::new()
            }
        } else {
            Brain::new()
        };

        if let Some(ref p) = brain_path {
            brain.set_storage(p.clone());
        }

        McpServer { brain, brain_path, semantic_mem: Vec::new() }
    }

    /// Route a parsed JSON-RPC request to the appropriate handler.
    /// Returns the serialised JSON-RPC response string.
    pub fn route(&mut self, request: &Value) -> String {
        let id     = &request["id"];
        let method = request["method"].as_str().unwrap_or("");
        let params = &request["params"];

        match method {
            "initialize"  => handle_initialize(id),
            "tools/list"  => handle_tools_list(id),
            "tools/call"  => self.handle_tool_call(id, params),
            _             => error_response(id.clone(), -32601, "Method not found"),
        }
    }

    // ── Autonomy gating ───────────────────────────────────────────────────────

    /// Returns `Some(error_response)` if the current tier is below `required`.
    fn check_tier(&self, required: AutonomyTier, id: &Value) -> Option<String> {
        if self.brain.hypothalamus.autonomy_tier < required {
            Some(tool_error(id, &format!(
                "Operation requires '{}' tier or higher (current: '{}'). \
                 Use lu_configure to elevate the tier.",
                required.label(),
                self.brain.hypothalamus.autonomy_tier.label()
            )))
        } else {
            None
        }
    }

    // ── Tool dispatch ─────────────────────────────────────────────────────────

    pub fn handle_tool_call(&mut self, id: &Value, params: &Value) -> String {
        let tool_name = params["name"].as_str().unwrap_or("");
        let args      = &params["arguments"];

        match tool_name {
            // ── Conversation (no tier gating — querying is always allowed) ────
            "lu_conversation" => {
                let message = match args["message"].as_str() {
                    Some(m) if !m.is_empty() => m,
                    _ => return tool_error(id, "message parameter is required"),
                };
                let response = self.brain.respond(message);
                tool_result(id, &response)
            }

            // ── Teach ─────────────────────────────────────────────────────────
            "lu_teach" => {
                if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                    return err;
                }
                let content = match args["content"].as_str() {
                    Some(c) if !c.is_empty() => c,
                    _ => return tool_error(id, "content parameter is required"),
                };
                let confidence_str = args["confidence"].as_str().unwrap_or("Terminal");
                let confidence = match confidence_str {
                    "Verified"   => Confidence::Verified,
                    "Unverified" => Confidence::Unverified,
                    "Inferred"   => Confidence::Inferred,
                    _            => Confidence::Terminal,
                };
                let seeds = self.brain.learn_text(content, confidence);
                tool_result(id, &format!(
                    "Learned {} knowledge unit(s). Seeds: {}",
                    seeds.len(),
                    seeds.iter().map(|s| s.short()).collect::<Vec<_>>().join(", ")
                ))
            }

            // ── TeachImage ────────────────────────────────────────────────────
            "lu_teach_image" => {
                if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                    return err;
                }
                let path = match args["path"].as_str() {
                    Some(p) if !p.is_empty() => p,
                    _ => return tool_error(id, "path parameter is required"),
                };
                let caption = args["caption"].as_str();
                match self.brain.learn_image(path, caption) {
                    Ok(seed) => {
                        let msg = match caption {
                            Some(cap) => format!(
                                "Image learned. Seed: {}\nPath: {}\nCaption: {}",
                                seed.to_hex(), path, cap
                            ),
                            None => format!(
                                "Image learned. Seed: {}\nPath: {}",
                                seed.to_hex(), path
                            ),
                        };
                        tool_result(id, &msg)
                    }
                    Err(e) => tool_error(id, &format!("Failed to learn image '{}': {}", path, e)),
                }
            }

            // ── TeachFile ─────────────────────────────────────────────────────
            "lu_teach_file" => {
                if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                    return err;
                }
                let path = match args["path"].as_str() {
                    Some(p) if !p.is_empty() => p,
                    _ => return tool_error(id, "path parameter is required"),
                };
                match std::fs::read_to_string(path) {
                    Ok(content) => {
                        let seeds = self.brain.learn_text(&content, Confidence::Terminal);
                        tool_result(id, &format!(
                            "Ingested {} KU(s) from '{}'",
                            seeds.len(), path
                        ))
                    }
                    Err(e) => tool_error(id, &format!(
                        "Failed to read '{}': {}", path, e
                    )),
                }
            }

            // ── Approve ───────────────────────────────────────────────────────
            "lu_approve" => {
                if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                    return err;
                }
                let proposal_id = match args["proposal_id"].as_str() {
                    Some(p) if !p.is_empty() => p.to_string(),
                    _ => return tool_error(id, "proposal_id parameter is required"),
                };
                match self.brain.proposals.approve(&proposal_id) {
                    None => tool_error(id, &format!(
                        "No pending proposal with id '{}'", proposal_id
                    )),
                    Some(pending) => {
                        let outcome = apply_proposal(&mut self.brain, &pending.proposal);
                        tool_result(id, &format!(
                            "Proposal '{}' approved and applied: {}",
                            &proposal_id[..8.min(proposal_id.len())], outcome
                        ))
                    }
                }
            }

            // ── Reject ────────────────────────────────────────────────────────
            "lu_reject" => {
                if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                    return err;
                }
                let proposal_id = match args["proposal_id"].as_str() {
                    Some(p) if !p.is_empty() => p.to_string(),
                    _ => return tool_error(id, "proposal_id parameter is required"),
                };
                if self.brain.proposals.reject(&proposal_id) {
                    tool_result(id, &format!(
                        "Proposal '{}' rejected and discarded.",
                        &proposal_id[..8.min(proposal_id.len())]
                    ))
                } else {
                    tool_error(id, &format!(
                        "No pending proposal with id '{}'", proposal_id
                    ))
                }
            }

            // ── Revoke ────────────────────────────────────────────────────────
            "lu_revoke" => {
                if let Some(err) = self.check_tier(AutonomyTier::Collaborative, id) {
                    return err;
                }
                let seed_hex = match args["seed_hex"].as_str() {
                    Some(h) if !h.is_empty() => h,
                    _ => return tool_error(id, "seed_hex parameter is required"),
                };
                let reason = args["reason"].as_str().unwrap_or("user-revoked");
                match Seed::from_hex(seed_hex) {
                    Ok(seed) => {
                        self.brain.revoke(&seed, reason);
                        tool_result(id, &format!(
                            "Node {} revoked. Reason: {}",
                            &seed_hex[..16], reason
                        ))
                    }
                    Err(e) => tool_error(id, &format!(
                        "Invalid seed hex '{}': {}", seed_hex, e
                    )),
                }
            }

            // ── Configure ─────────────────────────────────────────────────────
            "lu_configure" => {
                let mut changes: Vec<String> = Vec::new();

                if let Some(tier_str) = args["autonomy_tier"].as_str() {
                    let tier = match tier_str {
                        "Supervised"    | "supervised"    => Some(AutonomyTier::Supervised),
                        "Assisted"      | "assisted"      => Some(AutonomyTier::Assisted),
                        "Collaborative" | "collaborative" => Some(AutonomyTier::Collaborative),
                        "Autonomous"    | "autonomous"    => Some(AutonomyTier::Autonomous),
                        other => {
                            return tool_error(id, &format!(
                                "Unknown autonomy tier '{}'. \
                                 Valid: Supervised, Assisted, Collaborative, Autonomous", other
                            ));
                        }
                    };
                    if let Some(t) = tier {
                        self.brain.hypothalamus.set_autonomy_tier(t);
                        changes.push(format!("tier → {}", t.label()));
                    }
                }

                if let Some(path_str) = args["storage_path"].as_str() {
                    let p = std::path::PathBuf::from(path_str);
                    self.brain.set_storage(p.clone());
                    self.brain_path = Some(p);
                    changes.push(format!("storage → {}", path_str));
                }

                if let Some(flag_name) = args["feature_flag"].as_str() {
                    let value = args["value"].as_bool().unwrap_or(true);
                    match self.brain.flags.set(flag_name, value) {
                        Ok(()) => changes.push(format!("{} → {}", flag_name, value)),
                        Err(e) => return tool_error(id, &e),
                    }
                }

                if changes.is_empty() {
                    tool_result(id, &format!(
                        "No changes. Current tier: {}",
                        self.brain.hypothalamus.autonomy_tier.label()
                    ))
                } else {
                    tool_result(id, &format!("Configured: {}", changes.join("; ")))
                }
            }

            // ── Status ────────────────────────────────────────────────────────
"lu_scar_action" => {
                if let Some(err) = self.check_tier(AutonomyTier::Collaborative, id) {
                    return err;
                }
                let text = match args["text"].as_str() {
                    Some(t) if !t.is_empty() => t,
                    _ => return tool_error(id, "text parameter is required"),
                };
                let reason = args["reason"].as_str().unwrap_or("CattleDog semantic policy scar");
                self.brain.semantic_scar_action(text, reason);
                tool_result(id, &format!("Semantic scar implanted over action: {}", text))
            }
            "lu_scar_check" => {
                let text = match args["text"].as_str() {
                    Some(t) if !t.is_empty() => t,
                    _ => return tool_error(id, "text parameter is required"),
                };
                let threshold = args["threshold"].as_f64().map(|x| x as f32);
                let hits = self.brain.semantic_scar_check(text, threshold);
                let payload = json!({
                    "triggered": !hits.is_empty(),
                    "threshold": threshold,
                    "scars": hits,
                });
                tool_result(id, &payload.to_string())
            }
"lu_reachable" => {
                // CattleDog Phase 3: genuine GRAPH REACHABILITY probe. Revoked/inactive
                // nodes are excluded by keyword_search (SeedGated::is_active), so a
                // severed capability becomes unreachable here. Structural signal the
                // gate decides on (not a name-set).
                let query = args["query"].as_str().unwrap_or("");
                let seed_hex = args["seed_hex"].as_str().unwrap_or("");
                let top_k = args["top_k"].as_u64().unwrap_or(10) as usize;
                let rr = self.brain.recall(query, 2, top_k);
                let cands: Vec<serde_json::Value> = rr.activated_nodes.iter()
                    .map(|n| json!({"seed": n.seed.to_hex(), "label": n.label})).collect();
                let reachable = if seed_hex.is_empty() {
                    !rr.activated_nodes.is_empty()
                } else {
                    rr.activated_nodes.iter().any(|n| {
                        let h = n.seed.to_hex();
                        h.starts_with(seed_hex) || seed_hex.starts_with(&h[..h.len().min(16)])
                    })
                };
                tool_result(id, &json!({"query":query,"seed_hex":seed_hex,
                    "reachable":reachable,"candidates":cands}).to_string())
            }
            "lu_remember" => {
                let text = match args["text"].as_str() {
                    Some(t) if !t.is_empty() => t,
                    _ => return tool_error(id, "text parameter is required"),
                };
                let emb = embedding::semantic_embed(text);
                self.semantic_mem.push((text.to_string(), emb));
                tool_result(id, &format!("Remembered ({} facts in semantic memory).", self.semantic_mem.len()))
            }
            "lu_semantic_recall" => {
                let query = match args["query"].as_str() {
                    Some(t) if !t.is_empty() => t,
                    _ => return tool_error(id, "query parameter is required"),
                };
                let top_k = args["top_k"].as_u64().unwrap_or(3) as usize;
                let threshold = args["threshold"].as_f64().unwrap_or(0.0) as f32;
                let qe = embedding::semantic_embed(query);
                let mut scored: Vec<(f32, String)> = self.semantic_mem.iter()
                    .map(|(t, e)| (embedding::cosine_similarity(&qe, e), t.clone()))
                    .filter(|(s, _)| *s >= threshold)
                    .collect();
                scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
                let hits: Vec<serde_json::Value> = scored.into_iter().take(top_k)
                    .map(|(s, t)| json!({"text": t, "similarity": s})).collect();
                tool_result(id, &json!({"query": query, "hits": hits}).to_string())
            }
            "lu_status" => {
                let stats  = self.brain.stats();
                let health = self.brain.health();
                let style  = &self.brain.hippocampus.style;
                let ep_count = self.brain.hippocampus.episodes.len();

                let pending_proposals = self.brain.proposals.pending_count();
                let si = &self.brain.self_improvement.metrics;
                let report = json!({
                    "nodes":         { "total": stats.total_nodes, "active": stats.active_nodes },
                    "bonds":         { "total": stats.total_bonds, "active": stats.active_bonds },
                    "episodes":      ep_count,
                    "scars":         health.scar_count,
                    "autonomy_tier": health.autonomy_tier.label(),
                    "style_vector":  {
                        "formality":    style.formality,
                        "verbosity":    style.verbosity,
                        "technicality": style.technicality,
                        "empathy":      style.empathy,
                        "samples":      style.sample_count
                    },
                    "pending_proposals": pending_proposals,
                    "performance": {
                        "recall_hit_rate": si.recall_hit_rate,
                        "avg_response_quality": si.avg_response_quality,
                        "consolidation_effectiveness": si.consolidation_effectiveness,
                        "total_queries": si.total_queries,
                    },
                    "warnings": health.warnings,
                });
                tool_result(id, &serde_json::to_string_pretty(&report).unwrap_or_default())
            }

            // ── Observe ───────────────────────────────────────────────────────
            "lu_observe" => {
                let stats      = self.brain.stats();
                let ep_count   = self.brain.hippocampus.episodes.len();
                let pending    = self.brain.hippocampus.pending_count();
                let wonder_pending = self.brain.curiosity.wonder_queue.pending_count();
                let wonder_items: Vec<serde_json::Value> = self.brain.curiosity.wonder_queue
                    .pending()
                    .into_iter()
                    .take(5)
                    .map(|w| json!({
                        "question": w.question,
                        "priority": w.priority,
                        "source":   format!("{:?}", w.source),
                    }))
                    .collect();
                let curiosity_status = if wonder_pending > 0 {
                    "active"
                } else if self.brain.curiosity.analysis_count > 0 {
                    "analyzed_empty"
                } else {
                    "idle"
                };
                let si = &self.brain.self_improvement.metrics;

                let obs = json!({
                    "active_nodes":          stats.active_nodes,
                    "active_bonds":          stats.active_bonds,
                    "episodes_total":        ep_count,
                    "pending_consolidation": pending,
                    "wonder_queue":          wonder_items,
                    "wonder_pending":        wonder_pending,
                    "curiosity_status":      curiosity_status,
                    "analysis_count":        self.brain.curiosity.analysis_count,
                    "total_queries":         si.total_queries,
                    "recall_hit_rate":       si.recall_hit_rate,
                });
                tool_result(id, &serde_json::to_string_pretty(&obs).unwrap_or_default())
            }

            // ── Subscribe — polling-based brain event stream ───────────────────
            "lu_subscribe" => {
                let channel = args["channel"].as_str().unwrap_or("all");
                let stats  = self.brain.stats();
                let health = self.brain.health();
                let ep_count = self.brain.hippocampus.episodes.len();
                let pending_proposals = self.brain.proposals.pending_count();
                let wonder_pending = self.brain.curiosity.wonder_queue.pending_count();

                let events = json!([
                    {
                        "event": "status_snapshot",
                        "channel": channel,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "data": {
                            "active_nodes":   stats.active_nodes,
                            "active_bonds":   stats.active_bonds,
                            "episodes":       ep_count,
                            "pending_proposals": pending_proposals,
                            "wonder_queue":   wonder_pending,
                            "scars":          health.scar_count,
                            "autonomy_tier":  health.autonomy_tier.label(),
                        }
                    },
                    {
                        "event": "proposals_snapshot",
                        "channel": channel,
                        "timestamp": chrono::Utc::now().to_rfc3339(),
                        "data": self.brain.proposals.pending().iter().map(|p| json!({
                            "id": p.id,
                            "kind": p.proposal.kind(),
                            "summary": p.proposal.summary(),
                            "created_at": p.created_at.to_rfc3339(),
                        })).collect::<Vec<_>>()
                    }
                ]);
                tool_result(id, &serde_json::to_string_pretty(&events).unwrap_or_default())
            }

            // ── Think ─────────────────────────────────────────────────────────
            "lu_think" => {
                if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                    return err;
                }
                let topic = match args["topic"].as_str() {
                    Some(t) if !t.is_empty() => t.to_string(),
                    _ => return tool_error(id, "topic parameter is required"),
                };
                let answer = self.brain.think_through(&topic);
                tool_result(id, &answer)
            }

            // ── Save ──────────────────────────────────────────────────────────
            "lu_save" => {
                if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                    return err;
                }
                match self.brain.save() {
                    Ok(()) => {
                        let stats = self.brain.stats();
                        tool_result(id, &format!(
                            "Brain saved. ({} nodes, {} bonds)",
                            stats.total_nodes, stats.total_bonds
                        ))
                    }
                    Err(e) => tool_error(id, &format!("Save failed: {}", e)),
                }
            }

            // ── Session ───────────────────────────────────────────────────────
            "lu_session" => {
                let action = args["action"].as_str().unwrap_or("");
                match action {
                    // ── Hippocampus thread management (existing) ──────────────
                    "new" => {
                        if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                            return err;
                        }
                        let session_id = format!("s-{}", chrono::Utc::now().timestamp());
                        self.brain.hippocampus.start_new_thread(session_id.clone());
                        tool_result(id, &format!("New session started: {}", session_id))
                    }
                    "list" => {
                        let archived = self.brain.hippocampus.threads.len();
                        let active = self.brain.hippocampus
                            .current_thread.as_ref().map(|t| t.id.as_str())
                            .unwrap_or("none");
                        tool_result(id, &format!(
                            "Active session: '{}'. Archived: {}.",
                            active, archived
                        ))
                    }
                    "summarise" | "summarize" => {
                        let turn_count = self.brain.hippocampus.current_thread
                            .as_ref().map(|t| t.turns.len()).unwrap_or(0);
                        tool_result(id, &format!(
                            "Current session has {} turn(s). Style samples: {}.",
                            turn_count, self.brain.hippocampus.style.sample_count
                        ))
                    }

                    // ── Read: beliefs ─────────────────────────────────────────
                    "beliefs" => {
                        let active = self.brain.session.active_beliefs();
                        if active.is_empty() {
                            return tool_result(id, "No active beliefs.");
                        }
                        let lines: Vec<String> = active.iter().map(|b| {
                            format!(
                                "[{:.0}%] {} ({}s remaining)",
                                b.confidence * 100.0,
                                b.statement,
                                b.remaining_secs(),
                            )
                        }).collect();
                        tool_result(id, &lines.join("\n"))
                    }

                    // ── Read: learnings ───────────────────────────────────────
                    "learnings" => {
                        let count = self.brain.session.learning_count();
                        if count == 0 {
                            return tool_result(id, "No learnings recorded this session.");
                        }
                        let lines: Vec<String> = self.brain.session.learnings.iter().map(|l| {
                            format!("• {}", l.insight)
                        }).collect();
                        tool_result(id, &lines.join("\n"))
                    }

                    // ── Read: tasks ───────────────────────────────────────────
                    "tasks" => {
                        let count = self.brain.session.task_count();
                        if count == 0 {
                            return tool_result(id, "No tasks recorded this session.");
                        }
                        let lines: Vec<String> = self.brain.session.tasks.iter().map(|t| {
                            format!("[{}] {} (attempts: {})", t.status.label(), t.description, t.attempts.len())
                        }).collect();
                        tool_result(id, &lines.join("\n"))
                    }

                    // ── Write: add_belief ─────────────────────────────────────
                    "add_belief" => {
                        if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                            return err;
                        }
                        let statement = match args["statement"].as_str() {
                            Some(s) if !s.is_empty() => s.to_string(),
                            _ => return tool_error(id, "statement parameter is required"),
                        };
                        let confidence = args["confidence"].as_f64().unwrap_or(0.7) as f32;
                        let ttl = args["ttl_seconds"].as_u64().unwrap_or(600);
                        let belief_id = self.brain.session.add_belief(&statement, confidence, ttl);
                        tool_result(id, &format!("Belief added: {} (id={})", statement, belief_id))
                    }

                    // ── Write: add_learning ───────────────────────────────────
                    "add_learning" => {
                        if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                            return err;
                        }
                        let insight = match args["insight"].as_str() {
                            Some(s) if !s.is_empty() => s.to_string(),
                            _ => return tool_error(id, "insight parameter is required"),
                        };
                        let source_task_id = args["source_task_id"].as_str().map(|s| s.to_string());
                        let learning_id = self.brain.session.add_learning(&insight, source_task_id);
                        tool_result(id, &format!("Learning recorded: {} (id={})", insight, learning_id))
                    }

                    // ── Write: add_task ───────────────────────────────────────
                    "add_task" => {
                        if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                            return err;
                        }
                        let description = match args["description"].as_str() {
                            Some(s) if !s.is_empty() => s.to_string(),
                            _ => return tool_error(id, "description parameter is required"),
                        };
                        let task_id = self.brain.session.add_task(&description);
                        tool_result(id, &format!("Task created: {} (id={})", description, task_id))
                    }

                    other => tool_error(id, &format!(
                        "Unknown session action '{}'. Valid: new, list, summarise, beliefs, \
                         learnings, tasks, add_belief, add_learning, add_task.", other
                    )),
                }
            }

            // ── WebResearch ───────────────────────────────────────────────────
            "lu_web_research" => {
                if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                    return err;
                }
                if !self.brain.flags.web_research {
                    return tool_error(id, "Web research is disabled. Enable it via lu_configure.");
                }
                let query = match args["query"].as_str() {
                    Some(q) if !q.is_empty() => q.to_string(),
                    _ => return tool_error(id, "query parameter is required"),
                };
                let report = self.brain.web_research(&query);
                let summary = json!({
                    "query":           report.query,
                    "pages_visited":   report.pages_visited,
                    "facts_extracted": report.facts_extracted,
                    "elapsed_secs":    report.elapsed_secs,
                    "sources":         report.sources,
                    "summary":         report.summary,
                });
                tool_result(id, &serde_json::to_string_pretty(&summary).unwrap_or_default())
            }

            // ── RecallQuality (Phase 1) ───────────────────────────────────────
            "lu_recall_quality" => {
                let last_n = args["last_n"].as_u64().unwrap_or(20) as usize;
                let log = &self.brain.self_improvement.quality_log;
                let avg_last_n = log.recent_avg(last_n);
                let avg_all_time = log.recent_avg(log.records.len());
                let total = log.records.len();

                let worst3: Vec<_> = log.worst_n(3).into_iter().map(|r| json!({
                    "query":       r.query_preview,
                    "relevance":   r.relevance_score,
                    "node_count":  r.node_count,
                    "recorded_at": r.recorded_at,
                })).collect();

                let best3: Vec<_> = log.best_n(3).into_iter().map(|r| json!({
                    "query":       r.query_preview,
                    "relevance":   r.relevance_score,
                    "node_count":  r.node_count,
                    "recorded_at": r.recorded_at,
                })).collect();

                let report = json!({
                    "avg_relevance_last_n":   avg_last_n,
                    "last_n_window":          last_n,
                    "avg_relevance_all_time": avg_all_time,
                    "total_queries_logged":   total,
                    "worst_3_queries":        worst3,
                    "best_3_queries":         best3,
                });
                tool_result(id, &serde_json::to_string_pretty(&report).unwrap_or_default())
            }

            // ── HotPaths (Phase 2) ────────────────────────────────────────────
            "lu_hot_paths" => {
                let top_n = args["top_n"].as_u64().unwrap_or(10) as usize;
                let paths: Vec<_> = self.brain.self_improvement.hot_path_index
                    .top_n(top_n)
                    .into_iter()
                    .map(|p| {
                        let chain: Vec<String> = p.bond_chain.iter()
                            .map(|b| format!("{:?}", b))
                            .collect();
                        json!({
                            "bond_chain": chain,
                            "quality":    p.quality,
                            "count":      p.count,
                        })
                    })
                    .collect();
                let report = json!({
                    "total_paths_tracked": self.brain.self_improvement.hot_path_index.paths.len(),
                    "top_paths": paths,
                });
                tool_result(id, &serde_json::to_string_pretty(&report).unwrap_or_default())
            }

            // ── WeakPatterns (Phase 2) ────────────────────────────────────────
            "lu_weak_patterns" => {
                let min_hits = args["min_hits"].as_u64().unwrap_or(3) as u32;
                let top_n    = args["top_n"].as_u64().unwrap_or(10) as usize;
                let patterns: Vec<_> = self.brain.self_improvement.pattern_index
                    .worst_patterns(top_n, min_hits)
                    .into_iter()
                    .map(|e| json!({
                        "pattern":     e.pattern_key,
                        "ema_quality": e.ema_quality,
                        "hit_count":   e.hit_count,
                    }))
                    .collect();
                let report = json!({
                    "total_patterns_tracked": self.brain.self_improvement.pattern_index.entries.len(),
                    "min_hits_filter": min_hits,
                    "weak_patterns": patterns,
                });
                tool_result(id, &serde_json::to_string_pretty(&report).unwrap_or_default())
            }

            // ── ApplyDemotions (Phase 2) ──────────────────────────────────────
            "lu_apply_demotions" => {
                if let Some(err) = self.check_tier(AutonomyTier::Assisted, id) {
                    return err;
                }
                let candidates = self.brain.self_improvement.demotion_candidates();
                let candidate_count = candidates.len();
                let demoted = self.brain.apply_demotions();
                let report = json!({
                    "candidates_found": candidate_count,
                    "demoted_count":    demoted,
                });
                tool_result(id, &serde_json::to_string_pretty(&report).unwrap_or_default())
            }

            // ── TuneStatus (Phase 3) ──────────────────────────────────────────
            "lu_tune_status" => {
                let p  = &self.brain.self_improvement.params;
                let tu = &self.brain.self_improvement.tuner;
                let m  = &self.brain.self_improvement.metrics;

                // Build sorted type_weighting for deterministic display.
                let mut weights: Vec<_> = p.type_weighting.iter().collect();
                weights.sort_by_key(|(k, _)| k.as_str());

                let history: Vec<serde_json::Value> = tu.adjustment_history.iter().map(|a| json!({
                    "at_query":    a.at_query,
                    "delta_top_k": a.delta_top_k,
                    "delta_depth": a.delta_depth,
                    "delta_threshold": a.delta_threshold,
                    "hit_rate_pct":    (a.recall_hit_rate_at_tune * 100.0) as u32,
                    "quality_pct":     (a.avg_quality_at_tune * 100.0) as u32,
                    "note": a.note,
                })).collect();

                let report = json!({
                    "recall_parameters": {
                        "top_k":                   p.top_k,
                        "activation_threshold":    p.activation_threshold,
                        "bond_traversal_depth":    p.bond_traversal_depth,
                        "type_weighting": weights.iter()
                            .map(|(k, v)| json!({ "kind": k, "weight": v }))
                            .collect::<Vec<_>>(),
                    },
                    "tuner": {
                        "tune_interval":  tu.tune_interval,
                        "passes_run":     tu.adjustment_history.len(),
                        "recent_history": history,
                    },
                    "current_metrics": {
                        "total_queries":      m.total_queries,
                        "recall_hit_rate_pct": (m.recall_hit_rate * 100.0) as u32,
                        "avg_quality_pct":    (m.avg_response_quality * 100.0) as u32,
                    }
                });
                tool_result(id, &serde_json::to_string_pretty(&report).unwrap_or_default())
            }

            // ── Unknown tool ──────────────────────────────────────────────────
            other => tool_error(id, &format!("Unknown tool: '{}'", other)),
        }
    }
}

// ── Proposal application ──────────────────────────────────────────────────────

/// Apply an approved `ThinkingProposal` to the brain.  Returns a human-readable
/// outcome description.
fn apply_proposal(brain: &mut Brain, proposal: &ThinkingProposal) -> String {
    match proposal {
        ThinkingProposal::CreateBond { source_seed, target_seed, bond_type, weight } => {
            let bond_t = match bond_type.to_lowercase().as_str() {
                "isa"         => BondType::IsA,
                "haspart"     => BondType::HasPart,
                "causes"      => BondType::Causes,
                "requires"    => BondType::Requires,
                "opposes"     => BondType::Opposes,
                "exampleof"   => BondType::ExampleOf,
                "sequence"    => BondType::Sequence,
                "derivesfrom" => BondType::DerivesFrom,
                "depicts"     => BondType::Depicts,
                "implements"  => BondType::Implements,
                _             => BondType::Context,
            };
            match (Seed::from_hex(source_seed), Seed::from_hex(target_seed)) {
                (Ok(src), Ok(tgt)) => {
                    match brain.associate(&src, &tgt, bond_t, *weight) {
                        Ok(_) => format!("Created {} bond (weight={:.2})", bond_type, weight),
                        Err(e) => format!("Bond creation failed: {}", e),
                    }
                }
                _ => "Invalid seed hex in CreateBond proposal".to_string(),
            }
        }

        ThinkingProposal::StrengthenBond { bond_seed, amount } => {
            match Seed::from_hex(bond_seed) {
                Ok(seed) => {
                    if let Some(bond) = brain.network.get_bond_mut(&seed) {
                        bond.strengthen(*amount);
                        format!("Strengthened bond by {:.2}", amount)
                    } else {
                        "Bond not found".to_string()
                    }
                }
                Err(_) => "Invalid bond seed hex".to_string(),
            }
        }

        ThinkingProposal::WeakenBond { bond_seed, amount } => {
            match Seed::from_hex(bond_seed) {
                Ok(seed) => {
                    if let Some(bond) = brain.network.get_bond_mut(&seed) {
                        bond.weaken(*amount);
                        "Bond weakened".to_string()
                    } else {
                        "Bond not found".to_string()
                    }
                }
                Err(_) => "Invalid bond seed hex".to_string(),
            }
        }

        ThinkingProposal::RevokeNode { seed, reason } => {
            match Seed::from_hex(seed) {
                Ok(s) => {
                    brain.revoke(&s, reason);
                    format!("Node revoked: {}", reason)
                }
                Err(_) => "Invalid node seed hex".to_string(),
            }
        }

        ThinkingProposal::LearnInsight { text, confidence } => {
            let conf = match confidence.to_lowercase().as_str() {
                "verified"   => Confidence::Verified,
                "unverified" => Confidence::Unverified,
                "inferred"   => Confidence::Inferred,
                _            => Confidence::Terminal,
            };
            let seeds = brain.learn_text(text, conf);
            format!("Learned {} KU(s) from insight", seeds.len())
        }

        ThinkingProposal::FormHypothesis { hypothesis, .. } => {
            let seeds = brain.learn_text(hypothesis, Confidence::Inferred);
            format!("Hypothesis recorded as {} inferred KU(s)", seeds.len())
        }
    }
}
