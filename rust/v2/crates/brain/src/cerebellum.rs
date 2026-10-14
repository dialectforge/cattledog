// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Cerebellum — computer use, code execution, file I/O, model bridge, tool orchestration.
//!
//! The cerebellum handles all *external* actions: running tools, calling models,
//! executing code, reading/writing files.  It exposes a uniform ToolRequest /
//! ToolResult interface to the rest of the brain.

use serde::{Deserialize, Serialize};
use std::time::Instant;

// ── ToolKind ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolKind {
    /// Mouse + keyboard computer use
    ComputerUse,
    /// Execute code in a sandboxed interpreter
    CodeExecution,
    /// Text-to-speech synthesis
    VoiceSynthesis,
    /// Speech-to-text transcription
    VoiceTranscription,
    /// Web search / scraping
    WebSearch,
    /// Read or write local filesystem
    FileSystem,
    /// Call an external LLM (model bridge)
    ModelBridge,
    /// Generic HTTP call
    Http,
}

impl ToolKind {
    pub fn label(&self) -> &'static str {
        match self {
            ToolKind::ComputerUse        => "computer_use",
            ToolKind::CodeExecution      => "code_exec",
            ToolKind::VoiceSynthesis     => "voice_synth",
            ToolKind::VoiceTranscription => "voice_transcribe",
            ToolKind::WebSearch          => "web_search",
            ToolKind::FileSystem         => "fs",
            ToolKind::ModelBridge        => "model_bridge",
            ToolKind::Http               => "http",
        }
    }

    /// Whether this tool kind requires explicit user approval above Supervised tier.
    pub fn requires_approval(&self) -> bool {
        matches!(
            self,
            ToolKind::ComputerUse | ToolKind::CodeExecution | ToolKind::FileSystem
        )
    }
}

// ── ToolRequest ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRequest {
    pub id: String,
    pub kind: ToolKind,
    pub params: serde_json::Value,
    /// 0 = lowest, 255 = highest urgency
    pub priority: u8,
    /// True if the caller expects a result before continuing.
    pub blocking: bool,
}

impl ToolRequest {
    pub fn new(kind: ToolKind, params: serde_json::Value) -> Self {
        ToolRequest {
            id: uuid::Uuid::new_v4().to_string(),
            kind,
            params,
            priority: 128,
            blocking: false,
        }
    }
}

// ── ToolResult ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub request_id: String,
    pub kind: ToolKind,
    pub success: bool,
    pub output: serde_json::Value,
    pub error: Option<String>,
    pub duration_ms: u64,
}

// ── ModelBridgeRequest ────────────────────────────────────────────────────────

/// Parameters for delegating generation to an external model via Ollama.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelBridgeRequest {
    pub model_id: String,
    pub system_prompt: Option<String>,
    pub user_message: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

// ── Cerebellum ────────────────────────────────────────────────────────────────

#[derive(Debug, Default)]
pub struct Cerebellum {
    pub pending_requests: Vec<ToolRequest>,
    pub completed_results: Vec<ToolResult>,
    pub total_tool_calls: u64,
    pub failed_tool_calls: u64,
}

impl Cerebellum {
    pub fn new() -> Self {
        Cerebellum::default()
    }

    pub fn enqueue(&mut self, request: ToolRequest) -> String {
        let id = request.id.clone();
        self.pending_requests.push(request);
        id
    }

    pub fn complete(&mut self, result: ToolResult) {
        self.total_tool_calls += 1;
        if !result.success {
            self.failed_tool_calls += 1;
        }
        self.pending_requests.retain(|r| r.id != result.request_id);
        self.completed_results.push(result);
    }

    pub fn get_result(&self, request_id: &str) -> Option<&ToolResult> {
        self.completed_results
            .iter()
            .rev()
            .find(|r| r.request_id == request_id)
    }

    pub fn pending_count(&self) -> usize {
        self.pending_requests.len()
    }

    pub fn success_rate(&self) -> f32 {
        if self.total_tool_calls == 0 {
            return 1.0;
        }
        1.0 - (self.failed_tool_calls as f32 / self.total_tool_calls as f32)
    }

