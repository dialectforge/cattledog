// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Tool Integration Layer
//!
//! Gives Lu the ability to interact with the world beyond its
//! knowledge graph. Tools are actions Lu can request — all gated
//! by the autonomy layer and human approval.
//!
//! Tool types:
//! - **FileRead**: read a file from disk
//! - **FileWrite**: write content to a file
//! - **WebFetch**: fetch a URL
//! - **ShellCommand**: run a shell command
//! - **Custom**: user-defined tool
//!
//! The intent resolver detects when a user query implies an action
//! (vs just a factual answer) and routes to the appropriate tool.
//!
//! All tool executions are gated by the autonomy engine.
//! Results feed back into the knowledge graph as session memory.
//!
//! Created by John Dean Martin

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

fn short_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("tool_{:08x}", (nanos & 0xFFFFFFFF) as u32)
}

// ━━━ Tool Types ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// What kind of tool is this?
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToolType {
    /// Read a file from disk
    FileRead,
    /// Write content to a file
    FileWrite,
    /// Fetch a URL (GET only — safe)
    WebFetch,
    /// Run a shell command
    ShellCommand,
    /// User-defined custom tool
    Custom(String),
}

impl std::fmt::Display for ToolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolType::FileRead => write!(f, "file_read"),
            ToolType::FileWrite => write!(f, "file_write"),
            ToolType::WebFetch => write!(f, "web_fetch"),
            ToolType::ShellCommand => write!(f, "shell_command"),
            ToolType::Custom(name) => write!(f, "custom:{}", name),
        }
    }
}

/// Risk level of a tool invocation. Maps to autonomy tiers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToolRisk {
    /// Safe: read-only, no side effects (FileRead, WebFetch GET)
    Safe,
    /// Moderate: writes data but reversible (FileWrite to new file)
    Moderate,
    /// Dangerous: destructive or irreversible (ShellCommand, overwrite)
    Dangerous,
}

// ━━━ Tool Request ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A tool invocation request. Created by the intent resolver
/// when Lu determines an action is needed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolRequest {
    pub id: String,
    pub tool_type: ToolType,
    pub parameters: HashMap<String, String>,
    pub risk: ToolRisk,
    pub reason: String,
    pub requested_at: DateTime<Utc>,
    pub status: ToolStatus,
}

/// Status of a tool request through the approval pipeline.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ToolStatus {
    /// Waiting for human approval
    Pending,
    /// Approved and ready to execute
    Approved,
    /// Currently executing
    Running,
    /// Completed successfully
    Completed,
    /// Failed during execution
    Failed(String),
    /// Rejected by human
    Rejected(String),
    /// Auto-approved by autonomy engine (Safe tier only)
    AutoApproved,
}

/// Result of a tool execution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResult {
    pub request_id: String,
    pub tool_type: ToolType,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub completed_at: DateTime<Utc>,
    pub duration_ms: u64,
}

// ━━━ Intent Detection ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Detected intent from a user query that implies tool usage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DetectedIntent {
    pub query: String,
    pub tool_type: ToolType,
    pub parameters: HashMap<String, String>,
    pub confidence: f32,
    pub explanation: String,
}

/// Simple keyword-based intent resolver.
/// Scans user input for action-implying patterns and returns
/// a ToolRequest if one is detected. Confidence must exceed
/// threshold to trigger (default 0.6).
pub fn detect_intent(query: &str) -> Option<DetectedIntent> {
    let lower = query.to_lowercase();
    let words: Vec<&str> = lower.split_whitespace().collect();

    // File read patterns
    if contains_any(&words, &["read", "open", "show", "cat", "view", "display"])
        && contains_any(&words, &["file", "document", "contents"])
    {
        let path = extract_path(query);
        let mut params = HashMap::new();
        if let Some(p) = path {
            params.insert("path".into(), p);
        }
        return Some(DetectedIntent {
            query: query.to_string(),
            tool_type: ToolType::FileRead,
            parameters: params,
            confidence: 0.7,
            explanation: "Query implies reading a file".into(),
        });
    }

    // File write patterns
    if contains_any(&words, &["write", "save", "create", "output"])
        && contains_any(&words, &["file", "document", "to"])
    {
        let path = extract_path(query);
        let mut params = HashMap::new();
        if let Some(p) = path {
            params.insert("path".into(), p);
        }
        return Some(DetectedIntent {
            query: query.to_string(),
            tool_type: ToolType::FileWrite,
            parameters: params,
            confidence: 0.65,
            explanation: "Query implies writing a file".into(),
        });
    }

    // Web fetch patterns
    if contains_any(&words, &["fetch", "download", "get", "request"])
        && contains_any(&words, &["url", "http", "https", "website", "page", "web"])
    {
        let url = extract_url(query);
        let mut params = HashMap::new();
        if let Some(u) = url {
            params.insert("url".into(), u);
        }
        return Some(DetectedIntent {
            query: query.to_string(),
            tool_type: ToolType::WebFetch,
            parameters: params,
            confidence: 0.7,
            explanation: "Query implies fetching a URL".into(),
        });
    }

    // Shell command patterns
    if contains_any(&words, &["run", "execute", "shell", "command", "cmd"])
        && contains_any(&words, &["command", "script", "process", "terminal"])
    {
        let mut params = HashMap::new();
        // Try to extract quoted command
        if let Some(cmd) = extract_quoted(query) {
            params.insert("command".into(), cmd);
        }
        return Some(DetectedIntent {
            query: query.to_string(),
            tool_type: ToolType::ShellCommand,
            parameters: params,
            confidence: 0.6,
            explanation: "Query implies running a shell command".into(),
        });
    }

    None
}

