// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # MCP Protocol Helpers
//!
//! JSON-RPC 2.0 response formatting and MCP handshake.

use serde_json::{json, Value};

/// MCP initialize response — declares capabilities and tools.
pub fn handle_initialize(id: &Value) -> String {
    let result = json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {}
        },
        "serverInfo": {
            "name": "lu_ai",
            "version": "0.1.0"
        }
    });
    success_response(id.clone(), result)
}

/// Format a JSON-RPC success response.
pub fn success_response(id: Value, result: Value) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    }).to_string()
}

/// Format a JSON-RPC error response.
pub fn error_response(id: Value, code: i32, message: &str) -> String {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    }).to_string()
}

/// Format an MCP tool result (success).
pub fn tool_result(id: &Value, text: &str) -> String {
    let result = json!({
        "content": [{
            "type": "text",
            "text": text
        }]
    });
    success_response(id.clone(), result)
}

/// Format an MCP tool error result.
pub fn tool_error(id: &Value, text: &str) -> String {
    let result = json!({
        "content": [{
            "type": "text",
            "text": text
        }],
        "isError": true
    });
    success_response(id.clone(), result)
}
