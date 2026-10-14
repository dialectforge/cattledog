// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Idle Exploration Engine
//!
//! Lu gets curious when idle. This module watches for periods of
//! inactivity and automatically kicks off research sessions using
//! the top items from the wonder queue.
//!
//! When you walk away from the terminal, Lu goes reading.
//! When you come back, it's learned new things.
//!
//! ARCHITECTURE: This is a lightweight coordinator, not a thread.
//! The MCP server or terminal calls `check_idle()` periodically.
//! If enough time has passed since the last interaction, Lu picks
//! the top wonder and researches it.
//!
//! Created by John Dean Martin

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

/// How long Lu waits before exploring on its own (seconds).
pub const IDLE_THRESHOLD_SECS: i64 = 300; // 5 minutes

/// Maximum explorations per idle period.
pub const MAX_IDLE_EXPLORATIONS: u32 = 3;

/// Cooldown between idle explorations (seconds).
pub const EXPLORATION_COOLDOWN_SECS: i64 = 120; // 2 minutes

/// Record of an idle exploration session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplorationRecord {
    pub wonder_id: String,
    pub query: String,
    pub pages_visited: usize,
    pub facts_learned: usize,
    pub images_found: usize,
    pub gap_filled: bool,
    pub started_at: DateTime<Utc>,
    pub duration_ms: u64,
}

/// The Idle Exploration Engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdleExplorer {
    /// When Lu last received user input
    pub last_interaction: DateTime<Utc>,
    /// When Lu last explored something
    pub last_exploration: DateTime<Utc>,
    /// How many explorations in the current idle period
    pub explorations_this_idle: u32,
    /// History of explorations
    pub history: Vec<ExplorationRecord>,
    /// Whether idle exploration is enabled
    pub enabled: bool,
    /// Custom idle threshold (seconds)
    pub idle_threshold: i64,
}

impl Default for IdleExplorer {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            last_interaction: now,
            last_exploration: now,
            explorations_this_idle: 0,
            history: Vec::new(),
            enabled: true,
            idle_threshold: IDLE_THRESHOLD_SECS,
        }
    }
}

impl IdleExplorer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that the user interacted with Lu.
    /// Resets the idle timer and exploration count.
    pub fn record_interaction(&mut self) {
        self.last_interaction = Utc::now();
        self.explorations_this_idle = 0;
    }

    /// Check if Lu should explore right now.
    /// Returns true if: idle long enough, cooldown passed, budget not exceeded.
    pub fn should_explore(&self) -> bool {
        if !self.enabled { return false; }

        let now = Utc::now();
        let idle_secs = (now - self.last_interaction).num_seconds();
        let since_last = (now - self.last_exploration).num_seconds();

        // Must be idle long enough
        if idle_secs < self.idle_threshold { return false; }

        // Must wait between explorations
        if since_last < EXPLORATION_COOLDOWN_SECS { return false; }

        // Must not exceed budget
        if self.explorations_this_idle >= MAX_IDLE_EXPLORATIONS { return false; }

        true
    }

    /// Record the result of an exploration.
    pub fn record_exploration(&mut self, record: ExplorationRecord) {
        self.last_exploration = Utc::now();
        self.explorations_this_idle += 1;
        self.history.push(record);
        // Keep history manageable
        if self.history.len() > 100 {
            self.history.drain(..50);
        }
    }

    /// Get recent explorations (for showing the user what Lu learned).
    pub fn recent_explorations(&self, count: usize) -> Vec<&ExplorationRecord> {
        self.history.iter().rev().take(count).collect()
    }

    /// How long Lu has been idle (seconds).
    pub fn idle_seconds(&self) -> i64 {
        (Utc::now() - self.last_interaction).num_seconds()
    }

    /// Set whether idle exploration is enabled.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Set custom idle threshold.
    pub fn set_threshold(&mut self, seconds: i64) {
        self.idle_threshold = seconds.max(60); // Minimum 1 minute
    }

    /// Get exploration stats.
    pub fn stats(&self) -> IdleExplorerStats {
        IdleExplorerStats {
            enabled: self.enabled,
            idle_seconds: self.idle_seconds(),
            explorations_this_idle: self.explorations_this_idle,
            total_explorations: self.history.len(),
            total_facts_learned: self.history.iter().map(|r| r.facts_learned).sum(),
            total_gaps_filled: self.history.iter().filter(|r| r.gap_filled).count(),
            should_explore: self.should_explore(),
        }
    }
}

/// Stats for the idle explorer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdleExplorerStats {
    pub enabled: bool,
    pub idle_seconds: i64,
    pub explorations_this_idle: u32,
    pub total_explorations: usize,
    pub total_facts_learned: usize,
    pub total_gaps_filled: usize,
    pub should_explore: bool,
}