/// Check if any of the target words appear in the word list.
fn contains_any(words: &[&str], targets: &[&str]) -> bool {
    words.iter().any(|w| targets.contains(w))
}

/// Extract a file path from a query (looks for quoted strings or path-like tokens).
fn extract_path(query: &str) -> Option<String> {
    // Check for quoted paths first
    if let Some(quoted) = extract_quoted(query) {
        if quoted.contains('/') || quoted.contains('\\') || quoted.contains('.') {
            return Some(quoted);
        }
    }
    // Look for path-like tokens
    for token in query.split_whitespace() {
        let clean = token.trim_matches(|c: char| !c.is_alphanumeric() && c != '/' && c != '\\' && c != '.' && c != ':' && c != '_' && c != '-');
        if (clean.contains('/') || clean.contains('\\')) && clean.len() > 2 {
            return Some(clean.to_string());
        }
    }
    None
}

/// Extract a URL from a query.
fn extract_url(query: &str) -> Option<String> {
    for token in query.split_whitespace() {
        let clean = token.trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == '.');
        if clean.starts_with("http://") || clean.starts_with("https://") {
            return Some(clean.to_string());
        }
    }
    None
}

/// Extract a quoted string from a query.
fn extract_quoted(query: &str) -> Option<String> {
    // Double quotes
    if let Some(start) = query.find('"') {
        if let Some(end) = query[start + 1..].find('"') {
            return Some(query[start + 1..start + 1 + end].to_string());
        }
    }
    // Single quotes
    if let Some(start) = query.find('\'') {
        if let Some(end) = query[start + 1..].find('\'') {
            return Some(query[start + 1..start + 1 + end].to_string());
        }
    }
    None
}

// ━━━ Tool Engine ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The tool engine manages pending tool requests and their lifecycle.
/// It does NOT execute tools directly — execution happens through
/// the MCP server or terminal. This tracks state and gating.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolEngine {
    /// Pending tool requests awaiting approval/execution
    pub pending: HashMap<String, ToolRequest>,
    /// Completed tool results (rolling history)
    pub history: Vec<ToolResult>,
    /// Whether the tool engine is enabled
    pub enabled: bool,
    /// Maximum history entries to keep
    pub max_history: usize,
    /// Custom tool definitions
    pub custom_tools: HashMap<String, CustomToolDef>,
}

/// Definition of a custom tool.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomToolDef {
    pub name: String,
    pub description: String,
    pub risk: ToolRisk,
    pub parameters: Vec<String>,
}

impl Default for ToolEngine {
    fn default() -> Self {
        Self {
            pending: HashMap::new(),
            history: Vec::new(),
            enabled: true,
            max_history: 100,
            custom_tools: HashMap::new(),
        }
    }
}

impl ToolEngine {
    /// Submit a new tool request. Returns the request ID.
    /// Safe tools are auto-approved. Others go to Pending.
    pub fn submit(&mut self, intent: DetectedIntent) -> String {
        let id = short_id();
        let risk = classify_risk(&intent.tool_type, &intent.parameters);
        let status = if risk == ToolRisk::Safe {
            ToolStatus::AutoApproved
        } else {
            ToolStatus::Pending
        };
        let request = ToolRequest {
            id: id.clone(),
            tool_type: intent.tool_type,
            parameters: intent.parameters,
            risk,
            reason: intent.explanation,
            requested_at: Utc::now(),
            status,
        };
        self.pending.insert(id.clone(), request);
        id
    }

    /// Approve a pending tool request by ID.
    pub fn approve(&mut self, id: &str) -> Result<(), String> {
        let req = self.pending.get_mut(id)
            .ok_or_else(|| format!("No pending request: {}", id))?;
        if req.status != ToolStatus::Pending {
            return Err(format!("Request {} is {:?}, not Pending", id, req.status));
        }
        req.status = ToolStatus::Approved;
        Ok(())
    }

    /// Reject a pending tool request.
    pub fn reject(&mut self, id: &str, reason: &str) -> Result<(), String> {
        let req = self.pending.get_mut(id)
            .ok_or_else(|| format!("No pending request: {}", id))?;
        if req.status != ToolStatus::Pending {
            return Err(format!("Request {} is {:?}, not Pending", id, req.status));
        }
        req.status = ToolStatus::Rejected(reason.to_string());
        Ok(())
    }

