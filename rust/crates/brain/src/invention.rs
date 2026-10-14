// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Cross-Domain Bridge Detection & Invention
//!
//! Detects creative moments where activation crosses between
//! distinct domain clusters. Bridge events feed into the variant
//! spawner in "invention mode" (novelty-weighted scoring).
//!
//! Also generates ProblemSpace nodes when curiosity signals
//! cluster in unmapped semantic regions.
//!
//! Created by John Dean Martin

use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use graph::network::{RecallResult, TraversalHop};

fn short_id() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("inv_{:08x}", (nanos & 0xFFFFFFFF) as u32)
}

// ━━━ Bridge Events ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A bridge event: activation crossed between two distinct
/// domain clusters, signaling a creative connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BridgeEvent {
    pub id: String,
    pub source_domain: String,
    pub source_hub_hex: String,
    pub target_domain: String,
    pub target_hub_hex: String,
    pub bridge_path: Vec<String>,  // hex chain from source to target
    pub activation_strength: f32,
    pub novelty_score: f32,
    pub detected_at: DateTime<Utc>,
}

/// Minimum bond count to be considered a "hub" node.
const HUB_THRESHOLD: usize = 20;
/// Minimum hop distance between hubs for a bridge to count.
const MIN_BRIDGE_HOPS: usize = 2;

/// Detect bridge events in a traversal path.
///
/// A bridge event occurs when the traversal path passes through
/// nodes in different domain clusters (identified by hub nodes).
///
/// `traversal`: the traversal path from a recall/activation.
/// `bond_counts`: per-node bond count for hub identification.
/// `node_labels`: hex→label map for domain inference.
pub fn detect_bridges(
    traversal: &[TraversalHop],
    bond_counts: &HashMap<String, usize>,
    node_labels: &HashMap<String, String>,
) -> Vec<BridgeEvent> {
    let mut bridges = Vec::new();

    // Identify hub nodes in the traversal
    let hubs: HashSet<&str> = traversal.iter()
        .flat_map(|hop| vec![hop.from.as_str(), hop.to.as_str()])
        .filter(|hex| bond_counts.get(*hex).copied().unwrap_or(0) >= HUB_THRESHOLD)
        .collect();

    if hubs.len() < 2 {
        return bridges; // Need at least 2 hubs for a bridge
    }

    // Infer domain from first word of label
    let domain_of = |hex: &str| -> String {
        node_labels.get(hex)
            .and_then(|l| l.split_whitespace().next())
            .map(|w| w.to_lowercase())
            .unwrap_or_else(|| "unknown".to_string())
    };

    // Find pairs of hubs from different domains connected by the traversal
    let hub_list: Vec<&str> = hubs.iter().copied().collect();
    for i in 0..hub_list.len() {
        for j in (i + 1)..hub_list.len() {
            let a = hub_list[i];
            let b = hub_list[j];
            let domain_a = domain_of(a);
            let domain_b = domain_of(b);

            if domain_a == domain_b { continue; } // same domain, not a bridge

            // Find the path between these hubs in the traversal
            let path = find_path_between(traversal, a, b);
            if path.len() < MIN_BRIDGE_HOPS { continue; }

            // Compute activation strength along the bridge
            let strength: f32 = traversal.iter()
                .filter(|hop| path.contains(&hop.from) || path.contains(&hop.to))
                .map(|hop| hop.strength)
                .sum::<f32>() / path.len() as f32;

            // Novelty: how rare is this domain combination?
            // Simple heuristic: longer bridges = more novel
            let novelty = (path.len() as f32 / 5.0).min(1.0);

            bridges.push(BridgeEvent {
                id: short_id(),
                source_domain: domain_a,
                source_hub_hex: a.to_string(),
                target_domain: domain_b,
                target_hub_hex: b.to_string(),
                bridge_path: path,
                activation_strength: strength,
                novelty_score: novelty,
                detected_at: Utc::now(),
            });
        }
    }

    // Sort by novelty descending
    bridges.sort_by(|a, b| b.novelty_score
        .partial_cmp(&a.novelty_score)
        .unwrap_or(std::cmp::Ordering::Equal));

    bridges
}

