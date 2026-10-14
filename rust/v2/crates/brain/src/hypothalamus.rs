// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! Hypothalamus — resource governor, autonomy tiers, health monitor.
//!
//! The hypothalamus is the brain's homeostatic controller.  It enforces
//! resource budgets (memory, embedding throughput, token rate) and manages
//! the autonomy tier that governs how independently Lu can act.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── AutonomyTier ──────────────────────────────────────────────────────────────

/// Controls how much Lu can do without explicit human approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub enum AutonomyTier {
    /// Every action — including saving and embedding — requires approval.
    #[default]
    Supervised,
    /// Pre-approved action types run automatically; new types ask.
    Assisted,
    /// Independent within a policy boundary; checks in at milestones.
    Collaborative,
    /// Full independence within defined policy limits.
    Autonomous,
}

impl AutonomyTier {
    pub fn label(&self) -> &'static str {
        match self {
            AutonomyTier::Supervised   => "supervised",
            AutonomyTier::Assisted     => "assisted",
            AutonomyTier::Collaborative => "collaborative",
            AutonomyTier::Autonomous   => "autonomous",
        }
    }

    /// True if Lu may act without per-action confirmation for this tier.
    pub fn allows_autonomous_action(&self) -> bool {
        matches!(self, AutonomyTier::Collaborative | AutonomyTier::Autonomous)
    }
}

// ── ResourceBudget ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceBudget {
    pub max_memory_mb: u64,
    pub max_cpu_pct: f32,
    pub max_tokens_per_min: u32,
    pub max_embeddings_per_min: u32,
    pub max_tool_calls_per_min: u32,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        ResourceBudget {
            max_memory_mb: 2048,
            max_cpu_pct: 80.0,
            max_tokens_per_min: 60_000,
            max_embeddings_per_min: 500,
            max_tool_calls_per_min: 20,
        }
    }
}

// ── ResourceCounters ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default)]
pub struct ResourceCounters {
    pub tokens_this_minute: u32,
    pub embeddings_this_minute: u32,
    pub tool_calls_this_minute: u32,
    pub window_start: Option<DateTime<Utc>>,
}

impl ResourceCounters {
    pub fn reset_if_new_window(&mut self) {
        let now = Utc::now();
        if let Some(start) = self.window_start {
            if now.signed_duration_since(start).num_seconds() >= 60 {
                self.tokens_this_minute = 0;
                self.embeddings_this_minute = 0;
                self.tool_calls_this_minute = 0;
                self.window_start = Some(now);
            }
        } else {
            self.window_start = Some(now);
        }
    }

    pub fn record_tokens(&mut self, n: u32) {
        self.reset_if_new_window();
        self.tokens_this_minute += n;
    }

    pub fn record_embedding(&mut self) {
        self.reset_if_new_window();
        self.embeddings_this_minute += 1;
    }

    pub fn record_tool_call(&mut self) {
        self.reset_if_new_window();
        self.tool_calls_this_minute += 1;
    }
}

// ── HealthReport ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthReport {
    pub timestamp: DateTime<Utc>,
    pub memory_mb: u64,
    pub node_count: usize,
    pub bond_count: usize,
    pub pending_consolidations: usize,
    pub scar_count: usize,
    pub autonomy_tier: AutonomyTier,
    pub tokens_this_min: u32,
    pub embeddings_this_min: u32,
    pub warnings: Vec<String>,
}

// ── Hypothalamus ─────────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct Hypothalamus {
    pub autonomy_tier: AutonomyTier,
    pub budget: ResourceBudget,
    pub counters: ResourceCounters,
    pub health_history: Vec<HealthReport>,
    pub max_health_history: usize,
}

impl Default for Hypothalamus {
    fn default() -> Self {
        Hypothalamus::new()
    }
}

impl Hypothalamus {
    pub fn new() -> Self {
        Hypothalamus {
            autonomy_tier: AutonomyTier::Supervised,
            budget: ResourceBudget::default(),
            counters: ResourceCounters::default(),
            health_history: Vec::new(),
            max_health_history: 100,
        }
    }

    pub fn set_autonomy_tier(&mut self, tier: AutonomyTier) {
        self.autonomy_tier = tier;
    }