    /// Record the result of a tool execution.
    pub fn record_result(&mut self, result: ToolResult) {
        let id = result.request_id.clone();
        if result.success {
            if let Some(req) = self.pending.get_mut(&id) {
                req.status = ToolStatus::Completed;
            }
        } else {
            if let Some(req) = self.pending.get_mut(&id) {
                req.status = ToolStatus::Failed(
                    result.error.clone().unwrap_or_default()
                );
            }
        }
        self.history.push(result);
        // Trim history
        while self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }

    /// Get all requests ready to execute (Approved or AutoApproved).
    pub fn ready_to_execute(&self) -> Vec<&ToolRequest> {
        self.pending.values()
            .filter(|r| r.status == ToolStatus::Approved || r.status == ToolStatus::AutoApproved)
            .collect()
    }

    /// Get all pending (awaiting approval) requests.
    pub fn awaiting_approval(&self) -> Vec<&ToolRequest> {
        self.pending.values()
            .filter(|r| r.status == ToolStatus::Pending)
            .collect()
    }

    /// Clean up completed/failed/rejected requests from pending map.
    pub fn gc(&mut self) -> usize {
        let before = self.pending.len();
        self.pending.retain(|_, r| {
            matches!(r.status,
                ToolStatus::Pending | ToolStatus::Approved |
                ToolStatus::Running | ToolStatus::AutoApproved)
        });
        before - self.pending.len()
    }

    /// Register a custom tool definition.
    pub fn register_custom(&mut self, def: CustomToolDef) {
        self.custom_tools.insert(def.name.clone(), def);
    }

    /// Process a user query through intent detection.
    /// Returns Some(request_id) if a tool intent was detected and submitted.
    pub fn process_query(&mut self, query: &str) -> Option<(String, DetectedIntent)> {
        if !self.enabled {
            return None;
        }
        let intent = detect_intent(query)?;
        if intent.confidence < 0.6 {
            return None;
        }
        let id = self.submit(intent.clone());
        Some((id, intent))
    }
}

/// Classify risk level based on tool type and parameters.
fn classify_risk(tool_type: &ToolType, params: &HashMap<String, String>) -> ToolRisk {
    match tool_type {
        ToolType::FileRead => ToolRisk::Safe,
        ToolType::WebFetch => ToolRisk::Safe,
        ToolType::FileWrite => {
            // Writing to existing file = dangerous, new file = moderate
            if let Some(path) = params.get("path") {
                if std::path::Path::new(path).exists() {
                    ToolRisk::Dangerous
                } else {
                    ToolRisk::Moderate
                }
            } else {
                ToolRisk::Moderate
            }
        }
        ToolType::ShellCommand => ToolRisk::Dangerous,
        ToolType::Custom(_) => ToolRisk::Moderate,
    }
}

// ━━━ Display Types ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Summary of tool engine state for display.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolEngineStatus {
    pub enabled: bool,
    pub pending_count: usize,
    pub awaiting_approval: usize,
    pub ready_to_execute: usize,
    pub history_count: usize,
    pub custom_tools: Vec<String>,
    pub recent_results: Vec<ToolResultSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ToolResultSummary {
    pub request_id: String,
    pub tool_type: String,
    pub success: bool,
    pub output_preview: String,
    pub duration_ms: u64,
}

impl From<&ToolEngine> for ToolEngineStatus {
    fn from(engine: &ToolEngine) -> Self {
        Self {
            enabled: engine.enabled,
            pending_count: engine.pending.len(),
            awaiting_approval: engine.awaiting_approval().len(),
            ready_to_execute: engine.ready_to_execute().len(),
            history_count: engine.history.len(),
            custom_tools: engine.custom_tools.keys().cloned().collect(),
            recent_results: engine.history.iter().rev().take(5).map(|r| {
                ToolResultSummary {
                    request_id: r.request_id.clone(),
                    tool_type: r.tool_type.to_string(),
                    success: r.success,
                    output_preview: if r.output.len() > 100 {
                        format!("{}...", &r.output[..100])
                    } else {
                        r.output.clone()
                    },
                    duration_ms: r.duration_ms,
                }
            }).collect(),
        }
    }
}

/// Summary of a pending tool request for display.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PendingToolSummary {
    pub id: String,
    pub tool_type: String,
    pub risk: String,
    pub reason: String,
    pub status: String,
    pub parameters: HashMap<String, String>,
}

impl From<&ToolRequest> for PendingToolSummary {
    fn from(req: &ToolRequest) -> Self {
        Self {
            id: req.id.clone(),
            tool_type: req.tool_type.to_string(),
            risk: format!("{:?}", req.risk),
            reason: req.reason.clone(),
            status: format!("{:?}", req.status),
            parameters: req.parameters.clone(),
        }
    }
}
