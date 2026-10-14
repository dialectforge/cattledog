// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Knowledge Compounding & Health
//!
//! Tracks the health of Lu's knowledge graph and drives compounding
//! growth through synthesis passes.
//!
//! - **Synthesis Pass**: detects knowledge triangles (A→B, B→C, missing A→C)
//!   and proposes the missing bond. Over time creates exponentially denser knowledge.
//! - **Knowledge Health Metrics**: dashboard of graph vitality.
//!
//! Created by John Dean Martin

use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

// ━━━ Health Metrics ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Snapshot of knowledge graph health.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct KnowledgeHealth {
    pub total_nodes: usize,
    pub active_nodes: usize,
    pub total_bonds: usize,
    pub active_bonds: usize,
    pub density: f32,           // bonds / (nodes * (nodes-1))
    pub orphan_count: usize,    // nodes with zero bonds
    pub scars: usize,
    pub domain_count: usize,    // estimated distinct domains
    pub avg_bonds_per_node: f32,
    pub max_hub_bonds: usize,    // most-connected node's bond count
    pub max_hub_label: String,
    pub coherence_estimate: f32, // avg bond weight across active bonds
    pub timestamp: DateTime<Utc>,
}

// ━━━ Synthesis Proposal ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A proposed missing bond detected by the synthesis pass.
/// A→B and B→C exist, but A→C does not.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SynthesisBond {
    pub node_a_hex: String,
    pub node_a_label: String,
    pub node_b_hex: String,   // the bridge node
    pub node_b_label: String,
    pub node_c_hex: String,
    pub node_c_label: String,
    pub inferred_weight: f32, // geometric mean of A→B and B→C weights
    pub inferred_label: String,
}

/// Result of a synthesis pass.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SynthesisReport {
    pub triangles_scanned: usize,
    pub missing_bonds_found: usize,
    pub proposals: Vec<SynthesisBond>,
}

// ━━━ Health Computation ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Compute health metrics from network stats + bond data.
/// `bond_counts`: per-node bond count map.
/// `bond_weights`: list of all active bond weights.
/// `stats`: network stats.
/// `scars`: scar count.
pub fn compute_health(
    stats: &graph::network::NetworkStats,
    bond_counts: &HashMap<String, usize>,
    bond_weights: &[f32],
    node_labels: &HashMap<String, String>,
    scars: usize,
) -> KnowledgeHealth {
    let n = stats.active_nodes;
    let b = stats.active_bonds;

    // Density: bonds / max possible edges (directed graph)
    let max_edges = if n > 1 { n * (n - 1) } else { 1 };
    let density = b as f32 / max_edges as f32;

    // Orphans: nodes with zero bonds
    let nodes_with_bonds: HashSet<&String> = bond_counts.keys().collect();
    let orphan_count = n.saturating_sub(nodes_with_bonds.len());

    // Avg bonds per node
    let avg_bonds = if n > 0 { b as f32 / n as f32 } else { 0.0 };

    // Max hub
    let (max_hub_hex, max_hub_bonds) = bond_counts
        .iter()
        .max_by_key(|(_, c)| *c)
        .map(|(h, c)| (h.clone(), *c))
        .unwrap_or_default();
    let max_hub_label = node_labels
        .get(&max_hub_hex)
        .cloned()
        .unwrap_or_else(|| max_hub_hex.clone());

    // Coherence estimate: average bond weight
    let coherence = if bond_weights.is_empty() {
        0.0
    } else {
        bond_weights.iter().sum::<f32>() / bond_weights.len() as f32
    };

    // Domain count: estimate from first-word clustering
    let domains: HashSet<String> = node_labels
        .values()
        .filter_map(|label| label.split_whitespace().next())
        .map(|w| w.to_lowercase())
        .collect();

    KnowledgeHealth {
        total_nodes: stats.total_nodes,
        active_nodes: n,
        total_bonds: stats.total_bonds,
        active_bonds: b,
        density,
        orphan_count,
        scars,
        domain_count: domains.len(),
        avg_bonds_per_node: avg_bonds,
        max_hub_bonds,
        max_hub_label,
        coherence_estimate: coherence,
        timestamp: Utc::now(),
    }
}

// ━━━ Synthesis Pass ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Maximum triangles to scan per pass (performance cap).
const MAX_TRIANGLES: usize = 10_000;
/// Maximum proposals per pass.
const MAX_PROPOSALS: usize = 50;

/// Run a synthesis pass: find knowledge triangles with missing edges.
///
/// `bonds`: list of (source_hex, target_hex, weight, label).
/// `existing_bonds`: set of "src→tgt" strings for fast lookup.
/// `node_labels`: hex→label map.
///
/// Returns proposals for missing bonds worth creating.
pub fn synthesis_pass(
    bonds: &[(String, String, f32, String)],
    existing_bonds: &HashSet<String>,
    node_labels: &HashMap<String, String>,
) -> SynthesisReport {
    // Build adjacency: node → [(neighbor, weight, label)]
    let mut adj: HashMap<&str, Vec<(&str, f32, &str)>> = HashMap::new();
    for (src, tgt, w, lbl) in bonds {
        adj.entry(src.as_str()).or_default().push((tgt.as_str(), *w, lbl.as_str()));
    }

    let mut proposals = Vec::new();
    let mut triangles_scanned: usize = 0;

    // For each node A, look at neighbors B, then B's neighbors C.
    // If A→C doesn't exist, propose it.
    'outer: for (a, a_neighbors) in &adj {
        for (b, ab_weight, _ab_label) in a_neighbors {
            if let Some(b_neighbors) = adj.get(b) {
                for (c, bc_weight, _bc_label) in b_neighbors {
                    triangles_scanned += 1;
                    if triangles_scanned > MAX_TRIANGLES { break 'outer; }

                    // Skip self-loops and existing bonds
                    if a == c { continue; }
                    let bond_key = format!("{}→{}", a, c);
                    if existing_bonds.contains(&bond_key) { continue; }

                    // Geometric mean of the two edge weights
                    let inferred = (ab_weight * bc_weight).sqrt();
                    // Only propose if inferred weight is meaningful
                    if inferred < 0.1 { continue; }

                    let a_label = node_labels.get(*a).cloned().unwrap_or_default();
                    let b_label = node_labels.get(*b).cloned().unwrap_or_default();
                    let c_label = node_labels.get(*c).cloned().unwrap_or_default();

                    let syn_label = format!("synthesis: via {}", b_label);
                    proposals.push(SynthesisBond {
                        node_a_hex: a.to_string(),
                        node_a_label: a_label,
                        node_b_hex: b.to_string(),
                        node_b_label: b_label,
                        node_c_hex: c.to_string(),
                        node_c_label: c_label,
                        inferred_weight: inferred,
                        inferred_label: syn_label,
                    });

                    if proposals.len() >= MAX_PROPOSALS { break 'outer; }
                }
            }
        }
    }

    // Sort by inferred weight descending (strongest first)
    proposals.sort_by(|a, b| b.inferred_weight
        .partial_cmp(&a.inferred_weight)
        .unwrap_or(std::cmp::Ordering::Equal));

    SynthesisReport {
        triangles_scanned,
        missing_bonds_found: proposals.len(),
        proposals,
    }
}