    // ── Tier-gated execution ──────────────────────────────────────────────────

    pub fn try_execute(
        &mut self,
        request: ToolRequest,
        tier: &crate::hypothalamus::AutonomyTier,
    ) -> Result<String, String> {
        use crate::hypothalamus::AutonomyTier;

        let allowed = match tier {
            AutonomyTier::Supervised => false,
            AutonomyTier::Assisted   => !request.kind.requires_approval(),
            AutonomyTier::Collaborative | AutonomyTier::Autonomous => true,
        };

        if !allowed {
            return Err(format!(
                "Tool '{}' blocked: tier '{}' does not permit this action",
                request.kind.label(),
                tier.label()
            ));
        }

        Ok(self.enqueue(request))
    }

    // ── Synchronous execution ─────────────────────────────────────────────────

    /// Execute a ToolRequest synchronously and return the ToolResult.
    ///
    /// Dispatches based on `request.kind`:
    /// - `FileSystem` → real std::fs read/write
    /// - `ModelBridge` → real HTTP POST to Ollama (localhost:11434)
    /// - `WebSearch` → not implemented (returns error)
    /// - `CodeExecution` → not implemented (returns error)
    /// - Others → not implemented
    pub fn execute_sync(&mut self, request: ToolRequest) -> ToolResult {
        let start = Instant::now();
        let id = request.id.clone();
        let kind = request.kind.clone();

        let (success, output, error) = match &request.kind {
            ToolKind::FileSystem => execute_filesystem(&request.params),
            ToolKind::ModelBridge => execute_model_bridge(&request.params),
            ToolKind::WebSearch => execute_web_search(&request.params),
            ToolKind::Http => execute_http(&request.params),
            ToolKind::CodeExecution => execute_code(&request.params),
            other => (
                false,
                serde_json::Value::Null,
                Some(format!("tool kind '{}' not yet implemented for synchronous execution", other.label())),
            ),
        };

        let duration_ms = start.elapsed().as_millis() as u64;
        let result = ToolResult {
            request_id: id,
            kind,
            success,
            output,
            error,
            duration_ms,
        };
        self.complete(result.clone());
        result
    }
}

// ── FileSystem executor ───────────────────────────────────────────────────────

/// Params: `{ "op": "read"|"write", "path": "...", "content": "..." (write only) }`
fn execute_filesystem(
    params: &serde_json::Value,
) -> (bool, serde_json::Value, Option<String>) {
    let op   = params["op"].as_str().unwrap_or("read");
    let path = match params["path"].as_str() {
        Some(p) if !p.is_empty() => p,
        _ => return (false, serde_json::Value::Null, Some("path parameter required".to_string())),
    };

    match op {
        "read" => match std::fs::read_to_string(path) {
            Ok(content) => (
                true,
                serde_json::json!({ "content": content, "bytes": content.len() }),
                None,
            ),
            Err(e) => (false, serde_json::Value::Null, Some(e.to_string())),
        },
        "write" => {
            let content = params["content"].as_str().unwrap_or("");
            match std::fs::write(path, content) {
                Ok(()) => (
                    true,
                    serde_json::json!({ "bytes_written": content.len() }),
                    None,
                ),
                Err(e) => (false, serde_json::Value::Null, Some(e.to_string())),
            }
        }
        "append" => {
            use std::io::Write;
            let content = params["content"].as_str().unwrap_or("");
            match std::fs::OpenOptions::new().append(true).create(true).open(path) {
                Ok(mut f) => match f.write_all(content.as_bytes()) {
                    Ok(()) => (
                        true,
                        serde_json::json!({ "bytes_appended": content.len() }),
                        None,
                    ),
                    Err(e) => (false, serde_json::Value::Null, Some(e.to_string())),
                },
                Err(e) => (false, serde_json::Value::Null, Some(e.to_string())),
            }
        }
        "exists" => {
            let exists = std::path::Path::new(path).exists();
            (true, serde_json::json!({ "exists": exists }), None)
        }
        other => (
            false,
            serde_json::Value::Null,
            Some(format!("unknown fs op '{}'; valid: read, write, append, exists", other)),
        ),
    }
}

