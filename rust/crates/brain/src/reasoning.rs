// SPDX-License-Identifier: Apache-2.0
// Copyright (C) 2026 DialectForge LLC
//! # Reasoning Engine
//!
//! Built on DF core. Internal operations use DF compression.
//! Output is DF-wrapped.
//!
//! The reasoning engine takes the brain from recall-and-stitch to
//! actual thinking. It chains associations to derive new conclusions
//! through multi-hop inference.
//!
//! Three reasoning modes:
//! 1. Chain reasoning: A→B, B→C, therefore A→C
//! 2. Analogy detection: A:B :: C:D (shared bond patterns)
//! 3. Synthesis: combine multiple recall paths into a novel conclusion

use std::collections::{HashMap, HashSet};
use graph::network::{ActivatedNode, TraversalHop};

/// A single step in a reasoning chain.
#[derive(Clone, Debug)]
pub struct ReasoningStep {
    /// The knowledge atom at this step
    pub node_hex: String,
    pub label: String,
    /// The bond that connected this step to the previous
    pub bond_label: String,
    /// How confident we are in this link
    pub link_strength: f32,
}

/// A complete chain of reasoning from premise to conclusion.
#[derive(Clone, Debug)]
pub struct ReasoningChain {
    pub steps: Vec<ReasoningStep>,
    /// Overall confidence (product of link strengths, normalized)
    pub confidence: f32,
    /// Natural language synthesis of the chain
    pub synthesis: String,
}

/// The result of a reasoning operation.
#[derive(Clone, Debug)]
pub struct ReasoningResult {
    /// Direct recall answer (what the brain already does)
    pub direct_answer: String,
    /// Reasoning chains discovered
    pub chains: Vec<ReasoningChain>,
    /// Synthesized conclusion combining chains
    pub conclusion: String,
    /// How many hops of reasoning were performed
    pub reasoning_depth: usize,
}

/// Extract reasoning chains from traversal paths.
///
/// The brain already computes traversal hops during spreading
/// activation. The reasoning engine analyzes these hops to find
/// meaningful chains — paths where each step adds information
/// rather than just finding synonyms.
pub fn extract_chains(
    activated_nodes: &[ActivatedNode],
    traversal_path: &[TraversalHop],
    node_labels: &HashMap<String, String>,
    bond_labels: &HashMap<String, String>,
    max_chains: usize,
) -> Vec<ReasoningChain> {
    if traversal_path.is_empty() || activated_nodes.len() < 2 {
        return Vec::new();
    }

    // Build adjacency from traversal hops
    let mut adjacency: HashMap<String, Vec<(String, String, f32)>> = HashMap::new();
    for hop in traversal_path {
        adjacency.entry(hop.from.clone())
            .or_default()
            .push((hop.to.clone(), hop.bond.clone(), hop.strength));
    }

    // Find the top activated nodes as chain starting points
    let top_nodes: Vec<&ActivatedNode> = activated_nodes.iter()
        .take(5)
        .collect();

    let mut chains: Vec<ReasoningChain> = Vec::new();

    // For each top node, try to build a reasoning chain
    for start_node in &top_nodes {
        let mut visited: HashSet<String> = HashSet::new();
        visited.insert(start_node.seed_hex.clone());

        let mut current = start_node.seed_hex.clone();
        let mut steps: Vec<ReasoningStep> = Vec::new();

        // Add the starting step
        steps.push(ReasoningStep {
            node_hex: current.clone(),
            label: start_node.label.clone(),
            bond_label: String::from("(start)"),
            link_strength: 1.0,
        });

        // Follow the strongest unvisited neighbor, up to 4 hops
        for _ in 0..4 {
            let neighbors = match adjacency.get(&current) {
                Some(n) => n,
                None => break,
            };

            // Find strongest unvisited neighbor
            let best = neighbors.iter()
                .filter(|(to, _, _)| !visited.contains(to))
                .max_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

            match best {
                Some((to, bond_hex, strength)) => {
                    visited.insert(to.clone());
                    let label = node_labels.get(to)
                        .cloned()
                        .unwrap_or_else(|| String::from("(unknown)"));
                    let blabel = bond_labels.get(bond_hex)
                        .cloned()
                        .unwrap_or_else(|| String::from("related to"));

                    steps.push(ReasoningStep {
                        node_hex: to.clone(),
                        label,
                        bond_label: blabel,
                        link_strength: *strength,
                    });
                    current = to.clone();
                }
                None => break,
            }
        }

        // Only keep chains with 2+ steps (a single node isn't reasoning)
        if steps.len() >= 2 {
            let confidence = compute_chain_confidence(&steps);
            let synthesis = synthesize_chain(&steps);
            chains.push(ReasoningChain {
                steps,
                confidence,
                synthesis,
            });
        }
    }

    // Sort by confidence, take top N
    chains.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence)
        .unwrap_or(std::cmp::Ordering::Equal));
    chains.truncate(max_chains);
    chains
}