/// Find the shortest path between two nodes in a traversal.
fn find_path_between(traversal: &[TraversalHop], start: &str, end: &str) -> Vec<String> {
    // Simple: collect all nodes on hops that connect start and end
    let mut path = Vec::new();
    let mut found_start = false;

    for hop in traversal {
        if hop.from == start || hop.to == start {
            found_start = true;
        }
        if found_start {
            if !path.contains(&hop.from) { path.push(hop.from.clone()); }
            if !path.contains(&hop.to) { path.push(hop.to.clone()); }
        }
        if hop.from == end || hop.to == end {
            break;
        }
    }
    path
}

// ━━━ Invention Engine ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Tracks bridge events and generates invention proposals.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventionEngine {
    pub bridge_history: Vec<BridgeEvent>,
    pub enabled: bool,
    pub total_bridges_detected: u64,
    max_history: usize,
}

impl Default for InventionEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl InventionEngine {
    pub fn new() -> Self {
        Self {
            bridge_history: Vec::new(),
            enabled: true,
            total_bridges_detected: 0,
            max_history: 100,
        }
    }

    /// Analyse a recall result for bridge events.
    pub fn after_recall(
        &mut self,
        result: &RecallResult,
        bond_counts: &HashMap<String, usize>,
        node_labels: &HashMap<String, String>,
    ) {
        if !self.enabled { return; }

        let bridges = detect_bridges(
            &result.traversal_path,
            bond_counts,
            node_labels,
        );

        self.total_bridges_detected += bridges.len() as u64;
        self.bridge_history.extend(bridges);

        // Trim history
        if self.bridge_history.len() > self.max_history {
            self.bridge_history.drain(0..self.bridge_history.len() - self.max_history);
        }
    }

    /// Get recent bridge events.
    pub fn recent_bridges(&self, n: usize) -> Vec<&BridgeEvent> {
        self.bridge_history.iter().rev().take(n).collect()
    }

    /// Get bridge count between two specific domains.
    pub fn domain_bridge_count(&self, domain_a: &str, domain_b: &str) -> usize {
        self.bridge_history.iter().filter(|b| {
            (b.source_domain == domain_a && b.target_domain == domain_b)
            || (b.source_domain == domain_b && b.target_domain == domain_a)
        }).count()
    }

    /// Build status summary.
    pub fn status(&self) -> InventionStatus {
        // Count unique domain pairs
        let mut pairs: HashSet<(String, String)> = HashSet::new();
        for b in &self.bridge_history {
            let pair = if b.source_domain < b.target_domain {
                (b.source_domain.clone(), b.target_domain.clone())
            } else {
                (b.target_domain.clone(), b.source_domain.clone())
            };
            pairs.insert(pair);
        }

        let recent: Vec<BridgeSummary> = self.bridge_history
            .iter().rev().take(10)
            .map(BridgeSummary::from)
            .collect();

        InventionStatus {
            enabled: self.enabled,
            total_bridges: self.total_bridges_detected,
            unique_domain_pairs: pairs.len(),
            recent_bridges: recent,
        }
    }
}

// ━━━ Display Types ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InventionStatus {
    pub enabled: bool,
    pub total_bridges: u64,
    pub unique_domain_pairs: usize,
    pub recent_bridges: Vec<BridgeSummary>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BridgeSummary {
    pub id: String,
    pub source_domain: String,
    pub target_domain: String,
    pub bridge_length: usize,
    pub strength: f32,
    pub novelty: f32,
}

impl From<&BridgeEvent> for BridgeSummary {
    fn from(b: &BridgeEvent) -> Self {
        BridgeSummary {
            id: b.id.clone(),
            source_domain: b.source_domain.clone(),
            target_domain: b.target_domain.clone(),
            bridge_length: b.bridge_path.len(),
            strength: b.activation_strength,
            novelty: b.novelty_score,
        }
    }
}