// ── ModelBridge executor (Ollama) ─────────────────────────────────────────────

/// Params: `{ "model": "...", "prompt": "...", "system": "..." (optional),
///            "max_tokens": 512, "temperature": 0.7 }`
///
/// Sends a POST to http://localhost:11434/api/generate and collects the full
/// streamed response (newline-delimited JSON objects with `"response"` field).
fn execute_model_bridge(
    params: &serde_json::Value,
) -> (bool, serde_json::Value, Option<String>) {
    let model   = params["model"].as_str().unwrap_or("llama3");
    let prompt  = match params["prompt"].as_str() {
        Some(p) if !p.is_empty() => p,
        _ => return (false, serde_json::Value::Null, Some("prompt parameter required".to_string())),
    };
    let system      = params["system"].as_str();
    let max_tokens  = params["max_tokens"].as_u64().unwrap_or(512);
    let temperature = params["temperature"].as_f64().unwrap_or(0.7);

    // Build the JSON request body.
    let mut body = serde_json::json!({
        "model":  model,
        "prompt": prompt,
        "stream": false,
        "options": {
            "num_predict": max_tokens,
            "temperature": temperature
        }
    });
    if let Some(sys) = system {
        body["system"] = serde_json::json!(sys);
    }

    // Make a synchronous TCP connection to Ollama.
    match call_ollama_generate(&body.to_string()) {
        Ok(response_text) => (
            true,
            serde_json::json!({
                "model": model,
                "response": response_text
            }),
            None,
        ),
        Err(e) => (
            false,
            serde_json::Value::Null,
            Some(format!("Ollama API error: {}", e)),
        ),
    }
}

// ── WebSearch executor ────────────────────────────────────────────────────────

/// Params: `{ "query": "..." }`
///
/// Runs the Grokopedia-first → DuckDuckGo Lite fallback search pipeline and
/// returns matching results as a JSON array.  Does NOT ingest into the brain
/// (ingestion happens in Brain::web_research).
fn execute_web_search(
    params: &serde_json::Value,
) -> (bool, serde_json::Value, Option<String>) {
    let query = match params["query"].as_str() {
        Some(q) if !q.is_empty() => q,
        _ => {
            return (
                false,
                serde_json::Value::Null,
                Some("query parameter required".to_string()),
            )
        }
    };

    let researcher = crate::web_research::WebResearcher::new();
    let results = researcher.search(query);

    if results.is_empty() {
        return (
            false,
            serde_json::Value::Null,
            Some(format!("no results found for query: {}", query)),
        );
    }

    let json_results: Vec<serde_json::Value> = results
        .iter()
        .map(|r| {
            serde_json::json!({
                "title":   r.title,
                "url":     r.url,
                "snippet": r.snippet,
            })
        })
        .collect();

    (
        true,
        serde_json::json!({
            "query":   query,
            "count":   json_results.len(),
            "results": json_results,
        }),
        None,
    )
}

// ── HTTP executor ─────────────────────────────────────────────────────────────

/// Params: `{ "url": "https://...", "method": "GET"|"POST" (default GET),
///            "body": "..." (POST only) }`
///
/// Enforces HTTPS-only via the safety layer.  Returns the raw response body.
fn execute_http(
    params: &serde_json::Value,
) -> (bool, serde_json::Value, Option<String>) {
    let url = match params["url"].as_str() {
        Some(u) if !u.is_empty() => u,
        _ => {
            return (
                false,
                serde_json::Value::Null,
                Some("url parameter required".to_string()),
            )
        }
    };

    // Safety check.
    let safety = crate::web_safety::WebSafety::new();
    let verdict = safety.check_url(url);
    if !verdict.safe {
        return (
            false,
            serde_json::Value::Null,
            Some(format!("blocked by safety layer: {}", verdict.reason)),
        );
    }

    let method = params["method"].as_str().unwrap_or("GET").to_uppercase();

    let result = match method.as_str() {
        "POST" => {
            let body = params["body"].as_str().unwrap_or("");
            crate::web_research::http_post_form(url, body, 15)
        }
        _ => crate::web_research::http_get(url, 15),
    };

    match result {
        Ok(body) => (
            true,
            serde_json::json!({
                "url":    url,
                "method": method,
                "bytes":  body.len(),
                "body":   body.chars().take(4096).collect::<String>(), // cap at 4 KiB in tool output
            }),
            None,
        ),
        Err(e) => (
            false,
            serde_json::Value::Null,
            Some(format!("HTTP {} {}: {}", method, url, e)),
        ),
    }
}