/// Compute overall confidence for a reasoning chain.
/// Longer chains = lower confidence (each hop introduces uncertainty).
/// Stronger bonds = higher confidence.
fn compute_chain_confidence(steps: &[ReasoningStep]) -> f32 {
    if steps.len() <= 1 {
        return 1.0;
    }

    // Start with 1.0 and decay with each hop
    let mut confidence = 1.0f32;
    let hop_decay = 0.75; // each hop reduces confidence by 25%

    for step in steps.iter().skip(1) {
        // Normalize link strength to 0-1 range
        let link_factor = (step.link_strength / 100.0).min(1.0).max(0.1);
        confidence *= hop_decay * link_factor;
    }

    // Normalize to a readable 0-100 scale
    (confidence * 100.0).min(100.0).max(0.0)
}

/// Synthesize a natural language description of a reasoning chain.
fn synthesize_chain(steps: &[ReasoningStep]) -> String {
    if steps.is_empty() {
        return String::new();
    }

    if steps.len() == 1 {
        return steps[0].label.clone();
    }

    // Build a "because" chain
    let mut parts: Vec<String> = Vec::new();
    for (i, step) in steps.iter().enumerate() {
        if i == 0 {
            parts.push(step.label.clone());
        } else {
            // Clean up the label — remove trailing periods for flow
            let label = step.label.trim_end_matches('.');
            parts.push(label.to_string());
        }
    }

    // Join with logical connectors
    if parts.len() == 2 {
        format!("{}, which relates to: {}", parts[0], parts[1])
    } else {
        let first = &parts[0];
        let middle: Vec<&str> = parts[1..parts.len()-1].iter()
            .map(|s| s.as_str())
            .collect();
        let last = &parts[parts.len()-1];
        format!("{} → {} → {}", first, middle.join(" → "), last)
    }
}

/// Detect if two nodes share an analogical relationship.
/// A:B :: C:D means the bond pattern between A-B resembles C-D.
pub fn detect_analogy(
    node_a: &str, node_b: &str,
    node_c: &str,
    bond_labels: &HashMap<String, String>,
    adjacency: &HashMap<String, Vec<(String, String)>>,
) -> Option<(String, f32)> {
    // Find the bond type between A and B
    let ab_bonds: Vec<&str> = adjacency.get(node_a)
        .map(|neighbors| {
            neighbors.iter()
                .filter(|(to, _)| to == node_b)
                .filter_map(|(_, bond)| bond_labels.get(bond).map(|s| s.as_str()))
                .collect()
        })
        .unwrap_or_default();

    if ab_bonds.is_empty() {
        return None;
    }

    // Find neighbors of C that share the same bond type
    let c_neighbors = adjacency.get(node_c)?;
    for (to, bond_hex) in c_neighbors {
        if let Some(blabel) = bond_labels.get(bond_hex) {
            for ab_bond in &ab_bonds {
                if blabel == ab_bond {
                    // Found D! C:D has the same bond type as A:B
                    return Some((to.clone(), 0.8));
                }
            }
        }
    }
    None
}

/// Perform full reasoning on a query.
/// This is the main entry point called by Brain::reason().
pub fn reason(
    direct_answer: &str,
    activated_nodes: &[ActivatedNode],
    traversal_path: &[TraversalHop],
    node_labels: &HashMap<String, String>,
    bond_labels: &HashMap<String, String>,
    depth: usize,
) -> ReasoningResult {
    let chains = extract_chains(
        activated_nodes,
        traversal_path,
        node_labels,
        bond_labels,
        3, // max 3 reasoning chains
    );

    // Build conclusion from chains
    let conclusion = if chains.is_empty() {
        direct_answer.to_string()
    } else {
        let mut parts: Vec<String> = Vec::new();
        parts.push(direct_answer.to_string());

        for chain in &chains {
            if chain.confidence > 5.0 && chain.steps.len() >= 2 {
                // Only add chains that are reasonably confident
                let last_step = &chain.steps[chain.steps.len() - 1];
                // Check the chain adds new info not in direct answer
                let direct_lower = direct_answer.to_lowercase();
                let last_lower = last_step.label.to_lowercase();
                let new_words: Vec<&str> = last_lower.split_whitespace()
                    .filter(|w| w.len() >= 4 && !direct_lower.contains(*w))
                    .collect();
                if new_words.len() >= 2 {
                    // This chain adds genuinely new information
                    parts.push(last_step.label.clone());
                }
            }
        }

        // Compose with proper sentence endings
        let mut composed = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i > 0 {
                let trimmed = composed.trim_end();
                if !trimmed.ends_with('.') && !trimmed.ends_with('!')
                    && !trimmed.ends_with('?') {
                    composed.push('.');
                }
                composed.push(' ');
            }
            composed.push_str(part);
        }
        composed
    };

    ReasoningResult {
        direct_answer: direct_answer.to_string(),
        chains,
        conclusion,
        reasoning_depth: depth,
    }
}
