// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Orchestrator - Lu's Input Router
//!
//! The smart front door. Takes raw user input and decides what to do:
//! question, research request, tool action, learning, thinking, etc.
//! Routes to the correct Brain method(s) behind the scenes.
//!
//! Everything stays proposal-gated and under the terminal bond.
//! Output is clean, conversational, no debug noise.
//!
//! This replaces the need for users to know /commands.
//! Just talk to Lu and Lu figures out what to do.
//!
//! Created by John Dean Martin

use serde::{Serialize, Deserialize};

/// What the orchestrator decided to do with the input.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OrchestratorAction {
    /// Conversational response (question/answer)
    Respond { query: String },
    /// Multi-hop reasoning for complex questions
    Reason { query: String },
    /// Start a thinking session for open-ended goals
    Think { goal: String },
    /// Detect and queue a tool action
    ToolAction { query: String },
    /// Learn new information directly
    Learn { content: String, source: String },
    /// Ingest external content (file, text block)
    Ingest { content: String },
    /// Greeting
    Greet,
    /// Meta question about Lu itself
    MetaSelf,
    /// Curiosity-driven: explore a knowledge gap
    Explore { topic: String },
    /// Unknown / can't classify
    Unknown { input: String },
}

/// The orchestrator's full response back to the user.
/// Clean, conversational, no debug metadata exposed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OrchestratorResponse {
    /// The action that was taken
    pub action: String,
    /// The clean, conversational response text
    pub response: String,
    /// Confidence in the response (0.0 - 1.0)
    pub confidence: f32,
    /// Whether a follow-up action is pending (e.g. tool approval)
    pub pending_action: Option<String>,
    /// Number of knowledge atoms that contributed
    pub atoms_used: usize,
    /// Scar warnings (if any recalled knowledge was near revoked seeds)
    pub scar_warnings: Vec<String>,
}

/// Classify raw user input into an orchestrator action.
/// This is the master router - examines intent, tool signals,
/// learning signals, and complexity to pick the right path.
pub fn classify_input(input: &str) -> OrchestratorAction {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return OrchestratorAction::Unknown { input: input.to_string() };
    }

    let lower = trimmed.to_lowercase();
    let lower = lower.trim_end_matches(&['?', '!', '.'][..]).trim();

    // --- Greetings ---
    let greetings = ["hello", "hi", "hey", "howdy", "greetings",
                     "good morning", "good afternoon", "good evening",
                     "yo", "sup", "what's up", "whats up"];
    if greetings.iter().any(|g| *g == lower) {
        return OrchestratorAction::Greet;
    }

    // --- Meta-self ---
    let meta = ["what are you", "who are you", "tell me about yourself",
                "describe yourself", "what can you do", "how do you work",
                "are you an ai", "are you alive", "are you sentient"];
    if meta.iter().any(|m| lower == *m || lower.starts_with(m)) {
        return OrchestratorAction::MetaSelf;
    }

    // --- Tool actions (file, web, shell) ---
    let tool_signals = [
        ("read file", true), ("open file", true), ("show file", true),
        ("write file", true), ("save file", true), ("create file", true),
        ("fetch url", true), ("fetch http", true), ("download", true),
        ("run command", true), ("execute command", true),
        ("shell command", true),
    ];
    for (signal, _) in &tool_signals {
        if lower.contains(signal) {
            return OrchestratorAction::ToolAction { query: trimmed.to_string() };
        }
    }
    // URL detection
    if trimmed.contains("http://") || trimmed.contains("https://") {
        if lower.contains("fetch") || lower.contains("get") || lower.contains("read") {
            return OrchestratorAction::ToolAction { query: trimmed.to_string() };
        }
    }
    // File path detection
    if (trimmed.contains('\\') || trimmed.contains('/'))
        && (lower.contains("read") || lower.contains("open") || lower.contains("show"))
    {
        return OrchestratorAction::ToolAction { query: trimmed.to_string() };
    }

    // --- Learning signals ---
    let learn_starters = [
        "learn that ", "remember that ", "know that ",
        "fact: ", "definition: ", "note: ",
    ];
    for starter in &learn_starters {
        if lower.starts_with(starter) {
            let content = trimmed[starter.len()..].trim().to_string();
            return OrchestratorAction::Learn {
                content,
                source: "user_direct".to_string(),
            };
        }
    }

    // --- Ingest signals (multi-line or explicit) ---
    if lower.starts_with("ingest:") || lower.starts_with("train on:")
        || lower.starts_with("study:") || lower.starts_with("read and learn:")
    {
        let sep = trimmed.find(':').unwrap_or(0);
        let content = trimmed[sep + 1..].trim().to_string();
        return OrchestratorAction::Ingest { content };
    }

    // --- Thinking / goal signals ---
    let think_starters = [
        "think about ", "figure out ", "plan how to ",
        "work out ", "solve ", "analyze ",
        "consider ", "brainstorm ",
    ];
    for starter in &think_starters {
        if lower.starts_with(starter) {
            let goal = trimmed[starter.len()..].trim().to_string();
            return OrchestratorAction::Think { goal };
        }
    }

    // --- Explore signals ---
    let explore_starters = [
        "tell me everything about ", "what do you know about ",
        "explore ", "deep dive into ",
    ];
    for starter in &explore_starters {
        if lower.starts_with(starter) {
            let topic = trimmed[starter.len()..].trim().to_string();
            return OrchestratorAction::Explore { topic };
        }
    }

    // --- Reasoning signals (complex, multi-hop) ---
    let reason_signals = [
        "why does ", "why is ", "why do ", "why are ", "why can ",
        "how does ", "how is ", "how do ", "how are ", "how can ",
        "explain how ", "explain why ",
        "what causes ", "what happens when ", "what would happen if ",
        "what is the relationship between ",
    ];
    for signal in &reason_signals {
        if lower.starts_with(signal) {
            return OrchestratorAction::Reason { query: trimmed.to_string() };
        }
    }

    // --- Default: conversational response ---
    OrchestratorAction::Respond { query: trimmed.to_string() }
}

/// Clean up a response for conversational output.
/// Strips debug artifacts, ensures proper punctuation,
/// and makes the output feel natural.
pub fn polish_response(raw: &str) -> String {
    let mut text = raw.trim().to_string();

    // Remove any stray debug markers
    text = text.replace("[DEBUG]", "").replace("[WARN]", "");

    // Ensure ends with punctuation
    if !text.is_empty()
        && !text.ends_with('.')
        && !text.ends_with('!')
        && !text.ends_with('?')
    {
        text.push('.');
    }

    // Capitalize first letter
    if let Some(first) = text.chars().next() {
        if first.is_lowercase() {
            let mut chars = text.chars();
            let upper: String = chars.next().unwrap().to_uppercase().collect();
            text = upper + chars.as_str();
        }
    }

    text
}