// ── CodeExecution executor ────────────────────────────────────────────────────

/// Params: `{ "language": "python"|"rust"|"powershell", "code": "...",
///            "timeout_ms": 30000 (optional), "working_dir": "subdir" (optional) }`
fn execute_code(
    params: &serde_json::Value,
) -> (bool, serde_json::Value, Option<String>) {
    use crate::code_sandbox::{CodeExecutionRequest, CodeSandbox};

    let language = match params["language"].as_str() {
        Some(l) if !l.is_empty() => l,
        _ => return (false, serde_json::Value::Null, Some("language parameter required".to_string())),
    };
    let code = match params["code"].as_str() {
        Some(c) if !c.is_empty() => c,
        _ => return (false, serde_json::Value::Null, Some("code parameter required".to_string())),
    };
    let timeout_ms = params["timeout_ms"].as_u64().unwrap_or(crate::code_sandbox::DEFAULT_TIMEOUT_MS);
    let working_dir = params["working_dir"].as_str().map(|s| s.to_string());

    let sandbox = match CodeSandbox::new() {
        Ok(s) => s,
        Err(e) => return (false, serde_json::Value::Null, Some(format!("sandbox init error: {}", e))),
    };

    let req = CodeExecutionRequest {
        language: language.to_string(),
        code: code.to_string(),
        timeout_ms,
        working_dir,
    };

    let result = sandbox.execute(&req);
    let success = result.exit_code == Some(0) && !result.timed_out;

    let output = serde_json::json!({
        "stdout":        result.stdout,
        "stderr":        result.stderr,
        "exit_code":     result.exit_code,
        "files_created": result.files_created,
        "timed_out":     result.timed_out,
        "duration_ms":   result.duration_ms,
    });

    let error = if !success {
        Some(if result.timed_out {
            "execution timed out".to_string()
        } else {
            format!("exit code {:?}", result.exit_code)
        })
    } else {
        None
    };

    (success, output, error)
}

/// Synchronous HTTP POST to `localhost:11434/api/generate` using only std::net.
fn call_ollama_generate(body: &str) -> Result<String, String> {
    use std::io::{Read, Write};
    use std::net::TcpStream;

    let host = "127.0.0.1:11434";
    let path = "/api/generate";

    let mut stream = TcpStream::connect(host)
        .map_err(|e| format!("cannot connect to Ollama at {}: {}", host, e))?;

    // Set a 30s read timeout so we don't block forever.
    stream
        .set_read_timeout(Some(std::time::Duration::from_secs(30)))
        .ok();

    let request = format!(
        "POST {} HTTP/1.1\r\n\
         Host: localhost\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        path,
        body.len(),
        body
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("write error: {}", e))?;

    // Read the full response.
    let mut raw = String::new();
    stream
        .read_to_string(&mut raw)
        .map_err(|e| format!("read error: {}", e))?;

    // Split headers from body.
    let body_start = raw.find("\r\n\r\n").map(|i| i + 4).unwrap_or(0);
    let resp_body = &raw[body_start..];

    // Ollama returns a single JSON object when stream=false.
    let json: serde_json::Value = serde_json::from_str(resp_body.trim())
        .map_err(|e| format!("JSON parse error: {} — body was: {}", e, &resp_body[..resp_body.len().min(200)]))?;

    json["response"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| format!("no 'response' field in Ollama reply: {}", resp_body))
}
