// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Feature flags — runtime toggles for Brain capabilities.
//!
//! Each flag can be read before executing a feature so the caller can opt-in
//! or opt-out without recompiling.  Flags are serialisable so they persist
//! with the brain state.

use serde::{Deserialize, Serialize};

// ── FeatureFlags ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureFlags {
    /// Enable arena-mode response competition (default: false).
    pub arena_mode: bool,
    /// Enable web research capabilities (default: true).
    pub web_research: bool,
    /// Enable code execution sandbox (default: true).
    pub code_execution: bool,
    /// Enable background rumination (default: true).
    pub rumination: bool,
    /// Enable idle auto-research — opt-in (default: false).
    pub idle_research: bool,
    /// Enable contradiction detection during rumination (default: true).
    pub contradiction_detection: bool,
    /// Enable multi-agent thinking pipeline (default: true).
    pub multi_agent_thinking: bool,
    /// Enable background daemon mode (default: false).
    pub daemon_mode: bool,
    /// Enable style adaptation based on conversation history (default: true).
    pub style_adaptation: bool,
    /// Enable per-KU source attribution in responses (default: true).
    pub source_attribution: bool,
}

impl Default for FeatureFlags {
    fn default() -> Self {
        FeatureFlags {
            arena_mode:              false,
            web_research:            true,
            code_execution:          true,
            rumination:              true,
            idle_research:           true,
            contradiction_detection: true,
            multi_agent_thinking:    true,
            daemon_mode:             false,
            style_adaptation:        true,
            source_attribution:      true,
        }
    }
}

impl FeatureFlags {
    pub fn new() -> Self {
        FeatureFlags::default()
    }

    /// Toggle a flag by name.  Returns the new value, or Err if unknown.
    pub fn toggle(&mut self, name: &str) -> Result<bool, String> {
        match name {
            "arena_mode"              => { self.arena_mode = !self.arena_mode; Ok(self.arena_mode) }
            "web_research"            => { self.web_research = !self.web_research; Ok(self.web_research) }
            "code_execution"          => { self.code_execution = !self.code_execution; Ok(self.code_execution) }
            "rumination"              => { self.rumination = !self.rumination; Ok(self.rumination) }
            "idle_research"           => { self.idle_research = !self.idle_research; Ok(self.idle_research) }
            "contradiction_detection" => { self.contradiction_detection = !self.contradiction_detection; Ok(self.contradiction_detection) }
            "multi_agent_thinking"    => { self.multi_agent_thinking = !self.multi_agent_thinking; Ok(self.multi_agent_thinking) }
            "daemon_mode"             => { self.daemon_mode = !self.daemon_mode; Ok(self.daemon_mode) }
            "style_adaptation"        => { self.style_adaptation = !self.style_adaptation; Ok(self.style_adaptation) }
            "source_attribution"      => { self.source_attribution = !self.source_attribution; Ok(self.source_attribution) }
            _ => Err(format!("Unknown flag: '{}'.  Known flags: arena_mode, web_research, \
                code_execution, rumination, idle_research, contradiction_detection, \
                multi_agent_thinking, daemon_mode, style_adaptation, source_attribution", name)),
        }
    }

    /// Set a flag to an explicit value.  Returns Err if the name is unrecognised.
    pub fn set(&mut self, name: &str, value: bool) -> Result<(), String> {
        match name {
            "arena_mode"              => { self.arena_mode = value; Ok(()) }
            "web_research"            => { self.web_research = value; Ok(()) }
            "code_execution"          => { self.code_execution = value; Ok(()) }
            "rumination"              => { self.rumination = value; Ok(()) }
            "idle_research"           => { self.idle_research = value; Ok(()) }
            "contradiction_detection" => { self.contradiction_detection = value; Ok(()) }
            "multi_agent_thinking"    => { self.multi_agent_thinking = value; Ok(()) }
            "daemon_mode"             => { self.daemon_mode = value; Ok(()) }
            "style_adaptation"        => { self.style_adaptation = value; Ok(()) }
            "source_attribution"      => { self.source_attribution = value; Ok(()) }
            _ => Err(format!("Unknown flag: '{}'", name)),
        }
    }

    /// List all flags and their current values (alphabetical order).
    pub fn list(&self) -> Vec<(&'static str, bool)> {
        vec![
            ("arena_mode",              self.arena_mode),
            ("code_execution",          self.code_execution),
            ("contradiction_detection", self.contradiction_detection),
            ("daemon_mode",             self.daemon_mode),
            ("idle_research",           self.idle_research),
            ("multi_agent_thinking",    self.multi_agent_thinking),
            ("rumination",              self.rumination),
            ("source_attribution",      self.source_attribution),
            ("style_adaptation",        self.style_adaptation),
            ("web_research",            self.web_research),
        ]
    }

    /// Format all flags for display (e.g. in a `:flags` REPL command).
    pub fn display(&self) -> String {
        self.list()
            .iter()
            .map(|(name, enabled)| {
                let mark = if *enabled { "✓" } else { "✗" };
                format!("  {} {}", mark, name)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}
