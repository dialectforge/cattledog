// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Recursive Self-Improvement
//!
//! Lu monitors its own performance and proposes improvements.
//! All proposals are Tier 2 — human approval required.
//!
//! Components:
//! - **PerformanceMonitor**: per-query metrics tracking.
//! - **MetaReflection**: analyses patterns in self_reflect and
//!   proposes heuristic adjustments.
//! - **ModuleProposer**: generates structured specs for new modules.
//!
//! Created by John Dean Martin

use std::collections::VecDeque;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

// ━━━ Performance Metrics ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Per-query performance metric.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueryMetric {
    pub query: String,
    pub activated_nodes: usize,
    pub traversal_hops: usize,
    pub top_strength: f32,
    pub gap_count: usize,
    pub bridge_count: usize,
    pub duration_ms: u64,
    pub timestamp: DateTime<Utc>,
}

/// Rolling performance monitor.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerformanceMonitor {
    metrics: VecDeque<QueryMetric>,
    max_history: usize,
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self {
            metrics: VecDeque::new(),
            max_history: 500,
        }
    }
}

impl PerformanceMonitor {
    pub fn new() -> Self { Self::default() }

    pub fn record(&mut self, metric: QueryMetric) {
        self.metrics.push_back(metric);
        if self.metrics.len() > self.max_history {
            self.metrics.pop_front();
        }
    }

    /// Average activated nodes across recent queries.
    pub fn avg_activation(&self) -> f32 {
        if self.metrics.is_empty() { return 0.0; }
        self.metrics.iter().map(|m| m.activated_nodes as f32).sum::<f32>()
            / self.metrics.len() as f32
    }

    /// Average gap count (lower = healthier).
    pub fn avg_gaps(&self) -> f32 {
        if self.metrics.is_empty() { return 0.0; }
        self.metrics.iter().map(|m| m.gap_count as f32).sum::<f32>()
            / self.metrics.len() as f32
    }

    /// Trend: are gaps increasing or decreasing?
    /// Returns positive if improving (fewer gaps), negative if degrading.
    pub fn gap_trend(&self) -> f32 {
        if self.metrics.len() < 10 { return 0.0; }
        let mid = self.metrics.len() / 2;
        let first_half: f32 = self.metrics.iter().take(mid)
            .map(|m| m.gap_count as f32).sum::<f32>() / mid as f32;
        let second_half: f32 = self.metrics.iter().skip(mid)
            .map(|m| m.gap_count as f32).sum::<f32>() / (self.metrics.len() - mid) as f32;
        first_half - second_half // positive = improving
    }

    /// Most common query patterns (first words).
    pub fn hot_domains(&self) -> Vec<(String, usize)> {
        let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        for m in &self.metrics {
            let domain = m.query.split_whitespace().next()
                .unwrap_or("unknown")
                .to_lowercase();
            *counts.entry(domain).or_insert(0) += 1;
        }
        let mut sorted: Vec<(String, usize)> = counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1));
        sorted.truncate(10);
        sorted
    }

    pub fn total_queries(&self) -> usize { self.metrics.len() }
}

// ━━━ Meta-Reflection ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A proposed heuristic adjustment from meta-reflection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MetaReflection {
    pub id: String,
    pub pattern_observed: String,
    pub current_heuristic: String,
    pub proposed_adjustment: String,
    pub confidence: f32,
    pub created_at: DateTime<Utc>,
}

/// A structured proposal for a new Rust module.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModuleProposal {
    pub name: String,
    pub purpose: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub integration_points: Vec<String>,
    pub pseudo_algorithm: String,
    pub created_at: DateTime<Utc>,
}

// ━━━ Self-Improvement Engine ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Top-level engine for recursive self-improvement.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelfImprovementEngine {
    pub monitor: PerformanceMonitor,
    pub reflections: Vec<MetaReflection>,
    pub module_proposals: Vec<ModuleProposal>,
    pub enabled: bool,
}

impl Default for SelfImprovementEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SelfImprovementEngine {
    pub fn new() -> Self {
        Self {
            monitor: PerformanceMonitor::new(),
            reflections: Vec::new(),
            module_proposals: Vec::new(),
            enabled: true,
        }
    }

    /// Record a query metric.
    pub fn record_query(&mut self, metric: QueryMetric) {
        if !self.enabled { return; }
        self.monitor.record(metric);
    }

    /// Run meta-reflection: analyse performance patterns and
    /// propose heuristic adjustments.
    pub fn meta_reflect(&mut self) -> Vec<MetaReflection> {
        let mut new_reflections = Vec::new();

        // Check: are gaps trending upward? Suggest more aggressive bonding.
        let trend = self.monitor.gap_trend();
        if trend < -0.5 {
            new_reflections.push(MetaReflection {
                id: format!("mr_{}", self.reflections.len()),
                pattern_observed: format!(
                    "Gap count increasing: trend {:.2}. Average gaps: {:.1}",
                    trend, self.monitor.avg_gaps()
                ),
                current_heuristic: "Default bond creation threshold".to_string(),
                proposed_adjustment: "Lower the ingest similarity threshold from 0.92 to 0.85 \
                    to create more bonds during ingestion, filling gaps faster.".to_string(),
                confidence: (-trend).min(1.0),
                created_at: Utc::now(),
            });
        }

        // Check: are activations very low? Suggest reindexing.
        let avg_act = self.monitor.avg_activation();
        if avg_act < 3.0 && self.monitor.total_queries() > 20 {
            new_reflections.push(MetaReflection {
                id: format!("mr_{}", self.reflections.len() + new_reflections.len()),
                pattern_observed: format!(
                    "Average activation is only {:.1} nodes per query — very sparse.",
                    avg_act
                ),
                current_heuristic: "Bond index may be stale or disconnected".to_string(),
                proposed_adjustment: "Run /reindex to rebuild pattern bonds across the graph. \
                    This should dramatically increase activation density.".to_string(),
                confidence: 0.8,
                created_at: Utc::now(),
            });
        }

        self.reflections.extend(new_reflections.clone());
        new_reflections
    }

    /// Build status summary.
    pub fn status(&self) -> SelfImprovementStatus {
        SelfImprovementStatus {
            enabled: self.enabled,
            total_queries: self.monitor.total_queries(),
            avg_activation: self.monitor.avg_activation(),
            avg_gaps: self.monitor.avg_gaps(),
            gap_trend: self.monitor.gap_trend(),
            hot_domains: self.monitor.hot_domains(),
            reflections_count: self.reflections.len(),
            module_proposals_count: self.module_proposals.len(),
            recent_reflections: self.reflections.iter().rev().take(5).cloned().collect(),
        }
    }
}

// ━━━ Display Types ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SelfImprovementStatus {
    pub enabled: bool,
    pub total_queries: usize,
    pub avg_activation: f32,
    pub avg_gaps: f32,
    pub gap_trend: f32,    // positive = improving
    pub hot_domains: Vec<(String, usize)>,
    pub reflections_count: usize,
    pub module_proposals_count: usize,
    pub recent_reflections: Vec<MetaReflection>,
}