    /// Check whether we're within budget for the given resource.
    pub fn within_token_budget(&self) -> bool {
        self.counters.tokens_this_minute < self.budget.max_tokens_per_min
    }

    pub fn within_embedding_budget(&self) -> bool {
        self.counters.embeddings_this_minute < self.budget.max_embeddings_per_min
    }

    pub fn within_tool_budget(&self) -> bool {
        self.counters.tool_calls_this_minute < self.budget.max_tool_calls_per_min
    }

    // ── Tool gating ───────────────────────────────────────────────────────────

    /// Gate a tool execution request against the current autonomy tier.
    ///
    /// | Tier          | Allowed tool kinds                              |
    /// |---------------|-------------------------------------------------|
    /// | Supervised    | None — observe-only, all execution blocked.     |
    /// | Assisted      | Read-style tools: WebSearch, ModelBridge,       |
    /// |               | VoiceTranscription, Http (read).                |
    /// | Collaborative | All tools — explicit approval assumed.          |
    /// | Autonomous    | All tools — fully self-directed.                |
    ///
    /// Returns `Ok(())` if permitted, `Err(reason)` if blocked.
    pub fn gate_tool(&self, kind: &crate::cerebellum::ToolKind) -> Result<(), String> {
        use crate::cerebellum::ToolKind;
        match self.autonomy_tier {
            AutonomyTier::Supervised => Err(format!(
                "Tier '{}' (observe-only) blocks all tool execution; \
                 elevate to Assisted or higher",
                self.autonomy_tier.label()
            )),
            AutonomyTier::Assisted => match kind {
                // Read-style, non-destructive tools allowed at Assisted.
                ToolKind::WebSearch
                | ToolKind::VoiceTranscription
                | ToolKind::ModelBridge
                | ToolKind::Http => Ok(()),
                _ => Err(format!(
                    "Tool '{}' requires Act tier (Collaborative or higher); \
                     current tier is '{}'",
                    kind.label(),
                    self.autonomy_tier.label()
                )),
            },
            AutonomyTier::Collaborative | AutonomyTier::Autonomous => Ok(()),
        }
    }

    pub fn snapshot_health(
        &mut self,
        node_count: usize,
        bond_count: usize,
        scar_count: usize,
        pending_consolidations: usize,
    ) -> &HealthReport {
        let mut warnings: Vec<String> = Vec::new();

        if !self.within_token_budget() {
            warnings.push("Token rate limit approaching".to_string());
        }
        if !self.within_embedding_budget() {
            warnings.push("Embedding rate limit approaching".to_string());
        }

        // Estimate memory usage from graph size.
        // Rough model: each node ≈ 2 KB (text + embedding + metadata),
        //              each bond ≈ 256 bytes.
        const NODE_BYTES: u64 = 2048;
        const BOND_BYTES: u64 = 256;
        let estimated_memory_mb =
            ((node_count as u64 * NODE_BYTES + bond_count as u64 * BOND_BYTES) / (1024 * 1024))
                .max(1);

        // Try to read actual process memory from /proc/self/status (Linux),
        // fall back to the estimate on other platforms.
        let memory_mb = read_process_memory_mb().unwrap_or(estimated_memory_mb);

        let report = HealthReport {
            timestamp: Utc::now(),
            memory_mb,
            node_count,
            bond_count,
            pending_consolidations,
            scar_count,
            autonomy_tier: self.autonomy_tier,
            tokens_this_min: self.counters.tokens_this_minute,
            embeddings_this_min: self.counters.embeddings_this_minute,
            warnings,
        };

        if self.health_history.len() >= self.max_health_history {
            self.health_history.remove(0);
        }
        self.health_history.push(report);
        self.health_history.last().unwrap()
    }
}

// ── Memory reading ────────────────────────────────────────────────────────────

/// Attempt to read the current process's resident set size in MB.
/// Uses /proc/self/status on Linux; returns None on other platforms.
fn read_process_memory_mb() -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        for line in status.lines() {
            if line.starts_with("VmRSS:") {
                // Format: "VmRSS:   12345 kB"
                let kb: u64 = line
                    .split_whitespace()
                    .nth(1)?
                    .parse()
                    .ok()?;
                return Some(kb / 1024);
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
